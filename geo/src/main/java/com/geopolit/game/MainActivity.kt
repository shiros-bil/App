package com.geopolit.game

import android.app.Activity
import android.graphics.BitmapFactory
import android.graphics.Color
import android.os.Bundle
import android.os.Handler
import android.os.Looper
import android.text.Editable
import android.text.TextWatcher
import android.view.Gravity
import android.view.View
import android.view.ViewGroup
import android.view.WindowInsets
import android.view.WindowInsetsController
import android.view.inputmethod.InputMethodManager
import android.widget.*
import org.json.JSONArray
import org.json.JSONObject
import java.io.File

class MainActivity : Activity() {
    private lateinit var u: Ui
    private lateinit var root: FrameLayout
    private lateinit var map: MapView
    private lateinit var hud: LinearLayout
    private lateinit var tbox: LinearLayout
    private lateinit var panel: ScrollView
    private lateinit var panelBox: LinearLayout
    private lateinit var seaCard: LinearLayout
    private lateinit var toastV: TextView
    private lateinit var ftb: LinearLayout
    private var modal: FrameLayout? = null
    private var loading: View? = null
    private var pcView: FrameLayout? = null
    private var loaded = false
    private var started = false
    private val h = Handler(Looper.getMainLooper())
    private var lastPanel = ""
    private var lastFtb = ""
    private var lastHud = ""
    private var lastPc = ""
    private val saveFile by lazy { File(filesDir, "save.json") }

    override fun onCreate(s: Bundle?) {
        super.onCreate(s)
        u = Ui(this)
        root = FrameLayout(this)
        root.setBackgroundColor(Color.rgb(10, 42, 74))
        map = MapView(this, this)
        root.addView(map, FrameLayout.LayoutParams(-1, -1))
        buildHud()
        panel = ScrollView(this).apply { visibility = View.GONE; background = u.bg(C.PANEL, 16f); isVerticalScrollBarEnabled = false }
        panelBox = u.col().apply { setPadding(u.dp(14), u.dp(14), u.dp(14), u.dp(14)) }
        panel.addView(panelBox)
        root.addView(panel, u.lp(0, -1, Gravity.END, 0, 58, 8, 8))
        seaCard = u.row(gap = 8).apply { visibility = View.GONE; background = u.bg(C.PANEL, 16f); clipToOutline = true }
        root.addView(seaCard, u.lp(u.dp(240), u.dp(54), Gravity.END or Gravity.BOTTOM, 0, 0, 8, 8))
        ftb = u.row(gap = 6).apply { visibility = View.GONE }
        root.addView(HorizontalScrollView(this).apply { addView(ftb); isHorizontalScrollBarEnabled = false }, u.lp(-2, -2, Gravity.START or Gravity.BOTTOM, 8, 0, 8, 8))
        toastV = u.text("", 13f).apply {
            visibility = View.GONE; background = u.bg(C.PANEL, 12f); setPadding(u.dp(12), u.dp(6), u.dp(12), u.dp(6)); gravity = Gravity.CENTER
        }
        root.addView(toastV, u.lp(-2, -2, Gravity.CENTER_HORIZONTAL or Gravity.TOP, 16, 62, 16, 0))
        setContentView(root)
        hideBars()
        root.post { sizePanel() }
        root.addOnLayoutChangeListener { _, _, _, _, _, _, _, _, _ -> sizePanel() }
        showLoading()
        Thread { loadCore() }.start()
        h.postDelayed(object : Runnable { override fun run() { if (started) refreshHudOnly(); h.postDelayed(this, 300) } }, 300)
    }

    private fun sizePanel() {
        val w = root.width
        if (w <= 0) return
        val pw = minOf(u.dp(340), (w * 0.46).toInt())
        (panel.layoutParams as FrameLayout.LayoutParams).width = pw
        panel.requestLayout()
    }

    private fun readAsset(n: String) = assets.open(n).use { it.readBytes() }
    private fun readFont(vararg names: String): ByteArray {
        for (n in names) { val f = File("/system/fonts/$n"); if (f.exists()) return f.readBytes() }
        return ByteArray(0)
    }

