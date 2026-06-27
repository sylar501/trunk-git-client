#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;
mod git;
mod recent;
mod settings;
mod state;
mod terminal;
mod workspace;

use std::sync::Mutex;

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(Mutex::new(state::AppState::default()))
        .manage(Mutex::new(None::<(String, git::GraphCache)>))
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
            commands::open_graph,
            commands::get_graph_rows,
            commands::list_branches,
            commands::get_commit_detail,
            commands::get_commit_file_diff,
            commands::cherry_pick_commit,
            commands::revert_commit,
            commands::create_branch_at,
            commands::get_settings,
            commands::save_settings,
        ])
        .run(tauri::generate_context!())
        .expect("error while running trunk");
}
