# M2 spikes (L00 section 2)

Status: S4 and S5 partly done on the owner's Windows 11 machine (October 2026). S1, S2, S3 not started.
Spike programs were throwaway code kept outside the repo. Canary strings only.

## S4: Windows Hello (`KeyCredentialManager`)

| Check | Result |
|---|---|
| `IsSupportedAsync` | true |
| Create key (`ReplaceExisting`) | Success |
| Same challenge, same process | 17 of 17 signatures byte-identical (run cut off by a timeout before 20) |
| Reopen with `OpenAsync` in a new process | Success; 3 of 3 identical to the first process |
| Signature length | 256 bytes (enough input for HKDF, docs/04 section 8) |
| Prompt latency | about 5 s per signature |
| Fresh user-presence prompt on every signature | **Verified** (2026-10-13, 5 of 5 signatures in a new process): the owner saw 5 Hello prompts; signatures took 9.5, 9.8, 10.4, 4.9 and 4.1 s and were byte-identical (hash prefix `0205656e`, same as the first run). This closes open question 1 of PR #35 |
| After reboot | NOT YET TESTED (owner action) |
| Cancelled prompt, Hello not set up, credential deleted | NOT YET TESTED (do in L01) |

Verdict so far: pass. The deterministic-signature design in docs/04 section 8 works. The reboot check and the failure paths remain open, so the fallback decision is not closed.

## S5: clipboard hygiene and capture exclusion

| Check | Result |
|---|---|
| Write with `ExcludeClipboardContentFromMonitorProcessing`, `CanIncludeClipboardContentInHistory=0`, `CanUploadToCloudClipboard=0` | Write succeeds |
| Fingerprint with `GetClipboardSequenceNumber` read after `CloseClipboard` | Works: unchanged after our write ("still ours"), changed after another app copied |
| Win+V history shows the plain canary / hides the hygienic one | NOT REPORTED |
| Cloud clipboard | NOT TESTED |
| `WDA_EXCLUDEFROMCAPTURE` toggles | Succeeds |
| Snipping Tool | The window disappeared from the capture; owner's description was ambiguous about exact result |
| PrintScreen, Teams/Zoom share | NOT REPORTED |

Notes for L01:
- One copy bumps the sequence number by 8 (286, 294, 302), so record it after `CloseClipboard`, never before or per format.
- Treat a null or unreadable fingerprint as "not ours" only through B03's existing rule (unconditional clear).
