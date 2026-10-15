// Conversions between the app's own DTOs (dtos.dart, what the UI and FakeVaultApi
// use) and the flutter_rust_bridge-generated ones (lib/src/rust). The two sets have
// the same field names (docs/14 §3); only a few types differ (`type` is `itemType`
// in the generated code, `challenge` is a Uint32List, `EntropyOptions` is freezed).
// No logic lives here: every function is a field-by-field copy.
import '../rust/api/dto.dart' as r;
import 'app_error.dart';
import 'dtos.dart';

// ------------------------------------------------------------------ errors

AppError toAppError(r.AppError e) => AppError(
  AppErrorCode.values.byName(e.code.name),
  e.message,
  field: e.field,
);

// ------------------------------------------------------------------- enums

r.KdfProfile kdfToRust(KdfProfile v) => r.KdfProfile.values.byName(v.name);
r.StdField fieldToRust(StdField v) => r.StdField.values.byName(v.name);
StdField fieldFromRust(r.StdField v) => StdField.values.byName(v.name);
r.ItemType itemTypeToRust(ItemType v) => r.ItemType.values.byName(v.name);
ItemType itemTypeFromRust(r.ItemType v) => ItemType.values.byName(v.name);
r.CustomKind customKindToRust(CustomKind v) =>
    r.CustomKind.values.byName(v.name);
CustomKind customKindFromRust(r.CustomKind v) =>
    CustomKind.values.byName(v.name);
r.ImportFormat importFormatToRust(ImportFormat v) =>
    r.ImportFormat.values.byName(v.name);

// --------------------------------------------------------------- lifecycle

QuickUnlockStatus quickUnlockFromRust(r.QuickUnlockStatus v) =>
    QuickUnlockStatus(
      supported: v.supported,
      enabled: v.enabled,
      kind: QuickUnlockKind.values.byName(v.kind.name),
    );

VaultStatus statusFromRust(r.VaultStatus v) => VaultStatus(
  exists: v.exists,
  locked: v.locked,
  onboardingComplete: v.onboardingComplete,
  formatVersion: v.formatVersion,
  quickUnlock: quickUnlockFromRust(v.quickUnlock),
);

RecoveryKeyResult recoveryKeyFromRust(r.RecoveryKeyResult v) =>
    RecoveryKeyResult(
      recoveryKey: v.recoveryKey,
      groups: v.groups,
      challenge: List<int>.of(v.challenge),
    );

r.GroupAnswer groupAnswerToRust(GroupAnswer v) =>
    r.GroupAnswer(index: v.index, text: v.text);

// ------------------------------------------------------------------- items

ItemSummary summaryFromRust(r.ItemSummary v) => ItemSummary(
  id: v.id,
  type: itemTypeFromRust(v.itemType),
  title: v.title,
  subtitle: v.subtitle,
  favorite: v.favorite,
  folderId: v.folderId,
  tags: v.tags,
  updatedAt: v.updatedAt,
  cursor: v.cursor,
  hasTotp: v.hasTotp,
  deleted: v.deleted,
);

ItemView itemViewFromRust(r.ItemView v) => ItemView(
  id: v.id,
  type: itemTypeFromRust(v.itemType),
  title: v.title,
  folderId: v.folderId,
  favorite: v.favorite,
  tags: v.tags,
  urls: [for (final u in v.urls) UrlView(id: u.id, url: u.url)],
  custom: [
    for (final c in v.custom)
      CustomView(
        id: c.id,
        label: c.label,
        kind: customKindFromRust(c.kind),
        valueIfNotHidden: c.valueIfNotHidden,
      ),
  ],
  fields: {for (final e in v.fields.entries) fieldFromRust(e.key): e.value},
  secretFieldsPresent: {
    for (final f in v.secretFieldsPresent) fieldFromRust(f),
  },
  createdAt: v.createdAt,
  updatedAt: v.updatedAt,
  otherVersions: v.otherVersions,
);

r.NewItem newItemToRust(NewItem v) => r.NewItem(
  itemType: itemTypeToRust(v.type),
  title: v.title,
  fields: {for (final e in v.fields.entries) fieldToRust(e.key): e.value},
  urls: v.urls,
  tags: v.tags,
  folderId: v.folderId,
  custom: [
    for (final c in v.custom)
      r.NewCustom(
        kind: customKindToRust(c.kind),
        label: c.label,
        value: c.value,
      ),
  ],
);

r.ListFilter filterToRust(ListFilter v) => r.ListFilter(
  types: v.types?.map(itemTypeToRust).toList(),
  folderId: v.folderId,
  tag: v.tag,
  favoritesOnly: v.favoritesOnly,
  includeTrash: v.includeTrash,
);

r.Page pageToRust(Page v) =>
    r.Page(offset: v.offset, limit: v.limit, after: v.after);

r.SearchQuery searchToRust(SearchQuery v) => r.SearchQuery(
  text: v.text,
  filter: filterToRust(v.filter),
  page: pageToRust(v.page),
);

Folder folderFromRust(r.Folder v) =>
    Folder(id: v.id, name: v.name, parentId: v.parentId);

