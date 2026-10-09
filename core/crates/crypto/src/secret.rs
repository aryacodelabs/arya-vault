//! Fixed-size secret byte container shared by all key types.
//!
//! Bytes are overwritten with zeros on drop (SEC-C06). This is `Zeroizing<[u8; N]>`
//! semantics, written out so tests can observe that the wipe really happens on the
//! live storage (see `test_hook`).

use zeroize::Zeroize;

pub(crate) struct Secret<const N: usize>([u8; N]);

impl<const N: usize> Secret<N> {
    pub(crate) fn new(bytes: [u8; N]) -> Self {
        Self(bytes)
    }

    pub(crate) fn zeroed() -> Self {
        Self([0u8; N])
    }

    pub(crate) fn as_bytes(&self) -> &[u8; N] {
        &self.0
    }

    pub(crate) fn as_mut_bytes(&mut self) -> &mut [u8; N] {
        &mut self.0
    }
}

impl<const N: usize> Drop for Secret<N> {
    fn drop(&mut self) {
        #[cfg(test)]
        let before = self.0.to_vec();
        self.0.zeroize();
        #[cfg(test)]
        test_hook::record(before, self.0.to_vec());
    }
}

#[cfg(test)]
pub(crate) mod test_hook {
    //! Test-only observation of the drop path: records the bytes held *immediately
    //! before* and *immediately after* the wipe in `Drop`. Limits: this proves the wipe
    //! is applied to the live storage; it cannot show that no stale copies exist
    //! elsewhere (moves, registers, swap), which is the job of memory-inspection tests
    //! (docs/08 SEC-C06 "MT").
    use std::cell::RefCell;

    thread_local! {
        static DROPS: RefCell<Vec<(Vec<u8>, Vec<u8>)>> = const { RefCell::new(Vec::new()) };
    }

    pub(crate) fn record(before: Vec<u8>, after: Vec<u8>) {
        DROPS.with(|d| d.borrow_mut().push((before, after)));
    }

    /// Removes and returns all `(before, after)` pairs recorded on this thread.
    pub(crate) fn take() -> Vec<(Vec<u8>, Vec<u8>)> {
        DROPS.with(|d| std::mem::take(&mut *d.borrow_mut()))
    }
}
