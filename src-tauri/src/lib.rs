pub mod agents;
pub mod catalog;
pub mod commands;
pub mod lockfile;
pub mod model;
pub mod scan;
pub mod service;
pub mod usage;

use commands::AppState;
use std::sync::{Arc, Mutex};
use usage::index::Index;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let home = dirs::home_dir().expect("no home directory");
    let db = dirs::data_dir()
        .expect("no data directory")
        .join("SkillsHub")
        .join("skillshub.db");
    let index = Index::open(&db).expect("failed to open usage index");

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(AppState { home, index: Arc::new(Mutex::new(index)) })
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
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
