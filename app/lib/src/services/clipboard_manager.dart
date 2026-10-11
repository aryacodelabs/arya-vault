import 'dart:async';
import 'dart:typed_data';

import '../api/secret_bytes.dart';
import 'services.dart';
import 'time_source.dart';

/// Copies secrets to the clipboard and clears them again (SEC-H01, docs/07 §7).
///
/// - A copy is written with `sensitive: true`; the buffer is wiped right after.
/// - After N seconds (the setting) the clipboard is cleared **only if it still
///   holds what we wrote**: the platform fingerprints the content, so something
///   the user copied since is left alone.
/// - A lock clears at once (same "only if ours" rule) and cancels the timer.
/// - If the platform cannot fingerprint (it returns null right after our
///   write) the manager cannot tell whether the content is still ours and
///   clears unconditionally: leaving a secret behind is the worse failure.
class ClipboardManager {
  ClipboardManager({
    required this._platform,
    required this._time,
    required this._clearAfter,
  });

  final ClipboardPlatform _platform;
  final TimeSource _time;
  Duration _clearAfter;

  Timer? _timer;
  DateTime? _copiedAt;

  /// Fingerprint of what we wrote; null with [_pending] means "cannot tell".
  String? _fingerprint;
  bool _pending = false;

  /// Bumped by every copy and every lock, so work that finishes late can tell
  /// it was superseded.
  int _generation = 0;
  int _lockGeneration = 0;
  bool _disposed = false;

  /// True while a copy of ours may still be on the clipboard.
  bool get hasPending => _pending;

  Duration get clearAfter => _clearAfter;

  /// Copies [secret], then wipes it (the manager takes ownership). The toast
  /// text is the caller's job and never contains the value.
  ///
  /// Throws what the platform throws (`ServiceUnavailable`); in that case no
  /// clear is scheduled and an earlier pending copy stays pending.
  Future<void> copySecret(SecretBytes secret) async {
    final int lockAtStart = _lockGeneration;
    final int generation = ++_generation;
    String? fingerprint;
    try {
      await secret.useAsync((Uint8List bytes) async {
        await _platform.write(bytes, sensitive: true);
      });
      fingerprint = await _platform.contentFingerprint();
    } on Object {
      if (_generation == generation) {
        _generation--;
      }
      rethrow;
    }
    if (_disposed || _generation != generation) {
      // A newer copy took over while we were fingerprinting; it owns the timer.
      return;
    }
    _timer?.cancel();
    _pending = true;
    _fingerprint = fingerprint;
    _copiedAt = _time.now();
    if (_lockGeneration != lockAtStart) {
      // The vault locked while this copy was being written: do not leave it.
      await _clearIfOurs();
      return;
    }
    _arm(_clearAfter);
  }

  /// [copySecret] for a value the caller only has as text.
  Future<void> copyText(String text) =>
      copySecret(SecretBytes.fromString(text));

  /// The "clear after" setting changed. A pending copy is re-timed from when it
  /// was copied, so lowering N never leaves a secret around longer than the new
  /// value and raising it does not strand an already-expired copy.
  void setClearAfter(Duration value) {
    _clearAfter = value;
    if (!_pending || _copiedAt == null) {
      return;
    }
    final Duration elapsed = _time.now().difference(_copiedAt!);
    final Duration remaining = value - elapsed;
    _arm(remaining.isNegative ? Duration.zero : remaining);
  }

  /// The vault locked: clear now if the content is ours.
  Future<void> onLock() async {
    _lockGeneration++;
    _timer?.cancel();
    _timer = null;
    await _clearIfOurs();
  }

  void dispose() {
    _disposed = true;
    _timer?.cancel();
    _timer = null;
  }

  void _arm(Duration delay) {
    _timer?.cancel();
    _timer = _time.timer(delay, () => unawaited(_clearIfOurs()));
  }

  Future<void> _clearIfOurs() async {
    if (!_pending) {
      return;
    }
    final int generation = _generation;
    final String? expected = _fingerprint;
    _timer?.cancel();
    _timer = null;
    bool ours;
    try {
      ours =
          expected == null ||
          (await _platform.contentFingerprint()) == expected;
    } on Object {
      ours = true; // cannot verify: err on the side of clearing
    }
    if (_generation != generation) {
      return; // a newer copy arrived while we were checking
    }
    _pending = false;
    _fingerprint = null;
    _copiedAt = null;
    if (!ours) {
      return;
    }
    try {
      await _platform.clear();
    } on Object {
      // Nothing more can be done from here; the content stays as the OS has it.
    }
  }
}
