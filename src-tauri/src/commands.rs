//! Tauri command surface invoked from the frontend.

use std::sync::Mutex;

use crate::git::{self, Repo};
use crate::recent::{self, RecentEntryView, RecentKind};
use crate::settings::{self, AppSettings};
use crate::state::{AppMode, AppState, AppStateView};
use crate::workspace;
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};

/// Server-side cache for the active repo's commit graph (PRD §7) — keyed by repo path so a
/// stale `get_graph_rows` call against a since-switched repo is rejected rather than silently
/// served. Single-entry: Trunk is single-window/single-context, only one repo's graph is ever
/// being viewed at a time (PRD §15).
pub type GraphState = Mutex<Option<(String, git::GraphCache)>>;

/// Resyncs `AppState.conflicted_repos` for one repo from disk (see that field's doc comment in
/// `state.rs`) — called whenever a repo becomes active so a conflict left over from outside
/// Trunk is picked up immediately. Best-effort: an unreadable repo just leaves the set
/// unchanged rather than failing whatever command called this as a side effect. `Repo::open` +
/// `has_conflict` is cheap (no graph walk), so this runs inline rather than via `spawn_blocking`,
/// same as the other lightweight synchronous commands in this file (`switch_active_repository`,
/// `add_existing_repository`).
fn resync_conflict_state(state: &Mutex<AppState>, repo_path: &str) {
    let conflicted = git::Repo::open(repo_path).map(|r| r.has_conflict()).unwrap_or(false);
    let mut s = state.lock().unwrap();
    if conflicted {
        s.conflicted_repos.insert(repo_path.to_string());
    } else {
        s.conflicted_repos.remove(repo_path);
    }
}

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
    resync_conflict_state(&state, &path);
    recent::record(&app, &path, RecentKind::Repository)?;
    Ok(handle)
}

