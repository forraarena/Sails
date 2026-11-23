//! Browser chrome: everything drawn by Sails itself, on top of the engine's
//! pixels. All geometry is flat RGBA quads + the bitmap-font glyph quads;
//! layout is computed from surface pixels and the density-scaled chrome bar.

use crate::events::Event;
use crate::font;
use crate::state::{ChromeHit, Shared};

// ---- palette -----------------------------------------------------------------
pub const COLOR_BAR: [f32; 4] = [0.043, 0.063, 0.149, 1.0]; // #0B1026
pub const COLOR_PILL: [f32; 4] = [0.118, 0.153, 0.278, 1.0]; // #1E2747
pub const COLOR_PILL_EDIT: [f32; 4] = [0.157, 0.204, 0.361, 1.0];
pub const COLOR_ACCENT: [f32; 4] = [0.310, 0.765, 0.968, 1.0]; // #4FC3F7
pub const COLOR_TEXT: [f32; 4] = [0.92, 0.94, 0.98, 1.0];
pub const COLOR_TEXT_DIM: [f32; 4] = [0.75, 0.78, 0.85, 1.0];
pub const COLOR_ICON_ON: [f32; 4] = [0.92, 0.94, 0.98, 1.0];
pub const COLOR_ICON_OFF: [f32; 4] = [0.92, 0.94, 0.98, 0.28];
pub const COLOR_PAGE_BG: [f32; 4] = [0.024, 0.039, 0.094, 1.0]; // #060A18

/// The chrome bar is 56dp tall plus a 4dp progress strip = 60dp total.
/// Must match CefriumHost.ensureBrowser (Kotlin).
pub const CHROME_DP: f32 = 60.0;
pub const BAR_DP: f32 = 56.0;
pub const PROGRESS_DP: f32 = 4.0;

pub struct Geom {
    pub data: Vec<f32>,
}

impl Geom {
    pub fn new() -> Self {
        Geom { data: Vec::with_capacity(4096) }
    }

    /// One axis-aligned quad (x, y = top-left in surface pixels).
    pub fn quad(&mut self, x: f32, y: f32, w: f32, h: f32, color: [f32; 4]) {
        self.quad_uv(x, y, w, h, color, [0.0, 0.0, 1.0, 1.0]);
    }

    /// Quad with a sub-rectangle of the bound texture (uv = [u0, v0, u1, v1]).
    pub fn quad_uv(&mut self, x: f32, y: f32, w: f32, h: f32, color: [f32; 4], uv: [f32; 4]) {
        let (r, g, b, a) = (color[0], color[1], color[2], color[3]);
        let (u0, v0, u1, v1) = (uv[0], uv[1], uv[2], uv[3]);
        let x1 = x + w;
        let y1 = y + h;
        // Two triangles: (p0, p1, p2) and (p1, p3, p2); CCW in screen space.
        let p = [
            (x, y, u0, v0),
            (x1, y, u1, v0),
            (x, y1, u0, v1),
            (x1, y, u1, v0),
            (x1, y1, u1, v1),
            (x, y1, u0, v1),
        ];
        for (px, py, u, v) in p {
            self.data.extend_from_slice(&[px, py, r, g, b, a, u, v]);
        }
    }

    /// Flat triangle (for chevrons, sails, spinner heads).
    pub fn tri(&mut self, p0: (f32, f32), p1: (f32, f32), p2: (f32, f32), color: [f32; 4]) {
        let (r, g, b, a) = (color[0], color[1], color[2], color[3]);
        for (px, py) in [p0, p1, p2] {
            self.data.extend_from_slice(&[px, py, r, g, b, a, 0.0, 0.0]);
        }
    }

    /// Ring segment (between two radii, two angles in radians) as a quad.
    pub fn ring_segment(
        &mut self,
        cx: f32,
        cy: f32,
        r_in: f32,
        r_out: f32,
        a0: f32,
        a1: f32,
        color: [f32; 4],
    ) {
        let (s0, c0, s1, c1) = (a0.sin(), a0.cos(), a1.sin(), a1.cos());
        let p = [
            (cx + r_out * c0, cy + r_out * s0),
            (cx + r_out * c1, cy + r_out * s1),
            (cx + r_in * c0, cy + r_in * s0),
            (cx + r_out * c1, cy + r_out * s1),
            (cx + r_in * c1, cy + r_in * s1),
            (cx + r_in * c0, cy + r_in * s0),
        ];
        for (px, py) in p {
            self.data.extend_from_slice(&[px, py, color[0], color[1], color[2], color[3], 0.0, 0.0]);
        }
    }

    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }
}

