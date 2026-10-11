// ignore: unused_import
import 'package:intl/intl.dart' as intl;

import 'app_localizations.dart';

// ignore_for_file: type=lint

/// The translations for English (`en`).
class AppLocalizationsEn extends AppLocalizations {
  AppLocalizationsEn([String locale = 'en']) : super(locale);

  @override
  String get appName => 'AryaVault';

  @override
  String get actionContinue => 'Continue';

  @override
  String get actionBack => 'Back';

  @override
  String get actionTryAgain => 'Try again';

  @override
  String get actionLock => 'Lock now';

  @override
  String get showPassword => 'Show password';

  @override
  String get hidePassword => 'Hide password';

  @override
  String get loadingVault => 'Loading your vault…';

  @override
  String get loadFailed => 'Couldn\'t read the vault status.';

  @override
  String get welcomeTitle => 'Welcome to AryaVault';

  @override
  String get welcomeTagline =>
      'Your passwords and notes, encrypted on this device. No account and no server.';

  @override
  String get welcomeCreate => 'Create a new vault';

  @override
  String get welcomeAddDevice => 'Add this device to an existing vault';

  @override
  String get welcomeAddDeviceUnavailable => 'Available with sync';

  @override
  String get createTitle => 'Create your vault';

  @override
  String get createIntro =>
      'Choose a master password. It is the only thing that unlocks your vault on this device.';

  @override
  String get masterPasswordLabel => 'Master password';

  @override
  String get confirmPasswordLabel => 'Confirm master password';

  @override
  String get passwordMismatch => 'The two passwords don\'t match.';

  @override
  String get passphraseTip =>
      'Tip: a passphrase of 16 or more characters is much stronger.';

  @override
  String get createSubmit => 'Create vault';

  @override
  String get createWorking => 'Creating your vault…';

  @override
  String get advancedTitle => 'Advanced';

  @override
  String get kdfLabel => 'Key-derivation strength';

  @override
  String get kdfHelp =>
      'Stronger settings make password guessing slower for an attacker but use more memory. Choose Low only on very old devices.';

  @override
  String get kdfLow => 'Low memory use';

  @override
  String get kdfDefault => 'Balanced (recommended)';

  @override
  String get kdfHigh => 'High (slower, more memory)';

  @override
  String get strengthLabel => 'Password strength';

  @override
  String get strengthVeryWeak => 'Very weak';

  @override
  String get strengthWeak => 'Weak';

  @override
  String get strengthFair => 'Fair';

  @override
  String get strengthGood => 'Good';

  @override
  String get strengthStrong => 'Strong';

  @override
  String strengthSemantics(String level) {
    return 'Password strength: $level';
  }

  @override
  String get policyMet => 'Meets the requirements.';

  @override
  String get policyEmpty => 'Enter a password of at least 12 characters.';

  @override
  String get explainerTitle => 'There is no password reset';

  @override
  String get explainerBody =>
      'AryaVault can\'t reset your password — we don\'t have your data or a server. Your recovery key is the only backup. Store it somewhere safe and separate from this device.';

  @override
  String get explainerPoint =>
      'Without your master password or your recovery key, nobody can open your data, including us.';

  @override
  String get explainerContinue => 'Show my recovery key';

  @override
  String get recoveryTitle => 'Your recovery key';

  @override
  String get recoveryIntro =>
      'Write it down or print it, and keep it somewhere safe and separate from this device. It is shown only now.';

  @override
  String get recoveryShowKey => 'Show key';

  @override
  String get recoveryHideKey => 'Hide key';

  @override
  String get recoveryHiddenPlaceholder => 'Recovery key hidden';

  @override
  String recoveryGroupSemantics(int n, int total, String chars) {
    return 'Group $n of $total: $chars';
  }

  @override
  String recoveryChecksumSemantics(String chars) {
    return 'Checksum group: $chars';
  }

  @override
  String get recoveryChecksumLabel => 'Checksum';

  @override
  String get recoveryPrint => 'Print';

  @override
  String get recoverySave => 'Save as file';

  @override
  String get recoveryNoCopy =>
      'This screen has no copy button on purpose: other apps can read the clipboard.';

  @override
  String get recoveryCaptureWarning =>
      'Don\'t screenshot or photograph this key into a cloud-synced folder. Anyone who sees it, together with a copy of your vault file, can open your vault.';

  @override
  String get recoveryStoredCheck => 'I stored it';

  @override
  String get recoveryContinue => 'Continue';

  @override
  String get recoverySaved => 'Saved.';

  @override
  String get recoverySaveFailed => 'Couldn\'t save the file.';

  @override
  String get recoveryPrintFailed => 'Couldn\'t print.';

  @override
  String get recoveryServiceUnavailable =>
      'This is not available in this build yet.';

  @override
  String get recoverySheetTitle => 'AryaVault recovery key';

