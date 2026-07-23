// src-tauri/src/shredder/progress.rs

use crate::shredder::errors::ShredError;
use crate::shredder::logging::{obfuscate_path, LogObfuscation};
use crate::shredder::traits::ProgressReporter;
use crate::shredder::types::*;
use std::path::Path;
use std::sync::Mutex;
use std::time::Instant;
use tauri::{AppHandle, Emitter};

/// No-op progress reporter for tests and contexts without a Tauri `AppHandle`.
#[cfg(test)]
pub struct NoopProgressReporter;

#[cfg(test)]
impl ProgressReporter for NoopProgressReporter {
    fn on_file_start(&self, _path: &Path, _file_size: u64) {}
    fn on_pass_start(&self, _pass: u32, _total_passes: u32) {}
    fn on_progress(&self, _bytes_written: u64, _total: u64) {}
    fn on_pass_complete(&self, _pass: u32, _total_passes: u32) {}
    fn on_file_complete(&self, _path: &Path, _result: &ShredResult, _total_passes: u32) {}
    fn on_error(&self, _path: &Path, _error: &ShredError) {}
    fn on_warning(&self, _path: &Path, _message: &str) {}
}

/// Stateful Tauri progress reporter with throttling
pub struct TauriProgressReporter {
    app: AppHandle,
    obfuscation: LogObfuscation,
    state: Mutex<ReporterState>,
    throttle_ms: u64,
}

struct ReporterState {
    /// Index for numbered obfuscation; incremented at each `on_file_start`.
    file_index: u32,
    /// Display string (obfuscated if mode != None) of the file currently being processed.
    current_file: String,
    current_file_size: u64,
    current_pass: u32,
    total_passes: u32,
    pass_start: Instant,
    last_emit: Instant,
}

impl TauriProgressReporter {
    pub fn new(app: AppHandle, obfuscation: LogObfuscation) -> Self {
        Self {
            app,
            obfuscation,
            state: Mutex::new(ReporterState {
                file_index: 0,
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
        let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        let now = Instant::now();
        if now.duration_since(state.last_emit).as_millis() as u64 >= self.throttle_ms {
            state.last_emit = now;
            drop(state);
            self.emit_or_log(event);
        }
    }

    fn emit_or_log(&self, event: ProgressEvent) {
        if let Err(e) = self.app.emit("shred-progress", event) {
            eprintln!("[KnockKnock] Failed to emit progress event: {}", e);
        }
    }
}

impl ProgressReporter for TauriProgressReporter {
    fn on_file_start(&self, path: &Path, file_size: u64) {
        let obfuscation = self.obfuscation;
        let (current_file, file_size_out) = {
            let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
            state.file_index += 1;
            state.current_file = obfuscate_path(path, obfuscation, state.file_index as usize);
            state.current_file_size = file_size;
            (state.current_file.clone(), state.current_file_size)
        };

        self.emit_or_log(ProgressEvent {
            file_path: current_file,
            file_size: file_size_out,
            bytes_written: 0,
            current_pass: 0,
            total_passes: 0,
            speed_bytes_per_sec: 0,
            estimated_time_remaining_secs: 0,
            status: ShredStatus::Shredding,
        });
    }

    fn on_pass_start(&self, pass: u32, total_passes: u32) {
        let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        state.current_pass = pass;
        state.total_passes = total_passes;
        state.pass_start = Instant::now();
    }

    fn on_progress(&self, bytes_written: u64, total: u64) {
        let (file_path, file_size, current_pass, total_passes, pass_start) = {
            let state = self.state.lock().unwrap_or_else(|e| e.into_inner());
            (
                state.current_file.clone(),
                state.current_file_size,
                state.current_pass,
                state.total_passes,
                state.pass_start,
            )
        };
        // Lock dropped here

        let elapsed = pass_start.elapsed().as_secs_f64();
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
            file_path,
            file_size,
            bytes_written,
            current_pass,
            total_passes,
            speed_bytes_per_sec: speed,
            estimated_time_remaining_secs: remaining,
            status: ShredStatus::Shredding,
        });
    }

    fn on_pass_complete(&self, pass: u32, total_passes: u32) {
        let (file_path, file_size) = {
            let state = self.state.lock().unwrap_or_else(|e| e.into_inner());
            (state.current_file.clone(), state.current_file_size)
        };
        self.emit_or_log(ProgressEvent {
            file_path,
            file_size,
            bytes_written: 0,
            current_pass: pass,
            total_passes,
            speed_bytes_per_sec: 0,
            estimated_time_remaining_secs: 0,
            status: ShredStatus::Shredding,
        });
    }

    fn on_file_complete(&self, path: &Path, result: &ShredResult, total_passes: u32) {
        let obfuscation = self.obfuscation;
        let index = self
            .state
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .file_index as usize;
        let display = obfuscate_path(path, obfuscation, index);
        self.emit_or_log(ProgressEvent {
            file_path: display,
            file_size: 0,
            bytes_written: result.bytes_written,
            current_pass: result.passes_completed,
            total_passes,
            speed_bytes_per_sec: 0,
            estimated_time_remaining_secs: 0,
            status: ShredStatus::Complete,
        });
    }

    fn on_error(&self, path: &Path, error: &ShredError) {
        let obfuscation = self.obfuscation;
        let index = self
            .state
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .file_index as usize;
        let display = obfuscate_path(path, obfuscation, index);
        self.emit_or_log(ProgressEvent {
            file_path: display,
            file_size: 0,
            bytes_written: 0,
            current_pass: 0,
            total_passes: 0,
            speed_bytes_per_sec: 0,
            estimated_time_remaining_secs: 0,
            status: ShredStatus::Error {
                message: error.to_string(),
            },
        });
    }

    fn on_warning(&self, path: &Path, message: &str) {
        let obfuscation = self.obfuscation;
        let index = self
            .state
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .file_index as usize;
        let display = obfuscate_path(path, obfuscation, index);
        self.emit_or_log(ProgressEvent {
            file_path: display,
            file_size: 0,
            bytes_written: 0,
            current_pass: 0,
            total_passes: 0,
            speed_bytes_per_sec: 0,
            estimated_time_remaining_secs: 0,
            status: ShredStatus::Warning {
                message: message.to_string(),
            },
        });
    }
}
