#!/usr/bin/env bash
# Generates the Dart bindings (app/lib/src/rust) and the Rust glue (core/crates/ffi/src/
# frb_generated.rs) from the `api` module of the ffi crate.
#
#   tools/codegen.sh           regenerate
#   tools/codegen.sh --check   regenerate and fail if anything differs from what is committed
#                              (the CI job; Linux, needs Flutter/Dart on PATH)
#
# Requires: a Rust toolchain, Flutter (which brings Dart). The codegen version is pinned in
# tools/codegen/FRB_VERSION and must equal the flutter_rust_bridge version in the ffi crate's
# Cargo.toml and in app/pubspec.yaml.
set -euo pipefail
root="$(cd "$(dirname "$0")/.." && pwd)"
cd "$root"

mode="generate"
[ "${1:-}" = "--check" ] && mode="check"

bash tools/codegen/check-pins.sh
version="$(tr -d '[:space:]' < tools/codegen/FRB_VERSION)"

for tool in cargo dart flutter; do
  command -v "$tool" >/dev/null || { echo "error: '$tool' is not on PATH" >&2; exit 2; }
done

if ! command -v flutter_rust_bridge_codegen >/dev/null \
   || [ "$(flutter_rust_bridge_codegen --version | awk '{print $NF}')" != "$version" ]; then
  cargo install flutter_rust_bridge_codegen --version "=$version" --locked
fi

(cd app && flutter_rust_bridge_codegen generate)

if [ "$mode" = "check" ]; then
  if ! git diff --exit-code -- app/lib/src/rust core/crates/ffi/src/frb_generated.rs; then
    echo "error: the generated bindings are out of date; run tools/codegen.sh and commit" >&2
    exit 1
  fi
  untracked="$(git ls-files --others --exclude-standard -- app/lib/src/rust core/crates/ffi/src/frb_generated.rs)"
  if [ -n "$untracked" ]; then
    echo "error: generated files are not committed:" >&2
    echo "$untracked" >&2
    exit 1
  fi
  echo "generated bindings are up to date"
fi
