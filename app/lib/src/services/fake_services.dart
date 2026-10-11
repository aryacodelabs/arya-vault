// Fakes of the platform services for tests and the fake-backed app.
import 'dart:async';
import 'dart:convert';
import 'dart:typed_data';

import 'services.dart';

class FakeClipboardService implements ClipboardService {
  final List<String> copied = <String>[];
  int clearCount = 0;

  @override
  Future<void> copy(String text) async => copied.add(text);

  @override
  Future<void> clear() async => clearCount++;
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

  @override
  Future<Uint8List?> pickFile({
    List<String> extensions = const <String>[],
  }) async => null;
}

class FakePrintService implements PrintService {
  bool fail = false;
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
