import 'package:arya_vault/src/ui/vault/markdown_parser.dart';
import 'package:arya_vault/src/ui/vault/markdown_view.dart';
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

import '../helpers.dart';

List<MdInline> firstInlines(String md) {
  final MdBlock b = parseMarkdown(md).blocks.first;
  return switch (b) {
    MdParagraph() => b.inlines,
    MdHeading() => b.inlines,
    MdListItem() => b.inlines,
    MdQuote() => b.inlines,
    _ => <MdInline>[],
  };
}

String plain(List<MdInline> inlines) => inlines
    .map(
      (MdInline i) => switch (i) {
        MdText() => i.text,
        MdLink() => i.text,
        MdImage() => i.alt,
      },
    )
    .join();

Future<void> pumpMarkdown(
  WidgetTester tester,
  String text, {
  void Function(String url)? onLink,
}) async {
  tester.view.physicalSize = const Size(600, 900);
  tester.view.devicePixelRatio = 1;
  addTearDown(tester.view.reset);
  await tester.pumpWidget(
    MaterialApp(
      localizationsDelegates: appDelegates,
      theme: appTheme,
      home: Scaffold(
        body: SingleChildScrollView(
          child: MarkdownView(text: text, onOpenLink: onLink ?? (_) {}),
        ),
      ),
    ),
  );
}

