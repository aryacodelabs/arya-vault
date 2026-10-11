@Tags(<String>['golden'])
library;

import 'package:flutter/material.dart' hide Page;
import 'package:flutter_test/flutter_test.dart';

import '../vault/vault_screens.dart';

/// List (with detail), detail masked and revealed (with a fake canary), edit
/// login, trash and history compare, in light and dark. Dates come from a fixed
/// clock; run with TZ=UTC (see test/golden/README.md).
///
/// Regenerate with (from app/):
///   TZ=UTC flutter test --update-goldens test/golden
const List<(String screen, Size size)> _cases = <(String, Size)>[
  ('list-selected', Size(1100, 700)),
  ('detail-masked', Size(400, 800)),
  ('detail-revealed', Size(400, 800)),
  ('edit-login-existing', Size(400, 800)),
  ('trash', Size(1100, 700)),
  ('compare', Size(400, 800)),
];

void main() {
  setUp(() => EditableText.debugDeterministicCursor = true);
  tearDown(() => EditableText.debugDeterministicCursor = false);

  for (final (String screen, Size size) in _cases) {
    for (final ThemeMode mode in <ThemeMode>[ThemeMode.light, ThemeMode.dark]) {
      testWidgets('$screen / ${mode.name}', (WidgetTester tester) async {
        await pumpVaultScreen(tester, screen, size: size, themeMode: mode);
        await expectLater(
          find.byType(MaterialApp),
          matchesGoldenFile('goldens/vault_${screen}_${mode.name}.png'),
        );
      });
    }
  }
}
