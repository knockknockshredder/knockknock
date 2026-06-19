// src-tauri/src/commands/shred.rs

use crate::shredder::algorithms::all_algorithms;
use crate::shredder::progress::TauriProgressReporter;
use crate::shredder::types::*;
use crate::shredder::VerificationLevel;
use std::path::PathBuf;
use std::sync::Arc;
use tauri::AppHandle;

#[tauri::command]
pub async fn shred_files(
    app: AppHandle,
    paths: Vec<String>,
    algorithm_index: usize,
    passes: u32,
    pattern: PatternType,
    verification_level: VerificationLevel,
) -> Result<ShredReport, String> {
    let algorithms = all_algorithms();
    let algorithm = algorithms
        .get(algorithm_index)
        .ok_or_else(|| format!("Invalid algorithm index: {}", algorithm_index))?
        .clone();

    if passes > algorithm.max_passes() {
        return Err(format!("Passes {} exceeds maximum {}", passes, algorithm.max_passes()));
    }

    let progress: Arc<dyn crate::shredder::traits::ProgressReporter> =
        Arc::new(TauriProgressReporter::new(app));

    let path_bufs: Vec<PathBuf> = paths.into_iter().map(PathBuf::from).collect();

    let report = tokio::task::spawn_blocking(move || {
        crate::shredder::shred_files(path_bufs, algorithm, passes, pattern, verification_level, progress)
    })
    .await
    .map_err(|e| format!("Task failed: {}", e))?;

    Ok(report)
}

#[derive(serde::Serialize)]
pub struct AlgorithmInfo {
    pub index: usize,
    pub name: String,
    pub description: String,
    pub default_passes: u32,
    pub max_passes: u32,
    pub accepted_patterns: Vec<String>,
    pub has_fixed_pattern_sequence: bool,
}

#[tauri::command]
pub fn get_algorithms() -> Vec<AlgorithmInfo> {
    all_algorithms()
        .iter()
        .enumerate()
        .map(|(i, algo)| AlgorithmInfo {
            index: i,
            name: algo.name().to_string(),
            description: algo.description().to_string(),
            default_passes: algo.default_passes(),
            max_passes: algo.max_passes(),
            accepted_patterns: algo.accepted_patterns().iter().map(|p| format!("{:?}", p)).collect(),
            has_fixed_pattern_sequence: algo.has_fixed_pattern_sequence(),
        })
        .collect()
}
