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
import '../../state/password_generator_hook.dart';
import '../../state/providers.dart';
import '../../state/vault_query.dart';
import '../../state/vault_session.dart';
import '../../theme/app_theme.dart';
import '../widgets/error_text.dart';
import '../widgets/l10n_ext.dart';
import '../widgets/secret_text_field.dart';
import 'dialogs.dart';
import 'vault_format.dart';

class _UrlEntry {
  _UrlEntry({this.id, String text = ''})
    : original = text,
      controller = TextEditingController(text: text);
  final String? id;
  final String original;
  final TextEditingController controller;
}

class _CustomEntry {
  _CustomEntry({
    this.id,
    required this.kind,
    String label = '',
    String value = '',
    this.hiddenExisting = false,
  }) : originalLabel = label,
       originalValue = value,
       labelController = TextEditingController(text: label),
       valueController = SecretTextController(text: value);
  final String? id;
  final CustomKind kind;
  final String originalLabel;
  final String originalValue;

  /// An existing hidden value: not loaded; empty means "keep it".
  final bool hiddenExisting;
  final TextEditingController labelController;
  final SecretTextController valueController;
}

/// Create or edit a login, note, card or identity.
///
/// Decisions (B02 item 5 and the "locked mid-edit" test line):
///  * Saving an existing item sends only what changed, with one atomic
///    `setFields` for the standard fields; create is one `createItem`.
///  * Existing secret values are not loaded into the form: an empty secret field
///    means "keep the current value", with an explicit "Remove" for clearing. The
///    one exception is a note body, which is the content being edited.
///  * If the session locks mid-edit, everything is discarded, non-secret drafts
///    included: the screen is torn down with the vault. Keeping a title or a
///    login's notes in memory past a lock buys little and breaks the promise
///    that a lock clears the app.
class ItemEditScreen extends ConsumerStatefulWidget {
  const ItemEditScreen({this.itemId, this.newType, super.key})
    : assert(itemId != null || newType != null);

  final String? itemId;
  final ItemType? newType;

  @override
  ConsumerState<ItemEditScreen> createState() => _ItemEditScreenState();
}

class _ItemEditScreenState extends ConsumerState<ItemEditScreen> {
  late final ItemType _type;
  ItemView? _view;
  bool _loading = true;
  String? _loadError;
  bool _dirty = false;
  bool _saving = false;
  String? _error;
  final Map<String, String> _fieldErrors = <String, String>{};

  final Map<StdField, TextEditingController> _c =
      <StdField, TextEditingController>{};
  final Map<StdField, String> _orig = <StdField, String>{};
  final Set<StdField> _present = <StdField>{};
  final Set<StdField> _clear = <StdField>{};
  final List<_UrlEntry> _urls = <_UrlEntry>[];
  final List<String> _removedUrlIds = <String>[];
  final List<_CustomEntry> _custom = <_CustomEntry>[];
  final List<String> _removedCustomIds = <String>[];
  final List<String> _tags = <String>[];
  final Set<String> _origTags = <String>{};
  final TextEditingController _tagInput = TextEditingController();
  String? _folderId;

  bool get _isNew => widget.itemId == null;

  @override
  void initState() {
    super.initState();
    _type = widget.newType ?? ItemType.login;
    _load();
  }

  bool _disposed = false;

  @override
  void dispose() {
    // Clearing the controllers below notifies _markDirty; it must not setState.
    _disposed = true;
    for (final TextEditingController c in _c.values) {
      if (c is SecretTextController) c.clearSecret();
      c.dispose();
    }
    for (final _UrlEntry u in _urls) {
      u.controller.dispose();
    }
    for (final _CustomEntry e in _custom) {
      e.labelController.dispose();
      e.valueController.clearSecret();
      e.valueController.dispose();
    }
    _tagInput.dispose();
    super.dispose();
  }

  TextEditingController _controllerFor(ItemType type, StdField f, String text) {
    final TextEditingController c =
        kSecretFields.contains(f) && f != StdField.body
        ? SecretTextController(text: text)
        : TextEditingController(text: text);
    c.addListener(_markDirty);
    return c;
  }

  void _markDirty() {
    if (!_dirty && !_disposed) {
      setState(() => _dirty = true);
    }
  }

