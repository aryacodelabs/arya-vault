import 'dart:io';

import 'package:flutter_test/flutter_test.dart';

/// `VaultApi` must be docs/14 §4 one to one: every row present, nothing extra.
/// The expected names are the operations listed in §4.1 to §4.6.
const Map<String, List<String>> _contract = <String, List<String>>{
  '4.1 lifecycle and credentials': <String>[
    'status',
    'createVault',
    'confirmRecoveryKey',
    'unlock',
    'unlockQuick',
    'lock',
    'changePassword',
    'recoverWithKey',
    'regenerateRecoveryKey',
    'quickUnlockEnable',
    'quickUnlockDisable',
    'verifyPassword',
  ],
  '4.2 items and folders': <String>[
    'list',
    'search',
    'itemCount',
    'getItem',
    'reveal',
    'revealCustom',
    'createItem',
    'setField',
    'clearField',
    'setFields',
    'toggleFavorite',
    'moveToFolder',
    'addTag',
    'removeTag',
    'addUrl',
    'setUrl',
    'removeUrl',
    'addCustomField',
    'setCustomValue',
    'setCustomLabel',
    'removeCustomField',
    'deleteItem',
    'restoreItem',
    'purgeItem',
    'listTrash',
    'emptyTrash',
    'purgeExpired',
    'history',
    'revealVersion',
    'restoreVersion',
    'createFolder',
    'renameFolder',
    'deleteFolder',
    'listFolders',
  ],
  '4.3 generator, strength, health': <String>[
    'generatePassword',
    'generatePassphrase',
    'entropyBits',
    'strength',
    'checkMasterPassword',
    'healthReport',
  ],
  '4.4 import and export': <String>[
    'importPreview',
    'importCommit',
    'exportCsv',
    'exportEncrypted',
    'importEncrypted',
  ],
  '4.5 settings': <String>['getSettings', 'setSettings'],
  '4.6 diagnostics': <String>['info', 'exportDiagnostics'],
};

void main() {
  test('VaultApi declares exactly the operations of docs/14 §4', () {
    final String source = File('lib/src/api/vault_api.dart').readAsStringSync();
    final List<String> declared = RegExp(
      r'^\s+Future<.*>\s+(\w+)\(',
      multiLine: true,
    ).allMatches(source).map((RegExpMatch m) => m.group(1)!).toList();
    final List<String> expected = <String>[
      for (final List<String> names in _contract.values) ...names,
    ];
    expect(declared.toSet(), expected.toSet());
    expect(
      declared,
      hasLength(declared.toSet().length),
      reason: 'no overloads',
    );
    expect(declared, hasLength(61));
  });

  test('declaration order follows the contract sections', () {
    final String source = File('lib/src/api/vault_api.dart').readAsStringSync();
    final List<String> declared = RegExp(
      r'^\s+Future<.*>\s+(\w+)\(',
      multiLine: true,
    ).allMatches(source).map((RegExpMatch m) => m.group(1)!).toList();
    expect(declared, <String>[
      for (final List<String> names in _contract.values) ...names,
    ]);
  });
}
