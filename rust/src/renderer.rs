//! The render thread: owns EGL + the GL context, uploads engine frames as a
//! texture, and composites page + native chrome every vsync.
//!
//! Loop shape: eglSwapBuffers blocks on vsync (swap interval 1), so the thread
//! paces itself; when paused or without a surface it idles on a short sleep.

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use crate::chrome::{self, Geom};
use crate::egl;
use crate::events::Event;
use crate::font;
use crate::gl;
use crate::native_window::ANativeWindow;
use crate::state::{Shared, SharedView};

pub fn spawn(shared: Arc<Mutex<Shared>>) {
    std::thread::Builder::new()
        .name("sails-render".to_string())
        .spawn(move || {
            run(shared);
        })
        .ok();
}

struct Renderer {
    display: egl::EGLDisplay,
    config: egl::EGLConfig,
    context: egl::EGLContext,
    surface: Option<egl::EGLSurface>,
    /// ANativeWindow* currently wrapped by `surface` (render thread owns the
    /// reference; 0 when none).
    window: usize,
    width: i32,
    height: i32,
    page_tex: u32,
    page_w: i32,
    page_h: i32,
    atlas_tex: u32,
    program: u32,
    u_resolution: i32,
    u_tex: i32,
    u_use_tex: i32,
    vbo_solid: u32,
    vbo_tex: u32,
    vbo_glyph: u32,
}

impl Renderer {
    unsafe fn new() -> Option<Renderer> {
        let (display, config, context) = egl::create_display_and_context()?;

        gl::glEnable(gl::GL_BLEND);
        gl::glBlendFunc(gl::GL_SRC_ALPHA, gl::GL_ONE_MINUS_SRC_ALPHA);
        gl::glDisable(gl::GL_DEPTH_TEST);
        gl::glPixelStorei(gl::GL_UNPACK_ALIGNMENT, 1);

        // Page texture (allocated on first frame).
        let mut page_tex: gl::GLuint = 0;
        gl::glGenTextures(1, &mut page_tex);
        gl::glBindTexture(gl::GL_TEXTURE_2D, page_tex);
        set_tex_params(gl::GL_LINEAR);
        gl::glActiveTexture(gl::GL_TEXTURE0);
        gl::glBindTexture(gl::GL_TEXTURE_2D, page_tex);

        // Font atlas (nearest -- hard pixel edges).
        let mut atlas_tex: gl::GLuint = 0;
        gl::glGenTextures(1, &mut atlas_tex);
        gl::glActiveTexture(gl::GL_TEXTURE1);
        gl::glBindTexture(gl::GL_TEXTURE_2D, atlas_tex);
        gl::glTexImage2D(
            gl::GL_TEXTURE_2D,
            0,
            gl::GL_RGBA as i32,
            font::FONT_ATLAS_W as i32,
            font::FONT_ATLAS_H as i32,
            0,
            gl::GL_RGBA,
            gl::GL_UNSIGNED_BYTE,
            font::FONT_ATLAS_RGBA.as_ptr() as *const std::ffi::c_void,
        );
        set_tex_params(gl::GL_NEAREST);
        gl::glActiveTexture(gl::GL_TEXTURE0);
        gl::glBindTexture(gl::GL_TEXTURE_2D, page_tex);

        let program = gl::build_program(gl::VERT_SRC, gl::FRAG_SRC)?;
        let name = |n: &str| -> i32 {
            let c = std::ffi::CString::new(n).unwrap();
            gl::glGetUniformLocation(program, c.as_ptr())
        };
        let (u_resolution, u_tex, u_use_tex) = (name("uResolution"), name("uTex"), name("uUseTex"));

        let mut vbo: [gl::GLuint; 3] = [0, 0, 0];
        gl::glGenBuffers(3, vbo.as_mut_ptr());

        gl::glUseProgram(program);
        gl::glUniform1i(u_tex, 0);

        Some(Renderer {
            display,
            config,
            context,
            surface: None,
            window: 0,
            width: 0,
            height: 0,
            page_tex,
            page_w: 0,
            page_h: 0,
            atlas_tex,
            program,
            u_resolution,
            u_tex,
            u_use_tex,
            vbo_solid: vbo[0],
            vbo_tex: vbo[1],
            vbo_glyph: vbo[2],
        })
    }

    /// Adopts a newly acquired ANativeWindow: tears down any previous surface,
    /// creates an EGL window surface and makes the context current on it.
    /// Returns the surface size so the caller can record it.
    unsafe fn adopt_window(&mut self, window: usize, shared: &Arc<Mutex<Shared>>) -> bool {
        self.teardown_surface();

        let win = window as *mut ANativeWindow as *mut std::ffi::c_void;
        let surface = egl::eglCreateWindowSurface(self.display, self.config, win, std::ptr::null());
        if surface.is_null() {
            crate::log_error(&format!(
                "eglCreateWindowSurface failed, eglGetError={:#x}",
                egl::eglGetError()
            ));
            return false;
        }
        if egl::eglMakeCurrent(self.display, surface, surface, self.context) == egl::EGL_FALSE {
            crate::log_error(&format!(
                "eglMakeCurrent failed, eglGetError={:#x}",
                egl::eglGetError()
            ));
            egl::eglDestroySurface(self.display, surface);
            return false;
        }
        egl::eglSwapInterval(self.display, 1);
        self.surface = Some(surface);
        self.window = window;
        let (w, h) = egl::surface_size(self.display, surface).unwrap_or((0, 0));
        self.width = w;
        self.height = h;
        gl::glViewport(0, 0, w, h);
        record_surface_size(shared, w, h);
        true
    }

