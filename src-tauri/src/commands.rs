//! Tauri command surface invoked from the frontend.

use crate::recent::{self, RecentEntryView, RecentKind};
use crate::workspace;
use tauri::{AppHandle, Manager};

#[tauri::command]
pub fn open_repository(app: AppHandle, path: String) -> Result<workspace::RepoHandle, String> {
    let handle = workspace::open_repository(&path)?;
    recent::record(&app, &path, RecentKind::Repository)?;
    Ok(handle)
}

#[tauri::command]
pub fn open_workspace(app: AppHandle, path: String) -> Result<workspace::WorkspaceFile, String> {
    let wf = workspace::load_workspace(&path)?;
    recent::record(&app, &path, RecentKind::Workspace)?;
    Ok(wf)
}

#[tauri::command]
pub fn list_recent(app: AppHandle) -> Result<Vec<RecentEntryView>, String> {
    recent::list(&app)
}

#[tauri::command]
pub fn remove_recent(app: AppHandle, path: String) -> Result<(), String> {
    recent::remove(&app, &path)
}

#[tauri::command]
pub fn detect_nested_repos(path: String) -> Result<workspace::NestedRepoDetection, String> {
    workspace::detect_nested_repos(&path)
}

/// Home directory, used by the frontend to default destination/workspace fields to an
/// absolute path (e.g. clone destination, create-workspace directory) instead of a bare
/// relative name.
#[tauri::command]
pub fn default_directory(app: AppHandle) -> Result<String, String> {
    app.path()
        .home_dir()
        .map(|p| p.to_string_lossy().into_owned())
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn create_workspace(
    app: AppHandle,
    name: String,
    directory: String,
    initial_repos: Vec<String>,
) -> Result<workspace::CreateWorkspaceResult, String> {
    let result = workspace::create_workspace(&name, &directory, initial_repos)?;
    recent::record(&app, &result.path, RecentKind::Workspace)?;
    Ok(result)
}

#[tauri::command]
pub async fn clone_repository(
    app: AppHandle,
    url: String,
    destination: String,
    workspace_path: Option<String>,
) -> Result<workspace::CloneOutcome, String> {
    workspace::clone_repository(app, url, destination, workspace_path).await
}

// Reserved for future sessions: commit graph walk, diff/staging, branch/tag/stash/remote CRUD,
// push/fetch/pull, interactive rebase, conflict resolution, terminal pty I/O (see git/terminal modules).
