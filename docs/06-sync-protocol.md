# 06 — Sync Protocol (v1 draft)

## 1. Problem statement
Google Drive, iCloud and plain folders offer **no transactions, no server-side merge, and weak change notification**. Two devices writing one file corrupts it. We need eventual consistency across N devices with zero server logic, over untrusted storage, without data loss.

## 2. Design summary
1. **Single-writer files.** Each device writes only inside its own directory. No file has two writers.
2. **Append-only encrypted op-log** per device, as numbered immutable *segments*.
3. **Deterministic merge** (field-level last-writer-wins on Hybrid Logical Clocks) — a state-based CRDT, so merge is commutative, associative and idempotent: devices converge regardless of arrival order or duplication.
4. **Snapshots** for fast onboarding and compaction.
5. **Hash chains + manifests** to detect truncation / rollback by a hostile provider.
6. Backup is simply "the cloud directory contains everything needed to rebuild the vault" (snapshot + segments).

## 3. Remote layout

```
<provider root>/AryaVault/<vault_id>/
├─ header-<epoch>-<version>-<device>.bin   # immutable; active = highest (epoch, version, device) per doc 04 §5
├─ devices/
│  └─ <device_id>/
│     ├─ 0000000001.seg         # immutable, written once
│     ├─ 0000000002.seg
│     └─ manifest-<counter>.bin # immutable, monotonically numbered; owner writes a new one, deletes older (§6.2)
└─ snapshots/
   └─ <hlc>-<device_id>.snap    # immutable
```
All names are opaque hex; nothing derived from item content.

