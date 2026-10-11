import 'dart:async';

import 'package:flutter/foundation.dart';
import 'package:flutter/widgets.dart';
import 'package:flutter_localizations/flutter_localizations.dart';
import 'package:intl/intl.dart' as intl;

import 'app_localizations_en.dart';

// ignore_for_file: type=lint

/// Callers can lookup localized strings with an instance of AppLocalizations
/// returned by `AppLocalizations.of(context)`.
///
/// Applications need to include `AppLocalizations.delegate()` in their app's
/// `localizationDelegates` list, and the locales they support in the app's
/// `supportedLocales` list. For example:
///
/// ```dart
/// import 'gen/app_localizations.dart';
///
/// return MaterialApp(
///   localizationsDelegates: AppLocalizations.localizationsDelegates,
///   supportedLocales: AppLocalizations.supportedLocales,
///   home: MyApplicationHome(),
/// );
/// ```
///
/// ## Update pubspec.yaml
///
/// Please make sure to update your pubspec.yaml to include the following
/// packages:
///
/// ```yaml
/// dependencies:
///   # Internationalization support.
///   flutter_localizations:
///     sdk: flutter
///   intl: any # Use the pinned version from flutter_localizations
///
///   # Rest of dependencies
/// ```
///
/// ## iOS Applications
///
/// iOS applications define key application metadata, including supported
/// locales, in an Info.plist file that is built into the application bundle.
/// To configure the locales supported by your app, you’ll need to edit this
/// file.
///
/// First, open your project’s ios/Runner.xcworkspace Xcode workspace file.
/// Then, in the Project Navigator, open the Info.plist file under the Runner
/// project’s Runner folder.
///
/// Next, select the Information Property List item, select Add Item from the
/// Editor menu, then select Localizations from the pop-up menu.
///
/// Select and expand the newly-created Localizations item then, for each
/// locale your application supports, add a new item and select the locale
/// you wish to add from the pop-up menu in the Value field. This list should
/// be consistent with the languages listed in the AppLocalizations.supportedLocales
/// property.
abstract class AppLocalizations {
  AppLocalizations(String locale)
    : localeName = intl.Intl.canonicalizedLocale(locale.toString());

  final String localeName;

  static AppLocalizations of(BuildContext context) {
    return Localizations.of<AppLocalizations>(context, AppLocalizations)!;
  }

  static const LocalizationsDelegate<AppLocalizations> delegate =
      _AppLocalizationsDelegate();

  /// A list of this localizations delegate along with the default localizations
  /// delegates.
  ///
  /// Returns a list of localizations delegates containing this delegate along with
  /// GlobalMaterialLocalizations.delegate, GlobalCupertinoLocalizations.delegate,
  /// and GlobalWidgetsLocalizations.delegate.
  ///
  /// Additional delegates can be added by appending to this list in
  /// MaterialApp. This list does not have to be used at all if a custom list
  /// of delegates is preferred or required.
  static const List<LocalizationsDelegate<dynamic>> localizationsDelegates =
      <LocalizationsDelegate<dynamic>>[
        delegate,
        GlobalMaterialLocalizations.delegate,
        GlobalCupertinoLocalizations.delegate,
        GlobalWidgetsLocalizations.delegate,
      ];

  /// A list of this localizations delegate's supported locales.
  static const List<Locale> supportedLocales = <Locale>[Locale('en')];

  /// No description provided for @appName.
  ///
  /// In en, this message translates to:
  /// **'AryaVault'**
  String get appName;

  /// No description provided for @actionContinue.
  ///
  /// In en, this message translates to:
  /// **'Continue'**
  String get actionContinue;

  /// No description provided for @actionBack.
  ///
  /// In en, this message translates to:
  /// **'Back'**
  String get actionBack;

  /// No description provided for @actionTryAgain.
  ///
  /// In en, this message translates to:
  /// **'Try again'**
  String get actionTryAgain;

  /// No description provided for @actionLock.
  ///
  /// In en, this message translates to:
  /// **'Lock now'**
  String get actionLock;