// ---- layout --------------------------------------------------------------------

pub struct Layout {
    pub bar_h: f32,
    pub btn: f32,     // square touch target per icon
    pub pad: f32,
    pub back_cx: f32,
    pub fwd_cx: f32,
    pub reload_cx: f32,
    pub pill_x: f32,
    pub pill_w: f32,
    pub pill_y: f32,
    pub pill_h: f32,
    pub chrome_h: i32,
}

pub fn layout(w: i32, chrome_h: i32) -> Layout {
    let bar_h = BAR_DP * chrome_h as f32 / CHROME_DP; // proportion
    let btn = 44.0 * chrome_h as f32 / CHROME_DP;
    let pad = 4.0 * chrome_h as f32 / CHROME_DP;
    let reload_cx = w as f32 - pad - btn * 0.5;
    let fwd_cx = pad + btn * 1.5;
    let back_cx = pad + btn * 0.5;
    let pill_x = pad + btn * 2.0 + pad;
    let pill_w = reload_cx - pad - pill_x;
    let pill_h = bar_h - 2.0 * (6.0 * chrome_h as f32 / CHROME_DP);
    let pill_y = (bar_h - pill_h) * 0.5;
    Layout {
        bar_h,
        btn,
        pad,
        back_cx,
        fwd_cx,
        reload_cx,
        pill_x,
        pill_w,
        pill_y,
        pill_h,
        chrome_h,
    }
}

/// Hit-test chrome controls at a point (surface pixels). Points on the bar.
pub fn hit_test(l: &Layout, x: f32, y: f32) -> Option<ChromeHit> {
    if y < 0.0 || y >= l.bar_h {
        return None;
    }
    let half = l.btn * 0.5;
    let in_btn = |cx: f32| x >= cx - half && x < cx + half;
    if in_btn(l.back_cx) {
        return Some(ChromeHit::Back);
    }
    if in_btn(l.fwd_cx) {
        return Some(ChromeHit::Forward);
    }
    if in_btn(l.reload_cx) {
        return Some(ChromeHit::Reload);
    }
    if x >= l.pill_x && x < l.pill_x + l.pill_w && y >= l.pill_y && y < l.pill_y + l.pill_h {
        return Some(ChromeHit::Omnibox);
    }
    None
}

// ---- drawing -------------------------------------------------------------------

