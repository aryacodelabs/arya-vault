import 'package:flutter/material.dart';
import 'package:go_router/go_router.dart';

import '../../state/routing.dart';
import '../../theme/app_theme.dart';
import '../widgets/l10n_ext.dart';
import '../widgets/page_scaffold.dart';

/// docs/07 §4 "Forgot both": plain, honest, no false hope.
class ForgotBothScreen extends StatelessWidget {
  const ForgotBothScreen({super.key});

  @override
  Widget build(BuildContext context) {
    final l10n = context.l10n;
    final AppTokens tokens = context.tokens;
    return PageScaffold(
      title: l10n.forgotTitle,
      leading: Align(
        alignment: AlignmentDirectional.centerStart,
        child: TextButton.icon(
          onPressed: () => context.go(AppRoutes.lock),
          icon: const Icon(Icons.arrow_back),
          label: Text(l10n.actionBack),
        ),
      ),
      children: <Widget>[
        Text(l10n.forgotBody1),
        SizedBox(height: tokens.gap),
        Text(l10n.forgotBody2),
        SizedBox(height: tokens.gap),
        Text(l10n.forgotBody3),
      ],
    );
  }
}
