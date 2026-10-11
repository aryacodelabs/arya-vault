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
