import 'dart:math';
import 'dart:typed_data';

import 'package:arya_vault/src/api/dtos.dart';
import 'package:arya_vault/src/api/fake_vault_api.dart';
import 'package:arya_vault/src/api/secret_bytes.dart';
import 'package:arya_vault/src/app.dart';
import 'package:arya_vault/src/l10n/gen/app_localizations.dart';
import 'package:arya_vault/src/l10n/gen/app_localizations_en.dart';
import 'package:arya_vault/src/services/fake_services.dart';
import 'package:arya_vault/src/state/onboarding_state.dart';
import 'package:arya_vault/src/state/providers.dart';
import 'package:arya_vault/src/theme/app_theme.dart';
import 'package:flutter/material.dart';
import 'package:flutter_localizations/flutter_localizations.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_riverpod/misc.dart' show Override;
import 'package:flutter_test/flutter_test.dart';

/// English strings, straight from the generated localizations so tests follow
/// copy changes.
final AppLocalizationsEn l10n = AppLocalizationsEn();

/// Obvious fake secrets (CLAUDE.md rule 4).
const String kPassword = 'CANARY-correct-horse-battery-staple-9142';
const String kNewPassword = 'CANARY-another-long-passphrase-5531!';

Uint8List bytesOf(String s) => Uint8List.fromList(s.codeUnits);

/// A fake API without sample items, so tests start fast.
FakeVaultApi newFakeApi({
  QuickUnlockStatus quickUnlock = QuickUnlockStatus.unsupported,
  DateTime Function()? clock,
  Random? random,
}) => FakeVaultApi(
  seedSampleItems: false,
  quickUnlock: quickUnlock,
  clock: clock,
  random: random,
);

const QuickUnlockStatus kHello = QuickUnlockStatus(
  supported: true,
  enabled: false,
  kind: QuickUnlockKind.windowsHello,
);

class Harness {
  Harness(this.api, this.container);
  final FakeVaultApi api;
  final ProviderContainer container;
}

class Services {
  final FakeClipboardService clipboard = FakeClipboardService();
  final FakeFilePickerService filePicker = FakeFilePickerService();
  final FakePrintService printer = FakePrintService();
  final FakePlatformEvents events = FakePlatformEvents();
  final FakeUrlLauncherService urlLauncher = FakeUrlLauncherService();
}

/// Pumps the whole app against [api] at [size] (logical pixels) and [textScale].
Future<Harness> pumpApp(
  WidgetTester tester, {
  FakeVaultApi? api,
  Services? services,
  Size size = const Size(420, 900),
  double textScale = 1,
  ThemeMode themeMode = ThemeMode.light,
  List<Override> overrides = const <Override>[],
  bool settle = true,
}) async {
  final FakeVaultApi fake = api ?? newFakeApi();
  final Services s = services ?? Services();
  tester.view.physicalSize = size;
  tester.view.devicePixelRatio = 1;
  tester.platformDispatcher.textScaleFactorTestValue = textScale;
  addTearDown(tester.view.reset);
  addTearDown(tester.platformDispatcher.clearAllTestValues);
  await tester.pumpWidget(
    ProviderScope(
      retry: noAutoRetry,
      overrides: [
        vaultApiProvider.overrideWithValue(fake),
        clipboardServiceProvider.overrideWithValue(s.clipboard),
        filePickerServiceProvider.overrideWithValue(s.filePicker),
        printServiceProvider.overrideWithValue(s.printer),
        platformEventsProvider.overrideWithValue(s.events),
        urlLauncherServiceProvider.overrideWithValue(s.urlLauncher),
        themeModeProvider.overrideWith(() => _FixedTheme(themeMode)),
        ...overrides,
      ],
      child: const AryaVaultApp(),
    ),
  );
  if (settle) {
    await tester.pumpAndSettle();
  } else {
    await tester.pump();
  }
  final ProviderContainer container = ProviderScope.containerOf(
    tester.element(find.byType(AryaVaultApp)),
  );
  return Harness(fake, container);
}

