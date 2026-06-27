#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;
mod git;
mod recent;
mod terminal;
mod workspace;

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            commands::open_repository,
            commands::open_workspace,
            commands::list_recent,
            commands::remove_recent,
            commands::detect_nested_repos,
            commands::create_workspace,
            commands::clone_repository,
            commands::default_directory,
        ])
        .run(tauri::generate_context!())
        .expect("error while running trunk");
}
