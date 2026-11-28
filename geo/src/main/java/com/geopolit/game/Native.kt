package com.geopolit.game

// JNI bridge to the Rust core (libgeopolit.so): simulation, map rendering (OpenGL ES 3) and game rules.
object Native {
    init { System.loadLibrary("geopolit") }

    external fun load(nations: ByteArray, lo: ByteArray, mid: ByteArray, hi: ByteArray, water: ByteArray, marine: ByteArray,
                      cities: ByteArray, fontRegular: ByteArray, fontSerif: ByteArray): Boolean
    external fun surfaceCreated(): String
    external fun image(slot: Int, w: Int, h: Int, rgba: ByteArray)
    external fun resize(w: Float, h: Float, dpr: Float)
    external fun frame(nowMs: Double): Long
    external fun pan(dx: Float, dy: Float)
    external fun zoom(f: Float, x: Float, y: Float)
    external fun tap(x: Float, y: Float): String
    external fun drawTouch(phase: Int, x: Float, y: Float): Boolean
    external fun pcTouch(phase: Int, x: Float, y: Float): Boolean
    external fun pcRect(x: Float, y: Float, w: Float, h: Float)
    external fun ui(): String
    external fun nations(): String
    external fun nationInfo(i: Int): String
    external fun cmd(c: String, a: Long, b: Long, s: String): String
    external fun zoomToNat(n: Int)
}