    private fun loadCore() {
        try {
            setLoad("Чтение данных…", 3)
            val nat = readAsset("nations.json"); val lo = readAsset("world_lo.json"); setLoad("Чтение данных…", 8)
            val mid = readAsset("world_mid.json"); val hi = readAsset("world.json"); setLoad("Чтение данных…", 14)
            val wat = readAsset("water.json"); val mar = readAsset("marine.json"); val cit = readAsset("cities.json")
            // poll the core's current stage while it builds the world
            val stages = listOf("Загрузка стран…" to 18, "Загрузка карты (1/3)…" to 24, "Загрузка карты (2/3)…" to 38, "Загрузка карты (3/3)…" to 55,
                "Загрузка озёр и рек…" to 72, "Загрузка морей…" to 82, "Загрузка городов…" to 90, "Подготовка графики…" to 96)
            val done = java.util.concurrent.atomic.AtomicBoolean(false)
            Thread {
                while (!done.get()) {
                    val st = Native.stage()
                    val p = stages.firstOrNull { it.first == st }?.second
                    if (st.isNotEmpty()) setLoad(st, p)
                    try { Thread.sleep(60) } catch (_: Throwable) {}
                }
            }.start()
            val ok = try { Native.load(nat, lo, mid, hi, wat, mar, cit,
                readFont("Roboto-Regular.ttf", "NotoSans-Regular.ttf", "DroidSans.ttf"),
                readFont("NotoSerif-Bold.ttf", "DroidSerif-Bold.ttf", "NotoSerif-Regular.ttf", "Roboto-Bold.ttf", "Roboto-Regular.ttf")) } finally { done.set(true) }
            setLoad("Подготовка графики…", 100)
            runOnUiThread {
                if (!ok) { fatal("Не удалось загрузить карту"); return@runOnUiThread }
                loaded = true
                map.start()
                loading?.let { root.removeView(it) }; loading = null; loadText = null; loadBar = null
                val sv = if (saveFile.exists()) saveFile.readText() else ""
                if (sv.isNotEmpty() && Native.cmd("load", 0, 0, sv) == "ok") { started = true; refreshUi(); map.queueEvent { Native.cmd("mynat", 0, 0, "") } } else pickScreen()
                if (intent?.getBooleanExtra("autotest", false) == true) Thread { AutoTest(this, map).run() }.start()
            }
        } catch (t: Throwable) {
            runOnUiThread { fatal("Ошибка загрузки: ${t.message}") }
        }
    }

    private var loadText: TextView? = null
    private var loadBar: View? = null
    private var loadPct = 0
    private fun setLoad(t: String, pct: Int? = null) = runOnUiThread {
        loadText?.text = t
        if (pct != null && pct > loadPct) {
            loadPct = pct
            loadBar?.let { b -> val lp = b.layoutParams; lp.width = u.dp(220) * pct / 100; b.layoutParams = lp }
        }
    }
    private fun showLoading() {
        val l = u.col().apply {
            gravity = Gravity.CENTER
            background = android.graphics.drawable.GradientDrawable().apply {
                gradientType = android.graphics.drawable.GradientDrawable.RADIAL_GRADIENT
                colors = intArrayOf(Color.rgb(19, 37, 74), Color.rgb(6, 10, 20))
                setGradientCenter(0.5f, 0.4f)
                gradientRadius = resources.displayMetrics.widthPixels * 0.6f
            }
            addView(u.text("GeoPolit", 28f, C.TXT, true).apply { gravity = Gravity.CENTER })
            addView(u.space(1, 18))
            val t = u.text("Загрузка карты мира…", 16f, C.TXT, true).apply { gravity = Gravity.CENTER; minWidth = u.dp(260) }
            loadText = t
            addView(t)
            addView(u.space(1, 12))
            val track = FrameLayout(this@MainActivity).apply {
                background = u.bg(Color.argb(30, 255, 255, 255), 4f, Color.TRANSPARENT)
                layoutParams = LinearLayout.LayoutParams(u.dp(220), u.dp(8))
            }
            val bar = View(this@MainActivity).apply { background = u.bg(C.ACC, 4f, Color.TRANSPARENT) }
            track.addView(bar, FrameLayout.LayoutParams(0, -1))
            loadBar = bar
            addView(track)
        }
        loading = l
        root.addView(l, FrameLayout.LayoutParams(-1, -1))
    }
    fun fatal(msg: String) {
        modal(u.col().apply { addView(u.text(msg, 16f, C.TXT, true)) }, dismissable = false)
    }

    // ---------------- HUD ----------------
    private lateinit var meChip: LinearLayout
    private lateinit var moneyT: TextView
    private lateinit var incT: TextView
    private lateinit var popT: TextView
    private lateinit var armyT: TextView
    private lateinit var dateT: TextView
    private lateinit var clockT: TextView
    private val spdBtns = ArrayList<View>()

