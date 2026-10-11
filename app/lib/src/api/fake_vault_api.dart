// In-memory VaultApi for development and tests (B01). NOT secure, NOT a
// vault: it stores passwords in plain memory, "encrypts" nothing, and its
// strength estimate and recovery-key checksum are toy heuristics. It exists so
// the UI is buildable and testable before the FFI (A04) is wired in (L01).
//
// It reproduces the contract's observable behavior: the lifecycle state
// machine (docs/14 §5), wrongCredentials, the master-password policy
// (min 12), the failure backoff (5 free wrong passwords, then busy with a
// doubling delay, like core/crates/session/src/backoff.rs), a pending recovery
// key that is confirmed through 3 groups, and 2,000 sample items.
import 'dart:convert';
import 'dart:math';
import 'dart:typed_data';

import 'app_error.dart';
import 'dtos.dart';
import 'vault_api.dart';

const String _crockford = '0123456789ABCDEFGHJKMNPQRSTVWXYZ';
const int _keyChars = 32;
const int _keyGroups = 8;

/// The 3 groups of the confirmation are drawn from the first 6 (5-char) groups,
/// like `CONFIRMATION_POOL` / `CONFIRMATION_GROUPS` in the core.
const int _confirmationPool = 6;
const int _confirmationGroups = 3;

const int _minPasswordChars = 12;
const int _minPasswordScore = 2;

/// Sample secrets are obvious fakes (CLAUDE.md rule 4).
const String _canary = 'CANARY';

class _FakeCustom {
  _FakeCustom(this.id, this.kind, this.label, this.value);
  final String id;
  final CustomKind kind;
  String label;
  String value;
}

class _FakeItem {
  _FakeItem({
    required this.id,
    required this.type,
    required this.title,
    required this.createdAt,
  }) : updatedAt = createdAt;

  final String id;
  final ItemType type;
  String title;
  String? folderId;
  bool favorite = false;
  final List<String> tags = <String>[];
  final Map<String, String> urls = <String, String>{};
  final List<_FakeCustom> custom = <_FakeCustom>[];
  final Map<StdField, String> fields = <StdField, String>{};
  final Map<StdField, List<(int, String)>> versions =
      <StdField, List<(int, String)>>{};

  /// Versions (by hlc) that lost to the current one but are not its ancestors
  /// (docs/06 §5.3). A derived view in the real core; seeded here.
  final Map<StdField, Set<int>> concurrent = <StdField, Set<int>>{};
  final int createdAt;
  int updatedAt;
  int? deletedAt;
}

class _FakeImportItem {
  const _FakeImportItem(
    this.title,
    this.username,
    this.password,
    this.url,
    this.notes,
    this.folder,
  );
  final String title;
  final String username;
  final String password;
  final String url;
  final String notes;
  final String folder;
}

class _FakePreview {
  const _FakePreview(
    this.format,
    this.items,
    this.skipped,
    this.warnings,
    this.folders,
  );
  final String format;
  final List<_FakeImportItem> items;
  final List<SkippedRecord> skipped;
  final List<ImportWarning> warnings;
  final Set<String> folders;
}

class FakeVaultApi implements VaultApi {
  FakeVaultApi({
    DateTime Function()? clock,
    Random? random,
    QuickUnlockStatus quickUnlock = QuickUnlockStatus.unsupported,
    bool seedSampleItems = true,
    int sampleItemCount = 2000,
    int sampleTrashCount = 5,
    this.freeAttempts = 5,
  }) : _clock = clock ?? DateTime.now,
       _random = random ?? Random.secure(),
       _quick = quickUnlock {
    if (seedSampleItems) {
      _seedSamples(sampleItemCount, sampleTrashCount);
    }
  }

  /// Wrong passwords tolerated before the first delay (core default: 5).
  final int freeAttempts;

  final DateTime Function() _clock;
  final Random _random;

  // ---------------------------------------------------------- lifecycle state
  bool _exists = false;
  bool _locked = true;
  bool _onboardingComplete = false;
  Uint8List _password = Uint8List(0);
  String _recoveryKey = ''; // canonical, no hyphens
  String? _pendingKey; // hyphenated display form, dropped on lock/confirm
  List<int> _challenge = <int>[];
  int _failures = 0;
  DateTime? _blockedUntil;
  QuickUnlockStatus _quick;
  AppSettings _settings = const AppSettings();
  int _nextId = 0;

  /// Test hook: make `unlockQuick` fail with `quickUnlockUnavailable`.
  bool quickUnlockUsable = true;

  /// Test hook: the profile passed to the last `createVault`.
  KdfProfile? lastProfile;

  /// Test hook: how many times `confirmRecoveryKey` returned true.
  int confirmSuccessCount = 0;

  final Map<String, _FakeItem> _items = <String, _FakeItem>{};

  /// Bumped by every change to items, so the sorted view can be cached (the
  /// 20,000-item mode pages and searches it constantly).
  int _version = 0;
  int _sortedVersion = -1;
  List<_FakeItem> _sorted = <_FakeItem>[];

  void _bump() => _version++;

  /// All items, newest first like the real core (`ORDER BY updated_hlc DESC, id`).
  List<_FakeItem> get _byNewest {
    if (_sortedVersion != _version) {
      _sorted = _items.values.toList()
        ..sort((_FakeItem a, _FakeItem b) {
          final int byTime = b.updatedAt.compareTo(a.updatedAt);
          return byTime != 0 ? byTime : a.id.compareTo(b.id);
        });
      _sortedVersion = _version;
    }
    return _sorted;
  }

  final Map<String, Folder> _folders = <String, Folder>{};

  // ------------------------------------------------------------------ helpers

  int get _nowMs => _clock().millisecondsSinceEpoch;

  void _requireUnlocked() {
    if (!_exists || _locked) {
      throw const AppError(AppErrorCode.locked, 'the vault is locked');
    }
  }

  bool _passwordEquals(Uint8List candidate) {
    if (candidate.length != _password.length) {
      return false;
    }
    var diff = 0;
    for (var i = 0; i < candidate.length; i++) {
      diff |= candidate[i] ^ _password[i];
    }
    return diff == 0;
  }

  void _checkPolicy(Uint8List password) {
    final PolicyResult result = _policy(password);
    if (!result.acceptable) {
      throw AppError(AppErrorCode.weakPassword, result.reasons.join(' '));
    }
  }

  void _setPassword(Uint8List password) {
    _password = Uint8List.fromList(password);
  }

  String _newId() {
    _nextId++;
    return _nextId.toRadixString(16).padLeft(32, '0');
  }

