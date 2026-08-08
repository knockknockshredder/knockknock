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
// mutex. Kept consistent with the cached token via begin_global_operation,
// cancel_global, and reset_global.
static CANCELLED: AtomicBool = AtomicBool::new(false);

pub fn begin_global_operation() -> CancellationToken {
    let mut guard = GLOBAL_TOKEN.lock().unwrap_or_else(|e| e.into_inner());
    let token = CancellationToken::new();
    *guard = Some(token.clone());
    CANCELLED.store(false, Ordering::Relaxed);
    token
}

pub fn get_global_token() -> CancellationToken {
    let mut guard = GLOBAL_TOKEN.lock().unwrap_or_else(|e| e.into_inner());
    if guard.is_none() {
        *guard = Some(CancellationToken::new());
    }
    guard.as_ref().unwrap().clone()
}

pub fn is_global_operation_cancelled() -> bool {
    get_global_token().is_cancelled()
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn acquiring_the_current_token_never_clears_stop() {
        let token = begin_global_operation();
        token.cancel();
        assert!(get_global_token().is_cancelled());
    }

    #[test]
    fn a_new_operation_gets_a_fresh_token() {
        let first = begin_global_operation();
        first.cancel();
        let second = begin_global_operation();
        assert!(!second.is_cancelled());
        assert!(
            first.is_cancelled(),
            "the old session remains stopped but is no longer global"
        );
        assert!(!get_global_token().is_cancelled());
    }

    #[test]
    fn operation_status_reports_the_current_shared_token_without_resetting_it() {
        let token = begin_global_operation();
        token.cancel();
        assert!(is_global_operation_cancelled());
        assert!(is_global_operation_cancelled());
    }
}
