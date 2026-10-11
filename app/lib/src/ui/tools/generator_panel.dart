import 'dart:async';
import 'dart:typed_data';

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../api/app_error.dart';
import '../../api/dtos.dart';
import '../../api/secret_bytes.dart';
import '../../services/services.dart';
import '../../state/generator_state.dart';
import '../../state/providers.dart';
import '../../state/security_services.dart';
import '../../state/vault_session.dart';
import '../../theme/app_theme.dart';
import '../widgets/error_text.dart';
import '../widgets/l10n_ext.dart';
import '../widgets/strength_meter.dart';

/// The generator: password or passphrase options, the value, live entropy and
/// strength (all from the core), regenerate, copy, and optionally "use".
///
/// Used by the Generator screen (no [onUse]) and by the generator sheet the
/// edit form opens ([onUse] hands the bytes back; the receiver wipes them).
class GeneratorPanel extends ConsumerStatefulWidget {
  const GeneratorPanel({this.onUse, super.key});

  /// Called with a fresh copy of the value when the user taps "Use this
  /// password". The callee owns (and wipes) the bytes.
  final void Function(Uint8List bytes)? onUse;

  @override
  ConsumerState<GeneratorPanel> createState() => _GeneratorPanelState();
}

class _GeneratorPanelState extends ConsumerState<GeneratorPanel> {
  late final GeneratorController _controller = GeneratorController(
    ref.read(vaultApiProvider),
  );
  final TextEditingController _symbols = TextEditingController(
    text: const PasswordOptions().symbolSet,
  );
  final TextEditingController _separator = TextEditingController(
    text: const PassphraseOptions().separator,
  );
  bool _visible = true;

  @override
  void initState() {
    super.initState();
    _controller.addListener(_onChanged);
    ref.listenManual(lockEpochProvider, (int? _, int _) => _controller.wipe());
    unawaited(_controller.regenerate());
  }

  void _onChanged() {
    final AppError? e = _controller.error;
    if (e != null) handleLockedWidget(ref, e);
    if (mounted) setState(() {});
  }

  @override
  void dispose() {
    _controller.removeListener(_onChanged);
    _controller.dispose(); // wipes the value
    _symbols.dispose();
    _separator.dispose();
    super.dispose();
  }

  Future<void> _copy() async {
    final l10n = context.l10n;
    final ScaffoldMessengerState messenger = ScaffoldMessenger.of(context);
    final SecretBytes? copy = _controller.takeCopy();
    if (copy == null) return;
    final manager = ref.read(clipboardManagerProvider);
    try {
      await manager.copySecret(copy);
      messenger
        ..hideCurrentSnackBar()
        ..showSnackBar(
          SnackBar(
            content: Text(l10n.copiedToast(manager.clearAfter.inSeconds)),
          ),
        );
    } on ServiceUnavailable {
      copy.wipe();
      messenger
        ..hideCurrentSnackBar()
        ..showSnackBar(
          SnackBar(content: Text(l10n.recoveryServiceUnavailable)),
        );
    }
  }

  void _use() {
    final SecretBytes? copy = _controller.takeCopy();
    if (copy == null) return;
    // Hand the buffer over; the receiver wipes it.
    widget.onUse?.call(copy.copyBytes());
    copy.wipe();
  }

