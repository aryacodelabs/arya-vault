import 'dart:async';
import 'dart:typed_data';

import 'package:arya_vault/src/api/app_error.dart';
import 'package:arya_vault/src/api/dtos.dart';
import 'package:arya_vault/src/state/routing.dart';
import 'package:flutter/material.dart' hide Page;
import 'package:flutter_riverpod/misc.dart' show Override;
import 'package:flutter_test/flutter_test.dart';

import '../helpers.dart';
import '../screen_cases.dart' show seeded;
import '../vault/vault_helpers.dart';

/// Pumps the unlocked app and pushes [route] on top of the vault, like tapping
/// its entry in the menu does.
Future<Harness> pumpRoute(
  WidgetTester tester,
  String route, {
  SpyApi? api,
  Services? services,
  Size size = const Size(700, 900),
  double textScale = 1,
  ThemeMode themeMode = ThemeMode.light,
  List<Override> overrides = const <Override>[],
}) async {
  final Harness h = await pumpVault(
    tester,
    api: api ?? await unlockedVault(items: 20, trash: 0),
    services: services,
    size: size,
    textScale: textScale,
    themeMode: themeMode,
    overrides: overrides,
  );
  unawaited(h.container.read(routerProvider).push(route));
  await tester.pumpAndSettle();
  return h;
}

/// True if every byte is zero (a wiped buffer).
bool isWiped(Uint8List b) => b.every((int v) => v == 0);

/// A fake that keeps the buffers it hands out or receives, so tests can check
/// that the UI wiped them.
class BufferSpyApi extends SpyApi {
  BufferSpyApi({
    super.sampleItemCount,
    super.sampleTrashCount,
    super.seedSampleItems,
    super.quickUnlock,
    super.random,
    super.clock,
  });

  final List<Uint8List> generated = <Uint8List>[];
  final List<PasswordOptions> passwordOptions = <PasswordOptions>[];
  final List<PassphraseOptions> passphraseOptions = <PassphraseOptions>[];
  final List<Uint8List> received = <Uint8List>[];
  final List<Uint8List> returnedExports = <Uint8List>[];
  int csvExports = 0;
  int encryptedExports = 0;

  @override
  Future<Uint8List> generatePassword(PasswordOptions options) async {
    passwordOptions.add(options);
    final Uint8List b = await super.generatePassword(options);
    generated.add(b);
    return b;
  }

  @override
  Future<Uint8List> generatePassphrase(PassphraseOptions options) async {
    passphraseOptions.add(options);
    final Uint8List b = await super.generatePassphrase(options);
    generated.add(b);
    return b;
  }

  @override
  Future<bool> verifyPassword(Uint8List password) {
    received.add(password);
    return super.verifyPassword(password);
  }

  @override
  Future<void> changePassword({
    required Uint8List oldPassword,
    required Uint8List newPassword,
    required bool rotateKeys,
  }) {
    received
      ..add(oldPassword)
      ..add(newPassword);
    return super.changePassword(
      oldPassword: oldPassword,
      newPassword: newPassword,
      rotateKeys: rotateKeys,
    );
  }

  @override
  Future<RecoveryKeyResult> regenerateRecoveryKey({
    required Uint8List password,
    required bool rotateKeys,
  }) {
    received.add(password);
    return super.regenerateRecoveryKey(
      password: password,
      rotateKeys: rotateKeys,
    );
  }

  @override
  Future<Uint8List> exportCsv(AcknowledgePlaintextRisk token) async {
    csvExports++;
    final Uint8List b = await super.exportCsv(token);
    returnedExports.add(b);
    return b;
  }

  @override
  Future<Uint8List> exportEncrypted(Uint8List exportPassword) async {
    encryptedExports++;
    received.add(exportPassword);
    final Uint8List b = await super.exportEncrypted(exportPassword);
    returnedExports.add(b);
    return b;
  }

  @override
  Future<ImportResult> importEncrypted({
    required Uint8List file,
    required Uint8List exportPassword,
  }) {
    received.add(exportPassword);
    return super.importEncrypted(file: file, exportPassword: exportPassword);
  }

  /// Test hook: make `info` fail while set.
  AppError? failInfo;

  @override
  Future<InfoDto> info() {
    final AppError? e = failInfo;
    if (e != null) throw e;
    return super.info();
  }

  /// Test hook: make `getSettings` fail while set.
  AppError? failGetSettings;
  int setSettingsCalls = 0;

  @override
  Future<AppSettings> getSettings() {
    final AppError? e = failGetSettings;
    if (e != null) throw e;
    return super.getSettings();
  }

  /// Test hook: make the next `setSettings` fail with this error.
  AppError? failSettings;

  @override
  Future<void> setSettings(AppSettings settings) {
    setSettingsCalls++;
    final AppError? e = failSettings;
    if (e != null) throw e;
    return super.setSettings(settings);
  }

  final List<Uint8List> importFiles = <Uint8List>[];

  @override
  Future<HealthReport> healthReport() {
    calls.add('healthReport');
    final AppError? e = failNext.remove('healthReport');
    if (e != null) throw e;
    return super.healthReport();
  }

  @override
  Future<ImportPreview> importPreview({
    required ImportFormat format,
    required Uint8List file,
  }) {
    importFiles.add(file);
    return super.importPreview(format: format, file: file);
  }
}

/// An unlocked, onboarded vault on [BufferSpyApi].
Future<BufferSpyApi> unlockedBufferVault({
  bool samples = false,
  int items = 20,
  DateTime Function()? clock,
  QuickUnlockStatus quickUnlock = kHello,
}) async {
  final BufferSpyApi api = BufferSpyApi(
    sampleItemCount: items,
    sampleTrashCount: 0,
    seedSampleItems: samples,
    random: seeded(),
    clock: clock ?? () => DateTime.utc(2026, 6, 1, 12),
    quickUnlock: quickUnlock,
  );
  final RecoveryKeyResult r = await createThroughApi(api);
  final List<String> g = keyGroups(r);
  await api.confirmRecoveryKey(<GroupAnswer>[
    for (final int i in r.challenge) GroupAnswer(index: i, text: g[i]),
  ]);
  api.calls.clear();
  return api;
}
