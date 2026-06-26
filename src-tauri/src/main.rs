#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;
mod git;
mod terminal;
mod workspace;

fn main() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            commands::open_repository,
            commands::open_workspace,
        ])
        .run(tauri::generate_context!())
        .expect("error while running trunk");
}
