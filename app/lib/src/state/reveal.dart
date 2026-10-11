import 'dart:async';
import 'dart:typed_data';

import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../api/secret_bytes.dart';
import 'vault_session.dart';

/// Identifies one revealable value: an item field, a custom field or an old
/// version.
String revealKey(String itemId, String what) => '$itemId|$what';

/// Holds revealed secrets as wipeable bytes and hides them again.
///
/// A secret is wiped when it times out (at most 15 s, SEC-H03), when the app
/// loses focus, when the user leaves the item, and when the vault locks. The
/// state is only the set of revealed keys; the bytes never leave this class
/// except as a copy handed to a widget for rendering. Honest limit: a widget
/// that renders the text holds a Dart `String` that cannot be wiped (see
/// SecretBytes); it is dropped on the next rebuild after the hide.
class RevealNotifier extends Notifier<Set<String>> {
  final Map<String, SecretBytes> _live = <String, SecretBytes>{};
  final Map<String, Timer> _timers = <String, Timer>{};

  @override
  Set<String> build() {
    ref.onDispose(_wipeAll);
    ref.listen(lockEpochProvider, (int? _, int _) => hideAll());
    return const <String>{};
  }

  bool isRevealed(String key) => _live.containsKey(key);

  /// A fresh copy of the revealed bytes (the caller wipes it), or null.
  Uint8List? copyOf(String key) => _live[key]?.copyBytes();

  /// Loads and shows the value for [key]. [hideAfter] is capped at 15 s.
  Future<void> reveal(
    String key,
    Future<Uint8List?> Function() load, {
    Duration hideAfter = const Duration(seconds: 15),
  }) async {
    if (_live.containsKey(key)) {
      return;
    }
    final Uint8List? bytes = await load();
    if (bytes == null) {
      return;
    }
    if (_live.containsKey(key)) {
      bytes.fillRange(0, bytes.length, 0);
      return;
    }
    _live[key] = SecretBytes(bytes);
    final Duration capped = hideAfter > const Duration(seconds: 15)
        ? const Duration(seconds: 15)
        : hideAfter;
    _timers[key] = Timer(capped, () => hide(key));
    state = <String>{...state, key};
  }

  void hide(String key) {
    _timers.remove(key)?.cancel();
    final SecretBytes? s = _live.remove(key);
    if (s != null) {
      s.wipe();
      state = <String>{...state}..remove(key);
    }
  }

  /// Hides everything whose key starts with [prefix] (an item's `id|`).
  void hideWithPrefix(String prefix) {
    for (final String k
        in _live.keys.where((String k) => k.startsWith(prefix)).toList()) {
      hide(k);
    }
  }

  void hideAll() {
    if (_live.isEmpty) {
      return;
    }
    _wipeAll();
    state = const <String>{};
  }

  void _wipeAll() {
    for (final Timer t in _timers.values) {
      t.cancel();
    }
    _timers.clear();
    for (final SecretBytes s in _live.values) {
      s.wipe();
    }
    _live.clear();
  }
}

final NotifierProvider<RevealNotifier, Set<String>> revealProvider =
    NotifierProvider<RevealNotifier, Set<String>>(RevealNotifier.new);
