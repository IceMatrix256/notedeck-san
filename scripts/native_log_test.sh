#!/usr/bin/env bash
set -euo pipefail

APK="${1:-crates/notedeck_chrome/android/app/build/outputs/apk/debug/app-debug.apk}"
if [ ! -f "$APK" ]; then
  echo "APK not found at $APK"
  exit 1
fi

adb wait-for-device

echo "Clearing logcat..."
adb logcat -c

echo "Installing APK..."
adb install --user 0 -r "$APK" || adb install -r "$APK"

echo "Starting app..."
adb shell am start -n com.damus.notedeck/.MainActivity

echo "Sleeping 2s for app to start..."
sleep 2

echo "Starting logcat capture (filters: native-echo, ui, MainActivityTouch) to native_logcat.txt"
adb logcat -v time | grep --line-buffered -E "native-echo|\bui\b|MainActivityTouch" | tee native_logcat.txt &

LOG_PID=$!

echo "You can now interact with the app. To run synthetic taps, provide coordinates as additional args after the apk path, e.g.:\n  $0 $APK 100 200 300 400"

shift || true
if [ "$#" -gt 0 ]; then
  echo "Sending taps..."
  while (( "$#" )); do
    x="$1"; y="$2"; shift 2 || true
    echo "Tapping $x,$y"
    adb shell input tap "$x" "$y"
    sleep 1
  done
fi

echo "Log capture running as PID $LOG_PID. Press Ctrl-C to stop and save native_logcat.txt"
wait $LOG_PID
