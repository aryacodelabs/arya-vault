//! Test-only probes: count drops of the unlocked state and of a pending recovery key, so tests
//! can prove `lock()` released them (the key types themselves are wiped on drop, SEC-C06).

use std::cell::Cell;

thread_local! {
    pub(crate) static UNLOCKED_DROPS: Cell<usize> = const { Cell::new(0) };
    pub(crate) static PENDING_DROPS: Cell<usize> = const { Cell::new(0) };
}

pub(crate) struct UnlockedProbe;
impl Drop for UnlockedProbe {
    fn drop(&mut self) {
        UNLOCKED_DROPS.with(|c| c.set(c.get() + 1));
    }
}

pub(crate) struct PendingProbe;
impl Drop for PendingProbe {
    fn drop(&mut self) {
        PENDING_DROPS.with(|c| c.set(c.get() + 1));
    }
}
