use crate::agents::{AGENTS, AgentInfo};
use crate::catalog;
use crate::lockfile;
use crate::model::*;
use crate::scan;
use crate::usage::index::{Index, discover};
use serde::Serialize;
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

pub fn lock_path(home: &Path) -> PathBuf {
    crate::lockfile::global_path(home)
}

pub fn load(home: &Path, idx: &mut Index, refresh: bool) -> Result<Vec<Skill>, String> {
    if refresh {
        idx.refresh(&discover(home)).map_err(|e| e.to_string())?;
    }
    let lock = lockfile::read(&lock_path(home));
    let mut skills = catalog::build(scan::scan(home, AGENTS), &lock);
    let mut tags = idx.all_tags().map_err(|e| e.to_string())?;
    for s in &mut skills {
        s.tags = tags.remove(&s.id).unwrap_or_default();
    }
    let events = idx.events().map_err(|e| e.to_string())?;
    catalog::attach_usage(&mut skills, &events, home);
    Ok(skills)
}

pub fn agents(home: &Path) -> Vec<AgentInfo> {
    AGENTS
        .iter()
        .map(|a| AgentInfo {
            id: a.id,
            label: a.label,
            detected: crate::agents::detected(home, a),
            installable: a.cli_name.is_some(),
            usage_supported: a.usage != crate::agents::UsageSource::None,
            roots: a
                .roots
                .iter()
                .map(|r| crate::agents::ScanRoot {
                    path: home.join(r.rel).to_string_lossy().into_owned(),
                    available: home.join(r.rel).is_dir(),
                    kind: r.kind,
                })
                .collect(),
        })
        .collect()
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillDetail {
    pub skill: Skill,
    pub skill_md: String,
    pub files: Vec<String>,
}

const MAX_MD_BYTES: usize = 256 * 1024;

pub fn detail(skills: Vec<Skill>, id: &str) -> Result<SkillDetail, String> {
    let skill = skills
        .into_iter()
        .find(|s| s.id == id)
        .ok_or("skill not found")?;
    let dir = Path::new(&skill.canonical_dir);
    let mut md = fs::read_to_string(dir.join("SKILL.md")).unwrap_or_default();
    if md.len() > MAX_MD_BYTES {
        let mut cut = MAX_MD_BYTES;
        while !md.is_char_boundary(cut) {
            cut -= 1;
        }
        md.truncate(cut);
    }
    let mut files = Vec::new();
    list_files(dir, dir, 3, &mut files);
    files.sort();
    Ok(SkillDetail {
        skill,
        skill_md: md,
        files,
    })
}

fn list_files(root: &Path, dir: &Path, depth: usize, out: &mut Vec<String>) {
    let Ok(read) = fs::read_dir(dir) else { return };
    for e in read.flatten() {
        if out.len() >= 200 {
            return;
        }
        let name = e.file_name().to_string_lossy().to_string();
        if name.starts_with('.') || name == "node_modules" {
            continue;
        }
        let Ok(ft) = e.file_type() else { continue };
        let path = e.path();
        if ft.is_dir() {
            if depth > 0 {
                list_files(root, &path, depth - 1, out);
            }
        } else if let Ok(rel) = path.strip_prefix(root) {
            out.push(rel.to_string_lossy().to_string());
        }
    }
}

#[derive(Serialize, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DayCount {
    pub day: String,
    pub count: u32,
}

pub fn heatmap(skills: &[Skill], id: Option<&str>, days: u32) -> Vec<DayCount> {
    let cutoff = (chrono::Local::now() - chrono::Duration::days(days as i64))
        .format("%Y-%m-%d")
        .to_string();
    let mut sum: BTreeMap<&str, u32> = BTreeMap::new();
    for s in skills
        .iter()
        .filter(|s| id.map(|i| i == s.id).unwrap_or(true))
    {
        for (day, n) in &s.usage.daily {
            if day.as_str() >= cutoff.as_str() {
                *sum.entry(day).or_default() += n;
            }
        }
    }
    sum.into_iter()
        .map(|(d, c)| DayCount {
            day: d.to_string(),
            count: c,
        })
        .collect()
}

pub fn plan_removal(skill: &Skill, selected: Option<&[String]>) -> Result<Vec<String>, String> {
    let mut chosen: Vec<&Install> = match selected {
        None => skill.installs.iter().collect(),
        Some(paths) => {
            let mut v = Vec::new();
            for p in paths {
                v.push(
                    skill
                        .installs
                        .iter()
                        .find(|i| &i.path == p)
                        .ok_or_else(|| format!("not an install of this skill: {p}"))?,
                );
            }
            v
        }
    };
    if let Some(i) = chosen.iter().find(|i| i.plugin) {
        return Err(format!(
            "{} is managed by a plugin; uninstall the plugin from its agent instead",
            i.path
        ));
    }
    if chosen.iter().any(|i| i.kind == InstallKind::Dir) {
        chosen = skill.installs.iter().collect();
        if let Some(i) = chosen.iter().find(|i| i.plugin) {
            return Err(format!(
                "{} is managed by a plugin; uninstall the plugin from its agent instead",
                i.path
            ));
        }
    }
    let mut paths: Vec<String> = chosen.iter().map(|i| i.path.clone()).collect();
    paths.sort();
    paths.dedup();
    if paths.is_empty() {
        return Err("nothing to remove".into());
    }
    Ok(paths)
}