    private fun buildHud() {
        meChip = u.chip()
        moneyT = u.text("", 12.5f, C.TXT, true); incT = u.text("", 11f, C.GOOD)
        popT = u.text("", 12.5f, C.TXT, true); armyT = u.text("", 12.5f, C.TXT, true)
        val army = ImageView(this).apply {
            try { setImageBitmap(assets.open("army.png").use { BitmapFactory.decodeStream(it) }) } catch (_: Throwable) {}
            layoutParams = LinearLayout.LayoutParams(u.dp(24), u.dp(17))
        }
        hud = u.row(meChip, u.chip(u.icon("coins", C.GOLD, 16), moneyT, incT), u.chip(u.icon("users", C.GOLD, 16), popT), u.chip(army, armyT), gap = 4)
        hud.visibility = View.GONE
        root.addView(hud, u.lp(-2, -2, Gravity.START or Gravity.TOP, 8, 8, 0, 0))
        dateT = u.text("", 12.5f, C.TXT, true); clockT = u.text("", 12f, C.MUT, true)
        val clock = u.row(u.icon("calendar", C.GOLD), dateT, clockT, gap = 6).apply { setPadding(u.dp(4), 0, u.dp(8), 0) }
        tbox = u.row(clock, gap = 2).apply { background = u.bg(C.PANEL, 12f); setPadding(u.dp(8), u.dp(3), u.dp(3), u.dp(3)); visibility = View.GONE }
        val icons = listOf("pause", "play", "chevrons-right", "fast-forward")
        for ((i, ic) in icons.withIndex()) {
            val b = FrameLayout(this).apply {
                addView(u.icon(ic, C.TXT, 18), FrameLayout.LayoutParams(u.dp(18), u.dp(18), Gravity.CENTER))
                setPadding(u.dp(8), u.dp(6), u.dp(8), u.dp(6))
                contentDescription = "speed$i"
                setOnClickListener { Native.cmd("speed", i.toLong(), 0, ""); refreshUi() }
            }
            spdBtns.add(b); tbox.addView(b)
        }
        tbox.addView(FrameLayout(this).apply {
            addView(u.icon("menu", C.TXT, 18), FrameLayout.LayoutParams(u.dp(18), u.dp(18), Gravity.CENTER))
            setPadding(u.dp(8), u.dp(6), u.dp(8), u.dp(6))
            contentDescription = "menu"
            setOnClickListener { menu() }
        })
        root.addView(tbox, u.lp(-2, -2, Gravity.END or Gravity.TOP, 0, 8, 8, 0))
    }

    private fun applyHud(j: JSONObject?) {
        if (j == null) { hud.visibility = View.GONE; tbox.visibility = View.GONE; return }
        hud.visibility = View.VISIBLE; tbox.visibility = View.VISIBLE
        val key = j.optString("flag") + j.optString("name")
        if (meChip.tag != key) {
            meChip.tag = key; meChip.removeAllViews()
            meChip.addView(u.flag(j.optString("flag"), 22, 16)); meChip.addView(u.space(5, 1)); meChip.addView(u.text(j.optString("name"), 12.5f, C.TXT, true))
        }
        moneyT.text = j.optString("money"); incT.text = j.optString("inc"); popT.text = j.optString("pop"); armyT.text = j.optString("army")
        dateT.text = j.optString("date"); clockT.text = j.optString("clock")
        val sp = j.optInt("speed")
        for ((i, b) in spdBtns.withIndex()) b.background = if (i == minOf(sp, 3)) u.bg(C.ACC, 9f, Color.TRANSPARENT) else null
    }
    private fun refreshHudOnly() {
        if (!loaded) return
        try {
            val j = JSONObject(Native.ui())
            applyHud(j.optJSONObject("hud"))
            handleEvents(j)
        } catch (_: Throwable) {}
    }

    // ---------------- UI refresh ----------------
    // coalesce refresh requests from the GL thread (at most ~6 per second)
    private var refreshPending = false
    private var lastRefresh = 0L
    fun requestRefresh() {
        runOnUiThread {
            if (refreshPending) return@runOnUiThread
            val wait = maxOf(0L, 160L - (System.currentTimeMillis() - lastRefresh))
            refreshPending = true
            h.postDelayed({ refreshPending = false; lastRefresh = System.currentTimeMillis(); refreshUi() }, wait)
        }
    }
    fun refreshUi() {
        if (!loaded) return
        val j = try { JSONObject(Native.ui()) } catch (_: Throwable) { return }
        started = j.optBoolean("started")
        applyHud(j.optJSONObject("hud"))
        handleEvents(j)
        val pc = j.optJSONObject("pc")
        if (pc != null) { showPc(pc); return } else if (pcView != null) closePcView()
        val p = j.optJSONObject("panel")
        val ps = p?.toString() ?: ""
        if (ps != lastPanel) { lastPanel = ps; showPanel(p) }
        val f = j.optJSONObject("ftb")
        val fs = f?.toString() ?: ""
        if (fs != lastFtb) { lastFtb = fs; showFtb(f) }
    }
    private fun handleEvents(j: JSONObject) {
        val t = j.optJSONArray("toasts")
        if (t != null && t.length() > 0) toast(t.getString(t.length() - 1))
        if (!j.isNull("over")) gameOver(j.optBoolean("over"))
    }
    private var toastR: Runnable? = null
    fun toast(s: String) {
        toastV.text = s; toastV.visibility = View.VISIBLE
        toastR?.let { h.removeCallbacks(it) }
        toastR = Runnable { toastV.visibility = View.GONE }.also { h.postDelayed(it, 2600) }
    }

    fun onTap(r: String) {
        try {
            val j = JSONObject(r)
            if (j.has("pick")) { confirmPick(j.getInt("pick")); return }
        } catch (_: Throwable) {}
        refreshUi()
    }

