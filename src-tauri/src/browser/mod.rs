// src-tauri/src/browser/mod.rs

pub mod detection;
pub mod paths;
pub mod process;
pub mod types;

#[cfg(test)]
mod tests;

pub use types::*;
