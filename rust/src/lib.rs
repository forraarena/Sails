//! Sails: native Rust + OpenGL ES compositor around the Cefrium engine.
//!
//! Split of responsibilities:
//!   - Kotlin (app/): owns the Cefrium SDK objects, forwards frames and
//!     touches in, drains the event queue every choreographer tick.
//!   - Rust (rust/, this crate): owns every visible pixel (EGL + GLES3 on an
//!     ANativeWindow), draws the browser chrome, and runs the input state
//!     machine. Zero external crates; raw FFI only.
//!
//! Threads:
//!   - UI thread: all JNI entry points below (called from Kotlin), plus the
//!     engine's onPaint callbacks.
//!   - "sails-render": EGL/GL loop (see renderer.rs).

mod chrome;
mod egl;
mod events;
mod font;
mod gl;
mod jni;
mod native_window;
mod renderer;
mod state;

use std::ffi::c_void;
use std::sync::{Arc, Mutex, OnceLock};

use events::Event;
use native_window::ANativeWindow;
use state::Shared;

// ---- logging ---------------------------------------------------------------

#[link(name = "log")]
extern "C" {
    fn __android_log_write(prio: i32, tag: *const i8, text: *const i8) -> i32;
}

const ANDROID_LOG_INFO: i32 = 4;
const ANDROID_LOG_ERROR: i32 = 6;

pub fn log_info(msg: &str) {
    log_write(ANDROID_LOG_INFO, msg);
}

pub fn log_error(msg: &str) {
    log_write(ANDROID_LOG_ERROR, msg);
}

fn log_write(prio: i32, msg: &str) {
    let tag = b"sails\0";
    let mut bytes = msg.as_bytes().to_vec();
    bytes.push(0);
    unsafe {
        __android_log_write(prio, tag.as_ptr() as *const i8, bytes.as_ptr() as *const i8);
    }
}

// ---- shared state -----------------------------------------------------------

static SHARED: OnceLock<Arc<Mutex<Shared>>> = OnceLock::new();
static RENDER_SPAWNED: OnceLock<()> = OnceLock::new();

fn shared() -> &'static Arc<Mutex<Shared>> {
    SHARED.get_or_init(|| Arc::new(Mutex::new(Shared::new())))
}

// ---- JNI entry points ---------------------------------------------------------

/// UI scale + one-time render-thread spawn.
#[no_mangle]
pub extern "system" fn Java_dev_sails_NativeLib_nativeConfig(
    _env: *mut c_void,
    _thiz: *mut c_void,
    density: f32,
) {
    if density > 0.1 {
        let mut st = shared().lock().unwrap();
        st.density = density;
        st.chrome_h = ((chrome::CHROME_DP * density).round() as i32).max(48);
    }
    if RENDER_SPAWNED.set(()).is_ok() {
        renderer::spawn(shared().clone());
        log_info("render thread spawned");
    }
}

/// One RGBA frame from Cefrium's onPaint (UI thread). The buffer is only
/// valid during this call, so we memcpy into staging before returning.
#[no_mangle]
pub extern "system" fn Java_dev_sails_NativeLib_nativeOnFrame(
    env: *mut c_void,
    _thiz: *mut c_void,
    width: i32,
    height: i32,
    pixels: *mut c_void,
) {
    if width <= 0 || height <= 0 || width > 8192 || height > 8192 {
        return;
    }
    let addr = unsafe { jni::get_direct_buffer_address(env, pixels) };
    if addr.is_null() {
        return;
    }
    let need = (width as usize) * (height as usize) * 4;
    let cap = unsafe { jni::get_direct_buffer_capacity(env, pixels) };
    if (cap as usize) < need {
        return;
    }
    let src = unsafe { std::slice::from_raw_parts(addr, need) };
    let mut st = shared().lock().unwrap();
    st.set_frame(width, height, src);
}

