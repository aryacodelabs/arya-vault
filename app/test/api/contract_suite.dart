import 'dart:convert';
import 'dart:typed_data';

import 'package:arya_vault/src/api/app_error.dart';
import 'package:arya_vault/src/api/dtos.dart';
import 'package:arya_vault/src/api/vault_api.dart';
import 'package:flutter_test/flutter_test.dart';

/// The shared docs/14 contract suite (L01 goal 1). The same scenarios run against
/// `FakeVaultApi` and against the real core through `FfiVaultApi`, so the fake cannot
/// drift from the contract the UI relies on.
///
/// One ordered flow over one vault: later steps use what earlier ones created. Fake
/// canary values only. `open` must return an api for an empty vault directory.
void runContractSuite(String name, Future<VaultApi> Function() open) {
  group('contract: $name', () {
    late VaultApi api;
    late RecoveryKeyResult recovery;
    const String password = 'Tangerine-Quasar-vinyl-48-Hollow-Mitten';
    final Uint8List pw = Uint8List.fromList(utf8.encode(password));
    final Uint8List wrong = Uint8List.fromList(
      utf8.encode('Walnut-Gryphon-lantern-71-Brisk-Ember'),
    );
    late String itemId;
    late String folderId;

    Matcher throwsCode(AppErrorCode code) =>
        throwsA(isA<AppError>().having((AppError e) => e.code, 'code', code));

    setUpAll(() async {
      api = await open();
    });

    test('info reports the expected API version (docs/14 §7)', () async {
      final InfoDto info = await api.info();
      expect(info.apiVersion, expectedApiVersion);
      expect(info.coreVersion, isNotEmpty);
    });

    test('a new directory has no vault', () async {
      final VaultStatus s = await api.status();
      expect(s.exists, isFalse);
    });

    test('a weak master password is refused', () async {
      await expectLater(
        api.createVault(
          password: Uint8List.fromList(utf8.encode('aaaa')),
          profile: KdfProfile.low,
        ),
        throwsCode(AppErrorCode.weakPassword),
      );
      expect((await api.status()).exists, isFalse);
    });

    test(
      'createVault returns the recovery key once and leaves it unlocked',
      () async {
        recovery = await api.createVault(password: pw, profile: KdfProfile.low);
        final List<String> groups = String.fromCharCodes(recovery.recoveryKey)
            .split('-');
        expect(groups.length, greaterThanOrEqualTo(6));
        expect(recovery.challenge, isNotEmpty);
        expect(
          recovery.challenge.every((int i) => i >= 0 && i < groups.length),
          isTrue,
        );
        final VaultStatus s = await api.status();
        expect(s.exists, isTrue);
        expect(s.locked, isFalse);
        expect(s.onboardingComplete, isFalse);
      },
    );

    test('a second createVault is alreadyExists', () async {
      await expectLater(
        api.createVault(password: pw, profile: KdfProfile.low),
        throwsCode(AppErrorCode.alreadyExists),
      );
    });

    test('confirmRecoveryKey: a mismatch returns false, a match completes onboarding', () async {
      final bool bad = await api.confirmRecoveryKey(<GroupAnswer>[
        for (final int i in recovery.challenge)
          GroupAnswer(index: i, text: 'AAAAA'),
      ]);
      expect(bad, isFalse);
      expect((await api.status()).onboardingComplete, isFalse);

      // A mismatch makes the core draw new groups (docs/14 section 4.1); the next
      // attempt must cover exactly those.
      final List<int> next = await api.recoveryChallenge();
      expect(next, hasLength(3));
      expect(next.toSet(), hasLength(3));
      expect(next, orderedEquals(<int>[...next]..sort()));
      final List<String> groups = String.fromCharCodes(recovery.recoveryKey)
          .split('-');
      final bool ok = await api.confirmRecoveryKey(<GroupAnswer>[
        for (final int i in next) GroupAnswer(index: i, text: groups[i]),
      ]);
      expect(ok, isTrue);
      expect((await api.status()).onboardingComplete, isTrue);
      await expectLater(
        api.recoveryChallenge(),
        throwsCode(AppErrorCode.validation),
      );
    });

    test(
      'createItem, getItem and reveal keep secrets out of the view (SEC-S02)',
      () async {
        itemId = await api.createItem(
          const NewItem(
            type: ItemType.login,
            title: 'Contract login',
            fields: <StdField, String>{
              StdField.username: 'canary-user',
              StdField.password: 'CANARY-item-secret-1',
            },
            urls: <String>['https://example.invalid/login'],
            tags: <String>['contract'],
          ),
        );
        final ItemView v = await api.getItem(itemId);
        expect(v.title, 'Contract login');
        expect(v.fields[StdField.username], 'canary-user');
        expect(v.fields[StdField.password], isNull);
        expect(v.secretFieldsPresent, contains(StdField.password));
        expect(v.urls.single.url, 'https://example.invalid/login');
        expect(v.tags, <String>['contract']);

        final Uint8List? secret = await api.reveal(itemId, StdField.password);
        expect(utf8.decode(secret!), 'CANARY-item-secret-1');
      },
    );

    test('list, search and itemCount agree', () async {
      final List<ItemSummary> all = await api.list(
        const ListFilter(),
        const Page(offset: 0, limit: 50),
      );
      expect(all.map((ItemSummary s) => s.id), contains(itemId));
      expect(await api.itemCount(), all.length);
      final List<ItemSummary> found = await api.search(
        const SearchQuery(text: 'Contract'),
      );
      expect(found.map((ItemSummary s) => s.id), contains(itemId));
    });

    test('setField writes history and toggleFavorite flips', () async {
      await api.setField(itemId, StdField.password, 'CANARY-item-secret-2');
      final List<VersionInfo> h = await api.history(itemId, StdField.password);
      expect(h.length, greaterThanOrEqualTo(2));
      expect(h.where((VersionInfo v) => v.isCurrent), hasLength(1));
      expect(await api.toggleFavorite(itemId), isTrue);
      expect(await api.toggleFavorite(itemId), isFalse);
    });

    test('folders: create, move, list', () async {
      folderId = await api.createFolder('Contract folder', null);
      await api.moveToFolder(itemId, folderId);
      expect((await api.getItem(itemId)).folderId, folderId);
      expect(
        (await api.listFolders()).map((Folder f) => f.id),
        contains(folderId),
      );
    });

    test('trash: delete, list, restore', () async {
      await api.deleteItem(itemId);
      expect(
        (await api.listTrash()).map((TrashEntry t) => t.summary.id),
        contains(itemId),
      );
      await api.restoreItem(itemId);
      expect(
        (await api.listTrash()).map((TrashEntry t) => t.summary.id),
        isNot(contains(itemId)),
      );
    });

    test('generator and strength work', () async {
      final Uint8List p = await api.generatePassword(
        const PasswordOptions(length: 24),
      );
      expect(p.length, 24);
      final Uint8List words = await api.generatePassphrase(
        const PassphraseOptions(wordCount: 5),
      );
      expect(utf8.decode(words).split('-'), hasLength(5));
      expect(
        await api.entropyBits(
          const PasswordEntropy(PasswordOptions(length: 24)),
        ),
        greaterThan(80),
      );
      expect((await api.strength(p)).score, inInclusiveRange(0, 4));
      expect((await api.checkMasterPassword(pw)).acceptable, isTrue);
    });

    test('settings round trip', () async {
      final AppSettings s = await api.getSettings();
      await api.setSettings(s.copyWith(autoLockMinutes: 7));
      expect((await api.getSettings()).autoLockMinutes, 7);
    });

    test('healthReport answers', () async {
      final HealthReport h = await api.healthReport();
      expect(h.reused, isNotNull);
    });

    test('encrypted export imports back as duplicates', () async {
      final Uint8List file = await api.exportEncrypted(pw);
      expect(file, isNotEmpty);
      final ImportResult r = await api.importEncrypted(
        file: file,
        exportPassword: pw,
      );
      expect(r.created, 0);
      expect(r.duplicates, greaterThan(0));
    });

    test('verifyPassword', () async {
      expect(await api.verifyPassword(pw), isTrue);
      expect(await api.verifyPassword(wrong), isFalse);
    });

    test('lock refuses vault calls, wrong password is wrongCredentials, right one unlocks', () async {
      await api.lock();
      await api.lock(); // idempotent
      expect((await api.status()).locked, isTrue);
      await expectLater(
        api.list(const ListFilter(), const Page(offset: 0, limit: 10)),
        throwsCode(AppErrorCode.locked),
      );
      await expectLater(
        api.unlock(wrong),
        throwsCode(AppErrorCode.wrongCredentials),
      );
      await api.unlock(pw);
      expect((await api.status()).locked, isFalse);
      expect((await api.getItem(itemId)).title, 'Contract login');
    });
  });
}
