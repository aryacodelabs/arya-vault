import 'dart:async';

import 'package:flutter/material.dart';

import '../../api/secret_bytes.dart';
import 'l10n_ext.dart';

/// Reveal auto-hides after this long (SEC-H03 caps it at 15 s).
const Duration kRevealAutoHide = Duration(seconds: 15);

/// A text controller for secrets: [takeSecret] snapshots the text into a
/// [SecretBytes] and [clearSecret] empties the field.
///
/// The text itself is a Dart `String` and cannot be wiped (see [SecretBytes]);
/// the field is cleared as soon as it is no longer needed.
class SecretTextController extends TextEditingController {
  SecretTextController({super.text});

  SecretBytes takeSecret() => SecretBytes.fromString(text);

  void clearSecret() => value = TextEditingValue.empty;

  /// A widget tree dump prints controllers; never print the content.
  @override
  String toString() => 'SecretTextController(<redacted>)';
}

/// A masked secret input with a reveal toggle that auto-hides.
class SecretTextField extends StatefulWidget {
  const SecretTextField({
    required this.controller,
    required this.label,
    this.errorText,
    this.helperText,
    this.autofocus = false,
    this.textInputAction,
    this.onSubmitted,
    this.onChanged,
    this.focusNode,
    super.key,
  });

  final SecretTextController controller;
  final String label;
  final String? errorText;
  final String? helperText;
  final bool autofocus;
  final TextInputAction? textInputAction;
  final VoidCallback? onSubmitted;
  final VoidCallback? onChanged;
  final FocusNode? focusNode;

  @override
  State<SecretTextField> createState() => _SecretTextFieldState();
}

class _SecretTextFieldState extends State<SecretTextField> {
  bool _revealed = false;
  Timer? _hideTimer;

  void _toggle() {
    setState(() => _revealed = !_revealed);
    _hideTimer?.cancel();
    if (_revealed) {
      _hideTimer = Timer(kRevealAutoHide, () {
        if (mounted) {
          setState(() => _revealed = false);
        }
      });
    }
  }

  @override
  void dispose() {
    _hideTimer?.cancel();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    return TextField(
      controller: widget.controller,
      focusNode: widget.focusNode,
      autofocus: widget.autofocus,
      obscureText: !_revealed,
      enableSuggestions: false,
      autocorrect: false,
      enableIMEPersonalizedLearning: false,
      keyboardType: TextInputType.visiblePassword,
      textInputAction: widget.textInputAction,
      onSubmitted: widget.onSubmitted == null
          ? null
          : (_) => widget.onSubmitted!(),
      onChanged: widget.onChanged == null ? null : (_) => widget.onChanged!(),
      decoration: InputDecoration(
        labelText: widget.label,
        errorText: widget.errorText,
        helperText: widget.helperText,
        suffixIcon: IconButton(
          tooltip: _revealed
              ? context.l10n.hidePassword
              : context.l10n.showPassword,
          icon: Icon(_revealed ? Icons.visibility_off : Icons.visibility),
          onPressed: _toggle,
        ),
      ),
    );
  }
}
