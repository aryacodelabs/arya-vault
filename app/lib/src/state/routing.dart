import 'package:flutter/widgets.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:go_router/go_router.dart';

import '../api/dtos.dart';
import '../ui/screens/create_vault_screen.dart';
import '../ui/screens/error_screen.dart';
import '../ui/screens/explainer_screen.dart';
import '../ui/screens/forgot_both_screen.dart';
import '../ui/screens/home_screen.dart';
import '../ui/screens/lock_screen.dart';
import '../ui/screens/quick_unlock_offer_screen.dart';
import '../ui/screens/recover_screen.dart';
import '../ui/screens/recovery_key_screen.dart';
import '../ui/screens/splash_screen.dart';
import '../ui/screens/verify_screen.dart';
import '../ui/screens/welcome_screen.dart';
import 'onboarding_state.dart';
import 'vault_status.dart';

abstract final class AppRoutes {
  static const String splash = '/splash';
  static const String error = '/error';
  static const String welcome = '/welcome';
  static const String create = '/create';
  static const String explainer = '/onboarding/explainer';
  static const String recoveryKey = '/onboarding/key';
  static const String verify = '/onboarding/verify';
  static const String quickOffer = '/onboarding/quick-unlock';
  static const String lock = '/lock';
  static const String recover = '/recover';
  static const String forgot = '/forgot';
  static const String home = '/home';

  static const List<String> all = <String>[
    splash,
    error,
    welcome,
    create,
    explainer,
    recoveryKey,
    verify,
    quickOffer,
    lock,
    recover,
    forgot,
    home,
  ];
}

/// The routing guard, as a pure function so it can be tested exhaustively.
///
/// Returns the location to redirect to, or null to stay. The table:
///
/// | status                                   | allowed locations            |
/// |------------------------------------------|------------------------------|
/// | loading / failed to load                 | splash / error               |
/// | no vault                                 | welcome, create              |
/// | vault, locked                            | lock, recover, forgot        |
/// | unlocked, onboarding incomplete          | exactly one onboarding step  |
/// | unlocked, complete, quick-unlock offer   | quick-unlock offer           |
/// | unlocked, complete                       | home                         |
///
/// "Exactly one onboarding step" is what makes SEC-A01 structural: while
/// `onboardingComplete` is false there is no location other than the
/// recovery-key steps, and the verify step is only reachable after the key
/// was shown and "I stored it" was confirmed.
String? redirectFor({
  required AsyncValue<VaultStatus> status,
  required OnboardingState onboarding,
  required String location,
}) {
  String? goto(String target) => location == target ? null : target;
  final VaultStatus? s = status.value;
  if (s == null) {
    return status.hasError ? goto(AppRoutes.error) : goto(AppRoutes.splash);
  }
  if (!s.exists) {
    return const <String>{
          AppRoutes.welcome,
          AppRoutes.create,
        }.contains(location)
        ? null
        : AppRoutes.welcome;
  }
  if (s.locked) {
    return const <String>{
          AppRoutes.lock,
          AppRoutes.recover,
          AppRoutes.forgot,
        }.contains(location)
        ? null
        : AppRoutes.lock;
  }
  if (!s.onboardingComplete) {
    return goto(_onboardingStep(onboarding));
  }
  if (onboarding.quickOfferPending) {
    return goto(AppRoutes.quickOffer);
  }
  return goto(AppRoutes.home);
}

String _onboardingStep(OnboardingState o) {
  if (o.key == null) {
    // The pending key is gone (restart or lock): the key screen offers a new one.
    return AppRoutes.recoveryKey;
  }
  if (!o.explainerSeen) {
    return AppRoutes.explainer;
  }
  return o.keySaved ? AppRoutes.verify : AppRoutes.recoveryKey;
}

class _RouterRefresh extends ChangeNotifier {
  void poke() => notifyListeners();
}

final Provider<GoRouter> routerProvider = Provider<GoRouter>((Ref ref) {
  final _RouterRefresh refresh = _RouterRefresh();
  ref.listen(vaultStatusProvider, (_, _) => refresh.poke());
  ref.listen(onboardingProvider, (_, _) => refresh.poke());
  ref.onDispose(refresh.dispose);

  GoRoute route(String path, Widget Function() build) => GoRoute(
    path: path,
    builder: (BuildContext context, GoRouterState state) => build(),
  );

  return GoRouter(
    initialLocation: AppRoutes.splash,
    refreshListenable: refresh,
    redirect: (BuildContext context, GoRouterState state) => redirectFor(
      status: ref.read(vaultStatusProvider),
      onboarding: ref.read(onboardingProvider),
      location: state.matchedLocation,
    ),
    routes: <RouteBase>[
      route(AppRoutes.splash, SplashScreen.new),
      route(AppRoutes.error, ErrorScreen.new),
      route(AppRoutes.welcome, WelcomeScreen.new),
      route(AppRoutes.create, CreateVaultScreen.new),
      route(AppRoutes.explainer, ExplainerScreen.new),
      route(AppRoutes.recoveryKey, RecoveryKeyScreen.new),
      route(AppRoutes.verify, VerifyScreen.new),
      route(AppRoutes.quickOffer, QuickUnlockOfferScreen.new),
      route(AppRoutes.lock, LockScreen.new),
      route(AppRoutes.recover, RecoverScreen.new),
      route(AppRoutes.forgot, ForgotBothScreen.new),
      route(AppRoutes.home, HomeScreen.new),
    ],
  );
});
