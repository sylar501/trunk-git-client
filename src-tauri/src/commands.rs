//! Tauri command surface invoked from the frontend.
//! Stub commands only — Session 1 scaffolds the surface, later sessions implement behaviour.

use crate::workspace;

#[tauri::command]
pub fn open_repository(path: String) -> Result<workspace::RepoHandle, String> {
    workspace::open_repository(&path).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn open_workspace(path: String) -> Result<workspace::WorkspaceFile, String> {
    workspace::load_workspace(&path).map_err(|e| e.to_string())
}

// Reserved for future sessions: commit graph walk, diff/staging, branch/tag/stash/remote CRUD,
// push/fetch/pull, interactive rebase, conflict resolution, terminal pty I/O (see git/terminal modules).
