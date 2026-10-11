import 'dart:typed_data';

import 'package:flutter/widgets.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../api/dtos.dart';
import 'providers.dart';

/// The inline "generate" button of the edit form calls this. B03 replaces the
/// default with the real generator dialog (length, character classes,
/// passphrase); the default just asks the core for a password with the default
/// options. Returns the bytes to put in the field (the form wipes them), or
/// null if the user cancelled.
typedef PasswordGeneratorLauncher = Future<Uint8List?> Function(
  BuildContext context,
);

final Provider<PasswordGeneratorLauncher> passwordGeneratorLauncherProvider =
    Provider<PasswordGeneratorLauncher>((Ref ref) {
      return (BuildContext context) =>
          ref.read(vaultApiProvider).generatePassword(const PasswordOptions());
    });
