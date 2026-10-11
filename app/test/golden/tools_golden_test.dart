@Tags(<String>['golden'])
library;

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

import '../tools/tool_screens.dart';

/// B03: Generator, Health, Security settings and Import preview, light and
/// dark. Regenerate on Linux with (from app/):
///   TZ=UTC flutter test --update-goldens test/golden
/// See test/golden/README.md.
void main() {
  setUp(() => EditableText.debugDeterministicCursor = true);
  tearDown(() => EditableText.debugDeterministicCursor = false);

  for (final String screen in kToolScreens) {
    for (final ThemeMode mode in <ThemeMode>[ThemeMode.light, ThemeMode.dark]) {
      testWidgets('$screen / ${mode.name}', (WidgetTester tester) async {
        await pumpToolScreen(
          tester,
          screen,
          size: const Size(420, 900),
          themeMode: mode,
        );
        await expectLater(
          find.byType(MaterialApp),
          matchesGoldenFile('goldens/tools_${screen}_${mode.name}.png'),
        );
      });
    }
  }
}
