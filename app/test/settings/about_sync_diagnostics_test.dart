import 'dart:convert';

import 'package:arya_vault/src/api/app_error.dart';
import 'package:arya_vault/src/app_info.dart';
import 'package:arya_vault/src/state/routing.dart';
import 'package:flutter/material.dart' hide Page;
import 'package:flutter_test/flutter_test.dart';

import '../helpers.dart';
import '../tools/tool_helpers.dart';

void main() {
  group('Settings home', () {
    testWidgets('lists Security, Sync, Import, Export and About', (
      WidgetTester tester,
    ) async {
      await pumpRoute(tester, AppRoutes.settings);
      for (final String t in <String>[
        l10n.settingsSecurity,
        l10n.settingsSync,
        l10n.navImport,
        l10n.navExport,
        l10n.settingsAbout,
      ]) {
        expect(find.text(t), findsOneWidget);
      }
    });

    testWidgets('each entry opens its page', (WidgetTester tester) async {
      final Harness h = await pumpRoute(tester, AppRoutes.settings);
      Future<void> visit(String entry, String title) async {
        await tapOn(tester, find.text(entry));
        expect(find.text(title), findsWidgets);
        h.container.read(routerProvider).pop();
        await tester.pumpAndSettle();
      }

      await visit(l10n.settingsSecurity, l10n.secAutoLockTitle);
      await visit(l10n.settingsSync, l10n.syncComingSoon);
      await visit(l10n.navImport, l10n.importIntro);
      await visit(l10n.navExport, l10n.exportIntro);
      await visit(l10n.settingsAbout, l10n.aboutApp);
    });
  });

  group('Sync & backup', () {
    testWidgets('is an honest "coming soon" with nothing to toggle', (
      WidgetTester tester,
    ) async {
      await pumpRoute(tester, AppRoutes.settingsSync);
      expect(find.text(l10n.syncComingSoon), findsOneWidget);
      expect(find.text(l10n.syncBody), findsOneWidget);
      expect(find.byType(Switch), findsNothing);
      expect(find.byType(Checkbox), findsNothing);
      expect(find.byType(FilledButton), findsNothing);
    });
  });

  group('About', () {
    testWidgets('shows the app version and the versions from info()', (
      WidgetTester tester,
    ) async {
      await pumpRoute(tester, AppRoutes.settingsAbout);
      expect(find.text(kAppVersion), findsOneWidget);
      expect(find.text('0.0.0-fake'), findsOneWidget);
      expect(find.text('1'), findsWidgets, reason: 'api and format version');
      expect(find.text(l10n.aboutRepoUrl), findsOneWidget);
      expect(find.text(l10n.aboutLicensesRust), findsOneWidget);
    });

    testWidgets('the licenses entry opens the Flutter license page', (
      WidgetTester tester,
    ) async {
      await pumpRoute(tester, AppRoutes.settingsAbout);
      await tapOn(tester, find.text(l10n.aboutLicenses));
      expect(find.byType(LicensePage), findsOneWidget);
    });

    testWidgets('an info() error is reported, the rest still shows', (
      WidgetTester tester,
    ) async {
      final BufferSpyApi api = await unlockedBufferVault();
      api.failInfo = const AppError(AppErrorCode.internal, 'x');
      await pumpRoute(tester, AppRoutes.settingsAbout, api: api);
      expect(find.text(l10n.aboutInfoFailed), findsOneWidget);
      expect(find.text(kAppVersion), findsOneWidget);
    });
  });

  group('Diagnostics', () {
    testWidgets('shows the report before anything is saved', (
      WidgetTester tester,
    ) async {
      final Services s = Services();
      await pumpRoute(tester, AppRoutes.settingsDiagnostics, services: s);
      expect(find.text(l10n.diagIntro), findsOneWidget);
      expect(find.text(l10n.diagSave), findsNothing);
      await tapOn(tester, find.text(l10n.diagPrepare));
      expect(find.text('{"fake":true}'), findsOneWidget);
      expect(s.filePicker.saved, isEmpty, reason: 'reviewed first, not saved');
    });

    testWidgets('Save hands the same bytes to the file chooser', (
      WidgetTester tester,
    ) async {
      final Services s = Services();
      await pumpRoute(tester, AppRoutes.settingsDiagnostics, services: s);
      await tapOn(tester, find.text(l10n.diagPrepare));
      await tapOn(tester, find.text(l10n.diagSave));
      expect(s.filePicker.saved.single.name, l10n.diagFileName);
      expect(utf8.decode(s.filePicker.saved.single.bytes), '{"fake":true}');
      expect(find.text(l10n.diagSaved), findsOneWidget);
    });

    testWidgets('a cancelled save says nothing was saved', (
      WidgetTester tester,
    ) async {
      final Services s = Services()..filePicker.saveResult = false;
      await pumpRoute(tester, AppRoutes.settingsDiagnostics, services: s);
      await tapOn(tester, find.text(l10n.diagPrepare));
      await tapOn(tester, find.text(l10n.diagSave));
      expect(find.text(l10n.diagSaved), findsNothing);
    });

    testWidgets('a missing file service is reported', (
      WidgetTester tester,
    ) async {
      final Services s = Services()..filePicker.failSave = true;
      await pumpRoute(tester, AppRoutes.settingsDiagnostics, services: s);
      await tapOn(tester, find.text(l10n.diagPrepare));
      await tapOn(tester, find.text(l10n.diagSave));
      expect(find.text(l10n.recoveryServiceUnavailable), findsOneWidget);
    });
  });
}
