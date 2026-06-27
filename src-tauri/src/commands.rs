//! Tauri command surface invoked from the frontend.

use std::sync::Mutex;

use crate::recent::{self, RecentEntryView, RecentKind};
use crate::state::{AppMode, AppState, AppStateView};
use crate::workspace;
use tauri::{AppHandle, Manager, State};

#[tauri::command]
pub fn open_repository(
    app: AppHandle,
    state: State<'_, Mutex<AppState>>,
    path: String,
) -> Result<workspace::RepoHandle, String> {
    let handle = workspace::open_repository(&path)?;
    {
        let mut s = state.lock().unwrap();
        s.mode = Some(AppMode::Repository);
        s.repo_path = Some(path.clone());
        s.workspace_path = None;
        s.workspace = None;
        s.active_repo = None;
    }
    recent::record(&app, &path, RecentKind::Repository)?;
    Ok(handle)
}

#[tauri::command]
pub fn open_workspace(
    app: AppHandle,
    state: State<'_, Mutex<AppState>>,
    path: String,
) -> Result<workspace::WorkspaceSession, String> {
    let session = workspace::open_workspace_session(&path)?;
    {
        let mut s = state.lock().unwrap();
        s.mode = Some(AppMode::Workspace);
        s.workspace_path = Some(path.clone());
        s.workspace = Some(session.workspace.clone());
        s.active_repo = session.active_repo.clone();
        s.repo_path = None;
    }
    recent::record(&app, &path, RecentKind::Workspace)?;
    Ok(session)
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
    state: State<'_, Mutex<AppState>>,
    url: String,
    destination: String,
    workspace_action: workspace::CloneWorkspaceAction,
) -> Result<workspace::CloneOutcome, String> {
    // `workspace::clone_repository` only writes to disk (repo + optional `.trunk`) — when
    // cloning into an already-open workspace (§15.5.2), AppState must be refreshed here too,
    // otherwise the sidebar keeps showing the pre-clone repo list until something else
    // happens to re-fetch it. The welcome-screen paths (None/CreateNew) instead rely on the
    // frontend calling `open_repository`/`open_workspace` right after — there's no open
    // workspace state to patch in those cases.
    let was_add_to_existing = matches!(
        workspace_action,
        workspace::CloneWorkspaceAction::AddToExisting { .. }
    );
    let outcome = workspace::clone_repository(app, url, destination, workspace_action).await?;
    if was_add_to_existing {
        if let Some(workspace_path) = &outcome.workspace_path {
            let mut s = state.lock().unwrap();
            if let Ok(wf) = workspace::load_workspace(workspace_path) {
                s.workspace = Some(wf);
                s.active_repo = Some(outcome.repo_path.clone());
            }
        }
    }
    Ok(outcome)
}

/// Switches the active repo within the open workspace (PRD §15.4.3). Hard-blocked mid-rebase
/// (§15.4.4) — no override, unlike the conflict-resolution case which is a frontend warn
/// dialog instead (see `sidebar.js`).
#[tauri::command]
pub fn switch_active_repository(
    state: State<'_, Mutex<AppState>>,
    repo_path: String,
) -> Result<(), String> {
    let mut s = state.lock().unwrap();
    if s.rebase_in_progress {
        return Err("Cannot switch repositories while a rebase is in progress.".into());
    }
    let workspace_path = s
        .workspace_path
        .clone()
        .ok_or("Not in workspace mode.".to_string())?;
    let wf = workspace::set_active_repository(&workspace_path, &repo_path)?;
    s.workspace = Some(wf);
    s.active_repo = Some(repo_path);
    Ok(())
}

/// Adds a repo to the open workspace (PRD §15.6's checklist, and plain "Add existing" with
/// no nesting) and makes it active.
#[tauri::command]
pub fn add_existing_repository(
    state: State<'_, Mutex<AppState>>,
    repo_path: String,
) -> Result<workspace::WorkspaceFile, String> {
    let mut s = state.lock().unwrap();
    let workspace_path = s
        .workspace_path
        .clone()
        .ok_or("Not in workspace mode.".to_string())?;
    let wf = workspace::add_repository_to_workspace(&workspace_path, &repo_path)?;
    s.workspace = Some(wf.clone());
    s.active_repo = Some(repo_path);
    Ok(wf)
}

/// Snapshot of the current mode/repo/workspace — lets a freshly-loaded `index.html` ask
/// "what's currently open" without re-deriving it from query params.
#[tauri::command]
pub fn get_app_state(state: State<'_, Mutex<AppState>>) -> AppStateView {
    AppStateView::from(&*state.lock().unwrap())
}

#[tauri::command]
pub fn repo_quick_info(path: String) -> Result<workspace::RepoQuickInfo, String> {
    workspace::repo_quick_info(&path)
}

// Reserved for future sessions: commit graph walk, diff/staging, branch/tag/stash/remote CRUD,
// push/fetch/pull, interactive rebase, conflict resolution, terminal pty I/O (see git/terminal modules).
