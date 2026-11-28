package com.geopolit.game

import android.util.Log
import android.view.View
import android.view.ViewGroup
import android.widget.TextView
import org.json.JSONArray
import java.util.concurrent.CountDownLatch
import java.util.concurrent.TimeUnit

// In-app UI test (started with `am start --ez autotest true`): drives the real Kotlin views via performClick
// and the GL map via the same Native calls the touch handler uses. Logs "GeoAT" lines; "SHOT <name>" asks
// the host script to take a screenshot (the test then waits so the frame is stable).
class AutoTest(private val a: MainActivity, private val map: MapView) {
    private val tag = "GeoAT"
    private var fails = 0

    private fun log(s: String) = Log.i(tag, s)
    private fun sleep(ms: Long) = Thread.sleep(ms)

    private fun <T> ui(f: () -> T): T {
        var r: T? = null
        val l = CountDownLatch(1)
        a.runOnUiThread { try { r = f() } finally { l.countDown() } }
        l.await(10, TimeUnit.SECONDS)
        @Suppress("UNCHECKED_CAST")
        return r as T
    }
    private fun <T> gl(f: () -> T): T {
        var r: T? = null
        val l = CountDownLatch(1)
        map.queueEvent { try { r = f() } finally { l.countDown() } }
        l.await(10, TimeUnit.SECONDS)
        @Suppress("UNCHECKED_CAST")
        return r as T
    }

    private fun shot(n: String) { sleep(1200); log("SHOT $n"); sleep(2500) }

    private fun find(v: View, t: String): View? {
        if (v.visibility != View.VISIBLE) return null
        if ((v is TextView && v.text.toString() == t) || v.contentDescription?.toString() == t) return v
        if (v is ViewGroup) for (i in 0 until v.childCount) find(v.getChildAt(i), t)?.let { return it }
        return null
    }
    private fun click(t: String): Boolean {
        val ok = ui {
            var v = find(a.window.decorView, t)
            while (v != null && !v.isClickable) v = v.parent as? View
            if (v != null && v.isEnabled) { v.performClick(); true } else false
        }
        log(if (ok) "click '$t' ok" else "FAIL click '$t'")
        if (!ok) fails++
        sleep(700)
        return ok
    }

    private fun natIdx(name: String): Int {
        val arr = JSONArray(Native.nations())
        for (i in 0 until arr.length()) { val o = arr.getJSONObject(i); if (o.getString("n") == name) return o.getInt("i") }
        return -1
    }
    private fun point(nat: Int, mode: Int): FloatArray? {
        val s = gl { Native.cmd("scr", nat.toLong(), mode.toLong(), "") }
        if (s.isEmpty()) return null
        val p = s.split(",")
        return floatArrayOf(p[0].toFloat(), p[1].toFloat())
    }
    private fun tapAt(p: FloatArray) {
        val r = gl { Native.tap(p[0], p[1]) }
        ui { a.onTap(r) }
        sleep(900)
    }
    private fun tapNat(nat: Int, mode: Int, what: String): Boolean {
        val p = point(nat, mode)
        if (p == null) { log("FAIL no point for $what"); fails++; return false }
        log("tap $what at ${p[0]},${p[1]}")
        tapAt(p)
        return true
    }
    private fun closePanel() { ui { Native.cmd("close", 0, 0, ""); a.refreshUi() }; sleep(500) }

    fun run() {
        try {
            log("START")
            for (i in 0 until 120) { if (ui { find(a.window.decorView, "Китай") != null }) break; sleep(500) }
            shot("01_pick")
            click("Китай"); sleep(1500); shot("02_confirm")
            click("Играть"); sleep(3500); shot("03_game")
            val cn = natIdx("Китай"); val mn = natIdx("Монголия")
            log("nations cn=$cn mn=$mn")
            // divisions from the own region panel
            tapNat(cn, 0, "own region"); shot("04_panel")
            click("Тест: +10 дивизий"); click("Тест: +10 дивизий"); sleep(800); shot("05_divs")
            closePanel()
            // war with Mongolia from its panel
            tapNat(mn, 0, "Mongolia"); shot("06_mongolia")
            click("Объявить войну"); closePanel()
            // front: one short touch on Mongolia in front mode
            click("Фронт")
            point(mn, 0)?.let { p -> gl { Native.drawTouch(0, p[0], p[1]); Native.drawTouch(2, p[0], p[1]) }; ui { a.refreshUi() } }
            click("speed3"); sleep(10000); shot("07_front")
            // plan: a line drawn across Mongolia
            click("План")
            point(mn, 0)?.let { p ->
                val x0 = p[0] - a.resources.displayMetrics.widthPixels * 0.18f
                val x1 = p[0] + a.resources.displayMetrics.widthPixels * 0.18f
                val y = p[1] - a.resources.displayMetrics.heightPixels * 0.04f
                gl {
                    Native.drawTouch(0, x0, y)
                    for (i in 1..30) Native.drawTouch(1, x0 + (x1 - x0) * i / 30f, y)
                    Native.drawTouch(2, x1, y)
                }
                ui { a.refreshUi() }
            }
            sleep(800); shot("08_plan")
            click("Наступать"); sleep(25000); shot("09_offensive")
            click("Отладка"); sleep(1500); shot("10_debug"); click("Отладка")
            click("speed0"); sleep(800)
            // occupied zone panel (if a free spot exists), otherwise any Mongolian region; then the peace conference
            if (point(mn, 1) != null) { tapNat(mn, 1, "occupied zone"); shot("11_zone") } else { tapNat(mn, 0, "Mongolia (peace)"); shot("11_zone") }
            if (!click("Мирная конференция")) { closePanel(); tapNat(mn, 0, "Mongolia (peace)"); click("Мирная конференция") }
            sleep(2500); shot("12_peace")
            click("Выбрать всё"); sleep(1500); shot("13_selected")
            click("Подписать мир"); sleep(3000); shot("14_signed")
            click("menu"); click("Сохранить"); sleep(1000)
            log("DONE fails=$fails")
        } catch (t: Throwable) {
            log("FAIL exception ${t}")
            log("DONE fails=${fails + 1}")
        }
    }
}
