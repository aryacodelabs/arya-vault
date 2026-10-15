// Fakes of the platform services for tests and the fake-backed app.
import 'dart:async';
import 'dart:convert';
import 'dart:typed_data';

import 'services.dart';
import 'time_source.dart';

/// A clipboard with one slot. [copied] keeps every text written (tests only),
/// [current] is what is on the clipboard right now.
class FakeClipboardPlatform implements ClipboardPlatform {
  final List<String> copied = <String>[];
  final List<bool> sensitiveFlags = <bool>[];
  int clearCount = 0;

  /// Raw content of the clipboard, null when empty.
  Uint8List? current;

  /// When false, [contentFingerprint] always answers null (a platform that
  /// cannot fingerprint).
  bool canFingerprint = true;
  bool failWrite = false;

  /// Simulates the user copying something in another app.
  void userCopies(String text) =>
      current = Uint8List.fromList(utf8.encode(text));

  @override
  Future<void> write(Uint8List bytes, {required bool sensitive}) async {
    if (failWrite) {
      throw const ServiceUnavailable('clipboard');
    }
    copied.add(utf8.decode(bytes, allowMalformed: true));
    sensitiveFlags.add(sensitive);
    current = Uint8List.fromList(bytes);
  }

  @override
  Future<String?> contentFingerprint() async {
    final Uint8List? c = current;
    if (!canFingerprint || c == null) return null;
    // Not the content: a salted rolling hash like a real platform would give.
    var h = 17;
    for (final int b in c) {
      h = (h * 31 + b) & 0x7fffffff;
    }
    return 'fp-$h-${c.length}';
  }

  @override
  Future<void> clear() async {
    clearCount++;
    current = null;
  }
}

class FakeCaptureProtection implements CaptureProtection {
  FakeCaptureProtection({this.available = true});

  @override
  final bool available;

  /// Every value `setBlocked` received, in order.
  final List<bool> calls = <bool>[];
  bool? get blocked => calls.isEmpty ? null : calls.last;

  @override
  Future<void> setBlocked(bool blocked) async => calls.add(blocked);
}

/// Deterministic time: [now] only moves with [advance], which fires due timers
/// in order (including timers they schedule).
class FakeTimeSource implements TimeSource {
  FakeTimeSource([DateTime? start])
    : _now = start ?? DateTime.utc(2026, 1, 1, 12);

  DateTime _now;
  final List<_FakeTimer> _timers = <_FakeTimer>[];

  @override
  DateTime now() => _now;

  int get activeTimers => _timers.where((_FakeTimer t) => t.isActive).length;

  @override
  Timer timer(Duration delay, void Function() callback) {
    final _FakeTimer t = _FakeTimer(_now.add(delay), callback);
    _timers.add(t);
    return t;
  }

  /// Moves the clock forward without firing timers (a machine that slept).
  void jump(Duration d) => _now = _now.add(d);

  /// Moves the clock forward by [d], firing every timer that becomes due.
  void advance(Duration d) {
    final DateTime target = _now.add(d);
    while (true) {
      final List<_FakeTimer> due =
          _timers
              .where((_FakeTimer t) => t.isActive && !t.due.isAfter(target))
              .toList()
            ..sort((_FakeTimer a, _FakeTimer b) => a.due.compareTo(b.due));
      if (due.isEmpty) break;
      final _FakeTimer next = due.first;
      if (next.due.isAfter(_now)) _now = next.due;
      next.fire();
    }
    _now = target;
    _timers.removeWhere((_FakeTimer t) => !t.isActive);
  }
}

class _FakeTimer implements Timer {
  _FakeTimer(this.due, this._callback);

  final DateTime due;
  final void Function() _callback;
  bool _active = true;

  void fire() {
    if (!_active) return;
    _active = false;
    _callback();
  }

  @override
  void cancel() => _active = false;

  @override
  bool get isActive => _active;

  @override
  int get tick => _active ? 0 : 1;
}

class FakeFilePickerService implements FilePickerService {
  /// What `saveFile` returns (false simulates a cancelled dialog).
  bool saveResult = true;
  bool failSave = false;
  final List<({String name, Uint8List bytes})> saved =
      <({String name, Uint8List bytes})>[];

  @override
  Future<bool> saveFile({
    required String suggestedName,
    required Uint8List bytes,
  }) async {
    if (failSave) {
      throw const ServiceUnavailable('file picker');
    }
    saved.add((name: suggestedName, bytes: Uint8List.fromList(bytes)));
    return saveResult;
  }

  /// What `pickFile` returns (null simulates a cancelled dialog).
  Uint8List? pickResult;
  bool failPick = false;
  final List<List<String>> pickedExtensions = <List<String>>[];

  @override
  Future<Uint8List?> pickFile({
    List<String> extensions = const <String>[],
  }) async {
    if (failPick) {
      throw const ServiceUnavailable('file picker');
    }
    pickedExtensions.add(extensions);
    final Uint8List? r = pickResult;
    return r == null ? null : Uint8List.fromList(r);
  }
}

class FakePrintService implements PrintService {
  bool fail = false;

  /// Test hook: pretend the platform has no printing (the Print button then disappears).
  bool available = true;
  @override
  bool get isAvailable => available;
  final List<({String title, String body})> printed =
      <({String title, String body})>[];

  @override
  Future<void> printText({
    required String title,
    required Uint8List body,
  }) async {
    if (fail) {
      throw const ServiceUnavailable('print');
    }
    printed.add((title: title, body: utf8.decode(body)));
  }
}

class FakePlatformEvents implements PlatformEvents {
  final StreamController<PlatformEvent> _controller =
      StreamController<PlatformEvent>.broadcast();

  void emit(PlatformEvent event) => _controller.add(event);

  Future<void> close() => _controller.close();

  @override
  Stream<PlatformEvent> get events => _controller.stream;
}

class FakeUrlLauncherService implements UrlLauncherService {
  final List<Uri> opened = <Uri>[];
  bool result = true;

  @override
  Future<bool> open(Uri uri) async {
    opened.add(uri);
    return result;
  }
}
