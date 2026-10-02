//! **Why do browsers get stuck in crags?** Measurement only (diagnosis,
//! `design/handoffs/browser-stuck-in-crags-2026-10-01.md`).
//!
//! ```text
//! crag_probe brain=<centre|heuristic> shredder=<centre|heuristic> [params=<genome or resolved file>]
//!            [seed=1] [minutes=60] [world=config/desktop/terrarium.toml]
//!            [window_min=5] [disp_bl=0.5] [effort=0.25] [small_m2=16] [t0_only=1]
//! ```
//!
//! Founds one world as `starve_probe` does, with the ecology of `params=` applied the way
//! the live app's `--params` applies it (the terrarium config has no `[flora]`/`[fauna]`
//! tables, so both are the defaults with the genome decoded over them).
//!
//! - **t = 0:** each lineage's walkable components under the seeder's own column rule
//!   ([`route_components`] on the adult body), and how many founders stand outside the
//!   largest; for browsers also each founder's **reach** under the disc replica below.
//! - **Stuck:** a body whose position stayed within `disp_bl` body lengths of where it was
//!   `window_min` minutes ago while its mean motor effort, `(|v| + r·|ω|) / cruise` as the
//!   tick charges it, stayed at or over `effort`.
//! - **Each stuck browser:** its reach from where it stands now. Under `small_m2` it is
//!   trapped: founded there (H1) or cut off later (H2: water, growth, or the step it
//!   entered by). Otherwise it can get out: H4 when it spent most of the window pressing
//!   forward into something that stopped it, H3 when it did not.
//!
//! **The disc replica.** A browser's footprint is a disc of radius `width/2` = 1.5 voxels
//! and `step::resolve_motion` checks the disc's clearance over every column it overlaps, so
//! the column rule (one voxel wide) joins corridors the body cannot pass. The replica
//! walks a half-voxel lattice of centre positions with `body::advance_candidate`'s own
//! test, re-implemented here from public `VoxelView` queries (the original is private):
//! the centre column's nearest support within the step, the disc's clearance over the
//! highest within-step ground under it, the wade rule and its shallower-water escape. It
//! is directed (a move is judged from the layer it starts on). Climbing is not
//! replicated, so shredders are judged by the column rule with their wall links.

use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::{Arc, Mutex};

use cubarium_search::es::voxel::live::{self, ecology::EcologyParams, search, window, world};
use cubarium_voxel::VoxelView;
use cubarium_voxel_fauna::{
    Body, Controller, Founder, Response, RouteRule, State, StepLimits, route_components,
    standable_faces,
};

// ---------------------------------------------------------------- diagnostic thresholds
// A diagnostic tool's own thresholds, not simulation config: each is an argument too.

/// The stuck window, minutes (`window_min=`).
const WINDOW_MIN: f64 = 5.0;
/// Stuck when the body stayed within this many body lengths of its position at the
/// window's start (`disp_bl=`).
const STUCK_DISP_BL: f64 = 0.5;
/// ... while its mean motor effort over the window was at least this (`effort=`).
const EFFORT_MIN: f64 = 0.25;
/// Not stuck but grazing when it fed on at least this share of its controller intervals
/// (a feed lands on one tick per interval, so `Cropping` ticks × cadence / ticks).
const CROPPING_MAX: f64 = 0.25;
/// A reach under this many m² is a trap (`small_m2=`).
const SMALL_REACH_M2: f64 = 16.0;
/// How often stuck is judged, seconds.
const CHECK_S: u64 = 30;
/// The pose history kept for finding the entry into a trap, seconds per sample.
const HIST_S: u64 = 10;
/// A tick counts as pressing when forward was asked (over this) ...
const PRESS_ASK: f64 = 0.05;
/// ... and less than this share of it was delivered.
const PRESS_DELIVERED: f64 = 0.1;
/// H4 when the stuck window was pressing for at least this share of its ticks.
const PRESS_SHARE_H4: f64 = 0.5;

fn arg(args: &[String], key: &str) -> Option<String> {
    args.iter().find_map(|a| a.strip_prefix(key).map(str::to_string))
}
fn argf(args: &[String], key: &str, d: f64) -> f64 {
    arg(args, key).and_then(|s| s.parse().ok()).unwrap_or(d)
}

// ---------------------------------------------------------------- the sense recorder

/// Per-sample senses: contact front/left/right, cone clear s0/s1/s2, cone all-proximity
/// mean, cone valid, cone foliage fraction mean, cone body fraction mean, wet response.
type Sense = [f32; 12];
type SenseLog = Arc<Mutex<HashMap<u64, VecDeque<Sense>>>>;

struct Recorder {
    id: u64,
    founder: Founder,
    inner: Box<dyn Controller>,
    log: SenseLog,
    keep: usize,
}

