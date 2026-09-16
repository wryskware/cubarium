//! Q1 fallback: EGL surfaceless + GLES2, render to an FBO and glReadPixels.
//!
//! Which stack is exercised is chosen by env:
//!   GPUS_EGL_LIB      library to dlopen (default libEGL.so.1; try
//!                     /usr/lib/aarch64-linux-gnu/libEGL_adreno.so for the blob,
//!                     libEGL_mesa.so.0 for Mesa)
//!   GPUS_GLES_LIB     GLES2 library for core symbols (default libGLESv2.so.2)
//!   GPUS_EGL_PLATFORM default | surfaceless | device
//! Args: [width] [height] [frames]
use std::time::Instant;

use anyhow::{anyhow, Context, Result};
use khronos_egl as egl;
use glow::HasContext;

const PLATFORM_SURFACELESS_MESA: egl::Enum = 0x31DD;
const PLATFORM_DEVICE_EXT: egl::Enum = 0x313F;
const PLATFORM_GBM_KHR: egl::Enum = 0x31D7;

const VS: &str = r#"
attribute vec2 a_pos;
varying vec2 v_uv;
void main(){ v_uv = a_pos; gl_Position = vec4(a_pos * 1.6 - 0.8, 0.0, 1.0); }
"#;
const FS: &str = r#"
precision mediump float;
varying vec2 v_uv;
uniform sampler2D u_tex;
void main(){ gl_FragColor = texture2D(u_tex, v_uv); }
"#;

