// src-tauri/src/shredder/algorithms/mod.rs

pub mod common;
pub mod dod_522022m;
pub mod nist_clear;
pub mod random_only;

use crate::shredder::traits::ShredAlgorithm;
use std::sync::Arc;

pub fn all_algorithms() -> Vec<Arc<dyn ShredAlgorithm>> {
    vec![
        Arc::new(nist_clear::NistClear),
        Arc::new(dod_522022m::Dod522022M),
        Arc::new(random_only::RandomOnly),
    ]
}
