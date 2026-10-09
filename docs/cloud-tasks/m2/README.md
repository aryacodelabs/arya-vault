# M2 Task Prompts: Windows MVP (Flutter app over the Rust core)

M2 goal (`docs/10-roadmap.md`): a usable **Windows** app: lock screen, onboarding with a verified recovery key, vault CRUD, notes, search, generator, health report, Windows Hello quick unlock, clipboard hygiene, capture exclusion, auto-lock, unsigned MSIX. Flutter code is written cross-platform, but only Windows is verified in M2 (Android is M3).

M1 left the vault lifecycle inside the **CLI crate**, so the first job is extracting it into a library the FFI layer can share.

## Read first
1. **`docs/14-app-api-contract.md`**: the shared API between the Rust FFI crate and the Dart app. Both sides build to it in parallel, so **review and approve it before launching anything**. Changes go through a doc PR.
2. Complete **L00 (local setup)** on your machine. Several tasks need Strawberry Perl, NASM, Flutter, etc. Cloud sessions don't, but your later local work does.

## Owner decisions needed before launch
| # | Decision | Default if you say nothing |
|---|---|---|
| 1 | Approve `docs/14-app-api-contract.md` (incl. its 3 open questions) | Assumptions in §8 stand |
| 2 | **Allow `unsafe` only inside `flutter_rust_bridge`-generated code** in the `ffi` crate (CLAUDE.md rule 3 requires maintainer approval). A04 is written assuming **yes**, scoped to the generated module only; all hand-written code stays `forbid(unsafe_code)` | A04 stops and reports if you say no |
| 3 | Default Argon2 calibration target (M1 exit check: 750 ms gave ~1.0 s unlock) | Keep 750 ms |
| 4 | Cloud credit check: M2 is bigger than M1 (see estimates) | You run waves in priority order |

## Task list

| ID | Task | Where | Depends on | Cloud? | Est. cost (guess) |
|---|---|---|---|---|---|
| L00 | Local setup and spikes | your machine (+ me) | none | no | n/a |
| A00 | M1 follow-ups (perf index, fuzz gap, SEC-C04, crash tests) | `storage`, `vault`, `crypto` tests | none | yes | $4-5 |
| A01 | Extract `session` library from the CLI | new crate `session`, `cli` | none | yes | $6-8 |
| B01 | Flutter foundation, fake API, lock + onboarding | `app/` | contract | yes* | $8-10 |
| A02 | Quick-unlock seam and policy (fake provider) | `session` | A01 | yes | $5-6 |
| A03 | Key rotation (SEC-A06, MUST) | `session`, `crypto`, `storage` | A01 | yes | $5-6 |
| B02 | Vault screens: list, search, detail, edit, trash, history | `app/` | B01 | yes* | $8-10 |
| B03 | Generator, health, settings, auto-lock and clipboard services | `app/` | B01 | yes* | $6-8 |
| A04 | `flutter_rust_bridge` FFI crate | `ffi`, `app/lib/src/rust` | A01-A03 | yes** | $8-10 |
| L01 | Windows integration: wire app to FFI, Hello provider, clipboard/capture, MSIX | your machine + me | A04, B01-B03 | no | n/a |
| L02 | M2 exit check and usability test | you + me | L01 | partly | n/a |

\* Flutter tasks need a Flutter SDK in the cloud environment. Each prompt tells the session to install a **pinned** version and to stop and report if that is impossible; then run the task locally with me instead.
\*\* `flutter_rust_bridge` codegen needs Dart/Flutter too; same fallback.

Rough total for the cloud tasks: **$50-65** (guess; compare with M1's real cost before committing). If credit is short, priority order: A01, A04, B01 (critical path), then A03 (MUST requirement), A02, B02, B03, A00. B02/B03 are also fine to do locally with me.

## Dependency graph
```
Wave 1 (parallel):   A00     A01     B01
Wave 2 (parallel):   A02<-A01   A03<-A01   B02<-B01   B03<-B01
Wave 3:              A04 <- A01,A02,A03
Wave 4 (local):      L01 <- A04,B01..B03        then L02
```
B01-B03 depend only on the **contract** (they use a fake in-memory `VaultApi`), so they can run before the FFI exists. A04 produces the real implementation of the same interface.

## Merge discipline (same as M1)
Merge in wave order, one PR at a time; rebase the rest (`Cargo.lock`, `pubspec.lock` will conflict: regenerate). You review every PR; for A01-A04 read "Spec questions" first. If the contract needs a change, stop all dependent sessions, fix the doc PR, then rerun them.

## Cross-cutting acceptance
Rust: `cargo fmt --check`, `cargo clippy -D warnings`, `cargo test --workspace --locked`, `cargo deny check`. Flutter: `dart format --set-exit-if-changed`, `flutter analyze`, `flutter test`. No secrets in logs, tests or screenshots; fake canary data only; PR template filled; requirement IDs referenced in tests.