TrashEntry trashFromRust(r.TrashEntry v) => TrashEntry(
  summary: summaryFromRust(v.summary),
  deletedAt: v.deletedAt,
  purgesAt: v.purgesAt,
);

VersionInfo versionFromRust(r.VersionInfo v) => VersionInfo(
  field: fieldFromRust(v.field),
  hlcMs: v.hlcMs,
  atMs: v.atMs,
  deviceName: v.deviceName,
  isCurrent: v.isCurrent,
  concurrent: v.concurrent,
);

// ------------------------------------------------- generator, strength, health

r.PasswordOptions passwordOptionsToRust(PasswordOptions v) => r.PasswordOptions(
  length: v.length,
  lower: v.lower,
  upper: v.upper,
  digits: v.digits,
  symbols: v.symbols,
  symbolSet: v.symbolSet,
  excludeAmbiguous: v.excludeAmbiguous,
  requireEachClass: v.requireEachClass,
);

r.PassphraseOptions passphraseOptionsToRust(PassphraseOptions v) =>
    r.PassphraseOptions(
      wordCount: v.wordCount,
      separator: v.separator,
      capitalize: v.capitalize,
      numberSuffix: v.numberSuffix,
    );

r.EntropyOptions entropyToRust(EntropyOptions v) => switch (v) {
  PasswordEntropy(:final options) => r.EntropyOptions.password(
    passwordOptionsToRust(options),
  ),
  PassphraseEntropy(:final options) => r.EntropyOptions.passphrase(
    passphraseOptionsToRust(options),
  ),
};

Strength strengthFromRust(r.Strength v) =>
    Strength(score: v.score, feedback: v.feedback);

PolicyResult policyFromRust(r.PolicyResult v) =>
    PolicyResult(acceptable: v.acceptable, reasons: v.reasons);

HealthReport healthFromRust(r.HealthReport v) => HealthReport(
  reused: [for (final g in v.reused) ReuseGroup(itemIds: g.itemIds)],
  weak: [
    for (final w in v.weak) WeakPassword(itemId: w.itemId, score: w.score),
  ],
  old: [
    for (final o in v.old) OldPassword(itemId: o.itemId, ageDays: o.ageDays),
  ],
);

// ---------------------------------------------------------- import / export

ImportPreview previewFromRust(r.ImportPreview v) => ImportPreview(
  format: v.format,
  itemCount: v.itemCount,
  folderCount: v.folderCount,
  duplicates: v.duplicates,
  warnings: [
    for (final w in v.warnings)
      ImportWarning(record: w.record, message: w.message),
  ],
  skipped: [
    for (final s in v.skipped)
      SkippedRecord(record: s.record, reason: s.reason),
  ],
  previewToken: v.previewToken,
);

r.ImportOptions importOptionsToRust(ImportOptions v) => r.ImportOptions(
  skipDuplicates: v.skipDuplicates,
  targetFolder: v.targetFolder,
);

ImportResult importResultFromRust(r.ImportResult v) => ImportResult(
  created: v.created,
  foldersCreated: v.foldersCreated,
  duplicates: v.duplicates,
  invalid: v.invalid,
  skipped: v.skipped,
);

// ----------------------------------------------------------------- settings

AppSettings settingsFromRust(r.AppSettings v) => AppSettings(
  autoLockMinutes: v.autoLockMinutes,
  lockOnSleep: v.lockOnSleep,
  lockOnScreenLock: v.lockOnScreenLock,
  clipboardClearSeconds: v.clipboardClearSeconds,
  blockScreenCapture: v.blockScreenCapture,
  revealHideSeconds: v.revealHideSeconds,
  vaultConfig: VaultConfig(
    historySensitive: v.vaultConfig.historySensitive,
    historyOther: v.vaultConfig.historyOther,
    trashDays: v.vaultConfig.trashDays,
    tombstoneDays: v.vaultConfig.tombstoneDays,
  ),
);

r.AppSettings settingsToRust(AppSettings v) => r.AppSettings(
  autoLockMinutes: v.autoLockMinutes,
  lockOnSleep: v.lockOnSleep,
  lockOnScreenLock: v.lockOnScreenLock,
  clipboardClearSeconds: v.clipboardClearSeconds,
  blockScreenCapture: v.blockScreenCapture,
  revealHideSeconds: v.revealHideSeconds,
  vaultConfig: r.VaultConfig(
    historySensitive: v.vaultConfig.historySensitive,
    historyOther: v.vaultConfig.historyOther,
    trashDays: v.vaultConfig.trashDays,
    tombstoneDays: v.vaultConfig.tombstoneDays,
  ),
);

InfoDto infoFromRust(r.InfoDto v) => InfoDto(
  coreVersion: v.coreVersion,
  apiVersion: v.apiVersion,
  formatVersion: v.formatVersion,
  schemaVersion: v.schemaVersion,
  sqlcipherSettings: [
    for (final p in v.sqlcipherSettings)
      PinnedSetting(key: p.key, value: p.value),
  ],
);
