import 'dart:math';

import 'package:arya_vault/src/theme/app_theme.dart';
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

double _lin(double c) =>
    c <= 0.03928 ? c / 12.92 : pow((c + 0.055) / 1.055, 2.4).toDouble();

double _luminance(Color c) =>
    0.2126 * _lin(c.r) + 0.7152 * _lin(c.g) + 0.0722 * _lin(c.b);

/// WCAG 2.x contrast ratio.
double contrast(Color a, Color b) {
  final double la = _luminance(a);
  final double lb = _luminance(b);
  return (max(la, lb) + 0.05) / (min(la, lb) + 0.05);
}

void main() {
  test('the contrast helper matches known values', () {
    expect(contrast(Colors.black, Colors.white), closeTo(21, 0.01));
    expect(contrast(Colors.white, Colors.white), closeTo(1, 0.001));
  });

  for (final (String, ColorScheme, AppTokens) t
      in <(String, ColorScheme, AppTokens)>[
        ('light', AppTheme.lightScheme, AppTheme.lightTokens),
        ('dark', AppTheme.darkScheme, AppTheme.darkTokens),
      ]) {
    final String mode = t.$1;
    final ColorScheme s = t.$2;
    final AppTokens k = t.$3;

    test('$mode: text pairs meet WCAG AA (4.5:1)', () {
      final Map<String, (Color, Color)> pairs = <String, (Color, Color)>{
        'onSurface/surface': (s.onSurface, s.surface),
        'onSurfaceVariant/surface': (s.onSurfaceVariant, s.surface),
        'primary/surface (text buttons, links)': (s.primary, s.surface),
        'onPrimary/primary (filled buttons)': (s.onPrimary, s.primary),
        'onPrimaryContainer/primaryContainer': (
          s.onPrimaryContainer,
          s.primaryContainer,
        ),
        'onSecondary/secondary': (s.onSecondary, s.secondary),
        'onSecondaryContainer/secondaryContainer': (
          s.onSecondaryContainer,
          s.secondaryContainer,
        ),
        'error/surface (error text)': (s.error, s.surface),
        'onError/error': (s.onError, s.error),
        'onErrorContainer/errorContainer': (
          s.onErrorContainer,
          s.errorContainer,
        ),
        'onSurface/surfaceContainerHighest': (
          s.onSurface,
          s.surfaceContainerHighest,
        ),
        'onSurfaceVariant/surfaceContainerHighest': (
          s.onSurfaceVariant,
          s.surfaceContainerHighest,
        ),
        'onKeyGroup/keyGroupBackground': (k.onKeyGroup, k.keyGroupBackground),
        'onWarning/warningBackground': (k.onWarning, k.warningBackground),
      };
      for (final MapEntry<String, (Color, Color)> e in pairs.entries) {
        expect(
          contrast(e.value.$1, e.value.$2),
          greaterThanOrEqualTo(4.5),
          reason: '$mode ${e.key}',
        );
      }
    });

    test('$mode: component borders and focus ring meet 3:1', () {
      expect(
        contrast(s.outline, s.surface),
        greaterThanOrEqualTo(3),
        reason: 'outline',
      );
      expect(
        contrast(s.primary, s.surface),
        greaterThanOrEqualTo(3),
        reason: 'focus border',
      );
      expect(
        contrast(s.onSurface, s.surface),
        greaterThanOrEqualTo(3),
        reason: 'button focus ring',
      );
    });
  }
}
