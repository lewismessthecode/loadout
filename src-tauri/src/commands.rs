use crate::agents::{self, AgentInfo};
use crate::model::Skill;
use crate::service::{self, DayCount, RemoveReport, SkillDetail};
use crate::usage::index::Index;
use regex::Regex;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{Arc, LazyLock, Mutex};
use tauri::{AppHandle, Emitter, State};

pub struct AppState {
    pub home: PathBuf,
    pub index: Arc<Mutex<Index>>,
    pub mutation: Arc<Mutex<()>>,
    pub updates: Arc<crate::updates::Updates>,
}

type Cmd<T> = Result<T, String>;
fn mutation_guard<'a>(mutation: &'a Mutex<()>, home: &Path) -> Cmd<std::sync::MutexGuard<'a, ()>> {
    let guard = mutation
        .try_lock()
        .map_err(|_| "Another installation change is in progress".to_string())?;
    crate::updates::recover(home)?;
    Ok(guard)
}

async fn blocking<T: Send + 'static>(f: impl FnOnce() -> Cmd<T> + Send + 'static) -> Cmd<T> {
    tauri::async_runtime::spawn_blocking(f)
        .await
        .map_err(|e| e.to_string())?
}

fn with_index<T>(
    state: &State<'_, AppState>,
    f: impl FnOnce(&Path, &mut Index) -> Cmd<T>,
) -> Cmd<T> {
    let mut idx = state.index.lock().map_err(|e| e.to_string())?;
    f(&state.home, &mut idx)
}

#[tauri::command]
pub fn list_agents(state: State<'_, AppState>) -> Vec<AgentInfo> {
    service::agents(&state.home)
}

#[tauri::command]
pub async fn list_skills(state: State<'_, AppState>) -> Cmd<Vec<Skill>> {
    let (home, index) = (state.home.clone(), state.index.clone());
    blocking(move || {
        let mut idx = index.lock().map_err(|e| e.to_string())?;
        service::load(&home, &mut idx, true)
    })
    .await
}

#[tauri::command]
pub async fn get_skill(state: State<'_, AppState>, id: String) -> Cmd<SkillDetail> {
    let (home, index) = (state.home.clone(), state.index.clone());
    blocking(move || {
        let mut idx = index.lock().map_err(|e| e.to_string())?;
        service::detail(service::load(&home, &mut idx, false)?, &id)
    })
    .await
}

#[tauri::command]
pub async fn usage_heatmap(
    state: State<'_, AppState>,
    id: Option<String>,
    days: Option<u32>,
) -> Cmd<Vec<DayCount>> {
    let (home, index) = (state.home.clone(), state.index.clone());
    blocking(move || {
        let mut idx = index.lock().map_err(|e| e.to_string())?;
        let skills = service::load(&home, &mut idx, false)?;
        Ok(service::heatmap(
            &skills,
            id.as_deref(),
            days.unwrap_or(365),
        ))
    })
    .await
}

#[tauri::command]
pub fn set_tags(state: State<'_, AppState>, id: String, tags: Vec<String>) -> Cmd<()> {
    with_index(&state, |_, idx| {
        idx.set_tags(&id, &tags).map_err(|e| e.to_string())
    })
}

