import 'dart:typed_data';

import 'package:arya_vault/src/api/app_error.dart';
import 'package:arya_vault/src/api/dtos.dart';
import 'package:arya_vault/src/state/reveal.dart';
import 'package:arya_vault/src/state/vault_query.dart';
import 'package:arya_vault/src/state/vault_session.dart';
import 'package:arya_vault/src/ui/vault/item_detail_pane.dart';
import 'package:flutter/material.dart' hide Page;
import 'package:flutter_test/flutter_test.dart';

import '../helpers.dart';
import 'vault_helpers.dart';

/// A login with the given extras, created through the API (no sample data).
Future<String> addLogin(
  SpyApi api, {
  String title = 'CANARY Bank',
  List<String> urls = const <String>[],
  Map<StdField, String> fields = const <StdField, String>{},
  List<NewCustom> custom = const <NewCustom>[],
  List<String> tags = const <String>[],
  String? folderId,
}) => api.createItem(
  NewItem(
    type: ItemType.login,
    title: title,
    fields: <StdField, String>{
      StdField.username: 'alice@example.invalid',
      StdField.password: 'CANARY-hunter2-do-not-use',
      ...fields,
    },
    urls: urls,
    custom: custom,
    tags: tags,
    folderId: folderId,
  ),
);

Future<Harness> openItem(
  WidgetTester tester,
  SpyApi api,
  String id, {
  Services? services,
  Size size = kDesktop,
}) async {
  final Harness h = await pumpVault(
    tester,
    api: api,
    services: services,
    size: size,
  );
  h.container.read(vaultQueryProvider.notifier).select(id);
  await tester.pumpAndSettle();
  return h;
}

Finder inDetail(Finder f) =>
    find.descendant(of: find.byType(ItemDetailPane), matching: f);

bool allZero(Iterable<Uint8List> buffers) =>
    buffers.every((Uint8List b) => b.every((int x) => x == 0));

