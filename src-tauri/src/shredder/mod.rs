// src-tauri/src/shredder/mod.rs

pub mod algorithms;
pub mod errors;
pub mod platform;
pub mod progress;
pub mod traits;
pub mod types;
pub mod validation;
pub mod verification;

#[cfg(test)]
mod tests;

pub use errors::ShredError;
pub use traits::{PlatformIo, ProgressReporter, ShredAlgorithm, VerificationStrategy};
pub use types::*;
pub use verification::VerificationLevel;
