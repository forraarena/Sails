//! Raw OpenGL ES 3.0 FFI (subset) -- no bindgen, no crates.

#![allow(non_snake_case, dead_code, clippy::too_many_arguments)]

use std::ffi::c_void;

pub type GLenum = u32;
pub type GLuint = u32;
pub type GLint = i32;
pub type GLsizei = i32;
pub type GLsizeiptr = isize;
pub type GLfloat = f32;
pub type GLboolean = u8;
/// c_char is `i8` on x86_64 but `u8` on aarch64 (all Android 64-bit targets) --
/// the JNI/GL string APIs must use the std alias, never a hardcoded i8.
pub type GLchar = std::ffi::c_char;

pub const GL_NO_ERROR: GLenum = 0;
pub const GL_TRUE: GLboolean = 1;
pub const GL_FALSE: GLboolean = 0;

// Buffers & drawing
pub const GL_ARRAY_BUFFER: GLenum = 0x8892;
pub const GL_STREAM_DRAW: GLenum = 0x88E0;
pub const GL_FLOAT: GLenum = 0x1406;
pub const GL_TRIANGLES: GLenum = 0x0004;

// Texture
pub const GL_TEXTURE_2D: GLenum = 0x0DE1;
pub const GL_TEXTURE0: GLenum = 0x84C0;
pub const GL_TEXTURE1: GLenum = 0x84C1;
pub const GL_RGBA: GLenum = 0x1908;
pub const GL_UNSIGNED_BYTE: GLenum = 0x1401;
pub const GL_TEXTURE_MIN_FILTER: GLenum = 0x2801;
pub const GL_TEXTURE_MAG_FILTER: GLenum = 0x2800;
pub const GL_TEXTURE_WRAP_S: GLenum = 0x2802;
pub const GL_TEXTURE_WRAP_T: GLenum = 0x2803;
pub const GL_NEAREST: GLenum = 0x2600;
pub const GL_LINEAR: GLenum = 0x2601;
pub const GL_CLAMP_TO_EDGE: GLenum = 0x812F;
pub const GL_UNPACK_ALIGNMENT: GLenum = 0x0CF5;

// State
pub const GL_BLEND: GLenum = 0x0BE2;
pub const GL_DEPTH_TEST: GLenum = 0x0B71;
pub const GL_SRC_ALPHA: GLenum = 0x0302;
pub const GL_ONE_MINUS_SRC_ALPHA: GLenum = 0x0303;
pub const GL_COLOR_BUFFER_BIT: GLenum = 0x4000;

// Shaders
pub const GL_VERTEX_SHADER: GLenum = 0x8B31;
pub const GL_FRAGMENT_SHADER: GLenum = 0x8B30;
pub const GL_COMPILE_STATUS: GLenum = 0x8B81;
pub const GL_LINK_STATUS: GLenum = 0x8B82;

#[link(name = "GLESv3")]
extern "C" {
    pub fn glViewport(x: GLint, y: GLint, w: GLsizei, h: GLsizei);
    pub fn glClearColor(r: GLfloat, g: GLfloat, b: GLfloat, a: GLfloat);
    pub fn glClear(mask: GLenum);
    pub fn glEnable(cap: GLenum);
    pub fn glDisable(cap: GLenum);
    pub fn glBlendFunc(sfactor: GLenum, dfactor: GLenum);
    pub fn glGetError() -> GLenum;

    pub fn glGenTextures(n: GLsizei, textures: *mut GLuint);
    pub fn glDeleteTextures(n: GLsizei, textures: *const GLuint);
    pub fn glBindTexture(target: GLenum, texture: GLuint);
    pub fn glActiveTexture(texture: GLenum);
    pub fn glTexImage2D(
        target: GLenum,
        level: GLint,
        internalformat: GLint,
        width: GLsizei,
        height: GLsizei,
        border: GLint,
        format: GLenum,
        ty: GLenum,
        pixels: *const c_void,
    );
    pub fn glTexSubImage2D(
        target: GLenum,
        level: GLint,
        xoffset: GLint,
        yoffset: GLint,
        width: GLsizei,
        height: GLsizei,
        format: GLenum,
        ty: GLenum,
        pixels: *const c_void,
    );
    pub fn glTexParameteri(target: GLenum, pname: GLenum, param: GLint);
    pub fn glPixelStorei(pname: GLenum, param: GLint);

    pub fn glCreateShader(ty: GLenum) -> GLuint;
    pub fn glShaderSource(
        shader: GLuint,
        count: GLsizei,
        source: *const *const GLchar,
        length: *const GLint,
    );
    pub fn glCompileShader(shader: GLuint);
    pub fn glGetShaderiv(shader: GLuint, pname: GLenum, params: *mut GLint);
    pub fn glGetShaderInfoLog(shader: GLuint, bufSize: GLsizei, length: *mut GLsizei, infoLog: *mut GLchar);
    pub fn glCreateProgram() -> GLuint;
    pub fn glAttachShader(program: GLuint, shader: GLuint);
    pub fn glLinkProgram(program: GLuint);
    pub fn glGetProgramiv(program: GLuint, pname: GLenum, params: *mut GLint);
    pub fn glUseProgram(program: GLuint);
    pub fn glGetUniformLocation(program: GLuint, name: *const GLchar) -> GLint;
    pub fn glUniform1i(location: GLint, v0: GLint);
    pub fn glUniform1f(location: GLint, v0: GLfloat);
    pub fn glUniform2f(location: GLint, v0: GLfloat, v1: GLfloat);

    pub fn glGenBuffers(n: GLsizei, buffers: *mut GLuint);
    pub fn glBindBuffer(target: GLenum, buffer: GLuint);
    pub fn glBufferData(target: GLenum, size: GLsizeiptr, data: *const c_void, usage: GLenum);
    pub fn glVertexAttribPointer(
        index: GLuint,
        size: GLint,
        ty: GLenum,
        normalized: GLboolean,
        stride: GLsizei,
        pointer: *const c_void,
    );
    pub fn glEnableVertexAttribArray(index: GLuint);
    pub fn glDrawArrays(mode: GLenum, first: GLint, count: GLsizei);
}

