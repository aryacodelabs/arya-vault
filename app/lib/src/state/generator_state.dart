import 'dart:convert';

import 'package:flutter/foundation.dart';

import '../api/app_error.dart';
import '../api/dtos.dart';
import '../api/secret_bytes.dart';
import '../api/vault_api.dart';

enum GeneratorMode { password, passphrase }

/// What is wrong with the current options, if anything. The UI shows it and
/// does not ask the core for a value until it is fixed.
enum GeneratorProblem { noCharacterClass, noSymbols }

/// State of the generator screen and sheet. **Dart never generates passwords**
/// (SEC-C09): every value comes from the core's `generatePassword` /
/// `generatePassphrase`, and so do the entropy and strength shown next to it.
///
/// The current value is held as [SecretBytes] and wiped when replaced, when
/// the user dismisses the generator, and when the controller is disposed.
class GeneratorController extends ChangeNotifier {
  GeneratorController(this._api);

  final VaultApi _api;

  GeneratorMode mode = GeneratorMode.password;
  PasswordOptions password = const PasswordOptions();
  PassphraseOptions passphrase = const PassphraseOptions();

  SecretBytes? _value;
  double? entropyBits;
  Strength? strength;
  AppError? error;
  bool busy = false;

  int _generation = 0;
  bool _disposed = false;

  GeneratorProblem? get problem {
    if (mode == GeneratorMode.passphrase) return null;
    if (!(password.lower ||
        password.upper ||
        password.digits ||
        password.symbols)) {
      return GeneratorProblem.noCharacterClass;
    }
    if (password.symbols && password.symbolSet.isEmpty) {
      return GeneratorProblem.noSymbols;
    }
    return null;
  }

  bool get hasValue => _value != null && !_value!.isWiped;

  /// The value as text for display. Dart cannot wipe the `String` (see
  /// SecretBytes); it is rebuilt on demand and not stored here.
  String? displayText() {
    final SecretBytes? v = _value;
    if (v == null || v.isWiped) return null;
    final Uint8List copy = v.copyBytes();
    try {
      return utf8.decode(copy, allowMalformed: true);
    } finally {
      copy.fillRange(0, copy.length, 0);
    }
  }

  /// A fresh wipeable copy for copy/use; null if there is no value. The caller
  /// owns it.
  SecretBytes? takeCopy() {
    final SecretBytes? v = _value;
    if (v == null || v.isWiped) return null;
    return SecretBytes(v.copyBytes());
  }

  void setMode(GeneratorMode next) {
    if (next == mode) return;
    mode = next;
    regenerate();
  }

  void setPassword(PasswordOptions next) {
    password = next;
    regenerate();
  }

  void setPassphrase(PassphraseOptions next) {
    passphrase = next;
    regenerate();
  }

  /// Asks the core for a new value with the current options.
  Future<void> regenerate() async {
    final int generation = ++_generation;
    error = null;
    if (problem != null) {
      _replace(null);
      entropyBits = null;
      strength = null;
      busy = false;
      notifyListeners();
      return;
    }
    busy = true;
    notifyListeners();
    SecretBytes? fresh;
    try {
      final Uint8List bytes = mode == GeneratorMode.password
          ? await _api.generatePassword(password)
          : await _api.generatePassphrase(passphrase);
      fresh = SecretBytes(bytes);
      final double bits = await _api.entropyBits(
        mode == GeneratorMode.password
            ? PasswordEntropy(password)
            : PassphraseEntropy(passphrase),
      );
      final Strength s = await SecretBytes(fresh.copyBytes())
          .useAsync(_api.strength);
      if (_disposed || generation != _generation) {
        fresh.wipe();
        return; // superseded by a newer request
      }
      _replace(fresh);
      entropyBits = bits;
      strength = s;
      busy = false;
      notifyListeners();
    } on AppError catch (e) {
      fresh?.wipe();
      if (_disposed || generation != _generation) return;
      _replace(null);
      entropyBits = null;
      strength = null;
      error = e;
      busy = false;
      notifyListeners();
    }
  }

  /// Wipes the value now (dismiss, lock).
  void wipe() {
    _generation++;
    _replace(null);
    entropyBits = null;
    strength = null;
    if (!_disposed) notifyListeners();
  }

  void _replace(SecretBytes? next) {
    _value?.wipe();
    _value = next;
  }

  @override
  void dispose() {
    _disposed = true;
    _generation++;
    _value?.wipe();
    _value = null;
    super.dispose();
  }

  /// Never prints the value.
  @override
  String toString() => 'GeneratorController(<redacted>, mode: ${mode.name})';
}