  Duration _delayAfter(int failures) {
    if (failures < freeAttempts) {
      return Duration.zero;
    }
    final int exp = min(failures - freeAttempts, 20);
    return Duration(seconds: min(1 << exp, 60));
  }

  // -------------------------------------------------------- recovery key text

  /// FAKE checksum (the real one is a SHA-256 prefix, docs/04 §4).
  static String _checksum(String chars) {
    var sum = 0;
    for (var i = 0; i < chars.length; i++) {
      sum += (_crockford.indexOf(chars[i]) + 1) * (i + 1);
    }
    sum %= 1024;
    return _crockford[sum >> 5] + _crockford[sum & 31];
  }

  String _generateKeyText() {
    final StringBuffer raw = StringBuffer();
    for (var i = 0; i < _keyChars; i++) {
      raw.write(_crockford[_random.nextInt(32)]);
    }
    final String chars = raw.toString();
    final List<String> groups = <String>[
      for (var g = 0; g < 6; g++) chars.substring(g * 5, g * 5 + 5),
      chars.substring(30),
      _checksum(chars),
    ];
    return groups.join('-');
  }

  /// Crockford normalization as the real parser and `confirm` do.
  static String _normalize(String text) {
    final StringBuffer out = StringBuffer();
    for (final int rune in text.runes) {
      final String c = String.fromCharCode(rune);
      if (c == '-' || c.trim().isEmpty) {
        continue;
      }
      final String up = c.toUpperCase();
      out.write(switch (up) {
        'I' || 'L' => '1',
        'O' => '0',
        _ => up,
      });
    }
    return out.toString();
  }

  RecoveryKeyResult _startPending() {
    final String text = _generateKeyText();
    _pendingKey = text;
    _recoveryKey = _normalize(text).substring(0, _keyChars);
    // Contract text: a failed confirmation re-shows the key and asks again.
    // (The real core also redraws the groups on a mismatch and the FFI has no
    // call to fetch them; this fake keeps them. See the PR "Spec questions".)
    final List<int> pool = <int>[for (var i = 0; i < _confirmationPool; i++) i]
      ..shuffle(_random);
    _challenge = pool.take(_confirmationGroups).toList()..sort();
    _onboardingComplete = false;
    return RecoveryKeyResult(
      recoveryKey: Uint8List.fromList(utf8.encode(text)),
      groups: _keyGroups,
      challenge: List<int>.unmodifiable(_challenge),
    );
  }

  // ------------------------------------------------------ 4.1 lifecycle

  @override
  Future<VaultStatus> status() async => VaultStatus(
    exists: _exists,
    locked: _locked,
    onboardingComplete: _onboardingComplete,
    formatVersion: _exists ? 1 : 0,
    quickUnlock: _quick,
  );

  @override
  Future<RecoveryKeyResult> createVault({
    required Uint8List password,
    required KdfProfile profile,
  }) async {
    if (_exists) {
      throw const AppError(
        AppErrorCode.alreadyExists,
        'a vault already exists',
      );
    }
    _checkPolicy(password);
    lastProfile = profile;
    _setPassword(password);
    final RecoveryKeyResult result = _startPending();
    _exists = true;
    _locked = false;
    _failures = 0;
    _blockedUntil = null;
    return result;
  }

  @override
  Future<bool> confirmRecoveryKey(List<GroupAnswer> answers) async {
    _requireUnlocked();
    final String? pending = _pendingKey;
    if (pending == null) {
      throw const AppError(
        AppErrorCode.validation,
        'there is no recovery key to confirm',
        field: 'answers',
      );
    }
    final List<int> indices = answers.map((GroupAnswer a) => a.index).toList()
      ..sort();
    if (indices.length != _challenge.length ||
        !List<bool>.generate(
          indices.length,
          (int i) => indices[i] == _challenge[i],
        ).every((bool b) => b)) {
      throw const AppError(
        AppErrorCode.validation,
        'the answers do not cover the requested groups',
        field: 'answers',
      );
    }
    final List<String> groups = pending.split('-');
    var allMatch = true;
    for (final GroupAnswer a in answers) {
      if (_normalize(a.text) != groups[a.index]) {
        allMatch = false;
      }
    }
    if (!allMatch) {
      return false;
    }
    _pendingKey = null;
    _onboardingComplete = true;
    confirmSuccessCount++;
    return true;
  }

  @override
  Future<void> unlock(Uint8List password) async {
    if (!_exists) {
      throw const AppError(AppErrorCode.notFound, 'there is no vault');
    }
    final DateTime blocked =
        _blockedUntil ?? DateTime.fromMillisecondsSinceEpoch(0);
    if (_clock().isBefore(blocked)) {
      throw const AppError(
        AppErrorCode.busy,
        'too many attempts; wait before trying again',
      );
    }
    if (!_passwordEquals(password)) {
      _failures++;
      final Duration delay = _delayAfter(_failures);
      if (delay > Duration.zero) {
        _blockedUntil = _clock().add(delay);
      }
      throw const AppError(
        AppErrorCode.wrongCredentials,
        'wrong master password or recovery key',
      );
    }
    _failures = 0;
    _blockedUntil = null;
    _locked = false;
  }

  @override
  Future<void> unlockQuick() async {
    if (!_exists) {
      throw const AppError(AppErrorCode.notFound, 'there is no vault');
    }
    if (!_quick.enabled || !quickUnlockUsable) {
      throw const AppError(
        AppErrorCode.quickUnlockUnavailable,
        'quick unlock is not available',
      );
    }
    _locked = false;
  }

  @override
  Future<void> lock() async {
    if (!_exists) {
      return;
    }
    _locked = true;
    _pendingKey = null; // the core drops the pending key on lock
  }

  @override
  Future<void> changePassword({
    required Uint8List oldPassword,
    required Uint8List newPassword,
    required bool rotateKeys,
  }) async {
    _requireUnlocked();
    if (!_passwordEquals(oldPassword)) {
      throw const AppError(
        AppErrorCode.wrongCredentials,
        'wrong master password or recovery key',
      );
    }
    _checkPolicy(newPassword);
    _setPassword(newPassword);
    if (rotateKeys) {
      // Like the core (ffi `change_password`): the rotation leaves a new
      // recovery key pending and onboarding incomplete until it is confirmed.
      _startPending();
    }
  }

