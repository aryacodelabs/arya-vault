import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../api/app_error.dart';
import '../../api/dtos.dart';
import '../../state/item_actions.dart';
import '../../state/item_list.dart';
import '../../state/vault_query.dart';
import '../../theme/app_theme.dart';
import '../widgets/error_text.dart';
import '../widgets/l10n_ext.dart';
import 'dialogs.dart';

/// Create, rename and delete folders (B02 item 8). Deleting a folder moves its
/// items to "No folder" and says so before it happens.
class FoldersScreen extends ConsumerWidget {
  const FoldersScreen({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final l10n = context.l10n;
    final AppTokens tokens = context.tokens;
    final AsyncValue<List<Folder>> async = ref.watch(foldersProvider);
    final ItemActions actions = ref.read(itemActionsProvider);
    Future<void> run(Future<void> Function() op) async {
      final ScaffoldMessengerState m = ScaffoldMessenger.of(context);
      try {
        await op();
      } on AppError catch (e) {
        m.showSnackBar(SnackBar(content: Text(errorMessage(l10n, e))));
      }
    }

    return Scaffold(
      appBar: AppBar(
        title: Semantics(header: true, child: Text(l10n.foldersTitle)),
      ),
      floatingActionButton: FloatingActionButton.extended(
        onPressed: () async {
          final String? name = await promptDialog(
            context,
            title: l10n.newFolder,
            label: l10n.folderNameLabel,
            confirmLabel: l10n.create,
          );
          if (name != null && name.isNotEmpty) {
            await run(() => actions.createFolder(name));
          }
        },
        icon: const Icon(Icons.create_new_folder),
        label: Text(l10n.newFolder),
      ),
      body: SafeArea(
        child: Center(
          child: ConstrainedBox(
            constraints: const BoxConstraints(maxWidth: 720),
            child: async.value == null
                ? (async.hasError
                      ? Padding(
                          padding: EdgeInsets.all(tokens.gap),
                          child: ErrorText(l10n.errInternal),
                        )
                      : const Center(child: CircularProgressIndicator()))
                : async.value!.isEmpty
                ? Center(child: Text(l10n.noFolders))
                : ListView(
                    padding: EdgeInsets.only(bottom: 88, top: tokens.gap / 2),
                    children: <Widget>[
                      for (final Folder f in async.value!)
                        ListTile(
                          leading: const Icon(Icons.folder_outlined),
                          title: Text(f.name),
                          trailing: Row(
                            mainAxisSize: MainAxisSize.min,
                            children: <Widget>[
                              IconButton(
                                tooltip: l10n.renameFolder(f.name),
                                icon: const Icon(Icons.edit),
                                onPressed: () async {
                                  final String? name = await promptDialog(
                                    context,
                                    title: l10n.renameFolderTitle,
                                    label: l10n.folderNameLabel,
                                    confirmLabel: l10n.save,
                                    initial: f.name,
                                  );
                                  if (name != null &&
                                      name.isNotEmpty &&
                                      name != f.name) {
                                    await run(
                                      () => actions.renameFolder(f.id, name),
                                    );
                                  }
                                },
                              ),
                              IconButton(
                                tooltip: l10n.deleteFolder(f.name),
                                icon: const Icon(Icons.delete_outline),
                                onPressed: () async {
                                  final bool ok = await confirmDialog(
                                    context,
                                    title: l10n.deleteFolderTitle(f.name),
                                    body: l10n.deleteFolderBody,
                                    confirmLabel: l10n.deleteFolderConfirm,
                                    destructive: true,
                                  );
                                  if (!ok) return;
                                  await run(() => actions.deleteFolder(f.id));
                                  final VaultQueryNotifier q = ref.read(
                                    vaultQueryProvider.notifier,
                                  );
                                  if (ref.read(vaultQueryProvider).view ==
                                      VaultView.folder(f.id)) {
                                    q.setView(VaultView.all);
                                  }
                                },
                              ),
                            ],
                          ),
                        ),
                    ],
                  ),
          ),
        ),
      ),
    );
  }
}
