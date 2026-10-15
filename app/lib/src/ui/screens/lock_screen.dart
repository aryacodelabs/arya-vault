import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:go_router/go_router.dart';

import '../../api/app_error.dart';
import '../../api/dtos.dart';
import '../../state/routing.dart';
import '../../state/vault_actions.dart';
import '../../state/vault_status.dart';
import '../../theme/app_theme.dart';
import '../widgets/error_text.dart';
import '../widgets/l10n_ext.dart';
import '../widgets/page_scaffold.dart';
import '../widgets/secret_text_field.dart';
import 'quick_unlock_offer_screen.dart';

/// The lock screen (docs/07 §4, §6).
class LockScreen extends ConsumerStatefulWidget {
  const LockScreen({super.key});

  @override
  ConsumerState<LockScreen> createState() => _LockScreenState();
}

class _LockScreenState extends ConsumerState<LockScreen> {
  final SecretTextController _password = SecretTextController();
  bool _busy = false;
  String? _error;

  @override
  void dispose() {
    _password.clearSecret();
    _password.dispose();
    super.dispose();
  }

  Future<void> _run(Future<void> Function() action) async {
    setState(() {
      _busy = true;
      _error = null;
    });
    final l10n = context.l10n;
    try {
      await action();
    } on AppError catch (e) {
      // The user closed the system prompt: nothing went wrong, so show nothing.
      if (mounted && e.code != AppErrorCode.quickUnlockCancelled) {
        setState(() => _error = errorMessage(l10n, e));
      }
    } finally {
      if (mounted) {
        setState(() => _busy = false);
      }
    }
  }

  Future<void> _unlock() {
    if (_busy || _password.text.isEmpty) {
      return Future<void>.value();
    }
    return _run(
      () => ref.read(vaultActionsProvider).unlock(_password.takeSecret()),
    );
  }

  @override
  Widget build(BuildContext context) {
    final l10n = context.l10n;
    final AppTokens tokens = context.tokens;
    final QuickUnlockStatus quick =
        ref.watch(vaultStatusProvider).value?.quickUnlock ??
        QuickUnlockStatus.unsupported;
    return PageScaffold(
      title: l10n.lockTitle,
      children: <Widget>[
        if (quick.enabled) ...<Widget>[
          OutlinedButton.icon(
            autofocus: true,
            onPressed: _busy
                ? null
                : () => _run(ref.read(vaultActionsProvider).unlockQuick),
            icon: const Icon(Icons.fingerprint),
            label: Text(
              l10n.lockQuick(quickUnlockMethodName(l10n, quick.kind)),
            ),
          ),
          SizedBox(height: tokens.gap),
        ],
        SecretTextField(
          controller: _password,
          label: l10n.masterPasswordLabel,
          autofocus: !quick.enabled,
          textInputAction: TextInputAction.done,
          onSubmitted: _unlock,
        ),
        if (_error != null) ...<Widget>[
          SizedBox(height: tokens.gap),
          ErrorText(_error!),
        ],
        SizedBox(height: tokens.gapLarge),
        FilledButton(
          onPressed: _busy ? null : _unlock,
          child: _busy
              ? Semantics(
                  liveRegion: true,
                  label: l10n.lockWorking,
                  child: const SizedBox(
                    height: 20,
                    width: 20,
                    child: CircularProgressIndicator(strokeWidth: 2),
                  ),
                )
              : Text(l10n.lockSubmit),
        ),
        SizedBox(height: tokens.gap),
        TextButton(
          onPressed: _busy ? null : () => context.go(AppRoutes.recover),
          child: Text(l10n.lockUseRecovery),
        ),
      ],
    );
  }
}
