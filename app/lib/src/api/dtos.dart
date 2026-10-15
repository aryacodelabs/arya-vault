// DTOs of docs/14 §3, mirroring core/crates/ffi/src/api/dto.rs. Field names
// are the contract's camelCase names. None of the list/summary DTOs carries a
// secret; DTOs that do (RecoveryKeyResult, GroupAnswer) redact in toString.
import 'dart:typed_data';

// ---------------------------------------------------------------- lifecycle

/// How expensive the Argon2id parameters of a new wrap are.
enum KdfProfile { low, default_, high }

/// Which platform mechanism quick unlock uses.
enum QuickUnlockKind {
  none,
  windowsHello,
  touchId,
  faceId,
  biometric,
  osKeyring,
}

class QuickUnlockStatus {
  const QuickUnlockStatus({
    required this.supported,
    required this.enabled,
    required this.kind,
  });

  static const QuickUnlockStatus unsupported = QuickUnlockStatus(
    supported: false,
    enabled: false,
    kind: QuickUnlockKind.none,
  );

  final bool supported;
  final bool enabled;
  final QuickUnlockKind kind;

  QuickUnlockStatus copyWith({bool? enabled}) => QuickUnlockStatus(
    supported: supported,
    enabled: enabled ?? this.enabled,
    kind: kind,
  );
}

class VaultStatus {
  const VaultStatus({
    required this.exists,
    required this.locked,
    required this.onboardingComplete,
    required this.formatVersion,
    required this.quickUnlock,
  });

  final bool exists;
  final bool locked;
  final bool onboardingComplete;
  final int formatVersion;
  final QuickUnlockStatus quickUnlock;
}

/// The recovery key, returned **once** by `createVault` /
/// `regenerateRecoveryKey` (docs/14 §4.1).
///
/// [challenge] is an addition the FFI crate already ships (A01 spec question
/// 1): the zero-based group indices `confirmRecoveryKey` will ask for. It is
/// not yet in docs/14; see the PR's "Spec questions".
class RecoveryKeyResult {
  RecoveryKeyResult({
    required this.recoveryKey,
    required this.groups,
    required this.challenge,
  });

  /// UTF-8 text `XXXXX-XXXXX-XXXXX-XXXXX-XXXXX-XXXXX-XX-CC`.
  final Uint8List recoveryKey;

  /// Number of groups in the displayed key (the last one is the checksum).
  final int groups;

  /// Zero-based indices of the groups the user must re-enter.
  final List<int> challenge;

  @override
  String toString() =>
      'RecoveryKeyResult(<redacted>, groups: $groups, challenge: $challenge)';
}

class GroupAnswer {
  const GroupAnswer({required this.index, required this.text});

  /// Zero-based group index.
  final int index;

  /// What the user typed.
  final String text;

  @override
  String toString() => 'GroupAnswer(index: $index, text: <redacted>)';
}

// -------------------------------------------------------------- vault data

enum ItemType { login, note, card, identity }

/// Exactly the core `StdField` set (core/crates/vault/src/model.rs).
enum StdField {
  title,
  username,
  password,
  totpSeed,
  notes,
  body,
  cardHolder,
  cardNumber,
  cardExpiry,
  cardCvv,
  cardPin,
  firstName,
  middleName,
  lastName,
  email,
  phone,
  address,
  ids,
}

enum CustomKind { text, hidden, url, date }

class ItemSummary {
  const ItemSummary({
    required this.id,
    required this.type,
    required this.title,
    required this.subtitle,
    required this.favorite,
    required this.tags,
    required this.updatedAt,
    required this.cursor,
    required this.hasTotp,
    required this.deleted,
    this.folderId,
  });

  final String id;
  final ItemType type;
  final String title;
  final String subtitle;
  final bool favorite;
  final String? folderId;
  final List<String> tags;
  final int updatedAt;

  /// Opaque position of this row in the list order; hand it back as `Page.after`
  /// (docs/14 section 4.2.1). Never parse or store it.
  final String cursor;
  final bool hasTotp;
  final bool deleted;
}

class UrlView {
  const UrlView({required this.id, required this.url});
  final String id;
  final String url;
}

class CustomView {
  const CustomView({
    required this.id,
    required this.label,
    required this.kind,
    this.valueIfNotHidden,
  });
  final String id;
  final String label;
  final CustomKind kind;
  final String? valueIfNotHidden;
}

