package com.geopolit.game

import android.annotation.SuppressLint
import android.content.Context
import android.graphics.Bitmap
import android.graphics.BitmapFactory
import android.opengl.GLSurfaceView
import android.view.MotionEvent
import android.view.ScaleGestureDetector
import java.nio.ByteBuffer
import javax.microedition.khronos.egl.EGLConfig
import javax.microedition.khronos.opengles.GL10
import kotlin.math.hypot

// GL surface: every frame is rendered by the Rust core; touches are turned into pan/zoom/tap/draw calls.
@SuppressLint("ViewConstructor")
class MapView(ctx: Context, private val host: MainActivity) : GLSurfaceView(ctx) {
    @Volatile var ready = false
    private var lastUi = -1L
    var drawMode = false
    var pcMode = false
    var pcPan = false

    init {
        setEGLContextClientVersion(3)
        setEGLConfigChooser(8, 8, 8, 8, 0, 8)
        preserveEGLContextOnPause = true
        setRenderer(object : Renderer {
            override fun onSurfaceCreated(gl: GL10?, config: EGLConfig?) {
                if (!ready) return
                val err = Native.surfaceCreated()
                if (err.isNotEmpty()) host.runOnUiThread { host.fatal("Ошибка графики: $err") }
                uploadImages()
                lastUi = -1
            }
            override fun onSurfaceChanged(gl: GL10?, w: Int, h: Int) {
                if (!ready) return
                Native.resize(w.toFloat(), h.toFloat(), resources.displayMetrics.density)
            }
            override fun onDrawFrame(gl: GL10?) {
                if (!ready) return
                val v = Native.frame(System.nanoTime() / 1e6)
                if (v != lastUi) { lastUi = v; host.requestRefresh() }
            }
        })
        renderMode = RENDERMODE_CONTINUOUSLY
    }

    // called once the core has loaded: create GL resources on the GL thread
    fun start() {
        ready = true
        queueEvent {
            val err = Native.surfaceCreated()
            if (err.isNotEmpty()) host.runOnUiThread { host.fatal("Ошибка графики: $err") }
            Native.resize(width.toFloat(), height.toFloat(), resources.displayMetrics.density)
            uploadImages()
        }
    }

    private fun uploadImages() {
        val list = mutableListOf(-1 to "bg.jpg")
        for (ty in 0..1) for (tx in 0..3) list.add((ty * 4 + tx) to "bg/${tx}_${ty}.jpg")
        for ((slot, name) in list) {
            try {
                val o = BitmapFactory.Options().apply { inPreferredConfig = Bitmap.Config.ARGB_8888 }
                val b = context.assets.open(name).use { BitmapFactory.decodeStream(it, null, o) } ?: continue
                val buf = ByteBuffer.allocate(b.byteCount)
                b.copyPixelsToBuffer(buf)
                Native.image(slot, b.width, b.height, buf.array())
                b.recycle()
            } catch (_: Throwable) {}
        }
    }

    // ---------- touch ----------
    private var lx = 0f
    private var ly = 0f
    private var downX = 0f
    private var downY = 0f
    private var downT = 0L
    private var moved = false
    private var multi = false
    private var drawing = false
    private val scaler = ScaleGestureDetector(ctx, object : ScaleGestureDetector.SimpleOnScaleGestureListener() {
        override fun onScale(d: ScaleGestureDetector): Boolean {
            val f = d.scaleFactor
            val x = d.focusX
            val y = d.focusY
            queueEvent { Native.zoom(f, x, y) }
            return true
        }
    })

    @SuppressLint("ClickableViewAccessibility")
    override fun onTouchEvent(e: MotionEvent): Boolean {
        if (!ready) return true
        scaler.onTouchEvent(e)
        val x = e.x
        val y = e.y
        when (e.actionMasked) {
            MotionEvent.ACTION_DOWN -> {
                lx = x; ly = y; downX = x; downY = y; downT = System.currentTimeMillis(); moved = false; multi = false; drawing = false
                if (pcMode && !pcPan) { drawing = true; queueEvent { Native.pcTouch(0, x, y) } }
                else if (drawMode) { drawing = true; queueEvent { Native.drawTouch(0, x, y) } }
            }
            MotionEvent.ACTION_POINTER_DOWN -> {
                multi = true
                if (drawing) { drawing = false; queueEvent { if (pcMode) Native.pcTouch(2, x, y) else Native.drawTouch(2, x, y) } }
                lx = focusX(e); ly = focusY(e)
            }
            MotionEvent.ACTION_MOVE -> {
                if (hypot(x - downX, y - downY) > 12 * resources.displayMetrics.density) moved = true
                if (drawing) {
                    queueEvent { if (pcMode) Native.pcTouch(1, x, y) else Native.drawTouch(1, x, y) }
                } else {
                    val fx = focusX(e); val fy = focusY(e)
                    val dx = fx - lx; val dy = fy - ly
                    lx = fx; ly = fy
                    if (moved || multi) queueEvent { Native.pan(dx, dy) }
                }
            }
            MotionEvent.ACTION_POINTER_UP -> {
                // re-anchor on the remaining finger(s)
                val idx = e.actionIndex
                var sx = 0f; var sy = 0f; var n = 0
                for (i in 0 until e.pointerCount) if (i != idx) { sx += e.getX(i); sy += e.getY(i); n++ }
                if (n > 0) { lx = sx / n; ly = sy / n }
            }
            MotionEvent.ACTION_UP -> {
                if (drawing) {
                    drawing = false
                    queueEvent { if (pcMode) Native.pcTouch(2, x, y) else Native.drawTouch(2, x, y); host.runOnUiThread { host.refreshUi() } }
                } else if (!moved && !multi && System.currentTimeMillis() - downT < 450) {
                    queueEvent {
                        val r = Native.tap(x, y)
                        host.runOnUiThread { host.onTap(r) }
                    }
                }
            }
            MotionEvent.ACTION_CANCEL -> {
                if (drawing) { drawing = false; queueEvent { if (pcMode) Native.pcTouch(2, x, y) else Native.drawTouch(2, x, y) } }
            }
        }
        return true
    }
    private fun focusX(e: MotionEvent): Float { var s = 0f; for (i in 0 until e.pointerCount) s += e.getX(i); return s / e.pointerCount }
    private fun focusY(e: MotionEvent): Float { var s = 0f; for (i in 0 until e.pointerCount) s += e.getY(i); return s / e.pointerCount }
}
