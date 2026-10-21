use crate::{
    agents::AGENTS,
    catalog, lockfile,
    model::{InstallKind, Skill},
    scan,
};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::{
    collections::{BTreeMap, HashMap},
    fs,
    io::{Read, Write},
    path::{Component, Path, PathBuf},
    process::{Command, Stdio},
    sync::{
        Mutex,
        atomic::{AtomicU64, Ordering},
    },
    time::{Duration, Instant},
};

type Result<T> = std::result::Result<T, String>;
const STALE: &str = "Installation changed. Check for updates again.";
static NEXT: AtomicU64 = AtomicU64::new(1);
fn nonce() -> String {
    format!(
        "{}-{}-{}",
        std::process::id(),
        Utc::now().timestamp_nanos_opt().unwrap_or_default(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    )
}

#[derive(Serialize, Clone, Debug)]
#[serde(
    tag = "status",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum UpdateState {
    Unchecked,
    Unsupported {
        reason: String,
    },
    Current {
        checked_at: String,
        revision: String,
    },
    Available {
        plan_id: String,
        checked_at: String,
        old_revision: String,
        new_revision: String,
        affected_agents: Vec<String>,
    },
    Failed {
        message: String,
    },
}
#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct UpdateInspection {
    pub skill_id: String,
    pub state: UpdateState,
}
#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct UpdateOutcome {
    pub backup_path: String,
    pub updated_at: String,
}

#[derive(Clone)]
struct Candidate {
    home: PathBuf,
    skill: Skill,
    key: String,
    entry: Value,
    repo: String,
    reference: Option<String>,
    skill_path: String,
    old_revision: String,
    target_fingerprint: String,
    placement_fingerprint: String,
    _lock_bytes: Vec<u8>,
    lock_path: PathBuf,
}
struct Plan {
    candidate: Candidate,
    commit: String,
    folder_sha: String,
    created: Instant,
}
#[derive(Default)]
pub struct Updates {
    plans: Mutex<HashMap<String, Plan>>,
}

fn ineligible(reason: String) -> UpdateState {
    match reason.as_str() {
        "pluginManaged" | "untracked" | "legacyLock" | "unsupportedSource"
        | "externalDirectory" | "independentCopy" | "ambiguousLock" => {
            UpdateState::Unsupported { reason }
        }
        _ => UpdateState::Failed { message: reason },
    }
}
fn local_catalog(home: &Path) -> Vec<Skill> {
    let canonical = fs::canonicalize(home).unwrap_or_else(|_| home.to_path_buf());
    let home = canonical.as_path();
    catalog::build(
        scan::scan(home, AGENTS),
        &lockfile::read(&lockfile::global_path(home)),
    )
}
fn strict_lock(path: &Path) -> Result<(Vec<u8>, Value)> {
    let bytes = fs::read(path).map_err(|e| format!("Cannot read tracking lock: {e}"))?;
    let value: Value =
        serde_json::from_slice(&bytes).map_err(|_| "Tracking lock is invalid JSON".to_string())?;
    if value.get("version").and_then(Value::as_u64) != Some(3) {
        return Err("legacyLock".into());
    }
    if !value.get("skills").is_some_and(Value::is_object) {
        return Err("Tracking lock has no skills object".into());
    }
    Ok((bytes, value))
}
fn text<'a>(value: &'a Value, key: &str) -> Result<&'a str> {
    value
        .get(key)
        .and_then(Value::as_str)
        .filter(|v| !v.is_empty())
        .ok_or_else(|| format!("Tracking record has no {key}"))
}
fn sha(value: &str) -> bool {
    value.len() == 40 && value.bytes().all(|b| b.is_ascii_hexdigit())
}
fn repo_valid(repo: &str) -> bool {
    let v: Vec<_> = repo.split('/').collect();
    v.len() == 2
        && v.iter().all(|s| {
            !s.is_empty()
                && *s != "."
                && *s != ".."
                && s.bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b))
        })
}
fn safe_relative(path: &str) -> bool {
    !path.is_empty()
        && !path.contains('\\')
        && Path::new(path)
            .components()
            .all(|c| matches!(c, Component::Normal(_)))
}
fn candidate(home: &Path, skill: &Skill, all: &[Skill]) -> Result<Candidate> {
    if skill.installs.iter().any(|i| i.plugin) {
        return Err("pluginManaged".into());
    }
    let target = Path::new(&skill.canonical_dir);
    let root = fs::canonicalize(home.join(".agents/skills")).map_err(|_| "externalDirectory")?;
    if root
        != fs::canonicalize(home)
            .map_err(|_| "externalDirectory")?
            .join(".agents/skills")
        || target.parent() != Some(root.as_path())
    {
        return Err("externalDirectory".into());
    }
    let meta = fs::symlink_metadata(target).map_err(|_| "Installation directory is missing")?;
    if !meta.is_dir() || meta.file_type().is_symlink() {
        return Err("externalDirectory".into());
    }
    let key = target
        .file_name()
        .and_then(|v| v.to_str())
        .ok_or("Invalid installation name")?
        .to_string();
    if all.iter().any(|s| {
        s.id != skill.id
            && !s.installs.iter().any(|i| i.plugin)
            && (s.dir_name == key || s.name == key)
    }) {
        return Err("ambiguousLock".into());
    }
    for i in &skill.installs {
        let p = Path::new(&i.path);
        if i.kind == InstallKind::Dir && p != target {
            return Err("independentCopy".into());
        }
        if fs::canonicalize(p).ok().as_deref() != Some(target) {
            return Err("Installation links changed".into());
        }
    }
    let lock_path = lockfile::global_path(home);
    if !lock_path.exists() {
        return Err("untracked".into());
    }
    let (lock_bytes, lock) = strict_lock(&lock_path)?;
    let entry = lock["skills"]
        .get(&key)
        .filter(|v| v.is_object())
        .ok_or("untracked")?
        .clone();
    if text(&entry, "sourceType")? != "github" {
        return Err("unsupportedSource".into());
    }
    let repo = text(&entry, "source")?.to_string();
    if !repo_valid(&repo) {
        return Err("unsupportedSource".into());
    }
    let source_url = text(&entry, "sourceUrl")?;
    if source_url.trim_end_matches(".git") != format!("https://github.com/{repo}") {
        return Err("unsupportedSource".into());
    }
    let skill_path = text(&entry, "skillPath")
        .map_err(|_| "legacyLock")?
        .to_string();
    if !safe_relative(&skill_path)
        || Path::new(&skill_path).file_name().and_then(|v| v.to_str()) != Some("SKILL.md")
    {
        return Err("legacyLock".into());
    }
    let old_revision = text(&entry, "skillFolderHash")
        .map_err(|_| "legacyLock")?
        .to_string();
    if !sha(&old_revision) {
        return Err("legacyLock".into());
    }
    let reference = match entry.get("ref") {
        None | Some(Value::Null) => None,
        Some(Value::String(v)) if !v.is_empty() && !v.chars().any(char::is_control) => {
            Some(v.clone())
        }
        _ => return Err("legacyLock".into()),
    };
    Ok(Candidate {
        home: fs::canonicalize(home).map_err(|e| e.to_string())?,
        skill: skill.clone(),
        key,
        entry,
        repo,
        reference,
        skill_path,
        old_revision,
        target_fingerprint: fingerprint(target)?,
        placement_fingerprint: placements(skill)?,
        _lock_bytes: lock_bytes,
        lock_path,
    })
}
fn placements(skill: &Skill) -> Result<String> {
    let mut data = Vec::new();
    for i in &skill.installs {
        let p = Path::new(&i.path);
        let m = fs::symlink_metadata(p).map_err(|_| STALE.to_string())?;
        data.push(format!(
            "{}:{}:{}:{}:{}:{:?}",
            i.path,
            m.dev(),
            m.ino(),
            m.mode(),
            m.file_type().is_symlink(),
            fs::read_link(p).ok()
        ));
    }
    data.sort();
    Ok(data.join("\n"))
}
fn fingerprint(path: &Path) -> Result<String> {
    fn walk(root: &Path, path: &Path, h: &mut Sha256) -> Result<()> {
        let m = fs::symlink_metadata(path).map_err(|e| e.to_string())?;
        h.update(
            path.strip_prefix(root)
                .unwrap()
                .as_os_str()
                .as_encoded_bytes(),
        );
        h.update(m.mode().to_le_bytes());
        h.update(m.dev().to_le_bytes());
        h.update(m.ino().to_le_bytes());
        if m.file_type().is_symlink() {
            h.update(b"link");
            h.update(
                fs::read_link(path)
                    .map_err(|e| e.to_string())?
                    .as_os_str()
                    .as_encoded_bytes(),
            );
        } else if m.is_dir() {
            h.update(b"dir");
            let mut entries = fs::read_dir(path)
                .map_err(|e| e.to_string())?
                .collect::<std::io::Result<Vec<_>>>()
                .map_err(|e| e.to_string())?;
            entries.sort_by_key(|e| e.file_name());
            for e in entries {
                walk(root, &e.path(), h)?;
            }
        } else if m.is_file() {
            h.update(b"file");
            let mut f = fs::File::open(path).map_err(|e| e.to_string())?;
            let mut buf = [0u8; 65536];
            loop {
                let n = f.read(&mut buf).map_err(|e| e.to_string())?;
                if n == 0 {
                    break;
                }
                h.update(&buf[..n]);
            }
        } else {
            return Err("Special files cannot be updated safely".into());
        }
        Ok(())
    }
    let mut h = Sha256::new();
    walk(path, path, &mut h)?;
    Ok(hex::encode(h.finalize()))
}

