//! SQLCipher-backed local store and migrations (docs/05, docs/04 section 1).
//!
//! * [`Db::create`] / [`Db::open`] open an encrypted database with a raw
//!   256-bit [`DbKey`] (SQLCipher's own KDF is bypassed) and **verify** the
//!   pinned cipher/SQLite settings (SEC-C13).
//! * [`Db::with_tx`] hands the vault layer a [`Store`] over a transaction.
//! * Migrations are embedded, forward-only and preceded by an encrypted backup.
//!
//! This crate derives no keys and does not depend on the `crypto` crate.

#![cfg_attr(
    test,
    allow(
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::panic,
        clippy::print_stderr
    )
)]

mod db;
mod error;
mod key;
mod migrations;
mod pragmas;
mod store;

pub use db::{CreateParams, Db};
pub use error::StorageError;
pub use key::DbKey;
pub use migrations::{BACKUP_RETENTION, latest_schema_version};
pub use pragmas::BUSY_TIMEOUT_MS;
pub use store::{
    DeviceRow, FieldHistoryRow, FieldRow, FolderRow, FtsDoc, Id, ItemFilter, ItemRow, LocalOp,
    OutboxRow, ProviderState, Result, SegmentSeen, Store, Tx,
};

#[cfg(test)]
mod tests;
