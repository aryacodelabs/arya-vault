//! The app <-> core API (docs/14), the input crate of `flutter_rust_bridge`.
//!
//! * [`api`] is the contract: one `pub fn` per operation of docs/14 §4 and the DTOs of §3. It is
//!   what `flutter_rust_bridge_codegen` reads (see `flutter_rust_bridge.yaml`).
//! * Everything else is `pub(crate)`: the process-wide session host (`host`), error mapping
//!   (`error`), conversions (`convert`), settings persistence (`prefs`) and diagnostics JSON.
//! * No raw key ever crosses this boundary. Secrets leave only as `Vec<u8>` from `reveal*`,
//!   `generate*` and the recovery key; see the README for the honest limits of wiping them.
//!
//! `unsafe`: none in hand-written code. The crate denies `unsafe_code`; the one owner-approved
//! exception (A04) is an `allow` on `mod frb_generated`, the code `flutter_rust_bridge` writes
//! (`tools/codegen.sh` regenerates it; never edit it by hand).

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]

#[cfg(all(feature = "test-support", not(debug_assertions)))]
compile_error!("the `test-support` feature is for tests only and must not be built in release");

pub mod api;
mod convert;
mod diag;
mod error;
// Owner-approved exception to the unsafe ban (A04): only the flutter_rust_bridge-generated module.
#[allow(
    unsafe_code,
    missing_docs,
    clippy::all,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::print_stdout
)]
#[rustfmt::skip]
mod frb_generated;
mod host;
mod prefs;

use arya_vault_session::QuickUnlockProvider;

pub use api::dto::{AppError, AppErrorCode};
pub use api::items::MAX_PAGE_LIMIT;

/// The version of the Dart <-> Rust contract. Bump on any change to [`api`]; `info()` returns it
/// and the Dart side asserts equality at start-up (docs/14 §7).
pub const API_VERSION: u32 = 3;

/// Registers the platform's quick-unlock provider (docs/14 §6). **Rust-side only**: it is not
/// part of [`api`] and Dart cannot call it. The platform glue (e.g. the Windows Hello crate,
/// task L01) calls it once at start-up, before `initCore`/the first API call; by default
/// quick unlock is unavailable ([`arya_vault_session::NoProvider`]).
///
/// # Errors
/// `busy` if a vault is currently unlocked.
pub fn register_quick_unlock_provider(
    provider: Box<dyn QuickUnlockProvider>,
) -> Result<(), AppError> {
    host::call(|h| h.set_provider(provider))
}

/// Test-only: a function that panics, to prove panics become `internal` and lock the session.
#[cfg(any(test, feature = "test-support"))]
#[doc(hidden)]
#[allow(clippy::panic)]
pub fn __panic_for_test() -> Result<(), AppError> {
    host::call(|_| -> Result<(), AppError> { panic!("test panic") })
}