    // ---------------- region panel / sea card ----------------
    private fun showPanel(p: JSONObject?) {
        if (p == null) { panel.visibility = View.GONE; seaCard.visibility = View.GONE; return }
        if (p.optString("type") == "sea") {
            panel.visibility = View.GONE
            seaCard.removeAllViews()
            val img = ImageView(this).apply {
                scaleType = ImageView.ScaleType.CENTER_CROP
                try { setImageBitmap(assets.open("sea/${p.optString("img")}.jpg").use { BitmapFactory.decodeStream(it) }) } catch (_: Throwable) {}
                layoutParams = LinearLayout.LayoutParams(u.dp(62), -1)
            }
            seaCard.addView(img)
            val st = u.col().apply {
                addView(u.text(p.optString("title"), 13f, C.TXT, true).apply { maxLines = 1; ellipsize = android.text.TextUtils.TruncateAt.END })
                addView(u.space(1, 3))
                addView(u.text(p.optString("tag"), 10f, C.MUT).apply { background = u.bg(Color.argb(18, 255, 255, 255), 7f, Color.TRANSPARENT); setPadding(u.dp(6), u.dp(1), u.dp(6), u.dp(1)) })
                layoutParams = LinearLayout.LayoutParams(0, -2, 1f)
            }
            seaCard.addView(st)
            seaCard.addView(FrameLayout(this).apply {
                addView(u.icon("x", C.TXT, 14), FrameLayout.LayoutParams(u.dp(14), u.dp(14), Gravity.CENTER))
                setPadding(u.dp(8), u.dp(8), u.dp(8), u.dp(8)); setOnClickListener { Native.cmd("close", 0, 0, ""); refreshUi() }
            })
            seaCard.visibility = View.VISIBLE
            return
        }
        seaCard.visibility = View.GONE
        panelBox.removeAllViews()
        val head = u.row(u.flag(p.optString("flag"), 66, 46), u.col().apply {
            addView(u.text(p.optString("title"), 18f, C.TXT, true))
            addView(u.space(1, 4))
            addView(u.row(u.icon("map-pin", C.MUT, 12), u.text(p.optString("tag"), 11f, C.MUT), gap = 4).apply {
                background = u.bg(Color.argb(18, 255, 255, 255), 7f, Color.TRANSPARENT); setPadding(u.dp(7), u.dp(3), u.dp(7), u.dp(3))
            })
            layoutParams = LinearLayout.LayoutParams(0, -2, 1f)
        }, FrameLayout(this).apply {
            addView(u.icon("x", C.TXT, 18), FrameLayout.LayoutParams(u.dp(18), u.dp(18), Gravity.CENTER))
            setPadding(u.dp(6), u.dp(6), u.dp(6), u.dp(6)); setOnClickListener { Native.cmd("close", 0, 0, ""); refreshUi() }
        }, gap = 10)
        panelBox.addView(head)
        p.optJSONObject("hist")?.let { hi ->
            panelBox.addView(u.space(1, 8))
            panelBox.addView(u.chip(u.flag(hi.optString("flag")), u.text("Исторически:", 13f, C.MUT), u.text(hi.optString("name"), 13f, C.TXT, true)))
        }
        panelBox.addView(u.space(1, 8))
        val rows = p.optJSONArray("rows") ?: JSONArray()
        for (i in 0 until rows.length()) {
            val r = rows.getJSONArray(i)
            val rv = LinearLayout(this).apply {
                orientation = LinearLayout.HORIZONTAL; setPadding(0, u.dp(6), 0, u.dp(6))
                addView(u.text(r.getString(0), 14f, C.MUT).apply { layoutParams = LinearLayout.LayoutParams(0, -2, 1f) })
                addView(u.text(r.getString(1), 14f, C.TXT, true))
            }
            panelBox.addView(rv)
            panelBox.addView(View(this).apply { setBackgroundColor(C.LINE); layoutParams = LinearLayout.LayoutParams(-1, u.dp(1)) })
        }
        val bts = p.optJSONArray("buttons") ?: JSONArray()
        val grid = GridLayout(this).apply { columnCount = 2 }
        for (i in 0 until bts.length()) {
            val b = bts.getJSONObject(i)
            val v = u.button(b.optString("label"), b.optString("icon"), b.optString("sub"), b.optString("cls"), b.optBoolean("en", true)) {
                onPanelCmd(b.optString("cmd"), b.optLong("a"), b.optLong("b"))
            }
            val lp = GridLayout.LayoutParams(GridLayout.spec(GridLayout.UNDEFINED), GridLayout.spec(GridLayout.UNDEFINED, 1f)).apply {
                width = 0; setMargins(u.dp(3), u.dp(3), u.dp(3), u.dp(3))
            }
            grid.addView(v, lp)
        }
        panelBox.addView(u.space(1, 8))
        panelBox.addView(grid)
        panel.visibility = View.VISIBLE
    }
    private fun onPanelCmd(c: String, a: Long, b: Long) {
        Native.cmd(c, a, b, "")
        if (c == "peace") lastPc = ""
        refreshUi()
    }

