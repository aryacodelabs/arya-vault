import 'dart:io';

import 'package:flutter_test/flutter_test.dart';

/// Source-level rules from CLAUDE.md and the B01 acceptance criteria.
void main() {
  final List<File> sources = Directory('lib')
      .listSync(recursive: true)
      .whereType<File>()
      .where(
        (File f) => f.path.endsWith('.dart') && !f.path.contains('/l10n/gen/'),
      )
      .toList();

  test('there are sources to check', () {
    expect(sources.length, greaterThan(20));
  });

  test('no network, no reflection, no analytics (SEC-H04, hard rule 6)', () {
    final RegExp forbidden = RegExp(
      r'''dart:html|dart:js|dart:mirrors|package:http/|package:dio|HttpClient|WebSocket|Socket\b|firebase|crashlytics|sentry|analytics|package:url_launcher''',
      caseSensitive: false,
    );
    final List<String> hits = <String>[];
    for (final File f in sources) {
      final String text = f.readAsStringSync();
      for (final RegExpMatch m in forbidden.allMatches(text)) {
        hits.add('${f.path}: ${m.group(0)}');
      }
    }
    expect(hits, isEmpty);
  });

  test(
    'dart:io is imported only by the FFI-backed api (library and data paths)',
    () {
      final List<String> users = <String>[
        for (final File f in sources)
          if (RegExp(r"import\s+'dart:io'").hasMatch(f.readAsStringSync()))
            f.path.replaceAll(r'\', '/'),
      ];
      expect(users, <String>['lib/src/api/ffi_vault_api.dart']);
    },
  );

  test('pubspec has no networking, analytics or crash-reporting packages', () {
    final String pubspec = File('pubspec.yaml').readAsStringSync();
    final String section = pubspec
        .split('\ndependencies:')
        .last
        .split('dev_dependencies:')
        .first;
    final Set<String> deps = RegExp(
      r'^  ([a-z_0-9]+):',
      multiLine: true,
    ).allMatches(section).map((RegExpMatch m) => m.group(1)!).toSet();
    expect(deps, <String>{
      'flutter',
      'flutter_localizations',
      'flutter_riverpod',
      // The generated bindings to the Rust core (L01); pinned, see tools/codegen.
      'flutter_rust_bridge',
      'freezed_annotation',
      'go_router',
      'intl',
    }, reason: 'a new dependency needs a justification in the PR');
  });

  test('no hard-coded user-visible strings in UI code', () {
    // String literals as the first argument of Text(...) or as the value of
    // labelText / hintText / helperText / tooltip / label / semanticLabel.
    final RegExp literal = RegExp(
      r'''(Text\(\s*|(labelText|hintText|helperText|errorText|tooltip|semanticLabel|semanticsLabel|label|hint|title)\s*:\s*)(const\s+)?['"]''',
    );
    final List<String> hits = <String>[];
    for (final File f in sources.where(
      (File f) => f.path.contains('/ui/') || f.path.endsWith('app.dart'),
    )) {
      final List<String> lines = f.readAsLinesSync();
      for (var i = 0; i < lines.length; i++) {
        final String line = lines[i];
        if (line.trimLeft().startsWith('//') ||
            line.trimLeft().startsWith('///')) {
          continue;
        }
        if (literal.hasMatch(line)) {
          hits.add('${f.path}:${i + 1}: ${line.trim()}');
        }
      }
    }
    expect(hits, isEmpty);
  });

  test('every ARB key is used (a missing key fails to compile)', () {
    final String arb = File('lib/src/l10n/app_en.arb').readAsStringSync();
    final Set<String> keys = RegExp(
      r'^  "([a-zA-Z0-9]+)":',
      multiLine: true,
    ).allMatches(arb).map((RegExpMatch m) => m.group(1)!).toSet();
    final String all = sources.map((File f) => f.readAsStringSync()).join('\n');
    final Set<String> unused = keys
        .where((String k) => !RegExp('\\b$k\\b').hasMatch(all))
        .toSet();
    expect(unused, isEmpty, reason: 'unused strings');
  });

  test('app/ pins the Flutter version', () {
    expect(File('.flutter-version').readAsStringSync().trim(), '3.47.7');
    expect(
      File('pubspec.yaml').readAsStringSync(),
      contains('flutter: 3.47.7'),
    );
  });
}
