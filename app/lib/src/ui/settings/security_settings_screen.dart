import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:go_router/go_router.dart';

import '../../api/app_error.dart';
import '../../api/dtos.dart';
import '../../api/secret_bytes.dart';
import '../../l10n/gen/app_localizations.dart';
import '../../state/providers.dart';
import '../../state/routing.dart';
import '../../state/settings_state.dart';
import '../../state/vault_session.dart';
import '../../state/vault_status.dart';
import '../../theme/app_theme.dart';
import '../screens/quick_unlock_offer_screen.dart';
import '../vault/dialogs.dart';
import '../widgets/error_text.dart';
import '../widgets/l10n_ext.dart';
import '../widgets/password_prompt.dart';
import '../widgets/tool_page.dart';

/// Values the UI offers. The core clamps anyway (docs/14 §4.5); a value the
/// core stored that is not in the list is added so the dropdown shows the truth.
const List<int> kAutoLockMinuteChoices = <int>[1, 5, 15, 30, 60];
const List<int> kClipboardSecondChoices = <int>[
  5,
  10,
  15,
  20,
  30,
  45,
  60,
  90,
  120,
];
const List<int> kRevealSecondChoices = <int>[5, 10, 15];

List<int> _choices(List<int> base, int current) =>
    base.contains(current) ? base : (<int>[...base, current]..sort());

/// Settings -> Security (docs/07 §2, §5, §7).
class SecuritySettingsScreen extends ConsumerWidget {
  const SecuritySettingsScreen({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final l10n = context.l10n;
    final AsyncValue<AppSettings> async = ref.watch(settingsEditorProvider);
    return ToolPage(
      title: l10n.settingsSecurity,
      children: <Widget>[
        if (async.hasError && !async.hasValue)
          Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: <Widget>[
              ErrorText(l10n.settingsLoadFailed),
              TextButton(
                onPressed: () => ref.invalidate(settingsEditorProvider),
                child: Text(l10n.actionTryAgain),
              ),
            ],
          )
        else if (!async.hasValue)
          const Center(child: CircularProgressIndicator())
        else
          _Body(settings: async.requireValue),
      ],
    );
  }
}

class _Body extends ConsumerWidget {
  const _Body({required this.settings});
  final AppSettings settings;

  Future<void> _save(
    BuildContext context,
    WidgetRef ref,
    AppSettings next,
  ) async {
    final l10n = context.l10n;
    final ScaffoldMessengerState messenger = ScaffoldMessenger.of(context);
    try {
      await ref.read(settingsEditorProvider.notifier).save(next);
    } on AppError catch (e) {
      handleLockedWidget(ref, e);
      messenger
        ..hideCurrentSnackBar()
        ..showSnackBar(
          SnackBar(
            content: Text(
              '${l10n.settingsSaveFailed} ${errorMessage(l10n, e)}',
            ),
          ),
        );
    }
  }

