package dev.sails

import android.content.Context
import android.view.Choreographer
import android.view.MotionEvent
import android.view.SurfaceHolder
import android.view.SurfaceView
import java.nio.ByteBuffer
import java.nio.ByteOrder

/**
 * The one View on screen. Everything visible is drawn by the Rust render
 * thread onto this surface (EGL + OpenGL ES); this class only:
 *  - forwards surface lifecycle to Rust,
 *  - forwards touches to Rust (in surface pixels),
 *  - runs a choreographer loop that drains Rust's event queue and executes
 *    the corresponding Cefrium calls on the UI thread,
 *  - pushes engine nav state (url/progress/history) to Rust each tick.
 */
class SailsSurfaceView(
    context: Context,
    private val host: CefriumHost,
) : SurfaceView(context), SurfaceHolder.Callback, Choreographer.FrameCallback {

    private val pollBuf: ByteBuffer =
        ByteBuffer.allocateDirect(64 * 1024).order(ByteOrder.LITTLE_ENDIAN)

    private var loopRunning = false
    private var surfaceReady = false

    init {
        holder.addCallback(this)
        isFocusable = true
        isFocusableInTouchMode = true
    }

    // ---- SurfaceHolder.Callback ------------------------------------------------

    override fun surfaceCreated(holder: SurfaceHolder) {
        NativeLib.nativeSurfaceCreated(holder.surface)
    }

    override fun surfaceChanged(holder: SurfaceHolder, format: Int, width: Int, height: Int) {
        val density = resources.displayMetrics.density
        NativeLib.nativeConfig(density)
        // Lazily construct the engine at the real page size (surface height
        // minus the 60dp native chrome bar).
        host.ensureBrowser(width, height, density)
        surfaceReady = true
        startLoop()
    }

    override fun surfaceDestroyed(holder: SurfaceHolder) {
        surfaceReady = false
        NativeLib.nativeSurfaceDestroyed()
    }

    // ---- Choreographer ---------------------------------------------------------

    private fun startLoop() {
        if (loopRunning || !isAttachedToWindow) return
        loopRunning = true
        Choreographer.getInstance().postFrameCallback(this)
    }

    fun onActivityResumed() {
        if (surfaceReady) startLoop()
    }

    fun onActivityPaused() {
        loopRunning = false
    }

    override fun doFrame(frameTimeNanos: Long) {
        if (!loopRunning) return
        drainEvents()
        host.pollNavState()
        Choreographer.getInstance().postFrameCallback(this)
    }

    // ---- Touch ------------------------------------------------------------------

    override fun onTouchEvent(event: MotionEvent): Boolean {
        val masked = event.actionMasked
        // Single-pointer PoC: a second finger cancels the ongoing gesture.
        if (event.pointerCount > 1) {
            if (masked == MotionEvent.ACTION_POINTER_DOWN) {
                NativeLib.nativeTouch(ACTION_CANCEL_RUST, 0f, 0f)
            }
            return true
        }
        when (masked) {
            MotionEvent.ACTION_DOWN,
            MotionEvent.ACTION_MOVE,
            MotionEvent.ACTION_UP,
            MotionEvent.ACTION_CANCEL,
            -> NativeLib.nativeTouch(masked, event.x, event.y)
        }
        return true
    }

    // ---- Event draining ----------------------------------------------------------

    private fun drainEvents() {
        pollBuf.rewind()
        val count = NativeLib.nativePoll(pollBuf)
        if (count <= 0) return

        pollBuf.rewind()
        for (i in 0 until count) {
            val kind = pollBuf.int
            val a = pollBuf.int
            val b = pollBuf.int
            val x = pollBuf.float
            val y = pollBuf.float
            val strLen = pollBuf.int
            var str = ""
            if (strLen > 0) {
                val bytes = ByteArray(strLen)
                pollBuf.get(bytes)
                str = String(bytes, Charsets.UTF_8)
            }
            execute(kind, a, b, x, y, str)
        }
    }

    private fun execute(kind: Int, a: Int, b: Int, x: Float, y: Float, str: String) {
        when (kind) {
            Events.LOAD_URL -> host.loadUrl(str)
            Events.GO_BACK -> host.goBack()
            Events.GO_FORWARD -> host.goForward()
            Events.RELOAD -> host.reload()
            Events.STOP -> host.stopLoading()
            Events.PAGE_TOUCH_DOWN -> host.sendTouchDown(x, y)
            Events.PAGE_TOUCH_MOVE -> host.sendTouchMove(x, y)
            Events.PAGE_TOUCH_UP -> host.sendTouchUp(x, y)
            Events.SHOW_IME -> ime.show(str)
            Events.HIDE_IME -> ime.hide()
            Events.SET_PAGE_FOCUS -> host.setFocus(a == 1)
            Events.RESIZE_BROWSER -> host.setSize(a, b)
        }
    }

    fun attachIme(ime: ImeController) {
        this.ime = ime
    }

    private var ime: ImeController = OmniboxIme.NO_OP

    companion object {
        // Sent to Rust when a second finger lands (Rust's ACTION_CANCEL).
        const val ACTION_CANCEL_RUST = 3
    }
}
