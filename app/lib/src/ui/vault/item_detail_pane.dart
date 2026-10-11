import 'dart:async';
import 'dart:convert';
import 'dart:typed_data';

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:go_router/go_router.dart';

import '../../api/app_error.dart';
import '../../api/dtos.dart';
import '../../api/vault_api.dart';
import '../../state/item_actions.dart';
import '../../state/item_list.dart';
import '../../state/providers.dart';
import '../../state/reveal.dart';
import '../../state/routing.dart';
import '../../state/vault_query.dart';
import '../../state/vault_session.dart';
import '../../theme/app_theme.dart';
import '../widgets/error_text.dart';
import '../widgets/l10n_ext.dart';
import 'copy_helper.dart';
import 'link_opener.dart';
import 'markdown_view.dart';
import 'vault_format.dart';

const String _mask = '••••••••';

/// One item: secrets masked until an explicit Reveal (auto-hides), Copy buttons,
/// URLs, custom fields, tags, folder and the "Other versions" badge.
class ItemDetailPane extends ConsumerStatefulWidget {
  const ItemDetailPane({required this.itemId, super.key});
  final String itemId;

  @override
  ConsumerState<ItemDetailPane> createState() => _ItemDetailPaneState();
}

class _ItemDetailPaneState extends ConsumerState<ItemDetailPane> {
  late final RevealNotifier _reveal;

  @override
  void initState() {
    super.initState();
    _reveal = ref.read(revealProvider.notifier);
  }

  @override
  void didUpdateWidget(ItemDetailPane old) {
    super.didUpdateWidget(old);
    if (old.itemId != widget.itemId) {
      _reveal.hideWithPrefix('${old.itemId}|');
    }
  }

  @override
  void dispose() {
    // Leaving the item hides what was revealed. Deferred: provider state must
    // not change while the tree is being torn down.
    final String prefix = '${widget.itemId}|';
    final RevealNotifier reveal = _reveal;
    Future<void>.microtask(() => reveal.hideWithPrefix(prefix));
    super.dispose();
  }

  Future<void> _openConflicts(ItemView view) async {
    final VaultApi api = ref.read(vaultApiProvider);
    final List<StdField> fields = <StdField>[
      ...view.fields.keys,
      ...view.secretFieldsPresent,
    ];
    StdField? target;
    try {
      for (final StdField f in fields) {
        final List<VersionInfo> h = await api.history(view.id, f);
        if (h.any((VersionInfo v) => v.concurrent)) {
          target = f;
          break;
        }
      }
    } on AppError catch (e) {
      handleLockedWidget(ref, e);
      return;
    }
    if (!mounted) return;
    unawaited(
      context.push(
        '${AppRoutes.history}/${view.id}/${(target ?? StdField.title).name}',
      ),
    );
  }

  Future<void> _trash(ItemView view) async {
    final l10n = context.l10n;
    final ScaffoldMessengerState messenger = ScaffoldMessenger.of(context);
    final ItemActions actions = ref.read(itemActionsProvider);
    try {
      await actions.trash(view.id);
    } on AppError catch (e) {
      messenger.showSnackBar(SnackBar(content: Text(errorMessage(l10n, e))));
      return;
    }
    ref.read(vaultQueryProvider.notifier).select(null);
    messenger
      ..hideCurrentSnackBar()
      ..showSnackBar(
        SnackBar(
          content: Text(l10n.movedToTrash(view.title)),
          action: SnackBarAction(
            label: l10n.undo,
            onPressed: () => actions.restore(view.id),
          ),
        ),
      );
  }

