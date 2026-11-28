package com.geopolit.game

import android.content.Context
import android.graphics.Color
import android.graphics.Typeface
import android.graphics.drawable.GradientDrawable
import android.text.TextUtils
import android.util.TypedValue
import android.view.Gravity
import android.view.View
import android.widget.FrameLayout
import android.widget.ImageView
import android.widget.LinearLayout
import android.widget.TextView
import android.graphics.BitmapFactory

// Small view toolkit reproducing the old HTML look (dark glass panels, rounded chips, lucide icons).
object C {
    val PANEL = Color.argb(219, 12, 18, 32)
    val PANEL2 = Color.argb(230, 20, 30, 52)
    val LINE = Color.argb(41, 140, 170, 220)
    val TXT = Color.rgb(238, 243, 255)
    val MUT = Color.rgb(147, 163, 196)
    val ACC = Color.rgb(76, 141, 255)
    val GOOD = Color.rgb(63, 208, 122)
    val BAD = Color.rgb(255, 90, 90)
    val GOLD = Color.rgb(242, 193, 78)
    val RED = Color.rgb(145, 33, 38)
    val GREEN = Color.rgb(27, 107, 61)
    val FTB_GO = Color.rgb(47, 158, 68)
}

class Ui(val ctx: Context) {
    val d = ctx.resources.displayMetrics.density
    fun dp(v: Float) = (v * d).toInt()
    fun dp(v: Int) = (v * d).toInt()

    fun bg(color: Int, radius: Float, stroke: Int = C.LINE): GradientDrawable =
        GradientDrawable().apply { setColor(color); cornerRadius = radius * d; setStroke(dp(1), stroke) }

    fun text(s: String, size: Float = 14f, color: Int = C.TXT, bold: Boolean = false): TextView = TextView(ctx).apply {
        text = s; setTextColor(color); setTextSize(TypedValue.COMPLEX_UNIT_SP, size)
        if (bold) typeface = Typeface.DEFAULT_BOLD
        includeFontPadding = false
    }

    fun icon(name: String, color: Int = C.TXT, size: Int = 18): ImageView = ImageView(ctx).apply {
        val id = ctx.resources.getIdentifier("ic_" + name.replace('-', '_'), "drawable", ctx.packageName)
        if (id != 0) setImageResource(id)
        setColorFilter(color)
        layoutParams = LinearLayout.LayoutParams(dp(size), dp(size))
    }

    private val flagCache = HashMap<String, android.graphics.Bitmap?>()
    fun flag(code: String, w: Int = 28, h: Int = 20): View {
        val iv = ImageView(ctx)
        iv.scaleType = ImageView.ScaleType.CENTER_CROP
        val bmp = flagCache.getOrPut(code) {
            try { ctx.assets.open("flagpng/$code.png").use { BitmapFactory.decodeStream(it) } } catch (_: Throwable) { null }
        }
        if (bmp != null) iv.setImageBitmap(bmp) else iv.setBackgroundColor(Color.WHITE)
        iv.clipToOutline = true
        iv.background = bg(Color.WHITE, 4f, Color.argb(38, 255, 255, 255))
        iv.layoutParams = LinearLayout.LayoutParams(dp(w), dp(h))
        return iv
    }

    fun row(vararg v: View, gap: Int = 6): LinearLayout = LinearLayout(ctx).apply {
        orientation = LinearLayout.HORIZONTAL; gravity = Gravity.CENTER_VERTICAL
        v.forEachIndexed { i, c -> if (i > 0) addView(space(gap, 1)); addView(c) }
    }
    fun col(): LinearLayout = LinearLayout(ctx).apply { orientation = LinearLayout.VERTICAL }
    fun space(w: Int, h: Int): View = View(ctx).apply { layoutParams = LinearLayout.LayoutParams(dp(w), dp(h)) }

    fun chip(vararg v: View): LinearLayout = row(*v, gap = 7).apply {
        background = bg(C.PANEL, 12f); setPadding(dp(10), dp(6), dp(10), dp(6))
    }

    // button with optional icon, label and small sub-label; cls: "", "p" (accent), "r" (red), "g" (green), "on"
    fun button(label: String, icon: String? = null, sub: String? = null, cls: String = "", enabled: Boolean = true, onClick: () -> Unit): LinearLayout {
        val b = col()
        b.gravity = Gravity.CENTER
        val top = LinearLayout(ctx).apply { orientation = LinearLayout.HORIZONTAL; gravity = Gravity.CENTER }
        if (icon != null) { top.addView(icon(icon, C.TXT, 18)); top.addView(space(6, 1)) }
        top.addView(text(label, 14f, C.TXT, true).apply { maxLines = 2; ellipsize = TextUtils.TruncateAt.END })
        b.addView(top)
        if (!sub.isNullOrEmpty()) b.addView(text(sub, 11f, Color.argb(190, 238, 243, 255)).apply { gravity = Gravity.CENTER })
        val color = when (cls) { "p", "on" -> C.ACC; "r" -> C.RED; "g" -> C.GREEN; "go" -> C.FTB_GO; else -> C.PANEL2 }
        b.background = bg(color, 12f, if (cls.isEmpty()) C.LINE else Color.TRANSPARENT)
        b.setPadding(dp(12), dp(8), dp(12), dp(8))
        b.isEnabled = enabled
        b.alpha = if (enabled) 1f else 0.38f
        b.setOnClickListener { if (enabled) onClick() }
        return b
    }

    fun lp(w: Int, h: Int, gravity: Int = Gravity.NO_GRAVITY, l: Int = 0, t: Int = 0, r: Int = 0, b: Int = 0) =
        FrameLayout.LayoutParams(w, h, gravity).apply { setMargins(dp(l), dp(t), dp(r), dp(b)) }
}