  Future<void> _load() async {
    final VaultApi api = ref.read(vaultApiProvider);
    try {
      ItemType type = _type;
      ItemView? view;
      if (!_isNew) {
        final ItemView loaded = await ref.read(
          itemViewProvider(widget.itemId!).future,
        );
        view = loaded;
        type = loaded.type;
      }
      String? body;
      if (view != null && view.secretFieldsPresent.contains(StdField.body)) {
        final Uint8List? bytes = await api.reveal(view.id, StdField.body);
        if (bytes != null) {
          body = utf8.decode(bytes, allowMalformed: true);
          bytes.fillRange(0, bytes.length, 0);
        }
      }
      if (!mounted) return;
      setState(() {
        _view = view;
        for (final StdField f in fieldsFor(type)) {
          final String initial = f == StdField.body
              ? (body ?? '')
              : (view?.fields[f] ?? '');
          _orig[f] = initial;
          _c[f] = _controllerFor(type, f, initial);
          if (view != null && view.secretFieldsPresent.contains(f)) {
            _present.add(f);
          }
        }
        if (view != null) {
          _folderId = view.folderId;
          _tags.addAll(view.tags);
          _origTags.addAll(view.tags);
          for (final UrlView u in view.urls) {
            _urls.add(
              _UrlEntry(id: u.id, text: u.url)
                ..controller.addListener(_markDirty),
            );
          }
          for (final CustomView cv in view.custom) {
            final _CustomEntry e = _CustomEntry(
              id: cv.id,
              kind: cv.kind,
              label: cv.label,
              value: cv.valueIfNotHidden ?? '',
              hiddenExisting: cv.kind == CustomKind.hidden,
            );
            e.labelController.addListener(_markDirty);
            e.valueController.addListener(_markDirty);
            _custom.add(e);
          }
        }
        _loading = false;
      });
    } on AppError catch (e) {
      handleLockedWidget(ref, e);
      if (mounted) {
        setState(() {
          _loading = false;
          _loadError = errorMessage(context.l10n, e);
        });
      }
    }
  }

  ItemType get _effectiveType => _view?.type ?? _type;

  // ---------------------------------------------------------------- saving

  Future<void> _save() async {
    if (_saving) return;
    final l10n = context.l10n;
    final String title = _c[StdField.title]!.text.trim();
    setState(() {
      _error = null;
      _fieldErrors.clear();
    });
    if (title.isEmpty) {
      setState(() => _fieldErrors[StdField.title.name] = l10n.titleRequired);
      return;
    }
    setState(() => _saving = true);
    final VaultApi api = ref.read(vaultApiProvider);
    final ItemActions actions = ref.read(itemActionsProvider);
    try {
      if (_isNew) {
        final String id = await api.createItem(_newItem(title));
        actions.changed();
        ref.read(vaultQueryProvider.notifier).select(id);
      } else {
        await _saveExisting(api, title);
        actions.changed(id: widget.itemId);
      }
      if (!mounted) return;
      _dirty = false;
      context.pop();
    } on AppError catch (e) {
      handleLockedWidget(ref, e);
      if (!mounted) return;
      setState(() {
        final String message = errorMessage(l10n, e);
        if (e.field != null && _c.keys.any((StdField f) => f.name == e.field)) {
          _fieldErrors[e.field!] = message;
        } else {
          _error = message;
        }
      });
    } finally {
      if (mounted) setState(() => _saving = false);
    }
  }

  NewItem _newItem(String title) => NewItem(
    type: _type,
    title: title,
    fields: <StdField, String>{
      for (final MapEntry<StdField, TextEditingController> e in _c.entries)
        if (e.key != StdField.title && e.value.text.isNotEmpty)
          e.key: e.value.text,
    },
    urls: <String>[
      for (final _UrlEntry u in _urls)
        if (u.controller.text.trim().isNotEmpty) u.controller.text.trim(),
    ],
    tags: List<String>.of(_tags),
    folderId: _folderId,
    custom: <NewCustom>[
      for (final _CustomEntry e in _custom)
        if (e.labelController.text.trim().isNotEmpty)
          NewCustom(
            kind: e.kind,
            label: e.labelController.text.trim(),
            value: e.valueController.text,
          ),
    ],
  );

