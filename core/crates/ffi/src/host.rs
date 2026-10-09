//! The process-wide session host: one vault per process (docs/14 §8 question 1), every call
//! serialized behind one mutex (docs/14 §1), panics contained (A04 deliverable 8).
//!
//! * `call` is the only way an API function reaches the session. It takes the host lock,
//!   runs the closure and converts a panic into `internal`, after locking the session (fail
//!   closed: a half-finished operation never leaves keys in memory).
//! * `lock()` must not wait behind a long unlock (Argon2, a biometric prompt): it first raises
//!   the session's lock-request flag through a handle kept *outside* the host mutex, so an
//!   in-flight unlock drops the key it obtained and leaves the session locked (docs/14 §5).
//! * Poisoning is ignored on purpose: a poisoned lock means a panic happened inside a call, and
//!   the guard already locked the session. Refusing every later call would turn one bug into a
//!   permanent lockout.

use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::PathBuf;
use std::sync::{Mutex, MutexGuard, PoisonError};

use arya_vault_session::{
    KdfProfile as CoreProfile, LockRequestHandle, QuickUnlockProvider, Session,
};
use arya_vault_vault::{ImportBundle, Vault};
use zeroize::Zeroizing;

use crate::api::dto::{AppError, AppErrorCode};
use crate::error::ApiResult;

/// A parsed import file waiting for `importCommit`.
pub(crate) struct Preview {
    pub(crate) token: String,
    pub(crate) bundle: ImportBundle,
}

pub(crate) struct Host {
    dir: Option<PathBuf>,
    session: Option<Session>,
    provider: Option<Box<dyn QuickUnlockProvider>>,
    pub(crate) preview: Option<Preview>,
    /// Cost of the next password wrap (change password, recover, export). Set by `createVault`;
    /// `Default` after a restart (spec question 7).
    pub(crate) profile: CoreProfile,
}

static HOST: Mutex<Host> = Mutex::new(Host::new());
static LOCK_HANDLE: Mutex<Option<LockRequestHandle>> = Mutex::new(None);

fn lock_host() -> MutexGuard<'static, Host> {
    HOST.lock().unwrap_or_else(PoisonError::into_inner)
}

impl Host {
    const fn new() -> Self {
        Self {
            dir: None,
            session: None,
            provider: None,
            preview: None,
            profile: CoreProfile::Default,
        }
    }

    pub(crate) fn set_dir(&mut self, dir: PathBuf) -> ApiResult<()> {
        if self.dir.as_ref() == Some(&dir) {
            return Ok(());
        }
        if self.session_is_unlocked() {
            return Err(AppError::new(
                AppErrorCode::Busy,
                "lock the vault before changing its directory",
            ));
        }
        self.session = None;
        self.preview = None;
        *lock_handle() = None;
        self.dir = Some(dir);
        Ok(())
    }

    pub(crate) fn set_provider(&mut self, provider: Box<dyn QuickUnlockProvider>) -> ApiResult<()> {
        if self.session_is_unlocked() {
            return Err(AppError::new(
                AppErrorCode::Busy,
                "lock the vault before registering a quick-unlock provider",
            ));
        }
        // A session that exists is rebuilt with the provider on the next call.
        self.session = None;
        *lock_handle() = None;
        self.provider = Some(provider);
        Ok(())
    }

    fn session_is_unlocked(&self) -> bool {
        use arya_vault_session::SessionState as S;
        self.session
            .as_ref()
            .is_some_and(|s| matches!(s.state(), S::Unlocked | S::UnlockedPendingConfirm))
    }

    /// The session, opened on first use.
    pub(crate) fn session(&mut self) -> ApiResult<&mut Session> {
        if self.session.is_none() {
            let dir = self.dir.clone().ok_or_else(|| {
                AppError::new(AppErrorCode::Internal, "the core has not been initialised")
            })?;
            let mut s = Session::open_dir(dir)?;
            if let Some(p) = self.provider.take() {
                s = s.with_provider(p);
            }
            *lock_handle() = Some(s.lock_request_handle());
            self.session = Some(s);
        }
        self.session
            .as_mut()
            .ok_or_else(|| AppError::new(AppErrorCode::Internal, "no session"))
    }

    /// Runs `f` on the unlocked vault. `locked` if the session is not unlocked.
    pub(crate) fn vault<R, E: Into<AppError>>(
        &mut self,
        f: impl FnOnce(&mut Vault) -> Result<R, E>,
    ) -> ApiResult<R> {
        self.session()?.with_vault(f)?.map_err(Into::into)
    }

    /// Drops what must not outlive the unlocked state.
    pub(crate) fn forget_unlocked_state(&mut self) {
        self.preview = None;
    }
}

fn lock_handle() -> MutexGuard<'static, Option<LockRequestHandle>> {
    LOCK_HANDLE.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Asks an in-flight unlock to give up, without waiting for the host lock.
pub(crate) fn request_lock() {
    if let Some(h) = lock_handle().as_ref() {
        h.request_lock();
    }
}

/// Runs `f` serialized with every other call; contains panics.
pub(crate) fn call<T>(f: impl FnOnce(&mut Host) -> ApiResult<T>) -> ApiResult<T> {
    guarded(|| {
        let mut host = lock_host();
        f(&mut host)
    })
}

/// Runs `f` with panics converted to `internal` (after locking the session).
///
/// With `panic = "abort"` (release profile) a panic aborts the process before this can run: no
/// unwinding crosses the boundary either way. In debug and test builds the unwinding is caught
/// here.
pub(crate) fn guarded<T>(f: impl FnOnce() -> ApiResult<T>) -> ApiResult<T> {
    match catch_unwind(AssertUnwindSafe(f)) {
        Ok(r) => r,
        Err(_) => {
            fail_closed();
            Err(AppError::internal())
        }
    }
}

fn fail_closed() {
    let mut host = lock_host();
    if let Some(s) = host.session.as_mut() {
        let _ = s.lock();
    }
    host.forget_unlocked_state();
}

/// A password argument: UTF-8 text in a buffer that is wiped when dropped.
///
/// The caller wraps every secret argument in `Zeroizing` *before* the first fallible step, so an
/// early error cannot leave a later argument unwiped. The bytes are moved (not copied) into the
/// string; the only copy left on this side is the returned `Zeroizing<String>`.
pub(crate) fn secret_text(
    mut bytes: Zeroizing<Vec<u8>>,
    field: &str,
) -> ApiResult<Zeroizing<String>> {
    match String::from_utf8(std::mem::take(&mut *bytes)) {
        Ok(s) => Ok(Zeroizing::new(s)),
        Err(e) => {
            drop(Zeroizing::new(e.into_bytes()));
            Err(AppError::validation(field, "the value is not valid UTF-8"))
        }
    }
}

/// Moves a secret out as bytes for the return value, wiping the source.
pub(crate) fn into_bytes(s: Zeroizing<String>) -> Vec<u8> {
    // `to_vec` copies; the source is wiped on drop. One unavoidable copy: frb then copies again
    // when it serialises the return value.
    s.as_bytes().to_vec()
}
