//! `glow_capture`: one live world drawn headless by two renderers side by side, to follow
//! its emitters (lanterns, latticevine flowers, glowcap lips, bloomcrowns) switching at
//! night (GLOW-EASE, `design/handoffs/glow-ease-2026-10-10.md`).
//!
//! ```text
//! cargo run --release -p cubarium --example glow_capture -- \
//!     [--config config/desktop/terrarium.toml] [--set 'KEY = VALUE']... \
//!     [--a 'KEY = VALUE']... [--b 'KEY = VALUE']... [--seed 1] [--threads N] \
//!     [--state DIR] [--load DIR] [--warm TICKS] [--until-night] [--max-ticks N] \
//!     [--seconds S] [--fps 60] [--speed K] [--px 6] [--scan] [--close W,H] [--zoom 2] \
//!     [--video FILE.mp4] [--close-video FILE.mp4] [--log FILE.csv] [--events FILE.csv] \
//!     [--colour] [--flora-at SECONDS 'KEY = VALUE']...
//! ```
//!
//! `--colour` (GLOW-EASE round 2) follows colour swaps instead of emitters: an **event** is
//! a plant cell whose style or a vine cell whose tile B began to swap that pack
//! (`VoxelGpuSink::colour_switches`), and those cells are the ones measured. `--flora-at S
//! LINE` sets the plant layer's config (`FloraConfig` TOML, e.g. `latticevine.dry_water =
//! 0.99`) once the clip reaches S seconds (0: before the first tick), to drive a dry spell
//! or a wake the world would not give in the clip.
//!
//! The world is founded as the live run founds one (or loaded from `--load`, or from
//! `--state`, which a first run saves once the wait is over), stepped `--warm` ticks and,
//! with `--until-night`, on until the sun is down. Then `--seconds` of video are drawn at
//! `--fps`, the sim stepping a tick each time the frame clock crosses one (`--speed K`: K
//! ticks' worth a frame's worth), both renderers packing every staged tick and drawing
//! every frame: **A** with the config and its `--a` lines, **B** with its `--b` lines.
//!
//! Every pack, A's emitting cells are compared with the last pack's: a cell that starts or
//! stops emitting, or changes colour, is an **event** (`--events`). Every frame, for every
//! emitting cell seen so far, the mean luminance of its own pixels (front and top face) and
//! of the area around it (three voxels each way, its own pixels out) is taken in both
//! pictures, and how far each moved since the frame before: the largest moves, overall and
//! within two seconds of an event at that cell, are printed for A and B at the end
//! (`--log` has them per frame). `--scan` runs the clip once undrawn first, to centre the
//! close-up (`--close W,H` pixels, drawn `--zoom` times larger, `--close-video`) on the
//! window with the most events; the sim then starts again from the same state, single
//! threaded by default so it replays them. No window is ever opened.

use std::collections::{HashMap, HashSet};
use std::io::Write as _;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::time::Instant;

use anyhow::{Context, Result, bail};

use cubarium::sink::GpuTargetKind;
use cubarium::sink::gpu::voxel::{VoxelGpuSink, VoxelGpuSinkOptions};
use cubarium::voxel::project::Projection;
use cubarium::voxel::{LayerConfig, Seeds, VoxelConfig, found as found_habitat, params_of};
use cubarium_voxel_sim::{Sim, SimConfig};

struct Args {
    config: PathBuf,
    set: Vec<String>,
    a: Vec<String>,
    b: Vec<String>,
    seed: u64,
    threads: usize,
    state: Option<PathBuf>,
    load: Option<PathBuf>,
    warm: u64,
    until_night: bool,
    max_ticks: u64,
    seconds: f64,
    fps: f64,
    speed: f64,
    px: u32,
    scan: bool,
    close: (u32, u32),
    zoom: u32,
    video: Option<PathBuf>,
    close_video: Option<PathBuf>,
    log: Option<PathBuf>,
    events: Option<PathBuf>,
    nectar_ticks: u64,
    nectar_every: u64,
    colour: bool,
    flora_at: Vec<(f64, String)>,
}

