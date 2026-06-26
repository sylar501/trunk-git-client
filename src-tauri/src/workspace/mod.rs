//! Workspace mode plumbing (PRD §15): `.trunk` TOML files, Repository vs Workspace mode
//! resolution, nested-repo detection (§15.2), and the recent-items list (§15.1).
//!
//! Session 1 scaffold only — defines the data shapes; mode logic, nested-repo detection,
//! and the welcome-screen recent list land in the Welcome screen / Workspace mode sessions.

use serde::{Deserialize, Serialize};
use std::path::Path;

/// A `.trunk` workspace file (PRD §15.4.1): a TOML config grouping repository paths.
#[derive(Debug, Serialize, Deserialize)]
pub struct WorkspaceFile {
    pub name: String,
    /// Absolute paths, with relative-to-file resolution as a fallback when a path has moved.
    pub repositories: Vec<String>,
    pub last_active_repository: Option<String>,
    #[serde(default)]
    pub overrides: WorkspaceOverrides,
}

/// Per-workspace config overrides (PRD §14.2 tag version-group pattern, §19.2.2 author identity).
#[derive(Debug, Default, Serialize, Deserialize)]
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

pub fn open_repository(path: &str) -> Result<RepoHandle, std::io::Error> {
    let name = Path::new(path)
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.to_string());
    Ok(RepoHandle {
        path: path.to_string(),
        name,
    })
}

pub fn load_workspace(path: &str) -> Result<WorkspaceFile, std::io::Error> {
    let contents = std::fs::read_to_string(path)?;
    toml::from_str(&contents).map_err(std::io::Error::other)
}
