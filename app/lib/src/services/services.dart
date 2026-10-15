// Platform-service seams (docs/02 §3). B01/B03 define the interfaces and test
// fakes only; the Windows implementations come in L01. Nothing here touches the
// network.
import 'dart:async';
import 'dart:typed_data';

/// Raw clipboard access of the platform (L01 implements it for Windows).
///
/// Everything with logic (the timed clear, "only if still ours", clear on lock)
/// lives in `ClipboardManager`; this seam stays dumb so it can be faked.
abstract interface class ClipboardPlatform {
  /// Puts [bytes] (UTF-8 text) on the clipboard. With [sensitive] the platform
  /// must keep it out of clipboard history and cloud sync (SEC-H01). The
  /// caller wipes [bytes] after the call returns.
  Future<void> write(Uint8List bytes, {required bool sensitive});

  /// An opaque fingerprint (for example a salted hash computed inside the
  /// platform layer) of what is on the clipboard right now, or null if it is
  /// empty, not text, or the platform cannot tell. It never contains the
  /// content itself, because the manager keeps it in memory.
  Future<String?> contentFingerprint();

  /// Empties the clipboard.
  Future<void> clear();
}

/// Blocks screenshots and screen recording of the app window (SEC-H02).
/// L01 implements it (Windows: exclude from capture).
abstract interface class CaptureProtection {
  /// False when this build has no implementation; the settings screen then says
  /// so instead of showing a toggle that does nothing.
  bool get available;

  /// Turns the block on or off for the app's windows.
  Future<void> setBlocked(bool blocked);
}

/// File dialogs; the core only ever sees bytes.
abstract interface class FilePickerService {
  /// Asks the user where to save [bytes]. Returns false if they cancelled.
  Future<bool> saveFile({
    required String suggestedName,
    required Uint8List bytes,
  });

  /// Lets the user pick a file; null if cancelled.
  Future<Uint8List?> pickFile({List<String> extensions = const <String>[]});
}

/// Printing of the recovery sheet.
abstract interface class PrintService {
  /// Whether this platform can print. When false the app does not offer a Print button
  /// (Windows has none: the print spooler would keep a copy of the recovery key on disk).
  bool get isAvailable;

  /// Prints [body] (UTF-8 text) under [title].
  Future<void> printText({required String title, required Uint8List body});
}

/// OS events the auto-lock logic reacts to.
enum PlatformEvent {
  sleep,
  resume,
  screenLocked,
  screenUnlocked,
  windowMinimized,
  windowRestored,
}

abstract interface class PlatformEvents {
  Stream<PlatformEvent> get events;
}

/// Opens a URL in the user's browser. The caller has already asked for
/// confirmation where it matters (non-https, links from notes).
abstract interface class UrlLauncherService {
  /// Returns false if nothing could handle the URL.
  Future<bool> open(Uri uri);
}

/// Thrown by the default services until L01 provides real ones.
class ServiceUnavailable implements Exception {
  const ServiceUnavailable(this.service);
  final String service;
  @override
  String toString() => 'ServiceUnavailable($service)';
}

class UnavailableClipboardPlatform implements ClipboardPlatform {
  const UnavailableClipboardPlatform();
  @override
  Future<void> write(Uint8List bytes, {required bool sensitive}) async =>
      throw const ServiceUnavailable('clipboard');
  @override
  Future<String?> contentFingerprint() async => null;
  @override
  Future<void> clear() async => throw const ServiceUnavailable('clipboard');
}

/// Reports `available == false`: the settings screen shows "not available on
/// this build" and nothing pretends to block capture.
class UnavailableCaptureProtection implements CaptureProtection {
  const UnavailableCaptureProtection();
  @override
  bool get available => false;
  @override
  Future<void> setBlocked(bool blocked) async {}
}

class UnavailableFilePickerService implements FilePickerService {
  const UnavailableFilePickerService();
  @override
  Future<bool> saveFile({
    required String suggestedName,
    required Uint8List bytes,
  }) async => throw const ServiceUnavailable('file picker');
  @override
  Future<Uint8List?> pickFile({
    List<String> extensions = const <String>[],
  }) async => throw const ServiceUnavailable('file picker');
}

class UnavailablePrintService implements PrintService {
  const UnavailablePrintService();
  @override
  bool get isAvailable => false;
  @override
  Future<void> printText({
    required String title,
    required Uint8List body,
  }) async => throw const ServiceUnavailable('print');
}

class UnavailableUrlLauncherService implements UrlLauncherService {
  const UnavailableUrlLauncherService();
  @override
  Future<bool> open(Uri uri) async =>
      throw const ServiceUnavailable('url launcher');
}

class NoPlatformEvents implements PlatformEvents {
  const NoPlatformEvents();
  @override
  Stream<PlatformEvent> get events => const Stream<PlatformEvent>.empty();
}