/// Raw touch from the SurfaceView (UI thread), surface pixels.
/// action: 0 down, 1 up, 2 move, 3 cancel (MotionEvent actionMasked).
#[no_mangle]
pub extern "system" fn Java_dev_sails_NativeLib_nativeTouch(
    _env: *mut c_void,
    _thiz: *mut c_void,
    action: i32,
    x: f32,
    y: f32,
) {
    let mut st = shared().lock().unwrap();
    let chrome_h = st.chrome_h as f32;
    let slop = st.tap_slop();
    let surface_w = st.surface_w.max(1);
    let in_chrome = y < chrome_h;
    let page_y = |py: f32| py - chrome_h;

    match action {
        0 => {
            // DOWN: start tracking; decide chrome vs page.
            let hit = if in_chrome {
                chrome::hit_at(surface_w, &st, x, y)
            } else {
                None
            };
            st.touch = state::TouchTracker {
                active: true,
                chrome: in_chrome,
                down_x: x,
                down_y: y,
                sent_down: false,
                hit,
                start_ms: now_ms(),
            };
            // A tap into the page while the omnibox is open closes it.
            if !in_chrome && st.omnibox_editing {
                st.omnibox_editing = false;
                st.push(Event::HideIme);
            }
        }
        2 => {
            if !st.touch.active || st.touch.chrome {
                return;
            }
            let moved = ((x - st.touch.down_x).powi(2) + (y - st.touch.down_y).powi(2)).sqrt();
            if !st.touch.sent_down {
                if moved > slop {
                    // It's a scroll: forward the full gesture from the start.
                    let (dx, dy) = (st.touch.down_x, st.touch.down_y);
                    st.push(Event::PageTouchDown(dx, page_y(dy)));
                    st.touch.sent_down = true;
                } else {
                    return;
                }
            }
            st.push(Event::PageTouchMove(x, page_y(y)));
        }
        1 => {
            if !st.touch.active {
                return;
            }
            let moved = ((x - st.touch.down_x).powi(2) + (y - st.touch.down_y).powi(2)).sqrt();
            if st.touch.chrome {
                // Button taps: release on the same control.
                let hit = st.touch.hit;
                if let Some(h) = hit {
                    if moved <= slop {
                        chrome::chrome_tap(&mut st, h);
                    }
                }
            } else if !st.touch.sent_down {
                // Clean tap on the page: synthesize down + up at the touch point.
                let (dx, dy) = (st.touch.down_x, st.touch.down_y);
                if now_ms().saturating_sub(st.touch.start_ms) < 800 && moved <= slop {
                    st.push(Event::PageTouchDown(dx, page_y(dy)));
                    st.push(Event::PageTouchUp(dx, page_y(dy)));
                }
            } else {
                st.push(Event::PageTouchUp(x, page_y(y)));
            }
            st.touch = state::TouchTracker::default();
        }
        3 => {
            // CANCEL: if we already started forwarding, lift the finger.
            if st.touch.active && st.touch.sent_down {
                st.push(Event::PageTouchUp(x, page_y(y)));
            }
            st.touch = state::TouchTracker::default();
        }
        _ => {}
    }
}

/// Engine state pushed by Kotlin every tick.
#[no_mangle]
pub extern "system" fn Java_dev_sails_NativeLib_nativeNavState(
    env: *mut c_void,
    _thiz: *mut c_void,
    url: *mut c_void,
    progress: i32,
    can_back: u8,
    can_fwd: u8,
) {
    let mut st = shared().lock().unwrap();
    if let Some(u) = unsafe { jni::read_string(env, url) } {
        st.url = u;
    }
    st.progress = progress.clamp(0, 100);
    st.can_back = can_back != 0;
    st.can_fwd = can_fwd != 0;
}

/// Omnibox text changed (hidden EditText).
#[no_mangle]
pub extern "system" fn Java_dev_sails_NativeLib_nativeImeText(
    env: *mut c_void,
    _thiz: *mut c_void,
    text: *mut c_void,
) {
    let mut st = shared().lock().unwrap();
    if let Some(s) = unsafe { jni::read_string(env, text) } {
        st.omnibox_text = s;
    }
}

/// Omnibox GO action: close the editor and navigate.
#[no_mangle]
pub extern "system" fn Java_dev_sails_NativeLib_nativeImeDone(
    _env: *mut c_void,
    _thiz: *mut c_void,
) {
    let mut st = shared().lock().unwrap();
    if !st.omnibox_editing {
        return;
    }
    let text = st.omnibox_text.trim().to_string();
    st.omnibox_editing = false;
    st.push(Event::HideIme);
    if !text.is_empty() {
        st.push(Event::LoadUrl(normalize_url(&text)));
    }
}

