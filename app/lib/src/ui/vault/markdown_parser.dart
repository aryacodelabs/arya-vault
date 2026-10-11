// A small, safe Markdown subset for secure notes (B02 item 4). In-house on
// purpose: no dependency, and it can only produce what the UI is willing to
// draw. It never produces HTML, images that load, or links that run code:
//   * raw HTML is plain text (what you type is what you see);
//   * an image becomes its alt text (nothing is fetched);
//   * a link keeps its target for display; the view decides if it is openable.
// Everything is bounded (input size, line length, block count) so hostile or
// giant input cannot make the parser quadratic or the screen unbounded.
import 'package:flutter/foundation.dart';

/// Input beyond this many characters is not rendered.
const int kMarkdownMaxChars = 200000;

/// A line longer than this is shown as plain text (no inline parsing), which
/// keeps inline scanning linear.
const int kMarkdownMaxInlineLine = 4000;

/// At most this many blocks are produced.
const int kMarkdownMaxBlocks = 2000;

@immutable
sealed class MdInline {
  const MdInline();
}

class MdText extends MdInline {
  const MdText(
    this.text, {
    this.bold = false,
    this.italic = false,
    this.code = false,
  });
  final String text;
  final bool bold;
  final bool italic;
  final bool code;
}

class MdLink extends MdInline {
  const MdLink(this.text, this.url);
  final String text;
  final String url;
}

/// An image reference, shown as its alt text. [url] is kept only so the UI can
/// say what was not loaded; it is never fetched.
class MdImage extends MdInline {
  const MdImage(this.alt, this.url);
  final String alt;
  final String url;
}

@immutable
sealed class MdBlock {
  const MdBlock();
}

class MdHeading extends MdBlock {
  const MdHeading(this.level, this.inlines);
  final int level;
  final List<MdInline> inlines;
}

class MdParagraph extends MdBlock {
  const MdParagraph(this.inlines);
  final List<MdInline> inlines;
}

class MdListItem extends MdBlock {
  const MdListItem(this.inlines, {required this.marker, required this.depth});
  final List<MdInline> inlines;

  /// `•` or `1.`.
  final String marker;
  final int depth;
}

class MdQuote extends MdBlock {
  const MdQuote(this.inlines);
  final List<MdInline> inlines;
}

class MdCode extends MdBlock {
  const MdCode(this.text);
  final String text;
}

class MdRule extends MdBlock {
  const MdRule();
}

class MdDocument {
  const MdDocument(this.blocks, {required this.truncated});
  final List<MdBlock> blocks;

  /// Input or block limit reached; the rest is not shown.
  final bool truncated;
}

MdDocument parseMarkdown(String source) {
  var truncated = false;
  var text = source;
  if (text.length > kMarkdownMaxChars) {
    text = text.substring(0, kMarkdownMaxChars);
    truncated = true;
  }
  final List<String> lines = text
      .replaceAll('\r\n', '\n')
      .replaceAll('\r', '\n')
      .split('\n');
  final List<MdBlock> blocks = <MdBlock>[];
  final List<String> paragraph = <String>[];
  var inFence = false;
  final StringBuffer fence = StringBuffer();

  bool full() => blocks.length >= kMarkdownMaxBlocks;

  void flushParagraph() {
    if (paragraph.isNotEmpty) {
      blocks.add(MdParagraph(_inline(paragraph.join(' '))));
      paragraph.clear();
    }
  }

  final RegExp heading = RegExp(r'^(#{1,6})\s+(.*)$');
  final RegExp bullet = RegExp(r'^(\s*)[-*+]\s+(.*)$');
  final RegExp ordered = RegExp(r'^(\s*)(\d{1,9})[.)]\s+(.*)$');
  final RegExp rule = RegExp(r'^\s{0,3}([-*_])(\s*\1){2,}\s*$');

  for (final String line in lines) {
    if (full()) {
      truncated = true;
      break;
    }
    if (line.trimLeft().startsWith('```')) {
      if (inFence) {
        blocks.add(MdCode(fence.toString()));
        fence.clear();
        inFence = false;
      } else {
        flushParagraph();
        inFence = true;
      }
      continue;
    }
    if (inFence) {
      if (fence.length < kMarkdownMaxChars) {
        if (fence.isNotEmpty) fence.write('\n');
        fence.write(line);
      }
      continue;
    }
    if (line.trim().isEmpty) {
      flushParagraph();
      continue;
    }
    final RegExpMatch? h = heading.firstMatch(line);
    if (h != null) {
      flushParagraph();
      blocks.add(MdHeading(h.group(1)!.length, _inline(h.group(2)!)));
      continue;
    }
    if (rule.hasMatch(line)) {
      flushParagraph();
      blocks.add(const MdRule());
      continue;
    }
    final RegExpMatch? b = bullet.firstMatch(line);
    if (b != null) {
      flushParagraph();
      blocks.add(
        MdListItem(
          _inline(b.group(2)!),
          marker: '•',
          depth: b.group(1)!.length ~/ 2,
        ),
      );
      continue;
    }
    final RegExpMatch? o = ordered.firstMatch(line);
    if (o != null) {
      flushParagraph();
      blocks.add(
        MdListItem(
          _inline(o.group(3)!),
          marker: '${o.group(2)}.',
          depth: o.group(1)!.length ~/ 2,
        ),
      );
      continue;
    }
    if (line.startsWith('>')) {
      flushParagraph();
      blocks.add(MdQuote(_inline(line.substring(1).trimLeft())));
      continue;
    }
    paragraph.add(line.trim());
  }
  if (inFence && fence.isNotEmpty && !full()) {
    blocks.add(MdCode(fence.toString()));
  }
  flushParagraph();
  return MdDocument(blocks, truncated: truncated);
}

