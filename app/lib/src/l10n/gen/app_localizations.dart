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

  /// No description provided for @startupFailedTitle.
  ///
  /// In en, this message translates to:
  /// **'AryaVault can\'t start'**
  String get startupFailedTitle;

  /// No description provided for @startupFailedBody.
  ///
  /// In en, this message translates to:
  /// **'The security core could not be loaded, or it does not match this version of the app. Reinstall AryaVault. Your vault file has not been touched.'**
  String get startupFailedBody;

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

  /// No description provided for @actionCancel.
  ///
  /// In en, this message translates to:
  /// **'Cancel'**
  String get actionCancel;

  /// No description provided for @typeLogin.
  ///
  /// In en, this message translates to:
  /// **'Login'**
  String get typeLogin;

  /// No description provided for @typeNote.
  ///
  /// In en, this message translates to:
  /// **'Secure note'**
  String get typeNote;

  /// No description provided for @typeCard.
  ///
  /// In en, this message translates to:
  /// **'Card'**
  String get typeCard;

  /// No description provided for @typeIdentity.
  ///
  /// In en, this message translates to:
  /// **'Identity'**
  String get typeIdentity;

  /// No description provided for @fieldTitle.
  ///
  /// In en, this message translates to:
  /// **'Title'**
  String get fieldTitle;

  /// No description provided for @fieldUsername.
  ///
  /// In en, this message translates to:
  /// **'Username'**
  String get fieldUsername;

  /// No description provided for @fieldPassword.
  ///
  /// In en, this message translates to:
  /// **'Password'**
  String get fieldPassword;

  /// No description provided for @fieldTotpSeed.
  ///
  /// In en, this message translates to:
  /// **'Authenticator code'**
  String get fieldTotpSeed;

  /// No description provided for @fieldNotes.
  ///
  /// In en, this message translates to:
  /// **'Notes'**
  String get fieldNotes;

  /// No description provided for @fieldBody.
  ///
  /// In en, this message translates to:
  /// **'Note'**
  String get fieldBody;

  /// No description provided for @fieldCardHolder.
  ///
  /// In en, this message translates to:
  /// **'Cardholder'**
  String get fieldCardHolder;

  /// No description provided for @fieldCardNumber.
  ///
  /// In en, this message translates to:
  /// **'Card number'**
  String get fieldCardNumber;

  /// No description provided for @fieldCardExpiry.
  ///
  /// In en, this message translates to:
  /// **'Expiry'**
  String get fieldCardExpiry;

  /// No description provided for @fieldCardCvv.
  ///
  /// In en, this message translates to:
  /// **'Security code'**
  String get fieldCardCvv;

  /// No description provided for @fieldCardPin.
  ///
  /// In en, this message translates to:
  /// **'PIN'**
  String get fieldCardPin;

  /// No description provided for @fieldFirstName.
  ///
  /// In en, this message translates to:
  /// **'First name'**
  String get fieldFirstName;

  /// No description provided for @fieldMiddleName.
  ///
  /// In en, this message translates to:
  /// **'Middle name'**
  String get fieldMiddleName;

  /// No description provided for @fieldLastName.
  ///
  /// In en, this message translates to:
  /// **'Last name'**
  String get fieldLastName;

  /// No description provided for @fieldEmail.
  ///
  /// In en, this message translates to:
  /// **'Email'**
  String get fieldEmail;

  /// No description provided for @fieldPhone.
  ///
  /// In en, this message translates to:
  /// **'Phone'**
  String get fieldPhone;

  /// No description provided for @fieldAddress.
  ///
  /// In en, this message translates to:
  /// **'Address'**
  String get fieldAddress;

  /// No description provided for @fieldIds.
  ///
  /// In en, this message translates to:
  /// **'ID numbers'**
  String get fieldIds;

  /// No description provided for @searchLabel.
  ///
  /// In en, this message translates to:
  /// **'Search'**
  String get searchLabel;

  /// No description provided for @searchClear.
  ///
  /// In en, this message translates to:
  /// **'Clear search'**
  String get searchClear;

  /// No description provided for @newItem.
  ///
  /// In en, this message translates to:
  /// **'New item'**
  String get newItem;

  /// No description provided for @newItemTitle.
  ///
  /// In en, this message translates to:
  /// **'New {type}'**
  String newItemTitle(String type);

  /// No description provided for @editItemTitle.
  ///
  /// In en, this message translates to:
  /// **'Edit item'**
  String get editItemTitle;

  /// No description provided for @itemListLabel.
  ///
  /// In en, this message translates to:
  /// **'Items'**
  String get itemListLabel;

  /// No description provided for @loadingItems.
  ///
  /// In en, this message translates to:
  /// **'Loading items…'**
  String get loadingItems;

  /// No description provided for @noMatches.
  ///
  /// In en, this message translates to:
  /// **'No matching items.'**
  String get noMatches;

  /// No description provided for @emptyVaultHint.
  ///
  /// In en, this message translates to:
  /// **'Add your first item with the + button.'**
  String get emptyVaultHint;

  /// No description provided for @resultsCount.
  ///
  /// In en, this message translates to:
  /// **'{count, plural, =1{1 item} other{{count} items}}'**
  String resultsCount(int count);

  /// No description provided for @resultsCountMore.
  ///
  /// In en, this message translates to:
  /// **'{count}+ items'**
  String resultsCountMore(int count);

  /// No description provided for @selectItemHint.
  ///
  /// In en, this message translates to:
  /// **'Select an item to see its details.'**
  String get selectItemHint;

  /// No description provided for @openMenu.
  ///
  /// In en, this message translates to:
  /// **'Open menu'**
  String get openMenu;

  /// No description provided for @sidebarLabel.
  ///
  /// In en, this message translates to:
  /// **'Vault navigation'**
  String get sidebarLabel;

  /// No description provided for @allItems.
  ///
  /// In en, this message translates to:
  /// **'All items'**
  String get allItems;

  /// No description provided for @favoritesView.
  ///
  /// In en, this message translates to:
  /// **'Favorites'**
  String get favoritesView;

  /// No description provided for @foldersTitle.
  ///
  /// In en, this message translates to:
  /// **'Folders'**
  String get foldersTitle;

  /// No description provided for @manageFolders.
  ///
  /// In en, this message translates to:
  /// **'Manage folders'**
  String get manageFolders;

  /// No description provided for @tagsLabel.
  ///
  /// In en, this message translates to:
  /// **'Tags'**
  String get tagsLabel;

  /// No description provided for @noTagsYet.
  ///
  /// In en, this message translates to:
  /// **'Tags appear here once items with tags have loaded.'**
  String get noTagsYet;

  /// No description provided for @trashTitle.
  ///
  /// In en, this message translates to:
  /// **'Trash'**
  String get trashTitle;

  /// No description provided for @trashRetention.
  ///
  /// In en, this message translates to:
  /// **'Items stay in the trash until their purge date, then they are deleted for good.'**
  String get trashRetention;

  /// No description provided for @trashEmpty.
  ///
  /// In en, this message translates to:
  /// **'The trash is empty.'**
  String get trashEmpty;

  /// No description provided for @purgesOn.
  ///
  /// In en, this message translates to:
  /// **'Deleted for good on {date}'**
  String purgesOn(String date);

  /// No description provided for @restore.
  ///
  /// In en, this message translates to:
  /// **'Restore'**
  String get restore;

  /// No description provided for @deleteForever.
  ///
  /// In en, this message translates to:
  /// **'Delete forever'**
  String get deleteForever;

  /// No description provided for @emptyTrash.
  ///
  /// In en, this message translates to:
  /// **'Empty trash'**
  String get emptyTrash;

  /// No description provided for @emptyTrashTitle.
  ///
  /// In en, this message translates to:
  /// **'Empty the trash?'**
  String get emptyTrashTitle;

  /// No description provided for @emptyTrashBody.
  ///
  /// In en, this message translates to:
  /// **'{count, plural, =1{1 item will be deleted for good.} other{{count} items will be deleted for good.}} This cannot be undone.'**
  String emptyTrashBody(int count);

  /// No description provided for @purgeTitle.
  ///
  /// In en, this message translates to:
  /// **'Delete forever?'**
  String get purgeTitle;

  /// No description provided for @purgeBody.
  ///
  /// In en, this message translates to:
  /// **'\"{title}\" will be deleted for good. This cannot be undone.'**
  String purgeBody(String title);

  /// No description provided for @movedToTrash.
  ///
  /// In en, this message translates to:
  /// **'\"{title}\" moved to the trash'**
  String movedToTrash(String title);

  /// No description provided for @undo.
  ///
  /// In en, this message translates to:
  /// **'Undo'**
  String get undo;

  /// No description provided for @favorite.
  ///
  /// In en, this message translates to:
  /// **'Add to favorites'**
  String get favorite;

  /// No description provided for @unfavorite.
  ///
  /// In en, this message translates to:
  /// **'Remove from favorites'**
  String get unfavorite;

  /// No description provided for @edit.
  ///
  /// In en, this message translates to:
  /// **'Edit'**
  String get edit;

  /// No description provided for @moveToTrash.
  ///
  /// In en, this message translates to:
  /// **'Move to trash'**
  String get moveToTrash;

  /// No description provided for @otherVersionsBadge.
  ///
  /// In en, this message translates to:
  /// **'Other versions'**
  String get otherVersionsBadge;

  /// No description provided for @history.
  ///
  /// In en, this message translates to:
  /// **'History'**
  String get history;

  /// No description provided for @copyField.
  ///
  /// In en, this message translates to:
  /// **'Copy {field}'**
  String copyField(String field);

  /// No description provided for @revealField.
  ///
  /// In en, this message translates to:
  /// **'Show {field}'**
  String revealField(String field);

  /// No description provided for @hideField.
  ///
  /// In en, this message translates to:
  /// **'Hide {field}'**
  String hideField(String field);

  /// No description provided for @hiddenValue.
  ///
  /// In en, this message translates to:
  /// **'Hidden value'**
  String get hiddenValue;

  /// No description provided for @copiedToast.
  ///
  /// In en, this message translates to:
  /// **'Copied; clears in {seconds} s'**
  String copiedToast(int seconds);

  /// No description provided for @totpLater.
  ///
  /// In en, this message translates to:
  /// **'Authenticator codes arrive in a later version.'**
  String get totpLater;

  /// No description provided for @urlLabel.
  ///
  /// In en, this message translates to:
  /// **'Website'**
  String get urlLabel;

  /// No description provided for @openUrl.
  ///
  /// In en, this message translates to:
  /// **'Open website'**
  String get openUrl;

  /// No description provided for @urlsLabel.
  ///
  /// In en, this message translates to:
  /// **'Websites'**
  String get urlsLabel;

  /// No description provided for @addUrl.
  ///
  /// In en, this message translates to:
  /// **'Add website'**
  String get addUrl;

  /// No description provided for @removeUrl.
  ///
  /// In en, this message translates to:
  /// **'Remove website'**
  String get removeUrl;

  /// No description provided for @folderLabel.
  ///
  /// In en, this message translates to:
  /// **'Folder'**
  String get folderLabel;

  /// No description provided for @noFolder.
  ///
  /// In en, this message translates to:
  /// **'No folder'**
  String get noFolder;

  /// No description provided for @createdOn.
  ///
  /// In en, this message translates to:
  /// **'Created {date}'**
  String createdOn(String date);

  /// No description provided for @updatedOn.
  ///
  /// In en, this message translates to:
  /// **'Updated {date}'**
  String updatedOn(String date);

  /// No description provided for @linkNotOpenable.
  ///
  /// In en, this message translates to:
  /// **'This link can\'t be opened.'**
  String get linkNotOpenable;

  /// No description provided for @openLinkTitle.
  ///
  /// In en, this message translates to:
  /// **'Open this link?'**
  String get openLinkTitle;

  /// No description provided for @openLinkBody.
  ///
  /// In en, this message translates to:
  /// **'This opens {url} in your browser.'**
  String openLinkBody(String url);

  /// No description provided for @openInsecureLinkTitle.
  ///
  /// In en, this message translates to:
  /// **'Open an unencrypted link?'**
  String get openInsecureLinkTitle;

  /// No description provided for @openInsecureLinkBody.
  ///
  /// In en, this message translates to:
  /// **'{url} does not use HTTPS, so what you send and receive can be read or changed on the way. Open it anyway?'**
  String openInsecureLinkBody(String url);

  /// No description provided for @openLinkConfirm.
  ///
  /// In en, this message translates to:
  /// **'Open'**
  String get openLinkConfirm;

  /// No description provided for @markdownImage.
  ///
  /// In en, this message translates to:
  /// **'image not loaded'**
  String get markdownImage;

  /// No description provided for @markdownTruncated.
  ///
  /// In en, this message translates to:
  /// **'This note is long; only the first part is shown.'**
  String get markdownTruncated;

  /// No description provided for @titleRequired.
  ///
  /// In en, this message translates to:
  /// **'A title is required.'**
  String get titleRequired;

  /// No description provided for @keepCurrentHint.
  ///
  /// In en, this message translates to:
  /// **'Leave empty to keep the current value.'**
  String get keepCurrentHint;

  /// No description provided for @willBeRemoved.
  ///
  /// In en, this message translates to:
  /// **'This value will be removed when you save.'**
  String get willBeRemoved;

  /// No description provided for @keepValue.
  ///
  /// In en, this message translates to:
  /// **'Keep the value'**
  String get keepValue;

  /// No description provided for @removeValue.
  ///
  /// In en, this message translates to:
  /// **'Remove {field}'**
  String removeValue(String field);

  /// No description provided for @generatePassword.
  ///
  /// In en, this message translates to:
  /// **'Generate a password'**
  String get generatePassword;

  /// No description provided for @sizeHint.
  ///
  /// In en, this message translates to:
  /// **'Using {used} of {max}'**
  String sizeHint(String used, String max);

  /// No description provided for @tagsCount.
  ///
  /// In en, this message translates to:
  /// **'Tags ({count} of {max})'**
  String tagsCount(int count, int max);

  /// No description provided for @addTagLabel.
  ///
  /// In en, this message translates to:
  /// **'New tag'**
  String get addTagLabel;

  /// No description provided for @addTag.
  ///
  /// In en, this message translates to:
  /// **'Add tag'**
  String get addTag;

  /// No description provided for @removeTag.
  ///
  /// In en, this message translates to:
  /// **'Remove tag {tag}'**
  String removeTag(String tag);

  /// No description provided for @customFieldsCount.
  ///
  /// In en, this message translates to:
  /// **'Custom fields ({count} of {max})'**
  String customFieldsCount(int count, int max);

  /// No description provided for @customLabel.
  ///
  /// In en, this message translates to:
  /// **'Label'**
  String get customLabel;

  /// No description provided for @customValue.
  ///
  /// In en, this message translates to:
  /// **'Value'**
  String get customValue;

  /// No description provided for @removeCustomField.
  ///
  /// In en, this message translates to:
  /// **'Remove custom field'**
  String get removeCustomField;

  /// No description provided for @kindText.
  ///
  /// In en, this message translates to:
  /// **'Text'**
  String get kindText;

  /// No description provided for @kindHidden.
  ///
  /// In en, this message translates to:
  /// **'Hidden'**
  String get kindHidden;

  /// No description provided for @kindUrl.
  ///
  /// In en, this message translates to:
  /// **'Website'**
  String get kindUrl;

  /// No description provided for @kindDate.
  ///
  /// In en, this message translates to:
  /// **'Date'**
  String get kindDate;

  /// No description provided for @addCustomField.
  ///
  /// In en, this message translates to:
  /// **'Add {kind} field'**
  String addCustomField(String kind);

  /// No description provided for @save.
  ///
  /// In en, this message translates to:
  /// **'Save'**
  String get save;

  /// No description provided for @saving.
  ///
  /// In en, this message translates to:
  /// **'Saving…'**
  String get saving;

  /// No description provided for @discardTitle.
  ///
  /// In en, this message translates to:
  /// **'Discard changes?'**
  String get discardTitle;

  /// No description provided for @discardBody.
  ///
  /// In en, this message translates to:
  /// **'You have unsaved changes. If you leave, they are lost.'**
  String get discardBody;

  /// No description provided for @discard.
  ///
  /// In en, this message translates to:
  /// **'Discard'**
  String get discard;

  /// No description provided for @historyTitle.
  ///
  /// In en, this message translates to:
  /// **'History: {title}'**
  String historyTitle(String title);

  /// No description provided for @noHistory.
  ///
  /// In en, this message translates to:
  /// **'No earlier versions of this field.'**
  String get noHistory;

  /// No description provided for @currentVersion.
  ///
  /// In en, this message translates to:
  /// **'Current'**
  String get currentVersion;

  /// No description provided for @otherVersion.
  ///
  /// In en, this message translates to:
  /// **'Other version'**
  String get otherVersion;

  /// No description provided for @concurrentVersion.
  ///
  /// In en, this message translates to:
  /// **'Edited on another device'**
  String get concurrentVersion;

  /// No description provided for @thisDevice.
  ///
  /// In en, this message translates to:
  /// **'This device'**
  String get thisDevice;

  /// No description provided for @otherDevice.
  ///
  /// In en, this message translates to:
  /// **'Another device'**
  String get otherDevice;

  /// No description provided for @restoreThisVersion.
  ///
  /// In en, this message translates to:
  /// **'Restore this version'**
  String get restoreThisVersion;

  /// No description provided for @versionRestored.
  ///
  /// In en, this message translates to:
  /// **'Version restored.'**
  String get versionRestored;

  /// No description provided for @compare.
  ///
  /// In en, this message translates to:
  /// **'Compare'**
  String get compare;

  /// No description provided for @compareTitle.
  ///
  /// In en, this message translates to:
  /// **'Compare: {field}'**
  String compareTitle(String field);

  /// No description provided for @keepAsSeparateNote.
  ///
  /// In en, this message translates to:
  /// **'Keep as separate note'**
  String get keepAsSeparateNote;

  /// No description provided for @keptAsNote.
  ///
  /// In en, this message translates to:
  /// **'Saved as a new note.'**
  String get keptAsNote;

  /// No description provided for @otherVersionNoteTitle.
  ///
  /// In en, this message translates to:
  /// **'{title} (other version)'**
  String otherVersionNoteTitle(String title);

  /// No description provided for @newFolder.
  ///
  /// In en, this message translates to:
  /// **'New folder'**
  String get newFolder;

  /// No description provided for @folderNameLabel.
  ///
  /// In en, this message translates to:
  /// **'Folder name'**
  String get folderNameLabel;

  /// No description provided for @create.
  ///
  /// In en, this message translates to:
  /// **'Create'**
  String get create;

  /// No description provided for @renameFolder.
  ///
  /// In en, this message translates to:
  /// **'Rename {name}'**
  String renameFolder(String name);

  /// No description provided for @renameFolderTitle.
  ///
  /// In en, this message translates to:
  /// **'Rename folder'**
  String get renameFolderTitle;

  /// No description provided for @deleteFolder.
  ///
  /// In en, this message translates to:
  /// **'Delete {name}'**
  String deleteFolder(String name);

  /// No description provided for @deleteFolderTitle.
  ///
  /// In en, this message translates to:
  /// **'Delete the folder \"{name}\"?'**
  String deleteFolderTitle(String name);

  /// No description provided for @deleteFolderBody.
  ///
  /// In en, this message translates to:
  /// **'The items in this folder are not deleted. They move to \"No folder\".'**
  String get deleteFolderBody;

  /// No description provided for @deleteFolderConfirm.
  ///
  /// In en, this message translates to:
  /// **'Delete folder'**
  String get deleteFolderConfirm;

  /// No description provided for @noFolders.
  ///
  /// In en, this message translates to:
  /// **'No folders yet.'**
  String get noFolders;

  /// No description provided for @navTools.
  ///
  /// In en, this message translates to:
  /// **'Tools'**
  String get navTools;

  /// No description provided for @navGenerator.
  ///
  /// In en, this message translates to:
  /// **'Password generator'**
  String get navGenerator;

  /// No description provided for @navHealth.
  ///
  /// In en, this message translates to:
  /// **'Password health'**
  String get navHealth;

  /// No description provided for @navImport.
  ///
  /// In en, this message translates to:
  /// **'Import'**
  String get navImport;

  /// No description provided for @navExport.
  ///
  /// In en, this message translates to:
  /// **'Export'**
  String get navExport;

  /// No description provided for @navSettings.
  ///
  /// In en, this message translates to:
  /// **'Settings'**
  String get navSettings;

  /// No description provided for @notAvailableBuild.
  ///
  /// In en, this message translates to:
  /// **'Not available on this build'**
  String get notAvailableBuild;

  /// No description provided for @notAvailableDevice.
  ///
  /// In en, this message translates to:
  /// **'Not available on this device'**
  String get notAvailableDevice;

  /// No description provided for @generatorTitle.
  ///
  /// In en, this message translates to:
  /// **'Password generator'**
  String get generatorTitle;

  /// No description provided for @generatorModePassword.
  ///
  /// In en, this message translates to:
  /// **'Password'**
  String get generatorModePassword;

  /// No description provided for @generatorModePassphrase.
  ///
  /// In en, this message translates to:
  /// **'Passphrase'**
  String get generatorModePassphrase;

  /// No description provided for @generatorLength.
  ///
  /// In en, this message translates to:
  /// **'Length: {n}'**
  String generatorLength(int n);

  /// No description provided for @generatorLower.
  ///
  /// In en, this message translates to:
  /// **'Lowercase letters (a-z)'**
  String get generatorLower;

  /// No description provided for @generatorUpper.
  ///
  /// In en, this message translates to:
  /// **'Uppercase letters (A-Z)'**
  String get generatorUpper;

  /// No description provided for @generatorDigits.
  ///
  /// In en, this message translates to:
  /// **'Digits (0-9)'**
  String get generatorDigits;

  /// No description provided for @generatorSymbols.
  ///
  /// In en, this message translates to:
  /// **'Symbols'**
  String get generatorSymbols;

  /// No description provided for @generatorSymbolSet.
  ///
  /// In en, this message translates to:
  /// **'Symbols to use'**
  String get generatorSymbolSet;

  /// No description provided for @generatorExcludeAmbiguous.
  ///
  /// In en, this message translates to:
  /// **'Leave out look-alike characters (O, 0, l, 1)'**
  String get generatorExcludeAmbiguous;

  /// No description provided for @generatorWords.
  ///
  /// In en, this message translates to:
  /// **'Words: {n}'**
  String generatorWords(int n);

  /// No description provided for @generatorSeparator.
  ///
  /// In en, this message translates to:
  /// **'Separator'**
  String get generatorSeparator;

  /// No description provided for @generatorCapitalize.
  ///
  /// In en, this message translates to:
  /// **'Capitalize each word'**
  String get generatorCapitalize;

  /// No description provided for @generatorNumber.
  ///
  /// In en, this message translates to:
  /// **'Add a number'**
  String get generatorNumber;

  /// No description provided for @generatorEntropy.
  ///
  /// In en, this message translates to:
  /// **'About {bits} bits of entropy'**
  String generatorEntropy(String bits);

  /// No description provided for @generatorNeedClass.
  ///
  /// In en, this message translates to:
  /// **'Turn on at least one kind of character.'**
  String get generatorNeedClass;

  /// No description provided for @generatorNeedSymbols.
  ///
  /// In en, this message translates to:
  /// **'Enter at least one symbol, or turn symbols off.'**
  String get generatorNeedSymbols;

  /// No description provided for @generatorRegenerate.
  ///
  /// In en, this message translates to:
  /// **'Generate another'**
  String get generatorRegenerate;

  /// No description provided for @generatorCopy.
  ///
  /// In en, this message translates to:
  /// **'Copy'**
  String get generatorCopy;

  /// No description provided for @generatorUse.
  ///
  /// In en, this message translates to:
  /// **'Use this password'**
  String get generatorUse;

  /// No description provided for @generatorShow.
  ///
  /// In en, this message translates to:
  /// **'Show the password'**
  String get generatorShow;

  /// No description provided for @generatorHide.
  ///
  /// In en, this message translates to:
  /// **'Hide the password'**
  String get generatorHide;

  /// No description provided for @generatorValueLabel.
  ///
  /// In en, this message translates to:
  /// **'Generated password'**
  String get generatorValueLabel;

  /// No description provided for @generatorFailed.
  ///
  /// In en, this message translates to:
  /// **'Couldn\'t generate a password.'**
  String get generatorFailed;

  /// No description provided for @generatorSheetTitle.
  ///
  /// In en, this message translates to:
  /// **'Generate a password'**
  String get generatorSheetTitle;

  /// No description provided for @healthTitle.
  ///
  /// In en, this message translates to:
  /// **'Password health'**
  String get healthTitle;

  /// No description provided for @healthIntro.
  ///
  /// In en, this message translates to:
  /// **'This checks the passwords in your vault on this device. Nothing is sent anywhere, and no password is shown here.'**
  String get healthIntro;

  /// No description provided for @healthLoading.
  ///
  /// In en, this message translates to:
  /// **'Checking your passwords…'**
  String get healthLoading;

  /// No description provided for @healthReusedTitle.
  ///
  /// In en, this message translates to:
  /// **'Reused passwords'**
  String get healthReusedTitle;

  /// No description provided for @healthReusedExplain.
  ///
  /// In en, this message translates to:
  /// **'The same password is used on more than one item. If one site leaks it, every item that shares it is at risk.'**
  String get healthReusedExplain;

  /// No description provided for @healthReusedNone.
  ///
  /// In en, this message translates to:
  /// **'No reused passwords.'**
  String get healthReusedNone;

  /// No description provided for @healthReusedGroup.
  ///
  /// In en, this message translates to:
  /// **'Shared by {count} items'**
  String healthReusedGroup(int count);

  /// No description provided for @healthWeakTitle.
  ///
  /// In en, this message translates to:
  /// **'Weak passwords'**
  String get healthWeakTitle;

  /// No description provided for @healthWeakExplain.
  ///
  /// In en, this message translates to:
  /// **'These passwords are easy to guess. Replace them with generated ones.'**
  String get healthWeakExplain;

  /// No description provided for @healthWeakNone.
  ///
  /// In en, this message translates to:
  /// **'No weak passwords.'**
  String get healthWeakNone;

  /// No description provided for @healthWeakScore.
  ///
  /// In en, this message translates to:
  /// **'Strength: {level}'**
  String healthWeakScore(String level);

  /// No description provided for @healthOldTitle.
  ///
  /// In en, this message translates to:
  /// **'Old passwords'**
  String get healthOldTitle;

  /// No description provided for @healthOldExplain.
  ///
  /// In en, this message translates to:
  /// **'These passwords have not changed for a long time. Changing them limits the damage of a leak you never heard about.'**
  String get healthOldExplain;

  /// No description provided for @healthOldNone.
  ///
  /// In en, this message translates to:
  /// **'No old passwords.'**
  String get healthOldNone;

  /// No description provided for @healthOldAge.
  ///
  /// In en, this message translates to:
  /// **'{days, plural, =1{Unchanged for 1 day} other{Unchanged for {days} days}}'**
  String healthOldAge(int days);

  /// No description provided for @healthItemGone.
  ///
  /// In en, this message translates to:
  /// **'Item no longer exists'**
  String get healthItemGone;

  /// No description provided for @reauthTitle.
  ///
  /// In en, this message translates to:
  /// **'Confirm it\'s you'**
  String get reauthTitle;

  /// No description provided for @reauthBody.
  ///
  /// In en, this message translates to:
  /// **'Enter your master password to continue.'**
  String get reauthBody;

  /// No description provided for @reauthConfirm.
  ///
  /// In en, this message translates to:
  /// **'Confirm'**
  String get reauthConfirm;

  /// No description provided for @reauthWrong.
  ///
  /// In en, this message translates to:
  /// **'That isn\'t your master password.'**
  String get reauthWrong;

  /// No description provided for @importTitle.
  ///
  /// In en, this message translates to:
  /// **'Import'**
  String get importTitle;

  /// No description provided for @importIntro.
  ///
  /// In en, this message translates to:
  /// **'Bring items in from a file. Nothing changes until you confirm the preview.'**
  String get importIntro;

  /// No description provided for @importFormatLabel.
  ///
  /// In en, this message translates to:
  /// **'File type'**
  String get importFormatLabel;

  /// No description provided for @importFormatCsv.
  ///
  /// In en, this message translates to:
  /// **'CSV file'**
  String get importFormatCsv;

  /// No description provided for @importFormatBitwarden.
  ///
  /// In en, this message translates to:
  /// **'Bitwarden export (JSON)'**
  String get importFormatBitwarden;

  /// No description provided for @importFormatEncrypted.
  ///
  /// In en, this message translates to:
  /// **'AryaVault encrypted export'**
  String get importFormatEncrypted;

  /// No description provided for @importChoose.
  ///
  /// In en, this message translates to:
  /// **'Choose a file…'**
  String get importChoose;

  /// No description provided for @importCancelledPick.
  ///
  /// In en, this message translates to:
  /// **'No file chosen.'**
  String get importCancelledPick;

  /// No description provided for @importReading.
  ///
  /// In en, this message translates to:
  /// **'Reading the file…'**
  String get importReading;

  /// No description provided for @importPreviewTitle.
  ///
  /// In en, this message translates to:
  /// **'Preview'**
  String get importPreviewTitle;

  /// No description provided for @importPreviewCounts.
  ///
  /// In en, this message translates to:
  /// **'{items, plural, =1{1 item} other{{items} items}} and {folders, plural, =1{1 folder} other{{folders} folders}} found'**
  String importPreviewCounts(int items, int folders);

  /// No description provided for @importPreviewDuplicates.
  ///
  /// In en, this message translates to:
  /// **'{count, plural, =1{1 item already exists in your vault} other{{count} items already exist in your vault}}'**
  String importPreviewDuplicates(int count);

  /// No description provided for @importPreviewNone.
  ///
  /// In en, this message translates to:
  /// **'Nothing to import in this file.'**
  String get importPreviewNone;

  /// No description provided for @importWarningsTitle.
  ///
  /// In en, this message translates to:
  /// **'Warnings'**
  String get importWarningsTitle;

  /// No description provided for @importSkippedTitle.
  ///
  /// In en, this message translates to:
  /// **'Skipped'**
  String get importSkippedTitle;

  /// No description provided for @importRecordLine.
  ///
  /// In en, this message translates to:
  /// **'Record {n}: {detail}'**
  String importRecordLine(int n, String detail);

  /// No description provided for @importMore.
  ///
  /// In en, this message translates to:
  /// **'and {count} more'**
  String importMore(int count);

  /// No description provided for @importSkipDuplicates.
  ///
  /// In en, this message translates to:
  /// **'Skip items that already exist'**
  String get importSkipDuplicates;

  /// No description provided for @importTargetFolder.
  ///
  /// In en, this message translates to:
  /// **'Put imported items in'**
  String get importTargetFolder;

  /// No description provided for @importCommit.
  ///
  /// In en, this message translates to:
  /// **'Import {count, plural, =1{1 item} other{{count} items}}'**
  String importCommit(int count);

  /// No description provided for @importCommitting.
  ///
  /// In en, this message translates to:
  /// **'Importing…'**
  String get importCommitting;

  /// No description provided for @importResultTitle.
  ///
  /// In en, this message translates to:
  /// **'Import finished'**
  String get importResultTitle;

  /// No description provided for @importResultCreated.
  ///
  /// In en, this message translates to:
  /// **'{count, plural, =1{1 item added} other{{count} items added}}'**
  String importResultCreated(int count);

  /// No description provided for @importResultFolders.
  ///
  /// In en, this message translates to:
  /// **'{count, plural, =1{1 folder created} other{{count} folders created}}'**
  String importResultFolders(int count);

  /// No description provided for @importResultDuplicates.
  ///
  /// In en, this message translates to:
  /// **'{count, plural, =1{1 duplicate skipped} other{{count} duplicates skipped}}'**
  String importResultDuplicates(int count);

  /// No description provided for @importResultInvalid.
  ///
  /// In en, this message translates to:
  /// **'{count, plural, =1{1 record was not valid} other{{count} records were not valid}}'**
  String importResultInvalid(int count);

  /// No description provided for @importResultSkipped.
  ///
  /// In en, this message translates to:
  /// **'{count, plural, =1{1 record skipped} other{{count} records skipped}}'**
  String importResultSkipped(int count);

  /// No description provided for @importAnother.
  ///
  /// In en, this message translates to:
  /// **'Import another file'**
  String get importAnother;

  /// No description provided for @importPasswordTitle.
  ///
  /// In en, this message translates to:
  /// **'Open the encrypted export'**
  String get importPasswordTitle;

  /// No description provided for @importPasswordBody.
  ///
  /// In en, this message translates to:
  /// **'Enter the password this export was protected with.'**
  String get importPasswordBody;

  /// No description provided for @importPasswordLabel.
  ///
  /// In en, this message translates to:
  /// **'Export password'**
  String get importPasswordLabel;

  /// No description provided for @importPasswordConfirm.
  ///
  /// In en, this message translates to:
  /// **'Import'**
  String get importPasswordConfirm;

  /// No description provided for @importDeleteAdvice.
  ///
  /// In en, this message translates to:
  /// **'You can delete the file you imported from. Passwords in it are not encrypted.'**
  String get importDeleteAdvice;

  /// No description provided for @exportTitle.
  ///
  /// In en, this message translates to:
  /// **'Export'**
  String get exportTitle;

  /// No description provided for @exportIntro.
  ///
  /// In en, this message translates to:
  /// **'Make a copy of your vault to keep or move to another app.'**
  String get exportIntro;

  /// No description provided for @exportEncryptedTitle.
  ///
  /// In en, this message translates to:
  /// **'Encrypted export (recommended)'**
  String get exportEncryptedTitle;

  /// No description provided for @exportEncryptedBody.
  ///
  /// In en, this message translates to:
  /// **'A file protected by a password you choose now. Only that password opens it.'**
  String get exportEncryptedBody;

  /// No description provided for @exportEncryptedButton.
  ///
  /// In en, this message translates to:
  /// **'Export encrypted file…'**
  String get exportEncryptedButton;

  /// No description provided for @exportCsvTitle.
  ///
  /// In en, this message translates to:
  /// **'Plain CSV (not encrypted)'**
  String get exportCsvTitle;

  /// No description provided for @exportCsvBody.
  ///
  /// In en, this message translates to:
  /// **'Every password and note in readable text. Use it only to move to another app.'**
  String get exportCsvBody;

  /// No description provided for @exportCsvButton.
  ///
  /// In en, this message translates to:
  /// **'Export CSV…'**
  String get exportCsvButton;

  /// No description provided for @exportPasswordTitle.
  ///
  /// In en, this message translates to:
  /// **'Choose an export password'**
  String get exportPasswordTitle;

  /// No description provided for @exportPasswordLabel.
  ///
  /// In en, this message translates to:
  /// **'Export password'**
  String get exportPasswordLabel;

  /// No description provided for @exportPasswordBody.
  ///
  /// In en, this message translates to:
  /// **'This password protects the exported file. It is separate from your master password.'**
  String get exportPasswordBody;

  /// No description provided for @exportKeepSafe.
  ///
  /// In en, this message translates to:
  /// **'Keep this password safe. Without it nobody can open the file, and we can\'t recover it for you.'**
  String get exportKeepSafe;

  /// No description provided for @exportPasswordSubmit.
  ///
  /// In en, this message translates to:
  /// **'Export'**
  String get exportPasswordSubmit;

  /// No description provided for @exportCsvWarnTitle.
  ///
  /// In en, this message translates to:
  /// **'Export without encryption?'**
  String get exportCsvWarnTitle;

  /// No description provided for @exportCsvWarnBody.
  ///
  /// In en, this message translates to:
  /// **'The file will contain all your passwords as plain text. Anyone who gets it can read them, including cloud backups and anyone using this computer. Delete it as soon as you have imported it elsewhere.'**
  String get exportCsvWarnBody;

  /// No description provided for @exportCsvTypePrompt.
  ///
  /// In en, this message translates to:
  /// **'To continue, type {phrase} below.'**
  String exportCsvTypePrompt(String phrase);

  /// No description provided for @exportCsvPhrase.
  ///
  /// In en, this message translates to:
  /// **'EXPORT'**
  String get exportCsvPhrase;

  /// No description provided for @exportCsvTypeLabel.
  ///
  /// In en, this message translates to:
  /// **'Confirmation'**
  String get exportCsvTypeLabel;

  /// No description provided for @exportCsvConfirm.
  ///
  /// In en, this message translates to:
  /// **'Export plain CSV'**
  String get exportCsvConfirm;

  /// No description provided for @exportWorking.
  ///
  /// In en, this message translates to:
  /// **'Preparing the file…'**
  String get exportWorking;

  /// No description provided for @exportSaved.
  ///
  /// In en, this message translates to:
  /// **'Saved.'**
  String get exportSaved;

  /// No description provided for @exportCancelled.
  ///
  /// In en, this message translates to:
  /// **'Not saved.'**
  String get exportCancelled;

  /// No description provided for @exportFailed.
  ///
  /// In en, this message translates to:
  /// **'Couldn\'t save the file.'**
  String get exportFailed;

  /// No description provided for @exportEncryptedFileName.
  ///
  /// In en, this message translates to:
  /// **'aryavault-export.avex'**
  String get exportEncryptedFileName;

  /// No description provided for @exportCsvFileName.
  ///
  /// In en, this message translates to:
  /// **'aryavault-export.csv'**
  String get exportCsvFileName;

  /// No description provided for @settingsTitle.
  ///
  /// In en, this message translates to:
  /// **'Settings'**
  String get settingsTitle;

  /// No description provided for @settingsSecurity.
  ///
  /// In en, this message translates to:
  /// **'Security'**
  String get settingsSecurity;

  /// No description provided for @settingsSecuritySub.
  ///
  /// In en, this message translates to:
  /// **'Auto-lock, clipboard, master password, recovery key'**
  String get settingsSecuritySub;

  /// No description provided for @settingsSync.
  ///
  /// In en, this message translates to:
  /// **'Sync & backup'**
  String get settingsSync;

  /// No description provided for @settingsSyncSub.
  ///
  /// In en, this message translates to:
  /// **'Coming soon'**
  String get settingsSyncSub;

  /// No description provided for @settingsAbout.
  ///
  /// In en, this message translates to:
  /// **'About'**
  String get settingsAbout;

  /// No description provided for @settingsAboutSub.
  ///
  /// In en, this message translates to:
  /// **'Version, licenses, diagnostics'**
  String get settingsAboutSub;

  /// No description provided for @settingsLoadFailed.
  ///
  /// In en, this message translates to:
  /// **'Couldn\'t read the settings.'**
  String get settingsLoadFailed;

  /// No description provided for @settingsSaveFailed.
  ///
  /// In en, this message translates to:
  /// **'Couldn\'t save that setting.'**
  String get settingsSaveFailed;

  /// No description provided for @secAutoLockTitle.
  ///
  /// In en, this message translates to:
  /// **'Lock automatically'**
  String get secAutoLockTitle;

  /// No description provided for @secAutoLockAfter.
  ///
  /// In en, this message translates to:
  /// **'After no activity for'**
  String get secAutoLockAfter;

  /// No description provided for @secAutoLockMinutes.
  ///
  /// In en, this message translates to:
  /// **'{n, plural, =1{1 minute} other{{n} minutes}}'**
  String secAutoLockMinutes(int n);

  /// No description provided for @secLockOnScreenLock.
  ///
  /// In en, this message translates to:
  /// **'When the screen locks'**
  String get secLockOnScreenLock;

  /// No description provided for @secLockOnSleep.
  ///
  /// In en, this message translates to:
  /// **'When the computer sleeps'**
  String get secLockOnSleep;

  /// No description provided for @secAutoLockNote.
  ///
  /// In en, this message translates to:
  /// **'There is no \"never\": a vault left open is the biggest risk.'**
  String get secAutoLockNote;

  /// No description provided for @secClipboardTitle.
  ///
  /// In en, this message translates to:
  /// **'Clipboard'**
  String get secClipboardTitle;

  /// No description provided for @secClipboardClear.
  ///
  /// In en, this message translates to:
  /// **'Clear copied passwords after'**
  String get secClipboardClear;

  /// No description provided for @secSeconds.
  ///
  /// In en, this message translates to:
  /// **'{n, plural, =1{1 second} other{{n} seconds}}'**
  String secSeconds(int n);

  /// No description provided for @secRevealHide.
  ///
  /// In en, this message translates to:
  /// **'Hide a shown password after'**
  String get secRevealHide;

  /// No description provided for @secCaptureTitle.
  ///
  /// In en, this message translates to:
  /// **'Screen capture'**
  String get secCaptureTitle;

  /// No description provided for @secCaptureBlock.
  ///
  /// In en, this message translates to:
  /// **'Block screenshots and screen recording'**
  String get secCaptureBlock;

  /// No description provided for @secCaptureNote.
  ///
  /// In en, this message translates to:
  /// **'Turn this off only if a screen reader or capture tool you rely on needs it.'**
  String get secCaptureNote;

  /// No description provided for @secQuickTitle.
  ///
  /// In en, this message translates to:
  /// **'Quick unlock'**
  String get secQuickTitle;

  /// No description provided for @secQuickToggle.
  ///
  /// In en, this message translates to:
  /// **'Unlock with {method}'**
  String secQuickToggle(String method);

  /// No description provided for @secQuickEnableBody.
  ///
  /// In en, this message translates to:
  /// **'Enter your master password to turn on quick unlock.'**
  String get secQuickEnableBody;

  /// No description provided for @secQuickOnlyPassword.
  ///
  /// In en, this message translates to:
  /// **'Quick unlock isn\'t available on this device or build. You unlock with your master password.'**
  String get secQuickOnlyPassword;

  /// No description provided for @secMasterTitle.
  ///
  /// In en, this message translates to:
  /// **'Master password'**
  String get secMasterTitle;

  /// No description provided for @secChangePassword.
  ///
  /// In en, this message translates to:
  /// **'Change master password'**
  String get secChangePassword;

  /// No description provided for @secRecoveryTitle.
  ///
  /// In en, this message translates to:
  /// **'Recovery key'**
  String get secRecoveryTitle;

  /// No description provided for @secViewRecovery.
  ///
  /// In en, this message translates to:
  /// **'View recovery key'**
  String get secViewRecovery;

  /// No description provided for @secRegenerateRecovery.
  ///
  /// In en, this message translates to:
  /// **'Make a new recovery key'**
  String get secRegenerateRecovery;

  /// No description provided for @secViewRecoveryTitle.
  ///
  /// In en, this message translates to:
  /// **'Your recovery key can\'t be shown again'**
  String get secViewRecoveryTitle;

  /// No description provided for @secViewRecoveryBody.
  ///
  /// In en, this message translates to:
  /// **'AryaVault never stores your recovery key: it was shown once, when you created it. If you can\'t find your copy, make a new one. The old key stops working right away.'**
  String get secViewRecoveryBody;

  /// No description provided for @secRegenerateTitle.
  ///
  /// In en, this message translates to:
  /// **'Make a new recovery key?'**
  String get secRegenerateTitle;

  /// No description provided for @secRegenerateBody.
  ///
  /// In en, this message translates to:
  /// **'Your current recovery key stops working immediately. You will have to write down and confirm the new one before you can continue.'**
  String get secRegenerateBody;

  /// No description provided for @secRegenerateConfirm.
  ///
  /// In en, this message translates to:
  /// **'Make new key'**
  String get secRegenerateConfirm;

  /// No description provided for @changePwTitle.
  ///
  /// In en, this message translates to:
  /// **'Change master password'**
  String get changePwTitle;

  /// No description provided for @changePwCurrent.
  ///
  /// In en, this message translates to:
  /// **'Current master password'**
  String get changePwCurrent;

  /// No description provided for @changePwNew.
  ///
  /// In en, this message translates to:
  /// **'New master password'**
  String get changePwNew;

  /// No description provided for @changePwRotate.
  ///
  /// In en, this message translates to:
  /// **'Also rotate keys (recommended after a suspected leak)'**
  String get changePwRotate;

  /// No description provided for @changePwRotateHelp.
  ///
  /// In en, this message translates to:
  /// **'Replaces the key that encrypts your vault, not just the password. You will get a new recovery key to save.'**
  String get changePwRotateHelp;

  /// No description provided for @changePwSubmit.
  ///
  /// In en, this message translates to:
  /// **'Change password'**
  String get changePwSubmit;

  /// No description provided for @changePwDone.
  ///
  /// In en, this message translates to:
  /// **'Master password changed.'**
  String get changePwDone;

  /// No description provided for @syncTitle.
  ///
  /// In en, this message translates to:
  /// **'Sync & backup'**
  String get syncTitle;

  /// No description provided for @syncComingSoon.
  ///
  /// In en, this message translates to:
  /// **'Sync is coming soon'**
  String get syncComingSoon;

  /// No description provided for @syncBody.
  ///
  /// In en, this message translates to:
  /// **'Encrypted sync through your own Google Drive, iCloud or a folder is not part of this version. Your vault lives only on this device. Until sync arrives, make backups with Export.'**
  String get syncBody;

  /// No description provided for @aboutTitle.
  ///
  /// In en, this message translates to:
  /// **'About AryaVault'**
  String get aboutTitle;

  /// No description provided for @aboutApp.
  ///
  /// In en, this message translates to:
  /// **'App version'**
  String get aboutApp;

  /// No description provided for @aboutCore.
  ///
  /// In en, this message translates to:
  /// **'Core version'**
  String get aboutCore;

  /// No description provided for @aboutApi.
  ///
  /// In en, this message translates to:
  /// **'API version'**
  String get aboutApi;

  /// No description provided for @aboutFormat.
  ///
  /// In en, this message translates to:
  /// **'Vault format version'**
  String get aboutFormat;

  /// No description provided for @aboutSchema.
  ///
  /// In en, this message translates to:
  /// **'Database schema version'**
  String get aboutSchema;

  /// No description provided for @aboutInfoFailed.
  ///
  /// In en, this message translates to:
  /// **'Couldn\'t read the core version.'**
  String get aboutInfoFailed;

  /// No description provided for @aboutLicenses.
  ///
  /// In en, this message translates to:
  /// **'Open-source licenses'**
  String get aboutLicenses;

  /// No description provided for @aboutLicensesRust.
  ///
  /// In en, this message translates to:
  /// **'Licenses of the Rust crates in the core will be listed here once the core is bundled with the app.'**
  String get aboutLicensesRust;

  /// No description provided for @aboutRepo.
  ///
  /// In en, this message translates to:
  /// **'Source code'**
  String get aboutRepo;

  /// No description provided for @aboutRepoUrl.
  ///
  /// In en, this message translates to:
  /// **'github.com/aryacodelabs/arya-vault'**
  String get aboutRepoUrl;

  /// No description provided for @aboutDiagnostics.
  ///
  /// In en, this message translates to:
  /// **'Diagnostics'**
  String get aboutDiagnostics;

  /// No description provided for @aboutDiagnosticsSub.
  ///
  /// In en, this message translates to:
  /// **'Review a scrubbed report before sharing it'**
  String get aboutDiagnosticsSub;

  /// No description provided for @diagTitle.
  ///
  /// In en, this message translates to:
  /// **'Diagnostics'**
  String get diagTitle;

  /// No description provided for @diagIntro.
  ///
  /// In en, this message translates to:
  /// **'This report has no passwords and no vault contents. Read it before you share it with anyone.'**
  String get diagIntro;

  /// No description provided for @diagPrepare.
  ///
  /// In en, this message translates to:
  /// **'Prepare report'**
  String get diagPrepare;

  /// No description provided for @diagSave.
  ///
  /// In en, this message translates to:
  /// **'Save report…'**
  String get diagSave;

  /// No description provided for @diagSaved.
  ///
  /// In en, this message translates to:
  /// **'Report saved.'**
  String get diagSaved;

  /// No description provided for @diagReportLabel.
  ///
  /// In en, this message translates to:
  /// **'Diagnostics report'**
  String get diagReportLabel;

  /// No description provided for @diagFileName.
  ///
  /// In en, this message translates to:
  /// **'aryavault-diagnostics.json'**
  String get diagFileName;

  /// No description provided for @errWrongExportPassword.
  ///
  /// In en, this message translates to:
  /// **'That password doesn\'t open this file, or the file is damaged.'**
  String get errWrongExportPassword;
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
