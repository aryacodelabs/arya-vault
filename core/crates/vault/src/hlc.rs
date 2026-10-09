//! Hybrid logical clock (docs/06 section 4).
//!
//! `Hlc` is a 48-bit physical time in milliseconds plus a 16-bit counter. It
//! lives in this crate and has no dependency on storage. Packing to `i64` for
//! SQLite is `(pt << 16) | counter`; to stay non-negative, `pt` is limited to
//! 47 bits (`MAX_PT`, about the year 6429), far beyond the one-year skew bound.

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use thiserror::Error;

/// Largest physical time (ms) representable in a non-negative packed `i64`.
pub const MAX_PT: u64 = (1 << 47) - 1;
/// A remote `pt` more than this far ahead of the local clock is flagged.
pub const SKEW_FLAG_MS: u64 = 24 * 60 * 60 * 1000;
/// A remote `pt` more than this far ahead is corrupt (segment quarantined).
pub const SKEW_CORRUPT_MS: u64 = 365 * 24 * 60 * 60 * 1000;

/// Clock errors.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum HlcError {
    /// A remote timestamp is more than one year ahead of the local clock.
    #[error("remote clock is {ahead_ms} ms ahead (more than one year): corrupt")]
    CorruptClock {
        /// How far ahead of local wall time, in milliseconds.
        ahead_ms: u64,
    },
    /// A value does not fit the packed representation.
    #[error("timestamp out of range")]
    OutOfRange,
}

/// A hybrid logical clock timestamp. Ordering is `(pt, counter)`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct Hlc {
    pt: u64,
    counter: u16,
}

impl Hlc {
    /// The zero timestamp.
    pub const ZERO: Hlc = Hlc { pt: 0, counter: 0 };

    /// Build a timestamp.
    ///
    /// # Errors
    /// [`HlcError::OutOfRange`] if `pt > MAX_PT`.
    pub fn new(pt: u64, counter: u16) -> Result<Self, HlcError> {
        if pt > MAX_PT {
            Err(HlcError::OutOfRange)
        } else {
            Ok(Self { pt, counter })
        }
    }

    /// Physical component (milliseconds since the Unix epoch, as seen by the author).
    #[must_use]
    pub fn pt(self) -> u64 {
        self.pt
    }

    /// Logical counter.
    #[must_use]
    pub fn counter(self) -> u16 {
        self.counter
    }

    /// Pack into a non-negative `i64` (`pt << 16 | counter`), order-preserving.
    #[must_use]
    pub fn to_i64(self) -> i64 {
        // pt <= MAX_PT (47 bits) so the shifted value fits in 63 bits.
        i64::try_from((self.pt << 16) | u64::from(self.counter)).unwrap_or(i64::MAX)
    }

    /// Unpack from a stored `i64`.
    ///
    /// # Errors
    /// [`HlcError::OutOfRange`] for negative values.
    pub fn from_i64(v: i64) -> Result<Self, HlcError> {
        let v = u64::try_from(v).map_err(|_| HlcError::OutOfRange)?;
        Ok(Self {
            pt: v >> 16,
            counter: (v & 0xFFFF) as u16,
        })
    }

    /// Successor with counter overflow handled: when the counter is exhausted the
    /// physical part advances by 1 ms (so ordering stays strict).
    fn next_counter(pt: u64, counter: u16) -> Result<Self, HlcError> {
        match counter.checked_add(1) {
            Some(c) => Self::new(pt, c),
            None => Self::new(pt + 1, 0),
        }
    }
}

/// Source of wall-clock time in milliseconds.
pub trait Clock: Send {
    /// Milliseconds since the Unix epoch.
    fn now_ms(&self) -> u64;
}

/// The system clock.
#[derive(Debug, Default, Clone, Copy)]
pub struct SystemClock;

impl Clock for SystemClock {
    fn now_ms(&self) -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| u64::try_from(d.as_millis()).unwrap_or(u64::MAX))
    }
}

/// A manually driven clock for tests; clones share the same time.
#[derive(Debug, Clone, Default)]
pub struct ManualClock(Arc<AtomicU64>);

impl ManualClock {
    /// Start at `ms`.
    #[must_use]
    pub fn new(ms: u64) -> Self {
        Self(Arc::new(AtomicU64::new(ms)))
    }
    /// Set the time (may go backwards).
    pub fn set(&self, ms: u64) {
        self.0.store(ms, Ordering::SeqCst);
    }
    /// Advance by `ms`.
    pub fn advance(&self, ms: u64) {
        self.0.fetch_add(ms, Ordering::SeqCst);
    }
}

impl Clock for ManualClock {
    fn now_ms(&self) -> u64 {
        self.0.load(Ordering::SeqCst)
    }
}

