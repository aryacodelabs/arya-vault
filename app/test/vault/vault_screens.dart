import 'package:arya_vault/src/api/dtos.dart';
import 'package:arya_vault/src/state/vault_query.dart';
import 'package:flutter/material.dart' hide Page;
import 'package:flutter_test/flutter_test.dart';

import '../helpers.dart';
import 'vault_helpers.dart';

/// Every distinct B02 screen, reachable in a known state (accessibility and
/// golden tests share these).
const List<String> kVaultScreens = <String>[
  'list',
  'list-selected',
  'detail-masked',
  'detail-revealed',
  'note-revealed',
  'edit-login-new',
  'edit-login-existing',
  'edit-note',
  'trash',
  'history',
  'compare',
  'folders',
  'drawer',
];

Future<Harness> pumpVaultScreen(
  WidgetTester tester,
  String name, {
  Size size = kDesktop,
  double textScale = 1,
  ThemeMode themeMode = ThemeMode.light,
}) async {
  final SpyApi api = await unlockedVault(items: 100);
  Future<Harness> boot() => pumpVault(
    tester,
    api: api,
    size: size,
    textScale: textScale,
    themeMode: themeMode,
  );
  Future<void> select(Harness h, String id) async {
    h.container.read(vaultQueryProvider.notifier).select(id);
    await tester.pumpAndSettle();
  }

  switch (name) {
    case 'list':
      return boot();
    case 'list-selected':
      final Harness h = await boot();
      await select(
        h,
        (await findItem(api, (ItemView v) => v.title == sampleTitle(99))).id,
      );
      return h;
    case 'detail-masked':
    case 'detail-revealed':
      final String id = await seedRichLogin(api);
      final Harness h = await boot();
      await select(h, id);
      if (name == 'detail-revealed') {
        await tapOn(
          tester,
          find.byTooltip(l10n.revealField(l10n.fieldPassword)),
        );
      }
      return h;
    case 'note-revealed':
      final String id = await api.createItem(
        const NewItem(
          type: ItemType.note,
          title: 'CANARY Diary',
          fields: <StdField, String>{
            StdField.body: '# Day one\n\nSome **bold** words and a [link](https://example.invalid).\n\n- first\n- second\n\n> a quote',
          },
        ),
      );
      final Harness h = await boot();
      await select(h, id);
      await tapOn(tester, find.byTooltip(l10n.revealField(l10n.fieldBody)));
      return h;
    case 'edit-login-new':
      final Harness h = await boot();
      await tapOn(tester, find.byTooltip(l10n.newItem));
      await tapOn(
        tester,
        find.descendant(
          of: find.byType(PopupMenuItem<ItemType>),
          matching: find.text(l10n.typeLogin),
        ),
      );
      return h;
    case 'edit-login-existing':
      final String id = await seedRichLogin(api);
      final Harness h = await boot();
      await select(h, id);
      await tapOn(tester, find.byTooltip(l10n.edit));
      return h;
    case 'edit-note':
      final String id = await api.createItem(
        const NewItem(
          type: ItemType.note,
          title: 'CANARY Diary',
          fields: <StdField, String>{StdField.body: 'line one\nline two'},
        ),
      );
      final Harness h = await boot();
      await select(h, id);
      await tapOn(tester, find.byTooltip(l10n.edit));
      return h;
    case 'trash':
      final Harness h = await boot();
      h.container.read(vaultQueryProvider.notifier).setView(VaultView.trash);
      await tester.pumpAndSettle();
      return h;
    case 'history':
      final ItemView v = await findItem(
        api,
        (ItemView v) => v.title == sampleTitle(35),
      );
      final Harness h = await boot();
      await select(h, v.id);
      await tapOn(tester, find.byTooltip(l10n.history).at(1));
      return h;
    case 'compare':
      final ItemView v = await findItem(
        api,
        (ItemView v) => v.otherVersions && v.type == ItemType.login,
      );
      final Harness h = await boot();
      await select(h, v.id);
      await tapOn(tester, find.text(l10n.otherVersionsBadge));
      await tapOn(tester, find.widgetWithText(FilledButton, l10n.compare));
      return h;
    case 'folders':
      final Harness h = await boot();
      if (size.width < 900) await tapOn(tester, find.byTooltip(l10n.openMenu));
      await tapOn(tester, find.text(l10n.manageFolders));
      return h;
    case 'drawer':
      final Harness h = await boot();
      if (size.width < 900) await tapOn(tester, find.byTooltip(l10n.openMenu));
      return h;
  }
  throw ArgumentError.value(name, 'name');
}
