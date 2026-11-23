//! Rust -> Java event queue.
//!
//! The Rust core never calls up into Java. Actions that must run on the
//! engine/UI thread are queued here; the Kotlin choreographer loop drains the
//! queue every frame via `NativeLib.nativePoll(ByteBuffer)` and executes them.
//!
//! Wire format (little-endian), written by [`Event::serialize`]:
//! `[kind:i32][a:i32][b:i32][x:f32][y:f32][str_len:i32][str_len bytes of UTF-8]`
//! -- a 24-byte header, matching the Kotlin parser field for field.

use std::collections::VecDeque;

pub const KIND_LOAD_URL: i32 = 1;
pub const KIND_GO_BACK: i32 = 2;
pub const KIND_GO_FORWARD: i32 = 3;
pub const KIND_RELOAD: i32 = 4;
pub const KIND_STOP: i32 = 5;
pub const KIND_PAGE_TOUCH_DOWN: i32 = 6;
pub const KIND_PAGE_TOUCH_MOVE: i32 = 7;
pub const KIND_PAGE_TOUCH_UP: i32 = 8;
pub const KIND_SHOW_IME: i32 = 9;
pub const KIND_HIDE_IME: i32 = 10;
pub const KIND_SET_PAGE_FOCUS: i32 = 11;
pub const KIND_RESIZE_BROWSER: i32 = 12;

#[derive(Debug, Clone)]
#[allow(dead_code)] // SetPageFocus is wired when in-page keyboard focus lands
pub enum Event {
    /// Navigate the engine (already normalized by the omnibox).
    LoadUrl(String),
    GoBack,
    GoForward,
    Reload,
    Stop,
    /// Forwarded finger events, in *browser* pixels (chrome bar removed).
    PageTouchDown(f32, f32),
    PageTouchMove(f32, f32),
    PageTouchUp(f32, f32),
    /// Open the soft keyboard with `String` as the omnibox content.
    ShowIme(String),
    HideIme,
    /// Give/take keyboard focus on the engine.
    SetPageFocus(bool),
    /// Resize the off-screen browser to (w, h) pixels.
    ResizeBrowser(i32, i32),
}

impl Event {
    pub fn kind(&self) -> i32 {
        match self {
            Event::LoadUrl(_) => KIND_LOAD_URL,
            Event::GoBack => KIND_GO_BACK,
            Event::GoForward => KIND_GO_FORWARD,
            Event::Reload => KIND_RELOAD,
            Event::Stop => KIND_STOP,
            Event::PageTouchDown(..) => KIND_PAGE_TOUCH_DOWN,
            Event::PageTouchMove(..) => KIND_PAGE_TOUCH_MOVE,
            Event::PageTouchUp(..) => KIND_PAGE_TOUCH_UP,
            Event::ShowIme(_) => KIND_SHOW_IME,
            Event::HideIme => KIND_HIDE_IME,
            Event::SetPageFocus(_) => KIND_SET_PAGE_FOCUS,
            Event::ResizeBrowser(..) => KIND_RESIZE_BROWSER,
        }
    }

    fn parts(&self) -> (i32, i32, f32, f32, &str) {
        match self {
            Event::LoadUrl(s) => (0, 0, 0.0, 0.0, s),
            Event::GoBack => (0, 0, 0.0, 0.0, ""),
            Event::GoForward => (0, 0, 0.0, 0.0, ""),
            Event::Reload => (0, 0, 0.0, 0.0, ""),
            Event::Stop => (0, 0, 0.0, 0.0, ""),
            Event::PageTouchDown(x, y) => (0, 0, *x, *y, ""),
            Event::PageTouchMove(x, y) => (0, 0, *x, *y, ""),
            Event::PageTouchUp(x, y) => (0, 0, *x, *y, ""),
            Event::ShowIme(s) => (0, 0, 0.0, 0.0, s),
            Event::HideIme => (0, 0, 0.0, 0.0, ""),
            Event::SetPageFocus(v) => (if *v { 1 } else { 0 }, 0, 0.0, 0.0, ""),
            Event::ResizeBrowser(w, h) => (*w, *h, 0.0, 0.0, ""),
        }
    }

    pub fn serialize(&self) -> Vec<u8> {
        let (a, b, x, y, s) = self.parts();
        let bytes = s.as_bytes();
        // Header: kind, a, b (i32) + x, y (f32) + str_len (i32) = 24 bytes.
        let mut out = Vec::with_capacity(24 + bytes.len());
        out.extend_from_slice(&self.kind().to_le_bytes());
        out.extend_from_slice(&a.to_le_bytes());
        out.extend_from_slice(&b.to_le_bytes());
        out.extend_from_slice(&x.to_le_bytes());
        out.extend_from_slice(&y.to_le_bytes());
        out.extend_from_slice(&(bytes.len() as i32).to_le_bytes());
        out.extend_from_slice(bytes);
        out
    }
}

/// Serializes up to `budget` bytes of queued events; events that don't fit
/// stay queued for the next poll. Returns (bytes_written, count).
pub fn drain(events: &mut VecDeque<Event>, out: &mut Vec<u8>, budget: usize) -> usize {
    out.clear();
    let mut count = 0usize;
    loop {
        let next = match events.front() {
            Some(ev) => ev.serialize(),
            None => break,
        };
        if out.len() + next.len() > budget {
            break;
        }
        out.extend_from_slice(&next);
        events.pop_front();
        count += 1;
    }
    count
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_layout() {
        let ev = Event::LoadUrl("https://sails.test/".to_string());
        let s = ev.serialize();
        assert_eq!(&s[0..4], &KIND_LOAD_URL.to_le_bytes());
        let slen = i32::from_le_bytes(s[20..24].try_into().unwrap());
        assert_eq!(slen as usize, "https://sails.test/".len());
        assert_eq!(s.len(), 24 + 19);
    }
}