fn args() -> Result<Args> {
    let mut a = Args {
        config: PathBuf::from("config/desktop/terrarium.toml"),
        set: Vec::new(),
        a: Vec::new(),
        b: Vec::new(),
        seed: 1,
        threads: 1,
        state: None,
        load: None,
        warm: 0,
        until_night: false,
        max_ticks: 400_000,
        seconds: 10.0,
        fps: 60.0,
        speed: 1.0,
        px: 6,
        scan: false,
        close: (320, 200),
        zoom: 2,
        video: None,
        close_video: None,
        log: None,
        events: None,
        nectar_ticks: 0,
        nectar_every: 200,
        colour: false,
        flora_at: Vec::new(),
    };
    let mut it = std::env::args().skip(1);
    while let Some(k) = it.next() {
        let mut v = || it.next().with_context(|| format!("{k} needs a value"));
        match k.as_str() {
            "--config" => a.config = PathBuf::from(v()?),
            "--set" => a.set.push(v()?),
            "--a" => a.a.push(v()?),
            "--b" => a.b.push(v()?),
            "--seed" => a.seed = v()?.parse()?,
            "--threads" => a.threads = v()?.parse()?,
            "--state" => a.state = Some(PathBuf::from(v()?)),
            "--load" => a.load = Some(PathBuf::from(v()?)),
            "--warm" => a.warm = v()?.parse()?,
            "--until-night" => a.until_night = true,
            "--max-ticks" => a.max_ticks = v()?.parse()?,
            "--seconds" => a.seconds = v()?.parse()?,
            "--fps" => a.fps = v()?.parse()?,
            "--speed" => a.speed = v()?.parse()?,
            "--px" => a.px = v()?.parse()?,
            "--scan" => a.scan = true,
            "--close" => {
                let s = v()?;
                let n: Vec<u32> = s.split(',').map(str::parse).collect::<Result<_, _>>()?;
                let [w, h] = n[..] else { bail!("--close W,H, not {s:?}") };
                a.close = (w, h);
            }
            "--zoom" => a.zoom = v()?.parse()?,
            "--video" => a.video = Some(PathBuf::from(v()?)),
            "--close-video" => a.close_video = Some(PathBuf::from(v()?)),
            "--log" => a.log = Some(PathBuf::from(v()?)),
            "--events" => a.events = Some(PathBuf::from(v()?)),
            "--nectar-ticks" => a.nectar_ticks = v()?.parse()?,
            "--nectar-every" => a.nectar_every = v()?.parse()?,
            "--colour" => a.colour = true,
            "--flora-at" => {
                let at: f64 = v()?.parse()?;
                a.flora_at.push((at, v()?));
            }
            _ => bail!("unexpected argument {k}"),
        }
    }
    Ok(a)
}

fn config(a: &Args, side: &[String]) -> Result<VoxelConfig> {
    let text = std::fs::read_to_string(&a.config).with_context(|| format!("reading {}", a.config.display()))?;
    let mut table: toml::Table = text.parse().with_context(|| format!("parsing {}", a.config.display()))?;
    for line in a.set.iter().chain(side) {
        let set: toml::Table = line.parse().with_context(|| format!("--set {line:?}"))?;
        merge(&mut table, set);
    }
    let cfg: VoxelConfig = toml::Value::Table(table).try_into().with_context(|| format!("{} with {:?}", a.config.display(), side))?;
    Ok(VoxelConfig { px_per_voxel: a.px, ..cfg })
}

fn merge(table: &mut toml::Table, set: toml::Table) {
    for (k, v) in set {
        match (table.get_mut(&k), v) {
            (Some(toml::Value::Table(t)), toml::Value::Table(s)) => merge(t, s),
            (_, v) => {
                table.insert(k, v);
            }
        }
    }
}

