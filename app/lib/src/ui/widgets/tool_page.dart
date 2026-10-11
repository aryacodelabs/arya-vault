import 'package:flutter/material.dart';

import '../../theme/app_theme.dart';

/// A pushed page with an app bar (the system back button works), a width limit
/// and scrolling; the shell of every tool and settings screen.
class ToolPage extends StatelessWidget {
  const ToolPage({
    required this.title,
    required this.children,
    this.actions = const <Widget>[],
    super.key,
  });

  final String title;
  final List<Widget> children;
  final List<Widget> actions;

  @override
  Widget build(BuildContext context) {
    final AppTokens tokens = context.tokens;
    return Scaffold(
      appBar: AppBar(
        title: Semantics(header: true, child: Text(title)),
        actions: actions,
      ),
      body: SafeArea(
        child: Center(
          child: ConstrainedBox(
            constraints: const BoxConstraints(maxWidth: 640),
            child: ListView(
              padding: EdgeInsets.all(tokens.gap),
              children: children,
            ),
          ),
        ),
      ),
    );
  }
}

/// A titled block on a tool page.
class ToolSection extends StatelessWidget {
  const ToolSection({
    required this.title,
    required this.children,
    this.explain,
    super.key,
  });

  final String title;
  final String? explain;
  final List<Widget> children;

  @override
  Widget build(BuildContext context) {
    final ThemeData theme = Theme.of(context);
    final AppTokens tokens = context.tokens;
    return Padding(
      padding: EdgeInsets.only(bottom: tokens.gapLarge),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: <Widget>[
          Semantics(
            header: true,
            child: Text(title, style: theme.textTheme.titleMedium),
          ),
          if (explain != null) ...<Widget>[
            SizedBox(height: tokens.gap / 4),
            Text(explain!, style: theme.textTheme.bodySmall),
          ],
          SizedBox(height: tokens.gap / 2),
          ...children,
        ],
      ),
    );
  }
}
