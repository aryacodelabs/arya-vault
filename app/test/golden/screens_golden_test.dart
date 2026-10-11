@Tags(<String>['golden'])
library;

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

import '../screen_cases.dart';

/// Welcome, Create, Recovery key, Verify and Lock in light, dark and 200% text.
///
/// Regenerate with (from app/):
///   flutter test --update-goldens test/golden
/// See test/golden/README.md for the font note and the platform it must run on.
const List<String> _screens = <String>[
  'welcome',
  'create-with-feedback',
  'recovery-key',
  'verify',
  'lock-quick',
];

enum _Variant {
  light(ThemeMode.light, 1),
  dark(ThemeMode.dark, 1),
  largeText(ThemeMode.light, 2);

  const _Variant(this.mode, this.textScale);
  final ThemeMode mode;
  final double textScale;
}

void main() {
  setUp(() => EditableText.debugDeterministicCursor = true);
  tearDown(() => EditableText.debugDeterministicCursor = false);

  for (final String screen in _screens) {
    for (final _Variant v in _Variant.values) {
      testWidgets('$screen / ${v.name}', (WidgetTester tester) async {
        await pumpScreen(
          tester,
          screen,
          size: const Size(400, 800),
          textScale: v.textScale,
          themeMode: v.mode,
        );
        await expectLater(
          find.byType(MaterialApp),
          matchesGoldenFile('goldens/${screen}_${v.name}.png'),
        );
      });
    }
  }
}