impl Controller for Recorder {
    fn drive(&mut self, o: &[f64]) -> Response {
        let mut s: Sense = [0.0; 12];
        s[10] = o[13] as f32;
        s[0] = o[8] as f32;
        s[1] = o[9] as f32;
        s[2] = o[10] as f32;
        if self.founder == Founder::Browser {
            s[3] = o[18] as f32;
            s[4] = o[24] as f32;
            s[5] = o[30] as f32;
            s[6] = ((o[19] + o[25] + o[31]) / 3.0) as f32;
            s[7] = o[36] as f32;
            s[8] = ((o[20] + o[26] + o[32]) / 3.0) as f32;
            s[9] = ((o[22] + o[28] + o[34]) / 3.0) as f32;
        }
        {
            let mut log = self.log.lock().unwrap();
            let q = log.entry(self.id).or_default();
            q.push_back(s);
            if q.len() > self.keep {
                q.pop_front();
            }
        }
        self.inner.drive(o)
    }
    fn reset(&mut self) {
        self.inner.reset();
    }
}

// ---------------------------------------------------------------- the disc replica

#[derive(Clone, Copy)]
struct Rules {
    r: f64,
    headroom: u32,
    wade: f64,
    step: StepLimits,
}

impl Rules {
    fn of(phys: &cubarium_voxel_fauna::FounderPhysiology, body: &Body, voxel_m: f64) -> Rules {
        Rules {
            r: body.footprint_radius(),
            headroom: body.headroom_voxels(voxel_m),
            wade: phys.wade_depth_m(body),
            step: phys.step_limits(voxel_m),
        }
    }
}

fn disc_box(v: f64, cx: f64, cz: f64, r: f64) -> (i64, i64, i64, i64) {
    (
        ((cx - r) / v).floor() as i64,
        ((cx + r) / v).floor() as i64,
        ((cz - r) / v).floor() as i64,
        ((cz + r) / v).floor() as i64,
    )
}

fn overlaps(v: f64, x: i64, z: i64, cx: f64, cz: f64, r: f64) -> bool {
    let qx = cx.clamp(x as f64 * v, (x as f64 + 1.0) * v);
    let qz = cz.clamp(z as f64 * v, (z as f64 + 1.0) * v);
    let (dx, dz) = (cx - qx, cz - qz);
    dx * dx + dz * dz < r * r
}

/// `body::advance_candidate`'s acceptance of a centre at `(nx, nz)` for a body standing on
/// `y` at `(px, pz)`: the layer it would stand on, or `None`. A centre outside the strip's
/// band is clamped into it, as the live sweep does.
fn accept(view: &VoxelView<'_>, rules: &Rules, from: (f64, f64, u32), nx: f64, nz: f64) -> Option<u32> {
    accept_why(view, rules, from, nx, nz).ok()
}

