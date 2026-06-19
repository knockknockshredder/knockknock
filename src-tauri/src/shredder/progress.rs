// src-tauri/src/shredder/progress.rs

use crate::shredder::errors::ShredError;
use crate::shredder::traits::ProgressReporter;
use crate::shredder::types::*;
use std::path::Path;
use std::sync::Mutex;
use std::time::Instant;
use tauri::{AppHandle, Emitter};

/// Stateful Tauri progress reporter with throttling
pub struct TauriProgressReporter {
    app: AppHandle,
    state: Mutex<ReporterState>,
    throttle_ms: u64,
}

struct ReporterState {
    current_file: String,
    current_file_size: u64,
    current_pass: u32,
    total_passes: u32,
    pass_start: Instant,
    last_emit: Instant,
}

impl TauriProgressReporter {
    pub fn new(app: AppHandle) -> Self {
        Self {
            app,
            state: Mutex::new(ReporterState {
                current_file: String::new(),
                current_file_size: 0,
                current_pass: 0,
                total_passes: 0,
                pass_start: Instant::now(),
                last_emit: Instant::now(),
            }),
            throttle_ms: 100,
        }
    }

    fn emit_throttled(&self, event: ProgressEvent) {
        let mut state = self.state.lock().unwrap();
        let now = Instant::now();
        if now.duration_since(state.last_emit).as_millis() as u64 >= self.throttle_ms {
            state.last_emit = now;
            drop(state);
            let _ = self.app.emit("shred-progress", event);
        }
    }
}

impl ProgressReporter for TauriProgressReporter {
    fn on_file_start(&self, path: &Path, file_size: u64) {
        let mut state = self.state.lock().unwrap();
        state.current_file = path.to_string_lossy().to_string();
        state.current_file_size = file_size;
        drop(state);

        let _ = self.app.emit(
            "shred-progress",
            ProgressEvent {
                file_path: path.to_string_lossy().to_string(),
                file_size,
                bytes_written: 0,
                current_pass: 0,
                total_passes: 0,
                speed_bytes_per_sec: 0,
                estimated_time_remaining_secs: 0,
                status: ShredStatus::Shredding,
            },
        );
    }

    fn on_pass_start(&self, pass: u32, total_passes: u32) {
        let mut state = self.state.lock().unwrap();
        state.current_pass = pass;
        state.total_passes = total_passes;
        state.pass_start = Instant::now();
    }

    fn on_progress(&self, bytes_written: u64, total: u64) {
        let state = self.state.lock().unwrap();
        let elapsed = state.pass_start.elapsed().as_secs_f64();
        let speed = if elapsed > 0.0 {
            (bytes_written as f64 / elapsed) as u64
        } else {
            0
        };
        let remaining = if speed > 0 {
            (total - bytes_written) / speed
        } else {
            0
        };

        self.emit_throttled(ProgressEvent {
            file_path: state.current_file.clone(),
            file_size: state.current_file_size,
            bytes_written,
            current_pass: state.current_pass,
            total_passes: state.total_passes,
            speed_bytes_per_sec: speed,
            estimated_time_remaining_secs: remaining,
            status: ShredStatus::Shredding,
        });
    }

    fn on_pass_complete(&self, pass: u32, total_passes: u32) {
        // Emit pass complete (not throttled)
    }

    fn on_file_complete(&self, path: &Path, result: &ShredResult) {
        let _ = self.app.emit(
            "shred-progress",
            ProgressEvent {
                file_path: path.to_string_lossy().to_string(),
                file_size: 0,
                bytes_written: result.bytes_written,
                current_pass: result.passes_completed,
                total_passes: result.passes_completed,
                speed_bytes_per_sec: 0,
                estimated_time_remaining_secs: 0,
                status: ShredStatus::Complete,
            },
        );
    }

    fn on_error(&self, path: &Path, error: &ShredError) {
        let _ = self.app.emit(
            "shred-progress",
            ProgressEvent {
                file_path: path.to_string_lossy().to_string(),
                file_size: 0,
                bytes_written: 0,
                current_pass: 0,
                total_passes: 0,
                speed_bytes_per_sec: 0,
                estimated_time_remaining_secs: 0,
                status: ShredStatus::Error {
                    message: error.to_string(),
                },
            },
        );
    }

    fn on_warning(&self, path: &Path, message: &str) {
        let _ = self.app.emit(
            "shred-progress",
            ProgressEvent {
                file_path: path.to_string_lossy().to_string(),
                file_size: 0,
                bytes_written: 0,
                current_pass: 0,
                total_passes: 0,
                speed_bytes_per_sec: 0,
                estimated_time_remaining_secs: 0,
                status: ShredStatus::Error {
                    message: format!("Warning: {}", message),
                },
            },
        );
    }
}

/// No-op reporter for testing
pub struct NoopProgressReporter;

impl ProgressReporter for NoopProgressReporter {
    fn on_file_start(&self, _: &Path, _: u64) {}
    fn on_pass_start(&self, _: u32, _: u32) {}
    fn on_progress(&self, _: u64, _: u64) {}
    fn on_pass_complete(&self, _: u32, _: u32) {}
    fn on_file_complete(&self, _: &Path, _: &ShredResult) {}
    fn on_error(&self, _: &Path, _: &ShredError) {}
    fn on_warning(&self, _: &Path, _: &str) {}
}