  Future<void> _saveExisting(VaultApi api, String title) async {
    final String id = widget.itemId!;
    final Map<StdField, String> changes = <StdField, String>{};
    final List<StdField> clears = <StdField>[];
    for (final StdField f in fieldsFor(_effectiveType)) {
      final String text = f == StdField.title ? title : _c[f]!.text;
      final bool secretKeep = kSecretFields.contains(f) && f != StdField.body;
      if (secretKeep) {
        if (text.isNotEmpty) {
          changes[f] = text;
        } else if (_clear.contains(f)) {
          clears.add(f);
        }
      } else if (text != (_orig[f] ?? '')) {
        if (text.isEmpty) {
          clears.add(f);
        } else {
          changes[f] = text;
        }
      }
    }
    if (changes.isNotEmpty) await api.setFields(id, changes);
    for (final StdField f in clears) {
      await api.clearField(id, f);
    }
    for (final _UrlEntry u in _urls) {
      final String text = u.controller.text.trim();
      if (u.id == null) {
        if (text.isNotEmpty) await api.addUrl(id, text);
      } else if (text != u.original) {
        if (text.isEmpty) {
          await api.removeUrl(id, u.id!);
        } else {
          await api.setUrl(id, u.id!, text);
        }
      }
    }
    for (final String urlId in _removedUrlIds) {
      await api.removeUrl(id, urlId);
    }
    for (final String t in _tags.where((String t) => !_origTags.contains(t))) {
      await api.addTag(id, t);
    }
    for (final String t in _origTags.where((String t) => !_tags.contains(t))) {
      await api.removeTag(id, t);
    }
    for (final _CustomEntry e in _custom) {
      final String label = e.labelController.text.trim();
      if (e.id == null) {
        if (label.isNotEmpty) {
          await api.addCustomField(id, e.kind, label, e.valueController.text);
        }
        continue;
      }
      if (label != e.originalLabel && label.isNotEmpty) {
        await api.setCustomLabel(id, e.id!, label);
      }
      final String value = e.valueController.text;
      final bool changed = e.hiddenExisting
          ? value.isNotEmpty
          : value != e.originalValue;
      if (changed) await api.setCustomValue(id, e.id!, value);
    }
    for (final String cid in _removedCustomIds) {
      await api.removeCustomField(id, cid);
    }
    if (_folderId != _view?.folderId) await api.moveToFolder(id, _folderId);
  }

  // ------------------------------------------------------------------- UI

  Future<void> _generate() async {
    final PasswordGeneratorLauncher launcher = ref.read(
      passwordGeneratorLauncherProvider,
    );
    final ScaffoldMessengerState messenger = ScaffoldMessenger.of(context);
    final l10n = context.l10n;
    try {
      final Uint8List? bytes = await launcher(context);
      if (bytes == null || !mounted) return;
      _c[StdField.password]!.text = utf8.decode(bytes, allowMalformed: true);
      bytes.fillRange(0, bytes.length, 0);
      _clear.remove(StdField.password);
    } on AppError catch (e) {
      handleLockedWidget(ref, e);
      messenger.showSnackBar(SnackBar(content: Text(errorMessage(l10n, e))));
    }
  }

  void _addTag() {
    final String t = _tagInput.text.trim();
    if (t.isEmpty || _tags.contains(t) || _tags.length >= kMaxTags) return;
    setState(() {
      _tags.add(t);
      _tagInput.clear();
      _dirty = true;
    });
  }

