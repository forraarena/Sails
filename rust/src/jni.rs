//! Minimal JNI access, by vtable index.
//!
//! The Rust core avoids depending on the `jni` crate (zero dependencies), so
//! we index into the `JNINativeInterface` vtable directly. The table's layout
//! is the stable JNI ABI; only five entries are used, and every entry is
//! sanity-checked against `GetVersion` on first use so a layout mistake fails
//! loudly instead of corrupting memory.
//!
//! Used indices (canonical jni.h order):
//!   4   GetVersion
//!   17  ExceptionClear
//!   169 GetStringUTFChars
//!   170 ReleaseStringUTFChars
//!   228 ExceptionCheck
//!   230 GetDirectBufferAddress
//!   231 GetDirectBufferCapacity

use std::ffi::{c_char, c_void, CStr};

const JNI_VERSION_1_6: i32 = 0x00010006;

/// JNIEnv* -> pointer to the vtable (array of function pointers).
unsafe fn vtable(env: *mut c_void) -> *const *const c_void {
    debug_assert!(!env.is_null(), "null JNIEnv");
    *(env as *const *const *const c_void)
}

unsafe fn fn_ptr(env: *mut c_void, index: usize) -> *const c_void {
    *vtable(env).add(index)
}

/// Calls GetVersion once and caches the result; returns true when the runtime
/// reports JNI 1.6 (the version every Android runtime guarantees).
pub unsafe fn verify_runtime(env: *mut c_void) -> bool {
    use std::sync::atomic::{AtomicBool, Ordering};
    static CHECKED: AtomicBool = AtomicBool::new(false);
    static OK: AtomicBool = AtomicBool::new(false);

    if CHECKED.load(Ordering::Relaxed) {
        return OK.load(Ordering::Relaxed);
    }
    let f: unsafe extern "system" fn(*mut c_void) -> i32 = std::mem::transmute(fn_ptr(env, 4));
    let version = f(env);
    let ok = version == JNI_VERSION_1_6;
    if !ok {
        crate::log_error(&format!(
            "JNI vtable mismatch: GetVersion returned {version:#x}, expected {JNI_VERSION_1_6:#x}"
        ));
    }
    OK.store(ok, Ordering::Relaxed);
    CHECKED.store(true, Ordering::Relaxed);
    ok
}

/// Returns the memory address of a direct ByteBuffer (null if not direct).
pub unsafe fn get_direct_buffer_address(env: *mut c_void, buf: *mut c_void) -> *mut u8 {
    if buf.is_null() || !verify_runtime(env) {
        return std::ptr::null_mut();
    }
    let f: unsafe extern "system" fn(*mut c_void, *mut c_void) -> *mut c_void =
        std::mem::transmute(fn_ptr(env, 230));
    f(env, buf) as *mut u8
}

/// Returns the capacity in bytes of a direct ByteBuffer (0 on error).
pub unsafe fn get_direct_buffer_capacity(env: *mut c_void, buf: *mut c_void) -> i64 {
    if buf.is_null() || !verify_runtime(env) {
        return 0;
    }
    let f: unsafe extern "system" fn(*mut c_void, *mut c_void) -> i64 =
        std::mem::transmute(fn_ptr(env, 231));
    f(env, buf)
}

pub unsafe fn exception_check(env: *mut c_void) -> bool {
    let f: unsafe extern "system" fn(*mut c_void) -> u8 = std::mem::transmute(fn_ptr(env, 228));
    f(env) != 0
}

pub unsafe fn exception_clear(env: *mut c_void) {
    let f: unsafe extern "system" fn(*mut c_void) = std::mem::transmute(fn_ptr(env, 17));
    f(env)
}

/// Copies a jstring into a Rust String (null-safe). Note: GetStringUTFChars
/// yields *modified* UTF-8; identical to UTF-8 for the URLs and omnibox text
/// this PoC round-trips (ASCII / BMP without embedded NUL).
pub unsafe fn read_string(env: *mut c_void, s: *mut c_void) -> Option<String> {
    if s.is_null() || !verify_runtime(env) {
        return None;
    }
    let f: unsafe extern "system" fn(*mut c_void, *mut c_void) -> *const c_char =
        std::mem::transmute(fn_ptr(env, 169));
    let chars = f(env, s);
    if chars.is_null() {
        if exception_check(env) {
            exception_clear(env);
        }
        return None;
    }
    let out = CStr::from_ptr(chars).to_string_lossy().into_owned();
    let r: unsafe extern "system" fn(*mut c_void, *mut c_void, *const c_char) =
        std::mem::transmute(fn_ptr(env, 170));
    r(env, s, chars);
    Some(out)
}
