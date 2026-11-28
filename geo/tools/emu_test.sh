#!/bin/bash
# CI emulator test: full game loop through the native UI (pick, panel, divisions, war, front, plan, offensive, peace, save/restore).
set -x
mkdir -p shots
adb install -r GeoPolit.apk
adb logcat -c
adb shell am start -n com.geopolit.game/.MainActivity
shot() { adb exec-out screencap -p > "shots/$1.png"; }
dump() { adb shell uiautomator dump /sdcard/u.xml >/dev/null 2>&1; adb pull /sdcard/u.xml u.xml >/dev/null 2>&1; }
tap_text() {
  dump
  python3 - "$1" <<'PY'
import re,sys,subprocess
t=sys.argv[1];x=open('u.xml',encoding='utf-8').read()
m=re.search(r'(?:text|content-desc)="'+re.escape(t)+r'"[^>]*bounds="\[(\d+),(\d+)\]\[(\d+),(\d+)\]"',x)
if m:
  a,b,c,d=map(int,m.groups());subprocess.run(['adb','shell','input','tap',str((a+c)//2),str((b+d)//2)]);print('tapped',t)
else: print('NOT FOUND',t)
PY
}
for i in $(seq 1 15); do sleep 8; dump; grep -q "Выберите" u.xml && break; done
shot 01_pick
tap_text "Китай"; sleep 4; shot 02_confirm
tap_text "Играть"; sleep 8; shot 03_game
W=$(adb shell wm size | grep -o '[0-9]*x[0-9]*' | tail -1); SW=${W%x*}; SH=${W#*x}; [ "$SW" -lt "$SH" ] && { t=$SW; SW=$SH; SH=$t; }
px() { echo $(( SW * $1 / 1000 )); }; py() { echo $(( SH * $1 / 1000 )); }
# own region panel -> test divisions (scroll the panel)
adb shell input tap $(px 560) $(py 520); sleep 3; shot 04_panel
adb shell input swipe $(px 800) $(py 800) $(px 800) $(py 250) 300; sleep 1
tap_text "Тест: +10 дивизий"; sleep 2; tap_text "Тест: +10 дивизий"; sleep 2; shot 05_divs
adb shell input keyevent 4; sleep 1
# Mongolia: declare war
adb shell input tap $(px 495) $(py 310); sleep 3; shot 06_mongolia
tap_text "Объявить войну"; sleep 2; adb shell input keyevent 4; sleep 1
# front by tapping the country
tap_text "Фронт"; sleep 1; adb shell input tap $(px 495) $(py 300); sleep 2
tap_text "speed3"; sleep 12; shot 07_front
# plan across Mongolia, then attack
tap_text "План"; sleep 1; adb shell input swipe $(px 380) $(py 270) $(px 630) $(py 270) 1500; sleep 1; shot 08_plan
tap_text "Наступать"; sleep 25; shot 09_offensive
tap_text "Отладка"; sleep 2; shot 10_debug; tap_text "Отладка"
tap_text "speed0"; sleep 1
# peace conference via occupied zone panel
adb shell input tap $(px 495) $(py 340); sleep 3; shot 11_zone
adb shell input swipe $(px 800) $(py 800) $(px 800) $(py 250) 300; sleep 1
tap_text "Мирная конференция"; sleep 4; shot 12_peace
tap_text "Выбрать всё"; sleep 2; shot 13_selected
tap_text "Подписать мир"; sleep 4; shot 14_signed
# save and restore
tap_text "menu"; sleep 1; tap_text "Сохранить"; sleep 2
adb shell am force-stop com.geopolit.game; sleep 2
adb shell am start -n com.geopolit.game/.MainActivity; sleep 40; shot 15_restored
adb logcat -d > shots/logcat.txt
adb logcat -d | grep -E "AndroidRuntime|FATAL|libc |geopolit|DEBUG|tombstone" > shots/crash.txt || true
