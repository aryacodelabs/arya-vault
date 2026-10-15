import 'dart:async';

import 'package:arya_vault/src/api/app_error.dart';
import 'package:arya_vault/src/api/dtos.dart';
import 'package:arya_vault/src/services/fake_services.dart';
import 'package:arya_vault/src/state/onboarding_state.dart';
import 'package:arya_vault/src/state/providers.dart';
import 'package:arya_vault/src/state/routing.dart';
import 'package:arya_vault/src/state/vault_status.dart';
import 'package:flutter/material.dart' hide Page;
import 'package:flutter_test/flutter_test.dart';

import '../helpers.dart';
import '../tools/tool_helpers.dart';

// SEC-A03/A04 (settings), SEC-H01/H02/H03 (settings), SEC-A06 (rotate keys).
Future<void> _choose(WidgetTester tester, String field, String option) async {
  await tapOn(tester, find.widgetWithText(DropdownButtonFormField<int>, field));
  await tester.tap(find.text(option).last);
  await tester.pumpAndSettle();
}

void main() {
  const Size tall = Size(700, 2200);

  group('auto-lock, clipboard, reveal, capture', () {
    testWidgets('shows the stored values and no "never" option', (
      WidgetTester tester,
    ) async {
      final BufferSpyApi api = await unlockedBufferVault();
      await pumpRoute(tester, AppRoutes.settingsSecurity, api: api, size: tall);
      expect(find.text(l10n.secAutoLockMinutes(5)), findsOneWidget);
      expect(find.text(l10n.secSeconds(30)), findsOneWidget);
      expect(find.text(l10n.secSeconds(15)), findsOneWidget);
      await tapOn(
        tester,
        find.widgetWithText(
          DropdownButtonFormField<int>,
          l10n.secAutoLockAfter,
        ),
      );
      for (final int m in <int>[1, 5, 15, 30, 60]) {
        expect(find.text(l10n.secAutoLockMinutes(m)), findsWidgets);
      }
      expect(
        find.textContaining(RegExp('never', caseSensitive: false)),
        findsOneWidget,
        reason: 'only the note that says there is no "never"',
      );
    });

    testWidgets('auto-lock minutes are saved through the API', (
      WidgetTester tester,
    ) async {
      final BufferSpyApi api = await unlockedBufferVault();
      await pumpRoute(tester, AppRoutes.settingsSecurity, api: api, size: tall);
      await _choose(tester, l10n.secAutoLockAfter, l10n.secAutoLockMinutes(15));
      expect((await api.getSettings()).autoLockMinutes, 15);
      expect(find.text(l10n.secAutoLockMinutes(15)), findsOneWidget);
    });

    testWidgets('clipboard clear and reveal-hide seconds are saved', (
      WidgetTester tester,
    ) async {
      final BufferSpyApi api = await unlockedBufferVault();
      await pumpRoute(tester, AppRoutes.settingsSecurity, api: api, size: tall);
      await _choose(tester, l10n.secClipboardClear, l10n.secSeconds(60));
      await _choose(tester, l10n.secRevealHide, l10n.secSeconds(5));
      final AppSettings s = await api.getSettings();
      expect(s.clipboardClearSeconds, 60);
      expect(s.revealHideSeconds, 5);
    });

    testWidgets('the screen-lock and sleep switches are saved', (
      WidgetTester tester,
    ) async {
      final BufferSpyApi api = await unlockedBufferVault();
      await pumpRoute(tester, AppRoutes.settingsSecurity, api: api, size: tall);
      await tapOn(
        tester,
        find.widgetWithText(SwitchListTile, l10n.secLockOnScreenLock),
      );
      await tapOn(
        tester,
        find.widgetWithText(SwitchListTile, l10n.secLockOnSleep),
      );
      await tapOn(
        tester,
        find.widgetWithText(SwitchListTile, l10n.secLockOnMinimize),
      );
      final AppSettings s = await api.getSettings();
      expect(s.lockOnScreenLock, isFalse);
      expect(s.lockOnSleep, isFalse);
      expect(
        s.lockOnMinimize,
        isTrue,
        reason: 'off by default, now switched on',
      );
    });

    testWidgets('settings persist across leaving and reopening', (
      WidgetTester tester,
    ) async {
      final BufferSpyApi api = await unlockedBufferVault();
      final Harness h = await pumpRoute(
        tester,
        AppRoutes.settingsSecurity,
        api: api,
        size: tall,
      );
      await _choose(tester, l10n.secAutoLockAfter, l10n.secAutoLockMinutes(30));
      h.container.read(routerProvider).pop();
      await tester.pumpAndSettle();
      unawaited(
        h.container.read(routerProvider).push(AppRoutes.settingsSecurity),
      );
      await tester.pumpAndSettle();
      expect(find.text(l10n.secAutoLockMinutes(30)), findsOneWidget);
    });

    testWidgets('a value the core stored outside the list is still shown', (
      WidgetTester tester,
    ) async {
      final BufferSpyApi api = await unlockedBufferVault();
      await api.setSettings(const AppSettings(autoLockMinutes: 7));
      await pumpRoute(tester, AppRoutes.settingsSecurity, api: api, size: tall);
      expect(find.text(l10n.secAutoLockMinutes(7)), findsOneWidget);
    });

    testWidgets('a failed save says so and keeps the old value', (
      WidgetTester tester,
    ) async {
      final BufferSpyApi api = await unlockedBufferVault();
      await pumpRoute(tester, AppRoutes.settingsSecurity, api: api, size: tall);
      api.failSettings = const AppError(AppErrorCode.io, 'disk');
      await _choose(tester, l10n.secAutoLockAfter, l10n.secAutoLockMinutes(60));
      expect(find.textContaining(l10n.settingsSaveFailed), findsOneWidget);
      expect((await api.getSettings()).autoLockMinutes, 5);
    });

    testWidgets('a settings read error offers Try again and writes nothing', (
      WidgetTester tester,
    ) async {
      final BufferSpyApi api = await unlockedBufferVault();
      await api.setSettings(const AppSettings(autoLockMinutes: 15));
      api.setSettingsCalls = 0;
      api.failGetSettings = const AppError(AppErrorCode.io, 'disk');
      await pumpRoute(tester, AppRoutes.settingsSecurity, api: api, size: tall);
      expect(find.text(l10n.settingsLoadFailed), findsOneWidget);
      expect(find.byType(DropdownButtonFormField<int>), findsNothing);
      api.failGetSettings = null;
      await tapOn(tester, find.text(l10n.actionTryAgain));
      expect(find.text(l10n.secAutoLockMinutes(15)), findsOneWidget);
      expect(
        api.setSettingsCalls,
        0,
        reason: 'defaults are never written back',
      );
    });

    testWidgets(
      'screen capture: not available on this build is shown, not faked',
      (WidgetTester tester) async {
        final BufferSpyApi api = await unlockedBufferVault();
        await pumpRoute(
          tester,
          AppRoutes.settingsSecurity,
          api: api,
          size: tall,
        );
        expect(find.text(l10n.notAvailableBuild), findsWidgets);
        final SwitchListTile t = tester.widget<SwitchListTile>(
          find.widgetWithText(SwitchListTile, l10n.secCaptureBlock),
        );
        expect(t.onChanged, isNull);
      },
    );

    testWidgets('screen capture: with a platform it toggles and is applied', (
      WidgetTester tester,
    ) async {
      final BufferSpyApi api = await unlockedBufferVault();
      final FakeCaptureProtection capture = FakeCaptureProtection();
      await pumpRoute(
        tester,
        AppRoutes.settingsSecurity,
        api: api,
        size: tall,
        overrides: [captureProtectionProvider.overrideWithValue(capture)],
      );
      expect(capture.calls.first, isTrue, reason: 'blocked from the start');
      await tapOn(
        tester,
        find.widgetWithText(SwitchListTile, l10n.secCaptureBlock),
      );
      expect((await api.getSettings()).blockScreenCapture, isFalse);
      expect(capture.blocked, isFalse);
      await tapOn(
        tester,
        find.widgetWithText(SwitchListTile, l10n.secCaptureBlock),
      );
      expect(capture.blocked, isTrue);
    });
  });

  group('quick unlock', () {
    testWidgets('unsupported: says so and offers no switch', (
      WidgetTester tester,
    ) async {
      final BufferSpyApi api = await unlockedBufferVault(
        quickUnlock: QuickUnlockStatus.unsupported,
      );
      await pumpRoute(tester, AppRoutes.settingsSecurity, api: api, size: tall);
      expect(find.text(l10n.secQuickOnlyPassword), findsOneWidget);
      expect(find.text(l10n.notAvailableDevice), findsOneWidget);
      expect(
        find.widgetWithText(
          SwitchListTile,
          l10n.secQuickToggle(l10n.quickKindWindowsHello),
        ),
        findsNothing,
      );
    });

    testWidgets('turning it on requires the master password', (
      WidgetTester tester,
    ) async {
      final BufferSpyApi api = await unlockedBufferVault();
      final Harness h = await pumpRoute(
        tester,
        AppRoutes.settingsSecurity,
        api: api,
        size: tall,
      );
      final Finder toggle = find.widgetWithText(
        SwitchListTile,
        l10n.secQuickToggle(l10n.quickKindWindowsHello),
      );
      await tapOn(tester, toggle);
      expect(find.text(l10n.secQuickEnableBody), findsOneWidget);
      await typeInto(tester, l10n.masterPasswordLabel, 'CANARY wrong password');
      await tapOn(tester, find.text(l10n.reauthConfirm));
      expect(find.text(l10n.reauthWrong), findsOneWidget);
      expect((await api.status()).quickUnlock.enabled, isFalse);
      await typeInto(tester, l10n.masterPasswordLabel, kPassword);
      await tapOn(tester, find.text(l10n.reauthConfirm));
      expect((await api.status()).quickUnlock.enabled, isTrue);
      expect(
        h.container.read(vaultStatusProvider).value!.quickUnlock.enabled,
        isTrue,
      );
      expect(tester.widget<SwitchListTile>(toggle).value, isTrue);
      expect(api.received.every(isWiped), isTrue);
    });

    testWidgets('cancelling the prompt leaves it off', (
      WidgetTester tester,
    ) async {
      final BufferSpyApi api = await unlockedBufferVault();
      await pumpRoute(tester, AppRoutes.settingsSecurity, api: api, size: tall);
      await tapOn(
        tester,
        find.widgetWithText(
          SwitchListTile,
          l10n.secQuickToggle(l10n.quickKindWindowsHello),
        ),
      );
      await tapOn(tester, find.text(l10n.actionCancel));
      expect((await api.status()).quickUnlock.enabled, isFalse);
    });

    testWidgets('turning it off needs no password', (
      WidgetTester tester,
    ) async {
      final BufferSpyApi api = await unlockedBufferVault();
      await api.quickUnlockEnable();
      await pumpRoute(tester, AppRoutes.settingsSecurity, api: api, size: tall);
      await tapOn(
        tester,
        find.widgetWithText(
          SwitchListTile,
          l10n.secQuickToggle(l10n.quickKindWindowsHello),
        ),
      );
      expect(find.text(l10n.secQuickEnableBody), findsNothing);
      expect((await api.status()).quickUnlock.enabled, isFalse);
    });
  });

  group('recovery key', () {
    testWidgets('"view" explains it cannot be shown and has no copy button', (
      WidgetTester tester,
    ) async {
      final BufferSpyApi api = await unlockedBufferVault();
      final Services s = Services();
      await pumpRoute(
        tester,
        AppRoutes.settingsSecurity,
        api: api,
        services: s,
        size: tall,
      );
      await tapOn(tester, find.text(l10n.secViewRecovery));
      expect(find.text(l10n.secViewRecoveryTitle), findsOneWidget);
      expect(find.text(l10n.secViewRecoveryBody), findsOneWidget);
      expect(find.byIcon(Icons.copy), findsNothing);
      expect(find.byIcon(Icons.content_copy), findsNothing);
      expect(s.clipboard.copied, isEmpty);
    });

    testWidgets('"view" leads to regeneration with the master password', (
      WidgetTester tester,
    ) async {
      final BufferSpyApi api = await unlockedBufferVault();
      final Harness h = await pumpRoute(
        tester,
        AppRoutes.settingsSecurity,
        api: api,
        size: tall,
      );
      await tapOn(tester, find.text(l10n.secViewRecovery));
      await tapOn(
        tester,
        find.widgetWithText(FilledButton, l10n.secRegenerateConfirm),
      );
      await typeInto(tester, l10n.masterPasswordLabel, kPassword);
      await tapOn(
        tester,
        find.widgetWithText(FilledButton, l10n.secRegenerateConfirm),
      );
      expect(find.text(l10n.recoveryTitle), findsOneWidget);
      expect(h.container.read(onboardingProvider).key, isNotNull);
    });

    testWidgets(
      'regenerate: confirmation, password, new key, verify, back home',
      (WidgetTester tester) async {
        final BufferSpyApi api = await unlockedBufferVault();
        final Harness h = await pumpRoute(
          tester,
          AppRoutes.settingsSecurity,
          api: api,
          size: tall,
        );
        await tapOn(tester, find.text(l10n.secRegenerateRecovery));
        expect(find.text(l10n.secRegenerateTitle), findsOneWidget);
        await tapOn(
          tester,
          find.widgetWithText(FilledButton, l10n.secRegenerateConfirm),
        );
        // Wrong password: stays on the prompt.
        await typeInto(
          tester,
          l10n.masterPasswordLabel,
          'CANARY wrong password',
        );
        await tapOn(
          tester,
          find.widgetWithText(FilledButton, l10n.secRegenerateConfirm),
        );
        expect(find.text(l10n.errWrongCredentials), findsOneWidget);
        expect((await api.status()).onboardingComplete, isTrue);
        await typeInto(tester, l10n.masterPasswordLabel, kPassword);
        await tapOn(
          tester,
          find.widgetWithText(FilledButton, l10n.secRegenerateConfirm),
        );
        // The key steps of onboarding: show the key (no copy), then verify.
        expect(find.text(l10n.recoveryTitle), findsOneWidget);
        expect((await api.status()).onboardingComplete, isFalse);
        expect(find.byIcon(Icons.copy), findsNothing);
        await storeKeyAndContinue(tester);
        expect(find.text(l10n.verifyTitle), findsOneWidget);
        final List<String> groups = pendingGroups(h);
        final List<int> ask = pendingChallenge(h);
        await typeAnswers(tester, <String>[for (final int i in ask) groups[i]]);
        await tapOn(tester, buttonText(l10n.verifySubmit));
        expect((await api.status()).onboardingComplete, isTrue);
        expect(find.text(l10n.verifyTitle), findsNothing);
        expect(find.text(l10n.recoveryTitle), findsNothing);
      },
    );
  });

  group('change master password', () {
    Future<void> fill(
      WidgetTester tester, {
      String old = kPassword,
      String fresh = kNewPassword,
      String? confirm,
    }) async {
      await typeInto(tester, l10n.changePwCurrent, old);
      await typeInto(tester, l10n.changePwNew, fresh);
      await typeInto(tester, l10n.confirmPasswordLabel, confirm ?? fresh);
    }

    FilledButton submit(WidgetTester tester) => tester.widget<FilledButton>(
      find.widgetWithText(FilledButton, l10n.changePwSubmit),
    );

    testWidgets('submit waits for a valid, matching new password', (
      WidgetTester tester,
    ) async {
      final BufferSpyApi api = await unlockedBufferVault();
      await pumpRoute(
        tester,
        AppRoutes.settingsChangePassword,
        api: api,
        size: tall,
      );
      expect(submit(tester).onPressed, isNull);
      await fill(tester, fresh: 'short');
      expect(find.textContaining('at least 12'), findsOneWidget);
      expect(submit(tester).onPressed, isNull);
      await fill(tester, confirm: 'CANARY-something-else-77!');
      expect(find.text(l10n.passwordMismatch), findsOneWidget);
      expect(submit(tester).onPressed, isNull);
      await fill(tester);
      expect(submit(tester).onPressed, isNotNull);
    });

    testWidgets('a wrong current password is reported on that field', (
      WidgetTester tester,
    ) async {
      final BufferSpyApi api = await unlockedBufferVault();
      await pumpRoute(
        tester,
        AppRoutes.settingsChangePassword,
        api: api,
        size: tall,
      );
      await fill(tester, old: 'CANARY-not-my-password-1');
      await tapOn(tester, find.text(l10n.changePwSubmit));
      expect(find.text(l10n.errWrongCredentials), findsOneWidget);
      expect(find.text(l10n.changePwTitle), findsWidgets);
      await api.lock();
      await api.unlock(bytesOf(kPassword)); // the password did not change
    });

    testWidgets('changing without rotation: confirmation, new password works', (
      WidgetTester tester,
    ) async {
      final BufferSpyApi api = await unlockedBufferVault();
      await pumpRoute(
        tester,
        AppRoutes.settingsChangePassword,
        api: api,
        size: tall,
      );
      await fill(tester);
      await tapOn(tester, find.text(l10n.changePwSubmit));
      expect(find.text(l10n.changePwDone), findsOneWidget);
      expect(
        find.text(l10n.changePwTitle),
        findsNothing,
        reason: 'the page closed',
      );
      expect((await api.status()).onboardingComplete, isTrue);
      await api.lock();
      await api.unlock(bytesOf(kNewPassword));
      expect(
        api.received.every(isWiped),
        isTrue,
        reason: 'both passwords wiped',
      );
    });

    testWidgets('with rotation: the recovery-key steps follow', (
      WidgetTester tester,
    ) async {
      final BufferSpyApi api = await unlockedBufferVault();
      final Harness h = await pumpRoute(
        tester,
        AppRoutes.settingsChangePassword,
        api: api,
        size: tall,
      );
      await fill(tester);
      await tapOn(tester, find.byType(CheckboxListTile));
      await tapOn(tester, find.text(l10n.changePwSubmit));
      expect(find.text(l10n.recoveryTitle), findsOneWidget);
      expect((await api.status()).onboardingComplete, isFalse);
      expect(find.byIcon(Icons.copy), findsNothing);
      await storeKeyAndContinue(tester);
      final List<String> groups = pendingGroups(h);
      await typeAnswers(tester, <String>[
        for (final int i in pendingChallenge(h)) groups[i],
      ]);
      await tapOn(tester, buttonText(l10n.verifySubmit));
      expect((await api.status()).onboardingComplete, isTrue);
      await api.lock();
      await api.unlock(bytesOf(kNewPassword));
      expect(api.received.every(isWiped), isTrue);
    });

    testWidgets(
      'a weak new password shows the reasons from the core and cannot be sent',
      (WidgetTester tester) async {
        final BufferSpyApi api = await unlockedBufferVault();
        await pumpRoute(
          tester,
          AppRoutes.settingsChangePassword,
          api: api,
          size: tall,
        );
        await fill(
          tester,
          fresh: 'password1234567',
          confirm: 'password1234567',
        );
        expect(find.textContaining('too easy to guess'), findsOneWidget);
        expect(submit(tester).onPressed, isNull);
      },
    );
  });
}
