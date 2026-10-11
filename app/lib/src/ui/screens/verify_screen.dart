import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../api/app_error.dart';
import '../../api/dtos.dart';
import '../../state/onboarding_state.dart';
import '../../state/vault_actions.dart';
import '../../theme/app_theme.dart';
import '../widgets/error_text.dart';
import '../widgets/l10n_ext.dart';
import '../widgets/page_scaffold.dart';
import '../widgets/secret_text_field.dart';

/// docs/07 §3 step H: re-enter three requested groups of the key.
///
/// There is no skip. Onboarding completes only when the core's
/// `confirmRecoveryKey` returns true (SEC-A01); a mismatch goes back to the key
/// screen. The router makes this structural: while the vault is unlocked and
/// `onboardingComplete` is false, no other location exists.
class VerifyScreen extends ConsumerStatefulWidget {
  const VerifyScreen({super.key});

  @override
  ConsumerState<VerifyScreen> createState() => _VerifyScreenState();
}

class _VerifyScreenState extends ConsumerState<VerifyScreen> {
  late final List<int> _challenge = ref.read(onboardingProvider).challenge;
  late final List<SecretTextController> _controllers = <SecretTextController>[
    for (final int _ in _challenge) SecretTextController(),
  ];
  bool _busy = false;
  String? _error;

  @override
  void dispose() {
    for (final SecretTextController c in _controllers) {
      c.clearSecret();
      c.dispose();
    }
    super.dispose();
  }

  bool get _complete =>
      _controllers.every((SecretTextController c) => c.text.trim().isNotEmpty);

  Future<void> _submit() async {
    if (_busy || !_complete) {
      return;
    }
    setState(() {
      _busy = true;
      _error = null;
    });
    final l10n = context.l10n;
    final List<GroupAnswer> answers = <GroupAnswer>[
      for (var i = 0; i < _challenge.length; i++)
        GroupAnswer(index: _challenge[i], text: _controllers[i].text),
    ];
    try {
      // On success the router leaves this screen; on a mismatch the action
      // sends the user back to the key screen.
      await ref.read(vaultActionsProvider).confirmRecoveryKey(answers);
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
    return PageScaffold(
      title: l10n.verifyTitle,
      children: <Widget>[
        Text(l10n.verifyIntro),
        SizedBox(height: tokens.gapLarge),
        ListenableBuilder(
          listenable: Listenable.merge(_controllers),
          builder: (BuildContext context, _) => Column(
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: <Widget>[
              for (var i = 0; i < _challenge.length; i++) ...<Widget>[
                TextField(
                  controller: _controllers[i],
                  autofocus: i == 0,
                  autocorrect: false,
                  enableSuggestions: false,
                  textCapitalization: TextCapitalization.characters,
                  textInputAction: i == _challenge.length - 1
                      ? TextInputAction.done
                      : TextInputAction.next,
                  onSubmitted: (_) => i == _challenge.length - 1
                      ? _submit()
                      : FocusScope.of(context).nextFocus(),
                  style: const TextStyle(
                    fontFamily: 'monospace',
                    letterSpacing: 2,
                  ),
                  decoration: InputDecoration(
                    labelText: l10n.verifyGroupField(_challenge[i] + 1),
                    helperText: l10n.verifyGroupHelp,
                  ),
                ),
                SizedBox(height: tokens.gap),
              ],
            ],
          ),
        ),
        if (_error != null) ...<Widget>[
          ErrorText(_error!),
          SizedBox(height: tokens.gap),
        ],
        ListenableBuilder(
          listenable: Listenable.merge(_controllers),
          builder: (BuildContext context, _) => FilledButton(
            onPressed: _busy || !_complete ? null : _submit,
            child: Text(l10n.verifySubmit),
          ),
        ),
        SizedBox(height: tokens.gap),
        TextButton(
          onPressed: _busy
              ? null
              : () => ref.read(onboardingProvider.notifier).backToKey(),
          child: Text(l10n.verifyShowKeyAgain),
        ),
      ],
    );
  }
}
