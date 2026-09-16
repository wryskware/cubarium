//! FW-P's lever bench: what the presenter's per-pass cost would become under each of the
//! four levers in `design/7_Research/presenter-budget-2026-09-16.md`.
//!
//! ```text
//! cargo run --release -p cubarium --example presenter_budget -- \
//!     --art assets/atelier --ticks 3000 --frames 400 --pin 0
//! ```
//!
//! `render_bench` (FW-0) measures what one frame costs today and is not touched. This
//! bench answers the *next* question — where that cost would go — with five measurements
//! the pass attribution cannot make on its own:
//!
//! * **(A) ablations.** The same presenter drawn against a doctored `RenderView`: rain on
//!   every cell, water taken away, organisms taken away. `draw` reads the view afresh each
//!   frame, so each difference is that pass's cost, measured rather than sampled. The
//!   profile's world has no rain at all, so the rain pass has to be turned *on* to be seen.
//! * **(B) the composite.** A cached background layer costs one `Canvas` copy per frame
//!   instead of the passes that built it; this times that copy.
//! * **(C) the pixel→cell geometry.** Every field pass recomputes
//!   `cell_of(Topology::Cube, Scale::ONE, SurfacePoint::pixel_center(Topology::Cube, ..))` for a pixel and its four neighbours, which
//!   is a constant of the raster. This times the recomputation against a table lookup.
//! * **(D) the stamp footprints.** `unfold_pixels` is called once per stamp from an anchor
//!   that never moves. This times the 1,280 plant-slot unfolds against cloning a cached
//!   answer, and reports how many take the cheap in-chart path.
//! * **(E) core scaling.** Four independent presenters drawing four independent canvases
//!   at once, pinned to four cores. That is the *ceiling* a row-band split could reach:
//!   perfectly disjoint work, no join, no shared canvas.

use cubarium_surface::{Scale, Topology};
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use clap::Parser;
use cubarium::art::ArtPack;
use cubarium::art_present::{ArtPresenter, slot_of};
use cubarium_core::World;
use cubarium_core::config::WorldConfig;
use cubarium_core::view::RenderView;
use cubarium_render::Canvas;
use cubarium_surface::{CellId, PixelImage, SurfacePoint, cell_of, pixel_neighbor, unfold_pixels};
use cube_proto::{FACE_SIZE, Face};

#[derive(Parser)]
#[command(about = "FW-P: where ArtPresenter::draw's milliseconds would go under each lever")]
struct Args {
    #[arg(long)]
    art: Option<std::path::PathBuf>,
    #[arg(long, default_value_t = 1)]
    seed: u64,
    #[arg(long, default_value_t = 3000)]
    ticks: u64,
    #[arg(long, default_value_t = 400)]
    frames: usize,
    /// CPU list to pin the serial measurements to, e.g. `0`.
    #[arg(long, default_value = "")]
    pin: String,
    /// CPUs the (E) scaling measurement spreads over, one thread each.
    #[arg(long, default_value = "0,1,2,3")]
    cores: String,
    #[arg(long, default_value = "")]
    label: String,
}

// --- statistics ----------------------------------------------------------------------

fn median_ms(mut xs: Vec<Duration>) -> f64 {
    xs.sort_unstable();
    xs[xs.len() / 2].as_secs_f64() * 1e3
}

fn row(name: &str, ms: f64, base: f64) {
    if base > 0.0 {
        println!("{name:<46} {ms:8.3} ms   Δ {:+8.3} ms", ms - base);
    } else {
        println!("{name:<46} {ms:8.3} ms");
    }
}

// --- pinning (same retry dance as render_bench) ---------------------------------------

fn parse_cpus(spec: &str) -> Result<Vec<usize>> {
    let mut out = Vec::new();
    for part in spec.split(',').filter(|s| !s.is_empty()) {
        match part.split_once('-') {
            None => out.push(part.trim().parse()?),
            Some((a, b)) => {
                let (a, b): (usize, usize) = (a.trim().parse()?, b.trim().parse()?);
                out.extend(a..=b);
            }
        }
    }
    Ok(out)
}

