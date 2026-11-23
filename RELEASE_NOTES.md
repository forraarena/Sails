# Sails v0.1.1-poc — arm64-only test build

**Fixes/changes since v0.1.0-poc**

- **arm64-v8a only**: the APK no longer bundles x86_64 — roughly 35% smaller.
- Release APK signed with the debug key (still sideload-friendly).

## Help us fix the launch crash (v0.1.0 report: blank screen + crash)

If the app still shows a blank screen or crashes, send us the log:

```
adb logcat -c            # clear old logs
# ... launch Sails now, wait for it to crash ...
adb logcat -d > sails-crash.log
```

Attach `sails-crash.log` to an issue (or paste the lines around
`FATAL EXCEPTION` / `Fatal signal`). Every CI build now runs an automated
emulator smoke test that captures the same evidence.

## Installing

**Requirements: Android 10+ (API 29), arm64-v8a device.**

| Asset | Use |
|---|---|
| `Sails-*-debug.apk` | Sideloading on a device |
| `Sails-*-release.apk` | Same, smaller. Debug-key signed |

Enable *Install unknown apps* if prompted. The APK is large (~200 MB) because
it embeds the Cefrium/Chromium engine (`libcef.so` ≈ 203 MB).

## What works (when it launches)

- Omnibox: tap the address pill, type a URL or search terms, press GO
- Back / forward / reload / stop with live progress bar + spinner
- Page taps and one-finger scrolling; JS dialogs and `<select>` pickers
- Hardware Back (closes omnibox, then navigates back)

## Building from source

See the README: `./scripts/build-rust.sh && ./gradlew :app:assembleDebug`
(Rust + NDK r27 + JDK 25). Every push builds APKs automatically in CI.

## Attribution

Includes the Cefrium SDK (Chromium/CEF) and a classic 5×7 GLCD bitmap font —
see NOTICE.md.
