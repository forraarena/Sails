#!/usr/bin/env python3
"""Byte-level simulation of the Rust->Kotlin event wire format.

Mirrors rust/src/events.rs (serialize) and SailsSurfaceView.drainEvents
(parse) exactly; asserts a serialized event parses back to the same values
and that the header size matches what both sides assume (24 bytes).
"""
import struct
import sys

KIND_LOAD_URL = 1
KIND_PAGE_TOUCH_MOVE = 7
KIND_RESIZE_BROWSER = 12


def rust_serialize(kind, a, b, x, y, s):
    """Rust: 6x LE u32/i32/f32 header + utf8 bytes (events.rs::serialize)."""
    payload = s.encode("utf-8")
    out = struct.pack("<iii ff i", kind, a, b, x, y, len(payload)) + payload
    return out


def kotlin_parse(buf):
    """Kotlin: ByteBuffer LITTLE_ENDIAN: int, int, int, float, float, int, bytes."""
    off = 0
    events = []
    while off < len(buf):
        kind, a, b, x, y, slen = struct.unpack_from("<iii ff i", buf, off)
        off += 24
        s = buf[off:off + slen].decode("utf-8")
        off += slen
        events.append((kind, a, b, x, y, s))
    return events


tests = [
    (KIND_LOAD_URL, 0, 0, 0.0, 0.0, "https://duckduckgo.com/?q=sails%20browser"),
    (KIND_PAGE_TOUCH_MOVE, 0, 0, 540.5, 1234.25, ""),
    (KIND_RESIZE_BROWSER, 1080, 2235, 0.0, 0.0, ""),
]

fail = 0
for kind, a, b, x, y, s in tests:
    raw = rust_serialize(kind, a, b, x, y, s)
    parsed = kotlin_parse(raw)
    assert len(parsed) == 1, f"expected 1 event, got {len(parsed)}"
    pk, pa, pb, px, py, ps = parsed[0]
    ok = (pk == kind and pa == a and pb == b and abs(px - x) < 1e-6
          and abs(py - y) < 1e-6 and ps == s)
    if not ok:
        print(f" ! MISMATCH: sent {(kind, a, b, x, y, s)} got {parsed[0]}")
        fail += 1
    else:
        print(f" ok {pk} a={pa} b={pb} x={px} y={py} str={ps[:48]!r}")

# header size consistency: Rust capacity comment and Kotlin parser both 24
hdr = struct.calcsize("<iii ff i")
print(f"header size = {hdr} bytes")
if hdr != 24:
    print(" ! header size != 24 -- Kotlin parser and Rust writer must agree")
    fail += 1

sys.exit(1 if fail else 0)
