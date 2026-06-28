//! Runtime app state (PRD §15.3, §15.4): which mode Trunk is in, which repo/workspace is
//! open, and which repo is active. Kept separate from `workspace` so that module's functions
//! stay pure and unit-testable without a `Mutex`/`AppHandle` in scope (mirrors how `recent`
//! is already a separate axis from `workspace`).

use std::collections::HashSet;

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
    /// Repo paths with a currently-unresolved merge/cherry-pick/revert conflict (PRD §4.6/§9,
    /// SPEC.md item 6) — a set, not a single path, because a workspace can have more than one
    /// repo conflicted at once and the user may navigate to a third, uninvolved repo without
    /// either of those resolving. Kept in sync by `cherry_pick_commit`/`revert_commit` (insert),
    /// `finish_conflict_resolution`/`abort_conflict_resolution` (remove), and resynced from disk
    /// (`Repo::has_conflict`) whenever a repo becomes active (`open_repository`,
    /// `switch_active_repository`) so a conflict left over from outside Trunk — a previous
    /// session, or a manual `git merge` in a terminal — is picked up the moment that repo is
    /// opened, without scanning every repo in a large workspace on every render.
    pub conflicted_repos: HashSet<String>,
    /// Destructive-switch gating (PRD §15.4.4) — always false until interactive rebase (item 10)
    /// exists and starts setting this.
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
                    conflicted: s.conflicted_repos.contains(stored),
                })
                .collect(),
            _ => Vec::new(),
        };
        // "Is there a conflict in progress" only means something relative to *which* repo is
        // currently being looked at — Repository mode has exactly one candidate, Workspace mode
        // the active one. Re-deriving this on every call (rather than storing the bool directly)
        // is what keeps it correct across a repo switch with no extra plumbing on that path.
        let active_path = s.repo_path.as_ref().or(s.active_repo.as_ref());
        let conflict_resolution_in_progress =
            active_path.is_some_and(|p| s.conflicted_repos.contains(p));
        AppStateView {
            mode: s.mode,
            repo_path: s.repo_path.clone(),
            workspace_path: s.workspace_path.clone(),
            workspace: s.workspace.clone(),
            active_repo: s.active_repo.clone(),
            repos,
            conflict_resolution_in_progress,
            rebase_in_progress: s.rebase_in_progress,
        }
    }
}