  @override
  Widget build(BuildContext context) {
    final l10n = context.l10n;
    final AppTokens tokens = context.tokens;
    final AsyncValue<ItemView> async = ref.watch(
      itemViewProvider(widget.itemId),
    );
    final ItemView? view = async.value;
    if (view == null) {
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
    final ThemeData theme = Theme.of(context);
    final List<Folder> folders =
        ref.watch(foldersProvider).value ?? const <Folder>[];
    final String? folderName = view.folderId == null
        ? null
        : folders
              .where((Folder f) => f.id == view.folderId)
              .map((Folder f) => f.name)
              .firstOrNull;
    final List<StdField> order = fieldsFor(view.type)
        .where((StdField f) => f != StdField.title && f != StdField.totpSeed)
        .toList();
    return SingleChildScrollView(
      padding: EdgeInsets.all(tokens.gap),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: <Widget>[
          Row(
            children: <Widget>[
              ExcludeSemantics(child: Icon(itemTypeIcon(view.type))),
              const SizedBox(width: 12),
              Expanded(
                child: Semantics(
                  header: true,
                  child: Text(view.title, style: theme.textTheme.headlineSmall),
                ),
              ),
            ],
          ),
          Wrap(
            children: <Widget>[
              IconButton(
                tooltip: view.favorite ? l10n.unfavorite : l10n.favorite,
                icon: Icon(view.favorite ? Icons.star : Icons.star_border),
                onPressed: () =>
                    ref.read(itemActionsProvider).toggleFavorite(view.id),
              ),
              IconButton(
                tooltip: l10n.edit,
                icon: const Icon(Icons.edit),
                onPressed: () => context.push('${AppRoutes.edit}/${view.id}'),
              ),
              IconButton(
                tooltip: l10n.moveToTrash,
                icon: const Icon(Icons.delete_outline),
                onPressed: () => _trash(view),
              ),
            ],
          ),
          if (view.otherVersions) ...<Widget>[
            SizedBox(height: tokens.gap / 2),
            Align(
              alignment: AlignmentDirectional.centerStart,
              child: ActionChip(
                avatar: const Icon(Icons.call_split, size: 18),
                label: Text(l10n.otherVersionsBadge),
                onPressed: () => _openConflicts(view),
              ),
            ),
          ],
          SizedBox(height: tokens.gap),
          for (final StdField f in order)
            if (view.secretFieldsPresent.contains(f))
              f == StdField.body
                  ? _NoteBody(view: view)
                  : _SecretRow(view: view, field: f)
            else if (view.fields[f] != null && view.fields[f]!.isNotEmpty)
              _ValueRow(
                label: stdFieldLabel(l10n, f),
                value: view.fields[f]!,
                itemId: view.id,
                field: f,
              ),
          if (view.secretFieldsPresent.contains(StdField.totpSeed))
            _TotpPlaceholder(label: stdFieldLabel(l10n, StdField.totpSeed)),
          for (final UrlView u in view.urls) _UrlRow(url: u.url),
          for (final CustomView c in view.custom)
            _CustomRow(view: view, field: c),
          SizedBox(height: tokens.gap),
          if (view.tags.isNotEmpty) ...<Widget>[
            Text(l10n.tagsLabel, style: theme.textTheme.labelLarge),
            Wrap(
              spacing: 8,
              children: <Widget>[
                for (final String t in view.tags)
                  ActionChip(
                    label: Text(t),
                    onPressed: () => ref
                        .read(vaultQueryProvider.notifier)
                        .setView(VaultView.tag(t)),
                  ),
              ],
            ),
            SizedBox(height: tokens.gap),
          ],
          Text(
            '${l10n.folderLabel}: ${folderName ?? l10n.noFolder}',
            style: theme.textTheme.bodyMedium,
          ),
          Text(
            l10n.createdOn(formatDateTime(context, view.createdAt)),
            style: theme.textTheme.bodySmall,
          ),
          Text(
            l10n.updatedOn(formatDateTime(context, view.updatedAt)),
            style: theme.textTheme.bodySmall,
          ),
        ],
      ),
    );
  }
}

/// A labelled block with trailing actions.
class _RowShell extends StatelessWidget {
  const _RowShell({
    required this.label,
    required this.content,
    required this.actions,
  });
  final String label;
  final Widget content;
  final List<Widget> actions;

