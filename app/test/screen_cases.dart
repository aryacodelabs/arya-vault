import 'dart:math';

import 'package:arya_vault/src/api/dtos.dart';
import 'package:arya_vault/src/api/fake_vault_api.dart';
import 'package:arya_vault/src/state/onboarding_state.dart';
import 'package:arya_vault/src/state/vault_status.dart';
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

import 'helpers.dart';

/// Every distinct screen of B01, reachable in a known state. Used by the
/// accessibility, text-scale, golden and secret-hygiene tests.
const List<String> kScreenNames = <String>[
  'welcome',
  'create',
  'create-with-feedback',
  'explainer',
  'recovery-key',
  'recovery-key-hidden',
  'regenerate-key',
  'verify',
  'quick-offer',
  'lock',
  'lock-quick',
  'lock-with-error',
  'recover-key',
  'recover-password',
  'forgot-both',
  'home',
];

/// Deterministic randomness so golden images are stable.
Random seeded() => Random(20261011);

Future<Harness> pumpScreen(
  WidgetTester tester,
  String name, {
  Size size = const Size(420, 900),
  double textScale = 1,
  ThemeMode themeMode = ThemeMode.light,
}) async {
  Future<Harness> boot(FakeVaultApi api) => pumpApp(
    tester,
    api: api,
    size: size,
    textScale: textScale,
    themeMode: themeMode,
  );

  Future<Harness> onboardingAt(int step, {FakeVaultApi? api}) async {
    final FakeVaultApi a = api ?? newFakeApi(random: seeded());
    final RecoveryKeyResult r = await createThroughApi(a);
    final Harness h = await boot(a);
    final OnboardingNotifier n = h.container.read(onboardingProvider.notifier);
    n.start(r);
    if (step >= 1) n.acknowledgeExplainer();
    if (step >= 2) n.keySavedContinue();
    await tester.pumpAndSettle();
    return h;
  }

  Future<FakeVaultApi> locked({bool quick = false}) async {
    final FakeVaultApi api = newFakeApi(
      random: seeded(),
      quickUnlock: quick ? kHello : QuickUnlockStatus.unsupported,
    );
    await createConfirmedLockedVault(api);
    if (quick) {
      await api.unlock(bytesOf(kPassword));
      await api.quickUnlockEnable();
      await api.lock();
    }
    return api;
  }

  switch (name) {
    case 'welcome':
      return boot(newFakeApi(random: seeded()));
    case 'create':
      final Harness h = await boot(newFakeApi(random: seeded()));
      await tapOn(tester, buttonText(l10n.welcomeCreate));
      return h;
    case 'create-with-feedback':
      final Harness h = await boot(newFakeApi(random: seeded()));
      await tapOn(tester, buttonText(l10n.welcomeCreate));
      await typeInto(tester, l10n.masterPasswordLabel, 'Xq7!mZ2#kLp9');
      await typeInto(tester, l10n.confirmPasswordLabel, 'Xq7!mZ2#kLp8');
      await tapOn(tester, find.text(l10n.advancedTitle));
      return h;
    case 'explainer':
      return onboardingAt(0);
    case 'recovery-key':
      return onboardingAt(1);
    case 'recovery-key-hidden':
      final Harness h = await onboardingAt(1);
      await tapOn(tester, find.text(l10n.recoveryHideKey));
      return h;
    case 'regenerate-key':
      final FakeVaultApi a = newFakeApi(random: seeded());
      await createThroughApi(a);
      return boot(a);
    case 'verify':
      return onboardingAt(2);
    case 'quick-offer':
      final FakeVaultApi a = newFakeApi(random: seeded(), quickUnlock: kHello);
      final RecoveryKeyResult r = await createThroughApi(a);
      final List<String> g = keyGroups(r);
      await a.confirmRecoveryKey(<GroupAnswer>[
        for (final int i in r.challenge) GroupAnswer(index: i, text: g[i]),
      ]);
      final Harness h = await boot(a);
      h.container.read(onboardingProvider.notifier).markQuickOffer(true);
      await h.container.read(vaultStatusProvider.notifier).refresh();
      await tester.pumpAndSettle();
      return h;
    case 'lock':
      return boot(await locked());
    case 'lock-quick':
      return boot(await locked(quick: true));
    case 'lock-with-error':
      final Harness h = await boot(await locked(quick: true));
      await typeInto(
        tester,
        l10n.masterPasswordLabel,
        'CANARY-not-the-password',
      );
      await tapOn(tester, buttonText(l10n.lockSubmit));
      return h;
    case 'recover-key':
      final Harness h = await boot(await locked());
      await tapOn(tester, find.text(l10n.lockUseRecovery));
      return h;
    case 'recover-password':
      final Harness h = await boot(await locked());
      await tapOn(tester, find.text(l10n.lockUseRecovery));
      await typeInto(tester, l10n.recoverKeyLabel, 'CANARY-key');
      await tapOn(tester, buttonText(l10n.actionContinue));
      return h;
    case 'forgot-both':
      final Harness h = await boot(await locked());
      await tapOn(tester, find.text(l10n.lockUseRecovery));
      await tapOn(tester, find.text(l10n.recoverNoKey));
      return h;
    case 'home':
      final FakeVaultApi a = newFakeApi(random: seeded());
      final RecoveryKeyResult r = await createThroughApi(a);
      final List<String> g = keyGroups(r);
      await a.confirmRecoveryKey(<GroupAnswer>[
        for (final int i in r.challenge) GroupAnswer(index: i, text: g[i]),
      ]);
      return boot(a);
  }
  throw ArgumentError.value(name, 'name', 'unknown screen');
}
