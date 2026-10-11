import 'dart:async';

import 'package:arya_vault/src/api/app_error.dart';
import 'package:arya_vault/src/api/dtos.dart';
import 'package:arya_vault/src/api/fake_vault_api.dart';
import 'package:arya_vault/src/state/vault_status.dart';
import 'package:arya_vault/src/ui/widgets/secret_text_field.dart';
import 'package:flutter/foundation.dart';
import 'package:flutter/material.dart';
import 'package:flutter/rendering.dart';
import 'package:flutter_test/flutter_test.dart';

import 'helpers.dart';

/// Keeps a reference to every secret buffer handed to the API so a test can
/// check that the UI wiped it afterwards.
class RecordingApi extends FakeVaultApi {
  RecordingApi() : super(seedSampleItems: false);
  final List<Uint8List> passwordBuffers = <Uint8List>[];
  final List<Uint8List> recoveryKeyBuffers = <Uint8List>[];

  @override
  Future<RecoveryKeyResult> createVault({
    required Uint8List password,
    required KdfProfile profile,
  }) {
    passwordBuffers.add(password);
    return super.createVault(password: password, profile: profile);
  }

  @override
  Future<void> unlock(Uint8List password) {
    passwordBuffers.add(password);
    return super.unlock(password);
  }

  @override
  Future<void> recoverWithKey({
    required Uint8List recoveryKey,
    required Uint8List newPassword,
  }) {
    recoveryKeyBuffers.add(recoveryKey);
    passwordBuffers.add(newPassword);
    return super.recoverWithKey(
      recoveryKey: recoveryKey,
      newPassword: newPassword,
    );
  }

  @override
  Future<RecoveryKeyResult> regenerateRecoveryKey({
    required Uint8List password,
    required bool rotateKeys,
  }) {
    passwordBuffers.add(password);
    return super.regenerateRecoveryKey(
      password: password,
      rotateKeys: rotateKeys,
    );
  }
}

/// Collected log output of a test body.
class Logs {
  final List<String> lines = <String>[];
  String get all => lines.join('\n');
}

Future<Logs> captureLogs(Future<void> Function() body) async {
  final Logs logs = Logs();
  final DebugPrintCallback original = debugPrint;
  final FlutterExceptionHandler? originalOnError = FlutterError.onError;
  debugPrint = (String? message, {int? wrapWidth}) {
    if (message != null) logs.lines.add(message);
  };
  FlutterError.onError = (FlutterErrorDetails d) {
    logs.lines.add(d.toString());
  };
  try {
    await runZoned(
      body,
      zoneSpecification: ZoneSpecification(
        print: (Zone self, ZoneDelegate parent, Zone zone, String line) =>
            logs.lines.add(line),
      ),
    );
  } finally {
    debugPrint = original;
    FlutterError.onError = originalOnError;
  }
  return logs;
}

String treeDumps(WidgetTester tester) => <String>[
  tester.binding.rootElement!.toStringDeep(),
  tester.binding.renderViews.first.toStringDeep(),
  tester.binding.renderViews.first.debugSemantics?.toStringDeep() ?? '',
].join('\n');