  @override
  Widget build(BuildContext context) {
    final ThemeData theme = Theme.of(context);
    return Padding(
      padding: const EdgeInsets.only(bottom: 12),
      child: Row(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: <Widget>[
          Expanded(
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: <Widget>[
                Text(label, style: theme.textTheme.labelLarge),
                content,
              ],
            ),
          ),
          ...actions,
        ],
      ),
    );
  }
}

Widget _historyButton(BuildContext context, String itemId, StdField field) =>
    IconButton(
      tooltip: context.l10n.history,
      icon: const Icon(Icons.history),
      onPressed: () =>
          context.push('${AppRoutes.history}/$itemId/${field.name}'),
    );

class _ValueRow extends ConsumerWidget {
  const _ValueRow({
    required this.label,
    required this.value,
    required this.itemId,
    required this.field,
  });
  final String label;
  final String value;
  final String itemId;
  final StdField field;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final l10n = context.l10n;
    return _RowShell(
      label: label,
      content: Text(value),
      actions: <Widget>[
        IconButton(
          tooltip: l10n.copyField(label),
          icon: const Icon(Icons.copy),
          onPressed: () => copyToClipboard(context, ref, text: value),
        ),
        _historyButton(context, itemId, field),
      ],
    );
  }
}

String _decode(Uint8List? bytes) {
  if (bytes == null) return '';
  final String s = utf8.decode(bytes, allowMalformed: true);
  bytes.fillRange(0, bytes.length, 0);
  return s;
}

/// Reveal with the configured timeout; errors become a snackbar.
Future<void> revealValue(
  BuildContext context,
  WidgetRef ref,
  String key,
  Future<Uint8List?> Function() load,
) async {
  final l10n = context.l10n;
  final ScaffoldMessengerState messenger = ScaffoldMessenger.of(context);
  final Duration after = revealDuration(ref.read(appSettingsProvider).value);
  try {
    await ref.read(revealProvider.notifier).reveal(key, load, hideAfter: after);
  } on AppError catch (e) {
    handleLockedWidget(ref, e);
    messenger.showSnackBar(SnackBar(content: Text(errorMessage(l10n, e))));
  }
}

class _SecretRow extends ConsumerWidget {
  const _SecretRow({required this.view, required this.field});
  final ItemView view;
  final StdField field;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final l10n = context.l10n;
    final String label = stdFieldLabel(l10n, field);
    final String key = revealKey(view.id, field.name);
    final bool revealed = ref.watch(revealProvider).contains(key);
    final VaultApi api = ref.read(vaultApiProvider);
    final String text = revealed
        ? _decode(ref.read(revealProvider.notifier).copyOf(key))
        : _mask;
    return _RowShell(
      label: label,
      content: Semantics(
        label: revealed ? null : l10n.hiddenValue,
        child: ExcludeSemantics(
          excluding: !revealed,
          child: Text(text, style: const TextStyle(fontFamily: 'monospace')),
        ),
      ),
      actions: <Widget>[
        IconButton(
          tooltip: revealed ? l10n.hideField(label) : l10n.revealField(label),
          icon: Icon(revealed ? Icons.visibility_off : Icons.visibility),
          onPressed: () => revealed
              ? ref.read(revealProvider.notifier).hide(key)
              : revealValue(
                  context,
                  ref,
                  key,
                  () => api.reveal(view.id, field),
                ),
        ),
        IconButton(
          tooltip: l10n.copyField(label),
          icon: const Icon(Icons.copy),
          onPressed: () => copyToClipboard(
            context,
            ref,
            loadSecret: () => api.reveal(view.id, field),
          ),
        ),
        _historyButton(context, view.id, field),
      ],
    );
  }
}

