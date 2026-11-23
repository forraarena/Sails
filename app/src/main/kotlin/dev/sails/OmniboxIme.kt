package dev.sails

import android.app.Activity
import android.content.Context
import android.text.Editable
import android.text.InputType
import android.text.TextWatcher
import android.view.inputmethod.EditorInfo
import android.view.inputmethod.InputMethodManager
import android.widget.EditText
import android.widget.FrameLayout

/** Show/hide the soft keyboard on behalf of the native Rust omnibox. */
interface ImeController {
    fun show(text: String)
    fun hide()
}

/** A 1x1 invisible EditText that owns the soft keyboard while the *native Rust*
 * omnibox is being edited.
 *
 * Flow: Rust decides the omnibox was tapped and queues SHOW_IME -> Kotlin
 * focuses this EditText and shows the IME -> every text change goes back to
 * Rust via [NativeLib.nativeImeText] -> the GO action (or Rust closing the
 * omnibox) commits navigation via [NativeLib.nativeImeDone] / HIDE_IME.
 *
 * (Forwarding IME text *into the page* — Cefrium's imeCommitText /
 * imeSetComposition — is a next step, see README.)
 */
class OmniboxIme(private val activity: Activity) : ImeController {

    private val edit: EditText

    init {
        edit = EditText(activity)
        edit.layoutParams = FrameLayout.LayoutParams(1, 1)
        edit.alpha = 0f
        edit.inputType = InputType.TYPE_CLASS_TEXT or InputType.TYPE_TEXT_VARIATION_URI
        edit.imeOptions = EditorInfo.IME_ACTION_GO or EditorInfo.IME_FLAG_NO_EXTRACT_UI
        edit.addTextChangedListener(object : TextWatcher {
            override fun afterTextChanged(s: Editable?) {
                NativeLib.nativeImeText(s?.toString())
            }

            override fun beforeTextChanged(s: CharSequence?, start: Int, count: Int, after: Int) {}
            override fun onTextChanged(s: CharSequence?, start: Int, before: Int, count: Int) {}
        })
        edit.setOnEditorActionListener { _, actionId, _ ->
            if (actionId == EditorInfo.IME_ACTION_GO) {
                NativeLib.nativeImeDone()
                true
            } else {
                false
            }
        }
        activity.addContentView(edit, FrameLayout.LayoutParams(1, 1))
    }

    override fun show(text: String) {
        if (edit.text.toString() != text) edit.setText(text)
        edit.requestFocus()
        val imm = activity.getSystemService(Context.INPUT_METHOD_SERVICE) as InputMethodManager
        imm.showSoftInput(edit, InputMethodManager.SHOW_IMPLICIT)
    }

    override fun hide() {
        val imm = activity.getSystemService(Context.INPUT_METHOD_SERVICE) as InputMethodManager
        imm.hideSoftInputFromWindow(edit.windowToken, 0)
        edit.clearFocus()
    }

    companion object {
        /** Used before MainActivity installs the real controller; does nothing. */
        val NO_OP: ImeController = object : ImeController {
            override fun show(text: String) {}
            override fun hide() {}
        }
    }
}
