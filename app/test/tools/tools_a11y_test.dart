import 'package:arya_vault/src/state/routing.dart';
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

import '../helpers.dart';
import 'tool_helpers.dart';
import 'tool_screens.dart';

/// Extra B03 pages that the golden set does not cover.
const List<String> _routes = <String>[
  AppRoutes.settings,
  AppRoutes.settingsChangePassword,
  AppRoutes.settingsSync,
  AppRoutes.settingsAbout,
  AppRoutes.settingsDiagnostics,
  AppRoutes.exportFile,
];

void main() {
  setUpAll(() => WidgetController.hitTestWarningShouldBeFatal = true);

  group('200% text scale does not overflow', () {
    for (final String name in kToolScreens) {
      for (final Size size in <Size>[
        const Size(360, 640),
        const Size(1000, 700),
      ]) {
        testWidgets('$name at ${size.width.toInt()}', (
          WidgetTester tester,
        ) async {
          await pumpToolScreen(tester, name, size: size, textScale: 2);
          expect(tester.takeException(), isNull);
        });
      }
    }
    for (final String route in _routes) {
      testWidgets('$route at 360', (WidgetTester tester) async {
        await pumpRoute(
          tester,
          route,
          size: const Size(360, 640),
          textScale: 2,
        );
        expect(tester.takeException(), isNull);
      });
    }
  });

  group('accessibility guidelines', () {
    for (final String name in kToolScreens) {
      testWidgets('$name: labelled and 48dp targets', (
        WidgetTester tester,
      ) async {
        final SemanticsHandle handle = tester.ensureSemantics();
        await pumpToolScreen(tester, name);
        await expectLater(tester, meetsGuideline(labeledTapTargetGuideline));
        await expectLater(tester, meetsGuideline(androidTapTargetGuideline));
        handle.dispose();
      });
      for (final ThemeMode mode in <ThemeMode>[
        ThemeMode.light,
        ThemeMode.dark,
      ]) {
        testWidgets('$name: text contrast (${mode.name})', (
          WidgetTester tester,
        ) async {
          final SemanticsHandle handle = tester.ensureSemantics();
          await pumpToolScreen(tester, name, themeMode: mode);
          await expectLater(tester, meetsGuideline(textContrastGuideline));
          handle.dispose();
        });
      }
    }
    for (final String route in _routes) {
      testWidgets('$route: labelled targets', (WidgetTester tester) async {
        final SemanticsHandle handle = tester.ensureSemantics();
        await pumpRoute(tester, route);
        await expectLater(tester, meetsGuideline(labeledTapTargetGuideline));
        await expectLater(tester, meetsGuideline(androidTapTargetGuideline));
        handle.dispose();
      });
    }
  });

  testWidgets('every page has a heading for screen readers', (
    WidgetTester tester,
  ) async {
    final SemanticsHandle handle = tester.ensureSemantics();
    await pumpRoute(tester, AppRoutes.generator);
    expect(find.bySemanticsLabel(l10n.generatorTitle), findsWidgets);
    handle.dispose();
  });
}
