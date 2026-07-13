// src-tauri/src/pin/mod.rs

pub mod config;

use bcrypt::{hash, verify, DEFAULT_COST};
use lazy_static::lazy_static;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

/// Maximum failed PIN attempts before triggering a lockout.
pub const MAX_ATTEMPTS: u32 = 3;

/// Duration of the lockout window after `MAX_ATTEMPTS` consecutive failures.
pub const LOCKOUT_DURATION: u64 = 300; // seconds

/// Minimum and maximum allowed PIN lengths (digits only).
const MIN_PIN_LEN: usize = 6;
const MAX_PIN_LEN: usize = 32;

/// Runtime lockout state. Persisted to disk via `config` so it survives
/// app restarts — without disk persistence, an attacker could simply
/// relaunch the app to reset the counter.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct PinState {
    pub failed_attempts: u32,
    pub lockout_until_unix: Option<u64>,
}

impl PinState {
    fn new() -> Self {
        Self {
            failed_attempts: 0,
            lockout_until_unix: None,
        }
    }
}

lazy_static! {
    static ref PIN_STATE: Mutex<PinState> = Mutex::new(PinState::new());
}

/// Load lockout state from disk into the in-memory mutex. Call once at
/// app startup so a previously locked-out user cannot bypass the lockout
/// by relaunching the app.
pub fn init_lockout_state() -> Result<(), String> {
    let loaded = config::load_lockout_state()?;
    let mut guard = PIN_STATE
        .lock()
        .map_err(|e| format!("Lock poisoned: {}", e))?;
    *guard = PinState {
        failed_attempts: loaded.failed_attempts,
        lockout_until_unix: loaded.lockout_until_unix,
    };
    Ok(())
}

fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn validate_pin_format(pin: &str) -> Result<(), String> {
    if pin.len() < MIN_PIN_LEN || pin.len() > MAX_PIN_LEN {
        return Err(format!(
            "PIN must be between {} and {} characters",
            MIN_PIN_LEN, MAX_PIN_LEN
        ));
    }
    if !pin.chars().all(|c| c.is_ascii_digit()) {
        return Err("PIN must contain only digits".to_string());
    }
    Ok(())
}

fn persist_state(state: &PinState) -> Result<(), String> {
    config::save_lockout_state(&config::LockoutState {
        failed_attempts: state.failed_attempts,
        lockout_until_unix: state.lockout_until_unix,
    })
}

/// Configure a new PIN. Any existing PIN is replaced. Resets the
/// lockout counter on success.
pub fn setup_pin(pin: &str) -> Result<(), String> {
    validate_pin_format(pin)?;

    let hashed = hash(pin, DEFAULT_COST).map_err(|e| format!("Failed to hash PIN: {}", e))?;
    config::save_pin_hash(&hashed)?;

    // Successful setup clears any prior lockout state.
    let mut guard = PIN_STATE
        .lock()
        .map_err(|e| format!("Lock poisoned: {}", e))?;
    *guard = PinState::new();
    config::clear_lockout_state()?;

    Ok(())
}

/// Verify a PIN attempt. Returns:
/// - `Ok(true)` on correct PIN (also resets the failure counter)
/// - `Ok(false)` on incorrect PIN (also increments the counter and
///    may trigger a lockout)
/// - `Err(_)` if locked out, format invalid, or PIN not set
pub fn verify_pin(pin: &str) -> Result<bool, String> {
    // Lockout check runs BEFORE format validation to avoid leaking
    // information about valid PIN shape while locked.
    if let Some(remaining) = lockout_remaining()? {
        return Err(format!(
            "PIN entry is locked. Try again in {} seconds.",
            remaining
        ));
    }

    validate_pin_format(pin)?;

    let stored_hash = config::load_pin_hash()?;
    let stored_hash = match stored_hash {
        Some(h) => h,
        None => return Ok(true), // No PIN configured = nothing to verify
    };

    let valid = verify(pin, &stored_hash).map_err(|e| format!("PIN verification failed: {}", e))?;

    let mut guard = PIN_STATE
        .lock()
        .map_err(|e| format!("Lock poisoned: {}", e))?;

    if valid {
        guard.failed_attempts = 0;
        guard.lockout_until_unix = None;
        config::clear_lockout_state()?;
    } else {
        guard.failed_attempts = guard.failed_attempts.saturating_add(1);
        if guard.failed_attempts >= MAX_ATTEMPTS {
            guard.lockout_until_unix = Some(now_unix() + LOCKOUT_DURATION);
        }
        persist_state(&guard)?;
    }

    Ok(valid)
}