fn main() -> Result<()> {
    let mut args = std::env::args().skip(1);
    let w: i32 = args.next().unwrap_or_else(|| "1920".into()).parse()?;
    let h: i32 = args.next().unwrap_or_else(|| "1080".into()).parse()?;
    let frames: usize = args.next().unwrap_or_else(|| "60".into()).parse()?;

    let egl_lib = std::env::var("GPUS_EGL_LIB").unwrap_or_else(|_| "libEGL.so.1".into());
    let gles_lib = std::env::var("GPUS_GLES_LIB").unwrap_or_else(|_| "libGLESv2.so.2".into());
    let platform = std::env::var("GPUS_EGL_PLATFORM").unwrap_or_else(|_| "default".into());
    println!("EGL lib={egl_lib} GLES lib={gles_lib} platform={platform}");

    let egl = unsafe { egl::DynamicInstance::<egl::EGL1_4>::load_required_from_filename(&egl_lib) }
        .map_err(|e| anyhow!("dlopen {egl_lib}: {e}"))?;

    println!("client extensions: {:?}", egl.query_string(None, egl::EXTENSIONS));

    // eglGetPlatformDisplayEXT is an EGL 1.4 client extension; khronos-egl only
    // exposes the 1.5 core entry point, so call the EXT one by hand.
    type GetPlatformDisplayExt = unsafe extern "C" fn(egl::Enum, *mut std::ffi::c_void, *const egl::Int) -> egl::EGLDisplay;
    // Keep the gbm device (and its fd) alive for the whole run.
    let mut _gbm_keep: Option<(libloading::Library, std::fs::File, *mut std::ffi::c_void)> = None;
    let dpy = match platform.as_str() {
        "gbm" => {
            let gbm_lib = std::env::var("GPUS_GBM_LIB").unwrap_or_else(|_| "libgbm.so.1".into());
            let node = std::env::var("GPUS_DRM_NODE").unwrap_or_else(|_| "/dev/dri/renderD128".into());
            println!("gbm lib={gbm_lib} node={node}");
            let lib = unsafe { libloading::Library::new(&gbm_lib) }.map_err(|e| anyhow!("dlopen {gbm_lib}: {e}"))?;
            let f = std::fs::OpenOptions::new().read(true).write(true).open(&node).with_context(|| format!("open {node}"))?;
            use std::os::unix::io::AsRawFd;
            let dev = unsafe {
                let create: libloading::Symbol<unsafe extern "C" fn(std::ffi::c_int) -> *mut std::ffi::c_void> =
                    lib.get(b"gbm_create_device\0").map_err(|e| anyhow!("gbm_create_device: {e}"))?;
                create(f.as_raw_fd())
            };
            if dev.is_null() { return Err(anyhow!("gbm_create_device({node}) returned NULL")); }
            let p = egl.get_proc_address("eglGetPlatformDisplayEXT").ok_or_else(|| anyhow!("no eglGetPlatformDisplayEXT"))?;
            let g: unsafe extern "C" fn(egl::Enum, *mut std::ffi::c_void, *const egl::Int) -> egl::EGLDisplay = unsafe { std::mem::transmute(p) };
            let d = unsafe { g(PLATFORM_GBM_KHR, dev, std::ptr::null()) };
            if d.is_null() { return Err(anyhow!("eglGetPlatformDisplayEXT(GBM) -> NO_DISPLAY: {:?}", egl.get_error())); }
            _gbm_keep = Some((lib, f, dev));
            unsafe { egl::Display::from_ptr(d) }
        }
        "surfaceless" | "device" => {
            let e = if platform == "surfaceless" { PLATFORM_SURFACELESS_MESA } else { PLATFORM_DEVICE_EXT };
            let p = egl
                .get_proc_address("eglGetPlatformDisplayEXT")
                .ok_or_else(|| anyhow!("eglGetPlatformDisplayEXT not exported by {egl_lib}"))?;
            let f: GetPlatformDisplayExt = unsafe { std::mem::transmute(p) };
            let d = unsafe { f(e, std::ptr::null_mut(), std::ptr::null()) };
            if d.is_null() { return Err(anyhow!("eglGetPlatformDisplayEXT(0x{e:x}) returned NO_DISPLAY: {:?}", egl.get_error())); }
            unsafe { egl::Display::from_ptr(d) }
        }
        _ => unsafe { egl.get_display(egl::DEFAULT_DISPLAY) }.ok_or_else(|| anyhow!("eglGetDisplay returned NO_DISPLAY"))?,
    };

    let (maj, min) = egl.initialize(dpy).context("eglInitialize")?;
    println!("EGL {maj}.{min} vendor={:?} version={:?}", egl.query_string(Some(dpy), egl::VENDOR), egl.query_string(Some(dpy), egl::VERSION));
    let dexts = egl.query_string(Some(dpy), egl::EXTENSIONS).map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
    println!("display extensions: {dexts}");
    for e in ["EGL_KHR_surfaceless_context", "EGL_KHR_image_base", "EGL_EXT_image_dma_buf_import", "EGL_MESA_image_dma_buf_export", "EGL_KHR_gl_renderbuffer_image", "EGL_ANDROID_native_fence_sync"] {
        println!("  [{}] {e}", if dexts.split_whitespace().any(|x| x == e) { "YES" } else { "no " });
    }

    egl.bind_api(egl::OPENGL_ES_API).context("eglBindAPI")?;
    let cfg = egl
        .choose_first_config(
            dpy,
            &[
                egl::SURFACE_TYPE, egl::PBUFFER_BIT,
                egl::RENDERABLE_TYPE, egl::OPENGL_ES2_BIT,
                egl::RED_SIZE, 8, egl::GREEN_SIZE, 8, egl::BLUE_SIZE, 8, egl::ALPHA_SIZE, 8,
                egl::NONE,
            ],
        )
        .context("eglChooseConfig")?
        .ok_or_else(|| anyhow!("no EGLConfig with PBUFFER|ES2|RGBA8"))?;

    let ctx = egl
        .create_context(dpy, cfg, None, &[egl::CONTEXT_CLIENT_VERSION, 2, egl::NONE])
        .context("eglCreateContext")?;

    // Surfaceless first; fall back to a 1x1 pbuffer if the driver refuses.
    let force_pbuf = std::env::var("GPUS_FORCE_PBUFFER").is_ok();
    let mut pbuf = None;
    if force_pbuf || egl.make_current(dpy, None, None, Some(ctx)).is_err() {
        let e = egl.get_error();
        println!("using a {w}x{h} pbuffer (surfaceless error: {e:?})");
        let s = egl
            .create_pbuffer_surface(dpy, cfg, &[egl::WIDTH, w, egl::HEIGHT, h, egl::NONE])
            .context("eglCreatePbufferSurface")?;
        egl.make_current(dpy, Some(s), Some(s), Some(ctx)).context("eglMakeCurrent(pbuffer)")?;
        pbuf = Some(s);
    } else {
        println!("surfaceless eglMakeCurrent: OK");
    }

    // Core GLES2 entry points come from the GLES library; extensions from EGL.
    let glesl = unsafe { libloading::Library::new(&gles_lib) }.map_err(|e| anyhow!("dlopen {gles_lib}: {e}"))?;
    let gl = unsafe {
        glow::Context::from_loader_function(|name| {
            let c = std::ffi::CString::new(name).unwrap();
            let from_lib = glesl.get::<unsafe extern "C" fn()>(c.as_bytes_with_nul()).ok().map(|s| *s as *const std::ffi::c_void);
            let from_egl = egl.get_proc_address(name).map_or(std::ptr::null(), |p| p as *const std::ffi::c_void);
            if std::env::var("GPUS_TRACE_SYMS").is_ok() && matches!(name, "glClear" | "glDrawArrays" | "glFinish" | "glReadPixels" | "glGetString") {
                println!("  sym {name}: lib={from_lib:?} egl={from_egl:?}");
            }
            // eglGetProcAddress FIRST. Dlopening libGLESv2_adreno.so ourselves can
            // map a SECOND copy of the driver (the blob's libEGL dlopens its own
            // path); that copy has no current context and silently no-ops every
            // draw call. EGL_KHR_get_all_proc_addresses makes this safe for core
            // entry points too. Set GPUS_SYMS=lib to get the old behaviour.
            if std::env::var("GPUS_SYMS").as_deref() == Ok("lib") {
                from_lib.unwrap_or(from_egl)
            } else if from_egl.is_null() {
                from_lib.unwrap_or(std::ptr::null())
            } else {
                from_egl
            }
        })
    };
    unsafe {
        println!("GL_VENDOR={} GL_RENDERER={} GL_VERSION={}",
            gl.get_parameter_string(glow::VENDOR), gl.get_parameter_string(glow::RENDERER), gl.get_parameter_string(glow::VERSION));
    }

    let use_fbo = !std::env::var("GPUS_DEFAULT_FB").is_ok();
    let (render_ms, read_ms, probe) = unsafe { run(&gl, w, h, frames, use_fbo) }?;
    println!("corner(4,4)={:?} centre={:?}", probe.0, probe.1);
    stat("clear + textured quad + glFinish", &render_ms);
    stat("glReadPixels RGBA8", &read_ms);

    egl.make_current(dpy, None, None, None).ok();
    if let Some(s) = pbuf { egl.destroy_surface(dpy, s).ok(); }
    egl.destroy_context(dpy, ctx).ok();
    Ok(())
}

