import 'package:arya_vault/src/api/dtos.dart';
import 'package:arya_vault/src/api/fake_vault_api.dart';
import 'package:arya_vault/src/api/secret_bytes.dart';
import 'package:arya_vault/src/state/onboarding_state.dart';
import 'package:arya_vault/src/state/vault_status.dart';
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

import '../helpers.dart';

/// Records every `confirmRecoveryKey` result and every status read, so a test
/// can prove onboarding never completes without a successful confirm.
class SpyApi extends FakeVaultApi {
  SpyApi({super.quickUnlock}) : super(seedSampleItems: false);

  final List<bool> confirmResults = <bool>[];
  final List<({bool complete, int confirmsSoFar})> statusLog =
      <({bool complete, int confirmsSoFar})>[];

  @override
  Future<bool> confirmRecoveryKey(List<GroupAnswer> answers) async {
    final bool ok = await super.confirmRecoveryKey(answers);
    confirmResults.add(ok);
    return ok;
  }

  @override
  Future<VaultStatus> status() async {
    final VaultStatus s = await super.status();
    statusLog.add((
      complete: s.onboardingComplete,
      confirmsSoFar: confirmResults.where((bool r) => r).length,
    ));
    return s;
  }
}

void main() {
  testWidgets(
    'full onboarding: create -> read key -> verify wrong -> verify right -> unlocked',
    (WidgetTester tester) async {
      final SpyApi api = SpyApi();
      final Harness h = await pumpApp(tester, api: api);

      // Welcome
      expect(textOf(l10n.welcomeTitle), findsOneWidget);
      await tapOn(tester, buttonText(l10n.welcomeCreate));

      // Create: submit is disabled until the policy passes and both fields match.
      expect(
        tester.widget<FilledButton>(buttonText(l10n.createSubmit)).onPressed,
        isNull,
      );
      await typeInto(tester, l10n.masterPasswordLabel, kPassword);
      await typeInto(tester, l10n.confirmPasswordLabel, kPassword);
      await tapOn(tester, buttonText(l10n.createSubmit));

      // Step E: no reset exists.
      expect(textOf(l10n.explainerTitle), findsOneWidget);
      expect(textOf(l10n.explainerBody), findsOneWidget);
      expect((await api.status()).onboardingComplete, isFalse);
      await tapOn(tester, buttonText(l10n.explainerContinue));

      // Steps F-G: the key.
      expect(textOf(l10n.recoveryTitle), findsOneWidget);
      final List<String> groups = pendingGroups(h);
      expect(groups, hasLength(8));
      expect(textOf(groups.first), findsOneWidget);
      expect(textOf(l10n.recoveryChecksumLabel), findsOneWidget);
      expect(
        tester
            .widget<FilledButton>(buttonText(l10n.recoveryContinue))
            .onPressed,
        isNull,
      );
      await storeKeyAndContinue(tester);

      // Step H, wrong answers: back to the key screen with a message.
      expect(textOf(l10n.verifyTitle), findsOneWidget);
      await typeAnswers(tester, <String>['AAAAA', 'BBBBB', 'CCCCC']);
      await tapOn(tester, buttonText(l10n.verifySubmit));
      expect(textOf(l10n.recoveryTitle), findsOneWidget);
      expect(textOf(l10n.verifyMismatch), findsOneWidget);
      expect(api.confirmResults, <bool>[false]);
      expect((await api.status()).onboardingComplete, isFalse);
      // The core drew new groups on the mismatch; the app asked for them.
      final List<int> challenge = pendingChallenge(h);
      expect(challenge, await api.recoveryChallenge());

      // Step H, right answers (lowercase is fine): unlocked home.
      await storeKeyAndContinue(tester);
      expect(textOf(l10n.verifyTitle), findsOneWidget);
      expect(textOf(l10n.verifyGroupField(challenge[0] + 1)), findsOneWidget);
      await typeAnswers(tester, <String>[
        for (final int i in challenge) groups[i].toLowerCase(),
      ]);
      await tapOn(tester, buttonText(l10n.verifySubmit));

      expect(textOf(l10n.homeUnlockedMessage), findsOneWidget);
      expect(api.confirmResults, <bool>[false, true]);
      expect((await api.status()).onboardingComplete, isTrue);
      // The app dropped the key.
      expect(h.container.read(onboardingProvider).key, isNull);
    },
  );

  testWidgets(
    'onboarding never reads as complete before a successful confirm',
    (WidgetTester tester) async {
      final SpyApi api = SpyApi();
      await pumpApp(tester, api: api);
      await createVaultViaUi(tester);
      await storeKeyAndContinue(tester);
      await typeAnswers(tester, <String>['AAAAA', 'AAAAA', 'AAAAA']);
      await tapOn(tester, buttonText(l10n.verifySubmit));
      for (final ({bool complete, int confirmsSoFar}) entry in api.statusLog) {
        if (entry.complete) {
          expect(entry.confirmsSoFar, greaterThan(0));
        }
      }
      expect(api.confirmResults.contains(true), isFalse);
      expect(textOf(l10n.homeUnlockedMessage), findsNothing);
    },
  );

  testWidgets('verify screen offers no skip and no other way out', (
    WidgetTester tester,
  ) async {
    await pumpApp(tester);
    await createVaultViaUi(tester);
    await storeKeyAndContinue(tester);
    expect(textOf(l10n.verifyTitle), findsOneWidget);
    expect(
      find.textContaining(
        RegExp(r"^\s*(skip|later|not now)\s*$", caseSensitive: false),
      ),
      findsNothing,
    );
    // Every button on the screen: Verify and "Show my key again", nothing else.
    final Iterable<String> labels = find
        .byWidgetPredicate((Widget w) => w is ButtonStyleButton)
        .evaluate()
        .map(
          (Element e) => ((e.widget as ButtonStyleButton).child! as Text).data!,
        );
    expect(labels.toSet(), <String>{
      l10n.verifySubmit,
      l10n.verifyShowKeyAgain,
    });
  });

  testWidgets('"Show my key again" returns to the key screen', (
    WidgetTester tester,
  ) async {
    final Harness h = await pumpApp(tester);
    await createVaultViaUi(tester);
    final List<String> before = pendingGroups(h);
    await storeKeyAndContinue(tester);
    await tapOn(tester, find.text(l10n.verifyShowKeyAgain));
    expect(textOf(l10n.recoveryTitle), findsOneWidget);
    expect(pendingGroups(h), before, reason: 'same key, not a new one');
  });

  testWidgets('quick-unlock offer appears after verify when supported', (
    WidgetTester tester,
  ) async {
    final SpyApi api = SpyApi(quickUnlock: kHello);
    final Harness h = await pumpApp(tester, api: api);
    await createVaultViaUi(tester);
    final List<String> groups = pendingGroups(h);
    final List<int> challenge = pendingChallenge(h);
    await storeKeyAndContinue(tester);
    await typeAnswers(tester, <String>[
      for (final int i in challenge) groups[i],
    ]);
    await tapOn(tester, buttonText(l10n.verifySubmit));

    expect(textOf(l10n.quickOfferTitle), findsOneWidget);
    expect(find.textContaining(l10n.quickKindWindowsHello), findsOneWidget);
    expect(textOf(l10n.homeUnlockedMessage), findsNothing);
    await tapOn(tester, buttonText(l10n.quickEnable));
    expect(textOf(l10n.homeUnlockedMessage), findsOneWidget);
    expect((await api.status()).quickUnlock.enabled, isTrue);
  });

  testWidgets('quick-unlock offer can be declined', (
    WidgetTester tester,
  ) async {
    final SpyApi api = SpyApi(quickUnlock: kHello);
    final Harness h = await pumpApp(tester, api: api);
    await createVaultViaUi(tester);
    final List<String> groups = pendingGroups(h);
    final List<int> challenge = pendingChallenge(h);
    await storeKeyAndContinue(tester);
    await typeAnswers(tester, <String>[
      for (final int i in challenge) groups[i],
    ]);
    await tapOn(tester, buttonText(l10n.verifySubmit));
    await tapOn(tester, find.text(l10n.quickNotNow));
    expect(textOf(l10n.homeUnlockedMessage), findsOneWidget);
    expect((await api.status()).quickUnlock.enabled, isFalse);
  });

  testWidgets('no offer when quick unlock is unsupported', (
    WidgetTester tester,
  ) async {
    final Harness h = await pumpApp(tester);
    await createVaultViaUi(tester);
    final List<String> groups = pendingGroups(h);
    final List<int> challenge = pendingChallenge(h);
    await storeKeyAndContinue(tester);
    await typeAnswers(tester, <String>[
      for (final int i in challenge) groups[i],
    ]);
    await tapOn(tester, buttonText(l10n.verifySubmit));
    expect(textOf(l10n.quickOfferTitle), findsNothing);
    expect(textOf(l10n.homeUnlockedMessage), findsOneWidget);
  });

  testWidgets(
    'unfinished onboarding after a restart: unlock, new recovery key, verify',
    (WidgetTester tester) async {
      final SpyApi api = SpyApi();
      await createThroughApi(api); // created, never confirmed
      await api.lock(); // "restart": the pending key is gone
      final Harness h = await pumpApp(tester, api: api);

      expect(textOf(l10n.lockTitle), findsOneWidget);
      await typeInto(tester, l10n.masterPasswordLabel, kPassword);
      await tapOn(tester, buttonText(l10n.lockSubmit));

      // Unlocked but unconfirmed, no key to show: create a new one.
      expect(textOf(l10n.recoveryRegenTitle), findsOneWidget);
      expect(textOf(l10n.homeUnlockedMessage), findsNothing);
      await typeInto(
        tester,
        l10n.masterPasswordLabel,
        'CANARY-wrong-password-1',
      );
      await tapOn(tester, buttonText(l10n.recoveryRegenSubmit));
      expect(textOf(l10n.errWrongCredentials), findsOneWidget);
      await typeInto(tester, l10n.masterPasswordLabel, kPassword);
      await tapOn(tester, buttonText(l10n.recoveryRegenSubmit));

      expect(textOf(l10n.recoveryTitle), findsOneWidget);
      final List<String> groups = pendingGroups(h);
      final List<int> challenge = pendingChallenge(h);
      await storeKeyAndContinue(tester);
      await typeAnswers(tester, <String>[
        for (final int i in challenge) groups[i],
      ]);
      await tapOn(tester, buttonText(l10n.verifySubmit));
      expect(textOf(l10n.homeUnlockedMessage), findsOneWidget);
      expect((await api.status()).onboardingComplete, isTrue);
    },
  );

  testWidgets('locking mid-onboarding wipes the pending key', (
    WidgetTester tester,
  ) async {
    final SpyApi api = SpyApi();
    final Harness h = await pumpApp(tester, api: api);
    await createVaultViaUi(tester);
    final SecretBytes key = h.container.read(onboardingProvider).key!;
    expect(key.isWiped, isFalse);
    await api.lock();
    await h.container.read(vaultStatusProvider.notifier).refresh();
    await tester.pumpAndSettle();
    expect(key.isWiped, isTrue);
    expect(h.container.read(onboardingProvider).key, isNull);
    expect(textOf(l10n.lockTitle), findsOneWidget);
  });

  testWidgets('back from Create returns to Welcome', (
    WidgetTester tester,
  ) async {
    await pumpApp(tester);
    await tapOn(tester, buttonText(l10n.welcomeCreate));
    await tapOn(tester, find.text(l10n.actionBack));
    expect(textOf(l10n.welcomeTitle), findsOneWidget);
  });
}
