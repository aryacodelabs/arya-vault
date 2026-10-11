import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:go_router/go_router.dart';

import '../../api/app_error.dart';
import '../../api/dtos.dart';
import '../../state/providers.dart';
import '../../state/routing.dart';
import '../../state/vault_actions.dart';
import '../../theme/app_theme.dart';
import '../widgets/error_text.dart';
import '../widgets/l10n_ext.dart';
import '../widgets/new_password_fields.dart';
import '../widgets/page_scaffold.dart';

/// docs/07 §3 steps C-D: master password, confirmation, live strength, and the
/// KDF profile hidden under "Advanced".
class CreateVaultScreen extends ConsumerStatefulWidget {
  const CreateVaultScreen({super.key});

  @override
  ConsumerState<CreateVaultScreen> createState() => _CreateVaultScreenState();
}

class _CreateVaultScreenState extends ConsumerState<CreateVaultScreen> {
  late final NewPasswordController _passwords;
  KdfProfile _profile = KdfProfile.default_;
  bool _busy = false;
  String? _error;

  @override
  void initState() {
    super.initState();
    _passwords = NewPasswordController(ref.read(vaultApiProvider));
  }

  @override
  void dispose() {
    _passwords.dispose();
    super.dispose();
  }

  Future<void> _submit() async {
    if (_busy || !_passwords.canSubmit) {
      return;
    }
    setState(() {
      _busy = true;
      _error = null;
    });
    final l10n = context.l10n;
    try {
      await ref
          .read(vaultActionsProvider)
          .createVault(_passwords.takePassword(), _profile);
      _passwords.clear();
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

  String _profileLabel(KdfProfile p) {
    final l10n = context.l10n;
    return switch (p) {
      KdfProfile.low => l10n.kdfLow,
      KdfProfile.default_ => l10n.kdfDefault,
      KdfProfile.high => l10n.kdfHigh,
    };
  }

  @override
  Widget build(BuildContext context) {
    final l10n = context.l10n;
    final AppTokens tokens = context.tokens;
    return PageScaffold(
      title: l10n.createTitle,
      leading: Align(
        alignment: AlignmentDirectional.centerStart,
        child: TextButton.icon(
          onPressed: _busy ? null : () => context.go(AppRoutes.welcome),
          icon: const Icon(Icons.arrow_back),
          label: Text(l10n.actionBack),
        ),
      ),
      children: <Widget>[
        Text(l10n.createIntro),
        SizedBox(height: tokens.gapLarge),
        NewPasswordFields(controller: _passwords, onSubmitted: _submit),
        SizedBox(height: tokens.gap),
        ExpansionTile(
          title: Text(l10n.advancedTitle),
          childrenPadding: EdgeInsets.all(tokens.gap / 2),
          children: <Widget>[
            Align(
              alignment: AlignmentDirectional.centerStart,
              child: Text(
                l10n.kdfLabel,
                style: Theme.of(context).textTheme.titleSmall,
              ),
            ),
            RadioGroup<KdfProfile>(
              groupValue: _profile,
              onChanged: (KdfProfile? p) =>
                  setState(() => _profile = p ?? _profile),
              child: Column(
                children: <Widget>[
                  for (final KdfProfile p in KdfProfile.values)
                    RadioListTile<KdfProfile>(
                      value: p,
                      title: Text(_profileLabel(p)),
                    ),
                ],
              ),
            ),
            Text(l10n.kdfHelp, style: Theme.of(context).textTheme.bodySmall),
          ],
        ),
        if (_error != null) ...<Widget>[
          SizedBox(height: tokens.gap),
          ErrorText(_error!),
        ],
        SizedBox(height: tokens.gapLarge),
        ListenableBuilder(
          listenable: _passwords,
          builder: (BuildContext context, _) => FilledButton(
            onPressed: _busy || !_passwords.canSubmit ? null : _submit,
            child: _busy
                ? Semantics(
                    liveRegion: true,
                    label: l10n.createWorking,
                    child: const SizedBox(
                      height: 20,
                      width: 20,
                      child: CircularProgressIndicator(strokeWidth: 2),
                    ),
                  )
                : Text(l10n.createSubmit),
          ),
        ),
      ],
    );
  }
}
