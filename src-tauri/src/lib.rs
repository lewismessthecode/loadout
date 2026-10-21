pub mod agents;
pub mod catalog;
pub mod commands;
pub mod lockfile;
pub mod model;
pub mod scan;
pub mod service;
pub mod updates;
pub mod usage;
mod watch;

use commands::AppState;
use std::sync::{Arc, Mutex};
use usage::index::Index;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let (home, db) = runtime_paths();
    if let Err(error) = updates::recover(&home) {
        eprintln!("Update recovery needs attention: {error}");
    }
    let index = Index::open(&db).expect("failed to open usage index");
    let watched_home = home.clone();

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(AppState {
            home,
            index: Arc::new(Mutex::new(index)),
            mutation: Arc::new(Mutex::new(())),
            updates: Arc::new(updates::Updates::default()),
        })
        .setup(move |app| {
            watch::start(app.handle().clone(), watched_home);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::list_agents,
            commands::list_skills,
            commands::get_skill,
            commands::usage_heatmap,
            commands::set_tags,
            commands::reveal_in_finder,
            commands::plan_removal,
            commands::remove_skill,
            commands::install,
            commands::inspect_updates,
            commands::check_updates,
            commands::apply_update,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

fn runtime_paths() -> (std::path::PathBuf, std::path::PathBuf) {
    runtime_paths_with(if cfg!(debug_assertions) {
        std::env::var_os("LOADOUT_HOME").map(std::path::PathBuf::from)
    } else {
        None
    })
}
fn runtime_paths_with(
    fixture: Option<std::path::PathBuf>,
) -> (std::path::PathBuf, std::path::PathBuf) {
    if let Some(home) = fixture.filter(|p| p.is_absolute()) {
        let home = std::fs::canonicalize(&home).unwrap_or(home);
        let db = home.join(".local/share/Loadout/loadout.db");
        return (home, db);
    }
    (
        dirs::home_dir().expect("no home directory"),
        dirs::data_dir()
            .expect("no data directory")
            .join("Loadout/loadout.db"),
    )
}
#[cfg(test)]
mod runtime_tests {
    #[test]
    fn fixture_moves_scanning_and_database_together() {
        let fixture = tempfile::tempdir().unwrap();
        let (home, db) = super::runtime_paths_with(Some(fixture.path().to_path_buf()));
        assert_eq!(home, std::fs::canonicalize(fixture.path()).unwrap());
        assert!(db.starts_with(&home));
        let _index = crate::usage::index::Index::open(&db).unwrap();
        assert!(db.is_file());
    }
}