    // ---------------- front toolbar ----------------
    private fun showFtb(f: JSONObject?) {
        if (f == null) { ftb.visibility = View.GONE; map.drawMode = false; return }
        ftb.removeAllViews()
        val draw = f.optString("draw")
        map.drawMode = draw.isNotEmpty()
        val n = f.optInt("n")
        fun tb(label: String, icon: String?, on: Boolean, go: Boolean = false, act: () -> Unit) =
            u.button(label, icon, null, if (on) "on" else if (go) "go" else "", true, act).apply {
                background = u.bg(if (on) C.ACC else if (go) C.FTB_GO else C.PANEL, 12f); setPadding(u.dp(11), u.dp(7), u.dp(11), u.dp(7))
            }
        ftb.addView(tb(if (n > 0) "$n выбр." else "Все", "users", f.optBoolean("sel")) { Native.cmd("selall", 0, 0, ""); refreshUi() })
        ftb.addView(u.space(6, 1))
        ftb.addView(tb("Фронт", "shield", draw == "front") { Native.cmd("draw", 0, 0, "front"); refreshUi() })
        ftb.addView(u.space(6, 1))
        ftb.addView(tb("План", "arrow-right", draw == "plan") { Native.cmd("draw", 0, 0, "plan"); refreshUi() })
        ftb.addView(u.space(6, 1))
        ftb.addView(tb("Прорыв", "zap", draw == "spear") { Native.cmd("draw", 0, 0, "spear"); refreshUi() })
        ftb.addView(u.space(6, 1))
        ftb.addView(tb("Наступать", "swords", false, f.optBoolean("ready")) { Native.cmd("exec", 0, 0, ""); refreshUi() })
        ftb.addView(u.space(6, 1))
        ftb.addView(tb("Отладка", null, f.optBoolean("debug")) { Native.cmd("debug", 0, 0, ""); refreshUi() })
        ftb.visibility = View.VISIBLE
    }

    // ---------------- modals ----------------
    private fun modal(content: View, dismissable: Boolean = true): FrameLayout {
        closeModal()
        val m = FrameLayout(this).apply {
            setBackgroundColor(Color.argb(184, 3, 6, 14))
            isClickable = true
            setOnClickListener { if (dismissable && started) closeModal() }
        }
        val card = u.col().apply {
            background = u.bg(C.PANEL2, 18f); setPadding(u.dp(20), u.dp(20), u.dp(20), u.dp(20)); gravity = Gravity.CENTER_HORIZONTAL
            isClickable = true
            content.minimumWidth = u.dp(300)
            addView(content)
        }
        m.addView(card, FrameLayout.LayoutParams(-2, -2, Gravity.CENTER).apply { setMargins(u.dp(16), u.dp(16), u.dp(16), u.dp(16)) })
        root.addView(m, FrameLayout.LayoutParams(-1, -1))
        modal = m
        Native.cmd("pause", 1, 0, "")
        return m
    }
    private fun closeModal() {
        modal?.let { root.removeView(it) }; modal = null
        Native.cmd("pause", 0, 0, "")
        hideKeyboard()
    }
    private fun hideKeyboard() {
        val imm = getSystemService(INPUT_METHOD_SERVICE) as InputMethodManager
        imm.hideSoftInputFromWindow(root.windowToken, 0)
    }

