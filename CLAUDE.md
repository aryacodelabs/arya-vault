# AryaVault: guidance for AI coding sessions

Open-source, local-first, zero-knowledge password/notes manager (Windows, macOS, Linux, Android, iOS). Flutter UI + shared Rust core. No server, no web app. Optional encrypted sync via the user's Google Drive / iCloud (Apple only) / folder.

**Status:** design docs complete (`docs/`), Rust workspace skeleton only (`core/`). Implement milestones in order per `docs/10-roadmap.md`.

## Source of truth
The specs in `docs/` define behavior. Read the relevant doc **before** coding and cite section numbers in PRs.
- Crypto/key hierarchy/formats: `docs/04-crypto-spec.md`
- Data model/schema: `docs/05-data-model.md`
- Sync/merge/providers: `docs/06-sync-protocol.md`
- Requirements to satisfy (IDs `SEC-*`): `docs/08-security-requirements.md`
- Test expectations: `docs/11-testing-strategy.md`

If code and spec disagree, or the spec is ambiguous or wrong, **stop and propose a spec change** (doc PR first, see CONTRIBUTING.md). Never silently diverge.

## Hard rules
1. **No custom crypto.** Only Argon2id, XChaCha20-Poly1305, HKDF-SHA256, SHA-256 via RustCrypto crates listed in docs/09. No new algorithms or constructions.
2. **All security logic in Rust.** Never implement crypto or parsing in Dart.
3. `#![forbid(unsafe_code)]` workspace-wide (enforced via workspace lints). Any exception needs a justified, commented `unsafe` in `ffi`/platform glue only, with maintainer approval.
4. Secrets: `Zeroizing`/`secrecy`; never implement `Debug`/`Display` that prints them; never log secrets or put them in error messages; test fixtures use obvious fake canary strings.
5. Randomness only from the OS CSPRNG. Nonces random 192-bit, never counters.
6. No telemetry/analytics; no network calls except the user-chosen provider (and opt-in update check).
7. Every parser of external bytes (cloud files, imports, FFI input) is bounded, returns typed errors (no panics), and gets a `cargo-fuzz` target.
8. New dependency = justification in the PR; prefer std/RustCrypto; crates must pass `cargo deny`.
9. Never commit secrets, keys, OAuth client secrets, or real vault data (see `.gitignore`).
10. Changes to `crypto`, `sync`, `ffi`, docs 04/06 need two-maintainer review. Do not weaken tests to make them pass.

## Layout
`core/` Rust workspace (`crates/{crypto,storage,vault,sync,generator,providers-*,ffi,cli}`, `testdata/` golden files) · `app/` Flutter · `platform/` native plugins · `tools/` simulator, fuzz, Python cross-check decryptor · `docs/` specs.

## Commands (run from `core/`)
```
cargo fmt --all --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
cargo deny check          # needs cargo-deny
```
CI (`.github/workflows/ci.yml`) runs these on Linux/Windows/macOS. All must pass before a PR is ready.

## Conventions
- Crate names `arya-vault-<name>`; canonical CBOR (deterministic) for anything hashed or used as AAD.
- Errors: `thiserror`-style typed enums; no `unwrap`/`expect`/`panic` in non-test code (clippy warns).
- Tests alongside code; known-answer vectors and golden files in `core/testdata/`; property tests (`proptest`) for merge laws and codecs.
- Commits: conventional style (`feat(crypto): ...`), signed off (`git commit -s`), small and focused. One crate / one concern per PR.
- Format versions: bump `format_version` on any on-disk/on-wire change; keep reading all old versions; add golden files.

## Working in cloud/remote sessions
- Stay within the assigned crate/task; list assumptions and open spec questions in the PR description.
- Don't request or handle real credentials (Google OAuth secrets, signing keys, Apple certs). Provider and platform work needing them is done locally by the owner.
- Linux-only environment: cannot build iOS/macOS/CloudKit/Windows-specific code; write it behind traits and leave platform glue stubs.
