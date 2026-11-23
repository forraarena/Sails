# Sails

A proof-of-concept Android browser where **everything you see is drawn by
Rust with OpenGL ES**, around the [Cefrium](https://cefrium.com/) engine —
a full pinned Chromium shipped inside the app.

```
┌──────────────────────────────────────────────────────────────┐
│                    Android app process                        │
│                                                              │
│  Kotlin (thin host)                 Rust (libsails.so)       │
│  ┌──────────────────────┐  frames  ┌───────────────────────┐ │
│  │ CefriumHost          │ ───────► │ render thread          │ │
│  │  CefriumBrowser      │  RGBA    │  EGL + GLES3 compositor│ │
│  │  (off-screen, OSR)   │          │  chrome bar + omnibox  │ │
│  │                      │          │  bitmap-font text      │ │
│  │ SailsSurfaceView     │ ◄─────── │  input state machine   │ │
│  │  SurfaceView         │  events  │  (hit test, tap/slop,  │ │
│  │  touch -> native     │          │   scroll forwarding)   │ │
│  │  poll -> Cefrium     │          │                        │ │
│  └──────────────────────┘          └───────────┬────────────┘ │
│         │                            EGL window│             │
│        ▼                                      ▼              │
│  Chromium (Cefrium, pinned)          ANativeWindow (screen)  │
└──────────────────────────────────────────────────────────────┘
```

The engine never draws to the screen: Cefrium renders each page off-screen
and hands Sails a premultiplied **RGBA** pixel buffer per frame
(`CefriumRenderHandler.onPaint`, UI thread). The Rust compositor uploads it
as a texture and draws the whole UI — chrome bar, omnibox, icons, progress,
URL text — on top with GLES3. All input is hit-tested in Rust and forwarded
to the engine in browser pixels. There is no Android UI toolkit on screen:
one `SurfaceView`, everything else is triangles.

## What works in this PoC

- Page rendering at native resolution (engine pixels → texture → screen)
- **Omnibox**: tap the address bar → native IME → type a URL or search
  (no scheme + spaces ⇒ DuckDuckGo search) → GO
- Tap: links, buttons, anything on the page
- **Scroll**: drag below the chrome bar (single finger; fling handled by the engine)
- Back / Forward buttons (enabled/disabled from live history state)
- Reload, and Stop while loading
- Progress bar + loading spinner, live URL display
- System Back: closes the omnibox, else walks page history, else backgrounds
- JS dialogs, `<select>`, date/time/color pickers, file chooser via
  `browser.setDialogActivity(activity)`

## What is deliberately not in yet

- Multi-touch pinch-zoom (second finger cancels the gesture)
- IME text *into the page* (needs the `imeCommitText`/`imeSetComposition`
  bridge described in https://cefrium.com/guides/off-screen-rendering/)
- Text selection quick-menu / drag handles
- Tabs, downloads UI, Vulkan backend (the renderer is a thin module —
  the EGL/GLES3 path can be swapped for Vulkan + `VK_KHR_android_surface`)
- In-page typing via hardware keyboards

## Repository layout

```
app/            Gradle module: manifest, resources, the 5 Kotlin files
  .../CefriumHost.kt        engine lifecycle (the only Cefrium SDK caller)
  .../SailsSurfaceView.kt   surface lifecycle, touch in, event drain loop
  .../OmniboxIme.kt         1x1 EditText owning the soft keyboard
  .../MainActivity.kt       wiring + Back handling
rust/           the native core (zero external crates -- raw FFI only)
  src/lib.rs        JNI entry points + input state machine
  src/renderer.rs   render thread: EGL lifecycle, frame upload, composite
  src/chrome.rs     chrome layout & drawing (icons, omnibox, progress)
  src/font.rs       generated 5x7 ASCII font atlas (see NOTICE.md)
  src/egl.rs, gl.rs hand-written EGL 1.4 / GLES 3.0 FFI subsets
  src/jni.rs        minimal JNI by vtable index (5 entries, runtime-checked)
  src/events.rs     Rust->Java event queue + wire format
  src/state.rs      shared state (frames, nav state, omnibox, touches)
scripts/build-rust.sh     cargo-ndk build -> app/src/main/jniLibs/
tools/gen-font-atlas.py   regenerates rust/src/font.rs
.github/workflows/ci.yml  builds the APK on push (artifact: sails-debug-apk)
```

## Building

Prerequisites:

- JDK 25, Android SDK (platform 37), an NDK (`sdkmanager "ndk;27.x"`)
- Rust (`rustup`) + `cargo-ndk` (installed automatically by the script)
- A device or emulator: **arm64-v8a** (Android 10 / API 29+). Release APKs
  ship arm64 only; the CI smoke test additionally builds x86_64 for the
  emulator.

```bash
# 1. native compositor -> app/src/main/jniLibs/<abi>/libsails.so
./scripts/build-rust.sh          # SAILS_BUILD_MODE=debug for a fast build

# 2. the APK
./gradlew :app:assembleDebug
adb install -r app/build/outputs/apk/debug/app-debug.apk
```

No Android Studio needed; the included Gradle wrapper (9.7.1) fetches the
rest. In Android Studio: open the repo, run `scripts/build-rust.sh` once
(a Run Configuration or before-launch task does it), then Run ▶.

**CI builds it for you:** every push runs
[`.github/workflows/ci.yml`](.github/workflows/ci.yml), which produces the
`sails-debug-apk` artifact — download and `adb install` it.

## Expect on first run

The APK is large (~200 MB uncompressed across ABIs): Cefrium bundles a full
Chromium (`libcef.so` ≈ 203 MB for arm64-v8a). First launch spends a few
seconds extracting engine libs; you'll see the dark "waiting for the first
engine frame" splash, then https://example.com renders. Everything above
the page — the bar, the URL, the spinner — is Sails' own GL output.

Useful log filtering: `adb logcat -s sails` (compositor), and Cefrium's
engine logs under their own tags.

## Design notes

- **Pixel format**: Cefrium 0.21.0 delivers RGBA premultiplied on Android
  (desktop CEF is BGRA — do not copy desktop code blindly). Frames are only
  valid during `onPaint`; the Kotlin callback passes the direct ByteBuffer
  to Rust, which memcpys into staging before returning.
- **No JNI upcalls**: Rust never calls into Java. Actions (navigate, back,
  resize, touch-forward, show IME) are queued as packed events
  (`rust/src/events.rs`) and drained by the Kotlin choreographer loop every
  frame. Java→Rust strings use five vtable-indexed JNI calls
  (`rust/src/jni.rs`) with a `GetVersion` sanity check, so the JNI surface
  stays auditable and tiny.
- **Threading**: all `CefriumBrowser` calls happen on the UI thread (a
  Cefrium requirement); all GL happens on the `sails-render` thread. The
  shared state is a single small mutex (`rust/src/state.rs`).
- **Sizes**: the chrome bar is 60dp (56dp bar + 4dp progress strip); the
  engine browser is created and resized to `surface − chrome` (never
  recreated — per the Cefrium OSR guide, resize with `setSize`).
- **Version pins**: Cefrium SDK/plugin `0.21.0`, Gradle `9.7.1`, AGP `9.4.1`,
  `compileSdk 37`, `minSdk 29`, JDK 25 (all hard requirements of the SDK,
  see AGENTS.md).

## Next steps if this becomes the real browser

1. IME→page bridge (`onVirtualKeyboardRequested` + `imeCommitText`) so page
   fields accept typing; then remove the omnibox-only EditText.
2. Multi-touch (`sendTouchEvent(action, x, y, pointerId)` supports it) and
   pinch zoom.
3. Vulkan compositor: same event/state split, `VK_KHR_android_surface`
   instead of EGL, one image per swapchain frame.
4. Tabs as additional `CefriumBrowser`s with per-tab staging buffers
   (see the Cefrium memory & processes guide for the browser pool).
5. Dirty-rect/frame pooling if the full-frame upload (w×h×4 per painted
   frame) ever shows on a profiler.
