//! Placeholder target proving the fuzz toolchain works in CI.
//! It exercises no project code; later tasks add real `fuzz_<parser>` targets.
#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    // Trivial deterministic property so the harness has something to run.
    let doubled: Vec<u8> = data.iter().chain(data.iter()).copied().collect();
    assert_eq!(doubled.len(), data.len() * 2);
});
