import 'dart:convert';
import 'dart:typed_data';

import 'package:arya_vault/src/api/dtos.dart';
import 'package:arya_vault/src/state/routing.dart';
import 'package:flutter/material.dart' hide Page;
import 'package:flutter_test/flutter_test.dart';

import '../helpers.dart';
import 'tool_helpers.dart';

const String _csv =
    'name,username,password,url,notes,folder\n'
    'CANARY Imported One,alice@example.invalid,CANARY-imp-pw-1,https://one.example.invalid,,Imports\n'
    'CANARY Imported Two,bob@example.invalid,,https://two.example.invalid,a note,\n'
    ',nobody,CANARY-imp-pw-3,,,\n';

Uint8List _bytes(String s) => Uint8List.fromList(utf8.encode(s));

Future<List<ItemSummary>> _all(BufferSpyApi api) =>
    api.list(const ListFilter(), const Page(offset: 0, limit: 200));

Future<void> _pickCsv(WidgetTester tester, Services s, String text) async {
  s.filePicker.pickResult = _bytes(text);
  await tapOn(tester, find.text(l10n.importChoose));
}

void main() {
  group('Import', () {
    testWidgets(
      'previews counts, duplicates, warnings and skipped reasons first',
      (WidgetTester tester) async {
        final BufferSpyApi api = await unlockedBufferVault();
        await api.createItem(
          const NewItem(
            type: ItemType.login,
            title: 'CANARY Imported One',
            fields: <StdField, String>{
              StdField.username: 'alice@example.invalid',
            },
          ),
        );
        final int before = (await _all(api)).length;
        final Services s = Services();
        await pumpRoute(
          tester,
          AppRoutes.importFile,
          api: api,
          services: s,
          size: const Size(700, 1400),
        );
        await _pickCsv(tester, s, _csv);
        expect(s.filePicker.pickedExtensions.single, <String>['csv']);
        expect(find.text(l10n.importPreviewTitle), findsOneWidget);
        expect(find.text(l10n.importPreviewCounts(2, 1)), findsOneWidget);
        expect(find.text(l10n.importPreviewDuplicates(1)), findsOneWidget);
        expect(find.text('${l10n.importWarningsTitle} (1)'), findsOneWidget);
        expect(
          find.text(l10n.importRecordLine(2, 'no password')),
          findsOneWidget,
        );
        expect(find.text('${l10n.importSkippedTitle} (1)'), findsOneWidget);
        expect(
          find.text(l10n.importRecordLine(3, 'missing name')),
          findsOneWidget,
        );
        expect(
          (await _all(api)).length,
          before,
          reason: 'nothing changes in a preview',
        );
        expect(
          api.importFiles.single.every((int b) => b == 0),
          isTrue,
          reason: 'the file buffer is wiped after the preview',
        );
      },
    );

    testWidgets('commit adds the items, skips duplicates, shows a summary', (
      WidgetTester tester,
    ) async {
      final BufferSpyApi api = await unlockedBufferVault();
      await api.createItem(
        const NewItem(
          type: ItemType.login,
          title: 'CANARY Imported One',
          fields: <StdField, String>{
            StdField.username: 'alice@example.invalid',
          },
        ),
      );
      final Services s = Services();
      await pumpRoute(
        tester,
        AppRoutes.importFile,
        api: api,
        services: s,
        size: const Size(700, 1400),
      );
      await _pickCsv(tester, s, _csv);
      await tapOn(tester, find.text(l10n.importCommit(2)));
      expect(find.text(l10n.importResultTitle), findsOneWidget);
      expect(find.text(l10n.importResultCreated(1)), findsOneWidget);
      expect(find.text(l10n.importResultFolders(1)), findsOneWidget);
      expect(find.text(l10n.importResultDuplicates(1)), findsOneWidget);
      expect(find.text(l10n.importResultInvalid(1)), findsOneWidget);
      final List<String> titles = (await _all(api))
          .map((ItemSummary i) => i.title)
          .toList();
      expect(
        titles.where((String t) => t == 'CANARY Imported One'),
        hasLength(1),
      );
      expect(titles, contains('CANARY Imported Two'));
      // Start over from the summary.
      await tapOn(tester, find.text(l10n.importAnother));
      expect(find.text(l10n.importChoose), findsOneWidget);
    });

    testWidgets('with "skip duplicates" off the duplicate is imported too', (
      WidgetTester tester,
    ) async {
      final BufferSpyApi api = await unlockedBufferVault();
      await api.createItem(
        const NewItem(
          type: ItemType.login,
          title: 'CANARY Imported One',
          fields: <StdField, String>{
            StdField.username: 'alice@example.invalid',
          },
        ),
      );
      final Services s = Services();
      await pumpRoute(
        tester,
        AppRoutes.importFile,
        api: api,
        services: s,
        size: const Size(700, 1400),
      );
      await _pickCsv(tester, s, _csv);
      await tapOn(
        tester,
        find.widgetWithText(SwitchListTile, l10n.importSkipDuplicates),
      );
      await tapOn(tester, find.text(l10n.importCommit(2)));
      expect(find.text(l10n.importResultCreated(2)), findsOneWidget);
      expect(
        (await _all(api))
            .where((ItemSummary i) => i.title == 'CANARY Imported One'),
        hasLength(2),
      );
    });

    testWidgets('the target folder option files the items', (
      WidgetTester tester,
    ) async {
      final BufferSpyApi api = await unlockedBufferVault();
      final String folder = await api.createFolder('CANARY Target', null);
      final Services s = Services();
      await pumpRoute(
        tester,
        AppRoutes.importFile,
        api: api,
        services: s,
        size: const Size(700, 1600),
      );
      await _pickCsv(tester, s, _csv);
      await tapOn(tester, find.byType(DropdownButtonFormField<String?>));
      await tapOn(tester, find.text('CANARY Target').last);
      await tapOn(tester, find.text(l10n.importCommit(2)));
      final List<ItemSummary> items = await _all(api);
      final ItemSummary two = items.firstWhere(
        (ItemSummary i) => i.title == 'CANARY Imported Two',
      );
      expect(two.folderId, folder);
    });

    testWidgets('cancel leaves the vault untouched', (
      WidgetTester tester,
    ) async {
      final BufferSpyApi api = await unlockedBufferVault();
      final Services s = Services();
      await pumpRoute(
        tester,
        AppRoutes.importFile,
        api: api,
        services: s,
        size: const Size(700, 1400),
      );
      await _pickCsv(tester, s, _csv);
      await tapOn(tester, find.text(l10n.actionCancel));
      expect(find.text(l10n.importChoose), findsOneWidget);
      expect(await _all(api), isEmpty);
    });

    testWidgets('Bitwarden JSON is previewed as JSON', (
      WidgetTester tester,
    ) async {
      final BufferSpyApi api = await unlockedBufferVault();
      final Services s = Services();
      await pumpRoute(
        tester,
        AppRoutes.importFile,
        api: api,
        services: s,
        size: const Size(700, 1400),
      );
      await tapOn(tester, find.text(l10n.importFormatBitwarden));
      s.filePicker.pickResult = _bytes(
        jsonEncode(<String, Object>{
          'folders': <Object>[
            <String, String>{'id': 'f1', 'name': 'BW Folder'},
          ],
          'items': <Object>[
            <String, Object>{
              'name': 'CANARY BW Login',
              'folderId': 'f1',
              'login': <String, Object>{
                'username': 'carol',
                'password': 'CANARY-bw-pw',
                'uris': <Object>[
                  <String, String>{'uri': 'https://bw.example.invalid'},
                ],
              },
            },
          ],
        }),
      );
      await tapOn(tester, find.text(l10n.importChoose));
      expect(s.filePicker.pickedExtensions.single, <String>['json']);
      expect(find.text(l10n.importPreviewCounts(1, 1)), findsOneWidget);
      await tapOn(tester, find.text(l10n.importCommit(1)));
      expect((await _all(api)).single.title, 'CANARY BW Login');
    });

    testWidgets('an empty file explains there is nothing to import', (
      WidgetTester tester,
    ) async {
      final BufferSpyApi api = await unlockedBufferVault();
      final Services s = Services();
      await pumpRoute(
        tester,
        AppRoutes.importFile,
        api: api,
        services: s,
        size: const Size(700, 1400),
      );
      await _pickCsv(tester, s, 'name,username\n');
      expect(find.text(l10n.importPreviewNone), findsOneWidget);
      final FilledButton commit = tester.widget<FilledButton>(
        find.widgetWithText(FilledButton, l10n.importCommit(0)),
      );
      expect(commit.onPressed, isNull);
    });

    testWidgets('core errors map to messages', (WidgetTester tester) async {
      final BufferSpyApi api = await unlockedBufferVault();
      api.importByteLimit = 10;
      final Services s = Services();
      await pumpRoute(
        tester,
        AppRoutes.importFile,
        api: api,
        services: s,
        size: const Size(700, 1400),
      );
      await _pickCsv(tester, s, _csv);
      expect(find.text(l10n.errLimitReached), findsOneWidget);
      api.importByteLimit = 1 << 20;
      await _pickCsv(tester, s, 'not,a,usable,file\n1,2,3\n');
      expect(find.text(l10n.errValidation), findsOneWidget);
    });

    testWidgets('a limitReached on commit keeps the preview and says so', (
      WidgetTester tester,
    ) async {
      final BufferSpyApi api = await unlockedBufferVault();
      api.importItemLimit = 1;
      final Services s = Services();
      await pumpRoute(
        tester,
        AppRoutes.importFile,
        api: api,
        services: s,
        size: const Size(700, 1400),
      );
      await _pickCsv(tester, s, _csv);
      await tapOn(tester, find.text(l10n.importCommit(2)));
      expect(find.text(l10n.errLimitReached), findsOneWidget);
      expect(find.text(l10n.importPreviewTitle), findsOneWidget);
      expect(await _all(api), isEmpty);
    });

    testWidgets('a cancelled or missing file chooser is reported', (
      WidgetTester tester,
    ) async {
      final BufferSpyApi api = await unlockedBufferVault();
      final Services s = Services();
      await pumpRoute(tester, AppRoutes.importFile, api: api, services: s);
      await tapOn(tester, find.text(l10n.importChoose));
      expect(find.text(l10n.importCancelledPick), findsOneWidget);
      s.filePicker.failPick = true;
      await tapOn(tester, find.text(l10n.importChoose));
      expect(find.text(l10n.recoveryServiceUnavailable), findsOneWidget);
    });

    testWidgets('long skipped lists are cut with "and N more"', (
      WidgetTester tester,
    ) async {
      final BufferSpyApi api = await unlockedBufferVault();
      final Services s = Services();
      await pumpRoute(
        tester,
        AppRoutes.importFile,
        api: api,
        services: s,
        size: const Size(700, 4000),
      );
      final String rows = List<String>.generate(
        60,
        (int i) => ',x$i',
      ).join('\n');
      await _pickCsv(tester, s, 'name,username\nCANARY ok,u\n$rows\n');
      expect(find.text('${l10n.importSkippedTitle} (60)'), findsOneWidget);
      expect(find.text(l10n.importMore(10)), findsOneWidget);
    });
  });

  group('Encrypted import', () {
    Future<Uint8List> exportFrom(BufferSpyApi api, String password) =>
        api.exportEncrypted(_bytes(password));

    testWidgets('asks for the export password; a wrong one is refused', (
      WidgetTester tester,
    ) async {
      final BufferSpyApi source = await unlockedBufferVault();
      await source.createItem(
        const NewItem(
          type: ItemType.login,
          title: 'CANARY Exported Login',
          fields: <StdField, String>{StdField.password: 'CANARY-exp-pw'},
        ),
      );
      final Uint8List file = await exportFrom(source, kNewPassword);
      final BufferSpyApi target = await unlockedBufferVault();
      final Services s = Services();
      await pumpRoute(tester, AppRoutes.importFile, api: target, services: s);
      await tapOn(tester, find.text(l10n.importFormatEncrypted));
      s.filePicker.pickResult = file;
      await tapOn(tester, find.text(l10n.importChoose));
      expect(s.filePicker.pickedExtensions.single, <String>['avex']);
      expect(find.text(l10n.importPasswordTitle), findsOneWidget);
      await typeInto(tester, l10n.importPasswordLabel, 'CANARY-wrong-password');
      await tapOn(
        tester,
        find.widgetWithText(FilledButton, l10n.importPasswordConfirm),
      );
      expect(find.text(l10n.errWrongExportPassword), findsOneWidget);
      expect(await _all(target), isEmpty);
      await typeInto(tester, l10n.importPasswordLabel, kNewPassword);
      await tapOn(
        tester,
        find.widgetWithText(FilledButton, l10n.importPasswordConfirm),
      );
      expect(find.text(l10n.importResultTitle), findsOneWidget);
      expect((await _all(target)).single.title, 'CANARY Exported Login');
      expect(
        target.received.every(isWiped),
        isTrue,
        reason: 'the export password buffers are wiped',
      );
    });

    testWidgets('a file that is not an export is reported as damaged', (
      WidgetTester tester,
    ) async {
      final BufferSpyApi target = await unlockedBufferVault();
      final Services s = Services();
      await pumpRoute(tester, AppRoutes.importFile, api: target, services: s);
      await tapOn(tester, find.text(l10n.importFormatEncrypted));
      s.filePicker.pickResult = _bytes('this is not an export');
      await tapOn(tester, find.text(l10n.importChoose));
      await typeInto(tester, l10n.importPasswordLabel, kNewPassword);
      await tapOn(
        tester,
        find.widgetWithText(FilledButton, l10n.importPasswordConfirm),
      );
      expect(find.text(l10n.errCorruptVault), findsOneWidget);
    });

    testWidgets('cancelling the password prompt drops the file', (
      WidgetTester tester,
    ) async {
      final BufferSpyApi source = await unlockedBufferVault();
      final Uint8List file = await exportFrom(source, kNewPassword);
      final BufferSpyApi target = await unlockedBufferVault();
      final Services s = Services();
      await pumpRoute(tester, AppRoutes.importFile, api: target, services: s);
      await tapOn(tester, find.text(l10n.importFormatEncrypted));
      s.filePicker.pickResult = file;
      await tapOn(tester, find.text(l10n.importChoose));
      await tapOn(tester, find.text(l10n.actionCancel));
      expect(find.text(l10n.importChoose), findsOneWidget);
      expect(await _all(target), isEmpty);
    });
  });

  group('Export', () {
    Future<void> seed(BufferSpyApi api) => api.createItem(
      const NewItem(
        type: ItemType.login,
        title: 'CANARY Export Me',
        fields: <StdField, String>{
          StdField.username: 'dave@example.invalid',
          StdField.password: 'CANARY-export-pw',
        },
      ),
    );

    Future<void> reauth(WidgetTester tester, String password) async {
      await typeInto(tester, l10n.masterPasswordLabel, password);
      await tapOn(tester, find.text(l10n.reauthConfirm));
    }

    testWidgets('CSV needs the typed confirmation, then the master password', (
      WidgetTester tester,
    ) async {
      final BufferSpyApi api = await unlockedBufferVault();
      await seed(api);
      final Services s = Services();
      await pumpRoute(tester, AppRoutes.exportFile, api: api, services: s);
      await tapOn(tester, find.text(l10n.exportCsvButton));
      expect(find.text(l10n.exportCsvWarnTitle), findsOneWidget);
      FilledButton confirm() => tester.widget<FilledButton>(
        find.widgetWithText(FilledButton, l10n.exportCsvConfirm),
      );
      expect(confirm().onPressed, isNull, reason: 'disabled until typed');
      await typeInto(tester, l10n.exportCsvTypeLabel, 'export pls');
      expect(confirm().onPressed, isNull);
      await typeInto(tester, l10n.exportCsvTypeLabel, l10n.exportCsvPhrase);
      expect(confirm().onPressed, isNotNull);
      expect(api.csvExports, 0, reason: 'nothing exported yet');
      await tapOn(tester, find.text(l10n.exportCsvConfirm));
      // Re-authentication: a wrong password stops it.
      await reauth(tester, 'CANARY-not-the-password');
      expect(find.text(l10n.reauthWrong), findsOneWidget);
      expect(api.csvExports, 0);
      await typeInto(tester, l10n.masterPasswordLabel, kPassword);
      await tapOn(tester, find.text(l10n.reauthConfirm));
      expect(api.csvExports, 1);
      expect(s.filePicker.saved.single.name, l10n.exportCsvFileName);
      final String csv = utf8.decode(s.filePicker.saved.single.bytes);
      expect(csv, contains('CANARY Export Me'));
      expect(csv, contains('CANARY-export-pw'));
      expect(find.text(l10n.exportSaved), findsOneWidget);
      expect(
        api.returnedExports.single.every((int b) => b == 0),
        isTrue,
        reason: 'the plaintext buffer is wiped after saving',
      );
    });

    testWidgets('cancelling the CSV warning exports nothing', (
      WidgetTester tester,
    ) async {
      final BufferSpyApi api = await unlockedBufferVault();
      final Services s = Services();
      await pumpRoute(tester, AppRoutes.exportFile, api: api, services: s);
      await tapOn(tester, find.text(l10n.exportCsvButton));
      await tapOn(tester, find.text(l10n.actionCancel));
      expect(api.csvExports, 0);
      expect(s.filePicker.saved, isEmpty);
    });

    testWidgets('a cancelled save dialog says "not saved"', (
      WidgetTester tester,
    ) async {
      final BufferSpyApi api = await unlockedBufferVault();
      await seed(api);
      final Services s = Services()..filePicker.saveResult = false;
      await pumpRoute(tester, AppRoutes.exportFile, api: api, services: s);
      await tapOn(tester, find.text(l10n.exportCsvButton));
      await typeInto(tester, l10n.exportCsvTypeLabel, l10n.exportCsvPhrase);
      await tapOn(tester, find.text(l10n.exportCsvConfirm));
      await reauth(tester, kPassword);
      expect(find.text(l10n.exportCancelled), findsOneWidget);
    });

    testWidgets('a failing or missing save service is reported', (
      WidgetTester tester,
    ) async {
      final BufferSpyApi api = await unlockedBufferVault();
      final Services s = Services()..filePicker.failSave = true;
      await pumpRoute(tester, AppRoutes.exportFile, api: api, services: s);
      await tapOn(tester, find.text(l10n.exportCsvButton));
      await typeInto(tester, l10n.exportCsvTypeLabel, l10n.exportCsvPhrase);
      await tapOn(tester, find.text(l10n.exportCsvConfirm));
      await reauth(tester, kPassword);
      expect(find.text(l10n.recoveryServiceUnavailable), findsOneWidget);
      expect(
        api.returnedExports.single.every((int b) => b == 0),
        isTrue,
        reason: 'wiped even when saving failed',
      );
    });

    testWidgets(
      'encrypted export: policy meter, keep-safe warning, wiped password',
      (WidgetTester tester) async {
        final BufferSpyApi api = await unlockedBufferVault();
        await seed(api);
        final Services s = Services();
        await pumpRoute(
          tester,
          AppRoutes.exportFile,
          api: api,
          services: s,
          size: const Size(700, 1400),
        );
        await tapOn(tester, find.text(l10n.exportEncryptedButton));
        await reauth(tester, kPassword);
        expect(find.text(l10n.exportPasswordTitle), findsOneWidget);
        expect(find.text(l10n.exportKeepSafe), findsOneWidget);
        FilledButton submit() => tester.widget<FilledButton>(
          find.widgetWithText(FilledButton, l10n.exportPasswordSubmit),
        );
        expect(submit().onPressed, isNull);
        await typeInto(tester, l10n.exportPasswordLabel, 'short');
        expect(find.textContaining('at least 12'), findsOneWidget);
        expect(submit().onPressed, isNull);
        await typeInto(tester, l10n.exportPasswordLabel, kNewPassword);
        await typeInto(
          tester,
          l10n.confirmPasswordLabel,
          'CANARY-different-one-1!',
        );
        expect(find.text(l10n.passwordMismatch), findsOneWidget);
        expect(submit().onPressed, isNull);
        await typeInto(tester, l10n.confirmPasswordLabel, kNewPassword);
        expect(submit().onPressed, isNotNull);
        await tapOn(
          tester,
          find.widgetWithText(FilledButton, l10n.exportPasswordSubmit),
        );
        expect(api.encryptedExports, 1);
        expect(s.filePicker.saved.single.name, l10n.exportEncryptedFileName);
        expect(
          utf8.decode(s.filePicker.saved.single.bytes),
          isNot(contains('CANARY Export Me')),
          reason: 'the encrypted file does not contain the title in clear',
        );
        expect(find.text(l10n.exportSaved), findsOneWidget);
        expect(api.received.every(isWiped), isTrue);
        expect(api.returnedExports.every(isWiped), isTrue);
      },
    );

    testWidgets('encrypted export needs the master password first', (
      WidgetTester tester,
    ) async {
      final BufferSpyApi api = await unlockedBufferVault();
      final Services s = Services();
      await pumpRoute(tester, AppRoutes.exportFile, api: api, services: s);
      await tapOn(tester, find.text(l10n.exportEncryptedButton));
      await tapOn(tester, find.text(l10n.actionCancel));
      expect(find.text(l10n.exportPasswordTitle), findsNothing);
      expect(api.encryptedExports, 0);
    });

    testWidgets('an export password round-trips through import', (
      WidgetTester tester,
    ) async {
      final BufferSpyApi api = await unlockedBufferVault();
      await seed(api);
      final Services s = Services();
      await pumpRoute(
        tester,
        AppRoutes.exportFile,
        api: api,
        services: s,
        size: const Size(700, 1400),
      );
      await tapOn(tester, find.text(l10n.exportEncryptedButton));
      await reauth(tester, kPassword);
      await typeInto(tester, l10n.exportPasswordLabel, kNewPassword);
      await typeInto(tester, l10n.confirmPasswordLabel, kNewPassword);
      await tapOn(
        tester,
        find.widgetWithText(FilledButton, l10n.exportPasswordSubmit),
      );
      final BufferSpyApi other = await unlockedBufferVault();
      final Uint8List imported = Uint8List.fromList(
        s.filePicker.saved.single.bytes,
      );
      final ImportResult r = await other.importEncrypted(
        file: imported,
        exportPassword: _bytes(kNewPassword),
      );
      expect(r.created, 1);
    });
  });
}
