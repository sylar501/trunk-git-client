#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;
mod git;
mod recent;
mod state;
mod terminal;
mod workspace;

use std::sync::Mutex;

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(Mutex::new(state::AppState::default()))
        .invoke_handler(tauri::generate_handler![
            commands::open_repository,
            commands::open_workspace,
            commands::list_recent,
            commands::remove_recent,
            commands::detect_nested_repos,
            commands::create_workspace,
            commands::clone_repository,
            commands::default_directory,
            commands::switch_active_repository,
            commands::add_existing_repository,
            commands::get_app_state,
            commands::repo_quick_info,
        ])
        .run(tauri::generate_context!())
        .expect("error while running trunk");
}
