# L02: M2 exit check and usability test (local + a cloud session for the document)

## Part 1: Exit-check document (cloud-suitable once L01 evidence exists)
Produce `docs/reviews/m2-exit-check.md` in the same honest format as `docs/reviews/m1-exit-check.md`:
- Section 1 "**Read this first: what is not verified**" listing every gap.
- Roadmap M2 exit criteria (`docs/10`): US-01..US-09, US-13, US-15 on Windows; usability test of onboarding with 5 users; SEC-A*, SEC-S*, SEC-H01-H03 verified on Windows.
- A requirement table for each `SEC-A*`, `SEC-S*`, `SEC-H01-H03` plus SEC-C06 (lock), SEC-A06 (rotation) with evidence (test names, manual checklist rows from `m2-windows-evidence.md`) and status words: **verified / partially verified / not verified**.
- Benchmarks on a **reference Windows laptop** (not a cloud container): cold unlock, 20k-item list/search, memory, start time.
- Deferred items and every "Spec questions" raised in A00-A04/B01-B03/L01.

## Part 2: Usability test (owner + 5 participants)
Tasks from `docs/07` §11, run on the Windows build with fake data, think-aloud, screen recording consented and stored locally:
1. Create a vault and save the recovery key (success = the verify step passes without help).
2. Add a login using the generator, find it again via search, copy the password.
3. Lock the app and unlock with Hello and with the password.
4. "Forget" the password and recover with the recovery key.
5. Change the master password; decide whether to rotate keys (observe comprehension of the checkbox).
Measure: success without help, time, errors, and verbatim confusion points. Record in `docs/reviews/m2-usability.md`; severity-rank issues; fix blockers before M3.

## Part 3: Release gate for 0.1 alpha (Windows + Android internal)
M2 delivers the Windows half. Tag `v0.1.0-alpha.1` only when: exit check has no unaccepted "not verified" MUST item; usability blockers fixed; unsigned MSIX/zip attached to a GitHub prerelease with checksums.
