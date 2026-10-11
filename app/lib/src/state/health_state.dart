import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../api/app_error.dart';
import '../api/dtos.dart';
import 'providers.dart';
import 'vault_session.dart';

/// The password-health report (`healthReport`, unlocked only). Recomputed each
/// time the screen opens; it holds item ids and scores, never a password.
final FutureProvider<HealthReport> healthReportProvider =
    FutureProvider.autoDispose<HealthReport>((Ref ref) async {
      ref.watch(lockEpochProvider);
      try {
        return await ref.read(vaultApiProvider).healthReport();
      } on AppError catch (e) {
        handleLocked(ref, e);
        rethrow;
      }
    });

/// The title of one item, for the health lists. Null if the item is gone.
final itemTitleProvider = FutureProvider.autoDispose.family<String?, String>((
  Ref ref,
  String id,
) async {
  ref.watch(lockEpochProvider);
  try {
    return (await ref.read(vaultApiProvider).getItem(id)).title;
  } on AppError catch (e) {
    if (e.code == AppErrorCode.notFound) return null;
    handleLocked(ref, e);
    rethrow;
  }
});

/// The core version and friends, for About.
final FutureProvider<InfoDto> infoProvider =
    FutureProvider.autoDispose<InfoDto>(
      (Ref ref) => ref.read(vaultApiProvider).info(),
    );