  @override
  String get recoverySheetBody =>
      'Keep this sheet somewhere safe, separate from your device. Anyone with this key and a copy of your vault file can open your vault.';

  @override
  String get recoverySheetKeyLabel => 'Recovery key:';

  @override
  String get recoveryFileName => 'aryavault-recovery-key.txt';

  @override
  String get recoveryRegenTitle => 'Create a new recovery key';

  @override
  String get recoveryRegenBody =>
      'Your earlier recovery key was never confirmed, so it can\'t be shown again. Enter your master password to create a new one.';

  @override
  String get recoveryRegenSubmit => 'Create recovery key';

  @override
  String get verifyTitle => 'Check your recovery key';

  @override
  String get verifyIntro =>
      'Type the groups below from your saved copy. This proves you can use it later.';

  @override
  String verifyGroupField(int n) {
    return 'Group $n';
  }

  @override
  String get verifyGroupHelp => 'Five letters or digits';

  @override
  String get verifySubmit => 'Verify';

  @override
  String get verifyMismatch =>
      'That doesn\'t match. Look at your key again, then retry.';

  @override
  String get verifyShowKeyAgain => 'Show my key again';

  @override
  String get quickOfferTitle => 'Unlock faster?';

  @override
  String quickOfferBody(String method) {
    return 'Unlock this vault with $method instead of typing your master password. You will still need your master password after a restart.';
  }

  @override
  String get quickEnable => 'Turn on';

  @override
  String get quickNotNow => 'Not now';

  @override
  String get quickKindWindowsHello => 'Windows Hello';

  @override
  String get quickKindTouchId => 'Touch ID';

  @override
  String get quickKindFaceId => 'Face ID';

  @override
  String get quickKindBiometric => 'biometrics';

  @override
  String get quickKindOsKeyring => 'your system keyring';

  @override
  String get quickKindNone => 'quick unlock';

  @override
  String get lockTitle => 'Unlock your vault';

  @override
  String get lockSubmit => 'Unlock';

  @override
  String get lockWorking => 'Unlocking…';

  @override
  String lockQuick(String method) {
    return 'Unlock with $method';
  }

  @override
  String get lockUseRecovery => 'Use recovery key';

  @override
  String get recoverTitle => 'Recover with your recovery key';

  @override
  String get recoverKeyLabel => 'Recovery key';

  @override
  String get recoverKeyHint => 'Spaces, dashes and lowercase letters are fine.';

  @override
  String get recoverNewPasswordTitle => 'Choose a new master password';

  @override
  String get recoverSubmit => 'Recover vault';

  @override
  String get recoverNoKey => 'I don\'t have my recovery key';

  @override
  String get forgotTitle => 'If you have neither';

  @override
  String get forgotBody1 =>
      'Your data is encrypted with keys protected by your master password and your recovery key. Without either one, nobody can recover it, including us.';

  @override
  String get forgotBody2 =>
      'If you remember an older master password, an older snapshot of your vault may still open with it.';

  @override
  String get forgotBody3 =>
      'As a last resort you will be able to erase this device\'s vault and start fresh. Erasing is permanent. This option comes in a later version.';

  @override
  String get homeTitle => 'Your vault';

  @override
  String get homeUnlockedMessage => 'Your vault is unlocked.';

  @override
  String get homeListPlaceholder => 'Items will appear here.';

  @override
  String get homeDetailPlaceholder => 'Select an item to see its details.';

  @override
  String get navVault => 'Vault';

  @override
  String get navGenerator => 'Generator';

  @override
  String get navHealth => 'Health';

  @override
  String get navSettings => 'Settings';

  @override
  String get errWrongCredentials =>
      'That didn\'t work. Check your password and try again.';

  @override
  String get errWrongRecoveryKey =>
      'That recovery key doesn\'t open this vault.';

  @override
  String get errRecoveryKeyMalformed =>
      'That recovery key doesn\'t look right. Check it for typos and try again.';

  @override
  String get errWeakPassword => 'That password is too weak.';

  @override
  String get errLocked => 'The vault is locked.';

  @override
  String get errNotFound => 'That item no longer exists.';

  @override
  String get errValidation => 'Some input isn\'t valid.';

  @override
  String get errLimitReached => 'A size or count limit was reached.';

  @override
  String get errCorruptVault =>
      'The vault data failed an integrity check. It was not opened.';

  @override
  String get errUnsupportedFormat =>
      'This vault was made by a newer version of AryaVault. Update the app to open it.';

  @override
  String get errQuickUnlockUnavailable =>
      'Quick unlock isn\'t available right now. Use your master password.';

  @override
  String get errAlreadyExists => 'A vault already exists here.';

  @override
  String get errIo => 'Couldn\'t read or write the vault files.';

  @override
  String get errBusy => 'Too many attempts. Wait a moment, then try again.';

  @override
  String get errInternal => 'Something went wrong.';
}
