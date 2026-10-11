import 'package:flutter/material.dart';

import '../widgets/l10n_ext.dart';

class SplashScreen extends StatelessWidget {
  const SplashScreen({super.key});

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      body: Center(
        child: Semantics(
          liveRegion: true,
          label: context.l10n.loadingVault,
          child: const CircularProgressIndicator(),
        ),
      ),
    );
  }
}
