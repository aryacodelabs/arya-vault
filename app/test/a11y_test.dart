import 'package:arya_vault/src/ui/shell/adaptive_shell.dart';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';

import 'helpers.dart';
import 'screen_cases.dart';

void main() {
  group('200% text scale does not overflow', () {
    for (final String name in kScreenNames) {
      for (final Size size in <Size>[
        const Size(360, 640),
        const Size(kDesktopBreakpoint, 700),
      ]) {
        testWidgets('$name at ${size.width.toInt()}x${size.height.toInt()}', (
          WidgetTester tester,
        ) async {
          await pumpScreen(tester, name, size: size, textScale: 2);
          expect(tester.takeException(), isNull);
        });
      }
    }
  });

  group('large text in dark mode does not overflow either', () {
    for (final String name in <String>[
      'create-with-feedback',
      'recovery-key',
      'verify',
      'lock-with-error',
    ]) {
      testWidgets(name, (WidgetTester tester) async {
        await pumpScreen(
          tester,
          name,
          size: const Size(360, 640),
          textScale: 2,
          themeMode: ThemeMode.dark,
        );
        expect(tester.takeException(), isNull);
      });
    }
  });

  group('accessibility guidelines', () {
    for (final String name in kScreenNames) {
      testWidgets('$name: labelled targets, 48dp targets', (
        WidgetTester tester,
      ) async {
        final SemanticsHandle handle = tester.ensureSemantics();
        await pumpScreen(tester, name);
        await expectLater(tester, meetsGuideline(labeledTapTargetGuideline));
        await expectLater(tester, meetsGuideline(androidTapTargetGuideline));
        handle.dispose();
      });
    }

    for (final ThemeMode mode in <ThemeMode>[ThemeMode.light, ThemeMode.dark]) {
      for (final String name in <String>[
        'welcome',
        'create-with-feedback',
        'recovery-key',
        'verify',
        'lock-with-error',
      ]) {
        testWidgets('$name: text contrast (${mode.name})', (
          WidgetTester tester,
        ) async {
          final SemanticsHandle handle = tester.ensureSemantics();
          await pumpScreen(tester, name, themeMode: mode);
          await expectLater(tester, meetsGuideline(textContrastGuideline));
          handle.dispose();
        });
      }
    }
  });

  group('focus order (keyboard navigation)', () {
    /// A short description of what has keyboard focus.
    String focused() {
      final BuildContext? ctx = FocusManager.instance.primaryFocus?.context;
      if (ctx == null) return 'none';
      String? result;
      ctx.visitAncestorElements((Element e) {
        final Widget w = e.widget;
        if (w is TextField) {
          result = 'field:${w.decoration?.labelText}';
          return false;
        }
        if (w is IconButton) {
          result = 'icon:${w.tooltip}';
          return false;
        }
        if (w is ButtonStyleButton && w is! IconButton) {
          final Iterable<Element> text = find
              .descendant(of: find.byWidget(w), matching: find.byType(Text))
              .evaluate();
          if (text.isEmpty) {
            return true; // the internals of an IconButton: keep climbing
          }
          result = 'button:${(text.first.widget as Text).data}';
          return false;
        }
        if (w is CheckboxListTile || w is Checkbox) {
          result = 'checkbox';
          return false;
        }
        return true;
      });
      return result ?? 'other';
    }

    Future<List<String>> tabThrough(WidgetTester tester, int times) async {
      final List<String> order = <String>[focused()];
      for (var i = 0; i < times; i++) {
        await tester.sendKeyEvent(LogicalKeyboardKey.tab);
        await tester.pumpAndSettle();
        order.add(focused());
      }
      return order;
    }

    testWidgets('lock screen: password, reveal, unlock, recovery link', (
      WidgetTester tester,
    ) async {
      await pumpScreen(tester, 'lock');
      expect(await tabThrough(tester, 3), <String>[
        'field:${l10n.masterPasswordLabel}',
        'icon:${l10n.showPassword}',
        'button:${l10n.lockSubmit}',
        'button:${l10n.lockUseRecovery}',
      ]);
    });

    testWidgets('lock screen with quick unlock: quick button comes first', (
      WidgetTester tester,
    ) async {
      await pumpScreen(tester, 'lock-quick');
      final List<String> order = await tabThrough(tester, 4);
      expect(
        order,
        contains('button:${l10n.lockQuick(l10n.quickKindWindowsHello)}'),
      );
      expect(
        order.first,
        'button:${l10n.lockQuick(l10n.quickKindWindowsHello)}',
      );
      expect(order, contains('field:${l10n.masterPasswordLabel}'));
    });

    testWidgets('create screen: password, reveal, confirm, reveal', (
      WidgetTester tester,
    ) async {
      await pumpScreen(tester, 'create');
      final List<String> order = await tabThrough(tester, 3);
      expect(order, <String>[
        'field:${l10n.masterPasswordLabel}',
        'icon:${l10n.showPassword}',
        'field:${l10n.confirmPasswordLabel}',
        'icon:${l10n.showPassword}',
      ]);
    });

    testWidgets('verify screen: three fields in order, then Verify', (
      WidgetTester tester,
    ) async {
      final Harness h = await pumpScreen(tester, 'verify');
      final List<int> challenge = pendingChallenge(h);
      await typeAnswers(tester, <String>['AAAAA', 'BBBBB', 'CCCCC']);
      await tester.tap(find.byType(TextField).first); // focus the first field
      await tester.pumpAndSettle();
      final List<String> order = await tabThrough(tester, 3);
      expect(order[0], 'field:${l10n.verifyGroupField(challenge[0] + 1)}');
      expect(order[1], 'field:${l10n.verifyGroupField(challenge[1] + 1)}');
      expect(order[2], 'field:${l10n.verifyGroupField(challenge[2] + 1)}');
      expect(order[3], 'button:${l10n.verifySubmit}');
    });

    testWidgets('welcome screen: create gets initial focus', (
      WidgetTester tester,
    ) async {
      await pumpScreen(tester, 'welcome');
      expect(focused(), 'button:${l10n.welcomeCreate}');
    });
  });
}
