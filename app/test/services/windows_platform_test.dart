import 'package:arya_vault/src/api/app_error.dart';
import 'package:arya_vault/src/services/services.dart';
import 'package:arya_vault/src/services/windows_platform.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';

/// The Dart half of the Windows platform seams. The native half (the runner) cannot run
/// here; it is covered by the manual checks in docs/reviews/m2-windows-evidence.md.
void main() {
  TestWidgetsFlutterBinding.ensureInitialized();
  const MethodChannel channel = MethodChannel(platformChannelName);
  final List<MethodCall> calls = <MethodCall>[];
  Object? reply;

  setUp(() {
    calls.clear();
    reply = null;
    TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger
        .setMockMethodCallHandler(channel, (MethodCall call) async {
          calls.add(call);
          return reply;
        });
  });
  tearDown(() {
    TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger
        .setMockMethodCallHandler(channel, null);
  });

  test('write passes the bytes and the sensitive flag (SEC-H01)', () async {
    final Uint8List bytes = Uint8List.fromList(<int>[1, 2, 3]);
    await const WindowsClipboardPlatform().write(bytes, sensitive: true);
    expect(calls.single.method, 'clipboard.write');
    final Map<Object?, Object?> args =
        calls.single.arguments as Map<Object?, Object?>;
    expect(args['bytes'], bytes);
    expect(args['sensitive'], true);
  });

  test('a non-sensitive write says so', () async {
    await const WindowsClipboardPlatform().write(
      Uint8List(0),
      sensitive: false,
    );
    expect(
      (calls.single.arguments as Map<Object?, Object?>)['sensitive'],
      false,
    );
  });

  test('fingerprint and clear are forwarded', () async {
    reply = 'seq:42';
    expect(
      await const WindowsClipboardPlatform().contentFingerprint(),
      'seq:42',
    );
    reply = null;
    expect(await const WindowsClipboardPlatform().contentFingerprint(), isNull);
    await const WindowsClipboardPlatform().clear();
    expect(calls.map((MethodCall c) => c.method), <String>[
      'clipboard.fingerprint',
      'clipboard.fingerprint',
      'clipboard.clear',
    ]);
  });

  test(
    'a native failure surfaces as an exception, not silent success',
    () async {
      TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger
          .setMockMethodCallHandler(channel, (MethodCall call) async {
            throw PlatformException(code: 'clipboard_failed');
          });
      await expectLater(
        const WindowsClipboardPlatform().write(Uint8List(1), sensitive: true),
        throwsA(isA<PlatformException>()),
      );
    },
  );

  test(
    'capture protection is available and forwards the flag (SEC-H02)',
    () async {
      const WindowsCaptureProtection capture = WindowsCaptureProtection();
      expect(capture.available, isTrue);
      await capture.setBlocked(true);
      await capture.setBlocked(false);
      expect(calls.map((MethodCall c) => c.arguments), <Object?>[true, false]);
      expect(
        calls.every((MethodCall c) => c.method == 'capture.setBlocked'),
        true,
      );
    },
  );

  test('native events become PlatformEvents (SEC-A04)', () async {
    final WindowsPlatformEvents events = WindowsPlatformEvents();
    final List<PlatformEvent> seen = <PlatformEvent>[];
    final sub = events.events.listen(seen.add);
    Future<void> push(String name) async {
      await TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger
          .handlePlatformMessage(
            platformChannelName,
            const StandardMethodCodec().encodeMethodCall(
              MethodCall('event', name),
            ),
            (ByteData? _) {},
          );
    }

    for (final String n in <String>[
      'sleep',
      'resume',
      'screenLocked',
      'screenUnlocked',
      'windowMinimized',
      'windowRestored',
      'unknown-event',
    ]) {
      await push(n);
    }
    await Future<void>.delayed(Duration.zero);
    expect(seen, <PlatformEvent>[
      PlatformEvent.sleep,
      PlatformEvent.resume,
      PlatformEvent.screenLocked,
      PlatformEvent.screenUnlocked,
      PlatformEvent.windowMinimized,
      PlatformEvent.windowRestored,
    ]);
    await sub.cancel();
  });

  group('file dialogs', () {
    test('saveFile passes the name and bytes and reports the choice', () async {
      reply = true;
      final Uint8List bytes = Uint8List.fromList(<int>[9, 8]);
      final bool saved = await const WindowsFilePickerService().saveFile(
        suggestedName: 'key.txt',
        bytes: bytes,
      );
      expect(saved, isTrue);
      expect(calls.single.method, 'file.save');
      final Map<Object?, Object?> args =
          calls.single.arguments as Map<Object?, Object?>;
      expect(args['name'], 'key.txt');
      expect(args['bytes'], bytes);
    });

    test('a cancelled save is false, not an error', () async {
      reply = false;
      expect(
        await const WindowsFilePickerService().saveFile(
          suggestedName: 'a.txt',
          bytes: Uint8List(1),
        ),
        isFalse,
      );
      reply = null;
      expect(
        await const WindowsFilePickerService().saveFile(
          suggestedName: 'a.txt',
          bytes: Uint8List(1),
        ),
        isFalse,
      );
    });

    test(
      'pickFile sends the extensions and returns the bytes or null',
      () async {
        reply = Uint8List.fromList(<int>[1, 2, 3]);
        expect(
          await const WindowsFilePickerService().pickFile(
            extensions: <String>['csv'],
          ),
          Uint8List.fromList(<int>[1, 2, 3]),
        );
        expect(calls.single.method, 'file.pick');
        expect(calls.single.arguments, <String>['csv']);
        reply = null;
        expect(await const WindowsFilePickerService().pickFile(), isNull);
      },
    );

    test('a file that is too large becomes limitReached', () async {
      TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger
          .setMockMethodCallHandler(channel, (MethodCall call) async {
            throw PlatformException(code: 'too_large');
          });
      await expectLater(
        const WindowsFilePickerService().pickFile(),
        throwsA(
          isA<AppError>().having(
            (AppError e) => e.code,
            'code',
            AppErrorCode.limitReached,
          ),
        ),
      );
    });
  });
}