class ItemView {
  const ItemView({
    required this.id,
    required this.type,
    required this.title,
    required this.favorite,
    required this.tags,
    required this.urls,
    required this.custom,
    required this.fields,
    required this.secretFieldsPresent,
    required this.createdAt,
    required this.updatedAt,
    required this.otherVersions,
    this.folderId,
  });

  final String id;
  final ItemType type;
  final String title;
  final String? folderId;
  final bool favorite;
  final List<String> tags;
  final List<UrlView> urls;
  final List<CustomView> custom;

  /// Non-secret standard fields only.
  final Map<StdField, String?> fields;
  final Set<StdField> secretFieldsPresent;
  final int createdAt;
  final int updatedAt;
  final bool otherVersions;
}

class NewCustom {
  const NewCustom({
    required this.kind,
    required this.label,
    required this.value,
  });
  final CustomKind kind;
  final String label;
  final String value;
}

class NewItem {
  const NewItem({
    required this.type,
    required this.title,
    this.fields = const <StdField, String>{},
    this.urls = const <String>[],
    this.tags = const <String>[],
    this.folderId,
    this.custom = const <NewCustom>[],
  });

  final ItemType type;
  final String title;
  final Map<StdField, String> fields;
  final List<String> urls;
  final List<String> tags;
  final String? folderId;
  final List<NewCustom> custom;
}

class ListFilter {
  const ListFilter({
    this.types,
    this.folderId,
    this.tag,
    this.favoritesOnly = false,
    this.includeTrash = false,
  });
  final List<ItemType>? types;
  final String? folderId;
  final String? tag;
  final bool favoritesOnly;
  final bool includeTrash;
}

/// `limit` must be at most 200 (docs/14 §3).
class Page {
  const Page({required this.offset, required this.limit, this.after});
  final int offset;
  final int limit;

  /// `list` only: rows strictly after this [ItemSummary.cursor]; [offset] must be 0.
  final String? after;
}

class SearchQuery {
  const SearchQuery({
    required this.text,
    this.filter = const ListFilter(),
    this.page = const Page(offset: 0, limit: 50),
  });
  final String text;
  final ListFilter filter;
  final Page page;
}

class Folder {
  const Folder({required this.id, required this.name, this.parentId});
  final String id;
  final String name;
  final String? parentId;
}

class TrashEntry {
  const TrashEntry({
    required this.summary,
    required this.deletedAt,
    required this.purgesAt,
  });
  final ItemSummary summary;
  final int deletedAt;
  final int purgesAt;
}

class VersionInfo {
  const VersionInfo({
    required this.field,
    required this.hlcMs,
    required this.atMs,
    required this.isCurrent,
    required this.concurrent,
    this.deviceName,
  });
  final StdField field;
  final int hlcMs;
  final int atMs;
  final String? deviceName;
  final bool isCurrent;
  final bool concurrent;
}

// ------------------------------------------------- generator, strength, health

class PasswordOptions {
  const PasswordOptions({
    this.length = 20,
    this.lower = true,
    this.upper = true,
    this.digits = true,
    this.symbols = true,
    this.symbolSet = r'!@#$%^&*()-_=+[]{};:,.?',
    this.excludeAmbiguous = false,
    this.requireEachClass = true,
  });
  final int length;
  final bool lower;
  final bool upper;
  final bool digits;
  final bool symbols;
  final String symbolSet;
  final bool excludeAmbiguous;
  final bool requireEachClass;
}

class PassphraseOptions {
  const PassphraseOptions({
    this.wordCount = 6,
    this.separator = '-',
    this.capitalize = false,
    this.numberSuffix = false,
  });
  final int wordCount;
  final String separator;
  final bool capitalize;
  final bool numberSuffix;
}

/// Options for `entropyBits` (password or passphrase).
sealed class EntropyOptions {
  const EntropyOptions();
}

class PasswordEntropy extends EntropyOptions {
  const PasswordEntropy(this.options);
  final PasswordOptions options;
}

class PassphraseEntropy extends EntropyOptions {
  const PassphraseEntropy(this.options);
  final PassphraseOptions options;
}

class Strength {
  const Strength({required this.score, required this.feedback});

  /// 0 (trivial) to 4 (strong).
  final int score;
  final List<String> feedback;
}

class PolicyResult {
  const PolicyResult({required this.acceptable, required this.reasons});
  final bool acceptable;
  final List<String> reasons;
}

