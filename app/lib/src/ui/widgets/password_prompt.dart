import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../api/app_error.dart';
import '../../api/secret_bytes.dart';
import '../../state/settings_state.dart';
import '../../state/vault_session.dart';
import 'error_text.dart';
import 'l10n_ext.dart';
import 'secret_text_field.dart';

/// Runs the action behind a password prompt. Returns null on success (the
/// dialog closes) or the message to show under the field (the dialog stays).
/// The prompt wipes [password] afterwards; the callback must not keep it.
typedef PromptSubmit = Future<String?> Function(SecretBytes password);

/// Asks for the master password and runs [onSubmit] with it. True if the
/// action succeeded, false if the user cancelled.
Future<bool> showPasswordPrompt(
  BuildContext context, {
  required String title,
  required String body,
  required String confirmLabel,
  required PromptSubmit onSubmit,
  String? fieldLabel,
}) async {
  final bool? ok = await showDialog<bool>(
    context: context,
    builder: (BuildContext ctx) => _PasswordPrompt(
      title: title,
      body: body,
      confirmLabel: confirmLabel,
      fieldLabel: fieldLabel,
      onSubmit: onSubmit,
    ),
  );
  return ok ?? false;
}

/// Re-authentication before a sensitive action (docs/14 §4.1 `verifyPassword`).
Future<bool> reauthenticate(
  BuildContext context,
  WidgetRef ref, {
  String? title,
  String? body,
  String? confirmLabel,
}) {
  final l10n = context.l10n;
  return showPasswordPrompt(
    context,
    title: title ?? l10n.reauthTitle,
    body: body ?? l10n.reauthBody,
    confirmLabel: confirmLabel ?? l10n.reauthConfirm,
    onSubmit: (SecretBytes password) async {
      try {
        final bool ok = await ref
            .read(securityActionsProvider)
            .verifyPassword(password);
        return ok ? null : l10n.reauthWrong;
      } on AppError catch (e) {
        handleLockedWidget(ref, e);
        return errorMessage(l10n, e);
      }
    },
  );
}

class _PasswordPrompt extends StatefulWidget {
  const _PasswordPrompt({
    required this.title,
    required this.body,
    required this.confirmLabel,
    required this.onSubmit,
    this.fieldLabel,
  });

  final String title;
  final String body;
  final String confirmLabel;
  final String? fieldLabel;
  final PromptSubmit onSubmit;

  @override
  State<_PasswordPrompt> createState() => _PasswordPromptState();
}

class _PasswordPromptState extends State<_PasswordPrompt> {
  final SecretTextController _password = SecretTextController();
  bool _busy = false;
  String? _error;

  @override
  void dispose() {
    _password.clearSecret();
    _password.dispose();
    super.dispose();
  }

  Future<void> _submit() async {
    if (_busy || _password.text.isEmpty) return;
    setState(() {
      _busy = true;
      _error = null;
    });
    final SecretBytes secret = _password.takeSecret();
    String? error;
    try {
      error = await widget.onSubmit(secret);
    } finally {
      secret.wipe();
    }
    if (!mounted) return;
    if (error == null) {
      _password.clearSecret();
      Navigator.of(context).pop(true);
    } else {
      setState(() {
        _busy = false;
        _error = error;
      });
    }
  }

  @override
  Widget build(BuildContext context) {
    final l10n = context.l10n;
    return AlertDialog(
      title: Text(widget.title),
      content: SingleChildScrollView(
        child: Column(
          mainAxisSize: MainAxisSize.min,
          crossAxisAlignment: CrossAxisAlignment.start,
          children: <Widget>[
            Text(widget.body),
            const SizedBox(height: 16),
            SecretTextField(
              controller: _password,
              label: widget.fieldLabel ?? l10n.masterPasswordLabel,
              autofocus: true,
              onSubmitted: _submit,
            ),
            if (_error != null) ...<Widget>[
              const SizedBox(height: 8),
              ErrorText(_error!),
            ],
          ],
        ),
      ),
      actions: <Widget>[
        TextButton(
          onPressed: _busy ? null : () => Navigator.of(context).pop(false),
          child: Text(l10n.actionCancel),
        ),
        FilledButton(
          onPressed: _busy ? null : _submit,
          child: Text(widget.confirmLabel),
        ),
      ],
    );
  }
}
