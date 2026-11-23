//! libandroid ANativeWindow FFI (subset).

#![allow(dead_code)]

use std::ffi::c_void;

/// Opaque ANativeWindow handle (repr is private to libandroid).
#[repr(C)]
pub struct ANativeWindow {
    _private: [u8; 0],
}

#[link(name = "android")]
extern "C" {
    fn ANativeWindow_fromSurface(env: *mut c_void, surface: *mut c_void) -> *mut ANativeWindow;
    fn ANativeWindow_release(window: *mut ANativeWindow);
    fn ANativeWindow_getWidth(window: *mut ANativeWindow) -> i32;
    fn ANativeWindow_getHeight(window: *mut ANativeWindow) -> i32;
}

impl ANativeWindow {
    /// Acquires a reference to the ANativeWindow backing a java Surface.
    pub unsafe fn from_surface(env: *mut c_void, surface: *mut c_void) -> *mut ANativeWindow {
        ANativeWindow_fromSurface(env, surface)
    }

    pub unsafe fn release(window: *mut ANativeWindow) {
        ANativeWindow_release(window)
    }

    pub unsafe fn width(window: *mut ANativeWindow) -> i32 {
        ANativeWindow_getWidth(window)
    }

    pub unsafe fn height(window: *mut ANativeWindow) -> i32 {
        ANativeWindow_getHeight(window)
    }
}
