import 'dart:typed_data';

import 'package:arya_vault/src/api/dtos.dart';
import 'package:arya_vault/src/state/item_list.dart';
import 'package:arya_vault/src/state/reveal.dart';
import 'package:arya_vault/src/state/vault_actions.dart';
import 'package:arya_vault/src/state/vault_query.dart';
import 'package:flutter/material.dart' hide Page;
import 'package:flutter_test/flutter_test.dart';

import '../helpers.dart';
import 'vault_helpers.dart';

/// B02 item 9: after a lock, no revealed secret and no vault data survives in
/// providers, and every screen is gone.
void main() {
  setUpAll(() => WidgetController.hitTestWarningShouldBeFatal = true);

  testWidgets(
    'a lock clears reveal, selection, search, filters and cached lists',
    (WidgetTester tester) async {
      final SpyApi api = await unlockedVault();
      final Harness h = await pumpVault(tester, api: api);
      final ItemView login = await findItem(
        api,
        (ItemView v) =>
            v.type == ItemType.login &&
            v.secretFieldsPresent.contains(StdField.password),
      );

      // Build up as much state as a session has: filter, search, select, reveal.
      await tapOn(tester, find.widgetWithText(FilterChip, l10n.typeLogin));
      await typeSearch(tester, 'Echo');
      await tester.pump(const Duration(milliseconds: 200));
      await tester.pumpAndSettle();
      h.container.read(vaultQueryProvider.notifier).select(login.id);
      await tester.pumpAndSettle();
      await tapOn(tester, find.byTooltip(l10n.revealField(l10n.fieldPassword)));
      expect(h.container.read(revealProvider), isNotEmpty);
      expect(h.container.read(itemListProvider).value!.items, isNotEmpty);
      final List<Uint8List> live = List<Uint8List>.of(api.revealed);
      expect(live, isNotEmpty);

      await h.container.read(vaultActionsProvider).lock();
      await tester.pumpAndSettle();

      expect(textOf(l10n.lockTitle), findsOneWidget);
      expect(h.container.read(revealProvider), isEmpty);
      expect(
        live.every((Uint8List b) => b.every((int x) => x == 0)),
        isTrue,
        reason: 'revealed bytes wiped',
      );
      final VaultQueryState q = h.container.read(vaultQueryProvider);
      expect(q.selectedId, isNull);
      expect(q.searchInput, isEmpty);
      expect(q.search, isEmpty);
      expect(q.types, isEmpty);
      expect(q.view, VaultView.all);
      // The cached list no longer holds vault rows.
      expect(
        h.container.read(itemListProvider).value?.items ?? <ItemSummary>[],
        isEmpty,
      );
      // Nothing from the vault remains on screen.
      expect(find.textContaining('CANARY'), findsNothing);
      expect(find.byType(ListView), findsNothing);
    },
  );

  testWidgets('a lock event (not a click) does the same', (
    WidgetTester tester,
  ) async {
    final SpyApi api = await unlockedVault();
    final Harness h = await pumpVault(tester, api: api, size: kCompact);
    final ItemSummary first = (await api.list(
      const ListFilter(),
      const Page(offset: 0, limit: 1),
    )).single;
    h.container.read(vaultQueryProvider.notifier).select(first.id);
    await tester.pumpAndSettle();
    await tapOn(
      tester,
      find.byTooltip(l10n.revealField(l10n.fieldPassword)).first,
    );
    await h.container
        .read(vaultActionsProvider)
        .lock(); // what auto-lock / sleep calls
    await tester.pumpAndSettle();
    expect(textOf(l10n.lockTitle), findsOneWidget);
    expect(h.container.read(revealProvider), isEmpty);
    expect(
      api.revealed.every((Uint8List b) => b.every((int x) => x == 0)),
      isTrue,
    );
  });

  testWidgets('no secret appears in provider state at any time', (
    WidgetTester tester,
  ) async {
    final SpyApi api = await unlockedVault(samples: false);
    final String id = await api.createItem(
      const NewItem(
        type: ItemType.login,
        title: 'CANARY t',
        fields: <StdField, String>{
          StdField.password: 'CANARY-in-provider-check',
        },
      ),
    );
    final Harness h = await pumpVault(tester, api: api);
    h.container.read(vaultQueryProvider.notifier).select(id);
    await tester.pumpAndSettle();
    await tapOn(tester, find.byTooltip(l10n.revealField(l10n.fieldPassword)));
    // The reveal provider's state is only the set of keys.
    expect(
      h.container.read(revealProvider).toString(),
      isNot(contains('CANARY-in-provider-check')),
    );
    expect(
      h.container.read(itemListProvider).toString(),
      isNot(contains('CANARY-in-provider-check')),
    );
    expect(
      h.container.read(vaultQueryProvider).toString(),
      isNot(contains('CANARY-in-provider-check')),
    );
  });
}
