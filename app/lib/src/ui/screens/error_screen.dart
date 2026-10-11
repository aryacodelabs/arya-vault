import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../api/app_error.dart';
import '../../state/vault_status.dart';
import '../widgets/error_text.dart';
import '../widgets/l10n_ext.dart';
import '../widgets/page_scaffold.dart';

/// Shown when `status()` itself fails (corrupt or too-new vault, I/O).
class ErrorScreen extends ConsumerWidget {
  const ErrorScreen({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final l10n = context.l10n;
    final Object? error = ref.watch(vaultStatusProvider).error;
    return PageScaffold(
      title: l10n.loadFailed,
      children: <Widget>[
        if (error is AppError) ErrorText(errorMessage(l10n, error)),
        const SizedBox(height: 16),
        FilledButton(
          onPressed: () => ref.read(vaultStatusProvider.notifier).refresh(),
          child: Text(l10n.actionTryAgain),
        ),
      ],
    );
  }
}
