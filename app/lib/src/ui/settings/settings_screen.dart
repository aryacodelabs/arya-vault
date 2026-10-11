import 'package:flutter/material.dart';
import 'package:go_router/go_router.dart';

import '../../state/routing.dart';
import '../widgets/l10n_ext.dart';
import '../widgets/tool_page.dart';

/// Settings home (docs/07 §2): Security, Sync & backup, Import/Export, About.
class SettingsScreen extends StatelessWidget {
  const SettingsScreen({super.key});

  @override
  Widget build(BuildContext context) {
    final l10n = context.l10n;
    Widget tile(IconData icon, String title, String? sub, String route) =>
        ListTile(
          leading: Icon(icon),
          title: Text(title),
          subtitle: sub == null ? null : Text(sub),
          trailing: const Icon(Icons.chevron_right),
          onTap: () => context.push(route),
        );
    return ToolPage(
      title: l10n.settingsTitle,
      children: <Widget>[
        tile(
          Icons.shield_outlined,
          l10n.settingsSecurity,
          l10n.settingsSecuritySub,
          AppRoutes.settingsSecurity,
        ),
        tile(
          Icons.cloud_outlined,
          l10n.settingsSync,
          l10n.settingsSyncSub,
          AppRoutes.settingsSync,
        ),
        tile(Icons.upload_file, l10n.navImport, null, AppRoutes.importFile),
        tile(Icons.download, l10n.navExport, null, AppRoutes.exportFile),
        tile(
          Icons.info_outline,
          l10n.settingsAbout,
          l10n.settingsAboutSub,
          AppRoutes.settingsAbout,
        ),
      ],
    );
  }
}
