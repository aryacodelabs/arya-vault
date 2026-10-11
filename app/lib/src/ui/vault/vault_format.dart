import 'package:flutter/material.dart';
import 'package:intl/intl.dart';

import '../../api/dtos.dart';
import '../../l10n/gen/app_localizations.dart';

String itemTypeLabel(AppLocalizations l10n, ItemType type) => switch (type) {
  ItemType.login => l10n.typeLogin,
  ItemType.note => l10n.typeNote,
  ItemType.card => l10n.typeCard,
  ItemType.identity => l10n.typeIdentity,
};

IconData itemTypeIcon(ItemType type) => switch (type) {
  ItemType.login => Icons.key,
  ItemType.note => Icons.sticky_note_2_outlined,
  ItemType.card => Icons.credit_card,
  ItemType.identity => Icons.badge_outlined,
};

String stdFieldLabel(AppLocalizations l10n, StdField f) => switch (f) {
  StdField.title => l10n.fieldTitle,
  StdField.username => l10n.fieldUsername,
  StdField.password => l10n.fieldPassword,
  StdField.totpSeed => l10n.fieldTotpSeed,
  StdField.notes => l10n.fieldNotes,
  StdField.body => l10n.fieldBody,
  StdField.cardHolder => l10n.fieldCardHolder,
  StdField.cardNumber => l10n.fieldCardNumber,
  StdField.cardExpiry => l10n.fieldCardExpiry,
  StdField.cardCvv => l10n.fieldCardCvv,
  StdField.cardPin => l10n.fieldCardPin,
  StdField.firstName => l10n.fieldFirstName,
  StdField.middleName => l10n.fieldMiddleName,
  StdField.lastName => l10n.fieldLastName,
  StdField.email => l10n.fieldEmail,
  StdField.phone => l10n.fieldPhone,
  StdField.address => l10n.fieldAddress,
  StdField.ids => l10n.fieldIds,
};

/// Exactly the core's `StdField::is_secret` (core/crates/vault/src/model.rs).
const Set<StdField> kSecretFields = <StdField>{
  StdField.password,
  StdField.totpSeed,
  StdField.body,
  StdField.cardNumber,
  StdField.cardCvv,
  StdField.cardPin,
  StdField.ids,
};

/// Exactly the core's `StdField::allowed_for`, in display order (title first).
List<StdField> fieldsFor(ItemType type) => switch (type) {
  ItemType.login => const <StdField>[
    StdField.title,
    StdField.username,
    StdField.password,
    StdField.totpSeed,
    StdField.notes,
  ],
  ItemType.note => const <StdField>[StdField.title, StdField.body],
  ItemType.card => const <StdField>[
    StdField.title,
    StdField.cardHolder,
    StdField.cardNumber,
    StdField.cardExpiry,
    StdField.cardCvv,
    StdField.cardPin,
    StdField.notes,
  ],
  ItemType.identity => const <StdField>[
    StdField.title,
    StdField.firstName,
    StdField.middleName,
    StdField.lastName,
    StdField.email,
    StdField.phone,
    StdField.address,
    StdField.ids,
    StdField.notes,
  ],
};

/// Limits from docs/05 §10 (bytes), mirrored from the core for inline hints.
/// The core enforces them; the UI only shows them.
const int kMaxFieldBytes = 64 * 1024;
const int kMaxBodyBytes = 1 << 20;
const int kMaxCustomFields = 100;
const int kMaxTags = 50;
const int kMaxTagBytes = 64;

int maxBytesOf(StdField f) =>
    f == StdField.body ? kMaxBodyBytes : kMaxFieldBytes;

String formatDate(BuildContext context, int epochMs) =>
    DateFormat.yMMMd(Localizations.localeOf(context).toString())
        .format(DateTime.fromMillisecondsSinceEpoch(epochMs));

String formatDateTime(BuildContext context, int epochMs) =>
    DateFormat.yMMMd(Localizations.localeOf(context).toString())
        .add_Hm()
        .format(DateTime.fromMillisecondsSinceEpoch(epochMs));

String formatBytes(int bytes) {
  if (bytes < 1024) return '$bytes B';
  if (bytes < 1024 * 1024) return '${(bytes / 1024).toStringAsFixed(1)} KiB';
  return '${(bytes / (1024 * 1024)).toStringAsFixed(1)} MiB';
}