  /// No description provided for @showPassword.
  ///
  /// In en, this message translates to:
  /// **'Show password'**
  String get showPassword;

  /// No description provided for @hidePassword.
  ///
  /// In en, this message translates to:
  /// **'Hide password'**
  String get hidePassword;

  /// No description provided for @loadingVault.
  ///
  /// In en, this message translates to:
  /// **'Loading your vault…'**
  String get loadingVault;

  /// No description provided for @loadFailed.
  ///
  /// In en, this message translates to:
  /// **'Couldn\'t read the vault status.'**
  String get loadFailed;

  /// No description provided for @welcomeTitle.
  ///
  /// In en, this message translates to:
  /// **'Welcome to AryaVault'**
  String get welcomeTitle;

  /// No description provided for @welcomeTagline.
  ///
  /// In en, this message translates to:
  /// **'Your passwords and notes, encrypted on this device. No account and no server.'**
  String get welcomeTagline;

  /// No description provided for @welcomeCreate.
  ///
  /// In en, this message translates to:
  /// **'Create a new vault'**
  String get welcomeCreate;

  /// No description provided for @welcomeAddDevice.
  ///
  /// In en, this message translates to:
  /// **'Add this device to an existing vault'**
  String get welcomeAddDevice;

  /// No description provided for @welcomeAddDeviceUnavailable.
  ///
  /// In en, this message translates to:
  /// **'Available with sync'**
  String get welcomeAddDeviceUnavailable;

  /// No description provided for @createTitle.
  ///
  /// In en, this message translates to:
  /// **'Create your vault'**
  String get createTitle;

  /// No description provided for @createIntro.
  ///
  /// In en, this message translates to:
  /// **'Choose a master password. It is the only thing that unlocks your vault on this device.'**
  String get createIntro;

  /// No description provided for @masterPasswordLabel.
  ///
  /// In en, this message translates to:
  /// **'Master password'**
  String get masterPasswordLabel;

  /// No description provided for @confirmPasswordLabel.
  ///
  /// In en, this message translates to:
  /// **'Confirm master password'**
  String get confirmPasswordLabel;

  /// No description provided for @passwordMismatch.
  ///
  /// In en, this message translates to:
  /// **'The two passwords don\'t match.'**
  String get passwordMismatch;

  /// No description provided for @passphraseTip.
  ///
  /// In en, this message translates to:
  /// **'Tip: a passphrase of 16 or more characters is much stronger.'**
  String get passphraseTip;

  /// No description provided for @createSubmit.
  ///
  /// In en, this message translates to:
  /// **'Create vault'**
  String get createSubmit;

  /// No description provided for @createWorking.
  ///
  /// In en, this message translates to:
  /// **'Creating your vault…'**
  String get createWorking;

  /// No description provided for @advancedTitle.
  ///
  /// In en, this message translates to:
  /// **'Advanced'**
  String get advancedTitle;

  /// No description provided for @kdfLabel.
  ///
  /// In en, this message translates to:
  /// **'Key-derivation strength'**
  String get kdfLabel;

  /// No description provided for @kdfHelp.
  ///
  /// In en, this message translates to:
  /// **'Stronger settings make password guessing slower for an attacker but use more memory. Choose Low only on very old devices.'**
  String get kdfHelp;

  /// No description provided for @kdfLow.
  ///
  /// In en, this message translates to:
  /// **'Low memory use'**
  String get kdfLow;

  /// No description provided for @kdfDefault.
  ///
  /// In en, this message translates to:
  /// **'Balanced (recommended)'**
  String get kdfDefault;

  /// No description provided for @kdfHigh.
  ///
  /// In en, this message translates to:
  /// **'High (slower, more memory)'**
  String get kdfHigh;

  /// No description provided for @strengthLabel.
  ///
  /// In en, this message translates to:
  /// **'Password strength'**
  String get strengthLabel;

  /// No description provided for @strengthVeryWeak.
  ///
  /// In en, this message translates to:
  /// **'Very weak'**
  String get strengthVeryWeak;

  /// No description provided for @strengthWeak.
  ///
  /// In en, this message translates to:
  /// **'Weak'**
  String get strengthWeak;

