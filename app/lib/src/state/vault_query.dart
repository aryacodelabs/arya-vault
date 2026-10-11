import 'dart:async';

import 'package:flutter/foundation.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../api/dtos.dart';
import 'vault_session.dart';

/// Search is debounced by this long (B02 item 1).
const Duration kSearchDebounce = Duration(milliseconds: 150);

enum VaultViewKind { all, favorites, folder, tag, trash }

/// Which slice of the vault the list shows (the sidebar selection).
@immutable
class VaultView {
  const VaultView(this.kind, {this.id});

  static const VaultView all = VaultView(VaultViewKind.all);
  static const VaultView favorites = VaultView(VaultViewKind.favorites);
  static const VaultView trash = VaultView(VaultViewKind.trash);
  const VaultView.folder(String folderId)
    : this(VaultViewKind.folder, id: folderId);
  const VaultView.tag(String tag) : this(VaultViewKind.tag, id: tag);

  final VaultViewKind kind;

  /// The folder id or the tag.
  final String? id;

  @override
  bool operator ==(Object other) =>
      other is VaultView && other.kind == kind && other.id == id;

  @override
  int get hashCode => Object.hash(kind, id);
}

/// What the list is showing: view + effective search text + type filter.
@immutable
class ListQuery {
  ListQuery(this.view, this.search, Set<ItemType> types)
    : types = (types.toList()
        ..sort((ItemType a, ItemType b) => a.index - b.index));

  final VaultView view;
  final String search;
  final List<ItemType> types;

  ListFilter toFilter() => ListFilter(
    types: types.isEmpty ? null : types,
    folderId: view.kind == VaultViewKind.folder ? view.id : null,
    tag: view.kind == VaultViewKind.tag ? view.id : null,
    favoritesOnly: view.kind == VaultViewKind.favorites,
  );

  @override
  bool operator ==(Object other) =>
      other is ListQuery &&
      other.view == view &&
      other.search == search &&
      listEquals(other.types, types);

  @override
  int get hashCode => Object.hash(view, search, Object.hashAll(types));
}

@immutable
class VaultQueryState {
  const VaultQueryState({
    this.view = VaultView.all,
    this.searchInput = '',
    this.search = '',
    this.types = const <ItemType>{},
    this.selectedId,
  });

  final VaultView view;

  /// What is in the search box right now.
  final String searchInput;

  /// The debounced text the list actually uses.
  final String search;
  final Set<ItemType> types;
  final String? selectedId;

  VaultQueryState copyWith({
    VaultView? view,
    String? searchInput,
    String? search,
    Set<ItemType>? types,
    Object? selectedId = _keep,
  }) => VaultQueryState(
    view: view ?? this.view,
    searchInput: searchInput ?? this.searchInput,
    search: search ?? this.search,
    types: types ?? this.types,
    selectedId: identical(selectedId, _keep)
        ? this.selectedId
        : selectedId as String?,
  );
}

const Object _keep = Object();

class VaultQueryNotifier extends Notifier<VaultQueryState> {
  Timer? _debounce;

  @override
  VaultQueryState build() {
    ref.onDispose(() => _debounce?.cancel());
    // A lock drops the whole selection, search and filters.
    ref.listen(lockEpochProvider, (int? _, int _) {
      _debounce?.cancel();
      state = const VaultQueryState();
    });
    return const VaultQueryState();
  }

  void setView(VaultView view) {
    state = state.copyWith(view: view, selectedId: null);
  }

  /// Called on every keystroke; the list sees the text after [kSearchDebounce].
  void setSearchInput(String text) {
    state = state.copyWith(searchInput: text);
    _debounce?.cancel();
    _debounce = Timer(kSearchDebounce, () {
      state = state.copyWith(search: text.trim(), selectedId: null);
    });
  }

  void clearSearch() {
    _debounce?.cancel();
    state = state.copyWith(searchInput: '', search: '');
  }

  void toggleType(ItemType type) {
    final Set<ItemType> next = <ItemType>{...state.types};
    if (!next.remove(type)) {
      next.add(type);
    }
    state = state.copyWith(types: next);
  }

  void select(String? id) => state = state.copyWith(selectedId: id);
}

final NotifierProvider<VaultQueryNotifier, VaultQueryState> vaultQueryProvider =
    NotifierProvider<VaultQueryNotifier, VaultQueryState>(
      VaultQueryNotifier.new,
    );

/// Only the parts that change what the list loads.
final Provider<ListQuery> listQueryProvider = Provider<ListQuery>((Ref ref) {
  final VaultQueryState q = ref.watch(vaultQueryProvider);
  return ListQuery(q.view, q.search, q.types);
});
