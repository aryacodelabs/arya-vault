# T04: Password and passphrase generator

**Branch:** `m1/t04-generator` · **Depends on:** nothing (use `getrandom` directly or a tiny local `Rng` trait; T01 may not be merged yet) · **Crate:** `core/crates/generator`

## Context
Read `CLAUDE.md`, `docs/04-crypto-spec.md` §12, `docs/08-security-requirements.md` (SEC-C09), `docs/01-product-requirements.md` US-05/US-06, `docs/11-testing-strategy.md` §1.

## Deliverables
1. `PasswordOptions { length (8-128), lower, upper, digits, symbols (configurable set), exclude_ambiguous (Il1O0 and similar), require_each_class }` with validation (typed errors; at least one class enabled; length >= number of required classes).
2. `generate_password(opts, rng) -> Zeroizing<String>`: OS CSPRNG only; uniform index selection via **rejection sampling** (no modulo bias); if `require_each_class`, place one char from each enabled class at random positions and fill the rest, then **Fisher-Yates shuffle** with the same unbiased sampler.
3. `generate_passphrase(opts, rng)`: EFF **large wordlist** (7776 words), word count 3-12 (default 6), separator, capitalize, optional number suffix. Bundle the wordlist in the crate (e.g. `include_str!`), with `crates/generator/data/ATTRIBUTION.md` (EFF list, CC BY 3.0 US: keep attribution and source URL), and a test asserting exactly 7776 unique entries and the expected format. State in the PR how you obtained the list and that you verified the count.
4. `entropy_bits(opts)` for the configured generator (exact: `length * log2(alphabet)` adjusted for constraints, or a documented conservative bound; words * log2(7776) for passphrases).
5. `estimate_strength(password) -> Strength { score 0-4, guesses_log10, feedback }` using the `zxcvbn` crate (justify), used later for master-password checks (SEC-A07: min length 12 enforced by caller; expose `meets_master_password_policy(&str)` returning reasons for rejection).
6. An injectable `RandomSource` trait so tests can use a seeded/deterministic source; OS source is the default and the only one available in non-test builds (`#[cfg(test)]` or a non-default feature, same compile-guard pattern as `deterministic-rng` in T01).

## Tests
- Uniformity: chi-square test over >= 1e6 draws for single-character selection across alphabets of awkward sizes (e.g. 62, 71, 94) and for shuffle position distribution; fixed seeds, thresholds documented so tests are not flaky.
- Every enabled class appears when required; no excluded characters ever appear (property test, 10k cases).
- Boundary validation: length limits, empty classes, conflicting options.
- Passphrase: wordlist integrity, separators, capitalization, entropy figures.
- Strength: known weak passwords (`password123`, keyboard walks) score low; long random passwords score high.
- Output type is zeroizing: confirm `Zeroizing` is used and no `String` copies are leaked in the API (no `Debug` printing of generated values).

## Acceptance criteria
SEC-C09 mapped to tests. `missing_docs` clean, no `unsafe`, no `unwrap` outside tests, `cargo deny` passes (check the EFF list file is not a licensed-code dependency: it is data with attribution).

## Out of scope
UI, clipboard handling, breached-password lookups (no network), Diceware in other languages.

---

## Delivery rules (all tasks)
- Work on the branch named above, created from the latest `main`. Open **one PR** against `main`; never push to `main`.
- Read `CLAUDE.md` first and follow its hard rules. **The specs in `docs/` are the source of truth.** If you find a defect or gap, implement the most conservative reading, and describe it in a **"Spec questions"** section of the PR. Do not edit specs except where this prompt explicitly says to, and then only in a separate `docs(spec): ...` commit.
- Stay inside the listed crate/area. Do not refactor unrelated code or change CI beyond what is listed.
- Run from `core/`: `cargo fmt --all --check`, `cargo clippy --workspace --all-targets --locked -- -D warnings`, `cargo test --workspace --locked`, `cargo deny check`. All must pass before you finish.
- PR description must contain: summary; **requirement -> test table** (SEC-* IDs); dependencies added with justification; benchmark numbers where requested; "Spec questions"; "Deferred / not done" (be honest); how you verified.
- Never use real secrets. Test data must be fake and clearly marked (`CANARY-...`). Do not ask for or use any credentials.
- Do not weaken or delete tests to make them pass. If a test cannot pass because the spec is wrong, say so in the PR.
- Commit style: `feat(crypto): ...`, small commits, sign off with `git commit -s`.