  Widget _fieldWidget(StdField f) {
    final l10n = context.l10n;
    final String label = stdFieldLabel(l10n, f);
    final String? error = _fieldErrors[f.name];
    final TextEditingController c = _c[f]!;
    final int max = maxBytesOf(f);
    final int used = utf8.encode(c.text).length;
    final String? limit = used * 2 >= max
        ? l10n.sizeHint(formatBytes(used), formatBytes(max))
        : null;
    if (kSecretFields.contains(f) && f != StdField.body) {
      final bool present = _present.contains(f);
      final bool removing = _clear.contains(f);
      return Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: <Widget>[
          Row(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: <Widget>[
              Expanded(
                child: SecretTextField(
                  controller: c as SecretTextController,
                  label: label,
                  errorText: error,
                  helperText: removing
                      ? l10n.willBeRemoved
                      : present
                      ? l10n.keepCurrentHint
                      : limit,
                ),
              ),
              if (f == StdField.password)
                IconButton(
                  tooltip: l10n.generatePassword,
                  icon: const Icon(Icons.auto_fix_high),
                  onPressed: _generate,
                ),
            ],
          ),
          if (present)
            Align(
              alignment: AlignmentDirectional.centerStart,
              child: TextButton(
                onPressed: () => setState(() {
                  removing ? _clear.remove(f) : _clear.add(f);
                  c.clearSecret();
                  _dirty = true;
                }),
                child: Text(
                  removing ? l10n.keepValue : l10n.removeValue(label),
                ),
              ),
            ),
        ],
      );
    }
    final bool multiline =
        f == StdField.body || f == StdField.notes || f == StdField.address;
    return TextField(
      controller: c,
      minLines: f == StdField.body ? 8 : (multiline ? 3 : 1),
      maxLines: multiline ? null : 1,
      keyboardType: multiline ? TextInputType.multiline : TextInputType.text,
      decoration: InputDecoration(
        labelText: label,
        errorText: error,
        helperText: limit,
        alignLabelWithHint: multiline,
      ),
      onChanged: (_) => setState(() {}),
    );
  }

  @override
  Widget build(BuildContext context) {
    final l10n = context.l10n;
    final AppTokens tokens = context.tokens;
    final ThemeData theme = Theme.of(context);
    final String title = _isNew
        ? l10n.newItemTitle(itemTypeLabel(l10n, _type))
        : l10n.editItemTitle;
    return PopScope(
      canPop: !_dirty || _saving,
      onPopInvokedWithResult: (bool didPop, Object? result) async {
        if (didPop) return;
        final bool discard = await confirmDialog(
          context,
          title: l10n.discardTitle,
          body: l10n.discardBody,
          confirmLabel: l10n.discard,
          destructive: true,
        );
        if (discard && context.mounted) {
          setState(() => _dirty = false);
          context.pop();
        }
      },
      child: Scaffold(
        appBar: AppBar(
          title: Semantics(header: true, child: Text(title)),
          actions: <Widget>[
            Padding(
              padding: const EdgeInsetsDirectional.only(end: 8),
              child: TextButton(
                onPressed: _loading || _saving ? null : _save,
                child: Text(l10n.save),
              ),
            ),
          ],
        ),
        body: SafeArea(
          child: _loading
              ? const Center(child: CircularProgressIndicator())
              : _loadError != null
              ? Padding(
                  padding: EdgeInsets.all(tokens.gap),
                  child: ErrorText(_loadError!),
                )
              : Center(
                  child: ConstrainedBox(
                    constraints: const BoxConstraints(maxWidth: 720),
                    child: SingleChildScrollView(
                      padding: EdgeInsets.all(tokens.gap),
                      child: Column(
                        crossAxisAlignment: CrossAxisAlignment.stretch,
                        children: <Widget>[
                          if (_error != null) ...<Widget>[
                            ErrorText(_error!),
                            SizedBox(height: tokens.gap),
                          ],
                          for (final StdField f in fieldsFor(
                            _effectiveType,
                          )) ...<Widget>[
                            _fieldWidget(f),
                            SizedBox(height: tokens.gap),
                          ],
                          if (_effectiveType == ItemType.login)
                            _urlsSection(theme),
                          _tagsSection(theme),
                          _customSection(theme),
                          _folderSection(),
                          SizedBox(height: tokens.gapLarge),
                          FilledButton(
                            onPressed: _saving ? null : _save,
                            child: _saving
                                ? Semantics(
                                    liveRegion: true,
                                    label: l10n.saving,
                                    child: const SizedBox(
                                      height: 20,
                                      width: 20,
                                      child: CircularProgressIndicator(
                                        strokeWidth: 2,
                                      ),
                                    ),
                                  )
                                : Text(l10n.save),
                          ),
                        ],
                      ),
                    ),
                  ),
                ),
        ),
      ),
    );
  }

  Widget _urlsSection(ThemeData theme) {
    final l10n = context.l10n;
    final AppTokens tokens = context.tokens;
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: <Widget>[
        Text(l10n.urlsLabel, style: theme.textTheme.titleSmall),
        for (final _UrlEntry u in _urls)
          Row(
            children: <Widget>[
              Expanded(
                child: Padding(
                  padding: const EdgeInsets.symmetric(vertical: 4),
                  child: TextField(
                    controller: u.controller,
                    keyboardType: TextInputType.url,
                    decoration: InputDecoration(labelText: l10n.urlLabel),
                  ),
                ),
              ),
              IconButton(
                tooltip: l10n.removeUrl,
                icon: const Icon(Icons.remove_circle_outline),
                onPressed: () => setState(() {
                  if (u.id != null) _removedUrlIds.add(u.id!);
                  _urls.remove(u);
                  u.controller.dispose();
                  _dirty = true;
                }),
              ),
            ],
          ),
        Align(
          alignment: AlignmentDirectional.centerStart,
          child: TextButton.icon(
            onPressed: () => setState(() {
              _urls.add(_UrlEntry()..controller.addListener(_markDirty));
              _dirty = true;
            }),
            icon: const Icon(Icons.add),
            label: Text(l10n.addUrl),
          ),
        ),
        SizedBox(height: tokens.gap),
      ],
    );
  }

  Widget _tagsSection(ThemeData theme) {
    final l10n = context.l10n;
    final AppTokens tokens = context.tokens;
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: <Widget>[
        Text(
          l10n.tagsCount(_tags.length, kMaxTags),
          style: theme.textTheme.titleSmall,
        ),
        Wrap(
          spacing: 8,
          children: <Widget>[
            for (final String t in _tags)
              InputChip(
                label: Text(t),
                deleteButtonTooltipMessage: l10n.removeTag(t),
                onDeleted: () => setState(() {
                  _tags.remove(t);
                  _dirty = true;
                }),
              ),
          ],
        ),
        Row(
          children: <Widget>[
            Expanded(
              child: TextField(
                controller: _tagInput,
                maxLength: kMaxTagBytes,
                decoration: InputDecoration(
                  labelText: l10n.addTagLabel,
                  counterText: '',
                ),
                onSubmitted: (_) => _addTag(),
              ),
            ),
            IconButton(
              tooltip: l10n.addTag,
              icon: const Icon(Icons.add),
              onPressed: _tags.length >= kMaxTags ? null : _addTag,
            ),
          ],
        ),
        SizedBox(height: tokens.gap),
      ],
    );
  }

  Widget _customSection(ThemeData theme) {
    final l10n = context.l10n;
    final AppTokens tokens = context.tokens;
    String kindLabel(CustomKind k) => switch (k) {
      CustomKind.text => l10n.kindText,
      CustomKind.hidden => l10n.kindHidden,
      CustomKind.url => l10n.kindUrl,
      CustomKind.date => l10n.kindDate,
    };
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: <Widget>[
        Text(
          l10n.customFieldsCount(_custom.length, kMaxCustomFields),
          style: theme.textTheme.titleSmall,
        ),
        for (final _CustomEntry e in _custom)
          Padding(
            padding: const EdgeInsets.symmetric(vertical: 6),
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.stretch,
              children: <Widget>[
                Row(
                  children: <Widget>[
                    Expanded(
                      child: TextField(
                        controller: e.labelController,
                        decoration: InputDecoration(
                          labelText: l10n.customLabel,
                        ),
                      ),
                    ),
                    IconButton(
                      tooltip: l10n.removeCustomField,
                      icon: const Icon(Icons.remove_circle_outline),
                      onPressed: () => setState(() {
                        if (e.id != null) _removedCustomIds.add(e.id!);
                        _custom.remove(e);
                        e.labelController.dispose();
                        e.valueController.clearSecret();
                        e.valueController.dispose();
                        _dirty = true;
                      }),
                    ),
                  ],
                ),
                const SizedBox(height: 8),
                if (e.kind == CustomKind.hidden)
                  SecretTextField(
                    controller: e.valueController,
                    label: l10n.customValue,
                    helperText: e.hiddenExisting ? l10n.keepCurrentHint : null,
                  )
                else
                  TextField(
                    controller: e.valueController,
                    decoration: InputDecoration(labelText: l10n.customValue),
                  ),
                Padding(
                  padding: const EdgeInsets.only(top: 4),
                  child: Text(
                    kindLabel(e.kind),
                    style: theme.textTheme.bodySmall,
                  ),
                ),
              ],
            ),
          ),
        Wrap(
          spacing: 8,
          children: <Widget>[
            for (final CustomKind k in CustomKind.values)
              OutlinedButton.icon(
                onPressed: _custom.length >= kMaxCustomFields
                    ? null
                    : () => setState(() {
                        final _CustomEntry entry = _CustomEntry(kind: k);
                        entry.labelController.addListener(_markDirty);
                        entry.valueController.addListener(_markDirty);
                        _custom.add(entry);
                        _dirty = true;
                      }),
                icon: const Icon(Icons.add),
                label: Text(l10n.addCustomField(kindLabel(k))),
              ),
          ],
        ),
        SizedBox(height: tokens.gap),
      ],
    );
  }

  Widget _folderSection() {
    final l10n = context.l10n;
    final List<Folder> folders =
        ref.watch(foldersProvider).value ?? const <Folder>[];
    return DropdownButtonFormField<String?>(
      isExpanded: true,
      initialValue: folders.any((Folder f) => f.id == _folderId)
          ? _folderId
          : null,
      decoration: InputDecoration(labelText: l10n.folderLabel),
      items: <DropdownMenuItem<String?>>[
        DropdownMenuItem<String?>(child: Text(l10n.noFolder)),
        for (final Folder f in folders)
          DropdownMenuItem<String?>(value: f.id, child: Text(f.name)),
      ],
      onChanged: (String? id) => setState(() {
        _folderId = id;
        _dirty = true;
      }),
    );
  }
}
