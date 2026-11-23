# Notices

## Embedded 5x7 bitmap font

`rust/src/font.rs` embeds the classic 5x7 fixed-width GLCD font (ASCII),
generated into an RGBA atlas by `tools/gen-font-atlas.py` from the
`glcdfont.c` table distributed with the Adafruit GFX Library
(https://github.com/adafruit/Adafruit-GFX-Library), which traces back to the
original public-domain GLCD font. The glyph table is regenerated, not copied
verbatim; the resulting pixel shapes are the standard 5x7 character set.

## Runtime dependencies

- Cefrium SDK (`com.cefrium:cefrium-sdk`) — LGPL-3.0-or-later (Chromium BSD).
  https://cefrium.com/ , https://codeberg.org/cefrium/cef-android
- androidx libraries — Apache 2.0.
