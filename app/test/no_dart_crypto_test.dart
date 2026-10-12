import 'dart:io';

import 'package:arya_vault/src/app_info.dart';
import 'package:flutter_test/flutter_test.dart';

/// SEC-C09 / CLAUDE.md hard rule 2: the app shows only values the core made.
/// Dart must not generate randomness or implement crypto. This scans `lib/`.
///
/// The one allowed exception is the in-memory stand-in `FakeVaultApi`, which
/// exists to pretend to be the core (it also fakes a "strength" and a recovery
/// key). It is not part of the real app path once the FFI is wired in (L01);
/// the second test pins down that nothing but `main.dart` can reach it.
const Set<String> _allowedRandom = <String>{'lib/src/api/fake_vault_api.dart'};

/// `dart:math` itself is fine (min/max); randomness is not.
final RegExp _random = RegExp(
  r'\bRandom\b|SecureRandom|\.nextInt\(|\.nextDouble\(|\.nextBool\(',
);
final RegExp _cryptoImport = RegExp(
  r"import\s+'(package:(crypto|cryptography|pointycastle|encrypt|steel_crypto|hashlib)\b|dart:ffi|dart:mirrors)",
);

/// `lib/src/rust/` is flutter_rust_bridge output (tools/codegen.sh), the one sanctioned FFI
/// path; it is generated, never edited, and the last test limits who may import it.
bool _isGenerated(File f) {
  final String p = _rel(f);
  return p.contains('/gen/') || p.startsWith('lib/src/rust/');
}

Iterable<File> _libFiles() =>
    Directory('lib')
        .listSync(recursive: true)
        .whereType<File>()
        .where((File f) => f.path.endsWith('.dart'))
        .where((File f) => !_isGenerated(f));

String _rel(File f) => f.path.replaceAll(r'\', '/');

void main() {
  test('lib/ uses no Random outside the fake core', () {
    final List<String> offenders = <String>[];
    for (final File f in _libFiles()) {
      if (_allowedRandom.contains(_rel(f))) continue;
      final String text = f.readAsStringSync();
      for (final RegExpMatch m in _random.allMatches(text)) {
        offenders.add('${_rel(f)}: ${m.group(0)}');
      }
    }
    expect(offenders, isEmpty, reason: 'Dart never generates passwords');
  });

  test('lib/ imports no crypto package and no FFI', () {
    final List<String> offenders = <String>[];
    for (final File f in _libFiles()) {
      final String text = f.readAsStringSync();
      for (final RegExpMatch m in _cryptoImport.allMatches(text)) {
        offenders.add('${_rel(f)}: ${m.group(0)}');
      }
    }
    expect(offenders, isEmpty);
  });

  test('only main.dart and the fake itself mention the fake core', () {
    final List<String> users = <String>[];
    for (final File f in _libFiles()) {
      final String path = _rel(f);
      if (path == 'lib/src/api/fake_vault_api.dart') continue;
      if (f.readAsStringSync().contains('fake_vault_api.dart')) users.add(path);
    }
    expect(users, <String>['lib/main.dart']);
  });

  test('only the FFI-backed api imports the generated bindings', () {
    final List<String> users = <String>[];
    for (final File f in _libFiles()) {
      if (RegExp(r"import\s+'(\.\./)*(\./)?(src/)?rust/")
          .hasMatch(f.readAsStringSync())) {
        users.add(_rel(f));
      }
    }
    users.sort();
    expect(users, <String>[
      'lib/src/api/ffi_mappers.dart',
      'lib/src/api/ffi_vault_api.dart',
    ]);
  });

  test('the scanner would catch a violation', () {
    expect(_random.hasMatch('final r = Random.secure();'), isTrue);
    expect(_random.hasMatch('final n = r.nextInt(10);'), isTrue);
    expect(_random.hasMatch('math.max(1, 2)'), isFalse);
    expect(
      _cryptoImport.hasMatch("import 'package:crypto/crypto.dart';"),
      isTrue,
    );
    expect(_random.hasMatch('final randomised = 3;'), isFalse);
  });

  test('kAppVersion matches pubspec.yaml', () {
    final String pubspec = File('pubspec.yaml').readAsStringSync();
    final RegExpMatch? m = RegExp(
      r'^version:\s*(\S+)\s*$',
      multiLine: true,
    ).firstMatch(pubspec);
    expect(m, isNotNull);
    expect(kAppVersion, m!.group(1));
  });
}
