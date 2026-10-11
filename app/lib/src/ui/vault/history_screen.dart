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
import '../../state/vault_session.dart';
import '../../theme/app_theme.dart';
import '../widgets/error_text.dart';
import '../widgets/l10n_ext.dart';
import 'item_detail_pane.dart' show revealValue;
import 'vault_format.dart';

/// Versions of one field, newest first (the current one included).
final historyProvider = FutureProvider.autoDispose
    .family<List<VersionInfo>, (String, StdField)>((
      Ref ref,
      (String, StdField) key,
    ) async {
      ref.watch(lockEpochProvider);
      try {
        return await ref.read(vaultApiProvider).history(key.$1, key.$2);
      } on AppError catch (e) {
        handleLocked(ref, e);
        rethrow;
      }
    });

/// A stored non-secret version value (loaded eagerly; secrets use Reveal).
final versionValueProvider = FutureProvider.autoDispose
    .family<String?, (String, StdField, int)>((
      Ref ref,
      (String, StdField, int) key,
    ) async {
      ref.watch(lockEpochProvider);
      final Uint8List? bytes = await ref
          .read(vaultApiProvider)
          .revealVersion(key.$1, key.$2, key.$3);
      if (bytes == null) return null;
      final String s = utf8.decode(bytes, allowMalformed: true);
      bytes.fillRange(0, bytes.length, 0);
      return s;
    });

String versionRevealKey(String itemId, StdField field, int hlc) =>
    revealKey(itemId, 'v:${field.name}:$hlc');

/// "Time, device, badges" line shared by history and compare.
String versionTitle(BuildContext context, VersionInfo v) {
  final l10n = context.l10n;
  final String when = formatDateTime(context, v.atMs);
  final String device =
      v.deviceName ?? (v.concurrent ? l10n.otherDevice : l10n.thisDevice);
  return '$when · $device';
}

/// A version's value: shown directly for ordinary fields, masked until Reveal
/// for secret ones (and auto-hidden like any reveal).
class VersionValue extends ConsumerWidget {
  const VersionValue({
    required this.itemId,
    required this.field,
    required this.hlc,
    super.key,
  });
  final String itemId;
  final StdField field;
  final int hlc;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final l10n = context.l10n;
    if (!kSecretFields.contains(field)) {
      final AsyncValue<String?> v = ref.watch(
        versionValueProvider((itemId, field, hlc)),
      );
      return Text(v.value ?? (v.hasError ? l10n.errInternal : '…'));
    }
    final String key = versionRevealKey(itemId, field, hlc);
    final bool revealed = ref.watch(revealProvider).contains(key);
    String text = '••••••••';
    if (revealed) {
      final Uint8List? bytes = ref.read(revealProvider.notifier).copyOf(key);
      if (bytes != null) {
        text = utf8.decode(bytes, allowMalformed: true);
        bytes.fillRange(0, bytes.length, 0);
      }
    }
    final VaultApi api = ref.read(vaultApiProvider);
    return Row(
      children: <Widget>[
        Expanded(
          child: Semantics(
            label: revealed ? null : l10n.hiddenValue,
            child: ExcludeSemantics(
              excluding: !revealed,
              child: Text(
                text,
                style: const TextStyle(fontFamily: 'monospace'),
              ),
            ),
          ),
        ),
        IconButton(
          tooltip: revealed
              ? l10n.hideField(stdFieldLabel(l10n, field))
              : l10n.revealField(stdFieldLabel(l10n, field)),
          icon: Icon(revealed ? Icons.visibility_off : Icons.visibility),
          onPressed: () => revealed
              ? ref.read(revealProvider.notifier).hide(key)
              : revealValue(
                  context,
                  ref,
                  key,
                  () => api.revealVersion(itemId, field, hlc),
                ),
        ),
      ],
    );
  }
}

/// Per-field history with restore, and a way into the compare view for
/// concurrent versions (docs/06 §5.3: a derived view; the UI only displays it).
class HistoryScreen extends ConsumerStatefulWidget {
  const HistoryScreen({required this.itemId, required this.field, super.key});
  final String itemId;
  final StdField field;

  @override
  ConsumerState<HistoryScreen> createState() => _HistoryScreenState();
}

class _HistoryScreenState extends ConsumerState<HistoryScreen> {
  late StdField _field = widget.field;
  late final RevealNotifier _reveal;

  @override
  void initState() {
    super.initState();
    _reveal = ref.read(revealProvider.notifier);
  }

  @override
  void dispose() {
    final RevealNotifier reveal = _reveal;
    final String prefix = '${widget.itemId}|v:';
    Future<void>.microtask(() => reveal.hideWithPrefix(prefix));
    super.dispose();
  }