pub fn move_to_trash(path: &str) -> Result<(), String> {
    let mut ctx = trash::TrashContext::default();
    #[cfg(target_os = "macos")]
    {
        use trash::macos::{DeleteMethod, TrashContextExtMacos};
        ctx.set_delete_method(DeleteMethod::NsFileManager);
    }
    ctx.delete(path).map_err(|e| e.to_string())
}

#[derive(Serialize, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RemoveReport {
    pub removed: Vec<String>,
    pub lock_entries_removed: usize,
}

pub fn execute_removal(
    home: &Path,
    skill: &Skill,
    plan: Vec<String>,
    trash: impl Fn(&str) -> Result<(), String>,
) -> Result<RemoveReport, String> {
    for p in &plan {
        trash(p).map_err(|e| format!("{p}: {e}"))?;
    }
    let keys = lock_keys_to_drop(skill, skill.installs.len() - plan.len());
    let lock_entries_removed = if keys.is_empty() {
        0
    } else {
        lockfile::remove_entries(&lock_path(home), &keys).unwrap_or(0)
    };
    Ok(RemoveReport {
        removed: plan,
        lock_entries_removed,
    })
}

pub fn lock_keys_to_drop(skill: &Skill, remaining_installs: usize) -> Vec<String> {
    if remaining_installs > 0 {
        return Vec::new();
    }
    let mut keys = vec![skill.dir_name.clone(), skill.name.clone()];
    keys.sort();
    keys.dedup();
    keys
}

#[cfg(test)]
mod tests {
    use super::*;

    fn skill(installs: Vec<(&str, InstallKind, bool)>) -> Skill {
        Skill {
            id: "id".into(),
            name: "tdd".into(),
            dir_name: "tdd".into(),
            description: String::new(),
            canonical_dir: "/h/.agents/skills/tdd".into(),
            installs: installs
                .into_iter()
                .map(|(p, k, plugin)| Install {
                    agent: "a".into(),
                    path: p.into(),
                    kind: k,
                    plugin,
                })
                .collect(),
            source: None,
            installed_at: None,
            updated_at: None,
            tags: vec![],
            auto_tags: vec![],
            usage: Usage::default(),
        }
    }

    #[test]
    fn removing_only_a_symlink_keeps_the_real_dir() {
        let s = skill(vec![
            ("/h/.agents/skills/tdd", InstallKind::Dir, false),
            ("/h/.cursor/skills/tdd", InstallKind::Symlink, false),
        ]);
        let p = plan_removal(&s, Some(&["/h/.cursor/skills/tdd".to_string()])).unwrap();
        assert_eq!(p, vec!["/h/.cursor/skills/tdd"]);
    }

    #[test]
    fn removing_the_real_dir_takes_dangling_symlinks_too() {
        let s = skill(vec![
            ("/h/.agents/skills/tdd", InstallKind::Dir, false),
            ("/h/.cursor/skills/tdd", InstallKind::Symlink, false),
        ]);
        let p = plan_removal(&s, Some(&["/h/.agents/skills/tdd".to_string()])).unwrap();
        assert_eq!(p.len(), 2);
    }

    #[test]
    fn rejects_foreign_paths_and_plugin_installs() {
        let s = skill(vec![("/h/.agents/skills/tdd", InstallKind::Dir, false)]);
        assert!(plan_removal(&s, Some(&["/etc".to_string()])).is_err());
        let p = skill(vec![(
            "/h/.cursor/plugins/cache/o/p/x/skills/tdd",
            InstallKind::Dir,
            true,
        )]);
        assert!(plan_removal(&p, None).is_err());
    }

    #[test]
    fn full_removal_drops_lock_entry_and_partial_keeps_it() {
        let home = tempfile::tempdir().unwrap();
        let lock = lock_path(home.path());
        std::fs::create_dir_all(lock.parent().unwrap()).unwrap();
        std::fs::write(&lock, r#"{"version":3,"skills":{"tdd":{"source":"a/b","sourceType":"github","installedAt":"2026-01-01T00:00:00Z"},"other":{"source":"a/c","sourceType":"github"}}}"#).unwrap();
        let s = skill(vec![
            ("/h/.agents/skills/tdd", InstallKind::Dir, false),
            ("/h/.cursor/skills/tdd", InstallKind::Symlink, false),
        ]);
        let trashed = std::cell::RefCell::new(Vec::new());
        let t = |p: &str| {
            trashed.borrow_mut().push(p.to_string());
            Ok(())
        };

        let r = execute_removal(home.path(), &s, vec!["/h/.cursor/skills/tdd".into()], &t).unwrap();
        assert_eq!(r.lock_entries_removed, 0);
        assert_eq!(lockfile::read(&lock).len(), 2);

        let r = execute_removal(
            home.path(),
            &s,
            vec![
                "/h/.agents/skills/tdd".into(),
                "/h/.cursor/skills/tdd".into(),
            ],
            &t,
        )
        .unwrap();
        assert_eq!(r.lock_entries_removed, 1);
        assert_eq!(
            lockfile::read(&lock).keys().collect::<Vec<_>>(),
            vec!["other"]
        );
        assert_eq!(trashed.borrow().len(), 3);
    }

    #[test]
    fn trash_failure_aborts_before_touching_the_lockfile() {
        let home = tempfile::tempdir().unwrap();
        let s = skill(vec![("/h/a", InstallKind::Dir, false)]);
        let e = execute_removal(home.path(), &s, vec!["/h/a".into()], |_| {
            Err("denied".into())
        })
        .unwrap_err();
        assert!(e.contains("denied"));
    }

    #[test]
    fn lock_entries_dropped_only_when_nothing_remains() {
        let s = skill(vec![]);
        assert!(lock_keys_to_drop(&s, 1).is_empty());
        assert_eq!(lock_keys_to_drop(&s, 0), vec!["tdd"]);
    }
}

#[cfg(test)]
mod real_home {
    use super::*;

