import 'dart:async';
import 'dart:typed_data';

import 'package:arya_vault/src/api/secret_bytes.dart';
import 'package:arya_vault/src/services/clipboard_manager.dart';
import 'package:arya_vault/src/services/fake_services.dart';
import 'package:arya_vault/src/services/services.dart';
import 'package:flutter_test/flutter_test.dart';

// SEC-H01 (logic): the clipboard is cleared after N seconds, only if it still
// holds what we wrote, with the sensitive flag set on every write.
void main() {
  late FakeClipboardPlatform platform;
  late FakeTimeSource time;
  late ClipboardManager manager;

  setUp(() {
    platform = FakeClipboardPlatform();
    time = FakeTimeSource();
    manager = ClipboardManager(
      platform: platform,
      time: time,
      clearAfter: const Duration(seconds: 30),
    );
  });

  tearDown(() => manager.dispose());

  test('writes with the sensitive flag and wipes the buffer', () async {
    final SecretBytes secret = SecretBytes.fromString('CANARY-clip-1');
    await manager.copySecret(secret);
    expect(platform.copied, <String>['CANARY-clip-1']);
    expect(platform.sensitiveFlags, <bool>[true]);
    expect(secret.isWiped, isTrue);
    expect(manager.hasPending, isTrue);
  });

  test('clears after N seconds when the content is still ours', () async {
    await manager.copySecret(SecretBytes.fromString('CANARY-clip-2'));
    time.advance(const Duration(seconds: 29));
    await Future<void>.delayed(Duration.zero);
    expect(platform.clearCount, 0);
    time.advance(const Duration(seconds: 1));
    await Future<void>.delayed(Duration.zero);
    expect(platform.clearCount, 1);
    expect(platform.current, isNull);
    expect(manager.hasPending, isFalse);
  });

  test('does not erase something the user copied since', () async {
    await manager.copySecret(SecretBytes.fromString('CANARY-clip-3'));
    platform.userCopies('the user copied this elsewhere');
    time.advance(const Duration(seconds: 30));
    await Future<void>.delayed(Duration.zero);
    expect(platform.clearCount, 0);
    expect(
      String.fromCharCodes(platform.current!),
      'the user copied this elsewhere',
    );
    expect(manager.hasPending, isFalse);
  });

  test(
    'a user copy of identical text is indistinguishable and is cleared',
    () async {
      // Documented limit: same content, same fingerprint.
      await manager.copySecret(SecretBytes.fromString('CANARY-same'));
      platform.userCopies('CANARY-same');
      time.advance(const Duration(seconds: 30));
      await Future<void>.delayed(Duration.zero);
      expect(platform.clearCount, 1);
    },
  );

  test('the clipboard emptied by the user is left alone', () async {
    await manager.copySecret(SecretBytes.fromString('CANARY-clip-4'));
    platform.current = null;
    time.advance(const Duration(seconds: 30));
    await Future<void>.delayed(Duration.zero);
    expect(platform.clearCount, 0);
  });

  test('a second copy replaces the first and restarts the timer', () async {
    await manager.copySecret(SecretBytes.fromString('CANARY-first'));
    time.advance(const Duration(seconds: 20));
    await manager.copySecret(SecretBytes.fromString('CANARY-second'));
    time.advance(const Duration(seconds: 20)); // 40 s after the first
    await Future<void>.delayed(Duration.zero);
    expect(platform.clearCount, 0, reason: 'the first timer must be gone');
    time.advance(const Duration(seconds: 10));
    await Future<void>.delayed(Duration.zero);
    expect(platform.clearCount, 1);
    expect(platform.current, isNull);
  });

  test('lock clears immediately when ours and cancels the timer', () async {
    await manager.copySecret(SecretBytes.fromString('CANARY-lock'));
    await manager.onLock();
    expect(platform.clearCount, 1);
    expect(platform.current, isNull);
    expect(time.activeTimers, 0);
    time.advance(const Duration(minutes: 5));
    expect(platform.clearCount, 1, reason: 'no second clear after the lock');
  });

  test('lock leaves a foreign clipboard alone', () async {
    await manager.copySecret(SecretBytes.fromString('CANARY-lock-2'));
    platform.userCopies('not ours');
    await manager.onLock();
    expect(platform.clearCount, 0);
  });

  test('lock with nothing pending does nothing', () async {
    await manager.onLock();
    expect(platform.clearCount, 0);
  });

  test('a copy that finishes after a lock is cleared at once', () async {
    final SlowWritePlatform slow = SlowWritePlatform();
    final ClipboardManager m = ClipboardManager(
      platform: slow,
      time: time,
      clearAfter: const Duration(seconds: 30),
    );
    final Future<void> copy = m.copySecret(
      SecretBytes.fromString('CANARY-late'),
    );
    await m.onLock(); // the vault locks while the write is in flight
    slow.finishWrite();
    await copy;
    expect(slow.clearCount, 1);
    expect(slow.current, isNull);
    m.dispose();
  });

  test('N changes: lowering it re-times the pending copy', () async {
    await manager.copySecret(SecretBytes.fromString('CANARY-n'));
    time.advance(const Duration(seconds: 10));
    manager.setClearAfter(const Duration(seconds: 15));
    time.advance(const Duration(seconds: 4));
    await Future<void>.delayed(Duration.zero);
    expect(platform.clearCount, 0);
    time.advance(const Duration(seconds: 1)); // 15 s since the copy
    await Future<void>.delayed(Duration.zero);
    expect(platform.clearCount, 1);
  });

  test(
    'N changes: lowering it below the elapsed time clears right away',
    () async {
      await manager.copySecret(SecretBytes.fromString('CANARY-n2'));
      time.advance(const Duration(seconds: 20));
      manager.setClearAfter(const Duration(seconds: 5));
      time.advance(Duration.zero);
      await Future<void>.delayed(Duration.zero);
      expect(platform.clearCount, 1);
    },
  );

  test('N changes: raising it postpones the clear', () async {
    await manager.copySecret(SecretBytes.fromString('CANARY-n3'));
    time.advance(const Duration(seconds: 10));
    manager.setClearAfter(const Duration(seconds: 60));
    time.advance(const Duration(seconds: 40));
    await Future<void>.delayed(Duration.zero);
    expect(platform.clearCount, 0);
    time.advance(const Duration(seconds: 10));
    await Future<void>.delayed(Duration.zero);
    expect(platform.clearCount, 1);
  });

  test(
    'a platform that cannot fingerprint is cleared unconditionally',
    () async {
      platform.canFingerprint = false;
      await manager.copySecret(SecretBytes.fromString('CANARY-nofp'));
      platform.userCopies('whatever');
      time.advance(const Duration(seconds: 30));
      await Future<void>.delayed(Duration.zero);
      expect(platform.clearCount, 1);
    },
  );

  test(
    'a failing write throws, schedules nothing and keeps the old copy pending',
    () async {
      await manager.copySecret(SecretBytes.fromString('CANARY-ok'));
      platform.failWrite = true;
      await expectLater(
        manager.copySecret(SecretBytes.fromString('CANARY-bad')),
        throwsA(isA<ServiceUnavailable>()),
      );
      expect(manager.hasPending, isTrue);
      platform.failWrite = false;
      time.advance(const Duration(seconds: 30));
      await Future<void>.delayed(Duration.zero);
      expect(platform.clearCount, 1);
    },
  );

  test('a failing clear does not throw out of the timer', () async {
    final FailingClearPlatform p = FailingClearPlatform();
    final ClipboardManager m = ClipboardManager(
      platform: p,
      time: time,
      clearAfter: const Duration(seconds: 5),
    );
    await m.copySecret(SecretBytes.fromString('CANARY-fail-clear'));
    time.advance(const Duration(seconds: 5));
    await Future<void>.delayed(Duration.zero);
    expect(m.hasPending, isFalse);
    m.dispose();
  });

  test('dispose cancels the timer', () async {
    await manager.copySecret(SecretBytes.fromString('CANARY-dispose'));
    manager.dispose();
    expect(time.activeTimers, 0);
  });

  test(
    'the manager never keeps the secret: nothing readable after the clear',
    () async {
      final SecretBytes secret = SecretBytes.fromString('CANARY-hygiene');
      await manager.copySecret(secret);
      time.advance(const Duration(seconds: 30));
      await Future<void>.delayed(Duration.zero);
      expect(platform.current, isNull);
      expect(() => secret.copyBytes(), throwsStateError);
      expect(manager.toString(), isNot(contains('CANARY')));
    },
  );
}

/// A platform whose write completes only when the test says so.
class SlowWritePlatform extends FakeClipboardPlatform {
  final Completer<void> _gate = Completer<void>();

  void finishWrite() => _gate.complete();

  @override
  Future<void> write(Uint8List bytes, {required bool sensitive}) async {
    final Uint8List copy = Uint8List.fromList(bytes);
    await _gate.future;
    await super.write(copy, sensitive: sensitive);
  }
}

class FailingClearPlatform extends FakeClipboardPlatform {
  @override
  Future<void> clear() async => throw const ServiceUnavailable('clipboard');
}
