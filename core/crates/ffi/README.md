# arya-vault-ffi

The app <-> core API of [docs/14](../../../docs/14-app-api-contract.md): the input crate of
`flutter_rust_bridge` (frb). Every operation of docs/14 §4 is one `pub fn` in `src/api/`, every DTO
of §3 is a plain struct or enum in `src/api/dto.rs`. Everything else in the crate is `pub(crate)`.

## Status of the Dart bindings

**Not generated yet.** The cloud session that wrote this crate has no Dart/Flutter toolchain, and
generated code must not be written by hand. What exists:

* the `api` module (compiled and tested through plain Rust calls, not through Dart),
* `app/flutter_rust_bridge.yaml`, `tools/codegen.sh`, `tools/codegen.ps1` and the pinned version in
  `tools/codegen/FRB_VERSION` (2.13.0),
* no `flutter_rust_bridge` dependency in `Cargo.toml` and no `src/frb_generated.rs`: both arrive with
  the generated code (task L01, which also adds the CI job that runs `tools/codegen.sh --check`).

`unsafe`: none in this crate today. The owner-approved exception covers the generated module only.
When it lands, the crate swaps the workspace `forbid(unsafe_code)` for `deny(unsafe_code)` and puts
one `#[allow(unsafe_code)]` on `mod frb_generated`.

## Build outputs

`crate-type = ["cdylib", "staticlib", "rlib"]`.

| Platform | Artifact (`cargo build -p arya-vault-ffi --release`) |
|---|---|
| Windows | `target/release/arya_vault_ffi.dll` (+ `.dll.lib`) |
| Linux / Android | `target/release/libarya_vault_ffi.so` |
| macOS | `target/release/libarya_vault_ffi.dylib` |
| iOS | `target/<triple>/release/libarya_vault_ffi.a` (staticlib) |

The release profile keeps `panic = "abort"` (workspace `Cargo.toml`): a panic aborts the process,
nothing unwinds across the boundary. Debug and test builds catch panics at the API boundary and
return `internal` after locking the session (`host::guarded`).

## Design

* **One vault per process.** `initCore(dir)` (not in docs/14; spec question 1) names the directory;
  the session is opened lazily behind a process-wide mutex. Every call takes the mutex, so calls are
  serialized (docs/14 §1).
* **`lock()` never waits behind an unlock.** It first raises the session's lock-request flag through
  a handle kept outside the mutex; an in-flight Argon2 unlock or biometric prompt drops the key it
  obtained and leaves the session locked (docs/14 §5).
* **Fail closed.** A panic inside a call locks the session. A poisoned mutex is ignored on purpose:
  refusing every later call would turn one bug into a permanent lockout.
* **Quick unlock.** `NoProvider` by default. Platform glue calls the Rust-only
  `register_quick_unlock_provider(Box<dyn QuickUnlockProvider>)` once at start-up (not part of
  `api`, so Dart cannot call it).
* **No secret in a list type.** `ItemSummary` and `ItemView` are separate structs that never had a
  secret field; `tests/contract.rs` destructures them exhaustively (a new field breaks the build) and
  scans the field names.

## Secrets and copies (honest limits)

* Inputs (`Uint8List` passwords, recovery key) arrive as `Vec<u8>`, are wrapped in `Zeroizing`
  immediately, moved (not copied) into a `Zeroizing<String>` and wiped when the call ends. frb copies
  the Dart buffer into Rust before our code runs; that copy is the `Vec<u8>` we wipe. Dart's own
  buffer is Dart's to wipe (`SecretBytes`, docs/14 §1).
* Outputs (`reveal*`, `generate*`, the recovery key, history values) leave as `Vec<u8>`. We build the
  return value from a `Zeroizing<String>` (one copy, source wiped on drop); frb then serializes the
  `Vec<u8>` into the Dart-bound buffer and drops it **without** wiping it. Unwiped copies therefore
  exist briefly in frb's own buffers. Reducing that needs a frb-side change (an `ZeroizeOnDrop`
  wrapper type) and is listed as deferred.
* `setField`, `addCustomField` and `setCustomValue` take `String` (as docs/14 specifies); the Rust
  copy is wiped, the Dart `String` cannot be.
* Nothing logs, and the secret-carrying DTOs have a redacted `Debug` (tested).

## Error mapping

`SessionError`, `VaultError` and `ImportError` convert to `AppError { code, message, field }`.
`message` is a static text or the `Display` of a typed core error (none echoes input); `Io` is
reduced to `"I/O error"` (OS error text can contain paths) and `internal` to `"internal error"`.
`tests/leaks.rs` pushes canary secrets through every credential and input path and asserts none
appears in `code`, `message`, `field` or the `Debug` output.

## Tests

`cargo test -p arya-vault-ffi` (about 2.5 minutes in debug: Argon2id at the floor profile runs
dozens of times). Integration tests drive the contract through `api::*`:
`tests/lifecycle.rs`, `tests/items.rs`, `tests/leaks.rs`, `tests/robustness.rs` (threads, lock
while in flight, panic containment, vaults shared with the session library), `tests/contract.rs`
(function list vs docs/14, `API_VERSION`, DTO shape).
