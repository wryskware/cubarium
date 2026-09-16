//! FW-0's render budget: how long one cube frame costs to draw and encode, and how
//! long one world tick costs, on a world that has actually populated.
//!
//! ```text
//! cargo run --release -p cubarium --example render_bench -- \
//!     --art assets/atelier --ticks 3000 --frames 600 --pin 7
//! ```
//!
//! Two measurements, each reported as a median and a p95 over its samples:
//!
//! A four-core render is deliberately *not* measured here. Parallel rendering arrives
//! with FW-3's deterministic row-band hook; `Canvas` exposes no band today and
//! `ArtPresenter::draw` takes `&mut self`, so anything this bench could time would be a
//! benchmark-only arrangement that the run loop will never execute. The plan's four-core
//! column stays pending an FW-3 rerun.
//!
//! * **(a) render + encode, one thread** — exactly what `Step::Render` does in the run
//!   loop (`runner/mod.rs:774-800`): `presenter.draw(view, f, &mut canvas)` followed by
//!   `canvas.encode(&mut frame)`. The two halves are also reported separately, because
//!   FW-3 needs to know which half a row-band split would actually shorten.
//! * **(b) one world tick** — `World::step` alone, and then the whole non-headless tick
//!   the loop really pays (`step` + `render_view` + `observe` + `observe_hunters`).
//!
//! Pinning. The board's `core_ctl` driver *isolates* idle big cores, and
//! `sched_setaffinity` to an isolated core fails with `EINVAL` — which is why
//! `taskset -c 7` refuses on an idle Tachyon. `--pin` therefore retries while briefly
//! loading every core, which is what brings the cluster back, and then reports the mask
//! it actually got.

use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use clap::Parser;
use cubarium::art::ArtPack;
use cubarium::art_present::ArtPresenter;
use cubarium::present::Presenter;
use cubarium_core::World;
use cubarium_core::config::WorldConfig;
use cubarium_core::view::RenderView;
use cubarium_render::Canvas;
use cube_proto::Frame;

#[derive(Parser)]
#[command(about = "FW-0: cube render, encode and tick cost on this machine")]
struct Args {
    /// Baked sprite pack. Without it the plain M2 image is measured instead.
    #[arg(long)]
    art: Option<std::path::PathBuf>,
    #[arg(long, default_value_t = 1)]
    seed: u64,
    /// Headless ticks run before anything is timed, so the world is populated.
    #[arg(long, default_value_t = 3000)]
    ticks: u64,
    /// Timed samples per measurement.
    #[arg(long, default_value_t = 600)]
    frames: usize,
    /// CPU list to pin to, e.g. `7` or `4-7`. Empty leaves the scheduler alone.
    #[arg(long, default_value = "")]
    pin: String,
    /// Timed ticks for measurement (b).
    #[arg(long, default_value_t = 200)]
    tick_samples: usize,
    /// A label printed with the results, e.g. the host and the pinning.
    #[arg(long, default_value = "")]
    label: String,
}

/// The presentation under test. Same two calls as the run loop's `Show`.
enum Show {
    Plain(Presenter),
    Art(Box<ArtPresenter>),
}

impl Show {
    fn open(art: Option<&std::path::Path>) -> Result<Show> {
        match art {
            None => Ok(Show::Plain(Presenter::new())),
            Some(dir) => {
                let pack = ArtPack::load(dir)
                    .with_context(|| format!("loading the art pack {}", dir.display()))?;
                Ok(Show::Art(Box::new(ArtPresenter::new(pack))))
            }
        }
    }

    fn observe(&mut self, world: &World, view: &RenderView, hunted: &[cubarium_core::HunterEvent]) {
        match self {
            Show::Plain(p) => p.observe(view),
            Show::Art(p) => {
                p.observe(view);
                p.observe_hunters(view, &world.hunter_view(), hunted)
                    .expect("the shipped pack draws this world's hunters");
            }
        }
    }

    fn draw(&mut self, view: &RenderView, f: f64, canvas: &mut Canvas) {
        match self {
            Show::Plain(p) => p.draw(view, f, canvas),
            Show::Art(p) => p.draw(view, f, canvas),
        }
    }
}

// --- statistics ----------------------------------------------------------------------

/// Median and p95 in milliseconds, from a sample of durations.
fn stats(mut xs: Vec<Duration>) -> (f64, f64, f64) {
    xs.sort_unstable();
    let ms = |d: Duration| d.as_secs_f64() * 1e3;
    let median = ms(xs[xs.len() / 2]);
    let p95 = ms(xs[(xs.len() * 95).div_ceil(100).min(xs.len()) - 1]);
    let min = ms(xs[0]);
    (median, p95, min)
}