## 4. Hybrid Logical Clock (HLC)
- `hlc = (physical_ms: 48 bits, counter: 16 bits)`; ordering key is `(hlc, device_id)`.
- Local event: `pt = max(now_ms, last.pt)`; if `pt == last.pt` then counter+1 else counter=0.
- On receiving remote hlc `r`: `pt = max(now_ms, last.pt, r.pt)`, counter updated per standard HLC rules.
- **Skew policy (review M5):** the receive rule above **always adopts** the remote `pt`. This is required: it is what lets any later edit on any device outrank a future-dated op, so a bad clock cannot dominate LWW forever. A remote op whose `pt > now + 24 h` is applied but flagged ("device X's clock is ahead"). A segment containing an op with `pt > now + 1 year` is treated as corrupt and quarantined (prevents poisoning every device's clock far into the future). Display timestamps (`created_at`, "edited on") use wall-clock fields, never HLC.

## 5. Ops and merge

### 5.1 Op
```
Op { item_id, key, value | tombstone, hlc, device_id, base_hlc }
```
- `base_hlc` = the hlc of the register value the author saw when editing (null if new).
- Segment = ordered list of ops produced by one device (≤1 MiB plaintext). Local saves are immediate; **uploads are batched ≥15 s** and flushed at once on lock/background (review L1: fewer, larger segments leak less timing and cost fewer API calls).

### 5.2 Merge rule (per register)
```
winner = argmax over (hlc, device_id) of {current, incoming}
```
- Loser value goes to `field_history`.
- Applying an op already applied (same `(device_id, seq, index)`) is a no-op (idempotent).
- Delete is the register `deleted=true` with its `deleted_hlc`. **Visibility (review M6):** an item is visible iff `deleted == false` **or** any of its field registers has `hlc > deleted_hlc`. So an edit made after a delete (including on another device that had not yet seen the delete) resurrects the item, deterministically on every replica (data-preserving choice).

### 5.3 Concurrent edits are a derived view, not a merge step (review H3)
Merge (§5.2) is a pure LWW CRDT and **never creates items**. Detecting "concurrent edit" while merging depends on arrival order (a causally later edit that arrives *before* its predecessor looks concurrent) and would create spurious copies that cannot be retracted. Instead:

- Every stored version in `field_history` keeps its `base_hlc`. Together they form an ancestry graph per field.
- A non-winning version `L` is **concurrent** with the winner `W` iff `L` is not an ancestor of `W` (following `base_hlc` links through retained versions). This is computed at read time from the full set of ops, so it is identical on every replica regardless of arrival order.
- The UI shows an "Other versions" badge and a compare/restore screen for any field with concurrent losers. Nothing is created automatically.
- For long text (`note.body`) the user can choose **Keep as separate note**, which creates a new item through ordinary ops (new UUIDv7 by the user's device), so there is no duplicate-creation problem.
- Collections (`tags`, `urls`) are stored as per-element registers (add/remove-wins by LWW per element), so concurrent edits merge naturally.
- If an ancestor version was pruned from history (limits in doc 05 §8), the loser is conservatively treated as concurrent (shown, not hidden).

### 5.4 Convergence argument
Each register merges via a total order `(hlc, device_id)` → `max` is commutative, associative, idempotent. Element-wise application of independent registers composes. Tombstones are registers. Conflicts are a derived read-time view and add no state to merge (§5.3). Therefore all replicas that receive the same set of ops reach the same state. Property tests verify this (doc 11).

## 6. Segments, manifests and integrity

**Segment upload** (device D, next seq n), review H2:
1. **Freeze first.** Collect pending local ops → plaintext CBOR list → pad → AEAD under `K_log` (doc 04 §6, `prev_hash` = hash of D's segment n-1). Persist the finished ciphertext bytes, the ops they contain and `n` in a local **outbox** table, in one DB transaction. New local ops after this point go into the next segment.
2. `put_if_absent devices/D/<n>.seg` with the **outbox bytes**. If it already exists with identical bytes (crash after upload) → success; mark the outbox entry uploaded. Retries always send the identical bytes, so a random nonce can never make a retry look like tampering.
3. If it exists with **different** bytes, another writer has used this `device_id` and `seq`. This is **not impossible**: restoring an OS/device backup, cloning a VM or disk, or a failed migration reuses the identity. Run fork recovery (§6.1). Never overwrite.
4. After a successful upload, publish a new manifest (§6.2) subject to the rate limit there.

**Segment download**:
1. Discover new files (change feed or directory listing diff).
2. For each device E, process segments in increasing seq: verify AEAD; verify `prev_hash` chain; apply ops in a single DB transaction; record in `segment_seen`.
3. **Gap handling:** a missing seq in the middle → stop for that device, retry later (eventual consistency). Persisting beyond 24 h while other devices' manifests confirm the head → raise "possible deletion" warning and fall back to snapshot.
4. **Rollback detection:** if any manifest says device E reached seq k but E's directory shows < k, or `prev_hash` mismatch, surface a **tamper/rollback warning** (don't silently overwrite local state). Local DB is never rolled back automatically.

### 6.1 Fork recovery (duplicate `device_id`)
Triggered when `put_if_absent` finds different bytes at `(device_id, n)`, or when a remote segment from *this* device's id has a hash that does not match this device's own record.
1. Stop uploading under this `device_id`; do not delete or modify anything remote.
2. Generate a new random `device_id` and create its directory; seq restarts at 1.
3. Re-emit every local op not confirmed to be in some remote segment, as new ops. This is safe because merge is idempotent and ops carry their own `(hlc, device_id-of-author)`; note the original HLC values are kept, so ordering is unaffected.
4. Download and merge everything under the old id (the other fork's segments are ordinary segments and merge normally).
5. Show the user a notice ("This device appears to have been restored or cloned. It has been given a new identity; no data was lost.").

### 6.2 Manifests: monotonic and immutable (review H5, L2)
- A manifest is `{ counter, own_head: (seq, hash), seen: { device_id → (seq, hash) }, device_name, written_at }`, encrypted under `K_manifest`, stored as `manifest-<counter>.bin` and **never overwritten**. The owner writes counter+1 and deletes older manifests (keeping the last 2).
- Each reader stores the highest manifest `counter` it has seen per device. If the provider serves a lower counter, or a device's manifests disappear while its segments remain, show a **rollback warning** (the provider may be withholding newer state).
- Rate limit: publish a new manifest at most every 5 minutes, or sooner when the device's `own_head` changes or a flush on lock/background occurs.
- Rollback detection remains best-effort against a hostile provider: it can show every device an equally stale but self-consistent view. A device that has seen newer state remembers it locally and will alarm; an entirely new device cannot.

## 7. Provider abstraction

```rust
trait SyncProvider {
  async fn list(&self, prefix: &Path) -> Result<Vec<RemoteEntry>>;      // name, size, etag/version, id
  async fn get(&self, path: &Path) -> Result<Bytes>;
  async fn put_if_absent(&self, path: &Path, bytes: Bytes) -> Result<PutOutcome>; // Created | AlreadyExists(etag)
  async fn put_overwrite(&self, path: &Path, bytes: Bytes, expect: Option<Etag>) -> Result<Etag>; // manifest only
  async fn delete(&self, path: &Path) -> Result<()>;
  async fn changes(&self, cursor: Option<Cursor>) -> Result<(Vec<Change>, Cursor)>; // optional; fallback to list
  fn capabilities(&self) -> Capabilities; // push notifications? cas? case-sensitive names? max file size
}
```
Provider semantics are tested by one shared **conformance suite** that every provider must pass.

### 7.1 Google Drive provider
- Scope: **`https://www.googleapis.com/auth/drive.appdata`** only — files live in the hidden `appDataFolder`, invisible to the user's Drive UI and inaccessible to other apps. (This scope is classified non-sensitive; confirm current verification requirements at implementation time.)
- Auth: OAuth 2.0 with PKCE. Desktop: loopback redirect; Android: Credential Manager / Google Identity authorization; iOS/macOS: `ASWebAuthenticationSession`. Refresh token stored in OS keystore.
- Paths are flattened: Drive has folders by ID, not path; the provider maintains a path→fileId cache and creates folders idempotently.
- **Drive permits duplicate file names in the same folder.** `put_if_absent` therefore: query by name → if exists return AlreadyExists; else create; after create, re-query — if multiple exist, the **earliest `createdTime` (tie: lowest id) wins deterministically** and others are deleted by their creator. Because files are single-writer, duplicates arise only from retries of the same device.
- Change detection: `changes.list` with `startPageToken` (spaces=appDataFolder) polled every 60 s foreground / OS-scheduled in background; optional `changes.watch` is not usable from clients without a server, so polling it is.
- Rate limits: exponential backoff with jitter, honor `Retry-After`.

### 7.2 iCloud (Apple platforms only)
- Primary: **CloudKit private database**, custom zone `AryaVault-<vault_id>`. Each segment/snapshot/manifest = a record with a `CKAsset`/bytes field; record names are the opaque paths.
- Benefits: **silent push notifications** via `CKDatabaseSubscription` (near-instant sync), per-record change tags (real compare-and-swap for manifests), server change tokens.
- `put_if_absent` = save with `.ifServerRecordUnchanged` on a new record ID (fails if exists).
- Alternative considered: iCloud Drive ubiquity container (file-based, needs `NSFileCoordinator`, placeholder downloads); kept as fallback if CloudKit quota/limits become a problem (ADR-0006).
- **Limitation:** unavailable on Windows/Linux/Android. UI must state this; users mixing ecosystems pick Drive or a folder.
- Migration between providers: "Move sync location" copies the entire remote tree to the new provider, verifies hashes, then switches; old location is left intact until the user deletes it.

### 7.3 Folder provider
- Any local/network path (Syncthing, OneDrive, Dropbox, NAS, USB).
- Writes use temp-file + atomic rename. Watches directory (`notify` crate) with periodic rescan.
- Third-party sync tools may create `name (conflicted copy)` files. Because we are single-writer, these are ignorable duplicates; the importer ignores files not matching the naming pattern and reports them.

## 8. Device lifecycle
| Event | Procedure |
|---|---|
| **First device** | Create vault → generate VK, header-1 → local DB. On enabling sync: upload header, device dir, initial snapshot |
| **Add device** | Install app → choose provider → authenticate → download newest header + newest snapshot → user enters master password (or recovery key) → unwrap VK → apply snapshot → apply segments newer than snapshot → create own device dir |
| **Rename / list devices** | Manifest contains device name (encrypted); UI lists last-seen times |
| **Revoke device** | Device marked revoked locally + epoch rotation (doc 04 §10); revoked device's directory ignored after rotation |
| **Device lost offline > 90 days** | On return: if its next segment predecessor was compacted away → it **rebases**: downloads latest snapshot, re-applies its own unsent local ops on top as new ops (merge is idempotent), continues |
| **App reinstall** | Treated as new device (new device_id). Old device directory becomes inactive and is cleaned up after compaction |

### 8.1 Epoch rotation and late devices (review M4)
After a key rotation (doc 04 §10) the rotation snapshot's `covers` map states, per device, up to which `(seq, hash)` old-epoch data is included.
- A device that comes online and sees a higher epoch: prompt for the master password → unwrap the new VK → re-key its local DB → adopt the rotation snapshot → for its **own** segments with `seq > covers[self]` and any unsent local ops, re-emit the ops under the new epoch (idempotent).
- Segments written in the old epoch by a revoked or late device after the snapshot cut are unreadable to new-epoch devices; they are ignored, and the late device's re-emission (above) is what brings its edits in.
- A revoked device can still read old-epoch files it has the key for, but nothing written under the new epoch.

## 9. Compaction and retention
- Any device may create a snapshot when: ≥ 200 segments since last snapshot or ≥ 7 days and new data.
- Snapshot = full current state (registers + retained history + tombstones) as CBOR, AEAD-encrypted under `K_snap`, plus a `covers: { device_id → (seq, hash) }` map. Carrying the hash lets a device that starts from this snapshot continue verifying each device's hash chain after older segments are deleted (review M8).
- A segment may be deleted by its **author only** once: (a) a snapshot covers it, and (b) all active devices' manifests ack that snapshot (or the device has been inactive > 90 days).
- **Stale devices (review L6):** any device may delete segments of a device that has been inactive >90 days, provided a snapshot covers them, so storage stays bounded when a phone is lost.
- Keep the last **3 snapshots** (rolling backup / point-in-time restore), plus on-demand "pin this snapshot" for the user.
- Cloud footprint estimate: ~3 snapshots × (vault ≤ 10 MB typical) + active segments → well within free quota (Drive 15 GB, iCloud 5 GB).

## 10. Sync scheduler
- Triggers: after local change (debounced), on app foreground, on change notification (CloudKit push), periodic poll (Drive/folder), manual "Sync now", OS background tasks (WorkManager, BGAppRefresh).
- Backoff: 2 s → 4 → … → 15 min cap, reset on success.
- Network: configurable "Wi-Fi only".
- Sync never blocks the UI; status states: `Idle · Syncing · Offline · Needs attention (auth/tamper/clock) · Paused`.

## 11. Error and edge cases
| Case | Behavior |
|---|---|
| Upload interrupted mid-file | Resume/retry; `put_if_absent` same bytes = success |
| Duplicate delivery | Idempotent apply |
| Partially downloaded / corrupt file | AEAD fail → quarantine file, retry, show after N failures |
| Clock set far in future on one device | Flagged (§4) |
| Provider quota full | Pause upload, warn, offer compaction/cleanup |
| OAuth revoked / expired | "Needs attention" – re-auth; local vault untouched |
| User wipes cloud folder | Local vault intact; offer "re-upload from this device" (new snapshot) |
| Two devices create same tag/folder name | Distinct IDs → both exist; UI offers merge |
| Header race (two devices change password concurrently) | Both write `header-<epoch>-<n+1>-<device>.bin` (distinct names); **highest `(epoch, header_version, device_id)` wins**; the loser's change is flagged to its user and can be re-applied |

## 12. Security properties (summary)
- Provider cannot read (AEAD), cannot undetectably modify (AEAD+AAD), cannot undetectably reorder within a device (hash chain), can delete/withhold (detected when peers' manifests disagree; local copy preserved).
- A new device with no peers cannot detect whole-vault rollback — documented limitation.

## 13. Out of scope for v1
Real-time collaborative editing of note bodies, partial/selective sync, shared vaults, server-assisted push for non-Apple providers.
