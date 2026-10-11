import 'package:flutter/material.dart';
import 'package:go_router/go_router.dart';

import '../../state/routing.dart';
import '../../theme/app_theme.dart';
import '../widgets/l10n_ext.dart';
import '../widgets/page_scaffold.dart';

/// docs/07 §3 step A-B.
class WelcomeScreen extends StatelessWidget {
  const WelcomeScreen({super.key});

  @override
  Widget build(BuildContext context) {
    final l10n = context.l10n;
    final AppTokens tokens = context.tokens;
    return PageScaffold(
      title: l10n.welcomeTitle,
      children: <Widget>[
        Text(l10n.welcomeTagline, style: Theme.of(context).textTheme.bodyLarge),
        SizedBox(height: tokens.gapLarge),
        FilledButton(
          autofocus: true,
          onPressed: () => context.go(AppRoutes.create),
          child: Text(l10n.welcomeCreate),
        ),
        SizedBox(height: tokens.gap),
        // Placeholder until sync exists (M4); disabled on purpose.
        Semantics(
          hint: l10n.welcomeAddDeviceUnavailable,
          child: OutlinedButton(
            onPressed: null,
            child: Text(l10n.welcomeAddDevice),
          ),
        ),
        SizedBox(height: tokens.gap / 2),
        Text(
          l10n.welcomeAddDeviceUnavailable,
          textAlign: TextAlign.center,
          style: Theme.of(context).textTheme.bodySmall,
        ),
      ],
    );
  }
}