/// Returns `true` if PIN entry is currently blocked due to too many
/// recent failed attempts.
pub fn is_pin_locked() -> Result<bool, String> {
    Ok(lockout_remaining()?.is_some())
}

/// Seconds remaining on the current lockout, or `None` if not locked.
pub fn lockout_remaining() -> Result<Option<u64>, String> {
    let guard = PIN_STATE
        .lock()
        .map_err(|e| format!("Lock poisoned: {}", e))?;

    match guard.lockout_until_unix {
        None => Ok(None),
        Some(until) => {
            let now = now_unix();
            if until <= now {
                Ok(None)
            } else {
                Ok(Some(until - now))
            }
        }
    }
}

/// Change an existing PIN. Requires the current PIN to be valid
/// (and not in a lockout window).
pub fn change_pin(old_pin: &str, new_pin: &str) -> Result<(), String> {
    if !verify_pin(old_pin)? {
        return Err("Current PIN is incorrect".to_string());
    }
    setup_pin(new_pin)
}

/// Wipe ALL app state (PIN, lockout counter, config). Requires the
/// current PIN to be valid as a safety check before destruction.
pub fn reset_app(current_pin: &str) -> Result<(), String> {
    if !verify_pin(current_pin)? {
        return Err("Current PIN is incorrect".to_string());
    }

    config::remove_pin_hash()?;
    config::clear_lockout_state()?;

    let mut guard = PIN_STATE
        .lock()
        .map_err(|e| format!("Lock poisoned: {}", e))?;
    *guard = PinState::new();

    Ok(())
}

pub fn is_pin_enabled() -> bool {
    config::load_pin_hash().ok().flatten().is_some()
}

