package dev.sails

import android.view.Surface
import java.nio.ByteBuffer

/**
 * JNI surface of the native Rust core (`rust/` -> libsails.so).
 *
 * All functions here are called from the UI thread only, except where noted.
 * The Rust side never calls back into Java: it pushes actions into an event
 * queue that [nativePoll] drains (see [Events] for the wire format).
 */
object NativeLib {
    init {
        System.loadLibrary("sails")
    }

    /** UI scale (density) + one-time spawn of the Rust render thread. */
    external fun nativeConfig(density: Float)

    /**
     * One RGBA frame from Cefrium's onPaint (UI thread). `pixels` must be a
     * direct ByteBuffer; Rust copies it out before we return, satisfying the
     * "copy before returning" contract of CefriumRenderHandler.onPaint.
     */
    external fun nativeOnFrame(width: Int, height: Int, pixels: ByteBuffer)

    /** Raw touch in *surface* pixels. action = MotionEvent actionMasked. */
    external fun nativeTouch(action: Int, x: Float, y: Float)

    /** Engine state pushed in every frame tick: url may be null before first navigation. */
    external fun nativeNavState(url: String?, progress: Int, canBack: Boolean, canFwd: Boolean)

    /** Omnibox text changed by the IME (see OmniboxIme). */
    external fun nativeImeText(text: String?)

    /** Omnibox GO action pressed. */
    external fun nativeImeDone()

    /**
     * Activity Back pressed. Returns true if Rust handled it (close omnibox /
     * go back in history), false if the app should move itself to the back.
     */
    external fun nativeBack(): Boolean

    /** Surface lifecycle. Rust owns the ANativeWindow from here on. */
    external fun nativeSurfaceCreated(surface: Surface)
    external fun nativeSurfaceDestroyed()

    external fun nativePause()
    external fun nativeResume()

    /**
     * Drains pending Rust->Java events into `out` (direct ByteBuffer,
     * LITTLE_ENDIAN). Returns the number of events written. Wire format, per
     * event: kind:i32, a:i32, b:i32, x:f32, y:f32, strLen:i32, str bytes.
     */
    external fun nativePoll(out: ByteBuffer): Int
}

/** Event kinds written by Rust into [NativeLib.nativePoll]'s buffer. */
object Events {
    const val LOAD_URL = 1
    const val GO_BACK = 2
    const val GO_FORWARD = 3
    const val RELOAD = 4
    const val STOP = 5
    const val PAGE_TOUCH_DOWN = 6
    const val PAGE_TOUCH_MOVE = 7
    const val PAGE_TOUCH_UP = 8
    const val SHOW_IME = 9
    const val HIDE_IME = 10
    const val SET_PAGE_FOCUS = 11
    const val RESIZE_BROWSER = 12
}