    /// Releases the EGL surface and the ANativeWindow reference.
    unsafe fn teardown_surface(&mut self) {
        if let Some(surface) = self.surface.take() {
            egl::eglMakeCurrent(
                self.display,
                egl::EGL_NO_SURFACE,
                egl::EGL_NO_SURFACE,
                egl::EGL_NO_CONTEXT,
            );
            egl::eglDestroySurface(self.display, surface);
        }
        if self.window != 0 {
            ANativeWindow_release(self.window as *mut ANativeWindow);
            self.window = 0;
            self.width = 0;
            self.height = 0;
        }
    }

    unsafe fn upload_page_frame(&mut self, w: i32, h: i32, pixels: &[u8]) {
        if self.page_w != w || self.page_h != h {
            self.page_w = w;
            self.page_h = h;
            gl::glActiveTexture(gl::GL_TEXTURE0);
            gl::glBindTexture(gl::GL_TEXTURE_2D, self.page_tex);
            gl::glTexImage2D(
                gl::GL_TEXTURE_2D,
                0,
                gl::GL_RGBA as i32,
                w,
                h,
                0,
                gl::GL_RGBA,
                gl::GL_UNSIGNED_BYTE,
                std::ptr::null(),
            );
        }
        gl::glActiveTexture(gl::GL_TEXTURE0);
        gl::glBindTexture(gl::GL_TEXTURE_2D, self.page_tex);
        gl::glTexSubImage2D(
            gl::GL_TEXTURE_2D,
            0,
            0,
            0,
            w,
            h,
            gl::GL_RGBA,
            gl::GL_UNSIGNED_BYTE,
            pixels.as_ptr() as *const std::ffi::c_void,
        );
    }

    /// One pass: bind a VBO, stream the vertices, set attribs, draw.
    unsafe fn draw_geom(&mut self, vbo: gl::GLuint, data: &[f32], textured: bool, tex_unit: i32) {
        if data.is_empty() {
            return;
        }
        gl::glBindBuffer(gl::GL_ARRAY_BUFFER, vbo);
        gl::glBufferData(
            gl::GL_ARRAY_BUFFER,
            (data.len() * std::mem::size_of::<f32>()) as isize,
            data.as_ptr() as *const std::ffi::c_void,
            gl::GL_STREAM_DRAW,
        );
        let stride = 8 * std::mem::size_of::<f32>() as i32;
        gl::glVertexAttribPointer(0, 2, gl::GL_FLOAT, gl::GL_FALSE as u8, stride, std::ptr::null());
        gl::glVertexAttribPointer(
            1,
            4,
            gl::GL_FLOAT,
            gl::GL_FALSE as u8,
            stride,
            (8 * std::mem::size_of::<f32>()) as *const std::ffi::c_void,
        );
        gl::glVertexAttribPointer(2, 2, gl::GL_FLOAT, gl::GL_FALSE as u8, stride, 24 as *const std::ffi::c_void);
        gl::glEnableVertexAttribArray(0);
        gl::glEnableVertexAttribArray(1);
        gl::glEnableVertexAttribArray(2);
        gl::glUniform1f(self.u_use_tex, if textured { 1.0 } else { 0.0 });
        gl::glUniform1i(self.u_tex, tex_unit);
        gl::glDrawArrays(gl::GL_TRIANGLES, 0, (data.len() / 8) as i32);
    }

    unsafe fn render_pass(&mut self, solid: &Geom, page: &Geom, glyph: &Geom) -> bool {
        gl::glClearColor(0.0, 0.0, 0.0, 1.0);
        gl::glClear(gl::GL_COLOR_BUFFER_BIT);
        gl::glUniform2f(self.u_resolution, self.width as f32, self.height as f32);

        self.draw_geom(self.vbo_solid, &solid.data, false, 0);
        self.draw_geom(self.vbo_tex, &page.data, true, 0);
        self.draw_geom(self.vbo_glyph, &glyph.data, true, 1);

        let surface = match self.surface {
            Some(s) => s,
            None => return false,
        };
        egl::eglSwapBuffers(self.display, surface) == egl::EGL_TRUE
    }
}

unsafe fn set_tex_params(filter: gl::GLenum) {
    gl::glTexParameteri(gl::GL_TEXTURE_2D, gl::GL_TEXTURE_MIN_FILTER, filter as i32);
    gl::glTexParameteri(gl::GL_TEXTURE_2D, gl::GL_TEXTURE_MAG_FILTER, filter as i32);
    gl::glTexParameteri(gl::GL_TEXTURE_2D, gl::GL_TEXTURE_WRAP_S, gl::GL_CLAMP_TO_EDGE as i32);
    gl::glTexParameteri(gl::GL_TEXTURE_2D, gl::GL_TEXTURE_WRAP_T, gl::GL_CLAMP_TO_EDGE as i32);
}