/// `--flora-at`: merge `line` into the plant layer's config.
fn flora_set(sim: &mut Sim, line: &str) -> Result<()> {
    let mut table: toml::Table = toml::Value::try_from(sim.flora().config().clone())?
        .try_into()
        .context("the flora config as a table")?;
    let set: toml::Table = line.parse().with_context(|| format!("--flora-at {line:?}"))?;
    merge(&mut table, set);
    let cfg: cubarium_voxel_flora::FloraConfig = toml::Value::Table(table).try_into().with_context(|| format!("--flora-at {line:?}"))?;
    sim.flora_mut().replace_config(cfg).map_err(anyhow::Error::msg)?;
    eprintln!("glow_capture: flora {line} at {}", status(sim));
    Ok(())
}

/// Apply every `--flora-at` line due by clip second `at` not yet applied (`*next` of them).
fn flora_due(a: &Args, sim: &mut Sim, at: f64, next: &mut usize) -> Result<()> {
    while let Some((t, line)) = a.flora_at.get(*next) {
        if *t > at {
            break;
        }
        flora_set(sim, line)?;
        *next += 1;
    }
    Ok(())
}

fn status(sim: &Sim) -> String {
    let w = sim.world();
    let v = w.view().weather;
    format!("tick {} phase {:.3} daylight {:.2} mode {:?}", w.tick(), v.day_phase, v.daylight, v.mode)
}

fn load_or_found(a: &Args, cfg: &VoxelConfig) -> Result<Sim> {
    let files = a.state.as_ref().map(|d| (d.join("world.voxel"), d.join("flora.bin"), d.join("fauna.bin")));
    let from = a.load.clone().or_else(|| files.as_ref().filter(|(w, _, _)| w.exists()).map(|_| a.state.clone().unwrap()));
    let (world, flora, fauna) = match &from {
        Some(dir) => {
            let s = cubarium_voxel_sim::saved::load(dir, &[]).map_err(anyhow::Error::msg)?;
            (s.world, s.flora, s.fauna)
        }
        None => {
            let params = params_of(cfg, &[])?;
            let f = found_habitat(
                &cfg.sim.world,
                &LayerConfig::new(&params, cfg.sim.world.voxel_m),
                cubarium_voxel_fauna::FaunaConfig::default().founder_counts(),
                Seeds::from(a.seed),
                false,
            );
            (f.world, f.flora, f.fauna)
        }
    };
    let mut sim = Sim::new(world, flora, fauna, SimConfig::with_threads(a.threads), None);
    if from.is_none() || a.load.is_some() {
        let started = Instant::now();
        for i in 0..a.warm {
            sim.step();
            if i % 5000 == 0 {
                eprintln!("glow_capture: warm {}", status(&sim));
            }
        }
        if a.until_night {
            let mut n = 0;
            while sim.world().view().weather.daylight > 0.0 && n < a.max_ticks {
                sim.step();
                n += 1;
                if n % 5000 == 0 {
                    eprintln!("glow_capture: waiting for night {}", status(&sim));
                }
            }
        }
        eprintln!("glow_capture: ready {} after {:.1} s", status(&sim), started.elapsed().as_secs_f64());
        if let Some((w, f, b)) = &files {
            std::fs::create_dir_all(a.state.as_ref().unwrap())?;
            std::fs::write(w, sim.world().save())?;
            std::fs::write(f, cubarium_voxel_flora::snapshot::encode(sim.flora()))?;
            std::fs::write(b, sim.fauna().save())?;
        }
    }
    Ok(sim)
}

fn sink(cfg: &VoxelConfig) -> Result<VoxelGpuSink> {
    let proj = Projection::new(cfg.tilt_degrees, cfg.px_per_voxel, cfg.raster_height, &cfg.sim.world)?;
    let models = cubarium::voxel::load_models(cfg);
    let mut sink = VoxelGpuSink::new(cfg, proj, VoxelGpuSinkOptions { target: GpuTargetKind::Headless, models, ..VoxelGpuSinkOptions::default() })?;
    sink.set_weather_preview(None);
    Ok(sink)
}

