import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import 'src/api/dtos.dart';
import 'src/api/fake_vault_api.dart';
import 'src/api/ffi_vault_api.dart';
import 'src/api/vault_api.dart';
import 'src/app.dart';
import 'src/startup_failed_app.dart';
import 'src/state/providers.dart';

/// `--dart-define=FAKE_API=true` runs the in-memory FakeVaultApi (demos, screenshots):
/// nothing is saved and nothing is encrypted. Without it the app loads the Rust core.
const bool _useFakeApi = bool.fromEnvironment('FAKE_API');

/// Development only: absolute path of `arya_vault_ffi.dll` (a `cargo build` output).
/// Empty means the file next to the executable, the only place a release build looks.
const String _libraryOverride = String.fromEnvironment('ARYA_FFI_LIB');

Future<void> main() async {
  WidgetsFlutterBinding.ensureInitialized();
  final VaultApi api;
  if (_useFakeApi) {
    api = FakeVaultApi(
      quickUnlock: const QuickUnlockStatus(
        supported: true,
        enabled: false,
        kind: QuickUnlockKind.windowsHello,
      ),
    );
  } else {
    try {
      api = await FfiVaultApi.open(
        vaultDir: FfiVaultApi.defaultVaultDir(),
        libraryPath: _libraryOverride.isEmpty ? null : _libraryOverride,
      );
    } on Object {
      runApp(const StartupFailedApp());
      return;
    }
  }
  runApp(
    ProviderScope(
      retry: noAutoRetry,
      overrides: [vaultApiProvider.overrideWithValue(api)],
      child: const AryaVaultApp(),
    ),
  );
}
