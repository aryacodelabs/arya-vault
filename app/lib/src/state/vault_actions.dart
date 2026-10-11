import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../api/dtos.dart';
import '../api/secret_bytes.dart';
import 'onboarding_state.dart';
import 'providers.dart';
import 'vault_status.dart';

/// Lifecycle operations the screens call. Each wipes the secrets it is given
/// (through [SecretBytes.useAsync]) and keeps `VaultStatus` and onboarding
/// state in step so the router sees a consistent picture. Errors are rethrown
/// as `AppError` for the screen to render.
class VaultActions {
  VaultActions(this._ref);
  final Ref _ref;

  Future<void> createVault(SecretBytes password, KdfProfile profile) async {
    final RecoveryKeyResult result = await password.useAsync(
      (bytes) => _ref
          .read(vaultApiProvider)
          .createVault(password: bytes, profile: profile),
    );
    _ref.read(onboardingProvider.notifier).start(result);
    await _ref.read(vaultStatusProvider.notifier).refresh();
  }

  /// Sends the answers; true means onboarding is complete.
  Future<bool> confirmRecoveryKey(List<GroupAnswer> answers) async {
    final bool ok = await _ref
        .read(vaultApiProvider)
        .confirmRecoveryKey(answers);
    if (!ok) {
      _ref.read(onboardingProvider.notifier).backToKey(failed: true);
      return false;
    }
    final VaultStatus status = await _ref.read(vaultApiProvider).status();
    final bool offer =
        status.quickUnlock.supported && !status.quickUnlock.enabled;
    final OnboardingNotifier onboarding = _ref.read(
      onboardingProvider.notifier,
    );
    // Order matters: the router must never see "complete" without the offer
    // flag, nor "incomplete" without the key.
    onboarding.markQuickOffer(offer);
    _ref.read(vaultStatusProvider.notifier).publish(status);
    onboarding.finish(offerQuickUnlock: offer);
    return true;
  }

  /// After an unfinished onboarding the pending key is gone; this creates a
  /// new one (docs/14 §4.1 `regenerateRecoveryKey`).
  Future<void> regenerateRecoveryKey(SecretBytes password) async {
    final RecoveryKeyResult result = await password.useAsync(
      (bytes) => _ref
          .read(vaultApiProvider)
          .regenerateRecoveryKey(password: bytes, rotateKeys: false),
    );
    _ref.read(onboardingProvider.notifier).start(result, explainerSeen: true);
    await _ref.read(vaultStatusProvider.notifier).refresh();
  }

  Future<void> unlock(SecretBytes password) async {
    await password.useAsync(_ref.read(vaultApiProvider).unlock);
    await _ref.read(vaultStatusProvider.notifier).refresh();
  }

  Future<void> unlockQuick() async {
    try {
      await _ref.read(vaultApiProvider).unlockQuick();
    } finally {
      // quickUnlockUnavailable may mean the policy expired: re-read status.
      await _ref.read(vaultStatusProvider.notifier).refresh();
    }
  }

  Future<void> lock() async {
    await _ref.read(vaultApiProvider).lock();
    _ref.read(onboardingProvider.notifier).clearKey();
    await _ref.read(vaultStatusProvider.notifier).refresh();
  }

  Future<void> recoverWithKey(
    SecretBytes recoveryKey,
    SecretBytes newPassword,
  ) async {
    try {
      await recoveryKey.useAsync(
        (keyBytes) => newPassword.useAsync(
          (pwBytes) => _ref
              .read(vaultApiProvider)
              .recoverWithKey(recoveryKey: keyBytes, newPassword: pwBytes),
        ),
      );
    } finally {
      recoveryKey.wipe();
      newPassword.wipe();
    }
    await _ref.read(vaultStatusProvider.notifier).refresh();
  }

  /// Turns quick unlock on (onboarding offer); false if the platform refused.
  Future<void> enableQuickUnlock() async {
    await _ref.read(vaultApiProvider).quickUnlockEnable();
    _ref.read(onboardingProvider.notifier).dismissQuickOffer();
    await _ref.read(vaultStatusProvider.notifier).refresh();
  }

  void skipQuickUnlockOffer() =>
      _ref.read(onboardingProvider.notifier).dismissQuickOffer();
}

final Provider<VaultActions> vaultActionsProvider = Provider<VaultActions>(
  VaultActions.new,
);