    private fun pickScreen() {
        val c = u.col()
        c.addView(u.row(u.icon("globe", C.TXT, 22), u.text("Мировая Держава", 22f, C.TXT, true), gap = 8))
        c.addView(u.space(1, 8))
        c.addView(u.text("Выберите страну — коснитесь её на карте или найдите в списке", 14f, C.MUT))
        c.addView(u.space(1, 8))
        val search = EditText(this).apply {
            hint = "Поиск страны…"; setHintTextColor(C.MUT); setTextColor(C.TXT); background = u.bg(Color.argb(77, 0, 0, 0), 12f)
            setPadding(u.dp(12), u.dp(10), u.dp(12), u.dp(10)); isSingleLine = true
        }
        c.addView(search, LinearLayout.LayoutParams(u.dp(420), -2))
        c.addView(u.space(1, 8))
        val list = u.col()
        val sc = ScrollView(this).apply { addView(list) }
        c.addView(sc, LinearLayout.LayoutParams(u.dp(420), (resources.displayMetrics.heightPixels * 0.5).toInt()))
        val all = JSONArray(Native.nations())
        fun fill(q: String) {
            list.removeAllViews()
            var n = 0
            for (i in 0 until all.length()) {
                val o = all.getJSONObject(i)
                if (q.isNotEmpty() && !o.getString("n").lowercase().contains(q) && !o.getString("e").lowercase().contains(q)) continue
                if (++n > 300) break
                val sw = View(this).apply { background = u.bg(Color.parseColor(o.getString("col")), 4f, Color.TRANSPARENT); layoutParams = LinearLayout.LayoutParams(u.dp(12), u.dp(12)) }
                val it = u.row(sw, u.flag(o.getString("f")), u.text(o.getString("n"), 14f).apply { layoutParams = LinearLayout.LayoutParams(0, -2, 1f) },
                    u.text(o.getString("gdp"), 11f, C.MUT).apply { background = u.bg(Color.argb(18, 255, 255, 255), 7f, Color.TRANSPARENT); setPadding(u.dp(7), u.dp(3), u.dp(7), u.dp(3)) }, gap = 10)
                it.setPadding(u.dp(6), u.dp(7), u.dp(6), u.dp(7))
                val idx = o.getInt("i")
                it.setOnClickListener { confirmPick(idx) }
                list.addView(it)
                list.addView(View(this).apply { setBackgroundColor(C.LINE); layoutParams = LinearLayout.LayoutParams(-1, u.dp(1)) })
            }
        }
        fill("")
        search.addTextChangedListener(object : TextWatcher {
            override fun beforeTextChanged(s: CharSequence?, a: Int, b: Int, c: Int) {}
            override fun onTextChanged(s: CharSequence?, a: Int, b: Int, c: Int) {}
            override fun afterTextChanged(s: Editable?) { fill(s.toString().lowercase().trim()) }
        })
        c.addView(u.space(1, 10))
        c.addView(u.button("Выбрать на карте") { closeModal() })
        modal(c, dismissable = false)
    }
    private fun confirmPick(i: Int) {
        val o = JSONObject(Native.nationInfo(i))
        Native.zoomToNat(i)
        val c = u.col().apply { gravity = Gravity.CENTER_HORIZONTAL }
        c.addView(u.flag(o.getString("f"), 66, 46))
        c.addView(u.space(1, 8))
        c.addView(u.text(o.getString("n"), 22f, C.TXT, true))
        c.addView(u.space(1, 8))
        c.addView(u.text("Население: ${o.getString("pop")}\nВВП: ${o.getString("gdp")}", 14f, C.TXT).apply { gravity = Gravity.CENTER })
        c.addView(u.space(1, 12))
        c.addView(u.row(u.button("Назад") { pickScreen() }, u.button("Играть", cls = "p") {
            closeModal(); Native.cmd("new", i.toLong(), 0, ""); started = true; lastPanel = "?"; lastFtb = "?"; refreshUi(); save()
        }, gap = 8))
        modal(c, dismissable = false)
    }
    private fun menu() {
        val c = u.col().apply { gravity = Gravity.CENTER_HORIZONTAL }
        c.addView(u.text("Меню", 22f, C.TXT, true))
        c.addView(u.space(1, 6))
        c.addView(u.text(dateT.text.toString(), 14f, C.MUT))
        c.addView(u.space(1, 10))
        val bl = LinearLayout.LayoutParams(u.dp(240), -2).apply { setMargins(0, u.dp(4), 0, u.dp(4)) }
        c.addView(u.button("Продолжить", cls = "p") { closeModal() }, bl)
        c.addView(u.button("К моей стране") { closeModal(); Native.cmd("mynat", 0, 0, "") }, LinearLayout.LayoutParams(bl))
        c.addView(u.button("Сохранить", "save") { save(); closeModal(); toast("Сохранено") }, LinearLayout.LayoutParams(bl))
        c.addView(u.button("Новая игра", cls = "r") { restart() }, LinearLayout.LayoutParams(bl))
        modal(c)
    }
    private fun gameOver(win: Boolean) {
        saveFile.delete()
        Native.cmd("speed", 0, 0, "")
        val c = u.col().apply { gravity = Gravity.CENTER_HORIZONTAL }
        c.addView(u.row(u.icon(if (win) "globe" else "skull", C.TXT, 22), u.text(if (win) "Мировое господство!" else "Поражение", 22f, C.TXT, true), gap = 8))
        c.addView(u.space(1, 8))
        c.addView(u.text(if (win) "Весь мир под вашим флагом." else "Ваше государство пало.", 14f))
        c.addView(u.space(1, 12))
        c.addView(u.button("Новая игра", cls = "p") { restart() })
        modal(c, dismissable = false)
    }
    private fun restart() {
        saveFile.delete()
        closeModal()
        Native.cmd("restart", 0, 0, "")
        started = false; lastPanel = "?"; lastFtb = "?"
        refreshUi()
        pickScreen()
    }
    private fun save() {
        if (!loaded || !started) return
        try { val s = Native.cmd("save", 0, 0, ""); if (s.length > 10) saveFile.writeText(s) } catch (_: Throwable) {}
    }

    // ---------------- peace conference ----------------
    private var pcSide: LinearLayout? = null
    private var pcName: EditText? = null
    private var pcSpinner: Spinner? = null
    private var pcRegs = JSONArray()
    private var pcInfo: TextView? = null
    private var pcItems: LinearLayout? = null
    private var pcTools = HashMap<String, View>()

