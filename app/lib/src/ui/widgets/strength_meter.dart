import 'package:flutter/material.dart';

import '../../api/dtos.dart';
import '../../theme/app_theme.dart';
import 'l10n_ext.dart';

/// Five-segment strength bar with a text label (never color alone) and the
/// policy messages from `checkMasterPassword`.
class StrengthMeter extends StatelessWidget {
  const StrengthMeter({
    required this.strength,
    required this.policy,
    this.showEmptyHint = true,
    super.key,
  });

  /// Null while the field is empty or the first check is pending.
  final Strength? strength;
  final PolicyResult? policy;
  final bool showEmptyHint;

  static String levelLabel(BuildContext context, int score) {
    final l10n = context.l10n;
    return switch (score) {
      0 => l10n.strengthVeryWeak,
      1 => l10n.strengthWeak,
      2 => l10n.strengthFair,
      3 => l10n.strengthGood,
      _ => l10n.strengthStrong,
    };
  }

  @override
  Widget build(BuildContext context) {
    final ThemeData theme = Theme.of(context);
    final ColorScheme scheme = theme.colorScheme;
    final l10n = context.l10n;
    final int? score = strength?.score;
    final String? label = score == null ? null : levelLabel(context, score);
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: <Widget>[
        Semantics(
          container: true,
          label: label == null
              ? l10n.strengthLabel
              : l10n.strengthSemantics(label),
          child: ExcludeSemantics(
            child: Row(
              children: <Widget>[
                for (var i = 0; i < 5; i++)
                  Expanded(
                    child: Container(
                      height: 8,
                      margin: EdgeInsets.only(right: i == 4 ? 0 : 4),
                      decoration: BoxDecoration(
                        color: score != null && i <= score
                            ? scheme.primary
                            : scheme.surfaceContainerHighest,
                        border: Border.all(color: scheme.outline),
                        borderRadius: BorderRadius.circular(4),
                      ),
                    ),
                  ),
              ],
            ),
          ),
        ),
        SizedBox(height: context.tokens.gap / 4),
        Text(
          label == null ? l10n.strengthLabel : '${l10n.strengthLabel}: $label',
          style: theme.textTheme.bodySmall,
        ),
        if (policy != null) ...<Widget>[
          SizedBox(height: context.tokens.gap / 4),
          if (policy!.acceptable)
            Text(l10n.policyMet, style: theme.textTheme.bodySmall)
          else
            for (final String reason in policy!.reasons)
              Text(reason, style: theme.textTheme.bodySmall),
        ] else if (showEmptyHint)
          Text(l10n.policyEmpty, style: theme.textTheme.bodySmall),
      ],
    );
  }
}