class ReuseGroup {
  const ReuseGroup({required this.itemIds});
  final List<String> itemIds;
}

class WeakPassword {
  const WeakPassword({required this.itemId, required this.score});
  final String itemId;
  final int score;
}

class OldPassword {
  const OldPassword({required this.itemId, required this.ageDays});
  final String itemId;
  final int ageDays;
}

class HealthReport {
  const HealthReport({
    required this.reused,
    required this.weak,
    required this.old,
  });
  final List<ReuseGroup> reused;
  final List<WeakPassword> weak;
  final List<OldPassword> old;
}

// ---------------------------------------------------------- import / export

enum ImportFormat { csv, bitwardenJson }

class SkippedRecord {
  const SkippedRecord({required this.record, required this.reason});
  final int record;
  final String reason;
}

class ImportWarning {
  const ImportWarning({required this.record, required this.message});
  final int record;
  final String message;
}

class ImportPreview {
  const ImportPreview({
    required this.format,
    required this.itemCount,
    required this.folderCount,
    required this.duplicates,
    required this.warnings,
    required this.skipped,
    required this.previewToken,
  });
  final String format;
  final int itemCount;
  final int folderCount;
  final int duplicates;
  final List<ImportWarning> warnings;
  final List<SkippedRecord> skipped;
  final String previewToken;
}

class ImportOptions {
  const ImportOptions({this.skipDuplicates = true, this.targetFolder});
  final bool skipDuplicates;
  final String? targetFolder;
}

class ImportResult {
  const ImportResult({
    required this.created,
    required this.foldersCreated,
    required this.duplicates,
    required this.invalid,
    required this.skipped,
  });
  final int created;
  final int foldersCreated;
  final int duplicates;
  final int invalid;
  final int skipped;
}

/// The user has been told a CSV export is unencrypted and confirmed.
class AcknowledgePlaintextRisk {
  const AcknowledgePlaintextRisk({required this.acknowledged});
  final bool acknowledged;
}

// ----------------------------------------------------------------- settings

class VaultConfig {
  const VaultConfig({
    this.historySensitive = 20,
    this.historyOther = 5,
    this.trashDays = 30,
    this.tombstoneDays = 180,
  });
  final int historySensitive;
  final int historyOther;
  final int trashDays;
  final int tombstoneDays;
}

class AppSettings {
  const AppSettings({
    this.autoLockMinutes = 5,
    this.lockOnSleep = true,
    this.lockOnScreenLock = true,
    this.clipboardClearSeconds = 30,
    this.blockScreenCapture = true,
    this.revealHideSeconds = 15,
    this.vaultConfig = const VaultConfig(),
  });
  final int autoLockMinutes;
  final bool lockOnSleep;
  final bool lockOnScreenLock;
  final int clipboardClearSeconds;
  final bool blockScreenCapture;
  final int revealHideSeconds;
  final VaultConfig vaultConfig;

  AppSettings copyWith({
    int? autoLockMinutes,
    bool? lockOnSleep,
    bool? lockOnScreenLock,
    int? clipboardClearSeconds,
    bool? blockScreenCapture,
    int? revealHideSeconds,
    VaultConfig? vaultConfig,
  }) => AppSettings(
    autoLockMinutes: autoLockMinutes ?? this.autoLockMinutes,
    lockOnSleep: lockOnSleep ?? this.lockOnSleep,
    lockOnScreenLock: lockOnScreenLock ?? this.lockOnScreenLock,
    clipboardClearSeconds: clipboardClearSeconds ?? this.clipboardClearSeconds,
    blockScreenCapture: blockScreenCapture ?? this.blockScreenCapture,
    revealHideSeconds: revealHideSeconds ?? this.revealHideSeconds,
    vaultConfig: vaultConfig ?? this.vaultConfig,
  );
}

// -------------------------------------------------------------- diagnostics

class PinnedSetting {
  const PinnedSetting({required this.key, this.value});
  final String key;
  final String? value;
}

class InfoDto {
  const InfoDto({
    required this.coreVersion,
    required this.apiVersion,
    required this.formatVersion,
    required this.schemaVersion,
    required this.sqlcipherSettings,
  });
  final String coreVersion;
  final int apiVersion;
  final int formatVersion;
  final int schemaVersion;
  final List<PinnedSetting> sqlcipherSettings;
}
