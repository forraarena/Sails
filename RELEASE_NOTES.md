# Sails PoC — first public test build

A proof-of-concept Android browser that runs the **Cefrium** engine (Chromium,
Rust-hosted) entirely off-screen and draws **every pixel with OpenGL ES 3 from
Rust** — the status bar, omnibox, buttons, progress strip and page texture are
all composited by `libsails.so`. No Android UI toolkit is used at runtime.

## What works

- Omnibox: tap the address pill, type a URL or search terms, press GO
- Back / forward / reload / stop buttons with live progress bar + spinner
- Page taps and one-finger scrolling forwarded to the engine
- JavaScript dialogs and `<select>` pickers (rendered by the engine's activity)
- Hardware Back button (closes omnibox, then navigates back)

## Not in the PoC yet

Multi-touch pinch-zoom, IME-to-page text bridge (typing into web forms), text
selection, Vulkan backend, desktop-mode / UA controls, downloads, tabs.

## Installing

**Requirements: Android 10+ (API 29), arm64-v8a device or x86_64 emulator.**

| Asset | Use |
|---|---|
| `Sails-*-debug.apk` | Sideloading on a device (`adb install` or tap in a file manager) |
| `Sails-*-release.apk` | Same, smaller (no debug overhead). Signed with the debug key |

Enable *Install unknown apps* for your file manager / adb if prompted. The APKs
are large (~270–300 MB) because they embed the Cefrium/Chromium engine
(`libcef.so` ≈ 203 MB per ABI).

## Security note

Release APKs are signed with the **debug key** — fine for testing, not for
distribution. Verify you downloaded them from this repository's Releases page.

## Building from source

See [README.md](https://github.com/forraarena/Sails/blob/main/README.md):
`./scripts/build-rust.sh && ./gradlew :app:assembleDebug` (Rust + NDK r27 +
JDK 25). Every push to this repo builds APKs automatically in CI.

## Attribution

Includes the Cefrium SDK (Chromium/CEF) and a classic 5×7 GLCD bitmap font —
see [NOTICE.md](https://github.com/forraarena/Sails/blob/main/NOTICE.md).
