import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../api/dtos.dart';
import '../api/secret_bytes.dart';
import 'vault_status.dart';

/// Where the user is in onboarding (docs/07 §3, steps E-I).
///
/// Holds the pending recovery key (as wipeable bytes) from `createVault` until
/// `confirmRecoveryKey` passes, the app locks, or the provider is disposed. The
/// key cannot be fetched again from the core, and a failed verification must
/// re-show it (docs/14 §4.1), so the app has to keep it that long. Rendering it
/// makes a `String` that Dart cannot wipe (see SecretBytes).
class OnboardingState {
  const OnboardingState({
    this.key,
    this.groups = 0,
    this.challenge = const <int>[],
    this.explainerSeen = false,
    this.keySaved = false,
    this.verifyFailed = false,
    this.quickOfferPending = false,
  });

  /// The pending recovery key text, or null (never created in this run, or
  /// already confirmed, or wiped by a lock).
  final SecretBytes? key;
  final int groups;

  /// Zero-based groups `confirmRecoveryKey` asks for.
  final List<int> challenge;
  final bool explainerSeen;

  /// The user ticked "I stored it" and continued to verification.
  final bool keySaved;

  /// Last verification failed; the key screen says so once.
  final bool verifyFailed;

  /// Quick unlock is supported and not yet enabled: offer it once.
  final bool quickOfferPending;

  OnboardingState copyWith({
    bool? explainerSeen,
    bool? keySaved,
    bool? verifyFailed,
    bool? quickOfferPending,
  }) => OnboardingState(
    key: key,
    groups: groups,
    challenge: challenge,
    explainerSeen: explainerSeen ?? this.explainerSeen,
    keySaved: keySaved ?? this.keySaved,
    verifyFailed: verifyFailed ?? this.verifyFailed,
    quickOfferPending: quickOfferPending ?? this.quickOfferPending,
  );
}

class OnboardingNotifier extends Notifier<OnboardingState> {
  /// The live key, tracked outside `state` because Riverpod forbids reading
  /// `state` while the provider is being disposed.
  SecretBytes? _live;

  void _setKey(SecretBytes? key) {
    _live?.wipe();
    _live = key;
  }

  @override
  OnboardingState build() {
    ref.onDispose(() => _live?.wipe());
    // A lock drops the core's pending key; drop ours too.
    ref.listen(vaultStatusProvider, (
      AsyncValue<VaultStatus>? previous,
      AsyncValue<VaultStatus> next,
    ) {
      final VaultStatus? s = next.value;
      if (s != null && (s.locked || !s.exists)) {
        clearKey();
      }
    });
    return const OnboardingState();
  }

  /// Starts (or restarts) the key steps with a fresh key from the core.
  /// [explainerSeen] is true when the user already read the "no reset" text,
  /// for example when regenerating after an unfinished onboarding.
  void start(RecoveryKeyResult result, {bool explainerSeen = false}) {
    final SecretBytes key = SecretBytes(result.recoveryKey);
    _setKey(key);
    state = OnboardingState(
      key: key,
      groups: result.groups,
      challenge: List<int>.unmodifiable(result.challenge),
      explainerSeen: explainerSeen,
    );
  }

  void acknowledgeExplainer() => state = state.copyWith(explainerSeen: true);

  /// "I stored it" + Continue.
  void keySavedContinue() =>
      state = state.copyWith(keySaved: true, verifyFailed: false);

  /// Back to the key screen, optionally flagging a failed verification.
  void backToKey({bool failed = false}) =>
      state = state.copyWith(keySaved: false, verifyFailed: failed);

  /// Verification passed: the key is no longer needed anywhere.
  void finish({required bool offerQuickUnlock}) {
    _setKey(null);
    state = OnboardingState(quickOfferPending: offerQuickUnlock);
  }

  /// Records, before the status flips to complete, whether to offer quick
  /// unlock next, so the router never sees a half-updated picture.
  void markQuickOffer(bool pending) =>
      state = state.copyWith(quickOfferPending: pending);

  void dismissQuickOffer() => state = state.copyWith(quickOfferPending: false);

  /// Wipes the pending key and returns to a blank state (lock, reset).
  void clearKey() {
    _setKey(null);
    state = OnboardingState(quickOfferPending: state.quickOfferPending);
  }
}

final NotifierProvider<OnboardingNotifier, OnboardingState> onboardingProvider =
    NotifierProvider<OnboardingNotifier, OnboardingState>(
      OnboardingNotifier.new,
    );