  @override
  Future<void> recoverWithKey({
    required Uint8List recoveryKey,
    required Uint8List newPassword,
  }) async {
    if (!_exists) {
      throw const AppError(AppErrorCode.notFound, 'there is no vault');
    }
    final String typed = _normalize(
      utf8.decode(recoveryKey, allowMalformed: true),
    );
    final bool wellFormed =
        typed.length == _keyChars + 2 &&
        typed.split('').every((String c) => _crockford.contains(c)) &&
        _checksum(typed.substring(0, _keyChars)) == typed.substring(_keyChars);
    if (!wellFormed) {
      throw const AppError(
        AppErrorCode.recoveryKeyMalformed,
        'the recovery key is malformed',
      );
    }
    if (typed.substring(0, _keyChars) != _recoveryKey) {
      throw const AppError(
        AppErrorCode.wrongCredentials,
        'wrong master password or recovery key',
      );
    }
    _checkPolicy(newPassword);
    _setPassword(newPassword);
    _failures = 0;
    _blockedUntil = null;
    _locked = false;
  }

  @override
  Future<RecoveryKeyResult> regenerateRecoveryKey({
    required Uint8List password,
    required bool rotateKeys,
  }) async {
    _requireUnlocked();
    if (!_passwordEquals(password)) {
      throw const AppError(
        AppErrorCode.wrongCredentials,
        'wrong master password or recovery key',
      );
    }
    return _startPending();
  }

  @override
  Future<void> quickUnlockEnable() async {
    _requireUnlocked();
    if (!_quick.supported) {
      throw const AppError(
        AppErrorCode.quickUnlockUnavailable,
        'quick unlock is not supported here',
      );
    }
    _quick = _quick.copyWith(enabled: true);
  }

  @override
  Future<void> quickUnlockDisable() async {
    _requireUnlocked();
    _quick = _quick.copyWith(enabled: false);
  }

  @override
  Future<bool> verifyPassword(Uint8List password) async {
    _requireUnlocked();
    return _passwordEquals(password);
  }

  // ------------------------------------------------ 4.3 strength / generator

  static double _poolBits(String text) {
    var pool = 0;
    if (RegExp('[a-z]').hasMatch(text)) pool += 26;
    if (RegExp('[A-Z]').hasMatch(text)) pool += 26;
    if (RegExp('[0-9]').hasMatch(text)) pool += 10;
    if (RegExp(r'[!-/:-@\[-`{-~]').hasMatch(text)) pool += 33;
    if (text.runes.any((int r) => r > 126)) pool += 100;
    return pool == 0 ? 0 : log(pool) / ln2;
  }

  /// FAKE zxcvbn stand-in: score 0..4 from distinct characters and classes.
  static int _score(String text) {
    final int len = text.runes.length;
    if (len == 0) {
      return 0;
    }
    final int distinct = text.runes.toSet().length;
    final double effective = distinct + (len - distinct) * 0.2;
    final double bits = effective * _poolBits(text) * 0.7;
    var score = bits < 28
        ? 0
        : bits < 36
        ? 1
        : bits < 60
        ? 2
        : bits < 80
        ? 3
        : 4;
    final String lower = text.toLowerCase();
    const List<String> common = <String>[
      'password',
      'qwerty',
      '123456',
      'letmein',
      'iloveyou',
      'abc123',
      'admin',
    ];
    if (common.any(lower.contains)) {
      score = min(score, 1);
    }
    return score;
  }

  static PolicyResult _policy(Uint8List password) {
    final String text = utf8.decode(password, allowMalformed: true);
    final int chars = text.runes.length;
    final int score = _score(text);
    final List<String> reasons = <String>[
      if (chars < _minPasswordChars)
        'master password must be at least $_minPasswordChars characters (got $chars)',
      if (score < _minPasswordScore)
        'master password is too easy to guess (strength $score of 4)',
    ];
    return PolicyResult(acceptable: reasons.isEmpty, reasons: reasons);
  }

  @override
  Future<PolicyResult> checkMasterPassword(Uint8List password) async =>
      _policy(password);

  @override
  Future<Strength> strength(Uint8List password) async {
    final String text = utf8.decode(password, allowMalformed: true);
    final int score = _score(text);
    return Strength(
      score: score,
      feedback: score >= 3
          ? const <String>[]
          : const <String>['Add more words or characters.'],
    );
  }

  String _randomFrom(String alphabet) =>
      alphabet[_random.nextInt(alphabet.length)];

  @override
  Future<Uint8List> generatePassword(PasswordOptions options) async {
    final List<String> classes = <String>[
      if (options.lower) 'abcdefghijkmnopqrstuvwxyz',
      if (options.upper) 'ABCDEFGHJKLMNPQRSTUVWXYZ',
      if (options.digits) '23456789',
      if (options.symbols && options.symbolSet.isNotEmpty) options.symbolSet,
    ];
    if (classes.isEmpty || options.length < 8 || options.length > 128) {
      throw const AppError(
        AppErrorCode.validation,
        'invalid generator options',
        field: 'options',
      );
    }
    final String all = classes.join();
    final List<String> chars = <String>[
      if (options.requireEachClass) ...classes.map(_randomFrom),
    ];
    while (chars.length < options.length) {
      chars.add(_randomFrom(all));
    }
    chars.shuffle(_random);
    return Uint8List.fromList(utf8.encode(chars.take(options.length).join()));
  }

  @override
  Future<Uint8List> generatePassphrase(PassphraseOptions options) async {
    const List<String> words = <String>[
      'amber',
      'brook',
      'cedar',
      'delta',
      'ember',
      'fjord',
      'grove',
      'harbor',
      'island',
      'jungle',
      'kernel',
      'lantern',
      'meadow',
      'nectar',
      'orchid',
      'pebble',
      'quartz',
      'river',
      'summit',
      'timber',
      'umber',
      'valley',
      'willow',
      'yonder',
      'zephyr',
    ];
    if (options.wordCount < 3 || options.wordCount > 20) {
      throw const AppError(
        AppErrorCode.validation,
        'invalid generator options',
        field: 'options',
      );
    }
    final List<String> picked = <String>[
      for (var i = 0; i < options.wordCount; i++)
        words[_random.nextInt(words.length)],
    ];
    final String joined = picked
        .map(
          (String w) =>
              options.capitalize ? w[0].toUpperCase() + w.substring(1) : w,
        )
        .join(options.separator);
    final String out = options.numberSuffix
        ? '$joined${options.separator}${_random.nextInt(100)}'
        : joined;
    return Uint8List.fromList(utf8.encode(out));
  }

  @override
  Future<double> entropyBits(EntropyOptions options) async => switch (options) {
    PasswordEntropy(:final PasswordOptions options) =>
      options.length *
          max(
            1.0,
            log(
                  (options.lower ? 26 : 0) +
                      (options.upper ? 26 : 0) +
                      (options.digits ? 10 : 0) +
                      (options.symbols ? options.symbolSet.length : 0) +
                      0.0,
                ) /
                ln2,
          ),
    PassphraseEntropy(:final PassphraseOptions options) =>
      options.wordCount * (log(7776) / ln2),
  };

