package dev.sails

import android.os.Bundle
import androidx.appcompat.app.AppCompatActivity
import androidx.activity.OnBackPressedCallback

/**
 * Hosts the single [SailsSurfaceView]. The activity itself stays out of the
 * UI: every pixel is drawn by the Rust compositor; the engine lives in the
 * [CefriumHost].
 */
class MainActivity : AppCompatActivity() {

    private lateinit var host: CefriumHost
    private lateinit var sailsView: SailsSurfaceView
    private lateinit var ime: OmniboxIme

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)

        host = CefriumHost(this)
        sailsView = SailsSurfaceView(this, host)
        setContentView(sailsView)

        ime = OmniboxIme(this)
        sailsView.attachIme(ime)

        // Rust decides what Back means: close the omnibox, go back in page
        // history, or let the activity background itself.
        onBackPressedDispatcher.addCallback(this, object : OnBackPressedCallback(true) {
            override fun handleOnBackPressed() {
                if (!NativeLib.nativeBack()) {
                    moveTaskToBack(true)
                }
            }
        })

        // One-time: gives Rust the UI scale and spawns its render thread.
        NativeLib.nativeConfig(resources.displayMetrics.density)
    }

    override fun onResume() {
        super.onResume()
        NativeLib.nativeResume()
        host.resume()
        sailsView.onActivityResumed()
    }

    override fun onPause() {
        super.onPause()
        NativeLib.nativePause()
        host.pause()
        sailsView.onActivityPaused()
    }

    override fun onDestroy() {
        host.destroy()
        super.onDestroy()
    }
}
