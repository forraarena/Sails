#!/usr/bin/env python3
"""Static sanity checks for the Sails PoC (no compilers in this environment):
 - bracket/paren/brace + string/comment balance per source file
 - Rust: every `mod x` has src/x.rs; no stray println!/dbg!
 - Kotlin<->Rust JNI symbol-name contract
 - Kotlin<->Rust event-kind wire contract
 - Cargo manifest shape
"""
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
problems = []
notes = []


def strip_code(text, line_comment="//", block_comment=None, hash_comment=None, rust=False):
    """Remove comments and string literals, preserving newlines, for balance checks."""
    out = []
    i = 0
    n = len(text)
    in_str = None  # quote char
    while i < n:
        c = text[i]
        nxt = text[i + 1] if i + 1 < n else ""
        if in_str:
            out.append("\x00" if c != "\n" else "\n")
            if c == "\\":
                out.append("\x00")
                i += 2
                continue
            if c == in_str:
                in_str = None
            i += 1
            continue
        if c in "\"'":
            # Rust: ' starts a char literal only if it closes like one ('x' or
            # '\x'); otherwise it is a lifetime marker and carries no string.
            if rust and c == "'":
                if nxt == "\\":
                    end = text.find("'", i + 2)
                    if end != -1:
                        out.append("\x00" * (end + 1 - i))
                        i = end + 1
                        continue
                if i + 2 < n and text[i + 2] == "'":
                    out.append("\x00\x00\x00")
                    i += 3
                    continue
                # lifetime: emit placeholder, move on
                out.append("\x00")
                i += 1
                continue
            in_str = c
            out.append("\x00")
            i += 1
            continue
        if hash_comment and c == hash_comment:
            while i < n and text[i] != "\n":
                out.append("\x00")
                i += 1
            continue
        if line_comment and text.startswith(line_comment, i):
            while i < n and text[i] != "\n":
                out.append("\x00")
                i += 1
            continue
        if block_comment and text.startswith(block_comment[0], i):
            end = text.find(block_comment[1], i + 2)
            if end == -1:
                problems.append(f"unterminated block comment at offset {i}")
                end = n
            out.append("".join("\n" if ch == "\n" else " " for ch in text[i:end + 2]))
            i = end + 2
            continue
        out.append(c)
        i += 1
    return "".join(out)


BALANCE_PAIRS = {"(": ")", "[": "]", "{": "}"}


def check_balance(name, text, brackets=True):
    # Only Rust/Kotlin/Java-family files have /* */ comments; shell globs like
    # ndk/* would false-positive on it.
    block = ("/*", "*/") if name.endswith((".rs", ".kt", ".kts", ".java")) else None
    code = strip_code(text, rust=name.endswith(".rs"), block_comment=block)
    stack = []
    for idx, c in enumerate(code):
        if not brackets:
            break
        if c in BALANCE_PAIRS:
            stack.append((c, idx))
        elif c in BALANCE_PAIRS.values():
            if not stack:
                line = code[:idx].count("\n") + 1
                problems.append(f"{name}:{line} unmatched '{c}'")
                continue
            open_c, _ = stack.pop()
            if BALANCE_PAIRS[open_c] != c:
                line = code[:idx].count("\n") + 1
                problems.append(f"{name}:{line} mismatched '{open_c}' closed by '{c}'")
    if brackets and stack:
        open_c, idx = stack[-1]
        line = code[:idx].count("\n") + 1
        problems.append(f"{name}:{line} unclosed '{open_c}'")

    # quote parity for shell scripts (case patterns use bare parens)
    if name.endswith(".sh"):
        quotes = strip_code(text, hash_comment="#").count('"')
        if quotes % 2 != 0:
            problems.append(f"{name}: odd number of quote chars")


# --- collect files -------------------------------------------------------------
rust_files = sorted((ROOT / "rust" / "src").glob("*.rs"))
kotlin_files = sorted((ROOT / "app" / "src" / "main" / "kotlin").rglob("*.kt"))

for f in rust_files + kotlin_files + [ROOT / "app" / "build.gradle.kts",
                                      ROOT / "settings.gradle.kts",
                                      ROOT / "build.gradle.kts",
                                      ROOT / ".github" / "workflows" / "ci.yml"]:
    if f.exists():
        check_balance(str(f.relative_to(ROOT)), f.read_text())
    else:
        problems.append(f"missing file: {f}")

# shell scripts: quote parity only (bash case patterns use bare parens)
build_sh = ROOT / "scripts" / "build-rust.sh"
if build_sh.exists():
    check_balance("scripts/build-rust.sh", build_sh.read_text(), brackets=False)
else:
    problems.append("missing file: scripts/build-rust.sh")

# --- Rust module declarations ---------------------------------------------------
lib_rs = (ROOT / "rust" / "src" / "lib.rs").read_text()
mods = re.findall(r"^\s*(?:pub\s+)?mod\s+(\w+)\s*;", lib_rs, re.M)
for m in mods:
    if not (ROOT / "rust" / "src" / f"{m}.rs").exists():
        problems.append(f"lib.rs declares mod {m} but rust/src/{m}.rs is missing")
