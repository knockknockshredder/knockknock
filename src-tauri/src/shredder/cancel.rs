// src-tauri/src/shredder/cancel.rs

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

#[derive(Clone)]
pub struct CancellationToken {
    cancelled: Arc<AtomicBool>,
}

impl CancellationToken {
    pub fn new() -> Self {
        Self {
            cancelled: Arc::new(AtomicBool::new(false)),
        }
    }
    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::Relaxed);
    }
    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Relaxed)
    }
}

static GLOBAL_TOKEN: Mutex<Option<CancellationToken>> = Mutex::new(None);
// Lock-free global flag: hot loops (write_pass) check this without taking the
// mutex. Kept consistent with the cached token via cancel_global / reset_global.
static CANCELLED: AtomicBool = AtomicBool::new(false);

pub fn get_global_token() -> CancellationToken {
    let mut guard = GLOBAL_TOKEN.lock().unwrap_or_else(|e| e.into_inner());
    if guard.is_none() {
        *guard = Some(CancellationToken::new());
    }
    guard.as_ref().unwrap().clone()
}

pub fn cancel_global() {
    let guard = GLOBAL_TOKEN.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(token) = guard.as_ref() {
        token.cancel();
    }
    drop(guard);
    CANCELLED.store(true, Ordering::Relaxed);
}

pub fn reset_global() {
    let mut guard = GLOBAL_TOKEN.lock().unwrap_or_else(|e| e.into_inner());
    *guard = Some(CancellationToken::new());
    CANCELLED.store(false, Ordering::Relaxed);
}

/// Lock-free cancellation check for hot paths.
pub fn is_cancelled_global() -> bool {
    CANCELLED.load(Ordering::Relaxed)
}
