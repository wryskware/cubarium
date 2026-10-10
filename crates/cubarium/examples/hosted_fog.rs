//! `hosted_fog`: the desktop app's own display path, headless (FOG-MOTION,
//! `design/handoffs/fog-motion-2026-10-10.md`). A session runs the world at speed 1 with
//! the hosted renderer hooked in (`cubarium::sink::gpu::voxel::hosted`, what the Godot app
//! drives), and this takes its frames as the app does, at 60 a second of wall time.
//!
//! ```text
//! cargo run --release -p cubarium --example hosted_fog -- --load DIR \
//!     [--config config/desktop/terrarium.toml] [--seconds 60] [--log FILE.csv] \
//!     [--video FILE.mp4] [--crop X0,Y0,X1,Y1]
//! ```
//!
//! `--load` is a state `fog_capture --state` saved. Each frame logs its tick, its mean
//! luminance and how far that moved since the frame before; `--video` pipes the frames to
//! ffmpeg (x264, yuv444p, crf 12) at the rate they arrive. No window is ever opened.

use std::io::Write as _;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use anyhow::{Context, Result, bail};

use cubarium::sink::gpu::voxel::hosted::{Hosted, HostedOptions};
use cubarium_session::{Prepared, Session, SessionOptions, SimParams};

fn main() -> Result<()> {
    let (mut load, mut config, mut seconds, mut log, mut video, mut crop) =
        (None, PathBuf::from("config/desktop/terrarium.toml"), 60.0f64, None, None, None);
    let mut it = std::env::args().skip(1);
    while let Some(k) = it.next() {
        let mut v = || it.next().with_context(|| format!("{k} needs a value"));
        match k.as_str() {
            "--load" => load = Some(PathBuf::from(v()?)),
            "--config" => config = PathBuf::from(v()?),
            "--seconds" => seconds = v()?.parse()?,
            "--log" => log = Some(PathBuf::from(v()?)),
            "--video" => video = Some(PathBuf::from(v()?)),
            "--crop" => {
                let s = v()?;
                let n: Vec<u32> = s.split(',').map(str::parse).collect::<Result<_, _>>()?;
                let [x0, y0, x1, y1] = n[..] else { bail!("--crop X0,Y0,X1,Y1, not {s:?}") };
                crop = Some([x0, y0, x1, y1]);
            }
            _ => bail!("unexpected argument {k}"),
        }
    }
    let load = load.context("--load DIR")?;
    let s = cubarium_voxel_sim::saved::load(&load, &[]).map_err(anyhow::Error::msg)?;
    let session = Session::new(SessionOptions::default());
    let hosted = Hosted::new(HostedOptions {
        config,
        root: std::env::current_dir()?,
        screen: (1920, 1080),
        fps: 60.0,
        highlight_mix: 0.4,
    })?;
    session.set_hook(Some(hosted.hook()));
    session.install(Prepared {
        world: s.world,
        flora: s.flora,
        fauna: s.fauna,
        label: format!("hosted_fog {}", load.display()),
        exec: SimParams::default(),
    });

    let mut log = match &log {
        Some(p) => {
            let mut f = std::io::BufWriter::new(std::fs::File::create(p)?);
            writeln!(f, "frame,wall_s,tick,mean_l,dl_mean")?;
            Some(f)
        }
        None => None,
    };
    let mut ffmpeg: Option<std::process::Child> = None;
    let mut prev: Option<Vec<f32>> = None;
    let start = Instant::now();
    let mut first: Option<Instant> = None;
    let mut k = 0u64;
    let mut next = Instant::now();
    loop {
        let now = Instant::now();
        if first.is_some_and(|f| now - f > Duration::from_secs_f64(seconds)) {
            break;
        }
        if first.is_none() && now - start > Duration::from_secs(120) {
            bail!("no frame in 120 s");
        }
        if let Some(f) = hosted.take_frame() {
            let t0 = *first.get_or_insert(now);
            let [x0, y0, x1, y1] = crop.unwrap_or([0, 0, f.width, f.height]);
            let (x1, y1) = (x1.min(f.width), y1.min(f.height));
            let (cw, ch) = (x1 - x0, y1 - y0);
            let mut px = Vec::with_capacity((cw * ch * 4) as usize);
            for y in y0..y1 {
                let row = ((y * f.width + x0) * 4) as usize;
                px.extend_from_slice(&f.rgba[row..row + (cw * 4) as usize]);
            }
            if ffmpeg.is_none()
                && let Some(path) = &video
            {
                ffmpeg = Some(
                    Command::new("ffmpeg")
                        .args(["-y", "-loglevel", "error", "-f", "rawvideo", "-pix_fmt", "rgba", "-s"])
                        .arg(format!("{cw}x{ch}"))
                        .args(["-r", "60", "-i", "-", "-c:v", "libx264", "-pix_fmt", "yuv444p", "-crf", "12"])
                        .args(["-preset", "fast"])
                        .arg(path)
                        .stdin(Stdio::piped())
                        .spawn()
                        .context("starting ffmpeg")?,
                );
            }
            let lum: Vec<f32> = px
                .chunks_exact(4)
                .map(|c| 0.2126 * f32::from(c[0]) + 0.7152 * f32::from(c[1]) + 0.0722 * f32::from(c[2]))
                .collect();
            let mean = lum.iter().sum::<f32>() / lum.len().max(1) as f32;
            let dl = prev.as_ref().map_or(0.0, |q| {
                lum.iter().zip(q).map(|(a, b)| (a - b).abs()).sum::<f32>() / lum.len().max(1) as f32
            });
            prev = Some(lum);
            if let Some(l) = &mut log {
                writeln!(l, "{k},{:.4},{},{mean:.3},{dl:.4}", (now - t0).as_secs_f64(), f.tick)?;
            }
            if let Some(ff) = &mut ffmpeg {
                ff.stdin.as_mut().context("ffmpeg stdin")?.write_all(&px)?;
            }
            hosted.recycle(f);
            k += 1;
        }
        next += Duration::from_secs_f64(1.0 / 60.0);
        let now = Instant::now();
        if next > now {
            std::thread::sleep(next - now);
        } else {
            next = now;
        }
    }
    if let Some(mut ff) = ffmpeg {
        drop(ff.stdin.take());
        ff.wait()?;
    }
    let st = session.status();
    eprintln!("hosted_fog: {k} frames; tick {}", st.tick);
    session.set_hook(None);
    session.close();
    Ok(())
}
