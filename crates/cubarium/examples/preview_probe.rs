//! `preview_probe`: the new-world picker's thumbnails, headless. A session draws terrain
//! candidates as the picker asks for them, and each ready one is shown on a hosted renderer
//! of its own, as a card's world view shows it. Prints when each candidate was ready and
//! when its first and final pictures arrived (milliseconds from the request), and saves
//! those pictures. A timing, not a check; no window is ever opened.
//!
//! ```text
//! CUBARIUM_RENDER_TIMING=1 cargo run --release -p cubarium --example preview_probe -- \
//!     [--config config/desktop/terrarium.toml] [--size 288,138,48] [--seed 5] [--count 4] \
//!     [--screen 576x324] [--batches 2] [--out DIR] [--water 0.75] [--fast] \
//!     [--video FILE.mp4]
//! ```
//!
//! `--fast` shows the fast tier alone ([`StillLighting::Fast`]). `--video` records card 0
//! of the first batch from its first picture until a second after its last, sampled at
//! 60 frames a second of wall time, scaled 2x (x264, yuv444p, crf 12): the sky's shading
//! easing in.

use std::path::PathBuf;
use std::time::{Duration, Instant};

use anyhow::{Context, Result, bail};

use std::io::Write as _;
use std::process::{Command, Stdio};

use cubarium::sink::gpu::voxel::hosted::{Hosted, HostedFrame, HostedOptions, StillLighting};
use cubarium_session::{Session, SessionOptions, WorldSpec};