trait Remote {
    fn get(&self, path: &str) -> Result<Value>;
}
struct Github;
impl Remote for Github {
    fn get(&self, path: &str) -> Result<Value> {
        let output = tempfile::NamedTempFile::new().map_err(|e| e.to_string())?;
        let mut cmd = Command::new("/usr/bin/curl");
        cmd.args([
            "-q",
            "--silent",
            "--show-error",
            "--fail",
            "--proto",
            "=https",
            "--connect-timeout",
            "10",
            "--max-time",
            "30",
            "--max-filesize",
            "16777216",
            "--config",
            "-",
            "-o",
        ])
        .arg(output.path())
        .arg(format!("https://api.github.com{path}"))
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
        let mut child = cmd
            .spawn()
            .map_err(|_| "Cannot start the GitHub request".to_string())?;
        let mut config =
            "header = \"Accept: application/vnd.github+json\"\nheader = \"User-Agent: Loadout\"\n"
                .to_string();
        if let Some(token) = std::env::var("GITHUB_TOKEN")
            .ok()
            .or_else(|| std::env::var("GH_TOKEN").ok())
            .filter(|s| {
                s.bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b"_-".contains(&b))
            })
        {
            config.push_str(&format!("header = \"Authorization: Bearer {token}\"\n"));
        }
        child
            .stdin
            .take()
            .unwrap()
            .write_all(config.as_bytes())
            .map_err(|_| "Cannot configure GitHub request")?;
        if !child
            .wait()
            .map_err(|_| "GitHub request interrupted")?
            .success()
        {
            return Err(
                "GitHub request failed. Check your connection, repository access, or API limit."
                    .into(),
            );
        }
        if output
            .as_file()
            .metadata()
            .map_err(|e| e.to_string())?
            .len()
            > 16777216
        {
            return Err("GitHub response exceeded the size limit".into());
        }
        serde_json::from_reader(output.reopen().map_err(|e| e.to_string())?)
            .map_err(|_| "GitHub returned invalid metadata".into())
    }
}
fn encode(v: &str) -> String {
    v.bytes()
        .map(|b| {
            if b.is_ascii_alphanumeric() || b"-._~".contains(&b) {
                (b as char).to_string()
            } else {
                format!("%{b:02X}")
            }
        })
        .collect()
}
struct RemoteTree {
    commit: String,
    root_sha: String,
    entries: HashMap<String, (String, String)>,
}
fn remote_tree(remote: &impl Remote, repo: &str, reference: Option<&str>) -> Result<RemoteTree> {
    let reference = match reference {
        Some(v) => v.to_string(),
        None => text(&remote.get(&format!("/repos/{repo}"))?, "default_branch")?.to_string(),
    };
    let commit = remote.get(&format!("/repos/{repo}/commits/{}", encode(&reference)))?;
    let commit_sha = text(&commit, "sha")?.to_string();
    let tree_sha = text(&commit["commit"]["tree"], "sha")?.to_string();
    if !sha(&commit_sha) || !sha(&tree_sha) {
        return Err("GitHub returned invalid object IDs".into());
    }
    let tree = remote.get(&format!("/repos/{repo}/git/trees/{tree_sha}?recursive=1"))?;
    if tree.get("truncated").and_then(Value::as_bool) != Some(false) {
        return Err("GitHub returned an incomplete repository tree".into());
    }
    let mut entries = HashMap::new();
    for e in tree
        .get("tree")
        .and_then(Value::as_array)
        .ok_or("GitHub returned no repository tree")?
    {
        let (p, t, s) = (text(e, "path")?, text(e, "type")?, text(e, "sha")?);
        if !sha(s) {
            return Err("GitHub returned an invalid tree object".into());
        }
        entries.insert(p.to_string(), (t.to_string(), s.to_string()));
    }
    Ok(RemoteTree {
        commit: commit_sha,
        root_sha: tree_sha,
        entries,
    })
}
impl Updates {
    pub fn inspect(&self, home: &Path) -> Vec<UpdateInspection> {
        let skills = local_catalog(home);
        skills
            .iter()
            .map(|s| UpdateInspection {
                skill_id: s.id.clone(),
                state: match candidate(home, s, &skills) {
                    Ok(_) => UpdateState::Unchecked,
                    Err(reason) => ineligible(reason),
                },
            })
            .collect()
    }
    pub fn check(&self, home: &Path, ids: &[String]) -> Result<Vec<UpdateInspection>> {
        self.check_with(home, ids, &Github)
    }
    fn check_with(
        &self,
        home: &Path,
        ids: &[String],
        remote: &impl Remote,
    ) -> Result<Vec<UpdateInspection>> {
        let skills = local_catalog(home);
        self.plans
            .lock()
            .map_err(|e| e.to_string())?
            .retain(|_, p| !ids.contains(&p.candidate.skill.id));
        let mut candidates = Vec::new();
        let mut results = Vec::new();
        for id in ids {
            let Some(skill) = skills.iter().find(|s| &s.id == id) else {
                results.push(UpdateInspection {
                    skill_id: id.clone(),
                    state: UpdateState::Failed {
                        message: "Installation no longer exists".into(),
                    },
                });
                continue;
            };
            match candidate(home, skill, &skills) {
                Ok(c) => candidates.push(c),
                Err(reason) => results.push(UpdateInspection {
                    skill_id: id.clone(),
                    state: ineligible(reason),
                }),
            }
        }
        let mut groups: BTreeMap<(String, Option<String>), Vec<Candidate>> = BTreeMap::new();
        for c in candidates {
            groups
                .entry((c.repo.clone(), c.reference.clone()))
                .or_default()
                .push(c);
        }
        for ((repo, reference), candidates) in groups {
            let tree = remote_tree(remote, &repo, reference.as_deref());
            for c in candidates {
                let state = match &tree {
                    Err(message) => UpdateState::Failed {
                        message: message.clone(),
                    },
                    Ok(tree) => {
                        let parent = Path::new(&c.skill_path).parent().unwrap().to_string_lossy();
                        let folder_sha = if parent.is_empty() {
                            Some(tree.root_sha.clone())
                        } else {
                            tree.entries
                                .get(parent.as_ref())
                                .filter(|(t, _)| t == "tree")
                                .map(|(_, s)| s.clone())
                        };
                        match folder_sha.filter(|_| {
                            tree.entries
                                .get(&c.skill_path)
                                .is_some_and(|(t, _)| t == "blob")
                        }) {
                            None => UpdateState::Failed {
                                message: "The tracked skill path is missing upstream".into(),
                            },
                            Some(folder_sha) if folder_sha == c.old_revision => {
                                UpdateState::Current {
                                    checked_at: Utc::now().to_rfc3339(),
                                    revision: folder_sha,
                                }
                            }
                            Some(folder_sha) => {
                                let id = nonce();
                                let agents =
                                    c.skill.installs.iter().map(|i| i.agent.clone()).collect();
                                let state = UpdateState::Available {
                                    plan_id: id.clone(),
                                    checked_at: Utc::now().to_rfc3339(),
                                    old_revision: c.old_revision.clone(),
                                    new_revision: folder_sha.clone(),
                                    affected_agents: agents,
                                };
                                let mut plans = self.plans.lock().map_err(|e| e.to_string())?;
                                plans.retain(|_, p| {
                                    p.created.elapsed() < Duration::from_secs(1800)
                                        && p.candidate.skill.id != c.skill.id
                                });
                                plans.insert(
                                    id,
                                    Plan {
                                        candidate: c.clone(),
                                        commit: tree.commit.clone(),
                                        folder_sha,
                                        created: Instant::now(),
                                    },
                                );
                                state
                            }
                        }
                    }
                };
                results.push(UpdateInspection {
                    skill_id: c.skill.id.clone(),
                    state,
                });
            }
        }
        Ok(results)
    }
    pub fn apply(&self, home: &Path, id: &str) -> Result<UpdateOutcome> {
        let plan = self
            .plans
            .lock()
            .map_err(|e| e.to_string())?
            .remove(id)
            .ok_or("Update plan expired. Check for updates again.")?;
        if plan.created.elapsed() > Duration::from_secs(1800) {
            return Err("Update plan expired. Check for updates again.".into());
        }
        self.apply_with(home, plan, materialize)
    }
    fn apply_with(
        &self,
        home: &Path,
        plan: Plan,
        materializer: impl FnOnce(&Plan, &Path) -> Result<PathBuf>,
    ) -> Result<UpdateOutcome> {
        recover(home)?;
        validate(&plan.candidate)?;
        let root = Path::new(&plan.candidate.skill.canonical_dir)
            .parent()
            .unwrap();
        let temp = tempfile::Builder::new()
            .prefix(".loadout-stage-")
            .tempdir_in(root)
            .map_err(|e| e.to_string())?;
        let staged = materializer(&plan, temp.path())?;
        if !staged.starts_with(temp.path())
            || fs::canonicalize(&staged).map_err(|e| e.to_string())? != staged
        {
            return Err("Staged skill escaped its isolated directory".into());
        }
        validate_stage_files(&staged)?;
        let staged_fingerprint = fingerprint(&staged)?;
        validate(&plan.candidate)?;
        if fingerprint(&staged)? != staged_fingerprint {
            return Err("Staged skill changed before publication".into());
        }
        publish(home, &plan, &staged)
    }
}
fn validate(c: &Candidate) -> Result<()> {
    let current = local_catalog(&c.home);
    if current
        .iter()
        .find(|s| s.id == c.skill.id)
        .is_none_or(|s| s.installs != c.skill.installs)
    {
        return Err(STALE.into());
    }
    if strict_lock(&c.lock_path)
        .ok()
        .is_none_or(|(_, lock)| lock["skills"].get(&c.key) != Some(&c.entry))
        || fingerprint(Path::new(&c.skill.canonical_dir)).ok().as_ref()
            != Some(&c.target_fingerprint)
        || placements(&c.skill).ok().as_ref() != Some(&c.placement_fingerprint)
    {
        return Err(STALE.into());
    }
    Ok(())
}
fn validate_stage_files(path: &Path) -> Result<()> {
    let m = fs::symlink_metadata(path).map_err(|e| e.to_string())?;
    if m.file_type().is_symlink() || (!m.is_file() && !m.is_dir()) {
        return Err("Staged skill contains unsupported links or special files".into());
    }
    if m.is_dir() {
        for e in fs::read_dir(path).map_err(|e| e.to_string())? {
            validate_stage_files(&e.map_err(|e| e.to_string())?.path())?;
        }
    }
    Ok(())
}
pub(crate) fn executable(name: &str) -> Result<PathBuf> {
    if let Some(path) = std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default())
        .map(|p| p.join(name))
        .find(|p| fs::metadata(p).is_ok_and(|m| m.is_file() && m.permissions().mode() & 0o111 != 0))
    {
        return Ok(path);
    }
    if name == "npx" {
        let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/zsh".into());
        let output = Command::new(shell)
            .args(["-l", "-i", "-c", "command -v npx"])
            .stdin(Stdio::null())
            .stderr(Stdio::null())
            .output()
            .map_err(|_| "Cannot locate Node.js")?;
        for line in String::from_utf8_lossy(&output.stdout).lines().rev() {
            let path = PathBuf::from(line.trim());
            if path.is_absolute()
                && fs::metadata(&path)
                    .is_ok_and(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
            {
                return Ok(path);
            }
        }
    }
    Err(format!(
        "{name} is unavailable. Install Node.js to update skills."
    ))
}
fn node_path(npx: &Path) -> std::ffi::OsString {
    let mut paths = vec![npx.parent().unwrap().to_path_buf()];
    paths.extend(std::env::split_paths(
        &std::env::var_os("PATH").unwrap_or_default(),
    ));
    std::env::join_paths(paths).unwrap_or_default()
}

