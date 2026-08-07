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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrowserShredRequest {
    pub browser_name: String,
    pub profile_path: String,
    pub data_types: Vec<BrowserDataType>,
    /// Deletion policy (Phase 3): method + write check, same policy model as
    /// file shredding. `explicit_consent` reflects the actual confirmation
    /// dialog state (M10) — it is never hardcoded `true` by the backend.
    pub method: crate::shredder::types::DeletionMethod,
    pub write_check: crate::shredder::types::WriteCheck,
    pub explicit_consent: bool,
}