  // ------------------------------------------------- 4.2 items and folders

  void _seedSamples(int count, int trashCount) {
    const List<String> sites = <String>[
      'Aurora',
      'Beacon',
      'Cobalt',
      'Dune',
      'Echo',
      'Falcon',
      'Garnet',
      'Horizon',
      'Indigo',
      'Juniper',
      'Kestrel',
      'Lumen',
      'Mosaic',
      'Nimbus',
      'Onyx',
      'Prism',
      'Quill',
      'Raven',
      'Sable',
      'Tundra',
    ];
    const List<String> suffixes = <String>[
      'Mail',
      'Bank',
      'Cloud',
      'Shop',
      'Social',
      'Docs',
      'Cards',
      'Travel',
    ];
    final List<String> folderIds = <String>[
      for (final String name in <String>['Work', 'Personal', 'Finance'])
        _addFolder(name),
    ];
    final DateTime base = DateTime.utc(2026, 1, 1);
    final int baseMs = base.millisecondsSinceEpoch;
    for (var i = 0; i < count + trashCount; i++) {
      final bool trashed = i >= count;
      final ItemType type = switch (i % 10) {
        0 || 1 => ItemType.note,
        2 => ItemType.card,
        3 => ItemType.identity,
        _ => ItemType.login,
      };
      final String title =
          '${sites[i % sites.length]} ${suffixes[(i ~/ sites.length) % suffixes.length]} $i';
      final _FakeItem item = _FakeItem(
        id: _newId(),
        type: type,
        title: title,
        createdAt: baseMs + i * 60000,
      );
      item.updatedAt = baseMs + i * 60000 + 1000;
      item.fields[StdField.title] = title;
      switch (type) {
        case ItemType.login:
          item.fields[StdField.username] = 'user$i@example.invalid';
          // Every 50th login reuses one password so the health report has groups.
          item.fields[StdField.password] = i % 50 == 4
              ? '$_canary-shared-password'
              : '$_canary-pw-$i-Xk9!';
          item.urls[_newId()] = 'https://example.invalid/site/$i';
          if (i % 7 == 0) {
            item.fields[StdField.totpSeed] = '$_canary-totp-$i';
          }
          if (i % 11 == 0) {
            item.fields[StdField.notes] = 'Sample note for item $i';
          }
        case ItemType.note:
          item.fields[StdField.body] =
              '# $_canary note $i\n\nSome **bold** text and a list:\n\n- one\n- two\n';
        case ItemType.card:
          item.fields[StdField.cardHolder] = 'Sample Holder $i';
          item.fields[StdField.cardNumber] = '$_canary-0000-0000-0000';
          item.fields[StdField.cardExpiry] = '12/30';
        case ItemType.identity:
          item.fields[StdField.firstName] = 'Sample';
          item.fields[StdField.lastName] = 'Person $i';
      }
      item.favorite = i % 40 == 0;
      if (i % 9 == 0) item.tags.add('work');
      if (i % 13 == 0) item.tags.add('personal');
      if (i % 6 < 3) item.folderId = folderIds[i % 6];
      if (type == ItemType.login && i % 30 == 5) {
        // A little password history: two older values, then the current one.
        final String current = item.fields[StdField.password]!;
        item.versions[StdField.password] = <(int, String)>[
          (item.updatedAt - 2 * 86400000, '$_canary-old-pw-$i-1'),
          (item.updatedAt - 86400000, '$_canary-old-pw-$i-2'),
          (item.updatedAt, current),
        ];
        if (i % 90 == 5) {
          final int hlc = item.updatedAt - 43200000;
          item.versions[StdField.password]!.insert(2, (
            hlc,
            '$_canary-other-device-pw-$i',
          ));
          item.concurrent[StdField.password] = <int>{hlc};
        }
      }
      if (trashed) {
        item.deletedAt = _nowMs - (i - count) * 86400000;
      }
      _items[item.id] = item;
    }
    _bump();
  }

  String _addFolder(String name) {
    final String id = _newId();
    _folders[id] = Folder(id: id, name: name);
    return id;
  }

  /// Test hook: records a version of [field] on [id] made "on another device"
  /// that is concurrent with the current one (docs/06 §5.3). Returns its hlc.
  int seedConcurrentVersion(String id, StdField field, String value) {
    final _FakeItem i = _item(id);
    final int hlc = _nowMs - 1000;
    final List<(int, String)> v = i.versions.putIfAbsent(
      field,
      () => <(int, String)>[
        if (i.fields[field] != null) (i.updatedAt, i.fields[field]!),
      ],
    );
    v.insert(v.isEmpty ? 0 : v.length - 1, (hlc, value));
    i.concurrent.putIfAbsent(field, () => <int>{}).add(hlc);
    _bump();
    return hlc;
  }

  static const Set<StdField> _secretFields = <StdField>{
    StdField.password,
    StdField.totpSeed,
    StdField.body,
    StdField.cardNumber,
    StdField.cardCvv,
    StdField.cardPin,
    StdField.ids,
  };

  _FakeItem _item(String id) {
    final _FakeItem? item = _items[id];
    if (item == null) {
      throw const AppError(AppErrorCode.notFound, 'unknown item');
    }
    return item;
  }

  ItemSummary _summary(_FakeItem i) {
    final String subtitle =
        i.fields[StdField.username] ??
        i.fields[StdField.cardHolder] ??
        i.fields[StdField.firstName] ??
        '';
    return ItemSummary(
      id: i.id,
      type: i.type,
      title: i.title,
      subtitle: subtitle,
      favorite: i.favorite,
      folderId: i.folderId,
      tags: List<String>.unmodifiable(i.tags),
      updatedAt: i.updatedAt,
      hasTotp: i.fields.containsKey(StdField.totpSeed),
      deleted: i.deletedAt != null,
    );
  }

  bool _matches(_FakeItem i, ListFilter f) {
    if (!f.includeTrash && i.deletedAt != null) return false;
    if (f.types != null && !f.types!.contains(i.type)) return false;
    if (f.folderId != null && i.folderId != f.folderId) return false;
    if (f.tag != null && !i.tags.contains(f.tag)) return false;
    if (f.favoritesOnly && !i.favorite) return false;
    return true;
  }

  /// [items] must already be in newest-first order.
  List<ItemSummary> _paged(Iterable<_FakeItem> items, Page page) {
    if (page.limit > 200 || page.limit < 1 || page.offset < 0) {
      throw const AppError(
        AppErrorCode.validation,
        'page limit must be 1..200',
        field: 'page',
      );
    }
    return items
        .skip(page.offset)
        .take(page.limit)
        .map(_summary)
        .toList(growable: false);
  }

