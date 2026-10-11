import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:go_router/go_router.dart';

import '../../api/dtos.dart';
import '../../state/health_state.dart';
import '../../state/routing.dart';
import '../../state/vault_query.dart';
import '../../theme/app_theme.dart';
import '../widgets/error_text.dart';
import '../widgets/l10n_ext.dart';
import '../widgets/strength_meter.dart';
import '../widgets/tool_page.dart';

/// Tools -> Password health (docs/07 §6): reused, weak and old passwords. Shows
/// item titles and scores only; no password is ever displayed here.
class HealthScreen extends ConsumerWidget {
  const HealthScreen({this.onOpenItem, super.key});

  /// Called when an entry is tapped. The default selects the item in the vault
  /// and goes back to it.
  final void Function(String itemId)? onOpenItem;

  void _open(BuildContext context, WidgetRef ref, String id) {
    if (onOpenItem != null) {
      onOpenItem!(id);
      return;
    }
    ref.read(vaultQueryProvider.notifier).select(id);
    context.go(AppRoutes.home);
  }

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final l10n = context.l10n;
    final AppTokens tokens = context.tokens;
    final AsyncValue<HealthReport> report = ref.watch(healthReportProvider);
    void open(String id) => _open(context, ref, id);
    return ToolPage(
      title: l10n.healthTitle,
      children: <Widget>[
        Text(l10n.healthIntro),
        SizedBox(height: tokens.gapLarge),
        if (report.isLoading && !report.hasValue)
          Center(
            child: Semantics(
              label: l10n.healthLoading,
              child: const CircularProgressIndicator(),
            ),
          )
        else if (report.hasError)
          Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: <Widget>[
              ErrorText(l10n.errInternal),
              TextButton(
                onPressed: () => ref.invalidate(healthReportProvider),
                child: Text(l10n.actionTryAgain),
              ),
            ],
          )
        else ...<Widget>[
          _ReusedSection(report: report.requireValue, onOpen: open),
          _WeakSection(report: report.requireValue, onOpen: open),
          _OldSection(report: report.requireValue, onOpen: open),
        ],
      ],
    );
  }
}

class _ItemTile extends ConsumerWidget {
  const _ItemTile({required this.itemId, required this.onOpen, this.subtitle});

  final String itemId;
  final String? subtitle;
  final void Function(String id) onOpen;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final l10n = context.l10n;
    final AsyncValue<String?> title = ref.watch(itemTitleProvider(itemId));
    final String? name = title.value;
    final bool gone = title.hasValue && name == null;
    final String text = gone ? l10n.healthItemGone : (name ?? '…');
    return ListTile(
      key: ValueKey<String>('health-$itemId'),
      contentPadding: EdgeInsets.zero,
      title: Text(text, maxLines: 2, overflow: TextOverflow.ellipsis),
      subtitle: subtitle == null ? null : Text(subtitle!),
      trailing: const Icon(Icons.chevron_right),
      enabled: !gone,
      onTap: gone ? null : () => onOpen(itemId),
    );
  }
}

class _ReusedSection extends StatelessWidget {
  const _ReusedSection({required this.report, required this.onOpen});
  final HealthReport report;
  final void Function(String id) onOpen;

  @override
  Widget build(BuildContext context) {
    final l10n = context.l10n;
    return ToolSection(
      title: l10n.healthReusedTitle,
      explain: l10n.healthReusedExplain,
      children: <Widget>[
        if (report.reused.isEmpty)
          Text(l10n.healthReusedNone)
        else
          for (final ReuseGroup g in report.reused) ...<Widget>[
            Semantics(
              header: true,
              child: Text(
                l10n.healthReusedGroup(g.itemIds.length),
                style: Theme.of(context).textTheme.labelLarge,
              ),
            ),
            for (final String id in g.itemIds)
              _ItemTile(itemId: id, onOpen: onOpen),
            const Divider(),
          ],
      ],
    );
  }
}

class _WeakSection extends StatelessWidget {
  const _WeakSection({required this.report, required this.onOpen});
  final HealthReport report;
  final void Function(String id) onOpen;

  @override
  Widget build(BuildContext context) {
    final l10n = context.l10n;
    return ToolSection(
      title: l10n.healthWeakTitle,
      explain: l10n.healthWeakExplain,
      children: <Widget>[
        if (report.weak.isEmpty)
          Text(l10n.healthWeakNone)
        else
          for (final WeakPassword w in report.weak)
            _ItemTile(
              itemId: w.itemId,
              onOpen: onOpen,
              subtitle: l10n.healthWeakScore(
                StrengthMeter.levelLabel(context, w.score),
              ),
            ),
      ],
    );
  }
}

class _OldSection extends StatelessWidget {
  const _OldSection({required this.report, required this.onOpen});
  final HealthReport report;
  final void Function(String id) onOpen;

  @override
  Widget build(BuildContext context) {
    final l10n = context.l10n;
    return ToolSection(
      title: l10n.healthOldTitle,
      explain: l10n.healthOldExplain,
      children: <Widget>[
        if (report.old.isEmpty)
          Text(l10n.healthOldNone)
        else
          for (final OldPassword o in report.old)
            _ItemTile(
              itemId: o.itemId,
              onOpen: onOpen,
              subtitle: l10n.healthOldAge(o.ageDays),
            ),
      ],
    );
  }
}
