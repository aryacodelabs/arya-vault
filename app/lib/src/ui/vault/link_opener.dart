import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../services/services.dart';
import '../../state/providers.dart';
import '../widgets/l10n_ext.dart';
import 'dialogs.dart';

/// Only these schemes are ever opened. `javascript:`, `data:`, `file:` and
/// anything else is shown as text and never launched.
bool isOpenableUri(Uri? uri) =>
    uri != null &&
    uri.hasAuthority &&
    (uri.scheme == 'https' || uri.scheme == 'http');

/// Asks before opening [raw] and then hands it to the launcher.
///
/// A confirmation is always shown for links from notes ([alwaysConfirm]); for a
/// stored URL it is shown when the scheme is not https (B02 item 3).
Future<void> openLink(
  BuildContext context,
  WidgetRef ref,
  String raw, {
  bool alwaysConfirm = false,
}) async {
  final l10n = context.l10n;
  final Uri? uri = Uri.tryParse(raw.trim());
  if (!isOpenableUri(uri)) {
    _say(context, l10n.linkNotOpenable);
    return;
  }
  final bool secure = uri!.scheme == 'https';
  if (alwaysConfirm || !secure) {
    final bool ok = await confirmDialog(
      context,
      title: secure ? l10n.openLinkTitle : l10n.openInsecureLinkTitle,
      body: secure
          ? l10n.openLinkBody(uri.toString())
          : l10n.openInsecureLinkBody(uri.toString()),
      confirmLabel: l10n.openLinkConfirm,
    );
    if (!ok || !context.mounted) {
      return;
    }
  }
  try {
    final bool opened = await ref.read(urlLauncherServiceProvider).open(uri);
    if (!opened && context.mounted) {
      _say(context, l10n.linkNotOpenable);
    }
  } on ServiceUnavailable {
    if (context.mounted) _say(context, l10n.linkNotOpenable);
  }
}

void _say(BuildContext context, String text) {
  ScaffoldMessenger.of(context)
    ..hideCurrentSnackBar()
    ..showSnackBar(SnackBar(content: Text(text)));
}