/// Whether a remote timestamp was suspiciously far ahead.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Skew {
    /// Within bounds.
    None,
    /// More than 24 h ahead of the local clock: applied, but flag it to the user.
    Ahead {
        /// How far ahead, in milliseconds.
        by_ms: u64,
    },
}

/// A hybrid logical clock bound to a wall-clock source.
pub struct HlcClock {
    last: Hlc,
    clock: Box<dyn Clock>,
}

impl core::fmt::Debug for HlcClock {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("HlcClock")
            .field("last", &self.last)
            .finish_non_exhaustive()
    }
}

impl HlcClock {
    /// Create a clock that resumes after `last` (persisted state).
    #[must_use]
    pub fn new(clock: Box<dyn Clock>, last: Hlc) -> Self {
        Self { last, clock }
    }

    /// The most recent timestamp issued or observed.
    #[must_use]
    pub fn last(&self) -> Hlc {
        self.last
    }

    /// Wall-clock milliseconds from the underlying source.
    #[must_use]
    pub fn wall_ms(&self) -> u64 {
        self.clock.now_ms()
    }

    /// Timestamp for a local event: `pt = max(wall, last.pt)`, counter
    /// incremented when `pt` did not advance. Strictly greater than every
    /// timestamp issued or observed before, even if the wall clock goes back.
    ///
    /// # Errors
    /// [`HlcError::OutOfRange`] if the wall clock exceeds the representable range.
    pub fn now(&mut self) -> Result<Hlc, HlcError> {
        let wall = self.clock.now_ms();
        let next = if wall > self.last.pt {
            Hlc::new(wall, 0)?
        } else {
            Hlc::next_counter(self.last.pt, self.last.counter)?
        };
        self.last = next;
        Ok(next)
    }

    /// Like [`HlcClock::now`] but the result is also strictly greater than `floor`
    /// (used so a mutation always outranks what is already stored). `floor` is
    /// adopted without skew checks: it came from our own database.
    ///
    /// # Errors
    /// [`HlcError::OutOfRange`].
    pub fn now_after(&mut self, floor: Hlc) -> Result<Hlc, HlcError> {
        if floor > self.last {
            self.last = floor;
        }
        self.now()
    }

