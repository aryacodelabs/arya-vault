import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../api/app_error.dart';
import '../../api/dtos.dart';
import '../../api/secret_bytes.dart';
import '../../state/import_export_state.dart';
import '../../state/item_list.dart';
import '../../state/providers.dart';
import '../../state/vault_session.dart';
import '../../theme/app_theme.dart';
import '../widgets/error_text.dart';
import '../widgets/l10n_ext.dart';
import '../widgets/password_prompt.dart';
import '../widgets/tool_page.dart';

/// Import (docs/14 §4.4): choose a file, **preview before anything is
/// committed** (counts, duplicates, warnings, skipped records with reasons),
/// commit with progress, then a summary.
class ImportScreen extends ConsumerStatefulWidget {
  const ImportScreen({super.key});

  @override
  ConsumerState<ImportScreen> createState() => _ImportScreenState();
}

/// Lists longer than this are cut with "and N more".
const int _kShownRecords = 50;

class _ImportScreenState extends ConsumerState<ImportScreen> {
  late final ImportController _c = ImportController(
    ref.read(vaultApiProvider),
    ref.read(filePickerServiceProvider),
  );

  @override
  void initState() {
    super.initState();
    _c.addListener(_changed);
  }

  void _changed() {
    final AppError? e = _c.error;
    if (e != null) handleLockedWidget(ref, e);
    if (mounted) setState(() {});
  }

  @override
  void dispose() {
    _c.removeListener(_changed);
    _c.dispose(); // wipes a held encrypted file
    super.dispose();
  }

  Future<void> _pick() async {
    final bool picked = await _c.pickFile();
    if (!picked || !mounted || _c.kind != ImportKind.encrypted) return;
    final l10n = context.l10n;
    final bool done = await showPasswordPrompt(
      context,
      title: l10n.importPasswordTitle,
      body: l10n.importPasswordBody,
      confirmLabel: l10n.importPasswordConfirm,
      fieldLabel: l10n.importPasswordLabel,
      onSubmit: (SecretBytes pw) async {
        final AppError? e = await _c.importEncrypted(pw);
        if (e == null) return null;
        handleLockedWidget(ref, e);
        return errorMessage(l10n, e, context: ErrorContext.exportFile);
      },
    );
    if (!done) _c.discardEncryptedFile();
  }

  @override
  Widget build(BuildContext context) {
    final l10n = context.l10n;
    final AppTokens tokens = context.tokens;
    final ImportController c = _c;
    return ToolPage(
      title: l10n.importTitle,
      children: <Widget>[
        Text(l10n.importIntro),
        SizedBox(height: tokens.gapLarge),
        switch (c.phase) {
          ImportPhase.idle => _chooser(context),
          ImportPhase.reading => _progress(l10n.importReading),
          ImportPhase.preview => _previewView(context),
          ImportPhase.committing => _progress(l10n.importCommitting),
          ImportPhase.done => _resultView(context),
        },
      ],
    );
  }

  Widget _progress(String label) => Semantics(
    liveRegion: true,
    label: label,
    child: Column(
      children: <Widget>[
        const LinearProgressIndicator(),
        const SizedBox(height: 8),
        Text(label),
      ],
    ),
  );

  String _errorText(AppError e) => errorMessage(
    context.l10n,
    e,
    context: _c.kind == ImportKind.encrypted
        ? ErrorContext.exportFile
        : ErrorContext.general,
  );

