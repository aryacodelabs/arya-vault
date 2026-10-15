// The Dart side of the app <-> core contract (docs/14, binding).
//
// One method per docs/14 §4 row, same names, same semantics. The real
// implementation is generated over FFI (task A04, wired in L01); until then
// FakeVaultApi implements it in memory. Secrets cross as Uint8List (UTF-8);
// callers wipe them with SecretBytes after the call. Every method may throw
// AppError (docs/14 §2).
import 'dart:typed_data';

import 'dtos.dart';

/// Expected `info().apiVersion` (docs/14 §7). The app asserts equality at
/// startup; a mismatch is a fatal developer error screen.
const int expectedApiVersion = 5;

abstract interface class VaultApi {
  // -------------------------------------------------- 4.1 lifecycle, credentials

  /// Works while locked.
  Future<VaultStatus> status();

  /// Creates the vault **unlocked**; the recovery key is returned once and
  /// `onboardingComplete` stays false until [confirmRecoveryKey] passes.
  Future<RecoveryKeyResult> createVault({
    required Uint8List password,
    required KdfProfile profile,
  });

  /// Compares [answers] with the pending key the core still holds. On success
  /// sets `onboardingComplete`; on mismatch returns false (no retry limit).
  Future<bool> confirmRecoveryKey(List<GroupAnswer> answers);

  /// The groups (zero-based, ascending) the next [confirmRecoveryKey] must
  /// cover: the `challenge` of the last create / regenerate, or the redrawn one
  /// after a mismatch. Throws `locked` while locked and `validation` when no key
  /// is pending.
  Future<List<int>> recoveryChallenge();

  /// Throws [AppErrorCode.wrongCredentials] (or [AppErrorCode.busy] while the
  /// local failure delay runs).
  Future<void> unlock(Uint8List password);

  /// Uses the platform quick-unlock provider inside the core.
  Future<void> unlockQuick();

  /// Zeroizes keys and closes the database. Idempotent.
  Future<void> lock();

  Future<void> changePassword({
    required Uint8List oldPassword,
    required Uint8List newPassword,
    required bool rotateKeys,
  });

  /// Leaves the vault unlocked (docs/04 §9).
  Future<void> recoverWithKey({
    required Uint8List recoveryKey,
    required Uint8List newPassword,
  });

  /// Sets `onboardingComplete = false` until confirmed again.
  Future<RecoveryKeyResult> regenerateRecoveryKey({
    required Uint8List password,
    required bool rotateKeys,
  });

  Future<void> quickUnlockEnable();
  Future<void> quickUnlockDisable();

  /// Re-authentication before sensitive actions.
  Future<bool> verifyPassword(Uint8List password);

  // ---------------------------------------------- 4.2 items and folders (unlocked)

  Future<List<ItemSummary>> list(ListFilter filter, Page page);
  Future<List<ItemSummary>> search(SearchQuery query);
  Future<int> itemCount();
  Future<ItemView> getItem(String id);
  Future<Uint8List?> reveal(String id, StdField field);
  Future<Uint8List?> revealCustom(String id, String elementId);
  Future<String> createItem(NewItem item);
  Future<void> setField(String id, StdField field, String value);
  Future<void> clearField(String id, StdField field);

  /// Atomic batch: one transaction.
  Future<void> setFields(String id, Map<StdField, String> fields);
  Future<bool> toggleFavorite(String id);
  Future<void> moveToFolder(String id, String? folderId);
  Future<void> addTag(String id, String tag);
  Future<void> removeTag(String id, String tag);
  Future<String> addUrl(String id, String url);
  Future<void> setUrl(String id, String urlId, String url);
  Future<void> removeUrl(String id, String urlId);
  Future<String> addCustomField(
    String id,
    CustomKind kind,
    String label,
    String value,
  );
  Future<void> setCustomValue(String id, String elementId, String value);
  Future<void> setCustomLabel(String id, String elementId, String label);
  Future<void> removeCustomField(String id, String elementId);
  Future<void> deleteItem(String id);
  Future<void> restoreItem(String id);
  Future<void> purgeItem(String id);
  Future<List<TrashEntry>> listTrash();
  Future<int> emptyTrash();
  Future<int> purgeExpired();
  Future<List<VersionInfo>> history(String id, StdField field);
  Future<Uint8List?> revealVersion(String id, StdField field, int hlcMs);
  Future<void> restoreVersion(String id, StdField field, int hlcMs);
  Future<String> createFolder(String name, String? parentId);
  Future<void> renameFolder(String id, String name);
  Future<void> deleteFolder(String id);
  Future<List<Folder>> listFolders();

  // -------------------------------------------- 4.3 generator, strength, health

  Future<Uint8List> generatePassword(PasswordOptions options);
  Future<Uint8List> generatePassphrase(PassphraseOptions options);
  Future<double> entropyBits(EntropyOptions options);
  Future<Strength> strength(Uint8List password);

  /// Works **while locked**; used by onboarding and recovery.
  Future<PolicyResult> checkMasterPassword(Uint8List password);
  Future<HealthReport> healthReport();

  // ------------------------------------------------------- 4.4 import / export

  Future<ImportPreview> importPreview({
    required ImportFormat format,
    required Uint8List file,
  });
  Future<ImportResult> importCommit({
    required String previewToken,
    required ImportOptions options,
  });
  Future<Uint8List> exportCsv(AcknowledgePlaintextRisk token);
  Future<Uint8List> exportEncrypted(Uint8List exportPassword);
  Future<ImportResult> importEncrypted({
    required Uint8List file,
    required Uint8List exportPassword,
  });

  // ------------------------------------------------------------- 4.5 settings

  Future<AppSettings> getSettings();
  Future<void> setSettings(AppSettings settings);

  // --------------------------------------------------------- 4.6 diagnostics

  Future<InfoDto> info();
  Future<Uint8List> exportDiagnostics();
}
