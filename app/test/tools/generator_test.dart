import 'dart:async';
import 'dart:typed_data';

import 'package:arya_vault/src/api/app_error.dart';
import 'package:arya_vault/src/api/dtos.dart';
import 'package:arya_vault/src/api/secret_bytes.dart';
import 'package:arya_vault/src/state/generator_state.dart';
import 'package:arya_vault/src/state/routing.dart';
import 'package:arya_vault/src/state/vault_actions.dart';
import 'package:flutter/material.dart' hide Page;
import 'package:flutter_test/flutter_test.dart';

import '../helpers.dart';
import 'tool_helpers.dart';

// SEC-C09 (UI side): the generator shows only core-generated values; Dart never
// generates. SEC-H01: copy goes through the clipboard manager.
void main() {
  group('GeneratorController', () {
    late BufferSpyApi api;

    setUp(() async => api = await unlockedBufferVault());

    test('asks the core for the value, entropy and strength', () async {
      final GeneratorController c = GeneratorController(api);
      await c.regenerate();
      expect(c.hasValue, isTrue);
      expect(c.displayText()!.length, 20);
      expect(c.entropyBits, greaterThan(50));
      expect(c.strength, isNotNull);
      expect(api.passwordOptions, hasLength(1));
      c.dispose();
    });

    test('a new value wipes the old one', () async {
      final GeneratorController c = GeneratorController(api);
      await c.regenerate();
      final Uint8List first = api.generated.first;
      await c.regenerate();
      expect(isWiped(first), isTrue, reason: 'the replaced buffer is wiped');
      expect(isWiped(api.generated.last), isFalse);
      c.dispose();
    });

    test('dispose and wipe zero the value', () async {
      final GeneratorController c = GeneratorController(api);
      await c.regenerate();
      c.wipe();
      expect(c.hasValue, isFalse);
      expect(isWiped(api.generated.single), isTrue);
      await c.regenerate();
      c.dispose();
      expect(isWiped(api.generated.last), isTrue);
    });

    test('options with no character class never reach the core', () async {
      final GeneratorController c = GeneratorController(api);
      c.password = const PasswordOptions(
        lower: false,
        upper: false,
        digits: false,
        symbols: false,
      );
      await c.regenerate();
      expect(c.problem, GeneratorProblem.noCharacterClass);
      expect(c.hasValue, isFalse);
      expect(api.passwordOptions, isEmpty);
      c.dispose();
    });

    test('symbols on with an empty set is a problem too', () async {
      final GeneratorController c = GeneratorController(api);
      c.password = const PasswordOptions(symbolSet: '');
      await c.regenerate();
      expect(c.problem, GeneratorProblem.noSymbols);
      expect(api.passwordOptions, isEmpty);
      c.dispose();
    });

    test('passphrase mode uses the passphrase call', () async {
      final GeneratorController c = GeneratorController(api)
        ..mode = GeneratorMode.passphrase;
      await c.regenerate();
      expect(api.passphraseOptions, hasLength(1));
      expect(c.displayText(), contains('-'));
      c.dispose();
    });

    test('a core error is kept, the value cleared', () async {
      final GeneratorController c = GeneratorController(api);
      await c.regenerate();
      c.password = const PasswordOptions(length: 4); // below the core's minimum
      await c.regenerate();
      expect(c.error?.code, AppErrorCode.validation);
      expect(c.hasValue, isFalse);
      c.dispose();
    });

    test('takeCopy hands out an independent wipeable copy', () async {
      final GeneratorController c = GeneratorController(api);
      await c.regenerate();
      final SecretBytes copy = c.takeCopy()!;
      final String text = c.displayText()!;
      copy.wipe();
      expect(c.displayText(), text, reason: 'wiping the copy keeps the value');
      c.dispose();
    });

    test('the latest request wins and the stale buffer is wiped', () async {
      final _SlowApi slow = await _slowApi();
      final GeneratorController c = GeneratorController(slow);
      final Completer<void> gate = slow.hold();
      final Future<void> first = c.regenerate();
      await Future<void>.delayed(Duration.zero);
      slow.release();
      final Future<void> second = c.regenerate();
      await second;
      gate.complete();
      await first;
      expect(slow.generated, hasLength(2));
      expect(isWiped(slow.generated.first), isTrue);
      expect(c.displayText(), String.fromCharCodes(slow.generated.last));
      c.dispose();
    });

    test('toString never prints the value', () async {
      final GeneratorController c = GeneratorController(api);
      await c.regenerate();
      expect(c.toString(), isNot(contains(c.displayText()!)));
      c.dispose();
    });
  });

  group('Generator screen', () {
    testWidgets('shows a value, live entropy and strength from the core', (
      WidgetTester tester,
    ) async {
      final BufferSpyApi api = await unlockedBufferVault();
      await pumpRoute(tester, AppRoutes.generator, api: api);
      expect(find.text(l10n.generatorTitle), findsOneWidget);
      expect(api.passwordOptions, hasLength(1));
      expect(find.textContaining('bits of entropy'), findsOneWidget);
      expect(
        find.text('${l10n.strengthLabel}: ${l10n.strengthStrong}'),
        findsOneWidget,
      );
      // The screen has no "use" button: nothing to fill.
      expect(find.text(l10n.generatorUse), findsNothing);
    });

    testWidgets('options go to the core: length, classes, ambiguous, symbols', (
      WidgetTester tester,
    ) async {
      final BufferSpyApi api = await unlockedBufferVault();
      await pumpRoute(tester, AppRoutes.generator, api: api);
      await tester.drag(find.byType(Slider), const Offset(-2000, 0));
      await tester.pumpAndSettle();
      expect(api.passwordOptions.last.length, 8);
      await tapOn(
        tester,
        find.widgetWithText(SwitchListTile, l10n.generatorDigits),
      );
      expect(api.passwordOptions.last.digits, isFalse);
      await tapOn(
        tester,
        find.widgetWithText(SwitchListTile, l10n.generatorExcludeAmbiguous),
      );
      expect(api.passwordOptions.last.excludeAmbiguous, isTrue);
      await tester.enterText(fieldLabelled(l10n.generatorSymbolSet), '!?');
      await tester.pumpAndSettle();
      expect(api.passwordOptions.last.symbolSet, '!?');
    });

    testWidgets('turning every class off explains it and offers no copy', (
      WidgetTester tester,
    ) async {
      final BufferSpyApi api = await unlockedBufferVault();
      await pumpRoute(tester, AppRoutes.generator, api: api);
      for (final String label in <String>[
        l10n.generatorLower,
        l10n.generatorUpper,
        l10n.generatorDigits,
        l10n.generatorSymbols,
      ]) {
        await tapOn(tester, find.widgetWithText(SwitchListTile, label));
      }
      expect(find.text(l10n.generatorNeedClass), findsOneWidget);
      final OutlinedButton copy = tester.widget<OutlinedButton>(
        find.widgetWithText(OutlinedButton, l10n.generatorCopy),
      );
      expect(copy.onPressed, isNull);
    });

    testWidgets('passphrase mode: words, separator, capitalize, number', (
      WidgetTester tester,
    ) async {
      final BufferSpyApi api = await unlockedBufferVault();
      await pumpRoute(tester, AppRoutes.generator, api: api);
      await tapOn(tester, find.text(l10n.generatorModePassphrase));
      expect(api.passphraseOptions.last.wordCount, 6);
      await tester.drag(find.byType(Slider), const Offset(-2000, 0));
      await tester.pumpAndSettle();
      expect(api.passphraseOptions.last.wordCount, 3);
      await tapOn(
        tester,
        find.widgetWithText(SwitchListTile, l10n.generatorCapitalize),
      );
      expect(api.passphraseOptions.last.capitalize, isTrue);
      await tapOn(
        tester,
        find.widgetWithText(SwitchListTile, l10n.generatorNumber),
      );
      expect(api.passphraseOptions.last.numberSuffix, isTrue);
      await tester.enterText(fieldLabelled(l10n.generatorSeparator), '.');
      await tester.pumpAndSettle();
      expect(api.passphraseOptions.last.separator, '.');
    });

    testWidgets('regenerate asks the core again', (WidgetTester tester) async {
      final BufferSpyApi api = await unlockedBufferVault();
      await pumpRoute(tester, AppRoutes.generator, api: api);
      await tapOn(tester, find.byTooltip(l10n.generatorRegenerate));
      expect(api.passwordOptions, hasLength(2));
    });

    testWidgets('copy goes through the clipboard manager, sensitive', (
      WidgetTester tester,
    ) async {
      final BufferSpyApi api = await unlockedBufferVault();
      final Services services = Services();
      await pumpRoute(
        tester,
        AppRoutes.generator,
        api: api,
        services: services,
      );
      final String shown = String.fromCharCodes(api.generated.last);
      await tapOn(
        tester,
        find.widgetWithText(OutlinedButton, l10n.generatorCopy),
      );
      expect(services.clipboard.copied, <String>[shown]);
      expect(services.clipboard.sensitiveFlags, <bool>[true]);
      expect(find.text(l10n.copiedToast(30)), findsOneWidget);
      expect(find.textContaining(shown), findsOneWidget);
    });

    testWidgets('hide masks the value on screen', (WidgetTester tester) async {
      final BufferSpyApi api = await unlockedBufferVault();
      await pumpRoute(tester, AppRoutes.generator, api: api);
      final String shown = String.fromCharCodes(api.generated.last);
      expect(find.text(shown), findsOneWidget);
      await tapOn(tester, find.byTooltip(l10n.generatorHide));
      expect(find.text(shown), findsNothing);
      expect(find.byTooltip(l10n.generatorShow), findsOneWidget);
    });

    testWidgets('leaving the screen wipes every generated buffer', (
      WidgetTester tester,
    ) async {
      final BufferSpyApi api = await unlockedBufferVault();
      final Harness h = await pumpRoute(tester, AppRoutes.generator, api: api);
      await tapOn(tester, find.byTooltip(l10n.generatorRegenerate));
      expect(api.generated, hasLength(2));
      h.container.read(routerProvider).pop();
      await tester.pumpAndSettle();
      expect(find.text(l10n.generatorTitle), findsNothing);
      expect(api.generated.every(isWiped), isTrue);
    });

    testWidgets('a lock while open wipes the value and shows the lock screen', (
      WidgetTester tester,
    ) async {
      final BufferSpyApi api = await unlockedBufferVault();
      final Harness h = await pumpRoute(tester, AppRoutes.generator, api: api);
      await h.container.read(vaultActionsProvider).lock();
      await tester.pumpAndSettle();
      expect(find.text(l10n.lockTitle), findsOneWidget);
      expect(api.generated.every(isWiped), isTrue);
    });
  });

  group('Generator sheet (edit form hook)', () {
    testWidgets('Use hands back the bytes; Cancel hands back nothing', (
      WidgetTester tester,
    ) async {
      final BufferSpyApi api = await unlockedBufferVault();
      await pumpRoute(tester, '/home/new/login', api: api);
      await tapOn(tester, find.byTooltip(l10n.generatePassword));
      expect(find.text(l10n.generatorSheetTitle), findsOneWidget);
      expect(find.text(l10n.generatorUse), findsOneWidget);
      await tapOn(tester, find.text(l10n.actionCancel));
      expect(find.text(l10n.generatorSheetTitle), findsNothing);
      expect(api.generated.every(isWiped), isTrue, reason: 'wiped on dismiss');
    });
  });
}

class _SlowApi extends BufferSpyApi {
  _SlowApi() : super(seedSampleItems: false, sampleTrashCount: 0);
  Completer<void>? _gate;

  Completer<void> hold() => _gate = Completer<void>();
  void release() => _gate = null;

  @override
  Future<Uint8List> generatePassword(PasswordOptions options) async {
    final Completer<void>? g = _gate;
    final Uint8List b = await super.generatePassword(options);
    if (g != null) await g.future;
    return b;
  }
}

Future<_SlowApi> _slowApi() async {
  final _SlowApi api = _SlowApi();
  final RecoveryKeyResult r = await createThroughApi(api);
  final List<String> g = keyGroups(r);
  await api.confirmRecoveryKey(<GroupAnswer>[
    for (final int i in r.challenge) GroupAnswer(index: i, text: g[i]),
  ]);
  return api;
}
