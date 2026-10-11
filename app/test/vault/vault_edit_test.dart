import 'dart:typed_data';

import 'package:arya_vault/src/api/app_error.dart';
import 'package:arya_vault/src/api/dtos.dart';
import 'package:arya_vault/src/state/password_generator_hook.dart';
import 'package:arya_vault/src/state/vault_actions.dart';
import 'package:arya_vault/src/state/vault_query.dart';
import 'package:flutter/material.dart' hide Page;
import 'package:flutter_riverpod/misc.dart' show Override;
import 'package:flutter_test/flutter_test.dart';

import '../helpers.dart';
import 'vault_helpers.dart';

Future<String> seedLogin(SpyApi api) => api.createItem(
  const NewItem(
    type: ItemType.login,
    title: 'CANARY Bank',
    fields: <StdField, String>{
      StdField.username: 'alice@example.invalid',
      StdField.password: 'CANARY-old-password',
      StdField.notes: 'old notes',
    },
    urls: <String>['https://bank.invalid/a', 'https://bank.invalid/b'],
    tags: <String>['money', 'bills'],
    custom: <NewCustom>[
      NewCustom(kind: CustomKind.text, label: 'Branch', value: 'North'),
      NewCustom(
        kind: CustomKind.hidden,
        label: 'PIN word',
        value: 'CANARY-secret-word',
      ),
    ],
  ),
);

Future<Harness> openEdit(
  WidgetTester tester,
  SpyApi api,
  String id, {
  Size size = kDesktop,
}) async {
  final Harness h = await pumpVault(tester, api: api, size: size);
  h.container.read(vaultQueryProvider.notifier).select(id);
  await tester.pumpAndSettle();
  await tapOn(tester, find.byTooltip(l10n.edit));
  return h;
}

Future<Harness> openNew(
  WidgetTester tester,
  SpyApi api,
  ItemType type, {
  List<Override> overrides = const <Override>[],
}) async {
  final Harness h = await pumpVault(tester, api: api, overrides: overrides);
  await tapOn(tester, find.byTooltip(l10n.newItem));
  await tapOn(
    tester,
    find.descendant(
      of: find.byType(PopupMenuItem<ItemType>),
      matching: find.text(_typeLabel(type)),
    ),
  );
  return h;
}

String _typeLabel(ItemType t) => switch (t) {
  ItemType.login => l10n.typeLogin,
  ItemType.note => l10n.typeNote,
  ItemType.card => l10n.typeCard,
  ItemType.identity => l10n.typeIdentity,
};

Future<void> fill(WidgetTester tester, String label, String text) async {
  await tester.ensureVisible(fieldLabelled(label));
  await tester.enterText(fieldLabelled(label), text);
  await tester.pump();
}

Future<void> save(WidgetTester tester) async {
  await tapOn(tester, find.widgetWithText(FilledButton, l10n.save));
}

