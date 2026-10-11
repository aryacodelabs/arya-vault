import 'dart:math' as math;

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:go_router/go_router.dart';

import '../../api/app_error.dart';
import '../../api/dtos.dart';
import '../../state/item_actions.dart';
import '../../state/item_list.dart';
import '../../state/routing.dart';
import '../../state/vault_query.dart';
import '../../theme/app_theme.dart';
import '../widgets/error_text.dart';
import '../widgets/l10n_ext.dart';
import 'dialogs.dart';
import 'vault_format.dart';

/// Rows built since the last reset; the performance test uses it to prove the
/// list is virtualized and not rebuilt as a whole when pages are appended.
@visibleForTesting
int debugRowBuilds = 0;

/// Search, type filters and the (virtualized, paged) item list, or the trash.
class ItemListPane extends ConsumerStatefulWidget {
  const ItemListPane({super.key});

  @override
  ConsumerState<ItemListPane> createState() => _ItemListPaneState();
}

class _ItemListPaneState extends ConsumerState<ItemListPane> {
  final TextEditingController _search = TextEditingController();
  final ScrollController _scroll = ScrollController();
  final FocusNode _listFocus = FocusNode(debugLabel: 'item list');
  double _extent = 76;

  @override
  void initState() {
    super.initState();
    _scroll.addListener(_maybeLoadMore);
  }

  @override
  void dispose() {
    _search.dispose();
    _scroll.dispose();
    _listFocus.dispose();
    super.dispose();
  }

  void _maybeLoadMore() {
    if (!_scroll.hasClients) return;
    final ScrollPosition p = _scroll.position;
    if (p.pixels >= p.maxScrollExtent - 4 * _extent) {
      ref.read(itemListProvider.notifier).loadMore();
    }
  }

  void _select(List<ItemSummary> items, int index) {
    if (index < 0 || index >= items.length) return;
    ref.read(vaultQueryProvider.notifier).select(items[index].id);
    if (_scroll.hasClients) {
      final double top = index * _extent;
      final double bottom = top + _extent;
      final ScrollPosition p = _scroll.position;
      if (top < p.pixels) {
        _scroll.jumpTo(top);
      } else if (bottom > p.pixels + p.viewportDimension) {
        _scroll.jumpTo(
          math.min(bottom - p.viewportDimension, p.maxScrollExtent),
        );
      }
    }
    if (index >= items.length - 4) _maybeLoadMore();
  }

  Future<void> _trash(ItemSummary item) async {
    final l10n = context.l10n;
    final ScaffoldMessengerState messenger = ScaffoldMessenger.of(context);
    final ItemActions actions = ref.read(itemActionsProvider);
    try {
      await actions.trash(item.id);
    } on AppError catch (e) {
      messenger.showSnackBar(SnackBar(content: Text(errorMessage(l10n, e))));
      return;
    }
    if (ref.read(vaultQueryProvider).selectedId == item.id) {
      ref.read(vaultQueryProvider.notifier).select(null);
    }
    messenger
      ..hideCurrentSnackBar()
      ..showSnackBar(
        SnackBar(
          content: Text(l10n.movedToTrash(item.title)),
          action: SnackBarAction(
            label: l10n.undo,
            onPressed: () => actions.restore(item.id),
          ),
        ),
      );
  }

  KeyEventResult _onKey(
    FocusNode node,
    KeyEvent event,
    List<ItemSummary> items,
  ) {
    if (event is! KeyDownEvent && event is! KeyRepeatEvent) {
      return KeyEventResult.ignored;
    }
    final String? selectedId = ref.read(vaultQueryProvider).selectedId;
    final int current = items.indexWhereId(selectedId);
    if (event.logicalKey == LogicalKeyboardKey.arrowDown) {
      _select(items, current < 0 ? 0 : math.min(current + 1, items.length - 1));
      return KeyEventResult.handled;
    }
    if (event.logicalKey == LogicalKeyboardKey.arrowUp) {
      _select(items, current <= 0 ? 0 : current - 1);
      return KeyEventResult.handled;
    }
    if (event.logicalKey == LogicalKeyboardKey.enter ||
        event.logicalKey == LogicalKeyboardKey.numpadEnter) {
      if (current < 0) _select(items, 0);
      return KeyEventResult.handled;
    }
    if (event.logicalKey == LogicalKeyboardKey.delete && current >= 0) {
      _trash(items[current]);
      return KeyEventResult.handled;
    }
    return KeyEventResult.ignored;
  }