#[allow(dead_code)]
pub fn disable_pin() -> Result<(), String> {
    config::remove_pin_hash()?;
    config::clear_lockout_state()?;
    let mut guard = PIN_STATE
        .lock()
        .map_err(|e| format!("Lock poisoned: {}", e))?;
    *guard = PinState::new();
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    // NOTE: tests touch the on-disk config dir. `bcrypt` hashing is
    // intentionally slow (~100ms with DEFAULT_COST) so these tests
    // cover the lockout logic, not the cryptographic path.
    //
    // All PIN tests share the global `PIN_STATE` mutex and the user's
    // config dir, so the test runner must be invoked with
    // `--test-threads=1` to avoid races on shared state. The package.json
    // `test` script enforces this.
    //
    // IMPORTANT: every `let g = PIN_STATE.lock().unwrap();` MUST be
    // followed by either an explicit `drop(g)` or be scoped to a
    // block — otherwise the lock guard outlives subsequent calls into
    // helpers that try to re-acquire the same lock from the same
    // thread, which deadlocks on Windows.

    fn reset_state() {
        if let Ok(mut g) = PIN_STATE.lock() {
            *g = PinState::new();
        }
        let _ = config::clear_lockout_state();
        let _ = config::remove_pin_hash();
    }

    #[test]
    fn validate_pin_format_rejects_short() {
        assert!(validate_pin_format("12345").is_err());
    }

    #[test]
    fn validate_pin_format_rejects_long() {
        let too_long = "1".repeat(MAX_PIN_LEN + 1);
        assert!(validate_pin_format(&too_long).is_err());
    }

    #[test]
    fn validate_pin_format_rejects_non_digits() {
        assert!(validate_pin_format("12345a").is_err());
        assert!(validate_pin_format("abcdef").is_err());
        assert!(validate_pin_format("12 456").is_err());
    }

    #[test]
    fn validate_pin_format_accepts_valid_range() {
        assert!(validate_pin_format("123456").is_ok());
        assert!(validate_pin_format(&"1".repeat(MAX_PIN_LEN)).is_ok());
    }

    #[test]
    fn lockout_remaining_none_when_unlocked() {
        reset_state();
        assert_eq!(lockout_remaining().unwrap(), None);
        assert!(!is_pin_locked().unwrap());
    }

    #[test]
    fn setup_pin_resets_lockout_state() {
        reset_state();
        // Simulate prior failures
        {
            let mut g = PIN_STATE.lock().unwrap();
            g.failed_attempts = MAX_ATTEMPTS;
            g.lockout_until_unix = Some(now_unix() + 1000);
        }
        assert!(is_pin_locked().unwrap());

        setup_pin("123456").unwrap();
        assert!(!is_pin_locked().unwrap());
        assert_eq!(lockout_remaining().unwrap(), None);
    }

    #[test]
    fn verify_pin_blocks_when_locked() {
        reset_state();
        {
            let mut g = PIN_STATE.lock().unwrap();
            g.failed_attempts = MAX_ATTEMPTS;
            g.lockout_until_unix = Some(now_unix() + 60);
        }
        let result = verify_pin("123456");
        assert!(
            result.is_err(),
            "expected Err while locked, got {:?}",
            result
        );
    }

    #[test]
    fn lockout_expires_after_duration() {
        reset_state();
        {
            let mut g = PIN_STATE.lock().unwrap();
            g.failed_attempts = MAX_ATTEMPTS;
            g.lockout_until_unix = Some(now_unix().saturating_sub(1)); // already expired
        }
        assert!(!is_pin_locked().unwrap());
        assert_eq!(lockout_remaining().unwrap(), None);
    }

    #[test]
    fn lockout_state_persists_to_disk() {
        reset_state();
        {
            let mut g = PIN_STATE.lock().unwrap();
            g.failed_attempts = 2;
            g.lockout_until_unix = None;
            persist_state(&g).unwrap();
        }
        let loaded = config::load_lockout_state().unwrap();
        assert_eq!(loaded.failed_attempts, 2);
        assert!(loaded.lockout_until_unix.is_none());

        reset_state();
    }

    #[test]
    fn init_lockout_state_restores_from_disk() {
        reset_state();
        // Write a "locked" state directly to disk
        let locked = config::LockoutState {
            failed_attempts: MAX_ATTEMPTS,
            lockout_until_unix: Some(now_unix() + 120),
        };
        config::save_lockout_state(&locked).unwrap();

        // In-memory state should be fresh before init
        {
            let g = PIN_STATE.lock().unwrap();
            assert_eq!(g.failed_attempts, 0);
        }

        init_lockout_state().unwrap();

        {
            let g = PIN_STATE.lock().unwrap();
            assert_eq!(g.failed_attempts, MAX_ATTEMPTS);
            assert!(g.lockout_until_unix.is_some());
        }

        reset_state();
    }

    #[test]
    fn reset_app_requires_valid_pin() {
        reset_state();
        setup_pin("654321").unwrap();

        // Wrong PIN -> error, state preserved
        assert!(reset_app("000000").is_err());
        assert!(is_pin_enabled());

        // Correct PIN -> succeeds, clears state
        assert!(reset_app("654321").is_ok());
        assert!(!is_pin_enabled());

        reset_state();
    }

    #[test]
    fn change_pin_rejects_wrong_old_pin() {
        reset_state();
        setup_pin("111111").unwrap();

        assert!(change_pin("000000", "222222").is_err());
        // Original PIN still works
        assert_eq!(verify_pin("111111").unwrap(), true);
        assert_eq!(verify_pin("222222").unwrap(), false);

        reset_state();
    }

    #[test]
    fn change_pin_accepts_correct_old_pin() {
        reset_state();
        setup_pin("111111").unwrap();
        change_pin("111111", "222222").unwrap();

        assert_eq!(verify_pin("222222").unwrap(), true);
        assert_eq!(verify_pin("111111").unwrap(), false);

        reset_state();
    }
}
