import 'dart:async';
import 'dart:math' as math;

import 'package:arya_vault/src/api/dtos.dart';
import 'package:arya_vault/src/state/item_list.dart';
import 'package:arya_vault/src/state/providers.dart';
import 'package:arya_vault/src/ui/vault/item_list_pane.dart';
import 'package:flutter/material.dart' hide Page;
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_riverpod/misc.dart' show Override;
import 'package:flutter_test/flutter_test.dart';

import '../helpers.dart';
import 'vault_helpers.dart';

/// The 20,000-item mode (docs/05 §10 target size). Numbers are printed so they
/// can be copied into the PR; the assertions are the acceptance limits.
const int kBig = 20000;

Future<SpyApi> bigVault() => unlockedVault(items: kBig, trash: 0);

Finder rowsBuilt() => find.byWidgetPredicate(
  (Widget w) =>
      w.key is ValueKey<String> &&
      (w.key! as ValueKey<String>).value.length == 32,
  description: 'item rows',
);

Finder bigList() => find.descendant(
  of: find.byType(ItemListPane),
  matching: find.byType(ListView),
);

void main() {
  setUpAll(() => WidgetController.hitTestWarningShouldBeFatal = true);

  testWidgets('the 20,000-item fake pages and counts correctly', (
    WidgetTester tester,
  ) async {
    final SpyApi api = await bigVault();
    expect(await api.itemCount(), kBig);
    final Stopwatch sw = Stopwatch()..start();
    final List<ItemSummary> p1 = await api.list(
      const ListFilter(),
      const Page(offset: 0, limit: 200),
    );
    final List<ItemSummary> deep = await api.list(
      const ListFilter(),
      const Page(offset: 19800, limit: 200),
    );
    sw.stop();
    expect(p1.first.title, sampleTitle(kBig - 1), reason: 'newest first');
    expect(deep.last.title, sampleTitle(0));
    debugPrint(
      'BENCH fake: first + last page of 20,000 items: ${sw.elapsedMilliseconds} ms',
    );
  });

  testWidgets('initial list: under 100 ms of frame work', (
    WidgetTester tester,
  ) async {
    // The list pane alone, on an already-running MaterialApp: the frame that
    // builds the first rows of a 20,000-item vault, without the cost of the
    // first frame of the whole app (measured separately below).
    final SpyApi api = await bigVault();
    final List<int> trials = <int>[];
    tester.view.physicalSize = const Size(420, 900);
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.reset);
    Widget app(Widget body) => ProviderScope(
      retry: noAutoRetry,
      overrides: <Override>[vaultApiProvider.overrideWithValue(api)],
      child: MaterialApp(
        localizationsDelegates: appDelegates,
        theme: appTheme,
        home: Scaffold(body: body),
      ),
    );
    for (var t = 0; t < 3; t++) {
      await tester.pumpWidget(app(const SizedBox()));
      await tester.pumpAndSettle();
      await tester.pumpWidget(app(const ItemListPane()));
      Duration? build;
      for (var i = 0; i < 60 && build == null; i++) {
        final Stopwatch sw = Stopwatch()..start();
        await tester.pump(const Duration(milliseconds: 16));
        sw.stop();
        if (rowsBuilt().evaluate().isNotEmpty) build = sw.elapsed;
      }
      expect(build, isNotNull, reason: 'rows appeared');
      trials.add(build!.inMilliseconds);
      await tester.pumpWidget(const SizedBox());
    }
    trials.sort();

    // Calibration: a bare 100-row ListView of ListTiles on the same runner. The
    // test runs on the JIT in debug mode, where even that takes about 100 ms, so
    // an absolute 100 ms bound can only be checked in profile mode on the target
    // (L01). Here the pane must stay within 2x of the bare list, or under 100 ms.
    final List<int> bare = <int>[];
    for (var t = 0; t < 3; t++) {
      await tester.pumpWidget(
        const MaterialApp(home: Scaffold(body: SizedBox())),
      );
      await tester.pumpAndSettle();
      final Stopwatch sw = Stopwatch()..start();
      await tester.pumpWidget(
        MaterialApp(
          home: Scaffold(
            body: ListView.builder(
              itemExtent: 76,
              itemCount: 101,
              itemBuilder: (BuildContext c, int i) => ListTile(
                leading: const Icon(Icons.key),
                title: Text('Title $i'),
                subtitle: Text('sub $i'),
                trailing: IconButton(
                  icon: const Icon(Icons.star_border),
                  onPressed: () {},
                ),
              ),
            ),
          ),
        ),
      );
      sw.stop();
      bare.add(sw.elapsedMilliseconds);
    }
    bare.sort();
    debugPrint(
      'BENCH initial list frame, list pane only (min of 3, debug JIT): '
      '${trials.first} ms (all: $trials); '
      'bare ListView of ListTiles on the same runner: ${bare.first} ms',
    );
    expect(trials.first, lessThan(math.max(100, bare.first * 2)));
  });

  testWidgets('whole-app first frame with 20,000 items (reported, loose bound)', (
    WidgetTester tester,
  ) async {
    final SpyApi api = await bigVault();
    await pumpVault(tester, api: api, settle: false);
    Duration? build;
    for (var i = 0; i < 60 && build == null; i++) {
      final Stopwatch sw = Stopwatch()..start();
      await tester.pump(const Duration(milliseconds: 16));
      sw.stop();
      if (rowsBuilt().evaluate().isNotEmpty) build = sw.elapsed;
    }
    debugPrint(
      'BENCH whole-app first frame with rows (single run, debug JIT): ${build!.inMilliseconds} ms',
    );
    expect(build.inMilliseconds, lessThan(1000));
  });

  testWidgets(
    'scrolling through 10 pages builds rows once, never the whole list',
    (WidgetTester tester) async {
      final SpyApi api = await bigVault();
      await pumpVault(tester, api: api);
      debugRowBuilds = 0;
      var maxConcurrent = rowsBuilt().evaluate().length;
      for (var page = 0; page < 10; page++) {
        await tester.drag(bigList(), const Offset(0, -20000));
        await tester.pumpAndSettle();
        maxConcurrent = maxConcurrent < rowsBuilt().evaluate().length
            ? rowsBuilt().evaluate().length
            : maxConcurrent;
      }
      final ProviderLoaded loaded = ProviderLoaded(tester);
      expect(
        loaded.items,
        greaterThanOrEqualTo(1000),
        reason: 'ten pages are loaded',
      );
      debugPrint(
        'BENCH scroll: ${loaded.items} items loaded, $debugRowBuilds row builds, max $maxConcurrent rows alive at once',
      );
      expect(
        maxConcurrent,
        lessThan(40),
        reason: 'virtualized: only visible rows exist',
      );
      // A whole-list rebuild per appended page would be ~sum(100..1000) = 5,500.
      expect(debugRowBuilds, lessThan((loaded.items * 1.5).round()));
    },
  );

  testWidgets('appending a page rebuilds only the rows in view', (
    WidgetTester tester,
  ) async {
    final SpyApi api = await bigVault();
    final Harness h = await pumpVault(tester, api: api);
    await tester.drag(bigList(), const Offset(0, -20000));
    await tester.pumpAndSettle();
    expect(h.container.read(itemListProvider).value!.items.length, 200);
    // Load one more page while standing still near the end.
    debugRowBuilds = 0;
    await h.container.read(itemListProvider.notifier).loadMore();
    await tester.pumpAndSettle();
    expect(h.container.read(itemListProvider).value!.items.length, 300);
    debugPrint('BENCH append: $debugRowBuilds row builds for a 100-row append');
    expect(debugRowBuilds, lessThan(40));
  });

  testWidgets(
    'search at 20,000 items: one request for fast typing, newest answer wins',
    (WidgetTester tester) async {
      final SpyApi api = await bigVault();
      final Completer<void> slow = Completer<void>();
      final Completer<void> mid = Completer<void>();
      api.gates['Falcon'] = slow;
      api.gates['Cobalt'] = mid;
      await pumpVault(tester, api: api);

      // Fast typing: many keystrokes inside the debounce window -> one request.
      for (final String t in <String>[
        'F',
        'Fa',
        'Fal',
        'Falc',
        'Falco',
        'Falcon',
      ]) {
        await typeSearch(tester, t);
        await tester.pump(const Duration(milliseconds: 20));
      }
      await tester.pump(const Duration(milliseconds: 200));
      expect(api.count('search'), 1);
      // Meanwhile the user changes their mind.
      await typeSearch(tester, 'Cobalt');
      await tester.pump(const Duration(milliseconds: 200));
      expect(api.count('search'), 2);
      await typeSearch(tester, 'Dune');
      await tester.pump(const Duration(milliseconds: 200));
      await tester.pumpAndSettle();
      expect(find.textContaining('Dune', findRichText: true), findsWidgets);

      // The older, slower answers arrive in the "wrong" order and are ignored.
      mid.complete();
      await tester.pumpAndSettle();
      slow.complete();
      await tester.pumpAndSettle();
      expect(find.textContaining('Falcon'), findsNothing);
      expect(find.textContaining('Cobalt'), findsNothing);
      expect(find.textContaining('Dune'), findsWidgets);
      expect(
        textOf(l10n.resultsCountMore(100)),
        findsOneWidget,
        reason: '1,000 Dune items, first page of 100',
      );
    },
  );

  testWidgets('search results page like the list does', (
    WidgetTester tester,
  ) async {
    final SpyApi api = await bigVault();
    final Harness h = await pumpVault(tester, api: api);
    await typeSearch(tester, 'Echo');
    await tester.pump(const Duration(milliseconds: 200));
    await tester.pumpAndSettle();
    for (var i = 0; i < 3; i++) {
      await tester.drag(bigList(), const Offset(0, -20000));
      await tester.pumpAndSettle();
    }
    final int n = h.container.read(itemListProvider).value!.items.length;
    expect(n, greaterThan(100));
    expect(
      h.container
          .read(itemListProvider)
          .value!
          .items
          .every(
            (ItemSummary s) =>
                s.title.startsWith('Echo') || s.subtitle.contains('Echo'),
          ),
      isTrue,
    );
  });
}

/// Reads how many items the list holds, without exposing the container.
class ProviderLoaded {
  ProviderLoaded(WidgetTester tester) {
    final Element e = tester.element(find.byType(ItemListPane));
    items =
        (ProviderScope.containerOf(e)
            .read(itemListProvider)
            .value
            ?.items
            .length) ??
        0;
  }
  late final int items;
}
