//! Local failure delay after wrong passwords (docs/07 §4).
//!
//! **Cosmetic by design:** it slows a person at the keyboard, not an attacker, who can run
//! Argon2 offline against a copy of the header. State lives in memory only and resets when the
//! process restarts. The policy never sleeps: it reports the remaining wait and the caller
//! (UI) shows it.

use std::time::Instant;

use crate::error::SessionError;

/// A monotonic millisecond clock, injectable so tests need not wait.
pub trait Clock: Send {
    /// Milliseconds since an arbitrary fixed point; never goes backwards.
    fn now_ms(&self) -> u64;
}

/// The real clock ([`Instant`]-based).
#[derive(Debug)]
pub struct MonotonicClock {
    start: Instant,
}

impl MonotonicClock {
    /// A clock that starts counting now.
    #[must_use]
    pub fn new() -> Self {
        Self {
            start: Instant::now(),
        }
    }
}

impl Default for MonotonicClock {
    fn default() -> Self {
        Self::new()
    }
}

impl Clock for MonotonicClock {
    fn now_ms(&self) -> u64 {
        u64::try_from(self.start.elapsed().as_millis()).unwrap_or(u64::MAX)
    }
}

/// The delay schedule: `free_attempts` wrong passwords cost nothing; the failure after that
/// blocks for `base_ms`, and each further failure doubles the block, capped at `max_ms`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BackoffPolicy {
    /// Consecutive wrong passwords tolerated before the first delay starts.
    pub free_attempts: u32,
    /// Delay after the first failure past the free attempts.
    pub base_ms: u64,
    /// Upper bound of any single delay.
    pub max_ms: u64,
}

impl Default for BackoffPolicy {
    /// Five free attempts (typos are normal), then 1 s, 2 s, 4 s ... capped at 60 s.
    fn default() -> Self {
        Self {
            free_attempts: 5,
            base_ms: 1_000,
            max_ms: 60_000,
        }
    }
}

impl BackoffPolicy {
    /// The block that starts after the `failures`-th consecutive wrong password.
    #[must_use]
    pub fn delay_after(&self, failures: u32) -> u64 {
        if failures < self.free_attempts {
            return 0;
        }
        let exp = (failures - self.free_attempts).min(32);
        self.base_ms.saturating_mul(1u64 << exp).min(self.max_ms)
    }
}

/// The running counter behind [`BackoffPolicy`].
pub struct FailureBackoff {
    policy: BackoffPolicy,
    clock: Box<dyn Clock>,
    failures: u32,
    blocked_until_ms: u64,
}

impl core::fmt::Debug for FailureBackoff {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("FailureBackoff")
            .field("failures", &self.failures)
            .finish_non_exhaustive()
    }
}

impl FailureBackoff {
    /// A fresh counter.
    #[must_use]
    pub fn new(policy: BackoffPolicy, clock: Box<dyn Clock>) -> Self {
        Self {
            policy,
            clock,
            failures: 0,
            blocked_until_ms: 0,
        }
    }

    /// Consecutive wrong passwords since the last success.
    #[must_use]
    pub fn failures(&self) -> u32 {
        self.failures
    }

    /// `Err(Backoff)` while a delay is running; the attempt must not be tried at all.
    ///
    /// # Errors
    /// [`SessionError::Backoff`] with the remaining wait.
    pub fn check(&self) -> Result<(), SessionError> {
        let now = self.clock.now_ms();
        if now < self.blocked_until_ms {
            return Err(SessionError::Backoff {
                retry_after_ms: self.blocked_until_ms - now,
            });
        }
        Ok(())
    }

    /// Records a wrong password and starts the next delay.
    pub fn record_failure(&mut self) {
        self.failures = self.failures.saturating_add(1);
        let delay = self.policy.delay_after(self.failures);
        self.blocked_until_ms = self.clock.now_ms().saturating_add(delay);
    }

    /// Records a correct password: the counter and any delay reset.
    pub fn record_success(&mut self) {
        self.failures = 0;
        self.blocked_until_ms = 0;
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicU64, Ordering};

    use super::*;

    /// A clock the test advances by hand.
    #[derive(Clone, Default)]
    pub(crate) struct FakeClock(pub(crate) Arc<AtomicU64>);

    impl FakeClock {
        pub(crate) fn advance(&self, ms: u64) {
            self.0.fetch_add(ms, Ordering::SeqCst);
        }
    }

    impl Clock for FakeClock {
        fn now_ms(&self) -> u64 {
            self.0.load(Ordering::SeqCst)
        }
    }

    fn policy() -> BackoffPolicy {
        BackoffPolicy {
            free_attempts: 2,
            base_ms: 1_000,
            max_ms: 5_000,
        }
    }

    #[test]
    fn schedule_is_free_then_doubling_then_capped() {
        let p = policy();
        let d: Vec<u64> = (0..8).map(|n| p.delay_after(n)).collect();
        assert_eq!(d, [0, 0, 1_000, 2_000, 4_000, 5_000, 5_000, 5_000]);
        assert_eq!(p.delay_after(u32::MAX), 5_000, "no overflow");
    }

    #[test]
    fn blocks_for_the_remaining_time_and_never_sleeps() {
        let clock = FakeClock::default();
        let mut b = FailureBackoff::new(policy(), Box::new(clock.clone()));
        b.check().unwrap();
        b.record_failure();
        b.record_failure();
        // the second failure starts a 1 s block
        let e = b.check().unwrap_err();
        assert_eq!(e.retry_after_ms(), Some(1_000));
        clock.advance(400);
        assert_eq!(b.check().unwrap_err().retry_after_ms(), Some(600));
        clock.advance(600);
        b.check().unwrap();
        // the next failure doubles it
        b.record_failure();
        assert_eq!(b.check().unwrap_err().retry_after_ms(), Some(2_000));
        assert_eq!(b.failures(), 3);
    }

    #[test]
    fn success_resets_counter_and_delay() {
        let clock = FakeClock::default();
        let mut b = FailureBackoff::new(policy(), Box::new(clock.clone()));
        for _ in 0..4 {
            b.record_failure();
        }
        assert!(b.check().is_err());
        b.record_success();
        b.check().unwrap();
        assert_eq!(b.failures(), 0);
        b.record_failure();
        b.check().unwrap();
    }

    #[test]
    fn monotonic_clock_does_not_go_backwards() {
        let c = MonotonicClock::default();
        let a = c.now_ms();
        let b = c.now_ms();
        assert!(b >= a);
    }
}
