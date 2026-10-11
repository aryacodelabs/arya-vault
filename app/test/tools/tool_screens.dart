import 'dart:convert';
import 'dart:typed_data';

import 'package:arya_vault/src/api/dtos.dart';
import 'package:arya_vault/src/state/routing.dart';
import 'package:flutter/material.dart' hide Page;
import 'package:flutter_test/flutter_test.dart';

import '../helpers.dart';
import 'tool_helpers.dart';

/// The B03 screens the golden tests photograph, in a fixed state: seeded
/// randomness, a fixed clock, fake canary data.
const List<String> kToolScreens = <String>[
  'generator',
  'health',
  'security-settings',
  'import-preview',
];

Future<Harness> pumpToolScreen(
  WidgetTester tester,
  String name, {
  Size size = const Size(420, 900),
  ThemeMode themeMode = ThemeMode.light,
  double textScale = 1,
}) async {
  final DateTime fixed = DateTime.utc(2026, 6, 1, 12);
  DateTime now = DateTime.utc(2025, 1, 1);
  final BufferSpyApi api = await unlockedBufferVault(clock: () => now);
  switch (name) {
    case 'generator':
      return pumpRoute(
        tester,
        AppRoutes.generator,
        api: api,
        size: size,
        themeMode: themeMode,
        textScale: textScale,
      );
    case 'health':
      Future<void> login(String title, String pw) => api.createItem(
        NewItem(
          type: ItemType.login,
          title: title,
          fields: <StdField, String>{StdField.password: pw},
        ),
      );
      await login('CANARY Old Bank', 'CANARY-Old-Unique-Pw-4417!x');
      now = fixed;
      await login('CANARY Shared One', 'CANARY-Shared-Pw-9921!zq');
      await login('CANARY Shared Two', 'CANARY-Shared-Pw-9921!zq');
      await login('CANARY Weak Site', 'password1');
      return pumpRoute(
        tester,
        AppRoutes.health,
        api: api,
        size: size,
        themeMode: themeMode,
        textScale: textScale,
      );
    case 'security-settings':
      return pumpRoute(
        tester,
        AppRoutes.settingsSecurity,
        api: api,
        size: size,
        themeMode: themeMode,
        textScale: textScale,
      );
    case 'import-preview':
      final Services s = Services()
        ..filePicker.pickResult = Uint8List.fromList(
          utf8.encode(
            'name,username,password,url,notes,folder\n'
            'CANARY Imported One,alice@example.invalid,CANARY-imp-pw-1,,,Imports\n'
            'CANARY Imported Two,bob@example.invalid,,,,\n'
            ',nobody,CANARY-imp-pw-3,,,\n',
          ),
        );
      final Harness h = await pumpRoute(
        tester,
        AppRoutes.importFile,
        api: api,
        services: s,
        size: size,
        themeMode: themeMode,
        textScale: textScale,
      );
      await tapOn(tester, find.text(l10n.importChoose));
      return h;
  }
  throw ArgumentError('unknown tool screen $name');
}
