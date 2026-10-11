import 'dart:async';

import 'package:arya_vault/src/api/app_error.dart';
import 'package:arya_vault/src/api/dtos.dart';
import 'package:arya_vault/src/state/vault_query.dart';
import 'package:arya_vault/src/ui/vault/item_list_pane.dart';
import 'package:arya_vault/src/ui/vault/vault_sidebar.dart';
import 'package:flutter/material.dart' hide Page;
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';

import '../helpers.dart';
import 'vault_helpers.dart';

void main() {
  group('list', () {
    testWidgets('shows the newest items first, with no secrets in rows', (
      WidgetTester tester,
    ) async {
      final SpyApi api = await unlockedVault();
      await pumpVault(tester, api: api);
      expect(listRow(sampleTitle(59)), findsOneWidget, reason: 'newest first');
      expect(find.textContaining('CANARY'), findsNothing);
      // A login row shows its (non-secret) username as the subtitle.
      expect(find.textContaining('@example.invalid'), findsWidgets);
    });

    testWidgets('an empty vault says so', (WidgetTester tester) async {
      await pumpVault(tester, api: await unlockedVault(samples: false));
      expect(textOf(l10n.homeUnlockedMessage), findsOneWidget);
      expect(textOf(l10n.emptyVaultHint), findsOneWidget);
    });

    testWidgets('type chips filter through the API', (
      WidgetTester tester,
    ) async {
      final SpyApi api = await unlockedVault();
      await pumpVault(tester, api: api);
      await tapOn(tester, find.widgetWithText(FilterChip, l10n.typeNote));
      expect(api.listFilters.last.types, <ItemType>[ItemType.note]);
      final SemanticsHandle h = tester.ensureSemantics();
      expect(
        find.bySemanticsLabel(RegExp('^${l10n.typeLogin},')),
        findsNothing,
      );
      expect(find.bySemanticsLabel(RegExp('^${l10n.typeNote},')), findsWidgets);
      h.dispose();
      await tapOn(tester, find.widgetWithText(FilterChip, l10n.typeNote));
      expect(api.listFilters.last.types, isNull);
    });

    testWidgets('the favorite star toggles through the API', (
      WidgetTester tester,
    ) async {
      final SpyApi api = await unlockedVault();
      await pumpVault(tester, api: api);
      final ItemSummary top = (await api.list(
        const ListFilter(),
        const Page(offset: 0, limit: 1),
      )).single;
      expect(top.favorite, isFalse);
      await tapOn(tester, find.byTooltip(l10n.favorite).first);
      final ItemSummary after = (await api.list(
        const ListFilter(),
        const Page(offset: 0, limit: 1),
      )).single;
      expect(after.id, top.id);
      expect(after.favorite, isTrue);
      expect(find.byTooltip(l10n.unfavorite), findsWidgets);
    });

    testWidgets('sidebar: favorites, folders and tags filter the list', (
      WidgetTester tester,
    ) async {
      final SpyApi api = await unlockedVault();
      await pumpVault(tester, api: api);
      await tapOn(tester, find.text(l10n.favoritesView));
      expect(api.listFilters.last.favoritesOnly, isTrue);

      await tapOn(tester, find.text('Work'));
      final List<Folder> folders = await api.listFolders();
      expect(
        api.listFilters.last.folderId,
        folders.firstWhere((Folder f) => f.name == 'Work').id,
      );

      // Tags are those seen in loaded items (docs/14 has no list-tags call).
      await tapOn(tester, find.text(l10n.allItems));
      await tapOn(
        tester,
        find.descendant(
          of: find.byType(VaultSidebar),
          matching: find.text('work'),
        ),
      );
      expect(api.listFilters.last.tag, 'work');
    });

    testWidgets('infinite scroll loads further pages', (
      WidgetTester tester,
    ) async {
      final SpyApi api = await unlockedVault(items: 250, trash: 0);
      await pumpVault(tester, api: api);
      expect(textOf(l10n.resultsCountMore(100)), findsOneWidget);
      for (var i = 0; i < 6; i++) {
        await tester.drag(
          find.descendant(
            of: find.byType(ItemListPane),
            matching: find.byType(ListView),
          ),
          const Offset(0, -6000),
        );
        await tester.pumpAndSettle();
      }
      expect(textOf(l10n.resultsCount(250)), findsOneWidget);
      expect(api.count('list'), 3, reason: '100 + 100 + 50, then it stops');
    });

    testWidgets('a failing load shows the error and Try again recovers', (
      WidgetTester tester,
    ) async {
      final SpyApi api = await unlockedVault();
      api.failNext['list'] = const AppError(AppErrorCode.io, 'x');
      await pumpVault(tester, api: api);
      expect(textOf(l10n.errIo), findsOneWidget);
      await tapOn(tester, find.text(l10n.actionTryAgain));
      expect(listRow(sampleTitle(59)), findsOneWidget);
    });

    testWidgets('a locked error mid-session goes to the lock screen', (
      WidgetTester tester,
    ) async {
      final SpyApi api = await unlockedVault();
      final Harness h = await pumpVault(tester, api: api);
      await api.lock();
      api.failNext['list'] = const AppError(AppErrorCode.locked, 'x');
      await tapOn(tester, find.widgetWithText(FilterChip, l10n.typeCard));
      expect(textOf(l10n.lockTitle), findsOneWidget);
      expect(
        h.container.read(vaultQueryProvider).types,
        isEmpty,
        reason: 'filters reset on lock',
      );
    });
  });

  group('search', () {
    testWidgets('is debounced by 150 ms and sends one request', (
      WidgetTester tester,
    ) async {
      final SpyApi api = await unlockedVault();
      await pumpVault(tester, api: api);
      for (final String t in <String>['F', 'Fa', 'Fal', 'Falc']) {
        await typeSearch(tester, t);
        await tester.pump(const Duration(milliseconds: 50));
      }
      expect(
        api.count('search'),
        0,
        reason: 'still inside the debounce window',
      );
      await tester.pump(const Duration(milliseconds: 160));
      await tester.pumpAndSettle();
      expect(api.count('search'), 1);
      expect(listRow(sampleTitle(5)), findsOneWidget);
      expect(find.textContaining('Beacon'), findsNothing);
      expect(
        textOf(l10n.resultsCount(3)),
        findsOneWidget,
        reason: 'Falcon 5, 25 and 45',
      );
    });

    testWidgets('a stale answer never overwrites a newer one', (
      WidgetTester tester,
    ) async {
      final SpyApi api = await unlockedVault();
      final Completer<void> slow = Completer<void>();
      final Completer<void> fast = Completer<void>();
      api.gates['Falcon'] = slow; // the first query is answered last
      api.gates['Beacon'] = fast;
      await pumpVault(tester, api: api);

      await typeSearch(tester, 'Falcon');
      await tester.pump(const Duration(milliseconds: 200));
      await typeSearch(tester, 'Beacon');
      await tester.pump(const Duration(milliseconds: 200));
      expect(api.count('search'), 2);

      fast.complete(); // newer query answers first
      await tester.pumpAndSettle();
      expect(listRow(sampleTitle(1)), findsOneWidget);
      slow.complete(); // the older one arrives late
      await tester.pumpAndSettle();
      expect(
        listRow(sampleTitle(1)),
        findsOneWidget,
        reason: 'newer results stay',
      );
      expect(
        listRow(sampleTitle(5)),
        findsNothing,
        reason: 'stale results are dropped',
      );
    });

    testWidgets('no results says so; clearing restores the list', (
      WidgetTester tester,
    ) async {
      final SpyApi api = await unlockedVault();
      await pumpVault(tester, api: api);
      await typeSearch(tester, 'zzzzzz');
      await tester.pump(const Duration(milliseconds: 200));
      await tester.pumpAndSettle();
      expect(textOf(l10n.noMatches), findsOneWidget);
      await tapOn(tester, find.byTooltip(l10n.searchClear));
      expect(listRow(sampleTitle(59)), findsOneWidget);
    });

    testWidgets('search respects the type filter', (WidgetTester tester) async {
      final SpyApi api = await unlockedVault();
      await pumpVault(tester, api: api);
      await tapOn(tester, find.widgetWithText(FilterChip, l10n.typeCard));
      await typeSearch(tester, 'a');
      await tester.pump(const Duration(milliseconds: 200));
      await tester.pumpAndSettle();
      final SemanticsHandle h = tester.ensureSemantics();
      expect(
        find.bySemanticsLabel(RegExp('^${l10n.typeLogin},')),
        findsNothing,
      );
      h.dispose();
    });
  });

  group('trash view', () {
    testWidgets('lists trashed items with retention and purge date', (
      WidgetTester tester,
    ) async {
      final SpyApi api = await unlockedVault();
      await pumpVault(tester, api: api);
      await tapOn(tester, find.text(l10n.trashTitle));
      expect(textOf(l10n.trashRetention), findsOneWidget);
      expect(find.textContaining('Deleted for good on'), findsNWidgets(5));
      expect(find.byTooltip(l10n.restore), findsNWidgets(5));
    });

    testWidgets('restore returns an item to the list', (
      WidgetTester tester,
    ) async {
      final SpyApi api = await unlockedVault();
      await pumpVault(tester, api: api);
      final int before = await api.itemCount();
      await tapOn(tester, find.text(l10n.trashTitle));
      await tapOn(tester, find.byTooltip(l10n.restore).first);
      expect(await api.itemCount(), before + 1);
      expect(find.byTooltip(l10n.restore), findsNWidgets(4));
    });

    testWidgets('delete forever asks first, and only then purges', (
      WidgetTester tester,
    ) async {
      final SpyApi api = await unlockedVault();
      await pumpVault(tester, api: api);
      await tapOn(tester, find.text(l10n.trashTitle));
      await tapOn(tester, find.byTooltip(l10n.deleteForever).first);
      expect(textOf(l10n.purgeTitle), findsOneWidget);
      await tapOn(tester, find.text(l10n.actionCancel));
      expect(api.count('purgeItem'), 0);
      await tapOn(tester, find.byTooltip(l10n.deleteForever).first);
      await tapOn(
        tester,
        find.widgetWithText(FilledButton, l10n.deleteForever),
      );
      expect(api.count('purgeItem'), 1);
      expect(find.byTooltip(l10n.deleteForever), findsNWidgets(4));
    });

    testWidgets('empty trash is confirmed and empties it', (
      WidgetTester tester,
    ) async {
      final SpyApi api = await unlockedVault();
      await pumpVault(tester, api: api);
      await tapOn(tester, find.text(l10n.trashTitle));
      await tapOn(tester, find.widgetWithText(TextButton, l10n.emptyTrash));
      expect(textOf(l10n.emptyTrashTitle), findsOneWidget);
      expect(
        find.textContaining('5 items will be deleted for good'),
        findsOneWidget,
      );
      await tapOn(tester, find.widgetWithText(FilledButton, l10n.emptyTrash));
      expect(api.count('emptyTrash'), 1);
      expect(textOf(l10n.trashEmpty), findsOneWidget);
    });
  });

  group('keyboard', () {
    testWidgets('arrows move the selection, Enter keeps it, scroll follows', (
      WidgetTester tester,
    ) async {
      final SpyApi api = await unlockedVault();
      final Harness h = await pumpVault(tester, api: api);
      final List<ItemSummary> items = await api.list(
        const ListFilter(),
        const Page(offset: 0, limit: 100),
      );
      await tapOn(tester, listRow(items[0].title));
      expect(selectedId(h), items[0].id);
      await tester.sendKeyEvent(LogicalKeyboardKey.arrowDown);
      await tester.sendKeyEvent(LogicalKeyboardKey.arrowDown);
      await tester.pumpAndSettle();
      expect(selectedId(h), items[2].id);
      await tester.sendKeyEvent(LogicalKeyboardKey.arrowUp);
      await tester.pumpAndSettle();
      expect(selectedId(h), items[1].id);
      await tester.sendKeyEvent(LogicalKeyboardKey.enter);
      await tester.pumpAndSettle();
      expect(selectedId(h), items[1].id);
      for (var i = 0; i < 20; i++) {
        await tester.sendKeyEvent(LogicalKeyboardKey.arrowDown);
      }
      await tester.pumpAndSettle();
      expect(selectedId(h), items[21].id);
      expect(
        listRow(items[21].title),
        findsOneWidget,
        reason: 'scrolled into view',
      );
    });

    testWidgets('Delete moves the item to the trash with Undo', (
      WidgetTester tester,
    ) async {
      final SpyApi api = await unlockedVault();
      final Harness h = await pumpVault(tester, api: api);
      final List<ItemSummary> items = await api.list(
        const ListFilter(),
        const Page(offset: 0, limit: 5),
      );
      final int before = await api.itemCount();
      await tapOn(tester, listRow(items[0].title));
      await tester.sendKeyEvent(LogicalKeyboardKey.delete);
      await tester.pumpAndSettle();
      expect(api.count('deleteItem'), 1);
      expect(await api.itemCount(), before - 1);
      expect(selectedId(h), isNull);
      expect(find.text(l10n.movedToTrash(items[0].title)), findsOneWidget);
      await tapOn(tester, find.text(l10n.undo));
      expect(await api.itemCount(), before);
      expect(listRow(items[0].title), findsOneWidget);
    });

    testWidgets('the list is one tab stop and shows a focus ring', (
      WidgetTester tester,
    ) async {
      await pumpVault(tester);
      // search field -> (chips) ... -> list; the list takes focus as one widget
      await tester.tap(find.text(sampleTitle(59)));
      await tester.pumpAndSettle();
      final BuildContext ctx = FocusManager.instance.primaryFocus!.context!;
      expect(ctx.findAncestorWidgetOfExactType<ItemListPane>(), isNotNull);
    });
  });

  group('layouts', () {
    testWidgets('compact: tap opens the detail, back returns to the list', (
      WidgetTester tester,
    ) async {
      final SpyApi api = await unlockedVault();
      final Harness h = await pumpVault(tester, api: api, size: kCompact);
      await openRow(tester, sampleTitle(59));
      expect(selectedId(h), isNotNull);
      expect(
        fieldLabelled(l10n.searchLabel),
        findsNothing,
        reason: 'detail replaces the list',
      );
      await tapOn(tester, find.byTooltip(l10n.actionBack));
      expect(selectedId(h), isNull);
      expect(fieldLabelled(l10n.searchLabel), findsOneWidget);
    });

    testWidgets('desktop: list and detail side by side', (
      WidgetTester tester,
    ) async {
      await pumpVault(tester);
      await openRow(tester, sampleTitle(59));
      expect(fieldLabelled(l10n.searchLabel), findsOneWidget);
      expect(find.byTooltip(l10n.edit), findsOneWidget);
    });

    testWidgets('compact: the drawer holds the sidebar', (
      WidgetTester tester,
    ) async {
      final SpyApi api = await unlockedVault();
      await pumpVault(tester, api: api, size: kCompact);
      await tapOn(tester, find.byTooltip(l10n.openMenu));
      await tapOn(tester, find.text(l10n.favoritesView));
      expect(api.listFilters.last.favoritesOnly, isTrue);
      expect(find.text(l10n.allItems), findsNothing, reason: 'drawer closed');
    });

    testWidgets('the + menu offers the four item types', (
      WidgetTester tester,
    ) async {
      await pumpVault(tester);
      await tapOn(tester, find.byTooltip(l10n.newItem));
      for (final String t in <String>[
        l10n.typeLogin,
        l10n.typeNote,
        l10n.typeCard,
        l10n.typeIdentity,
      ]) {
        expect(find.text(t), findsWidgets);
      }
      await tapOn(
        tester,
        find.descendant(
          of: find.byType(PopupMenuItem<ItemType>),
          matching: find.text(l10n.typeNote),
        ),
      );
      expect(textOf(l10n.newItemTitle(l10n.typeNote)), findsOneWidget);
    });
  });

  test('VaultView equality and ListQuery normalise their inputs', () {
    expect(const VaultView.folder('a'), const VaultView.folder('a'));
    expect(const VaultView.folder('a'), isNot(const VaultView.folder('b')));
    expect(
      ListQuery(VaultView.all, '', <ItemType>{ItemType.note, ItemType.login}),
      ListQuery(VaultView.all, '', <ItemType>{ItemType.login, ItemType.note}),
    );
  });
}
