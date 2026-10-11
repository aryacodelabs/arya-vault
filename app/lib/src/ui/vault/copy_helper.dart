import 'dart:convert';
import 'dart:typed_data';

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../api/app_error.dart';
import '../../services/services.dart';
import '../../state/providers.dart';
import '../../state/vault_session.dart';
import '../widgets/error_text.dart';
import '../widgets/l10n_ext.dart';

/// Copies a value to the clipboard and shows "Copied; clears in N s".
///
/// For a secret, pass [loadSecret] (the core's `reveal*`); the bytes are wiped
/// as soon as they are decoded. The decoded `String` is what the clipboard API
/// takes and cannot be wiped (see SecretBytes). The toast never contains the
/// value. The timed clear itself is B03/L01; this only calls the interface.
Future<void> copyToClipboard(
  BuildContext context,
  WidgetRef ref, {
  String? text,
  Future<Uint8List?> Function()? loadSecret,
}) async {
  final l10n = context.l10n;
  final ScaffoldMessengerState messenger = ScaffoldMessenger.of(context);
  void say(String message) => messenger
    ..hideCurrentSnackBar()
    ..showSnackBar(SnackBar(content: Text(message)));
  String? value = text;
  if (loadSecret != null) {
    try {
      final Uint8List? bytes = await loadSecret();
      if (bytes == null) {
        return;
      }
      value = utf8.decode(bytes, allowMalformed: true);
      bytes.fillRange(0, bytes.length, 0);
    } on AppError catch (e) {
      handleLockedWidget(ref, e);
      say(errorMessage(l10n, e));
      return;
    }
  }
  if (value == null) {
    return;
  }
  try {
    await ref.read(clipboardServiceProvider).copy(value);
    final int seconds =
        ref.read(appSettingsProvider).value?.clipboardClearSeconds ?? 30;
    say(l10n.copiedToast(seconds));
  } on ServiceUnavailable {
    say(l10n.recoveryServiceUnavailable);
  }
}
