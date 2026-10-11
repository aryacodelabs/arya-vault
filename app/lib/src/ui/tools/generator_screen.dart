import 'dart:typed_data';

import 'package:flutter/material.dart';

import '../widgets/l10n_ext.dart';
import '../widgets/tool_page.dart';
import 'generator_panel.dart';

/// Tools -> Password generator (docs/07 §6).
class GeneratorScreen extends StatelessWidget {
  const GeneratorScreen({super.key});

  @override
  Widget build(BuildContext context) {
    return ToolPage(
      title: context.l10n.generatorTitle,
      children: const <Widget>[GeneratorPanel()],
    );
  }
}

/// Opens the generator in a dialog and returns the chosen value (the caller
/// wipes it), or null if the user dismissed it. This is the real implementation
/// behind `passwordGeneratorLauncherProvider`.
Future<Uint8List?> showPasswordGeneratorSheet(BuildContext context) {
  return showDialog<Uint8List>(
    context: context,
    builder: (BuildContext ctx) => const PasswordGeneratorSheet(),
  );
}

/// The dialog body: the generator panel with "Use this password" and Cancel.
class PasswordGeneratorSheet extends StatelessWidget {
  const PasswordGeneratorSheet({super.key});

  @override
  Widget build(BuildContext context) {
    final l10n = context.l10n;
    return AlertDialog(
      title: Text(l10n.generatorSheetTitle),
      scrollable: true,
      content: SizedBox(
        width: 480,
        child: GeneratorPanel(
          onUse: (Uint8List bytes) => Navigator.of(context).pop(bytes),
        ),
      ),
      actions: <Widget>[
        TextButton(
          onPressed: () => Navigator.of(context).pop(),
          child: Text(l10n.actionCancel),
        ),
      ],
    );
  }
}