fn materializer_command(npx: &Path, plan: &Plan, isolated: &Path) -> Command {
    let mut cmd = Command::new(npx);
    let folder = Path::new(&plan.candidate.skill_path)
        .parent()
        .unwrap()
        .to_string_lossy();
    let source = if folder.is_empty() {
        plan.candidate.repo.clone()
    } else {
        format!("{}/{}", plan.candidate.repo, folder)
    };
    cmd.args(["-y", "skills@1.7.4", "add"])
        .arg(format!("{source}#{}", plan.commit))
        .args([
            "--skill",
            &plan.candidate.key,
            "-g",
            "-y",
            "-a",
            "codex",
            "--json",
        ])
        .current_dir(isolated)
        .env("PATH", node_path(npx))
        .env("HOME", isolated)
        .env("XDG_STATE_HOME", isolated.join("state"))
        .env("XDG_DATA_HOME", isolated.join("data"))
        .env("XDG_CONFIG_HOME", isolated.join("config"))
        .env("XDG_CACHE_HOME", isolated.join("cache"))
        .env("npm_config_cache", isolated.join("npm-cache"))
        .env("NPM_CONFIG_CACHE", isolated.join("npm-cache"))
        .env("npm_config_userconfig", isolated.join("npmrc"))
        .env("NPM_CONFIG_USERCONFIG", isolated.join("npmrc"))
        .env("CODEX_HOME", isolated.join(".codex"))
        .env("SKILLS_NO_TELEMETRY", "1")
        .env("DO_NOT_TRACK", "1")
        .env("CI", "1")
        .env("NO_COLOR", "1")
        .env_remove("LOADOUT_HOME")
        .env_remove("NODE_OPTIONS")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    cmd
}
fn installer_json(bytes: &[u8]) -> Result<Value> {
    // Some npm launchers print notices before the CLI's final JSON array.
    for (index, byte) in bytes.iter().enumerate() {
        if *byte == b'[' {
            if let Ok(value) = serde_json::from_slice::<Value>(&bytes[index..]) {
                if value.is_array() {
                    return Ok(value);
                }
            }
        }
    }
    Err("Installer returned invalid JSON".into())
}