fn row(name: &str, xs: Vec<Duration>) {
    let n = xs.len();
    let (median, p95, min) = stats(xs);
    println!("{name:<44} n={n:<5} median {median:8.3} ms   p95 {p95:8.3} ms   min {min:8.3} ms");
}

// --- pinning -------------------------------------------------------------------------

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

#[cfg(target_os = "linux")]
fn current_affinity() -> Vec<usize> {
    unsafe {
        let mut set: libc::cpu_set_t = std::mem::zeroed();
        libc::CPU_ZERO(&mut set);
        if libc::sched_getaffinity(0, size_of::<libc::cpu_set_t>(), &mut set) != 0 {
            return Vec::new();
        }
        (0..libc::CPU_SETSIZE as usize)
            .filter(|&c| libc::CPU_ISSET(c, &set))
            .collect()
    }
}

#[cfg(not(target_os = "linux"))]
fn set_affinity(_cpus: &[usize]) -> bool {
    false
}

#[cfg(not(target_os = "linux"))]
fn current_affinity() -> Vec<usize> {
    Vec::new()
}

/// Spin every logical CPU for `ms`, so an isolating governor brings the big cluster back.
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

/// `core_ctl` isolates idle big cores and then refuses to schedule onto them, so the
/// first attempt at `cpu7` on an idle board fails. Load the machine and try again.
fn pin(cpus: &[usize]) -> Result<()> {
    for attempt in 0..25 {
        if set_affinity(cpus) {
            println!(
                "pinned to {:?} (attempt {})",
                current_affinity(),
                attempt + 1
            );
            return Ok(());
        }
        wake_cluster(60);
    }
    anyhow::bail!(
        "could not pin to {cpus:?} after 25 attempts; the cores are isolated \
         (current mask {:?})",
        current_affinity()
    )
}

// --- the bench -----------------------------------------------------------------------

fn main() -> Result<()> {
    let args = Args::parse();
    if !args.pin.is_empty() {
        pin(&parse_cpus(&args.pin)?)?;
    }

    let config = WorldConfig {
        seed: args.seed,
        ..Default::default()
    };
    let mut world = World::new(config).map_err(|e| anyhow::anyhow!("invalid world config: {e}"))?;
    let mut show = Show::open(args.art.as_deref())?;

    let warm = Instant::now();
    for _ in 0..args.ticks {
        world.step();
        let _ = world.drain_events();
        let _ = world.drain_hunter_events();
    }
    let view = world.render_view();
    let hunted = world.drain_hunter_events();
    show.observe(&world, &view, &hunted);
    println!(
        "{}\nworld: seed {} warmed {} ticks in {:.1} s; population {}, organisms in view {}",
        args.label,
        args.seed,
        args.ticks,
        warm.elapsed().as_secs_f64(),
        world.population(),
        view.organisms.len(),
    );
    println!(
        "presentation: {}",
        match &args.art {
            None => "plain (M2 discs)".to_string(),
            Some(d) => format!("art pack {}", d.display()),
        }
    );

    // (a) one thread: draw, encode, and the two together.
    let mut canvas = Canvas::cube();
    let mut frame = Frame::black();
    for i in 0..60 {
        show.draw(&view, (i % 20) as f64 / 20.0, &mut canvas);
        canvas.encode(&mut frame);
    }
    let mut draws = Vec::with_capacity(args.frames);
    let mut encodes = Vec::with_capacity(args.frames);
    let mut both = Vec::with_capacity(args.frames);
    for i in 0..args.frames {
        let f = (i % 20) as f64 / 20.0;
        let t0 = Instant::now();
        show.draw(&view, f, &mut canvas);
        let t1 = Instant::now();
        canvas.encode(&mut frame);
        let t2 = Instant::now();
        draws.push(t1 - t0);
        encodes.push(t2 - t1);
        both.push(t2 - t0);
    }
    println!("\n(a) one thread, the run loop's Step::Render");
    row("    presenter.draw", draws);
    row("    canvas.encode", encodes);
    row("    R = draw + encode", both);

    // (b) one tick, bare and as the loop pays it.
    let mut bare = Vec::with_capacity(args.tick_samples);
    for _ in 0..args.tick_samples {
        let t0 = Instant::now();
        world.step();
        bare.push(t0.elapsed());
        let _ = world.drain_events();
        let _ = world.drain_hunter_events();
    }
    let mut full = Vec::with_capacity(args.tick_samples);
    for _ in 0..args.tick_samples {
        let t0 = Instant::now();
        world.step();
        let _ = world.drain_events();
        let hunted = world.drain_hunter_events();
        let published = world.render_view();
        show.observe(&world, &published, &hunted);
        full.push(t0.elapsed());
    }
    println!("\n(b) one world tick");
    row("    World::step", bare);
    row("    tick as the loop pays it", full);
    println!("\npopulation after timing: {}", world.population());
    Ok(())
}
