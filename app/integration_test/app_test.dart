import 'dart:io';
import 'dart:typed_data';

import 'package:arya_vault/src/api/dtos.dart';
import 'package:arya_vault/src/api/ffi_vault_api.dart';
import 'package:arya_vault/src/app.dart';
import 'package:arya_vault/src/l10n/gen/app_localizations_en.dart';
import 'package:arya_vault/src/state/providers.dart';
import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:integration_test/integration_test.dart';

/// The built Windows app against the real Rust core (L02): unlock through the UI, see the
/// item, lock again. Run with:
/// `flutter test integration_test -d windows` (after `cargo build -p arya-vault-ffi --release`
/// in `core/`; the runner build installs the library next to the executable).
///
/// Fake canary data in a temporary directory; the real vault folder is not touched.
const String _password = 'CANARY-integration-Tangerine-Quasar-48-Hollow';

void main() {
  IntegrationTestWidgetsFlutterBinding.ensureInitialized();
  final AppLocalizationsEn l10n = AppLocalizationsEn();

  testWidgets('real core: unlock, list, lock', (WidgetTester tester) async {
    final Directory dir = Directory.systemTemp.createTempSync('aryavault_it_');
    addTearDown(() {
      try {
        dir.deleteSync(recursive: true);
      } on FileSystemException {
        // Windows may hold the database file a moment longer; the temp cleaner takes it.
      }
    });
    final FfiVaultApi api = await FfiVaultApi.open(vaultDir: dir.path);
    final RecoveryKeyResult r = await api.createVault(
      password: Uint8List.fromList(_password.codeUnits),
      profile: KdfProfile.low,
    );
    final List<String> groups = String.fromCharCodes(r.recoveryKey).split('-');
    expect(
      await api.confirmRecoveryKey(<GroupAnswer>[
        for (final int i in r.challenge) GroupAnswer(index: i, text: groups[i]),
      ]),
      isTrue,
    );
    await api.createItem(
      const NewItem(
        type: ItemType.login,
        title: 'CANARY-integration-item',
        fields: <StdField, String>{StdField.username: 'CANARY-user'},
      ),
    );
    await api.lock();

    await tester.pumpWidget(
      ProviderScope(
        retry: noAutoRetry,
        overrides: [vaultApiProvider.overrideWithValue(api)],
        child: const AryaVaultApp(),
      ),
    );
    await tester.pumpAndSettle();
    expect(find.text(l10n.lockTitle), findsOneWidget);

    await tester.enterText(
      find.byWidgetPredicate(
        (Widget w) =>
            w is TextField &&
            w.decoration?.labelText == l10n.masterPasswordLabel,
      ),
      _password,
    );
    await tester.pumpAndSettle();
    await tester.tap(find.widgetWithText(FilledButton, l10n.lockSubmit));
    // Argon2id runs in the core; give it time.
    for (
      var i = 0;
      i < 50 && find.text(l10n.lockTitle).evaluate().isNotEmpty;
      i++
    ) {
      await tester.pump(const Duration(milliseconds: 200));
    }
    await tester.pumpAndSettle();
    expect(find.text('CANARY-integration-item'), findsOneWidget);

    await tester.tap(find.byTooltip(l10n.actionLock).first);
    await tester.pumpAndSettle();
    expect(find.text('CANARY-integration-item'), findsNothing);
    expect(find.text(l10n.lockTitle), findsOneWidget);
  });
}
