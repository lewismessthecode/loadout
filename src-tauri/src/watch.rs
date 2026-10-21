use crate::agents::AGENTS;
use crate::scan::skipped_dir_name;
use notify::event::ModifyKind;
use notify::{Config, Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, SyncSender};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter};

const SKILLS_CHANGED: &str = "skills-changed";
const QUIET: Duration = Duration::from_millis(500);

enum FsKind {
    Access,
    Create,
    Modify,
    Remove,
    Other,
}

struct FsNote {
    kind: FsKind,
    paths: Vec<PathBuf>,
    rescan: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum WatchDepth {
    ChildOnly,
    Recursive,
}

struct Debounce {
    deadline: Option<Instant>,
}

impl Debounce {
    fn poke(&mut self, now: Instant) {
        self.deadline = Some(now + QUIET);
    }

    fn take(&mut self, now: Instant) -> bool {
        match self.deadline {
            Some(deadline) if now >= deadline => {
                self.deadline = None;
                true
            }
            _ => false,
        }
    }
}

pub(crate) fn start(app: AppHandle, home: PathBuf) {
    std::thread::spawn(move || {
        let (tx, rx) = mpsc::sync_channel(1);
        let mut watcher = match watcher(home.clone(), tx) {
            Ok(watcher) => watcher,
            Err(err) => {
                eprintln!("catalog watch disabled: {err}");
                return;
            }
        };
        let mut applied = BTreeMap::new();
        apply(&home, &mut watcher, &mut applied, &desired_watches(&home));
        pump(app, home, &mut watcher, rx, &mut applied);
    });
}

fn watcher(home: PathBuf, tx: SyncSender<()>) -> notify::Result<RecommendedWatcher> {
    RecommendedWatcher::new(
        move |res: Result<Event, notify::Error>| {
            let Ok(event) = res else { return };
            let rescan = event.need_rescan();
            let note = to_fs_note(event.kind, event.paths, rescan);
            if note_is_relevant(&home, &note) {
                let _ = tx.try_send(());
            }
        },
        Config::default(),
    )
}

fn pump(
    app: AppHandle,
    home: PathBuf,
    watcher: &mut RecommendedWatcher,
    rx: Receiver<()>,
    applied: &mut BTreeMap<PathBuf, WatchDepth>,
) {
    let mut debounce = Debounce { deadline: None };
    loop {
        let now = Instant::now();
        let poked = if debounce.deadline.is_none() {
            match rx.recv() {
                Ok(()) => true,
                Err(_) => return,
            }
        } else if debounce.take(now) {
            false
        } else {
            let wait = debounce.deadline.unwrap().saturating_duration_since(now);
            match rx.recv_timeout(wait) {
                Ok(()) => true,
                Err(RecvTimeoutError::Timeout) => false,
                Err(RecvTimeoutError::Disconnected) => return,
            }
        };
        if poked {
            debounce.poke(Instant::now());
            continue;
        }
        debounce.deadline = None;
        apply(&home, watcher, applied, &desired_watches(&home));
        let _ = app.emit(SKILLS_CHANGED, ());
    }
}

fn to_fs_note(kind: EventKind, paths: Vec<PathBuf>, rescan: bool) -> FsNote {
    let (kind, any) = match kind {
        EventKind::Access(_) => (FsKind::Access, false),
        EventKind::Create(_) => (FsKind::Create, false),
        EventKind::Remove(_) => (FsKind::Remove, false),
        EventKind::Modify(ModifyKind::Name(_)) => (FsKind::Create, false),
        EventKind::Modify(ModifyKind::Data(_) | ModifyKind::Any) => (FsKind::Modify, false),
        EventKind::Modify(_) => (FsKind::Other, false),
        EventKind::Any => (FsKind::Other, true),
        EventKind::Other => (FsKind::Other, false),
    };
    FsNote {
        kind,
        paths,
        rescan: rescan || any,
    }
}

fn note_is_relevant(home: &Path, note: &FsNote) -> bool {
    if note.rescan {
        return true;
    }
    match note.kind {
        FsKind::Access | FsKind::Other => false,
        FsKind::Modify => note
            .paths
            .iter()
            .any(|path| skill_file_under_root(home, path)),
        FsKind::Create | FsKind::Remove => {
            note.paths.iter().any(|path| in_catalog_scope(home, path))
        }
    }
}

fn skill_file_under_root(home: &Path, path: &Path) -> bool {
    path.file_name().and_then(|n| n.to_str()) == Some("SKILL.md")
        && !path_skipped(path)
        && configured_roots(home)
            .iter()
            .any(|root| path.starts_with(root))
}

fn in_catalog_scope(home: &Path, path: &Path) -> bool {
    if path_skipped(path) || !path.starts_with(home) {
        return false;
    }
    configured_roots(home)
        .iter()
        .any(|root| path.starts_with(root) || (root.starts_with(path) && root != path))
}

fn path_skipped(path: &Path) -> bool {
    path.components()
        .any(|c| c.as_os_str().to_str().is_some_and(skipped_dir_name))
}

fn configured_roots(home: &Path) -> Vec<PathBuf> {
    AGENTS
        .iter()
        .flat_map(|agent| agent.roots.iter().map(|root| home.join(root.rel)))
        .collect()
}

fn watch_depth(home: &Path, path: &Path, depth: WatchDepth) -> WatchDepth {
    if path == home {
        WatchDepth::ChildOnly
    } else {
        depth
    }
}

fn desired_watches(home: &Path) -> BTreeMap<PathBuf, WatchDepth> {
    let mut watches = BTreeMap::new();
    for root in configured_roots(home) {
        if root.is_dir() {
            let depth = watch_depth(home, &root, WatchDepth::Recursive);
            watches.insert(root, depth);
            continue;
        }
        let ancestor = nearest_existing_ancestor(home, &root);
        if watches.get(&ancestor) != Some(&WatchDepth::Recursive) {
            let depth = watch_depth(home, &ancestor, WatchDepth::ChildOnly);
            watches.insert(ancestor, depth);
        }
    }
    watches
}

fn nearest_existing_ancestor(home: &Path, missing: &Path) -> PathBuf {
    let mut cursor = missing.to_path_buf();
    while let Some(parent) = cursor.parent() {
        if parent == home || parent.starts_with(home) {
            if parent.exists() {
                return parent.to_path_buf();
            }
            cursor = parent.to_path_buf();
            continue;
        }
        break;
    }
    home.to_path_buf()
}

fn apply(
    home: &Path,
    watcher: &mut RecommendedWatcher,
    applied: &mut BTreeMap<PathBuf, WatchDepth>,
    desired: &BTreeMap<PathBuf, WatchDepth>,
) {
    let stale: Vec<PathBuf> = applied
        .iter()
        .filter(|(path, depth)| desired.get(*path) != Some(*depth))
        .map(|(path, _)| path.clone())
        .collect();
    for path in stale {
        if watcher.unwatch(&path).is_ok() {
            applied.remove(&path);
        }
    }
    for (path, depth) in desired {
        let depth = watch_depth(home, path, *depth);
        if applied.get(path) == Some(&depth) || applied.contains_key(path) {
            continue;
        }
        let mode = match depth {
            WatchDepth::Recursive => RecursiveMode::Recursive,
            WatchDepth::ChildOnly => RecursiveMode::NonRecursive,
        };
        if watcher.watch(path, mode).is_ok() {
            applied.insert(path.clone(), depth);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use notify::event::{AccessKind, CreateKind, DataChange, MetadataKind, ModifyKind, RemoveKind};
    use std::time::Duration;

    fn note(kind: EventKind, path: PathBuf) -> FsNote {
        to_fs_note(kind, vec![path], false)
    }

    #[test]
    fn debounce_collapses_two_pokes() {
        let start = Instant::now();
        let mut debounce = Debounce { deadline: None };
        debounce.poke(start);
        debounce.poke(start + Duration::from_millis(100));
        assert!(!debounce.take(start + Duration::from_millis(400)));
        assert!(debounce.take(start + QUIET + Duration::from_millis(100)));
        assert!(!debounce.take(start + QUIET + Duration::from_millis(100)));
    }

    #[test]
    fn missing_root_watches_ancestor_not_home_recursively() {
        let home = tempfile::tempdir().unwrap();
        let skills = home.path().join(".agents/skills");
        std::fs::create_dir_all(&skills).unwrap();
        let first = desired_watches(home.path());
        let second = desired_watches(home.path());
        assert_eq!(first, second);
        assert_eq!(first.get(home.path()), Some(&WatchDepth::ChildOnly));
        assert_eq!(first.get(&skills), Some(&WatchDepth::Recursive));
    }

    #[test]
    fn relevance_ignores_unrelated_home_creates_and_keeps_skill_files() {
        let home = tempfile::tempdir().unwrap();
        let desktop = home.path().join("Desktop/note.txt");
        assert!(!note_is_relevant(
            home.path(),
            &note(EventKind::Create(CreateKind::Any), desktop)
        ));

        let skill = home.path().join(".agents/skills/tdd/SKILL.md");
        assert!(note_is_relevant(
            home.path(),
            &note(EventKind::Modify(ModifyKind::Data(DataChange::Any)), skill)
        ));

        let readme = home.path().join(".agents/skills/tdd/README.md");
        assert!(!note_is_relevant(
            home.path(),
            &note(EventKind::Modify(ModifyKind::Data(DataChange::Any)), readme)
        ));

        let modules = home.path().join(".cursor/plugins/cache/node_modules/pkg");
        assert!(!note_is_relevant(
            home.path(),
            &note(EventKind::Create(CreateKind::Any), modules)
        ));

        let removed = home.path().join(".codex/skills/old");
        assert!(note_is_relevant(
            home.path(),
            &note(EventKind::Remove(RemoveKind::Any), removed)
        ));

        assert!(!note_is_relevant(
            home.path(),
            &note(
                EventKind::Access(AccessKind::Any),
                home.path().join(".agents/skills/tdd/SKILL.md")
            ),
        ));
        assert!(!note_is_relevant(
            home.path(),
            &note(
                EventKind::Modify(ModifyKind::Metadata(MetadataKind::Any)),
                home.path().join(".agents/skills/tdd/SKILL.md"),
            ),
        ));

        let rescan = to_fs_note(EventKind::Any, Vec::new(), false);
        assert!(note_is_relevant(home.path(), &rescan));
        let rename = to_fs_note(
            EventKind::Modify(ModifyKind::Name(notify::event::RenameMode::Any)),
            vec![home.path().join(".gemini")],
            false,
        );
        assert_eq!(matches!(rename.kind, FsKind::Create), true);
        assert!(note_is_relevant(home.path(), &rename));
    }
}
