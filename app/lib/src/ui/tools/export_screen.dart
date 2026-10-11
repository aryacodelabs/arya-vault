import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../api/app_error.dart';
import '../../api/secret_bytes.dart';
import '../../state/import_export_state.dart';
import '../../state/providers.dart';
import '../../state/vault_session.dart';
import '../../theme/app_theme.dart';
import '../widgets/error_text.dart';
import '../widgets/l10n_ext.dart';
import '../widgets/new_password_fields.dart';
import '../widgets/password_prompt.dart';
import '../widgets/tool_page.dart';

/// Export (docs/14 §4.4): an encrypted file protected by an export password
/// (recommended) or a plain CSV behind a type-to-confirm warning. Both ask for
/// the master password first (docs/14 §4.1 `verifyPassword`).
class ExportScreen extends ConsumerStatefulWidget {
  const ExportScreen({super.key});

  @override
  ConsumerState<ExportScreen> createState() => _ExportScreenState();
}

class _ExportScreenState extends ConsumerState<ExportScreen> {
  bool _busy = false;
  String? _notice;
  bool _noticeIsError = false;

  void _say(String text, {bool error = false}) {
    if (!mounted) return;
    setState(() {
      _notice = text;
      _noticeIsError = error;
    });
  }

  void _report(ExportOutcome outcome) {
    final l10n = context.l10n;
    switch (outcome) {
      case ExportOutcome.saved:
        _say(l10n.exportSaved);
      case ExportOutcome.cancelled:
        _say(l10n.exportCancelled);
      case ExportOutcome.failed:
        _say(l10n.exportFailed, error: true);
      case ExportOutcome.unavailable:
        _say(l10n.recoveryServiceUnavailable, error: true);
    }
  }

  Future<T?> _guard<T>(Future<T> Function() op) async {
    setState(() {
      _busy = true;
      _notice = null;
    });
    try {
      return await op();
    } on AppError catch (e) {
      handleLockedWidget(ref, e);
      _say(errorMessage(context.l10n, e), error: true);
      return null;
    } finally {
      if (mounted) setState(() => _busy = false);
    }
  }

  Future<void> _exportEncrypted() async {
    if (!await reauthenticate(context, ref) || !mounted) return;
    final SecretBytes? password = await showDialog<SecretBytes>(
      context: context,
      builder: (BuildContext ctx) => const _ExportPasswordDialog(),
    );
    if (password == null || !mounted) return;
    final l10n = context.l10n;
    final ExportOutcome? outcome = await _guard(
      () => ref
          .read(exportActionsProvider)
          .exportEncrypted(password, fileName: l10n.exportEncryptedFileName),
    );
    password.wipe();
    if (outcome != null) _report(outcome);
  }

  Future<void> _exportCsv() async {
    final bool understood =
        await showDialog<bool>(
          context: context,
          builder: (BuildContext ctx) => const _PlaintextWarningDialog(),
        ) ??
        false;
    if (!understood || !mounted) return;
    if (!await reauthenticate(context, ref) || !mounted) return;
    final l10n = context.l10n;
    final ExportOutcome? outcome = await _guard(
      () => ref
          .read(exportActionsProvider)
          .exportCsv(fileName: l10n.exportCsvFileName),
    );
    if (outcome != null) _report(outcome);
  }

  @override
  Widget build(BuildContext context) {
    final l10n = context.l10n;
    final AppTokens tokens = context.tokens;
    return ToolPage(
      title: l10n.exportTitle,
      children: <Widget>[
        Text(l10n.exportIntro),
        SizedBox(height: tokens.gapLarge),
        ToolSection(
          title: l10n.exportEncryptedTitle,
          explain: l10n.exportEncryptedBody,
          children: <Widget>[
            Align(
              alignment: AlignmentDirectional.centerStart,
              child: FilledButton.icon(
                onPressed: _busy ? null : _exportEncrypted,
                icon: const Icon(Icons.lock),
                label: Text(l10n.exportEncryptedButton),
              ),
            ),
          ],
        ),
        ToolSection(
          title: l10n.exportCsvTitle,
          explain: l10n.exportCsvBody,
          children: <Widget>[
            Align(
              alignment: AlignmentDirectional.centerStart,
              child: OutlinedButton.icon(
                onPressed: _busy ? null : _exportCsv,
                icon: const Icon(Icons.warning_amber),
                label: Text(l10n.exportCsvButton),
              ),
            ),
          ],
        ),
        if (_busy) ...<Widget>[
          Semantics(
            liveRegion: true,
            label: l10n.exportWorking,
            child: const LinearProgressIndicator(),
          ),
          SizedBox(height: tokens.gap / 2),
        ],
        if (_notice != null)
          _noticeIsError
              ? ErrorText(_notice!)
              : Semantics(liveRegion: true, child: Text(_notice!)),
      ],
    );
  }
}

