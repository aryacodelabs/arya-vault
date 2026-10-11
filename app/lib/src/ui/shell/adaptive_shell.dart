import 'package:flutter/material.dart';

import '../widgets/l10n_ext.dart';

/// Width at and above which the three-pane desktop layout is used (docs/07 §8).
const double kDesktopBreakpoint = 900;

/// Breakpoint-based shell: desktop gets navigation, list and detail panes;
/// compact widths get a single pane under an app bar.
class AdaptiveShell extends StatelessWidget {
  const AdaptiveShell({
    required this.title,
    required this.listPane,
    required this.detailPane,
    this.actions = const <Widget>[],
    super.key,
  });

  final String title;
  final Widget listPane;
  final Widget detailPane;
  final List<Widget> actions;

  @override
  Widget build(BuildContext context) {
    return LayoutBuilder(
      builder: (BuildContext context, BoxConstraints constraints) {
        final bool desktop = constraints.maxWidth >= kDesktopBreakpoint;
        final l10n = context.l10n;
        final Widget appBarTitle = Semantics(header: true, child: Text(title));
        if (!desktop) {
          return Scaffold(
            appBar: AppBar(
              title: appBarTitle,
              actions: <Widget>[
                for (final Widget a in actions)
                  Padding(
                    padding: const EdgeInsetsDirectional.only(end: 8),
                    child: Center(child: a),
                  ),
              ],
            ),
            body: SafeArea(child: detailPane),
          );
        }
        return Scaffold(
          appBar: AppBar(
            title: appBarTitle,
            actions: <Widget>[
              for (final Widget a in actions)
                Padding(
                  padding: const EdgeInsetsDirectional.only(end: 16),
                  child: Center(child: a),
                ),
            ],
          ),
          body: Row(
            children: <Widget>[
              NavigationRail(
                selectedIndex: 0,
                labelType: NavigationRailLabelType.all,
                destinations: <NavigationRailDestination>[
                  NavigationRailDestination(
                    icon: const Icon(Icons.lock_outline),
                    selectedIcon: const Icon(Icons.lock),
                    label: Text(l10n.navVault),
                  ),
                  NavigationRailDestination(
                    icon: const Icon(Icons.password),
                    label: Text(l10n.navGenerator),
                    disabled: true,
                  ),
                  NavigationRailDestination(
                    icon: const Icon(Icons.health_and_safety_outlined),
                    label: Text(l10n.navHealth),
                    disabled: true,
                  ),
                  NavigationRailDestination(
                    icon: const Icon(Icons.settings_outlined),
                    label: Text(l10n.navSettings),
                    disabled: true,
                  ),
                ],
              ),
              const VerticalDivider(width: 1),
              SizedBox(width: 320, child: listPane),
              const VerticalDivider(width: 1),
              Expanded(child: detailPane),
            ],
          ),
        );
      },
    );
  }
}
