//! SEC-C08: every release must open the golden vault files of all previous format
//! versions. This test opens `core/testdata/golden/v1` (fake, canary-only data; see its
//! README) with the master password and with the recovery key, and checks every
//! plaintext. The expected values are hard-coded here, not read from the README, and the
//! file hashes are pinned so that any edit or regeneration of an existing golden
//! directory fails CI (golden directories are append-only).
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::path::PathBuf;

use arya_vault_crypto::format::envelope::{
    Envelope, Kind, envelope_hash, open_envelope_at_path, verify_chain,
};
use arya_vault_crypto::format::header::{Header, HeaderCandidate, select_active};
use arya_vault_crypto::format::path::parse_path;
use arya_vault_crypto::hkdf::{SubKeyLabel, subkey};
use arya_vault_crypto::recovery_key;
use arya_vault_crypto::vault_key::{HeaderWraps, unlock_with_password, unlock_with_recovery_key};

const PASSWORD: &str = "CANARY-golden-password-v1";
const RECOVERY_KEY: &str = "MH365-7P3RV-640RY-YD7T8-RN1R5-65JCX-SA-65";
const VAULT_ID: &str = "b00c5e3414af91687d1befad815c084f";
const HEADER: &str = "header-00000001-00000001-d1d1d1d1d1d1d1d1d1d1d1d1d1d1d1d1.bin";
const SEGMENT: &str = "devices/d1d1d1d1d1d1d1d1d1d1d1d1d1d1d1d1/0000000000000001.seg";
const SNAPSHOT: &str = "snapshots/0000000100000001-d1d1d1d1d1d1d1d1d1d1d1d1d1d1d1d1.snap";
const MANIFEST: &str = "devices/d1d1d1d1d1d1d1d1d1d1d1d1d1d1d1d1/manifest-0000000000000001.bin";

const PINNED: [(&str, &str); 4] = [
    (
        HEADER,
        "3bc58c5d74576dfe9662123e66b30d92f57648be2f8e037c6d14670a09699bb0",
    ),
    (
        SEGMENT,
        "ca67341bed22fa41ee201cc1f1fc1d718b1b719f695a6286b0f44f8e170cbb96",
    ),
    (
        SNAPSHOT,
        "6dafe17a51e87994395c581c4acac1a0634e516a949f640d9169b202efcf807a",
    ),
    (
        MANIFEST,
        "981caedfdb46cc2308dfc21c74437abf897d9911f5ae1da1da035af9f5dd5a0e",
    ),
];

fn read(rel: &str) -> Vec<u8> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../testdata/golden/v1");
    std::fs::read(root.join(rel)).unwrap_or_else(|e| panic!("{rel}: {e}"))
}

fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

#[test]
fn sec_c08_golden_files_are_unchanged() {
    for (rel, want) in PINNED {
        assert_eq!(
            hex(&envelope_hash(&read(rel))),
            want,
            "{rel} changed: golden files are append-only"
        );
    }
}

#[test]
fn sec_c08_golden_v1_opens_with_password_and_recovery_key() {
    let header = Header::decode(&read(HEADER)).unwrap();
    assert_eq!(hex(&header.vault_id), VAULT_ID);
    assert_eq!(
        (header.format_version, header.epoch, header.header_version),
        (1, 1, 1)
    );
    assert_eq!(
        (header.kdf.m_kib, header.kdf.t, header.kdf.p),
        (65_536, 3, 1)
    );
    assert_eq!(header.created_at, 1_700_000_000);

    // Active-header selection works on the golden directory listing.
    let sel = select_active(
        &[HeaderCandidate {
            file_name: HEADER.into(),
            header: header.clone(),
        }],
        &header.vault_id,
        0,
    );
    assert_eq!(sel.active, Some(0));

    let wraps = HeaderWraps {
        vault_id: header.vault_id,
        epoch: header.epoch,
        kdf: header.kdf.clone(),
        wrap_pw: header.wrap_pw.clone(),
        wrap_rk: header.wrap_rk.clone(),
    };
    let vk_pw = unlock_with_password(PASSWORD, &wraps).unwrap();
    let rk = recovery_key::parse(RECOVERY_KEY).unwrap();
    let vk_rk = unlock_with_recovery_key(&rk, &wraps).unwrap();
    assert_eq!(vk_pw.expose_secret(), vk_rk.expose_secret());
    assert!(unlock_with_password("CANARY-wrong", &wraps).is_err());

    let key = |label| subkey(&vk_pw, &header.vault_id, label, header.epoch).unwrap();

    let seg_bytes = read(SEGMENT);
    let seg = open_envelope_at_path(
        &key(SubKeyLabel::Log),
        &seg_bytes,
        &header.vault_id,
        &parse_path(SEGMENT).unwrap(),
    )
    .unwrap();
    assert_eq!(
        seg.plaintext.as_slice(),
        b"CANARY-golden-segment-plaintext-v1"
    );
    assert_eq!((seg.fields.kind, seg.fields.seq), (Kind::Segment, 1));
    verify_chain(&seg.fields, None).unwrap();
    assert_eq!(Envelope::decode(&seg_bytes).unwrap().format_version, 1);

    let snap = open_envelope_at_path(
        &key(SubKeyLabel::Snapshot),
        &read(SNAPSHOT),
        &header.vault_id,
        &parse_path(SNAPSHOT).unwrap(),
    )
    .unwrap();
    assert_eq!(
        snap.plaintext.as_slice(),
        b"CANARY-golden-snapshot-plaintext-v1"
    );
    assert_eq!(snap.fields.kind, Kind::Snapshot);

    let man = open_envelope_at_path(
        &key(SubKeyLabel::Manifest),
        &read(MANIFEST),
        &header.vault_id,
        &parse_path(MANIFEST).unwrap(),
    )
    .unwrap();
    assert_eq!(
        man.plaintext.as_slice(),
        b"CANARY-golden-manifest-plaintext-v1"
    );
    assert_eq!((man.fields.kind, man.fields.seq), (Kind::Manifest, 1));
}
