import 'package:flutter/material.dart';

import '../../theme/app_theme.dart';

/// A single-column page: centered, width-limited, scrollable (so it survives
/// 200% text scale), with a header title. Used by every onboarding and lock
/// screen.
class PageScaffold extends StatelessWidget {
  const PageScaffold({
    required this.title,
    required this.children,
    this.leading,
    super.key,
  });

  final String title;
  final List<Widget> children;

  /// Optional widget above the title (for example a back button).
  final Widget? leading;

  @override
  Widget build(BuildContext context) {
    final AppTokens tokens = context.tokens;
    return Scaffold(
      body: SafeArea(
        child: Center(
          child: ConstrainedBox(
            constraints: BoxConstraints(maxWidth: tokens.contentMaxWidth),
            child: SingleChildScrollView(
              padding: EdgeInsets.all(tokens.gap),
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.stretch,
                children: <Widget>[
                  ?leading,
                  Semantics(
                    header: true,
                    child: Text(
                      title,
                      style: Theme.of(context).textTheme.headlineSmall,
                    ),
                  ),
                  SizedBox(height: tokens.gap),
                  ...children,
                ],
              ),
            ),
          ),
        ),
      ),
    );
  }
}