/// Activity Back: true = handled natively, false = background the app.
#[no_mangle]
pub extern "system" fn Java_dev_sails_NativeLib_nativeBack(
    _env: *mut c_void,
    _thiz: *mut c_void,
) -> u8 {
    let mut st = shared().lock().unwrap();
    if st.omnibox_editing {
        st.omnibox_editing = false;
        st.push(Event::HideIme);
        return 1;
    }
    if st.can_back {
        st.push(Event::GoBack);
        return 1;
    }
    0
}

// ---- surface lifecycle ---------------------------------------------------------

#[no_mangle]
pub extern "system" fn Java_dev_sails_NativeLib_nativeSurfaceCreated(
    env: *mut c_void,
    _thiz: *mut c_void,
    surface: *mut c_void,
) {
    let window = unsafe { ANativeWindow::from_surface(env, surface) };
    if window.is_null() {
        log_error("ANativeWindow_fromSurface returned null");
        return;
    }
    let mut st = shared().lock().unwrap();
    // If a previous window is still queued (never adopted), drop it first.
    if st.pending_window != 0 {
        unsafe { ANativeWindow::release(st.pending_window as *mut ANativeWindow) };
    }
    st.pending_window = window as usize;
    st.window_valid = true;
}

#[no_mangle]
pub extern "system" fn Java_dev_sails_NativeLib_nativeSurfaceDestroyed(
    _env: *mut c_void,
    _thiz: *mut c_void,
) {
    let mut st = shared().lock().unwrap();
    st.window_valid = false;
}

#[no_mangle]
pub extern "system" fn Java_dev_sails_NativeLib_nativePause(
    _env: *mut c_void,
    _thiz: *mut c_void,
) {
    shared().lock().unwrap().paused = true;
}

#[no_mangle]
pub extern "system" fn Java_dev_sails_NativeLib_nativeResume(
    _env: *mut c_void,
    _thiz: *mut c_void,
) {
    shared().lock().unwrap().paused = false;
}

/// Drains queued Rust->Java events into the provided direct ByteBuffer.
/// Returns the number of events written.
#[no_mangle]
pub extern "system" fn Java_dev_sails_NativeLib_nativePoll(
    env: *mut c_void,
    _thiz: *mut c_void,
    out: *mut c_void,
) -> i32 {
    let addr = unsafe { jni::get_direct_buffer_address(env, out) };
    if addr.is_null() {
        return 0;
    }
    let cap = unsafe { jni::get_direct_buffer_capacity(env, out) };
    if cap <= 4 {
        return 0;
    }
    let cap = (cap as usize).min(1 << 20);
    let mut st = shared().lock().unwrap();
    let mut buf: Vec<u8> = Vec::with_capacity(4096);
    let count = events::drain(&mut st.events, &mut buf, cap);
    if count > 0 {
        unsafe {
            std::ptr::copy_nonoverlapping(buf.as_ptr(), addr, buf.len());
        }
    }
    count as i32
}

// ---- helpers -----------------------------------------------------------------

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

fn normalize_url(input: &str) -> String {
    let t = input.trim();
    if t.is_empty() {
        return "about:blank".to_string();
    }
    let lower = t.to_ascii_lowercase();
    if lower.starts_with("http://")
        || lower.starts_with("https://")
        || lower.starts_with("file://")
        || lower.starts_with("about:")
        || lower.contains("://")
    {
        return t.to_string();
    }
    // No scheme: a dot implies a host, otherwise treat it as a search.
    if t.contains(' ') || !t.contains('.') {
        return format!("https://duckduckgo.com/?q={}", percent_encode(t));
    }
    format!("https://{}", t)
}

fn percent_encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len() * 2);
    for b in s.as_bytes() {
        let c = *b;
        if c.is_ascii_alphanumeric() || matches!(c, b'-' | b'.' | b'_' | b'~') {
            out.push(c as char);
        } else {
            out.push_str(&format!("%{:02X}", c));
        }
    }
    out
}