void main() {
  group('parser', () {
    test('headings, paragraphs, lists, quotes, rules, code', () {
      final MdDocument d = parseMarkdown(
        '# Title\n\nHello **bold** and *it* and `code`.\n\n- one\n- two\n1. a\n> quoted\n---\n```\ncode block\n```\n',
      );
      expect(d.blocks.whereType<MdHeading>().single.level, 1);
      expect(
        d.blocks.whereType<MdListItem>().map((MdListItem i) => i.marker),
        <String>['•', '•', '1.'],
      );
      expect(d.blocks.whereType<MdQuote>(), hasLength(1));
      expect(d.blocks.whereType<MdRule>(), hasLength(1));
      expect(d.blocks.whereType<MdCode>().single.text, 'code block');
      final MdParagraph p = d.blocks.whereType<MdParagraph>().single;
      expect(
        p.inlines.whereType<MdText>().any(
          (MdText t) => t.bold && t.text == 'bold',
        ),
        isTrue,
      );
      expect(
        p.inlines.whereType<MdText>().any(
          (MdText t) => t.italic && t.text == 'it',
        ),
        isTrue,
      );
      expect(
        p.inlines.whereType<MdText>().any(
          (MdText t) => t.code && t.text == 'code',
        ),
        isTrue,
      );
    });

    test('unmatched markers stay literal', () {
      expect(
        plain(firstInlines('2 * 3 = 6 and **not closed')),
        '2 * 3 = 6 and **not closed',
      );
      expect(
        plain(firstInlines('snake_case_name stays')),
        'snake_case_name stays',
      );
    });

    test('raw HTML is plain text', () {
      final List<MdInline> i = firstInlines(
        '<script>alert(1)</script> <img src=x onerror=alert(1)> <b>bold?</b>',
      );
      expect(
        plain(i),
        '<script>alert(1)</script> <img src=x onerror=alert(1)> <b>bold?</b>',
      );
      expect(i.every((MdInline e) => e is MdText && !e.bold), isTrue);
    });

    test('an image is its alt text, never a fetch', () {
      final List<MdInline> i = firstInlines(
        '![logo](https://tracker.invalid/pixel.png)',
      );
      expect(i.single, isA<MdImage>());
      expect((i.single as MdImage).alt, 'logo');
    });

    test('links keep their target for the view to judge', () {
      final List<MdInline> i = firstInlines(
        '[a](https://example.invalid) [b](javascript:alert(1))',
      );
      expect(i.whereType<MdLink>().map((MdLink l) => l.url), <String>[
        'https://example.invalid',
        'javascript:alert(1)',
      ]);
    });

    test('giant input is cut off, not rendered whole', () {
      final MdDocument d = parseMarkdown('a ' * (kMarkdownMaxChars));
      expect(d.truncated, isTrue);
    });

    test('many blocks are capped', () {
      final MdDocument d = parseMarkdown(
        List<String>.generate(
          kMarkdownMaxBlocks + 500,
          (int i) => '- item $i',
        ).join('\n'),
      );
      expect(d.blocks.length, kMarkdownMaxBlocks);
      expect(d.truncated, isTrue);
    });

    test('pathological inline input stays fast (no quadratic scan)', () {
      final Stopwatch sw = Stopwatch()..start();
      parseMarkdown('* ' * 1999); // many unmatched markers on one allowed line
      parseMarkdown('[' * 3000);
      parseMarkdown('`' * 3000);
      parseMarkdown('${'**' * 1500}x');
      parseMarkdown(
        '${'*' * 100000}\n${'_' * 100000}',
      ); // over the inline line limit
      expect(sw.elapsedMilliseconds, lessThan(1500));
    });

    test('an over-long line is not inline-parsed', () {
      final MdParagraph p =
          parseMarkdown('**${'x' * (kMarkdownMaxInlineLine + 10)}**')
                  .blocks
                  .single
              as MdParagraph;
      expect(p.inlines, hasLength(1));
      expect((p.inlines.single as MdText).bold, isFalse);
    });

    test('unterminated code fence still yields the code', () {
      expect(parseMarkdown('```\nunclosed').blocks.single, isA<MdCode>());
    });
  });

  group('view', () {
    testWidgets('renders text without any Image widget', (
      WidgetTester tester,
    ) async {
      await pumpMarkdown(
        tester,
        '# Hi\n\n![x](https://tracker.invalid/p.png) text',
      );
      expect(find.byType(Image), findsNothing);
      expect(
        find.textContaining('image not loaded', findRichText: true),
        findsOneWidget,
      );
    });

    testWidgets('an https link is tappable and shows its destination', (
      WidgetTester tester,
    ) async {
      final List<String> opened = <String>[];
      await pumpMarkdown(
        tester,
        'See [the docs](https://example.invalid/a).',
        onLink: opened.add,
      );
      expect(
        find.textContaining('(https://example.invalid/a)', findRichText: true),
        findsOneWidget,
      );
      await tester.tapAt(
        tester.getTopLeft(find.byType(MarkdownView)) + const Offset(40, 10),
      );
      await tester.pump();
      // Tap the link text span itself.
      final Finder rich = find.byType(RichText).first;
      final RenderBox box = tester.renderObject<RenderBox>(rich);
      expect(box.size.width, greaterThan(0));
      // Use the semantics-free approach: find and invoke the recognizer.
      final RichText widget = tester.widget<RichText>(rich);
      TextSpan? linkSpan;
      widget.text.visitChildren((InlineSpan s) {
        if (s is TextSpan && s.recognizer != null) linkSpan = s;
        return true;
      });
      expect(linkSpan, isNotNull);
      (linkSpan!.recognizer! as dynamic).onTap();
      expect(opened, <String>['https://example.invalid/a']);
    });

    for (final String bad in <String>[
      'javascript:alert(1)',
      'JaVaScRiPt:alert(1)',
      'data:text/html;base64,PHNjcmlwdD4=',
      'file:///etc/passwd',
      'vbscript:x',
      '//evil.invalid/x',
      '/relative',
    ]) {
      testWidgets('a "$bad" link is text only, never tappable', (
        WidgetTester tester,
      ) async {
        final List<String> opened = <String>[];
        await pumpMarkdown(
          tester,
          '[click](<$bad>) [click2]($bad)',
          onLink: opened.add,
        );
        final Finder rich = find.byType(RichText);
        var recognizers = 0;
        for (final Element e in rich.evaluate()) {
          (e.widget as RichText).text.visitChildren((InlineSpan s) {
            if (s is TextSpan && s.recognizer != null) recognizers++;
            return true;
          });
        }
        expect(recognizers, 0);
        expect(opened, isEmpty);
      });
    }

    testWidgets('giant input renders a bounded tree with a notice', (
      WidgetTester tester,
    ) async {
      final Stopwatch sw = Stopwatch()..start();
      await pumpMarkdown(
        tester,
        List<String>.generate(5000, (int i) => '- line $i').join('\n'),
      );
      expect(
        find.textContaining('only the first part is shown'),
        findsOneWidget,
      );
      expect(sw.elapsedMilliseconds, lessThan(8000));
    });
  });
}
