# Wordlist attribution

`eff_large_wordlist.txt` is the **EFF Long Wordlist** (7776 words, for use
with five dice) by the Electronic Frontier Foundation.

- Source: <https://www.eff.org/files/2016/07/18/eff_large_wordlist.txt>
  (announcement: <https://www.eff.org/deeplinks/2016/07/new-wordlists-random-passphrases>)
- Author: Electronic Frontier Foundation
- License: [Creative Commons Attribution 3.0 United States](https://creativecommons.org/licenses/by/3.0/us/) (CC BY 3.0 US)
- Changes: none to the words or their order. The file is stored in the original
  `NNNNN<TAB>word` format with LF line endings.

This file is **data**, not code; it is not a Cargo dependency and is not
subject to `cargo deny` license checks.

## Provenance of this copy

`www.eff.org` was not reachable from the build environment (blocked by egress
policy), so this copy was reconstructed from the data table of the
`eff-wordlist` 1.0.3 crate on crates.io (sourced from the EFF list) and
re-serialised into the original format. It was checked for: exactly 7776
entries, 7776 unique words, dice keys `11111`..`66666` in sequential order,
first word `abacus`, last word `zoom`, and the four hyphenated entries of the
original list (`drop-down`, `felt-tip`, `t-shirt`, `yo-yo`).
SHA-256 of the file: `addd35536511597a02fa0a9ff1e5284677b8883b83e986e43f15a3db996b903e`.
A maintainer should diff it against the file at the EFF URL above before release.
