# AryaVault (working name)

An open-source, **local-first, zero-knowledge** password and notes manager for **Windows, macOS, Linux, Android and iOS**. Your vault lives encrypted on your devices. Optionally it is backed up and synced through **your own** Google Drive or iCloud account. There is no AryaVault server and no web app.

> **Status: documentation / design phase. No code has been written yet.**

## Core promises

1. **Local-first.** The app works fully offline. The cloud is optional.
2. **Zero-knowledge.** Cloud providers only ever see ciphertext. We never see anything: there is no server.
3. **No invented crypto.** Only vetted primitives from audited libraries (Argon2id, XChaCha20-Poly1305, HKDF-SHA256).
4. **Recovery key is mandatory.** No server means no password reset. See [UX and recovery](docs/07-ux-and-recovery.md).
5. **Open source.** Core and apps are public so anyone can review them.

## Documentation index

| # | Document | Purpose |
|---|---|---|
| 01 | [Product requirements](docs/01-product-requirements.md) | Goals, non-goals, user stories, feature scope |
| 02 | [Architecture](docs/02-architecture.md) | Components, diagrams, repo layout, platform layer |
| 03 | [Threat model](docs/03-threat-model.md) | Assets, adversaries, attack surface, mitigations |
| 04 | [Cryptography spec](docs/04-crypto-spec.md) | Key hierarchy, KDF, ciphers, formats, key rotation |
| 05 | [Data model](docs/05-data-model.md) | Items, fields, local database schema |
| 06 | [Sync protocol](docs/06-sync-protocol.md) | Op-log, HLC, merge rules, providers, compaction |
| 07 | [UX and recovery](docs/07-ux-and-recovery.md) | Onboarding, unlock, recovery-key flows, screens |
| 08 | [Security requirements](docs/08-security-requirements.md) | Numbered, testable requirements per platform |
| 09 | [Tech stack and decisions (ADRs)](docs/09-tech-stack-and-decisions.md) | Why each technology was chosen |
| 10 | [Roadmap](docs/10-roadmap.md) | Phases, milestones, exit criteria |
| 11 | [Testing strategy](docs/11-testing-strategy.md) | Unit, property, fuzz, sync simulation, platform matrix |
| 12 | [Release and distribution](docs/12-release-and-distribution.md) | Signing, stores, updates, reproducibility |
| — | [SECURITY.md](SECURITY.md) | Vulnerability disclosure policy |
| — | [CONTRIBUTING.md](CONTRIBUTING.md) | Contribution rules, crypto-change policy |

## Decisions already made

| Question | Decision |
|---|---|
| Account recovery | A forgotten master password with no recovery key means permanent data loss. A recovery key is generated at vault creation and onboarding cannot be completed without confirming it was saved. |
| Platform order | **Windows + Android first**, then macOS, iOS, Linux. |
| Open source | Yes, under MPL-2.0 (ADR-0009, confirmed). |
| Crypto | Vetted libraries and published designs only. No custom primitives or protocols. |
| Stack | Flutter UI + shared Rust core (see [ADRs](docs/09-tech-stack-and-decisions.md)). |

## Open items (need an owner decision)

- Final product name and domain / bundle identifiers.
- Budget for an external security audit before 1.0.
