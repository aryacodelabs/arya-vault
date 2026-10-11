import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../api/app_error.dart';
import '../api/dtos.dart';
import '../api/vault_api.dart';
import 'item_list.dart';
import 'providers.dart';
import 'vault_session.dart';

/// Mutations the vault screens perform. Each keeps the cached lists and details
/// in step, and a `locked` error from any call re-reads the status so the
/// router goes to the lock screen (B02 item 9). Errors are rethrown as
/// `AppError` for the screen to show.
class ItemActions {
  ItemActions(this._ref);
  final Ref _ref;

  VaultApi get _api => _ref.read(vaultApiProvider);

  Future<T> _run<T>(Future<T> Function() op) async {
    try {
      return await op();
    } on AppError catch (e) {
      handleLocked(_ref, e);
      rethrow;
    }
  }

  void changed({String? id, bool folders = false}) {
    _ref.invalidate(itemListProvider);
    _ref.invalidate(trashProvider);
    if (id != null) {
      _ref.invalidate(itemViewProvider(id));
    }
    if (folders) {
      _ref.invalidate(foldersProvider);
    }
  }

  Future<bool> toggleFavorite(String id) => _run(() async {
    final bool now = await _api.toggleFavorite(id);
    changed(id: id);
    return now;
  });

  /// Moves an item to the trash (the caller offers Undo).
  Future<void> trash(String id) => _run(() async {
    await _api.deleteItem(id);
    changed(id: id);
  });

  Future<void> restore(String id) => _run(() async {
    await _api.restoreItem(id);
    changed(id: id);
  });

  Future<void> purge(String id) => _run(() async {
    await _api.purgeItem(id);
    changed(id: id);
  });

  Future<int> emptyTrash() => _run(() async {
    final int n = await _api.emptyTrash();
    changed();
    return n;
  });

  Future<String> createFolder(String name) => _run(() async {
    final String id = await _api.createFolder(name, null);
    changed(folders: true);
    return id;
  });

  Future<void> renameFolder(String id, String name) => _run(() async {
    await _api.renameFolder(id, name);
    changed(folders: true);
  });

  /// Deleting a folder moves its items to "no folder" (docs/14 semantics).
  Future<void> deleteFolder(String id) => _run(() async {
    await _api.deleteFolder(id);
    changed(folders: true);
  });

  Future<void> restoreVersion(String id, StdField field, int hlcMs) =>
      _run(() async {
        await _api.restoreVersion(id, field, hlcMs);
        changed(id: id);
      });

  /// "Keep as separate note": a new note item holding [body] (docs/06 §5.3).
  Future<String> keepAsSeparateNote({
    required String title,
    required String body,
    String? folderId,
  }) => _run(() async {
    final String id = await _api.createItem(
      NewItem(
        type: ItemType.note,
        title: title,
        fields: <StdField, String>{StdField.body: body},
        folderId: folderId,
      ),
    );
    changed();
    return id;
  });
}

final Provider<ItemActions> itemActionsProvider = Provider<ItemActions>(
  ItemActions.new,
);
