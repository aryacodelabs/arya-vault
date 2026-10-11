import 'dart:async';
import 'dart:typed_data';

import 'package:arya_vault/src/api/app_error.dart';
import 'package:arya_vault/src/api/dtos.dart';
import 'package:arya_vault/src/api/fake_vault_api.dart';
import 'package:arya_vault/src/state/vault_query.dart';
import 'package:arya_vault/src/ui/vault/item_list_pane.dart';
import 'package:flutter/material.dart' hide Page;
import 'package:flutter_riverpod/misc.dart' show Override;
import 'package:flutter_test/flutter_test.dart';

import '../helpers.dart';
import '../screen_cases.dart';

/// Title of sample item [i] (see FakeVaultApi._seedSamples).
String sampleTitle(int i) {
  const List<String> sites = <String>[
    'Aurora',
    'Beacon',
    'Cobalt',
    'Dune',
    'Echo',
    'Falcon',
    'Garnet',
    'Horizon',
    'Indigo',
    'Juniper',
    'Kestrel',
    'Lumen',
    'Mosaic',
    'Nimbus',
    'Onyx',
    'Prism',
    'Quill',
    'Raven',
    'Sable',
    'Tundra',
  ];
  const List<String> suffixes = <String>[
    'Mail',
    'Bank',
    'Cloud',
    'Shop',
    'Social',
    'Docs',
    'Cards',
    'Travel',
  ];
  return '${sites[i % sites.length]} ${suffixes[(i ~/ sites.length) % suffixes.length]} $i';
}

/// A fake that records calls, captures revealed buffers and can hold searches.
class SpyApi extends FakeVaultApi {
  SpyApi({
    super.sampleItemCount = 60,
    super.sampleTrashCount = 5,
    super.seedSampleItems = true,
    super.quickUnlock,
    super.random,
    super.clock,
  });

  final List<String> calls = <String>[];
  final List<Uint8List> revealed = <Uint8List>[];
  final List<ListFilter> listFilters = <ListFilter>[];

  /// Searches whose text is a key wait for the matching completer.
  final Map<String, Completer<void>> gates = <String, Completer<void>>{};

  /// If set, the next call to the named method throws this.
  final Map<String, AppError> failNext = <String, AppError>{};

  void _maybeFail(String name) {
    final AppError? e = failNext.remove(name);
    if (e != null) throw e;
  }

  int count(String name) => calls.where((String c) => c == name).length;

  @override
  Future<List<ItemSummary>> list(ListFilter filter, Page page) {
    calls.add('list');
    listFilters.add(filter);
    _maybeFail('list');
    return super.list(filter, page);
  }

  @override
  Future<List<ItemSummary>> search(SearchQuery query) async {
    calls.add('search');
    _maybeFail('search');
    final Completer<void>? gate = gates[query.text];
    if (gate != null) await gate.future;
    return super.search(query);
  }

  @override
  Future<ItemView> getItem(String id) {
    calls.add('getItem');
    _maybeFail('getItem');
    return super.getItem(id);
  }

  @override
  Future<Uint8List?> reveal(String id, StdField field) async {
    calls.add('reveal');
    _maybeFail('reveal');
    final Uint8List? b = await super.reveal(id, field);
    if (b != null) revealed.add(b);
    return b;
  }

  @override
  Future<Uint8List?> revealCustom(String id, String elementId) async {
    calls.add('revealCustom');
    final Uint8List? b = await super.revealCustom(id, elementId);
    if (b != null) revealed.add(b);
    return b;
  }

  @override
  Future<Uint8List?> revealVersion(String id, StdField field, int hlcMs) async {
    calls.add('revealVersion');
    final Uint8List? b = await super.revealVersion(id, field, hlcMs);
    if (b != null) revealed.add(b);
    return b;
  }

  @override
  Future<void> setFields(String id, Map<StdField, String> fields) {
    calls.add('setFields');
    _maybeFail('setFields');
    return super.setFields(id, fields);
  }

  @override
  Future<void> setField(String id, StdField field, String value) {
    calls.add('setField');
    return super.setField(id, field, value);
  }

  @override
  Future<String> createItem(NewItem item) {
    calls.add('createItem');
    _maybeFail('createItem');
    return super.createItem(item);
  }