/// Records the surface size into shared state so the input hit-testing sees
/// the same geometry the chrome was drawn with.
fn record_surface_size(shared: &Arc<Mutex<Shared>>, w: i32, h: i32) {
    if let Ok(mut st) = shared.lock() {
        if st.surface_w != w || st.surface_h != h {
            st.surface_w = w;
            st.surface_h = h;
        }
    }
}

fn run(shared: Arc<Mutex<Shared>>) {
    unsafe {
        let mut renderer = match Renderer::new() {
            Some(r) => r,
            None => {
                crate::log_error("EGL/GL init failed; render thread exiting");
                return;
            }
        };
        let mut geom_solid = Geom::new();
        let mut geom_tex = Geom::new();
        let mut geom_glyph = Geom::new();
        let start = Instant::now();

        loop {
            // Pause: park without touching EGL.
            let paused = shared.lock().map(|s| s.paused).unwrap_or(false);
            if paused {
                std::thread::sleep(Duration::from_millis(80));
                continue;
            }

            // Adopt a new window if the UI thread acquired one.
            let pending = shared.lock().map(|s| s.pending_window).unwrap_or(0);
            if pending != 0 && pending != renderer.window {
                if renderer.adopt_window(pending, &shared) {
                    shared.lock().map(|mut s| s.pending_window = 0).ok();
                } else {
                    std::thread::sleep(Duration::from_millis(50));
                }
            }

            // Surface gone (surfaceDestroyed): tear down and wait.
            let valid = shared.lock().map(|s| s.window_valid).unwrap_or(false);
            if !valid && renderer.surface.is_some() {
                renderer.teardown_surface();
            }
            if renderer.surface.is_none() {
                std::thread::sleep(Duration::from_millis(30));
                continue;
            }

            // Surface resize -> re-viewport + tell the engine its page size.
            if let Some(surface) = renderer.surface {
                if let Some((w, h)) = egl::surface_size(renderer.display, surface) {
                    if w != renderer.width || h != renderer.height {
                        renderer.width = w;
                        renderer.height = h;
                        gl::glViewport(0, 0, w, h);
                        record_surface_size(&shared, w, h);
                        if let Ok(mut st) = shared.lock() {
                            st.push(Event::ResizeBrowser(w, (h - st.chrome_h).max(1)));
                        }
                    }
                }
            }

            // Snapshot the state + take the latest frame if fresh.
            let (frame, snapshot) = {
                let mut st = match shared.lock() {
                    Ok(s) => s,
                    Err(_) => {
                        std::thread::sleep(Duration::from_millis(100));
                        continue;
                    }
                };
                let frame = if st.frame_dirty && st.frame_valid {
                    st.frame_dirty = false;
                    Some((st.frame_w, st.frame_h, st.frame.clone()))
                } else {
                    None
                };
                let view = SharedView::from(&*st);
                // SharedView borrows `st`; copy the borrowed strings into
                // owned ones for the draw call below.
                let owned = (
                    view.chrome_h,
                    view.density,
                    view.url.to_string(),
                    view.omnibox_text.to_string(),
                    view.omnibox_editing,
                    view.progress,
                    view.can_back,
                    view.can_fwd,
                    view.frame_valid,
                );
                (frame, owned)
            };
            let (
                chrome_h,
                density,
                url,
                omnibox_text,
                omnibox_editing,
                progress,
                can_back,
                can_fwd,
                frame_valid,
            ) = snapshot;

            if let Some((fw, fh, px)) = &frame {
                renderer.upload_page_frame(*fw, *fh, px);
            }

            let w = renderer.width;
            let h = renderer.height;
            if w == 0 || h == 0 {
                continue;
            }

            // Build geometry.
            geom_solid.data.clear();
            geom_tex.data.clear();
            geom_glyph.data.clear();

            // Page quad (UV 0,0 at top-left: row 0 of the uploaded frame is
            // the image top, which lands at v=0 in texture space).
            if frame_valid {
                let page_top = chrome_h as f32;
                geom_tex.quad_uv(
                    0.0,
                    page_top,
                    w as f32,
                    h as f32 - page_top,
                    [1.0, 1.0, 1.0, 1.0],
                    [0.0, 0.0, 1.0, 1.0],
                );
            }

            let view = SharedView {
                chrome_h,
                density,
                url: &url,
                omnibox_text: &omnibox_text,
                omnibox_editing,
                progress,
                can_back,
                can_fwd,
                frame_valid,
            };
            chrome::draw_chrome(
                &mut geom_solid,
                &mut geom_glyph,
                w,
                h,
                &view,
                start.elapsed().as_millis() as u64,
            );

            let ok = renderer.render_pass(&geom_solid, &geom_tex, &geom_glyph);
            if !ok {
                crate::log_error("eglSwapBuffers failed; tearing down surface");
                renderer.teardown_surface();
            }
        }
    }
}
