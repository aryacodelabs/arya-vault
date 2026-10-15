import 'dart:convert';
import 'dart:typed_data';

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../api/app_error.dart';
import '../../api/secret_bytes.dart';
import '../../l10n/gen/app_localizations.dart';
import '../../services/services.dart';
import '../../state/onboarding_state.dart';
import '../../state/providers.dart';
import '../../state/vault_actions.dart';
import '../../theme/app_theme.dart';
import '../widgets/error_text.dart';
import '../widgets/l10n_ext.dart';
import '../widgets/page_scaffold.dart';
import '../widgets/secret_text_field.dart';

/// docs/07 §3 steps F-G: shows the recovery key once, in grouped monospace
/// blocks with the checksum group.
///
/// There is **no clipboard button** on this screen (docs/14 §8 question 3):
/// other apps can read the clipboard. The key is offered as print or file. The
/// text is rendered from the key bytes into a Dart `String`, which cannot be
/// wiped (see SecretBytes); the hide toggle and leaving the screen are the
/// mitigations, and the bytes themselves are wiped when onboarding ends or the
/// vault locks.
class RecoveryKeyScreen extends ConsumerStatefulWidget {
  const RecoveryKeyScreen({super.key});

  @override
  ConsumerState<RecoveryKeyScreen> createState() => _RecoveryKeyScreenState();
}

class _RecoveryKeyScreenState extends ConsumerState<RecoveryKeyScreen> {
  bool _visible = true;
  bool _stored = false;
  String? _notice;
  bool _noticeIsError = false;

  void _say(String text, {bool error = false}) {
    if (mounted) {
      setState(() {
        _notice = text;
        _noticeIsError = error;
      });
    }
  }

  /// The printable/saveable sheet as UTF-8 bytes; the caller wipes it.
  Uint8List _sheet(AppLocalizations l10n, SecretBytes key) {
    final Uint8List keyBytes = key.copyBytes();
    try {
      final BytesBuilder out = BytesBuilder(copy: false)
        ..add(utf8.encode('${l10n.recoverySheetTitle}\n\n'))
        ..add(utf8.encode('${l10n.recoverySheetBody}\n\n'))
        ..add(utf8.encode('${l10n.recoverySheetKeyLabel}\n'))
        ..add(keyBytes)
        ..add(utf8.encode('\n'));
      return out.takeBytes();
    } finally {
      keyBytes.fillRange(0, keyBytes.length, 0);
    }
  }

  Future<void> _print(SecretBytes key) async {
    final l10n = context.l10n;
    final PrintService printer = ref.read(printServiceProvider);
    final Uint8List sheet = _sheet(l10n, key);
    try {
      await printer.printText(title: l10n.recoverySheetTitle, body: sheet);
    } on ServiceUnavailable {
      _say(l10n.recoveryServiceUnavailable, error: true);
    } on Object {
      _say(l10n.recoveryPrintFailed, error: true);
    } finally {
      sheet.fillRange(0, sheet.length, 0);
    }
  }

  Future<void> _save(SecretBytes key) async {
    final l10n = context.l10n;
    final FilePickerService picker = ref.read(filePickerServiceProvider);
    final Uint8List sheet = _sheet(l10n, key);
    try {
      final bool saved = await picker.saveFile(
        suggestedName: l10n.recoveryFileName,
        bytes: sheet,
      );
      if (saved) {
        _say(l10n.recoverySaved);
      }
    } on ServiceUnavailable {
      _say(l10n.recoveryServiceUnavailable, error: true);
    } on Object {
      _say(l10n.recoverySaveFailed, error: true);
    } finally {
      sheet.fillRange(0, sheet.length, 0);
    }
  }

  @override
  Widget build(BuildContext context) {
    final OnboardingState onboarding = ref.watch(onboardingProvider);
    final SecretBytes? key = onboarding.key;
    if (key == null || key.isWiped) {
      return const _RegenerateKeyForm();
    }
    final l10n = context.l10n;
    final AppTokens tokens = context.tokens;
    // While hidden, the key text is not even decoded into Strings.
    var groups = const <String>[];
    if (_visible) {
      final Uint8List keyBytes = key.copyBytes();
      groups = utf8.decode(keyBytes).split('-');
      keyBytes.fillRange(0, keyBytes.length, 0);
    }
    return PageScaffold(
      title: l10n.recoveryTitle,
      children: <Widget>[
        Text(l10n.recoveryIntro),
        if (onboarding.verifyFailed) ...<Widget>[
          SizedBox(height: tokens.gap),
          ErrorText(l10n.verifyMismatch),
        ],
        SizedBox(height: tokens.gap),
        _KeyGroups(groups: groups, visible: _visible),
        SizedBox(height: tokens.gap / 2),
        Align(
          alignment: AlignmentDirectional.centerStart,
          child: TextButton.icon(
            onPressed: () => setState(() => _visible = !_visible),
            icon: Icon(_visible ? Icons.visibility_off : Icons.visibility),
            label: Text(_visible ? l10n.recoveryHideKey : l10n.recoveryShowKey),
          ),
        ),
        SizedBox(height: tokens.gap / 2),
        Wrap(
          spacing: tokens.gap,
          runSpacing: tokens.gap / 2,
          children: <Widget>[
            if (ref.watch(printServiceProvider).isAvailable)
              OutlinedButton.icon(
                onPressed: () => _print(key),
                icon: const Icon(Icons.print),
                label: Text(l10n.recoveryPrint),
              ),
            OutlinedButton.icon(
              onPressed: () => _save(key),
              icon: const Icon(Icons.save_alt),
              label: Text(l10n.recoverySave),
            ),
          ],
        ),
        if (_notice != null) ...<Widget>[
          SizedBox(height: tokens.gap / 2),
          _noticeIsError
              ? ErrorText(_notice!)
              : Semantics(liveRegion: true, child: Text(_notice!)),
        ],
        SizedBox(height: tokens.gap),
        Text(l10n.recoveryNoCopy, style: Theme.of(context).textTheme.bodySmall),
        SizedBox(height: tokens.gap / 2),
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
                  l10n.recoveryCaptureWarning,
                  style: TextStyle(color: tokens.onWarning),
                ),
              ),
            ],
          ),
        ),
        SizedBox(height: tokens.gap),
        CheckboxListTile(
          value: _stored,
          onChanged: (bool? v) => setState(() => _stored = v ?? false),
          title: Text(l10n.recoveryStoredCheck),
          controlAffinity: ListTileControlAffinity.leading,
          contentPadding: EdgeInsets.zero,
        ),
        SizedBox(height: tokens.gap),
        FilledButton(
          onPressed: _stored
              ? () => ref.read(onboardingProvider.notifier).keySavedContinue()
              : null,
          child: Text(l10n.recoveryContinue),
        ),
      ],
    );
  }
}

