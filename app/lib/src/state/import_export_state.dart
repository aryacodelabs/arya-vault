import 'package:flutter/foundation.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../api/app_error.dart';
import '../api/dtos.dart';
import '../api/secret_bytes.dart';
import '../api/vault_api.dart';
import '../services/services.dart';
import 'providers.dart';

/// Which kind of file the user is importing.
enum ImportKind { csv, bitwarden, encrypted }

enum ImportPhase { idle, reading, preview, committing, done }

/// The import flow (docs/14 §4.4): pick a file, **preview before commit**,
/// commit, summary. Files and passwords are wiped as soon as they were sent to
/// the core. The core does all parsing; Dart only moves bytes.
class ImportController extends ChangeNotifier {
  ImportController(this._api, this._picker);

  final VaultApi _api;
  final FilePickerService _picker;

  ImportKind kind = ImportKind.csv;
  ImportPhase phase = ImportPhase.idle;
  ImportPreview? preview;
  ImportResult? result;
  AppError? error;

  /// The file chooser itself is missing on this build.
  bool pickerUnavailable = false;

  /// The user opened the chooser and cancelled.
  bool pickCancelled = false;

  bool skipDuplicates = true;
  String? targetFolder;

  Uint8List? _encryptedFile;
  bool _disposed = false;

  bool get hasEncryptedFile => _encryptedFile != null;

  static List<String> extensionsFor(ImportKind kind) => switch (kind) {
    ImportKind.csv => const <String>['csv'],
    ImportKind.bitwarden => const <String>['json'],
    ImportKind.encrypted => const <String>['avex'],
  };

  void setKind(ImportKind next) {
    if (phase != ImportPhase.idle) return;
    kind = next;
    error = null;
    pickCancelled = false;
    notifyListeners();
  }

  void setSkipDuplicates(bool v) {
    skipDuplicates = v;
    notifyListeners();
  }

  void setTargetFolder(String? id) {
    targetFolder = id;
    notifyListeners();
  }

  /// Picks a file. CSV and Bitwarden JSON go straight to `importPreview`; an
  /// encrypted export is kept until [importEncrypted] gets its password.
  /// Returns true if a file was chosen.
  Future<bool> pickFile() async {
    error = null;
    pickCancelled = false;
    pickerUnavailable = false;
    Uint8List? bytes;
    try {
      bytes = await _picker.pickFile(extensions: extensionsFor(kind));
    } on ServiceUnavailable {
      pickerUnavailable = true;
      notifyListeners();
      return false;
    }
    if (bytes == null) {
      pickCancelled = true;
      notifyListeners();
      return false;
    }
    if (kind == ImportKind.encrypted) {
      _wipeEncrypted();
      _encryptedFile = bytes;
      notifyListeners();
      return true;
    }
    phase = ImportPhase.reading;
    notifyListeners();
    try {
      final ImportPreview p = await _api.importPreview(
        format: kind == ImportKind.csv
            ? ImportFormat.csv
            : ImportFormat.bitwardenJson,
        file: bytes,
      );
      if (_disposed) return true;
      preview = p;
      phase = ImportPhase.preview;
    } on AppError catch (e) {
      error = e;
      phase = ImportPhase.idle;
    } finally {
      bytes.fillRange(0, bytes.length, 0);
    }
    if (!_disposed) notifyListeners();
    return true;
  }

  /// Commits the previewed import with the chosen options.
  Future<void> commit() async {
    final ImportPreview? p = preview;
    if (p == null || phase != ImportPhase.preview) return;
    phase = ImportPhase.committing;
    error = null;
    notifyListeners();
    try {
      final ImportResult r = await _api.importCommit(
        previewToken: p.previewToken,
        options: ImportOptions(
          skipDuplicates: skipDuplicates,
          targetFolder: targetFolder,
        ),
      );
      if (_disposed) return;
      result = r;
      phase = ImportPhase.done;
    } on AppError catch (e) {
      error = e;
      phase = ImportPhase.preview;
    }
    if (!_disposed) notifyListeners();
  }

  /// Imports the chosen encrypted export with [password] (wiped). Returns the
  /// error to show, or null on success.
  Future<AppError?> importEncrypted(SecretBytes password) async {
    final Uint8List? file = _encryptedFile;
    if (file == null) return null;
    try {
      final ImportResult r = await password.useAsync(
        (Uint8List pw) => _api.importEncrypted(file: file, exportPassword: pw),
      );
      if (_disposed) return null;
      result = r;
      phase = ImportPhase.done;
      _wipeEncrypted();
      notifyListeners();
      return null;
    } on AppError catch (e) {
      return e;
    }
  }

  void discardEncryptedFile() {
    _wipeEncrypted();
    notifyListeners();
  }

  /// Back to the start (another file).
  void reset() {
    _wipeEncrypted();
    preview = null;
    result = null;
    error = null;
    pickCancelled = false;
    phase = ImportPhase.idle;
    notifyListeners();
  }

  void _wipeEncrypted() {
    final Uint8List? f = _encryptedFile;
    if (f != null) f.fillRange(0, f.length, 0);
    _encryptedFile = null;
  }

  @override
  void dispose() {
    _disposed = true;
    _wipeEncrypted();
    super.dispose();
  }
}

enum ExportOutcome { saved, cancelled, failed, unavailable }

/// The export flows (docs/14 §4.4). The exported bytes are wiped after saving.
class ExportActions {
  ExportActions(this._ref);
  final Ref _ref;

  /// Plain CSV. The caller has shown the plaintext-risk dialog and the user
  /// confirmed (type-to-confirm); that is what `acknowledged: true` records.
  Future<ExportOutcome> exportCsv({required String fileName}) async {
    final Uint8List bytes = await _ref
        .read(vaultApiProvider)
        .exportCsv(const AcknowledgePlaintextRisk(acknowledged: true));
    return _save(fileName, bytes);
  }

  /// Password-protected export. Wipes [password].
  Future<ExportOutcome> exportEncrypted(
    SecretBytes password, {
    required String fileName,
  }) async {
    final Uint8List bytes = await password.useAsync(
      _ref.read(vaultApiProvider).exportEncrypted,
    );
    return _save(fileName, bytes);
  }

  Future<ExportOutcome> _save(String fileName, Uint8List bytes) async {
    try {
      final bool saved = await _ref
          .read(filePickerServiceProvider)
          .saveFile(suggestedName: fileName, bytes: bytes);
      return saved ? ExportOutcome.saved : ExportOutcome.cancelled;
    } on ServiceUnavailable {
      return ExportOutcome.unavailable;
    } on Object {
      return ExportOutcome.failed;
    } finally {
      bytes.fillRange(0, bytes.length, 0);
    }
  }
}

final Provider<ExportActions> exportActionsProvider = Provider<ExportActions>(
  ExportActions.new,
);