  /// No description provided for @strengthFair.
  ///
  /// In en, this message translates to:
  /// **'Fair'**
  String get strengthFair;

  /// No description provided for @strengthGood.
  ///
  /// In en, this message translates to:
  /// **'Good'**
  String get strengthGood;

  /// No description provided for @strengthStrong.
  ///
  /// In en, this message translates to:
  /// **'Strong'**
  String get strengthStrong;

  /// No description provided for @strengthSemantics.
  ///
  /// In en, this message translates to:
  /// **'Password strength: {level}'**
  String strengthSemantics(String level);

  /// No description provided for @policyMet.
  ///
  /// In en, this message translates to:
  /// **'Meets the requirements.'**
  String get policyMet;

  /// No description provided for @policyEmpty.
  ///
  /// In en, this message translates to:
  /// **'Enter a password of at least 12 characters.'**
  String get policyEmpty;

  /// No description provided for @explainerTitle.
  ///
  /// In en, this message translates to:
  /// **'There is no password reset'**
  String get explainerTitle;

  /// No description provided for @explainerBody.
  ///
  /// In en, this message translates to:
  /// **'AryaVault can\'t reset your password — we don\'t have your data or a server. Your recovery key is the only backup. Store it somewhere safe and separate from this device.'**
  String get explainerBody;

  /// No description provided for @explainerPoint.
  ///
  /// In en, this message translates to:
  /// **'Without your master password or your recovery key, nobody can open your data, including us.'**
  String get explainerPoint;

  /// No description provided for @explainerContinue.
  ///
  /// In en, this message translates to:
  /// **'Show my recovery key'**
  String get explainerContinue;

  /// No description provided for @recoveryTitle.
  ///
  /// In en, this message translates to:
  /// **'Your recovery key'**
  String get recoveryTitle;

  /// No description provided for @recoveryIntro.
  ///
  /// In en, this message translates to:
  /// **'Write it down or print it, and keep it somewhere safe and separate from this device. It is shown only now.'**
  String get recoveryIntro;

  /// No description provided for @recoveryShowKey.
  ///
  /// In en, this message translates to:
  /// **'Show key'**
  String get recoveryShowKey;

  /// No description provided for @recoveryHideKey.
  ///
  /// In en, this message translates to:
  /// **'Hide key'**
  String get recoveryHideKey;

  /// No description provided for @recoveryHiddenPlaceholder.
  ///
  /// In en, this message translates to:
  /// **'Recovery key hidden'**
  String get recoveryHiddenPlaceholder;

  /// No description provided for @recoveryGroupSemantics.
  ///
  /// In en, this message translates to:
  /// **'Group {n} of {total}: {chars}'**
  String recoveryGroupSemantics(int n, int total, String chars);

  /// No description provided for @recoveryChecksumSemantics.
  ///
  /// In en, this message translates to:
  /// **'Checksum group: {chars}'**
  String recoveryChecksumSemantics(String chars);

  /// No description provided for @recoveryChecksumLabel.
  ///
  /// In en, this message translates to:
  /// **'Checksum'**
  String get recoveryChecksumLabel;

  /// No description provided for @recoveryPrint.
  ///
  /// In en, this message translates to:
  /// **'Print'**
  String get recoveryPrint;

  /// No description provided for @recoverySave.
  ///
  /// In en, this message translates to:
  /// **'Save as file'**
  String get recoverySave;

  /// No description provided for @recoveryNoCopy.
  ///
  /// In en, this message translates to:
  /// **'This screen has no copy button on purpose: other apps can read the clipboard.'**
  String get recoveryNoCopy;

  /// No description provided for @recoveryCaptureWarning.
  ///
  /// In en, this message translates to:
  /// **'Don\'t screenshot or photograph this key into a cloud-synced folder. Anyone who sees it, together with a copy of your vault file, can open your vault.'**
  String get recoveryCaptureWarning;

  /// No description provided for @recoveryStoredCheck.
  ///
  /// In en, this message translates to:
  /// **'I stored it'**
  String get recoveryStoredCheck;