class _NoteBody extends ConsumerWidget {
  const _NoteBody({required this.view});
  final ItemView view;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final l10n = context.l10n;
    final String label = stdFieldLabel(l10n, StdField.body);
    final String key = revealKey(view.id, StdField.body.name);
    final bool revealed = ref.watch(revealProvider).contains(key);
    final VaultApi api = ref.read(vaultApiProvider);
    return _RowShell(
      label: label,
      content: revealed
          ? MarkdownView(
              text: _decode(ref.read(revealProvider.notifier).copyOf(key)),
              onOpenLink: (String url) =>
                  openLink(context, ref, url, alwaysConfirm: true),
            )
          : Semantics(
              label: l10n.hiddenValue,
              child: const ExcludeSemantics(child: Text(_mask)),
            ),
      actions: <Widget>[
        IconButton(
          tooltip: revealed ? l10n.hideField(label) : l10n.revealField(label),
          icon: Icon(revealed ? Icons.visibility_off : Icons.visibility),
          onPressed: () => revealed
              ? ref.read(revealProvider.notifier).hide(key)
              : revealValue(
                  context,
                  ref,
                  key,
                  () => api.reveal(view.id, StdField.body),
                ),
        ),
        IconButton(
          tooltip: l10n.copyField(label),
          icon: const Icon(Icons.copy),
          onPressed: () => copyToClipboard(
            context,
            ref,
            loadSecret: () => api.reveal(view.id, StdField.body),
          ),
        ),
        _historyButton(context, view.id, StdField.body),
      ],
    );
  }
}

class _TotpPlaceholder extends StatelessWidget {
  const _TotpPlaceholder({required this.label});
  final String label;

  @override
  Widget build(BuildContext context) => _RowShell(
    label: label,
    content: Text(context.l10n.totpLater),
    actions: const <Widget>[],
  );
}

class _UrlRow extends ConsumerWidget {
  const _UrlRow({required this.url});
  final String url;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final l10n = context.l10n;
    return _RowShell(
      label: l10n.urlLabel,
      content: Text(url),
      actions: <Widget>[
        IconButton(
          tooltip: l10n.openUrl,
          icon: const Icon(Icons.open_in_new),
          onPressed: () => openLink(context, ref, url),
        ),
        IconButton(
          tooltip: l10n.copyField(l10n.urlLabel),
          icon: const Icon(Icons.copy),
          onPressed: () => copyToClipboard(context, ref, text: url),
        ),
      ],
    );
  }
}

class _CustomRow extends ConsumerWidget {
  const _CustomRow({required this.view, required this.field});
  final ItemView view;
  final CustomView field;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final l10n = context.l10n;
    final VaultApi api = ref.read(vaultApiProvider);
    if (field.kind != CustomKind.hidden) {
      final String value = field.valueIfNotHidden ?? '';
      return _RowShell(
        label: field.label,
        content: Text(value),
        actions: <Widget>[
          if (field.kind == CustomKind.url)
            IconButton(
              tooltip: l10n.openUrl,
              icon: const Icon(Icons.open_in_new),
              onPressed: () => openLink(context, ref, value),
            ),
          IconButton(
            tooltip: l10n.copyField(field.label),
            icon: const Icon(Icons.copy),
            onPressed: () => copyToClipboard(context, ref, text: value),
          ),
        ],
      );
    }
    final String key = revealKey(view.id, 'custom:${field.id}');
    final bool revealed = ref.watch(revealProvider).contains(key);
    return _RowShell(
      label: field.label,
      content: Text(
        revealed
            ? _decode(ref.read(revealProvider.notifier).copyOf(key))
            : _mask,
        style: const TextStyle(fontFamily: 'monospace'),
      ),
      actions: <Widget>[
        IconButton(
          tooltip: revealed
              ? l10n.hideField(field.label)
              : l10n.revealField(field.label),
          icon: Icon(revealed ? Icons.visibility_off : Icons.visibility),
          onPressed: () => revealed
              ? ref.read(revealProvider.notifier).hide(key)
              : revealValue(
                  context,
                  ref,
                  key,
                  () => api.revealCustom(view.id, field.id),
                ),
        ),
        IconButton(
          tooltip: l10n.copyField(field.label),
          icon: const Icon(Icons.copy),
          onPressed: () => copyToClipboard(
            context,
            ref,
            loadSecret: () => api.revealCustom(view.id, field.id),
          ),
        ),
      ],
    );
  }
}
