// Injectable clock and timers, so the security timers (auto-lock, clipboard
// clear) are testable with fake time and never depend on real waiting.
import 'dart:async';

abstract interface class TimeSource {
  DateTime now();

  /// A one-shot timer.
  Timer timer(Duration delay, void Function() callback);
}

class SystemTimeSource implements TimeSource {
  const SystemTimeSource();

  @override
  DateTime now() => DateTime.now();

  @override
  Timer timer(Duration delay, void Function() callback) =>
      Timer(delay, callback);
}