type Px = (u8, u8, u8, u8);

unsafe fn run(gl: &glow::Context, w: i32, h: i32, frames: usize, use_fbo: bool) -> Result<(Vec<f64>, Vec<f64>, (Px, Px))> {
    // Checker texture.
    let tw = 256usize;
    let mut texels = vec![0u8; tw * tw * 4];
    for y in 0..tw { for x in 0..tw {
        let i = (y * tw + x) * 4;
        texels[i] = if (x / 16 + y / 16) % 2 == 0 { 255 } else { 40 };
        texels[i + 1] = (x * 255 / tw) as u8;
        texels[i + 2] = (y * 255 / tw) as u8;
        texels[i + 3] = 255;
    }}
    let tex = gl.create_texture().map_err(|e| anyhow!("{e}"))?;
    gl.bind_texture(glow::TEXTURE_2D, Some(tex));
    gl.tex_image_2d(glow::TEXTURE_2D, 0, glow::RGBA as i32, tw as i32, tw as i32, 0, glow::RGBA, glow::UNSIGNED_BYTE, glow::PixelUnpackData::Slice(Some(&texels)));
    gl.tex_parameter_i32(glow::TEXTURE_2D, glow::TEXTURE_MIN_FILTER, glow::LINEAR as i32);
    gl.tex_parameter_i32(glow::TEXTURE_2D, glow::TEXTURE_MAG_FILTER, glow::LINEAR as i32);
    gl.tex_parameter_i32(glow::TEXTURE_2D, glow::TEXTURE_WRAP_S, glow::CLAMP_TO_EDGE as i32);
    gl.tex_parameter_i32(glow::TEXTURE_2D, glow::TEXTURE_WRAP_T, glow::CLAMP_TO_EDGE as i32);

    // Offscreen colour target.
    let color = gl.create_texture().map_err(|e| anyhow!("{e}"))?;
    gl.bind_texture(glow::TEXTURE_2D, Some(color));
    gl.tex_image_2d(glow::TEXTURE_2D, 0, glow::RGBA as i32, w, h, 0, glow::RGBA, glow::UNSIGNED_BYTE, glow::PixelUnpackData::Slice(None));
    gl.tex_parameter_i32(glow::TEXTURE_2D, glow::TEXTURE_MIN_FILTER, glow::NEAREST as i32);
    gl.tex_parameter_i32(glow::TEXTURE_2D, glow::TEXTURE_MAG_FILTER, glow::NEAREST as i32);
    if use_fbo {
        let fbo = gl.create_framebuffer().map_err(|e| anyhow!("{e}"))?;
        gl.bind_framebuffer(glow::FRAMEBUFFER, Some(fbo));
        gl.framebuffer_texture_2d(glow::FRAMEBUFFER, glow::COLOR_ATTACHMENT0, glow::TEXTURE_2D, Some(color), 0);
        let st = gl.check_framebuffer_status(glow::FRAMEBUFFER);
        if st != glow::FRAMEBUFFER_COMPLETE {
            return Err(anyhow!("FBO incomplete: 0x{st:x}"));
        }
    } else {
        println!("rendering to the default framebuffer (pbuffer)");
    }
    println!("FBO complete; err after setup = 0x{:x}", gl.get_error());
    println!("IMPLEMENTATION_COLOR_READ_FORMAT=0x{:x} TYPE=0x{:x}",
        gl.get_parameter_i32(glow::IMPLEMENTATION_COLOR_READ_FORMAT),
        gl.get_parameter_i32(glow::IMPLEMENTATION_COLOR_READ_TYPE));

    let prog = gl.create_program().map_err(|e| anyhow!("{e}"))?;
    for (kind, src) in [(glow::VERTEX_SHADER, VS), (glow::FRAGMENT_SHADER, FS)] {
        let s = gl.create_shader(kind).map_err(|e| anyhow!("{e}"))?;
        gl.shader_source(s, src);
        gl.compile_shader(s);
        if !gl.get_shader_compile_status(s) { return Err(anyhow!("shader: {}", gl.get_shader_info_log(s))); }
        gl.attach_shader(prog, s);
    }
    gl.link_program(prog);
    if !gl.get_program_link_status(prog) { return Err(anyhow!("link: {}", gl.get_program_info_log(prog))); }
    gl.use_program(Some(prog));

    let verts: [f32; 8] = [0.0, 0.0, 1.0, 0.0, 0.0, 1.0, 1.0, 1.0];
    let vbo = gl.create_buffer().map_err(|e| anyhow!("{e}"))?;
    gl.bind_buffer(glow::ARRAY_BUFFER, Some(vbo));
    gl.buffer_data_u8_slice(glow::ARRAY_BUFFER, bytemuck::cast_slice(&verts), glow::STATIC_DRAW);
    let loc = gl.get_attrib_location(prog, "a_pos").ok_or_else(|| anyhow!("a_pos"))? ;
    gl.enable_vertex_attrib_array(loc);
    gl.vertex_attrib_pointer_f32(loc, 2, glow::FLOAT, false, 8, 0);
    gl.active_texture(glow::TEXTURE0);
    gl.bind_texture(glow::TEXTURE_2D, Some(tex));
    gl.uniform_1_i32(gl.get_uniform_location(prog, "u_tex").as_ref(), 0);
    gl.viewport(0, 0, w, h);

    let mut render_ms = Vec::new();
    let mut read_ms = Vec::new();
    let mut out = vec![0u8; (w * h * 4) as usize];
    for f in 0..frames {
        let t0 = Instant::now();
        gl.clear_color(0.05, 0.02, 0.12, 1.0);
        gl.clear(glow::COLOR_BUFFER_BIT);
        gl.draw_arrays(glow::TRIANGLE_STRIP, 0, 4);
        if f == 0 { println!("err after draw = 0x{:x}", gl.get_error()); }
        gl.finish();
        let t1 = Instant::now();
        gl.read_pixels(0, 0, w, h, glow::RGBA, glow::UNSIGNED_BYTE, glow::PixelPackData::Slice(Some(&mut out)));
        let t2 = Instant::now();
        let e = gl.get_error();
        if e != 0 { return Err(anyhow!("GL error 0x{e:x} on frame {f}")); }
        if f == 0 {
            println!("frame0 draw={:.3}us read={:.3}us nonzero_bytes={}",
                (t1 - t0).as_secs_f64() * 1e6, (t2 - t1).as_secs_f64() * 1e6,
                out.iter().filter(|b| **b != 0).count());
        }
        if f >= 5 {
            render_ms.push((t1 - t0).as_secs_f64() * 1e3);
            read_ms.push((t2 - t1).as_secs_f64() * 1e3);
        }
    }
    let px = |x: i32, y: i32| { let i = ((y * w + x) * 4) as usize; (out[i], out[i+1], out[i+2], out[i+3]) };
    Ok((render_ms, read_ms, (px(4, 4), px(w / 2, h / 2))))
}

fn stat(name: &str, v: &[f64]) {
    if v.is_empty() { return; }
    let mut s = v.to_vec();
    s.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let mean = s.iter().sum::<f64>() / s.len() as f64;
    println!("{name}: n={} min={:.2} p50={:.2} p95={:.2} max={:.2} mean={:.2} ms",
        s.len(), s[0], s[s.len()/2], s[(s.len()*95/100).min(s.len()-1)], s[s.len()-1], mean);
}
