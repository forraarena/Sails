## Cefrium (embedded Chromium, com.cefrium:*)

Copy this block into your repository's AGENTS.md (or CLAUDE.md). Coding
agents load that file in every session; they do not load this one.
Source: https://cefrium.com/agents.md

- Public API only: com.cefrium.* (reference: https://cefrium.com/docs/).
  Never call org.chromium.*, org.jni_zero.* or GEN_JNI, and no reflection
  on the engine: it breaks on every engine update. Internals -> public
  calls: https://cefrium.com/guides/public-api/
- Toolchain: JDK 25, Gradle 9.7.1, AGP 9.4, compileSdk 37, minSdk 29.
  ABIs arm64-v8a, armeabi-v7a and x86_64. Apply the com.cefrium Gradle
  plugin; its version equals the SDK version.
- Repository: https://maven.cefrium.com with
  content { includeGroup("com.cefrium") }. The -sources.jar there is the
  source of truth for each version.
- Your Application class also runs in the engine's isolated child
  processes (renderer, GPU, utility). They cannot open your files or
  preferences, so main-process work there kills the engine and no page
  renders, not even a local one. First line of Application.onCreate():
  if (!getPackageName().equals(Application.getProcessName())) return;
  Sample: https://codeberg.org/cefrium/cefrium-sample/src/branch/main/child-processes
- R8: the SDK ships its own keep rules. Do not add keeps for engine
  classes. A crash only in the minified build is a bug to report.
- Page <-> app bridge: window.cefriumQuery reaches a QueryHandler, which
  must answer synchronously from onQuery; one handler per browser. To
  push events to the page, call evaluateJavaScript from the app instead
  of polling from the page.
- evaluateJavaScript(script, ValueCallback<String>) returns the script's
  value as JSON. DevTools protocol in process: executeDevToolsMethod.
- Off-screen browser: input via sendMouse*/sendKeyEvent/sendTouchEvent,
  resize via setSize (do not recreate the browser).
- android.webkit.WebView and Cefrium share a process in either order
  since 0.18.0.
- Test on a real device before release. An emulator without KVM, or CI
  that only compiles, cannot show that pages render.
- Bugs: https://codeberg.org/cefrium/cef-android/issues
