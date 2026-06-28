//! App-level settings (not per-repo/workspace), persisted as a flat JSON file at the app
//! config dir — same storage shape as `recent.rs`, just a single object instead of a list.
//! Holds the drag-resizable panel widths (sidebar, commit-detail overlay, staging file list,
//! and any future one — see SPEC.md's "Resizable panels" note for the shared pattern these all
//! follow) plus whatever SPEC.md item 14 (Preferences) grows this same struct/file with later
//! — `#[serde(default)]` means an older settings file missing those future fields still loads
//! cleanly instead of failing.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tauri::{AppHandle, Manager};

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AppSettings {
    pub sidebar_width: f64,
    pub commit_overlay_width: f64,
    pub staging_files_width: f64,
    pub resolve_merged_height: f64,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            sidebar_width: 156.0,
            commit_overlay_width: 264.0,
            staging_files_width: 196.0,
            resolve_merged_height: 220.0,
        }
    }
}

fn settings_file_path(app: &AppHandle) -> Result<PathBuf, String> {
    let dir = app.path().app_config_dir().map_err(|e| e.to_string())?;
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir.join("settings.json"))
}

/// Falls back to `AppSettings::default()` on a missing or corrupt file rather than erroring —
/// a broken settings file should never block the app from starting.
pub fn load(app: &AppHandle) -> AppSettings {
    settings_file_path(app)
        .ok()
        .filter(|path| path.exists())
        .and_then(|path| std::fs::read_to_string(path).ok())
        .and_then(|contents| serde_json::from_str(&contents).ok())
        .unwrap_or_default()
}

/// Applies each `Some(..)` field onto the loaded `base`, leaving the others untouched — split
/// out from `save` so the merge logic is testable without an `AppHandle`/real filesystem.
fn merge(
    base: AppSettings,
    sidebar_width: Option<f64>,
    commit_overlay_width: Option<f64>,
    staging_files_width: Option<f64>,
    resolve_merged_height: Option<f64>,
) -> AppSettings {
    AppSettings {
        sidebar_width: sidebar_width.unwrap_or(base.sidebar_width),
        commit_overlay_width: commit_overlay_width.unwrap_or(base.commit_overlay_width),
        staging_files_width: staging_files_width.unwrap_or(base.staging_files_width),
        resolve_merged_height: resolve_merged_height.unwrap_or(base.resolve_merged_height),
    }
}

/// Read-modify-write: each call only needs to know the *one* field it's changing — a resize-end
/// on one panel's handle doesn't need to also know every other panel's current width just to
/// round it back unchanged.
pub fn save(
    app: &AppHandle,
    sidebar_width: Option<f64>,
    commit_overlay_width: Option<f64>,
    staging_files_width: Option<f64>,
    resolve_merged_height: Option<f64>,
) -> Result<(), String> {
    let merged = merge(load(app), sidebar_width, commit_overlay_width, staging_files_width, resolve_merged_height);
    let path = settings_file_path(app)?;
    let contents = serde_json::to_string_pretty(&merged).map_err(|e| e.to_string())?;
    std::fs::write(&path, contents).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn merge_overwrites_only_the_provided_field() {
        let base = AppSettings {
            sidebar_width: 156.0,
            commit_overlay_width: 264.0,
            staging_files_width: 196.0,
            resolve_merged_height: 220.0,
        };

        let sidebar_only = merge(base, Some(200.0), None, None, None);
        assert_eq!(sidebar_only.sidebar_width, 200.0);
        assert_eq!(sidebar_only.commit_overlay_width, 264.0);
        assert_eq!(sidebar_only.staging_files_width, 196.0);

        let overlay_only = merge(base, None, Some(320.0), None, None);
        assert_eq!(overlay_only.sidebar_width, 156.0);
        assert_eq!(overlay_only.commit_overlay_width, 320.0);

        let files_only = merge(base, None, None, Some(240.0), None);
        assert_eq!(files_only.staging_files_width, 240.0);
        assert_eq!(files_only.sidebar_width, 156.0);

        let merged_height_only = merge(base, None, None, None, Some(300.0));
        assert_eq!(merged_height_only.resolve_merged_height, 300.0);
        assert_eq!(merged_height_only.sidebar_width, 156.0);

        let neither = merge(base, None, None, None, None);
        assert_eq!(neither, base);
    }

    #[test]
    fn missing_fields_deserialize_to_defaults() {
        let parsed: AppSettings = serde_json::from_str("{}").unwrap();
        assert_eq!(parsed, AppSettings::default());

        let partial: AppSettings = serde_json::from_str(r#"{"sidebar_width": 200.0}"#).unwrap();
        assert_eq!(partial.sidebar_width, 200.0);
        assert_eq!(partial.commit_overlay_width, AppSettings::default().commit_overlay_width);
        assert_eq!(partial.staging_files_width, AppSettings::default().staging_files_width);
        assert_eq!(partial.resolve_merged_height, AppSettings::default().resolve_merged_height);
    }
}