  @override
  Future<List<ItemSummary>> list(ListFilter filter, Page page) async {
    _requireUnlocked();
    return _paged(_byNewest.where((_FakeItem i) => _matches(i, filter)), page);
  }

  @override
  Future<List<ItemSummary>> search(SearchQuery query) async {
    _requireUnlocked();
    final String needle = query.text.toLowerCase();
    // Like the real core: prefix search over non-secret fields; the trash is
    // never searched.
    return _paged(
      _byNewest.where(
        (_FakeItem i) =>
            i.deletedAt == null &&
            _matches(i, query.filter) &&
            (needle.isEmpty ||
                i.title.toLowerCase().contains(needle) ||
                (i.fields[StdField.username]?.toLowerCase().contains(needle) ??
                    false)),
      ),
      query.page,
    );
  }

  @override
  Future<int> itemCount() async {
    _requireUnlocked();
    return _items.values.where((_FakeItem i) => i.deletedAt == null).length;
  }

  @override
  Future<ItemView> getItem(String id) async {
    _requireUnlocked();
    final _FakeItem i = _item(id);
    return ItemView(
      id: i.id,
      type: i.type,
      title: i.title,
      folderId: i.folderId,
      favorite: i.favorite,
      tags: List<String>.unmodifiable(i.tags),
      urls: <UrlView>[
        for (final MapEntry<String, String> e in i.urls.entries)
          UrlView(id: e.key, url: e.value),
      ],
      custom: <CustomView>[
        for (final _FakeCustom c in i.custom)
          CustomView(
            id: c.id,
            label: c.label,
            kind: c.kind,
            valueIfNotHidden: c.kind == CustomKind.hidden ? null : c.value,
          ),
      ],
      fields: <StdField, String?>{
        for (final MapEntry<StdField, String> e in i.fields.entries)
          if (!_secretFields.contains(e.key)) e.key: e.value,
      },
      secretFieldsPresent: <StdField>{
        for (final StdField f in i.fields.keys)
          if (_secretFields.contains(f)) f,
      },
      createdAt: i.createdAt,
      updatedAt: i.updatedAt,
      otherVersions: i.concurrent.values.any((Set<int> s) => s.isNotEmpty),
    );
  }

  Uint8List? _bytes(String? s) =>
      s == null ? null : Uint8List.fromList(utf8.encode(s));

  @override
  Future<Uint8List?> reveal(String id, StdField field) async {
    _requireUnlocked();
    final _FakeItem i = _item(id);
    if (!_secretFields.contains(field)) {
      throw AppError(
        AppErrorCode.validation,
        'field is not secret',
        field: field.name,
      );
    }
    return _bytes(i.fields[field]);
  }

  @override
  Future<Uint8List?> revealCustom(String id, String elementId) async {
    _requireUnlocked();
    final _FakeItem i = _item(id);
    final _FakeCustom c = i.custom.firstWhere(
      (_FakeCustom c) => c.id == elementId,
      orElse: () =>
          throw const AppError(AppErrorCode.notFound, 'unknown field'),
    );
    return _bytes(c.value);
  }

  /// Sets [field] and records the history the way the core does: the value
  /// being replaced becomes a version the first time a field is edited.
  void _setValue(_FakeItem i, StdField field, String value) {
    final String? old = i.fields[field];
    final List<(int, String)> v = i.versions.putIfAbsent(
      field,
      () => <(int, String)>[],
    );
    if (v.isEmpty && old != null) {
      v.add((i.updatedAt, old));
    }
    i.fields[field] = value;
    if (field == StdField.title) i.title = value;
    i.updatedAt = max(_nowMs, i.updatedAt + 1);
    v.add((i.updatedAt, value));
    _bump();
  }

  void _touch(_FakeItem i) {
    i.updatedAt = max(_nowMs, i.updatedAt + 1);
    _bump();
  }

  @override
  Future<String> createItem(NewItem item) async {
    _requireUnlocked();
    if (item.title.isEmpty) {
      throw const AppError(
        AppErrorCode.validation,
        'title is required',
        field: 'title',
      );
    }
    final _FakeItem i =
        _FakeItem(
            id: _newId(),
            type: item.type,
            title: item.title,
            createdAt: _nowMs,
          )
          ..folderId = item.folderId
          ..tags.addAll(item.tags);
    i.fields[StdField.title] = item.title;
    i.fields.addAll(item.fields);
    for (final String url in item.urls) {
      i.urls[_newId()] = url;
    }
    for (final NewCustom c in item.custom) {
      i.custom.add(_FakeCustom(_newId(), c.kind, c.label, c.value));
    }
    _items[i.id] = i;
    _bump();
    return i.id;
  }

  @override
  Future<void> setField(String id, StdField field, String value) async {
    _requireUnlocked();
    _setValue(_item(id), field, value);
  }

  @override
  Future<void> clearField(String id, StdField field) async {
    _requireUnlocked();
    final _FakeItem i = _item(id);
    i.fields.remove(field);
    _touch(i);
  }

  @override
  Future<void> setFields(String id, Map<StdField, String> fields) async {
    _requireUnlocked();
    final _FakeItem i = _item(id);
    fields.forEach((StdField f, String v) => _setValue(i, f, v));
  }

  @override
  Future<bool> toggleFavorite(String id) async {
    _requireUnlocked();
    final _FakeItem i = _item(id);
    i.favorite = !i.favorite;
    _touch(i);
    return i.favorite;
  }

  @override
  Future<void> moveToFolder(String id, String? folderId) async {
    _requireUnlocked();
    if (folderId != null && !_folders.containsKey(folderId)) {
      throw const AppError(AppErrorCode.notFound, 'unknown folder');
    }
    final _FakeItem i = _item(id);
    i.folderId = folderId;
    _touch(i);
  }

  @override
  Future<void> addTag(String id, String tag) async {
    _requireUnlocked();
    final _FakeItem i = _item(id);
    if (!i.tags.contains(tag)) i.tags.add(tag);
    _touch(i);
  }

  @override
  Future<void> removeTag(String id, String tag) async {
    _requireUnlocked();
    final _FakeItem i = _item(id);
    i.tags.remove(tag);
    _touch(i);
  }

  @override
  Future<String> addUrl(String id, String url) async {
    _requireUnlocked();
    final _FakeItem i = _item(id);
    final String urlId = _newId();
    i.urls[urlId] = url;
    _touch(i);
    return urlId;
  }

  @override
  Future<void> setUrl(String id, String urlId, String url) async {
    _requireUnlocked();
    final _FakeItem i = _item(id);
    if (!i.urls.containsKey(urlId)) {
      throw const AppError(AppErrorCode.notFound, 'unknown url');
    }
    i.urls[urlId] = url;
    _touch(i);
  }

