import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../api/vault_api.dart';
import '../services/services.dart';

/// The core API. `main.dart` and tests override this; there is deliberately no
/// default so a missing override fails loudly instead of using a fake.
final Provider<VaultApi> vaultApiProvider = Provider<VaultApi>(
  (Ref ref) => throw UnimplementedError('override vaultApiProvider'),
);

final Provider<ClipboardService> clipboardServiceProvider =
    Provider<ClipboardService>(
      (Ref ref) => const UnavailableClipboardService(),
    );

final Provider<FilePickerService> filePickerServiceProvider =
    Provider<FilePickerService>(
      (Ref ref) => const UnavailableFilePickerService(),
    );

final Provider<PrintService> printServiceProvider = Provider<PrintService>(
  (Ref ref) => const UnavailablePrintService(),
);

final Provider<UrlLauncherService> urlLauncherServiceProvider =
    Provider<UrlLauncherService>(
      (Ref ref) => const UnavailableUrlLauncherService(),
    );

final Provider<PlatformEvents> platformEventsProvider =
    Provider<PlatformEvents>((Ref ref) => const NoPlatformEvents());

/// App-level (non-vault) theme choice. Persisting it is B03's settings work.
class ThemeModeNotifier extends Notifier<ThemeMode> {
  @override
  ThemeMode build() => ThemeMode.system;

  void set(ThemeMode mode) => state = mode;
}

final NotifierProvider<ThemeModeNotifier, ThemeMode> themeModeProvider =
    NotifierProvider<ThemeModeNotifier, ThemeMode>(ThemeModeNotifier.new);
