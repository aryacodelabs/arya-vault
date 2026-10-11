import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../api/vault_api.dart';
import '../services/services.dart';
import '../services/time_source.dart';

/// Riverpod 3 retries a provider that throws. Off here: an `AppError` is an
/// answer (locked, validation, io...), not a glitch, and the screens offer an
/// explicit "Try again". Passed to every `ProviderScope`.
Duration? noAutoRetry(int retryCount, Object error) => null;

/// The core API. `main.dart` and tests override this; there is deliberately no
/// default so a missing override fails loudly instead of using a fake.
final Provider<VaultApi> vaultApiProvider = Provider<VaultApi>(
  (Ref ref) => throw UnimplementedError('override vaultApiProvider'),
);

final Provider<ClipboardPlatform> clipboardPlatformProvider =
    Provider<ClipboardPlatform>(
      (Ref ref) => const UnavailableClipboardPlatform(),
    );

final Provider<CaptureProtection> captureProtectionProvider =
    Provider<CaptureProtection>(
      (Ref ref) => const UnavailableCaptureProtection(),
    );

/// Clock and timers of the security logic; tests override it with fake time.
final Provider<TimeSource> timeSourceProvider = Provider<TimeSource>(
  (Ref ref) => const SystemTimeSource(),
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

/// App-level (non-vault) theme choice. Not persisted yet: that needs the
/// app-preferences store L01 provides (docs/14 §4.5).
class ThemeModeNotifier extends Notifier<ThemeMode> {
  @override
  ThemeMode build() => ThemeMode.system;

  void set(ThemeMode mode) => state = mode;
}

final NotifierProvider<ThemeModeNotifier, ThemeMode> themeModeProvider =
    NotifierProvider<ThemeModeNotifier, ThemeMode>(ThemeModeNotifier.new);
