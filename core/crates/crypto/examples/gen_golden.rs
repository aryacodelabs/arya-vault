//! Generates the golden vault files in a new, empty directory (docs/04 §13, SEC-C08).
//!
//! ```text
//! cd core
//! cargo run -p arya-vault-crypto --example gen_golden --features deterministic-rng -- testdata/golden/v1
//! ```
//!
//! Everything is derived from fixed seeds with the **non-secure** deterministic RNG, so the
//! output is bit-for-bit reproducible. All data is fake and canary-labelled. Golden
//! directories are append-only: this tool refuses to write into a non-empty directory, and
//! an existing version directory must never be regenerated.
#![allow(
    clippy::print_stdout,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic
)]

use std::fs;
use std::path::Path;

use arya_vault_crypto::format::envelope::{EnvelopeFields, Kind, envelope_hash, seal_envelope};
use arya_vault_crypto::format::header::Header;
use arya_vault_crypto::format::path::{header_path, manifest_path, segment_path, snapshot_path};
use arya_vault_crypto::hkdf::{SubKeyLabel, subkey};
use arya_vault_crypto::kdf::KdfParams;
use arya_vault_crypto::recovery_key;
use arya_vault_crypto::rng::{DeterministicRng, Rng};
use arya_vault_crypto::vault_key::create_vault;

const PASSWORD: &str = "CANARY-golden-password-v1";
const SEED: [u8; 32] = *b"aryavault-golden-v1-seed-0000000";
const DEVICE_ID: [u8; 16] = [0xd1; 16];
const EPOCH: u32 = 1;
const HEADER_VERSION: u32 = 1;
const CREATED_AT: u64 = 1_700_000_000;
const SNAPSHOT_HLC: u64 = 0x0000_0001_0000_0001;
const SEGMENT_PLAINTEXT: &[u8] = b"CANARY-golden-segment-plaintext-v1";
const SNAPSHOT_PLAINTEXT: &[u8] = b"CANARY-golden-snapshot-plaintext-v1";
const MANIFEST_PLAINTEXT: &[u8] = b"CANARY-golden-manifest-plaintext-v1";

fn write(root: &Path, rel: &str, bytes: &[u8]) {
    let p = root.join(rel);
    fs::create_dir_all(p.parent().unwrap()).unwrap();
    fs::write(&p, bytes).unwrap();
}

fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

fn main() {
    let out = std::env::args()
        .nth(1)
        .expect("usage: gen_golden <new-empty-output-dir>");
    let root = Path::new(&out);
    if root.exists() && fs::read_dir(root).unwrap().next().is_some() {
        panic!("{out} is not empty: golden directories are append-only, never regenerate");
    }
    fs::create_dir_all(root).unwrap();

    let mut rng = DeterministicRng::from_seed(SEED);
    let mut salt = [0u8; 16];
    rng.fill_bytes(&mut salt).unwrap();
    let nv = create_vault(PASSWORD, KdfParams::floor(salt), EPOCH, &mut rng).unwrap();
    let w = &nv.wraps;
    let header = Header::new(
        w.vault_id,
        HEADER_VERSION,
        EPOCH,
        w.kdf.clone(),
        w.wrap_pw.clone(),
        w.wrap_rk.clone(),
        CREATED_AT,
    );

    write(
        root,
        &header_path(EPOCH, HEADER_VERSION, &DEVICE_ID),
        &header.encode().unwrap(),
    );

    let fields = |kind, seq| EnvelopeFields {
        kind,
        vault_id: w.vault_id,
        epoch: EPOCH,
        device_id: DEVICE_ID,
        seq,
        prev_hash: [0; 32],
    };
    let k = |label| subkey(&nv.vault_key, &w.vault_id, label, EPOCH).unwrap();

    let seg = seal_envelope(
        &k(SubKeyLabel::Log),
        &fields(Kind::Segment, 1),
        SEGMENT_PLAINTEXT,
        &mut rng,
    )
    .unwrap();
    write(root, &segment_path(&DEVICE_ID, 1), &seg);
    let snap = seal_envelope(
        &k(SubKeyLabel::Snapshot),
        &fields(Kind::Snapshot, 0),
        SNAPSHOT_PLAINTEXT,
        &mut rng,
    )
    .unwrap();
    write(root, &snapshot_path(SNAPSHOT_HLC, &DEVICE_ID), &snap);
    let man = seal_envelope(
        &k(SubKeyLabel::Manifest),
        &fields(Kind::Manifest, 1),
        MANIFEST_PLAINTEXT,
        &mut rng,
    )
    .unwrap();
    write(root, &manifest_path(&DEVICE_ID, 1), &man);

    let rk = recovery_key::encode(&nv.recovery_key).to_string();
    let readme = format!(
        "# Golden vault v1 (FAKE test data)\n\n\
All values below are fake and canary-labelled. Generated deterministically with the\n\
non-secure `deterministic-rng` feature from a fixed seed. **Append-only: never regenerate\n\
or edit this directory; add `v2/` for a new format.**\n\n\
Regenerate into a NEW empty directory (to verify reproducibility) from `core/`:\n\n\
```sh\ncargo run -p arya-vault-crypto --example gen_golden --features deterministic-rng -- /tmp/golden-v1-check\ndiff -r /tmp/golden-v1-check testdata/golden/v1   # only README.md may differ if edited\n```\n\n\
| item | value |\n|---|---|\n\
| master password | `{PASSWORD}` |\n\
| recovery key | `{rk}` |\n\
| vault_id | `{vault_id}` |\n\
| device_id | `{device}` |\n\
| epoch / header_version | {EPOCH} / {HEADER_VERSION} |\n\
| KDF | argon2id v19, m=65536 KiB, t=3, p=1, salt `{salt}` |\n\
| created_at | {CREATED_AT} |\n\
| seed (ASCII) | `aryavault-golden-v1-seed-0000000` |\n\n\
| file | plaintext | SHA-256 of file |\n|---|---|---|\n\
| `{hp}` | (header) | `{hh}` |\n\
| `{sp}` | `{SEG}` | `{sh}` |\n\
| `{np}` | `{SNAP}` | `{nh}` |\n\
| `{mp}` | `{MAN}` | `{mh}` |\n",
        vault_id = hex(&w.vault_id),
        device = hex(&DEVICE_ID),
        salt = hex(&w.kdf.salt),
        hp = header_path(EPOCH, HEADER_VERSION, &DEVICE_ID),
        hh = hex(&envelope_hash(&header.encode().unwrap())),
        sp = segment_path(&DEVICE_ID, 1),
        sh = hex(&envelope_hash(&seg)),
        np = snapshot_path(SNAPSHOT_HLC, &DEVICE_ID),
        nh = hex(&envelope_hash(&snap)),
        mp = manifest_path(&DEVICE_ID, 1),
        mh = hex(&envelope_hash(&man)),
        SEG = String::from_utf8_lossy(SEGMENT_PLAINTEXT),
        SNAP = String::from_utf8_lossy(SNAPSHOT_PLAINTEXT),
        MAN = String::from_utf8_lossy(MANIFEST_PLAINTEXT),
    );
    write(root, "README.md", readme.as_bytes());
    println!("wrote golden vault to {out}");
}
