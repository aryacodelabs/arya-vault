import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../api/app_error.dart';
import '../api/dtos.dart';
import '../api/secret_bytes.dart';
import 'providers.dart';
import 'vault_actions.dart';
import 'vault_session.dart';
import 'vault_status.dart';

/// The vault settings for the editing screens. Unlike `appSettingsProvider`
/// (which falls back to defaults so the rest of the app keeps working), a
/// failed read is an error here: writing defaults over real settings by
/// accident would be worse than showing "try again".
class SettingsEditor extends AsyncNotifier<AppSettings> {
  @override
  Future<AppSettings> build() async {
    ref.watch(lockEpochProvider);
    try {
      return await ref.read(vaultApiProvider).getSettings();
    } on AppError catch (e) {
      handleLocked(ref, e);
      rethrow;
    }
  }

  /// Saves [next] and shows what the core really stored (it clamps to the
  /// allowed ranges, docs/14 §4.5). Throws `AppError`.
  Future<void> save(AppSettings next) async {
    final api = ref.read(vaultApiProvider);
    await api.setSettings(next);
    final AppSettings stored = await api.getSettings();
    state = AsyncData<AppSettings>(stored);
    ref.invalidate(appSettingsProvider);
  }
}

final AsyncNotifierProvider<SettingsEditor, AppSettings>
settingsEditorProvider = AsyncNotifierProvider<SettingsEditor, AppSettings>(
  SettingsEditor.new,
);

/// Security actions behind the settings screens (re-authentication, quick
/// unlock, password and recovery-key changes). Each wipes the secrets it gets.
class SecurityActions {
  SecurityActions(this._ref);
  final Ref _ref;

  /// Re-authentication: true if [password] is the master password. Wipes it.
  Future<bool> verifyPassword(SecretBytes password) => password.useAsync(
    (bytes) => _ref.read(vaultApiProvider).verifyPassword(bytes),
  );

  /// Turns quick unlock on. The caller has already re-authenticated.
  Future<void> enableQuickUnlock() async {
    await _ref.read(vaultApiProvider).quickUnlockEnable();
    await _ref.read(vaultStatusProvider.notifier).refresh();
  }

  Future<void> disableQuickUnlock() async {
    await _ref.read(vaultApiProvider).quickUnlockDisable();
    await _ref.read(vaultStatusProvider.notifier).refresh();
  }

  /// Changes the master password. With [rotateKeys] the core also replaces the
  /// vault key and leaves a new recovery key pending (`onboardingComplete`
  /// false). The contract's `changePassword` returns nothing, so the key to
  /// show comes from `regenerateRecoveryKey` (FFI note on `change_password`,
  /// spec question 4); the router then moves to the recovery-key steps.
  Future<void> changePassword({
    required SecretBytes oldPassword,
    required SecretBytes newPassword,
    required bool rotateKeys,
  }) async {
    final SecretBytes forKey = SecretBytes(newPassword.copyBytes());
    try {
      await oldPassword.useAsync(
        (old) => newPassword.useAsync(
          (fresh) => _ref
              .read(vaultApiProvider)
              .changePassword(
                oldPassword: old,
                newPassword: fresh,
                rotateKeys: rotateKeys,
              ),
        ),
      );
      if (rotateKeys) {
        try {
          await _ref.read(vaultActionsProvider).regenerateRecoveryKey(forKey);
        } finally {
          // Onboarding is pending in the core now; make the router see it even
          // if fetching the key failed (the key screen then offers a new one).
          await _ref.read(vaultStatusProvider.notifier).refresh();
        }
      }
    } finally {
      forKey.wipe();
      oldPassword.wipe();
      newPassword.wipe();
    }
  }

  /// "Regenerate recovery key": re-authenticates inside the core, then the
  /// router shows the key steps (docs/07 §5).
  Future<void> regenerateRecoveryKey(SecretBytes password) =>
      _ref.read(vaultActionsProvider).regenerateRecoveryKey(password);
}

final Provider<SecurityActions> securityActionsProvider =
    Provider<SecurityActions>(SecurityActions.new);