fn materialize(plan: &Plan, isolated: &Path) -> Result<PathBuf> {
    materialize_with(&executable("npx")?, plan, isolated)
}
fn materialize_with(npx: &Path, plan: &Plan, isolated: &Path) -> Result<PathBuf> {
    let stdout = tempfile::NamedTempFile::new_in(isolated).map_err(|e| e.to_string())?;
    let mut command = materializer_command(npx, plan, isolated);
    use std::os::unix::process::CommandExt;
    command
        .process_group(0)
        .stdout(stdout.reopen().map_err(|e| e.to_string())?);
    let mut child = command
        .spawn()
        .map_err(|_| "Cannot start isolated skills installer")?;
    let started = Instant::now();
    let status = loop {
        if let Some(status) = child
            .try_wait()
            .map_err(|_| "Isolated installer wait failed")?
        {
            break status;
        }
        if started.elapsed() > Duration::from_secs(180)
            || stdout
                .as_file()
                .metadata()
                .map_err(|e| e.to_string())?
                .len()
                > 2097152
        {
            let _ = Command::new("/bin/kill")
                .args(["-TERM", "--", &format!("-{}", child.id())])
                .status();
            let _ = child.kill();
            let _ = child.wait();
            return Err("Isolated installer timed out or exceeded the output limit. Existing files were preserved.".into());
        }
        std::thread::sleep(Duration::from_millis(100));
    };
    if !status.success() {
        return Err(
            "Isolated skills installation failed. Your existing files were preserved.".into(),
        );
    }
    let bytes = fs::read(stdout.path()).map_err(|e| e.to_string())?;
    let results = installer_json(&bytes)?;
    let rows = results
        .as_array()
        .filter(|v| v.len() == 1)
        .ok_or("Installer returned an unexpected skill count")?;
    let row = &rows[0];
    let staged = isolated.join(".agents/skills").join(&plan.candidate.key);
    if text(row, "status")? != "installed"
        || text(row, "name")? != plan.candidate.key
        || text(row, "source")? != plan.candidate.repo
        || text(row, "ref")? != plan.commit
        || text(row, "scope")? != "global"
        || Path::new(text(row, "path")?) != staged
        || !row
            .get("hash")
            .and_then(Value::as_str)
            .is_some_and(|s| s.len() == 64 && s.bytes().all(|b| b.is_ascii_hexdigit()))
    {
        return Err("Installer result did not match the reviewed skill".into());
    }
    let (_, lock) = strict_lock(&isolated.join("state/skills/.skill-lock.json"))?;
    let entries = lock["skills"].as_object().unwrap();
    if entries.len() != 1 {
        return Err("Installer created unexpected tracking entries".into());
    }
    let roots = fs::read_dir(isolated.join(".agents/skills"))
        .map_err(|e| e.to_string())?
        .collect::<std::io::Result<Vec<_>>>()
        .map_err(|e| e.to_string())?;
    if roots.len() != 1 || roots[0].file_name() != plan.candidate.key.as_str() {
        return Err("Installer created unexpected canonical directories".into());
    }
    if row.get("agents") != Some(&json!(["Codex"])) {
        return Err("Installer used unexpected agent targets".into());
    }
    let entry = entries
        .get(&plan.candidate.key)
        .ok_or("Installer did not write a tracking entry")?;
    if text(entry, "sourceType")? != "github"
        || text(entry, "source")? != plan.candidate.repo
        || text(entry, "sourceUrl")?.trim_end_matches(".git")
            != format!("https://github.com/{}", plan.candidate.repo)
        || text(entry, "skillPath")? != plan.candidate.skill_path
        || text(entry, "ref")? != plan.commit
        || text(entry, "skillFolderHash")? != plan.folder_sha
    {
        return Err(
            "Staged source, path, or upstream hash did not match the reviewed update".into(),
        );
    }
    if !staged.join("SKILL.md").is_file() {
        return Err("Installer produced no SKILL.md".into());
    }
    Ok(staged)
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Transaction {
    target: PathBuf,
    backup: PathBuf,
    staged: PathBuf,
    lock_path: PathBuf,
    old_lock: Vec<u8>,
    new_lock: Vec<u8>,
    old_fingerprint: String,
    new_fingerprint: String,
    phase: Phase,
}
#[derive(Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
enum Phase {
    Prepared,
    BackedUp,
    Promoted,
    Committed,
    Restored,
}
fn transaction_dir(home: &Path) -> PathBuf {
    home.join(".agents/loadout-transactions")
}
fn atomic_write(path: &Path, bytes: &[u8]) -> Result<()> {
    let mut temp = tempfile::NamedTempFile::new_in(path.parent().ok_or("No parent directory")?)
        .map_err(|e| e.to_string())?;
    if let Ok(m) = fs::metadata(path) {
        temp.as_file()
            .set_permissions(m.permissions())
            .map_err(|e| e.to_string())?;
    }
    temp.write_all(bytes).map_err(|e| e.to_string())?;
    temp.as_file().sync_all().map_err(|e| e.to_string())?;
    temp.persist(path).map_err(|e| e.to_string())?;
    fs::File::open(path.parent().unwrap())
        .and_then(|f| f.sync_all())
        .map_err(|e| e.to_string())?;
    Ok(())
}
fn save(path: &Path, tx: &Transaction) -> Result<()> {
    atomic_write(
        path,
        &serde_json::to_vec_pretty(tx).map_err(|e| e.to_string())?,
    )
}
fn sync_parent(path: &Path) -> Result<()> {
    fs::File::open(path.parent().ok_or("No parent directory")?)
        .and_then(|f| f.sync_all())
        .map_err(|e| e.to_string())
}
fn publish(home: &Path, plan: &Plan, staged: &Path) -> Result<UpdateOutcome> {
    let c = &plan.candidate;
    validate(c)?;
    let (old_lock, mut lock) = strict_lock(&c.lock_path)?;
    if lock["skills"].get(&c.key) != Some(&c.entry) {
        return Err(STALE.into());
    }
    let mut entry = c.entry.clone();
    let updated_at = Utc::now().to_rfc3339();
    entry["skillFolderHash"] = json!(plan.folder_sha);
    entry["updatedAt"] = json!(updated_at);
    lock["skills"][&c.key] = entry;
    let mut new_lock = serde_json::to_vec_pretty(&lock).map_err(|e| e.to_string())?;
    new_lock.push(b'\n');
    let target = PathBuf::from(&c.skill.canonical_dir);
    let backup = target.with_file_name(format!(".loadout-backup-{}-{}", c.key, nonce()));
    let dir = transaction_dir(home);
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let manifest = dir.join(format!("{}.json", nonce()));
    let mut tx = Transaction {
        target,
        backup,
        staged: staged.to_path_buf(),
        lock_path: c.lock_path.clone(),
        old_lock,
        new_lock,
        old_fingerprint: c.target_fingerprint.clone(),
        new_fingerprint: fingerprint(staged)?,
        phase: Phase::Prepared,
    };
    save(&manifest, &tx)?;
    let result = (|| {
        validate(c)?;
        fs::rename(&tx.target, &tx.backup).map_err(|e| e.to_string())?;
        sync_parent(&tx.target)?;
        tx.phase = Phase::BackedUp;
        save(&manifest, &tx)?;
        fs::rename(&tx.staged, &tx.target).map_err(|e| e.to_string())?;
        sync_parent(&tx.target)?;
        tx.phase = Phase::Promoted;
        save(&manifest, &tx)?;
        if fs::read(&tx.lock_path).ok().as_ref() != Some(&tx.old_lock) {
            return Err("Tracking lock changed during publication".to_string());
        }
        atomic_write(&tx.lock_path, &tx.new_lock)?;
        tx.phase = Phase::Committed;
        save(&manifest, &tx)?;
        Ok(())
    })();
    if let Err(e) = result {
        return match restore(&mut tx, &manifest) {
            Ok(()) => Err(format!("{e}. Previous installation restored.")),
            Err(recovery) => Err(format!(
                "{e}. Recovery needs attention: {recovery}. Backup: {}. Manifest: {}",
                tx.backup.display(),
                manifest.display()
            )),
        };
    }
    Ok(UpdateOutcome {
        backup_path: tx.backup.to_string_lossy().into_owned(),
        updated_at,
    })
}
fn restore(tx: &mut Transaction, manifest: &Path) -> Result<()> {
    if tx.backup.exists() {
        if fingerprint(&tx.backup)? != tx.old_fingerprint {
            return Err("Backup changed outside Loadout".into());
        }
        if tx.target.exists() {
            if fingerprint(&tx.target)? != tx.new_fingerprint {
                return Err("Published directory changed outside Loadout".into());
            }
            let displaced = tx
                .target
                .with_file_name(format!(".loadout-recovery-{}", nonce()));
            fs::rename(&tx.target, displaced).map_err(|e| e.to_string())?;
        }
        fs::rename(&tx.backup, &tx.target).map_err(|e| e.to_string())?;
        sync_parent(&tx.target)?;
    } else if fingerprint(&tx.target).ok().as_ref() != Some(&tx.old_fingerprint) {
        return Err("Original installation cannot be located".into());
    }
    let latest_lock = fs::read(&tx.lock_path).map_err(|e| e.to_string())?;
    if latest_lock == tx.new_lock {
        atomic_write(&tx.lock_path, &tx.old_lock)?;
    }
    if latest_lock != tx.old_lock && latest_lock != tx.new_lock {
        return Err("External lock changes were preserved; inspect the recovery manifest".into());
    }
    tx.phase = Phase::Restored;
    save(manifest, tx)
}
pub fn recover(home: &Path) -> Result<()> {
    let dir = transaction_dir(home);
    if !dir.exists() {
        return Ok(());
    }
    for e in fs::read_dir(&dir).map_err(|e| e.to_string())? {
        let path = e.map_err(|e| e.to_string())?.path();
        if path.extension().and_then(|v| v.to_str()) != Some("json") {
            continue;
        }
        let mut tx: Transaction =
            serde_json::from_slice(&fs::read(&path).map_err(|e| e.to_string())?)
                .map_err(|_| format!("Unreadable recovery manifest {}", path.display()))?;
        if tx.phase == Phase::Committed || tx.phase == Phase::Restored {
            continue;
        }
        if tx.target.parent()
            != fs::canonicalize(home.join(".agents/skills"))
                .ok()
                .as_deref()
            || tx.backup.parent() != tx.target.parent()
            || tx.lock_path != lockfile::global_path(home)
        {
            return Err("Unsafe recovery manifest paths".into());
        }
        restore(&mut tx, &path)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    const OLD: &str = "1111111111111111111111111111111111111111";
    const NEW: &str = "2222222222222222222222222222222222222222";
    const COMMIT: &str = "3333333333333333333333333333333333333333";
    const ROOT: &str = "4444444444444444444444444444444444444444";
    struct Fixture {
        home: tempfile::TempDir,
        updates: Updates,
        id: String,
    }
    fn write_skill(path: &Path, body: &str) {
        fs::create_dir_all(path).unwrap();
        fs::write(
            path.join("SKILL.md"),
            format!("---\nname: example\n---\n{body}"),
        )
        .unwrap();
    }
    fn fixture() -> Fixture {
        let home = tempfile::tempdir().unwrap();
        write_skill(&home.path().join(".agents/skills/example"), "old");
        fs::write(
            home.path().join(".agents/skills/example/local.txt"),
            "local customization",
        )
        .unwrap();
        fs::create_dir_all(home.path().join(".codex/skills")).unwrap();
        std::os::unix::fs::symlink(
            home.path().join(".agents/skills/example"),
            home.path().join(".codex/skills/example"),
        )
        .unwrap();
        fs::write(lockfile::global_path(home.path()), serde_json::to_vec(&json!({"version":3,"customTop":true,"skills":{"example":{"source":"owner/repo","sourceType":"github","sourceUrl":"https://github.com/owner/repo.git","ref":"release","skillPath":"skills/example/SKILL.md","skillFolderHash":OLD,"installedAt":"2025-01-01T00:00:00Z","updatedAt":"2025-01-01T00:00:00Z","unknown":{"preserve":1}}},"dismissed":{"x":true}})).unwrap()).unwrap();
        let id = local_catalog(home.path())[0].id.clone();
        Fixture {
            home,
            updates: Updates::default(),
            id,
        }
    }
    struct FakeRemote {
        hash: String,
        truncated: bool,
        missing: bool,
        fail: bool,
        calls: RefCell<Vec<String>>,
    }
    impl FakeRemote {
        fn new(hash: &str) -> Self {
            Self {
                hash: hash.into(),
                truncated: false,
                missing: false,
                fail: false,
                calls: RefCell::new(vec![]),
            }
        }
    }
    impl Remote for FakeRemote {
        fn get(&self, path: &str) -> Result<Value> {
            self.calls.borrow_mut().push(path.into());
            if self.fail {
                return Err("network unavailable".into());
            }
            if path.contains("/commits/") {
                return Ok(json!({"sha":COMMIT,"commit":{"tree":{"sha":ROOT}}}));
            }
            if path.contains("/git/trees/") {
                return Ok(
                    json!({"truncated":self.truncated,"tree": if self.missing { json!([]) } else { json!([{"path":"skills/example","type":"tree","sha":self.hash},{"path":"skills/example/SKILL.md","type":"blob","sha":OLD}]) }}),
                );
            }
            Ok(json!({"default_branch":"main"}))
        }
    }
    fn checked(f: &Fixture) -> Plan {
        let result = f
            .updates
            .check_with(
                f.home.path(),
                std::slice::from_ref(&f.id),
                &FakeRemote::new(NEW),
            )
            .unwrap();
        let UpdateState::Available { plan_id, .. } = &result[0].state else {
            panic!("expected available")
        };
        f.updates.plans.lock().unwrap().remove(plan_id).unwrap()
    }
    fn stage(_plan: &Plan, isolated: &Path) -> Result<PathBuf> {
        let path = isolated.join(".agents/skills/example");
        write_skill(&path, "new");
        Ok(path)
    }

    #[test]
    fn inspect_is_local_and_checks_current_available_without_writes() {
        let f = fixture();
        let before = fingerprint(f.home.path()).unwrap();
        assert!(matches!(
            f.updates.inspect(f.home.path())[0].state,
            UpdateState::Unchecked
        ));
        let current = f
            .updates
            .check_with(
                f.home.path(),
                std::slice::from_ref(&f.id),
                &FakeRemote::new(OLD),
            )
            .unwrap();
        assert!(matches!(current[0].state, UpdateState::Current { .. }));
        let plan = checked(&f);
        assert_eq!(plan.commit, COMMIT);
        assert_eq!(plan.folder_sha, NEW);
        assert_eq!(before, fingerprint(f.home.path()).unwrap());
    }
    #[test]
    fn checks_group_same_repo_ref_and_fail_on_incomplete_metadata() {
        let f = fixture();
        let remote = FakeRemote::new(NEW);
        f.updates
            .check_with(f.home.path(), &[f.id.clone(), f.id.clone()], &remote)
            .unwrap();
        assert_eq!(remote.calls.borrow().len(), 2);
        for mode in 0..3 {
            let mut remote = FakeRemote::new(NEW);
            if mode == 0 {
                remote.truncated = true;
            } else if mode == 1 {
                remote.missing = true;
            } else {
                remote.fail = true;
            }
            assert!(matches!(
                f.updates
                    .check_with(f.home.path(), std::slice::from_ref(&f.id), &remote)
                    .unwrap()[0]
                    .state,
                UpdateState::Failed { .. }
            ));
        }
    }
    #[test]
    fn local_file_selected_lock_and_placement_changes_make_plan_stale() {
        for mode in 0..3 {
            let f = fixture();
            let plan = checked(&f);
            if mode == 0 {
                fs::write(
                    f.home.path().join(".agents/skills/example/local.txt"),
                    "edited",
                )
                .unwrap();
            } else if mode == 1 {
                let p = lockfile::global_path(f.home.path());
                let mut value: Value = serde_json::from_slice(&fs::read(&p).unwrap()).unwrap();
                value["skills"]["example"]["ref"] = json!("other");
                fs::write(p, serde_json::to_vec(&value).unwrap()).unwrap();
            } else {
                fs::create_dir_all(f.home.path().join(".cursor/skills")).unwrap();
                std::os::unix::fs::symlink(
                    f.home.path().join(".agents/skills/example"),
                    f.home.path().join(".cursor/skills/example"),
                )
                .unwrap();
            }
            assert_eq!(
                f.updates
                    .apply_with(f.home.path(), plan, |_, _| panic!(
                        "stale plans must not materialize"
                    ))
                    .err()
                    .unwrap(),
                STALE
            );
        }
    }
    #[test]
    fn apply_keeps_whole_backup_exact_links_and_unrelated_lock_edits() {
        let f = fixture();
        let plan = checked(&f);
        let link = fs::read_link(f.home.path().join(".codex/skills/example")).unwrap();
        let p = lockfile::global_path(f.home.path());
        let mut lock: Value = serde_json::from_slice(&fs::read(&p).unwrap()).unwrap();
        lock["skills"]["other"] = json!({"external":true});
        fs::write(&p, serde_json::to_vec(&lock).unwrap()).unwrap();
        let result = f.updates.apply_with(f.home.path(), plan, stage).unwrap();
        assert!(
            fs::read_to_string(Path::new(&result.backup_path).join("SKILL.md"))
                .unwrap()
                .ends_with("old")
        );
        assert_eq!(
            fs::read_to_string(Path::new(&result.backup_path).join("local.txt")).unwrap(),
            "local customization"
        );
        assert!(
            fs::read_to_string(f.home.path().join(".agents/skills/example/SKILL.md"))
                .unwrap()
                .ends_with("new")
        );
        assert_eq!(
            fs::read_link(f.home.path().join(".codex/skills/example")).unwrap(),
            link
        );
        assert!(!f.home.path().join(".cursor").exists());
        assert_eq!(local_catalog(f.home.path()).len(), 1);
        let updated: Value = serde_json::from_slice(&fs::read(p).unwrap()).unwrap();
        assert_eq!(updated["skills"]["example"]["ref"], "release");
        assert_eq!(
            updated["skills"]["example"]["installedAt"],
            "2025-01-01T00:00:00Z"
        );
        assert_eq!(updated["skills"]["example"]["unknown"]["preserve"], 1);
        assert_eq!(updated["skills"]["other"]["external"], true);
        assert_eq!(updated["skills"]["example"]["skillFolderHash"], NEW);
        assert_eq!(updated["customTop"], true);
    }
    #[test]
    fn stage_links_and_target_changed_during_materialization_are_rejected() {
        let f = fixture();
        let plan = checked(&f);
        assert!(
            f.updates
                .apply_with(f.home.path(), plan, |plan, isolated| {
                    let path = stage(plan, isolated)?;
                    std::os::unix::fs::symlink("/etc/passwd", path.join("outside")).unwrap();
                    Ok(path)
                })
                .is_err()
        );
        let plan = checked(&f);
        assert_eq!(
            f.updates
                .apply_with(f.home.path(), plan, |plan, isolated| {
                    fs::write(
                        f.home.path().join(".agents/skills/example/local.txt"),
                        "edited",
                    )
                    .unwrap();
                    stage(plan, isolated)
                })
                .err()
                .unwrap(),
            STALE
        );
    }
    #[test]
    fn plugin_collision_and_legacy_lock_are_unsupported() {
        let f = fixture();
        write_skill(
            &f.home
                .path()
                .join(".codex/plugins/cache/org/plugin/1/skills/example"),
            "plugin",
        );
        let skills = local_catalog(f.home.path());
        let plugin = skills
            .iter()
            .find(|s| s.installs.iter().any(|i| i.plugin))
            .unwrap();
        assert_eq!(
            plugin.source.as_ref().unwrap().kind,
            crate::model::SourceKind::Plugin
        );
        assert_eq!(
            candidate(f.home.path(), plugin, &skills).err().unwrap(),
            "pluginManaged"
        );
        let p = lockfile::global_path(f.home.path());
        fs::write(p, r#"{"version":2,"skills":{}}"#).unwrap();
        assert!(f.updates.inspect(f.home.path()).iter().all(|s| matches!(&s.state, UpdateState::Unsupported { reason } if reason == "legacyLock" || reason == "pluginManaged" || reason == "ambiguousLock")));
    }
    fn interrupted(f: &Fixture, phase: Phase, new_lock_written: bool) -> (Transaction, PathBuf) {
        let plan = checked(f);
        let root = fs::canonicalize(f.home.path().join(".agents/skills")).unwrap();
        let staged = root.join(".loadout-stage-test");
        write_skill(&staged, "new");
        let mut tx = Transaction {
            target: root.join("example"),
            backup: root.join(".loadout-backup-test"),
            staged,
            lock_path: lockfile::global_path(f.home.path()),
            old_lock: plan.candidate._lock_bytes.clone(),
            new_lock: b"{\"version\":3,\"skills\":{}}".to_vec(),
            old_fingerprint: plan.candidate.target_fingerprint,
            new_fingerprint: String::new(),
            phase,
        };
        tx.new_fingerprint = fingerprint(&tx.staged).unwrap();
        if tx.phase != Phase::Prepared {
            fs::rename(&tx.target, &tx.backup).unwrap();
        }
        if tx.phase == Phase::Promoted {
            fs::rename(&tx.staged, &tx.target).unwrap();
        }
        if new_lock_written {
            fs::write(&tx.lock_path, &tx.new_lock).unwrap();
        }
        let dir = transaction_dir(f.home.path());
        fs::create_dir_all(&dir).unwrap();
        let manifest = dir.join("fixture.json");
        save(&manifest, &tx).unwrap();
        (tx, manifest)
    }
    #[test]
    fn startup_recovers_each_interrupted_phase_and_is_idempotent() {
        for (phase, written) in [
            (Phase::Prepared, false),
            (Phase::BackedUp, false),
            (Phase::Promoted, false),
            (Phase::Promoted, true),
        ] {
            let f = fixture();
            let (tx, _) = interrupted(&f, phase, written);
            recover(f.home.path()).unwrap();
            recover(f.home.path()).unwrap();
            assert_eq!(fingerprint(&tx.target).unwrap(), tx.old_fingerprint);
            assert_eq!(fs::read(&tx.lock_path).unwrap(), tx.old_lock);
        }
    }
    #[test]
    fn recovery_preserves_external_lock_change() {
        let f = fixture();
        let (tx, _) = interrupted(&f, Phase::Promoted, false);
        let external = b"{\"version\":3,\"skills\":{},\"external\":true}";
        fs::write(&tx.lock_path, external).unwrap();
        assert!(recover(f.home.path()).is_err());
        assert_eq!(fs::read(&tx.lock_path).unwrap(), external);
        assert_eq!(fingerprint(&tx.target).unwrap(), tx.old_fingerprint);
    }
    #[test]
    fn failed_lock_publication_restores_original_directory() {
        let f = fixture();
        let plan = checked(&f);
        fs::create_dir_all(transaction_dir(f.home.path())).unwrap();
        let agents = f.home.path().join(".agents");
        fs::set_permissions(&agents, fs::Permissions::from_mode(0o500)).unwrap();
        let result = f.updates.apply_with(f.home.path(), plan, stage);
        fs::set_permissions(agents, fs::Permissions::from_mode(0o700)).unwrap();
        assert!(
            result
                .unwrap_err()
                .contains("Previous installation restored")
        );
        let manifests = fs::read_dir(transaction_dir(f.home.path()))
            .unwrap()
            .collect::<std::io::Result<Vec<_>>>()
            .unwrap();
        assert_eq!(manifests.len(), 1);
        let manifest: Transaction =
            serde_json::from_slice(&fs::read(manifests[0].path()).unwrap()).unwrap();
        assert!(manifest.phase == Phase::Restored);
        assert!(!manifest.backup.exists());
        assert!(
            fs::read_dir(f.home.path().join(".agents/skills"))
                .unwrap()
                .any(|e| e
                    .unwrap()
                    .file_name()
                    .to_string_lossy()
                    .starts_with(".loadout-recovery-"))
        );
        assert!(
            fs::read_to_string(f.home.path().join(".agents/skills/example/SKILL.md"))
                .unwrap()
                .ends_with("old")
        );
        assert_eq!(
            strict_lock(&lockfile::global_path(f.home.path()))
                .unwrap()
                .1["skills"]["example"]["skillFolderHash"],
            OLD
        );
    }
    #[test]
    fn materializer_has_pinned_argv_and_isolated_environment() {
        let f = fixture();
        let plan = checked(&f);
        let isolated = tempfile::tempdir().unwrap();
        let cmd = materializer_command(Path::new("/opt/node/bin/npx"), &plan, isolated.path());
        let args: Vec<_> = cmd
            .get_args()
            .map(|v| v.to_string_lossy().into_owned())
            .collect();
        assert_eq!(
            args,
            [
                "-y",
                "skills@1.7.4",
                "add",
                &format!("owner/repo/skills/example#{COMMIT}"),
                "--skill",
                "example",
                "-g",
                "-y",
                "-a",
                "codex",
                "--json"
            ]
        );
        let env: HashMap<_, _> = cmd
            .get_envs()
            .map(|(k, v)| {
                (
                    k.to_string_lossy().into_owned(),
                    v.map(|v| v.to_string_lossy().into_owned()),
                )
            })
            .collect();
        assert_eq!(env["HOME"].as_deref(), isolated.path().to_str());
        assert_eq!(
            env["XDG_STATE_HOME"].as_deref(),
            isolated.path().join("state").to_str()
        );
        for (key, rel) in [
            ("XDG_CONFIG_HOME", "config"),
            ("XDG_CACHE_HOME", "cache"),
            ("XDG_DATA_HOME", "data"),
            ("XDG_STATE_HOME", "state"),
            ("npm_config_cache", "npm-cache"),
            ("NPM_CONFIG_CACHE", "npm-cache"),
            ("npm_config_userconfig", "npmrc"),
            ("NPM_CONFIG_USERCONFIG", "npmrc"),
        ] {
            assert_eq!(env[key].as_deref(), isolated.path().join(rel).to_str());
        }
        assert!(env["PATH"].as_ref().unwrap().starts_with("/opt/node/bin:"));
        assert_eq!(env["LOADOUT_HOME"], None);
        assert_eq!(env["NODE_OPTIONS"], None);
        assert_eq!(cmd.get_current_dir(), Some(isolated.path()));
    }
    #[test]
    fn fake_materializer_executes_in_isolation_and_rejects_staged_hash_mismatch() {
        let f = fixture();
        let plan = checked(&f);
        let before = fingerprint(f.home.path()).unwrap();
        for hash in [NEW, OLD] {
            let temp = tempfile::tempdir().unwrap();
            let isolated = fs::canonicalize(temp.path()).unwrap();
            let runner = isolated.join("fake-npx");
            let staged_lock = json!({"version":3,"skills":{"example":{"source":"owner/repo","sourceType":"github","sourceUrl":"https://github.com/owner/repo.git","ref":COMMIT,"skillPath":"skills/example/SKILL.md","skillFolderHash":hash}}});
            let script = format!(
                r#"#!/bin/sh
set -eu
/usr/bin/env > "$HOME/observed-env"
printf '%s\n' "$@" > "$HOME/observed-args"
mkdir -p "$HOME/.agents/skills/example" "$XDG_STATE_HOME/skills"
printf '%s' 'new body' > "$HOME/.agents/skills/example/SKILL.md"
cat > "$XDG_STATE_HOME/skills/.skill-lock.json" <<'LOCK'
{staged_lock}
LOCK
printf 'npm launcher notice\n'
printf '[{{"name":"example","status":"installed","source":"owner/repo","ref":"{COMMIT}","hash":"0000000000000000000000000000000000000000000000000000000000000000","path":"%s/.agents/skills/example","scope":"global","agents":["Codex"],"mode":"copy"}}]\n' "$HOME"
"#
            );
            fs::write(&runner, script).unwrap();
            fs::set_permissions(&runner, fs::Permissions::from_mode(0o700)).unwrap();
            let result = materialize_with(&runner, &plan, &isolated);
            if hash == NEW {
                assert_eq!(result.unwrap(), isolated.join(".agents/skills/example"));
            } else {
                assert!(result.unwrap_err().contains("upstream hash"));
            }
            let env = fs::read_to_string(isolated.join("observed-env")).unwrap();
            for (key, suffix) in [
                ("HOME", ""),
                ("XDG_CONFIG_HOME", "/config"),
                ("XDG_DATA_HOME", "/data"),
                ("XDG_CACHE_HOME", "/cache"),
                ("XDG_STATE_HOME", "/state"),
            ] {
                assert!(
                    env.lines()
                        .any(|l| l == format!("{key}={}{suffix}", isolated.display()))
                );
            }
            let args = fs::read_to_string(isolated.join("observed-args")).unwrap();
            assert!(args.contains(&format!("owner/repo/skills/example#{COMMIT}")));
            assert!(args.ends_with("-a\ncodex\n--json\n"));
        }
        assert_eq!(fingerprint(f.home.path()).unwrap(), before);
    }
    #[test]
    fn distinct_checked_skills_share_requests_and_both_plans_apply_in_sequence() {
        let f = fixture();
        let second = f.home.path().join(".agents/skills/second");
        fs::create_dir_all(&second).unwrap();
        fs::write(
            second.join("SKILL.md"),
            "---\nname: second\n---\nold second",
        )
        .unwrap();
        let lock_path = lockfile::global_path(f.home.path());
        let mut lock = strict_lock(&lock_path).unwrap().1;
        lock["skills"]["second"] = lock["skills"]["example"].clone();
        lock["skills"]["second"]["skillPath"] = json!("skills/second/SKILL.md");
        fs::write(&lock_path, serde_json::to_vec(&lock).unwrap()).unwrap();
        struct TwoSkills(FakeRemote);
        impl Remote for TwoSkills {
            fn get(&self, path: &str) -> Result<Value> {
                let mut result = self.0.get(path)?;
                if path.contains("/git/trees/") {
                    result["tree"].as_array_mut().unwrap().extend([
                        json!({"path":"skills/second","type":"tree","sha":NEW}),
                        json!({"path":"skills/second/SKILL.md","type":"blob","sha":OLD}),
                    ]);
                }
                Ok(result)
            }
        }
        let remote = TwoSkills(FakeRemote::new(NEW));
        let ids = local_catalog(f.home.path())
            .iter()
            .map(|s| s.id.clone())
            .collect::<Vec<_>>();
        let results = f.updates.check_with(f.home.path(), &ids, &remote).unwrap();
        assert_eq!(remote.0.calls.borrow().len(), 2);
        for result in results {
            let UpdateState::Available { plan_id, .. } = result.state else {
                panic!("expected available")
            };
            let plan = f.updates.plans.lock().unwrap().remove(&plan_id).unwrap();
            f.updates
                .apply_with(f.home.path(), plan, |plan, isolated| {
                    let staged = isolated.join(".agents/skills").join(&plan.candidate.key);
                    fs::create_dir_all(&staged).unwrap();
                    fs::write(staged.join("SKILL.md"), "updated").unwrap();
                    Ok(staged)
                })
                .unwrap();
        }
        let lock = strict_lock(&lock_path).unwrap().1;
        assert_eq!(lock["skills"]["example"]["skillFolderHash"], NEW);
        assert_eq!(lock["skills"]["second"]["skillFolderHash"], NEW);
        assert_eq!(local_catalog(f.home.path()).len(), 2);
    }
}