  @override
  Widget build(BuildContext context) {
    final l10n = context.l10n;
    final AppTokens tokens = context.tokens;
    final ThemeData theme = Theme.of(context);
    final GeneratorController c = _controller;
    final String? text = c.displayText();
    final TextStyle mono = theme.textTheme.titleMedium!.copyWith(
      fontFamily: 'monospace',
      fontFamilyFallback: const <String>['Consolas', 'Menlo', 'Courier New'],
      color: tokens.onKeyGroup,
    );
    final String? problem = switch (c.problem) {
      GeneratorProblem.noCharacterClass => l10n.generatorNeedClass,
      GeneratorProblem.noSymbols => l10n.generatorNeedSymbols,
      null => null,
    };
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: <Widget>[
        SegmentedButton<GeneratorMode>(
          segments: <ButtonSegment<GeneratorMode>>[
            ButtonSegment<GeneratorMode>(
              value: GeneratorMode.password,
              label: Text(l10n.generatorModePassword),
            ),
            ButtonSegment<GeneratorMode>(
              value: GeneratorMode.passphrase,
              label: Text(l10n.generatorModePassphrase),
            ),
          ],
          selected: <GeneratorMode>{c.mode},
          onSelectionChanged: (Set<GeneratorMode> s) => c.setMode(s.first),
        ),
        SizedBox(height: tokens.gap),
        Container(
          padding: EdgeInsets.all(tokens.gap),
          decoration: BoxDecoration(
            color: tokens.keyGroupBackground,
            borderRadius: BorderRadius.circular(8),
          ),
          child: Row(
            children: <Widget>[
              Expanded(
                child: Semantics(
                  container: true,
                  label: l10n.generatorValueLabel,
                  child: SelectionContainer.disabled(
                    child: Text(
                      text == null
                          ? ' '
                          : (_visible
                                ? text
                                : '•' * text.runes.length.clamp(1, 40)),
                      style: mono,
                      softWrap: true,
                    ),
                  ),
                ),
              ),
              IconButton(
                tooltip: _visible ? l10n.generatorHide : l10n.generatorShow,
                icon: Icon(_visible ? Icons.visibility_off : Icons.visibility),
                onPressed: () => setState(() => _visible = !_visible),
              ),
              IconButton(
                tooltip: l10n.generatorRegenerate,
                icon: const Icon(Icons.refresh),
                onPressed: c.busy ? null : () => unawaited(c.regenerate()),
              ),
            ],
          ),
        ),
        if (c.error != null) ...<Widget>[
          SizedBox(height: tokens.gap / 2),
          ErrorText(
            c.error!.code == AppErrorCode.validation
                ? l10n.generatorFailed
                : errorMessage(l10n, c.error!),
          ),
        ],
        if (problem != null) ...<Widget>[
          SizedBox(height: tokens.gap / 2),
          ErrorText(problem),
        ],
        SizedBox(height: tokens.gap / 2),
        if (c.entropyBits != null)
          Text(
            l10n.generatorEntropy(c.entropyBits!.round().toString()),
            style: theme.textTheme.bodyMedium,
          ),
        SizedBox(height: tokens.gap / 2),
        StrengthMeter(strength: c.strength, policy: null, showEmptyHint: false),
        SizedBox(height: tokens.gap),
        Wrap(
          spacing: tokens.gap,
          runSpacing: tokens.gap / 2,
          children: <Widget>[
            OutlinedButton.icon(
              onPressed: c.hasValue ? _copy : null,
              icon: const Icon(Icons.copy),
              label: Text(l10n.generatorCopy),
            ),
            if (widget.onUse != null)
              FilledButton(
                onPressed: c.hasValue ? _use : null,
                child: Text(l10n.generatorUse),
              ),
          ],
        ),
        SizedBox(height: tokens.gapLarge),
        if (c.mode == GeneratorMode.password)
          ..._passwordOptions(context, c)
        else
          ..._passphraseOptions(context, c),
      ],
    );
  }

  List<Widget> _passwordOptions(BuildContext context, GeneratorController c) {
    final l10n = context.l10n;
    final PasswordOptions o = c.password;
    PasswordOptions copy({
      int? length,
      bool? lower,
      bool? upper,
      bool? digits,
      bool? symbols,
      String? symbolSet,
      bool? excludeAmbiguous,
    }) => PasswordOptions(
      length: length ?? o.length,
      lower: lower ?? o.lower,
      upper: upper ?? o.upper,
      digits: digits ?? o.digits,
      symbols: symbols ?? o.symbols,
      symbolSet: symbolSet ?? o.symbolSet,
      excludeAmbiguous: excludeAmbiguous ?? o.excludeAmbiguous,
      requireEachClass: o.requireEachClass,
    );
    return <Widget>[
      Semantics(
        container: true,
        label: l10n.generatorLength(o.length),
        child: ExcludeSemantics(child: Text(l10n.generatorLength(o.length))),
      ),
      Slider(
        value: o.length.toDouble(),
        min: 8,
        max: 128,
        divisions: 120,
        label: o.length.toString(),
        semanticFormatterCallback: (double v) =>
            l10n.generatorLength(v.round()),
        onChanged: (double v) => c.setPassword(copy(length: v.round())),
      ),
      SwitchListTile(
        title: Text(l10n.generatorLower),
        value: o.lower,
        onChanged: (bool v) => c.setPassword(copy(lower: v)),
      ),
      SwitchListTile(
        title: Text(l10n.generatorUpper),
        value: o.upper,
        onChanged: (bool v) => c.setPassword(copy(upper: v)),
      ),
      SwitchListTile(
        title: Text(l10n.generatorDigits),
        value: o.digits,
        onChanged: (bool v) => c.setPassword(copy(digits: v)),
      ),
      SwitchListTile(
        title: Text(l10n.generatorSymbols),
        value: o.symbols,
        onChanged: (bool v) => c.setPassword(copy(symbols: v)),
      ),
      if (o.symbols)
        Padding(
          padding: const EdgeInsets.symmetric(vertical: 8),
          child: TextField(
            controller: _symbols,
            decoration: InputDecoration(labelText: l10n.generatorSymbolSet),
            onChanged: (String v) => c.setPassword(copy(symbolSet: v)),
          ),
        ),
      SwitchListTile(
        title: Text(l10n.generatorExcludeAmbiguous),
        value: o.excludeAmbiguous,
        onChanged: (bool v) => c.setPassword(copy(excludeAmbiguous: v)),
      ),
    ];
  }

  List<Widget> _passphraseOptions(BuildContext context, GeneratorController c) {
    final l10n = context.l10n;
    final PassphraseOptions o = c.passphrase;
    PassphraseOptions copy({
      int? wordCount,
      String? separator,
      bool? capitalize,
      bool? numberSuffix,
    }) => PassphraseOptions(
      wordCount: wordCount ?? o.wordCount,
      separator: separator ?? o.separator,
      capitalize: capitalize ?? o.capitalize,
      numberSuffix: numberSuffix ?? o.numberSuffix,
    );
    return <Widget>[
      Semantics(
        container: true,
        label: l10n.generatorWords(o.wordCount),
        child: ExcludeSemantics(child: Text(l10n.generatorWords(o.wordCount))),
      ),
      Slider(
        value: o.wordCount.toDouble(),
        min: 3,
        max: 12,
        divisions: 9,
        label: o.wordCount.toString(),
        semanticFormatterCallback: (double v) => l10n.generatorWords(v.round()),
        onChanged: (double v) => c.setPassphrase(copy(wordCount: v.round())),
      ),
      Padding(
        padding: const EdgeInsets.symmetric(vertical: 8),
        child: TextField(
          controller: _separator,
          maxLength: 3,
          decoration: InputDecoration(labelText: l10n.generatorSeparator),
          onChanged: (String v) => c.setPassphrase(copy(separator: v)),
        ),
      ),
      SwitchListTile(
        title: Text(l10n.generatorCapitalize),
        value: o.capitalize,
        onChanged: (bool v) => c.setPassphrase(copy(capitalize: v)),
      ),
      SwitchListTile(
        title: Text(l10n.generatorNumber),
        value: o.numberSuffix,
        onChanged: (bool v) => c.setPassphrase(copy(numberSuffix: v)),
      ),
    ];
  }
}
