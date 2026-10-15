import 'dart:convert';

import 'package:arya_vault/src/api/dtos.dart';
import 'package:arya_vault/src/api/fake_vault_api.dart';
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

import '../helpers.dart';

Future<Harness> openCreate(WidgetTester tester, {FakeVaultApi? api}) async {
  final Harness h = await pumpApp(tester, api: api);
  await tapOn(tester, buttonText(l10n.welcomeCreate));
  return h;
}

FilledButton submit(WidgetTester tester) =>
    tester.widget<FilledButton>(buttonText(l10n.createSubmit));

void main() {
  group('Welcome', () {
    testWidgets('offers create; add-device is a disabled placeholder', (
      WidgetTester tester,
    ) async {
      await pumpApp(tester);
      expect(textOf(l10n.welcomeTitle), findsOneWidget);
      expect(buttonText(l10n.welcomeCreate), findsOneWidget);
      final OutlinedButton add = tester.widget<OutlinedButton>(
        find.widgetWithText(OutlinedButton, l10n.welcomeAddDevice),
      );
      expect(add.onPressed, isNull);
      expect(textOf(l10n.welcomeAddDeviceUnavailable), findsOneWidget);
    });
  });

  group('Create vault', () {
    testWidgets('live strength meter and policy messages', (
      WidgetTester tester,
    ) async {
      await openCreate(tester);
      expect(textOf(l10n.policyEmpty), findsOneWidget);

      await typeInto(tester, l10n.masterPasswordLabel, 'abc');
      expect(
        find.textContaining('${l10n.strengthLabel}: ${l10n.strengthVeryWeak}'),
        findsOneWidget,
      );
      expect(find.textContaining('at least 12 characters'), findsOneWidget);
      expect(submit(tester).onPressed, isNull);

      await typeInto(tester, l10n.masterPasswordLabel, kPassword);
      expect(textOf(l10n.policyMet), findsOneWidget);
      expect(find.textContaining('at least 12'), findsNothing);
      expect(
        find.textContaining('${l10n.strengthLabel}: ${l10n.strengthStrong}'),
        findsOneWidget,
      );
    });

    testWidgets(
      'weak but long enough password is rejected by the core policy',
      (WidgetTester tester) async {
        await openCreate(tester);
        await typeInto(tester, l10n.masterPasswordLabel, 'password1234');
        await typeInto(tester, l10n.confirmPasswordLabel, 'password1234');
        expect(find.textContaining('too easy to guess'), findsOneWidget);
        expect(submit(tester).onPressed, isNull);
      },
    );

    testWidgets('mismatched confirmation blocks creation', (
      WidgetTester tester,
    ) async {
      await openCreate(tester);
      await typeInto(tester, l10n.masterPasswordLabel, kPassword);
      await typeInto(tester, l10n.confirmPasswordLabel, '${kPassword}x');
      expect(textOf(l10n.passwordMismatch), findsOneWidget);
      expect(submit(tester).onPressed, isNull);
      await typeInto(tester, l10n.confirmPasswordLabel, kPassword);
      expect(textOf(l10n.passwordMismatch), findsNothing);
      expect(submit(tester).onPressed, isNotNull);
    });

    testWidgets('suggests a passphrase between 12 and 15 characters', (
      WidgetTester tester,
    ) async {
      await openCreate(tester);
      await typeInto(tester, l10n.masterPasswordLabel, 'Xq7!mZ2#kLp9');
      expect(textOf(l10n.passphraseTip), findsOneWidget);
      await typeInto(tester, l10n.masterPasswordLabel, kPassword);
      expect(textOf(l10n.passphraseTip), findsNothing);
    });

    testWidgets('KDF profile is hidden under Advanced and is passed on', (
      WidgetTester tester,
    ) async {
      final FakeVaultApi api = newFakeApi();
      await openCreate(tester, api: api);
      expect(textOf(l10n.kdfLabel), findsNothing);
      await tapOn(tester, find.text(l10n.advancedTitle));
      expect(textOf(l10n.kdfLabel), findsOneWidget);
      await tapOn(tester, find.text(l10n.kdfHigh));
      await typeInto(tester, l10n.masterPasswordLabel, kPassword);
      await typeInto(tester, l10n.confirmPasswordLabel, kPassword);
      await tapOn(tester, buttonText(l10n.createSubmit));
      expect(api.lastProfile, KdfProfile.high);
    });

    testWidgets('default profile is the balanced one', (
      WidgetTester tester,
    ) async {
      final FakeVaultApi api = newFakeApi();
      await openCreate(tester, api: api);
      await typeInto(tester, l10n.masterPasswordLabel, kPassword);
      await typeInto(tester, l10n.confirmPasswordLabel, kPassword);
      await tapOn(tester, buttonText(l10n.createSubmit));
      expect(api.lastProfile, KdfProfile.default_);
    });

    testWidgets('reveal toggle shows the text and auto-hides after 15 s', (
      WidgetTester tester,
    ) async {
      await openCreate(tester);
      await typeInto(tester, l10n.masterPasswordLabel, kPassword);
      TextField first() =>
          tester.widget<TextField>(fieldLabelled(l10n.masterPasswordLabel));
      expect(first().obscureText, isTrue);
      await tester.tap(find.byTooltip(l10n.showPassword).first);
      await tester.pump();
      expect(first().obscureText, isFalse);
      await tester.pump(const Duration(seconds: 14));
      expect(first().obscureText, isFalse);
      await tester.pump(const Duration(seconds: 2));
      expect(first().obscureText, isTrue, reason: 'SEC-H03: hides within 15 s');
    });

    testWidgets('the fields are cleared after the vault is created', (
      WidgetTester tester,
    ) async {
      final Harness h = await openCreate(tester);
      await typeInto(tester, l10n.masterPasswordLabel, kPassword);
      await typeInto(tester, l10n.confirmPasswordLabel, kPassword);
      await tapOn(tester, buttonText(l10n.createSubmit));
      expect((await h.api.status()).exists, isTrue);
      expect(find.byType(TextField), findsNothing);
    });
  });

  group('Explainer', () {
    testWidgets('says plainly that no reset exists', (
      WidgetTester tester,
    ) async {
      await pumpApp(tester);
      await tapOn(tester, buttonText(l10n.welcomeCreate));
      await typeInto(tester, l10n.masterPasswordLabel, kPassword);
      await typeInto(tester, l10n.confirmPasswordLabel, kPassword);
      await tapOn(tester, buttonText(l10n.createSubmit));
      expect(textOf(l10n.explainerTitle), findsOneWidget);
      expect(
        textOf(
          "AryaVault can't reset your password — we don't have your data or a "
          'server. Your recovery key is the only backup. Store it somewhere '
          'safe and separate from this device.',
        ),
        findsOneWidget,
      );
    });
  });

  group('Recovery key screen', () {
    testWidgets('shows grouped key, checksum group, no copy affordance', (
      WidgetTester tester,
    ) async {
      final Services services = Services();
      final Harness h = await pumpApp(tester, services: services);
      await createVaultViaUi(tester);

      final List<String> groups = pendingGroups(h);
      for (final String g in groups) {
        expect(textOf(g), findsOneWidget);
      }
      expect(groups.last.length, 2, reason: 'checksum group');
      expect(textOf(l10n.recoveryChecksumLabel), findsOneWidget);
      expect(textOf(l10n.recoveryCaptureWarning), findsOneWidget);
      expect(textOf(l10n.recoveryNoCopy), findsOneWidget);

      // No clipboard affordance: no copy icon, no "copy" label, no service call.
      expect(find.byIcon(Icons.copy), findsNothing);
      expect(find.byIcon(Icons.content_copy), findsNothing);
      // The only actions are these four; none is a copy action.
      final Iterable<String> labels = find
          .descendant(
            of: find.byWidgetPredicate((Widget w) => w is ButtonStyleButton),
            matching: find.byType(Text),
          )
          .evaluate()
          .map((Element e) => (e.widget as Text).data!);
      expect(labels.toSet(), <String>{
        l10n.recoveryPrint,
        l10n.recoverySave,
        l10n.recoveryHideKey,
        l10n.recoveryContinue,
      });
      expect(services.clipboard.copied, isEmpty);
      expect(services.clipboard.clearCount, 0);
    });

    testWidgets('Continue is enabled only after "I stored it"', (
      WidgetTester tester,
    ) async {
      await pumpApp(tester);
      await createVaultViaUi(tester);
      FilledButton cont() =>
          tester.widget<FilledButton>(buttonText(l10n.recoveryContinue));
      expect(cont().onPressed, isNull);
      await tapOn(tester, find.byType(CheckboxListTile));
      expect(cont().onPressed, isNotNull);
      await tapOn(tester, find.byType(CheckboxListTile));
      expect(cont().onPressed, isNull);
    });

    testWidgets('show/hide toggle removes the key text from the tree', (
      WidgetTester tester,
    ) async {
      final Harness h = await pumpApp(tester);
      await createVaultViaUi(tester);
      final List<String> groups = pendingGroups(h);
      await tapOn(tester, find.text(l10n.recoveryHideKey));
      for (final String g in groups) {
        expect(textOf(g), findsNothing);
      }
      expect(find.text(l10n.recoveryShowKey), findsOneWidget);
      await tapOn(tester, find.text(l10n.recoveryShowKey));
      expect(textOf(groups.first), findsOneWidget);
    });

    testWidgets('no Print button where the platform cannot print', (
      WidgetTester tester,
    ) async {
      final Services services = Services()..printer.available = false;
      await pumpApp(tester, services: services);
      await createVaultViaUi(tester);
      expect(find.text(l10n.recoveryPrint), findsNothing);
      expect(find.text(l10n.recoverySave), findsOneWidget);
    });

    testWidgets('Print sends the sheet through PrintService', (
      WidgetTester tester,
    ) async {
      final Services services = Services();
      final Harness h = await pumpApp(tester, services: services);
      await createVaultViaUi(tester);
      await tapOn(tester, find.text(l10n.recoveryPrint));
      expect(services.printer.printed, hasLength(1));
      expect(services.printer.printed.single.title, l10n.recoverySheetTitle);
      expect(
        services.printer.printed.single.body,
        contains(pendingGroups(h).join('-')),
      );
    });

    testWidgets('Save as file goes through FilePickerService', (
      WidgetTester tester,
    ) async {
      final Services services = Services();
      final Harness h = await pumpApp(tester, services: services);
      await createVaultViaUi(tester);
      await tapOn(tester, find.text(l10n.recoverySave));
      expect(services.filePicker.saved, hasLength(1));
      expect(services.filePicker.saved.single.name, l10n.recoveryFileName);
      expect(
        utf8.decode(services.filePicker.saved.single.bytes),
        contains(pendingGroups(h).join('-')),
      );
      expect(textOf(l10n.recoverySaved), findsOneWidget);
    });

    testWidgets('a cancelled save dialog is not an error', (
      WidgetTester tester,
    ) async {
      final Services services = Services()..filePicker.saveResult = false;
      await pumpApp(tester, services: services);
      await createVaultViaUi(tester);
      await tapOn(tester, find.text(l10n.recoverySave));
      expect(textOf(l10n.recoverySaved), findsNothing);
      expect(textOf(l10n.recoverySaveFailed), findsNothing);
    });

    testWidgets('failures are reported, not swallowed', (
      WidgetTester tester,
    ) async {
      final Services services = Services()
        ..filePicker.failSave = true
        ..printer.fail = true;
      await pumpApp(tester, services: services);
      await createVaultViaUi(tester);
      await tapOn(tester, find.text(l10n.recoverySave));
      expect(textOf(l10n.recoveryServiceUnavailable), findsOneWidget);
      await tapOn(tester, find.text(l10n.recoveryPrint));
      expect(textOf(l10n.recoveryServiceUnavailable), findsOneWidget);
    });

    testWidgets('every group has a screen-reader label', (
      WidgetTester tester,
    ) async {
      final SemanticsHandle handle = tester.ensureSemantics();
      final Harness h = await pumpApp(tester);
      await createVaultViaUi(tester);
      final List<String> groups = pendingGroups(h);
      expect(
        find.bySemanticsLabel(
          l10n.recoveryGroupSemantics(
            1,
            groups.length,
            groups.first.split('').join(' '),
          ),
        ),
        findsOneWidget,
      );
      expect(
        find.bySemanticsLabel(
          l10n.recoveryChecksumSemantics(groups.last.split('').join(' ')),
        ),
        findsOneWidget,
      );
      handle.dispose();
    });
  });
}
