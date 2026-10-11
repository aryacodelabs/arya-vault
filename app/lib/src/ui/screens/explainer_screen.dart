import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../state/onboarding_state.dart';
import '../../theme/app_theme.dart';
import '../widgets/l10n_ext.dart';
import '../widgets/page_scaffold.dart';

/// docs/07 §3 step E: "no reset exists", said plainly and once.
class ExplainerScreen extends ConsumerWidget {
  const ExplainerScreen({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final l10n = context.l10n;
    final AppTokens tokens = context.tokens;
    final ThemeData theme = Theme.of(context);
    return PageScaffold(
      title: l10n.explainerTitle,
      children: <Widget>[
        Container(
          padding: EdgeInsets.all(tokens.gap),
          decoration: BoxDecoration(
            color: tokens.warningBackground,
            borderRadius: BorderRadius.circular(8),
          ),
          child: Text(
            l10n.explainerBody,
            style: theme.textTheme.bodyLarge?.copyWith(color: tokens.onWarning),
          ),
        ),
        SizedBox(height: tokens.gap),
        Text(l10n.explainerPoint),
        SizedBox(height: tokens.gapLarge),
        FilledButton(
          autofocus: true,
          onPressed: () =>
              ref.read(onboardingProvider.notifier).acknowledgeExplainer(),
          child: Text(l10n.explainerContinue),
        ),
      ],
    );
  }
}
