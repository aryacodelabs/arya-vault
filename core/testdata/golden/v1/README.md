# Golden vault v1 (FAKE test data)

All values below are fake and canary-labelled. Generated deterministically with the
non-secure `deterministic-rng` feature from a fixed seed. **Append-only: never regenerate
or edit this directory; add `v2/` for a new format.**

Regenerate into a NEW empty directory (to verify reproducibility) from `core/`:

```sh
cargo run -p arya-vault-crypto --example gen_golden --features deterministic-rng -- /tmp/golden-v1-check
diff -r /tmp/golden-v1-check testdata/golden/v1   # only README.md may differ if edited
```

| item | value |
|---|---|
| master password | `CANARY-golden-password-v1` |
| recovery key | `MH365-7P3RV-640RY-YD7T8-RN1R5-65JCX-SA-65` |
| vault_id | `b00c5e3414af91687d1befad815c084f` |
| device_id | `d1d1d1d1d1d1d1d1d1d1d1d1d1d1d1d1` |
| epoch / header_version | 1 / 1 |
| KDF | argon2id v19, m=65536 KiB, t=3, p=1, salt `d558a1baab60d1b7c00b24c448b662d1` |
| created_at | 1700000000 |
| seed (ASCII) | `aryavault-golden-v1-seed-0000000` |

| file | plaintext | SHA-256 of file |
|---|---|---|
| `header-00000001-00000001-d1d1d1d1d1d1d1d1d1d1d1d1d1d1d1d1.bin` | (header) | `3bc58c5d74576dfe9662123e66b30d92f57648be2f8e037c6d14670a09699bb0` |
| `devices/d1d1d1d1d1d1d1d1d1d1d1d1d1d1d1d1/0000000000000001.seg` | `CANARY-golden-segment-plaintext-v1` | `ca67341bed22fa41ee201cc1f1fc1d718b1b719f695a6286b0f44f8e170cbb96` |
| `snapshots/0000000100000001-d1d1d1d1d1d1d1d1d1d1d1d1d1d1d1d1.snap` | `CANARY-golden-snapshot-plaintext-v1` | `6dafe17a51e87994395c581c4acac1a0634e516a949f640d9169b202efcf807a` |
| `devices/d1d1d1d1d1d1d1d1d1d1d1d1d1d1d1d1/manifest-0000000000000001.bin` | `CANARY-golden-manifest-plaintext-v1` | `981caedfdb46cc2308dfc21c74437abf897d9911f5ae1da1da035af9f5dd5a0e` |