    /// Receive rule for a remote timestamp: **always adopts** the remote `pt`
    /// (docs/06 section 4). Returns the new local timestamp and the skew flag.
    ///
    /// # Errors
    /// [`HlcError::CorruptClock`] if `remote.pt` is more than a year ahead of
    /// the local wall clock; the clock is left unchanged.
    pub fn observe(&mut self, remote: Hlc) -> Result<(Hlc, Skew), HlcError> {
        let wall = self.clock.now_ms();
        let ahead = remote.pt.saturating_sub(wall);
        if ahead > SKEW_CORRUPT_MS {
            return Err(HlcError::CorruptClock { ahead_ms: ahead });
        }
        let skew = if ahead > SKEW_FLAG_MS {
            Skew::Ahead { by_ms: ahead }
        } else {
            Skew::None
        };
        let pt = wall.max(self.last.pt).max(remote.pt);
        let next = if pt == self.last.pt && pt == remote.pt {
            Hlc::next_counter(pt, self.last.counter.max(remote.counter))?
        } else if pt == self.last.pt {
            Hlc::next_counter(pt, self.last.counter)?
        } else if pt == remote.pt {
            Hlc::next_counter(pt, remote.counter)?
        } else {
            Hlc::new(pt, 0)?
        };
        self.last = next;
        Ok((next, skew))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    fn h(pt: u64, c: u16) -> Hlc {
        Hlc::new(pt, c).unwrap()
    }
    fn clock(ms: u64) -> (HlcClock, ManualClock) {
        let m = ManualClock::new(ms);
        (HlcClock::new(Box::new(m.clone()), Hlc::ZERO), m)
    }

    #[test]
    fn local_events_are_strictly_monotonic_even_when_wall_clock_goes_back() {
        let (mut c, m) = clock(1_000);
        let a = c.now().unwrap();
        let b = c.now().unwrap();
        assert_eq!((a, b), (h(1_000, 0), h(1_000, 1)));
        m.set(500); // clock set backwards
        let d = c.now().unwrap();
        assert!(d > b);
        assert_eq!(d.pt(), 1_000);
        m.set(2_000);
        assert_eq!(c.now().unwrap(), h(2_000, 0));
    }

    #[test]
    fn counter_overflow_advances_physical_time() {
        let (mut c, _m) = clock(10);
        c.last = h(10, u16::MAX);
        let n = c.now().unwrap();
        assert_eq!(n, h(11, 0));
        assert!(n > h(10, u16::MAX));
        let mut c2 = HlcClock::new(Box::new(ManualClock::new(10)), h(10, u16::MAX));
        let (o, _) = c2.observe(h(10, u16::MAX)).unwrap();
        assert_eq!(o, h(11, 0));
    }

    #[test]
    fn observe_always_adopts_remote_pt_and_outranks_both() {
        let (mut c, _m) = clock(1_000);
        let l = c.now().unwrap();
        let r = h(1_000 + 3_600_000, 7); // one hour ahead
        let (o, skew) = c.observe(r).unwrap();
        assert_eq!(skew, Skew::None);
        assert!(o > l && o > r);
        assert_eq!(o, h(r.pt(), 8));
        // A later local edit outranks the future-dated remote op (the point of always adopting).
        let e = c.now().unwrap();
        assert!(e > r);
    }

    #[test]
    fn observe_counter_rules() {
        let (mut c, _m) = clock(100);
        c.last = h(200, 5);
        assert_eq!(c.observe(h(200, 9)).unwrap().0, h(200, 10)); // both equal pt: max+1
        c.last = h(200, 5);
        assert_eq!(c.observe(h(150, 99)).unwrap().0, h(200, 6)); // local ahead
        c.last = h(100, 5);
        assert_eq!(c.observe(h(300, 2)).unwrap().0, h(300, 3)); // remote ahead
        c.last = h(100, 5);
        let m = ManualClock::new(900);
        let mut c = HlcClock::new(Box::new(m), h(100, 5));
        assert_eq!(c.observe(h(300, 2)).unwrap().0, h(900, 0)); // wall ahead of both
    }

    #[test]
    fn skew_thresholds() {
        let (mut c, _m) = clock(1_000_000);
        let day = SKEW_FLAG_MS;
        assert_eq!(
            c.observe(h(1_000_000 + day, 0)).unwrap().1,
            Skew::None,
            "exactly 24h is not flagged"
        );
        assert_eq!(
            c.observe(h(1_000_000 + day + 1, 0)).unwrap().1,
            Skew::Ahead { by_ms: day + 1 }
        );
        let year = SKEW_CORRUPT_MS;
        let before = c.last();
        assert!(
            c.observe(h(1_000_000 + year, 0)).is_ok(),
            "exactly one year is accepted (flagged)"
        );
        let before2 = c.last();
        assert!(before2 > before);
        let err = c.observe(h(1_000_000 + year + 1, 0)).unwrap_err();
        assert_eq!(err, HlcError::CorruptClock { ahead_ms: year + 1 });
        assert_eq!(
            c.last(),
            before2,
            "a corrupt timestamp must not move the clock"
        );
    }

    #[test]
    fn now_after_outranks_floor() {
        let (mut c, _m) = clock(100);
        let f = h(5_000, 3);
        let n = c.now_after(f).unwrap();
        assert!(n > f);
    }

    #[test]
    fn pack_round_trip_and_range() {
        for (pt, ctr) in [
            (0, 0),
            (1, 0),
            (1_700_000_000_000, 0),
            (1_700_000_000_000, u16::MAX),
            (MAX_PT, u16::MAX),
        ] {
            let x = h(pt, ctr);
            let p = x.to_i64();
            assert!(p >= 0);
            assert_eq!(Hlc::from_i64(p).unwrap(), x);
        }
        assert_eq!(Hlc::new(MAX_PT + 1, 0), Err(HlcError::OutOfRange));
        assert_eq!(Hlc::from_i64(-1), Err(HlcError::OutOfRange));
    }

    proptest! {
        #[test]
        fn packing_preserves_order(a in 0..=MAX_PT, ac in any::<u16>(), b in 0..=MAX_PT, bc in any::<u16>()) {
            let (x, y) = (h(a, ac), h(b, bc));
            prop_assert_eq!(x.cmp(&y), x.to_i64().cmp(&y.to_i64()));
            prop_assert_eq!(Hlc::from_i64(x.to_i64()).unwrap(), x);
        }

        #[test]
        fn issued_timestamps_strictly_increase(steps in proptest::collection::vec((any::<bool>(), 0u64..5_000, any::<u16>(), 0u64..2_000), 1..200)) {
            let m = ManualClock::new(10_000);
            let mut c = HlcClock::new(Box::new(m.clone()), Hlc::ZERO);
            let mut prev = Hlc::ZERO;
            for (is_observe, wall, ctr, delta) in steps {
                m.set(wall); // wall clock jumps around, including backwards
                let t = if is_observe {
                    match c.observe(h(wall + delta, ctr)) { Ok((t, _)) => t, Err(_) => continue }
                } else {
                    c.now().unwrap()
                };
                prop_assert!(t > prev, "{:?} !> {:?}", t, prev);
                prev = t;
            }
        }
    }
}
