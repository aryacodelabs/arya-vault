@Tags(<String>['ffi'])
library;

import 'dart:io';

import 'package:arya_vault/src/api/ffi_vault_api.dart';
import 'package:arya_vault/src/api/vault_api.dart';
import 'package:flutter_test/flutter_test.dart';

import 'contract_suite.dart';

/// The docs/14 contract suite against the real core (L01 goal 1).
///
/// Needs the release library: build it with `cargo build -p arya-vault-ffi --release`
/// in `core/` and point `ARYA_FFI_LIB` at `core/target/release/arya_vault_ffi.dll`.
/// Without that variable the suite is skipped, so plain `flutter test` still passes
/// on machines (and CI jobs) that have no Rust build.
///
/// The core allows one vault directory per process, so every scenario shares one
/// temporary directory, which is deleted at the end.
void main() {
  final String? lib = Platform.environment['ARYA_FFI_LIB'];
  if (lib == null || lib.isEmpty) {
    test(
      'FfiVaultApi contract suite',
      () {},
      skip: 'set ARYA_FFI_LIB to the built arya_vault_ffi library',
    );
    return;
  }
  late Directory dir;
  VaultApi? opened;
  setUpAll(() => dir = Directory.systemTemp.createTempSync('aryavault_ffi_'));
  tearDownAll(() async {
    // The core keeps the database file open until the vault is locked.
    await opened?.lock();
    try {
      dir.deleteSync(recursive: true);
    } on FileSystemException {
      // Windows can hold the handle a moment longer; the OS temp cleaner takes it.
    }
  });

  runContractSuite('FfiVaultApi', () async {
    final VaultApi api = await FfiVaultApi.open(
      vaultDir: '${dir.path}${Platform.pathSeparator}vault',
      libraryPath: lib,
    );
    opened = api;
    return api;
  });
}
