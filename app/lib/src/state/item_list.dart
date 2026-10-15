import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../api/app_error.dart';
import '../api/dtos.dart';
import '../api/vault_api.dart';
import 'providers.dart';
import 'vault_query.dart';
import 'vault_session.dart';

/// Items per request (the contract allows up to 200).
const int kPageSize = 100;

class ItemListState {
  const ItemListState({
    required this.items,
    required this.hasMore,
    this.loadingMore = false,
    this.knownTags = const <String>{},
    this.loadMoreError,
  });

  final List<ItemSummary> items;
  final bool hasMore;
  final bool loadingMore;

  /// Every tag seen so far. `docs/14` has no "list tags" call, so the sidebar
  /// shows the tags of the items loaded in this session (a spec question).
  final Set<String> knownTags;
  final AppError? loadMoreError;

  ItemListState copyWith({
    List<ItemSummary>? items,
    bool? hasMore,
    bool? loadingMore,
    Set<String>? knownTags,
    Object? loadMoreError = _keep,
  }) => ItemListState(
    items: items ?? this.items,
    hasMore: hasMore ?? this.hasMore,
    loadingMore: loadingMore ?? this.loadingMore,
    knownTags: knownTags ?? this.knownTags,
    loadMoreError: identical(loadMoreError, _keep)
        ? this.loadMoreError
        : loadMoreError as AppError?,
  );
}

const Object _keep = Object();

/// The paged list for the current [ListQuery]. A query change rebuilds it, and
/// Riverpod discards the result of the superseded request, so an older, slower
/// answer can never overwrite a newer one (B02 item 1).
class ItemListNotifier extends AsyncNotifier<ItemListState> {
  Object _token = Object();
  Set<String> _known = <String>{};

  VaultApi get _api => ref.read(vaultApiProvider);

  @override
  Future<ItemListState> build() async {
    ref.watch(lockEpochProvider);
    final ListQuery query = ref.watch(listQueryProvider);
    _token = Object();
    if (sessionKnownLocked(ref)) {
      // Nothing to load, and nothing worth holding, while locked.
      return const ItemListState(items: <ItemSummary>[], hasMore: false);
    }
    if (query.view.kind == VaultViewKind.trash) {
      return ItemListState(
        items: const <ItemSummary>[],
        hasMore: false,
        knownTags: _known,
      );
    }
    try {
      final List<ItemSummary> page = await _fetch(query, const <ItemSummary>[]);
      return _withTags(
        ItemListState(items: page, hasMore: page.length == kPageSize),
        page,
      );
    } on AppError catch (e) {
      handleLocked(ref, e);
      rethrow;
    }
  }

  ItemListState _withTags(ItemListState s, List<ItemSummary> page) {
    _known = <String>{..._known, for (final ItemSummary i in page) ...i.tags};
    return s.copyWith(knownTags: _known);
  }

  /// The page after [loaded]. A plain list pages by cursor (cost independent of depth, no
  /// skipped or repeated rows when the list changes); search ranks results, so it pages by offset.
  Future<List<ItemSummary>> _fetch(ListQuery query, List<ItemSummary> loaded) {
    final ListFilter filter = query.toFilter();
    if (query.search.isNotEmpty) {
      return _api.search(
        SearchQuery(
          text: query.search,
          filter: filter,
          page: Page(offset: loaded.length, limit: kPageSize),
        ),
      );
    }
    return _api.list(
      filter,
      Page(
        offset: 0,
        limit: kPageSize,
        after: loaded.isEmpty ? null : loaded.last.cursor,
      ),
    );
  }

  /// Loads the next page; no-op while loading or at the end.
  Future<void> loadMore() async {
    final ItemListState? current = state.value;
    if (current == null || !current.hasMore || current.loadingMore) {
      return;
    }
    final Object token = _token;
    final ListQuery query = ref.read(listQueryProvider);
    state = AsyncData<ItemListState>(
      current.copyWith(loadingMore: true, loadMoreError: null),
    );
    try {
      final List<ItemSummary> page = await _fetch(query, current.items);
      if (token != _token) {
        return; // the query changed meanwhile; this page belongs to the old one
      }
      state = AsyncData<ItemListState>(
        _withTags(
          current.copyWith(
            items: <ItemSummary>[...current.items, ...page],
            hasMore: page.length == kPageSize,
            loadingMore: false,
          ),
          page,
        ),
      );
    } on AppError catch (e) {
      if (token != _token) return;
      handleLocked(ref, e);
      state = AsyncData<ItemListState>(
        current.copyWith(loadingMore: false, loadMoreError: e),
      );
    }
  }

  /// Reloads from the first page (after an edit, delete, restore).
  void reload() => ref.invalidateSelf();
}

final AsyncNotifierProvider<ItemListNotifier, ItemListState> itemListProvider =
    AsyncNotifierProvider<ItemListNotifier, ItemListState>(
      ItemListNotifier.new,
    );

/// The trash (docs/14 `listTrash`).
final FutureProvider<List<TrashEntry>> trashProvider =
    FutureProvider<List<TrashEntry>>((Ref ref) async {
      ref.watch(lockEpochProvider);
      if (sessionKnownLocked(ref)) return const <TrashEntry>[];
      try {
        return await ref.read(vaultApiProvider).listTrash();
      } on AppError catch (e) {
        handleLocked(ref, e);
        rethrow;
      }
    });

final FutureProvider<List<Folder>> foldersProvider =
    FutureProvider<List<Folder>>((Ref ref) async {
      ref.watch(lockEpochProvider);
      if (sessionKnownLocked(ref)) return const <Folder>[];
      try {
        return await ref.read(vaultApiProvider).listFolders();
      } on AppError catch (e) {
        handleLocked(ref, e);
        rethrow;
      }
    });

/// One item without its secrets.
final itemViewProvider = FutureProvider.family<ItemView, String>((
  Ref ref,
  String id,
) async {
  ref.watch(lockEpochProvider);
  try {
    return await ref.read(vaultApiProvider).getItem(id);
  } on AppError catch (e) {
    handleLocked(ref, e);
    rethrow;
  }
});
