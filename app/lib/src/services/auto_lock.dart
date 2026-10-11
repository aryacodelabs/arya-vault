import 'dart:async';

import 'services.dart';
import 'time_source.dart';

/// Why the vault was locked automatically.
enum AutoLockReason { timeout, sleep, screenLocked, windowMinimized }

/// The user's auto-lock choices (docs/07 §7; there is deliberately no "never").
class AutoLockConfig {
  const AutoLockConfig({
    this.timeout = const Duration(minutes: 5),
    this.lockOnSleep = true,
    this.lockOnScreenLock = true,
    this.lockOnMinimize = false,
  });

  final Duration timeout;
  final bool lockOnSleep;
  final bool lockOnScreenLock;

  /// App-level preference; not part of the vault settings (docs/14 §4.5).
  final bool lockOnMinimize;
}

/// Locks the vault on inactivity and on OS events (SEC-A04). Pure logic: the
/// owner arms it while the vault is unlocked, feeds it user input and
/// [PlatformEvent]s, and gives it the function that really locks.
///
/// Inactivity is measured from the last user input. Input is reported often
/// (every pointer move), so it records the time but re-arms the timer at most
/// once per [_rearmEvery]: the lock can come up to a second early, never late.
class AutoLockController {
  AutoLockController({
    required this._time,
    required this._lock,
    this._config = const AutoLockConfig(),
    this._retryAfter = const Duration(seconds: 5),
  });

  static const Duration _rearmEvery = Duration(seconds: 1);

  final TimeSource _time;
  final Future<void> Function(AutoLockReason reason) _lock;
  final Duration _retryAfter;
  AutoLockConfig _config;

  Timer? _timer;
  bool _armed = false;
  bool _locking = false;
  bool _disposed = false;
  DateTime _lastActivity = DateTime.fromMillisecondsSinceEpoch(0);
  DateTime _lastRearm = DateTime.fromMillisecondsSinceEpoch(0);

  bool get armed => _armed;
  AutoLockConfig get config => _config;

  /// True while a countdown or a retry is scheduled.
  bool get hasPendingTimer => _timer?.isActive ?? false;

  /// The vault is unlocked: start counting from now.
  void arm() {
    if (_disposed) return;
    _armed = true;
    _lastActivity = _time.now();
    _schedule(_config.timeout);
  }

  /// The vault is locked (by anyone): stop everything.
  void disarm() {
    _armed = false;
    _timer?.cancel();
    _timer = null;
  }

  /// New settings while armed. The timeout counts from the last input, so a
  /// shorter value can lock sooner but never later than it would have.
  void configure(AutoLockConfig config) {
    _config = config;
    if (!_armed || _locking) return;
    _scheduleFromLastActivity();
  }

  /// A pointer or key event.
  void activity() {
    if (!_armed || _locking) return;
    final DateTime now = _time.now();
    _lastActivity = now;
    if (now.difference(_lastRearm) >= _rearmEvery) {
      _scheduleFromLastActivity();
    }
  }

  /// An OS event. Returns when a resulting lock has finished.
  Future<void> onPlatformEvent(PlatformEvent event) async {
    if (!_armed) return;
    switch (event) {
      case PlatformEvent.sleep:
        if (_config.lockOnSleep) await _lockNow(AutoLockReason.sleep);
      case PlatformEvent.screenLocked:
        if (_config.lockOnScreenLock) {
          await _lockNow(AutoLockReason.screenLocked);
        }
      case PlatformEvent.windowMinimized:
        if (_config.lockOnMinimize) {
          await _lockNow(AutoLockReason.windowMinimized);
        }
      case PlatformEvent.resume:
      case PlatformEvent.screenUnlocked:
      case PlatformEvent.windowRestored:
        // Timers can be late or paused while the machine slept: re-check the
        // deadline against the clock instead of trusting the timer.
        if (_time.now().difference(_lastActivity) >= _config.timeout) {
          await _lockNow(AutoLockReason.timeout);
        } else {
          _scheduleFromLastActivity();
        }
    }
  }

  void dispose() {
    _disposed = true;
    disarm();
  }

  void _scheduleFromLastActivity() {
    final Duration remaining =
        _config.timeout - _time.now().difference(_lastActivity);
    _schedule(remaining.isNegative ? Duration.zero : remaining);
  }

  void _schedule(Duration delay) {
    _timer?.cancel();
    _lastRearm = _time.now();
    _timer = _time.timer(
      delay,
      () => unawaited(_lockNow(AutoLockReason.timeout)),
    );
  }

  Future<void> _lockNow(AutoLockReason reason) async {
    if (!_armed || _locking || _disposed) return;
    _locking = true;
    _timer?.cancel();
    _timer = null;
    try {
      await _lock(reason);
      // The owner disarms us when the status flips to locked; do it here too so
      // a late timer can never fire a second lock.
      _armed = false;
    } on Object {
      // The lock did not go through: try again soon rather than stay unlocked.
      if (_armed && !_disposed) {
        _timer = _time.timer(
          _retryAfter,
          () => unawaited(_lockNow(AutoLockReason.timeout)),
        );
      }
    } finally {
      _locking = false;
    }
  }
}
