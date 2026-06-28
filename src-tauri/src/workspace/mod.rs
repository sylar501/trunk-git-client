//! Workspace mode plumbing (PRD §15): `.trunk` TOML files, Repository vs Workspace mode
//! resolution, nested-repo detection (§15.2), and repository/workspace creation.
//!
//! Repository/Workspace-mode sidebar plumbing (§15.3, §15.4) and the interactive nested-repo
//! checklist picker (§15.6) are out of scope here — see SPEC.md item 2.

use git2::{Repository, RepositoryOpenFlags};
use serde::{Deserialize, Serialize};
use std::path::Path;
use tauri::{AppHandle, Emitter};

use crate::git::Repo;
use crate::recent::{self, RecentKind};

/// A `.trunk` workspace file (PRD §15.4.1): a TOML config grouping repository paths.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceFile {
    pub name: String,
    /// Absolute paths, with relative-to-file resolution as a fallback when a path has moved.
    pub repositories: Vec<String>,
    pub last_active_repository: Option<String>,
    #[serde(default)]
    pub overrides: WorkspaceOverrides,
}

/// Per-workspace config overrides (PRD §14.2 tag version-group pattern, §19.2.2 author identity).
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct WorkspaceOverrides {
    pub tag_version_group_pattern: Option<String>,
    pub author_name: Option<String>,
    pub author_email: Option<String>,
}

/// Result of opening a single repository in Repository mode (PRD §15.3).
#[derive(Debug, Serialize)]
pub struct RepoHandle {
    pub path: String,
    pub name: String,
}

pub fn open_repository(path: &str) -> Result<RepoHandle, String> {
    Repo::open(path).map_err(|e| format!("not a git repository: {e}"))?;
    let name = Path::new(path)
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.to_string());
    Ok(RepoHandle {
        path: path.to_string(),
        name,
    })
}

pub fn load_workspace(path: &str) -> Result<WorkspaceFile, String> {
    let contents = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
    toml::from_str(&contents).map_err(|e| e.to_string())
}

// --- Repository/Workspace mode plumbing (PRD §15.3, §15.4) -------------------------------

/// Resolves a workspace-stored repo path (PRD §15.4.1): try it absolute first; if that
/// doesn't exist on disk, retry as relative to the `.trunk` file's own directory (handles
/// the workspace having moved together with its repos). Reader-side only — `create_workspace`
/// keeps writing absolute paths, so the `.trunk` file format is unchanged.
pub fn resolve_repo_path(workspace_file_path: &str, stored: &str) -> Option<String> {
    if Path::new(stored).exists() {
        return Some(stored.to_string());
    }
    let base = Path::new(workspace_file_path).parent()?;
    let candidate = base.join(stored);
    candidate.exists().then(|| candidate.to_string_lossy().into_owned())
}

pub(crate) fn repo_display_name(stored: &str) -> String {
    Path::new(stored)
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| stored.to_string())
}

/// A single row in the Workspace-mode sidebar's Repositories section (PRD §15.4.2).
#[derive(Debug, Serialize)]
pub struct RepoSidebarEntry {
    pub path: String,
    pub name: String,
    pub stale: bool,
    pub active: bool,
    /// Has an unresolved merge/cherry-pick/revert conflict (PRD §4.6/§9, SPEC.md item 6) —
    /// always `false` from `open_workspace_session` below (that constructor has no access to
    /// `AppState.conflicted_repos`); the canonical, actually-rendered value comes from
    /// `state.rs`'s `AppStateView::from`, the only place this struct is built that the frontend
    /// reads on every render. No UI surfaces this yet — it's plumbed through for a later
    /// per-row indicator/button.
    pub conflicted: bool,
}

/// A resolved workspace ready to drive the sidebar + active-repo state (PRD §15.4.3, §15.7).
#[derive(Debug, Serialize)]
pub struct WorkspaceSession {
    pub workspace_path: String,
    pub workspace: WorkspaceFile,
    pub repos: Vec<RepoSidebarEntry>,
    pub active_repo: Option<String>,
}