fn main() -> Result<()> {
    let mut config = PathBuf::from("config/desktop/terrarium.toml");
    let (mut size, mut seed, mut count, mut screen, mut batches, mut out, mut water) =
        ([288u32, 138, 48], 5u64, 4usize, (576u32, 324u32), 1usize, None::<PathBuf>, Some(0.75f64));
    let (mut fast, mut video) = (false, None::<PathBuf>);
    let mut it = std::env::args().skip(1);
    while let Some(k) = it.next() {
        let mut v = || it.next().with_context(|| format!("{k} needs a value"));
        match k.as_str() {
            "--config" => config = PathBuf::from(v()?),
            "--size" => {
                let n: Vec<u32> = v()?.split(',').map(str::parse).collect::<Result<_, _>>()?;
                let [w, h, d] = n[..] else { bail!("--size W,H,D") };
                size = [w, h, d];
            }
            "--seed" => seed = v()?.parse()?,
            "--count" => count = v()?.parse()?,
            "--screen" => {
                let s = v()?;
                let (w, h) = s.split_once('x').context("--screen WxH")?;
                screen = (w.parse()?, h.parse()?);
            }
            "--batches" => batches = v()?.parse()?,
            "--out" => out = Some(PathBuf::from(v()?)),
            "--water" => water = Some(v()?.parse()?).filter(|w: &f64| *w > 0.0),
            "--fast" => fast = true,
            "--video" => video = Some(PathBuf::from(v()?)),
            _ => bail!("unexpected argument {k}"),
        }
    }
    if let Some(o) = &out {
        std::fs::create_dir_all(o)?;
    }
    let root = std::env::current_dir()?;
    let session = Session::new(SessionOptions::default());
    for b in 0..batches {
        let name = if b == 0 { "cold".to_string() } else { format!("warm{b}") };
        let seeds: Vec<Option<u64>> = (0..count).map(|i| Some(seed + (b * 100 + i) as u64)).collect();
        let spec = WorldSpec {
            config: Some(root.join(&config)),
            size: Some(size),
            water_level_m: water,
            ..Default::default()
        };
        let t0 = Instant::now();
        session.generate_candidates_seeded(&spec, &seeds).map_err(anyhow::Error::msg)?;
        let mut cards: Vec<Option<Card>> = (0..count).map(|_| None).collect();
        let deadline = t0 + Duration::from_secs(280);
        // Card 0's newest picture, and the recording of it (first batch only).
        let mut shown: Option<HostedFrame> = None;
        // ffmpeg, when it started, when the picture settled, and the frames written.
        let mut film: Option<(std::process::Child, Instant, Option<Instant>, u64)> = None;
        loop {
            let list = session.candidates();
            for c in &list {
                if c.state == "ready" && cards[c.index].is_none() {
                    let hosted = Hosted::new(HostedOptions {
                        config: root.join(&config),
                        root: root.clone(),
                        screen,
                        fps: 60.0,
                        highlight_mix: 0.4,
                    })?;
                    if fast {
                        hosted.set_still_lighting(StillLighting::Fast);
                    }
                    hosted.show_static(session.candidate_source(), c.index, true);
                    cards[c.index] = Some(Card { hosted, ready: t0.elapsed(), first: None, last: None });
                }
            }
            for (i, card) in cards.iter_mut().enumerate() {
                let Some(card) = card else { continue };
                while let Some(f) = card.hosted.take_frame() {
                    let at = t0.elapsed();
                    if card.first.is_none() {
                        card.first = Some(at);
                        save(&out, &format!("{}x{}x{}-{name}-card{i}-first", size[0], size[1], size[2]), &f)?;
                    }
                    if f.settled && card.last.is_none() {
                        card.last = Some(at);
                        save(&out, &format!("{}x{}x{}-{name}-card{i}-final", size[0], size[1], size[2]), &f)?;
                    }
                    if i == 0 && b == 0 && video.is_some() {
                        if let Some(old) = shown.replace(f) {
                            card.hosted.recycle(old);
                        }
                    } else {
                        card.hosted.recycle(f);
                    }
                }
                if let Some(e) = card.hosted.error() {
                    bail!("card {i}: {e}");
                }
            }
            if let (Some(path), Some(f)) = (&video, &shown) {
                if film.is_none() {
                    let child = Command::new("ffmpeg")
                        .args(["-loglevel", "error", "-y", "-f", "rawvideo", "-pix_fmt", "rgba", "-s"])
                        .arg(format!("{}x{}", f.width, f.height))
                        .args(["-r", "60", "-i", "-", "-vf", "scale=iw*2:ih*2:flags=neighbor", "-c:v", "libx264", "-pix_fmt", "yuv444p", "-crf", "12"])
                        .arg(path)
                        .stdin(Stdio::piped())
                        .spawn()
                        .context("starting ffmpeg")?;
                    film = Some((child, Instant::now(), None, 0));
                }
                let (child, start, done, written) = film.as_mut().expect("just made");
                // One picture every 1/60 s of wall time since the first: what a viewer saw.
                let due = (start.elapsed().as_secs_f64() * 60.0) as u64;
                while *written <= due {
                    child.stdin.as_mut().context("ffmpeg's input")?.write_all(&f.rgba)?;
                    *written += 1;
                }
                if f.settled && done.is_none() {
                    *done = Some(Instant::now());
                }
            }
            let filmed = film.as_ref().is_none_or(|(_, _, d, _)| d.is_some_and(|d| d.elapsed() > Duration::from_secs(1)));
            let failed = list.iter().filter(|c| c.state == "failed" || c.state == "cancelled").count();
            let done = cards.iter().filter(|c| c.as_ref().is_some_and(|c| c.last.is_some())).count();
            if (done + failed >= count && (filmed || b > 0 || video.is_none())) || Instant::now() > deadline {
                break;
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        if let Some((mut child, _, _, _)) = film.take() {
            drop(child.stdin.take());
            child.wait()?;
            video = None;
        }
        for (i, c) in cards.iter().enumerate() {
            let ms = |d: Option<Duration>| d.map_or("none".to_string(), |d| format!("{}", d.as_millis()));
            match c {
                Some(c) => println!(
                    "PREVIEW {}x{}x{} {name} card {i}: terrain {} ms, first picture {} ms, final {} ms",
                    size[0], size[1], size[2], c.ready.as_millis(), ms(c.first), ms(c.last)
                ),
                None => println!("PREVIEW {}x{}x{} {name} card {i}: no terrain", size[0], size[1], size[2]),
            }
        }
    }
    Ok(())
}

struct Card {
    hosted: Hosted,
    ready: Duration,
    first: Option<Duration>,
    last: Option<Duration>,
}

fn save(out: &Option<PathBuf>, name: &str, f: &HostedFrame) -> Result<()> {
    let Some(dir) = out else { return Ok(()) };
    let file = std::fs::File::create(dir.join(format!("{name}.png")))?;
    let mut encoder = png::Encoder::new(std::io::BufWriter::new(file), f.width, f.height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder.write_header()?.write_image_data(&f.rgba)?;
    Ok(())
}