pub fn draw_chrome(
    solid: &mut Geom,
    glyph: &mut Geom,
    w: i32,
    h: i32,
    st: &crate::state::SharedView,
    time_ms: u64,
) {
    let l = layout(w, st.chrome_h);
    let d = st.density;

    // Page background (visible before the first engine frame arrives).
    if !st.frame_valid {
        solid.quad(0.0, l.bar_h, w as f32, h as f32 - l.bar_h, COLOR_PAGE_BG);
        draw_text_centered(
            glyph,
            "SAILS - native Rust compositor",
            w as f32 * 0.5,
            l.bar_h + (h as f32 - l.bar_h) * 0.5 - 26.0,
            3.0,
            COLOR_TEXT_DIM,
        );
        draw_text_centered(
            glyph,
            "waiting for the first engine frame",
            w as f32 * 0.5,
            l.bar_h + (h as f32 - l.bar_h) * 0.5 + 6.0,
            2.0,
            COLOR_TEXT_DIM,
        );
    }

    // Chrome bar.
    solid.quad(0.0, 0.0, w as f32, l.bar_h, COLOR_BAR);

    // Back / forward.
    let chev_w = 10.0 * d;
    let chev_h = 16.0 * d;
    let cy = l.bar_h * 0.5;
    let back_col = if st.can_back { COLOR_ICON_ON } else { COLOR_ICON_OFF };
    solid.tri(
        (l.back_cx + chev_w * 0.5, cy - chev_h * 0.5),
        (l.back_cx - chev_w * 0.5, cy),
        (l.back_cx + chev_w * 0.5, cy + chev_h * 0.5),
        back_col,
    );
    let fwd_col = if st.can_fwd { COLOR_ICON_ON } else { COLOR_ICON_OFF };
    solid.tri(
        (l.fwd_cx - chev_w * 0.5, cy - chev_h * 0.5),
        (l.fwd_cx + chev_w * 0.5, cy),
        (l.fwd_cx - chev_w * 0.5, cy + chev_h * 0.5),
        fwd_col,
    );

    // Omnibox pill.
    let pill_col = if st.omnibox_editing { COLOR_PILL_EDIT } else { COLOR_PILL };
    solid.quad(l.pill_x, l.pill_y, l.pill_w, l.pill_h, pill_col);
    if st.omnibox_editing {
        // 2dp accent underline while editing.
        let uw = 2.0 * d;
        solid.quad(l.pill_x, l.pill_y + l.pill_h - uw, l.pill_w, uw, COLOR_ACCENT);
    }

    // Sails logo inside the pill: a tiny white sail on an accent hull.
    let logo_x = l.pill_x + 9.0 * d;
    let logo_h = 14.0 * d;
    let logo_top = cy - logo_h * 0.5;
    solid.tri(
        (logo_x, logo_top + logo_h),
        (logo_x, logo_top),
        (logo_x + logo_h * 0.75, logo_top + logo_h),
        COLOR_TEXT,
    );
    solid.quad(logo_x - 4.0 * d, logo_top + logo_h + 1.5 * d, logo_h * 0.95, 2.5 * d, COLOR_ACCENT);

    // Omnibox text: while editing show the live text + caret; otherwise the
    // current URL (dimmed when empty).
    let text_scale = if d >= 2.0 { 3.0 } else { 2.0 };
    let text_x = logo_x + logo_h + 10.0 * d;
    let max_w = l.pill_x + l.pill_w - text_x - 12.0 * d;
    if st.omnibox_editing {
        draw_text(glyph, &st.omnibox_text, text_x, cy, text_scale, COLOR_TEXT, max_w);
        let text_w = measure_text(&st.omnibox_text, text_scale, max_w);
        let blink = ((time_ms / 500) % 2) == 0;
        if blink {
            solid.quad(
                text_x + text_w + 1.0,
                cy - 7.0 * text_scale * 0.5 - 2.0,
                2.0 * (d / 2.75).max(0.8),
                7.0 * text_scale + 4.0,
                COLOR_TEXT,
            );
        }
    } else {
        let shown: &str = if st.url.is_empty() { "" } else { st.url.as_str() };
        let col = if st.loading() { COLOR_TEXT_DIM } else { COLOR_TEXT };
        draw_text(glyph, shown, text_x, cy, text_scale, col, max_w);
    }

    // Reload / stop.
    let r_out = 9.0 * d;
    let r_in = 5.5 * d;
    if st.loading() {
        // Spinner: rotating arc.
        let speed = 0.0055;
        let phase = (time_ms as f32) * speed;
        let col = COLOR_ACCENT;
        let segments = 3;
        let span = std::f32::consts::FRAC_PI_2 * 1.2;
        for i in 0..segments {
            let a0 = phase + (i as f32) * span * 1.6;
            let a1 = a0 + span;
            solid.ring_segment(l.reload_cx, cy, r_in, r_out, a0, a1, col);
        }
    } else {
        // Circular arrow: 270 degree ring + arrowhead.
        let start = -std::f32::consts::FRAC_PI_2;
        let end = start + std::f32::consts::PI * 1.5;
        let segments = 18;
        let step = (end - start) / segments as f32;
        for i in 0..segments {
            let a0 = start + (i as f32) * step;
            let a1 = a0 + step + 0.01;
            solid.ring_segment(l.reload_cx, cy, r_in, r_out, a0, a1, COLOR_ICON_ON);
        }
        // Arrowhead at the ring's end.
        let hx = l.reload_cx + r_out * end.cos();
        let hy = cy + r_out * end.sin();
        let tip = (hx + 4.0 * d * end.cos() + 2.5 * d * end.sin(),
                   hy + 4.0 * d * end.sin() - 2.5 * d * end.cos());
        let base0 = (hx + 3.0 * d * end.sin(), hy - 3.0 * d * end.cos());
        let base1 = (hx - 3.0 * d * end.sin(), hy + 3.0 * d * end.cos());
        solid.tri(base0, base1, tip, COLOR_ICON_ON);
    }

    // Progress strip under the bar.
    let prog_h = PROGRESS_DP * st.chrome_h as f32 / CHROME_DP;
    solid.quad(0.0, l.bar_h, w as f32, prog_h, [0.0, 0.0, 0.0, 0.35]);
    if st.loading() {
        let pw = (st.progress.max(0).min(100) as f32 / 100.0) * w as f32;
        solid.quad(0.0, l.bar_h, pw, prog_h, COLOR_ACCENT);
    }
}

