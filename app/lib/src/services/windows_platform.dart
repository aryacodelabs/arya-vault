// Windows implementations of the platform seams (services.dart), talking to the
// runner's PlatformChannel (app/windows/runner/platform_channel.cpp) over one
// MethodChannel. No logic lives here: the clipboard timing and the lock decisions are
// ClipboardManager and AutoLockController.
import 'dart:async';

import 'package:flutter/services.dart';

import 'services.dart';

/// Name shared with the C++ side.
const String platformChannelName = 'aryavault/platform';

const MethodChannel _channel = MethodChannel(platformChannelName);

/// Clipboard through the Win32 clipboard. A sensitive write also sets the three
/// "keep out of history / cloud / monitors" formats (SEC-H01); the native side fails
/// the write if any of them cannot be set.
class WindowsClipboardPlatform implements ClipboardPlatform {
  const WindowsClipboardPlatform();

  @override
  Future<void> write(Uint8List bytes, {required bool sensitive}) async {
    await _channel.invokeMethod<void>('clipboard.write', <String, Object>{
      'bytes': bytes,
      'sensitive': sensitive,
    });
  }

  @override
  Future<String?> contentFingerprint() =>
      _channel.invokeMethod<String>('clipboard.fingerprint');

  @override
  Future<void> clear() => _channel.invokeMethod<void>('clipboard.clear');
}

/// `SetWindowDisplayAffinity(WDA_EXCLUDEFROMCAPTURE)` on the main window (SEC-H02).
class WindowsCaptureProtection implements CaptureProtection {
  const WindowsCaptureProtection();

  @override
  bool get available => true;

  @override
  Future<void> setBlocked(bool blocked) =>
      _channel.invokeMethod<void>('capture.setBlocked', blocked);
}

/// Sleep/resume, workstation lock/unlock and minimize/restore, pushed by the runner.
class WindowsPlatformEvents implements PlatformEvents {
  WindowsPlatformEvents() {
    _channel.setMethodCallHandler(_onCall);
  }

  final StreamController<PlatformEvent> _controller =
      StreamController<PlatformEvent>.broadcast();

  Future<Object?> _onCall(MethodCall call) async {
    if (call.method != 'event') {
      return null;
    }
    final PlatformEvent? event = _parse(call.arguments);
    if (event != null) {
      _controller.add(event);
    }
    return null;
  }

  static PlatformEvent? _parse(Object? name) => switch (name) {
    'sleep' => PlatformEvent.sleep,
    'resume' => PlatformEvent.resume,
    'screenLocked' => PlatformEvent.screenLocked,
    'screenUnlocked' => PlatformEvent.screenUnlocked,
    'windowMinimized' => PlatformEvent.windowMinimized,
    'windowRestored' => PlatformEvent.windowRestored,
    _ => null,
  };

  @override
  Stream<PlatformEvent> get events => _controller.stream;
}
