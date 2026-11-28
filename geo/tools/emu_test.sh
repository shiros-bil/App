#!/bin/bash
# CI emulator test: the app's built-in AutoTest drives the real Kotlin UI (pick, panel, divisions, war, front,
# plan, offensive, peace, save); this script screenshots on its "SHOT" log lines, then checks save/restore.
set -x
mkdir -p shots
adb install -r GeoPolit.apk
adb shell pm clear com.geopolit.game
adb logcat -c
adb shell am start -n com.geopolit.game/.MainActivity --ez autotest true
# react to "SHOT name" lines as they are logged (streaming, not polling)
timeout 600 adb logcat -v brief -s GeoAT:I | while read -r line; do
  case "$line" in
    *"SHOT "*) n=${line##*SHOT }; n=${n//[^0-9a-z_]/}; adb exec-out screencap -p > "shots/$n.png"; echo "shot $n";;
    *DONE*) pkill -f "logcat -v brief"; break;;
  esac
done
adb logcat -d -s GeoAT:I > shots/autotest.txt
adb shell am force-stop com.geopolit.game; sleep 2
adb shell am start -n com.geopolit.game/.MainActivity; sleep 40
adb exec-out screencap -p > shots/15_restored.png
adb logcat -d > shots/logcat.txt
adb logcat -d | grep -E "AndroidRuntime|FATAL|libc |geopolit|DEBUG|tombstone" > shots/crash.txt || true
