/// Error codes of docs/14 §2. Names are canonical; do not rename.
enum AppErrorCode {
  /// Wrong master password or recovery key (indistinguishable by design).
  wrongCredentials,

  /// Typo or bad checksum in a recovery key; ask the user to re-enter it.
  recoveryKeyMalformed,

  /// Master password fails policy; [AppError.message] carries the reasons.
  weakPassword,

  /// The operation requires an unlocked session.
  locked,

  /// Item or folder id unknown.
  notFound,

  /// Input rejected; [AppError.field] names the field.
  validation,

  /// A field, item or size limit was reached (docs/05 §10).
  limitReached,

  /// Authentication or integrity failure of vault data.
  corruptVault,

  /// The vault is newer than this app understands ("update required").
  unsupportedFormat,

  /// Hardware/OS key missing, policy expired or invalidated; use the password.
  quickUnlockUnavailable,

  /// Something already exists (for example a vault in the target folder).
  alreadyExists,

  /// An I/O failure.
  io,

  /// Resource busy, or a local failure delay is running.
  busy,

  /// Anything else; opaque.
  internal,
}

/// What every [VaultApi] call can throw (docs/14 §2).
///
/// [message] is safe to show or log: it never contains a secret. The class
/// deliberately has no field that could hold one, and [toString] prints only
/// the code and that safe message.
class AppError implements Exception {
  const AppError(this.code, this.message, {this.field});

  final AppErrorCode code;
  final String message;

  /// The offending input for [AppErrorCode.validation], otherwise null.
  final String? field;

  @override
  String toString() => 'AppError(${code.name}: $message)';
}
