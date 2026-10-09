//! Fault injection for atomicity tests: `point()` fails on the Nth call.

use crate::error::Result;

#[cfg(test)]
thread_local! {
    static COUNT: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    static FAIL_AT: std::cell::Cell<Option<usize>> = const { std::cell::Cell::new(None) };
}

/// Arm the injector: the `n`th (0-based) subsequent `point()` call fails.
#[cfg(test)]
pub(crate) fn arm(n: usize) {
    COUNT.with(|c| c.set(0));
    FAIL_AT.with(|f| f.set(Some(n)));
}

/// Disarm and return how many points were passed.
#[cfg(test)]
pub(crate) fn disarm() -> usize {
    FAIL_AT.with(|f| f.set(None));
    COUNT.with(std::cell::Cell::get)
}

/// A place where a crash or I/O error could interrupt a mutation. No-op outside tests.
#[cfg(test)]
pub(crate) fn point() -> Result<()> {
    let n = COUNT.with(|c| {
        let n = c.get();
        c.set(n + 1);
        n
    });
    if FAIL_AT.with(std::cell::Cell::get) == Some(n) {
        Err(crate::error::VaultError::Injected)
    } else {
        Ok(())
    }
}

/// No-op in production builds.
#[cfg(not(test))]
#[inline(always)]
pub(crate) fn point() -> Result<()> {
    Ok(())
}
