#!/usr/bin/env bash
# Build a debug APK for an arm64 phone, install it over USB and start the app.
set -euo pipefail
cd "$(dirname "$0")/../app"
. ../scripts/android-env.sh
apk=src-tauri/gen/android/app/build/outputs/apk/universal/debug/app-universal-debug.apk
log=${TMPDIR:-/tmp}/finco-android-build.log
npx tauri android build --debug --apk --target aarch64 > "$log" 2>&1 || {
  grep -E 'error|Error|e: ' "$log" | head -30; exit 1; }
adb install -r "$apk" | tail -1
adb shell am force-stop dev.finco.app
adb shell monkey -p dev.finco.app -c android.intent.category.LAUNCHER 1 > /dev/null 2>&1
echo "installed and started"
