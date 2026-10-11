import 'package:flutter/material.dart';

/// Design tokens that are not part of Material's ColorScheme.
@immutable
class AppTokens extends ThemeExtension<AppTokens> {
  const AppTokens({
    required this.contentMaxWidth,
    required this.gap,
    required this.gapLarge,
    required this.keyGroupBackground,
    required this.onKeyGroup,
    required this.warningBackground,
    required this.onWarning,
  });

  /// Maximum width of single-column pages (forms, onboarding).
  final double contentMaxWidth;
  final double gap;
  final double gapLarge;
  final Color keyGroupBackground;
  final Color onKeyGroup;
  final Color warningBackground;
  final Color onWarning;

  @override
  AppTokens copyWith({
    double? contentMaxWidth,
    double? gap,
    double? gapLarge,
    Color? keyGroupBackground,
    Color? onKeyGroup,
    Color? warningBackground,
    Color? onWarning,
  }) => AppTokens(
    contentMaxWidth: contentMaxWidth ?? this.contentMaxWidth,
    gap: gap ?? this.gap,
    gapLarge: gapLarge ?? this.gapLarge,
    keyGroupBackground: keyGroupBackground ?? this.keyGroupBackground,
    onKeyGroup: onKeyGroup ?? this.onKeyGroup,
    warningBackground: warningBackground ?? this.warningBackground,
    onWarning: onWarning ?? this.onWarning,
  );

  @override
  AppTokens lerp(ThemeExtension<AppTokens>? other, double t) =>
      other is AppTokens ? (t < 0.5 ? this : other) : this;
}

/// Contrast-checked palettes. `test/theme_contrast_test.dart` asserts WCAG AA
/// (4.5:1) for every foreground/background pair below, so edit them together.
abstract final class AppTheme {
  static const ColorScheme _lightScheme = ColorScheme(
    brightness: Brightness.light,
    primary: Color(0xFF1B4DB3),
    onPrimary: Color(0xFFFFFFFF),
    primaryContainer: Color(0xFFDCE6FF),
    onPrimaryContainer: Color(0xFF0B2A66),
    secondary: Color(0xFF3F4B63),
    onSecondary: Color(0xFFFFFFFF),
    secondaryContainer: Color(0xFFE3E8F2),
    onSecondaryContainer: Color(0xFF1B2333),
    error: Color(0xFFB3261E),
    onError: Color(0xFFFFFFFF),
    errorContainer: Color(0xFFF9DEDC),
    onErrorContainer: Color(0xFF410E0B),
    surface: Color(0xFFFFFFFF),
    onSurface: Color(0xFF1A1C20),
    onSurfaceVariant: Color(0xFF454B57),
    outline: Color(0xFF6B7280),
    outlineVariant: Color(0xFFC8CCD4),
    surfaceContainerHighest: Color(0xFFEDEFF3),
  );

  static const ColorScheme _darkScheme = ColorScheme(
    brightness: Brightness.dark,
    primary: Color(0xFFA9C2FF),
    onPrimary: Color(0xFF0B2A66),
    primaryContainer: Color(0xFF254A9A),
    onPrimaryContainer: Color(0xFFDCE6FF),
    secondary: Color(0xFFBCC6DB),
    onSecondary: Color(0xFF1B2333),
    secondaryContainer: Color(0xFF3A4458),
    onSecondaryContainer: Color(0xFFE3E8F2),
    error: Color(0xFFFFB4AB),
    onError: Color(0xFF410E0B),
    errorContainer: Color(0xFF8C1D18),
    onErrorContainer: Color(0xFFF9DEDC),
    surface: Color(0xFF121418),
    onSurface: Color(0xFFE4E6EB),
    onSurfaceVariant: Color(0xFFC0C5D0),
    outline: Color(0xFF8D93A0),
    outlineVariant: Color(0xFF3B4049),
    surfaceContainerHighest: Color(0xFF23262C),
  );

  static const AppTokens _lightTokens = AppTokens(
    contentMaxWidth: 520,
    gap: 16,
    gapLarge: 24,
    keyGroupBackground: Color(0xFFEDEFF3),
    onKeyGroup: Color(0xFF1A1C20),
    warningBackground: Color(0xFFFFF1CC),
    onWarning: Color(0xFF3D2E00),
  );

  static const AppTokens _darkTokens = AppTokens(
    contentMaxWidth: 520,
    gap: 16,
    gapLarge: 24,
    keyGroupBackground: Color(0xFF23262C),
    onKeyGroup: Color(0xFFE4E6EB),
    warningBackground: Color(0xFF4A3A00),
    onWarning: Color(0xFFFFE9A8),
  );

  static ThemeData get light => _build(_lightScheme, _lightTokens);
  static ThemeData get dark => _build(_darkScheme, _darkTokens);

  /// The palettes, exposed for the contrast test.
  static ColorScheme get lightScheme => _lightScheme;
  static ColorScheme get darkScheme => _darkScheme;
  static AppTokens get lightTokens => _lightTokens;
  static AppTokens get darkTokens => _darkTokens;

  static ThemeData _build(ColorScheme scheme, AppTokens tokens) {
    final OutlineInputBorder base = OutlineInputBorder(
      borderRadius: BorderRadius.circular(8),
      borderSide: BorderSide(color: scheme.outline),
    );
    return ThemeData(
      useMaterial3: true,
      colorScheme: scheme,
      scaffoldBackgroundColor: scheme.surface,
      // Visible keyboard focus everywhere (docs/07 §10).
      focusColor: scheme.primary.withValues(alpha: 0.24),
      inputDecorationTheme: InputDecorationTheme(
        border: base,
        enabledBorder: base,
        focusedBorder: base.copyWith(
          borderSide: BorderSide(color: scheme.primary, width: 2.5),
        ),
        errorBorder: base.copyWith(
          borderSide: BorderSide(color: scheme.error, width: 1.5),
        ),
        focusedErrorBorder: base.copyWith(
          borderSide: BorderSide(color: scheme.error, width: 2.5),
        ),
      ),
      filledButtonTheme: FilledButtonThemeData(
        style: FilledButton.styleFrom(minimumSize: const Size(64, 48))
            .copyWith(side: _focusSide(scheme)),
      ),
      outlinedButtonTheme: OutlinedButtonThemeData(
        style: OutlinedButton.styleFrom(minimumSize: const Size(64, 48))
            .copyWith(side: _focusSide(scheme, rest: scheme.outline)),
      ),
      textButtonTheme: TextButtonThemeData(
        style: TextButton.styleFrom(minimumSize: const Size(64, 48))
            .copyWith(side: _focusSide(scheme)),
      ),
      extensions: <ThemeExtension<dynamic>>[tokens],
    );
  }

  /// A 3 px ring in the primary color while a button has keyboard focus.
  static WidgetStateProperty<BorderSide?> _focusSide(
    ColorScheme scheme, {
    Color? rest,
  }) => WidgetStateProperty.resolveWith<BorderSide?>(
    (Set<WidgetState> states) => states.contains(WidgetState.focused)
        ? BorderSide(color: scheme.onSurface, width: 3)
        : (rest == null ? null : BorderSide(color: rest)),
  );
}

extension AppTokensContext on BuildContext {
  AppTokens get tokens => Theme.of(this).extension<AppTokens>()!;
}
