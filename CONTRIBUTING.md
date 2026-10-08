# Contributing

Thanks for helping build a trustworthy password manager. Because this project handles secrets, some rules are stricter than usual.

## Status
The project is in the **documentation/design phase**. Right now the most valuable contributions are **reviews of the specs** in `docs/` (especially `04-crypto-spec.md` and `06-sync-protocol.md`) and threat-model feedback.

## Ground rules
1. **No custom cryptography.** Only the primitives and libraries in `docs/04-crypto-spec.md`. Proposals for new algorithms go through the spec-change process below.
2. **No telemetry, analytics, or network calls** except to the user-chosen sync provider (and the opt-in update check).
3. **No secrets in logs, tests, or fixtures** (use clearly fake canary values).
4. **Small dependency footprint.** New dependencies need justification (maintenance, audit status, transitive size). Crypto dependencies are allowlisted.
5. **Every parser of external data needs a fuzz target.**

## Spec-change process (crypto / sync / data formats)
1. Open an issue labelled `spec-change` describing the problem and threat impact.
2. Submit a PR modifying the relevant doc **before** any code, including new test vectors.
3. Requires approval from **two maintainers** (CODEOWNERS covers `core/crates/crypto`, `core/crates/sync`, `core/crates/ffi`, `docs/04-*`, `docs/06-*`).
4. Bump `format_version` when on-disk/on-wire formats change; keep reading all old versions; add golden files.

## Development workflow (once code exists)
- Branch from `main`; one logical change per PR; link the issue/spec section.
- Rust: `cargo fmt`, `cargo clippy -- -D warnings`, `cargo test`, `cargo deny check`, `cargo audit`.
- Dart: `dart format`, `dart analyze`, `flutter test`.
- Add tests: unit + (where relevant) property tests and simulator scenarios for sync changes.
- Update docs and the threat-model checklist (`docs/03-threat-model.md` §7).

## Commit sign-off (DCO)
Sign your commits with `git commit -s` (Developer Certificate of Origin). No CLA is required.

## Code style
- Rust: idiomatic, `#![forbid(unsafe_code)]` in all crates except an explicitly justified FFI/platform crate; document every `unsafe` block with a safety comment.
- Secrets: use `Zeroizing`/`SecretString`; never `Debug`/`Display` secret types.
- Dart: no crypto in Dart; treat secrets as short-lived `Uint8List`.
- Error messages must never contain secret material.

## Reporting security issues
See [SECURITY.md](SECURITY.md). Do not file public issues for vulnerabilities.

## Code of conduct
Be respectful and constructive. A `CODE_OF_CONDUCT.md` (Contributor Covenant) will be added when the repo goes public.

## License
By contributing you agree your work is licensed under the project license (MPL-2.0; see `docs/09-tech-stack-and-decisions.md` ADR-0009).