class _FixedTheme extends ThemeModeNotifier {
  _FixedTheme(this.mode);
  final ThemeMode mode;
  @override
  ThemeMode build() => mode;
}

/// A [TextField] by its label text.
Finder fieldLabelled(String label) => find.byWidgetPredicate(
  (Widget w) => w is TextField && w.decoration?.labelText == label,
  description: 'TextField labelled "$label"',
);

Future<void> typeInto(WidgetTester tester, String label, String text) async {
  await tester.enterText(fieldLabelled(label), text);
  await tester.pumpAndSettle();
}

/// Creates a vault through the API (no UI) and leaves it unlocked with the
/// onboarding still pending; returns the key result.
Future<RecoveryKeyResult> createThroughApi(
  FakeVaultApi api, {
  String password = kPassword,
}) =>
    api.createVault(password: bytesOf(password), profile: KdfProfile.default_);

/// Creates, confirms (right answers) and locks, via the API only.
Future<void> createConfirmedLockedVault(
  FakeVaultApi api, {
  String password = kPassword,
}) async {
  final RecoveryKeyResult r = await createThroughApi(api, password: password);
  final List<String> groups = String.fromCharCodes(r.recoveryKey).split('-');
  final bool ok = await api.confirmRecoveryKey(<GroupAnswer>[
    for (final int i in r.challenge) GroupAnswer(index: i, text: groups[i]),
  ]);
  expect(ok, isTrue);
  await api.lock();
}

/// The text of the key shown on the recovery-key screen, read from the API
/// result (tests that need to type correct answers).
List<String> keyGroups(RecoveryKeyResult r) =>
    String.fromCharCodes(r.recoveryKey).split('-');

Future<void> tapOn(WidgetTester tester, Finder finder) async {
  await tester.pumpAndSettle(); // let earlier actions finish moving things
  await tester.ensureVisible(finder);
  await tester.pumpAndSettle();
  await tester.tap(finder);
  await tester.pumpAndSettle();
}

Finder buttonText(String text) => find.widgetWithText(FilledButton, text);

/// Visible text anywhere on screen.
Finder textOf(String text) => find.text(text);

/// The pending recovery key as the UI holds it, as groups.
List<String> pendingGroups(Harness h) {
  final SecretBytes key = h.container.read(onboardingProvider).key!;
  return String.fromCharCodes(key.copyBytes()).split('-');
}

List<int> pendingChallenge(Harness h) =>
    h.container.read(onboardingProvider).challenge;

/// Drives the UI from the Welcome screen through create + explainer so the
/// recovery-key screen is showing.
Future<void> createVaultViaUi(
  WidgetTester tester, {
  String password = kPassword,
}) async {
  await tapOn(tester, buttonText(l10n.welcomeCreate));
  await typeInto(tester, l10n.masterPasswordLabel, password);
  await typeInto(tester, l10n.confirmPasswordLabel, password);
  await tapOn(tester, buttonText(l10n.createSubmit));
  await tapOn(tester, buttonText(l10n.explainerContinue));
}

/// On the key screen: tick "I stored it" and continue to verification.
Future<void> storeKeyAndContinue(WidgetTester tester) async {
  await tapOn(tester, find.byType(CheckboxListTile));
  await tapOn(tester, buttonText(l10n.recoveryContinue));
}

/// On the verify screen: type [answers] into the fields in order.
Future<void> typeAnswers(WidgetTester tester, List<String> answers) async {
  final Finder fields = find.byType(TextField);
  expect(fields, findsNWidgets(answers.length));
  for (var i = 0; i < answers.length; i++) {
    await tester.enterText(fields.at(i), answers[i]);
  }
  await tester.pumpAndSettle();
}

/// Localization delegates and theme for tests that build a bare MaterialApp.
const List<LocalizationsDelegate<dynamic>> appDelegates =
    <LocalizationsDelegate<dynamic>>[
      AppLocalizations.delegate,
      GlobalMaterialLocalizations.delegate,
      GlobalWidgetsLocalizations.delegate,
      GlobalCupertinoLocalizations.delegate,
    ];

ThemeData get appTheme => AppTheme.light;
