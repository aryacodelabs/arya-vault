# Golden images

15 images: Welcome, Create (with strength feedback), Recovery key, Verify and
Lock (with quick unlock), each in light, dark and 200% text.

## Regenerate

From `app/`:

```
flutter test --update-goldens test/golden
```

Review the diff of `test/golden/goldens/*.png` before committing.

## Why they are stable across machines

- Flutter's test harness renders text with the built-in **Ahem** font (every
  glyph is a square) unless a test loads another font. No test loads one, so
  glyph metrics do not depend on system fonts. The images therefore look like
  blocks; they check layout, color, spacing and overflow, not typography.
- The recovery key is generated from a seeded `Random` (`seeded()` in
  `test/screen_cases.dart`), so the key groups are identical every run.
- `EditableText.debugDeterministicCursor` pins the caret.
- The text scale, window size (400x800 @ 1x) and theme mode are set per test.

If a real font is bundled later (app branding is out of scope for B01), load it
for tests with `FontLoader` from the asset bundle and regenerate every image.

## Platform

Goldens are generated and verified on **Linux** (CI job `flutter`). Software
rasterization can differ in anti-aliasing by a few pixels on Windows or macOS, so
a local failure there is not necessarily a bug; regenerate on Linux (for example
in a container) before committing. Run the rest of the suite without goldens
with `flutter test --exclude-tags golden`.