  Future<void> _restore(VersionInfo v) async {
    final l10n = context.l10n;
    final ScaffoldMessengerState m = ScaffoldMessenger.of(context);
    try {
      await ref
          .read(itemActionsProvider)
          .restoreVersion(widget.itemId, _field, v.hlcMs);
      ref.invalidate(historyProvider((widget.itemId, _field)));
      m.showSnackBar(SnackBar(content: Text(l10n.versionRestored)));
    } on AppError catch (e) {
      m.showSnackBar(SnackBar(content: Text(errorMessage(l10n, e))));
    }
  }

  @override
  Widget build(BuildContext context) {
    final l10n = context.l10n;
    final AppTokens tokens = context.tokens;
    final ThemeData theme = Theme.of(context);
    final ItemView? view = ref.watch(itemViewProvider(widget.itemId)).value;
    final List<StdField> fields = view == null
        ? <StdField>[widget.field]
        : fieldsFor(view.type)
              .where((StdField f) => f != StdField.totpSeed)
              .toList();
    final AsyncValue<List<VersionInfo>> async = ref.watch(
      historyProvider((widget.itemId, _field)),
    );
    return Scaffold(
      appBar: AppBar(
        title: Semantics(
          header: true,
          child: Text(l10n.historyTitle(view?.title ?? '')),
        ),
      ),
      body: SafeArea(
        child: Center(
          child: ConstrainedBox(
            constraints: const BoxConstraints(maxWidth: 720),
            child: ListView(
              padding: EdgeInsets.all(tokens.gap),
              children: <Widget>[
                Wrap(
                  spacing: 8,
                  runSpacing: 4,
                  children: <Widget>[
                    for (final StdField f in fields)
                      ChoiceChip(
                        label: Text(stdFieldLabel(l10n, f)),
                        selected: f == _field,
                        onSelected: (_) => setState(() => _field = f),
                      ),
                  ],
                ),
                SizedBox(height: tokens.gap),
                if (async.value == null)
                  async.hasError
                      ? ErrorText(
                          async.error is AppError
                              ? errorMessage(l10n, async.error! as AppError)
                              : l10n.errInternal,
                        )
                      : const Center(child: CircularProgressIndicator())
                else if (async.value!.length <= 1 &&
                    async.value!.every((VersionInfo v) => v.isCurrent))
                  Text(l10n.noHistory)
                else
                  for (final VersionInfo v in async.value!)
                    Card(
                      child: Padding(
                        padding: EdgeInsets.all(tokens.gap),
                        child: Column(
                          crossAxisAlignment: CrossAxisAlignment.stretch,
                          children: <Widget>[
                            Text(
                              versionTitle(context, v),
                              style: theme.textTheme.titleSmall,
                            ),
                            Wrap(
                              spacing: 8,
                              children: <Widget>[
                                if (v.isCurrent)
                                  Chip(label: Text(l10n.currentVersion)),
                                if (v.concurrent)
                                  Chip(
                                    avatar: const Icon(
                                      Icons.call_split,
                                      size: 18,
                                    ),
                                    label: Text(l10n.concurrentVersion),
                                  ),
                              ],
                            ),
                            VersionValue(
                              itemId: widget.itemId,
                              field: _field,
                              hlc: v.hlcMs,
                            ),
                            Wrap(
                              spacing: 8,
                              children: <Widget>[
                                if (!v.isCurrent)
                                  OutlinedButton(
                                    onPressed: () => _restore(v),
                                    child: Text(l10n.restoreThisVersion),
                                  ),
                                if (v.concurrent)
                                  FilledButton(
                                    onPressed: () => context.push(
                                      '${AppRoutes.compare}/${widget.itemId}/${_field.name}/${v.hlcMs}',
                                    ),
                                    child: Text(l10n.compare),
                                  ),
                              ],
                            ),
                          ],
                        ),
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

/// Current vs another device's version of one field, with "Restore this
/// version" and, for note bodies, "Keep as separate note" (docs/06 §5.3).
class CompareScreen extends ConsumerStatefulWidget {
  const CompareScreen({
    required this.itemId,
    required this.field,
    required this.hlc,
    super.key,
  });
  final String itemId;
  final StdField field;
  final int hlc;

  @override
  ConsumerState<CompareScreen> createState() => _CompareScreenState();
}

class _CompareScreenState extends ConsumerState<CompareScreen> {
  late final RevealNotifier _reveal;

  @override
  void initState() {
    super.initState();
    _reveal = ref.read(revealProvider.notifier);
  }

  @override
  void dispose() {
    final RevealNotifier reveal = _reveal;
    final String prefix = '${widget.itemId}|v:';
    Future<void>.microtask(() => reveal.hideWithPrefix(prefix));
    super.dispose();
  }

  Future<void> _restore() async {
    final l10n = context.l10n;
    final ScaffoldMessengerState m = ScaffoldMessenger.of(context);
    try {
      await ref
          .read(itemActionsProvider)
          .restoreVersion(widget.itemId, widget.field, widget.hlc);
      ref.invalidate(historyProvider((widget.itemId, widget.field)));
      m.showSnackBar(SnackBar(content: Text(l10n.versionRestored)));
      if (mounted) context.pop();
    } on AppError catch (e) {
      m.showSnackBar(SnackBar(content: Text(errorMessage(l10n, e))));
    }
  }

  Future<void> _keepAsNote(ItemView view) async {
    final l10n = context.l10n;
    final ScaffoldMessengerState m = ScaffoldMessenger.of(context);
    try {
      final Uint8List? bytes = await ref
          .read(vaultApiProvider)
          .revealVersion(widget.itemId, widget.field, widget.hlc);
      if (bytes == null) return;
      final String body = utf8.decode(bytes, allowMalformed: true);
      bytes.fillRange(0, bytes.length, 0);
      await ref
          .read(itemActionsProvider)
          .keepAsSeparateNote(
            title: l10n.otherVersionNoteTitle(view.title),
            body: body,
            folderId: view.folderId,
          );
      m.showSnackBar(SnackBar(content: Text(l10n.keptAsNote)));
    } on AppError catch (e) {
      handleLockedWidget(ref, e);
      m.showSnackBar(SnackBar(content: Text(errorMessage(l10n, e))));
    }
  }

  @override
  Widget build(BuildContext context) {
    final l10n = context.l10n;
    final AppTokens tokens = context.tokens;
    final ThemeData theme = Theme.of(context);
    final ItemView? view = ref.watch(itemViewProvider(widget.itemId)).value;
    final AsyncValue<List<VersionInfo>> async = ref.watch(
      historyProvider((widget.itemId, widget.field)),
    );
    final List<VersionInfo>? versions = async.value;
    Widget column(String heading, VersionInfo v, {required bool other}) => Card(
      child: Padding(
        padding: EdgeInsets.all(tokens.gap),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: <Widget>[
            Semantics(
              header: true,
              child: Text(heading, style: theme.textTheme.titleMedium),
            ),
            Text(versionTitle(context, v), style: theme.textTheme.bodySmall),
            VersionValue(
              itemId: widget.itemId,
              field: widget.field,
              hlc: v.hlcMs,
            ),
            if (other) ...<Widget>[
              SizedBox(height: tokens.gap / 2),
              FilledButton(
                onPressed: _restore,
                child: Text(l10n.restoreThisVersion),
              ),
              if (widget.field == StdField.body && view != null) ...<Widget>[
                SizedBox(height: tokens.gap / 2),
                OutlinedButton(
                  onPressed: () => _keepAsNote(view),
                  child: Text(l10n.keepAsSeparateNote),
                ),
              ],
            ],
          ],
        ),
      ),
    );
    return Scaffold(
      appBar: AppBar(
        title: Semantics(
          header: true,
          child: Text(l10n.compareTitle(stdFieldLabel(l10n, widget.field))),
        ),
      ),
      body: SafeArea(
        child: versions == null
            ? (async.hasError
                  ? Padding(
                      padding: EdgeInsets.all(tokens.gap),
                      child: ErrorText(l10n.errInternal),
                    )
                  : const Center(child: CircularProgressIndicator()))
            : Builder(
                builder: (BuildContext context) {
                  final VersionInfo? current = versions
                      .where((VersionInfo v) => v.isCurrent)
                      .firstOrNull;
                  final VersionInfo? other = versions
                      .where((VersionInfo v) => v.hlcMs == widget.hlc)
                      .firstOrNull;
                  if (current == null || other == null) {
                    return Padding(
                      padding: EdgeInsets.all(tokens.gap),
                      child: Text(l10n.errNotFound),
                    );
                  }
                  final List<Widget> cards = <Widget>[
                    column(l10n.currentVersion, current, other: false),
                    column(l10n.otherVersion, other, other: true),
                  ];
                  return LayoutBuilder(
                    builder: (BuildContext context, BoxConstraints c) =>
                        SingleChildScrollView(
                          padding: EdgeInsets.all(tokens.gap),
                          child: c.maxWidth >= 700
                              ? Row(
                                  crossAxisAlignment: CrossAxisAlignment.start,
                                  children: <Widget>[
                                    for (final Widget w in cards)
                                      Expanded(child: w),
                                  ],
                                )
                              : Column(children: cards),
                        ),
                  );
                },
              ),
      ),
    );
  }
}
