import 'dart:typed_data';

import 'package:arya_vault/src/api/app_error.dart';
import 'package:arya_vault/src/api/dtos.dart';
import 'package:arya_vault/src/api/fake_vault_api.dart';
import 'package:arya_vault/src/ui/shell/adaptive_shell.dart';
import 'package:arya_vault/src/ui/widgets/error_text.dart';
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

import '../helpers.dart';

/// A fake whose selected calls throw a chosen error.
class FailingApi extends FakeVaultApi {
  FailingApi() : super(seedSampleItems: false);
  AppError? onUnlock;
  AppError? onStatus;
  AppError? onQuick;

  @override
  Future<void> unlock(Uint8List password) async {
    final AppError? e = onUnlock;
    if (e != null) throw e;
    return super.unlock(password);
  }

  @override
  Future<void> unlockQuick() async {
    final AppError? e = onQuick;
    if (e != null) throw e;
    return super.unlockQuick();
  }

  @override
  Future<VaultStatus> status() async {
    final AppError? e = onStatus;
    if (e != null) throw e;
    return super.status();
  }
}

Future<FakeVaultApi> lockedVault({
  QuickUnlockStatus? quick,
  bool enableQuick = false,
}) async {
  final FakeVaultApi api = newFakeApi(
    quickUnlock: quick ?? QuickUnlockStatus.unsupported,
  );
  await createConfirmedLockedVault(api);
  if (enableQuick) {
    await api.unlock(bytesOf(kPassword));
    await api.quickUnlockEnable();
    await api.lock();
  }
  return api;
}

