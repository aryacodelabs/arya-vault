// Platform-service seams (docs/02 §3). B01 defines the interfaces and test
// fakes only; the Windows implementations come in L01. Nothing here touches the
// network.
import 'dart:async';
import 'dart:typed_data';

/// Clipboard access with auto-clear (B03 builds the clear timer on top).
abstract interface class ClipboardService {
  /// Puts [text] on the clipboard. The recovery-key screen must never call
  /// this (docs/14 §8 question 3).
  Future<void> copy(String text);

  Future<void> clear();
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
  /// Prints [body] (UTF-8 text) under [title].
  Future<void> printText({required String title, required Uint8List body});
}

/// OS events the auto-lock logic reacts to (B03 consumes them).
enum PlatformEvent { sleep, resume, screenLocked, screenUnlocked }

abstract interface class PlatformEvents {
  Stream<PlatformEvent> get events;
}

/// Thrown by the default services until L01 provides real ones.
class ServiceUnavailable implements Exception {
  const ServiceUnavailable(this.service);
  final String service;
  @override
  String toString() => 'ServiceUnavailable($service)';
}

class UnavailableClipboardService implements ClipboardService {
  const UnavailableClipboardService();
  @override
  Future<void> copy(String text) async =>
      throw const ServiceUnavailable('clipboard');
  @override
  Future<void> clear() async => throw const ServiceUnavailable('clipboard');
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
  Future<void> printText({
    required String title,
    required Uint8List body,
  }) async => throw const ServiceUnavailable('print');
}

class NoPlatformEvents implements PlatformEvents {
  const NoPlatformEvents();
  @override
  Stream<PlatformEvent> get events => const Stream<PlatformEvent>.empty();
}
