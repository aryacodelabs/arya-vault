import 'package:flutter/material.dart';

import '../../theme/app_theme.dart';
import '../widgets/l10n_ext.dart';
import '../widgets/tool_page.dart';

/// Settings -> Sync & backup: an honest "coming soon" panel. There are no
/// toggles because there is nothing behind them yet.
class SyncSettingsScreen extends StatelessWidget {
  const SyncSettingsScreen({super.key});

  @override
  Widget build(BuildContext context) {
    final l10n = context.l10n;
    final AppTokens tokens = context.tokens;
    final ThemeData theme = Theme.of(context);
    return ToolPage(
      title: l10n.syncTitle,
      children: <Widget>[
        Container(
          padding: EdgeInsets.all(tokens.gapLarge),
          decoration: BoxDecoration(
            color: theme.colorScheme.surfaceContainerHighest,
            borderRadius: BorderRadius.circular(12),
          ),
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: <Widget>[
              ExcludeSemantics(
                child: Icon(
                  Icons.cloud_off_outlined,
                  size: 32,
                  color: theme.colorScheme.onSurfaceVariant,
                ),
              ),
              SizedBox(height: tokens.gap / 2),
              Semantics(
                header: true,
                child: Text(
                  l10n.syncComingSoon,
                  style: theme.textTheme.titleMedium,
                ),
              ),
              SizedBox(height: tokens.gap / 2),
              Text(l10n.syncBody),
            ],
          ),
        ),
      ],
    );
  }
}
