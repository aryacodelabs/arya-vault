import 'dart:convert';
import 'dart:typed_data';

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../api/app_error.dart';
import '../../services/services.dart';
import '../../state/providers.dart';
import '../../state/vault_session.dart';
import '../../theme/app_theme.dart';
import '../widgets/error_text.dart';
import '../widgets/l10n_ext.dart';
import '../widgets/tool_page.dart';

/// Settings -> About -> Diagnostics: shows the scrubbed JSON report **before**
/// anything is saved, so the user can read what would be shared (docs/14 §4.6).
class DiagnosticsScreen extends ConsumerStatefulWidget {
  const DiagnosticsScreen({super.key});

  @override
  ConsumerState<DiagnosticsScreen> createState() => _DiagnosticsScreenState();
}

class _DiagnosticsScreenState extends ConsumerState<DiagnosticsScreen> {
  Uint8List? _report;
  String? _error;
  String? _notice;
  bool _busy = false;

  @override
  void dispose() {
    final Uint8List? r = _report;
    if (r != null) r.fillRange(0, r.length, 0);
    super.dispose();
  }

  Future<void> _prepare() async {
    final l10n = context.l10n;
    setState(() {
      _busy = true;
      _error = null;
      _notice = null;
    });
    try {
      final Uint8List bytes = await ref
          .read(vaultApiProvider)
          .exportDiagnostics();
      if (!mounted) return;
      setState(() => _report = bytes);
    } on AppError catch (e) {
      handleLockedWidget(ref, e);
      if (mounted) setState(() => _error = errorMessage(l10n, e));
    } finally {
      if (mounted) setState(() => _busy = false);
    }
  }

  Future<void> _save() async {
    final l10n = context.l10n;
    final Uint8List? report = _report;
    if (report == null) return;
    try {
      final bool saved = await ref
          .read(filePickerServiceProvider)
          .saveFile(suggestedName: l10n.diagFileName, bytes: report);
      if (mounted && saved) setState(() => _notice = l10n.diagSaved);
    } on ServiceUnavailable {
      if (mounted) setState(() => _error = l10n.recoveryServiceUnavailable);
    } on Object {
      if (mounted) setState(() => _error = l10n.exportFailed);
    }
  }

  @override
  Widget build(BuildContext context) {
    final l10n = context.l10n;
    final AppTokens tokens = context.tokens;
    final ThemeData theme = Theme.of(context);
    final Uint8List? report = _report;
    return ToolPage(
      title: l10n.diagTitle,
      children: <Widget>[
        Text(l10n.diagIntro),
        SizedBox(height: tokens.gap),
        Align(
          alignment: AlignmentDirectional.centerStart,
          child: FilledButton(
            onPressed: _busy ? null : _prepare,
            child: Text(l10n.diagPrepare),
          ),
        ),
        if (_error != null) ...<Widget>[
          SizedBox(height: tokens.gap / 2),
          ErrorText(_error!),
        ],
        if (report != null) ...<Widget>[
          SizedBox(height: tokens.gap),
          Semantics(
            container: true,
            label: l10n.diagReportLabel,
            child: Container(
              padding: EdgeInsets.all(tokens.gap),
              decoration: BoxDecoration(
                color: tokens.keyGroupBackground,
                borderRadius: BorderRadius.circular(8),
              ),
              child: SelectableText(
                utf8.decode(report, allowMalformed: true),
                style: theme.textTheme.bodySmall!.copyWith(
                  fontFamily: 'monospace',
                  fontFamilyFallback: const <String>[
                    'Consolas',
                    'Menlo',
                    'Courier New',
                  ],
                  color: tokens.onKeyGroup,
                ),
              ),
            ),
          ),
          SizedBox(height: tokens.gap),
          Align(
            alignment: AlignmentDirectional.centerStart,
            child: OutlinedButton.icon(
              onPressed: _save,
              icon: const Icon(Icons.save_alt),
              label: Text(l10n.diagSave),
            ),
          ),
          if (_notice != null) ...<Widget>[
            SizedBox(height: tokens.gap / 2),
            Semantics(liveRegion: true, child: Text(_notice!)),
          ],
        ],
      ],
    );
  }
}