  @override
  Widget build(BuildContext context) {
    final l10n = context.l10n;
    final AppTokens tokens = context.tokens;
    final VaultQueryState q = ref.watch(vaultQueryProvider);
    final bool trash = q.view.kind == VaultViewKind.trash;
    final double scale = MediaQuery.textScalerOf(context).scale(1);
    _extent = math.max(76, 24 + 62 * scale);
    if (_search.text != q.searchInput) {
      _search.value = TextEditingValue(
        text: q.searchInput,
        selection: TextSelection.collapsed(offset: q.searchInput.length),
      );
    }
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: <Widget>[
        if (!trash) ...<Widget>[
          Padding(
            padding: EdgeInsets.fromLTRB(tokens.gap, tokens.gap, tokens.gap, 0),
            child: Row(
              children: <Widget>[
                Expanded(
                  child: TextField(
                    controller: _search,
                    textInputAction: TextInputAction.search,
                    decoration: InputDecoration(
                      labelText: l10n.searchLabel,
                      prefixIcon: const Icon(Icons.search),
                      suffixIcon: q.searchInput.isEmpty
                          ? null
                          : IconButton(
                              tooltip: l10n.searchClear,
                              icon: const Icon(Icons.close),
                              onPressed: () => ref
                                  .read(vaultQueryProvider.notifier)
                                  .clearSearch(),
                            ),
                    ),
                    onChanged: ref
                        .read(vaultQueryProvider.notifier)
                        .setSearchInput,
                  ),
                ),
                const SizedBox(width: 8),
                PopupMenuButton<ItemType>(
                  tooltip: l10n.newItem,
                  icon: const Icon(Icons.add),
                  onSelected: (ItemType t) =>
                      context.push('${AppRoutes.newItem}/${t.name}'),
                  itemBuilder: (BuildContext context) =>
                      <PopupMenuEntry<ItemType>>[
                        for (final ItemType t in ItemType.values)
                          PopupMenuItem<ItemType>(
                            value: t,
                            child: Row(
                              children: <Widget>[
                                Icon(itemTypeIcon(t)),
                                const SizedBox(width: 12),
                                Text(itemTypeLabel(l10n, t)),
                              ],
                            ),
                          ),
                      ],
                ),
              ],
            ),
          ),
          Padding(
            padding: EdgeInsets.symmetric(
              horizontal: tokens.gap,
              vertical: tokens.gap / 2,
            ),
            child: SingleChildScrollView(
              scrollDirection: Axis.horizontal,
              child: Row(
                children: <Widget>[
                  for (final ItemType t in ItemType.values)
                    Padding(
                      padding: const EdgeInsetsDirectional.only(end: 8),
                      child: FilterChip(
                        label: Text(itemTypeLabel(l10n, t)),
                        avatar: Icon(itemTypeIcon(t), size: 18),
                        selected: q.types.contains(t),
                        onSelected: (_) =>
                            ref.read(vaultQueryProvider.notifier).toggleType(t),
                      ),
                    ),
                ],
              ),
            ),
          ),
        ],
        Expanded(child: trash ? const _TrashList() : _buildItems(context, q)),
      ],
    );
  }

  Widget _buildItems(BuildContext context, VaultQueryState q) {
    final l10n = context.l10n;
    final AppTokens tokens = context.tokens;
    final AsyncValue<ItemListState> async = ref.watch(itemListProvider);
    final ItemListState? data = async.value;
    if (data == null) {
      if (async.hasError) {
        final Object? e = async.error;
        return SingleChildScrollView(
          padding: EdgeInsets.all(tokens.gap),
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: <Widget>[
              ErrorText(
                e is AppError ? errorMessage(l10n, e) : l10n.errInternal,
              ),
              TextButton(
                onPressed: () => ref.invalidate(itemListProvider),
                child: Text(l10n.actionTryAgain),
              ),
            ],
          ),
        );
      }
      return Center(
        child: Semantics(
          liveRegion: true,
          label: l10n.loadingItems,
          child: const CircularProgressIndicator(),
        ),
      );
    }
    final bool filtered =
        q.search.isNotEmpty ||
        q.types.isNotEmpty ||
        q.view.kind != VaultViewKind.all;
    if (data.items.isEmpty) {
      // Scrollable so it survives large text in a short pane.
      return LayoutBuilder(
        builder: (BuildContext context, BoxConstraints c) =>
            SingleChildScrollView(
              child: ConstrainedBox(
                constraints: BoxConstraints(minHeight: c.maxHeight),
                child: Center(
                  child: Padding(
                    padding: EdgeInsets.all(tokens.gap),
                    child: filtered
                        ? Text(l10n.noMatches, textAlign: TextAlign.center)
                        : Column(
                            mainAxisSize: MainAxisSize.min,
                            children: <Widget>[
                              Semantics(
                                liveRegion: true,
                                child: Text(
                                  l10n.homeUnlockedMessage,
                                  style: Theme.of(context)
                                      .textTheme
                                      .titleMedium,
                                ),
                              ),
                              SizedBox(height: tokens.gap / 2),
                              Text(
                                l10n.emptyVaultHint,
                                textAlign: TextAlign.center,
                              ),
                            ],
                          ),
                  ),
                ),
              ),
            ),
      );
    }
    final int extra =
        (data.hasMore || data.loadingMore || data.loadMoreError != null)
        ? 1
        : 0;
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: <Widget>[
        Padding(
          padding: EdgeInsets.symmetric(horizontal: tokens.gap),
          child: Semantics(
            liveRegion: true,
            child: Text(
              data.hasMore
                  ? l10n.resultsCountMore(data.items.length)
                  : l10n.resultsCount(data.items.length),
              style: Theme.of(context).textTheme.bodySmall,
            ),
          ),
        ),
        Expanded(
          child: Focus(
            focusNode: _listFocus,
            onKeyEvent: (FocusNode n, KeyEvent e) => _onKey(n, e, data.items),
            child: ListenableBuilder(
              listenable: _listFocus,
              builder: (BuildContext context, _) => DecoratedBox(
                position: DecorationPosition.foreground,
                decoration: BoxDecoration(
                  border: _listFocus.hasFocus
                      ? Border.all(
                          color: Theme.of(context).colorScheme.primary,
                          width: 2,
                        )
                      : null,
                ),
                child: Semantics(
                  label: l10n.itemListLabel,
                  child: ListView.builder(
                    controller: _scroll,
                    itemExtent: _extent,
                    itemCount: data.items.length + extra,
                    itemBuilder: (BuildContext context, int index) {
                      if (index >= data.items.length) {
                        return _ListFooter(state: data);
                      }
                      final ItemSummary item = data.items[index];
                      return _ItemRow(
                        key: ValueKey<String>(item.id),
                        item: item,
                        selected: item.id == q.selectedId,
                        onTap: () {
                          _listFocus.requestFocus();
                          ref.read(vaultQueryProvider.notifier).select(item.id);
                        },
                      );
                    },
                  ),
                ),
              ),
            ),
          ),
        ),
      ],
    );
  }
}