notes.append(f"lib.rs modules: {', '.join(mods)}")

for f in rust_files:
    t = f.read_text()
    for bad in ("println!", "dbg!", "eprintln!"):
        if bad in t:
            problems.append(f"{f.name}: contains {bad} (Android has no stdout)")

# --- JNI symbol contract --------------------------------------------------------
rust_syms = {}
for f in rust_files:
    for m in re.finditer(r"pub extern \"system\" fn (Java_[A-Za-z0-9_]+)", f.read_text()):
        rust_syms[m.group(1)] = f.name

kt_syms = set()
for f in kotlin_files:
    t = f.read_text()
    if "external fun" in t:
        for m in re.finditer(r"external fun (\w+)", t):
            kt_syms.add(f"Java_dev_sails_NativeLib_{m.group(1)}")

expected = set(rust_syms.keys())
missing_in_rust = kt_syms - expected
missing_in_kotlin = expected - kt_syms
if missing_in_rust:
    problems.append(f"Kotlin external funs with no Rust export: {sorted(missing_in_rust)}")
if missing_in_kotlin:
    problems.append(f"Rust exports with no Kotlin external fun: {sorted(missing_in_kotlin)}")
notes.append(f"JNI surface: {len(kt_syms)} functions, all matched: {not missing_in_rust and not missing_in_kotlin}")

# --- event kind contract ---------------------------------------------------------
rust_kinds = {}
ev = (ROOT / "rust" / "src" / "events.rs").read_text()
for m in re.finditer(r"pub const (KIND_\w+): i32 = (\d+);", ev):
    rust_kinds[m.group(1)] = int(m.group(2))

nativelib_kt = (ROOT / "app" / "src" / "main" / "kotlin" / "dev" / "sails" / "NativeLib.kt").read_text()
kt_kinds = {}
for m in re.finditer(r"const val (\w+) = (\d+)", nativelib_kt):
    kt_kinds[m.group(1)] = int(m.group(2))

for k in ("LOAD_URL", "GO_BACK", "GO_FORWARD", "RELOAD", "STOP", "PAGE_TOUCH_DOWN",
          "PAGE_TOUCH_MOVE", "PAGE_TOUCH_UP", "SHOW_IME", "HIDE_IME", "SET_PAGE_FOCUS",
          "RESIZE_BROWSER"):
    rk = rust_kinds.get(f"KIND_{k}")
    kk = kt_kinds.get(k)
    if rk is None or kk is None:
        problems.append(f"event kind {k}: rust={rk} kotlin={kk}")
    elif rk != kk:
        problems.append(f"event kind {k} mismatch: rust={rk} kotlin={kk}")
notes.append(f"event kinds verified: {len(kt_kinds)} on both sides")

# --- Event enum arms vs kinds -----------------------------------------------------
enum_kinds = len(re.findall(r"Event::\w+", ev.split("impl Event")[0])) - 1  # minus 'enum Event'
if enum_kinds != len(rust_kinds):
    notes.append(f"note: Event enum variants={enum_kinds}, KIND consts={len(rust_kinds)} (check manually)")

# --- Cargo manifest -----------------------------------------------------------------
cargo = (ROOT / "rust" / "Cargo.toml").read_text()
if 'crate-type = ["cdylib"]' not in cargo:
    problems.append("Cargo.toml: cdylib missing")
if re.search(r"^\[dependencies\]", cargo, re.M):
    problems.append("Cargo.toml: [dependencies] section present (must stay zero-dep)")
else:
    notes.append("Cargo.toml: zero dependencies confirmed")

# --- wire format: header size 24 (kind,a,b,x,y,str_len) ------------------------------
ser = ev.split("fn serialize")[1] if "fn serialize" in ev else ""
if "with_capacity(24" not in ser:
    problems.append("events.rs: serialize header size seems wrong (expected 24 bytes)")

# --- GLSL sanity ----------------------------------------------------------------------
rend = (ROOT / "rust" / "src" / "gl.rs").read_text()
for shader in ("VERT_SRC", "FRAG_SRC"):
    m = re.search(shader + r': &str = concat!\((.*?)\n\);', rend, re.S)
    if not m:
        problems.append(f"gl.rs: {shader} not found")
        continue
    body = m.group(1)
    joined = "".join(re.findall(r'"((?:[^"\\]|\\.)*)"', body))
    braces = joined.count("{") - joined.count("}")
    if braces != 0:
        problems.append(f"gl.rs: {shader} braces unbalanced ({braces})")
    if shader == "VERT_SRC" and "#version 300 es" not in joined:
        problems.append("gl.rs: vertex shader missing #version 300 es")

# --- report ---------------------------------------------------------------------------
print("=== notes ===")
for n in notes:
    print(" *", n)
print("=== problems ===" if problems else "=== problems: none ===")
for p in problems:
    print(" !", p)
sys.exit(1 if problems else 0)
