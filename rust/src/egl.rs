//! Raw EGL 1.4 FFI (subset) -- no bindgen, no crates.
//!
//! Only what an on-screen GLES3 context on an ANativeWindow needs.

#![allow(non_camel_case_types, dead_code)]

use std::ffi::c_void;

pub type EGLDisplay = *mut c_void;
pub type EGLSurface = *mut c_void;
pub type EGLContext = *mut c_void;
pub type EGLConfig = *mut c_void;
pub type EGLBoolean = u32;
pub type EGLint = i32;

pub const EGL_FALSE: EGLBoolean = 0;
pub const EGL_TRUE: EGLBoolean = 1;

pub const EGL_NO_DISPLAY: EGLDisplay = std::ptr::null_mut();
pub const EGL_NO_SURFACE: EGLSurface = std::ptr::null_mut();
pub const EGL_NO_CONTEXT: EGLContext = std::ptr::null_mut();

// Attribute values
pub const EGL_NONE: EGLint = 0x3038;
pub const EGL_SURFACE_TYPE: EGLint = 0x3033;
pub const EGL_WINDOW_BIT: EGLint = 0x0004;
pub const EGL_RENDERABLE_TYPE: EGLint = 0x3040;
pub const EGL_OPENGL_ES3_BIT: EGLint = 0x0040;
pub const EGL_RED_SIZE: EGLint = 0x3024;
pub const EGL_GREEN_SIZE: EGLint = 0x3023;
pub const EGL_BLUE_SIZE: EGLint = 0x3022;
pub const EGL_ALPHA_SIZE: EGLint = 0x3021;
pub const EGL_DEPTH_SIZE: EGLint = 0x3025;
pub const EGL_CONTEXT_CLIENT_VERSION: EGLint = 0x3098;

// Surface queries
pub const EGL_WIDTH: EGLint = 0x3057;
pub const EGL_HEIGHT: EGLint = 0x3056;

pub const EGL_DEFAULT_DISPLAY: usize = 0;

#[link(name = "EGL")]
extern "C" {
    pub fn eglGetDisplay(display_id: usize) -> EGLDisplay;
    pub fn eglInitialize(
        display: EGLDisplay,
        major: *mut EGLint,
        minor: *mut EGLint,
    ) -> EGLBoolean;
    pub fn eglTerminate(display: EGLDisplay) -> EGLBoolean;
    pub fn eglChooseConfig(
        display: EGLDisplay,
        attrib_list: *const EGLint,
        configs: *mut EGLConfig,
        config_size: EGLint,
        num_config: *mut EGLint,
    ) -> EGLBoolean;
    pub fn eglCreateWindowSurface(
        display: EGLDisplay,
        config: EGLConfig,
        native_window: *mut c_void,
        attrib_list: *const EGLint,
    ) -> EGLSurface;
    pub fn eglDestroySurface(display: EGLDisplay, surface: EGLSurface) -> EGLBoolean;
    pub fn eglCreateContext(
        display: EGLDisplay,
        config: EGLConfig,
        share_context: EGLContext,
        attrib_list: *const EGLint,
    ) -> EGLContext;
    pub fn eglDestroyContext(display: EGLDisplay, context: EGLContext) -> EGLBoolean;
    pub fn eglMakeCurrent(
        display: EGLDisplay,
        draw: EGLSurface,
        read: EGLSurface,
        context: EGLContext,
    ) -> EGLBoolean;
    pub fn eglSwapBuffers(display: EGLDisplay, surface: EGLSurface) -> EGLBoolean;
    pub fn eglSwapInterval(display: EGLDisplay, interval: EGLint) -> EGLBoolean;
    pub fn eglQuerySurface(
        display: EGLDisplay,
        surface: EGLSurface,
        attribute: EGLint,
        value: *mut EGLint,
    ) -> EGLBoolean;
    pub fn eglGetError() -> EGLint;
}

/// Picks an RGBA8888 / GLES3 window config and creates a context.
/// Returns None (with an error log) if EGL setup fails.
pub unsafe fn create_display_and_context(
) -> Option<(EGLDisplay, EGLConfig, EGLContext)> {
    let display = eglGetDisplay(EGL_DEFAULT_DISPLAY);
    if display.is_null() {
        return None;
    }
    let (mut major, mut minor): (EGLint, EGLint) = (0, 0);
    if eglInitialize(display, &mut major, &mut minor) == EGL_FALSE {
        crate::log_error(&format!(
            "eglInitialize failed, eglGetError={:#x}",
            eglGetError()
        ));
        return None;
    }

    let attribs: [EGLint; 11] = [
        EGL_SURFACE_TYPE,
        EGL_WINDOW_BIT,
        EGL_RENDERABLE_TYPE,
        EGL_OPENGL_ES3_BIT,
        EGL_RED_SIZE,
        8,
        EGL_GREEN_SIZE,
        8,
        EGL_BLUE_SIZE,
        8,
        EGL_NONE,
    ];
    let mut config: EGLConfig = std::ptr::null_mut();
    let mut num_configs: EGLint = 0;
    if eglChooseConfig(display, attribs.as_ptr(), &mut config, 1, &mut num_configs) == EGL_FALSE
        || num_configs < 1
    {
        crate::log_error(&format!(
            "eglChooseConfig failed, eglGetError={:#x}",
            eglGetError()
        ));
        return None;
    }

    let ctx_attribs: [EGLint; 3] = [EGL_CONTEXT_CLIENT_VERSION, 3, EGL_NONE];
    let context = eglCreateContext(display, config, EGL_NO_CONTEXT, ctx_attribs.as_ptr());
    if context.is_null() {
        crate::log_error(&format!(
            "eglCreateContext failed, eglGetError={:#x}",
            eglGetError()
        ));
        return None;
    }

    Some((display, config, context))
}

pub unsafe fn surface_size(display: EGLDisplay, surface: EGLSurface) -> Option<(i32, i32)> {
    let (mut w, mut h): (EGLint, EGLint) = (0, 0);
    if eglQuerySurface(display, surface, EGL_WIDTH, &mut w) == EGL_FALSE
        || eglQuerySurface(display, surface, EGL_HEIGHT, &mut h) == EGL_FALSE
    {
        return None;
    }
    Some((w, h))
}
