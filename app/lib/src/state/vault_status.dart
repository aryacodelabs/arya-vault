import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../api/dtos.dart';
import 'providers.dart';

/// The vault's `VaultStatus` (works while locked). Drives all routing.
class VaultStatusNotifier extends AsyncNotifier<VaultStatus> {
  @override
  Future<VaultStatus> build() => ref.read(vaultApiProvider).status();

  /// Re-reads the status without flashing a loading state.
  Future<void> refresh() async {
    state = await AsyncValue.guard(ref.read(vaultApiProvider).status);
  }

  /// Publishes a status the caller just fetched.
  void publish(VaultStatus status) => state = AsyncData<VaultStatus>(status);
}

final AsyncNotifierProvider<VaultStatusNotifier, VaultStatus>
vaultStatusProvider = AsyncNotifierProvider<VaultStatusNotifier, VaultStatus>(
  VaultStatusNotifier.new,
);
