# Security Policy

AryaVault (working name) is a password manager; security reports are taken seriously.

## Supported versions
Until 1.0, only the latest release (and the current `main`) receives security fixes. After 1.0, the latest minor release and the previous one.

## Reporting a vulnerability
**Do not open a public issue.** Report privately via one of:
- GitHub "Report a vulnerability" (private security advisory) on this repository
- Email: `security@<project-domain>` (**TODO: set up before first public release**), PGP key published in the repo

Please include: affected version/platform, reproduction steps or proof of concept, impact assessment, and whether you want credit.

## Our commitments (targets)
| Step | Target |
|---|---|
| Acknowledge report | 3 business days |
| Initial assessment/severity | 7 days |
| Fix for critical/high | ≤ 14 days (critical ≤ 7 where feasible) |
| Coordinated public disclosure | After a fix is available, typically within 90 days of report |

We credit reporters unless they prefer anonymity. We will not pursue legal action against good-faith research that follows this policy: no access to other people's data, no service disruption, no social engineering of maintainers/users.

## In scope
- Cryptographic flaws (key handling, nonce reuse, KDF downgrade, format confusion)
- Sync-protocol flaws (undetected tampering/rollback, data loss, plaintext leakage to providers)
- Secret exposure (logs, clipboard, backups, crash dumps, screenshots)
- Memory safety issues in the Rust core / FFI
- Autofill matching bypass (phishing)
- Supply-chain issues in build/release pipeline

## Out of scope
- Attacks requiring root/admin/kernel malware or a keylogger on an unlocked device (see docs/03-threat-model.md §5)
- Weak user-chosen master passwords
- Loss of data caused by losing both master password and recovery key (by design)
- Issues in third-party cloud providers themselves

## Safe harbor and testing guidance
Use your own vaults and accounts only. Do not test against other users' cloud accounts.

## Security documentation
Threat model: `docs/03-threat-model.md` · Crypto spec: `docs/04-crypto-spec.md` · Requirements: `docs/08-security-requirements.md`.