  Widget _chooser(BuildContext context) {
    final l10n = context.l10n;
    final AppTokens tokens = context.tokens;
    final String label = l10n.importFormatLabel;
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: <Widget>[
        Semantics(
          header: true,
          child: Text(label, style: Theme.of(context).textTheme.titleSmall),
        ),
        RadioGroup<ImportKind>(
          groupValue: _c.kind,
          onChanged: (ImportKind? k) {
            if (k != null) _c.setKind(k);
          },
          child: Column(
            children: <Widget>[
              RadioListTile<ImportKind>(
                value: ImportKind.csv,
                title: Text(l10n.importFormatCsv),
              ),
              RadioListTile<ImportKind>(
                value: ImportKind.bitwarden,
                title: Text(l10n.importFormatBitwarden),
              ),
              RadioListTile<ImportKind>(
                value: ImportKind.encrypted,
                title: Text(l10n.importFormatEncrypted),
              ),
            ],
          ),
        ),
        SizedBox(height: tokens.gap),
        if (_c.error != null) ...<Widget>[
          ErrorText(_errorText(_c.error!)),
          SizedBox(height: tokens.gap / 2),
        ],
        if (_c.pickerUnavailable) ...<Widget>[
          ErrorText(l10n.recoveryServiceUnavailable),
          SizedBox(height: tokens.gap / 2),
        ] else if (_c.pickCancelled) ...<Widget>[
          Text(l10n.importCancelledPick),
          SizedBox(height: tokens.gap / 2),
        ],
        Align(
          alignment: AlignmentDirectional.centerStart,
          child: FilledButton.icon(
            onPressed: _pick,
            icon: const Icon(Icons.upload_file),
            label: Text(l10n.importChoose),
          ),
        ),
      ],
    );
  }

  Widget _previewView(BuildContext context) {
    final l10n = context.l10n;
    final AppTokens tokens = context.tokens;
    final ThemeData theme = Theme.of(context);
    final ImportPreview p = _c.preview!;
    final List<Folder> folders =
        ref.watch(foldersProvider).value ?? const <Folder>[];
    Widget records(String title, List<(int, String)> rows) => Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: <Widget>[
        SizedBox(height: tokens.gap),
        Semantics(
          header: true,
          child: Text(
            '$title (${rows.length})',
            style: theme.textTheme.titleSmall,
          ),
        ),
        for (final (int, String) r in rows.take(_kShownRecords))
          Text(l10n.importRecordLine(r.$1, r.$2)),
        if (rows.length > _kShownRecords)
          Text(l10n.importMore(rows.length - _kShownRecords)),
      ],
    );
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: <Widget>[
        Semantics(
          header: true,
          child: Text(
            l10n.importPreviewTitle,
            style: theme.textTheme.titleMedium,
          ),
        ),
        SizedBox(height: tokens.gap / 2),
        Text(l10n.importPreviewCounts(p.itemCount, p.folderCount)),
        if (p.duplicates > 0) Text(l10n.importPreviewDuplicates(p.duplicates)),
        if (p.itemCount == 0) Text(l10n.importPreviewNone),
        if (p.warnings.isNotEmpty)
          records(l10n.importWarningsTitle, <(int, String)>[
            for (final ImportWarning w in p.warnings) (w.record, w.message),
          ]),
        if (p.skipped.isNotEmpty)
          records(l10n.importSkippedTitle, <(int, String)>[
            for (final SkippedRecord s in p.skipped) (s.record, s.reason),
          ]),
        SizedBox(height: tokens.gap),
        SwitchListTile(
          contentPadding: EdgeInsets.zero,
          title: Text(l10n.importSkipDuplicates),
          value: _c.skipDuplicates,
          onChanged: _c.setSkipDuplicates,
        ),
        DropdownButtonFormField<String?>(
          initialValue: _c.targetFolder,
          isExpanded: true,
          decoration: InputDecoration(labelText: l10n.importTargetFolder),
          items: <DropdownMenuItem<String?>>[
            DropdownMenuItem<String?>(child: Text(l10n.noFolder)),
            for (final Folder f in folders)
              DropdownMenuItem<String?>(value: f.id, child: Text(f.name)),
          ],
          onChanged: _c.setTargetFolder,
        ),
        if (_c.error != null) ...<Widget>[
          SizedBox(height: tokens.gap),
          ErrorText(_errorText(_c.error!)),
        ],
        SizedBox(height: tokens.gapLarge),
        Wrap(
          spacing: tokens.gap,
          runSpacing: tokens.gap / 2,
          children: <Widget>[
            FilledButton(
              onPressed: p.itemCount == 0 ? null : _c.commit,
              child: Text(l10n.importCommit(p.itemCount)),
            ),
            TextButton(onPressed: _c.reset, child: Text(l10n.actionCancel)),
          ],
        ),
      ],
    );
  }

  Widget _resultView(BuildContext context) {
    final l10n = context.l10n;
    final AppTokens tokens = context.tokens;
    final ImportResult r = _c.result!;
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: <Widget>[
        Semantics(
          header: true,
          liveRegion: true,
          child: Text(
            l10n.importResultTitle,
            style: Theme.of(context).textTheme.titleMedium,
          ),
        ),
        SizedBox(height: tokens.gap / 2),
        Text(l10n.importResultCreated(r.created)),
        if (r.foldersCreated > 0)
          Text(l10n.importResultFolders(r.foldersCreated)),
        if (r.duplicates > 0) Text(l10n.importResultDuplicates(r.duplicates)),
        if (r.invalid > 0) Text(l10n.importResultInvalid(r.invalid)),
        if (r.skipped > 0) Text(l10n.importResultSkipped(r.skipped)),
        SizedBox(height: tokens.gap),
        if (_c.kind != ImportKind.encrypted)
          Text(
            l10n.importDeleteAdvice,
            style: Theme.of(context).textTheme.bodySmall,
          ),
        SizedBox(height: tokens.gapLarge),
        Align(
          alignment: AlignmentDirectional.centerStart,
          child: OutlinedButton(
            onPressed: _c.reset,
            child: Text(l10n.importAnother),
          ),
        ),
      ],
    );
  }
}
