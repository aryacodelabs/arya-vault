import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import 'src/api/dtos.dart';
import 'src/api/fake_vault_api.dart';
import 'src/api/vault_api.dart';
import 'src/app.dart';
import 'src/state/providers.dart';

/// Entry point. Until the FFI-backed implementation is wired in (task L01) the
/// app runs against the in-memory FakeVaultApi: nothing is saved and nothing is
/// encrypted. Quick unlock is simulated so the offer screen can be exercised.
void main() {
  WidgetsFlutterBinding.ensureInitialized();
  final VaultApi api = FakeVaultApi(
    quickUnlock: const QuickUnlockStatus(
      supported: true,
      enabled: false,
      kind: QuickUnlockKind.windowsHello,
    ),
  );
  runApp(
    ProviderScope(
      retry: noAutoRetry,
      overrides: [vaultApiProvider.overrideWithValue(api)],
      child: const AryaVaultApp(),
    ),
  );
}
