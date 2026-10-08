# Fuzzing (`cargo-fuzz`)

Fuzz targets for every parser of external bytes (CLAUDE.md hard rule 7; docs/11 §4, §8).
This crate is **not** a member of the `core/` workspace: it has its own `[workspace]`
table, its own `Cargo.lock`, and needs nightly Rust.

## Running locally

```sh
cargo install cargo-fuzz --locked      # once
cd core
cargo +nightly fuzz run smoke -- -max_total_time=5
cargo +nightly fuzz list
```

Crashing inputs are written to `core/fuzz/artifacts/<target>/`. Reproduce with
`cargo +nightly fuzz run <target> core/fuzz/artifacts/<target>/<crash-file>`.
A crash is a release blocker (docs/11 §4): fix the parser, then commit the minimized
input as a regression test in the owning crate.

## Adding a target

1. Name it `fuzz_<parser>` (e.g. `fuzz_envelope`, `fuzz_header`, `fuzz_csv_import`).
   The nightly workflow discovers targets from the `fuzz_targets/` directory listing,
   so the file name (without `.rs`) must equal the `[[bin]]` name.
2. Add `fuzz_targets/fuzz_<parser>.rs` using `libfuzzer_sys::fuzz_target!`. Feed the bytes
   to the public parse entry point; the target must never panic, abort, or allocate
   unboundedly on any input (a typed `Err` is the correct outcome).
3. Register it in `Cargo.toml`:
   ```toml
   [[bin]]
   name = "fuzz_<parser>"
   path = "fuzz_targets/fuzz_<parser>.rs"
   test = false
   doc = false
   bench = false
   ```
   and add a path dependency on the crate under test.
4. Seed the corpus under `corpus/fuzz_<parser>/` from golden files in `core/testdata/`
   (copy or symlink-free copies; keep seeds small). Only fake data (`CANARY-...`), never
   real vault contents. Commit seeds; do not commit machine-generated corpus growth.
5. Optionally add a dictionary of magic bytes at `dict/fuzz_<parser>.dict` and pass
   `-dict=` in the nightly workflow.
6. Remove nothing from existing targets to make them pass. Never weaken a target.

The `smoke` target is a placeholder that exercises no project code; it only proves the
toolchain works in CI. Keep it until real targets exist.
