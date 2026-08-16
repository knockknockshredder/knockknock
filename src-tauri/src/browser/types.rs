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
    pub running_state: BrowserRunningStatus,
    pub profiles: Vec<BrowserProfile>,
}

/// Whether KnockKnock can safely permit cleanup of a browser's local profile
/// data. Only `Closed` permits cleanup; `Running` and `Unknown` both block it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BrowserRunningStatus {
    Closed,
    Running,
    Unknown,
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
    /// Stable configured browser identity. The backend validates this against
    /// the configured browser table before applying a running-state policy.
    pub browser_id: String,
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

/// Current running state for one requested browser. `Unknown` is deliberately
/// distinct from `Closed` so callers cannot accidentally permit cleanup when
/// the configured browser has no reliable running-state policy or inspection
/// fails.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BrowserRunningState {
    pub browser_id: String,
    pub state: BrowserRunningStatus,
}
