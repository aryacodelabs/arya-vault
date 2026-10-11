import 'package:flutter/material.dart';

import '../../api/dtos.dart';
import '../../api/secret_bytes.dart';
import '../../api/vault_api.dart';
import '../../theme/app_theme.dart';
import 'l10n_ext.dart';
import 'secret_text_field.dart';
import 'strength_meter.dart';

/// State behind the "new master password" form: two secret fields plus the live
/// strength and policy answers from the core (`strength`,
/// `checkMasterPassword`; both work while locked).
class NewPasswordController extends ChangeNotifier {
  NewPasswordController(this._api);

  final VaultApi _api;
  final SecretTextController password = SecretTextController();
  final SecretTextController confirm = SecretTextController();

  Strength? strength;
  PolicyResult? policy;
  int _generation = 0;
  bool _disposed = false;

  bool get matches => password.text.isNotEmpty && password.text == confirm.text;

  /// Both fields agree and the core accepts the password.
  bool get canSubmit => (policy?.acceptable ?? false) && matches;

  bool get showMismatch => confirm.text.isNotEmpty && !matches;

  /// Soft warning under 16 characters (docs/07 §7). A UI hint only: the core
  /// owns the real policy.
  bool get suggestPassphrase =>
      (policy?.acceptable ?? false) && password.text.runes.length < 16;

  /// Call from the fields' onChanged.
  Future<void> changed() async {
    final int generation = ++_generation;
    notifyListeners();
    if (password.text.isEmpty) {
      strength = null;
      policy = null;
      notifyListeners();
      return;
    }
    final SecretBytes forStrength = password.takeSecret();
    final SecretBytes forPolicy = password.takeSecret();
    final Strength newStrength = await forStrength.useAsync(_api.strength);
    final PolicyResult newPolicy = await forPolicy.useAsync(
      _api.checkMasterPassword,
    );
    if (_disposed || generation != _generation) {
      return; // a newer keystroke superseded this answer
    }
    strength = newStrength;
    policy = newPolicy;
    notifyListeners();
  }

  /// The password as wipeable bytes. The caller passes it to an action that
  /// wipes it.
  SecretBytes takePassword() => password.takeSecret();

  void clear() {
    _generation++;
    password.clearSecret();
    confirm.clearSecret();
    strength = null;
    policy = null;
    notifyListeners();
  }

  @override
  void dispose() {
    _disposed = true;
    password.clearSecret();
    confirm.clearSecret();
    password.dispose();
    confirm.dispose();
    super.dispose();
  }
}

/// Password + confirmation + meter + policy text.
class NewPasswordFields extends StatelessWidget {
  const NewPasswordFields({
    required this.controller,
    this.passwordLabel,
    this.onSubmitted,
    this.autofocus = true,
    super.key,
  });

  final NewPasswordController controller;
  final String? passwordLabel;
  final VoidCallback? onSubmitted;
  final bool autofocus;

  @override
  Widget build(BuildContext context) {
    final l10n = context.l10n;
    final AppTokens tokens = context.tokens;
    return ListenableBuilder(
      listenable: controller,
      builder: (BuildContext context, _) => Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: <Widget>[
          SecretTextField(
            controller: controller.password,
            label: passwordLabel ?? l10n.masterPasswordLabel,
            autofocus: autofocus,
            textInputAction: TextInputAction.next,
            onChanged: controller.changed,
          ),
          SizedBox(height: tokens.gap / 2),
          StrengthMeter(
            strength: controller.strength,
            policy: controller.policy,
          ),
          if (controller.suggestPassphrase) ...<Widget>[
            SizedBox(height: tokens.gap / 4),
            Text(
              l10n.passphraseTip,
              style: Theme.of(context).textTheme.bodySmall,
            ),
          ],
          SizedBox(height: tokens.gap),
          SecretTextField(
            controller: controller.confirm,
            label: l10n.confirmPasswordLabel,
            errorText: controller.showMismatch ? l10n.passwordMismatch : null,
            textInputAction: TextInputAction.done,
            onChanged: controller.changed,
            onSubmitted: onSubmitted,
          ),
        ],
      ),
    );
  }
}
