//! App-level settings (not per-repo/workspace), persisted as a flat JSON file at the app
//! config dir — same storage shape as `recent.rs`, just a single object instead of a list.
//! Currently holds only the two drag-resizable panel widths introduced alongside the
//! commit-detail overlay (SPEC.md item 4 follow-up); SPEC.md item 14 (Preferences) is expected
//! to grow this same struct/file with its own fields later rather than introduce a second one —
//! `#[serde(default)]` means an older settings file missing those future fields still loads
//! cleanly instead of failing.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tauri::{AppHandle, Manager};

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AppSettings {
    pub sidebar_width: f64,
    pub commit_overlay_width: f64,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            sidebar_width: 156.0,
            commit_overlay_width: 264.0,
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

/// Applies `sidebar_width`/`commit_overlay_width` onto the loaded `base` wherever the caller
/// passed `Some(..)`, leaving the other field untouched — split out from `save` so the merge
/// logic is testable without an `AppHandle`/real filesystem.
fn merge(base: AppSettings, sidebar_width: Option<f64>, commit_overlay_width: Option<f64>) -> AppSettings {
    AppSettings {
        sidebar_width: sidebar_width.unwrap_or(base.sidebar_width),
        commit_overlay_width: commit_overlay_width.unwrap_or(base.commit_overlay_width),
    }
}

/// Read-modify-write: each call only needs to know the *one* field it's changing — a resize-end
/// on the sidebar handle doesn't need to also know the overlay's current width just to round
/// it back unchanged, and vice versa.
pub fn save(app: &AppHandle, sidebar_width: Option<f64>, commit_overlay_width: Option<f64>) -> Result<(), String> {
    let merged = merge(load(app), sidebar_width, commit_overlay_width);
    let path = settings_file_path(app)?;
    let contents = serde_json::to_string_pretty(&merged).map_err(|e| e.to_string())?;
    std::fs::write(&path, contents).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn merge_overwrites_only_the_provided_field() {
        let base = AppSettings { sidebar_width: 156.0, commit_overlay_width: 264.0 };

        let sidebar_only = merge(base, Some(200.0), None);
        assert_eq!(sidebar_only.sidebar_width, 200.0);
        assert_eq!(sidebar_only.commit_overlay_width, 264.0);

        let overlay_only = merge(base, None, Some(320.0));
        assert_eq!(overlay_only.sidebar_width, 156.0);
        assert_eq!(overlay_only.commit_overlay_width, 320.0);

        let neither = merge(base, None, None);
        assert_eq!(neither, base);
    }

    #[test]
    fn missing_fields_deserialize_to_defaults() {
        let parsed: AppSettings = serde_json::from_str("{}").unwrap();
        assert_eq!(parsed, AppSettings::default());

        let partial: AppSettings = serde_json::from_str(r#"{"sidebar_width": 200.0}"#).unwrap();
        assert_eq!(partial.sidebar_width, 200.0);
        assert_eq!(partial.commit_overlay_width, AppSettings::default().commit_overlay_width);
    }
}
