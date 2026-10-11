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

  @override
  String get actionCancel => 'Cancel';

  @override
  String get typeLogin => 'Login';

  @override
  String get typeNote => 'Secure note';

  @override
  String get typeCard => 'Card';

  @override
  String get typeIdentity => 'Identity';

  @override
  String get fieldTitle => 'Title';

  @override
  String get fieldUsername => 'Username';

  @override
  String get fieldPassword => 'Password';

  @override
  String get fieldTotpSeed => 'Authenticator code';

  @override
  String get fieldNotes => 'Notes';

  @override
  String get fieldBody => 'Note';

  @override
  String get fieldCardHolder => 'Cardholder';

  @override
  String get fieldCardNumber => 'Card number';

  @override
  String get fieldCardExpiry => 'Expiry';

  @override
  String get fieldCardCvv => 'Security code';

  @override
  String get fieldCardPin => 'PIN';

  @override
  String get fieldFirstName => 'First name';

  @override
  String get fieldMiddleName => 'Middle name';

  @override
  String get fieldLastName => 'Last name';

  @override
  String get fieldEmail => 'Email';

  @override
  String get fieldPhone => 'Phone';

  @override
  String get fieldAddress => 'Address';

  @override
  String get fieldIds => 'ID numbers';

  @override
  String get searchLabel => 'Search';

  @override
  String get searchClear => 'Clear search';

  @override
  String get newItem => 'New item';

  @override
  String newItemTitle(String type) {
    return 'New $type';
  }

  @override
  String get editItemTitle => 'Edit item';

  @override
  String get itemListLabel => 'Items';

  @override
  String get loadingItems => 'Loading items…';

  @override
  String get noMatches => 'No matching items.';

  @override
  String get emptyVaultHint => 'Add your first item with the + button.';

  @override
  String resultsCount(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count items',
      one: '1 item',
    );
    return '$_temp0';
  }

  @override
  String resultsCountMore(int count) {
    return '$count+ items';
  }

  @override
  String get selectItemHint => 'Select an item to see its details.';

  @override
  String get openMenu => 'Open menu';

  @override
  String get sidebarLabel => 'Vault navigation';

  @override
  String get allItems => 'All items';

  @override
  String get favoritesView => 'Favorites';

  @override
  String get foldersTitle => 'Folders';

  @override
  String get manageFolders => 'Manage folders';

  @override
  String get tagsLabel => 'Tags';

  @override
  String get noTagsYet => 'Tags appear here once items with tags have loaded.';

  @override
  String get trashTitle => 'Trash';

  @override
  String get trashRetention =>
      'Items stay in the trash until their purge date, then they are deleted for good.';

  @override
  String get trashEmpty => 'The trash is empty.';

  @override
  String purgesOn(String date) {
    return 'Deleted for good on $date';
  }

  @override
  String get restore => 'Restore';

  @override
  String get deleteForever => 'Delete forever';

  @override
  String get emptyTrash => 'Empty trash';

  @override
  String get emptyTrashTitle => 'Empty the trash?';

  @override
  String emptyTrashBody(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count items will be deleted for good.',
      one: '1 item will be deleted for good.',
    );
    return '$_temp0 This cannot be undone.';
  }

  @override
  String get purgeTitle => 'Delete forever?';

  @override
  String purgeBody(String title) {
    return '\"$title\" will be deleted for good. This cannot be undone.';
  }

  @override
  String movedToTrash(String title) {
    return '\"$title\" moved to the trash';
  }

  @override
  String get undo => 'Undo';

  @override
  String get favorite => 'Add to favorites';

  @override
  String get unfavorite => 'Remove from favorites';

  @override
  String get edit => 'Edit';

  @override
  String get moveToTrash => 'Move to trash';

  @override
  String get otherVersionsBadge => 'Other versions';

  @override
  String get history => 'History';

  @override
  String copyField(String field) {
    return 'Copy $field';
  }

  @override
  String revealField(String field) {
    return 'Show $field';
  }

  @override
  String hideField(String field) {
    return 'Hide $field';
  }

  @override
  String get hiddenValue => 'Hidden value';

  @override
  String copiedToast(int seconds) {
    return 'Copied; clears in $seconds s';
  }

  @override
  String get totpLater => 'Authenticator codes arrive in a later version.';

  @override
  String get urlLabel => 'Website';

  @override
  String get openUrl => 'Open website';

  @override
  String get urlsLabel => 'Websites';

  @override
  String get addUrl => 'Add website';

  @override
  String get removeUrl => 'Remove website';

  @override
  String get folderLabel => 'Folder';

  @override
  String get noFolder => 'No folder';

  @override
  String createdOn(String date) {
    return 'Created $date';
  }

  @override
  String updatedOn(String date) {
    return 'Updated $date';
  }

  @override
  String get linkNotOpenable => 'This link can\'t be opened.';

  @override
  String get openLinkTitle => 'Open this link?';

  @override
  String openLinkBody(String url) {
    return 'This opens $url in your browser.';
  }

  @override
  String get openInsecureLinkTitle => 'Open an unencrypted link?';

  @override
  String openInsecureLinkBody(String url) {
    return '$url does not use HTTPS, so what you send and receive can be read or changed on the way. Open it anyway?';
  }

  @override
  String get openLinkConfirm => 'Open';

  @override
  String get markdownImage => 'image not loaded';

  @override
  String get markdownTruncated =>
      'This note is long; only the first part is shown.';

  @override
  String get titleRequired => 'A title is required.';

  @override
  String get keepCurrentHint => 'Leave empty to keep the current value.';

  @override
  String get willBeRemoved => 'This value will be removed when you save.';

  @override
  String get keepValue => 'Keep the value';

  @override
  String removeValue(String field) {
    return 'Remove $field';
  }

  @override
  String get generatePassword => 'Generate a password';

  @override
  String sizeHint(String used, String max) {
    return 'Using $used of $max';
  }

  @override
  String tagsCount(int count, int max) {
    return 'Tags ($count of $max)';
  }

  @override
  String get addTagLabel => 'New tag';

  @override
  String get addTag => 'Add tag';

  @override
  String removeTag(String tag) {
    return 'Remove tag $tag';
  }

  @override
  String customFieldsCount(int count, int max) {
    return 'Custom fields ($count of $max)';
  }

  @override
  String get customLabel => 'Label';

  @override
  String get customValue => 'Value';

  @override
  String get removeCustomField => 'Remove custom field';

  @override
  String get kindText => 'Text';

  @override
  String get kindHidden => 'Hidden';

  @override
  String get kindUrl => 'Website';

  @override
  String get kindDate => 'Date';

  @override
  String addCustomField(String kind) {
    return 'Add $kind field';
  }

  @override
  String get save => 'Save';

  @override
  String get saving => 'Saving…';

  @override
  String get discardTitle => 'Discard changes?';

  @override
  String get discardBody =>
      'You have unsaved changes. If you leave, they are lost.';

  @override
  String get discard => 'Discard';

  @override
  String historyTitle(String title) {
    return 'History: $title';
  }

  @override
  String get noHistory => 'No earlier versions of this field.';

  @override
  String get currentVersion => 'Current';

  @override
  String get otherVersion => 'Other version';

  @override
  String get concurrentVersion => 'Edited on another device';

  @override
  String get thisDevice => 'This device';

  @override
  String get otherDevice => 'Another device';

  @override
  String get restoreThisVersion => 'Restore this version';

  @override
  String get versionRestored => 'Version restored.';

  @override
  String get compare => 'Compare';

  @override
  String compareTitle(String field) {
    return 'Compare: $field';
  }

  @override
  String get keepAsSeparateNote => 'Keep as separate note';

  @override
  String get keptAsNote => 'Saved as a new note.';

  @override
  String otherVersionNoteTitle(String title) {
    return '$title (other version)';
  }

  @override
  String get newFolder => 'New folder';

  @override
  String get folderNameLabel => 'Folder name';

  @override
  String get create => 'Create';

  @override
  String renameFolder(String name) {
    return 'Rename $name';
  }

  @override
  String get renameFolderTitle => 'Rename folder';

  @override
  String deleteFolder(String name) {
    return 'Delete $name';
  }

  @override
  String deleteFolderTitle(String name) {
    return 'Delete the folder \"$name\"?';
  }

  @override
  String get deleteFolderBody =>
      'The items in this folder are not deleted. They move to \"No folder\".';

  @override
  String get deleteFolderConfirm => 'Delete folder';

  @override
  String get noFolders => 'No folders yet.';

  @override
  String get navTools => 'Tools';

  @override
  String get navGenerator => 'Password generator';

  @override
  String get navHealth => 'Password health';

  @override
  String get navImport => 'Import';

  @override
  String get navExport => 'Export';

  @override
  String get navSettings => 'Settings';

  @override
  String get notAvailableBuild => 'Not available on this build';

  @override
  String get notAvailableDevice => 'Not available on this device';

  @override
  String get generatorTitle => 'Password generator';

  @override
  String get generatorModePassword => 'Password';

  @override
  String get generatorModePassphrase => 'Passphrase';

  @override
  String generatorLength(int n) {
    return 'Length: $n';
  }

  @override
  String get generatorLower => 'Lowercase letters (a-z)';

  @override
  String get generatorUpper => 'Uppercase letters (A-Z)';

  @override
  String get generatorDigits => 'Digits (0-9)';

  @override
  String get generatorSymbols => 'Symbols';

  @override
  String get generatorSymbolSet => 'Symbols to use';

  @override
  String get generatorExcludeAmbiguous =>
      'Leave out look-alike characters (O, 0, l, 1)';

  @override
  String generatorWords(int n) {
    return 'Words: $n';
  }

  @override
  String get generatorSeparator => 'Separator';

  @override
  String get generatorCapitalize => 'Capitalize each word';

  @override
  String get generatorNumber => 'Add a number';

  @override
  String generatorEntropy(String bits) {
    return 'About $bits bits of entropy';
  }

  @override
  String get generatorNeedClass => 'Turn on at least one kind of character.';

  @override
  String get generatorNeedSymbols =>
      'Enter at least one symbol, or turn symbols off.';

  @override
  String get generatorRegenerate => 'Generate another';

  @override
  String get generatorCopy => 'Copy';

  @override
  String get generatorUse => 'Use this password';

  @override
  String get generatorShow => 'Show the password';

  @override
  String get generatorHide => 'Hide the password';

  @override
  String get generatorValueLabel => 'Generated password';

  @override
  String get generatorFailed => 'Couldn\'t generate a password.';

  @override
  String get generatorSheetTitle => 'Generate a password';

  @override
  String get healthTitle => 'Password health';

  @override
  String get healthIntro =>
      'This checks the passwords in your vault on this device. Nothing is sent anywhere, and no password is shown here.';

  @override
  String get healthLoading => 'Checking your passwords…';

  @override
  String get healthReusedTitle => 'Reused passwords';

  @override
  String get healthReusedExplain =>
      'The same password is used on more than one item. If one site leaks it, every item that shares it is at risk.';

  @override
  String get healthReusedNone => 'No reused passwords.';

  @override
  String healthReusedGroup(int count) {
    return 'Shared by $count items';
  }

  @override
  String get healthWeakTitle => 'Weak passwords';

  @override
  String get healthWeakExplain =>
      'These passwords are easy to guess. Replace them with generated ones.';

  @override
  String get healthWeakNone => 'No weak passwords.';

  @override
  String healthWeakScore(String level) {
    return 'Strength: $level';
  }

  @override
  String get healthOldTitle => 'Old passwords';

  @override
  String get healthOldExplain =>
      'These passwords have not changed for a long time. Changing them limits the damage of a leak you never heard about.';

  @override
  String get healthOldNone => 'No old passwords.';

  @override
  String healthOldAge(int days) {
    String _temp0 = intl.Intl.pluralLogic(
      days,
      locale: localeName,
      other: 'Unchanged for $days days',
      one: 'Unchanged for 1 day',
    );
    return '$_temp0';
  }

  @override
  String get healthItemGone => 'Item no longer exists';

  @override
  String get reauthTitle => 'Confirm it\'s you';

  @override
  String get reauthBody => 'Enter your master password to continue.';

  @override
  String get reauthConfirm => 'Confirm';

  @override
  String get reauthWrong => 'That isn\'t your master password.';

  @override
  String get importTitle => 'Import';

  @override
  String get importIntro =>
      'Bring items in from a file. Nothing changes until you confirm the preview.';

  @override
  String get importFormatLabel => 'File type';

  @override
  String get importFormatCsv => 'CSV file';

  @override
  String get importFormatBitwarden => 'Bitwarden export (JSON)';

  @override
  String get importFormatEncrypted => 'AryaVault encrypted export';

  @override
  String get importChoose => 'Choose a file…';

  @override
  String get importCancelledPick => 'No file chosen.';

  @override
  String get importReading => 'Reading the file…';

  @override
  String get importPreviewTitle => 'Preview';

  @override
  String importPreviewCounts(int items, int folders) {
    String _temp0 = intl.Intl.pluralLogic(
      items,
      locale: localeName,
      other: '$items items',
      one: '1 item',
    );
    String _temp1 = intl.Intl.pluralLogic(
      folders,
      locale: localeName,
      other: '$folders folders',
      one: '1 folder',
    );
    return '$_temp0 and $_temp1 found';
  }

  @override
  String importPreviewDuplicates(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count items already exist in your vault',
      one: '1 item already exists in your vault',
    );
    return '$_temp0';
  }

  @override
  String get importPreviewNone => 'Nothing to import in this file.';

  @override
  String get importWarningsTitle => 'Warnings';

  @override
  String get importSkippedTitle => 'Skipped';

  @override
  String importRecordLine(int n, String detail) {
    return 'Record $n: $detail';
  }

  @override
  String importMore(int count) {
    return 'and $count more';
  }

  @override
  String get importSkipDuplicates => 'Skip items that already exist';

  @override
  String get importTargetFolder => 'Put imported items in';

  @override
  String importCommit(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count items',
      one: '1 item',
    );
    return 'Import $_temp0';
  }

  @override
  String get importCommitting => 'Importing…';

  @override
  String get importResultTitle => 'Import finished';

  @override
  String importResultCreated(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count items added',
      one: '1 item added',
    );
    return '$_temp0';
  }

  @override
  String importResultFolders(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count folders created',
      one: '1 folder created',
    );
    return '$_temp0';
  }

  @override
  String importResultDuplicates(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count duplicates skipped',
      one: '1 duplicate skipped',
    );
    return '$_temp0';
  }

  @override
  String importResultInvalid(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count records were not valid',
      one: '1 record was not valid',
    );
    return '$_temp0';
  }

  @override
  String importResultSkipped(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count records skipped',
      one: '1 record skipped',
    );
    return '$_temp0';
  }

  @override
  String get importAnother => 'Import another file';

  @override
  String get importPasswordTitle => 'Open the encrypted export';

  @override
  String get importPasswordBody =>
      'Enter the password this export was protected with.';

  @override
  String get importPasswordLabel => 'Export password';

  @override
  String get importPasswordConfirm => 'Import';

  @override
  String get importDeleteAdvice =>
      'You can delete the file you imported from. Passwords in it are not encrypted.';

  @override
  String get exportTitle => 'Export';

  @override
  String get exportIntro =>
      'Make a copy of your vault to keep or move to another app.';

  @override
  String get exportEncryptedTitle => 'Encrypted export (recommended)';

  @override
  String get exportEncryptedBody =>
      'A file protected by a password you choose now. Only that password opens it.';

  @override
  String get exportEncryptedButton => 'Export encrypted file…';

  @override
  String get exportCsvTitle => 'Plain CSV (not encrypted)';

  @override
  String get exportCsvBody =>
      'Every password and note in readable text. Use it only to move to another app.';

  @override
  String get exportCsvButton => 'Export CSV…';

  @override
  String get exportPasswordTitle => 'Choose an export password';

  @override
  String get exportPasswordLabel => 'Export password';

  @override
  String get exportPasswordBody =>
      'This password protects the exported file. It is separate from your master password.';

  @override
  String get exportKeepSafe =>
      'Keep this password safe. Without it nobody can open the file, and we can\'t recover it for you.';

  @override
  String get exportPasswordSubmit => 'Export';

  @override
  String get exportCsvWarnTitle => 'Export without encryption?';

  @override
  String get exportCsvWarnBody =>
      'The file will contain all your passwords as plain text. Anyone who gets it can read them, including cloud backups and anyone using this computer. Delete it as soon as you have imported it elsewhere.';

  @override
  String exportCsvTypePrompt(String phrase) {
    return 'To continue, type $phrase below.';
  }

  @override
  String get exportCsvPhrase => 'EXPORT';

  @override
  String get exportCsvTypeLabel => 'Confirmation';

  @override
  String get exportCsvConfirm => 'Export plain CSV';

  @override
  String get exportWorking => 'Preparing the file…';

  @override
  String get exportSaved => 'Saved.';

  @override
  String get exportCancelled => 'Not saved.';

  @override
  String get exportFailed => 'Couldn\'t save the file.';

  @override
  String get exportEncryptedFileName => 'aryavault-export.avex';

  @override
  String get exportCsvFileName => 'aryavault-export.csv';

  @override
  String get settingsTitle => 'Settings';

  @override
  String get settingsSecurity => 'Security';

  @override
  String get settingsSecuritySub =>
      'Auto-lock, clipboard, master password, recovery key';

  @override
  String get settingsSync => 'Sync & backup';

  @override
  String get settingsSyncSub => 'Coming soon';

  @override
  String get settingsAbout => 'About';

  @override
  String get settingsAboutSub => 'Version, licenses, diagnostics';

  @override
  String get settingsLoadFailed => 'Couldn\'t read the settings.';

  @override
  String get settingsSaveFailed => 'Couldn\'t save that setting.';

  @override
  String get secAutoLockTitle => 'Lock automatically';

  @override
  String get secAutoLockAfter => 'After no activity for';

  @override
  String secAutoLockMinutes(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '$n minutes',
      one: '1 minute',
    );
    return '$_temp0';
  }

  @override
  String get secLockOnScreenLock => 'When the screen locks';

  @override
  String get secLockOnSleep => 'When the computer sleeps';

  @override
  String get secAutoLockNote =>
      'There is no \"never\": a vault left open is the biggest risk.';

  @override
  String get secClipboardTitle => 'Clipboard';

  @override
  String get secClipboardClear => 'Clear copied passwords after';

  @override
  String secSeconds(int n) {
    String _temp0 = intl.Intl.pluralLogic(
      n,
      locale: localeName,
      other: '$n seconds',
      one: '1 second',
    );
    return '$_temp0';
  }

  @override
  String get secRevealHide => 'Hide a shown password after';

  @override
  String get secCaptureTitle => 'Screen capture';

  @override
  String get secCaptureBlock => 'Block screenshots and screen recording';

  @override
  String get secCaptureNote =>
      'Turn this off only if a screen reader or capture tool you rely on needs it.';

  @override
  String get secQuickTitle => 'Quick unlock';

  @override
  String secQuickToggle(String method) {
    return 'Unlock with $method';
  }

  @override
  String get secQuickEnableBody =>
      'Enter your master password to turn on quick unlock.';

  @override
  String get secQuickOnlyPassword =>
      'Quick unlock isn\'t available on this device or build. You unlock with your master password.';

  @override
  String get secMasterTitle => 'Master password';

  @override
  String get secChangePassword => 'Change master password';

  @override
  String get secRecoveryTitle => 'Recovery key';

  @override
  String get secViewRecovery => 'View recovery key';

  @override
  String get secRegenerateRecovery => 'Make a new recovery key';

  @override
  String get secViewRecoveryTitle => 'Your recovery key can\'t be shown again';

  @override
  String get secViewRecoveryBody =>
      'AryaVault never stores your recovery key: it was shown once, when you created it. If you can\'t find your copy, make a new one. The old key stops working right away.';

  @override
  String get secRegenerateTitle => 'Make a new recovery key?';

  @override
  String get secRegenerateBody =>
      'Your current recovery key stops working immediately. You will have to write down and confirm the new one before you can continue.';

  @override
  String get secRegenerateConfirm => 'Make new key';

  @override
  String get changePwTitle => 'Change master password';

  @override
  String get changePwCurrent => 'Current master password';

  @override
  String get changePwNew => 'New master password';

  @override
  String get changePwRotate =>
      'Also rotate keys (recommended after a suspected leak)';

  @override
  String get changePwRotateHelp =>
      'Replaces the key that encrypts your vault, not just the password. You will get a new recovery key to save.';

  @override
  String get changePwSubmit => 'Change password';

  @override
  String get changePwDone => 'Master password changed.';

  @override
  String get syncTitle => 'Sync & backup';

  @override
  String get syncComingSoon => 'Sync is coming soon';

  @override
  String get syncBody =>
      'Encrypted sync through your own Google Drive, iCloud or a folder is not part of this version. Your vault lives only on this device. Until sync arrives, make backups with Export.';

  @override
  String get aboutTitle => 'About AryaVault';

  @override
  String get aboutApp => 'App version';

  @override
  String get aboutCore => 'Core version';

  @override
  String get aboutApi => 'API version';

  @override
  String get aboutFormat => 'Vault format version';

  @override
  String get aboutSchema => 'Database schema version';

  @override
  String get aboutInfoFailed => 'Couldn\'t read the core version.';

  @override
  String get aboutLicenses => 'Open-source licenses';

  @override
  String get aboutLicensesRust =>
      'Licenses of the Rust crates in the core will be listed here once the core is bundled with the app.';

  @override
  String get aboutRepo => 'Source code';

  @override
  String get aboutRepoUrl => 'github.com/aryacodelabs/arya-vault';

  @override
  String get aboutDiagnostics => 'Diagnostics';

  @override
  String get aboutDiagnosticsSub =>
      'Review a scrubbed report before sharing it';

  @override
  String get diagTitle => 'Diagnostics';

  @override
  String get diagIntro =>
      'This report has no passwords and no vault contents. Read it before you share it with anyone.';

  @override
  String get diagPrepare => 'Prepare report';

  @override
  String get diagSave => 'Save report…';

  @override
  String get diagSaved => 'Report saved.';

  @override
  String get diagReportLabel => 'Diagnostics report';

  @override
  String get diagFileName => 'aryavault-diagnostics.json';

  @override
  String get errWrongExportPassword =>
      'That password doesn\'t open this file, or the file is damaged.';
}
