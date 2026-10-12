// The real VaultApi: every call goes over flutter_rust_bridge to the Rust core
// (core/crates/ffi, docs/14). No logic lives here; see ffi_mappers.dart for the
// DTO conversions. Secrets cross as Uint8List and are the caller's to wipe
// (SecretBytes, docs/14 §1).
import 'dart:io';

import 'package:flutter_rust_bridge/flutter_rust_bridge_for_generated.dart';

import '../rust/api/diagnostics.dart' as dg;
import '../rust/api/dto.dart' as r;
import '../rust/api/items.dart' as it;
import '../rust/api/lifecycle.dart' as lc;
import '../rust/api/settings.dart' as st;
import '../rust/api/tools.dart' as tl;
import '../rust/api/transfer.dart' as tr;
import '../rust/frb_generated.dart';
import 'app_error.dart';
import 'dtos.dart';
import 'ffi_mappers.dart';
import 'vault_api.dart';

/// File name of the core library next to the executable (Windows build).
const String coreLibraryName = 'arya_vault_ffi.dll';

class FfiVaultApi implements VaultApi {
  FfiVaultApi._();

  /// Loads the core, points it at [vaultDir] and checks `API_VERSION`.
  ///
  /// The library is opened by **absolute path** (never by bare name), so the
  /// OS DLL search order cannot substitute another file. In a release build it
  /// is the file next to the executable; [libraryPath] overrides that for
  /// development (`flutter run` with a `cargo build` output).
  ///
  /// Throws [AppError] (`internal`) on a version mismatch: a fatal developer
  /// error, shown as an error screen (docs/14 §7).
  static Future<FfiVaultApi> open({
    required String vaultDir,
    String? libraryPath,
  }) async {
    final path = libraryPath ?? defaultLibraryPath();
    await RustLib.init(externalLibrary: ExternalLibrary.open(path));
    final api = FfiVaultApi._();
    await api._run(() => lc.initCore(vaultDir: vaultDir));
    final info = await api.info();
    if (info.apiVersion != expectedApiVersion) {
      throw AppError(
        AppErrorCode.internal,
        'core API version ${info.apiVersion}, app expects $expectedApiVersion',
      );
    }
    return api;
  }

  /// The per-user vault directory (docs/07): LOCALAPPDATA, then AryaVault, then vault. Restricting its
  /// ACL to the current user is native work (L01, platform-windows), not done here.
  static String defaultVaultDir() {
    final base = Platform.environment['LOCALAPPDATA'];
    if (base == null || base.isEmpty) {
      throw const AppError(AppErrorCode.io, 'no per-user data directory');
    }
    final s = Platform.pathSeparator;
    return '$base${s}AryaVault${s}vault';
  }

  /// `<directory of the executable>/arya_vault_ffi.dll`.
  static String defaultLibraryPath() {
    final exe = Platform.resolvedExecutable;
    final dir = exe.substring(0, exe.lastIndexOf(Platform.pathSeparator));
    return '$dir${Platform.pathSeparator}$coreLibraryName';
  }

  /// Runs [call], mapping every failure to [AppError] (docs/14 §2). Anything
  /// that is not an `AppError` from the core (a panic report, a bridge error)
  /// becomes `internal` with a fixed message: its text may carry paths.
  Future<T> _run<T>(Future<T> Function() call) async {
    try {
      return await call();
    } on r.AppError catch (e) {
      throw toAppError(e);
    } on AppError {
      rethrow;
    } on Object {
      throw const AppError(AppErrorCode.internal, 'internal error');
    }
  }

  // -------------------------------------------------- 4.1 lifecycle, credentials

  @override
  Future<VaultStatus> status() async => statusFromRust(await _run(lc.status));

  @override
  Future<RecoveryKeyResult> createVault({
    required Uint8List password,
    required KdfProfile profile,
  }) async => recoveryKeyFromRust(
    await _run(
      () => lc.createVault(password: password, profile: kdfToRust(profile)),
    ),
  );

  @override
  Future<bool> confirmRecoveryKey(List<GroupAnswer> answers) => _run(
    () =>
        lc.confirmRecoveryKey(answers: answers.map(groupAnswerToRust).toList()),
  );

  @override
  Future<List<int>> recoveryChallenge() async =>
      List<int>.of(await _run(lc.recoveryChallenge));

  @override
  Future<void> unlock(Uint8List password) =>
      _run(() => lc.unlock(password: password));

  @override
  Future<void> unlockQuick() => _run(lc.unlockQuick);