  /// No description provided for @recoveryContinue.
  ///
  /// In en, this message translates to:
  /// **'Continue'**
  String get recoveryContinue;

  /// No description provided for @recoverySaved.
  ///
  /// In en, this message translates to:
  /// **'Saved.'**
  String get recoverySaved;

  /// No description provided for @recoverySaveFailed.
  ///
  /// In en, this message translates to:
  /// **'Couldn\'t save the file.'**
  String get recoverySaveFailed;

  /// No description provided for @recoveryPrintFailed.
  ///
  /// In en, this message translates to:
  /// **'Couldn\'t print.'**
  String get recoveryPrintFailed;

  /// No description provided for @recoveryServiceUnavailable.
  ///
  /// In en, this message translates to:
  /// **'This is not available in this build yet.'**
  String get recoveryServiceUnavailable;

  /// No description provided for @recoverySheetTitle.
  ///
  /// In en, this message translates to:
  /// **'AryaVault recovery key'**
  String get recoverySheetTitle;

  /// No description provided for @recoverySheetBody.
  ///
  /// In en, this message translates to:
  /// **'Keep this sheet somewhere safe, separate from your device. Anyone with this key and a copy of your vault file can open your vault.'**
  String get recoverySheetBody;

  /// No description provided for @recoverySheetKeyLabel.
  ///
  /// In en, this message translates to:
  /// **'Recovery key:'**
  String get recoverySheetKeyLabel;

  /// No description provided for @recoveryFileName.
  ///
  /// In en, this message translates to:
  /// **'aryavault-recovery-key.txt'**
  String get recoveryFileName;

  /// No description provided for @recoveryRegenTitle.
  ///
  /// In en, this message translates to:
  /// **'Create a new recovery key'**
  String get recoveryRegenTitle;

  /// No description provided for @recoveryRegenBody.
  ///
  /// In en, this message translates to:
  /// **'Your earlier recovery key was never confirmed, so it can\'t be shown again. Enter your master password to create a new one.'**
  String get recoveryRegenBody;

  /// No description provided for @recoveryRegenSubmit.
  ///
  /// In en, this message translates to:
  /// **'Create recovery key'**
  String get recoveryRegenSubmit;

  /// No description provided for @verifyTitle.
  ///
  /// In en, this message translates to:
  /// **'Check your recovery key'**
  String get verifyTitle;

  /// No description provided for @verifyIntro.
  ///
  /// In en, this message translates to:
  /// **'Type the groups below from your saved copy. This proves you can use it later.'**
  String get verifyIntro;

  /// No description provided for @verifyGroupField.
  ///
  /// In en, this message translates to:
  /// **'Group {n}'**
  String verifyGroupField(int n);

  /// No description provided for @verifyGroupHelp.
  ///
  /// In en, this message translates to:
  /// **'Five letters or digits'**
  String get verifyGroupHelp;

  /// No description provided for @verifySubmit.
  ///
  /// In en, this message translates to:
  /// **'Verify'**
  String get verifySubmit;

  /// No description provided for @verifyMismatch.
  ///
  /// In en, this message translates to:
  /// **'That doesn\'t match. Look at your key again, then retry.'**
  String get verifyMismatch;

  /// No description provided for @verifyShowKeyAgain.
  ///
  /// In en, this message translates to:
  /// **'Show my key again'**
  String get verifyShowKeyAgain;

  /// No description provided for @quickOfferTitle.
  ///
  /// In en, this message translates to:
  /// **'Unlock faster?'**
  String get quickOfferTitle;

  /// No description provided for @quickOfferBody.
  ///
  /// In en, this message translates to:
  /// **'Unlock this vault with {method} instead of typing your master password. You will still need your master password after a restart.'**
  String quickOfferBody(String method);

  /// No description provided for @quickEnable.
  ///
  /// In en, this message translates to:
  /// **'Turn on'**
  String get quickEnable;

  /// No description provided for @quickNotNow.
  ///
  /// In en, this message translates to:
  /// **'Not now'**
  String get quickNotNow;

  /// No description provided for @quickKindWindowsHello.
  ///
  /// In en, this message translates to:
  /// **'Windows Hello'**
  String get quickKindWindowsHello;

