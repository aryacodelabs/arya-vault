import 'dart:async';

import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../api/app_error.dart';
import '../api/dtos.dart';
import 'providers.dart';
import 'vault_status.dart';

/// Increments every time the vault locks. Providers that hold vault data watch
/// it, so a lock makes them drop what they hold (docs/14 §5, B02 item 9).
class LockEpochNotifier extends Notifier<int> {
  @override
  int build() {
    ref.listen(vaultStatusProvider, (
      AsyncValue<VaultStatus>? previous,
      AsyncValue<VaultStatus> next,
    ) {
      final VaultStatus? s = next.value;
      final bool wasLocked =
          previous?.value == null ||
          previous!.value!.locked ||
          !previous.value!.exists;
      // Only the transition counts: re-reading a status that is still "locked"
      // must not make the data providers rebuild again.
      if (s != null && (s.locked || !s.exists) && !wasLocked) {
        state = state + 1;
      }
    });
    return 0;
  }
}

final NotifierProvider<LockEpochNotifier, int> lockEpochProvider =
    NotifierProvider<LockEpochNotifier, int>(LockEpochNotifier.new);

/// True once the app already knows the vault is locked (or gone).
bool _knownLocked(AsyncValue<VaultStatus> status) {
  final VaultStatus? s = status.value;
  return s != null && (s.locked || !s.exists);
}

/// If [error] says the session is locked, re-read the status so the router
/// goes to the lock screen. Returns true if it was a `locked` error. Does
/// nothing more once the status already says locked (no refresh loops).
bool handleLocked(Ref ref, Object error) {
  if (error is AppError && error.code == AppErrorCode.locked) {
    if (!_knownLocked(ref.read(vaultStatusProvider))) {
      unawaited(ref.read(vaultStatusProvider.notifier).refresh());
    }
    return true;
  }
  return false;
}

/// Whether the vault is known to be locked; data providers skip the API then.
bool sessionKnownLocked(Ref ref) => _knownLocked(ref.read(vaultStatusProvider));

/// [handleLocked] for widget code.
bool handleLockedWidget(WidgetRef ref, Object error) {
  if (error is AppError && error.code == AppErrorCode.locked) {
    if (!_knownLocked(ref.read(vaultStatusProvider))) {
      unawaited(ref.read(vaultStatusProvider.notifier).refresh());
    }
    return true;
  }
  return false;
}

/// Vault-level settings the screens need (reveal and clipboard timings).
/// Falls back to the defaults if they cannot be read.
final FutureProvider<AppSettings> appSettingsProvider =
    FutureProvider<AppSettings>((Ref ref) async {
      ref.watch(lockEpochProvider);
      try {
        return await ref.read(vaultApiProvider).getSettings();
      } on AppError catch (e) {
        handleLocked(ref, e);
        return const AppSettings();
      }
    });

/// How long a revealed secret stays visible: the setting, never above 15 s (SEC-H03).
Duration revealDuration(AppSettings? settings) {
  final int seconds = (settings?.revealHideSeconds ?? 15).clamp(1, 15);
  return Duration(seconds: seconds);
}
