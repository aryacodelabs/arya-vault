import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:go_router/go_router.dart';

import '../../api/app_error.dart';
import '../../state/providers.dart';
import '../../state/settings_state.dart';
import '../../state/vault_session.dart';
import '../../theme/app_theme.dart';
import '../widgets/error_text.dart';
import '../widgets/l10n_ext.dart';
import '../widgets/new_password_fields.dart';
import '../widgets/secret_text_field.dart';
import '../widgets/tool_page.dart';

/// Settings -> Security -> Change master password (docs/07 §2, SEC-A06).
///
/// Old password, new password with the core's policy meter, and an opt-in
/// "also rotate keys". With rotation the core leaves a new recovery key
/// pending and the router continues with the recovery-key steps; without it the
/// page closes with a confirmation.
class ChangePasswordScreen extends ConsumerStatefulWidget {
  const ChangePasswordScreen({super.key});

  @override
  ConsumerState<ChangePasswordScreen> createState() =>
      _ChangePasswordScreenState();
}

class _ChangePasswordScreenState extends ConsumerState<ChangePasswordScreen> {
  final SecretTextController _old = SecretTextController();
  late final NewPasswordController _new = NewPasswordController(
    ref.read(vaultApiProvider),
  );
  bool _rotate = false;
  bool _busy = false;
  String? _oldError;
  String? _error;

  @override
  void dispose() {
    _old.clearSecret();
    _old.dispose();
    _new.dispose();
    super.dispose();
  }

  Future<void> _submit() async {
    if (_busy || _old.text.isEmpty || !_new.canSubmit) return;
    final l10n = context.l10n;
    final ScaffoldMessengerState messenger = ScaffoldMessenger.of(context);
    final GoRouter router = GoRouter.of(context);
    setState(() {
      _busy = true;
      _oldError = null;
      _error = null;
    });
    final bool rotate = _rotate;
    try {
      await ref
          .read(securityActionsProvider)
          .changePassword(
            oldPassword: _old.takeSecret(),
            newPassword: _new.takePassword(),
            rotateKeys: rotate,
          );
      _old.clearSecret();
      _new.clear();
      if (!rotate) {
        messenger.showSnackBar(SnackBar(content: Text(l10n.changePwDone)));
        if (router.canPop()) router.pop();
      }
    } on AppError catch (e) {
      handleLockedWidget(ref, e);
      if (!mounted) return;
      setState(() {
        if (e.code == AppErrorCode.wrongCredentials) {
          _oldError = errorMessage(l10n, e);
        } else {
          _error = errorMessage(l10n, e);
        }
      });
    } finally {
      if (mounted) setState(() => _busy = false);
    }
  }

  @override
  Widget build(BuildContext context) {
    final l10n = context.l10n;
    final AppTokens tokens = context.tokens;
    return ToolPage(
      title: l10n.changePwTitle,
      children: <Widget>[
        SecretTextField(
          controller: _old,
          label: l10n.changePwCurrent,
          autofocus: true,
          errorText: _oldError,
          textInputAction: TextInputAction.next,
          onChanged: () => setState(() => _oldError = null),
        ),
        SizedBox(height: tokens.gapLarge),
        NewPasswordFields(
          controller: _new,
          passwordLabel: l10n.changePwNew,
          autofocus: false,
          onSubmitted: _submit,
        ),
        SizedBox(height: tokens.gap),
        CheckboxListTile(
          contentPadding: EdgeInsets.zero,
          controlAffinity: ListTileControlAffinity.leading,
          value: _rotate,
          onChanged: _busy
              ? null
              : (bool? v) => setState(() => _rotate = v ?? false),
          title: Text(l10n.changePwRotate),
          subtitle: Text(l10n.changePwRotateHelp),
        ),
        if (_error != null) ...<Widget>[
          SizedBox(height: tokens.gap / 2),
          ErrorText(_error!),
        ],
        SizedBox(height: tokens.gapLarge),
        ListenableBuilder(
          listenable: Listenable.merge(<Listenable>[_new, _old]),
          builder: (BuildContext context, _) => FilledButton(
            onPressed: (!_busy && _old.text.isNotEmpty && _new.canSubmit)
                ? _submit
                : null,
            child: Text(l10n.changePwSubmit),
          ),
        ),
      ],
    );
  }
}
