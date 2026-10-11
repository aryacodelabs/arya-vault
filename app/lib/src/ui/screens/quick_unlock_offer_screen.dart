import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../api/app_error.dart';
import '../../api/dtos.dart';
import '../../l10n/gen/app_localizations.dart';
import '../../state/vault_actions.dart';
import '../../state/vault_status.dart';
import '../../theme/app_theme.dart';
import '../widgets/error_text.dart';
import '../widgets/l10n_ext.dart';
import '../widgets/page_scaffold.dart';

/// Localized name of a quick-unlock mechanism.
String quickUnlockMethodName(AppLocalizations l10n, QuickUnlockKind kind) =>
    switch (kind) {
      QuickUnlockKind.windowsHello => l10n.quickKindWindowsHello,
      QuickUnlockKind.touchId => l10n.quickKindTouchId,
      QuickUnlockKind.faceId => l10n.quickKindFaceId,
      QuickUnlockKind.biometric => l10n.quickKindBiometric,
      QuickUnlockKind.osKeyring => l10n.quickKindOsKeyring,
      QuickUnlockKind.none => l10n.quickKindNone,
    };

/// Optional onboarding step (docs/07 §3 step I); shown only when
/// `status.quickUnlock.supported`.
class QuickUnlockOfferScreen extends ConsumerStatefulWidget {
  const QuickUnlockOfferScreen({super.key});

  @override
  ConsumerState<QuickUnlockOfferScreen> createState() =>
      _QuickUnlockOfferScreenState();
}

class _QuickUnlockOfferScreenState
    extends ConsumerState<QuickUnlockOfferScreen> {
  bool _busy = false;
  String? _error;

  Future<void> _enable() async {
    setState(() {
      _busy = true;
      _error = null;
    });
    final l10n = context.l10n;
    try {
      await ref.read(vaultActionsProvider).enableQuickUnlock();
    } on AppError catch (e) {
      if (mounted) {
        setState(() => _error = errorMessage(l10n, e));
      }
    } finally {
      if (mounted) {
        setState(() => _busy = false);
      }
    }
  }

  @override
  Widget build(BuildContext context) {
    final l10n = context.l10n;
    final AppTokens tokens = context.tokens;
    final QuickUnlockKind kind =
        ref.watch(vaultStatusProvider).value?.quickUnlock.kind ??
        QuickUnlockKind.none;
    return PageScaffold(
      title: l10n.quickOfferTitle,
      children: <Widget>[
        Text(l10n.quickOfferBody(quickUnlockMethodName(l10n, kind))),
        if (_error != null) ...<Widget>[
          SizedBox(height: tokens.gap),
          ErrorText(_error!),
        ],
        SizedBox(height: tokens.gapLarge),
        FilledButton(
          autofocus: true,
          onPressed: _busy ? null : _enable,
          child: Text(l10n.quickEnable),
        ),
        SizedBox(height: tokens.gap),
        TextButton(
          onPressed: _busy
              ? null
              : ref.read(vaultActionsProvider).skipQuickUnlockOffer,
          child: Text(l10n.quickNotNow),
        ),
      ],
    );
  }
}
