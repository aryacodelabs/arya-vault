import 'package:arya_vault/src/api/app_error.dart';
import 'package:arya_vault/src/api/dtos.dart';
import 'package:arya_vault/src/l10n/gen/app_localizations.dart';
import 'package:arya_vault/src/state/providers.dart';
import 'package:arya_vault/src/state/routing.dart';
import 'package:arya_vault/src/state/vault_query.dart';
import 'package:arya_vault/src/ui/tools/health_screen.dart';
import 'package:flutter/material.dart' hide Page;
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_riverpod/misc.dart' show Override;
import 'package:flutter_test/flutter_test.dart';

import '../helpers.dart';
import 'tool_helpers.dart';

Future<String> _login(BufferSpyApi api, String title, String password) =>
    api.createItem(
      NewItem(
        type: ItemType.login,
        title: title,
        fields: <StdField, String>{
          StdField.username: 'user@example.invalid',
          StdField.password: password,
        },
      ),
    );

void main() {
  late DateTime now;

  /// A vault with a reused pair, a weak one and an old one.
  Future<BufferSpyApi> seededVault() async {
    now = DateTime.utc(2025, 1, 1);
    final BufferSpyApi api = await unlockedBufferVault(clock: () => now);
    await _login(api, 'CANARY Old Bank', 'CANARY-Old-Unique-Pw-4417!x');
    now = DateTime.utc(2026, 6, 1); // the old item is now ~17 months old
    await _login(api, 'CANARY Shared One', 'CANARY-Shared-Pw-9921!zq');
    await _login(api, 'CANARY Shared Two', 'CANARY-Shared-Pw-9921!zq');
    await _login(api, 'CANARY Weak Site', 'password1');
    await _login(api, 'CANARY Fine Site', 'CANARY-Fine-Unique-Pw-5308!w');
    return api;
  }

  testWidgets('lists reused, weak and old passwords with what each means', (
    WidgetTester tester,
  ) async {
    final BufferSpyApi api = await seededVault();
    await pumpRoute(
      tester,
      AppRoutes.health,
      api: api,
      size: const Size(700, 1600),
    );
    expect(find.text(l10n.healthTitle), findsOneWidget);
    expect(find.text(l10n.healthReusedExplain), findsOneWidget);
    expect(find.text(l10n.healthWeakExplain), findsOneWidget);
    expect(find.text(l10n.healthOldExplain), findsOneWidget);
    expect(find.text(l10n.healthReusedGroup(2)), findsOneWidget);
    expect(find.text('CANARY Shared One'), findsOneWidget);
    expect(find.text('CANARY Shared Two'), findsOneWidget);
    expect(find.text('CANARY Weak Site'), findsOneWidget);
    expect(find.text('CANARY Old Bank'), findsOneWidget);
    expect(find.text('CANARY Fine Site'), findsNothing);
    expect(find.textContaining('Unchanged for'), findsOneWidget);
    expect(find.textContaining('Strength:'), findsOneWidget);
    expect(find.text(l10n.healthReusedNone), findsNothing);
  });

  testWidgets('shows no password anywhere', (WidgetTester tester) async {
    final BufferSpyApi api = await seededVault();
    await pumpRoute(
      tester,
      AppRoutes.health,
      api: api,
      size: const Size(700, 1600),
    );
    expect(find.textContaining('Pw-9921'), findsNothing);
    expect(find.textContaining('password1'), findsNothing);
    expect(find.textContaining('Pw-4417'), findsNothing);
    expect(api.count('reveal'), 0, reason: 'health never reveals a secret');
  });

  testWidgets('empty states say so in each section', (
    WidgetTester tester,
  ) async {
    final BufferSpyApi api = await unlockedBufferVault();
    await _login(api, 'CANARY Only Fine', 'CANARY-Fine-Unique-Pw-5308!w');
    await pumpRoute(
      tester,
      AppRoutes.health,
      api: api,
      size: const Size(700, 1600),
    );
    expect(find.text(l10n.healthReusedNone), findsOneWidget);
    expect(find.text(l10n.healthWeakNone), findsOneWidget);
    expect(find.text(l10n.healthOldNone), findsOneWidget);
  });

  testWidgets('tapping an entry opens the item in the vault', (
    WidgetTester tester,
  ) async {
    final BufferSpyApi api = await seededVault();
    final Harness h = await pumpRoute(
      tester,
      AppRoutes.health,
      api: api,
      size: const Size(1200, 1600),
    );
    await tapOn(tester, find.text('CANARY Weak Site'));
    expect(find.text(l10n.healthTitle), findsNothing);
    final String? id = h.container.read(vaultQueryProvider).selectedId;
    expect(id, isNotNull);
    expect((await api.getItem(id!)).title, 'CANARY Weak Site');
  });

  testWidgets('the navigation callback receives the item id', (
    WidgetTester tester,
  ) async {
    final BufferSpyApi api = await seededVault();
    final List<String> opened = <String>[];
    tester.view.physicalSize = const Size(700, 1600);
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.reset);
    await tester.pumpWidget(
      ProviderScope(
        overrides: <Override>[vaultApiProvider.overrideWithValue(api)],
        child: MaterialApp(
          theme: appTheme,
          supportedLocales: AppLocalizations.supportedLocales,
          localizationsDelegates: appDelegates,
          home: HealthScreen(onOpenItem: opened.add),
        ),
      ),
    );
    await tester.pumpAndSettle();
    await tester.tap(find.text('CANARY Old Bank'));
    expect(opened, hasLength(1));
    expect((await api.getItem(opened.single)).title, 'CANARY Old Bank');
  });

  testWidgets('a failing report offers Try again', (WidgetTester tester) async {
    final BufferSpyApi api = await seededVault();
    api.failNext['healthReport'] = const AppError(AppErrorCode.internal, 'x');
    await pumpRoute(
      tester,
      AppRoutes.health,
      api: api,
      size: const Size(700, 1200),
    );
    expect(find.text(l10n.errInternal), findsOneWidget);
    await tapOn(tester, find.text(l10n.actionTryAgain));
    expect(find.text(l10n.healthReusedTitle), findsOneWidget);
  });

  testWidgets('large text does not overflow', (WidgetTester tester) async {
    final BufferSpyApi api = await seededVault();
    await pumpRoute(
      tester,
      AppRoutes.health,
      api: api,
      size: const Size(400, 800),
      textScale: 2,
    );
    expect(tester.takeException(), isNull);
  });
}
