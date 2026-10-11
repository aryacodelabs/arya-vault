import 'dart:typed_data';

import 'package:arya_vault/src/api/dtos.dart';
import 'package:arya_vault/src/api/secret_bytes.dart';
import 'package:arya_vault/src/state/onboarding_state.dart';
import 'package:arya_vault/src/state/routing.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';

VaultStatus status({
  bool exists = true,
  bool locked = false,
  bool complete = true,
  QuickUnlockStatus quick = QuickUnlockStatus.unsupported,
}) => VaultStatus(
  exists: exists,
  locked: locked,
  onboardingComplete: complete,
  formatVersion: 1,
  quickUnlock: quick,
);

OnboardingState withKey({
  bool explainerSeen = false,
  bool keySaved = false,
  bool offer = false,
}) => OnboardingState(
  key: SecretBytes(Uint8List.fromList(<int>[65])),
  groups: 8,
  challenge: const <int>[0, 1, 2],
  explainerSeen: explainerSeen,
  keySaved: keySaved,
  quickOfferPending: offer,
);

String? at(AsyncValue<VaultStatus> s, OnboardingState o, String location) =>
    redirectFor(status: s, onboarding: o, location: location);

void main() {
  const OnboardingState none = OnboardingState();

  test('loading goes to splash, failure to error', () {
    expect(
      at(const AsyncLoading<VaultStatus>(), none, AppRoutes.home),
      AppRoutes.splash,
    );
    expect(
      at(const AsyncLoading<VaultStatus>(), none, AppRoutes.splash),
      isNull,
    );
    final AsyncValue<VaultStatus> failed = AsyncError<VaultStatus>(
      Exception('x'),
      StackTrace.empty,
    );
    expect(at(failed, none, AppRoutes.splash), AppRoutes.error);
    expect(at(failed, none, AppRoutes.error), isNull);
  });

  test('no vault -> welcome; welcome and create are allowed', () {
    final AsyncValue<VaultStatus> s = AsyncData<VaultStatus>(
      status(exists: false),
    );
    expect(at(s, none, AppRoutes.splash), AppRoutes.welcome);
    expect(at(s, none, AppRoutes.home), AppRoutes.welcome);
    expect(at(s, none, AppRoutes.lock), AppRoutes.welcome);
    expect(at(s, none, AppRoutes.welcome), isNull);
    expect(at(s, none, AppRoutes.create), isNull);
  });

  test('vault + locked -> lock; lock, recover, forgot are allowed', () {
    final AsyncValue<VaultStatus> s = AsyncData<VaultStatus>(
      status(locked: true),
    );
    expect(at(s, none, AppRoutes.splash), AppRoutes.lock);
    expect(at(s, none, AppRoutes.home), AppRoutes.lock);
    expect(at(s, none, AppRoutes.welcome), AppRoutes.lock);
    for (final String ok in <String>[
      AppRoutes.lock,
      AppRoutes.recover,
      AppRoutes.forgot,
    ]) {
      expect(at(s, none, ok), isNull);
    }
  });

  test('unlocked + complete -> home', () {
    final AsyncValue<VaultStatus> s = AsyncData<VaultStatus>(status());
    expect(at(s, none, AppRoutes.splash), AppRoutes.home);
    expect(at(s, none, AppRoutes.lock), AppRoutes.home);
    expect(at(s, none, AppRoutes.home), isNull);
  });

  test('the quick-unlock offer comes before home, once', () {
    final AsyncValue<VaultStatus> s = AsyncData<VaultStatus>(status());
    expect(at(s, withKey(offer: true), AppRoutes.home), AppRoutes.quickOffer);
    expect(at(s, withKey(offer: true), AppRoutes.quickOffer), isNull);
    expect(at(s, none, AppRoutes.quickOffer), AppRoutes.home);
  });

  group('onboarding incomplete (SEC-A01)', () {
    final AsyncValue<VaultStatus> s = AsyncData<VaultStatus>(
      status(complete: false),
    );

    test('steps are strictly ordered: explainer, key, verify', () {
      expect(at(s, withKey(), AppRoutes.create), AppRoutes.explainer);
      expect(
        at(s, withKey(explainerSeen: true), AppRoutes.explainer),
        AppRoutes.recoveryKey,
      );
      expect(
        at(s, withKey(explainerSeen: true), AppRoutes.recoveryKey),
        isNull,
      );
      expect(
        at(
          s,
          withKey(explainerSeen: true, keySaved: true),
          AppRoutes.recoveryKey,
        ),
        AppRoutes.verify,
      );
      expect(
        at(s, withKey(explainerSeen: true, keySaved: true), AppRoutes.verify),
        isNull,
      );
    });

    test('without a pending key only the key screen (new key form) exists', () {
      expect(at(s, none, AppRoutes.home), AppRoutes.recoveryKey);
      expect(at(s, none, AppRoutes.recoveryKey), isNull);
    });

    test('NO location other than the single current step is reachable', () {
      final List<OnboardingState> states = <OnboardingState>[
        none,
        withKey(),
        withKey(explainerSeen: true),
        withKey(explainerSeen: true, keySaved: true),
        withKey(offer: true),
        withKey(explainerSeen: true, keySaved: true, offer: true),
      ];
      const Set<String> onboardingRoutes = <String>{
        AppRoutes.explainer,
        AppRoutes.recoveryKey,
        AppRoutes.verify,
      };
      for (final OnboardingState o in states) {
        final List<String> reachable = <String>[
          for (final String location in AppRoutes.all)
            if (at(s, o, location) == null) location,
        ];
        expect(reachable, hasLength(1), reason: 'state $o');
        expect(onboardingRoutes, contains(reachable.single));
        expect(reachable.single, isNot(AppRoutes.home));
        expect(reachable.single, isNot(AppRoutes.quickOffer));
      }
    });
  });
}
