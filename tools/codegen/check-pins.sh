#!/usr/bin/env bash
# The flutter_rust_bridge version must be the same in tools/codegen/FRB_VERSION, the ffi crate's
# Cargo.toml and app/pubspec.yaml. A file that does not exist yet (the dependency arrives with
# the generated code) is skipped; one that exists must match.
set -euo pipefail
root="$(cd "$(dirname "$0")/../.." && pwd)"
want="$(tr -d '[:space:]' < "$root/tools/codegen/FRB_VERSION")"
status=0

cargo_toml="$root/core/crates/ffi/Cargo.toml"
if grep -q '^flutter_rust_bridge' "$cargo_toml"; then
  have="$(sed -n 's/^flutter_rust_bridge *= *"=\{0,1\}\([^"]*\)".*/\1/p' "$cargo_toml" | head -n1)"
  if [ "$have" != "$want" ]; then
    echo "Cargo.toml pins flutter_rust_bridge $have, expected $want (use \"=$want\")" >&2
    status=1
  fi
  grep -q "^flutter_rust_bridge *= *\"=$want\"" "$cargo_toml" || {
    echo "Cargo.toml must pin flutter_rust_bridge exactly: \"=$want\"" >&2
    status=1
  }
else
  echo "note: ffi/Cargo.toml has no flutter_rust_bridge dependency yet (generated code not added)"
fi

pubspec="$root/app/pubspec.yaml"
if [ -f "$pubspec" ] && grep -q '^ *flutter_rust_bridge:' "$pubspec"; then
  have="$(sed -n 's/^ *flutter_rust_bridge: *\([0-9][^ #]*\).*/\1/p' "$pubspec" | head -n1)"
  if [ "$have" != "$want" ]; then
    echo "pubspec.yaml pins flutter_rust_bridge $have, expected $want (no caret)" >&2
    status=1
  fi
else
  echo "note: app/pubspec.yaml has no flutter_rust_bridge dependency yet"
fi
exit "$status"
