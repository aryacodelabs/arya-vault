import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:go_router/go_router.dart';

import '../../api/dtos.dart';
import '../../app_info.dart';
import '../../state/health_state.dart';
import '../../state/routing.dart';
import '../../theme/app_theme.dart';
import '../widgets/error_text.dart';
import '../widgets/l10n_ext.dart';
import '../widgets/tool_page.dart';

/// Settings -> About: versions from `info()`, licenses, repository, and the
/// diagnostics entry (docs/07 §2, docs/14 §4.6).
class AboutScreen extends ConsumerWidget {
  const AboutScreen({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final l10n = context.l10n;
    final AppTokens tokens = context.tokens;
    final AsyncValue<InfoDto> info = ref.watch(infoProvider);
    Widget row(String label, String value) => ListTile(
      contentPadding: EdgeInsets.zero,
      dense: true,
      title: Text(label),
      trailing: Text(value),
    );
    return ToolPage(
      title: l10n.aboutTitle,
      children: <Widget>[
        row(l10n.aboutApp, kAppVersion),
        if (info.hasValue) ...<Widget>[
          row(l10n.aboutCore, info.requireValue.coreVersion),
          row(l10n.aboutApi, '${info.requireValue.apiVersion}'),
          row(l10n.aboutFormat, '${info.requireValue.formatVersion}'),
          row(l10n.aboutSchema, '${info.requireValue.schemaVersion}'),
        ] else if (info.hasError)
          Padding(
            padding: EdgeInsets.symmetric(vertical: tokens.gap / 2),
            child: ErrorText(l10n.aboutInfoFailed),
          ),
        const Divider(),
        ListTile(
          contentPadding: EdgeInsets.zero,
          leading: const Icon(Icons.description_outlined),
          title: Text(l10n.aboutLicenses),
          trailing: const Icon(Icons.chevron_right),
          onTap: () => showLicensePage(
            context: context,
            applicationName: l10n.appName,
            applicationVersion: kAppVersion,
          ),
        ),
        Padding(
          padding: EdgeInsets.only(bottom: tokens.gap),
          child: Text(
            l10n.aboutLicensesRust,
            style: Theme.of(context).textTheme.bodySmall,
          ),
        ),
        ListTile(
          contentPadding: EdgeInsets.zero,
          leading: const Icon(Icons.code),
          title: Text(l10n.aboutRepo),
          // Plain text on purpose: nothing here opens the network.
          subtitle: Text(l10n.aboutRepoUrl),
        ),
        ListTile(
          contentPadding: EdgeInsets.zero,
          leading: const Icon(Icons.bug_report_outlined),
          title: Text(l10n.aboutDiagnostics),
          subtitle: Text(l10n.aboutDiagnosticsSub),
          trailing: const Icon(Icons.chevron_right),
          onTap: () => context.push(AppRoutes.settingsDiagnostics),
        ),
      ],
    );
  }
}
