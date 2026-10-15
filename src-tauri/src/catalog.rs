use crate::agents::RootKind;
use crate::lockfile::{Lock, LockEntry};
use crate::model::*;
use crate::scan::{read_meta, Found};
use chrono::{DateTime, Local, Utc};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::path::Path;

pub fn skill_id(canonical: &Path) -> String {
    let digest = Sha256::digest(canonical.to_string_lossy().as_bytes());
    hex::encode(&digest[..6])
}

pub fn build(found: Vec<Found>, lock: &Lock) -> Vec<Skill> {
    let mut groups: BTreeMap<String, Vec<Found>> = BTreeMap::new();
    for f in found {
        groups.entry(f.canonical.to_string_lossy().to_string()).or_default().push(f);
    }

    let mut skills: Vec<Skill> = groups
        .into_iter()
        .map(|(canonical, mut group)| {
            group.sort_by_key(|f| (f.is_symlink, f.path.clone()));
            let canonical_path = group[0].canonical.clone();
            let meta = read_meta(&canonical_path);
            let dir_names: Vec<String> = group
                .iter()
                .filter_map(|f| f.path.file_name())
                .map(|n| n.to_string_lossy().to_string())
                .collect();
            let dir_name = dir_names.first().cloned().unwrap_or_default();
            let name = meta.name.clone().unwrap_or_else(|| dir_name.clone());

            let lock_entry = dir_names
                .iter()
                .chain(std::iter::once(&name))
                .find_map(|k| lock.get(k));
            let source = lock_entry
                .map(source_from_lock)
                .or_else(|| plugin_source(&canonical_path));
            let (installed_at, updated_at) = match lock_entry {
                Some(e) if e.installed_at.is_some() => (e.installed_at, e.updated_at),
                _ => (fs_created(&canonical_path), fs_modified(&canonical_path)),
            };

            let installs = group
                .iter()
                .map(|f| Install {
                    agent: f.agent.to_string(),
                    path: f.path.to_string_lossy().to_string(),
                    kind: if f.is_symlink { InstallKind::Symlink } else { InstallKind::Dir },
                    plugin: f.kind == RootKind::PluginCache,
                })
                .collect();

            Skill {
                id: skill_id(&canonical_path),
                name,
                dir_name,
                description: meta.description,
                canonical_dir: canonical,
                installs,
                source,
                installed_at,
                updated_at,
                tags: Vec::new(),
                auto_tags: meta.tags,
                usage: Usage::default(),
            }
        })
        .collect();

    skills.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()).then(a.id.cmp(&b.id)));
    skills
}

fn source_from_lock(e: &LockEntry) -> Source {
    let kind = if e.source_type == "github" { SourceKind::Github } else { SourceKind::WellKnown };
    let author = e.source.split('/').next().unwrap_or(&e.source).to_string();
    let url = if e.source_url.is_empty() {
        format!("https://github.com/{}", e.source)
    } else {
        e.source_url.trim_end_matches(".git").to_string()
    };
    Source { repo: e.source.clone(), author, url, kind }
}

fn plugin_source(canonical: &Path) -> Option<Source> {
    let comps: Vec<String> = canonical
        .components()
        .map(|c| c.as_os_str().to_string_lossy().to_string())
        .collect();
    let i = comps.windows(2).position(|w| w[0] == "plugins" && w[1] == "cache")?;
    let org = comps.get(i + 2)?;
    let plugin = comps.get(i + 3)?;
    Some(Source {
        repo: format!("{org}/{plugin}"),
        author: org.clone(),
        url: String::new(),
        kind: SourceKind::Plugin,
    })
}

fn fs_created(p: &Path) -> Option<DateTime<Utc>> {
    let m = fs::metadata(p).ok()?;
    m.created().or_else(|_| m.modified()).ok().map(DateTime::<Utc>::from)
}

fn fs_modified(p: &Path) -> Option<DateTime<Utc>> {
    fs::metadata(p).ok()?.modified().ok().map(DateTime::<Utc>::from)
}

pub fn local_day(at: DateTime<Utc>) -> String {
    at.with_timezone(&Local).format("%Y-%m-%d").to_string()
}

struct Resolver {
    by_path: HashMap<String, usize>,
    by_name: HashMap<String, Vec<usize>>,
    home: String,
}

impl Resolver {
    fn new(skills: &[Skill], home: &Path) -> Self {
        let mut by_path = HashMap::new();
        let mut by_name: HashMap<String, Vec<usize>> = HashMap::new();
        for (i, s) in skills.iter().enumerate() {
            by_path.insert(s.canonical_dir.clone(), i);
            let mut names = vec![s.name.clone(), s.dir_name.clone()];
            for inst in &s.installs {
                by_path.insert(inst.path.clone(), i);
                if let Some(n) = Path::new(&inst.path).file_name() {
                    names.push(n.to_string_lossy().to_string());
                }
            }
            names.sort();
            names.dedup();
            for n in names {
                by_name.entry(n).or_default().push(i);
            }
        }
        Resolver { by_path, by_name, home: home.to_string_lossy().to_string() }
    }

    fn resolve(&self, skills: &[Skill], ev: &StoredEvent) -> Option<usize> {
        let dir = ev.skill_dir.replacen('~', &self.home, 1);
        let dir = dir.trim_end_matches('/');
        if let Some(i) = self.by_path.get(dir) {
            return Some(*i);
        }
        let base = Path::new(dir)
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .filter(|n| !dir.is_empty() && !n.is_empty())
            .unwrap_or_else(|| ev.skill_name.clone());
        let candidates = self.by_name.get(&base).or_else(|| self.by_name.get(&ev.skill_name))?;
        candidates
            .iter()
            .copied()
            .find(|&i| skills[i].source.as_ref().map(|s| s.kind != SourceKind::Plugin).unwrap_or(true))
            .or_else(|| candidates.first().copied())
    }
}