/// Placeholder for a hidden group; not a message.
const String _mask = '•••••  ';

/// The key groups. The last group is the checksum and is labelled as such.
class _KeyGroups extends StatelessWidget {
  const _KeyGroups({required this.groups, required this.visible});
  final List<String> groups;
  final bool visible;

  @override
  Widget build(BuildContext context) {
    final l10n = context.l10n;
    final AppTokens tokens = context.tokens;
    final ThemeData theme = Theme.of(context);
    final TextStyle mono = theme.textTheme.titleLarge!.copyWith(
      fontFamily: 'monospace',
      fontFamilyFallback: const <String>['Consolas', 'Menlo', 'Courier New'],
      letterSpacing: 2,
      color: tokens.onKeyGroup,
    );
    if (!visible) {
      return Semantics(
        label: l10n.recoveryHiddenPlaceholder,
        container: true,
        child: ExcludeSemantics(
          child: Container(
            padding: EdgeInsets.all(tokens.gap),
            decoration: BoxDecoration(
              color: tokens.keyGroupBackground,
              borderRadius: BorderRadius.circular(8),
            ),
            child: Text(_mask * 4, style: mono),
          ),
        ),
      );
    }
    return Wrap(
      spacing: tokens.gap / 2,
      runSpacing: tokens.gap / 2,
      children: <Widget>[
        for (var i = 0; i < groups.length; i++)
          Semantics(
            container: true,
            label: i == groups.length - 1
                ? l10n.recoveryChecksumSemantics(groups[i].split('').join(' '))
                : l10n.recoveryGroupSemantics(
                    i + 1,
                    groups.length,
                    groups[i].split('').join(' '),
                  ),
            child: ExcludeSemantics(
              child: Container(
                padding: EdgeInsets.symmetric(
                  horizontal: tokens.gap * 0.75,
                  vertical: tokens.gap / 2,
                ),
                decoration: BoxDecoration(
                  color: tokens.keyGroupBackground,
                  borderRadius: BorderRadius.circular(8),
                  border: i == groups.length - 1
                      ? Border.all(color: theme.colorScheme.outline, width: 2)
                      : null,
                ),
                child: Column(
                  mainAxisSize: MainAxisSize.min,
                  children: <Widget>[
                    Text(groups[i], style: mono),
                    if (i == groups.length - 1)
                      Text(
                        l10n.recoveryChecksumLabel,
                        style: theme.textTheme.labelSmall?.copyWith(
                          color: tokens.onKeyGroup,
                        ),
                      ),
                  ],
                ),
              ),
            ),
          ),
      ],
    );
  }
}

/// Shown when onboarding is unfinished but the pending key is gone (the app
/// restarted or locked): the core cannot show it again, so the user creates a
/// new one with their master password.
class _RegenerateKeyForm extends ConsumerStatefulWidget {
  const _RegenerateKeyForm();

  @override
  ConsumerState<_RegenerateKeyForm> createState() => _RegenerateKeyFormState();
}

class _RegenerateKeyFormState extends ConsumerState<_RegenerateKeyForm> {
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
    if (_busy || _password.text.isEmpty) {
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
          .regenerateRecoveryKey(_password.takeSecret());
      _password.clearSecret();
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
      title: l10n.recoveryRegenTitle,
      children: <Widget>[
        Text(l10n.recoveryRegenBody),
        SizedBox(height: tokens.gapLarge),
        SecretTextField(
          controller: _password,
          label: l10n.masterPasswordLabel,
          autofocus: true,
          onSubmitted: _submit,
          errorText: _error,
        ),
        SizedBox(height: tokens.gapLarge),
        FilledButton(
          onPressed: _busy ? null : _submit,
          child: Text(l10n.recoveryRegenSubmit),
        ),
      ],
    );
  }
}