void main() {
  testWidgets('wrong and right passwords leave no trace in logs or trees', (
    WidgetTester tester,
  ) async {
    final RecordingApi api = RecordingApi();
    await createConfirmedLockedVault(api);
    final SemanticsHandle handle = tester.ensureSemantics();
    late String dumpWhileTyping;
    final Logs logs = await captureLogs(() async {
      await pumpApp(tester, api: api);
      await typeInto(
        tester,
        l10n.masterPasswordLabel,
        'CANARY-wrong-password-777',
      );
      dumpWhileTyping = treeDumps(tester);
      await tapOn(tester, buttonText(l10n.lockSubmit));
      dumpWhileTyping += treeDumps(tester);
      await typeInto(tester, l10n.masterPasswordLabel, kPassword);
      dumpWhileTyping += treeDumps(tester);
      await tapOn(tester, buttonText(l10n.lockSubmit));
    });
    handle.dispose();

    for (final String secret in <String>[
      'CANARY-wrong-password-777',
      kPassword,
    ]) {
      expect(logs.all, isNot(contains(secret)));
      expect(dumpWhileTyping, isNot(contains(secret)));
    }
    expect(dumpWhileTyping, isNotEmpty);
  });

  testWidgets('the UI wipes every password buffer it hands to the API', (
    WidgetTester tester,
  ) async {
    final RecordingApi api = RecordingApi();
    final Harness h = await pumpApp(tester, api: api);
    await createVaultViaUi(tester);
    expect(api.passwordBuffers, hasLength(1));
    expect(api.passwordBuffers.single, everyElement(0), reason: 'createVault');

    await api.lock();
    await h.container.read(vaultStatusProvider.notifier).refresh();
    await tester.pumpAndSettle();
    await typeInto(tester, l10n.masterPasswordLabel, kPassword);
    await tapOn(tester, buttonText(l10n.lockSubmit));
    expect(api.passwordBuffers, hasLength(2));
    expect(api.passwordBuffers.last, everyElement(0), reason: 'unlock');
  });

  testWidgets('recovery wipes both the key and the new password', (
    WidgetTester tester,
  ) async {
    final RecordingApi api = RecordingApi();
    final RecoveryKeyResult r = await createThroughApi(api);
    final List<String> g = keyGroups(r);
    await api.confirmRecoveryKey(<GroupAnswer>[
      for (final int i in r.challenge) GroupAnswer(index: i, text: g[i]),
    ]);
    await api.lock();
    await pumpApp(tester, api: api);
    await tapOn(tester, find.text(l10n.lockUseRecovery));
    await typeInto(
      tester,
      l10n.recoverKeyLabel,
      String.fromCharCodes(r.recoveryKey),
    );
    await tapOn(tester, buttonText(l10n.actionContinue));
    await typeInto(tester, l10n.masterPasswordLabel, kNewPassword);
    await typeInto(tester, l10n.confirmPasswordLabel, kNewPassword);
    await tapOn(tester, buttonText(l10n.recoverSubmit));
    expect(textOf(l10n.homeUnlockedMessage), findsOneWidget);
    expect(api.recoveryKeyBuffers.single, everyElement(0));
    expect(api.passwordBuffers.last, everyElement(0));
  });

  testWidgets('a hidden recovery key is absent from every tree dump and log', (
    WidgetTester tester,
  ) async {
    final SemanticsHandle handle = tester.ensureSemantics();
    late Harness h;
    late String dump;
    final Logs logs = await captureLogs(() async {
      h = await pumpApp(tester);
      await createVaultViaUi(tester);
      await tapOn(tester, find.text(l10n.recoveryHideKey));
      dump = treeDumps(tester);
    });
    handle.dispose();
    final List<String> groups = pendingGroups(h);
    final String full = groups.join('-');
    expect(logs.all, isNot(contains(full)));
    for (final String g in groups.take(7)) {
      // Groups are short; require the 5-char groups not to appear as text.
      if (g.length == 5) {
        expect(dump, isNot(contains('"$g"')));
        expect(dump, isNot(contains("'$g'")));
      }
    }
    expect(dump, isNot(contains(full)));
  });

  testWidgets('even the shown key never reaches logs', (
    WidgetTester tester,
  ) async {
    late Harness h;
    final Logs logs = await captureLogs(() async {
      h = await pumpApp(tester);
      await createVaultViaUi(tester);
      await storeKeyAndContinue(tester);
      await typeAnswers(tester, <String>['AAAAA', 'BBBBB', 'CCCCC']);
      await tapOn(tester, buttonText(l10n.verifySubmit));
    });
    for (final String g in pendingGroups(h)) {
      expect(logs.all, isNot(contains(g)));
    }
    expect(logs.all, isNot(contains('AAAAA')));
  });

  group('error and string representations', () {
    test('API errors never echo what was sent', () async {
      final FakeVaultApi api = newFakeApi();
      final List<AppError> errors = <AppError>[];
      Future<void> collect(Future<void> Function() call) async {
        try {
          await call();
        } on AppError catch (e) {
          errors.add(e);
        }
      }

      await collect(
        () => api.createVault(
          password: bytesOf('CANARY-sh'),
          profile: KdfProfile.default_,
        ),
      );
      final RecoveryKeyResult r = await createThroughApi(api);
      await api.lock();
      await collect(() => api.unlock(bytesOf('CANARY-wrong-pw-aaaa')));
      await collect(
        () => api.recoverWithKey(
          recoveryKey: bytesOf('CANARY-not-a-key'),
          newPassword: bytesOf('CANARY-new-password-xx1'),
        ),
      );
      await api.unlock(bytesOf(kPassword));
      await collect(
        () => api.confirmRecoveryKey(<GroupAnswer>[
          const GroupAnswer(index: 9, text: 'CANARY-answer'),
        ]),
      );
      expect(errors, isNotEmpty);
      final String key = String.fromCharCodes(r.recoveryKey);
      for (final AppError e in errors) {
        final String text = '${e.message} ${e.field} $e';
        expect(text, isNot(contains('CANARY')));
        expect(text, isNot(contains(kPassword)));
        expect(text, isNot(contains(key.substring(0, 5))));
      }
    });

    test('SecretTextController and TextField diagnostics are redacted', () {
      final SecretTextController c = SecretTextController(text: kPassword);
      expect(c.toString(), isNot(contains('CANARY')));
      final TextField field = TextField(controller: c, obscureText: true);
      expect(field.toStringDeep(), isNot(contains('CANARY')));
      c.dispose();
    });
  });
}
