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
}

type Cmd<T> = Result<T, String>;

async fn blocking<T: Send + 'static>(f: impl FnOnce() -> Cmd<T> + Send + 'static) -> Cmd<T> {
    tauri::async_runtime::spawn_blocking(f).await.map_err(|e| e.to_string())?
}

fn with_index<T>(state: &State<'_, AppState>, f: impl FnOnce(&Path, &mut Index) -> Cmd<T>) -> Cmd<T> {
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
pub async fn usage_heatmap(state: State<'_, AppState>, id: Option<String>, days: Option<u32>) -> Cmd<Vec<DayCount>> {
    let (home, index) = (state.home.clone(), state.index.clone());
    blocking(move || {
        let mut idx = index.lock().map_err(|e| e.to_string())?;
        let skills = service::load(&home, &mut idx, false)?;
        Ok(service::heatmap(&skills, id.as_deref(), days.unwrap_or(365)))
    })
    .await
}

#[tauri::command]
pub fn set_tags(state: State<'_, AppState>, id: String, tags: Vec<String>) -> Cmd<()> {
    with_index(&state, |_, idx| idx.set_tags(&id, &tags).map_err(|e| e.to_string()))
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
pub async fn plan_removal(state: State<'_, AppState>, id: String, paths: Option<Vec<String>>) -> Cmd<Vec<String>> {
    let (home, index) = (state.home.clone(), state.index.clone());
    blocking(move || {
        let mut idx = index.lock().map_err(|e| e.to_string())?;
        let skills = service::load(&home, &mut idx, false)?;
        let skill = skills.iter().find(|s| s.id == id).ok_or("skill not found")?;
        service::plan_removal(skill, paths.as_deref())
    })
    .await
}

#[tauri::command]
pub async fn remove_skill(state: State<'_, AppState>, id: String, paths: Option<Vec<String>>) -> Cmd<RemoveReport> {
    let (home, index) = (state.home.clone(), state.index.clone());
    blocking(move || {
        let mut idx = index.lock().map_err(|e| e.to_string())?;
        let skills = service::load(&home, &mut idx, false)?;
        let skill = skills.iter().find(|s| s.id == id).ok_or("skill not found")?;
        let plan = service::plan_removal(skill, paths.as_deref())?;
        service::execute_removal(&home, skill, plan, service::move_to_trash)
    })
    .await
}

static ANSI: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\x1b\[[0-9;?]*[A-Za-z]|\r").unwrap());

fn clean_arg(v: &str, what: &str) -> Cmd<String> {
    let v = v.trim();
    if v.is_empty() || v.starts_with('-') || v.chars().any(|c| c.is_whitespace() || c.is_control()) {
        return Err(format!("invalid {what}"));
    }
    Ok(v.to_string())
}

pub fn install_argv(repo: &str, agent_ids: &[String], skill: Option<&str>) -> Cmd<Vec<String>> {
    let mut argv = vec!["-y".to_string(), "skills".into(), "add".into(), clean_arg(repo, "repository")?, "-g".into(), "-y".into()];
    for id in agent_ids {
        let cli = agents::find(id).and_then(|a| a.cli_name).ok_or_else(|| format!("agent {id} cannot be installed to"))?;
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
pub async fn install(app: AppHandle, repo: String, agents: Vec<String>, skill: Option<String>) -> Cmd<i32> {
    let argv = install_argv(&repo, &agents, skill.as_deref())?;
    blocking(move || {
        let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/zsh".into());
        let mut child = Command::new(shell)
            .args(["-l", "-i", "-c", r#"exec npx "$@""#, "loadout"])
            .args(&argv)
            .env("NO_COLOR", "1")
            .env("CI", "1")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| format!("failed to start npx: {e}"))?;
        let _ = app.emit("install-log", format!("$ npx {}", argv.join(" ")));

        let pump = |reader: Box<dyn std::io::Read + Send>, app: AppHandle| {
            std::thread::spawn(move || {
                for line in BufReader::new(reader).lines().map_while(Result::ok) {
                    let line = ANSI.replace_all(&line, "").to_string();
                    if line.trim().is_empty() || line.contains("no job control") || line.contains("tty process group") {
                        continue;
                    }
                    let _ = app.emit("install-log", line);
                }
            })
        };
        let out = pump(Box::new(child.stdout.take().unwrap()), app.clone());
        let err = pump(Box::new(child.stderr.take().unwrap()), app.clone());
        let status = child.wait().map_err(|e| e.to_string())?;
        let _ = (out.join(), err.join());
        Ok(status.code().unwrap_or(-1))
    })
    .await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_global_noninteractive_argv_with_cli_agent_names() {
        let argv = install_argv("vercel-labs/agent-skills", &["claude".into(), "codex".into()], Some("pr-review")).unwrap();
        assert_eq!(
            argv,
            ["-y", "skills", "add", "vercel-labs/agent-skills", "-g", "-y", "-a", "claude-code", "-a", "codex", "-s", "pr-review"]
        );
    }

    #[test]
    fn rejects_flag_injection_and_non_installable_agents() {
        assert!(install_argv("--all", &[], None).is_err());
        assert!(install_argv("a/b c", &[], None).is_err());
        assert!(install_argv("a/b", &["shared".into()], None).is_err());
        assert!(install_argv("a/b", &["nope".into()], None).is_err());
        assert!(install_argv("a/b", &[], Some("-x")).is_err());
    }
}
