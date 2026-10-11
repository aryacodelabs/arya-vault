import 'package:arya_vault/src/api/dtos.dart';
import 'package:arya_vault/src/services/auto_lock.dart';
import 'package:arya_vault/src/services/fake_services.dart';
import 'package:arya_vault/src/services/services.dart';
import 'package:arya_vault/src/state/providers.dart';
import 'package:arya_vault/src/state/security_services.dart';
import 'package:arya_vault/src/state/settings_state.dart';
import 'package:arya_vault/src/state/vault_query.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';

import 'helpers.dart';
import 'tools/tool_helpers.dart';
import 'vault/vault_helpers.dart';

// SEC-A04 / SEC-H01 / SEC-H02 through the real app wiring (SecurityScope).
void main() {
  late FakeTimeSource time;
  late FakeCaptureProtection capture;

  setUp(() {
    time = FakeTimeSource();
    capture = FakeCaptureProtection();
  });

  Future<Harness> pump(
    WidgetTester tester, {
    BufferSpyApi? api,
    Services? services,
  }) async => pumpVault(
    tester,
    api: api ?? await unlockedBufferVault(),
    services: services,
    overrides: [
      timeSourceProvider.overrideWithValue(time),
      captureProtectionProvider.overrideWithValue(capture),
    ],
  );

  group('auto-lock in the app', () {
    testWidgets('locks after the idle time and shows the lock screen', (
      WidgetTester tester,
    ) async {
      final Harness h = await pump(tester);
      expect(h.container.read(autoLockProvider).armed, isTrue);
      time.advance(const Duration(minutes: 4, seconds: 59));
      await tester.pumpAndSettle();
      expect(find.text(l10n.lockTitle), findsNothing);
      time.advance(const Duration(seconds: 1));
      await tester.pumpAndSettle();
      expect(find.text(l10n.lockTitle), findsOneWidget);
      expect((await h.api.status()).locked, isTrue);
      expect(h.container.read(autoLockProvider).armed, isFalse);
    });

    testWidgets('pointer input resets the countdown', (
      WidgetTester tester,
    ) async {
      await pump(tester);
      time.advance(const Duration(minutes: 4));
      await tester.tapAt(const Offset(300, 400));
      time.advance(const Duration(minutes: 4));
      await tester.pumpAndSettle();
      expect(find.text(l10n.lockTitle), findsNothing);
      time.advance(const Duration(minutes: 1, seconds: 1));
      await tester.pumpAndSettle();
      expect(find.text(l10n.lockTitle), findsOneWidget);
    });

    testWidgets('key input resets it too', (WidgetTester tester) async {
      await pump(tester);
      time.advance(const Duration(minutes: 4));
      await tester.sendKeyEvent(LogicalKeyboardKey.keyA);
      time.advance(const Duration(minutes: 4));
      await tester.pumpAndSettle();
      expect(find.text(l10n.lockTitle), findsNothing);
    });

    testWidgets('sleep and screen lock lock at once', (
      WidgetTester tester,
    ) async {
      final Services s = Services();
      await pump(tester, services: s);
      s.events.emit(PlatformEvent.sleep);
      await tester.pumpAndSettle();
      expect(find.text(l10n.lockTitle), findsOneWidget);
    });

    testWidgets('screen lock locks, unless the setting is off', (
      WidgetTester tester,
    ) async {
      final BufferSpyApi api = await unlockedBufferVault();
      await api.setSettings(const AppSettings(lockOnScreenLock: false));
      final Services s = Services();
      await pump(tester, api: api, services: s);
      s.events.emit(PlatformEvent.screenLocked);
      await tester.pumpAndSettle();
      expect(find.text(l10n.lockTitle), findsNothing);
      expect((await api.status()).locked, isFalse);
    });

    testWidgets('a changed timeout applies without restarting', (
      WidgetTester tester,
    ) async {
      final Harness h = await pump(tester);
      final AppSettings s = await h.api.getSettings();
      await h.container
          .read(settingsEditorProvider.notifier)
          .save(s.copyWith(autoLockMinutes: 1));
      await tester.pumpAndSettle();
      expect(
        h.container.read(autoLockProvider).config.timeout,
        const Duration(minutes: 1),
      );
      time.advance(const Duration(seconds: 61));
      await tester.pumpAndSettle();
      expect(find.text(l10n.lockTitle), findsOneWidget);
    });

    testWidgets('unlocking again arms a fresh countdown', (
      WidgetTester tester,
    ) async {
      final Harness h = await pump(tester);
      time.advance(const Duration(minutes: 5));
      await tester.pumpAndSettle();
      expect(find.text(l10n.lockTitle), findsOneWidget);
      await typeInto(tester, l10n.masterPasswordLabel, kPassword);
      await tapOn(tester, buttonText(l10n.lockSubmit));
      expect(find.text(l10n.lockTitle), findsNothing);
      expect(h.container.read(autoLockProvider).armed, isTrue);
      time.advance(const Duration(minutes: 4));
      await tester.pumpAndSettle();
      expect(find.text(l10n.lockTitle), findsNothing);
      time.advance(const Duration(minutes: 1));
      await tester.pumpAndSettle();
      expect(find.text(l10n.lockTitle), findsOneWidget);
    });

    testWidgets('nothing is armed on the lock or welcome screens', (
      WidgetTester tester,
    ) async {
      final Harness h = await pumpApp(
        tester,
        overrides: [timeSourceProvider.overrideWithValue(time)],
      );
      expect(find.text(l10n.welcomeTitle), findsOneWidget);
      expect(h.container.read(autoLockProvider).armed, isFalse);
      time.advance(const Duration(hours: 1));
      await tester.pumpAndSettle();
      expect(find.text(l10n.welcomeTitle), findsOneWidget);
    });

    testWidgets('a lock hides revealed secrets and clears our clipboard copy', (
      WidgetTester tester,
    ) async {
      final BufferSpyApi api = await unlockedBufferVault(
        samples: true,
        items: 30,
      );
      final Services s = Services();
      final Harness h = await pump(tester, api: api, services: s);
      final ItemView login = await findItem(
        api,
        (ItemView v) => v.type == ItemType.login,
      );
      h.container.read(vaultQueryProvider.notifier).select(login.id);
      await tester.pumpAndSettle();
      await tapOn(tester, find.byTooltip(l10n.copyField(l10n.fieldPassword)));
      expect(s.clipboard.current, isNotNull);
      expect(s.clipboard.sensitiveFlags.last, isTrue);
      time.advance(const Duration(minutes: 5));
      await tester.pumpAndSettle();
      expect(find.text(l10n.lockTitle), findsOneWidget);
      expect(s.clipboard.current, isNull, reason: 'cleared on lock');
      expect(s.clipboard.clearCount, 1);
    });
  });

  group('clipboard in the app', () {
    testWidgets('clears after the configured seconds, only if still ours', (
      WidgetTester tester,
    ) async {
      final Services s = Services();
      final Harness h = await pump(tester, services: s);
      final manager = h.container.read(clipboardManagerProvider);
      await manager.copyText('CANARY-clip-app');
      expect(s.clipboard.current, isNotNull);
      time.advance(const Duration(seconds: 29));
      await tester.pump();
      expect(s.clipboard.clearCount, 0);
      time.advance(const Duration(seconds: 1));
      await tester.pump();
      expect(s.clipboard.clearCount, 1);
      // A second copy that the user then replaces is left alone.
      await manager.copyText('CANARY-clip-app-2');
      s.clipboard.userCopies('something else');
      time.advance(const Duration(seconds: 30));
      await tester.pump();
      expect(s.clipboard.clearCount, 1);
    });

    testWidgets('follows the clipboard setting', (WidgetTester tester) async {
      final Services s = Services();
      final Harness h = await pump(tester, services: s);
      final AppSettings cur = await h.api.getSettings();
      await h.container
          .read(settingsEditorProvider.notifier)
          .save(cur.copyWith(clipboardClearSeconds: 10));
      await tester.pumpAndSettle();
      await h.container.read(clipboardManagerProvider).copyText('CANARY-ten');
      time.advance(const Duration(seconds: 10));
      await tester.pump();
      expect(s.clipboard.clearCount, 1);
    });

    testWidgets('a platform without a clipboard reports it and keeps working', (
      WidgetTester tester,
    ) async {
      final BufferSpyApi api = await unlockedBufferVault(
        samples: true,
        items: 30,
      );
      final Services s = Services();
      s.clipboard.failWrite = true; // what the Unavailable platform does
      final Harness h = await pump(tester, api: api, services: s);
      final ItemView login = await findItem(
        api,
        (ItemView v) => v.type == ItemType.login,
      );
      h.container.read(vaultQueryProvider.notifier).select(login.id);
      await tester.pumpAndSettle();
      await tapOn(tester, find.byTooltip(l10n.copyField(l10n.fieldPassword)));
      expect(find.text(l10n.recoveryServiceUnavailable), findsOneWidget);
    });
  });

  group('screen capture in the app', () {
    testWidgets(
      'is blocked from the first frame, before any setting is known',
      (WidgetTester tester) async {
        await pumpApp(
          tester,
          overrides: [captureProtectionProvider.overrideWithValue(capture)],
        );
        expect(capture.calls.first, isTrue);
      },
    );

    testWidgets('follows the saved setting once unlocked', (
      WidgetTester tester,
    ) async {
      final BufferSpyApi api = await unlockedBufferVault();
      await api.setSettings(const AppSettings(blockScreenCapture: false));
      await pump(tester, api: api);
      expect(capture.calls.first, isTrue);
      expect(capture.blocked, isFalse);
    });

    testWidgets('a missing implementation never breaks the app', (
      WidgetTester tester,
    ) async {
      final Harness h = await pumpVault(
        tester,
        api: await unlockedBufferVault(),
        overrides: [timeSourceProvider.overrideWithValue(time)],
      );
      expect(h.container.read(captureProtectionProvider).available, isFalse);
      expect(find.text(l10n.homeTitle), findsOneWidget);
    });
  });

  test('the auto-lock configuration is built from the vault settings', () {
    final AutoLockConfig c = autoLockConfigFor(
      const AppSettings(
        autoLockMinutes: 15,
        lockOnSleep: false,
        lockOnScreenLock: true,
      ),
    );
    expect(c.timeout, const Duration(minutes: 15));
    expect(c.lockOnSleep, isFalse);
    expect(c.lockOnScreenLock, isTrue);
    expect(autoLockConfigFor(null).timeout, const Duration(minutes: 5));
  });
}