extension on List<ItemSummary> {
  int indexWhereId(String? id) =>
      id == null ? -1 : indexWhere((ItemSummary i) => i.id == id);
}

class _ListFooter extends ConsumerWidget {
  const _ListFooter({required this.state});
  final ItemListState state;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final l10n = context.l10n;
    if (state.loadMoreError != null) {
      return Padding(
        padding: const EdgeInsets.all(8),
        child: Row(
          children: <Widget>[
            Expanded(
              child: ErrorText(errorMessage(l10n, state.loadMoreError!)),
            ),
            TextButton(
              onPressed: () => ref.read(itemListProvider.notifier).loadMore(),
              child: Text(l10n.actionTryAgain),
            ),
          ],
        ),
      );
    }
    return Center(
      child: Semantics(
        liveRegion: true,
        label: l10n.loadingItems,
        child: const SizedBox(
          width: 24,
          height: 24,
          child: CircularProgressIndicator(strokeWidth: 2),
        ),
      ),
    );
  }
}

class _ItemRow extends ConsumerWidget {
  const _ItemRow({
    required this.item,
    required this.selected,
    required this.onTap,
    super.key,
  });

  final ItemSummary item;
  final bool selected;
  final VoidCallback onTap;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    debugRowBuilds++;
    final l10n = context.l10n;
    final ThemeData theme = Theme.of(context);
    final ColorScheme scheme = theme.colorScheme;
    final String type = itemTypeLabel(l10n, item.type);
    return Material(
      color: selected ? scheme.primaryContainer : Colors.transparent,
      child: Row(
        children: <Widget>[
          Expanded(
            child: Semantics(
              button: true,
              selected: selected,
              container: true,
              label: <String>[
                type,
                item.title,
                if (item.subtitle.isNotEmpty) item.subtitle,
                if (item.tags.isNotEmpty) item.tags.join(', '),
              ].join(', '),
              onTap: onTap,
              child: ExcludeSemantics(
                child: InkWell(
                  canRequestFocus: false,
                  onTap: onTap,
                  child: Padding(
                    padding: const EdgeInsets.symmetric(
                      horizontal: 16,
                      vertical: 8,
                    ),
                    child: Row(
                      children: <Widget>[
                        Icon(
                          itemTypeIcon(item.type),
                          color: selected
                              ? scheme.onPrimaryContainer
                              : scheme.onSurfaceVariant,
                        ),
                        const SizedBox(width: 16),
                        Expanded(
                          child: Column(
                            mainAxisAlignment: MainAxisAlignment.center,
                            crossAxisAlignment: CrossAxisAlignment.start,
                            children: <Widget>[
                              Text(
                                item.title,
                                maxLines: 1,
                                overflow: TextOverflow.ellipsis,
                                style: theme.textTheme.bodyLarge?.copyWith(
                                  color: selected
                                      ? scheme.onPrimaryContainer
                                      : scheme.onSurface,
                                ),
                              ),
                              Text(
                                item.subtitle.isEmpty ? type : item.subtitle,
                                maxLines: 1,
                                overflow: TextOverflow.ellipsis,
                                style: theme.textTheme.bodyMedium?.copyWith(
                                  color: selected
                                      ? scheme.onPrimaryContainer
                                      : scheme.onSurfaceVariant,
                                ),
                              ),
                              if (item.tags.isNotEmpty)
                                _TagPills(tags: item.tags, selected: selected),
                            ],
                          ),
                        ),
                      ],
                    ),
                  ),
                ),
              ),
            ),
          ),
          IconButton(
            tooltip: item.favorite ? l10n.unfavorite : l10n.favorite,
            icon: Icon(item.favorite ? Icons.star : Icons.star_border),
            color: selected
                ? scheme.onPrimaryContainer
                : scheme.onSurfaceVariant,
            onPressed: () =>
                ref.read(itemActionsProvider).toggleFavorite(item.id),
          ),
        ],
      ),
    );
  }
}

