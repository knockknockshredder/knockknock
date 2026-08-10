// src-tauri/src/browser/types.rs

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BrowserProfile {
    pub id: String,
    pub name: String,
    pub path: String,
    pub size: u64,
    pub selected: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DetectedBrowser {
    pub id: String,
    pub name: String,
    pub icon: String,
    pub is_running: bool,
    pub profiles: Vec<BrowserProfile>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum BrowserDataType {
    Profile,
    Cache,
    Cookies,
    History,
    Passwords,
    Extensions,
}

/// Request to shred selected data from one browser profile.
///
/// The destructive DELETE confirmation is consent to destructive deletion.
/// It is never consent to deleting browser data while the browser is
/// running — the running-state gate has no override and is rechecked by the
/// backend immediately before collection.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrowserShredRequest {
    pub browser_name: String,
    pub profile_path: String,
    pub data_types: Vec<BrowserDataType>,
    /// Deletion policy (Phase 3): method + write check, same policy model as
    /// file shredding.
    pub method: crate::shredder::types::DeletionMethod,
    pub write_check: crate::shredder::types::WriteCheck,
}

/// Request to check the current running state of an already-discovered
/// browser. `profile_paths` are the known profile directories; no
/// installed-browser discovery or profile enumeration happens.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BrowserRunningCheck {
    pub browser_id: String,
    pub profile_paths: Vec<String>,
}

/// Current running state for one requested browser: `true` when any of its
/// known profiles currently holds a browser lock file.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BrowserRunningState {
    pub browser_id: String,
    pub is_running: bool,
}