void main() {
  group('Lock screen', () {
    testWidgets('unlocks with the right password', (WidgetTester tester) async {
      final FakeVaultApi api = await lockedVault();
      await pumpApp(tester, api: api);
      expect(textOf(l10n.lockTitle), findsOneWidget);
      await typeInto(tester, l10n.masterPasswordLabel, kPassword);
      await tapOn(tester, buttonText(l10n.lockSubmit));
      expect(textOf(l10n.homeUnlockedMessage), findsOneWidget);
    });

    testWidgets('the Enter key submits', (WidgetTester tester) async {
      final FakeVaultApi api = await lockedVault();
      await pumpApp(tester, api: api);
      await tester.enterText(
        fieldLabelled(l10n.masterPasswordLabel),
        kPassword,
      );
      await tester.testTextInput.receiveAction(TextInputAction.done);
      await tester.pumpAndSettle();
      expect(textOf(l10n.homeUnlockedMessage), findsOneWidget);
    });

    testWidgets('wrong password shows an error and keeps the screen', (
      WidgetTester tester,
    ) async {
      final FakeVaultApi api = await lockedVault();
      await pumpApp(tester, api: api);
      await typeInto(
        tester,
        l10n.masterPasswordLabel,
        'CANARY-wrong-password-0',
      );
      await tapOn(tester, buttonText(l10n.lockSubmit));
      expect(textOf(l10n.errWrongCredentials), findsOneWidget);
      expect(textOf(l10n.lockTitle), findsOneWidget);
      expect((await api.status()).locked, isTrue);
    });

    testWidgets('backoff: after five misses the screen says to wait', (
      WidgetTester tester,
    ) async {
      DateTime now = DateTime.utc(2026, 1, 1);
      final FakeVaultApi api = newFakeApi(clock: () => now);
      await createConfirmedLockedVault(api);
      await pumpApp(tester, api: api);
      for (var i = 0; i < 5; i++) {
        await typeInto(
          tester,
          l10n.masterPasswordLabel,
          'CANARY-wrong-$i-xxxx',
        );
        await tapOn(tester, buttonText(l10n.lockSubmit));
        expect(textOf(l10n.errWrongCredentials), findsOneWidget);
      }
      await typeInto(tester, l10n.masterPasswordLabel, kPassword);
      await tapOn(tester, buttonText(l10n.lockSubmit));
      expect(textOf(l10n.errBusy), findsOneWidget);
      expect(textOf(l10n.lockTitle), findsOneWidget);
      now = now.add(const Duration(seconds: 5));
      await tapOn(tester, buttonText(l10n.lockSubmit));
      expect(textOf(l10n.homeUnlockedMessage), findsOneWidget);
    });

    testWidgets('no quick unlock button when it is not enabled', (
      WidgetTester tester,
    ) async {
      final FakeVaultApi api = await lockedVault();
      await pumpApp(tester, api: api);
      expect(find.byIcon(Icons.fingerprint), findsNothing);
      expect(find.textContaining('Unlock with'), findsNothing);
    });

    testWidgets('quick unlock button shows when enabled and works', (
      WidgetTester tester,
    ) async {
      final FakeVaultApi api = await lockedVault(
        quick: kHello,
        enableQuick: true,
      );
      await pumpApp(tester, api: api);
      final Finder quick = find.text(
        l10n.lockQuick(l10n.quickKindWindowsHello),
      );
      expect(quick, findsOneWidget);
      await tapOn(tester, quick);
      expect(textOf(l10n.homeUnlockedMessage), findsOneWidget);
    });

    testWidgets('quickUnlockUnavailable falls back to the password', (
      WidgetTester tester,
    ) async {
      final FakeVaultApi api = await lockedVault(
        quick: kHello,
        enableQuick: true,
      );
      api.quickUnlockUsable = false;
      await pumpApp(tester, api: api);
      await tapOn(
        tester,
        find.text(l10n.lockQuick(l10n.quickKindWindowsHello)),
      );
      expect(textOf(l10n.errQuickUnlockUnavailable), findsOneWidget);
      expect(textOf(l10n.lockTitle), findsOneWidget);
      await typeInto(tester, l10n.masterPasswordLabel, kPassword);
      await tapOn(tester, buttonText(l10n.lockSubmit));
      expect(textOf(l10n.homeUnlockedMessage), findsOneWidget);
    });

    testWidgets('"Use recovery key" opens the recover flow', (
      WidgetTester tester,
    ) async {
      final FakeVaultApi api = await lockedVault();
      await pumpApp(tester, api: api);
      await tapOn(tester, find.text(l10n.lockUseRecovery));
      expect(textOf(l10n.recoverTitle), findsOneWidget);
    });

    for (final (AppErrorCode, String Function()) c
        in <(AppErrorCode, String Function())>[
          (AppErrorCode.wrongCredentials, () => l10n.errWrongCredentials),
          (AppErrorCode.busy, () => l10n.errBusy),
          (AppErrorCode.corruptVault, () => l10n.errCorruptVault),
          (AppErrorCode.unsupportedFormat, () => l10n.errUnsupportedFormat),
          (AppErrorCode.io, () => l10n.errIo),
          (AppErrorCode.internal, () => l10n.errInternal),
          (AppErrorCode.locked, () => l10n.errLocked),
          (AppErrorCode.limitReached, () => l10n.errLimitReached),
        ]) {
      testWidgets('renders ${c.$1.name} from unlock', (
        WidgetTester tester,
      ) async {
        final FailingApi api = FailingApi();
        await createConfirmedLockedVault(api);
        api.onUnlock = AppError(c.$1, 'CANARY-internal-detail');
        await pumpApp(tester, api: api);
        await typeInto(tester, l10n.masterPasswordLabel, kPassword);
        await tapOn(tester, buttonText(l10n.lockSubmit));
        expect(textOf(c.$2()), findsOneWidget);
        expect(
          find.textContaining('CANARY-internal-detail'),
          findsNothing,
          reason: 'raw AppError.message is never shown',
        );
      });
    }
  });

  group('error text mapping', () {
    test('every AppErrorCode has distinct, non-empty text', () {
      final Set<String> seen = <String>{};
      for (final AppErrorCode code in AppErrorCode.values) {
        final String t = errorMessage(l10n, AppError(code, 'm'));
        expect(t, isNotEmpty, reason: code.name);
        seen.add(t);
      }
      expect(seen, hasLength(AppErrorCode.values.length));
    });

    test('wrongCredentials reads differently for a recovery key', () {
      const AppError e = AppError(AppErrorCode.wrongCredentials, 'm');
      expect(
        errorMessage(l10n, e, context: ErrorContext.recoveryKey),
        isNot(errorMessage(l10n, e)),
      );
    });
  });

  group('status error screen', () {
    testWidgets('shows the error and retries', (WidgetTester tester) async {
      final FailingApi api = FailingApi()
        ..onStatus = const AppError(AppErrorCode.unsupportedFormat, 'newer');
      await pumpApp(tester, api: api);
      expect(textOf(l10n.loadFailed), findsOneWidget);
      expect(textOf(l10n.errUnsupportedFormat), findsOneWidget);
      api.onStatus = null;
      await tapOn(tester, buttonText(l10n.actionTryAgain));
      expect(textOf(l10n.welcomeTitle), findsOneWidget);
    });
  });

  group('Recover with recovery key', () {
    late FakeVaultApi api;
    late RecoveryKeyResult key;

    Future<void> setUpVault(WidgetTester tester) async {
      api = newFakeApi();
      key = await createThroughApi(api);
      final List<String> g = keyGroups(key);
      await api.confirmRecoveryKey(<GroupAnswer>[
        for (final int i in key.challenge) GroupAnswer(index: i, text: g[i]),
      ]);
      await api.lock();
      await pumpApp(tester, api: api);
      await tapOn(tester, find.text(l10n.lockUseRecovery));
    }

    testWidgets('key -> new password -> unlocked', (WidgetTester tester) async {
      await setUpVault(tester);
      // Tolerant formatting: lowercase, spaces instead of hyphens.
      await typeInto(
        tester,
        l10n.recoverKeyLabel,
        String.fromCharCodes(key.recoveryKey)
            .toLowerCase()
            .replaceAll('-', ' '),
      );
      await tapOn(tester, buttonText(l10n.actionContinue));
      expect(textOf(l10n.recoverNewPasswordTitle), findsOneWidget);
      expect(
        tester.widget<FilledButton>(buttonText(l10n.recoverSubmit)).onPressed,
        isNull,
      );

      await typeInto(tester, l10n.masterPasswordLabel, 'short');
      expect(find.textContaining('at least 12'), findsOneWidget);
      await typeInto(tester, l10n.masterPasswordLabel, kNewPassword);
      await typeInto(tester, l10n.confirmPasswordLabel, kNewPassword);
      await tapOn(tester, buttonText(l10n.recoverSubmit));
      expect(textOf(l10n.homeUnlockedMessage), findsOneWidget);

      await api.lock();
      await api.unlock(bytesOf(kNewPassword));
    });

    testWidgets('a malformed key returns to the key step with a message', (
      WidgetTester tester,
    ) async {
      await setUpVault(tester);
      await typeInto(tester, l10n.recoverKeyLabel, 'NOT-A-KEY');
      await tapOn(tester, buttonText(l10n.actionContinue));
      await typeInto(tester, l10n.masterPasswordLabel, kNewPassword);
      await typeInto(tester, l10n.confirmPasswordLabel, kNewPassword);
      await tapOn(tester, buttonText(l10n.recoverSubmit));
      expect(textOf(l10n.errRecoveryKeyMalformed), findsOneWidget);
      expect(textOf(l10n.recoverTitle), findsOneWidget);
      expect((await api.status()).locked, isTrue);
    });

    testWidgets('a well-formed key of another vault is rejected', (
      WidgetTester tester,
    ) async {
      await setUpVault(tester);
      final RecoveryKeyResult other = await createThroughApi(newFakeApi());
      await typeInto(
        tester,
        l10n.recoverKeyLabel,
        String.fromCharCodes(other.recoveryKey),
      );
      await tapOn(tester, buttonText(l10n.actionContinue));
      await typeInto(tester, l10n.masterPasswordLabel, kNewPassword);
      await typeInto(tester, l10n.confirmPasswordLabel, kNewPassword);
      await tapOn(tester, buttonText(l10n.recoverSubmit));
      expect(textOf(l10n.errWrongRecoveryKey), findsOneWidget);
      expect((await api.status()).locked, isTrue);
    });

    testWidgets('Continue needs a key; Back returns to the lock screen', (
      WidgetTester tester,
    ) async {
      await setUpVault(tester);
      expect(
        tester.widget<FilledButton>(buttonText(l10n.actionContinue)).onPressed,
        isNull,
      );
      await tapOn(tester, find.text(l10n.actionBack));
      expect(textOf(l10n.lockTitle), findsOneWidget);
    });

    testWidgets('"I don\'t have my recovery key" explains, then goes back', (
      WidgetTester tester,
    ) async {
      await setUpVault(tester);
      await tapOn(tester, find.text(l10n.recoverNoKey));
      expect(textOf(l10n.forgotTitle), findsOneWidget);
      expect(textOf(l10n.forgotBody1), findsOneWidget);
      expect(textOf(l10n.forgotBody2), findsOneWidget);
      expect(textOf(l10n.forgotBody3), findsOneWidget);
      await tapOn(tester, find.text(l10n.actionBack));
      expect(textOf(l10n.lockTitle), findsOneWidget);
    });
  });

  group('Home shell', () {
    Future<void> unlockedHome(WidgetTester tester, {required Size size}) async {
      final FakeVaultApi api = newFakeApi();
      final RecoveryKeyResult r = await createThroughApi(api);
      final List<String> g = keyGroups(r);
      await api.confirmRecoveryKey(<GroupAnswer>[
        for (final int i in r.challenge) GroupAnswer(index: i, text: g[i]),
      ]);
      await pumpApp(tester, api: api, size: size);
    }

    testWidgets('desktop width shows three panes', (WidgetTester tester) async {
      await unlockedHome(tester, size: const Size(kDesktopBreakpoint, 700));
      expect(find.byType(NavigationRail), findsOneWidget);
      expect(textOf(l10n.homeListPlaceholder), findsOneWidget);
      expect(textOf(l10n.homeUnlockedMessage), findsOneWidget);
    });

    testWidgets('compact width is a single pane', (WidgetTester tester) async {
      await unlockedHome(tester, size: const Size(kDesktopBreakpoint - 1, 700));
      expect(find.byType(NavigationRail), findsNothing);
      expect(textOf(l10n.homeListPlaceholder), findsNothing);
      expect(textOf(l10n.homeUnlockedMessage), findsOneWidget);
    });

    testWidgets('Lock returns to the lock screen', (WidgetTester tester) async {
      await unlockedHome(tester, size: const Size(420, 800));
      await tapOn(tester, find.text(l10n.actionLock));
      expect(textOf(l10n.lockTitle), findsOneWidget);
    });
  });
}