pub fn attach_usage(skills: &mut [Skill], events: &[StoredEvent], home: &Path) {
    let resolver = Resolver::new(skills, home);
    for ev in events {
        let Some(i) = resolver.resolve(skills, ev) else { continue };
        let u = &mut skills[i].usage;
        u.total += 1;
        *u.by_agent.entry(ev.agent.clone()).or_default() += 1;
        *u.daily.entry(local_day(ev.at)).or_default() += 1;
        if u.last_used.map(|l| l < ev.at).unwrap_or(true) {
            u.last_used = Some(ev.at);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agents::{AgentDef, Root, UsageSource};
    use crate::scan::scan;
    use chrono::TimeZone;

    static SHARED: [Root; 1] = [Root { rel: ".agents/skills", kind: RootKind::Skills }];
    static CURSOR: [Root; 1] = [Root { rel: ".cursor/skills", kind: RootKind::Skills }];
    static CURSOR_PLUGINS: [Root; 1] = [Root { rel: ".cursor/plugins/cache", kind: RootKind::PluginCache }];

    fn agents() -> [AgentDef; 3] {
        [
            AgentDef { id: "shared", label: "s", cli_name: None, roots: &SHARED, usage: UsageSource::None },
            AgentDef { id: "cursor", label: "c", cli_name: Some("cursor"), roots: &CURSOR, usage: UsageSource::Cursor },
            AgentDef { id: "cursor-plugins", label: "p", cli_name: None, roots: &CURSOR_PLUGINS, usage: UsageSource::Cursor },
        ]
    }

    fn write_skill(dir: &Path, name: &str) {
        fs::create_dir_all(dir).unwrap();
        fs::write(dir.join("SKILL.md"), format!("---\nname: {name}\ndescription: {name} desc\n---\n")).unwrap();
    }

    fn fixture() -> (tempfile::TempDir, Vec<Skill>) {
        let home = tempfile::tempdir().unwrap();
        let h = home.path();
        write_skill(&h.join(".agents/skills/tdd"), "tdd");
        write_skill(&h.join(".agents/skills/shadcn"), "shadcn");
        fs::create_dir_all(h.join(".cursor/skills")).unwrap();
        std::os::unix::fs::symlink(h.join(".agents/skills/tdd"), h.join(".cursor/skills/tdd")).unwrap();
        write_skill(&h.join(".cursor/plugins/cache/cursor-public/pstack/abc123/skills/poteto-mode"), "poteto-mode");

        let lock = crate::lockfile::parse(
            r#"{"version":3,"skills":{"tdd":{"source":"mattpocock/skills","sourceType":"github","sourceUrl":"https://github.com/mattpocock/skills.git","installedAt":"2026-08-16T03:27:34.945Z","updatedAt":"2026-08-17T03:27:34.945Z"}}}"#,
        );
        let skills = build(scan(h, &agents()), &lock);
        (home, skills)
    }

    #[test]
    fn symlinks_merge_into_one_skill_with_lock_source() {
        let (_home, skills) = fixture();
        assert_eq!(skills.len(), 3);
        let tdd = skills.iter().find(|s| s.name == "tdd").unwrap();
        assert_eq!(tdd.installs.len(), 2);
        assert_eq!(tdd.installs[0].kind, InstallKind::Dir);
        assert_eq!(tdd.installs[1].kind, InstallKind::Symlink);
        let src = tdd.source.as_ref().unwrap();
        assert_eq!((src.repo.as_str(), src.author.as_str()), ("mattpocock/skills", "mattpocock"));
        assert_eq!(src.url, "https://github.com/mattpocock/skills");
        assert_eq!(tdd.installed_at.unwrap().to_rfc3339(), "2026-08-16T03:27:34.945+00:00");
        let shadcn = skills.iter().find(|s| s.name == "shadcn").unwrap();
        assert!(shadcn.source.is_none());
        let plug = skills.iter().find(|s| s.name == "poteto-mode").unwrap();
        let ps = plug.source.as_ref().unwrap();
        assert_eq!((ps.kind, ps.repo.as_str()), (SourceKind::Plugin, "cursor-public/pstack"));
        assert!(plug.installs[0].plugin);
    }

    #[test]
    fn usage_resolves_by_path_then_name_and_counts_per_agent() {
        let (home, mut skills) = fixture();
        let h = home.path().to_string_lossy().to_string();
        let at = |d: u32| Utc.with_ymd_and_hms(2026, 10, d, 6, 0, 0).unwrap();
        let ev = |agent: &str, dir: &str, name: &str, d: u32| StoredEvent {
            agent: agent.into(),
            skill_dir: dir.into(),
            skill_name: name.into(),
            at: at(d),
        };
        let events = vec![
            ev("codex", &format!("{h}/.agents/skills/tdd"), "tdd", 5),
            ev("cursor", "~/.cursor/skills/tdd", "tdd", 6),
            ev("claude", "/Users/gone/.claude/skills/tdd", "tdd", 7),
            ev("cursor", "tdd", "tdd", 8),
            ev("codex", "/x/y/unknown-skill", "unknown-skill", 9),
        ];
        attach_usage(&mut skills, &events, home.path());
        let tdd = skills.iter().find(|s| s.name == "tdd").unwrap();
        assert_eq!(tdd.usage.total, 4);
        assert_eq!(tdd.usage.by_agent["cursor"], 2);
        assert_eq!(tdd.usage.last_used, Some(at(8)));
        assert_eq!(tdd.usage.daily.len(), 4);
        assert_eq!(skills.iter().map(|s| s.usage.total).sum::<u32>(), 4);
    }
}
