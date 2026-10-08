# M1 Cloud-Session Task Prompts (Rust core)

Self-contained prompts for Claude Code cloud sessions. Each file is one session, one branch, one PR. Paste the file's contents as the session's first message (the repo's `CLAUDE.md` is read automatically).

## Before you launch anything
1. **Merge the spec-review PR** (`docs/spec-review-1`) into `main`. These prompts cite requirement IDs (SEC-C11, SEC-Y10 ...) and doc sections that exist only after it.
2. Merge this prompts PR too (optional but keeps prompts versioned).
3. Confirm cloud sessions can access `aryacodelabs/arya-vault` and open PRs.
4. Run **T00 first** as a cheap calibration run: check what it cost before launching the rest.

## Task list, dependencies and order

| ID | Task | Crate / area | Depends on | Est. cost (rough guess) |
|---|---|---|---|---|
| T00 | CI, fuzz and coverage infrastructure | `core/fuzz`, `.github/` | none | $2-3 |
| T01 | Crypto primitives and key hierarchy | `crypto` | none | $6-8 |
| T04 | Password and passphrase generator | `generator` | none | $2-3 |
| T05 | Encrypted local storage (SQLCipher) | `storage` | none | $4-5 |
| T02 | Header, envelope and container formats | `crypto` (formats) | T01 | $5-6 |
| T06 | Vault model, history and search | `vault` | T05 | $6-8 |
| T03 | Python cross-check decryptor and golden files | `tools/crosscheck`, `core/testdata` | T02 | $3-4 |
| T07 | Import and export | `vault` (import/export) | T01, T06 | $5-6 |
| T08 | CLI harness, benchmarks, M1 exit check | `cli` | all | $4-5 |

Total: roughly **$35-50** (guess, not measured). Compare T00's real cost to the estimate and rescale.

```
Wave 1 (parallel):  T00   T01   T04   T05
Wave 2:                   T02 <-T01   T06 <-T05
Wave 3:                   T03 <-T02   T07 <-T01,T06
Wave 4:                   T08 <- everything
```

## Merge discipline
- Merge PRs in wave order. Within a wave, merge one at a time and rebase the others (`Cargo.lock` will conflict; regenerate it).
- You review every PR yourself, especially T01, T02, T05 (crypto, formats, storage keys). Use the PR's "Spec questions" section as your reading guide.
- If a session proposes a spec change, it must be in its own commit touching `docs/` only. Do not merge it casually: update the docs first, then rerun the task if needed.
- A failed or confused session is cheap to discard: close the PR, adjust the prompt, rerun.

## Known spec gaps the sessions will hit (decided here)
| Gap | Decision for M1 |
|---|---|
| Recovery-key checksum algorithm (doc 04 §4 says "2-char checksum" only) | First 10 bits of `SHA-256("aryavault/rk-check/v1" ‖ 20 key bytes)` as 2 Crockford chars. T01 documents it in a doc-04 commit. |
| Deterministic RNG for golden files | A `Rng` trait with an OS implementation; a deterministic implementation only under the non-default `deterministic-rng` feature, never compiled into release builds. |
| Where `Hlc` lives | `vault` crate owns `Hlc`/`Register` in M1; `sync` (M4) depends on `vault`. |
| Encrypted export format | T07 drafts `docs/13-export-format.md`; owner reviews before merge. |
| SQLCipher/OpenSSL license in `cargo-deny` | T05 may add `OpenSSL` (and any needed) license to `core/deny.toml` with justification in the PR. |

## Cross-cutting acceptance for every PR
`cargo fmt --check`, `cargo clippy -D warnings`, `cargo test --workspace --locked` and `cargo deny check` all pass; no new `unsafe`; no `unwrap`/`expect` outside tests; docs for all public items; requirement IDs referenced in tests; PR description filled using the template.