    private fun showPc(pc: JSONObject) {
        val tool = pc.optString("tool")
        map.pcMode = true
        map.pcPan = tool == "pan"
        if (pcView == null) buildPc(pc)
        val s = pc.toString()
        if (s == lastPc) return
        lastPc = s
        pcInfo?.text = pc.optString("info")
        for ((k, v) in pcTools) v.background = u.bg(if (k == tool) C.ACC else C.PANEL2, 12f)
        val regs = pc.optJSONArray("regs") ?: JSONArray()
        if (regs.toString() != pcRegs.toString()) {
            pcRegs = regs
            val names = (0 until regs.length()).map { regs.getJSONArray(it).getString(1) }
            pcSpinner?.adapter = object : ArrayAdapter<String>(this, android.R.layout.simple_spinner_item, names) {
                override fun getView(p: Int, cv: View?, parent: ViewGroup): View = (super.getView(p, cv, parent) as TextView).apply { setTextColor(C.TXT) }
            }.apply { setDropDownViewResource(android.R.layout.simple_spinner_dropdown_item) }
        }
        val items = pc.optJSONArray("items") ?: JSONArray()
        pcItems?.removeAllViews()
        if (items.length() == 0) pcItems?.addView(u.text("Пусто — подписание вернёт всё", 12f, C.MUT))
        for (i in 0 until items.length()) {
            val it = items.getJSONObject(i)
            val sw = View(this).apply { background = u.bg(Color.parseColor(it.getString("color")), 3f, Color.TRANSPARENT); layoutParams = LinearLayout.LayoutParams(u.dp(10), u.dp(10)) }
            val k = it.getLong("k")
            val del = u.text("✕", 12f).apply { setPadding(u.dp(8), u.dp(2), u.dp(8), u.dp(2)); background = u.bg(C.PANEL2, 8f); setOnClickListener { Native.cmd("pcdel", k, 0, ""); refreshUi() } }
            pcItems?.addView(u.row(sw, u.text(it.getString("label"), 13f).apply { layoutParams = LinearLayout.LayoutParams(0, -2, 1f) }, u.text(it.getString("pop"), 12f, C.MUT), del, gap = 6).apply { setPadding(0, u.dp(5), 0, u.dp(5)) })
        }
    }
    private fun buildPc(pc: JSONObject) {
        panel.visibility = View.GONE; seaCard.visibility = View.GONE; ftb.visibility = View.GONE; hud.visibility = View.GONE; tbox.visibility = View.GONE
        val v = FrameLayout(this)
        val bar = u.row(u.icon("bird", C.TXT, 18), u.text("Мирная конференция", 15f, C.TXT, true), u.space(10, 1),
            u.flag(pc.optString("meFlag")), u.text(pc.optString("meName"), 14f), u.text("—", 14f, C.MUT), u.flag(pc.optString("xFlag")), u.text(pc.optString("xName"), 14f), gap = 6).apply {
            setBackgroundColor(Color.rgb(11, 18, 32)); setPadding(u.dp(12), u.dp(8), u.dp(12), u.dp(8))
        }
        v.addView(bar, FrameLayout.LayoutParams(-1, u.dp(44), Gravity.TOP))
        val sideW = minOf(u.dp(320), (root.width * 0.42).toInt())
        val side = u.col().apply { setPadding(u.dp(10), u.dp(10), u.dp(10), u.dp(10)) }
        val sc = ScrollView(this).apply { addView(side); background = u.bg(C.PANEL, 0f) }
        v.addView(sc, FrameLayout.LayoutParams(sideW, -1, Gravity.END).apply { topMargin = u.dp(44) })
        pcInfo = u.text("", 13f)
        side.addView(pcInfo)
        side.addView(u.space(1, 8))
        val tools = GridLayout(this).apply { columnCount = 2 }
        for ((t, l, ic) in listOf(Triple("brush", "Кисть", "brush"), Triple("erase", "Ластик", "eraser"), Triple("zone", "Зона", "mouse-pointer-click"), Triple("pan", "Двигать", "move"))) {
            val b = u.button(l, ic) { Native.cmd("pctool", 0, 0, t); refreshUi() }
            pcTools[t] = b
            tools.addView(b, GridLayout.LayoutParams(GridLayout.spec(GridLayout.UNDEFINED), GridLayout.spec(GridLayout.UNDEFINED, 1f)).apply { width = 0; setMargins(u.dp(3), u.dp(3), u.dp(3), u.dp(3)) })
        }
        side.addView(tools)
        val sb = SeekBar(this).apply { max = 13; progress = 3; setOnSeekBarChangeListener(object : SeekBar.OnSeekBarChangeListener {
            override fun onProgressChanged(s: SeekBar?, p: Int, f: Boolean) { Native.cmd("pcbr", (p + 1).toLong(), 0, "") }
            override fun onStartTrackingTouch(s: SeekBar?) {}
            override fun onStopTrackingTouch(s: SeekBar?) {}
        }) }
        side.addView(u.row(u.text("Кисть", 13f, C.MUT), sb.apply { layoutParams = LinearLayout.LayoutParams(0, -2, 1f) }, gap = 8))
        side.addView(u.row(u.button("Выбрать всё", "check-check") { Native.cmd("pcall", 1, 0, ""); refreshUi() }.apply { layoutParams = LinearLayout.LayoutParams(0, -2, 1f) },
            u.button("Снять выбор", "x") { Native.cmd("pcall", 0, 0, ""); refreshUi() }.apply { layoutParams = LinearLayout.LayoutParams(0, -2, 1f) }, gap = 6))
        side.addView(u.space(1, 12))
        side.addView(u.text("Выделенное", 13f, C.TXT, true))
        pcName = EditText(this).apply {
            hint = "Название нового региона"; setHintTextColor(C.MUT); setTextColor(C.TXT); background = u.bg(Color.argb(102, 0, 0, 0), 9f); isSingleLine = true
            setPadding(u.dp(8), u.dp(7), u.dp(8), u.dp(7))
        }
        side.addView(pcName, LinearLayout.LayoutParams(-1, -2).apply { setMargins(0, u.dp(6), 0, u.dp(6)) })
        side.addView(u.row(u.button("Новый регион", "plus") { Native.cmd("pcitem", 0, -1, pcName?.text.toString()); pcName?.setText(""); hideKeyboard(); refreshUi() }.apply { layoutParams = LinearLayout.LayoutParams(0, -2, 1f) },
            u.button("Присоединить", "link") {
                val pos = pcSpinner?.selectedItemPosition ?: -1
                val r = if (pos >= 0 && pos < pcRegs.length()) pcRegs.getJSONArray(pos).getLong(0) else -1L
                Native.cmd("pcitem", 1, r, ""); refreshUi()
            }.apply { layoutParams = LinearLayout.LayoutParams(0, -2, 1f) }, gap = 6))
        pcSpinner = Spinner(this).apply { background = u.bg(Color.argb(102, 0, 0, 0), 9f) }
        side.addView(pcSpinner, LinearLayout.LayoutParams(-1, u.dp(40)).apply { setMargins(0, u.dp(6), 0, u.dp(6)) })
        side.addView(u.space(1, 6))
        side.addView(u.text("Условия договора", 13f, C.TXT, true))
        pcItems = u.col()
        side.addView(pcItems)
        side.addView(u.space(1, 10))
        side.addView(u.row(u.button("Подписать мир", "pen-line", cls = "p") { Native.cmd("pcsign", 0, 0, ""); lastPc = ""; refreshUi(); save() }.apply { layoutParams = LinearLayout.LayoutParams(0, -2, 1f) },
            u.button("Отмена") { Native.cmd("pcclose", 0, 0, ""); lastPc = ""; refreshUi() }.apply { layoutParams = LinearLayout.LayoutParams(0, -2, 1f) }, gap = 6))
        pcSide = side
        // transparent area over the map: the GL view below receives touches
        root.addView(v, FrameLayout.LayoutParams(-1, -1))
        v.post { Native.pcRect(0f, u.dp(44).toFloat(), (root.width - sideW).toFloat(), (root.height - u.dp(44)).toFloat()) }
        // let touches on the map area fall through to the GL view
        v.isClickable = false
        pcView = v
    }
    private fun closePcView() {
        pcView?.let { root.removeView(it) }; pcView = null; pcTools.clear(); lastPc = ""
        map.pcMode = false; map.pcPan = false
        lastPanel = "?"; lastFtb = "?"
        hideKeyboard()
        refreshUi()
    }

