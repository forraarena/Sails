//! Shared state between the JNI entry points (UI thread) and the render
//! thread. Single mutex; every critical section is a memcpy or a few fields.

use std::collections::VecDeque;

use crate::events::Event;

/// How long (ms) a page tap may travel before it stops being a tap.
const TAP_SLOP_DP: f32 = 8.0;

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum ChromeHit {
    Back,
    Forward,
    Reload,
    Omnibox,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct TouchTracker {
    /// Gesture in progress (finger down seen, not yet up).
    pub active: bool,
    /// The gesture began in the chrome bar (vs the page).
    pub chrome: bool,
    pub down_x: f32,
    pub down_y: f32,
    /// We already forwarded a finger-down to the engine (scrolling).
    pub sent_down: bool,
    /// Which chrome control the gesture started on.
    pub hit: Option<ChromeHit>,
    pub start_ms: u64,
}

pub struct Shared {
    // --- UI scale ------------------------------------------------------
    pub density: f32,
    pub chrome_h: i32,

    // --- Frame staging (latest Cefrium onPaint frame) -------------------
    pub frame: Vec<u8>,
    pub frame_w: i32,
    pub frame_h: i32,
    pub frame_dirty: bool,
    pub frame_valid: bool,

    // --- Engine state pushed from Kotlin every tick ----------------------
    pub url: String,
    pub progress: i32,
    pub can_back: bool,
    pub can_fwd: bool,

    // --- Native chrome state -------------------------------------------
    pub omnibox_editing: bool,
    pub omnibox_text: String,

    // --- Rust -> Java events --------------------------------------------
    pub events: VecDeque<Event>,

    // --- Input ------------------------------------------------------------
    pub touch: TouchTracker,

    // --- Surface plumbing ---------------------------------------------------
    /// ANativeWindow* acquired in nativeSurfaceCreated, waiting for the
    /// render thread to adopt it (raw pointer as usize; render thread owns it).
    pub pending_window: usize,
    /// false after nativeSurfaceDestroyed: render thread must tear down EGL.
    pub window_valid: bool,
    pub paused: bool,

    // --- Surface size the render thread last saw ---------------------------
    pub surface_w: i32,
    pub surface_h: i32,
}

impl Shared {
    pub fn new() -> Self {
        Shared {
            density: 2.75,
            chrome_h: 165,
            frame: Vec::new(),
            frame_w: 0,
            frame_h: 0,
            frame_dirty: false,
            frame_valid: false,
            url: String::new(),
            progress: 100,
            can_back: false,
            can_fwd: false,
            omnibox_editing: false,
            omnibox_text: String::new(),
            events: VecDeque::new(),
            touch: TouchTracker::default(),
            pending_window: 0,
            window_valid: false,
            paused: false,
            surface_w: 0,
            surface_h: 0,
        }
    }

    /// Copies one onPaint frame into staging (called on the UI thread, inside
    /// the Cefrium render callback -- the buffer is only valid during the call).
    pub fn set_frame(&mut self, w: i32, h: i32, src: &[u8]) {
        let need = (w as usize) * (h as usize) * 4;
        if self.frame.len() != need {
            self.frame = vec![0u8; need];
        }
        self.frame.copy_from_slice(src);
        self.frame_w = w;
        self.frame_h = h;
        self.frame_dirty = true;
        self.frame_valid = true;
    }

    pub fn push(&mut self, event: Event) {
        self.events.push_back(event);
    }

    pub fn tap_slop(&self) -> f32 {
        TAP_SLOP_DP * self.density
    }

    pub fn loading(&self) -> bool {
        self.progress < 100
    }
}

/// Borrowed, lock-free view of the state for the chrome drawer, taken from
/// the render thread after copying the string fields.
pub struct SharedView<'a> {
    pub chrome_h: i32,
    pub density: f32,
    pub url: &'a str,
    pub omnibox_text: &'a str,
    pub omnibox_editing: bool,
    pub progress: i32,
    pub can_back: bool,
    pub can_fwd: bool,
    pub frame_valid: bool,
}

impl<'a> From<&'a Shared> for SharedView<'a> {
    fn from(s: &'a Shared) -> Self {
        SharedView {
            chrome_h: s.chrome_h,
            density: s.density,
            url: &s.url,
            omnibox_text: &s.omnibox_text,
            omnibox_editing: s.omnibox_editing,
            progress: s.progress,
            can_back: s.can_back,
            can_fwd: s.can_fwd,
            frame_valid: s.frame_valid,
        }
    }
}

impl<'a> SharedView<'a> {
    /// The engine reports 100 when nothing is loading (CefriumBrowser.getProgress).
    pub fn loading(&self) -> bool {
        self.progress < 100
    }
}
