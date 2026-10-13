//! Manual test against the real Windows Hello (needs a person at the screen).
//!
//! ```text
//! cargo test -p arya-vault-platform-windows --test hello_manual -- --ignored --nocapture
//! ```
//!
//! Expect Windows Hello prompts: while enabling (create + two signatures) and once for the
//! unseal. Approve each one. It creates and then deletes the credential
//! `aryavault/quick-unlock/v1`, so run it only on a machine where that is acceptable (it would
//! remove a real quick-unlock credential of AryaVault).
#![cfg(windows)]
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::print_stdout
)]

use arya_vault_crypto::keys::VaultKey;
use arya_vault_platform_windows::windows_hello::WindowsHello;
use arya_vault_platform_windows::{HelloPlatform, HelloProvider};
use arya_vault_session::quick::QuickUnlockProvider;

#[test]
#[ignore = "needs Windows Hello and a person to approve the prompts"]
fn real_hello_seal_and_unseal() {
    let provider = HelloProvider::new(WindowsHello);
    assert!(provider.available(), "Windows Hello is not available here");
    println!("boot id: {:?}", WindowsHello.boot_id());

    let vk = VaultKey::from_bytes([0x5A; 32]);
    println!("sealing: approve the Hello prompts (create + 2 signatures)...");
    let blob = provider.seal(&vk).expect("seal");
    println!("unsealing: approve the Hello prompt...");
    let back = provider.unseal(&blob).expect("unseal");
    assert_eq!(back.expose_secret(), vk.expose_secret());

    provider.revoke();
    println!("revoked; unsealing again must now say the credential is gone");
    assert!(provider.unseal(&blob).is_err());
}