    // ---------------- lifecycle ----------------
    private fun hideBars() {
        if (android.os.Build.VERSION.SDK_INT >= 30) {
            window.insetsController?.let {
                it.hide(WindowInsets.Type.systemBars())
                it.systemBarsBehavior = WindowInsetsController.BEHAVIOR_SHOW_TRANSIENT_BARS_BY_SWIPE
            }
        } else {
            @Suppress("DEPRECATION")
            window.decorView.systemUiVisibility = View.SYSTEM_UI_FLAG_FULLSCREEN or View.SYSTEM_UI_FLAG_HIDE_NAVIGATION or View.SYSTEM_UI_FLAG_IMMERSIVE_STICKY
        }
    }
    override fun onPause() { save(); map.onPause(); super.onPause() }
    override fun onResume() { super.onResume(); map.onResume(); hideBars() }

    @Deprecated("Deprecated in Java")
    override fun onBackPressed() {
        when {
            modal != null -> { if (started) closeModal() }
            pcView != null -> { Native.cmd("pcclose", 0, 0, ""); refreshUi() }
            panel.visibility == View.VISIBLE || seaCard.visibility == View.VISIBLE -> { Native.cmd("close", 0, 0, ""); refreshUi() }
            started -> menu()
            else -> @Suppress("DEPRECATION") super.onBackPressed()
        }
    }
}
