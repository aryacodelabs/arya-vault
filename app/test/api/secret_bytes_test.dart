import 'dart:typed_data';

import 'package:arya_vault/src/api/secret_bytes.dart';
import 'package:flutter_test/flutter_test.dart';

void main() {
  group('SecretBytes', () {
    test('wipe zeroes the underlying buffer', () {
      final Uint8List raw = Uint8List.fromList(<int>[1, 2, 3, 4]);
      final SecretBytes s = SecretBytes(raw);
      expect(s.isWiped, isFalse);
      s.wipe();
      expect(s.isWiped, isTrue);
      expect(raw, everyElement(0));
    });

    test('use wipes after the action', () {
      final Uint8List raw = Uint8List.fromList(<int>[9, 9, 9]);
      final SecretBytes s = SecretBytes(raw);
      final int sum = s.use(
        (Uint8List b) => b.fold(0, (int a, int c) => a + c),
      );
      expect(sum, 27);
      expect(raw, everyElement(0));
      expect(s.isWiped, isTrue);
    });

    test('use wipes even when the action throws', () {
      final Uint8List raw = Uint8List.fromList(<int>[7, 7]);
      final SecretBytes s = SecretBytes(raw);
      expect(
        () => s.use<void>((_) => throw StateError('boom')),
        throwsStateError,
      );
      expect(raw, everyElement(0));
    });

    test('useAsync wipes after the future completes, also on error', () async {
      final Uint8List ok = Uint8List.fromList(<int>[5, 5]);
      final SecretBytes a = SecretBytes(ok);
      await a.useAsync((Uint8List b) async {
        await Future<void>.delayed(Duration.zero);
        expect(b, everyElement(5), reason: 'still live inside the action');
      });
      expect(ok, everyElement(0));

      final Uint8List bad = Uint8List.fromList(<int>[6, 6]);
      final SecretBytes b = SecretBytes(bad);
      await expectLater(
        b.useAsync<void>((_) async => throw StateError('boom')),
        throwsStateError,
      );
      expect(bad, everyElement(0));
    });

    test('use after wipe throws instead of returning zeros', () {
      final SecretBytes s = SecretBytes.fromString('CANARY')..wipe();
      expect(() => s.use((_) {}), throwsStateError);
      expect(s.copyBytes, throwsStateError);
    });

    test('copyBytes is independent of the original', () {
      final SecretBytes s = SecretBytes.fromString('CANARY');
      final Uint8List copy = s.copyBytes();
      s.wipe();
      expect(String.fromCharCodes(copy), 'CANARY');
    });

    test('wipe is idempotent', () {
      final SecretBytes s = SecretBytes.fromString('x')..wipe();
      s.wipe();
      expect(s.isWiped, isTrue);
    });

    test('toString never prints the content', () {
      final SecretBytes s = SecretBytes.fromString('CANARY-secret');
      expect(s.toString(), isNot(contains('CANARY')));
      expect('$s', contains('redacted'));
    });

    test('length counts UTF-8 bytes', () {
      expect(SecretBytes.fromString('é').length, 2);
    });
  });
}
