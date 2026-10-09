# Storage fixture v1 (golden file, append-only)

`vault.db` is an encrypted AryaVault database at **schema_version 1**, written by
`Db::create` and used by the migration tests. Never regenerate or edit it: a
future release must still open this exact file. New schema versions get a new
`v<N>/` directory.

- Key (fake, test-only): 32 bytes of `0x42` (`4242...42`)
- vault_id `A1` x16, device_id `B2` x16, epoch 1, header_version 1
- Contents: one `login` item (id `01` x16) with canary field
  `CANARY-FIXTURE-v1`, one history row, one folder, one device, one local op,
  one FTS document. All values are fake canary strings.

Created with:

```
ARYA_WRITE_FIXTURE=1 cargo test -p arya-vault-storage --lib -- --ignored regenerate_v1_fixture
```

(the test refuses to overwrite an existing file).