  @override
  Future<void> removeUrl(String id, String urlId) async {
    _requireUnlocked();
    final _FakeItem i = _item(id);
    i.urls.remove(urlId);
    _touch(i);
  }

  @override
  Future<String> addCustomField(
    String id,
    CustomKind kind,
    String label,
    String value,
  ) async {
    _requireUnlocked();
    final _FakeItem i = _item(id);
    final String elementId = _newId();
    i.custom.add(_FakeCustom(elementId, kind, label, value));
    _touch(i);
    return elementId;
  }

  _FakeCustom _custom(_FakeItem i, String elementId) => i.custom.firstWhere(
    (_FakeCustom c) => c.id == elementId,
    orElse: () => throw const AppError(AppErrorCode.notFound, 'unknown field'),
  );

  @override
  Future<void> setCustomValue(String id, String elementId, String value) async {
    _requireUnlocked();
    final _FakeItem i = _item(id);
    _custom(i, elementId).value = value;
    _touch(i);
  }

  @override
  Future<void> setCustomLabel(String id, String elementId, String label) async {
    _requireUnlocked();
    final _FakeItem i = _item(id);
    _custom(i, elementId).label = label;
    _touch(i);
  }

  @override
  Future<void> removeCustomField(String id, String elementId) async {
    _requireUnlocked();
    final _FakeItem i = _item(id);
    i.custom.remove(_custom(i, elementId));
    _touch(i);
  }

  @override
  Future<void> deleteItem(String id) async {
    _requireUnlocked();
    _item(id).deletedAt = _nowMs;
    _bump();
  }

  @override
  Future<void> restoreItem(String id) async {
    _requireUnlocked();
    _item(id).deletedAt = null;
    _bump();
  }

  @override
  Future<void> purgeItem(String id) async {
    _requireUnlocked();
    _item(id);
    _items.remove(id);
    _bump();
  }

  TrashEntry _trashEntry(_FakeItem i) {
    final int deletedAt = i.deletedAt ?? 0;
    return TrashEntry(
      summary: _summary(i),
      deletedAt: deletedAt,
      purgesAt: deletedAt + _settings.vaultConfig.trashDays * 86400000,
    );
  }

  @override
  Future<List<TrashEntry>> listTrash() async {
    _requireUnlocked();
    return _items.values
        .where((_FakeItem i) => i.deletedAt != null)
        .map(_trashEntry)
        .toList(growable: false);
  }

  @override
  Future<int> emptyTrash() async {
    _requireUnlocked();
    final List<String> ids = _items.values
        .where((_FakeItem i) => i.deletedAt != null)
        .map((_FakeItem i) => i.id)
        .toList();
    ids.forEach(_items.remove);
    _bump();
    return ids.length;
  }

  @override
  Future<int> purgeExpired() async {
    _requireUnlocked();
    final int cutoff = _nowMs - _settings.vaultConfig.trashDays * 86400000;
    final List<String> ids = _items.values
        .where((_FakeItem i) => i.deletedAt != null && i.deletedAt! <= cutoff)
        .map((_FakeItem i) => i.id)
        .toList();
    ids.forEach(_items.remove);
    _bump();
    return ids.length;
  }

  @override
  Future<List<VersionInfo>> history(String id, StdField field) async {
    _requireUnlocked();
    final List<(int, String)> v =
        _item(id).versions[field] ?? <(int, String)>[];
    return <VersionInfo>[
      for (var n = v.length - 1; n >= 0; n--)
        VersionInfo(
          field: field,
          hlcMs: v[n].$1,
          atMs: v[n].$1,
          isCurrent: n == v.length - 1,
          concurrent: _item(id).concurrent[field]?.contains(v[n].$1) ?? false,
          deviceName: (_item(id).concurrent[field]?.contains(v[n].$1) ?? false)
              ? 'Other device'
              : 'This device',
        ),
    ];
  }

  @override
  Future<Uint8List?> revealVersion(String id, StdField field, int hlcMs) async {
    _requireUnlocked();
    for (final (int, String) v
        in _item(id).versions[field] ?? <(int, String)>[]) {
      if (v.$1 == hlcMs) return _bytes(v.$2);
    }
    throw const AppError(AppErrorCode.notFound, 'unknown version');
  }

  @override
  Future<void> restoreVersion(String id, StdField field, int hlcMs) async {
    _requireUnlocked();
    final _FakeItem i = _item(id);
    String? value;
    for (final (int, String) v in i.versions[field] ?? <(int, String)>[]) {
      if (v.$1 == hlcMs) value = v.$2;
    }
    if (value == null) {
      throw const AppError(AppErrorCode.notFound, 'unknown version');
    }
    _setValue(i, field, value);
    // The user resolved the conflict: nothing is a concurrent loser any more.
    i.concurrent.remove(field);
  }

  @override
  Future<String> createFolder(String name, String? parentId) async {
    _requireUnlocked();
    if (name.trim().isEmpty) {
      throw const AppError(
        AppErrorCode.validation,
        'name is required',
        field: 'name',
      );
    }
    final String id = _newId();
    _folders[id] = Folder(id: id, name: name, parentId: parentId);
    return id;
  }

  @override
  Future<void> renameFolder(String id, String name) async {
    _requireUnlocked();
    final Folder? f = _folders[id];
    if (f == null) {
      throw const AppError(AppErrorCode.notFound, 'unknown folder');
    }
    _folders[id] = Folder(id: id, name: name, parentId: f.parentId);
  }

  @override
  Future<void> deleteFolder(String id) async {
    _requireUnlocked();
    if (_folders.remove(id) == null) {
      throw const AppError(AppErrorCode.notFound, 'unknown folder');
    }
    for (final _FakeItem i in _items.values) {
      if (i.folderId == id) i.folderId = null;
    }
  }

  @override
  Future<List<Folder>> listFolders() async {
    _requireUnlocked();
    return _folders.values.toList(growable: false);
  }

  // -------------------------------------------------------------- 4.3 health

  @override
  Future<HealthReport> healthReport() async {
    _requireUnlocked();
    final Map<String, List<String>> byPassword = <String, List<String>>{};
    final List<WeakPassword> weak = <WeakPassword>[];
    final List<OldPassword> old = <OldPassword>[];
    for (final _FakeItem i in _items.values) {
      final String? pw = i.fields[StdField.password];
      if (i.deletedAt != null || pw == null) continue;
      byPassword.putIfAbsent(pw, () => <String>[]).add(i.id);
      final int score = _score(pw);
      if (score <= 1) weak.add(WeakPassword(itemId: i.id, score: score));
      final int ageDays = (_nowMs - i.updatedAt) ~/ 86400000;
      if (ageDays > 365) old.add(OldPassword(itemId: i.id, ageDays: ageDays));
    }
    return HealthReport(
      reused: <ReuseGroup>[
        for (final List<String> ids in byPassword.values)
          if (ids.length > 1) ReuseGroup(itemIds: ids),
      ],
      weak: weak,
      old: old,
    );
  }

