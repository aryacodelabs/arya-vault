import 'dart:convert';
import 'dart:typed_data';

/// A wipeable buffer for secrets that cross the API boundary (docs/14 §1).
///
/// Every password and recovery-key buffer in the app is a [SecretBytes]. Call
/// [use] / [useAsync] so the buffer is wiped in a `finally` block, or call
/// [wipe] explicitly.
///
/// **Honest limit (docs/14 §1, SEC-H03):** this only protects the byte buffer.
/// A secret that has been rendered as a Dart `String` or a widget (a text
/// field's text, the recovery key on screen) lives in the Dart heap, cannot be
/// wiped, and is only freed when the garbage collector decides. The UI keeps
/// such lifetimes short; it does not claim more than that.
final class SecretBytes {
  /// Takes ownership of [bytes]; the caller must not keep or reuse them.
  SecretBytes(Uint8List bytes) : _bytes = bytes;

  /// UTF-8 encodes [text] into a fresh buffer.
  factory SecretBytes.fromString(String text) =>
      SecretBytes(Uint8List.fromList(utf8.encode(text)));

  /// An empty, not-yet-wiped buffer.
  factory SecretBytes.empty() => SecretBytes(Uint8List(0));

  final Uint8List _bytes;
  bool _wiped = false;

  /// True once [wipe] ran (explicitly or through [use]).
  bool get isWiped => _wiped;

  /// Number of bytes (UTF-8), not characters.
  int get length => _bytes.length;

  /// A fresh copy of the bytes for an API call that takes ownership. The
  /// caller hands it to the API and the copy is the callee's to wipe or drop.
  /// Throws [StateError] after [wipe].
  Uint8List copyBytes() {
    _checkLive();
    return Uint8List.fromList(_bytes);
  }

  /// Runs [action] with the live bytes and wipes them afterwards, whether or
  /// not [action] throws. Do not let the bytes escape [action].
  T use<T>(T Function(Uint8List bytes) action) {
    _checkLive();
    try {
      return action(_bytes);
    } finally {
      wipe();
    }
  }

  /// Like [use] for asynchronous work; wipes after the future completes.
  Future<T> useAsync<T>(Future<T> Function(Uint8List bytes) action) async {
    _checkLive();
    try {
      return await action(_bytes);
    } finally {
      wipe();
    }
  }

  /// Overwrites every byte with zero. Idempotent.
  void wipe() {
    _bytes.fillRange(0, _bytes.length, 0);
    _wiped = true;
  }

  void _checkLive() {
    if (_wiped) {
      throw StateError('SecretBytes used after wipe');
    }
  }

  /// Never prints the content.
  @override
  String toString() => 'SecretBytes(<redacted>, $length bytes)';
}
