import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../state/vault_actions.dart';
import '../../theme/app_theme.dart';
import '../shell/adaptive_shell.dart';
import '../widgets/l10n_ext.dart';

/// Placeholder home (B02 fills in the vault screens): an unlocked message and a
/// Lock button inside the adaptive shell.
class HomeScreen extends ConsumerWidget {
  const HomeScreen({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final l10n = context.l10n;
    final AppTokens tokens = context.tokens;
    final Widget lockButton = FilledButton.icon(
      onPressed: () => ref.read(vaultActionsProvider).lock(),
      icon: const Icon(Icons.lock),
      label: Text(l10n.actionLock),
    );
    return AdaptiveShell(
      title: l10n.homeTitle,
      actions: <Widget>[lockButton],
      listPane: Center(child: Text(l10n.homeListPlaceholder)),
      detailPane: Center(
        child: Padding(
          padding: EdgeInsets.all(tokens.gap),
          child: Column(
            mainAxisSize: MainAxisSize.min,
            children: <Widget>[
              Semantics(
                liveRegion: true,
                child: Text(
                  l10n.homeUnlockedMessage,
                  style: Theme.of(context).textTheme.titleMedium,
                ),
              ),
              SizedBox(height: tokens.gap),
              Text(l10n.homeDetailPlaceholder),
            ],
          ),
        ),
      ),
    );
  }
}
