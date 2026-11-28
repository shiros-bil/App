#!/bin/bash
# Smoke test on the CI emulator: install, start, pick a nation, interact, collect screenshots + logcat.
set -x
mkdir -p shots
adb install -r GeoPolit.apk
adb logcat -c
adb shell am start -n com.geopolit.game/.MainActivity
shot() { adb exec-out screencap -p > "shots/$1.png"; }
tap_text() {
  adb shell uiautomator dump /sdcard/u.xml >/dev/null 2>&1
  adb pull /sdcard/u.xml u.xml >/dev/null 2>&1
  python3 - "$1" <<'PY'
import re,sys,subprocess
t=sys.argv[1];x=open('u.xml',encoding='utf-8').read()
m=re.search(r'text="'+re.escape(t)+r'"[^>]*bounds="\[(\d+),(\d+)\]\[(\d+),(\d+)\]"',x)
if m:
  a,b,c,d=map(int,m.groups());subprocess.run(['adb','shell','input','tap',str((a+c)//2),str((b+d)//2)])
  print('tapped',t)
else: print('not found',t)
PY
}
for i in 1 2 3 4 5 6 7 8 9 10 11 12; do sleep 10; adb shell uiautomator dump /sdcard/u.xml >/dev/null 2>&1; adb pull /sdcard/u.xml u.xml >/dev/null 2>&1; grep -q "Мировая Держава\|Выберите" u.xml && break; done
shot 01_pick
tap_text "Китай"; sleep 4; shot 02_confirm
tap_text "Играть"; sleep 10; shot 03_game
W=$(adb shell wm size | grep -o '[0-9]*x[0-9]*' | tail -1); SW=${W%x*}; SH=${W#*x}; [ "$SW" -lt "$SH" ] && { t=$SW; SW=$SH; SH=$t; }
adb shell input tap $((SW*55/100)) $((SH*50/100)); sleep 4; shot 04_panel
tap_text "Тест: +10 дивизий"; sleep 2; tap_text "Тест: +10 дивизий"; sleep 3; shot 05_divs
adb shell input swipe $((SW/2)) $((SH/2)) $((SW/2-300)) $((SH/2)) 400; sleep 2; shot 06_pan
tap_text "Фронт"; sleep 1; shot 07_front
adb logcat -d > shots/logcat.txt
adb logcat -d | grep -E "AndroidRuntime|FATAL|libc |geopolit|DEBUG" > shots/crash.txt || true
