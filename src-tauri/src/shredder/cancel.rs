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

pub fn get_global_token() -> Option<CancellationToken> {
    let guard = GLOBAL_TOKEN.lock().unwrap_or_else(|e| e.into_inner());
    guard.clone()
}

pub fn is_global_operation_cancelled() -> bool {
    let guard = GLOBAL_TOKEN.lock().unwrap_or_else(|e| e.into_inner());
    guard.as_ref().is_some_and(CancellationToken::is_cancelled)
}

pub fn cancel_global() {
    let guard = GLOBAL_TOKEN.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(token) = guard.as_ref() {
        token.cancel();
    }
    // Keep this write under the token lock. Otherwise a cancellation of the
    // previous session can set the compatibility flag after a new session has
    // installed its token and cleared the flag.
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
static GLOBAL_STATE_TEST_LOCK: Mutex<()> = Mutex::new(());

#[cfg(test)]
pub(crate) struct GlobalStateTestGuard {
    _lock: std::sync::MutexGuard<'static, ()>,
}

#[cfg(test)]
impl Drop for GlobalStateTestGuard {
    fn drop(&mut self) {
        clear_global_state_for_test();
    }
}

#[cfg(test)]
pub(crate) fn global_state_test_guard() -> GlobalStateTestGuard {
    let lock = GLOBAL_STATE_TEST_LOCK
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    clear_global_state_for_test();
    GlobalStateTestGuard { _lock: lock }
}

#[cfg(test)]
fn clear_global_state_for_test() {
    let mut guard = GLOBAL_TOKEN.lock().unwrap_or_else(|e| e.into_inner());
    *guard = None;
    CANCELLED.store(false, Ordering::Relaxed);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn acquiring_the_current_token_never_clears_stop() {
        let _state = global_state_test_guard();
        let token = begin_global_operation();
        token.cancel();
        assert!(get_global_token()
            .expect("operation session exists")
            .is_cancelled());
    }

    #[test]
    fn a_new_operation_gets_a_fresh_token() {
        let _state = global_state_test_guard();
        let first = begin_global_operation();
        first.cancel();
        let second = begin_global_operation();
        assert!(!second.is_cancelled());
        assert!(
            first.is_cancelled(),
            "the old session remains stopped but is no longer global"
        );
        assert!(!get_global_token()
            .expect("operation session exists")
            .is_cancelled());
    }

    #[test]
    fn operation_status_reports_the_current_shared_token_without_resetting_it() {
        let _state = global_state_test_guard();
        let token = begin_global_operation();
        token.cancel();
        assert!(is_global_operation_cancelled());
        assert!(is_global_operation_cancelled());
    }

    #[test]
    fn beginning_a_new_operation_clears_the_legacy_cancelled_flag() {
        let _state = global_state_test_guard();
        begin_global_operation();
        cancel_global();
        assert!(is_cancelled_global());

        let token = begin_global_operation();
        assert!(!token.is_cancelled());
        assert!(!is_cancelled_global());
    }

    #[test]
    fn querying_or_acquiring_without_a_session_does_not_create_one() {
        let _state = global_state_test_guard();
        assert!(get_global_token().is_none());
        assert!(!is_global_operation_cancelled());
        assert!(get_global_token().is_none());
    }
}
