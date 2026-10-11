import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:go_router/go_router.dart';

import '../../api/dtos.dart';
import '../../state/item_list.dart';
import '../../state/routing.dart';
import '../../state/vault_query.dart';
import '../widgets/l10n_ext.dart';

/// All items, Favorites, Folders, Tags and Trash (docs/07 §2).
class VaultSidebar extends ConsumerWidget {
  const VaultSidebar({this.onNavigate, super.key});

  /// Called after a selection (the compact layout closes its drawer).
  final VoidCallback? onNavigate;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final l10n = context.l10n;
    final ThemeData theme = Theme.of(context);
    final VaultView current = ref.watch(
      vaultQueryProvider.select((VaultQueryState s) => s.view),
    );
    final List<Folder> folders =
        ref.watch(foldersProvider).value ?? const <Folder>[];
    final List<String> tags =
        (ref.watch(itemListProvider).value?.knownTags.toList() ?? <String>[])
          ..sort();
    void go(VaultView v) {
      ref.read(vaultQueryProvider.notifier).setView(v);
      onNavigate?.call();
    }

    Widget tile(IconData icon, String label, VaultView v) => ListTile(
      leading: Icon(icon),
      title: Text(label, maxLines: 1, overflow: TextOverflow.ellipsis),
      selected: current == v,
      selectedTileColor: theme.colorScheme.primaryContainer,
      selectedColor: theme.colorScheme.onPrimaryContainer,
      onTap: () => go(v),
    );

    Widget heading(String text) => Padding(
      padding: const EdgeInsetsDirectional.fromSTEB(16, 16, 16, 4),
      child: Semantics(
        header: true,
        child: Text(text, style: theme.textTheme.titleSmall),
      ),
    );

    return Semantics(
      container: true,
      label: l10n.sidebarLabel,
      child: ListView(
        children: <Widget>[
          tile(Icons.all_inbox, l10n.allItems, VaultView.all),
          tile(Icons.star_outline, l10n.favoritesView, VaultView.favorites),
          heading(l10n.foldersTitle),
          for (final Folder f in folders)
            tile(Icons.folder_outlined, f.name, VaultView.folder(f.id)),
          ListTile(
            leading: const Icon(Icons.settings_outlined),
            title: Text(l10n.manageFolders),
            onTap: () {
              onNavigate?.call();
              context.push(AppRoutes.folders);
            },
          ),
          heading(l10n.tagsLabel),
          if (tags.isEmpty)
            Padding(
              padding: const EdgeInsetsDirectional.fromSTEB(16, 0, 16, 8),
              child: Text(l10n.noTagsYet, style: theme.textTheme.bodySmall),
            ),
          for (final String t in tags)
            tile(Icons.sell_outlined, t, VaultView.tag(t)),
          const Divider(),
          tile(Icons.delete_outline, l10n.trashTitle, VaultView.trash),
        ],
      ),
    );
  }
}