void main() {
  // A tap that misses its target is a test bug, not a warning.
  setUpAll(() => WidgetController.hitTestWarningShouldBeFatal = true);

  group('create', () {
    testWidgets('a login with everything: one createItem', (
      WidgetTester tester,
    ) async {
      final SpyApi api = await unlockedVault(samples: false);
      final String folder = await api.createFolder('Work', null);
      final Harness h = await openNew(tester, api, ItemType.login);
      expect(textOf(l10n.newItemTitle(l10n.typeLogin)), findsOneWidget);

      await fill(tester, l10n.fieldTitle, 'CANARY Mail');
      await fill(tester, l10n.fieldUsername, 'bob@example.invalid');
      await fill(tester, l10n.fieldPassword, 'CANARY-new-password');
      await fill(tester, l10n.fieldNotes, 'some notes');
      await tapOn(tester, find.text(l10n.addUrl));
      await tester.enterText(
        fieldLabelled(l10n.urlLabel),
        'https://mail.invalid',
      );
      await fill(tester, l10n.addTagLabel, 'inbox');
      await tapOn(tester, find.byTooltip(l10n.addTag));
      await tapOn(
        tester,
        find.widgetWithText(OutlinedButton, l10n.addCustomField(l10n.kindText)),
      );
      await fill(tester, l10n.customLabel, 'Plan');
      await fill(tester, l10n.customValue, 'Pro');
      await tapOn(tester, find.byType(DropdownButtonFormField<String?>));
      await tapOn(tester, find.text('Work').last);
      api.calls.clear();
      await save(tester);

      expect(api.count('createItem'), 1);
      expect(api.count('setFields') + api.count('setField'), 0);
      final String? id = h.container.read(vaultQueryProvider).selectedId;
      expect(id, isNotNull, reason: 'the new item is selected');
      final ItemView v = await api.getItem(id!);
      expect(v.title, 'CANARY Mail');
      expect(v.fields[StdField.username], 'bob@example.invalid');
      expect(v.fields[StdField.notes], 'some notes');
      expect(v.secretFieldsPresent, contains(StdField.password));
      expect(v.urls.single.url, 'https://mail.invalid');
      expect(v.tags, <String>['inbox']);
      expect(v.custom.single.label, 'Plan');
      expect(v.folderId, folder);
      expect(
        String.fromCharCodes((await api.reveal(id, StdField.password))!),
        'CANARY-new-password',
      );
      // Back on the list, the new item is there.
      expect(textOf(l10n.newItemTitle(l10n.typeLogin)), findsNothing);
      expect(listRow('CANARY Mail'), findsOneWidget);
    });

    testWidgets('a note', (WidgetTester tester) async {
      final SpyApi api = await unlockedVault(samples: false);
      final Harness h = await openNew(tester, api, ItemType.note);
      await fill(tester, l10n.fieldTitle, 'CANARY Diary');
      await fill(tester, l10n.fieldBody, '# Day 1\n\nDear diary');
      await save(tester);
      final ItemView v = await api.getItem(
        h.container.read(vaultQueryProvider).selectedId!,
      );
      expect(v.type, ItemType.note);
      expect(v.secretFieldsPresent, contains(StdField.body));
    });

    testWidgets('card and identity show their own fields', (
      WidgetTester tester,
    ) async {
      final SpyApi api = await unlockedVault(samples: false);
      await openNew(tester, api, ItemType.card);
      for (final String l in <String>[
        l10n.fieldCardHolder,
        l10n.fieldCardNumber,
        l10n.fieldCardExpiry,
        l10n.fieldCardCvv,
        l10n.fieldCardPin,
      ]) {
        expect(fieldLabelled(l), findsOneWidget, reason: l);
      }
      expect(fieldLabelled(l10n.fieldUsername), findsNothing);
      await tester.pageBack();
      await tester.pumpAndSettle();
      await tapOn(tester, find.byTooltip(l10n.newItem));
      await tapOn(
        tester,
        find.descendant(
          of: find.byType(PopupMenuItem<ItemType>),
          matching: find.text(l10n.typeIdentity),
        ),
      );
      for (final String l in <String>[
        l10n.fieldFirstName,
        l10n.fieldMiddleName,
        l10n.fieldLastName,
        l10n.fieldEmail,
        l10n.fieldPhone,
        l10n.fieldAddress,
        l10n.fieldIds,
      ]) {
        expect(fieldLabelled(l), findsOneWidget, reason: l);
      }
    });

    testWidgets('an empty title is rejected before any call', (
      WidgetTester tester,
    ) async {
      final SpyApi api = await unlockedVault(samples: false);
      await openNew(tester, api, ItemType.login);
      api.calls.clear();
      await save(tester);
      expect(textOf(l10n.titleRequired), findsOneWidget);
      expect(api.calls, isEmpty);
    });

    testWidgets('a core validation error lands on its field', (
      WidgetTester tester,
    ) async {
      final SpyApi api = await unlockedVault(samples: false);
      await openNew(tester, api, ItemType.login);
      await fill(tester, l10n.fieldTitle, 'CANARY x');
      api.failNext['createItem'] = const AppError(
        AppErrorCode.validation,
        'bad',
        field: 'username',
      );
      await save(tester);
      expect(textOf(l10n.errValidation), findsOneWidget);
      // The error is attached to the Username field and the form stays open.
      expect(
        tester
            .widget<TextField>(fieldLabelled(l10n.fieldUsername))
            .decoration!
            .errorText,
        l10n.errValidation,
      );
      expect(textOf(l10n.newItemTitle(l10n.typeLogin)), findsOneWidget);
    });

    testWidgets('limit and other errors show above the form', (
      WidgetTester tester,
    ) async {
      final SpyApi api = await unlockedVault(samples: false);
      await openNew(tester, api, ItemType.login);
      await fill(tester, l10n.fieldTitle, 'CANARY x');
      api.failNext['createItem'] = const AppError(
        AppErrorCode.limitReached,
        'x',
      );
      await save(tester);
      expect(textOf(l10n.errLimitReached), findsOneWidget);
    });
  });

  group('unsaved changes', () {
    testWidgets('leaving a clean form needs no question', (
      WidgetTester tester,
    ) async {
      final SpyApi api = await unlockedVault(samples: false);
      await openNew(tester, api, ItemType.login);
      await tester.pageBack();
      await tester.pumpAndSettle();
      expect(textOf(l10n.discardTitle), findsNothing);
      expect(fieldLabelled(l10n.searchLabel), findsOneWidget);
    });

    testWidgets('a dirty form asks; Cancel stays, Discard leaves', (
      WidgetTester tester,
    ) async {
      final SpyApi api = await unlockedVault(samples: false);
      await openNew(tester, api, ItemType.login);
      await fill(tester, l10n.fieldTitle, 'CANARY draft');
      await tester.pageBack();
      await tester.pumpAndSettle();
      expect(textOf(l10n.discardTitle), findsOneWidget);
      await tapOn(tester, find.text(l10n.actionCancel));
      expect(textOf(l10n.newItemTitle(l10n.typeLogin)), findsOneWidget);
      await tester.pageBack();
      await tester.pumpAndSettle();
      await tapOn(tester, find.widgetWithText(FilledButton, l10n.discard));
      expect(fieldLabelled(l10n.searchLabel), findsOneWidget);
      expect(await api.itemCount(), 0, reason: 'nothing was saved');
    });
  });

  group('edit an existing item', () {
    testWidgets('loads values; secrets stay empty with a keep hint', (
      WidgetTester tester,
    ) async {
      final SpyApi api = await unlockedVault(samples: false);
      final String id = await seedLogin(api);
      await openEdit(tester, api, id);
      expect(
        tester
            .widget<TextField>(fieldLabelled(l10n.fieldTitle))
            .controller!
            .text,
        'CANARY Bank',
      );
      expect(
        tester
            .widget<TextField>(fieldLabelled(l10n.fieldUsername))
            .controller!
            .text,
        'alice@example.invalid',
      );
      expect(
        tester
            .widget<TextField>(fieldLabelled(l10n.fieldPassword))
            .controller!
            .text,
        isEmpty,
      );
      expect(find.text(l10n.keepCurrentHint), findsWidgets);
      expect(
        api.count('reveal'),
        0,
        reason: 'the password is not fetched for editing',
      );
    });

    testWidgets('saving without changes calls nothing', (
      WidgetTester tester,
    ) async {
      final SpyApi api = await unlockedVault(samples: false);
      final String id = await seedLogin(api);
      await openEdit(tester, api, id);
      api.calls.clear();
      await save(tester);
      expect(
        api.calls.where((String c) => c != 'getItem' && c != 'list'),
        isEmpty,
      );
    });

    testWidgets('only the changed fields go out, in one atomic setFields', (
      WidgetTester tester,
    ) async {
      final SpyApi api = await unlockedVault(samples: false);
      final String id = await seedLogin(api);
      await openEdit(tester, api, id);
      await fill(tester, l10n.fieldTitle, 'CANARY Bank 2');
      await fill(tester, l10n.fieldUsername, 'carol@example.invalid');
      api.calls.clear();
      await save(tester);
      expect(api.count('setFields'), 1);
      expect(api.count('setField'), 0);
      final ItemView v = await api.getItem(id);
      expect(v.title, 'CANARY Bank 2');
      expect(v.fields[StdField.username], 'carol@example.invalid');
      expect(v.fields[StdField.notes], 'old notes');
      expect(
        String.fromCharCodes((await api.reveal(id, StdField.password))!),
        'CANARY-old-password',
      );
    });

    testWidgets('a typed secret replaces the old one', (
      WidgetTester tester,
    ) async {
      final SpyApi api = await unlockedVault(samples: false);
      final String id = await seedLogin(api);
      await openEdit(tester, api, id);
      await fill(tester, l10n.fieldPassword, 'CANARY-replaced');
      await save(tester);
      expect(
        String.fromCharCodes((await api.reveal(id, StdField.password))!),
        'CANARY-replaced',
      );
    });

    testWidgets('Remove clears a secret; Keep undoes that', (
      WidgetTester tester,
    ) async {
      final SpyApi api = await unlockedVault(samples: false);
      final String id = await seedLogin(api);
      await openEdit(tester, api, id);
      await tapOn(tester, find.text(l10n.removeValue(l10n.fieldPassword)));
      expect(find.text(l10n.willBeRemoved), findsOneWidget);
      await tapOn(tester, find.text(l10n.keepValue));
      expect(find.text(l10n.willBeRemoved), findsNothing);
      await tapOn(tester, find.text(l10n.removeValue(l10n.fieldPassword)));
      await save(tester);
      expect(
        (await api.getItem(id)).secretFieldsPresent,
        isNot(contains(StdField.password)),
      );
    });

    testWidgets('clearing a non-secret field clears it', (
      WidgetTester tester,
    ) async {
      final SpyApi api = await unlockedVault(samples: false);
      final String id = await seedLogin(api);
      await openEdit(tester, api, id);
      await fill(tester, l10n.fieldNotes, '');
      await save(tester);
      expect((await api.getItem(id)).fields[StdField.notes], isNull);
    });

    testWidgets('websites: change, remove and add', (
      WidgetTester tester,
    ) async {
      final SpyApi api = await unlockedVault(samples: false);
      final String id = await seedLogin(api);
      await openEdit(tester, api, id);
      final Finder urls = fieldLabelled(l10n.urlLabel);
      await tester.enterText(urls.at(0), 'https://bank.invalid/changed');
      await tapOn(tester, find.byTooltip(l10n.removeUrl).at(1));
      await tapOn(tester, find.text(l10n.addUrl));
      await tester.enterText(
        fieldLabelled(l10n.urlLabel).last,
        'https://new.invalid',
      );
      await save(tester);
      expect((await api.getItem(id)).urls.map((UrlView u) => u.url), <String>[
        'https://bank.invalid/changed',
        'https://new.invalid',
      ]);
    });

    testWidgets('tags: add and remove', (WidgetTester tester) async {
      final SpyApi api = await unlockedVault(samples: false);
      final String id = await seedLogin(api);
      await openEdit(tester, api, id);
      expect(find.text(l10n.tagsCount(2, 50)), findsOneWidget);
      await tapOn(tester, find.byTooltip(l10n.removeTag('bills')));
      await fill(tester, l10n.addTagLabel, 'fresh');
      await tapOn(tester, find.byTooltip(l10n.addTag));
      expect(find.text(l10n.tagsCount(2, 50)), findsOneWidget);
      await save(tester);
      expect((await api.getItem(id)).tags.toSet(), <String>{'money', 'fresh'});
    });

    testWidgets('custom fields: edit, hidden keep/replace, add, remove', (
      WidgetTester tester,
    ) async {
      final SpyApi api = await unlockedVault(samples: false);
      final String id = await seedLogin(api);
      await openEdit(tester, api, id);
      final ItemView before = await api.getItem(id);
      final CustomView hidden = before.custom.firstWhere(
        (CustomView c) => c.kind == CustomKind.hidden,
      );
      expect(find.text(l10n.customFieldsCount(2, 100)), findsOneWidget);
      // Rename the text field's label and change its value.
      await tester.enterText(
        fieldLabelled(l10n.customLabel).at(0),
        'Branch office',
      );
      await tester.enterText(fieldLabelled(l10n.customValue).at(0), 'South');
      // Leave the hidden one alone, add a new hidden one, remove nothing yet.
      await tapOn(
        tester,
        find.widgetWithText(
          OutlinedButton,
          l10n.addCustomField(l10n.kindHidden),
        ),
      );
      await tester.enterText(
        fieldLabelled(l10n.customLabel).last,
        'New secret',
      );
      await tester.enterText(
        fieldLabelled(l10n.customValue).last,
        'CANARY-new-hidden',
      );
      await save(tester);
      final ItemView after = await api.getItem(id);
      expect(
        after.custom.map((CustomView c) => c.label),
        containsAll(<String>['Branch office', 'PIN word', 'New secret']),
      );
      expect(
        after.custom
            .firstWhere((CustomView c) => c.label == 'Branch office')
            .valueIfNotHidden,
        'South',
      );
      expect(
        String.fromCharCodes((await api.revealCustom(id, hidden.id))!),
        'CANARY-secret-word',
        reason: 'untouched hidden value kept',
      );
    });

    testWidgets('custom fields can be removed', (WidgetTester tester) async {
      final SpyApi api = await unlockedVault(samples: false);
      final String id = await seedLogin(api);
      await openEdit(tester, api, id);
      await tester.ensureVisible(find.byTooltip(l10n.removeCustomField).first);
      await tester.tap(find.byTooltip(l10n.removeCustomField).first);
      await tester.pumpAndSettle();
      await save(tester);
      expect(
        (await api.getItem(id)).custom.map((CustomView c) => c.label),
        <String>['PIN word'],
      );
    });

    testWidgets('the folder can be changed and cleared', (
      WidgetTester tester,
    ) async {
      final SpyApi api = await unlockedVault(samples: false);
      final String folder = await api.createFolder('Banking', null);
      final String id = await seedLogin(api);
      await openEdit(tester, api, id);
      await tapOn(tester, find.byType(DropdownButtonFormField<String?>));
      await tapOn(tester, find.text('Banking').last);
      await save(tester);
      expect((await api.getItem(id)).folderId, folder);
      await tapOn(tester, find.byTooltip(l10n.edit));
      await tapOn(tester, find.byType(DropdownButtonFormField<String?>));
      await tapOn(tester, find.text(l10n.noFolder).last);
      await save(tester);
      expect((await api.getItem(id)).folderId, isNull);
    });

    testWidgets('a note body is prefilled so it can be edited', (
      WidgetTester tester,
    ) async {
      final SpyApi api = await unlockedVault(samples: false);
      final String id = await api.createItem(
        const NewItem(
          type: ItemType.note,
          title: 'CANARY n',
          fields: <StdField, String>{StdField.body: 'line one'},
        ),
      );
      await openEdit(tester, api, id);
      expect(
        tester
            .widget<TextField>(fieldLabelled(l10n.fieldBody))
            .controller!
            .text,
        'line one',
      );
      expect(
        api.revealed.every((Uint8List b) => b.every((int x) => x == 0)),
        isTrue,
        reason: 'prefill bytes wiped',
      );
      await fill(tester, l10n.fieldBody, 'line one\nline two');
      await save(tester);
      expect(
        String.fromCharCodes((await api.reveal(id, StdField.body))!),
        'line one\nline two',
      );
    });
  });

  group('hints and hooks', () {
    testWidgets('a size hint appears once a field is half full', (
      WidgetTester tester,
    ) async {
      final SpyApi api = await unlockedVault(samples: false);
      await openNew(tester, api, ItemType.login);
      await fill(tester, l10n.fieldNotes, 'a' * 40000);
      expect(find.text(l10n.sizeHint('39.1 KiB', '64.0 KiB')), findsOneWidget);
    });

    testWidgets('the password generator hook fills the password field', (
      WidgetTester tester,
    ) async {
      final SpyApi api = await unlockedVault(samples: false);
      await openNew(
        tester,
        api,
        ItemType.login,
        overrides: <Override>[
          passwordGeneratorLauncherProvider.overrideWithValue(
            (BuildContext c) async =>
                Uint8List.fromList('CANARY-generated'.codeUnits),
          ),
        ],
      );
      await tapOn(tester, find.byTooltip(l10n.generatePassword));
      expect(
        tester
            .widget<TextField>(fieldLabelled(l10n.fieldPassword))
            .controller!
            .text,
        'CANARY-generated',
      );
    });

    testWidgets('the default generator opens the sheet; Use fills the field', (
      WidgetTester tester,
    ) async {
      final SpyApi api = await unlockedVault(samples: false);
      await openNew(tester, api, ItemType.login);
      await tapOn(tester, find.byTooltip(l10n.generatePassword));
      expect(find.text(l10n.generatorSheetTitle), findsOneWidget);
      await tapOn(tester, find.text(l10n.generatorUse));
      expect(find.text(l10n.generatorSheetTitle), findsNothing);
      expect(
        tester
            .widget<TextField>(fieldLabelled(l10n.fieldPassword))
            .controller!
            .text
            .length,
        20,
      );
    });

    testWidgets('cancelling the generator sheet leaves the field alone', (
      WidgetTester tester,
    ) async {
      final SpyApi api = await unlockedVault(samples: false);
      await openNew(tester, api, ItemType.login);
      await tapOn(tester, find.byTooltip(l10n.generatePassword));
      await tapOn(tester, find.text(l10n.actionCancel));
      expect(
        tester
            .widget<TextField>(fieldLabelled(l10n.fieldPassword))
            .controller!
            .text,
        isEmpty,
      );
    });
  });

  group('lock mid-edit', () {
    testWidgets(
      'a locked error on save returns to the lock screen and drops every draft',
      (WidgetTester tester) async {
        final SpyApi api = await unlockedVault(samples: false);
        final Harness h = await openNew(tester, api, ItemType.login);
        await fill(tester, l10n.fieldTitle, 'CANARY draft title');
        await fill(tester, l10n.fieldPassword, 'CANARY-draft-password');
        await api.lock();
        api.failNext['createItem'] = const AppError(AppErrorCode.locked, 'x');
        await save(tester);
        expect(textOf(l10n.lockTitle), findsOneWidget);
        expect(find.textContaining('CANARY draft'), findsNothing);
        expect(h.container.read(vaultQueryProvider).selectedId, isNull);

        await typeInto(tester, l10n.masterPasswordLabel, kPassword);
        await tapOn(tester, buttonText(l10n.lockSubmit));
        await tapOn(tester, find.byTooltip(l10n.newItem));
        await tapOn(
          tester,
          find.descendant(
            of: find.byType(PopupMenuItem<ItemType>),
            matching: find.text(l10n.typeLogin),
          ),
        );
        expect(
          tester
              .widget<TextField>(fieldLabelled(l10n.fieldTitle))
              .controller!
              .text,
          isEmpty,
          reason: 'no draft restored',
        );
        expect(
          tester
              .widget<TextField>(fieldLabelled(l10n.fieldPassword))
              .controller!
              .text,
          isEmpty,
        );
        expect(await api.itemCount(), 0);
      },
    );

    testWidgets('locking while the form is open leaves nothing behind', (
      WidgetTester tester,
    ) async {
      final SpyApi api = await unlockedVault(samples: false);
      final Harness h = await openNew(tester, api, ItemType.login);
      await fill(tester, l10n.fieldTitle, 'CANARY draft title');
      await fill(tester, l10n.fieldPassword, 'CANARY-draft-password');
      await h.container.read(vaultActionsProvider).lock();
      await tester.pumpAndSettle();
      expect(textOf(l10n.lockTitle), findsOneWidget);
      expect(find.textContaining('CANARY draft'), findsNothing);
      expect(find.textContaining('CANARY-draft'), findsNothing);
    });
  });
}