  @override
  Future<void> deleteItem(String id) {
    calls.add('deleteItem');
    _maybeFail('deleteItem');
    return super.deleteItem(id);
  }

  @override
  Future<void> restoreItem(String id) {
    calls.add('restoreItem');
    return super.restoreItem(id);
  }

  @override
  Future<void> purgeItem(String id) {
    calls.add('purgeItem');
    return super.purgeItem(id);
  }

  @override
  Future<int> emptyTrash() {
    calls.add('emptyTrash');
    return super.emptyTrash();
  }
}

/// An unlocked, onboarded vault with sample data.
Future<SpyApi> unlockedVault({
  int items = 60,
  int trash = 5,
  bool samples = true,
}) async {
  final SpyApi api = SpyApi(
    sampleItemCount: items,
    sampleTrashCount: trash,
    seedSampleItems: samples,
    random: seeded(),
    // A fixed clock keeps dates (and so golden images) stable.
    clock: () => DateTime.utc(2026, 6, 1, 12),
  );
  final RecoveryKeyResult r = await createThroughApi(api);
  final List<String> g = keyGroups(r);
  await api.confirmRecoveryKey(<GroupAnswer>[
    for (final int i in r.challenge) GroupAnswer(index: i, text: g[i]),
  ]);
  api.calls.clear();
  api.listFilters.clear();
  return api;
}

const Size kDesktop = Size(1200, 800);
const Size kCompact = Size(420, 900);

Future<Harness> pumpVault(
  WidgetTester tester, {
  SpyApi? api,
  Size size = kDesktop,
  double textScale = 1,
  ThemeMode themeMode = ThemeMode.light,
  Services? services,
  List<Override> overrides = const <Override>[],
  bool settle = true,
}) async => pumpApp(
  tester,
  api: api ?? await unlockedVault(),
  size: size,
  textScale: textScale,
  themeMode: themeMode,
  services: services,
  overrides: overrides,
  settle: settle,
);

/// A list row by its title (inside the list pane only).
Finder listRow(String title) =>
    find.descendant(of: find.byType(ItemListPane), matching: find.text(title));

Future<void> openRow(WidgetTester tester, String title) async {
  await tapOn(tester, listRow(title));
}

String? selectedId(Harness h) =>
    h.container.read(vaultQueryProvider).selectedId;

/// Finds an item through the API (the first one for which [test] is true).
Future<ItemView> findItem(
  SpyApi api,
  bool Function(ItemView v) test, {
  ListFilter filter = const ListFilter(),
}) async {
  for (var offset = 0; ; offset += 100) {
    final List<ItemSummary> page = await api.list(
      filter,
      Page(offset: offset, limit: 100),
    );
    if (page.isEmpty) break;
    for (final ItemSummary s in page) {
      final ItemView v = await api.getItem(s.id);
      if (test(v)) return v;
    }
  }
  throw StateError('no matching item');
}

Future<void> typeSearch(WidgetTester tester, String text) async {
  await tester.enterText(fieldLabelled(l10n.searchLabel), text);
}

/// Pumps [d] without `pumpAndSettle`, in small steps.
Future<void> pumpFor(WidgetTester tester, Duration d) async {
  await tester.pump(d);
  await tester.pump();
}

/// A login with every kind of content, for detail/edit/golden tests.
Future<String> seedRichLogin(SpyApi api, {String title = 'CANARY Bank'}) async {
  final String folder = await api.createFolder('Banking', null);
  return api.createItem(
    NewItem(
      type: ItemType.login,
      title: title,
      fields: const <StdField, String>{
        StdField.username: 'alice@example.invalid',
        StdField.password: 'CANARY-hunter2-do-not-use',
        StdField.totpSeed: 'CANARY-seed',
        StdField.notes: 'Main account',
      },
      urls: const <String>[
        'https://bank.invalid/login',
        'http://old.bank.invalid/',
      ],
      tags: const <String>['money', 'bills'],
      folderId: folder,
      custom: const <NewCustom>[
        NewCustom(kind: CustomKind.text, label: 'Branch', value: 'North'),
        NewCustom(
          kind: CustomKind.hidden,
          label: 'Security answer',
          value: 'CANARY-blue',
        ),
      ],
    ),
  );
}