fn stage_ready(sink: &mut VoxelGpuSink, sim: &Sim, film0: Instant) -> Result<()> {
    sink.set_presenter_clock(Some(film0));
    loop {
        if !sink.stage_world(sim.world(), sim.flora(), sim.fauna()) {
            bail!("the renderer refused a pack");
        }
        if !sink.light_pending() {
            return Ok(());
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
}

type Cell = (u32, u32, u32);

/// A's emitters now, as a map, and the events against `before`.
fn events_of(now: &[(Cell, [f32; 3])], before: &HashMap<Cell, [f32; 3]>) -> (HashMap<Cell, [f32; 3]>, Vec<(Cell, &'static str)>) {
    let map: HashMap<Cell, [f32; 3]> = now.iter().copied().collect();
    let mut ev = Vec::new();
    for (c, rgb) in &map {
        match before.get(c) {
            None => ev.push((*c, "on")),
            Some(b) if b.iter().zip(rgb).any(|(p, q)| (p - q).abs() > 1e-3) => ev.push((*c, "colour")),
            _ => {}
        }
    }
    for c in before.keys() {
        if !map.contains_key(c) {
            ev.push((*c, "off"));
        }
    }
    ev.sort_unstable_by_key(|e| e.0);
    (map, ev)
}

/// A cell's own pixels (front and top face) and the area three voxels round it.
fn rects(proj: &Projection, c: Cell, s: i32) -> ([i32; 4], [i32; 4]) {
    let (col, row, w, h) = proj.front_rect(i64::from(c.0), c.1, c.2);
    let (_, top, _, th) = proj.top_rect(i64::from(c.0), c.1, c.2);
    let own = [col, top, col + w as i32, row + h as i32];
    let _ = th;
    let r = 3 * s;
    (own, [own[0] - r, own[1] - r, own[2] + r, own[3] + r])
}

/// A summed-area table of luminance.
struct Sat {
    w: usize,
    h: usize,
    s: Vec<f64>,
}

impl Sat {
    fn new(rgba: &[u8], w: usize, h: usize) -> Sat {
        let mut s = vec![0.0; (w + 1) * (h + 1)];
        for y in 0..h {
            let mut row = 0.0;
            for x in 0..w {
                let p = &rgba[(y * w + x) * 4..];
                row += 0.2126 * f64::from(p[0]) + 0.7152 * f64::from(p[1]) + 0.0722 * f64::from(p[2]);
                s[(y + 1) * (w + 1) + x + 1] = s[y * (w + 1) + x + 1] + row;
            }
        }
        Sat { w, h, s }
    }

    /// Sum and count over `[x0, x1) × [y0, y1)`, clipped.
    fn sum(&self, r: [i32; 4]) -> (f64, f64) {
        let cl = |v: i32, m: usize| v.clamp(0, m as i32) as usize;
        let (x0, y0, x1, y1) = (cl(r[0], self.w), cl(r[1], self.h), cl(r[2], self.w), cl(r[3], self.h));
        if x1 <= x0 || y1 <= y0 {
            return (0.0, 0.0);
        }
        let at = |x: usize, y: usize| self.s[y * (self.w + 1) + x];
        (at(x1, y1) - at(x0, y1) - at(x1, y0) + at(x0, y0), ((x1 - x0) * (y1 - y0)) as f64)
    }

    fn means(&self, own: [i32; 4], around: [i32; 4]) -> Option<(f64, f64)> {
        let (so, no) = self.sum(own);
        let (sa, na) = self.sum(around);
        (no > 0.0 && na > no).then(|| (so / no, (sa - so) / (na - no)))
    }
}

/// The largest per-frame moves, overall and near an event at the same cell.
#[derive(Default, Clone, Copy)]
struct Steps {
    own: f64,
    around: f64,
    own_event: f64,
    around_event: f64,
    own_at: (u64, Cell),
    own_event_at: (u64, Cell),
}

/// `--nectar-ticks N`: step N ticks undrawn and print, every `--nectar-every`, the open
/// latticevine flowers' nectar fill (their nectar over what a flower opens with,
/// `VineConfig::nectar_per_flower`, as the renderer reads it): how many are open, how many hold any, and the
/// fill's spread; then each flower's own range over the run.
fn nectar_range(a: &Args, sim: &mut Sim) -> Result<()> {
    use cubarium_voxel_flora::SpurPhase;
    let full = sim.flora().config().latticevine.nectar_per_flower();
    let mut seen: HashMap<(u32, u32, u32, u8), (f64, f64)> = HashMap::new();
    let (mut lo_all, mut hi_all) = (f64::MAX, f64::MIN);
    println!("tick,daylight,open,with_nectar,fill_min,fill_p10,fill_median,fill_p90,fill_max");
    for k in 0..=a.nectar_ticks {
        if k % a.nectar_every.max(1) == 0 {
            let mut fills: Vec<f64> = Vec::new();
            let mut open = 0;
            for c in sim.flora().view().cover.iter() {
                if c.spur != SpurPhase::Flower {
                    continue;
                }
                open += 1;
                let f = if full > 0.0 { (c.nectar / full).clamp(0.0, 1.0) } else { 1.0 };
                fills.push(f);
                let key = (c.face.x, c.face.y, c.face.z, c.face.dir as u8);
                let e = seen.entry(key).or_insert((f, f));
                e.0 = e.0.min(f);
                e.1 = e.1.max(f);
                lo_all = lo_all.min(f);
                hi_all = hi_all.max(f);
            }
            fills.sort_by(f64::total_cmp);
            let q = |p: f64| fills.get(((fills.len() as f64 - 1.0) * p).round().max(0.0) as usize).copied().unwrap_or(f64::NAN);
            println!(
                "{},{:.2},{open},{},{:.3},{:.3},{:.3},{:.3},{:.3}",
                sim.world().tick(),
                sim.world().view().weather.daylight,
                fills.iter().filter(|&&f| f > 0.0).count(),
                q(0.0),
                q(0.1),
                q(0.5),
                q(0.9),
                q(1.0)
            );
        }
        sim.step();
    }
    let spans: Vec<f64> = seen.values().map(|(lo, hi)| hi - lo).collect();
    let moved = spans.iter().filter(|&&d| d > 0.05).count();
    eprintln!(
        "glow_capture: nectar fill over {} ticks: {} flowers seen open, fill {lo_all:.3}..{hi_all:.3};          {moved} of them moved more than 0.05 while open, the largest by {:.3}",
        a.nectar_ticks,
        seen.len(),
        spans.iter().copied().fold(0.0, f64::max)
    );
    Ok(())
}

fn ffmpeg(path: &PathBuf, w: u32, h: u32, fps: f64) -> Result<Child> {
    Command::new("ffmpeg")
        .args(["-y", "-loglevel", "error", "-f", "rawvideo", "-pix_fmt", "rgba", "-s"])
        .arg(format!("{w}x{h}"))
        .args(["-r"])
        .arg(format!("{fps}"))
        .args(["-i", "-", "-c:v", "libx264", "-pix_fmt", "yuv444p", "-crf", "12", "-preset", "fast"])
        .arg(path)
        .stdin(Stdio::piped())
        .spawn()
        .context("starting ffmpeg")
}

fn main() -> Result<()> {
    let a = args()?;
    let (cfg_a, cfg_b) = (config(&a, &a.a)?, config(&a, &a.b)?);
    let tick_hz = f64::from(cubarium_voxel::TICK_HZ);
    let frames = (a.seconds * a.fps).round() as u64;
    let proj = Projection::new(cfg_a.tilt_degrees, cfg_a.px_per_voxel, cfg_a.raster_height, &cfg_a.sim.world)?;
    let s = a.px as i32;
    // Both renderers' film clock (the foliage fades'): the same instant for both from
    // their first pack on, so their plants fade alike.
    let film0 = Instant::now();

    // The close-up's window: the most events in a scan, else the most emitters now.
    let mut sim = load_or_found(&a, &cfg_a)?;
    let mut a = a;
    a.flora_at.sort_by(|x, y| x.0.total_cmp(&y.0));
    let a = a;
    if a.nectar_ticks > 0 {
        return nectar_range(&a, &mut sim);
    }
    let mut sa = sink(&cfg_a)?;
    stage_ready(&mut sa, &sim, film0)?;
    let p = sa.params();
    let (rw, rh) = (p.raster_w, p.raster_h);
    let mut points: Vec<(i32, i32)> = sa.emitters().iter().map(|&(c, _)| {
        let (col, row, _, _) = proj.front_rect(i64::from(c.0), c.1, c.2);
        (col, row)
    }).collect();
    if a.scan {
        let started = Instant::now();
        let tick0 = sim.world().tick();
        // `--colour` scans with B, whose swaps are the events.
        if a.colour {
            drop(sa);
            sa = sink(&cfg_b)?;
            stage_ready(&mut sa, &sim, film0)?;
        }
        let mut before: HashMap<Cell, [f32; 3]> = sa.emit_targets().into_iter().collect();
        let mut ev_points = Vec::new();
        let want = tick0 + (a.seconds * a.speed * tick_hz).floor() as u64;
        let mut next = 0;
        while sim.world().tick() < want {
            let clip_s = (sim.world().tick() - tick0) as f64 / (a.speed * tick_hz);
            flora_due(&a, &mut sim, clip_s, &mut next)?;
            sim.step();
            sa.set_clock(sim.world().tick(), 0.0);
            sa.stage_world(sim.world(), sim.flora(), sim.fauna());
            let ev: Vec<(Cell, &str)> = if a.colour {
                sa.colour_switches().into_iter().map(|c| (c, "colour")).collect()
            } else {
                let (map, ev) = events_of(&sa.emit_targets(), &before);
                before = map;
                ev
            };
            ev_points.extend(ev.iter().map(|&(c, _)| {
                let (col, row, _, _) = proj.front_rect(i64::from(c.0), c.1, c.2);
                (col, row)
            }));
        }
        eprintln!("glow_capture: scan {} events in {:.1} s", ev_points.len(), started.elapsed().as_secs_f64());
        if !ev_points.is_empty() {
            points = ev_points;
        }
        drop(sa);
        let dir = a.state.clone().or(a.load.clone()).context("--scan replays the clip from --state or --load")?;
        let saved = cubarium_voxel_sim::saved::load(&dir, &[]).map_err(anyhow::Error::msg)?;
        sim = Sim::new(saved.world, saved.flora, saved.fauna, SimConfig::with_threads(a.threads), None);
        sa = sink(&cfg_a)?;
        stage_ready(&mut sa, &sim, film0)?;
    }
    let (cw, ch) = (a.close.0.min(rw), a.close.1.min(rh));
    let mut best = (0, 0, 0usize);
    for y0 in (0..=rh - ch).step_by(8) {
        for x0 in (0..=rw - cw).step_by(8) {
            let n = points
                .iter()
                .filter(|&&(x, y)| x >= x0 as i32 && x < (x0 + cw) as i32 && y >= y0 as i32 && y < (y0 + ch) as i32)
                .count();
            if n > best.2 {
                best = (x0, y0, n);
            }
        }
    }
    let (cx, cy) = (best.0, best.1);
    eprintln!("glow_capture: raster {rw}x{rh}, close-up {cw}x{ch} at ({cx}, {cy}) holding {} points", best.2);

    // A again, beside B, so both start from the same first pack.
    drop(sa);
    let mut sa = sink(&cfg_a)?;
    let mut sb = sink(&cfg_b)?;
    stage_ready(&mut sa, &sim, film0)?;
    stage_ready(&mut sb, &sim, film0)?;
    let mut full = match &a.video {
        Some(p) => Some(ffmpeg(p, rw * 2 + 8, rh, a.fps)?),
        None => None,
    };
    let z = a.zoom.max(1);
    let mut close = match &a.close_video {
        Some(p) => Some(ffmpeg(p, (cw * z) * 2 + 8, ch * z, a.fps)?),
        None => None,
    };
    let mut log = match &a.log {
        Some(path) => {
            let mut f = std::io::BufWriter::new(std::fs::File::create(path)?);
            writeln!(f, "frame,video_s,tick,events,cells,a_own,a_around,b_own,b_around,a_ms,b_ms")?;
            Some(f)
        }
        None => None,
    };
    let mut events_out = match &a.events {
        Some(path) => {
            let mut f = std::io::BufWriter::new(std::fs::File::create(path)?);
            writeln!(f, "tick,video_s,x,y,z,kind,px,py")?;
            Some(f)
        }
        None => None,
    };

    let tick0 = sim.world().tick();
    let mut before: HashMap<Cell, [f32; 3]> = sa.emit_targets().into_iter().collect();
    let mut tracked: Vec<Cell> = before.keys().copied().collect();
    let mut tracked_set: HashSet<Cell> = tracked.iter().copied().collect();
    let mut last_event: HashMap<Cell, u64> = HashMap::new();
    let mut prev: [HashMap<Cell, (f64, f64)>; 2] = [HashMap::new(), HashMap::new()];
    let mut steps = [Steps::default(); 2];
    let (mut ms, mut n_events) = ([0.0f64; 2], 0usize);
    let fade_s = if a.colour { cfg_b.light.colour_fade_s } else { cfg_b.light.emit_fade_s };
    let near = (2.0 * a.speed * tick_hz).ceil() as u64 + (f64::from(fade_s) * a.speed * tick_hz).ceil() as u64;
    if a.colour {
        tracked.clear();
        tracked_set.clear();
    }
    let mut next = 0;
    for k in 0..frames {
        let video_s = k as f64 / a.fps;
        let t = video_s * a.speed * tick_hz;
        let want = tick0 + t.floor() as u64;
        let mut frame_events = 0;
        while sim.world().tick() < want {
            let clip_s = (sim.world().tick() - tick0) as f64 / (a.speed * tick_hz);
            flora_due(&a, &mut sim, clip_s, &mut next)?;
            sim.step();
            let tick = sim.world().tick();
            for sk in [&mut sa, &mut sb] {
                sk.set_clock(tick, 0.0);
                sk.stage_world(sim.world(), sim.flora(), sim.fauna());
            }
            let (map, ev) = if a.colour {
                (HashMap::new(), sb.colour_switches().into_iter().map(|c| (c, "colour")).collect())
            } else {
                events_of(&sa.emit_targets(), &before)
            };
            for &(c, kind) in &ev {
                last_event.insert(c, tick);
                if tracked_set.insert(c) {
                    tracked.push(c);
                }
                if let Some(f) = &mut events_out {
                    let (col, row, _, _) = proj.front_rect(i64::from(c.0), c.1, c.2);
                    writeln!(f, "{tick},{video_s:.3},{},{},{},{kind},{col},{row}", c.0, c.1, c.2)?;
                }
            }
            if !a.colour {
                for &(c, _) in sb.emitters() {
                    if tracked_set.insert(c) {
                        tracked.push(c);
                    }
                }
            }
            frame_events += ev.len();
            before = map;
        }
        n_events += frame_events;
        let mut pics = Vec::with_capacity(2);
        for (j, sk) in [&mut sa, &mut sb].into_iter().enumerate() {
            sk.set_presenter_clock(Some(film0 + std::time::Duration::from_secs_f64(video_s)));
            sk.set_clock(want, t.fract());
            let at = Instant::now();
            sk.render()?;
            let rgba = sk.read_raster()?;
            ms[j] += at.elapsed().as_secs_f64() * 1e3;
            pics.push(rgba);
        }
        let tick = sim.world().tick();
        let mut worst = [(0.0f64, 0.0f64); 2];
        for (j, rgba) in pics.iter().enumerate() {
            let sat = Sat::new(rgba, rw as usize, rh as usize);
            for &c in &tracked {
                let (own, around) = rects(&proj, c, s);
                let Some((mo, ma)) = sat.means(own, around) else { continue };
                if let Some(&(po, pa)) = prev[j].get(&c) {
                    let (dot, da) = ((mo - po).abs(), (ma - pa).abs());
                    let st = &mut steps[j];
                    if dot > st.own {
                        st.own = dot;
                        st.own_at = (k, c);
                    }
                    st.around = st.around.max(da);
                    if last_event.get(&c).is_some_and(|&e| tick.saturating_sub(e) <= near) {
                        if dot > st.own_event {
                            st.own_event = dot;
                            st.own_event_at = (k, c);
                        }
                        st.around_event = st.around_event.max(da);
                    }
                    worst[j].0 = worst[j].0.max(dot);
                    worst[j].1 = worst[j].1.max(da);
                }
                prev[j].insert(c, (mo, ma));
            }
        }
        if let Some(f) = &mut log {
            writeln!(
                f,
                "{k},{video_s:.4},{tick},{frame_events},{},{:.2},{:.2},{:.2},{:.2},{:.2},{:.2}",
                tracked.len(),
                worst[0].0,
                worst[0].1,
                worst[1].0,
                worst[1].1,
                ms[0] / (k + 1) as f64,
                ms[1] / (k + 1) as f64
            )?;
        }
        if let Some(ff) = &mut full {
            let mut out = vec![0u8; ((rw * 2 + 8) * rh * 4) as usize];
            for y in 0..rh as usize {
                let row = y * (rw as usize * 2 + 8) * 4;
                let src = y * rw as usize * 4;
                out[row..row + rw as usize * 4].copy_from_slice(&pics[0][src..src + rw as usize * 4]);
                let off = row + (rw as usize + 8) * 4;
                out[off..off + rw as usize * 4].copy_from_slice(&pics[1][src..src + rw as usize * 4]);
            }
            ff.stdin.as_mut().context("ffmpeg stdin")?.write_all(&out)?;
        }
        if let Some(ff) = &mut close {
            let ow = (cw * z * 2 + 8) as usize;
            let mut out = vec![0u8; ow * (ch * z) as usize * 4];
            for (j, pic) in pics.iter().enumerate() {
                let x_off = j * ((cw * z) as usize + 8);
                for y in 0..(ch * z) as usize {
                    let sy = cy as usize + y / z as usize;
                    for x in 0..(cw * z) as usize {
                        let sx = cx as usize + x / z as usize;
                        let si = (sy * rw as usize + sx) * 4;
                        let di = (y * ow + x_off + x) * 4;
                        out[di..di + 4].copy_from_slice(&pic[si..si + 4]);
                    }
                }
            }
            ff.stdin.as_mut().context("ffmpeg stdin")?.write_all(&out)?;
        }
    }
    for ff in [full, close].into_iter().flatten() {
        let mut ff = ff;
        drop(ff.stdin.take());
        ff.wait()?;
    }
    for (j, name) in ["A", "B"].iter().enumerate() {
        let st = steps[j];
        eprintln!(
            "glow_capture: {name}: largest frame-to-frame move of an emitter's own pixels {:.2} (frame {}, cell {:?}), \
             of the area round one {:.2}; within {} ticks of an event at that cell: own {:.2} (frame {}, cell {:?}), around {:.2}; \
             mean draw+readback {:.2} ms",
            st.own,
            st.own_at.0,
            st.own_at.1,
            st.around,
            near,
            st.own_event,
            st.own_event_at.0,
            st.own_event_at.1,
            st.around_event,
            ms[j] / frames.max(1) as f64
        );
    }
    eprintln!("glow_capture: {frames} frames, {n_events} events, {} cells tracked; end {}", tracked.len(), status(&sim));
    Ok(())
}