  /// No description provided for @quickKindTouchId.
  ///
  /// In en, this message translates to:
  /// **'Touch ID'**
  String get quickKindTouchId;

  /// No description provided for @quickKindFaceId.
  ///
  /// In en, this message translates to:
  /// **'Face ID'**
  String get quickKindFaceId;

  /// No description provided for @quickKindBiometric.
  ///
  /// In en, this message translates to:
  /// **'biometrics'**
  String get quickKindBiometric;

  /// No description provided for @quickKindOsKeyring.
  ///
  /// In en, this message translates to:
  /// **'your system keyring'**
  String get quickKindOsKeyring;

  /// No description provided for @quickKindNone.
  ///
  /// In en, this message translates to:
  /// **'quick unlock'**
  String get quickKindNone;

  /// No description provided for @lockTitle.
  ///
  /// In en, this message translates to:
  /// **'Unlock your vault'**
  String get lockTitle;

  /// No description provided for @lockSubmit.
  ///
  /// In en, this message translates to:
  /// **'Unlock'**
  String get lockSubmit;

  /// No description provided for @lockWorking.
  ///
  /// In en, this message translates to:
  /// **'Unlocking…'**
  String get lockWorking;

  /// No description provided for @lockQuick.
  ///
  /// In en, this message translates to:
  /// **'Unlock with {method}'**
  String lockQuick(String method);

  /// No description provided for @lockUseRecovery.
  ///
  /// In en, this message translates to:
  /// **'Use recovery key'**
  String get lockUseRecovery;

  /// No description provided for @recoverTitle.
  ///
  /// In en, this message translates to:
  /// **'Recover with your recovery key'**
  String get recoverTitle;

  /// No description provided for @recoverKeyLabel.
  ///
  /// In en, this message translates to:
  /// **'Recovery key'**
  String get recoverKeyLabel;

  /// No description provided for @recoverKeyHint.
  ///
  /// In en, this message translates to:
  /// **'Spaces, dashes and lowercase letters are fine.'**
  String get recoverKeyHint;

  /// No description provided for @recoverNewPasswordTitle.
  ///
  /// In en, this message translates to:
  /// **'Choose a new master password'**
  String get recoverNewPasswordTitle;

  /// No description provided for @recoverSubmit.
  ///
  /// In en, this message translates to:
  /// **'Recover vault'**
  String get recoverSubmit;

  /// No description provided for @recoverNoKey.
  ///
  /// In en, this message translates to:
  /// **'I don\'t have my recovery key'**
  String get recoverNoKey;

  /// No description provided for @forgotTitle.
  ///
  /// In en, this message translates to:
  /// **'If you have neither'**
  String get forgotTitle;

  /// No description provided for @forgotBody1.
  ///
  /// In en, this message translates to:
  /// **'Your data is encrypted with keys protected by your master password and your recovery key. Without either one, nobody can recover it, including us.'**
  String get forgotBody1;

  /// No description provided for @forgotBody2.
  ///
  /// In en, this message translates to:
  /// **'If you remember an older master password, an older snapshot of your vault may still open with it.'**
  String get forgotBody2;

  /// No description provided for @forgotBody3.
  ///
  /// In en, this message translates to:
  /// **'As a last resort you will be able to erase this device\'s vault and start fresh. Erasing is permanent. This option comes in a later version.'**
  String get forgotBody3;

  /// No description provided for @homeTitle.
  ///
  /// In en, this message translates to:
  /// **'Your vault'**
  String get homeTitle;

  /// No description provided for @homeUnlockedMessage.
  ///
  /// In en, this message translates to:
  /// **'Your vault is unlocked.'**
  String get homeUnlockedMessage;

  /// No description provided for @homeListPlaceholder.
  ///
  /// In en, this message translates to:
  /// **'Items will appear here.'**
  String get homeListPlaceholder;

  /// No description provided for @homeDetailPlaceholder.
  ///
  /// In en, this message translates to:
  /// **'Select an item to see its details.'**
  String get homeDetailPlaceholder;

