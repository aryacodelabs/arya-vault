import 'package:flutter/gestures.dart';
import 'package:flutter/material.dart';

import '../../theme/app_theme.dart';
import '../widgets/l10n_ext.dart';
import 'link_opener.dart';
import 'markdown_parser.dart';

/// Renders [text] with the safe subset of [parseMarkdown]. Links are only
/// tappable for http(s); [onOpenLink] is expected to confirm first.
class MarkdownView extends StatefulWidget {
  const MarkdownView({required this.text, required this.onOpenLink, super.key});

  final String text;
  final void Function(String url) onOpenLink;

  @override
  State<MarkdownView> createState() => _MarkdownViewState();
}

class _MarkdownViewState extends State<MarkdownView> {
  final List<TapGestureRecognizer> _recognizers = <TapGestureRecognizer>[];
  MdDocument? _doc;
  String? _parsedFor;

  @override
  void dispose() {
    _disposeRecognizers();
    super.dispose();
  }

  void _disposeRecognizers() {
    for (final TapGestureRecognizer r in _recognizers) {
      r.dispose();
    }
    _recognizers.clear();
  }

  List<InlineSpan> _spans(List<MdInline> inlines, TextStyle base) {
    final ColorScheme scheme = Theme.of(context).colorScheme;
    return <InlineSpan>[
      for (final MdInline i in inlines)
        switch (i) {
          MdText() => TextSpan(
            text: i.text,
            style: base.copyWith(
              fontWeight: i.bold ? FontWeight.bold : null,
              fontStyle: i.italic ? FontStyle.italic : null,
              fontFamily: i.code ? 'monospace' : null,
              backgroundColor: i.code ? scheme.surfaceContainerHighest : null,
            ),
          ),
          MdImage() => TextSpan(
            // Never fetched: only the alt text and a marker that it was an image.
            text:
                '[${context.l10n.markdownImage}: ${i.alt.isEmpty ? i.url : i.alt}]',
            style: base.copyWith(fontStyle: FontStyle.italic),
          ),
          MdLink() => _link(i, base, scheme),
        },
    ];
  }

  InlineSpan _link(MdLink l, TextStyle base, ColorScheme scheme) {
    final String label = l.text.isEmpty ? l.url : l.text;
    if (!isOpenableUri(Uri.tryParse(l.url))) {
      // javascript:, data:, file:, relative... shown, never launched.
      return TextSpan(text: '$label (${l.url})', style: base);
    }
    final TapGestureRecognizer r = TapGestureRecognizer()
      ..onTap = () => widget.onOpenLink(l.url);
    _recognizers.add(r);
    return TextSpan(
      children: <InlineSpan>[
        TextSpan(
          text: label,
          recognizer: r,
          style: base.copyWith(
            color: scheme.primary,
            decoration: TextDecoration.underline,
          ),
        ),
        // The destination is always visible, so a link cannot hide where it goes.
        TextSpan(text: ' (${l.url})', style: base),
      ],
    );
  }

  Widget _block(MdBlock b, ThemeData theme, AppTokens tokens) {
    final TextStyle body = theme.textTheme.bodyMedium!;
    switch (b) {
      case MdHeading():
        final TextStyle style = switch (b.level) {
          1 => theme.textTheme.headlineSmall!,
          2 => theme.textTheme.titleLarge!,
          _ => theme.textTheme.titleMedium!,
        };
        return Padding(
          padding: EdgeInsets.only(top: tokens.gap / 2),
          child: Semantics(
            header: true,
            child: Text.rich(TextSpan(children: _spans(b.inlines, style))),
          ),
        );
      case MdParagraph():
        return Text.rich(TextSpan(children: _spans(b.inlines, body)));
      case MdListItem():
        return Padding(
          padding: EdgeInsetsDirectional.only(start: 16.0 * (b.depth + 1)),
          child: Row(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: <Widget>[
              SizedBox(width: 28, child: Text(b.marker, style: body)),
              Expanded(
                child: Text.rich(TextSpan(children: _spans(b.inlines, body))),
              ),
            ],
          ),
        );
      case MdQuote():
        return Container(
          padding: const EdgeInsetsDirectional.only(start: 12),
          decoration: BoxDecoration(
            border: BorderDirectional(
              start: BorderSide(color: theme.colorScheme.outline, width: 3),
            ),
          ),
          child: Text.rich(
            TextSpan(
              children: _spans(
                b.inlines,
                body.copyWith(fontStyle: FontStyle.italic),
              ),
            ),
          ),
        );
      case MdCode():
        return Container(
          width: double.infinity,
          padding: EdgeInsets.all(tokens.gap / 2),
          decoration: BoxDecoration(
            color: theme.colorScheme.surfaceContainerHighest,
            borderRadius: BorderRadius.circular(6),
          ),
          child: Text(b.text, style: body.copyWith(fontFamily: 'monospace')),
        );
      case MdRule():
        return const Divider();
    }
  }

  @override
  Widget build(BuildContext context) {
    if (_parsedFor != widget.text) {
      _doc = parseMarkdown(widget.text);
      _parsedFor = widget.text;
    }
    _disposeRecognizers();
    final MdDocument doc = _doc!;
    final ThemeData theme = Theme.of(context);
    final AppTokens tokens = context.tokens;
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: <Widget>[
        for (final MdBlock b in doc.blocks) ...<Widget>[
          _block(b, theme, tokens),
          SizedBox(height: tokens.gap / 2),
        ],
        if (doc.truncated)
          Text(
            context.l10n.markdownTruncated,
            style: theme.textTheme.bodySmall,
          ),
      ],
    );
  }
}