  Widget _dropdown(
    BuildContext context,
    WidgetRef ref, {
    required String label,
    required int value,
    required List<int> options,
    required String Function(int) text,
    required AppSettings Function(int) apply,
  }) {
    return DropdownButtonFormField<int>(
      key: ValueKey<String>('$label-$value'),
      initialValue: value,
      isExpanded: true,
      decoration: InputDecoration(labelText: label),
      items: <DropdownMenuItem<int>>[
        for (final int o in _choices(options, value))
          DropdownMenuItem<int>(value: o, child: Text(text(o))),
      ],
      onChanged: (int? v) {
        if (v != null && v != value) _save(context, ref, apply(v));
      },
    );
  }

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final l10n = context.l10n;
    final AppTokens tokens = context.tokens;
    final AppSettings s = settings;
    final bool captureAvailable = ref
        .watch(captureProtectionProvider)
        .available;
    final VaultStatus? status = ref.watch(vaultStatusProvider).value;
    final QuickUnlockStatus quick =
        status?.quickUnlock ?? QuickUnlockStatus.unsupported;
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: <Widget>[
        ToolSection(
          title: l10n.secAutoLockTitle,
          explain: l10n.secAutoLockNote,
          children: <Widget>[
            _dropdown(
              context,
              ref,
              label: l10n.secAutoLockAfter,
              value: s.autoLockMinutes,
              options: kAutoLockMinuteChoices,
              text: l10n.secAutoLockMinutes,
              apply: (int v) => s.copyWith(autoLockMinutes: v),
            ),
            SwitchListTile(
              contentPadding: EdgeInsets.zero,
              title: Text(l10n.secLockOnScreenLock),
              value: s.lockOnScreenLock,
              onChanged: (bool v) =>
                  _save(context, ref, s.copyWith(lockOnScreenLock: v)),
            ),
            SwitchListTile(
              contentPadding: EdgeInsets.zero,
              title: Text(l10n.secLockOnSleep),
              value: s.lockOnSleep,
              onChanged: (bool v) =>
                  _save(context, ref, s.copyWith(lockOnSleep: v)),
            ),
          ],
        ),
        ToolSection(
          title: l10n.secClipboardTitle,
          children: <Widget>[
            _dropdown(
              context,
              ref,
              label: l10n.secClipboardClear,
              value: s.clipboardClearSeconds,
              options: kClipboardSecondChoices,
              text: l10n.secSeconds,
              apply: (int v) => s.copyWith(clipboardClearSeconds: v),
            ),
            SizedBox(height: tokens.gap),
            _dropdown(
              context,
              ref,
              label: l10n.secRevealHide,
              value: s.revealHideSeconds,
              options: kRevealSecondChoices,
              text: l10n.secSeconds,
              apply: (int v) => s.copyWith(revealHideSeconds: v),
            ),
          ],
        ),
        ToolSection(
          title: l10n.secCaptureTitle,
          explain: captureAvailable
              ? l10n.secCaptureNote
              : l10n.notAvailableBuild,
          children: <Widget>[
            SwitchListTile(
              contentPadding: EdgeInsets.zero,
              title: Text(l10n.secCaptureBlock),
              subtitle: captureAvailable ? null : Text(l10n.notAvailableBuild),
              value: s.blockScreenCapture,
              // Without a platform implementation the switch would do nothing;
              // it is shown disabled instead of pretending.
              onChanged: captureAvailable
                  ? (bool v) =>
                        _save(context, ref, s.copyWith(blockScreenCapture: v))
                  : null,
            ),
          ],
        ),
        ToolSection(
          title: l10n.secQuickTitle,
          children: <Widget>[_QuickUnlockTile(quick: quick)],
        ),
        ToolSection(
          title: l10n.secMasterTitle,
          children: <Widget>[
            ListTile(
              contentPadding: EdgeInsets.zero,
              leading: const Icon(Icons.password),
              title: Text(l10n.secChangePassword),
              trailing: const Icon(Icons.chevron_right),
              onTap: () => context.push(AppRoutes.settingsChangePassword),
            ),
          ],
        ),
        ToolSection(
          title: l10n.secRecoveryTitle,
          children: <Widget>[
            ListTile(
              contentPadding: EdgeInsets.zero,
              leading: const Icon(Icons.key),
              title: Text(l10n.secViewRecovery),
              onTap: () => _viewRecovery(context, ref),
            ),
            ListTile(
              contentPadding: EdgeInsets.zero,
              leading: const Icon(Icons.autorenew),
              title: Text(l10n.secRegenerateRecovery),
              onTap: () => _regenerate(context, ref),
            ),
          ],
        ),
      ],
    );
  }

  /// The core never stores the recovery key, so there is nothing to view; say
  /// so and offer the only honest alternative (docs/07 §5).
  Future<void> _viewRecovery(BuildContext context, WidgetRef ref) async {
    final l10n = context.l10n;
    final bool regenerate = await confirmDialog(
      context,
      title: l10n.secViewRecoveryTitle,
      body: l10n.secViewRecoveryBody,
      confirmLabel: l10n.secRegenerateConfirm,
    );
    if (regenerate && context.mounted) await _askAndRegenerate(context, ref);
  }

  Future<void> _regenerate(BuildContext context, WidgetRef ref) async {
    final l10n = context.l10n;
    final bool ok = await confirmDialog(
      context,
      title: l10n.secRegenerateTitle,
      body: l10n.secRegenerateBody,
      confirmLabel: l10n.secRegenerateConfirm,
      destructive: true,
    );
    if (ok && context.mounted) await _askAndRegenerate(context, ref);
  }

  Future<void> _askAndRegenerate(BuildContext context, WidgetRef ref) async {
    final l10n = context.l10n;
    await showPasswordPrompt(
      context,
      title: l10n.reauthTitle,
      body: l10n.reauthBody,
      confirmLabel: l10n.secRegenerateConfirm,
      onSubmit: (SecretBytes pw) async {
        try {
          await ref.read(securityActionsProvider).regenerateRecoveryKey(pw);
          return null; // the router now shows the new key
        } on AppError catch (e) {
          handleLockedWidget(ref, e);
          return errorMessage(l10n, e);
        }
      },
    );
  }
}

class _QuickUnlockTile extends ConsumerWidget {
  const _QuickUnlockTile({required this.quick});
  final QuickUnlockStatus quick;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final l10n = context.l10n;
    if (!quick.supported) {
      return ListTile(
        contentPadding: EdgeInsets.zero,
        leading: const Icon(Icons.fingerprint),
        title: Text(l10n.secQuickOnlyPassword),
        subtitle: Text(l10n.notAvailableDevice),
      );
    }
    final String method = quickUnlockMethodName(l10n, quick.kind);
    return SwitchListTile(
      contentPadding: EdgeInsets.zero,
      title: Text(l10n.secQuickToggle(method)),
      value: quick.enabled,
      onChanged: (bool on) => on
          ? _enable(context, ref, l10n)
          : _run(context, ref, l10n, () {
              return ref.read(securityActionsProvider).disableQuickUnlock();
            }),
    );
  }

  Future<void> _run(
    BuildContext context,
    WidgetRef ref,
    AppLocalizations l10n,
    Future<void> Function() op,
  ) async {
    final ScaffoldMessengerState m = ScaffoldMessenger.of(context);
    try {
      await op();
    } on AppError catch (e) {
      handleLockedWidget(ref, e);
      m.showSnackBar(SnackBar(content: Text(errorMessage(l10n, e))));
    }
  }

  /// Turning it on needs the master password first (re-authentication).
  Future<void> _enable(
    BuildContext context,
    WidgetRef ref,
    AppLocalizations l10n,
  ) async {
    final ScaffoldMessengerState m = ScaffoldMessenger.of(context);
    await showPasswordPrompt(
      context,
      title: l10n.reauthTitle,
      body: l10n.secQuickEnableBody,
      confirmLabel: l10n.reauthConfirm,
      onSubmit: (SecretBytes pw) async {
        try {
          final SecurityActions actions = ref.read(securityActionsProvider);
          if (!await actions.verifyPassword(pw)) return l10n.reauthWrong;
          await actions.enableQuickUnlock();
          return null;
        } on AppError catch (e) {
          handleLockedWidget(ref, e);
          if (e.code == AppErrorCode.quickUnlockUnavailable) {
            m.showSnackBar(SnackBar(content: Text(errorMessage(l10n, e))));
            return null;
          }
          return errorMessage(l10n, e);
        }
      },
    );
  }
}
