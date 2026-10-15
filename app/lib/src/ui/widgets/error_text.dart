import 'package:flutter/material.dart';

import '../../api/app_error.dart';
import '../../l10n/gen/app_localizations.dart';
import '../../theme/app_theme.dart';

/// Which flow an error came from; `wrongCredentials` reads differently when the
/// user typed a recovery key or an export password.
enum ErrorContext { general, recoveryKey, exportFile }

/// Localized text for an [AppError]. Never shows [AppError.message] (it is
/// English and meant for logs); every code the onboarding and lock flows can
/// receive has its own string.
String errorMessage(
  AppLocalizations l10n,
  AppError error, {
  ErrorContext context = ErrorContext.general,
}) => switch (error.code) {
  AppErrorCode.wrongCredentials => switch (context) {
    ErrorContext.recoveryKey => l10n.errWrongRecoveryKey,
    ErrorContext.exportFile => l10n.errWrongExportPassword,
    ErrorContext.general => l10n.errWrongCredentials,
  },
  AppErrorCode.recoveryKeyMalformed => l10n.errRecoveryKeyMalformed,
  AppErrorCode.weakPassword => l10n.errWeakPassword,
  AppErrorCode.locked => l10n.errLocked,
  AppErrorCode.notFound => l10n.errNotFound,
  AppErrorCode.validation => l10n.errValidation,
  AppErrorCode.limitReached => l10n.errLimitReached,
  AppErrorCode.corruptVault => l10n.errCorruptVault,
  AppErrorCode.unsupportedFormat => l10n.errUnsupportedFormat,
  AppErrorCode.quickUnlockUnavailable => l10n.errQuickUnlockUnavailable,
  // Callers skip this one on the lock screen; the text is for any place that shows it anyway.
  AppErrorCode.quickUnlockCancelled => l10n.errQuickUnlockCancelled,
  AppErrorCode.quickUnlockInvalidated => l10n.errQuickUnlockInvalidated,
  AppErrorCode.alreadyExists => l10n.errAlreadyExists,
  AppErrorCode.io => l10n.errIo,
  AppErrorCode.busy => l10n.errBusy,
  AppErrorCode.internal => l10n.errInternal,
};

/// An error line with an icon (never color alone), announced by screen readers.
class ErrorText extends StatelessWidget {
  const ErrorText(this.message, {super.key});
  final String message;

  @override
  Widget build(BuildContext context) {
    final ColorScheme scheme = Theme.of(context).colorScheme;
    return Semantics(
      liveRegion: true,
      container: true,
      child: Row(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: <Widget>[
          ExcludeSemantics(
            child: Icon(Icons.error_outline, color: scheme.error, size: 20),
          ),
          SizedBox(width: context.tokens.gap / 2),
          Expanded(
            child: Text(message, style: TextStyle(color: scheme.error)),
          ),
        ],
      ),
    );
  }
}
