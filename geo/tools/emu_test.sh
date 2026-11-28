#!/bin/bash
# CI emulator test: the app's built-in AutoTest drives the real Kotlin UI (pick, panel, divisions, war, front,
# plan, offensive, peace, save); this script screenshots on its "SHOT" log lines, then checks save/restore.
set -x
mkdir -p shots
adb install -r GeoPolit.apk
adb shell pm clear com.geopolit.game
adb logcat -c
adb shell am start -n com.geopolit.game/.MainActivity --ez autotest true
end=$((SECONDS + 600))
seen=""
while [ $SECONDS -lt $end ]; do
  sleep 1
  log=$(adb logcat -d -s GeoAT:I)
  for n in $(echo "$log" | grep -o 'SHOT [0-9a-z_]*' | awk '{print $2}'); do
    case " $seen " in *" $n "*) ;; *) adb exec-out screencap -p > "shots/$n.png"; seen="$seen $n";; esac
  done
  echo "$log" | grep -q "DONE" && break
done
adb logcat -d -s GeoAT:I > shots/autotest.txt
adb shell am force-stop com.geopolit.game; sleep 2
adb shell am start -n com.geopolit.game/.MainActivity; sleep 40
adb exec-out screencap -p > shots/15_restored.png
adb logcat -d > shots/logcat.txt
adb logcat -d | grep -E "AndroidRuntime|FATAL|libc |geopolit|DEBUG|tombstone" > shots/crash.txt || true
