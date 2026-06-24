//! Recursion-depth guard — a port of `cpp/support/recursion_guard.{h,cc}`.
//!
//! A process-global maximum depth (configurable, env-overridable) bounds recursion in
//! the parser and grammar functors; a per-thread counter tracks the live depth via an
//! RAII [`RecursionGuard`]. Recursion is always within a single thread, so the counter is
//! thread-local (each rayon worker gets its own).

use std::cell::Cell;
use std::sync::atomic::{AtomicI32, Ordering};

/// Default maximum recursion depth when unset and no env override is present.
pub const DEFAULT_MAX_RECURSION_DEPTH: i32 = 10_000;

/// Hard upper bound on any configured maximum.
pub const MAX_REASONABLE_RECURSION_DEPTH: i32 = 1_000_000;

const ENV_VAR: &str = "XGRAMMAR_MAX_RECURSION_DEPTH";

// 0 is a sentinel meaning "not yet initialized".
static MAX_RECURSION_DEPTH: AtomicI32 = AtomicI32::new(0);

thread_local! {
    static CURRENT_DEPTH: Cell<i32> = const { Cell::new(0) };
}

/// Error returned when an invalid maximum recursion depth is requested, or when the live
/// depth exceeds the configured maximum.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum RecursionError {
    /// A requested maximum was non-positive or exceeded [`MAX_REASONABLE_RECURSION_DEPTH`].
    #[error(
        "maximum recursion depth must be positive and at most {MAX_REASONABLE_RECURSION_DEPTH}, got {0}"
    )]
    InvalidMax(i32),
    /// The live recursion depth exceeded the configured maximum.
    #[error("maximum recursion depth exceeded: current depth {current}, max allowed {max}")]
    DepthExceeded {
        /// The depth reached when the limit was hit.
        current: i32,
        /// The configured maximum.
        max: i32,
    },
}

fn load_max_from_env() -> i32 {
    match std::env::var(ENV_VAR).ok().and_then(|v| v.parse::<i32>().ok()) {
        Some(v) if v > 0 && v <= MAX_REASONABLE_RECURSION_DEPTH => v,
        _ => DEFAULT_MAX_RECURSION_DEPTH,
    }
}

/// Returns the configured maximum recursion depth, initializing it from the environment
/// (or the default) on first access.
#[must_use]
pub fn get_max_recursion_depth() -> i32 {
    let current = MAX_RECURSION_DEPTH.load(Ordering::Relaxed);
    if current != 0 {
        return current;
    }
    let initial = load_max_from_env();
    MAX_RECURSION_DEPTH.store(initial, Ordering::Relaxed);
    initial
}

/// Sets the maximum recursion depth.
///
/// # Errors
/// Returns [`RecursionError::InvalidMax`] if `max_depth` is non-positive or exceeds
/// [`MAX_REASONABLE_RECURSION_DEPTH`].
pub fn set_max_recursion_depth(max_depth: i32) -> Result<(), RecursionError> {
    if max_depth <= 0 || max_depth > MAX_REASONABLE_RECURSION_DEPTH {
        return Err(RecursionError::InvalidMax(max_depth));
    }
    MAX_RECURSION_DEPTH.store(max_depth, Ordering::Relaxed);
    Ok(())
}

/// Resets the calling thread's live recursion depth to zero.
pub fn reset_recursion_depth() {
    CURRENT_DEPTH.with(|d| d.set(0));
}

/// RAII guard that increments the calling thread's recursion depth on creation and
/// decrements it on drop.
#[derive(Debug)]
#[must_use = "the guard must be held for the duration of the recursive call"]
pub struct RecursionGuard {
    // Not constructible outside `enter`; not `Send` (thread-local bound).
    _private: (),
}

impl RecursionGuard {
    /// Enters one level of recursion.
    ///
    /// # Errors
    /// Returns [`RecursionError::DepthExceeded`] if entering would exceed the configured
    /// maximum; the depth counter is left unchanged in that case.
    pub fn enter() -> Result<Self, RecursionError> {
        let max = get_max_recursion_depth();
        let current = CURRENT_DEPTH.with(|d| {
            let next = d.get() + 1;
            d.set(next);
            next
        });
        if current > max {
            CURRENT_DEPTH.with(|d| d.set(d.get() - 1));
            return Err(RecursionError::DepthExceeded { current, max });
        }
        Ok(Self { _private: () })
    }
}

impl Drop for RecursionGuard {
    fn drop(&mut self) {
        CURRENT_DEPTH.with(|d| d.set(d.get() - 1));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;

    #[test]
    #[serial]
    fn default_is_10000() {
        // Reset to the sentinel so initialization runs.
        MAX_RECURSION_DEPTH.store(0, Ordering::Relaxed);
        assert_eq!(get_max_recursion_depth(), DEFAULT_MAX_RECURSION_DEPTH);
    }

    #[test]
    #[serial]
    fn set_and_get() {
        set_max_recursion_depth(1234).unwrap();
        assert_eq!(get_max_recursion_depth(), 1234);
        set_max_recursion_depth(DEFAULT_MAX_RECURSION_DEPTH).unwrap();
    }

    #[test]
    #[serial]
    fn invalid_max_rejected() {
        assert_eq!(
            set_max_recursion_depth(-1),
            Err(RecursionError::InvalidMax(-1))
        );
        assert_eq!(
            set_max_recursion_depth(0),
            Err(RecursionError::InvalidMax(0))
        );
        assert_eq!(
            set_max_recursion_depth(100_000_000),
            Err(RecursionError::InvalidMax(100_000_000))
        );
    }

    #[test]
    #[serial]
    fn guard_increments_and_decrements() {
        reset_recursion_depth();
        set_max_recursion_depth(DEFAULT_MAX_RECURSION_DEPTH).unwrap();
        {
            let _g1 = RecursionGuard::enter().unwrap();
            let _g2 = RecursionGuard::enter().unwrap();
            assert_eq!(CURRENT_DEPTH.with(Cell::get), 2);
        }
        assert_eq!(CURRENT_DEPTH.with(Cell::get), 0);
    }

    #[test]
    #[serial]
    fn guard_errors_past_max() {
        reset_recursion_depth();
        set_max_recursion_depth(2).unwrap();
        let _g1 = RecursionGuard::enter().unwrap();
        let _g2 = RecursionGuard::enter().unwrap();
        let err = RecursionGuard::enter().unwrap_err();
        assert_eq!(err, RecursionError::DepthExceeded { current: 3, max: 2 });
        // The failed entry did not leave the counter incremented.
        assert_eq!(CURRENT_DEPTH.with(Cell::get), 2);
        drop((_g1, _g2));
        assert_eq!(CURRENT_DEPTH.with(Cell::get), 0);
        set_max_recursion_depth(DEFAULT_MAX_RECURSION_DEPTH).unwrap();
    }
}
