// src-tauri/src/shredder/algorithms/common.rs

use crate::shredder::errors::ShredError;
use crate::shredder::traits::ProgressReporter;
use crate::shredder::types::PatternType;
use crate::shredder::verification::PrngSeed;
use chacha20::cipher::{StreamCipher, StreamCipherSeek};
use chacha20::ChaCha20;
use std::fs::File;
use std::io::{Seek, SeekFrom, Write};
use std::path::PathBuf;

const PROGRESS_INTERVAL: u64 = 1024 * 1024; // Report every 1 MB

/// Write data to file with progress reporting.
///
/// For `PatternType::Random`, the buffer is filled from `ChaCha20(seed)` when a
/// seed is supplied (deterministic, verifiable) or from the OS CSPRNG
/// (`getrandom`) when no seed is available. For fixed patterns (Zeros, Ones)
/// the buffer is used as-is.
pub fn write_pass(
    file: &mut File,
    file_size: u64,
    pattern: PatternType,
    buffer: &mut [u8],
    progress: &dyn ProgressReporter,
    bytes_written_so_far: u64,
    total_bytes: u64,
    seed: Option<&PrngSeed>,
) -> Result<u64, ShredError> {
    file.seek(SeekFrom::Start(0))
        .map_err(|e| ShredError::from_io_error(PathBuf::from("<file>"), e))?;

    let mut written = 0u64;
    let mut remaining = file_size;
    let mut last_progress = 0u64;
    let mut cipher = seed.map(|s| s.cipher());

    while remaining > 0 {
        // Check cancellation between chunks (chunk size = buffer.len or remaining)
        if crate::shredder::cancel::is_cancelled_global() {
            return Err(ShredError::IoError {
                path: PathBuf::from("<file>"),
                kind: "Cancelled".to_string(),
                message: "Shredding cancelled during write".to_string(),
            });
        }

        let to_write = std::cmp::min(remaining, buffer.len() as u64) as usize;

        // Fill buffer based on pattern at absolute file position `written`.
        // For Random-with-seed this seeks the cipher to `written` (O(1)) and
        // runs the keystream into the buffer.
        fill_pattern_buffer(buffer, pattern, &mut cipher, written)?;

        file.write_all(&buffer[..to_write])
            .map_err(|e| ShredError::from_io_error(PathBuf::from("<file>"), e))?;
        written += to_write as u64;
        remaining -= to_write as u64;

        if written - last_progress >= PROGRESS_INTERVAL {
            progress.on_progress(bytes_written_so_far + written, total_bytes);
            last_progress = written;
        }
    }

    Ok(written)
}

/// Fill `buffer` with the configured pattern at keystream offset `pos`.
///
/// `pos` is the absolute byte offset within the file (and thus within the
/// ChaCha20 keystream). For Random-with-seed we use `try_seek` to jump
/// directly to that offset in O(1), avoiding skip buffers.
fn fill_pattern_buffer(
    buffer: &mut [u8],
    pattern: PatternType,
    cipher: &mut Option<ChaCha20>,
    pos: u64,
) -> Result<(), ShredError> {
    match pattern {
        PatternType::Zeros => buffer.fill(0x00),
        PatternType::Ones => buffer.fill(0xFF),
        PatternType::Random => {
            if let Some(cipher_ref) = cipher.as_mut() {
                cipher_ref.try_seek(pos).map_err(|e| ShredError::IoError {
                    path: PathBuf::from("<cipher>"),
                    kind: "CipherSeek".to_string(),
                    message: e.to_string(),
                })?;
                cipher_ref.apply_keystream(buffer);
            } else {
                getrandom::getrandom(buffer).map_err(|e| ShredError::IoError {
                    path: PathBuf::from("<random>"),
                    kind: "RandomGeneration".to_string(),
                    message: e.to_string(),
                })?;
            }
        }
    }
    Ok(())
}