/// Asks for the export password (with confirmation and the policy meter) and
/// the "keep this password safe" warning. Pops the password as [SecretBytes].
class _ExportPasswordDialog extends ConsumerStatefulWidget {
  const _ExportPasswordDialog();

  @override
  ConsumerState<_ExportPasswordDialog> createState() =>
      _ExportPasswordDialogState();
}

class _ExportPasswordDialogState extends ConsumerState<_ExportPasswordDialog> {
  late final NewPasswordController _fields = NewPasswordController(
    ref.read(vaultApiProvider),
  );

  @override
  void dispose() {
    _fields.dispose(); // clears both fields
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final l10n = context.l10n;
    final AppTokens tokens = context.tokens;
    return AlertDialog(
      title: Text(l10n.exportPasswordTitle),
      scrollable: true,
      content: SizedBox(
        width: 460,
        child: ListenableBuilder(
          listenable: _fields,
          builder: (BuildContext context, _) => Column(
            mainAxisSize: MainAxisSize.min,
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: <Widget>[
              Text(l10n.exportPasswordBody),
              SizedBox(height: tokens.gap),
              NewPasswordFields(
                controller: _fields,
                passwordLabel: l10n.exportPasswordLabel,
              ),
              SizedBox(height: tokens.gap),
              Container(
                padding: EdgeInsets.all(tokens.gap),
                decoration: BoxDecoration(
                  color: tokens.warningBackground,
                  borderRadius: BorderRadius.circular(8),
                ),
                child: Row(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: <Widget>[
                    ExcludeSemantics(
                      child: Icon(Icons.warning_amber, color: tokens.onWarning),
                    ),
                    SizedBox(width: tokens.gap / 2),
                    Expanded(
                      child: Text(
                        l10n.exportKeepSafe,
                        style: TextStyle(color: tokens.onWarning),
                      ),
                    ),
                  ],
                ),
              ),
            ],
          ),
        ),
      ),
      actions: <Widget>[
        TextButton(
          onPressed: () => Navigator.of(context).pop(),
          child: Text(l10n.actionCancel),
        ),
        ListenableBuilder(
          listenable: _fields,
          builder: (BuildContext context, _) => FilledButton(
            onPressed: _fields.canSubmit
                ? () => Navigator.of(context).pop(_fields.takePassword())
                : null,
            child: Text(l10n.exportPasswordSubmit),
          ),
        ),
      ],
    );
  }
}

/// "Export without encryption?" with type-to-confirm. True only if the user
/// typed the confirmation phrase and pressed the button.
class _PlaintextWarningDialog extends StatefulWidget {
  const _PlaintextWarningDialog();

  @override
  State<_PlaintextWarningDialog> createState() =>
      _PlaintextWarningDialogState();
}

class _PlaintextWarningDialogState extends State<_PlaintextWarningDialog> {
  final TextEditingController _typed = TextEditingController();

  @override
  void dispose() {
    _typed.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final l10n = context.l10n;
    final String phrase = l10n.exportCsvPhrase;
    final bool matches = _typed.text.trim().toUpperCase() == phrase;
    return AlertDialog(
      title: Text(l10n.exportCsvWarnTitle),
      scrollable: true,
      content: Column(
        mainAxisSize: MainAxisSize.min,
        crossAxisAlignment: CrossAxisAlignment.start,
        children: <Widget>[
          Text(l10n.exportCsvWarnBody),
          const SizedBox(height: 16),
          Text(l10n.exportCsvTypePrompt(phrase)),
          const SizedBox(height: 8),
          TextField(
            controller: _typed,
            autofocus: true,
            autocorrect: false,
            enableSuggestions: false,
            decoration: InputDecoration(labelText: l10n.exportCsvTypeLabel),
            onChanged: (_) => setState(() {}),
          ),
        ],
      ),
      actions: <Widget>[
        TextButton(
          autofocus: false,
          onPressed: () => Navigator.of(context).pop(false),
          child: Text(l10n.actionCancel),
        ),
        FilledButton(
          style: FilledButton.styleFrom(
            backgroundColor: Theme.of(context).colorScheme.error,
            foregroundColor: Theme.of(context).colorScheme.onError,
          ),
          onPressed: matches ? () => Navigator.of(context).pop(true) : null,
          child: Text(l10n.exportCsvConfirm),
        ),
      ],
    );
  }
}
