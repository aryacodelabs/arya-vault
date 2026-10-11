import 'package:arya_vault/src/api/dtos.dart';
import 'package:arya_vault/src/state/vault_query.dart';
import 'package:flutter/material.dart' hide Page;
import 'package:flutter_test/flutter_test.dart';

import '../helpers.dart';
import 'vault_helpers.dart';

Future<void> openFolders(WidgetTester tester) async {
  await tapOn(tester, find.text(l10n.manageFolders));
}

void main() {
  setUpAll(() => WidgetController.hitTestWarningShouldBeFatal = true);

  testWidgets('lists the folders', (WidgetTester tester) async {
    final SpyApi api = await unlockedVault();
    await pumpVault(tester, api: api);
    await openFolders(tester);
    expect(find.text(l10n.foldersTitle), findsWidgets);
    for (final String n in <String>['Work', 'Personal', 'Finance']) {
      expect(find.widgetWithText(ListTile, n), findsOneWidget);
    }
  });

  testWidgets('an empty vault says there are no folders', (
    WidgetTester tester,
  ) async {
    await pumpVault(tester, api: await unlockedVault(samples: false));
    await openFolders(tester);
    expect(find.text(l10n.noFolders), findsOneWidget);
  });

  testWidgets('create', (WidgetTester tester) async {
    final SpyApi api = await unlockedVault(samples: false);
    await pumpVault(tester, api: api);
    await openFolders(tester);
    await tapOn(tester, find.text(l10n.newFolder));
    await tester.enterText(fieldLabelled(l10n.folderNameLabel), 'Travel');
    await tapOn(tester, find.widgetWithText(FilledButton, l10n.create));
    expect((await api.listFolders()).map((Folder f) => f.name), <String>[
      'Travel',
    ]);
    expect(find.widgetWithText(ListTile, 'Travel'), findsOneWidget);
  });

  testWidgets('a blank name creates nothing', (WidgetTester tester) async {
    final SpyApi api = await unlockedVault(samples: false);
    await pumpVault(tester, api: api);
    await openFolders(tester);
    await tapOn(tester, find.text(l10n.newFolder));
    await tester.enterText(fieldLabelled(l10n.folderNameLabel), '   ');
    await tapOn(tester, find.widgetWithText(FilledButton, l10n.create));
    expect(await api.listFolders(), isEmpty);
  });

  testWidgets('rename', (WidgetTester tester) async {
    final SpyApi api = await unlockedVault(samples: false);
    final String id = await api.createFolder('Old', null);
    await pumpVault(tester, api: api);
    await openFolders(tester);
    await tapOn(tester, find.byTooltip(l10n.renameFolder('Old')));
    expect(
      tester
          .widget<TextField>(fieldLabelled(l10n.folderNameLabel))
          .controller!
          .text,
      'Old',
    );
    await tester.enterText(fieldLabelled(l10n.folderNameLabel), 'New');
    await tapOn(tester, find.widgetWithText(FilledButton, l10n.save));
    expect(
      (await api.listFolders()).single,
      isA<Folder>()
          .having((Folder f) => f.id, 'id', id)
          .having((Folder f) => f.name, 'name', 'New'),
    );
  });

  testWidgets('delete says its items move to "No folder", and does so', (
    WidgetTester tester,
  ) async {
    final SpyApi api = await unlockedVault(samples: false);
    final String folder = await api.createFolder('Doomed', null);
    final String item = await api.createItem(
      NewItem(
        type: ItemType.login,
        title: 'CANARY in folder',
        folderId: folder,
      ),
    );
    final Harness h = await pumpVault(tester, api: api);
    h.container
        .read(vaultQueryProvider.notifier)
        .setView(VaultView.folder(folder));
    await tester.pumpAndSettle();
    await openFolders(tester);
    await tapOn(tester, find.byTooltip(l10n.deleteFolder('Doomed')));
    expect(find.text(l10n.deleteFolderTitle('Doomed')), findsOneWidget);
    expect(find.text(l10n.deleteFolderBody), findsOneWidget);
    expect(l10n.deleteFolderBody, contains('No folder'));
    await tapOn(
      tester,
      find.widgetWithText(FilledButton, l10n.deleteFolderConfirm),
    );
    expect(await api.listFolders(), isEmpty);
    final ItemView v = await api.getItem(item);
    expect(v.folderId, isNull, reason: 'the item survives, without a folder');
    expect(v.title, 'CANARY in folder');
    expect(
      h.container.read(vaultQueryProvider).view,
      VaultView.all,
      reason: 'the deleted folder is no longer the view',
    );
  });

  testWidgets('cancelling the delete keeps the folder', (
    WidgetTester tester,
  ) async {
    final SpyApi api = await unlockedVault(samples: false);
    await api.createFolder('Keep', null);
    await pumpVault(tester, api: api);
    await openFolders(tester);
    await tapOn(tester, find.byTooltip(l10n.deleteFolder('Keep')));
    await tapOn(tester, find.text(l10n.actionCancel));
    expect(await api.listFolders(), hasLength(1));
  });

  testWidgets('the sidebar follows a new folder', (WidgetTester tester) async {
    final SpyApi api = await unlockedVault(samples: false);
    await pumpVault(tester, api: api);
    await openFolders(tester);
    await tapOn(tester, find.text(l10n.newFolder));
    await tester.enterText(fieldLabelled(l10n.folderNameLabel), 'Fresh');
    await tapOn(tester, find.widgetWithText(FilledButton, l10n.create));
    await tester.pageBack();
    await tester.pumpAndSettle();
    expect(
      find.descendant(of: find.byType(ListTile), matching: find.text('Fresh')),
      findsOneWidget,
    );
  });
}