  @override
  Future<void> lock() => _run(lc.lock);

  @override
  Future<void> changePassword({
    required Uint8List oldPassword,
    required Uint8List newPassword,
    required bool rotateKeys,
  }) => _run(
    () => lc.changePassword(
      oldPassword: oldPassword,
      newPassword: newPassword,
      rotateKeys: rotateKeys,
    ),
  );

  @override
  Future<void> recoverWithKey({
    required Uint8List recoveryKey,
    required Uint8List newPassword,
  }) => _run(
    () => lc.recoverWithKey(recoveryKey: recoveryKey, newPassword: newPassword),
  );

  @override
  Future<RecoveryKeyResult> regenerateRecoveryKey({
    required Uint8List password,
    required bool rotateKeys,
  }) async => recoveryKeyFromRust(
    await _run(
      () =>
          lc.regenerateRecoveryKey(password: password, rotateKeys: rotateKeys),
    ),
  );

  @override
  Future<void> quickUnlockEnable() => _run(lc.quickUnlockEnable);

  @override
  Future<void> quickUnlockDisable() => _run(lc.quickUnlockDisable);

  @override
  Future<bool> verifyPassword(Uint8List password) =>
      _run(() => lc.verifyPassword(password: password));

  // ---------------------------------------------- 4.2 items and folders (unlocked)

  @override
  Future<List<ItemSummary>> list(ListFilter filter, Page page) async {
    final rows = await _run(
      () => it.list(filter: filterToRust(filter), page: pageToRust(page)),
    );
    return rows.map(summaryFromRust).toList();
  }

  @override
  Future<List<ItemSummary>> search(SearchQuery query) async {
    final rows = await _run(() => it.search(query: searchToRust(query)));
    return rows.map(summaryFromRust).toList();
  }

  @override
  Future<int> itemCount() async => (await _run(it.itemCount)).toInt();

  @override
  Future<ItemView> getItem(String id) async =>
      itemViewFromRust(await _run(() => it.getItem(id: id)));

  @override
  Future<Uint8List?> reveal(String id, StdField field) =>
      _run(() => it.reveal(id: id, field: fieldToRust(field)));

  @override
  Future<Uint8List?> revealCustom(String id, String elementId) =>
      _run(() => it.revealCustom(id: id, elementId: elementId));

  @override
  Future<String> createItem(NewItem item) =>
      _run(() => it.createItem(new_: newItemToRust(item)));

  @override
  Future<void> setField(String id, StdField field, String value) =>
      _run(() => it.setField(id: id, field: fieldToRust(field), value: value));

  @override
  Future<void> clearField(String id, StdField field) =>
      _run(() => it.clearField(id: id, field: fieldToRust(field)));

  @override
  Future<void> setFields(String id, Map<StdField, String> fields) => _run(
    () => it.setFields(
      id: id,
      fields: {for (final e in fields.entries) fieldToRust(e.key): e.value},
    ),
  );

  @override
  Future<bool> toggleFavorite(String id) =>
      _run(() => it.toggleFavorite(id: id));

  @override
  Future<void> moveToFolder(String id, String? folderId) =>
      _run(() => it.moveToFolder(id: id, folderId: folderId));

  @override
  Future<void> addTag(String id, String tag) =>
      _run(() => it.addTag(id: id, tag: tag));

  @override
  Future<void> removeTag(String id, String tag) =>
      _run(() => it.removeTag(id: id, tag: tag));

  @override
  Future<String> addUrl(String id, String url) =>
      _run(() => it.addUrl(id: id, url: url));

  @override
  Future<void> setUrl(String id, String urlId, String url) =>
      _run(() => it.setUrl(id: id, urlId: urlId, url: url));

  @override
  Future<void> removeUrl(String id, String urlId) =>
      _run(() => it.removeUrl(id: id, urlId: urlId));

  @override
  Future<String> addCustomField(
    String id,
    CustomKind kind,
    String label,
    String value,
  ) => _run(
    () => it.addCustomField(
      id: id,
      kind: customKindToRust(kind),
      label: label,
      value: value,
    ),
  );

  @override
  Future<void> setCustomValue(String id, String elementId, String value) =>
      _run(() => it.setCustomValue(id: id, elementId: elementId, value: value));

  @override
  Future<void> setCustomLabel(String id, String elementId, String label) =>
      _run(() => it.setCustomLabel(id: id, elementId: elementId, label: label));

  @override
  Future<void> removeCustomField(String id, String elementId) =>
      _run(() => it.removeCustomField(id: id, elementId: elementId));