/// Loads, compiles and links a vertex+fragment pair; returns None on failure.
/// Attribute locations are fixed by `layout(location=N)` in the shader source.
pub unsafe fn build_program(vert_src: &str, frag_src: &str) -> Option<GLuint> {
    let mut vs_bytes = vert_src.as_bytes().to_vec();
    let mut fs_bytes = frag_src.as_bytes().to_vec();
    vs_bytes.push(0);
    fs_bytes.push(0);

    let vs = glCreateShader(GL_VERTEX_SHADER);
    let vs_ptr = [vs_bytes.as_ptr() as *const GLchar];
    glShaderSource(vs, 1, vs_ptr.as_ptr(), std::ptr::null());
    glCompileShader(vs);
    if !shader_ok(vs) {
        return None;
    }

    let fs = glCreateShader(GL_FRAGMENT_SHADER);
    let fs_ptr = [fs_bytes.as_ptr() as *const GLchar];
    glShaderSource(fs, 1, fs_ptr.as_ptr(), std::ptr::null());
    glCompileShader(fs);
    if !shader_ok(fs) {
        return None;
    }

    let program = glCreateProgram();
    glAttachShader(program, vs);
    glAttachShader(program, fs);
    glLinkProgram(program);
    let mut linked: GLint = 0;
    glGetProgramiv(program, GL_LINK_STATUS, &mut linked);
    if linked == 0 {
        crate::log_error(&format!("program link failed, glGetError={:#x}", glGetError()));
        return None;
    }
    Some(program)
}

unsafe fn shader_ok(shader: GLuint) -> bool {
    let mut compiled: GLint = 0;
    glGetShaderiv(shader, GL_COMPILE_STATUS, &mut compiled);
    if compiled == 0 {
        let mut buf = [0 as GLchar; 512];
        let mut len: GLsizei = 0;
        glGetShaderInfoLog(shader, 512, &mut len, buf.as_mut_ptr());
        let msg = std::ffi::CStr::from_ptr(buf.as_ptr())
            .to_string_lossy()
            .into_owned();
        crate::log_error(&format!("shader compile failed: {msg}"));
        return false;
    }
    true
}

/// GLSL sources shared by the compositor. One program renders both the page
/// texture quad and all flat chrome geometry: a uniform picks per-vertex color
/// vs textured mode, and the sampler is bound to either texture unit.
pub const VERT_SRC: &str = concat!(
    "#version 300 es\n",
    "layout(location=0) in vec2 aPos;\n",
    "layout(location=1) in vec4 aColor;\n",
    "layout(location=2) in vec2 aUV;\n",
    "uniform vec2 uResolution;\n",
    "out vec4 vColor;\n",
    "out vec2 vUV;\n",
    "void main() {\n",
    "    vec2 ndc = vec2(aPos.x / uResolution.x * 2.0 - 1.0,\n",
    "                    1.0 - aPos.y / uResolution.y * 2.0);\n",
    "    gl_Position = vec4(ndc, 0.0, 1.0);\n",
    "    vColor = aColor;\n",
    "    vUV = aUV;\n",
    "}\n",
);

pub const FRAG_SRC: &str = concat!(
    "#version 300 es\n",
    "precision mediump float;\n",
    "in vec4 vColor;\n",
    "in vec2 vUV;\n",
    "uniform sampler2D uTex;\n",
    "uniform float uUseTex;\n",
    "out vec4 fragColor;\n",
    "void main() {\n",
    "    vec4 tex = texture(uTex, vUV);\n",
    "    fragColor = mix(vColor, vColor * tex, uUseTex);\n",
    "}\n",
);
