import 'dart:async';

import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../api/dtos.dart';
import '../services/auto_lock.dart';
import '../services/clipboard_manager.dart';
import 'providers.dart';
import 'vault_actions.dart';
import 'vault_session.dart';

/// The clipboard logic (timed, "only if still ours" clear). It follows the
/// clipboard-clear setting and clears at once when the vault locks.
final Provider<ClipboardManager> clipboardManagerProvider =
    Provider<ClipboardManager>((Ref ref) {
      final AppSettings? initial = ref.read(appSettingsProvider).value;
      final ClipboardManager manager = ClipboardManager(
        platform: ref.watch(clipboardPlatformProvider),
        time: ref.watch(timeSourceProvider),
        clearAfter: Duration(seconds: initial?.clipboardClearSeconds ?? 30),
      );
      ref.listen(appSettingsProvider, (
        AsyncValue<AppSettings>? _,
        AsyncValue<AppSettings> next,
      ) {
        final AppSettings? s = next.value;
        if (s != null) {
          manager.setClearAfter(Duration(seconds: s.clipboardClearSeconds));
        }
      });
      ref.listen(lockEpochProvider, (int? _, int _) {
        unawaited(manager.onLock());
      });
      ref.onDispose(manager.dispose);
      return manager;
    });

/// Maps the vault settings to the auto-lock controller's configuration.
AutoLockConfig autoLockConfigFor(AppSettings? s) => s == null
    ? const AutoLockConfig()
    : AutoLockConfig(
        timeout: Duration(minutes: s.autoLockMinutes),
        lockOnSleep: s.lockOnSleep,
        lockOnScreenLock: s.lockOnScreenLock,
      );

/// The auto-lock controller. `SecurityScope` arms it and feeds it input.
final Provider<AutoLockController> autoLockProvider =
    Provider<AutoLockController>((Ref ref) {
      final AutoLockController controller = AutoLockController(
        time: ref.watch(timeSourceProvider),
        lock: (AutoLockReason reason) => ref.read(vaultActionsProvider).lock(),
        config: autoLockConfigFor(ref.read(appSettingsProvider).value),
      );
      ref.onDispose(controller.dispose);
      return controller;
    });
