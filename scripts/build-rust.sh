#!/usr/bin/env bash
#
# Builds the Sails Rust compositor (libsails.so) for Android and places it in
# app/src/main/jniLibs/<abi>/ where the Gradle build picks it up.
#
# Requires: a Rust toolchain (rustup), an Android NDK (ANDROID_HOME/ndk/<ver>
# or ANDROID_NDK_HOME), and cargo-ndk (installed automatically if missing).
#
# Environment overrides:
#   SAILS_TARGETS     e.g. "aarch64-linux-android"           (default: arm64 + x86_64)
#   SAILS_BUILD_MODE  "release" (default) or "debug"
set -euo pipefail

cd "$(dirname "$0")/.."

echo "== environment =="
echo "cargo: $(command -v cargo || echo MISSING)"
cargo --version 2>&1 || true
rustc --version 2>&1 || true
rustup target list --installed 2>&1 || true
echo "ANDROID_HOME=${ANDROID_HOME:-unset} ANDROID_NDK_HOME=${ANDROID_NDK_HOME:-unset}"

MODE="${SAILS_BUILD_MODE:-release}"
if [ "$MODE" = "debug" ]; then
  BUILD_FLAG=""
else
  BUILD_FLAG="--release"
fi

TARGETS="${SAILS_TARGETS:-aarch64-linux-android x86_64-linux-android}"

if ! command -v cargo >/dev/null 2>&1; then
  echo "error: no Rust toolchain found -- install from https://rustup.rs" >&2
  exit 1
fi

# Make sure the Android targets are actually installed (CI runners may only
# have the host toolchain, despite rustup being present).
for t in $TARGETS; do
  rustup target add "$t" 2>&1 || echo "warning: rustup target add $t failed" >&2
done

# Locate the NDK the way cargo-ndk does, and fail with a clear message early.
NDK_ROOT_CANDIDATES=(
  "${ANDROID_NDK_HOME:-}"
  "${ANDROID_HOME:-}/ndk-bundle"
)
if [ -n "${ANDROID_HOME:-}" ]; then
  for d in "$ANDROID_HOME"/ndk/*; do
    [ -d "$d" ] && NDK_ROOT_CANDIDATES+=("$d")
  done
fi
NDK_FOUND=""
for d in "${NDK_ROOT_CANDIDATES[@]}"; do
  if [ -n "$d" ] && [ -d "$d" ]; then
    NDK_FOUND="$d"
    break
  fi
done
if [ -z "$NDK_FOUND" ]; then
  echo "error: no Android NDK found (looked at ANDROID_NDK_HOME, \$ANDROID_HOME/ndk/*, ndk-bundle)" >&2
  echo "       install one with: sdkmanager \"ndk;27.0.12077973\"" >&2
  exit 1
fi
echo "NDK: $NDK_FOUND"

if ! command -v cargo-ndk >/dev/null 2>&1; then
  echo "Installing cargo-ndk..."
  cargo install cargo-ndk --locked
fi

# cargo-ndk locates the NDK via ANDROID_NDK_HOME or ANDROID_HOME/ndk/<latest>.
export ANDROID_NDK_HOME="$NDK_FOUND"
cargo ndk $(for t in $TARGETS; do printf -- "-t %s " "$t"; done) $BUILD_FLAG build

OUT="app/src/main/jniLibs"
for t in $TARGETS; do
  case "$t" in
    aarch64-linux-android)     abi=arm64-v8a ;;
    armv7a-linux-androideabi*) abi=armeabi-v7a ;;
    x86_64-linux-android)      abi=x86_64 ;;
    i686-linux-android)        abi=x86 ;;
    *) echo "warning: unknown target $t, skipping"; continue ;;
  esac
  mkdir -p "$OUT/$abi"
  cp "target/$t/$MODE/libsails.so" "$OUT/$abi/libsails.so"
  echo "placed $OUT/$abi/libsails.so"
done

echo "done -- now run: ./gradlew :app:assembleDebug"
