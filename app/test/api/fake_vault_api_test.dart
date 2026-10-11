import 'dart:convert';
import 'dart:typed_data';

import 'package:arya_vault/src/api/app_error.dart';
import 'package:arya_vault/src/api/dtos.dart';
import 'package:arya_vault/src/api/fake_vault_api.dart';
import 'package:flutter_test/flutter_test.dart';

import '../helpers.dart';

Matcher throwsCode(AppErrorCode code) =>
    throwsA(isA<AppError>().having((AppError e) => e.code, 'code', code));

void main() {
  group('lifecycle state machine (docs/14 §5)', () {
    test('NoVault -> create (unlocked, onboarding pending)', () async {
      final FakeVaultApi api = newFakeApi();
      VaultStatus s = await api.status();
      expect(s.exists, isFalse);
      await createThroughApi(api);
      s = await api.status();
      expect(s.exists, isTrue);
      expect(s.locked, isFalse);
      expect(s.onboardingComplete, isFalse);
    });

    test('a second createVault is alreadyExists', () async {
      final FakeVaultApi api = newFakeApi();
      await createThroughApi(api);
      await expectLater(
        createThroughApi(api),
        throwsCode(AppErrorCode.alreadyExists),
      );
    });

    test(
      'onboardingComplete flips only after a correct confirm (SEC-A01)',
      () async {
        final FakeVaultApi api = newFakeApi();
        final RecoveryKeyResult r = await createThroughApi(api);
        final List<String> groups = keyGroups(r);
        expect(groups, hasLength(8));
        expect(r.challenge, hasLength(3));
        expect(r.challenge.toSet(), hasLength(3));
        expect(r.challenge.every((int i) => i >= 0 && i < 6), isTrue);

        final bool wrong = await api.confirmRecoveryKey(<GroupAnswer>[
          for (final int i in r.challenge) GroupAnswer(index: i, text: 'AAAAA'),
        ]);
        expect(wrong, isFalse);
        expect((await api.status()).onboardingComplete, isFalse);

        // Tolerant formatting: lowercase and stray spaces/hyphens.
        final bool right = await api.confirmRecoveryKey(<GroupAnswer>[
          for (final int i in r.challenge)
            GroupAnswer(index: i, text: ' ${groups[i].toLowerCase()}-'),
        ]);
        expect(right, isTrue);
        expect((await api.status()).onboardingComplete, isTrue);
      },
    );

    test(
      'confirm with the wrong set of groups is a validation error',
      () async {
        final FakeVaultApi api = newFakeApi();
        final RecoveryKeyResult r = await createThroughApi(api);
        final int other = <int>[
          0,
          1,
          2,
          3,
          4,
          5,
        ].firstWhere((int i) => !r.challenge.contains(i));
        await expectLater(
          api.confirmRecoveryKey(<GroupAnswer>[
            GroupAnswer(index: other, text: 'AAAAA'),
          ]),
          throwsA(
            isA<AppError>()
                .having((AppError e) => e.code, 'code', AppErrorCode.validation)
                .having((AppError e) => e.field, 'field', 'answers'),
          ),
        );
      },
    );

    test(
      'lock drops the pending key; unlock keeps onboarding pending',
      () async {
        final FakeVaultApi api = newFakeApi();
        await createThroughApi(api);
        await api.lock();
        expect((await api.status()).locked, isTrue);
        await api.unlock(bytesOf(kPassword));
        final VaultStatus s = await api.status();
        expect(s.locked, isFalse);
        expect(s.onboardingComplete, isFalse);
        await expectLater(
          api.confirmRecoveryKey(const <GroupAnswer>[]),
          throwsCode(AppErrorCode.validation),
        );
        // Regenerating starts a new pending key.
        final RecoveryKeyResult again = await api.regenerateRecoveryKey(
          password: bytesOf(kPassword),
          rotateKeys: false,
        );
        expect(again.challenge, hasLength(3));
      },
    );

    test('lock is idempotent and works with no vault', () async {
      final FakeVaultApi api = newFakeApi();
      await api.lock();
      await createConfirmedLockedVault(api);
      await api.lock();
      expect((await api.status()).locked, isTrue);
    });

    test('operations while locked throw locked', () async {
      final FakeVaultApi api = FakeVaultApi(sampleItemCount: 5);
      await createConfirmedLockedVault(api);
      await expectLater(
        api.list(const ListFilter(), const Page(offset: 0, limit: 10)),
        throwsCode(AppErrorCode.locked),
      );
      await expectLater(api.itemCount(), throwsCode(AppErrorCode.locked));
      await expectLater(api.getSettings(), throwsCode(AppErrorCode.locked));
    });
  });

  group('credentials', () {
    test('wrong password -> wrongCredentials, right one unlocks', () async {
      final FakeVaultApi api = newFakeApi();
      await createConfirmedLockedVault(api);
      await expectLater(
        api.unlock(bytesOf('CANARY-not-the-password-1234')),
        throwsCode(AppErrorCode.wrongCredentials),
      );
      await api.unlock(bytesOf(kPassword));
      expect((await api.status()).locked, isFalse);
    });

    test('five free wrong attempts, then busy with a doubling delay', () async {
      DateTime now = DateTime.utc(2026, 1, 1);
      final FakeVaultApi api = newFakeApi(clock: () => now);
      await createConfirmedLockedVault(api);
      for (var i = 0; i < 5; i++) {
        await expectLater(
          api.unlock(bytesOf('CANARY-wrong-$i-aaaa')),
          throwsCode(AppErrorCode.wrongCredentials),
        );
      }
      // The 5th failure started a 1 s block: even the right password is busy.
      await expectLater(
        api.unlock(bytesOf(kPassword)),
        throwsCode(AppErrorCode.busy),
      );
      now = now.add(const Duration(seconds: 2));
      await expectLater(
        api.unlock(bytesOf('CANARY-wrong-again-aaaa')),
        throwsCode(AppErrorCode.wrongCredentials),
      );
      now = now.add(const Duration(seconds: 1)); // the block is now 2 s
      await expectLater(
        api.unlock(bytesOf(kPassword)),
        throwsCode(AppErrorCode.busy),
      );
      now = now.add(const Duration(seconds: 3));
      await api.unlock(bytesOf(kPassword));
      expect((await api.status()).locked, isFalse);
    });

    test(
      'master password policy: min 12 characters and not trivially weak',
      () async {
        final FakeVaultApi api = newFakeApi();
        PolicyResult r = await api.checkMasterPassword(bytesOf('short'));
        expect(r.acceptable, isFalse);
        expect(r.reasons.join(' '), contains('at least 12'));

        r = await api.checkMasterPassword(bytesOf('password1234'));
        expect(r.acceptable, isFalse, reason: 'common word, 12 chars');

        r = await api.checkMasterPassword(bytesOf('aaaaaaaaaaaa'));
        expect(r.acceptable, isFalse, reason: 'repeated character');

        r = await api.checkMasterPassword(bytesOf(kPassword));
        expect(r.acceptable, isTrue);
        expect(r.reasons, isEmpty);

        await expectLater(
          api.createVault(
            password: bytesOf('short'),
            profile: KdfProfile.default_,
          ),
          throwsCode(AppErrorCode.weakPassword),
        );
      },
    );

    test(
      'checkMasterPassword works while locked and without a vault',
      () async {
        final FakeVaultApi api = newFakeApi();
        expect(
          (await api.checkMasterPassword(bytesOf(kPassword))).acceptable,
          isTrue,
        );
        await createConfirmedLockedVault(api);
        expect(
          (await api.checkMasterPassword(bytesOf(kPassword))).acceptable,
          isTrue,
        );
      },
    );

    test('recoverWithKey: malformed, wrong, right', () async {
      final FakeVaultApi api = newFakeApi();
      final RecoveryKeyResult r = await createThroughApi(api);
      final String key = String.fromCharCodes(r.recoveryKey);
      final List<String> groups = keyGroups(r);
      await api.confirmRecoveryKey(<GroupAnswer>[
        for (final int i in r.challenge) GroupAnswer(index: i, text: groups[i]),
      ]);
      await api.lock();

      await expectLater(
        api.recoverWithKey(
          recoveryKey: bytesOf('NOPE'),
          newPassword: bytesOf(kNewPassword),
        ),
        throwsCode(AppErrorCode.recoveryKeyMalformed),
      );

      // A different, well-formed key (flip the last key char and re-checksum is
      // not possible from outside, so build another vault's key).
      final FakeVaultApi other = newFakeApi();
      final RecoveryKeyResult otherKey = await createThroughApi(other);
      await expectLater(
        api.recoverWithKey(
          recoveryKey: otherKey.recoveryKey,
          newPassword: bytesOf(kNewPassword),
        ),
        throwsCode(AppErrorCode.wrongCredentials),
      );

      await api.recoverWithKey(
        recoveryKey: bytesOf(key.toLowerCase().replaceAll('-', ' ')),
        newPassword: bytesOf(kNewPassword),
      );
      expect((await api.status()).locked, isFalse);
      await api.lock();
      await api.unlock(bytesOf(kNewPassword));
      await expectLater(
        api.unlock(bytesOf(kPassword)),
        throwsCode(AppErrorCode.wrongCredentials),
      );
    });

    test('recoverWithKey enforces the policy on the new password', () async {
      final FakeVaultApi api = newFakeApi();
      final RecoveryKeyResult r = await createThroughApi(api);
      await expectLater(
        api.recoverWithKey(
          recoveryKey: r.recoveryKey,
          newPassword: bytesOf('weak'),
        ),
        throwsCode(AppErrorCode.weakPassword),
      );
    });

    test('quick unlock: unsupported, enable, expire', () async {
      final FakeVaultApi none = newFakeApi();
      await createConfirmedLockedVault(none);
      await expectLater(
        none.unlockQuick(),
        throwsCode(AppErrorCode.quickUnlockUnavailable),
      );

      final FakeVaultApi api = newFakeApi(quickUnlock: kHello);
      await createConfirmedLockedVault(api);
      await api.unlock(bytesOf(kPassword));
      await api.quickUnlockEnable();
      await api.lock();
      expect((await api.status()).quickUnlock.enabled, isTrue);
      await api.unlockQuick();
      expect((await api.status()).locked, isFalse);
      await api.lock();
      api.quickUnlockUsable = false;
      await expectLater(
        api.unlockQuick(),
        throwsCode(AppErrorCode.quickUnlockUnavailable),
      );
    });

    test('changePassword and verifyPassword', () async {
      final FakeVaultApi api = newFakeApi();
      await createConfirmedLockedVault(api);
      await api.unlock(bytesOf(kPassword));
      expect(await api.verifyPassword(bytesOf(kPassword)), isTrue);
      expect(await api.verifyPassword(bytesOf('nope')), isFalse);
      await expectLater(
        api.changePassword(
          oldPassword: bytesOf('wrong'),
          newPassword: bytesOf(kNewPassword),
          rotateKeys: false,
        ),
        throwsCode(AppErrorCode.wrongCredentials),
      );
      await api.changePassword(
        oldPassword: bytesOf(kPassword),
        newPassword: bytesOf(kNewPassword),
        rotateKeys: true,
      );
      expect(await api.verifyPassword(bytesOf(kNewPassword)), isTrue);
    });

    test(
      'the fake does not keep a reference to the caller\'s buffer',
      () async {
        final FakeVaultApi api = newFakeApi();
        final Uint8List pw = bytesOf(kPassword);
        await api.createVault(password: pw, profile: KdfProfile.low);
        pw.fillRange(0, pw.length, 0); // the caller wipes
        expect(await api.verifyPassword(bytesOf(kPassword)), isTrue);
        expect(api.lastProfile, KdfProfile.low);
      },
    );
  });

  group('sample data and DTO hygiene', () {
    test('seeds 2,000 items by default', () async {
      final FakeVaultApi api = FakeVaultApi();
      await createConfirmedLockedVault(api);
      await api.unlock(bytesOf(kPassword));
      expect(await api.itemCount(), 2000);
    });

    test('summaries and views never carry secrets', () async {
      final FakeVaultApi api = FakeVaultApi(sampleItemCount: 60);
      await createConfirmedLockedVault(api);
      await api.unlock(bytesOf(kPassword));
      final List<ItemSummary> page = await api.list(
        const ListFilter(),
        const Page(offset: 0, limit: 200),
      );
      expect(page, isNotEmpty);
      for (final ItemSummary s in page) {
        expect(
          jsonEncode(<String>[s.title, s.subtitle]),
          isNot(contains('CANARY-pw')),
        );
        final ItemView v = await api.getItem(s.id);
        expect(
          v.fields.keys.toSet().intersection(<StdField>{
            StdField.password,
            StdField.totpSeed,
            StdField.body,
            StdField.cardNumber,
            StdField.cardCvv,
            StdField.cardPin,
          }),
          isEmpty,
        );
      }
      final ItemSummary login = page.firstWhere(
        (ItemSummary s) => s.type == ItemType.login,
      );
      final Uint8List? pw = await api.reveal(login.id, StdField.password);
      expect(String.fromCharCodes(pw!), startsWith('CANARY'));
      await expectLater(
        api.reveal(login.id, StdField.username),
        throwsCode(AppErrorCode.validation),
      );
    });

    test('page limit above 200 is rejected', () async {
      final FakeVaultApi api = FakeVaultApi(sampleItemCount: 5);
      await createConfirmedLockedVault(api);
      await api.unlock(bytesOf(kPassword));
      await expectLater(
        api.list(const ListFilter(), const Page(offset: 0, limit: 201)),
        throwsCode(AppErrorCode.validation),
      );
    });

    test('RecoveryKeyResult and GroupAnswer redact in toString', () async {
      final FakeVaultApi api = newFakeApi();
      final RecoveryKeyResult r = await createThroughApi(api);
      final String key = String.fromCharCodes(r.recoveryKey);
      expect(r.toString(), isNot(contains(key.substring(0, 5))));
      expect(
        const GroupAnswer(index: 1, text: 'CANARY').toString(),
        isNot(contains('CANARY')),
      );
    });

    test('AppError.toString shows code and the safe message only', () {
      const AppError e = AppError(
        AppErrorCode.wrongCredentials,
        'safe message',
      );
      expect(e.toString(), 'AppError(wrongCredentials: safe message)');
    });
  });
}
