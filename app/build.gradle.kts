plugins {
    id("com.android.application")
    id("com.cefrium")
}

val cefriumVersion = providers.gradleProperty("cefriumVersion").get()

android {
    namespace = "dev.sails"
    compileSdk = 37

    // Cefrium/Chromium reads its native libraries from disk, so they must be
    // extracted at install time. AGP 9 rejects android:extractNativeLibs in the
    // manifest; the build-script equivalent is legacy jniLibs packaging.
    // (Mirrors the official osr-hud sample.)
    packaging {
        jniLibs {
            useLegacyPackaging = true
        }
    }

    // Chromium reads assets via memory mapping -- they must be uncompressed.
    androidResources {
        noCompress.addAll(listOf("dat", "pak", "bin"))
    }

    defaultConfig {
        applicationId = "dev.sails"
        minSdk = 29
        targetSdk = 34
        versionCode = 1
        versionName = "0.1.1"

        // Cefrium's AAR bundles libcef.so (~150-200 MB) for every ABI; filter
        // to arm64 so the APK ships one engine instead of two.
        ndk {
            abiFilters += listOf("arm64-v8a")
        }
    }

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_11
        targetCompatibility = JavaVersion.VERSION_11
    }

    buildTypes {
        release {
            isMinifyEnabled = false
            // Sign the CI-built release APK with the debug key so it can be
            // sideloaded directly. Fine for PoC releases; swap for a real
            // key before any distribution outside this repo.
            signingConfig = signingConfigs.getByName("debug")
        }
    }
}

// Fail the build early (with a clear message) when the native compositor
// hasn't been built -- its absence would otherwise crash at app start.
val checkNativeLibs by tasks.registering {
    doLast {
        val libs = fileTree("src/main/jniLibs") { include("**/libsails.so") }.files
        require(libs.isNotEmpty()) {
            "libsails.so is missing from app/src/main/jniLibs -- run ./scripts/build-rust.sh first (see README)"
        }
        logger.lifecycle("native compositor present: {}", libs.map { it.parentFile.name })
    }
}

tasks.named("preBuild") {
    dependsOn(checkNativeLibs)
}

dependencies {
    // The engine. Bring the runtime dependencies it expects, mirroring the
    // official Cefrium samples:
    implementation("com.cefrium:cefrium-sdk:" + cefriumVersion)

    // AppCompat for resource strings (abc_*) referenced by Chrome themes.
    implementation("androidx.appcompat:appcompat:1.7.0") {
        exclude(group = "org.jspecify")
        exclude(group = "org.jetbrains.kotlin")
    }
    // MediaRouter -- Chromium loads this dynamically for Cast support.
    implementation("androidx.mediarouter:mediarouter:1.7.0") {
        exclude(group = "org.jspecify")
        exclude(group = "org.jetbrains.kotlin")
    }
    // AndroidX core 1.16+ needed for BuildCompat.isAtLeastB().
    implementation("androidx.core:core:1.16.0") {
        exclude(group = "org.jspecify")
        exclude(group = "org.jetbrains.kotlin")
    }
    // CustomTabs/TWA -- Chromium's notification display path references
    // androidx.browser.trusted.* (TrustedWebActivityServiceConnectionPool).
    implementation("androidx.browser:browser:1.8.0") {
        exclude(group = "org.jspecify")
    }
    // Kotlin stdlib -- needed by Chromium's AndroidX dependencies.
    implementation("org.jetbrains.kotlin:kotlin-stdlib:1.8.22")

    configurations.all {
        exclude(group = "org.jspecify")
    }
}
