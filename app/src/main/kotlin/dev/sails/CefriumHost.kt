package dev.sails

import android.app.Activity
import com.cefrium.CefriumBrowser
import com.cefrium.CefriumRenderHandler
import java.nio.ByteBuffer

/**
 * The only class in the app that talks to the Cefrium SDK directly.
 *
 * The engine runs off-screen: [CefriumBrowser] is constructed with a pixel
 * size and a scale (like the headless path in the Cefrium quickstart) and
 * every painted frame is handed to the Rust compositor via
 * [NativeLib.nativeOnFrame]. Rendering the pixels, the browser chrome, and
 * all input handling live in Rust; this class is a thin engine host.
 */
class CefriumHost(private val activity: Activity) {

    companion object {
        /** The page the PoC boots to. Fast, and proves the pipeline end to end. */
        const val START_URL = "https://example.com"
    }

    private var browser: CefriumBrowser? = null

    /**
     * Creates the off-screen browser on the first surface size we see.
     * Page height = surface height minus the native chrome bar (60dp — the
     * same constant the Rust chrome layout uses).
     */
    fun ensureBrowser(width: Int, height: Int, density: Float) {
        if (browser != null) return
        val chrome = Math.round(60f * density)
        val pageW = width.coerceAtLeast(1)
        val pageH = (height - chrome).coerceAtLeast(1)

        val b = CefriumBrowser(pageW, pageH, density)
        // Default Android dialogs for <select>, date/time, color, JS alert/
        // confirm/prompt and the file chooser -- an off-screen browser has no
        // window of its own, so it needs one.
        b.setDialogActivity(activity)
        b.setRenderHandler(object : CefriumRenderHandler {
            override fun onPaint(width: Int, height: Int, pixels: ByteBuffer) {
                pixels.rewind()
                NativeLib.nativeOnFrame(width, height, pixels)
            }
        })
        browser = b
        b.loadUrl(START_URL)
    }

    fun loadUrl(url: String) {
        browser?.loadUrl(url)
    }

    fun goBack() = browser?.goBack()
    fun goForward() = browser?.goForward()
    fun reload() = browser?.reload()
    fun stopLoading() = browser?.stopLoading()

    fun setSize(width: Int, height: Int) {
        browser?.setSize(width, height)
    }

    // Touch forwarding: coordinates are already in *browser* pixels (the Rust
    // input state machine subtracts the chrome bar height before queueing).
    fun sendTouchDown(x: Float, y: Float) = browser?.sendTouchDown(x, y)
    fun sendTouchMove(x: Float, y: Float) = browser?.sendTouchMove(x, y)
    fun sendTouchUp(x: Float, y: Float) = browser?.sendTouchUp(x, y)

    fun setFocus(focus: Boolean) {
        browser?.setFocus(focus)
    }

    /** Pushed to Rust every choreographer tick (cheap getters, UI thread). */
    fun pollNavState() {
        val b = browser ?: return
        NativeLib.nativeNavState(b.url, b.progress, b.canGoBack(), b.canGoForward())
    }

    fun pause() {
        browser?.onPause()
    }

    fun resume() {
        browser?.onResume()
    }

    fun destroy() {
        browser?.close()
        browser = null
    }
}
