#!/usr/bin/env bash
# API_VERSION in the ffi crate and expectedApiVersion in the Dart app must be the same number
# (docs/14 section 7: the app asserts equality at start-up).
set -euo pipefail
root="$(cd "$(dirname "$0")/../.." && pwd)"
rust="$(sed -n 's/^pub const API_VERSION: u32 = \([0-9][0-9]*\);.*/\1/p' "$root/core/crates/ffi/src/lib.rs")"
dart="$(sed -n 's/^const int expectedApiVersion = \([0-9][0-9]*\);.*/\1/p' "$root/app/lib/src/api/vault_api.dart")"
if [ -z "$rust" ] || [ -z "$dart" ]; then
  echo "error: could not read API_VERSION (ffi lib.rs) or expectedApiVersion (vault_api.dart)" >&2
  exit 2
fi
if [ "$rust" != "$dart" ]; then
  echo "error: API_VERSION is $rust in the ffi crate but expectedApiVersion is $dart in the app" >&2
  exit 1
fi
echo "API version $rust matches in the ffi crate and the app"