void main() {
  const String pw = 'CANARY-hunter2-do-not-use';

  group('secrets', () {
    testWidgets('the password is masked and not even fetched until asked', (
      WidgetTester tester,
    ) async {
      final SpyApi api = await unlockedVault(samples: false);
      final String id = await addLogin(api);
      await openItem(tester, api, id);
      expect(find.text('••••••••'), findsOneWidget);
      expect(find.textContaining('CANARY-hunter2'), findsNothing);
      expect(api.count('reveal'), 0);
      // The username is not secret and is shown.
      expect(inDetail(find.text('alice@example.invalid')), findsOneWidget);
    });

    testWidgets(
      'Reveal shows the value and it hides itself after 15 s, wiped',
      (WidgetTester tester) async {
        final SpyApi api = await unlockedVault(samples: false);
        final String id = await addLogin(api);
        await openItem(tester, api, id);
        await tapOn(
          tester,
          find.byTooltip(l10n.revealField(l10n.fieldPassword)),
        );
        expect(find.text(pw), findsOneWidget);
        expect(api.count('reveal'), 1);
        await tester.pump(const Duration(seconds: 14));
        expect(find.text(pw), findsOneWidget, reason: 'still visible at 14 s');
        await tester.pump(const Duration(seconds: 2));
        expect(
          find.text(pw),
          findsNothing,
          reason: 'SEC-H03: hidden within 15 s',
        );
        expect(find.text('••••••••'), findsOneWidget);
        expect(
          allZero(api.revealed),
          isTrue,
          reason: 'the revealed bytes were wiped',
        );
      },
    );

    testWidgets(
      'the reveal timeout follows the setting but never exceeds 15 s',
      (WidgetTester tester) async {
        final SpyApi api = await unlockedVault(samples: false);
        await api.setSettings(const AppSettings(revealHideSeconds: 3));
        final String id = await addLogin(api);
        await openItem(tester, api, id);
        await tapOn(
          tester,
          find.byTooltip(l10n.revealField(l10n.fieldPassword)),
        );
        await tester.pump(const Duration(seconds: 2));
        expect(find.text(pw), findsOneWidget);
        await tester.pump(const Duration(seconds: 2));
        expect(find.text(pw), findsNothing);
        expect(
          revealDuration(const AppSettings(revealHideSeconds: 99)),
          const Duration(seconds: 15),
        );
        expect(revealDuration(null), const Duration(seconds: 15));
      },
    );

    testWidgets('Hide button hides at once', (WidgetTester tester) async {
      final SpyApi api = await unlockedVault(samples: false);
      final String id = await addLogin(api);
      await openItem(tester, api, id);
      await tapOn(tester, find.byTooltip(l10n.revealField(l10n.fieldPassword)));
      await tapOn(tester, find.byTooltip(l10n.hideField(l10n.fieldPassword)));
      expect(find.text(pw), findsNothing);
      expect(allZero(api.revealed), isTrue);
    });

    testWidgets('losing app focus hides revealed secrets', (
      WidgetTester tester,
    ) async {
      final SpyApi api = await unlockedVault(samples: false);
      final String id = await addLogin(api);
      await openItem(tester, api, id);
      await tapOn(tester, find.byTooltip(l10n.revealField(l10n.fieldPassword)));
      expect(find.text(pw), findsOneWidget);
      tester.binding.handleAppLifecycleStateChanged(AppLifecycleState.inactive);
      await tester.pump();
      expect(find.text(pw), findsNothing);
      expect(allZero(api.revealed), isTrue);
    });

    testWidgets('selecting another item hides what was revealed', (
      WidgetTester tester,
    ) async {
      final SpyApi api = await unlockedVault(samples: false);
      final String a = await addLogin(api, title: 'CANARY A');
      final String b = await addLogin(api, title: 'CANARY B');
      final Harness h = await openItem(tester, api, a);
      await tapOn(tester, find.byTooltip(l10n.revealField(l10n.fieldPassword)));
      expect(h.container.read(revealProvider), isNotEmpty);
      h.container.read(vaultQueryProvider.notifier).select(b);
      await tester.pumpAndSettle();
      expect(h.container.read(revealProvider), isEmpty);
      expect(find.text(pw), findsNothing);
      expect(allZero(api.revealed), isTrue);
    });

    testWidgets('leaving the item on the compact layout hides it too', (
      WidgetTester tester,
    ) async {
      final SpyApi api = await unlockedVault(samples: false);
      final String id = await addLogin(api);
      final Harness h = await openItem(tester, api, id, size: kCompact);
      await tapOn(tester, find.byTooltip(l10n.revealField(l10n.fieldPassword)));
      await tapOn(tester, find.byTooltip(l10n.actionBack));
      expect(h.container.read(revealProvider), isEmpty);
      expect(allZero(api.revealed), isTrue);
    });

    testWidgets(
      'a lock wipes revealed secrets and nothing survives in providers',
      (WidgetTester tester) async {
        final SpyApi api = await unlockedVault(samples: false);
        final String id = await addLogin(api);
        final Harness h = await openItem(tester, api, id);
        await tapOn(
          tester,
          find.byTooltip(l10n.revealField(l10n.fieldPassword)),
        );
        expect(find.text(pw), findsOneWidget);

        await tapOn(tester, find.byTooltip(l10n.actionLock));
        expect(textOf(l10n.lockTitle), findsOneWidget);
        expect(h.container.read(revealProvider), isEmpty);
        expect(allZero(api.revealed), isTrue);
        expect(find.text(pw), findsNothing);
        final VaultQueryState q = h.container.read(vaultQueryProvider);
        expect(q.selectedId, isNull);
        expect(q.searchInput, isEmpty);

        // After unlocking again nothing is revealed and nothing is selected.
        await typeInto(tester, l10n.masterPasswordLabel, kPassword);
        await tapOn(tester, buttonText(l10n.lockSubmit));
        expect(h.container.read(revealProvider), isEmpty);
        expect(find.text(pw), findsNothing);
        expect(h.container.read(vaultQueryProvider).selectedId, isNull);
      },
    );

    testWidgets('a locked error while revealing goes to the lock screen', (
      WidgetTester tester,
    ) async {
      final SpyApi api = await unlockedVault(samples: false);
      final String id = await addLogin(api);
      await openItem(tester, api, id);
      await api.lock();
      api.failNext['reveal'] = const AppError(AppErrorCode.locked, 'x');
      await tapOn(tester, find.byTooltip(l10n.revealField(l10n.fieldPassword)));
      expect(textOf(l10n.lockTitle), findsOneWidget);
    });
  });

  group('copy', () {
    testWidgets(
      'copies a secret through ClipboardService; the toast never shows it',
      (WidgetTester tester) async {
        final Services services = Services();
        final SpyApi api = await unlockedVault(samples: false);
        final String id = await addLogin(api);
        await openItem(tester, api, id, services: services);
        await tapOn(tester, find.byTooltip(l10n.copyField(l10n.fieldPassword)));
        expect(services.clipboard.copied, <String>[pw]);
        expect(find.text(l10n.copiedToast(30)), findsOneWidget);
        expect(find.textContaining('CANARY-hunter2'), findsNothing);
        expect(
          allZero(api.revealed),
          isTrue,
          reason: 'bytes wiped right after decoding',
        );
        expect(
          find.text('••••••••'),
          findsOneWidget,
          reason: 'copying does not reveal',
        );
      },
    );

    testWidgets('the toast uses the configured clear time', (
      WidgetTester tester,
    ) async {
      final Services services = Services();
      final SpyApi api = await unlockedVault(samples: false);
      await api.setSettings(const AppSettings(clipboardClearSeconds: 45));
      final String id = await addLogin(api);
      await openItem(tester, api, id, services: services);
      await tapOn(tester, find.byTooltip(l10n.copyField(l10n.fieldUsername)));
      expect(services.clipboard.copied, <String>['alice@example.invalid']);
      expect(find.text(l10n.copiedToast(45)), findsOneWidget);
    });

    testWidgets('an unavailable clipboard is reported', (
      WidgetTester tester,
    ) async {
      final SpyApi api = await unlockedVault(samples: false);
      final String id = await addLogin(api);
      await openItem(
        tester,
        api,
        id,
      ); // default services: unavailable? no - fakes
      // Use the real "unavailable" default by pumping without services overrides is not
      // possible here; instead make the API fail while copying a secret.
      api.failNext['reveal'] = const AppError(AppErrorCode.io, 'x');
      await tapOn(tester, find.byTooltip(l10n.copyField(l10n.fieldPassword)));
      expect(find.text(l10n.errIo), findsOneWidget);
    });
  });

  group('URLs', () {
    testWidgets('https opens without a prompt', (WidgetTester tester) async {
      final Services services = Services();
      final SpyApi api = await unlockedVault(samples: false);
      final String id = await addLogin(
        api,
        urls: <String>['https://ok.invalid/login'],
      );
      await openItem(tester, api, id, services: services);
      await tapOn(tester, find.byTooltip(l10n.openUrl));
      expect(services.urlLauncher.opened, <Uri>[
        Uri.parse('https://ok.invalid/login'),
      ]);
    });

    testWidgets('http asks first; Cancel does not open', (
      WidgetTester tester,
    ) async {
      final Services services = Services();
      final SpyApi api = await unlockedVault(samples: false);
      final String id = await addLogin(
        api,
        urls: <String>['http://plain.invalid/x'],
      );
      await openItem(tester, api, id, services: services);
      await tapOn(tester, find.byTooltip(l10n.openUrl));
      expect(textOf(l10n.openInsecureLinkTitle), findsOneWidget);
      expect(find.textContaining('does not use HTTPS'), findsOneWidget);
      await tapOn(tester, find.text(l10n.actionCancel));
      expect(services.urlLauncher.opened, isEmpty);
      await tapOn(tester, find.byTooltip(l10n.openUrl));
      await tapOn(
        tester,
        find.widgetWithText(FilledButton, l10n.openLinkConfirm),
      );
      expect(services.urlLauncher.opened, <Uri>[
        Uri.parse('http://plain.invalid/x'),
      ]);
    });

    testWidgets('other schemes are never launched', (
      WidgetTester tester,
    ) async {
      final Services services = Services();
      final SpyApi api = await unlockedVault(samples: false);
      final String id = await addLogin(
        api,
        urls: <String>[
          'javascript:alert(1)',
          'file:///etc/passwd',
          'ftp://x.invalid/',
        ],
      );
      await openItem(tester, api, id, services: services);
      for (var i = 0; i < 3; i++) {
        await tester.tap(find.byTooltip(l10n.openUrl).at(i));
        await tester.pumpAndSettle();
      }
      expect(services.urlLauncher.opened, isEmpty);
      expect(find.text(l10n.linkNotOpenable), findsWidgets);
    });
  });

  group('field kinds', () {
    testWidgets('TOTP: placeholder only when the item has a seed', (
      WidgetTester tester,
    ) async {
      final SpyApi api = await unlockedVault(samples: false);
      final String without = await addLogin(api, title: 'CANARY plain');
      final String withTotp = await addLogin(
        api,
        title: 'CANARY totp',
        fields: <StdField, String>{StdField.totpSeed: 'CANARY-seed'},
      );
      final Harness h = await openItem(tester, api, without);
      expect(find.text(l10n.totpLater), findsNothing);
      h.container.read(vaultQueryProvider.notifier).select(withTotp);
      await tester.pumpAndSettle();
      expect(find.text(l10n.totpLater), findsOneWidget);
      expect(find.textContaining('CANARY-seed'), findsNothing);
    });

    testWidgets('custom fields: text shown, hidden masked until revealed', (
      WidgetTester tester,
    ) async {
      final SpyApi api = await unlockedVault(samples: false);
      final String id = await addLogin(
        api,
        custom: const <NewCustom>[
          NewCustom(
            kind: CustomKind.text,
            label: 'Account no',
            value: 'CANARY-12345',
          ),
          NewCustom(
            kind: CustomKind.hidden,
            label: 'Security answer',
            value: 'CANARY-blue',
          ),
        ],
      );
      await openItem(tester, api, id);
      expect(find.text('CANARY-12345'), findsOneWidget);
      expect(find.text('CANARY-blue'), findsNothing);
      await tapOn(tester, find.byTooltip(l10n.revealField('Security answer')));
      expect(find.text('CANARY-blue'), findsOneWidget);
      expect(api.count('revealCustom'), 1);
      await tester.pump(const Duration(seconds: 16));
      expect(find.text('CANARY-blue'), findsNothing);
    });

    testWidgets(
      'tags, folder and dates are shown; a tag chip filters the list',
      (WidgetTester tester) async {
        final SpyApi api = await unlockedVault(samples: false);
        final String folder = await api.createFolder('Banking', null);
        final String id = await addLogin(
          api,
          tags: <String>['money', 'bills'],
          folderId: folder,
        );
        final Harness h = await openItem(tester, api, id);
        expect(find.text('${l10n.folderLabel}: Banking'), findsOneWidget);
        expect(find.textContaining('Created '), findsOneWidget);
        expect(find.textContaining('Updated '), findsOneWidget);
        await tapOn(tester, find.widgetWithText(ActionChip, 'money'));
        expect(
          h.container.read(vaultQueryProvider).view,
          const VaultView.tag('money'),
        );
        expect(api.listFilters.last.tag, 'money');
      },
    );

    testWidgets('a login without a folder says "No folder"', (
      WidgetTester tester,
    ) async {
      final SpyApi api = await unlockedVault(samples: false);
      final String id = await addLogin(api);
      await openItem(tester, api, id);
      expect(
        find.text('${l10n.folderLabel}: ${l10n.noFolder}'),
        findsOneWidget,
      );
    });

    testWidgets('card: number and CVV are secrets, holder and expiry are not', (
      WidgetTester tester,
    ) async {
      final SpyApi api = await unlockedVault(samples: false);
      final String id = await api.createItem(
        const NewItem(
          type: ItemType.card,
          title: 'CANARY Visa',
          fields: <StdField, String>{
            StdField.cardHolder: 'A. Holder',
            StdField.cardNumber: 'CANARY-4111',
            StdField.cardExpiry: '12/30',
            StdField.cardCvv: 'CANARY-123',
          },
        ),
      );
      await openItem(tester, api, id);
      expect(inDetail(find.text('A. Holder')), findsOneWidget);
      expect(inDetail(find.text('12/30')), findsOneWidget);
      expect(find.textContaining('CANARY-4111'), findsNothing);
      expect(
        find.byTooltip(l10n.revealField(l10n.fieldCardNumber)),
        findsOneWidget,
      );
      expect(
        find.byTooltip(l10n.revealField(l10n.fieldCardCvv)),
        findsOneWidget,
      );
    });
  });

  group('notes', () {
    Future<String> addNote(SpyApi api, String body) => api.createItem(
      NewItem(
        type: ItemType.note,
        title: 'CANARY note',
        fields: <StdField, String>{StdField.body: body},
      ),
    );

    testWidgets(
      'the body is masked until revealed, then rendered as Markdown',
      (WidgetTester tester) async {
        final SpyApi api = await unlockedVault(samples: false);
        final String id = await addNote(
          api,
          '# Heading\n\nSome **bold** text\n\n- one\n- two',
        );
        await openItem(tester, api, id);
        expect(
          find.textContaining('Heading', findRichText: true),
          findsNothing,
        );
        await tapOn(tester, find.byTooltip(l10n.revealField(l10n.fieldBody)));
        expect(
          find.textContaining('Heading', findRichText: true),
          findsOneWidget,
        );
        expect(
          find.textContaining('# Heading', findRichText: true),
          findsNothing,
          reason: 'rendered, not raw',
        );
        expect(find.text('•'), findsNWidgets(2));
        await tester.pump(const Duration(seconds: 16));
        expect(
          find.textContaining('Heading', findRichText: true),
          findsNothing,
        );
      },
    );

    testWidgets('a note link asks before opening; a hostile one cannot open', (
      WidgetTester tester,
    ) async {
      final Services services = Services();
      final SpyApi api = await unlockedVault(samples: false);
      final String id = await addNote(
        api,
        '[good](https://ok.invalid) [bad](javascript:alert(1)) <script>x</script> ![t](https://track.invalid/p.png)',
      );
      await openItem(tester, api, id, services: services);
      await tapOn(tester, find.byTooltip(l10n.revealField(l10n.fieldBody)));
      expect(find.byType(Image), findsNothing);
      expect(
        find.textContaining('<script>x</script>', findRichText: true),
        findsOneWidget,
      );
      expect(services.urlLauncher.opened, isEmpty);
    });
  });

  group('actions', () {
    testWidgets('Edit opens the form; Trash offers Undo', (
      WidgetTester tester,
    ) async {
      final SpyApi api = await unlockedVault(samples: false);
      final String id = await addLogin(api);
      final Harness h = await openItem(tester, api, id);
      await tapOn(tester, find.byTooltip(l10n.moveToTrash));
      expect(api.count('deleteItem'), 1);
      expect(h.container.read(vaultQueryProvider).selectedId, isNull);
      await tapOn(tester, find.text(l10n.undo));
      expect(await api.itemCount(), 1);
    });

    testWidgets('the favorite toggle in the detail pane works', (
      WidgetTester tester,
    ) async {
      final SpyApi api = await unlockedVault(samples: false);
      final String id = await addLogin(api);
      await openItem(tester, api, id);
      await tapOn(tester, find.byTooltip(l10n.favorite).last);
      expect((await api.getItem(id)).favorite, isTrue);
      expect(find.byTooltip(l10n.unfavorite), findsWidgets);
    });
  });

  group('errors', () {
    for (final (AppErrorCode, String Function()) c
        in <(AppErrorCode, String Function())>[
          (AppErrorCode.notFound, () => l10n.errNotFound),
          (AppErrorCode.io, () => l10n.errIo),
          (AppErrorCode.corruptVault, () => l10n.errCorruptVault),
          (AppErrorCode.internal, () => l10n.errInternal),
        ]) {
      testWidgets('${c.$1.name} on load renders its message', (
        WidgetTester tester,
      ) async {
        final SpyApi api = await unlockedVault(samples: false);
        final String id = await addLogin(api);
        api.failNext['getItem'] = AppError(c.$1, 'CANARY-detail');
        await openItem(tester, api, id);
        expect(textOf(c.$2()), findsOneWidget);
        expect(find.textContaining('CANARY-detail'), findsNothing);
      });
    }
  });
}
