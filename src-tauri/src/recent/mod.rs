//! Welcome-screen recent-items list (PRD §15.1): cross-cutting because an entry may be
//! either a standalone repository or a workspace, so it doesn't belong inside `workspace`.
//! Persisted as a flat JSON array at the app config dir, independent of any single `.trunk`.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use tauri::{AppHandle, Manager};

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RecentKind {
    Repository,
    Workspace,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecentEntry {
    pub path: String,
    pub kind: RecentKind,
    pub last_opened: u64,
}

/// What the frontend actually receives: a `RecentEntry` plus a freshly-computed `stale` flag.
#[derive(Debug, Clone, Serialize)]
pub struct RecentEntryView {
    pub path: String,
    pub kind: RecentKind,
    pub last_opened: u64,
    pub stale: bool,
}

fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn recent_file_path(app: &AppHandle) -> Result<PathBuf, String> {
    let dir = app.path().app_config_dir().map_err(|e| e.to_string())?;
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir.join("recent.json"))
}

fn read_all(app: &AppHandle) -> Result<Vec<RecentEntry>, String> {
    let path = recent_file_path(app)?;
    if !path.exists() {
        return Ok(Vec::new());
    }
    let contents = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
    serde_json::from_str(&contents).map_err(|e| e.to_string())
}

fn write_all(app: &AppHandle, entries: &[RecentEntry]) -> Result<(), String> {
    let path = recent_file_path(app)?;
    let contents = serde_json::to_string_pretty(entries).map_err(|e| e.to_string())?;
    std::fs::write(&path, contents).map_err(|e| e.to_string())
}

/// Upsert by path and bump its `last_opened` timestamp to now.
pub fn record(app: &AppHandle, path: &str, kind: RecentKind) -> Result<(), String> {
    let mut entries = read_all(app)?;
    if let Some(existing) = entries.iter_mut().find(|e| e.path == path) {
        existing.kind = kind;
        existing.last_opened = now_secs();
    } else {
        entries.push(RecentEntry {
            path: path.to_string(),
            kind,
            last_opened: now_secs(),
        });
    }
    write_all(app, &entries)
}

/// All recent entries, newest first, with staleness computed against the current filesystem.
pub fn list(app: &AppHandle) -> Result<Vec<RecentEntryView>, String> {
    let mut entries = read_all(app)?;
    entries.sort_by(|a, b| b.last_opened.cmp(&a.last_opened));
    Ok(entries
        .into_iter()
        .map(|e| RecentEntryView {
            stale: !Path::new(&e.path).exists(),
            path: e.path,
            kind: e.kind,
            last_opened: e.last_opened,
        })
        .collect())
}

pub fn remove(app: &AppHandle, path: &str) -> Result<(), String> {
    let mut entries = read_all(app)?;
    entries.retain(|e| e.path != path);
    write_all(app, &entries)
}