#[tauri::command]
pub fn reveal_in_finder(state: State<'_, AppState>, path: String) -> Cmd<()> {
    let p = Path::new(&path);
    if !p.starts_with(&state.home) || !p.exists() {
        return Err("path is not inside your home directory or does not exist".into());
    }
    tauri_plugin_opener::reveal_item_in_dir(p).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn plan_removal(
    state: State<'_, AppState>,
    id: String,
    paths: Option<Vec<String>>,
) -> Cmd<Vec<String>> {
    let (home, index) = (state.home.clone(), state.index.clone());
    blocking(move || {
        let mut idx = index.lock().map_err(|e| e.to_string())?;
        let skills = service::load(&home, &mut idx, false)?;
        let skill = skills
            .iter()
            .find(|s| s.id == id)
            .ok_or("skill not found")?;
        service::plan_removal(skill, paths.as_deref())
    })
    .await
}

#[tauri::command]
pub async fn remove_skill(
    app: AppHandle,
    state: State<'_, AppState>,
    id: String,
    paths: Option<Vec<String>>,
) -> Cmd<RemoveReport> {
    let mutation = state.mutation.clone();
    let (home, index) = (state.home.clone(), state.index.clone());
    blocking(move || {
        let _guard = mutation_guard(&mutation, &home)?;
        let mut idx = index.lock().map_err(|e| e.to_string())?;
        let skills = service::load(&home, &mut idx, false)?;
        let skill = skills
            .iter()
            .find(|s| s.id == id)
            .ok_or("skill not found")?;
        let plan = service::plan_removal(skill, paths.as_deref())?;
        let report = service::execute_removal(&home, skill, plan, service::move_to_trash)?;
        let _ = app.emit("skills-changed", ());
        Ok(report)
    })
    .await
}

static ANSI: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\x1b\[[0-9;?]*[A-Za-z]|\r").unwrap());

fn clean_arg(v: &str, what: &str) -> Cmd<String> {
    let v = v.trim();
    if v.is_empty() || v.starts_with('-') || v.chars().any(|c| c.is_whitespace() || c.is_control())
    {
        return Err(format!("invalid {what}"));
    }
    Ok(v.to_string())
}

pub fn install_argv(repo: &str, agent_ids: &[String], skill: Option<&str>) -> Cmd<Vec<String>> {
    if agent_ids.is_empty() {
        return Err("Select at least one agent".into());
    }
    if repo.split_once("://").is_some_and(|(_, rest)| {
        rest.split('/')
            .next()
            .is_some_and(|authority| authority.contains('@'))
    }) {
        return Err("Repository URLs cannot contain credentials".into());
    }
    let mut argv = vec![
        "-y".to_string(),
        "skills".into(),
        "add".into(),
        clean_arg(repo, "repository")?,
        "-g".into(),
        "-y".into(),
    ];
    for id in agent_ids {
        let cli = agents::find(id)
            .and_then(|a| a.cli_name)
            .ok_or_else(|| format!("agent {id} cannot be installed to"))?;
        argv.push("-a".into());
        argv.push(cli.into());
    }
    if let Some(s) = skill.filter(|s| !s.trim().is_empty()) {
        argv.push("-s".into());
        argv.push(clean_arg(s, "skill name")?);
    }
    Ok(argv)
}

#[tauri::command]
pub async fn install(
    app: AppHandle,
    state: State<'_, AppState>,
    repo: String,
    agents: Vec<String>,
    skill: Option<String>,
    operation_id: String,
) -> Cmd<i32> {
    let argv = install_argv(&repo, &agents, skill.as_deref())?;
    let mutation = state.mutation.clone();
    let home = state.home.clone();
    blocking(move || {
        let _guard = mutation_guard(&mutation, &home)?;
        let npx = crate::updates::executable("npx")?;
        let mut install_path = vec![npx.parent().unwrap().to_path_buf()];
        install_path.extend(std::env::split_paths(
            &std::env::var_os("PATH").unwrap_or_default(),
        ));
        let mut command = Command::new(&npx);
        command.args(&argv).env(
            "PATH",
            std::env::join_paths(install_path).map_err(|e| e.to_string())?,
        );
        if dirs::home_dir().as_deref() != Some(home.as_path()) {
            command
                .env_remove("XDG_STATE_HOME")
                .env("XDG_CONFIG_HOME", home.join(".config"))
                .env("XDG_DATA_HOME", home.join(".local/share"))
                .env("XDG_CACHE_HOME", home.join(".cache"))
                .env("CODEX_HOME", home.join(".codex"))
                .env("CLAUDE_CONFIG_DIR", home.join(".claude"));
        }
        let mut child = command
            .env("HOME", &home)
            .env("NO_COLOR", "1")
            .env("CI", "1")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| format!("failed to start npx: {e}"))?;
        let _ = app.emit(
            "install-log",
            InstallLog {
                operation_id: operation_id.clone(),
                line: "Installing selected skills…".into(),
            },
        );

        let pump = |reader: Box<dyn std::io::Read + Send>, app: AppHandle, operation_id: String| {
            std::thread::spawn(move || {
                for line in BufReader::new(reader).lines().map_while(Result::ok) {
                    let line = ANSI.replace_all(&line, "").to_string();
                    if line.trim().is_empty()
                        || line.contains("no job control")
                        || line.contains("tty process group")
                    {
                        continue;
                    }
                    let _ = app.emit(
                        "install-log",
                        InstallLog {
                            operation_id: operation_id.clone(),
                            line,
                        },
                    );
                }
            })
        };
        let out = pump(
            Box::new(child.stdout.take().unwrap()),
            app.clone(),
            operation_id.clone(),
        );
        let err = pump(
            Box::new(child.stderr.take().unwrap()),
            app.clone(),
            operation_id.clone(),
        );
        let status = child.wait().map_err(|e| e.to_string())?;
        let _ = (out.join(), err.join());
        let _ = app.emit("skills-changed", ());
        Ok(status.code().unwrap_or(-1))
    })
    .await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mutation_boundary_rejects_a_second_writer_and_releases_after_finish() {
        let home = tempfile::tempdir().unwrap();
        let mutation = Arc::new(Mutex::new(()));
        let (entered_tx, entered_rx) = std::sync::mpsc::channel();
        let (finish_tx, finish_rx) = std::sync::mpsc::channel();
        let writer = mutation.clone();
        let writer_home = home.path().to_path_buf();
        let first = std::thread::spawn(move || {
            let _guard = mutation_guard(&writer, &writer_home).unwrap();
            entered_tx.send(()).unwrap();
            finish_rx.recv().unwrap();
        });
        entered_rx.recv().unwrap();
        assert_eq!(
            mutation_guard(&mutation, home.path()).err().unwrap(),
            "Another installation change is in progress"
        );
        finish_tx.send(()).unwrap();
        first.join().unwrap();
        assert!(mutation_guard(&mutation, home.path()).is_ok());
    }
    #[test]
    fn builds_global_noninteractive_argv_with_cli_agent_names() {
        let argv = install_argv(
            "vercel-labs/agent-skills",
            &["claude".into(), "codex".into()],
            Some("pr-review"),
        )
        .unwrap();
        assert_eq!(
            argv,
            [
                "-y",
                "skills",
                "add",
                "vercel-labs/agent-skills",
                "-g",
                "-y",
                "-a",
                "claude-code",
                "-a",
                "codex",
                "-s",
                "pr-review"
            ]
        );
    }

    #[test]
    fn rejects_flag_injection_and_non_installable_agents() {
        assert!(install_argv("https://secret@github.com/a/b", &["codex".into()], None).is_err());
        assert!(install_argv("a/b", &[], None).is_err());
        assert!(install_argv("--all", &[], None).is_err());
        assert!(install_argv("a/b c", &[], None).is_err());
        assert!(install_argv("a/b", &["shared".into()], None).is_err());
        assert!(install_argv("a/b", &["nope".into()], None).is_err());
        assert!(install_argv("a/b", &[], Some("-x")).is_err());
    }
}

#[derive(serde::Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct InstallLog {
    operation_id: String,
    line: String,
}

#[tauri::command]
pub async fn inspect_updates(
    state: State<'_, AppState>,
) -> Cmd<Vec<crate::updates::UpdateInspection>> {
    let (home, updates) = (state.home.clone(), state.updates.clone());
    blocking(move || Ok(updates.inspect(&home))).await
}
#[tauri::command]
pub async fn check_updates(
    state: State<'_, AppState>,
    ids: Vec<String>,
) -> Cmd<Vec<crate::updates::UpdateInspection>> {
    let (home, updates) = (state.home.clone(), state.updates.clone());
    blocking(move || updates.check(&home, &ids)).await
}
#[tauri::command]
pub async fn apply_update(
    app: AppHandle,
    state: State<'_, AppState>,
    plan_id: String,
) -> Cmd<crate::updates::UpdateOutcome> {
    let (home, updates, mutation) = (
        state.home.clone(),
        state.updates.clone(),
        state.mutation.clone(),
    );
    blocking(move || {
        let _guard = mutation_guard(&mutation, &home)?;
        let result = updates.apply(&home, &plan_id);
        let _ = app.emit("skills-changed", ());
        result
    })
    .await
}