/// [`accept`], with the test that refused.
fn accept_why(view: &VoxelView<'_>, rules: &Rules, (px, pz, y): (f64, f64, u32), nx: f64, nz: f64) -> Result<u32, &'static str> {
    let c = view.config;
    let v = c.voxel_m;
    let width_m = f64::from(c.width) * v;
    let depth_m = f64::from(c.depth) * v;
    let nx = nx.rem_euclid(width_m);
    // The strip's z ends are walls: the live sweep clamps the centre into the band.
    let nz = nz.clamp(rules.r, depth_m - rules.r);
    let cz = (nz / v).floor();
    if cz < 0.0 || cz >= f64::from(c.depth) {
        return Err("off the strip");
    }
    let cz = cz as u32;
    let cx = ((nx / v).floor() as i64).rem_euclid(i64::from(c.width));
    let (down, up) = (rules.step.down, rules.step.up);
    let ny = view.nearest_support(cx, cz, y, down, up).ok_or("no support within step")?;
    let (x0, x1, z0, z1) = disc_box(v, nx, nz, rules.r);
    let mut ground = y;
    if up > 0 {
        for x in x0..=x1 {
            for z in z0..=z1 {
                if z < 0 || z >= i64::from(c.depth) || !overlaps(v, x, z, nx, nz, rules.r) {
                    continue;
                }
                if let Some(g) = view.nearest_support(x, z as u32, y, down, up) {
                    ground = ground.max(g);
                }
            }
        }
    }
    let clearance = ground.max(y).max(ny);
    let (lo, hi) = (clearance + 1, clearance + rules.headroom);
    if rules.headroom > 0 && lo < c.height {
        for x in x0..=x1 {
            for z in z0..=z1 {
                if z < 0 || z >= i64::from(c.depth) {
                    continue;
                }
                if view.solid_in(x, z as u32, lo, hi) && overlaps(v, x, z, nx, nz, rules.r) {
                    return Err("disc hits solid");
                }
            }
        }
    }
    let there = view.water_depth_m(cx, ny, cz);
    if there > rules.wade {
        let hz = ((pz / v).floor().max(0.0) as u32).min(c.depth.saturating_sub(1));
        let hx = ((px / v).floor() as i64).rem_euclid(i64::from(c.width));
        let same_face = (hx, hz) == (cx, cz) && ny == y;
        if !same_face && !(there < view.water_depth_m(hx, y, hz)) {
            return Err("water over wade");
        }
    }
    Ok(ny)
}

/// How many of 72 headings the body could move along this tick: any of the sub-step
/// lengths the live sweep and its bisection try (the full `d`, then halves down to
/// 1/64 of it) is accepted. Zero is **frozen**: only turning is left.
fn open_headings(view: &VoxelView<'_>, rules: &Rules, px: f64, pz: f64, y: u32, d: f64) -> usize {
    (0..72)
        .filter(|k| {
            let h = std::f64::consts::TAU * f64::from(*k) / 72.0;
            let (fx, fz) = (h.sin(), h.cos());
            (0..7).any(|j| {
                let s = d / f64::from(1u32 << j);
                accept(view, rules, (px, pz, y), px + fx * s, pz + fz * s).is_some()
            })
        })
        .count()
}

/// One replica reach: the faces `(x, y, z)` under every lattice centre the body can get to.
struct Reach {
    faces: HashSet<(u32, u32, u32)>,
    started: bool,
}

impl Reach {
    fn area(&self, v: f64) -> f64 {
        self.faces.len() as f64 * v * v
    }
}

/// The directed reach from a pose, on a lattice of `v / 2`.
fn reach(view: &VoxelView<'_>, rules: &Rules, px: f64, pz: f64, y: u32) -> Reach {
    let c = view.config;
    let v = c.voxel_m;
    let h = v / 2.0;
    let ni = i64::from(c.width) * 2;
    let nk = i64::from(c.depth) * 2;
    let pos = |i: i64, k: i64| ((i as f64 + 0.5) * h, (k as f64 + 0.5) * h);
    let face = |x: f64, z: f64, y: u32| {
        (
            ((x / v).floor() as i64).rem_euclid(i64::from(c.width)) as u32,
            y,
            (z / v).floor() as u32,
        )
    };
    let mut seen: HashSet<(i64, i64, u32)> = HashSet::new();
    let mut queue: VecDeque<(i64, i64, u32)> = VecDeque::new();
    // Start at the lattice centres around the body's own pose, nearest first.
    let (i0, k0) = ((px / h - 0.5).floor() as i64, (pz / h - 0.5).floor() as i64);
    let mut starts: Vec<(f64, i64, i64)> = Vec::new();
    for di in -1..=2 {
        for dk in -1..=2 {
            let (i, k) = ((i0 + di).rem_euclid(ni), k0 + dk);
            if k < 0 || k >= nk {
                continue;
            }
            let (x, z) = pos(i, k);
            let mut dx = (x - px).abs();
            dx = dx.min(f64::from(c.width) * v - dx);
            starts.push((dx * dx + (z - pz) * (z - pz), i, k));
        }
    }
    starts.sort_by(|a, b| a.0.total_cmp(&b.0));
    for (_, i, k) in starts {
        let (x, z) = pos(i, k);
        if let Some(ny) = accept(view, rules, (px, pz, y), x, z) {
            seen.insert((i, k, ny));
            queue.push_back((i, k, ny));
            break;
        }
    }
    let started = !queue.is_empty();
    let mut faces = HashSet::new();
    faces.insert(face(px, pz, y));
    while let Some((i, k, yy)) = queue.pop_front() {
        let (x, z) = pos(i, k);
        faces.insert(face(x, z, yy));
        for (di, dk) in [(1, 0), (-1, 0), (0, 1), (0, -1), (1, 1), (1, -1), (-1, 1), (-1, -1)] {
            let (j, l) = ((i + di).rem_euclid(ni), k + dk);
            if l < 0 || l >= nk {
                continue;
            }
            let (nx, nz) = pos(j, l);
            if let Some(ny) = accept(view, rules, (x, z, yy), nx, nz) {
                if seen.insert((j, l, ny)) {
                    queue.push_back((j, l, ny));
                }
            }
        }
    }
    Reach { faces, started }
}

// ---------------------------------------------------------------- per-body tracking

#[derive(Default)]
struct Bin {
    ticks: f64,
    effort: f64,
    fwd: f64,
    turn: f64,
    pressed: f64,
    asleep: f64,
    cropping: f64,
}

struct Track {
    founder: Founder,
    founded: bool,
    born_tick: u64,
    born_body: f64,
    start: (f64, f64, u32),
    t0_reach_m2: Option<f64>,
    t0_faces: Option<HashSet<(u32, u32, u32)>>,
    /// 1 Hz pose samples over the window.
    pos: VecDeque<(f64, f64)>,
    bins: VecDeque<Bin>,
    cur: Bin,
    hist: Vec<(u64, f64, f64, u32)>,
    ledges: Vec<(u64, u32, u32)>,
    last: (f64, f64, u32),
    stuck_since: Option<u64>,
    stuck_s: u64,
    t0_open: usize,
    /// The tick and body (organic) of the last tick it moved at all.
    last_move: (u64, f64),
    episodes: u32,
    left_t0: bool,
    /// Ever still and spinning while feeding on most intervals (not counted as stuck).
    grazing: bool,
}

fn ring_dx(a: f64, b: f64, w: f64) -> f64 {
    let d = (a - b).rem_euclid(w);
    d.min(w - d)
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let seed: u64 = arg(&args, "seed=").and_then(|s| s.parse().ok()).unwrap_or(1);
    let minutes: u64 = arg(&args, "minutes=").and_then(|s| s.parse().ok()).unwrap_or(60);
    let world_file = arg(&args, "world=").unwrap_or("config/desktop/terrarium.toml".into());
    let window_min = argf(&args, "window_min=", WINDOW_MIN);
    let disp_bl = argf(&args, "disp_bl=", STUCK_DISP_BL);
    let effort_min = argf(&args, "effort=", EFFORT_MIN);
    let small_m2 = argf(&args, "small_m2=", SMALL_REACH_M2);
    let t0_only = arg(&args, "t0_only=").is_some_and(|s| s == "1");
    let brains = [
        (Founder::Browser, arg(&args, "brain=").expect("brain=")),
        (Founder::Blind, arg(&args, "shredder=").expect("shredder=")),
    ];
    let ecology = match arg(&args, "params=") {
        Some(p) => EcologyParams::load(std::path::Path::new(&p)).expect("params"),
        None => EcologyParams::defaults(),
    };
    let cfg = search::check_world_config(Some(std::path::Path::new(&world_file))).expect("world");
    let (mut sim, info) =
        world::found_world(
            &cfg,
            seed,
            cubarium_voxel_fauna::FaunaConfig::default().founder_counts(),
            &ecology,
        );
    let drivers: Vec<_> = brains
        .iter()
        .map(|(f, b)| (*f, search::brain_driver(*f, b).expect("a brain")))
        .collect();
    search::check_mass(&drivers, &ecology).expect("mass");
    live::install_centres(sim.fauna_mut(), &drivers);
    let teachers = sim.fauna().config().teachers;
    let cfgf = sim.fauna().config().clone();
    let v = cfg.voxel_m;
    let width_m = f64::from(cfg.width) * v;
    let tick_hz = cubarium_voxel::TICK_HZ as u64;
    let window_s = (window_min * 60.0).round() as usize;
    let senses: SenseLog = Arc::new(Mutex::new(HashMap::new()));
    // Controller samples per window (the senses update every UPDATE_TICKS).
    let keep = (window_s as u64 * tick_hz / cubarium_voxel_fauna::UPDATE_TICKS.max(1) as u64) as usize + 4;
    println!(
        "# seed {} world {} accepted {} founders {:?} brains {:?} ecology {} | stuck: < {disp_bl} BL in {window_min} min at effort >= {effort_min}; trap < {small_m2} m2",
        seed,
        info.seed,
        info.accepted,
        info.founders,
        brains.iter().map(|b| &b.1).collect::<Vec<_>>(),
        if ecology.is_shipped() { "shipped".to_string() } else { arg(&args, "params=").unwrap_or_default() }
    );
    for f in [Founder::Browser, Founder::Blind] {
        let p = cfgf.founder(f);
        let b = p.adult_body();
        let m = cfgf.manifest(f);
        println!(
            "# {}: adult L {:.3} W {:.3} H {:.3} m; disc r {:.4} m = {:.2} vox; headroom {} vox; step up/down {:?}; wade {:.3} m; climbs {}; cruise {:.3} m/s yaw {:.2} rad/s",
            f.name(),
            b.length_m,
            b.width_m,
            b.height_m,
            b.footprint_radius(),
            b.footprint_radius() / v,
            b.headroom_voxels(v),
            (p.step_limits(v).up, p.step_limits(v).down),
            p.wade_depth_m(&b),
            p.climbs_walls,
            m.cruise_m_per_s,
            m.yaw_cap_rad_per_s
        );
    }

    let mut tracks: HashMap<u64, Track> = HashMap::new();
    let wrap = |sim: &mut cubarium_voxel_sim::Sim, id: u64, f: Founder, senses: &SenseLog| {
        if let Some((_, d)) = drivers.iter().find(|(g, _)| *g == f) {
            let rec = Recorder {
                id,
                founder: f,
                inner: d.fresh(teachers),
                log: Arc::clone(senses),
                keep,
            };
            sim.fauna_mut().set_controller(id, Box::new(rec));
        }
    };

    // ------------------------------------------------------------ t = 0
    {
        let wv = sim.world().view();
        let fv = sim.fauna().view();
        for f in [Founder::Browser, Founder::Blind] {
            let p = cfgf.founder(f);
            let body = p.adult_body();
            let faces = standable_faces(&wv, &body, p.wade_depth_m(&body));
            let comps = route_components(
                &wv,
                &faces,
                RouteRule { step: p.step_limits(v), climbs_walls: p.climbs_walls },
                body.headroom_voxels(v),
            );
            let mut size: HashMap<usize, usize> = HashMap::new();
            for c in &comps {
                *size.entry(*c).or_default() += 1;
            }
            let (big, big_n) = size.iter().max_by_key(|e| *e.1).map(|(a, b)| (*a, *b)).unwrap_or((0, 0));
            let index: HashMap<(u32, u32, u32), usize> =
                faces.iter().enumerate().map(|(i, s)| ((s.x, s.y, s.z), i)).collect();
            let mut outside = Vec::new();
            let mut n = 0;
            for a in fv.animals.iter().filter(|a| a.founder == Some(f)) {
                n += 1;
                match index.get(&(a.site.x, a.site.y, a.site.z)) {
                    Some(&i) if comps[i] == big => {}
                    Some(&i) => outside.push(format!("#{} comp {:.1} m2", a.id, size[&comps[i]] as f64 * v * v)),
                    None => outside.push(format!("#{} not on a standable face", a.id)),
                }
            }
            println!(
                "t0 {} column rule: {} components, largest {:.1} m2 of {:.1} m2 standable; founders outside it {}/{} {:?}",
                f.name(),
                size.len(),
                big_n as f64 * v * v,
                faces.len() as f64 * v * v,
                outside.len(),
                n,
                outside
            );
        }
        // The disc replica for browsers.
        let p = cfgf.founder(Founder::Browser);
        let rules = Rules::of(p, &p.adult_body(), v);
        let mut areas = Vec::new();
        for a in fv.animals.iter() {
            let Some(f) = a.founder else { continue };
            if f == Founder::Lanternjaw {
                continue;
            }
            let start = (a.pose.x, a.pose.z, a.site.y);
            let mut t = Track {
                founder: f,
                founded: true,
                born_tick: 0,
                born_body: a.body,
                start,
                t0_reach_m2: None,
                t0_faces: None,
                pos: VecDeque::new(),
                bins: VecDeque::new(),
                cur: Bin::default(),
                hist: vec![(0, a.pose.x, a.pose.z, a.site.y)],
                ledges: Vec::new(),
                last: start,
                stuck_since: None,
                stuck_s: 0,
                t0_open: 72,
                last_move: (0, a.body),
                episodes: 0,
                left_t0: false,
                grazing: false,
            };
            if f == Founder::Browser {
                let rules = Rules::of(p, &p.body_at(a.body), v);
                let r = reach(&wv, &rules, a.pose.x, a.pose.z, a.site.y);
                let m2 = r.area(v);
                let open = open_headings(&wv, &rules, a.pose.x, a.pose.z, a.site.y, cfgf.manifest(f).cruise_m_per_s * cubarium_voxel::DT);
                t.t0_open = open;
                if !r.started || open == 0 {
                    println!(
                        "  t0 #{} at {:?} pose ({:.3}, {:.3}): open headings {open}/72; own pose: {:?}",
                        a.id,
                        (a.site.x, a.site.y, a.site.z),
                        a.pose.x,
                        a.pose.z,
                        accept_why(&wv, &rules, start, a.pose.x, a.pose.z)
                    );
                }
                areas.push((a.id, m2, r.started));
                t.t0_reach_m2 = Some(m2);
                t.t0_faces = Some(r.faces);
            }
            tracks.insert(a.id, t);
        }
        let _ = rules;
        let frozen = tracks.values().filter(|t| t.founder == Founder::Browser && t.t0_open == 0).count();
        println!("t0 frondgrazer founders frozen (0/72 headings open at founding): {frozen}/{}", areas.len());
        let max = areas.iter().map(|a| a.1).fold(0.0, f64::max);
        let small: Vec<String> = areas
            .iter()
            .filter(|a| a.1 < small_m2 || !a.2)
            .map(|a| format!("#{} {:.1} m2{}", a.0, a.1, if a.2 { "" } else { " (no valid start)" }))
            .collect();
        let mut sorted: Vec<f64> = areas.iter().map(|a| a.1).collect();
        sorted.sort_by(f64::total_cmp);
        println!(
            "t0 frondgrazer disc replica: reach m2 per founder (sorted) {:?}; largest {:.1}; founders under {small_m2} m2: {}/{} {:?}",
            sorted.iter().map(|a| (a * 10.0).round() / 10.0).collect::<Vec<_>>(),
            max,
            small.len(),
            areas.len(),
            small
        );
    }
    if t0_only {
        return;
    }
    let ids: Vec<(u64, Founder)> = tracks.iter().map(|(id, t)| (*id, t.founder)).collect();
    for (id, f) in ids {
        wrap(&mut sim, id, f, &senses);
    }

    // ------------------------------------------------------------ the run
    let ticks = minutes * window::TICKS_PER_MIN;
    let mut verdicts: Vec<(Founder, &'static str)> = Vec::new();
    let mut stuck_bodies: HashSet<u64> = HashSet::new();
    let mut left_t0 = 0usize;
    let mut grazers: HashSet<u64> = HashSet::new();
    for t in 1..=ticks {
        sim.step();
        // New bodies: a recorder over a fresh controller of their lineage.
        let fresh: Vec<(u64, Founder, f64, (f64, f64, u32))> = sim
            .fauna()
            .view()
            .animals
            .iter()
            .filter_map(|a| {
                let f = a.founder?;
                (f != Founder::Lanternjaw && !tracks.contains_key(&a.id))
                    .then_some((a.id, f, a.body, (a.pose.x, a.pose.z, a.site.y)))
            })
            .collect();
        for (id, f, body, start) in fresh {
            wrap(&mut sim, id, f, &senses);
            tracks.insert(
                id,
                Track {
                    founder: f,
                    founded: false,
                    born_tick: t,
                    born_body: body,
                    start,
                    t0_reach_m2: None,
                    t0_faces: None,
                    pos: VecDeque::new(),
                    bins: VecDeque::new(),
                    cur: Bin::default(),
                    hist: vec![(t, start.0, start.1, start.2)],
                    ledges: Vec::new(),
                    last: start,
                    stuck_since: None,
                    stuck_s: 0,
                    t0_open: 72,
                    last_move: (t, body),
                    episodes: 0,
                    left_t0: false,
                    grazing: false,
                },
            );
        }
        let fv = sim.fauna().view();
        let mut alive: HashSet<u64> = HashSet::new();
        for a in fv.animals.iter() {
            let Some(tr) = tracks.get_mut(&a.id) else { continue };
            alive.insert(a.id);
            let p = cfgf.founder(tr.founder);
            let m = cfgf.manifest(tr.founder);
            let g = p.body_at(a.body);
            let held = a.founder_state.held;
            let effort = (held.forward.abs() * m.cruise_m_per_s
                + g.footprint_radius() * held.turn.abs() * m.yaw_cap_rad_per_s)
                / m.cruise_m_per_s;
            let moved = {
                let dx = ring_dx(a.pose.x, tr.last.0, width_m);
                (dx * dx + (a.pose.z - tr.last.1).powi(2)).sqrt()
            };
            let asked = held.forward.max(0.0) * m.cruise_m_per_s * cubarium_voxel::DT;
            let b = &mut tr.cur;
            b.ticks += 1.0;
            b.effort += effort;
            b.fwd += held.forward;
            b.turn += held.turn.abs();
            if held.forward > PRESS_ASK && moved < PRESS_DELIVERED * asked {
                b.pressed += 1.0;
            }
            if a.state == State::Asleep {
                b.asleep += 1.0;
            }
            if a.state == State::Cropping {
                b.cropping += 1.0;
            }
            if moved > 1e-6 {
                tr.last_move = (t, a.body);
            }
            if a.site.y != tr.last.2 {
                tr.ledges.push((t, tr.last.2, a.site.y));
            }
            tr.last = (a.pose.x, a.pose.z, a.site.y);
            if t % tick_hz == 0 {
                let done = std::mem::take(&mut tr.cur);
                tr.bins.push_back(done);
                tr.pos.push_back((a.pose.x, a.pose.z));
                while tr.bins.len() > window_s + 1 {
                    tr.bins.pop_front();
                    tr.pos.pop_front();
                }
            }
            if t % (HIST_S * tick_hz) == 0 {
                tr.hist.push((t, a.pose.x, a.pose.z, a.site.y));
            }
        }
        tracks.retain(|id, _| alive.contains(id));
        if t % (CHECK_S * tick_hz) != 0 {
            continue;
        }
        // ---------------------------------------------------------- the check
        let wv = sim.world().view();
        let fv = sim.fauna().view();
        let minute = t as f64 / window::TICKS_PER_MIN as f64;
        for a in fv.animals.iter() {
            let Some(tr) = tracks.get_mut(&a.id) else { continue };
            let p = cfgf.founder(tr.founder);
            let g = p.body_at(a.body);
            // The disc replica's own check: a founder outside its t = 0 reach.
            if let Some(f0) = &tr.t0_faces {
                if !tr.left_t0 && !f0.contains(&(a.site.x, a.site.y, a.site.z)) {
                    tr.left_t0 = true;
                    left_t0 += 1;
                    println!(
                        "  replica: #{} at min {minute:.1} stands on {:?}, outside its t0 reach of {:.1} m2",
                        a.id,
                        (a.site.x, a.site.y, a.site.z),
                        tr.t0_reach_m2.unwrap_or(0.0)
                    );
                }
            }
            if tr.pos.len() <= window_s {
                continue;
            }
            let (x0, z0) = tr.pos[0];
            let disp = tr
                .pos
                .iter()
                .map(|(x, z)| {
                    let dx = ring_dx(*x, x0, width_m);
                    (dx * dx + (z - z0).powi(2)).sqrt()
                })
                .fold(0.0, f64::max);
            let n: f64 = tr.bins.iter().map(|b| b.ticks).sum::<f64>().max(1.0);
            let effort = tr.bins.iter().map(|b| b.effort).sum::<f64>() / n;
            let cadence = tr.founder.manifest().cadence_ticks() as f64;
            let cropping = (tr.bins.iter().map(|b| b.cropping).sum::<f64>() * cadence / n).min(1.0);
            let still = disp < disp_bl * g.length_m && effort >= effort_min;
            if still && cropping >= CROPPING_MAX && tr.stuck_since.is_none() && !tr.grazing {
                tr.grazing = true;
                grazers.insert(a.id);
            }
            let stuck = still && cropping < CROPPING_MAX;
            if !stuck {
                if let Some(s) = tr.stuck_since.take() {
                    println!(
                        "  #{} {} got out at min {minute:.1} after {:.1} min stuck",
                        a.id,
                        tr.founder.name(),
                        (t - s) as f64 / window::TICKS_PER_MIN as f64
                    );
                }
                continue;
            }
            if tr.stuck_since.is_some() {
                tr.stuck_s += CHECK_S;
                continue;
            }
            tr.stuck_since = Some(t);
            tr.stuck_s += CHECK_S;
            tr.episodes += 1;
            stuck_bodies.insert(a.id);
            // What it did over the window.
            let fwd = tr.bins.iter().map(|b| b.fwd).sum::<f64>() / n;
            let turn = tr.bins.iter().map(|b| b.turn).sum::<f64>() / n;
            let pressed = tr.bins.iter().map(|b| b.pressed).sum::<f64>() / n;
            let asleep = tr.bins.iter().map(|b| b.asleep).sum::<f64>() / n;
            let sense = {
                let log = senses.lock().unwrap();
                let mut s = [0.0f64; 12];
                let mut k = 0.0;
                if let Some(q) = log.get(&a.id) {
                    for e in q {
                        for i in 0..12 {
                            s[i] += f64::from(e[i]);
                        }
                        k += 1.0;
                    }
                }
                s.map(|x| x / f64::max(k, 1.0))
            };
            let did = format!(
                "window: disp {:.2} BL effort {:.2} fwd {:.2} |turn| {:.2} pressing {:.0}% asleep {:.0}%; sensed contact F/L/R {:.2}/{:.2}/{:.2} cone clear {:.2}/{:.2}/{:.2} prox {:.2} valid {:.2} foliage {:.2} bodies {:.2}; wet {:.2}; water at feet {:.3} m (eye {:.3} m); fed on {:.0}% of intervals",
                disp / g.length_m,
                effort,
                fwd,
                turn,
                100.0 * pressed,
                100.0 * asleep,
                sense[0],
                sense[1],
                sense[2],
                sense[3],
                sense[4],
                sense[5],
                sense[6],
                sense[7],
                sense[8],
                sense[9],
                sense[10],
                wv.water_depth_m(i64::from(a.site.x), a.site.y, a.site.z),
                g.eye_m,
                100.0 * cropping
            );
            let here = (a.site.x, a.site.y, a.site.z);
            let (verdict, why): (&'static str, String) = if tr.founder == Founder::Browser {
                let rules = Rules::of(p, &g, v);
                let now = reach(&wv, &rules, a.pose.x, a.pose.z, a.site.y);
                let area = now.area(v);
                let step_d = cfgf.manifest(tr.founder).cruise_m_per_s * cubarium_voxel::DT;
                let open = open_headings(&wv, &rules, a.pose.x, a.pose.z, a.site.y, step_d);
                let own = accept_why(&wv, &rules, (a.pose.x, a.pose.z, a.site.y), a.pose.x, a.pose.z);
                let moved_ever = tr.hist.iter().chain(std::iter::once(&(t, a.pose.x, a.pose.z, a.site.y))).any(|h| {
                    let dx = ring_dx(h.1, tr.start.0, width_m);
                    (dx * dx + (h.2 - tr.start.1).powi(2)).sqrt() > 0.05
                });
                if open == 0 {
                    let open_then = open_headings(&wv, &Rules::of(p, &p.body_at(tr.last_move.1), v), a.pose.x, a.pose.z, a.site.y, step_d);
                    let open_born = open_headings(&wv, &Rules::of(p, &p.body_at(tr.born_body), v), a.pose.x, a.pose.z, a.site.y, step_d);
                    let (down, up) = (rules.step.down, rules.step.up);
                    let freeing: Vec<(u32, usize)> = (a.site.y.saturating_sub(down)..=a.site.y + up)
                        .filter(|y| *y != a.site.y)
                        .map(|y| (y, open_headings(&wv, &rules, a.pose.x, a.pose.z, y, step_d)))
                        .filter(|e| e.1 > 0)
                        .collect();
                    let label = if tr.founded && !moved_ever {
                        "H1 founded frozen (disc in a wall)"
                    } else if open_then > 0 || open_born > 0 {
                        "H2 grew into the wall (frozen)"
                    } else if tr.ledges.last().is_some_and(|l| l.2 < l.1 && l.2 == a.site.y)
                        && freeing.iter().any(|e| e.0 > a.site.y)
                    {
                        "H2 stepped down into a frozen spot"
                    } else {
                        "H2 walked into a frozen spot"
                    };
                    let steps: Vec<String> = tr
                        .ledges
                        .iter()
                        .rev()
                        .take(4)
                        .map(|l| format!("{}->{}@{:.2}", l.1, l.2, l.0 as f64 / window::TICKS_PER_MIN as f64))
                        .collect();
                    (
                        label,
                        format!(
                            "0/72 headings open at a {:.4} m sub-step; own pose {:?}; moved since founding/birth {}; t0 open {}; last moved min {:.2} at body {:.3} (now {:.3}): open at that size {open_then}, at birth size {open_born}; standing on another layer would free it: {freeing:?}; last ledges {:?}; lattice reach {area:.1} m2",
                            step_d, own, moved_ever, tr.t0_open, tr.last_move.0 as f64 / window::TICKS_PER_MIN as f64, tr.last_move.1, a.body, steps
                        ),
                    )
                } else if area >= small_m2 {
                    let label = if pressed >= PRESS_SHARE_H4 { "H4 motor" } else { "H3 navigation" };
                    (label, format!("reach {area:.1} m2, {open}/72 headings open (start ok {})", now.started))
                } else if tr.founded && tr.t0_reach_m2.is_some_and(|m| m < small_m2) {
                    (
                        "H1 founded in pocket",
                        format!("reach {area:.1} m2; t0 reach {:.1} m2", tr.t0_reach_m2.unwrap()),
                    )
                } else {
                    // H2: what cut it off.
                    let dry = reach(&wv, &Rules { wade: f64::INFINITY, ..rules }, a.pose.x, a.pose.z, a.site.y).area(v);
                    let small_body = p.body_at(tr.born_body);
                    let young = reach(&wv, &Rules::of(p, &small_body, v), a.pose.x, a.pose.z, a.site.y).area(v);
                    // The last history sample outside the trap, and the ledges after it.
                    let out = tr.hist.iter().rposition(|h| {
                        let f = (
                            ((h.1 / v).floor() as i64).rem_euclid(i64::from(cfg.width)) as u32,
                            h.3,
                            (h.2 / v).floor() as u32,
                        );
                        !now.faces.contains(&f)
                    });
                    let entry = match out {
                        Some(k) => {
                            let h = tr.hist[k];
                            let back = reach(&wv, &rules, h.1, h.2, h.3);
                            let can_return = back.faces.contains(&here);
                            let steps: Vec<String> = tr
                                .ledges
                                .iter()
                                .filter(|l| l.0 > h.0)
                                .take(6)
                                .map(|l| format!("{}->{}@{:.1}", l.1, l.2, l.0 as f64 / window::TICKS_PER_MIN as f64))
                                .collect();
                            format!(
                                "last outside at min {:.1} ({:.1} m2 reach from there, which {} here); ledges since {:?}",
                                h.0 as f64 / window::TICKS_PER_MIN as f64,
                                back.area(v),
                                if can_return { "includes" } else { "excludes" },
                                steps
                            )
                        }
                        None => "never outside since founding/birth".into(),
                    };
                    let label = if dry >= small_m2 {
                        "H2 water"
                    } else if young >= small_m2 {
                        "H2 growth"
                    } else if out.is_none() {
                        "H2 world changed under it"
                    } else {
                        "H2 one-way step"
                    };
                    (
                        label,
                        format!("reach {area:.1} m2, dry {dry:.1}, at birth size {young:.1}; {entry}"),
                    )
                }
            } else {
                // A climber: the column rule with wall links, on its own body now.
                let faces = standable_faces(&wv, &g, p.wade_depth_m(&g));
                let comps = route_components(
                    &wv,
                    &faces,
                    RouteRule { step: p.step_limits(v), climbs_walls: p.climbs_walls },
                    g.headroom_voxels(v),
                );
                let mine = faces
                    .iter()
                    .position(|s| (s.x, s.y, s.z) == here)
                    .map(|i| comps[i]);
                let area = mine.map_or(0.0, |c| comps.iter().filter(|x| **x == c).count() as f64 * v * v);
                let label = if area < small_m2 {
                    "trapped (column rule)"
                } else if pressed >= PRESS_SHARE_H4 {
                    "H4 motor"
                } else {
                    "H3 navigation"
                };
                (label, format!("column-rule component {area:.1} m2"))
            };
            verdicts.push((tr.founder, verdict));
            println!(
                "  STUCK #{} {} {} at min {minute:.1} at {:?} body {:.3} ({}): {verdict}: {why}; {did}",
                a.id,
                tr.founder.name(),
                if tr.founded { "founder" } else { "born" },
                here,
                a.body,
                if tr.founded {
                    "founded".to_string()
                } else {
                    format!("born min {:.1}", tr.born_tick as f64 / window::TICKS_PER_MIN as f64)
                },
            );
        }
        if t % (10 * window::TICKS_PER_MIN) == 0 {
            let now_stuck = tracks.values().filter(|t| t.stuck_since.is_some()).count();
            let per: Vec<String> = [Founder::Browser, Founder::Blind]
                .iter()
                .map(|f| format!("{} {}", f.name(), tracks.values().filter(|t| t.founder == *f).count()))
                .collect();
            println!("# min {minute:.0}: alive {:?}, stuck now {now_stuck}", per);
        }
    }
    let mut counts: HashMap<(Founder, &str), usize> = HashMap::new();
    for (f, v) in &verdicts {
        *counts.entry((*f, v)).or_default() += 1;
    }
    let mut list: Vec<String> = counts.iter().map(|((f, v), n)| format!("{} {v}: {n}", f.name())).collect();
    list.sort();
    let stuck_min: f64 = tracks.values().map(|t| t.stuck_s as f64 / 60.0).sum();
    println!(
        "SUMMARY seed {seed} brain {}: {} stuck episodes over {} bodies {:?}; stuck body-min (alive at end) {:.1}; bodies still-and-spinning while feeding (grazing, not counted) {}; replica violations (founder left its t0 reach) {left_t0}",
        brains[0].1,
        verdicts.len(),
        stuck_bodies.len(),
        list,
        stuck_min,
        grazers.len()
    );
}