  // ------------------------------------------------------- 4.4 import / export

  /// Largest file `importPreview` accepts (the core's bound is in docs/13).
  int importByteLimit = 5 * 1024 * 1024;

  /// Test hook: most items a vault may hold before an import hits `limitReached`.
  int importItemLimit = 100000;

  final Map<String, _FakePreview> _previews = <String, _FakePreview>{};
  int _previewCounter = 0;

  static const String _exportMagic = 'AVEX-FAKE';

  /// Minimal CSV reader: quotes, doubled quotes, CRLF. Not the core's parser.
  static List<List<String>> _parseCsv(String text) {
    final List<List<String>> rows = <List<String>>[];
    List<String> row = <String>[];
    final StringBuffer cell = StringBuffer();
    var quoted = false;
    for (var i = 0; i < text.length; i++) {
      final String c = text[i];
      if (quoted) {
        if (c == '"') {
          if (i + 1 < text.length && text[i + 1] == '"') {
            cell.write('"');
            i++;
          } else {
            quoted = false;
          }
        } else {
          cell.write(c);
        }
      } else if (c == '"') {
        quoted = true;
      } else if (c == ',') {
        row.add(cell.toString());
        cell.clear();
      } else if (c == '\n' || c == '\r') {
        if (c == '\r' && i + 1 < text.length && text[i + 1] == '\n') i++;
        row.add(cell.toString());
        cell.clear();
        if (row.any((String v) => v.isNotEmpty) || row.length > 1) {
          rows.add(row);
        }
        row = <String>[];
      } else {
        cell.write(c);
      }
    }
    if (cell.isNotEmpty || row.isNotEmpty) {
      row.add(cell.toString());
      rows.add(row);
    }
    return rows;
  }

  static String _csvCell(String v) =>
      v.contains(RegExp('["\n\r,]')) ? '"${v.replaceAll('"', '""')}"' : v;

  bool _isDuplicate(String title, String? username) => _items.values.any(
    (_FakeItem i) =>
        i.deletedAt == null &&
        i.title == title &&
        (i.fields[StdField.username] ?? '') == (username ?? ''),
  );

  _FakePreview _parseImport(ImportFormat format, Uint8List file) {
    final String text = utf8.decode(file, allowMalformed: true);
    final List<_FakeImportItem> items = <_FakeImportItem>[];
    final List<SkippedRecord> skipped = <SkippedRecord>[];
    final List<ImportWarning> warnings = <ImportWarning>[];
    final Set<String> folders = <String>{};
    if (format == ImportFormat.csv) {
      final List<List<String>> rows = _parseCsv(text);
      if (rows.isEmpty) {
        throw const AppError(
          AppErrorCode.validation,
          'the file has no header row',
          field: 'file',
        );
      }
      final List<String> header = rows.first
          .map((String h) => h.trim().toLowerCase())
          .toList();
      int col(List<String> names) => header.indexWhere(names.contains);
      final int title = col(<String>['name', 'title']);
      final int user = col(<String>['username', 'login_username']);
      final int pass = col(<String>['password', 'login_password']);
      final int url = col(<String>['url', 'login_uri', 'website']);
      final int notes = col(<String>['notes']);
      final int folder = col(<String>['folder']);
      if (title < 0) {
        throw const AppError(
          AppErrorCode.validation,
          'the file has no name column',
          field: 'file',
        );
      }
      String at(List<String> r, int c) =>
          c >= 0 && c < r.length ? r[c].trim() : '';
      for (var n = 1; n < rows.length; n++) {
        final List<String> r = rows[n];
        final String name = at(r, title);
        if (name.isEmpty) {
          skipped.add(SkippedRecord(record: n, reason: 'missing name'));
          continue;
        }
        if (at(r, pass).isEmpty) {
          warnings.add(ImportWarning(record: n, message: 'no password'));
        }
        final String f = at(r, folder);
        if (f.isNotEmpty) folders.add(f);
        items.add(
          _FakeImportItem(
            name,
            at(r, user),
            at(r, pass),
            at(r, url),
            at(r, notes),
            f,
          ),
        );
      }
    } else {
      final Object? decoded;
      try {
        decoded = jsonDecode(text);
      } on FormatException {
        throw const AppError(
          AppErrorCode.validation,
          'the file is not valid JSON',
          field: 'file',
        );
      }
      if (decoded is! Map<String, Object?> || decoded['items'] is! List) {
        throw const AppError(
          AppErrorCode.validation,
          'the file has no items list',
          field: 'file',
        );
      }
      final Object? rawFolders = decoded['folders'];
      final Map<String, String> folderNames = <String, String>{
        if (rawFolders is List<Object?>)
          for (final Object? f in rawFolders)
            if (f is Map<String, Object?>) '${f['id']}': '${f['name']}',
      };
      var n = 0;
      for (final Object? raw in decoded['items'] as List<Object?>) {
        n++;
        if (raw is! Map<String, Object?> || '${raw['name'] ?? ''}'.isEmpty) {
          skipped.add(SkippedRecord(record: n, reason: 'missing name'));
          continue;
        }
        final Object? login = raw['login'];
        final Map<String, Object?> l = login is Map<String, Object?>
            ? login
            : <String, Object?>{};
        final Object? uris = l['uris'];
        final String url = uris is List<Object?> && uris.isNotEmpty
            ? '${(uris.first as Map<String, Object?>)['uri'] ?? ''}'
            : '';
        final String f = folderNames['${raw['folderId']}'] ?? '';
        if (f.isNotEmpty) folders.add(f);
        items.add(
          _FakeImportItem(
            '${raw['name']}',
            '${l['username'] ?? ''}',
            '${l['password'] ?? ''}',
            url,
            '${raw['notes'] ?? ''}',
            f,
          ),
        );
      }
    }
    return _FakePreview(format.name, items, skipped, warnings, folders);
  }

  @override
  Future<ImportPreview> importPreview({
    required ImportFormat format,
    required Uint8List file,
  }) async {
    _requireUnlocked();
    if (file.length > importByteLimit) {
      throw const AppError(
        AppErrorCode.limitReached,
        'the file is too large',
        field: 'file',
      );
    }
    final _FakePreview p = _parseImport(format, file);
    final String token = 'preview-${++_previewCounter}';
    _previews[token] = p;
    return ImportPreview(
      format: p.format,
      itemCount: p.items.length,
      folderCount: p.folders.length,
      duplicates: p.items
          .where((_FakeImportItem i) => _isDuplicate(i.title, i.username))
          .length,
      warnings: p.warnings,
      skipped: p.skipped,
      previewToken: token,
    );
  }