    #[test]
    #[ignore]
    fn smoke_against_real_home() {
        let home = dirs::home_dir().unwrap();
        let db = tempfile::tempdir().unwrap();
        let mut idx = Index::open(&db.path().join("t.db")).unwrap();
        let t = std::time::Instant::now();
        let skills = load(&home, &mut idx, true).unwrap();
        eprintln!("loaded {} skills in {:?}", skills.len(), t.elapsed());
        let t = std::time::Instant::now();
        let again = load(&home, &mut idx, true).unwrap();
        eprintln!("second load {} in {:?}", again.len(), t.elapsed());
        let used = skills.iter().filter(|s| s.usage.total > 0).count();
        eprintln!("skills with usage: {used}");
        let mut by_total: Vec<_> = skills.iter().map(|s| (s.usage.total, &s.name)).collect();
        by_total.sort_by(|a, b| b.cmp(a));
        eprintln!("top: {:?}", &by_total[..10.min(by_total.len())]);
        let multi = skills.iter().filter(|s| s.installs.len() > 1).count();
        eprintln!(
            "multi-install: {multi}, unknown source: {}",
            skills.iter().filter(|s| s.source.is_none()).count()
        );
        for a in agents(&home) {
            let n = skills
                .iter()
                .filter(|s| s.installs.iter().any(|i| i.agent == a.id))
                .count();
            eprintln!("agent {} detected={} skills={}", a.id, a.detected, n);
        }
        if let Some(d) = skills.iter().find(|s| s.dir_name == "diagnosing-bugs") {
            eprintln!("diagnosing-bugs daily: {:?}", d.usage.daily);
        }
    }
}

#[cfg(test)]
mod dump {
    use super::*;

    #[test]
    #[ignore]
    fn dump_real_home_json() {
        let home = dirs::home_dir().unwrap();
        let db = dirs::data_dir().unwrap().join("Loadout/loadout.db");
        let mut idx = Index::open(&db).unwrap();
        let skills = load(&home, &mut idx, true).unwrap();
        let heat = heatmap(&skills, None, 365);
        let out = serde_json::json!({ "skills": skills, "agents": agents(&home), "heat": heat });
        std::fs::write(
            "/tmp/loadout_dump.json",
            serde_json::to_string(&out).unwrap(),
        )
        .unwrap();
    }
}

#[cfg(test)]
mod e2e_fake_home {
    use super::*;

    #[test]
    #[ignore]
    fn install_then_remove_with_real_trash() {
        let home = PathBuf::from(std::env::var("E2E_HOME").unwrap());
        let db = tempfile::tempdir().unwrap();
        let mut idx = Index::open(&db.path().join("t.db")).unwrap();
        let trash = move_to_trash;

        let skills = load(&home, &mut idx, false).unwrap();
        assert_eq!(skills.len(), 1, "{skills:#?}");
        let s = &skills[0];
        assert_eq!(s.name, "web-design-guidelines");
        assert_eq!(s.installs.len(), 2);
        assert_eq!(s.source.as_ref().unwrap().repo, "vercel-labs/agent-skills");

        let link = s
            .installs
            .iter()
            .find(|i| i.kind == InstallKind::Symlink)
            .unwrap()
            .path
            .clone();
        let plan = plan_removal(s, Some(&[link.clone()])).unwrap();
        let r = execute_removal(&home, s, plan, trash).unwrap();
        assert_eq!((r.removed.len(), r.lock_entries_removed), (1, 0));
        assert!(!Path::new(&link).exists() && std::fs::symlink_metadata(&link).is_err());
        assert!(
            home.join(".agents/skills/web-design-guidelines/SKILL.md")
                .exists()
        );

        let skills = load(&home, &mut idx, false).unwrap();
        let plan = plan_removal(&skills[0], None).unwrap();
        let r = execute_removal(&home, &skills[0], plan, trash).unwrap();
        assert_eq!((r.removed.len(), r.lock_entries_removed), (1, 1));
        assert!(load(&home, &mut idx, false).unwrap().is_empty());
        assert!(lockfile::read(&lock_path(&home)).is_empty());
    }
}
