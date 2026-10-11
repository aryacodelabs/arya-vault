import 'dart:typed_data';

import 'package:flutter/widgets.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../ui/tools/generator_screen.dart';

/// The inline "generate" button of the edit form calls this: it opens the
/// generator sheet (length, character classes, passphrase, live entropy) and
/// returns the bytes of the password the user chose, or null if they cancelled.
/// The form puts the bytes in the field and wipes them. Tests override it.
typedef PasswordGeneratorLauncher = Future<Uint8List?> Function(
  BuildContext context,
);

final Provider<PasswordGeneratorLauncher> passwordGeneratorLauncherProvider =
    Provider<PasswordGeneratorLauncher>(
      (Ref ref) => showPasswordGeneratorSheet,
    );