  ImportResult _commit(_FakePreview p, ImportOptions options) {
    final int visible = _items.values
        .where((_FakeItem i) => i.deletedAt == null)
        .length;
    if (visible + p.items.length > importItemLimit) {
      throw const AppError(
        AppErrorCode.limitReached,
        'the vault item limit would be exceeded',
      );
    }
    var created = 0;
    var duplicates = 0;
    var foldersCreated = 0;
    final Map<String, String> folderIds = <String, String>{};
    for (final String name in p.folders) {
      final String? existing = _folders.values
          .where((Folder f) => f.name == name)
          .map((Folder f) => f.id)
          .firstOrNull;
      if (existing != null) {
        folderIds[name] = existing;
      } else {
        folderIds[name] = _addFolder(name);
        foldersCreated++;
      }
    }
    for (final _FakeImportItem it in p.items) {
      if (options.skipDuplicates && _isDuplicate(it.title, it.username)) {
        duplicates++;
        continue;
      }
      final _FakeItem item = _FakeItem(
        id: _newId(),
        type: ItemType.login,
        title: it.title,
        createdAt: _nowMs,
      );
      item.fields[StdField.title] = it.title;
      if (it.username.isNotEmpty) item.fields[StdField.username] = it.username;
      if (it.password.isNotEmpty) item.fields[StdField.password] = it.password;
      if (it.notes.isNotEmpty) item.fields[StdField.notes] = it.notes;
      if (it.url.isNotEmpty) item.urls[_newId()] = it.url;
      item.folderId = options.targetFolder ?? folderIds[it.folder];
      _items[item.id] = item;
      created++;
    }
    _bump();
    return ImportResult(
      created: created,
      foldersCreated: foldersCreated,
      duplicates: duplicates,
      invalid: p.skipped.length,
      skipped: 0,
    );
  }

  @override
  Future<ImportResult> importCommit({
    required String previewToken,
    required ImportOptions options,
  }) async {
    _requireUnlocked();
    final _FakePreview? p = _previews[previewToken];
    if (p == null) {
      throw const AppError(
        AppErrorCode.notFound,
        'unknown or already used preview',
      );
    }
    if (options.targetFolder != null &&
        !_folders.containsKey(options.targetFolder)) {
      throw const AppError(AppErrorCode.notFound, 'unknown folder');
    }
    final ImportResult r = _commit(p, options);
    _previews.remove(previewToken);
    return r;
  }

  /// All visible logins as CSV text (obviously not the core's exporter).
  String _csvText() {
    final StringBuffer out = StringBuffer('name,username,password,url,notes\n');
    for (final _FakeItem i in _byNewest) {
      if (i.deletedAt != null || i.type != ItemType.login) continue;
      out.writeln(
        <String>[
          i.title,
          i.fields[StdField.username] ?? '',
          i.fields[StdField.password] ?? '',
          i.urls.values.firstOrNull ?? '',
          i.fields[StdField.notes] ?? '',
        ].map(_csvCell).join(','),
      );
    }
    return out.toString();
  }

  @override
  Future<Uint8List> exportCsv(AcknowledgePlaintextRisk token) async {
    _requireUnlocked();
    if (!token.acknowledged) {
      throw const AppError(
        AppErrorCode.validation,
        'plaintext risk not acknowledged',
        field: 'token',
      );
    }
    return Uint8List.fromList(utf8.encode(_csvText()));
  }

  /// FAKE container tag of a password (not a KDF, not a MAC).
  static String _tag(Uint8List password) {
    var h = 7;
    for (final int b in password) {
      h = (h * 131 + b) & 0x7fffffff;
    }
    return h.toRadixString(16);
  }

  @override
  Future<Uint8List> exportEncrypted(Uint8List exportPassword) async {
    _requireUnlocked();
    if (exportPassword.isEmpty) {
      throw const AppError(
        AppErrorCode.validation,
        'the export password is empty',
        field: 'exportPassword',
      );
    }
    final String body = base64Encode(utf8.encode(_csvText()));
    return Uint8List.fromList(
      utf8.encode('$_exportMagic\n${_tag(exportPassword)}\n$body'),
    );
  }

  @override
  Future<ImportResult> importEncrypted({
    required Uint8List file,
    required Uint8List exportPassword,
  }) async {
    _requireUnlocked();
    final List<String> parts = utf8
        .decode(file, allowMalformed: true)
        .split('\n');
    if (parts.length != 3 || parts[0] != _exportMagic) {
      throw const AppError(
        AppErrorCode.corruptVault,
        'not an AryaVault export',
      );
    }
    if (parts[1] != _tag(exportPassword)) {
      throw const AppError(
        AppErrorCode.wrongCredentials,
        'wrong export password or damaged file',
      );
    }
    final String csv = utf8.decode(base64Decode(parts[2]));
    final _FakePreview p = _parseImport(
      ImportFormat.csv,
      Uint8List.fromList(utf8.encode(csv)),
    );
    return _commit(p, const ImportOptions());
  }

  // ------------------------------------------------- 4.5 settings, 4.6 info

  @override
  Future<AppSettings> getSettings() async {
    _requireUnlocked();
    return _settings;
  }

  @override
  Future<void> setSettings(AppSettings settings) async {
    _requireUnlocked();
    // The real core clamps to the allowed ranges (docs/14 §4.5).
    _settings = AppSettings(
      autoLockMinutes: settings.autoLockMinutes.clamp(1, 60),
      lockOnSleep: settings.lockOnSleep,
      lockOnScreenLock: settings.lockOnScreenLock,
      clipboardClearSeconds: settings.clipboardClearSeconds.clamp(5, 120),
      blockScreenCapture: settings.blockScreenCapture,
      revealHideSeconds: settings.revealHideSeconds.clamp(1, 15),
      vaultConfig: settings.vaultConfig,
    );
  }

  @override
  Future<InfoDto> info() async => InfoDto(
    coreVersion: '0.0.0-fake',
    apiVersion: expectedApiVersion,
    formatVersion: _exists ? 1 : 0,
    schemaVersion: _exists && !_locked ? 2 : 0,
    sqlcipherSettings: const <PinnedSetting>[],
  );

  @override
  Future<Uint8List> exportDiagnostics() async =>
      Uint8List.fromList(utf8.encode('{"fake":true}'));
}
