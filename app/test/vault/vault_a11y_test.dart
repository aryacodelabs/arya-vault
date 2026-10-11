import 'package:flutter/material.dart' hide Page;
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';

import '../helpers.dart';
import 'vault_helpers.dart';
import 'vault_screens.dart';

void main() {
  setUpAll(() => WidgetController.hitTestWarningShouldBeFatal = true);

  group('200% text scale does not overflow', () {
    for (final String name in kVaultScreens) {
      for (final Size size in <Size>[
        const Size(360, 640),
        const Size(1000, 700),
      ]) {
        testWidgets('$name at ${size.width.toInt()}x${size.height.toInt()}', (
          WidgetTester tester,
        ) async {
          await pumpVaultScreen(tester, name, size: size, textScale: 2);
          expect(tester.takeException(), isNull);
        });
      }
    }
  });

  group('accessibility guidelines', () {
    for (final String name in kVaultScreens) {
      for (final Size size in <Size>[kCompact, kDesktop]) {
        testWidgets(
          '$name at ${size.width.toInt()}: labelled targets, 48dp targets',
          (WidgetTester tester) async {
            final SemanticsHandle handle = tester.ensureSemantics();
            await pumpVaultScreen(tester, name, size: size);
            await expectLater(
              tester,
              meetsGuideline(labeledTapTargetGuideline),
            );
            await expectLater(
              tester,
              meetsGuideline(androidTapTargetGuideline),
            );
            handle.dispose();
          },
        );
      }
    }

    for (final ThemeMode mode in <ThemeMode>[ThemeMode.light, ThemeMode.dark]) {
      for (final String name in <String>[
        'list-selected',
        'detail-revealed',
        'note-revealed',
        'edit-login-existing',
        'trash',
        'compare',
      ]) {
        testWidgets('$name: text contrast (${mode.name})', (
          WidgetTester tester,
        ) async {
          final SemanticsHandle handle = tester.ensureSemantics();
          await pumpVaultScreen(tester, name, size: kCompact, themeMode: mode);
          await expectLater(tester, meetsGuideline(textContrastGuideline));
          handle.dispose();
        });
      }
    }
  });

  group('screen-reader semantics', () {
    testWidgets('rows read as "type, title, subtitle"', (
      WidgetTester tester,
    ) async {
      final SemanticsHandle handle = tester.ensureSemantics();
      await pumpVaultScreen(tester, 'list');
      expect(
        find.bySemanticsLabel(RegExp('^${l10n.typeLogin}, ')),
        findsWidgets,
      );
      expect(find.bySemanticsLabel(l10n.itemListLabel), findsOneWidget);
      handle.dispose();
    });

    testWidgets(
      'a masked secret is announced as hidden and its dots are not read',
      (WidgetTester tester) async {
        final SemanticsHandle handle = tester.ensureSemantics();
        await pumpVaultScreen(tester, 'detail-masked', size: kCompact);
        expect(find.bySemanticsLabel(l10n.hiddenValue), findsWidgets);
        expect(find.bySemanticsLabel('••••••••'), findsNothing);
        expect(
          find.byTooltip(l10n.revealField(l10n.fieldPassword)),
          findsOneWidget,
        );
        handle.dispose();
      },
    );

    testWidgets('headings are marked as headers', (WidgetTester tester) async {
      final SemanticsHandle handle = tester.ensureSemantics();
      await pumpVaultScreen(tester, 'detail-masked', size: kCompact);
      expect(
        tester.getSemantics(find.bySemanticsLabel('CANARY Bank').first),
        isSemantics(isHeader: true),
      );
      handle.dispose();
    });
  });

  group('keyboard only', () {
    String focusLabel() => FocusManager.instance.primaryFocus?.debugLabel ?? '';

    Future<void> tab(WidgetTester tester) async {
      await tester.sendKeyEvent(LogicalKeyboardKey.tab);
      await tester.pumpAndSettle();
    }

    testWidgets(
      'search -> + menu -> filters -> list, then arrows and Enter open an item',
      (WidgetTester tester) async {
        final Harness h = await pumpVaultScreen(tester, 'list');
        await tester.tap(fieldLabelled(l10n.searchLabel));
        await tester.pumpAndSettle();
        var reachedList = false;
        for (var i = 0; i < 12 && !reachedList; i++) {
          await tab(tester);
          reachedList = focusLabel() == 'item list';
        }
        expect(reachedList, isTrue, reason: 'the list is one tab stop');
        await tester.sendKeyEvent(LogicalKeyboardKey.arrowDown);
        await tester.sendKeyEvent(LogicalKeyboardKey.enter);
        await tester.pumpAndSettle();
        expect(selectedId(h), isNotNull);
        expect(find.byTooltip(l10n.edit), findsOneWidget);
      },
    );

    testWidgets('Tab moves on from the list into the detail actions', (
      WidgetTester tester,
    ) async {
      await pumpVaultScreen(tester, 'list-selected');
      // Put focus on the list, then Tab until the Edit button has it.
      await tester.tap(find.text(sampleTitle(99)).first);
      await tester.pumpAndSettle();
      expect(focusLabel(), 'item list');
      var onEdit = false;
      for (var i = 0; i < 12 && !onEdit; i++) {
        await tab(tester);
        final BuildContext? ctx = FocusManager.instance.primaryFocus?.context;
        onEdit =
            ctx?.findAncestorWidgetOfExactType<Tooltip>()?.message == l10n.edit;
      }
      expect(onEdit, isTrue);
    });

    testWidgets('the edit form is reachable and operable by keyboard', (
      WidgetTester tester,
    ) async {
      await pumpVaultScreen(tester, 'edit-login-new');
      // The first field has focus order position one; typing goes into the focused field.
      await tester.tap(fieldLabelled(l10n.fieldTitle));
      await tester.pumpAndSettle();
      await tester.enterText(fieldLabelled(l10n.fieldTitle), 'CANARY kb');
      await tab(tester);
      await tester.enterText(
        fieldLabelled(l10n.fieldUsername),
        'kb@example.invalid',
      );
      expect(
        tester
            .widget<TextField>(fieldLabelled(l10n.fieldTitle))
            .controller!
            .text,
        'CANARY kb',
      );
    });
  });
}