class _TagPills extends StatelessWidget {
  const _TagPills({required this.tags, required this.selected});
  final List<String> tags;
  final bool selected;

  @override
  Widget build(BuildContext context) {
    final ColorScheme scheme = Theme.of(context).colorScheme;
    return Row(
      children: <Widget>[
        for (final String t in tags.take(3))
          Container(
            margin: const EdgeInsetsDirectional.only(end: 4, top: 2),
            padding: const EdgeInsets.symmetric(horizontal: 6),
            decoration: BoxDecoration(
              border: Border.all(color: scheme.outline),
              borderRadius: BorderRadius.circular(8),
            ),
            child: Text(
              t,
              maxLines: 1,
              style: Theme.of(context).textTheme.labelSmall?.copyWith(
                color: selected
                    ? scheme.onPrimaryContainer
                    : scheme.onSurfaceVariant,
              ),
            ),
          ),
      ],
    );
  }
}

class _TrashList extends ConsumerWidget {
  const _TrashList();

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final l10n = context.l10n;
    final AppTokens tokens = context.tokens;
    final AsyncValue<List<TrashEntry>> async = ref.watch(trashProvider);
    final List<TrashEntry>? entries = async.value;
    if (entries == null) {
      if (async.hasError) {
        final Object? e = async.error;
        return Padding(
          padding: EdgeInsets.all(tokens.gap),
          child: ErrorText(
            e is AppError ? errorMessage(l10n, e) : l10n.errInternal,
          ),
        );
      }
      return const Center(child: CircularProgressIndicator());
    }
    final ItemActions actions = ref.read(itemActionsProvider);
    Future<void> run(Future<void> Function() op) async {
      final ScaffoldMessengerState m = ScaffoldMessenger.of(context);
      try {
        await op();
      } on AppError catch (e) {
        m.showSnackBar(SnackBar(content: Text(errorMessage(l10n, e))));
      }
    }

    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: <Widget>[
        Padding(
          padding: EdgeInsets.all(tokens.gap),
          child: Row(
            children: <Widget>[
              Expanded(
                child: Semantics(
                  header: true,
                  child: Text(
                    l10n.trashTitle,
                    style: Theme.of(context).textTheme.titleMedium,
                  ),
                ),
              ),
              TextButton.icon(
                onPressed: entries.isEmpty
                    ? null
                    : () async {
                        final bool ok = await confirmDialog(
                          context,
                          title: l10n.emptyTrashTitle,
                          body: l10n.emptyTrashBody(entries.length),
                          confirmLabel: l10n.emptyTrash,
                          destructive: true,
                        );
                        if (ok) await run(actions.emptyTrash);
                      },
                icon: const Icon(Icons.delete_forever),
                label: Text(l10n.emptyTrash),
              ),
            ],
          ),
        ),
        Padding(
          padding: EdgeInsets.symmetric(horizontal: tokens.gap),
          child: Text(
            l10n.trashRetention,
            style: Theme.of(context).textTheme.bodySmall,
          ),
        ),
        Expanded(
          child: entries.isEmpty
              ? Center(child: Text(l10n.trashEmpty))
              : ListView.builder(
                  itemCount: entries.length,
                  itemBuilder: (BuildContext context, int index) {
                    final TrashEntry e = entries[index];
                    return ListTile(
                      leading: Icon(itemTypeIcon(e.summary.type)),
                      title: Text(
                        e.summary.title,
                        maxLines: 1,
                        overflow: TextOverflow.ellipsis,
                      ),
                      subtitle: Text(
                        l10n.purgesOn(formatDate(context, e.purgesAt)),
                      ),
                      isThreeLine: false,
                      trailing: Row(
                        mainAxisSize: MainAxisSize.min,
                        children: <Widget>[
                          IconButton(
                            tooltip: l10n.restore,
                            icon: const Icon(Icons.restore_from_trash),
                            onPressed: () =>
                                run(() => actions.restore(e.summary.id)),
                          ),
                          IconButton(
                            tooltip: l10n.deleteForever,
                            icon: const Icon(Icons.delete_forever),
                            onPressed: () async {
                              final bool ok = await confirmDialog(
                                context,
                                title: l10n.purgeTitle,
                                body: l10n.purgeBody(e.summary.title),
                                confirmLabel: l10n.deleteForever,
                                destructive: true,
                              );
                              if (ok) {
                                await run(() => actions.purge(e.summary.id));
                              }
                            },
                          ),
                        ],
                      ),
                    );
                  },
                ),
        ),
      ],
    );
  }
}