  @override
  Future<void> deleteItem(String id) => _run(() => it.deleteItem(id: id));

  @override
  Future<void> restoreItem(String id) => _run(() => it.restoreItem(id: id));

  @override
  Future<void> purgeItem(String id) => _run(() => it.purgeItem(id: id));

  @override
  Future<List<TrashEntry>> listTrash() async =>
      (await _run(it.listTrash)).map(trashFromRust).toList();

  @override
  Future<int> emptyTrash() => _run(it.emptyTrash);

  @override
  Future<int> purgeExpired() => _run(it.purgeExpired);

  @override
  Future<List<VersionInfo>> history(String id, StdField field) async {
    final rows = await _run(
      () => it.history(id: id, field: fieldToRust(field)),
    );
    return rows.map(versionFromRust).toList();
  }

  @override
  Future<Uint8List?> revealVersion(String id, StdField field, int hlcMs) =>
      _run(
        () => it.revealVersion(id: id, field: fieldToRust(field), hlcMs: hlcMs),
      );

  @override
  Future<void> restoreVersion(String id, StdField field, int hlcMs) => _run(
    () => it.restoreVersion(id: id, field: fieldToRust(field), hlcMs: hlcMs),
  );

  @override
  Future<String> createFolder(String name, String? parentId) =>
      _run(() => it.createFolder(name: name, parentId: parentId));

  @override
  Future<void> renameFolder(String id, String name) =>
      _run(() => it.renameFolder(id: id, name: name));

  @override
  Future<void> deleteFolder(String id) => _run(() => it.deleteFolder(id: id));

  @override
  Future<List<Folder>> listFolders() async =>
      (await _run(it.listFolders)).map(folderFromRust).toList();

  // -------------------------------------------- 4.3 generator, strength, health

  @override
  Future<Uint8List> generatePassword(PasswordOptions options) =>
      _run(() => tl.generatePassword(options: passwordOptionsToRust(options)));

  @override
  Future<Uint8List> generatePassphrase(PassphraseOptions options) => _run(
    () => tl.generatePassphrase(options: passphraseOptionsToRust(options)),
  );

  @override
  Future<double> entropyBits(EntropyOptions options) =>
      _run(() => tl.entropyBits(options: entropyToRust(options)));

  @override
  Future<Strength> strength(Uint8List password) async =>
      strengthFromRust(await _run(() => tl.strength(password: password)));

  @override
  Future<PolicyResult> checkMasterPassword(Uint8List password) async =>
      policyFromRust(
        await _run(() => tl.checkMasterPassword(password: password)),
      );

  @override
  Future<HealthReport> healthReport() async =>
      healthFromRust(await _run(tl.healthReport));

  // ------------------------------------------------------- 4.4 import / export

  @override
  Future<ImportPreview> importPreview({
    required ImportFormat format,
    required Uint8List file,
  }) async => previewFromRust(
    await _run(
      () => tr.importPreview(format: importFormatToRust(format), file: file),
    ),
  );

  @override
  Future<ImportResult> importCommit({
    required String previewToken,
    required ImportOptions options,
  }) async => importResultFromRust(
    await _run(
      () => tr.importCommit(
        previewToken: previewToken,
        options: importOptionsToRust(options),
      ),
    ),
  );

  @override
  Future<Uint8List> exportCsv(AcknowledgePlaintextRisk token) => _run(
    () => tr.exportCsv(
      token: r.AcknowledgePlaintextRisk(acknowledged: token.acknowledged),
    ),
  );

  @override
  Future<Uint8List> exportEncrypted(Uint8List exportPassword) =>
      _run(() => tr.exportEncrypted(exportPassword: exportPassword));

  @override
  Future<ImportResult> importEncrypted({
    required Uint8List file,
    required Uint8List exportPassword,
  }) async => importResultFromRust(
    await _run(
      () => tr.importEncrypted(file: file, exportPassword: exportPassword),
    ),
  );

  // ------------------------------------------------------------- 4.5 settings

  @override
  Future<AppSettings> getSettings() async =>
      settingsFromRust(await _run(st.getSettings));

  @override
  Future<void> setSettings(AppSettings settings) =>
      _run(() => st.setSettings(settings: settingsToRust(settings)));

  // --------------------------------------------------------- 4.6 diagnostics

  @override
  Future<InfoDto> info() async => infoFromRust(await _run(dg.info));

  @override
  Future<Uint8List> exportDiagnostics() => _run(dg.exportDiagnostics);
}
