import 'package:arya_vault/src/api/dtos.dart';
import 'package:arya_vault/src/state/vault_query.dart';
import 'package:flutter/material.dart' hide Page;
import 'package:flutter_test/flutter_test.dart';

import '../helpers.dart';
import 'vault_helpers.dart';

Future<Harness> openItem(
  WidgetTester tester,
  SpyApi api,
  String id, {
  Size size = kDesktop,
}) async {
  final Harness h = await pumpVault(tester, api: api, size: size);
  h.container.read(vaultQueryProvider.notifier).select(id);
  await tester.pumpAndSettle();
  return h;
}

void main() {
  setUpAll(() => WidgetController.hitTestWarningShouldBeFatal = true);

  group('history', () {
    testWidgets('lists versions, newest first, secrets masked', (
      WidgetTester tester,
    ) async {
      final SpyApi api = await unlockedVault(items: 100);
      final ItemView v = await findItem(
        api,
        (ItemView v) => v.title == sampleTitle(35),
      );
      await openItem(tester, api, v.id);
      await tapOn(
        tester,
        find.byTooltip(l10n.history).at(1),
      ); // the password row (after the username)
      expect(find.text(l10n.historyTitle(sampleTitle(35))), findsOneWidget);
      expect(find.byType(Card), findsNWidgets(3));
      expect(find.text(l10n.currentVersion), findsWidgets);
      expect(
        find.byType(Chip),
        findsNWidgets(1),
        reason: 'only the current version is badged',
      );
      expect(find.text('••••••••'), findsNWidgets(3));
      expect(find.textContaining('CANARY-old-pw'), findsNothing);
      expect(
        find.text(l10n.restoreThisVersion),
        findsNWidgets(2),
        reason: 'not on the current one',
      );
    });

    testWidgets('an old value can be revealed; it hides itself and is wiped', (
      WidgetTester tester,
    ) async {
      final SpyApi api = await unlockedVault(items: 100);
      final ItemView v = await findItem(
        api,
        (ItemView v) => v.title == sampleTitle(35),
      );
      await openItem(tester, api, v.id);
      await tapOn(tester, find.byTooltip(l10n.history).at(1));
      await tapOn(
        tester,
        find.byTooltip(l10n.revealField(l10n.fieldPassword)).at(1),
      );
      expect(find.textContaining('CANARY-old-pw-35'), findsOneWidget);
      expect(api.count('revealVersion'), 1);
      await tester.pump(const Duration(seconds: 16));
      expect(find.textContaining('CANARY-old-pw-35'), findsNothing);
      expect(api.revealed.every((b) => b.every((int x) => x == 0)), isTrue);
    });

    testWidgets(
      'Restore this version writes the old value as the current one',
      (WidgetTester tester) async {
        final SpyApi api = await unlockedVault(items: 100);
        final ItemView v = await findItem(
          api,
          (ItemView v) => v.title == sampleTitle(35),
        );
        await openItem(tester, api, v.id);
        await tapOn(tester, find.byTooltip(l10n.history).at(1));
        final List<VersionInfo> before = await api.history(
          v.id,
          StdField.password,
        );
        await tapOn(tester, find.text(l10n.restoreThisVersion).first);
        expect(find.text(l10n.versionRestored), findsOneWidget);
        final List<VersionInfo> after = await api.history(
          v.id,
          StdField.password,
        );
        expect(after.length, before.length + 1);
        expect(
          String.fromCharCodes((await api.reveal(v.id, StdField.password))!),
          startsWith('CANARY-'),
        );
        expect(find.byType(Card), findsNWidgets(after.length));
      },
    );

    testWidgets('ordinary fields show their old values directly', (
      WidgetTester tester,
    ) async {
      final SpyApi api = await unlockedVault(samples: false);
      final String id = await api.createItem(
        const NewItem(
          type: ItemType.login,
          title: 'CANARY h',
          fields: <StdField, String>{
            StdField.username: 'first@example.invalid',
          },
        ),
      );
      await api.setField(id, StdField.username, 'second@example.invalid');
      await openItem(tester, api, id);
      await tapOn(tester, find.byTooltip(l10n.history).first);
      expect(find.text('first@example.invalid'), findsOneWidget);
      expect(find.text('second@example.invalid'), findsOneWidget);
    });

    testWidgets('field chips switch the field', (WidgetTester tester) async {
      final SpyApi api = await unlockedVault(items: 100);
      final ItemView v = await findItem(
        api,
        (ItemView v) => v.title == sampleTitle(35),
      );
      await openItem(tester, api, v.id);
      await tapOn(tester, find.byTooltip(l10n.history).at(1));
      await tapOn(tester, find.widgetWithText(ChoiceChip, l10n.fieldTitle));
      expect(find.text(l10n.noHistory), findsOneWidget);
    });

    testWidgets('a field never edited has no history', (
      WidgetTester tester,
    ) async {
      final SpyApi api = await unlockedVault(samples: false);
      final String id = await api.createItem(
        const NewItem(
          type: ItemType.login,
          title: 'CANARY fresh',
          fields: <StdField, String>{StdField.username: 'u'},
        ),
      );
      await openItem(tester, api, id);
      await tapOn(tester, find.byTooltip(l10n.history).first);
      expect(find.text(l10n.noHistory), findsOneWidget);
    });

    testWidgets('a locked error returns to the lock screen', (
      WidgetTester tester,
    ) async {
      final SpyApi api = await unlockedVault(items: 100);
      final ItemView v = await findItem(
        api,
        (ItemView v) => v.title == sampleTitle(35),
      );
      await openItem(tester, api, v.id);
      await api.lock();
      await tapOn(tester, find.byTooltip(l10n.history).at(1));
      expect(textOf(l10n.lockTitle), findsOneWidget);
    });
  });

  group('other versions and compare', () {
    Future<ItemView> conflicted(SpyApi api) => findItem(
      api,
      (ItemView v) => v.otherVersions && v.type == ItemType.login,
    );

    testWidgets(
      'the badge shows only for items with a concurrent version and opens the compare entry',
      (WidgetTester tester) async {
        final SpyApi api = await unlockedVault(items: 100);
        final ItemView plain = await findItem(
          api,
          (ItemView v) => v.title == sampleTitle(35),
        );
        final ItemView c = await conflicted(api);
        final Harness h = await openItem(tester, api, plain.id);
        expect(find.text(l10n.otherVersionsBadge), findsNothing);
        h.container.read(vaultQueryProvider.notifier).select(c.id);
        await tester.pumpAndSettle();
        expect(find.text(l10n.otherVersionsBadge), findsOneWidget);
        await tapOn(tester, find.text(l10n.otherVersionsBadge));
        expect(find.text(l10n.historyTitle(c.title)), findsOneWidget);
        expect(find.text(l10n.concurrentVersion), findsOneWidget);
        expect(find.widgetWithText(FilledButton, l10n.compare), findsOneWidget);
      },
    );

    testWidgets(
      'compare shows current and other side by side; restoring makes the other current',
      (WidgetTester tester) async {
        final SpyApi api = await unlockedVault(items: 100);
        final ItemView c = await conflicted(api);
        await openItem(tester, api, c.id);
        await tapOn(tester, find.text(l10n.otherVersionsBadge));
        await tapOn(tester, find.widgetWithText(FilledButton, l10n.compare));
        expect(
          find.text(l10n.compareTitle(l10n.fieldPassword)),
          findsOneWidget,
        );
        expect(find.text(l10n.currentVersion), findsOneWidget);
        expect(find.text(l10n.otherVersion), findsOneWidget);
        expect(
          find.text(l10n.keepAsSeparateNote),
          findsNothing,
          reason: 'only for note bodies',
        );
        // Both values reveal independently and stay masked until asked.
        expect(find.text('••••••••'), findsNWidgets(2));
        await tapOn(
          tester,
          find.byTooltip(l10n.revealField(l10n.fieldPassword)).last,
        );
        expect(find.textContaining('CANARY-other-device-pw'), findsOneWidget);
        await tapOn(tester, find.text(l10n.restoreThisVersion));
        expect(find.text(l10n.versionRestored), findsOneWidget);
        expect(
          String.fromCharCodes((await api.reveal(c.id, StdField.password))!),
          startsWith('CANARY-other-device-pw'),
        );
        expect(
          (await api.getItem(c.id)).otherVersions,
          isFalse,
          reason: 'the conflict is resolved',
        );
      },
    );

    testWidgets('for a note body: Keep as separate note creates a new note', (
      WidgetTester tester,
    ) async {
      final SpyApi api = await unlockedVault(samples: false);
      final String folder = await api.createFolder('Journal', null);
      final String id = await api.createItem(
        NewItem(
          type: ItemType.note,
          title: 'CANARY Diary',
          fields: const <StdField, String>{StdField.body: 'my version'},
          folderId: folder,
        ),
      );
      api.seedConcurrentVersion(id, StdField.body, 'their version');
      await openItem(tester, api, id);
      await tapOn(tester, find.text(l10n.otherVersionsBadge));
      await tapOn(tester, find.widgetWithText(FilledButton, l10n.compare));
      expect(find.text(l10n.keepAsSeparateNote), findsOneWidget);
      await tapOn(tester, find.text(l10n.keepAsSeparateNote));
      expect(find.text(l10n.keptAsNote), findsOneWidget);
      expect(api.count('createItem'), 2);
      final ItemView copy = await findItem(api, (ItemView v) => v.id != id);
      expect(copy.type, ItemType.note);
      expect(copy.title, l10n.otherVersionNoteTitle('CANARY Diary'));
      expect(copy.folderId, folder);
      expect(
        String.fromCharCodes((await api.reveal(copy.id, StdField.body))!),
        'their version',
      );
      // The original is untouched.
      expect(
        String.fromCharCodes((await api.reveal(id, StdField.body))!),
        'my version',
      );
      expect(
        api.revealed.every((b) => b.every((int x) => x == 0)),
        isFalse,
        reason: 'the last reveal is not wiped by this test',
      );
    });

    testWidgets('compact: the two versions stack', (WidgetTester tester) async {
      final SpyApi api = await unlockedVault(items: 100);
      final ItemView c = await conflicted(api);
      await openItem(tester, api, c.id, size: kCompact);
      await tapOn(tester, find.text(l10n.otherVersionsBadge));
      await tapOn(tester, find.widgetWithText(FilledButton, l10n.compare));
      final double a = tester.getTopLeft(find.text(l10n.currentVersion)).dy;
      final double b = tester.getTopLeft(find.text(l10n.otherVersion)).dy;
      expect(b, greaterThan(a));
    });

    testWidgets(
      'revealed versions are hidden when leaving the compare screen',
      (WidgetTester tester) async {
        final SpyApi api = await unlockedVault(items: 100);
        final ItemView c = await conflicted(api);
        await openItem(tester, api, c.id);
        await tapOn(tester, find.text(l10n.otherVersionsBadge));
        await tapOn(tester, find.widgetWithText(FilledButton, l10n.compare));
        await tapOn(
          tester,
          find.byTooltip(l10n.revealField(l10n.fieldPassword)).first,
        );
        await tester.pageBack();
        await tester.pumpAndSettle();
        await tester.pageBack();
        await tester.pumpAndSettle();
        expect(api.revealed.every((b) => b.every((int x) => x == 0)), isTrue);
      },
    );
  });
}
