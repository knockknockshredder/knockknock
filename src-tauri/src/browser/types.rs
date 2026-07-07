// src-tauri/src/browser/types.rs

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize)]
pub struct BrowserProfile {
    pub id: String,
    pub name: String,
    pub path: String,
    pub size: u64,
    pub selected: bool,
}

#[derive(Debug, Clone, Serialize)]
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
    pub algorithm_index: usize,
    pub passes: u32,
    pub pattern: crate::shredder::PatternType,
    pub verification_level: crate::shredder::VerificationLevel,
}
