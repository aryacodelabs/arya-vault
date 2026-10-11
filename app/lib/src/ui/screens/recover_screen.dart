import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:go_router/go_router.dart';

import '../../api/app_error.dart';
import '../../api/secret_bytes.dart';
import '../../state/providers.dart';
import '../../state/routing.dart';
import '../../state/vault_actions.dart';
import '../../theme/app_theme.dart';
import '../widgets/error_text.dart';
import '../widgets/l10n_ext.dart';
import '../widgets/new_password_fields.dart';
import '../widgets/page_scaffold.dart';
import '../widgets/secret_text_field.dart';

/// docs/07 §4 "Forgot master password": recovery key, then a new master
/// password. The key is passed to the core as typed; the core parses and
/// checksums it (no parsing in Dart), so the only hint here is static text.
class RecoverScreen extends ConsumerStatefulWidget {
  const RecoverScreen({super.key});

  @override
  ConsumerState<RecoverScreen> createState() => _RecoverScreenState();
}

class _RecoverScreenState extends ConsumerState<RecoverScreen> {
  final SecretTextController _key = SecretTextController();
  late final NewPasswordController _passwords;
  bool _enteringPassword = false;
  bool _busy = false;
  String? _error;

  @override
  void initState() {
    super.initState();
    _passwords = NewPasswordController(ref.read(vaultApiProvider));
  }

  @override
  void dispose() {
    _key.clearSecret();
    _key.dispose();
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
    final SecretBytes keyBytes = _key.takeSecret();
    try {
      await ref
          .read(vaultActionsProvider)
          .recoverWithKey(keyBytes, _passwords.takePassword());
      _key.clearSecret();
      _passwords.clear();
    } on AppError catch (e) {
      keyBytes.wipe();
      if (mounted) {
        setState(() {
          // A key problem sends the user back to the key step.
          if (e.code == AppErrorCode.wrongCredentials ||
              e.code == AppErrorCode.recoveryKeyMalformed) {
            _enteringPassword = false;
          }
          _error = errorMessage(l10n, e, context: ErrorContext.recoveryKey);
        });
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
    final Widget back = Align(
      alignment: AlignmentDirectional.centerStart,
      child: TextButton.icon(
        onPressed: _busy
            ? null
            : () {
                if (_enteringPassword) {
                  setState(() => _enteringPassword = false);
                } else {
                  context.go(AppRoutes.lock);
                }
              },
        icon: const Icon(Icons.arrow_back),
        label: Text(l10n.actionBack),
      ),
    );
    if (!_enteringPassword) {
      return PageScaffold(
        title: l10n.recoverTitle,
        leading: back,
        children: <Widget>[
          SecretTextField(
            controller: _key,
            label: l10n.recoverKeyLabel,
            helperText: l10n.recoverKeyHint,
            autofocus: true,
            textInputAction: TextInputAction.next,
            onChanged: () => setState(() {}),
            onSubmitted: () => _goToPassword(),
          ),
          if (_error != null) ...<Widget>[
            SizedBox(height: tokens.gap),
            ErrorText(_error!),
          ],
          SizedBox(height: tokens.gapLarge),
          FilledButton(
            onPressed: _key.text.trim().isEmpty ? null : _goToPassword,
            child: Text(l10n.actionContinue),
          ),
          SizedBox(height: tokens.gap),
          TextButton(
            onPressed: () => context.go(AppRoutes.forgot),
            child: Text(l10n.recoverNoKey),
          ),
        ],
      );
    }
    return PageScaffold(
      title: l10n.recoverNewPasswordTitle,
      leading: back,
      children: <Widget>[
        NewPasswordFields(controller: _passwords, onSubmitted: _submit),
        if (_error != null) ...<Widget>[
          SizedBox(height: tokens.gap),
          ErrorText(_error!),
        ],
        SizedBox(height: tokens.gapLarge),
        ListenableBuilder(
          listenable: _passwords,
          builder: (BuildContext context, _) => FilledButton(
            onPressed: _busy || !_passwords.canSubmit ? null : _submit,
            child: Text(l10n.recoverSubmit),
          ),
        ),
      ],
    );
  }

  void _goToPassword() {
    if (_key.text.trim().isEmpty) {
      return;
    }
    setState(() {
      _error = null;
      _enteringPassword = true;
    });
  }
}
