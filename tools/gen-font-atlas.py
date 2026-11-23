import re

src = open('glcdfont.c').read()
m = re.search(r'font\[\]\s*PROGMEM\s*=\s*\{([^}]*)\}', src, re.S)
body = m.group(1)
vals = [int(x, 16) for x in re.findall(r'0x[0-9A-Fa-f]{2}', body)]
print("total bytes:", len(vals), "=> glyphs:", len(vals) / 5)

# Each byte = one COLUMN (LSB = top row); 5 columns x 7 rows per glyph.
# Atlas: 16x8 grid of 6x8 px cells (5x7 glyph + 1px right/bottom padding).
COLS, ROWS = 16, 8
GW, GH = 6, 8
W, H = COLS * GW, ROWS * GH
px = [[0] * 4 for _ in range(W * H)]
for g in range(128):
    gx, gy = (g % COLS) * GW, (g // COLS) * GH
    for col in range(5):
        idx = g * 5 + col
        b = vals[idx] if idx < len(vals) else 0
        for row in range(7):
            if b & (1 << row):
                x, y = gx + col, gy + row
                px[y * W + x] = [255, 255, 255, 255]

raw = bytearray()
for p in px:
    raw += bytes(p)
open('font_atlas.rgba', 'wb').write(raw)
print("atlas:", W, "x", H)

lines = []
for i in range(0, len(raw), 16):
    lines.append(', '.join(f'0x{b:02x}' for b in raw[i:i + 16]))
rust = f"//! 128-glyph 5x7 ASCII font atlas: 16x8 cells of 6x8 px, RGBA8.\n//! Generated from the classic GLCD 5x7 font (see NOTICE.md).\n"
rust += f"pub const FONT_ATLAS_RGBA: [u8; {len(raw)}] = [\n"
for l in lines:
    rust += "    " + l + ",\n"
rust += "];\n\n"
rust += f"pub const FONT_ATLAS_W: u32 = {W};\npub const FONT_ATLAS_H: u32 = {H};\n"
rust += "pub const FONT_GLYPH_W: f32 = 5.0;\npub const FONT_GLYPH_H: f32 = 7.0;\n"
rust += "pub const FONT_CELL_W: f32 = 6.0;\npub const FONT_CELL_H: f32 = 8.0;\n"
open('font_atlas.rs', 'w').write(rust)
print("rust file written,", len(rust), "bytes")

ppm = f"P6 {W} {H} 255\n".encode() + bytes(v for p in px for v in p[:3])
open('font_atlas.ppm', 'wb').write(ppm)
