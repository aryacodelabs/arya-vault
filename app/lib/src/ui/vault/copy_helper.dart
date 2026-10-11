import 'dart:typed_data';

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../api/app_error.dart';
import '../../api/secret_bytes.dart';
import '../../services/clipboard_manager.dart';
import '../../services/services.dart';
import '../../state/security_services.dart';
import '../../state/vault_session.dart';
import '../widgets/error_text.dart';
import '../widgets/l10n_ext.dart';

/// Copies a value to the clipboard and shows "Copied; clears in N s".
///
/// For a secret, pass [loadSecret] (the core's `reveal*`); the bytes go straight
/// to the `ClipboardManager`, which writes them with the sensitive flag, wipes
/// them, and clears the clipboard after N seconds if it still holds them. The
/// toast never contains the value.
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
  SecretBytes? value;
  if (loadSecret != null) {
    try {
      final Uint8List? bytes = await loadSecret();
      if (bytes == null) {
        return;
      }
      value = SecretBytes(bytes);
    } on AppError catch (e) {
      handleLockedWidget(ref, e);
      say(errorMessage(l10n, e));
      return;
    }
  } else if (text != null) {
    value = SecretBytes.fromString(text);
  }
  if (value == null) {
    return;
  }
  final ClipboardManager manager = ref.read(clipboardManagerProvider);
  try {
    await manager.copySecret(value);
    say(l10n.copiedToast(manager.clearAfter.inSeconds));
  } on ServiceUnavailable {
    value.wipe();
    say(l10n.recoveryServiceUnavailable);
  }
}