/// Opens a workspace file and resolves which repo should be active (PRD §15.7): prefer
/// `last_active_repository` if it still resolves; otherwise fall back to the first
/// non-stale repository. `active_repo` is `None` only when `repositories` is empty or every
/// entry is stale — never a silently-picked stale path.
pub fn open_workspace_session(path: &str) -> Result<WorkspaceSession, String> {
    let workspace = load_workspace(path)?;

    let repos: Vec<RepoSidebarEntry> = workspace
        .repositories
        .iter()
        .map(|stored| RepoSidebarEntry {
            path: stored.clone(),
            name: repo_display_name(stored),
            stale: resolve_repo_path(path, stored).is_none(),
            active: false,
            conflicted: false,
        })
        .collect();

    let active_repo = workspace
        .last_active_repository
        .clone()
        .filter(|p| repos.iter().any(|r| &r.path == p && !r.stale))
        .or_else(|| repos.iter().find(|r| !r.stale).map(|r| r.path.clone()));

    let repos = repos
        .into_iter()
        .map(|mut r| {
            r.active = Some(&r.path) == active_repo.as_ref();
            r
        })
        .collect();

    Ok(WorkspaceSession {
        workspace_path: path.to_string(),
        workspace,
        repos,
        active_repo,
    })
}

/// Updates the active repo and persists `last_active_repository` back to the `.trunk` file
/// so the choice survives a restart (PRD §15.4.3).
pub fn set_active_repository(workspace_path: &str, repo_path: &str) -> Result<WorkspaceFile, String> {
    let mut workspace = load_workspace(workspace_path)?;
    if !workspace.repositories.iter().any(|r| r == repo_path) {
        return Err(format!("{repo_path} is not part of this workspace"));
    }
    workspace.last_active_repository = Some(repo_path.to_string());
    write_workspace_file(workspace_path, &workspace)?;
    Ok(workspace)
}

/// Adds a repo path to an existing workspace and makes it active — used by both "Add
/// existing repository" and the clone-into-an-already-open-workspace path (PRD §15.5.2).
pub fn add_repository_to_workspace(workspace_path: &str, repo_path: &str) -> Result<WorkspaceFile, String> {
    let mut workspace = load_workspace(workspace_path)?;
    if !workspace.repositories.iter().any(|r| r == repo_path) {
        workspace.repositories.push(repo_path.to_string());
    }
    workspace.last_active_repository = Some(repo_path.to_string());
    write_workspace_file(workspace_path, &workspace)?;
    Ok(workspace)
}

fn write_workspace_file(workspace_path: &str, workspace: &WorkspaceFile) -> Result<(), String> {
    let toml_str = toml::to_string_pretty(workspace).map_err(|e| e.to_string())?;
    std::fs::write(workspace_path, toml_str).map_err(|e| e.to_string())
}

/// One-shot read-only hint for the nested-repo workspace picker (PRD §15.6) — not a
/// long-lived `git::Repo` handle, just enough to label a checklist row.
#[derive(Debug, Serialize)]
pub struct RepoQuickInfo {
    pub remote_url: Option<String>,
    pub last_commit_summary: Option<String>,
}

pub fn repo_quick_info(path: &str) -> Result<RepoQuickInfo, String> {
    let repo = Repository::open(path).map_err(|e| e.to_string())?;
    let remote_url = repo
        .find_remote("origin")
        .ok()
        .and_then(|r| r.url().map(String::from));
    let last_commit_summary = repo
        .head()
        .ok()
        .and_then(|h| h.peel_to_commit().ok())
        .and_then(|c| c.summary().map(String::from));
    Ok(RepoQuickInfo {
        remote_url,
        last_commit_summary,
    })
}

// --- Nested-repo detection (PRD §15.2) ---------------------------------------------------

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum NestedRepoDetection {
    NotARepo,
    PlainRepo,
    HasNested { nested: Vec<String> },
}

