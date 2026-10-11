import 'dart:async';

import 'package:flutter/services.dart';
import 'package:flutter/widgets.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../api/dtos.dart';
import '../../services/auto_lock.dart';
import '../../services/services.dart';
import '../../state/providers.dart';
import '../../state/security_services.dart';
import '../../state/vault_session.dart';
import '../../state/vault_status.dart';

/// Sits above the whole app and runs the security services (SEC-A04, SEC-H02):
///
/// - feeds pointer and key input to the auto-lock controller and arms it while
///   the vault is unlocked;
/// - forwards OS events (sleep, screen lock, minimize) to it;
/// - applies the vault settings to the controller and to screen-capture
///   protection, which is **on until the settings say otherwise**.
class SecurityScope extends ConsumerStatefulWidget {
  const SecurityScope({required this.child, super.key});

  final Widget child;

  @override
  ConsumerState<SecurityScope> createState() => _SecurityScopeState();
}

class _SecurityScopeState extends ConsumerState<SecurityScope>
    with WidgetsBindingObserver {
  StreamSubscription<PlatformEvent>? _events;

  @override
  void initState() {
    super.initState();
    WidgetsBinding.instance.addObserver(this);
    HardwareKeyboard.instance.addHandler(_onKey);
    _events = ref
        .read(platformEventsProvider)
        .events
        .listen(
          (PlatformEvent e) =>
              unawaited(ref.read(autoLockProvider).onPlatformEvent(e)),
        );
    // Fail safe: blocked from the first frame, until the settings are known.
    _applyCapture(true);
    ref.listenManual(vaultStatusProvider, (_, next) => _syncArmed());
    ref.listenManual(appSettingsProvider, (
      AsyncValue<AppSettings>? _,
      AsyncValue<AppSettings> next,
    ) {
      final AppSettings? s = next.value;
      if (s == null) return;
      ref.read(autoLockProvider).configure(autoLockConfigFor(s));
      _applyCapture(s.blockScreenCapture);
    });
    WidgetsBinding.instance.addPostFrameCallback((_) => _syncArmed());
  }

  @override
  void dispose() {
    unawaited(_events?.cancel());
    HardwareKeyboard.instance.removeHandler(_onKey);
    WidgetsBinding.instance.removeObserver(this);
    super.dispose();
  }

  bool _onKey(KeyEvent event) {
    ref.read(autoLockProvider).activity();
    return false; // never consume the event
  }

  void _syncArmed() {
    if (!mounted) return;
    final AutoLockController lock = ref.read(autoLockProvider);
    final VaultStatus? status = ref.read(vaultStatusProvider).value;
    final bool unlocked = status != null && status.exists && !status.locked;
    if (unlocked && !lock.armed) {
      lock.configure(autoLockConfigFor(ref.read(appSettingsProvider).value));
      lock.arm();
    } else if (!unlocked && lock.armed) {
      lock.disarm();
    }
  }

  void _applyCapture(bool blocked) {
    final CaptureProtection capture = ref.read(captureProtectionProvider);
    unawaited(
      capture.setBlocked(blocked).catchError((Object _) {
        // A platform that cannot block capture must not break the app.
      }),
    );
  }

  @override
  void didChangeAppLifecycleState(AppLifecycleState state) {
    final AutoLockController lock = ref.read(autoLockProvider);
    switch (state) {
      case AppLifecycleState.hidden:
        unawaited(lock.onPlatformEvent(PlatformEvent.windowMinimized));
      case AppLifecycleState.resumed:
        unawaited(lock.onPlatformEvent(PlatformEvent.windowRestored));
      case AppLifecycleState.inactive:
      case AppLifecycleState.paused:
      case AppLifecycleState.detached:
        break;
    }
  }

  @override
  Widget build(BuildContext context) {
    void touch(PointerEvent _) => ref.read(autoLockProvider).activity();
    return Listener(
      behavior: HitTestBehavior.translucent,
      onPointerDown: touch,
      onPointerMove: touch,
      onPointerHover: touch,
      onPointerSignal: touch,
      child: widget.child,
    );
  }
}