  /// No description provided for @navVault.
  ///
  /// In en, this message translates to:
  /// **'Vault'**
  String get navVault;

  /// No description provided for @navGenerator.
  ///
  /// In en, this message translates to:
  /// **'Generator'**
  String get navGenerator;

  /// No description provided for @navHealth.
  ///
  /// In en, this message translates to:
  /// **'Health'**
  String get navHealth;

  /// No description provided for @navSettings.
  ///
  /// In en, this message translates to:
  /// **'Settings'**
  String get navSettings;

  /// No description provided for @errWrongCredentials.
  ///
  /// In en, this message translates to:
  /// **'That didn\'t work. Check your password and try again.'**
  String get errWrongCredentials;

  /// No description provided for @errWrongRecoveryKey.
  ///
  /// In en, this message translates to:
  /// **'That recovery key doesn\'t open this vault.'**
  String get errWrongRecoveryKey;

  /// No description provided for @errRecoveryKeyMalformed.
  ///
  /// In en, this message translates to:
  /// **'That recovery key doesn\'t look right. Check it for typos and try again.'**
  String get errRecoveryKeyMalformed;

  /// No description provided for @errWeakPassword.
  ///
  /// In en, this message translates to:
  /// **'That password is too weak.'**
  String get errWeakPassword;

  /// No description provided for @errLocked.
  ///
  /// In en, this message translates to:
  /// **'The vault is locked.'**
  String get errLocked;

  /// No description provided for @errNotFound.
  ///
  /// In en, this message translates to:
  /// **'That item no longer exists.'**
  String get errNotFound;

  /// No description provided for @errValidation.
  ///
  /// In en, this message translates to:
  /// **'Some input isn\'t valid.'**
  String get errValidation;

  /// No description provided for @errLimitReached.
  ///
  /// In en, this message translates to:
  /// **'A size or count limit was reached.'**
  String get errLimitReached;

  /// No description provided for @errCorruptVault.
  ///
  /// In en, this message translates to:
  /// **'The vault data failed an integrity check. It was not opened.'**
  String get errCorruptVault;

  /// No description provided for @errUnsupportedFormat.
  ///
  /// In en, this message translates to:
  /// **'This vault was made by a newer version of AryaVault. Update the app to open it.'**
  String get errUnsupportedFormat;

  /// No description provided for @errQuickUnlockUnavailable.
  ///
  /// In en, this message translates to:
  /// **'Quick unlock isn\'t available right now. Use your master password.'**
  String get errQuickUnlockUnavailable;

  /// No description provided for @errAlreadyExists.
  ///
  /// In en, this message translates to:
  /// **'A vault already exists here.'**
  String get errAlreadyExists;

  /// No description provided for @errIo.
  ///
  /// In en, this message translates to:
  /// **'Couldn\'t read or write the vault files.'**
  String get errIo;

  /// No description provided for @errBusy.
  ///
  /// In en, this message translates to:
  /// **'Too many attempts. Wait a moment, then try again.'**
  String get errBusy;

  /// No description provided for @errInternal.
  ///
  /// In en, this message translates to:
  /// **'Something went wrong.'**
  String get errInternal;
}

class _AppLocalizationsDelegate
    extends LocalizationsDelegate<AppLocalizations> {
  const _AppLocalizationsDelegate();

  @override
  Future<AppLocalizations> load(Locale locale) {
    return SynchronousFuture<AppLocalizations>(lookupAppLocalizations(locale));
  }

  @override
  bool isSupported(Locale locale) =>
      <String>['en'].contains(locale.languageCode);

  @override
  bool shouldReload(_AppLocalizationsDelegate old) => false;
}

AppLocalizations lookupAppLocalizations(Locale locale) {
  // Lookup logic when only language code is specified.
  switch (locale.languageCode) {
    case 'en':
      return AppLocalizationsEn();
  }

  throw FlutterError(
    'AppLocalizations.delegate failed to load unsupported locale "$locale". This is likely '
    'an issue with the localizations generation tool. Please file an issue '
    'on GitHub with a reproducible sample app and the gen-l10n configuration '
    'that was used.',
  );
}
