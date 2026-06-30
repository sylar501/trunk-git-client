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
            commands::promote_to_workspace,
            commands::clone_repository,
            commands::default_directory,
            commands::switch_active_repository,
            commands::add_existing_repository,
            commands::get_app_state,
            commands::repo_quick_info,
            commands::open_graph,
            commands::get_graph_rows,
            commands::get_commit_index,
            commands::list_branches,
            commands::get_commit_detail,
            commands::get_commit_file_diff,
            commands::cherry_pick_commit,
            commands::revert_commit,
            commands::get_conflict_status,
            commands::get_conflict_file,
            commands::finish_conflict_resolution,
            commands::abort_conflict_resolution,
            commands::start_interactive_rebase,
            commands::begin_rebase_execution,
            commands::get_rebase_session,
            commands::resume_rebase_execution,
            commands::continue_rebase_after_edit,
            commands::abort_interactive_rebase,
            commands::create_branch_at,
            commands::list_branches_for_switch,
            commands::checkout_branch,
            commands::get_branch_delete_info,
            commands::rename_branch,
            commands::delete_branch,
            commands::get_settings,
            commands::save_settings,
            commands::get_working_tree_status,
            commands::get_working_file_diff,
            commands::stage_file,
            commands::unstage_file,
            commands::stage_hunk,
            commands::unstage_hunk,
            commands::stage_line,
            commands::unstage_line,
            commands::get_last_commit_message,
            commands::commit_changes,
            commands::list_remotes,
            commands::get_remote_url,
            commands::list_branches_with_tracking,
            commands::list_commits_ahead,
            commands::list_commits_behind,
            commands::push_branch,
            commands::fetch_remote,
            commands::pull_branch,
        ])
        .run(tauri::generate_context!())
        .expect("error while running trunk");
}
