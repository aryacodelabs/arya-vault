# B02: Vault screens (list, search, detail, edit, trash, history)

**Branch:** `m2/b02-vault-screens` · **Depends on:** B01 merged · **Area:** `app/lib/src/ui/vault/`, `app/lib/src/state/` · Uses only `VaultApi` (fake now, FFI later)

## Context
Read `CLAUDE.md`, `docs/07-ux-and-recovery.md` §2, §6-9, `docs/01-product-requirements.md` US-03/04/08/14, **`docs/14-app-api-contract.md` §3-4.2**, `docs/05-data-model.md` §2, `docs/06-sync-protocol.md` §5.3 (concurrent-versions is a **derived view**; UI only displays `otherVersions`/`VersionInfo.concurrent`), `docs/08` (SEC-H03). Follow B01's architecture exactly (Riverpod, `VaultApi`, `SecretBytes`, l10n, services interfaces). The fake API from B01 holds 2,000 sample items; extend it (in `fake_vault_api.dart`, backwards compatible) as needed for these screens, including history versions, folders, trash, and a **20,000-item mode** for scroll/search performance tests.

## Deliverables
1. **Shell**: desktop three-pane (sidebar: All items, Favorites, Folders, Tags, Trash; item list; detail) and compact layout (list -> detail push). Persistent search bar (debounced 150 ms, cancels stale requests, shows count), type filter chips, sort controls if the API supports them (do not invent API: use only what `docs/14` defines; otherwise note as a spec question).
2. **List**: virtualized, **paged** (limit <= 200, infinite scroll via `Page`), no secrets in rows, favorite star, tag chips, trash rows show purge date; keyboard navigation (arrows, Enter, Delete -> trash with undo snackbar), multi-select deferred.
3. **Detail** (`getItem`): masked password with explicit **Reveal** (calls `reveal`, shows for `revealHideSeconds`, default 15 s, hides on lock/blur/navigation; wipes `SecretBytes`), **Copy** buttons through `ClipboardService` (this task uses the interface only; the actual timed-clear behavior is B03/L01) with a "Copied; clears in N s" toast, URLs (open through a `UrlLauncherService` interface; **confirm before opening non-https**), TOTP code field placeholder (feature arrives in M8; show nothing if `hasTotp` is false), custom fields (hidden kind masked), notes/Markdown body, tags, folder, created/updated, "Other versions" badge when `otherVersions` is true -> opens history.
4. **Markdown notes** rendering that is safe: no remote images/network fetches, no raw HTML, links shown as text and opened only after confirmation; use a vetted package or a minimal in-house renderer (justify); tests with hostile Markdown (image URLs, `javascript:` links, HTML, giant input).
5. **Edit forms** for login, note, card, identity: validation messages from `AppError(validation, field)`, inline length/limit hints from `docs/05` §10, unsaved-changes guard, atomic save via `setFields`, create via `createItem`, add/remove URLs, tags, custom fields (kind: text/hidden/url/date), folder picker. The inline **password generator** button is a stub hook that B03 fills (define a small `PasswordGeneratorLauncher` callback type now).
6. **Trash**: list, restore, purge, empty trash (confirm), shows retention.
7. **History & versions**: per-field history list (`history`), reveal an old value (hidden fields stay masked until revealed), **restore version**; for fields with concurrent versions show a clear compare view (current vs other) with "Restore this version" (an ordinary op) and, for note bodies, "Keep as separate note" (creates a new item via `createItem` with the other version's text) per `docs/06` §5.3.
8. **Folders and tags management**: create/rename/delete (delete moves items to "no folder" and says so), tag add/remove from detail and edit.
9. **Lock behavior**: when the session locks (`locked` error from any call or a lock event), every screen clears sensitive state and returns to the Lock screen; a test proves no revealed secret survives in state/providers after lock.

## Tests
- Widget tests for all screens/flows above, including keyboard-only operation, screen-reader semantics, 200% text scale, compact and desktop layouts.
- Performance tests with the 20,000-item fake: initial list < 100 ms of frame work in test, scroll through 10 pages without rebuilding the whole list, search debounce/cancel correctness (no out-of-order results).
- Secret hygiene: reveal auto-hide (fake async), wipes on lock, no secrets in `toString`/logs; copy toast never prints the value.
- Golden tests: list, detail (masked and revealed with a fake canary), edit login, trash, history compare (light/dark).
- Error handling: each relevant `AppErrorCode` renders a human message; `locked` mid-edit preserves unsaved **non-secret** drafts only if safe (decide and document; secrets are discarded).

## Acceptance criteria
`flutter analyze`, format, tests green; no new dependency without justification (Markdown package if used); no API methods used that are not in `docs/14`; PR includes a screenshot table (generated golden images are fine) and a contract-usage table.

## Out of scope
Generator/health/settings (B03), import/export screens (B03), real clipboard/capture behavior (L01), attachments, sharing, TOTP generation.

---

## Delivery rules (all tasks)
- Work on the branch named above, created from the latest `main`. Open **one PR** against `main`; never push to `main`.
- Read `CLAUDE.md` first and follow its hard rules. **The specs in `docs/` and the API contract `docs/14-app-api-contract.md` are the source of truth.** If you find a defect or gap, implement the most conservative reading and describe it in a **"Spec questions"** section of the PR. Do not edit specs or the contract except where this prompt explicitly says so, and then only in a separate `docs(spec): ...` commit.
- Stay inside the listed crate/area. Do not refactor unrelated code or change CI beyond what is listed.
- Rust checks, run from `core/`: `cargo fmt --all --check`, `cargo clippy --workspace --all-targets --locked -- -D warnings`, `cargo test --workspace --locked`, `cargo deny check`. Flutter checks, from `app/`: `dart format --set-exit-if-changed .`, `flutter analyze`, `flutter test`. Everything that applies must pass before you finish.
- PR description must contain: summary; **requirement -> test table** (SEC-* IDs); dependencies added with justification; benchmark numbers where requested; "Spec questions"; "Deferred / not done" (be honest); how you verified (commands + results). If an environment limit stopped you from verifying something (for example Flutter or Dart could not be installed), say exactly what was not run.
- Never use real secrets. Test data must be fake and clearly marked (`CANARY-...`). Do not ask for or use any credentials.
- Do not weaken or delete tests to make them pass. If a test cannot pass because the spec is wrong, say so in the PR.
- Commit style: `feat(session): ...` / `feat(app): ...`, small commits, sign off with `git commit -s`.
