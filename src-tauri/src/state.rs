//! Runtime app state (PRD §15.3, §15.4): which mode Trunk is in, which repo/workspace is
//! open, and which repo is active. Kept separate from `workspace` so that module's functions
//! stay pure and unit-testable without a `Mutex`/`AppHandle` in scope (mirrors how `recent`
//! is already a separate axis from `workspace`).

use serde::Serialize;

use crate::workspace::{self, RepoSidebarEntry, WorkspaceFile};

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AppMode {
    Repository,
    Workspace,
}

/// Trunk is single-window, single-context (PRD §15): exactly one repository or workspace is
/// open at a time.
#[derive(Debug, Default)]
pub struct AppState {
    pub mode: Option<AppMode>,
    /// Repository mode: the one open repo. Workspace mode: `None` (`workspace_path` is set).
    pub repo_path: Option<String>,
    pub workspace_path: Option<String>,
    pub workspace: Option<WorkspaceFile>,
    /// Active repo within the open workspace (mirrors `WorkspaceFile.last_active_repository`,
    /// but lives in memory so switching doesn't require a disk round-trip on every read).
    pub active_repo: Option<String>,
    /// Destructive-switch gating (PRD §15.4.4) — always false until conflict resolution
    /// (item 6) / interactive rebase (item 10) exist and start setting these.
    pub conflict_resolution_in_progress: bool,
    pub rebase_in_progress: bool,
}

/// Serializable snapshot returned to the frontend by `get_app_state`. `repos` carries the
/// resolved (stale-flag-included) sidebar rows for Workspace mode — re-derived from disk
/// each call via `resolve_repo_path`, same as `open_workspace_session`, since staleness can
/// change between calls (a repo folder can be moved/deleted while Trunk is open).
#[derive(Debug, Serialize)]
pub struct AppStateView {
    pub mode: Option<AppMode>,
    pub repo_path: Option<String>,
    pub workspace_path: Option<String>,
    pub workspace: Option<WorkspaceFile>,
    pub active_repo: Option<String>,
    pub repos: Vec<RepoSidebarEntry>,
    pub conflict_resolution_in_progress: bool,
    pub rebase_in_progress: bool,
}

impl From<&AppState> for AppStateView {
    fn from(s: &AppState) -> Self {
        let repos = match (&s.workspace_path, &s.workspace) {
            (Some(workspace_path), Some(workspace)) => workspace
                .repositories
                .iter()
                .map(|stored| RepoSidebarEntry {
                    path: stored.clone(),
                    name: workspace::repo_display_name(stored),
                    stale: workspace::resolve_repo_path(workspace_path, stored).is_none(),
                    active: Some(stored) == s.active_repo.as_ref(),
                })
                .collect(),
            _ => Vec::new(),
        };
        AppStateView {
            mode: s.mode,
            repo_path: s.repo_path.clone(),
            workspace_path: s.workspace_path.clone(),
            workspace: s.workspace.clone(),
            active_repo: s.active_repo.clone(),
            repos,
            conflict_resolution_in_progress: s.conflict_resolution_in_progress,
            rebase_in_progress: s.rebase_in_progress,
        }
    }
}