/// Hit helper used by the input state machine (lib.rs).
pub fn hit_at(surface_w: i32, st: &Shared, x: f32, y: f32) -> Option<ChromeHit> {
    if y >= st.chrome_h as f32 {
        return None;
    }
    hit_test(&layout(surface_w, st.chrome_h), x, y)
}

// ---- text ----------------------------------------------------------------------

/// Advances one string's width in px, clamped to max_w. Full measure if max_w is f32::MAX.
pub fn measure_text(text: &str, scale: f32, max_w: f32) -> f32 {
    let cell = font::FONT_CELL_W * scale;
    let mut used = 0.0;
    for ch in text.chars() {
        if (ch as u32) < 32 || (ch as u32) > 126 {
            continue;
        }
        if used + cell > max_w {
            break;
        }
        used += cell;
    }
    used
}

/// Draws `text` top-aligned at (x, center_y), scaled, clipped to max_w.
#[allow(clippy::too_many_arguments)]
pub fn draw_text(
    glyph: &mut Geom,
    text: &str,
    x: f32,
    center_y: f32,
    scale: f32,
    color: [f32; 4],
    max_w: f32,
) {
    let cell_w = font::FONT_CELL_W * scale;
    let glyph_h = font::FONT_GLYPH_H * scale;
    let mut cx = x;
    let ty = center_y - glyph_h * 0.5;
    for ch in text.chars() {
        if (ch as u32) < 32 || (ch as u32) > 126 {
            continue;
        }
        if cx - x + cell_w > max_w {
            break;
        }
        let g = ch as u32;
        let gx = ((g % 16) as f32) * font::FONT_CELL_W;
        let gy = ((g / 16) as f32) * font::FONT_CELL_H;
        let u0 = gx / font::FONT_ATLAS_W as f32;
        let v0 = gy / font::FONT_ATLAS_H as f32;
        let u1 = u0 + (font::FONT_GLYPH_W / font::FONT_ATLAS_W as f32);
        let v1 = v0 + (font::FONT_GLYPH_H / font::FONT_ATLAS_H as f32);
        glyph.quad_uv(cx, ty, font::FONT_GLYPH_W * scale, glyph_h, color, [u0, v0, u1, v1]);
        cx += cell_w;
    }
}

pub fn draw_text_centered(glyph: &mut Geom, text: &str, center_x: f32, center_y: f32, scale: f32, color: [f32; 4]) {
    let w = measure_text(text, scale, f32::MAX);
    draw_text(glyph, text, center_x - w * 0.5, center_y, scale, color, f32::MAX);
}

/// The event a chrome hit maps to, on tap-release.
pub fn chrome_tap(st: &mut Shared, hit: ChromeHit) {
    match hit {
        ChromeHit::Back => {
            if st.can_back {
                st.push(Event::GoBack);
            }
        }
        ChromeHit::Forward => {
            if st.can_fwd {
                st.push(Event::GoForward);
            }
        }
        ChromeHit::Reload => {
            if st.loading() {
                st.push(Event::Stop);
            } else {
                st.push(Event::Reload);
            }
        }
        ChromeHit::Omnibox => {
            st.omnibox_editing = true;
            let seed = if st.url.is_empty() { String::new() } else { st.url.clone() };
            st.omnibox_text = seed.clone();
            st.push(Event::ShowIme(seed));
        }
    }
}