/// Inline parsing: `**bold**`, `*italic*` / `_italic_`, `` `code` ``,
/// `[text](url)` and `![alt](url)`. A marker without its closing partner stays
/// literal. Lines over [kMarkdownMaxInlineLine] are plain text.
List<MdInline> _inline(String s) {
  if (s.length > kMarkdownMaxInlineLine) {
    return <MdInline>[MdText(s)];
  }
  final List<MdInline> out = <MdInline>[];
  final StringBuffer plain = StringBuffer();
  var bold = false;
  var italic = false;

  void flush() {
    if (plain.isNotEmpty) {
      out.add(MdText(plain.toString(), bold: bold, italic: italic));
      plain.clear();
    }
  }

  var i = 0;
  while (i < s.length) {
    final String c = s[i];
    if (c == '`') {
      final int end = s.indexOf('`', i + 1);
      if (end > i + 1) {
        flush();
        out.add(MdText(s.substring(i + 1, end), code: true));
        i = end + 1;
        continue;
      }
    } else if (c == '!' && i + 1 < s.length && s[i + 1] == '[') {
      final ({String text, String url, int end})? l = _linkAt(s, i + 1);
      if (l != null) {
        flush();
        out.add(MdImage(l.text, l.url));
        i = l.end;
        continue;
      }
    } else if (c == '[') {
      final ({String text, String url, int end})? l = _linkAt(s, i);
      if (l != null) {
        flush();
        out.add(MdLink(l.text, l.url));
        i = l.end;
        continue;
      }
    } else if (c == '*' && i + 1 < s.length && s[i + 1] == '*') {
      if (bold || s.indexOf('**', i + 2) > 0) {
        flush();
        bold = !bold;
        i += 2;
        continue;
      }
    } else if ((c == '*' || c == '_') && _canToggleItalic(s, i, italic)) {
      flush();
      italic = !italic;
      i += 1;
      continue;
    }
    plain.write(c);
    i++;
  }
  flush();
  return out;
}

bool _isWordChar(String c) => RegExp(r'[A-Za-z0-9]').hasMatch(c);

bool _canToggleItalic(String s, int i, bool open) {
  final String c = s[i];
  // `snake_case_name` is not italic: an underscore inside a word is literal.
  if (c == '_') {
    final bool before = i > 0 && _isWordChar(s[i - 1]);
    final bool after = i + 1 < s.length && _isWordChar(s[i + 1]);
    if (before && after) return false;
  }
  if (open) return true;
  final bool nextIsSpace = i + 1 >= s.length || s[i + 1] == ' ';
  return !nextIsSpace && s.indexOf(c, i + 1) > 0;
}

/// `[text](url)` starting at [start] (which must be `[`).
({String text, String url, int end})? _linkAt(String s, int start) {
  final int close = s.indexOf(']', start + 1);
  if (close < 0 || close + 1 >= s.length || s[close + 1] != '(') return null;
  final int endParen = s.indexOf(')', close + 2);
  if (endParen < 0) return null;
  return (
    text: s.substring(start + 1, close),
    url: s.substring(close + 2, endParen).trim(),
    end: endParen + 1,
  );
}
