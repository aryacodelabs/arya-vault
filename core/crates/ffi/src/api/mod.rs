//! The app <-> core API of docs/14 §4. Every `pub fn` in the submodules is one contract
//! operation (the golden test `tests/contract.rs` compares the two lists); `dto` holds the types.
//!
//! This module is the input of `flutter_rust_bridge_codegen` (`flutter_rust_bridge.yaml`): keep
//! helper functions out of it (use `pub(crate)` outside `api/`).

pub mod diagnostics;
pub mod dto;
pub mod items;
pub mod lifecycle;
pub mod settings;
pub mod tools;
pub mod transfer;
