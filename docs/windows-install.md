# AryaVault for Windows: install, verify, back up

> **This is a preview, not a finished product.** It has not had an independent security audit,
> it is **not code-signed**, and it has **no sync**: your vault lives only on this PC. Use it with
> data you can afford to lose, and keep encrypted exports (see "Back up" below).

Requirements: Windows 10 (2004) or Windows 11, 64-bit. Windows Hello quick unlock needs Hello set up
with a PIN or biometrics and a TPM.

## Verify the download

The release page lists a `SHA256SUMS.txt`. Check the file you downloaded against it:

```powershell
Get-FileHash .\AryaVault-<version>-windows-x64-portable.zip -Algorithm SHA256
```

The hash must match the line in `SHA256SUMS.txt` exactly. If it does not, do not run it.

## Portable zip (recommended)

1. Unzip it anywhere you can write to, for example `C:\Users\<you>\AryaVault`. Do not run it from
   inside the zip.
2. Run `AryaVault\arya_vault.exe`.
3. **Windows SmartScreen will warn "Windows protected your PC / unknown publisher".** That is
   because the build is not signed (signing is planned). Choose *More info*, then *Run anyway*,
   only if the checksum above matched.

To remove it: delete the folder. Your vault is not inside it (next section), so removing the program
does not delete your data.

## Where your data is

`%LOCALAPPDATA%\AryaVault\vault\`: the encrypted vault database, a small header file and the
quick-unlock files. The folder is restricted to your Windows account and excluded from Windows Search
indexing. Everything in it is encrypted except the header file, which holds only key-wrapping
parameters. Nothing is sent anywhere: the app makes no network connections.

## Back up (important)

- **Write down the recovery key** when you create the vault (paper is the safest). Without the master
  password *and* the recovery key, nobody, including the project, can open your vault.
- **There is no sync yet.** If this PC is lost or the disk fails, the vault is gone. Use
  *Tools > Export* to make an **encrypted** export now and then, and keep it on another drive.
- Do not use the plain CSV export except to move data to another tool, and delete that file afterwards.

## MSIX package (not installable while unsigned)

`AryaVault-<version>-windows-x64.msix` is built for completeness, but **it cannot be installed
yet**. Tested on Windows 11 (build 26300): `Add-AppxPackage -AllowUnsigned` refuses it with
0x80073D2C (the publisher is not in the unsigned namespace), and even with a publisher that is,
with 0x80073D2B ("an unsigned package cannot include Executable activations"), which every
desktop app like this one has. It needs a signing certificate that the machine trusts (planned for
M9). **Use the portable zip.** Whether uninstalling a signed MSIX would delete the vault data is
still untested.

## Update

There is no auto-update and no update check. To update: close AryaVault, unzip the new version over
a new folder, and run it; your vault stays where it is. Make an encrypted export first.

## Report a problem

See `SECURITY.md` for security issues. Please never attach your vault files or a recovery key to a bug
report.