#[cfg(target_os = "linux")]
fn set_affinity(cpus: &[usize]) -> bool {
    unsafe {
        let mut set: libc::cpu_set_t = std::mem::zeroed();
        libc::CPU_ZERO(&mut set);
        for &c in cpus {
            libc::CPU_SET(c, &mut set);
        }
        libc::sched_setaffinity(0, size_of::<libc::cpu_set_t>(), &set) == 0
    }
}

#[cfg(not(target_os = "linux"))]
fn set_affinity(_cpus: &[usize]) -> bool {
    false
}

fn wake_cluster(ms: u64) {
    let n = std::thread::available_parallelism().map_or(8, |n| n.get()) * 2;
    std::thread::scope(|s| {
        for _ in 0..n {
            s.spawn(move || {
                let end = Instant::now() + Duration::from_millis(ms);
                let mut x = 0u64;
                while Instant::now() < end {
                    for i in 0..4096u64 {
                        x = x.wrapping_mul(6364136223846793005).wrapping_add(i);
                    }
                }
                std::hint::black_box(x);
            });
        }
    });
}

fn pin(cpus: &[usize]) -> bool {
    for _ in 0..25 {
        if set_affinity(cpus) {
            return true;
        }
        wake_cluster(60);
    }
    false
}

// --- the world under test -------------------------------------------------------------

/// A presenter warmed on `world`'s view, exactly as the run loop warms one.
fn presenter(art: &std::path::Path, world: &World, view: &RenderView) -> Result<Box<ArtPresenter>> {
    let pack =
        ArtPack::load(art).with_context(|| format!("loading the art pack {}", art.display()))?;
    let mut p = Box::new(ArtPresenter::new(pack));
    p.observe(view);
    p.observe_hunters(view, &world.hunter_view(), &[])
        .map_err(|e| anyhow::anyhow!("hunter observe: {e}"))?;
    Ok(p)
}

/// Median `draw` over `frames` frames against one view, after a short warm-up.
fn draw_ms(p: &mut ArtPresenter, view: &RenderView, canvas: &mut Canvas, frames: usize) -> f64 {
    for i in 0..30 {
        p.draw(view, (i % 20) as f64 / 20.0, canvas);
    }
    let mut xs = Vec::with_capacity(frames);
    for i in 0..frames {
        let f = (i % 20) as f64 / 20.0;
        let t0 = Instant::now();
        p.draw(view, f, canvas);
        xs.push(t0.elapsed());
    }
    median_ms(xs)
}

