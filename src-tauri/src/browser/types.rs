// src-tauri/src/browser/types.rs

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize)]
pub struct DetectedBrowser {
    pub name: String,
    pub profile_path: String,
    pub is_running: bool,
    pub data_types: Vec<BrowserDataType>,
    pub estimated_size_bytes: u64,
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
}