/// `async` + `spawn_blocking` for the conflict scan below — same reason as `open_graph`/
/// `list_branches`: opening every repo in the workspace (PRD §4.6/§9, SPEC.md item 6, see
/// "instant sidebar conflict badges" requirement) is individually cheap but adds up for a
/// workspace with many repos, and shouldn't freeze the webview's main thread while it runs.
#[tauri::command]
pub async fn open_workspace(
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

    // Scans every repo, not just the one becoming active, so the sidebar's conflict badges are
    // accurate the instant the workspace opens — not just for whichever repo you happen to
    // click into first. Best-effort per repo (an unreadable/stale path just reports `false`),
    // mirroring `resync_conflict_state`'s single-repo behaviour.
    let repo_paths: Vec<String> = session.repos.iter().map(|r| r.path.clone()).collect();
    let conflict_results = tauri::async_runtime::spawn_blocking(move || {
        repo_paths
            .into_iter()
            .map(|p| {
                let conflicted = git::Repo::open(&p).map(|r| r.has_conflict()).unwrap_or(false);
                (p, conflicted)
            })
            .collect::<Vec<_>>()
    })
    .await
    .map_err(|e| e.to_string())?;
    {
        let mut s = state.lock().unwrap();
        for (p, conflicted) in conflict_results {
            if conflicted {
                s.conflicted_repos.insert(p);
            } else {
                s.conflicted_repos.remove(&p);
            }
        }
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
    s.active_repo = Some(repo_path.clone());
    drop(s);
    resync_conflict_state(&state, &repo_path);
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
    s.active_repo = Some(repo_path.clone());
    drop(s);
    resync_conflict_state(&state, &repo_path);
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

// --- Commit graph (PRD §7, SPEC.md item 3) -------------------------------------------------

#[derive(Debug, Serialize)]
pub struct GraphMeta {
    pub total_count: usize,
    pub head_sha: Option<String>,
    pub max_lane: u32,
}

/// Walks the repo once and caches the result; `get_graph_rows` only ever slices this cache.
/// Must be called (again) after switching repos/branches change history — there's no
/// file-watcher yet, so the frontend re-calls this on repo switch and on an explicit refresh.
///
/// `async` + `spawn_blocking` is load-bearing here, not just style: Tauri runs non-`async`
/// commands on the main thread, which the webview shares — a large repo's walk took multiple
/// seconds and froze the whole UI (no spinner could even paint) for that entire span. Moving
/// the git2 work onto a blocking thread (same pattern `workspace::clone_repository` already
/// uses) keeps the main thread free to service the webview while this runs.
#[tauri::command]
pub async fn open_graph(graph_state: State<'_, GraphState>, repo_path: String) -> Result<GraphMeta, String> {
    let path_for_walk = repo_path.clone();
    let cache = tauri::async_runtime::spawn_blocking(move || -> Result<git::GraphCache, String> {
        let repo = Repo::open(&path_for_walk).map_err(|e| format!("not a git repository: {e}"))?;
        repo.build_graph().map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())??;

    let meta = GraphMeta {
        total_count: cache.rows.len(),
        head_sha: cache.head_sha.clone(),
        max_lane: cache.max_lane,
    };
    *graph_state.lock().unwrap() = Some((repo_path, cache));
    Ok(meta)
}

/// Slices the cached graph for the virtualised scroll window `[start, start+count)` and
/// applies `filter` to just that slice — cheap filters (author/message/SHA/date) are O(1)
/// per row from already-cached data; branch ancestry is an O(history) in-memory BFS; path
/// filtering only opens a live repo handle and diffs trees for rows actually in this window,
/// never the whole history (the perf-sensitive part of §7.3).
#[tauri::command]
pub fn get_graph_rows(
    graph_state: State<'_, GraphState>,
    repo_path: String,
    start: usize,
    count: usize,
    filter: git::GraphFilter,
) -> Result<Vec<git::GraphRow>, String> {
    let mut guard = graph_state.lock().unwrap();
    let (cached_path, cache) = guard
        .as_mut()
        .ok_or_else(|| "Graph not opened for this repo — call open_graph first.".to_string())?;
    if cached_path != &repo_path {
        return Err("Graph cache is for a different repository — call open_graph first.".into());
    }

    let end = (start + count).min(cache.rows.len());
    if start >= end {
        return Ok(Vec::new());
    }

    let branch_ancestors = filter.branch.as_deref().map(|b| cache.branch_ancestors(b));
    let path_repo = if filter.path.is_some() {
        git2::Repository::open(&repo_path).ok()
    } else {
        None
    };

    let rows = cache.rows[start..end]
        .iter()
        .cloned()
        .map(|mut row| {
            let mut matched = git::matches_basic(&row, &filter);
            if matched {
                if let Some(ancestors) = &branch_ancestors {
                    matched = ancestors.contains(&row.sha);
                }
            }
            if matched {
                if let (Some(path), Some(repo)) = (&filter.path, &path_repo) {
                    matched = git::row_matches_path(repo, &row, path);
                }
            }
            row.matches = matched;
            row
        })
        .collect();
    Ok(rows)
}

/// Local branches for the sidebar Branches section (PRD §4.2) — colour-hashed the same way
/// as graph lanes (`git::BranchInfo::color_index`) so sidebar dots match the graph exactly.
/// `async` + `spawn_blocking` for the same reason as `open_graph` — keeps a slow/large repo
/// from blocking the main thread the webview shares.
#[tauri::command]
pub async fn list_branches(repo_path: String) -> Result<Vec<git::BranchInfo>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let repo = Repo::open(&repo_path).map_err(|e| format!("not a git repository: {e}"))?;
        repo.list_branches().map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

// --- Commit detail overlay (PRD §4.3, SPEC.md item 4) -------------------------------------

/// `async` + `spawn_blocking` for the same reason as `open_graph`/`list_branches` — keeps the
/// main thread free to service the webview regardless of how large the commit's diff turns out
/// to be, matching the established convention rather than judging this command's workload alone.
#[tauri::command]
pub async fn get_commit_detail(repo_path: String, sha: String) -> Result<git::CommitDetail, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let repo = Repo::open(&repo_path).map_err(|e| format!("not a git repository: {e}"))?;
        repo.commit_detail(&sha).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn get_commit_file_diff(
    repo_path: String,
    sha: String,
    file_path: String,
) -> Result<Vec<git::DiffLineRow>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let repo = Repo::open(&repo_path).map_err(|e| format!("not a git repository: {e}"))?;
        repo.commit_file_diff(&sha, &file_path).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Rejects starting a cherry-pick/revert on a repo that already has a conflict from a previous
/// one unresolved — same hard-block, no-override shape as `switch_active_repository`'s
/// `rebase_in_progress` check above, since starting another one on top would stack two
/// in-progress operations into the same index. Scoped to `repo_path` specifically — a conflict
/// in some *other* repo in the workspace doesn't block this one.
fn guard_no_conflict_in_progress(state: &State<'_, Mutex<AppState>>, repo_path: &str) -> Result<(), String> {
    if state.lock().unwrap().conflicted_repos.contains(repo_path) {
        return Err("Resolve the current conflict before starting another operation.".into());
    }
    Ok(())
}

#[tauri::command]
pub async fn cherry_pick_commit(
    state: State<'_, Mutex<AppState>>,
    repo_path: String,
    sha: String,
    no_commit: bool,
) -> Result<git::ConflictableOutcome, String> {
    guard_no_conflict_in_progress(&state, &repo_path)?;
    let path_for_open = repo_path.clone();
    let outcome = tauri::async_runtime::spawn_blocking(move || {
        let repo = Repo::open(&path_for_open).map_err(|e| format!("not a git repository: {e}"))?;
        repo.cherry_pick(&sha, no_commit)
    })
    .await
    .map_err(|e| e.to_string())??;
    if matches!(outcome, git::ConflictableOutcome::Conflict) {
        state.lock().unwrap().conflicted_repos.insert(repo_path);
    }
    Ok(outcome)
}

#[tauri::command]
pub async fn revert_commit(
    state: State<'_, Mutex<AppState>>,
    repo_path: String,
    sha: String,
    no_commit: bool,
) -> Result<git::ConflictableOutcome, String> {
    guard_no_conflict_in_progress(&state, &repo_path)?;
    let path_for_open = repo_path.clone();
    let outcome = tauri::async_runtime::spawn_blocking(move || {
        let repo = Repo::open(&path_for_open).map_err(|e| format!("not a git repository: {e}"))?;
        repo.revert_commit(&sha, no_commit)
    })
    .await
    .map_err(|e| e.to_string())??;
    if matches!(outcome, git::ConflictableOutcome::Conflict) {
        state.lock().unwrap().conflicted_repos.insert(repo_path);
    }
    Ok(outcome)
}

// --- Conflict resolver (PRD §4.6/§9, SPEC.md item 6) --------------------------------------

#[tauri::command]
pub async fn get_conflict_status(repo_path: String) -> Result<Option<git::ConflictSession>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let repo = Repo::open(&repo_path).map_err(|e| format!("not a git repository: {e}"))?;
        repo.conflict_status()
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn get_conflict_file(
    repo_path: String,
    file_path: String,
) -> Result<Vec<git::ConflictSegment>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let repo = Repo::open(&repo_path).map_err(|e| format!("not a git repository: {e}"))?;
        repo.conflict_file(&file_path)
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Returns `ConflictableOutcome` rather than a bare sha: a resolved rebase step can hand back
/// another `Conflict` (the next commit in the rebase also conflicts) instead of finishing, in
/// which case `conflicted_repos` deliberately stays untouched below.
#[tauri::command]
pub async fn finish_conflict_resolution(
    state: State<'_, Mutex<AppState>>,
    repo_path: String,
    files: Vec<git::ResolvedFile>,
) -> Result<git::ConflictableOutcome, String> {
    let path_for_open = repo_path.clone();
    let outcome = tauri::async_runtime::spawn_blocking(move || {
        let repo = Repo::open(&path_for_open).map_err(|e| format!("not a git repository: {e}"))?;
        repo.finish_conflict_resolution(files)
    })
    .await
    .map_err(|e| e.to_string())??;
    if !matches!(outcome, git::ConflictableOutcome::Conflict) {
        state.lock().unwrap().conflicted_repos.remove(&repo_path);
    }
    Ok(outcome)
}

#[tauri::command]
pub async fn abort_conflict_resolution(
    state: State<'_, Mutex<AppState>>,
    repo_path: String,
) -> Result<(), String> {
    let path_for_open = repo_path.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let repo = Repo::open(&path_for_open).map_err(|e| format!("not a git repository: {e}"))?;
        repo.abort_conflict_resolution()
    })
    .await
    .map_err(|e| e.to_string())??;
    state.lock().unwrap().conflicted_repos.remove(&repo_path);
    Ok(())
}

#[tauri::command]
pub async fn create_branch_at(repo_path: String, sha: String, name: String) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        let repo = Repo::open(&repo_path).map_err(|e| format!("not a git repository: {e}"))?;
        repo.create_branch_at(&sha, &name)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn list_branches_for_switch(repo_path: String) -> Result<Vec<git::SwitchBranchEntry>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let repo = Repo::open(&repo_path).map_err(|e| format!("not a git repository: {e}"))?;
        repo.list_branches_for_switch()
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn checkout_branch(
    state: State<'_, Mutex<AppState>>,
    repo_path: String,
    name: String,
    remote_name: Option<String>,
    remote_branch: Option<String>,
    dirty_strategy: Option<git::DirtyTreeStrategy>,
) -> Result<(), String> {
    guard_no_conflict_in_progress(&state, &repo_path)?;
    tauri::async_runtime::spawn_blocking(move || {
        let repo = Repo::open(&repo_path).map_err(|e| format!("not a git repository: {e}"))?;
        let remote = match (&remote_name, &remote_branch) {
            (Some(r), Some(b)) => Some((r.as_str(), b.as_str())),
            _ => None,
        };
        repo.checkout_branch(&name, remote, dirty_strategy)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn get_branch_delete_info(repo_path: String, name: String) -> Result<git::BranchDeleteInfo, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let repo = Repo::open(&repo_path).map_err(|e| format!("not a git repository: {e}"))?;
        repo.get_branch_delete_info(&name)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn rename_branch(repo_path: String, old_name: String, new_name: String) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        let repo = Repo::open(&repo_path).map_err(|e| format!("not a git repository: {e}"))?;
        repo.rename_branch(&old_name, &new_name)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn delete_branch(
    repo_path: String,
    name: String,
    force: bool,
    also_delete_remote: bool,
) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        let repo = Repo::open(&repo_path).map_err(|e| format!("not a git repository: {e}"))?;
        repo.delete_branch(&name, force, also_delete_remote)
    })
    .await
    .map_err(|e| e.to_string())?
}

// --- App-level settings (sidebar/commit-overlay drag-resize widths) -----------------------

#[tauri::command]
pub fn get_settings(app: AppHandle) -> AppSettings {
    settings::load(&app)
}

#[tauri::command]
pub fn save_settings(
    app: AppHandle,
    sidebar_width: Option<f64>,
    commit_overlay_width: Option<f64>,
    staging_files_width: Option<f64>,
    resolve_merged_height: Option<f64>,
) -> Result<(), String> {
    settings::save(&app, sidebar_width, commit_overlay_width, staging_files_width, resolve_merged_height)
}

// --- Staging & committing (PRD §4.4, §8, SPEC.md item 5) ----------------------------------

/// `async` + `spawn_blocking` for the same reason as `open_graph`/`get_commit_detail` — a large
/// working tree's status walk shouldn't freeze the webview's main thread.
#[tauri::command]
pub async fn get_working_tree_status(repo_path: String) -> Result<git::WorkingTreeStatus, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let repo = Repo::open(&repo_path).map_err(|e| format!("not a git repository: {e}"))?;
        repo.working_tree_status()
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn get_working_file_diff(repo_path: String, file_path: String) -> Result<git::FileHunkDiff, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let repo = Repo::open(&repo_path).map_err(|e| format!("not a git repository: {e}"))?;
        repo.working_file_diff(&file_path)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub fn stage_file(repo_path: String, file_path: String) -> Result<(), String> {
    let repo = Repo::open(&repo_path).map_err(|e| format!("not a git repository: {e}"))?;
    repo.stage_file(&file_path)
}

#[tauri::command]
pub fn unstage_file(repo_path: String, file_path: String) -> Result<(), String> {
    let repo = Repo::open(&repo_path).map_err(|e| format!("not a git repository: {e}"))?;
    repo.unstage_file(&file_path)
}

#[tauri::command]
pub fn stage_hunk(repo_path: String, file_path: String, new_start: u32) -> Result<(), String> {
    let repo = Repo::open(&repo_path).map_err(|e| format!("not a git repository: {e}"))?;
    repo.stage_hunk(&file_path, new_start)
}

#[tauri::command]
pub fn unstage_hunk(repo_path: String, file_path: String, old_start: u32) -> Result<(), String> {
    let repo = Repo::open(&repo_path).map_err(|e| format!("not a git repository: {e}"))?;
    repo.unstage_hunk(&file_path, old_start)
}

#[tauri::command]
pub fn stage_line(repo_path: String, file_path: String, new_start: u32, line_index_in_hunk: u32) -> Result<(), String> {
    let repo = Repo::open(&repo_path).map_err(|e| format!("not a git repository: {e}"))?;
    repo.stage_line(&file_path, new_start, line_index_in_hunk)
}

#[tauri::command]
pub fn unstage_line(repo_path: String, file_path: String, old_start: u32, line_index_in_hunk: u32) -> Result<(), String> {
    let repo = Repo::open(&repo_path).map_err(|e| format!("not a git repository: {e}"))?;
    repo.unstage_line(&file_path, old_start, line_index_in_hunk)
}

#[tauri::command]
pub fn get_last_commit_message(repo_path: String) -> Result<Option<String>, String> {
    let repo = Repo::open(&repo_path).map_err(|e| format!("not a git repository: {e}"))?;
    repo.last_commit_message()
}

#[tauri::command]
pub fn commit_changes(repo_path: String, message: String, amend: bool, ssh_sign: bool) -> Result<String, String> {
    let repo = Repo::open(&repo_path).map_err(|e| format!("not a git repository: {e}"))?;
    repo.commit_changes(&message, amend, ssh_sign)
}

// --- Push / Fetch / Pull (PRD §12, SPEC.md item 7) ----------------------------------------

#[tauri::command]
pub fn list_remotes(repo_path: String) -> Result<Vec<String>, String> {
    let repo = Repo::open(&repo_path).map_err(|e| format!("not a git repository: {e}"))?;
    repo.list_remotes()
}

#[tauri::command]
pub fn get_remote_url(repo_path: String, remote_name: String) -> Result<String, String> {
    let repo = Repo::open(&repo_path).map_err(|e| format!("not a git repository: {e}"))?;
    repo.remote_url(&remote_name)
}

#[tauri::command]
pub fn list_branches_with_tracking(repo_path: String) -> Result<Vec<git::RemoteBranchInfo>, String> {
    let repo = Repo::open(&repo_path).map_err(|e| format!("not a git repository: {e}"))?;
    repo.list_local_branches_with_tracking()
}

#[tauri::command]
pub fn list_commits_ahead(
    repo_path: String,
    local_branch: String,
    remote_name: String,
    remote_branch: String,
) -> Result<Vec<git::CommitSummary>, String> {
    let repo = Repo::open(&repo_path).map_err(|e| format!("not a git repository: {e}"))?;
    repo.list_commits_ahead(&local_branch, &remote_name, &remote_branch)
}

#[tauri::command]
pub fn list_commits_behind(
    repo_path: String,
    local_branch: String,
    remote_name: String,
    remote_branch: String,
) -> Result<Vec<git::CommitSummary>, String> {
    let repo = Repo::open(&repo_path).map_err(|e| format!("not a git repository: {e}"))?;
    repo.list_commits_behind(&local_branch, &remote_name, &remote_branch)
}

#[tauri::command]
pub async fn push_branch(
    app: AppHandle,
    state: State<'_, Mutex<AppState>>,
    repo_path: String,
    local_branch: String,
    remote_name: String,
    remote_branch: String,
    set_upstream: bool,
    force: bool,
    force_with_lease: bool,
) -> Result<(), String> {
    guard_no_conflict_in_progress(&state, &repo_path)?;
    tauri::async_runtime::spawn_blocking(move || {
        let repo = Repo::open(&repo_path).map_err(|e| format!("not a git repository: {e}"))?;
        repo.push_branch(
            move |payload| {
                let _ = app.emit("push-progress", payload);
            },
            &local_branch,
            &remote_name,
            &remote_branch,
            set_upstream,
            force,
            force_with_lease,
        )
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn fetch_remote(
    app: AppHandle,
    state: State<'_, Mutex<AppState>>,
    repo_path: String,
    remote_name: Option<String>,
    prune: bool,
    tags: bool,
    submodules: bool,
) -> Result<git::FetchOutcome, String> {
    guard_no_conflict_in_progress(&state, &repo_path)?;
    tauri::async_runtime::spawn_blocking(move || {
        let repo = Repo::open(&repo_path).map_err(|e| format!("not a git repository: {e}"))?;
        repo.fetch_remote(
            move |payload| {
                let _ = app.emit("fetch-progress", payload);
            },
            remote_name.as_deref(),
            prune,
            tags,
            submodules,
        )
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn pull_branch(
    app: AppHandle,
    state: State<'_, Mutex<AppState>>,
    repo_path: String,
    local_branch: String,
    remote_name: String,
    remote_branch: String,
    strategy: git::PullStrategy,
) -> Result<git::ConflictableOutcome, String> {
    guard_no_conflict_in_progress(&state, &repo_path)?;
    let path_for_open = repo_path.clone();
    let outcome = tauri::async_runtime::spawn_blocking(move || {
        let repo = Repo::open(&path_for_open).map_err(|e| format!("not a git repository: {e}"))?;
        repo.pull_branch(
            move |payload| {
                let _ = app.emit("fetch-progress", payload);
            },
            &local_branch,
            &remote_name,
            &remote_branch,
            strategy,
        )
    })
    .await
    .map_err(|e| e.to_string())??;
    if matches!(outcome, git::ConflictableOutcome::Conflict) {
        state.lock().unwrap().conflicted_repos.insert(repo_path);
    }
    Ok(outcome)
}

// Reserved for future sessions: branch/tag/stash/remote CRUD (beyond push/fetch/pull above),
// interactive rebase, terminal pty I/O (see git/terminal modules).