/// True if `dir` itself (not an ancestor) has a `.git` — exact-directory test, no upward search.
fn is_repo_root(dir: &Path) -> bool {
    Repository::open_ext(dir, RepositoryOpenFlags::NO_SEARCH, Vec::<&str>::new()).is_ok()
}

const MAX_WALK_DEPTH: u32 = 8;

fn walk_for_nested(root: &Path, dir: &Path, depth: u32, found: &mut Vec<String>) {
    if depth > MAX_WALK_DEPTH {
        return;
    }
    let Ok(read_dir) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in read_dir.flatten() {
        let p = entry.path();
        if !p.is_dir() {
            continue;
        }
        let name = entry.file_name();
        if name == ".git" || name == "node_modules" || name == "target" {
            continue;
        }
        if p != root && is_repo_root(&p) {
            found.push(p.to_string_lossy().into_owned());
            continue; // don't look for repos nested inside this nested repo
        }
        walk_for_nested(root, &p, depth + 1, found);
    }
}

pub fn detect_nested_repos(path: &str) -> Result<NestedRepoDetection, String> {
    let root = Path::new(path);
    if !is_repo_root(root) {
        return Ok(NestedRepoDetection::NotARepo);
    }
    let mut nested = Vec::new();
    walk_for_nested(root, root, 0, &mut nested);
    if nested.is_empty() {
        Ok(NestedRepoDetection::PlainRepo)
    } else {
        Ok(NestedRepoDetection::HasNested { nested })
    }
}

// --- Workspace creation -------------------------------------------------------------------

#[derive(Debug, Serialize)]
pub struct CreateWorkspaceResult {
    pub path: String,
    pub workspace: WorkspaceFile,
}

/// Serves both "Create empty workspace" (`initial_repos: []`) and "Open as workspace" from
/// nested-repo detection (`initial_repos: [root, ...nested]`) — one command, not two.
pub fn create_workspace(
    name: &str,
    directory: &str,
    initial_repos: Vec<String>,
) -> Result<CreateWorkspaceResult, String> {
    let file_path = Path::new(directory).join(format!("{name}.trunk"));
    let workspace = WorkspaceFile {
        name: name.to_string(),
        last_active_repository: initial_repos.first().cloned(),
        repositories: initial_repos,
        overrides: WorkspaceOverrides::default(),
    };
    let toml_str = toml::to_string_pretty(&workspace).map_err(|e| e.to_string())?;
    std::fs::write(&file_path, toml_str).map_err(|e| e.to_string())?;
    Ok(CreateWorkspaceResult {
        path: file_path.to_string_lossy().into_owned(),
        workspace,
    })
}

// --- Clone (PRD §15.5.1) ------------------------------------------------------------------

#[derive(Debug, Clone, Serialize)]
pub struct CloneProgressPayload {
    pub received_objects: usize,
    pub total_objects: usize,
    pub indexed_objects: usize,
    pub received_bytes: usize,
}

#[derive(Debug, Serialize)]
pub struct CloneOutcome {
    pub repo_path: String,
    pub workspace_path: Option<String>,
}

/// What the clone dialog should do with the resulting repo once the clone itself succeeds.
/// `workspace_path: Option<String>` used to be overloaded to mean "create a new workspace
/// here" — wrong once a workspace is already open (PRD §15.5.2 wants the clone *added* to
/// it, not the `.trunk` file overwritten with a single-repo workspace).
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum CloneWorkspaceAction {
    /// Plain repo clone, no workspace involved (welcome-screen context, checkbox unchecked).
    None,
    /// Welcome-screen context, "Also create a workspace" checked — item 1's existing path.
    CreateNew { trunk_path: String },
    /// Workspace context (§15.5.2) — a workspace is already open, add the clone to it.
    AddToExisting { trunk_path: String },
}