fn main() -> Result<()> {
    let args = Args::parse();
    let serial = parse_cpus(&args.pin)?;
    if !serial.is_empty() && !pin(&serial) {
        anyhow::bail!("could not pin to {serial:?}");
    }
    let art = args
        .art
        .clone()
        .unwrap_or_else(|| std::path::PathBuf::from("assets/atelier"));

    let config = WorldConfig {
        seed: args.seed,
        ..Default::default()
    };
    let mut world = World::new(config).map_err(|e| anyhow::anyhow!("invalid world config: {e}"))?;
    for _ in 0..args.ticks {
        world.step();
        let _ = world.drain_events();
        let _ = world.drain_hunter_events();
    }
    let view = world.render_view();
    let mut p = presenter(&art, &world, &view)?;
    let mut canvas = Canvas::cube();
    println!(
        "{}\nworld: seed {} warmed {} ticks; population {}, organisms in view {}; art {}",
        args.label,
        args.seed,
        args.ticks,
        world.population(),
        view.organisms.len(),
        art.display(),
    );
    let wet = view.water.iter().filter(|&&w| w > 0.0).count();
    let raining = view.rain.iter().filter(|&&r| r > 0.0).count();
    println!("cells with water {wet}/{}, with rain {raining}", view.water.len());

    // --- (A) ablations ----------------------------------------------------------------
    println!("\n(A) ablations: the same presenter, the same frame, one input taken away");
    let base = draw_ms(&mut p, &view, &mut canvas, args.frames);
    row("full frame (baseline)", base, 0.0);

    let mut rainy = view.clone();
    rainy.rain.iter_mut().for_each(|r| *r = 0.5);
    row(
        "+ rain on every cell",
        draw_ms(&mut p, &rainy, &mut canvas, args.frames),
        base,
    );

    let mut flooded = view.clone();
    flooded.water.iter_mut().for_each(|w| *w = 0.8);
    row(
        "+ water on every cell",
        draw_ms(&mut p, &flooded, &mut canvas, args.frames),
        base,
    );

    let mut dry = view.clone();
    dry.water.iter_mut().for_each(|w| *w = 0.0);
    row(
        "- water depth zeroed (pass still walks)",
        draw_ms(&mut p, &dry, &mut canvas, args.frames),
        base,
    );

    let mut dryer = view.clone();
    dryer.water = Vec::new();
    row(
        "- water field empty (pass skipped)",
        draw_ms(&mut p, &dryer, &mut canvas, args.frames),
        base,
    );

    let mut empty = view.clone();
    empty.organisms.clear();
    row(
        "- organisms (bodies + hunters)",
        draw_ms(&mut p, &empty, &mut canvas, args.frames),
        base,
    );

    let mut quiet = view.clone();
    quiet.producer.iter_mut().for_each(|v| *v = 0.0);
    quiet.detritus.iter_mut().for_each(|v| *v = 0.0);
    quiet.carrion.iter_mut().for_each(|v| *v = 0.0);
    row(
        "- producer and detritus zeroed",
        draw_ms(&mut p, &quiet, &mut canvas, args.frames),
        base,
    );

    // --- (B) the composite a cached background would cost ------------------------------
    println!("\n(B) compositing a cached layer instead of rebuilding it");
    let cached = canvas.clone();
    let mut dst = Canvas::cube();
    let mut xs = Vec::with_capacity(args.frames);
    for _ in 0..args.frames {
        let t0 = Instant::now();
        dst.clone_from(std::hint::black_box(&cached));
        xs.push(t0.elapsed());
    }
    std::hint::black_box(&dst);
    row("Canvas::clone_from (5 x 64 x 64 x RGB f32)", median_ms(xs), 0.0);
    let mut xs = Vec::with_capacity(args.frames);
    for _ in 0..args.frames {
        let t0 = Instant::now();
        std::hint::black_box(&mut dst).clear();
        xs.push(t0.elapsed());
    }
    row("Canvas::clear, for scale", median_ms(xs), 0.0);

    // --- (C) the pixel -> cell geometry ------------------------------------------------
    println!("\n(C) pixel->cell geometry, one field pass' worth (5 lookups x 20,480 px)");
    let mut xs = Vec::with_capacity(args.frames);
    for _ in 0..args.frames {
        let t0 = Instant::now();
        let mut acc = 0usize;
        for face in Face::ALL {
            for y in 0..FACE_SIZE as u16 {
                for x in 0..FACE_SIZE as u16 {
                    acc += cell_of(Topology::Cube, Scale::ONE, &SurfacePoint::pixel_center(Topology::Cube, face, x, y)).index();
                    for edge in cubarium_surface::Edge::ALL {
                        if let Some((nf, nx, ny)) = pixel_neighbor(Topology::Cube, face, x, y, edge) {
                            acc += cell_of(Topology::Cube, Scale::ONE, &SurfacePoint::pixel_center(Topology::Cube, nf, nx, ny)).index();
                        }
                    }
                }
            }
        }
        std::hint::black_box(acc);
        xs.push(t0.elapsed());
    }
    row("recomputed, as every field pass does today", median_ms(xs), 0.0);

    // The same answers from a table built once. Five u16 per pixel: own cell and four
    // neighbours, `u16::MAX` where the neighbour does not exist.
    let mut table = vec![u16::MAX; 5 * FACE_SIZE * FACE_SIZE * 5];
    for face in Face::ALL {
        for y in 0..FACE_SIZE as u16 {
            for x in 0..FACE_SIZE as u16 {
                let p = (face.index() * FACE_SIZE + usize::from(y)) * FACE_SIZE + usize::from(x);
                table[p * 5] = cell_of(Topology::Cube, Scale::ONE, &SurfacePoint::pixel_center(Topology::Cube, face, x, y)).index() as u16;
                for (i, edge) in cubarium_surface::Edge::ALL.into_iter().enumerate() {
                    if let Some((nf, nx, ny)) = pixel_neighbor(Topology::Cube, face, x, y, edge) {
                        table[p * 5 + 1 + i] =
                            cell_of(Topology::Cube, Scale::ONE, &SurfacePoint::pixel_center(Topology::Cube, nf, nx, ny)).index() as u16;
                    }
                }
            }
        }
    }
    let mut xs = Vec::with_capacity(args.frames);
    for _ in 0..args.frames {
        let t0 = Instant::now();
        let mut acc = 0usize;
        for &v in table.iter() {
            if v != u16::MAX {
                acc += usize::from(v);
            }
        }
        std::hint::black_box(acc);
        xs.push(t0.elapsed());
    }
    row("from a 51 KiB table built once", median_ms(xs), 0.0);

    // --- (D) the stamp footprints ------------------------------------------------------
    println!("\n(D) unfold_pixels for the 1,280 plant slots (radius 9 px, one per stamp)");
    let anchors: Vec<SurfacePoint> = CellId::all(Topology::Cube, Scale::ONE).map(|c| slot_of(c).at).collect();
    let mut scratch: Vec<PixelImage> = Vec::new();
    let mut seam = 0usize;
    let mut pixels = 0usize;
    for a in &anchors {
        unfold_pixels(Topology::Cube, *a, 9.0, &mut scratch);
        pixels += scratch.len();
        // The in-chart fast path needs the whole disk inside the face.
        let c = a.chart();
        let margin = c.x.min(64.0 - c.x).min(c.y).min(64.0 - c.y);
        if margin < 9.0 {
            seam += 1;
        }
    }
    println!(
        "    {seam}/{} slots take the seam path; {pixels} pixels unfolded in total",
        anchors.len()
    );
    let mut xs = Vec::with_capacity(args.frames.min(200));
    for _ in 0..args.frames.min(200) {
        let t0 = Instant::now();
        for a in &anchors {
            unfold_pixels(Topology::Cube, *a, 9.0, &mut scratch);
            std::hint::black_box(&scratch);
        }
        xs.push(t0.elapsed());
    }
    row("recomputed every frame", median_ms(xs), 0.0);

    let cache: Vec<Vec<PixelImage>> = anchors
        .iter()
        .map(|a| {
            unfold_pixels(Topology::Cube, *a, 9.0, &mut scratch);
            scratch.clone()
        })
        .collect();
    let mut xs = Vec::with_capacity(args.frames.min(200));
    for _ in 0..args.frames.min(200) {
        let t0 = Instant::now();
        for c in &cache {
            std::hint::black_box(c.as_slice());
        }
        xs.push(t0.elapsed());
    }
    row("borrowed from a per-slot cache", median_ms(xs), 0.0);

    // --- (D2) the sRGB encode ----------------------------------------------------------
    // Not part of `draw`, but the other half of `R`, and it scales with the raster exactly
    // as the field passes do. `srgb_encode` widens to `f64` and calls `powf` per channel;
    // the transfer curve is one dimensional, so a table over the linear range answers it.
    println!("\n(D2) the sRGB encode, and what a table would cost instead");
    let mut frame = cube_proto::Frame::black();
    let mut xs = Vec::with_capacity(args.frames);
    for _ in 0..args.frames {
        let t0 = Instant::now();
        std::hint::black_box(&cached).encode(&mut frame);
        xs.push(t0.elapsed());
    }
    row("Canvas::encode as shipped", median_ms(xs), 0.0);
    // 4,096 entries over [0, 1] with linear interpolation: the curve's worst second
    // derivative is near zero, so the error is well under half a code everywhere above
    // the toe. This times the lookup, not its accuracy.
    const N: usize = 4096;
    let lut: Vec<f32> = (0..=N)
        .map(|i| f32::from(cubarium_render::srgb_encode(i as f32 / N as f32)))
        .collect();
    let mut xs = Vec::with_capacity(args.frames);
    for _ in 0..args.frames {
        let t0 = Instant::now();
        let mut acc = 0u32;
        for face in Face::ALL {
            for y in 0..FACE_SIZE as u16 {
                for x in 0..FACE_SIZE as u16 {
                    let px = std::hint::black_box(&cached).get(face, x, y);
                    for c in px {
                        let t = c.clamp(0.0, 1.0) * N as f32;
                        let i = t as usize;
                        let v = lut[i] + (lut[(i + 1).min(N)] - lut[i]) * (t - i as f32);
                        acc += v as u32;
                    }
                }
            }
        }
        std::hint::black_box(acc);
        xs.push(t0.elapsed());
    }
    row("a 4,096-entry table with interpolation", median_ms(xs), 0.0);

    // --- (E) core scaling ---------------------------------------------------------------
    let cores = parse_cpus(&args.cores)?;
    if cores.len() > 1 {
        println!(
            "\n(E) independent presenters in parallel on {} cores (the split's ceiling)",
            cores.len()
        );
        let frames = args.frames.min(200);
        let one = {
            let mut c = Canvas::cube();
            draw_ms(&mut p, &view, &mut c, frames)
        };
        row("one presenter, one core", one, 0.0);
        let mut packs = Vec::new();
        for _ in 0..cores.len() {
            packs.push(presenter(&art, &world, &view)?);
        }
        let view_ref = &view;
        let results: Vec<f64> = std::thread::scope(|s| {
            let handles: Vec<_> = packs
                .iter_mut()
                .zip(cores.iter())
                .map(|(p, &cpu)| {
                    s.spawn(move || {
                        pin(&[cpu]);
                        let mut c = Canvas::cube();
                        draw_ms(p, view_ref, &mut c, frames)
                    })
                })
                .collect();
            handles.into_iter().map(|h| h.join().unwrap()).collect()
        });
        let slowest = results.iter().cloned().fold(0.0f64, f64::max);
        println!(
            "    per-thread medians: {}",
            results
                .iter()
                .map(|m| format!("{m:.3}"))
                .collect::<Vec<_>>()
                .join(", ")
        );
        row("slowest of the parallel threads", slowest, 0.0);
        println!(
            "    aggregate throughput scaling: {:.2}x on {} cores ({:.0} % efficiency)",
            one * cores.len() as f64 / slowest,
            cores.len(),
            100.0 * one / slowest,
        );
        if !serial.is_empty() {
            pin(&serial);
        }
    }

    // --- (F) the deterministic row-band split ------------------------------------------
    // (E) is the ceiling: four presenters drawing four whole, independent images. This is
    // the real thing — one image, cut into row bands, each band drawn by its own presenter
    // on its own core and spliced back. The composite is checked against the serial image
    // before anything is timed, because a split that is faster and different is worthless.
    if cores.len() > 1 {
        println!("\n(F) one image over {} row bands, one core each", cores.len());
        let frames = args.frames.min(200);
        let n = cores.len();
        let mut serial_canvas = Canvas::cube();
        let one = draw_ms(&mut p, &view, &mut serial_canvas, frames);
        row("serial, one core", one, 0.0);

        let mut band_presenters = Vec::new();
        for _ in 0..n {
            band_presenters.push(presenter(&art, &world, &view)?);
        }
        let mut canvas = Canvas::cube();
        let mut bands = canvas.bands(n);
        let view_ref = &view;

        // Correctness first: one frame, split, against the serial image.
        p.draw(&view, 0.0, &mut serial_canvas);
        canvas.split_into(&mut bands);
        std::thread::scope(|s| {
            for (band, bp) in bands.iter_mut().zip(band_presenters.iter_mut()) {
                s.spawn(move || bp.draw(view_ref, 0.0, band));
            }
        });
        canvas.gather(&bands);
        println!(
            "    composite vs serial: {}",
            if canvas.pixels() == serial_canvas.pixels() {
                "bit-identical"
            } else {
                "DIFFERS"
            }
        );

        let cpus = cores.clone();
        let mut xs = Vec::with_capacity(frames);
        for i in 0..frames + 30 {
            let f = (i % 20) as f64 / 20.0;
            let t0 = Instant::now();
            canvas.split_into(&mut bands);
            std::thread::scope(|s| {
                for ((band, bp), &cpu) in
                    bands.iter_mut().zip(band_presenters.iter_mut()).zip(cpus.iter())
                {
                    s.spawn(move || {
                        pin(&[cpu]);
                        bp.draw(view_ref, f, band);
                    });
                }
            });
            canvas.gather(&bands);
            if i >= 30 {
                xs.push(t0.elapsed());
            }
        }
        let split = median_ms(xs);
        row("split over the bands, wall clock", split, one);
        println!("    speedup: {:.2}x on {n} bands ({:.0} % efficiency)", one / split, 100.0 * one / (split * n as f64));
        if !serial.is_empty() {
            pin(&serial);
        }
    }

    Ok(())
}