/// Real `git2` clone with streamed progress (PRD §15.5.1) — not a spinner. Runs the blocking
/// libgit2 call on a dedicated blocking thread (`spawn_blocking`), since `RepoBuilder::clone`
/// has no internal yield points and would otherwise stall a tokio worker for the whole clone.
///
/// No `RemoteCallbacks::credentials()` is wired up — there's no auth UI in this session
/// (§15.5.1 scope). Private-repo clones fail with an inline libgit2 auth error, surfaced
/// through the existing Step-3 error/Retry path.
pub async fn clone_repository(
    app: AppHandle,
    url: String,
    destination: String,
    workspace_action: CloneWorkspaceAction,
) -> Result<CloneOutcome, String> {
    let dest = destination.clone();
    let progress_app = app.clone();

    tauri::async_runtime::spawn_blocking(move || -> Result<(), String> {
        let mut callbacks = git2::RemoteCallbacks::new();
        callbacks.transfer_progress(move |progress: git2::Progress<'_>| {
            let _ = progress_app.emit(
                "clone-progress",
                CloneProgressPayload {
                    received_objects: progress.received_objects(),
                    total_objects: progress.total_objects(),
                    indexed_objects: progress.indexed_objects(),
                    received_bytes: progress.received_bytes(),
                },
            );
            true
        });
        let mut fetch_opts = git2::FetchOptions::new();
        fetch_opts.remote_callbacks(callbacks);
        let mut builder = git2::build::RepoBuilder::new();
        builder.fetch_options(fetch_opts);
        builder
            .clone(&url, Path::new(&dest))
            .map(|_repo| ())
            .map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())??;

    let workspace_path = match workspace_action {
        CloneWorkspaceAction::CreateNew { trunk_path } => {
            let trunk_file = Path::new(&trunk_path);
            let name = trunk_file
                .file_stem()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_else(|| "workspace".to_string());
            let directory = trunk_file
                .parent()
                .map(|p| p.to_string_lossy().into_owned())
                .unwrap_or_else(|| destination.clone());
            let result = create_workspace(&name, &directory, vec![destination.clone()])?;
            recent::record(&app, &result.path, RecentKind::Workspace)?;
            Some(result.path)
        }
        CloneWorkspaceAction::AddToExisting { trunk_path } => {
            add_repository_to_workspace(&trunk_path, &destination)?;
            recent::record(&app, &trunk_path, RecentKind::Workspace)?;
            Some(trunk_path)
        }
        CloneWorkspaceAction::None => {
            recent::record(&app, &destination, RecentKind::Repository)?;
            None
        }
    };

    Ok(CloneOutcome {
        repo_path: destination,
        workspace_path,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    static COUNTER: AtomicU64 = AtomicU64::new(0);

    fn temp_dir() -> std::path::PathBuf {
        let n = COUNTER.fetch_add(1, Ordering::SeqCst);
        let dir = std::env::temp_dir().join(format!(
            "trunk-test-{}-{n}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn not_a_repo_for_plain_folder() {
        let dir = temp_dir();
        let result = detect_nested_repos(dir.to_str().unwrap()).unwrap();
        assert!(matches!(result, NestedRepoDetection::NotARepo));
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn plain_repo_with_no_nesting() {
        let dir = temp_dir();
        Repository::init(&dir).unwrap();
        let result = detect_nested_repos(dir.to_str().unwrap()).unwrap();
        assert!(matches!(result, NestedRepoDetection::PlainRepo));
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn detects_nested_repo() {
        let dir = temp_dir();
        Repository::init(&dir).unwrap();
        let nested_dir = dir.join("vendor/lib");
        std::fs::create_dir_all(&nested_dir).unwrap();
        Repository::init(&nested_dir).unwrap();

        let result = detect_nested_repos(dir.to_str().unwrap()).unwrap();
        match result {
            NestedRepoDetection::HasNested { nested } => {
                assert_eq!(nested.len(), 1);
                assert!(nested[0].ends_with("vendor/lib") || nested[0].ends_with("vendor\\lib"));
            }
            other => panic!("expected HasNested, got {other:?}"),
        }
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn does_not_descend_past_a_found_nested_repo() {
        let dir = temp_dir();
        Repository::init(&dir).unwrap();
        let nested_dir = dir.join("vendor");
        Repository::init(&nested_dir).unwrap();
        let deeper = nested_dir.join("deeper");
        std::fs::create_dir_all(&deeper).unwrap();
        Repository::init(&deeper).unwrap();

        let result = detect_nested_repos(dir.to_str().unwrap()).unwrap();
        match result {
            NestedRepoDetection::HasNested { nested } => assert_eq!(nested.len(), 1),
            other => panic!("expected HasNested, got {other:?}"),
        }
        std::fs::remove_dir_all(&dir).unwrap();
    }

    fn write_workspace(dir: &Path, repos: Vec<String>, last_active: Option<String>) -> std::path::PathBuf {
        let workspace = WorkspaceFile {
            name: "ws".to_string(),
            repositories: repos,
            last_active_repository: last_active,
            overrides: WorkspaceOverrides::default(),
        };
        let path = dir.join("ws.trunk");
        std::fs::write(&path, toml::to_string_pretty(&workspace).unwrap()).unwrap();
        path
    }

    #[test]
    fn resolve_repo_path_absolute_hit() {
        let dir = temp_dir();
        let repo_dir = dir.join("repo");
        Repository::init(&repo_dir).unwrap();
        let resolved = resolve_repo_path(
            dir.join("ws.trunk").to_str().unwrap(),
            repo_dir.to_str().unwrap(),
        );
        assert_eq!(resolved, Some(repo_dir.to_string_lossy().into_owned()));
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn resolve_repo_path_relative_fallback() {
        let dir = temp_dir();
        let repo_dir = dir.join("repo");
        Repository::init(&repo_dir).unwrap();
        let ws_path = dir.join("ws.trunk");
        // Stored as a path that no longer exists where it points, but resolves relative to
        // the workspace file's own directory.
        let resolved = resolve_repo_path(ws_path.to_str().unwrap(), "repo");
        assert_eq!(resolved, Some(repo_dir.to_string_lossy().into_owned()));
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn resolve_repo_path_miss() {
        let dir = temp_dir();
        let ws_path = dir.join("ws.trunk");
        let resolved = resolve_repo_path(ws_path.to_str().unwrap(), "does-not-exist");
        assert_eq!(resolved, None);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn open_workspace_session_prefers_last_active() {
        let dir = temp_dir();
        let repo_a = dir.join("a");
        let repo_b = dir.join("b");
        Repository::init(&repo_a).unwrap();
        Repository::init(&repo_b).unwrap();
        let ws_path = write_workspace(
            &dir,
            vec![
                repo_a.to_string_lossy().into_owned(),
                repo_b.to_string_lossy().into_owned(),
            ],
            Some(repo_b.to_string_lossy().into_owned()),
        );
        let session = open_workspace_session(ws_path.to_str().unwrap()).unwrap();
        assert_eq!(session.active_repo, Some(repo_b.to_string_lossy().into_owned()));
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn open_workspace_session_falls_back_to_first_when_last_active_stale() {
        let dir = temp_dir();
        let repo_a = dir.join("a");
        Repository::init(&repo_a).unwrap();
        let ws_path = write_workspace(
            &dir,
            vec![repo_a.to_string_lossy().into_owned()],
            Some(dir.join("missing").to_string_lossy().into_owned()),
        );
        let session = open_workspace_session(ws_path.to_str().unwrap()).unwrap();
        assert_eq!(session.active_repo, Some(repo_a.to_string_lossy().into_owned()));
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn open_workspace_session_empty_repos_is_none() {
        let dir = temp_dir();
        let ws_path = write_workspace(&dir, vec![], None);
        let session = open_workspace_session(ws_path.to_str().unwrap()).unwrap();
        assert_eq!(session.active_repo, None);
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
