//! The plant succession study (`design/handoffs/plant-succession-study-2026-10-03.md`,
//! Astra's review `design/7_Research/astra-plant-succession-review-2026-10-03.md`), with the
//! springturf-spread repairs (`design/handoffs/springturf-spread-2026-10-04.md` §8,
//! `design/7_Research/astra-succession-results-review-2026-10-04.md` findings 2, 3, 9).
//!
//! ```text
//! succession_study found  seed=N save=DIR [minutes=120] [fauna=1] [threads=T] [config=FILE] [set=P=V ...]
//! succession_study plants load=DIR/mM arm=NAME hours=H [every=30] [cohort=FILE] [map=FILE] [save=DIR save_min=M,..] [set=...]
//! succession_study fauna  load=DIR/mM arm=NAME hours=H [set=...]
//! succession_study fate   from=DIR/mM to=DIR2/mN
//! ```
//!
//! Every subcommand resolves the layers as `velvet_niche` does: code defaults, the file's
//! `[flora]`/`[fauna]` (default `config/desktop/terrarium.toml`), then `set=` lines and a
//! `sets=` file.
//!
//! - `found`: the terrarium from founding to `minutes`, saved at the end (`DIR/mM`), with
//!   its **cohort file** (`DIR/mM/cohort.csv`) read off the untouched save. `fauna=1`
//!   founds the config's own founding table with it (burrs need animals from the start);
//!   without it, plants only. One per seed: the shared baseline every arm branches from.
//! - `plants`: branch a save under the resolved config, with whatever animals it holds.
//!   The cohorts are built from the **untouched** loaded layer and its own config, before
//!   the arm's config replaces it (finding 2), and, given `cohort=FILE`, asserted equal to
//!   the founding's file site for site and class for class. Samples every `every` minutes;
//!   at minutes 240 and 360 (hours 4 and 6) the S1 sites' states go to `map=FILE`
//!   (`x,y,z,state` lines: the geographic coverage map). At the end: flux windows, deaths
//!   by cause and disappearances on the tracked sites, delivery receipts, burr deliveries,
//!   the attempt classes with the failed gates split, and with animals present the fauna
//!   balance per lineage with the Veilhound's kills, carcasses, bites and assimilation.
//!   Each sample also prints a `juv` line (turf-establish): springturf that starved
//!   without ever being seen at donor size, and turf births, per 10 minutes since the last
//!   sample, with the share of the births that came up within two columns of a mature turf.
//!   A `canopy` line gives the canopy gate's and runner support's ledger counters.
//! - `fauna`: branch a save, introduce the code-default founding table through the
//!   seeder's own fauna placement, and run with the heuristics (the earlier study's arm).
//!
//! **Cohorts** (on soil support faces, read at the load; the save's springturf gates):
//! - `S1` (succession): velvetpad-held, springturf's water, aeration and standing-water
//!   gates pass, its light gate (terrain sky ≥ `establish_light_min`) passes, and the raw
//!   light at a turf crown's top `L = sky · canopy ≥ l_min` (0.5).
//! - `S2` (refuge): velvetpad-held and **wet** (turf's aeration or standing-water gate
//!   fails) or **shaded** (turf's water gates pass, its sky gate or the `l_min` band
//!   fails). A velvetpad site failing only turf's water minimum is **dry**, reported apart.
//! - `D` (drained ground, S3): every soil face, any occupant, where turf's water, aeration
//!   and standing-water gates pass.
//! - `W` (waterlogged): every soil face whose velvetpad root box is at least
//!   [`WET_SATURATED`] saturated and whose standing water is within velvetpad's drown
//!   depth — the faces the old pad ceiling (0.6) refused and the new one (1.0) admits —
//!   split by occupant at the load; the vacant ones are the "vacant wet faces".

use std::collections::{HashMap, HashSet};
use std::path::Path;

use cubarium_search::es::voxel::live::balance;
use cubarium_voxel::{Material, World};
use cubarium_voxel_fauna::{Departure, Fauna, Food, Founder, TICK_HZ};
use cubarium_voxel_flora::study::canopy_at;
use cubarium_voxel_flora::{
    DeathCause, Flora, FloraConfig, FloraLedger, Site, Species, Trophic, Via, establishment_gates_with_sky,
};
use cubarium_voxel_sim::params::{Layers, Params};
use cubarium_voxel_sim::staged;
use cubarium_voxel_sim::{Sim, SimConfig};

const WOODY: [Species; 3] = [Species::Bloomcrown, Species::Lanternberry, Species::Vaulttree];
const TURF: Species = Species::Springturf;
const PAD: Species = Species::Velvetpad;
/// The readout's "waterlogged" threshold: velvetpad's root box at least this saturated
/// (its establishment ceiling before 2026-10-04). A study threshold, not a model knob.
const WET_SATURATED: f64 = 0.6;
/// The map's minutes: hours 2 (the load), 4 and 6.
const MAP_MINUTES: [u64; 3] = [120, 240, 360];

fn arg<'a>(args: &'a [String], k: &str) -> Option<&'a str> {
    args.iter().find_map(|a| a.strip_prefix(k))
}

/// The resolved parameter set: code defaults, the file's `[flora]`/`[fauna]`, the sets.
fn resolve(args: &[String]) -> Params {
    let path = arg(args, "config=").unwrap_or("config/desktop/terrarium.toml");
    let cfg = cubarium::voxel::load_config(Path::new(path)).unwrap_or_else(|e| panic!("config {path}: {e}"));
    let mut file = toml::Table::new();
    if !cfg.flora.is_empty() {
        file.insert("flora".into(), toml::Value::Table(cfg.flora));
    }
    if !cfg.fauna.is_empty() {
        file.insert("fauna".into(), toml::Value::Table(cfg.fauna));
    }
    let mut sets: Vec<String> = args.iter().filter_map(|a| a.strip_prefix("set=").map(str::to_string)).collect();
    if let Some(f) = arg(args, "sets=") {
        let text = std::fs::read_to_string(f).unwrap_or_else(|e| panic!("sets {f}: {e}"));
        sets.extend(
            text.lines()
                .map(str::trim)
                .filter(|l| !l.is_empty() && !l.starts_with('#'))
                .map(str::to_string),
        );
    }
    for s in &sets {
        eprintln!("set {s}");
    }
    Params::resolve_over(
        Params {
            world: cfg.world,
            ..Params::defaults()
        },
        &Layers {
            file: Some(file),
            sets,
            ..Layers::default()
        },
    )
    .unwrap_or_else(|e| panic!("the parameter set: {e}"))
}

/// Arguments that only shape a fresh founding: `resume=` leaves them out (the checkpoint
/// carries the world). A bare word matches exactly; `k=` matches a prefix.
const FOUNDING_ONLY: &[&str] = &["seed=", "minutes=", "save=", "save_min=", "cohort=", "fauna="];

fn main() {
    let raw: Vec<String> = std::env::args().collect();
    // `resume=DIR` continues a staged run (`run=DIR`) from its latest checkpoint
    // (`cubarium_voxel_sim::staged`). `found` resumes as nothing: branch its checkpoints
    // with `plants load=DIR/ckpt/mN`.
    let args = staged::resume_args(&raw, FOUNDING_ONLY).unwrap_or_else(|e| panic!("{e}"));
    assert!(
        !(staged::resumed(&args) && args.get(1).map(String::as_str) == Some("found")),
        "a founding is not resumed: branch from its checkpoints with `plants load=DIR/ckpt/mN`"
    );
    // Training, search and experiment runs take the coarse water (`water_every=`).
    cubarium_voxel_sim::experiment_args(&args);
    if let Some(n) = arg(&args, "threads=").and_then(|s| s.parse().ok()) {
        cubarium_voxel::set_thread_override(n);
    }
    match args.get(1).map(String::as_str) {
        Some("found") => found(&args),
        Some("plants") => plants(&args),
        Some("fauna") => fauna(&args),
        Some("fate") => fate(&args),
        _ => panic!("usage: succession_study found|plants|fauna|fate [...]"),
    }
}

fn save(dir: &Path, sim: &Sim, minute: u64) {
    std::fs::create_dir_all(dir).unwrap();
    let (world, flora, fauna) = sim.layers();
    std::fs::write(dir.join("world.bin"), world.save()).unwrap();
    std::fs::write(dir.join("flora.bin"), cubarium_voxel_flora::snapshot::encode(flora)).unwrap();
    std::fs::write(dir.join("fauna.bin"), fauna.save()).unwrap();
    std::fs::write(dir.join("minute"), minute.to_string()).unwrap();
    eprintln!("saved {} at minute {minute}", dir.display());
}

/// A save, **untouched**: its own plant config is still the layer's. Its founder bodies
/// get the founding's heuristics back through the one safe load
/// (`cubarium_voxel_sim::saved`; controllers are not saved), and a policy-driven save is
/// refused: this study runs heuristics.
fn load_raw(dir: &Path) -> (World, Flora, Fauna, u64) {
    let s = cubarium_voxel_sim::saved::load(dir, &[]).unwrap_or_else(|e| panic!("load: {e}"));
    (s.world, s.flora, s.fauna, s.minute)
}

/// The staged run (`run=DIR`), started on the world as it stands at `minute0`; `None`
/// unstaged. A failed load check exits with status 2 and `FAILED` written.
fn stage(args: &[String], sim: &mut Sim, minute0: u64, end: u64) -> Option<staged::Runner> {
    let cfg = staged::StageConfig::from_args(args).unwrap_or_else(|e| panic!("{e}"))?;
    let r = staged::Runner::start(cfg, args, sim, minute0, end).unwrap_or_else(|e| {
        eprintln!("staged: the load check failed — {e}");
        std::process::exit(2)
    });
    eprintln!("staged run {} to minute {}", r.dir().display(), r.end_minute());
    Some(r)
}

/// Tell the runner a minute ended; true means stop.
fn stop_at(runner: &mut Option<staged::Runner>, sim: &Sim, minute: u64) -> bool {
    let Some(r) = runner.as_mut() else { return false };
    match r.minute(sim, minute) {
        None => false,
        Some(ended) => {
            eprintln!("staged: {ended:?} at minute {minute}");
            ended != staged::Ended::Done
        }
    }
}

/// Replace a loaded layer's plant config with the arm's.
fn arm_config(flora: &mut Flora, want: &FloraConfig, what: &str) {
    flora.replace_config(want.clone()).expect("the resolved config validates");
    assert!(flora.config() == want, "the configuration replacement took");
    let t = flora.config().species(TURF);
    eprintln!(
        "{what}: springturf overtop {} covers {:?} dispersal {:?} wald {} m burr {} runners {} x{} through covers {} cure {} s alpha {}; velvetpad saturated max {}",
        t.overtop_per_check,
        t.overtop_covers,
        t.dispersal,
        t.wald_mean_m,
        t.burr_pickup,
        t.clonal_share,
        t.runner_reach,
        t.runners_overtop,
        t.cure_after_s,
        t.alpha,
        flora.config().species(PAD).establish_saturated_max,
    );
}

/// A save, its plant config replaced by `want`; the save's own config is returned too.
fn load(dir: &Path, want: &FloraConfig) -> (World, Flora, Fauna, u64, FloraConfig) {
    let (world, mut flora, fauna, minute) = load_raw(dir);
    let saved = flora.config().clone();
    arm_config(&mut flora, want, &format!("loaded {} at minute {minute}", dir.display()));
    (world, flora, fauna, minute, saved)
}

// -------------------------------------------------------------------------- found

fn found(args: &[String]) {
    let params = resolve(args);
    let seed: u64 = arg(args, "seed=").map_or(1, |s| s.parse().unwrap());
    let minutes: u64 = arg(args, "minutes=").map_or(120, |m| m.parse().unwrap());
    let with_fauna = arg(args, "fauna=").is_some_and(|v| v == "1");
    let l_min: f64 = arg(args, "l_min=").map_or(0.5, |v| v.parse().unwrap());
    let dir = Path::new(arg(args, "save=").expect("save=DIR"));
    let wc = params.world.clone();
    // The plants on the asked terrain seed (no animals in the habitat judgement, so the
    // seed is the terrain the earlier studies used), then — with `fauna=1` — the
    // config's founding table introduced at minute 0 through the seeder's own placement.
    let founded = cubarium::voxel::found(
        &wc,
        &cubarium::voxel::LayerConfig::new(&params, wc.voxel_m),
        [0; Founder::COUNT],
        cubarium::voxel::Seeds::from(seed),
        false,
    );
    eprintln!(
        "founded seed {} ({}): stands {}",
        founded.seed,
        if founded.accepted { "accepted" } else { "NOT accepted" },
        founded.seeded.stands,
    );
    let (mut world, mut flora, mut fauna) = (founded.world, founded.flora, founded.fauna);
    if with_fauna {
        introduce(&arg(args, "arm=").unwrap_or("found").to_string(), 0, &params, &mut world, &mut flora, &mut fauna);
    }
    // The overlapped tick, as the display runs it, animals or not.
    let mut sim = Sim::ambient(world, flora, fauna, SimConfig::experiment());
    let per_min = 60 * u64::from(TICK_HZ);
    let t0 = std::time::Instant::now();
    let mut runner = stage(args, &mut sim, 0, minutes);
    for minute in 1..=minutes {
        for _ in 0..per_min {
            sim.step();
            if let Some(r) = runner.as_mut() {
                r.tick(&sim);
            }
        }
        if minute % 10 == 0 {
            eprintln!("minute {minute} ({:.0} s wall, {} animals)", t0.elapsed().as_secs_f64(), sim.fauna().view().animals.len());
        }
        if stop_at(&mut runner, &sim, minute) {
            return;
        }
    }
    let at = dir.join(format!("m{minutes}"));
    save(&at, &sim, minutes);
    // The cohort file, off the untouched save (read back, so it is exactly what an arm
    // will load).
    let (world, flora, _, _) = load_raw(&at);
    let co = cohorts(&world, &flora, flora.config(), l_min);
    std::fs::write(at.join("cohort.csv"), co.to_csv()).unwrap();
    eprintln!("wrote {}", at.join("cohort.csv").display());
}

// ------------------------------------------------------------------------ cohorts

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
enum Class {
    S1,
    Wet,
    Shaded,
    Dry,
}

impl Class {
    fn name(self) -> &'static str {
        match self {
            Class::S1 => "S1",
            Class::Wet => "wet",
            Class::Shaded => "shaded",
            Class::Dry => "dry",
        }
    }
}

struct Cohorts {
    /// Velvetpad-held sites at the load, by class.
    pad: Vec<(Site, Class)>,
    /// Drained ground (any occupant): turf's water gates pass.
    drained: Vec<Site>,
    /// Waterlogged, unflooded faces (`W`), with their occupant at the load.
    wet: Vec<(Site, Option<Species>)>,
    /// Soil faces in all.
    soil: usize,
    /// Each S1 site's distance to the nearest springturf donor at the load, voxels
    /// (horizontal, the ring wrapped).
    reach: HashMap<Site, f64>,
}

impl Cohorts {
    /// One line per site: `kind,x,y,z,class` — what an arm's cohort is asserted against.
    fn to_csv(&self) -> String {
        let mut out = String::new();
        for (s, c) in &self.pad {
            out += &format!("pad,{},{},{},{}\n", s.x, s.y, s.z, c.name());
        }
        for s in &self.drained {
            out += &format!("drained,{},{},{},-\n", s.x, s.y, s.z);
        }
        for (s, o) in &self.wet {
            out += &format!("wet,{},{},{},{}\n", s.x, s.y, s.z, occ_name(occ_index(*o)));
        }
        out
    }
}

fn soil_faces(world: &World) -> Vec<Site> {
    let view = world.view();
    let c = view.config;
    let mut out = Vec::new();
    for z in 0..c.depth {
        for x in 0..c.width {
            for y in view.supports_in_column(x as i64, z) {
                if view.material_at(x as i64, y, z) == Material::Soil {
                    out.push(Site { x, y, z });
                }
            }
        }
    }
    out
}

/// Whether a face is waterlogged and unflooded by `reference`'s velvetpad box.
fn waterlogged(view: &cubarium_voxel::VoxelView<'_>, site: Site, reference: &FloraConfig) -> bool {
    let pad = reference.species(PAD);
    let sky = view.sky_visibility(site.x as i64, site.y, site.z);
    let p = establishment_gates_with_sky(view, site, pad, sky, 0.0, 0.0);
    p.saturated_fraction >= WET_SATURATED && p.water_depth_m <= pad.drown_depth_m
}

fn cohorts(world: &World, flora: &Flora, reference: &FloraConfig, l_min: f64) -> Cohorts {
    let view = world.view();
    let c = view.config;
    let fv = flora.view();
    // The save's own springturf: its gates, crown and donor size define the cohorts.
    let sc = reference.species(TURF);
    let top = sc.crown_height(sc.wood_max, c.voxel_m);
    let mut out = Cohorts {
        pad: Vec::new(),
        drained: Vec::new(),
        wet: Vec::new(),
        soil: 0,
        reach: HashMap::new(),
    };
    let donors: Vec<Site> = fv
        .stands
        .iter()
        .filter(|s| s.species == TURF && s.wood >= sc.donor_min)
        .map(|s| s.site)
        .collect();
    let w = f64::from(c.width);
    for site in soil_faces(world) {
        out.soil += 1;
        let sky = view.sky_visibility(site.x as i64, site.y, site.z);
        let g = establishment_gates_with_sky(&view, site, sc, sky, 0.0, 0.0);
        let water = g.pore_ok && g.aeration_ok && g.depth_ok;
        if water {
            out.drained.push(site);
        }
        if waterlogged(&view, site, reference) {
            out.wet.push((site, fv.stand_at(site).map(|s| s.species)));
        }
        if fv.stand_at(site).is_none_or(|s| s.species != PAD) {
            continue;
        }
        let light = sky * canopy_at(&fv, c.width, c.depth, site, f64::from(site.y) + top);
        let class = if !(g.aeration_ok && g.depth_ok) {
            Class::Wet
        } else if !g.pore_ok {
            Class::Dry
        } else if !g.light_ok || light < l_min {
            Class::Shaded
        } else {
            Class::S1
        };
        out.pad.push((site, class));
        if class == Class::S1 {
            let d = donors
                .iter()
                .map(|d| wrapped(*d, site, w))
                .fold(f64::INFINITY, f64::min);
            out.reach.insert(site, d);
        }
    }
    out
}

/// Horizontal distance between two faces, voxels, `x` wrapped round a ring `w` wide.
fn wrapped(a: Site, b: Site, w: f64) -> f64 {
    let dx = (f64::from(a.x) - f64::from(b.x)).rem_euclid(w);
    let dx = dx.min(w - dx);
    let dz = f64::from(a.z) - f64::from(b.z);
    (dx * dx + dz * dz).sqrt()
}

// ------------------------------------------------------------------------- plants

/// What one tracked site has been through, from the tick's death log and receipts and the
/// per-minute occupancy.
#[derive(Clone, Copy, Default)]
struct Track {
    /// The occupant at the last minute check.
    cur: Option<(Species, u64)>,
    /// The **current** turf stand has reached donor size: its own tenure, so an unrelated
    /// later seedling does not inherit an earlier one's (finding 3).
    tenure_donor: bool,
    /// The current woody stand came up through a donor-size turf on this site: the
    /// continuous turf → woody trajectory.
    woody_from_turf: bool,
    /// A turf here died overtopped since the last check: whether it was donor size.
    overtopped_turf: Option<bool>,
    /// A woody stand seen before any turf.
    woody_direct: bool,
    turf_seen: bool,
    turf_births: u32,
    pad_births: u32,
    /// Turf packages received, by [`Via`] (seed, runner, burr).
    receipts: [u32; 3],
    /// Deaths here by cause, `[turf, pad]` (the death log: events, not inferences).
    deaths: [[u32; DeathCause::COUNT]; 2],
    /// An occupant gone between checks with no death record: removed, not killed.
    disappeared: u32,
}

impl Track {
    fn success(&self) -> bool {
        match self.cur {
            Some((s, _)) if s == TURF => self.tenure_donor,
            Some((s, _)) if is_woody(s) => self.woody_from_turf,
            _ => false,
        }
    }

    fn state(&self) -> &'static str {
        match self.cur {
            None => "vacant",
            Some((s, _)) if s == TURF && self.tenure_donor => "turf_donor",
            Some((s, _)) if s == TURF => "turf_young",
            Some((s, _)) if is_woody(s) && self.woody_from_turf => "woody_from_turf",
            Some((s, _)) if is_woody(s) => "woody_other",
            Some((s, _)) if s == PAD => "pad",
            Some(_) => "other",
        }
    }
}

fn via_index(v: Via) -> usize {
    match v {
        Via::Seed => 0,
        Via::Runner => 1,
        Via::Burr => 2,
    }
}

const VIA_NAMES: [&str; 3] = ["seed", "runner", "burr"];
/// Delivery distance bins, voxels.
const DIST_BINS: [(f64, &str); 5] = [(6.0, "<=6"), (12.0, "6-12"), (24.0, "12-24"), (48.0, "24-48"), (f64::INFINITY, ">48")];

fn is_woody(s: Species) -> bool {
    WOODY.contains(&s)
}

fn occ_index(o: Option<Species>) -> usize {
    o.map_or(Species::COUNT, |s| s.index())
}

fn occ_name(i: usize) -> &'static str {
    if i == Species::COUNT { "vacant" } else { Species::ALL[i].name() }
}

/// Edible foliage (above every layer's grazing floor) on one stand.
fn edible(fv: &cubarium_voxel_flora::FloraView<'_>, s: &cubarium_voxel_flora::Stand) -> f64 {
    fv.layers(s).map(|l| l.edible()).sum()
}

struct Window {
    ledger: FloraLedger,
    minute: u64,
}

/// Everything the run tracks besides the cohorts.
struct Watch {
    track: HashMap<Site, Track>,
    /// The tracked sites' classes: S1, the refuge classes.
    class: HashMap<Site, Class>,
    /// Ids the death log has recorded (so a vanished id is a death, not a removal).
    died: HashSet<u64>,
    /// Refuge velvetpad births observed (minute checks) and deaths (events) per two-hour
    /// window and class.
    refuge_bd: HashMap<(u64, Class), (u64, u64)>,
    /// Turf deliveries, all sites: count and distance histogram by via.
    deliveries: [[u64; DIST_BINS.len()]; 3],
    /// Burr landings waiting to be seen established: site → the birth counter at landing.
    burr_pending: HashMap<Site, u64>,
    burr_sites: u64,
    burr_established: u64,
    /// The most cured turf stands seen at any minute check.
    cured_max: usize,
}

/// **Young springturf** (turf-establish, 2026-10-05): starvation of turf that was never
/// seen at donor size, and births, with the share that came up next to mature turf.
#[derive(Default)]
struct Juv {
    /// Turf ids seen at donor size at a minute check.
    been_mature: HashSet<u64>,
    /// Turf ids alive at the last minute check.
    known: HashSet<u64>,
    /// Since the last sample: young turf starved (death-log events), turf births seen at
    /// the minute checks, and of those the ones with a mature turf within two columns.
    starved: u64,
    births: u64,
    births_near_mature: u64,
    since: u64,
}

impl Juv {
    fn tick(&mut self, flora: &Flora) {
        for d in flora.death_log() {
            if d.species == TURF && d.cause == DeathCause::Starved && !self.been_mature.contains(&d.id) {
                self.starved += 1;
            }
        }
    }

    /// The minute check: who is at donor size, who is new, and whether a mature turf
    /// stands within two columns of a newcomer (`x` wrapped).
    fn minute(&mut self, flora: &Flora, width: u32) {
        let fv = flora.view();
        let min = fv.config.species(TURF).donor_min;
        let mut mature_cols: HashSet<(u32, u32)> = HashSet::new();
        let mut now: HashSet<u64> = HashSet::new();
        for s in fv.stands.iter().filter(|s| s.species == TURF) {
            now.insert(s.id);
            if s.wood >= min {
                self.been_mature.insert(s.id);
                mature_cols.insert((s.site.x, s.site.z));
            }
        }
        for s in fv.stands.iter().filter(|s| s.species == TURF && !self.known.contains(&s.id)) {
            self.births += 1;
            let near = (-2i64..=2).any(|dx| {
                (-2i64..=2).any(|dz| {
                    let x = (i64::from(s.site.x) + dx).rem_euclid(i64::from(width)) as u32;
                    let z = i64::from(s.site.z) + dz;
                    z >= 0 && (dx, dz) != (0, 0) && mature_cols.contains(&(x, z as u32))
                })
            });
            self.births_near_mature += u64::from(near);
        }
        self.known = now;
    }

    fn line(&mut self, arm: &str, minute: u64) {
        let span = (minute - self.since).max(1) as f64;
        println!(
            "juv,{arm},{minute},starved_per10={:.1},births_per10={:.1},near_mature_share={:.3}",
            10.0 * self.starved as f64 / span,
            10.0 * self.births as f64 / span,
            self.births_near_mature as f64 / (self.births.max(1)) as f64
        );
        self.starved = 0;
        self.births = 0;
        self.births_near_mature = 0;
        self.since = minute;
    }
}

fn plants(args: &[String]) {
    let hours: f64 = arg(args, "hours=").map_or(4.0, |h| h.parse().unwrap());
    let every: u64 = arg(args, "every=").map_or(30, |m| m.parse().unwrap());
    let l_min: f64 = arg(args, "l_min=").map_or(0.5, |v| v.parse().unwrap());
    let arm = arg(args, "arm=").unwrap_or("arm").to_string();
    let params = resolve(args);
    let want = params.flora_for_world();
    let dir = Path::new(arg(args, "load=").expect("load=DIR"));
    let (world, mut flora, fauna, minute0) = load_raw(dir);
    // The cohorts, from the untouched save under its own config, before the arm's
    // config replaces it (finding 2).
    let saved = flora.config().clone();
    let co = cohorts(&world, &flora, &saved, l_min);
    let resumed = staged::resumed(args);
    if resumed {
        eprintln!("resumed at minute {minute0}: the cohorts and every readout cover the run from here");
    }
    if let Some(file) = arg(args, "cohort=").filter(|_| !resumed) {
        let expect = std::fs::read_to_string(file).unwrap_or_else(|e| panic!("cohort {file}: {e}"));
        let got = co.to_csv();
        assert!(
            expect == got,
            "the cohort differs from {file}: {} against {} lines",
            got.lines().count(),
            expect.lines().count()
        );
        eprintln!("cohort matches {file}: {} sites and classes", got.lines().count());
    }
    arm_config(&mut flora, &want, &format!("{arm} at minute {minute0}"));
    let count = |k: Class| co.pad.iter().filter(|(_, c)| *c == k).count();
    let pads = co.pad.len();
    let wet_vacant: Vec<Site> = co.wet.iter().filter(|w| w.1.is_none()).map(|w| w.0).collect();
    println!(
        "cohort,{arm},{minute0},soil={},pad_sites={pads},s1={},wet={},shaded={},dry={},drained={},waterlogged={},waterlogged_vacant={},s1_share_of_pad={:.4}",
        co.soil,
        count(Class::S1),
        count(Class::Wet),
        count(Class::Shaded),
        count(Class::Dry),
        co.drained.len(),
        co.wet.len(),
        wet_vacant.len(),
        count(Class::S1) as f64 / pads.max(1) as f64
    );
    flora.trace_attempts(Some(TURF));
    let with_fauna = !fauna.view().animals.is_empty();
    // The overlapped tick, as the display runs it, animals or not.
    let mut sim = Sim::ambient(world, flora, fauna, SimConfig::experiment());
    let save_dir = arg(args, "save=");
    let save_min: Vec<u64> = arg(args, "save_min=").map_or(Vec::new(), |m| m.split(',').map(|v| v.parse().unwrap()).collect());
    let mut map = arg(args, "map=").map(|f| std::fs::File::create(f).unwrap_or_else(|e| panic!("map {f}: {e}")));
    let s1: Vec<Site> = co.pad.iter().filter(|(_, c)| *c == Class::S1).map(|(s, _)| *s).collect();
    let refuge: Vec<(Site, Class)> = co.pad.iter().filter(|(_, c)| matches!(c, Class::Wet | Class::Shaded)).copied().collect();
    let mut w = Watch {
        track: HashMap::new(),
        class: HashMap::new(),
        died: HashSet::new(),
        refuge_bd: HashMap::new(),
        deliveries: [[0; DIST_BINS.len()]; 3],
        burr_pending: HashMap::new(),
        burr_sites: 0,
        burr_established: 0,
        cured_max: 0,
    };
    {
        let fv = sim.flora().view();
        for &(site, class) in co.pad.iter().filter(|(_, c)| *c != Class::Dry) {
            let cur = fv.stand_at(site).map(|s| (s.species, s.id));
            w.track.insert(
                site,
                Track {
                    cur,
                    turf_seen: cur.is_some_and(|c| c.0 == TURF),
                    ..Track::default()
                },
            );
            w.class.insert(site, class);
        }
    }
    let start = Window {
        ledger: sim.flora().view().ledger.clone(),
        minute: minute0,
    };
    let mut windows: Vec<Window> = vec![start];
    let per_min = 60 * u64::from(TICK_HZ);
    let mut runner = stage(args, &mut sim, minute0, minute0 + (hours * 60.0).round() as u64);
    let end = runner.as_ref().map_or(minute0 + (hours * 60.0).round() as u64, staged::Runner::end_minute);
    let total = (end - minute0) * per_min;
    let mut fw = FaunaWatch::new(&sim, total / 2);
    let width = f64::from(sim.world().config().width);
    let mut juv = Juv {
        since: minute0,
        ..Juv::default()
    };
    juv.minute(sim.flora(), sim.world().config().width);
    juv.births = 0;
    juv.births_near_mature = 0;
    let t0 = std::time::Instant::now();
    sample(&sim, &arm, minute0, &co, &s1, &refuge, &w, &wet_vacant, &saved, &windows[0]);
    if let Some(f) = map.as_mut() {
        write_map(f, &arm, minute0, &s1, &w);
    }
    let mut tick = 0u64;
    for minute in minute0 + 1..=end {
        let win = (minute - minute0 - 1) / 120;
        for _ in 0..per_min {
            sim.step();
            tick += 1;
            if let Some(r) = runner.as_mut() {
                r.tick(&sim);
            }
            if with_fauna {
                fw.tick(&sim, &params, tick);
            }
            // The tick's deaths and receipts.
            let flora = sim.flora();
            juv.tick(flora);
            for d in flora.death_log() {
                let Some(t) = w.track.get_mut(&d.site) else {
                    continue;
                };
                w.died.insert(d.id);
                let k = match d.species {
                    s if s == TURF => 0,
                    s if s == PAD => 1,
                    _ => continue,
                };
                t.deaths[k][d.cause.index()] += 1;
                if d.species == TURF && d.cause == DeathCause::Overtopped {
                    t.overtopped_turf = Some(t.cur.is_some_and(|c| c.1 == d.id) && t.tenure_donor);
                }
                if d.species == PAD
                    && let Some(&class) = w.class.get(&d.site)
                    && class != Class::S1
                {
                    w.refuge_bd.entry((win, class)).or_default().1 += 1;
                }
            }
            let births = flora.view().ledger.births;
            for d in flora.deliveries() {
                if d.species != TURF {
                    continue;
                }
                let v = via_index(d.via);
                let dist = wrapped(d.origin, d.recipient, width);
                let b = DIST_BINS.iter().position(|(hi, _)| dist <= *hi).unwrap();
                w.deliveries[v][b] += 1;
                if let Some(t) = w.track.get_mut(&d.recipient) {
                    t.receipts[v] += 1;
                }
                if d.via == Via::Burr && !w.burr_pending.contains_key(&d.recipient) {
                    w.burr_pending.insert(d.recipient, births);
                    w.burr_sites += 1;
                }
            }
        }
        juv.minute(sim.flora(), sim.world().config().width);
        // The minute check: births, disappearances, tenure, burr establishment.
        {
            let fv = sim.flora().view();
            let sc = fv.config.species(TURF);
            for (site, t) in w.track.iter_mut() {
                let now = fv.stand_at(*site);
                let now_id = now.map(|s| (s.species, s.id));
                if now_id.map(|n| n.1) != t.cur.map(|c| c.1) {
                    if let Some((_, id)) = t.cur
                        && !w.died.contains(&id)
                    {
                        t.disappeared += 1;
                    }
                    t.tenure_donor = false;
                    t.woody_from_turf = false;
                    if let Some(s) = now {
                        if s.species == TURF {
                            t.turf_births += 1;
                            t.turf_seen = true;
                            t.tenure_donor = s.wood >= sc.donor_min;
                        } else if is_woody(s.species) {
                            t.woody_from_turf = t.overtopped_turf == Some(true);
                            if !t.turf_seen {
                                t.woody_direct = true;
                            }
                        } else if s.species == PAD {
                            t.pad_births += 1;
                            if let Some(&class) = w.class.get(site)
                                && class != Class::S1
                            {
                                w.refuge_bd.entry((win, class)).or_default().0 += 1;
                            }
                        }
                    }
                    t.overtopped_turf = None;
                    t.cur = now_id;
                } else if let Some(s) = now
                    && s.species == TURF
                    && s.wood >= sc.donor_min
                {
                    t.tenure_donor = true;
                }
            }
            w.burr_pending.retain(|site, at| {
                let up = fv.stand_at(*site).is_some_and(|s| s.species == TURF && s.id >= *at);
                if up {
                    w.burr_established += 1;
                }
                !up
            });
            w.cured_max = w.cured_max.max(fv.stands.iter().filter(|s| s.cured).count());
            w.died.clear();
        }
        if (minute - minute0) % every == 0 || minute == end {
            if (minute - minute0) % 120 == 0 {
                windows.push(Window {
                    ledger: sim.flora().view().ledger.clone(),
                    minute,
                });
            }
            sample(&sim, &arm, minute, &co, &s1, &refuge, &w, &wet_vacant, &saved, &windows[0]);
            juv.line(&arm, minute);
            {
                // The canopy gate and runner support, cumulative since the load.
                let l = &sim.flora().view().ledger;
                let (t, p) = (TURF.index(), PAD.index());
                println!(
                    "canopy,{arm},{minute},turf_refused={},turf_runner_refused={},turf_runner_kept={},turf_support={:.4},turf_weaned={},turf_support_ended={},pad_refused={},pad_runner_refused={},pad_support={:.4}",
                    l.canopy_refused[t], l.runner_canopy_refused[t], l.runner_kept[t], l.runner_support_organic[t],
                    l.runner_weaned[t], l.runner_support_ended[t], l.canopy_refused[p], l.runner_canopy_refused[p],
                    l.runner_support_organic[p]
                );
            }
            if with_fauna {
                fw.minute_line(&sim, &arm, minute);
            }
            species_ledger_line(&arm, minute, &windows[0].ledger, sim.flora().view().ledger);
            eprintln!("minute {minute} ({:.0} s wall)", t0.elapsed().as_secs_f64());
        }
        if MAP_MINUTES.contains(&minute)
            && let Some(f) = map.as_mut()
        {
            write_map(f, &arm, minute, &s1, &w);
        }
        if let Some(d) = save_dir
            && save_min.contains(&minute)
        {
            save(&Path::new(d).join(format!("m{minute}")), &sim, minute);
        }
        if stop_at(&mut runner, &sim, minute) {
            break;
        }
    }
    // Flux windows: the ledger's food-side lines over each two hours.
    for pair in windows.windows(2) {
        flux_line(&arm, &pair[0], &pair[1], sim.flora().view().config);
    }
    let mut bd: Vec<_> = w.refuge_bd.iter().collect();
    bd.sort_by_key(|((win, c), _)| (*win, c.name()));
    for ((win, class), (b, d)) in bd {
        println!(
            "refuge_bd,{arm},{}-{},{},births_observed={b},death_events={d}",
            minute0 + win * 120,
            minute0 + (win + 1) * 120,
            class.name()
        );
    }
    tracked_line(&arm, &w);
    deliveries_line(&arm, &w);
    plant_ledger_line(&arm, &windows[0].ledger, sim.flora().view().ledger);
    attempts(&sim, &arm, &co, &s1, &w);
    if with_fauna {
        fw.report(&sim, &params, &arm);
    }
}

fn write_map(f: &mut std::fs::File, arm: &str, minute: u64, s1: &[Site], w: &Watch) {
    use std::io::Write;
    for site in s1 {
        let t = w.track[site];
        writeln!(f, "{arm},{minute},{},{},{},{}", site.x, site.y, site.z, t.state()).unwrap();
    }
}

#[allow(clippy::too_many_arguments)]
fn sample(
    sim: &Sim,
    arm: &str,
    minute: u64,
    co: &Cohorts,
    s1: &[Site],
    refuge: &[(Site, Class)],
    w: &Watch,
    wet_vacant: &[Site],
    reference: &FloraConfig,
    start: &Window,
) {
    let fv = sim.flora().view();
    let config = fv.config;
    let area = sim.world().config().cell_area();
    // The occupant is the face's plant (`stand_at`); its fungus is counted on its own.
    let occ = |sites: &mut dyn Iterator<Item = Site>| {
        let mut v = vec![0u64; Species::COUNT + 1];
        let mut food = 0.0;
        let mut litter = 0.0;
        let mut n = 0usize;
        let mut fungi = [0u64; 2];
        // Velvetpad and springturf donors on these sites.
        let mut donors = [0u64; 2];
        for site in sites {
            n += 1;
            let st = fv.stand_at(site);
            v[occ_index(st.map(|s| s.species))] += 1;
            if fv.fungus_at(site).is_some() {
                fungi[0] += 1;
                fungi[1] += u64::from(st.is_some());
            }
            if let Some(s) = st {
                food += edible(&fv, s);
                for (k, sp) in [PAD, TURF].into_iter().enumerate() {
                    if s.species == sp && s.wood >= config.species(sp).donor_min {
                        donors[k] += 1;
                    }
                }
            }
            litter += fv.ground_at(site).map_or(0.0, |g| g.litter);
        }
        (v, food, litter, n, donors, fungi)
    };
    let line = |name: &str, (v, food, litter, n, donors, fungi): (Vec<u64>, f64, f64, usize, [u64; 2], [u64; 2])| {
        let photo: u64 = Species::ALL
            .iter()
            .filter(|s| config.species(**s).trophic == Trophic::Photo)
            .map(|s| v[s.index()])
            .sum();
        let parts: Vec<String> = (0..=Species::COUNT).filter(|&i| v[i] > 0).map(|i| format!("{}={}", occ_name(i), v[i])).collect();
        println!(
            "occ,{arm},{minute},{name},n={n},photo={photo},fungal={},both={},pad_donors={},turf_donors={},edible={food:.4},edible_per_m2={:.5},litter={litter:.4},{}",
            fungi[0],
            fungi[1],
            donors[0],
            donors[1],
            food / (n.max(1) as f64 * area),
            parts.join(",")
        );
    };
    line("S1", occ(&mut s1.iter().copied()));
    line("wet", occ(&mut refuge.iter().filter(|r| r.1 == Class::Wet).map(|r| r.0)));
    line("shaded", occ(&mut refuge.iter().filter(|r| r.1 == Class::Shaded).map(|r| r.0)));
    line("dry", occ(&mut co.pad.iter().filter(|r| r.1 == Class::Dry).map(|r| r.0)));
    line("drained", occ(&mut co.drained.iter().copied()));
    line("waterlogged_vacant_at_load", occ(&mut wet_vacant.iter().copied()));
    // The waterlogged faces on the water as it is now, by the same rule.
    {
        let view = sim.world().view();
        let now: Vec<Site> = soil_faces(sim.world())
            .into_iter()
            .filter(|s| waterlogged(&view, *s, reference))
            .collect();
        line("waterlogged_now", occ(&mut now.iter().copied()));
    }
    // S1: success is a donor-size turf in its own tenure, or a woody stand that came up
    // through one (the continuous trajectory).
    let (mut success, mut turf_now, mut direct_woody) = (0usize, 0usize, 0usize);
    for site in s1 {
        let t = w.track[site];
        success += usize::from(t.success());
        turf_now += usize::from(t.cur.is_some_and(|c| c.0 == TURF));
        direct_woody += usize::from(t.woody_direct);
    }
    println!(
        "s1,{arm},{minute},n={},success={success},share={:.4},turf_now={turf_now},direct_woody={direct_woody}",
        s1.len(),
        success as f64 / s1.len().max(1) as f64
    );
    // The world: stands, donors and edible foliage per species; litter and dead wood;
    // the mature stand's edible foliage for turf and pad (donor-size stands: mean and
    // median); cured turf.
    let mut parts = Vec::new();
    for sp in Species::ALL {
        let sc = config.species(sp);
        let st: Vec<_> = fv.stands.iter().filter(|s| s.species == sp).collect();
        if st.is_empty() {
            continue;
        }
        let donors = st.iter().filter(|s| s.wood >= sc.donor_min).count();
        let food: f64 = st.iter().map(|s| edible(&fv, s)).sum();
        parts.push(format!("{}={}/{donors}/{food:.3}", sp.name(), st.len()));
    }
    let mature = |sp: Species| {
        let sc = config.species(sp);
        let mut e: Vec<f64> = fv.stands.iter().filter(|s| s.species == sp && s.wood >= sc.donor_min).map(|s| edible(&fv, s)).collect();
        e.sort_by(f64::total_cmp);
        let mean = e.iter().sum::<f64>() / e.len().max(1) as f64;
        let median = if e.is_empty() { 0.0 } else { e[e.len() / 2] };
        (e.len(), mean, median)
    };
    let (tn, tmean, tmed) = mature(TURF);
    let (pn, pmean, pmed) = mature(PAD);
    let litter: f64 = fv.ground.iter().map(|g| g.litter).sum();
    let wood: f64 = fv.ground.iter().map(|g| g.dead_wood).sum();
    let food: f64 = fv
        .stands
        .iter()
        .filter(|s| config.species(s.species).trophic == Trophic::Photo)
        .map(|s| edible(&fv, s))
        .sum();
    let cured = fv.stands.iter().filter(|s| s.cured).count();
    let l = fv.ledger;
    let p = |a: Species, b: Species| l.overtop_pairs[a.index()][b.index()] - start.ledger.overtop_pairs[a.index()][b.index()];
    let woody_from = |a: Species| WOODY.iter().map(|&w| p(a, w)).sum::<u64>();
    println!(
        "world,{arm},{minute},edible={food:.4},litter={litter:.4},edible_plus_litter={:.4},dead_wood={wood:.4},pad_to_turf={},pad_to_turf_by_runner={},pad_to_woody={},turf_to_woody={},mature_turf_n={tn},mature_turf_edible_mean={tmean:.5},mature_turf_edible_median={tmed:.5},mature_pad_n={pn},mature_pad_edible_mean={pmean:.5},mature_pad_edible_median={pmed:.5},turf_over_pad_mean={:.3},cured_turf={cured},burrs_riding={},stands/donors/edible: {}",
        food + litter,
        p(PAD, TURF),
        l.runner_displaced[PAD.index()] - start.ledger.runner_displaced[PAD.index()],
        woody_from(PAD),
        woody_from(TURF),
        if pmean > 0.0 { tmean / pmean } else { f64::NAN },
        fv.burrs.len(),
        parts.join(" ")
    );
    // Glowcap yield (2026-10-05): glowcap stands on dark terrain (the glowcap's own
    // darkness above zero) against lit ones, which live on the litter and dead wood; the lit
    // ones on drained ground; and the glowcap faces plants took since the load, by seed and
    // by runner.
    let gsc = config.species(Species::Glowcap);
    let view = sim.world().view();
    let drained: HashSet<Site> = co.drained.iter().copied().collect();
    let (mut dark, mut lit, mut lit_drained, mut dark_drained) = (0usize, 0usize, 0usize, 0usize);
    for st in fv.stands.iter().filter(|s| s.species == Species::Glowcap) {
        let sky = view.sky_visibility(i64::from(st.site.x), st.site.y, st.site.z);
        let on_drained = drained.contains(&st.site);
        if gsc.darkness(sky) > 0.0 {
            dark += 1;
            dark_drained += usize::from(on_drained);
        } else {
            lit += 1;
            lit_drained += usize::from(on_drained);
        }
    }
    let g = Species::Glowcap.index();
    let taken: u64 = (0..Species::COUNT).map(|j| l.overtop_pairs[g][j] - start.ledger.overtop_pairs[g][j]).sum();
    // Faces holding a plant and a fungus at once (`FaceLayer`), world-wide.
    let both = fv.stands.windows(2).filter(|w| w[0].site == w[1].site).count();
    println!(
        "glowcap,{arm},{minute},dark={dark},lit={lit},dark_drained={dark_drained},lit_drained={lit_drained},both={both},taken_by_plants={taken},taken_by_runner={}",
        l.runner_displaced[g] - start.ledger.runner_displaced[g]
    );
}

fn flux_line(arm: &str, a: &Window, b: &Window, config: &FloraConfig) {
    let (la, lb) = (&a.ledger, &b.ledger);
    let d = |x: &[f64; Species::COUNT], y: &[f64; Species::COUNT], s: Species| y[s.index()] - x[s.index()];
    let sum = |x: &[f64; Species::COUNT], y: &[f64; Species::COUNT]| {
        Species::ALL
            .iter()
            .filter(|s| config.species(**s).trophic == Trophic::Photo)
            .map(|&s| d(x, y, s))
            .sum::<f64>()
    };
    let per = |x: &[f64; Species::COUNT], y: &[f64; Species::COUNT]| {
        [PAD, TURF, Species::Bloomcrown, Species::Lanternberry, Species::Stonecushion, Species::Umbrellafrond]
            .iter()
            .map(|&s| format!("{}={:.4}", s.name(), d(x, y, s)))
            .collect::<Vec<_>>()
            .join(" ")
    };
    println!(
        "flux,{arm},{}-{},built={:.4},shed={:.4},overtop_pulse={:.4},foliage_taken={:.4},litter_taken={:.4},turf_cure_shed={:.4} | built: {} | shed: {} | pulse: {}",
        a.minute,
        b.minute,
        sum(&la.foliage_built, &lb.foliage_built),
        sum(&la.shed_organic, &lb.shed_organic),
        Species::ALL.iter().map(|&s| d(&la.overtopped_organic, &lb.overtopped_organic, s)).sum::<f64>(),
        sum(&la.foliage_taken, &lb.foliage_taken),
        lb.litter_taken - la.litter_taken,
        d(&la.cure_shed_organic, &lb.cure_shed_organic, TURF),
        per(&la.foliage_built, &lb.foliage_built),
        per(&la.shed_organic, &lb.shed_organic),
        per(&la.overtopped_organic, &lb.overtopped_organic),
    );
}

/// The tracked sites' events over the run, by class: deaths **with causes** (the death
/// log), disappearances (an occupant gone with no death: removed), births observed at the
/// minute checks, and turf receipts by route.
fn tracked_line(arm: &str, w: &Watch) {
    for class in [Class::S1, Class::Wet, Class::Shaded] {
        let mut deaths = [[0u64; DeathCause::COUNT]; 2];
        let (mut gone, mut turf_b, mut pad_b) = (0u64, 0u64, 0u64);
        let mut receipts = [0u64; 3];
        let mut receiving = 0u64;
        for (site, t) in &w.track {
            if w.class.get(site) != Some(&class) {
                continue;
            }
            for k in 0..2 {
                for c in 0..DeathCause::COUNT {
                    deaths[k][c] += u64::from(t.deaths[k][c]);
                }
            }
            gone += u64::from(t.disappeared);
            turf_b += u64::from(t.turf_births);
            pad_b += u64::from(t.pad_births);
            for v in 0..3 {
                receipts[v] += u64::from(t.receipts[v]);
            }
            receiving += u64::from(t.receipts.iter().any(|r| *r > 0));
        }
        let causes = |k: usize| {
            DeathCause::ALL
                .iter()
                .filter(|c| deaths[k][c.index()] > 0)
                .map(|c| format!("{}={}", c.name(), deaths[k][c.index()]))
                .collect::<Vec<_>>()
                .join(" ")
        };
        println!(
            "tracked,{arm},{},turf_deaths=[{}],pad_deaths=[{}],disappearances={gone},turf_births_observed={turf_b},pad_births_observed={pad_b},turf_receipts_seed={},turf_receipts_runner={},turf_receipts_burr={},sites_receiving_turf={receiving}",
            class.name(),
            causes(0),
            causes(1),
            receipts[0],
            receipts[1],
            receipts[2]
        );
    }
}

/// Every turf delivery in the world over the run: counts and distance histogram by
/// route; burr landings and how many sites a turf then established on.
fn deliveries_line(arm: &str, w: &Watch) {
    for (v, name) in VIA_NAMES.iter().enumerate() {
        let n: u64 = w.deliveries[v].iter().sum();
        let bins: Vec<String> = DIST_BINS.iter().zip(w.deliveries[v]).map(|((_, b), c)| format!("{b}:{c}")).collect();
        println!("delivered,{arm},{name},n={n},distance_voxels=[{}]", bins.join(" "));
    }
    println!(
        "burr_fate,{arm},sites_landed={},turf_established_after={},share={:.4},still_waiting={}",
        w.burr_sites,
        w.burr_established,
        w.burr_established as f64 / w.burr_sites.max(1) as f64,
        w.burr_pending.len()
    );
    println!("cured,{arm},max_cured_turf_at_a_minute={}", w.cured_max);
}

/// The plant ledger's spread and cure lines over the run.
fn plant_ledger_line(arm: &str, a: &FloraLedger, b: &FloraLedger) {
    let k = TURF.index();
    let d = |x: u64, y: u64| y - x;
    let causes: Vec<String> = DeathCause::ALL
        .iter()
        .flat_map(|c| {
            [TURF, PAD].into_iter().map(move |s| {
                format!(
                    "{}_{}={}",
                    s.name(),
                    c.name(),
                    b.deaths_by_cause[c.index()][s.index()] - a.deaths_by_cause[c.index()][s.index()]
                )
            })
        })
        .collect();
    println!(
        "plantledger,{arm},burrs_picked={},burrs_dropped={},burrs_dropped_at_carcass={},burr_picked_organic={:.5},turf_runner_births={},pad_displaced_by_runner={},turf_seeds_germinated={},cures={},cure_wakes={},cure_shed={:.4},{}",
        d(a.burrs_picked[k], b.burrs_picked[k]),
        d(a.burrs_dropped[k], b.burrs_dropped[k]),
        d(a.burrs_dropped_at_carcass[k], b.burrs_dropped_at_carcass[k]),
        b.burr_picked_organic[k] - a.burr_picked_organic[k],
        d(a.clonal_births[k], b.clonal_births[k]),
        d(a.runner_displaced[PAD.index()], b.runner_displaced[PAD.index()]),
        d(a.seeds_germinated[k], b.seeds_germinated[k]),
        d(a.cures[k], b.cures[k]),
        d(a.cure_wakes[k], b.cure_wakes[k]),
        b.cure_shed_organic[k] - a.cure_shed_organic[k],
        causes.join(",")
    );
}

/// **Every species' demography since the load** (the whole-web model's flora half,
/// `design/webmodel/flora.md`): one `sp` line per species and sample, cumulative from the
/// load's ledger `a` to now `b` — seeds landed, germinated, died in the bank and lost as
/// litter; runner births and runners kept; deaths by cause; stands overtopped (and by
/// whom), runner-displaced; propagule organic funded and foliage built and shed; the
/// canopy gate's refusals. Additive logging: no rule reads it.
fn species_ledger_line(arm: &str, minute: u64, a: &FloraLedger, b: &FloraLedger) {
    for sp in Species::ALL {
        let k = sp.index();
        let d = |x: u64, y: u64| y.saturating_sub(x);
        let causes: Vec<String> = DeathCause::ALL
            .iter()
            .map(|c| format!("{}={}", c.name(), d(a.deaths_by_cause[c.index()][k], b.deaths_by_cause[c.index()][k])))
            .collect();
        let by: Vec<String> = Species::ALL
            .iter()
            .filter_map(|n| {
                let v = d(a.overtop_pairs[k][n.index()], b.overtop_pairs[k][n.index()]);
                (v > 0).then(|| format!("{}={v}", n.name()))
            })
            .collect();
        println!(
            "sp,{arm},{minute},{},landed={},germinated={},seeds_died={},seeds_lost={},runner_births={},runner_kept={},deaths=[{}],overtopped_by=[{}],runner_displaced={},funded={:.4},built={:.3},shed={:.3},canopy_refused={},runner_canopy_refused={}",
            sp.name(),
            d(a.seeds_landed[k], b.seeds_landed[k]),
            d(a.seeds_germinated[k], b.seeds_germinated[k]),
            d(a.seeds_died[k], b.seeds_died[k]),
            d(a.seeds_lost[k], b.seeds_lost[k]),
            d(a.clonal_births[k], b.clonal_births[k]),
            d(a.runner_kept[k], b.runner_kept[k]),
            causes.join(" "),
            by.join(" "),
            d(a.runner_displaced[k], b.runner_displaced[k]),
            b.propagule_funded[k] - a.propagule_funded[k],
            b.foliage_built[k] - a.foliage_built[k],
            b.shed_organic[k] - a.shed_organic[k],
            d(a.canopy_refused[k], b.canopy_refused[k]),
            d(a.runner_canopy_refused[k], b.runner_canopy_refused[k]),
        );
    }
}

/// The S1 attempt classification at the end, by reach to the nearest turf donor at the
/// load, with the trace's totals behind each class and the failed gates split.
fn attempts(sim: &Sim, arm: &str, co: &Cohorts, s1: &[Site], w: &Watch) {
    let trace = sim.flora().attempts().expect("tracing");
    const BINS: [(f64, &str); 4] = [(6.0, "<=6"), (12.0, "6-12"), (24.0, "12-24"), (f64::INFINITY, ">24")];
    const CLASSES: [&str; 13] = [
        "success",
        "turf_young",
        "turf_born_then_lost",
        "direct_woody",
        "arm_lost",
        "gate_light",
        "gate_moisture",
        "gate_saturation",
        "gate_flooding",
        "gate_other",
        "cover_changed",
        "no_traced_check",
        "other",
    ];
    let mut table = vec![[0u64; CLASSES.len()]; BINS.len()];
    let mut totals = cubarium_voxel_flora::Attempts::default();
    let mut received = vec![[0u64; 2]; BINS.len()];
    for &site in s1 {
        let t = w.track[&site];
        let a = trace.sites.get(&site).copied().unwrap_or_default();
        totals.bank_checks += a.bank_checks;
        totals.cover_forbidden += a.cover_forbidden;
        totals.gate_fail += a.gate_fail;
        totals.gate_pass += a.gate_pass;
        totals.gate_light += a.gate_light;
        totals.gate_moisture += a.gate_moisture;
        totals.gate_saturation += a.gate_saturation;
        totals.gate_flooding += a.gate_flooding;
        totals.gate_other += a.gate_other;
        totals.lottery_lost += a.lottery_lost;
        totals.draw_failed += a.draw_failed;
        totals.arms += a.arms;
        totals.arm_lost += a.arm_lost;
        totals.births_bare += a.births_bare;
        totals.births_through_cover += a.births_through_cover;
        let now_turf = t.cur.is_some_and(|c| c.0 == TURF);
        let born = a.births_bare + a.births_through_cover > 0 || t.turf_births > 0 || t.turf_seen;
        // The gate that shut most often, for a site whose checks all failed on gates.
        let gate = {
            let g = [a.gate_light, a.gate_moisture, a.gate_saturation, a.gate_flooding, a.gate_other];
            5 + g.iter().enumerate().max_by_key(|(_, v)| **v).map_or(4, |(i, _)| i)
        };
        let class = if t.success() {
            0
        } else if now_turf {
            1
        } else if born {
            2
        } else if t.woody_direct {
            3
        } else if a.gate_pass > 0 {
            4
        } else if a.gate_fail > 0 {
            gate
        } else if a.cover_forbidden > 0 {
            10
        } else if a.bank_checks == 0 {
            11
        } else {
            12
        };
        let r = co.reach[&site];
        let bin = BINS.iter().position(|(hi, _)| r <= *hi).unwrap();
        table[bin][class] += 1;
        received[bin][usize::from(t.receipts.iter().any(|r| *r > 0))] += 1;
    }
    println!("attempts,{arm},reach,{},no_receipt,receipt", CLASSES.join(","));
    for (b, row) in table.iter().enumerate() {
        println!(
            "attempts,{arm},{},{},{},{}",
            BINS[b].1,
            row.iter().map(|v| v.to_string()).collect::<Vec<_>>().join(","),
            received[b][0],
            received[b][1]
        );
    }
    println!(
        "trace,{arm},bank_checks={},cover_forbidden={},gate_fail={},gate_light={},gate_moisture={},gate_saturation={},gate_flooding={},gate_other={},gate_pass={},lottery_lost={},draw_failed={},arms={},arm_lost={},births_bare={},births_through_cover={}",
        totals.bank_checks,
        totals.cover_forbidden,
        totals.gate_fail,
        totals.gate_light,
        totals.gate_moisture,
        totals.gate_saturation,
        totals.gate_flooding,
        totals.gate_other,
        totals.gate_pass,
        totals.lottery_lost,
        totals.draw_failed,
        totals.arms,
        totals.arm_lost,
        totals.births_bare,
        totals.births_through_cover
    );
}

// ------------------------------------------------------------------- fauna watch

/// The fauna's books over a run, in two halves and whole: per lineage B and its parts,
/// and the Veilhound's (`chorister`'s) kills, carcasses, bites and assimilation
/// (finding 9).
struct FaunaWatch {
    half: u64,
    seen: HashSet<u64>,
    born: [[u64; Founder::COUNT]; 2],
    adult_ticks: [[u64; Founder::COUNT]; 2],
    body_ticks: [[u64; Founder::COUNT]; 2],
    books0: Vec<balance::Books>,
    books_mid: Vec<balance::Books>,
    l0: cubarium_voxel_fauna::FaunaLedger,
    l_mid: cubarium_voxel_fauna::FaunaLedger,
    fl0: FloraLedger,
    fl_mid: FloraLedger,
    ticks: u64,
}

impl FaunaWatch {
    fn new(sim: &Sim, half: u64) -> FaunaWatch {
        let books: Vec<balance::Books> = Founder::ALL.iter().map(|&f| balance::Books::of(sim.fauna().view().ledger, f)).collect();
        FaunaWatch {
            half,
            seen: sim.fauna().view().animals.iter().map(|a| a.id).collect(),
            born: [[0; Founder::COUNT]; 2],
            adult_ticks: [[0; Founder::COUNT]; 2],
            body_ticks: [[0; Founder::COUNT]; 2],
            books0: books.clone(),
            books_mid: books,
            l0: sim.fauna().view().ledger.clone(),
            l_mid: sim.fauna().view().ledger.clone(),
            fl0: sim.flora().view().ledger.clone(),
            fl_mid: sim.flora().view().ledger.clone(),
            ticks: 0,
        }
    }

    fn tick(&mut self, sim: &Sim, params: &Params, tick: u64) {
        self.ticks = tick;
        let w = usize::from(tick > self.half);
        for a in sim.fauna().view().animals {
            let Some(f) = a.founder else { continue };
            self.body_ticks[w][f.index()] += 1;
            if balance::is_adult(a, &params.fauna) {
                self.adult_ticks[w][f.index()] += 1;
            }
            if self.seen.insert(a.id) {
                self.born[w][f.index()] += 1;
            }
        }
        if tick == self.half {
            self.books_mid = Founder::ALL.iter().map(|&f| balance::Books::of(sim.fauna().view().ledger, f)).collect();
            self.l_mid = sim.fauna().view().ledger.clone();
            self.fl_mid = sim.flora().view().ledger.clone();
            println!("stocks,mid,{},{}", tick / u64::from(TICK_HZ) / 60, stocks(sim));
            println!("bodies,mid,{},{}", tick / u64::from(TICK_HZ) / 60, bodies(sim, params));
        }
    }

    /// One line per sample for the capgnawer (glowcap yield, 2026-10-05): animals now,
    /// births, `B` over the run so far, and its bites by food since the load.
    fn minute_line(&self, sim: &Sim, arm: &str, minute: u64) {
        let v = sim.fauna().view();
        let l = v.ledger;
        let f = Founder::Capgnawer;
        let i = f.index();
        let hzf = f64::from(TICK_HZ);
        let n = v.animals.iter().filter(|a| a.founder == Some(f)).count();
        let born: u64 = self.born.iter().map(|b| b[i]).sum();
        let at: u64 = self.adult_ticks.iter().map(|b| b[i]).sum();
        let books = balance::Books::of(l, f).since(&self.books0[i]);
        let days = balance::days(self.ticks as f64 / hzf, v.config);
        let ctx = balance::context(v.animals, v.config, f, balance::days(at as f64 / hzf, v.config), days);
        let m = balance::measures(&books, &ctx);
        let bites = |food: Food| l.bites_by_founder_food[i][food.index()] - self.l0.bites_by_founder_food[i][food.index()];
        println!(
            "capgnawer,{arm},{minute},n={n},born={born},B={:.3},assim={:.6},spend={:.6},bites_cap={},bites_litter={}",
            m.energy_balance,
            books.assimilated,
            books.spend(),
            bites(Food::CapTissue),
            bites(Food::Litter),
        );
    }

    fn report(&self, sim: &Sim, params: &Params, arm: &str) {
        let v = sim.fauna().view();
        let l = v.ledger;
        let total = self.ticks;
        let half = self.half.min(total);
        println!("stocks,{arm},end,{}", stocks(sim));
        println!("bodies,{arm},end,{}", bodies(sim, params));
        let books_end: Vec<balance::Books> = Founder::ALL.iter().map(|&f| balance::Books::of(l, f)).collect();
        let hzf = f64::from(TICK_HZ);
        type Span<'a> = (&'a str, &'a Vec<balance::Books>, &'a Vec<balance::Books>, &'a cubarium_voxel_fauna::FaunaLedger, &'a cubarium_voxel_fauna::FaunaLedger, f64, [usize; 2]);
        let spans: [Span<'_>; 3] = [
            ("early", &self.books0, &self.books_mid, &self.l0, &self.l_mid, half as f64 / hzf, [0, 1]),
            ("late", &self.books_mid, &books_end, &self.l_mid, l, (total - half) as f64 / hzf, [1, 2]),
            ("all", &self.books0, &books_end, &self.l0, l, total as f64 / hzf, [0, 2]),
        ];
        for (name, b0, b1, la, lb, secs, [w0, w1]) in spans {
            let days = balance::days(secs, v.config);
            for f in Founder::ALL {
                let i = f.index();
                let bt: u64 = (w0..w1).map(|w| self.body_ticks[w][i]).sum();
                let br: u64 = (w0..w1).map(|w| self.born[w][i]).sum();
                if bt == 0 && br == 0 {
                    continue;
                }
                let at: u64 = (w0..w1).map(|w| self.adult_ticks[w][i]).sum();
                let books = b1[i].since(&b0[i]);
                let adult_days = balance::days(at as f64 / hzf, v.config);
                let ctx = balance::context(v.animals, v.config, f, adult_days, days);
                let m = balance::measures(&books, &ctx);
                let deaths: Vec<String> = Departure::ALL
                    .iter()
                    .filter_map(|d| {
                        let n = lb.deaths_by_founder_cause[i][d.index()] - la.deaths_by_founder_cause[i][d.index()];
                        (n > 0).then(|| format!("{}={n}", d.name()))
                    })
                    .collect();
                let bites: Vec<String> = Food::ALL
                    .iter()
                    .filter_map(|food| {
                        let n = lb.bites_by_founder_food[i][food.index()] - la.bites_by_founder_food[i][food.index()];
                        (n > 0).then(|| format!("{}={n}", food.name()))
                    })
                    .collect();
                println!(
                    "balance,{arm},{name},{},B={:.3},r_hat={:.3},access={:.3},sated={:.3},assim={:.6},spend={:.6},born={br},eggs_laid={},eggs_hatched={},deaths=[{}],bites=[{}],body_days={:.3}",
                    f.name(),
                    m.energy_balance,
                    m.r_hat,
                    m.access,
                    m.sated_share,
                    books.assimilated,
                    books.spend(),
                    lb.eggs_laid_by_founder[i] - la.eggs_laid_by_founder[i],
                    lb.eggs_hatched_by_founder[i] - la.eggs_hatched_by_founder[i],
                    deaths.join(" "),
                    bites.join(" "),
                    balance::days(bt as f64 / hzf, v.config),
                );
            }
            // The Veilhound: kills, the carcasses the roster produced (every death but a
            // removal leaves carrion), the carrion it ate, its bites and assimilation.
            let c = Founder::Chorister.index();
            let removed = Departure::Removed.index();
            let carcasses: u64 = Founder::ALL
                .iter()
                .map(|q| {
                    let qi = q.index();
                    (0..Departure::COUNT)
                        .filter(|&d| d != removed)
                        .map(|d| lb.deaths_by_founder_cause[qi][d] - la.deaths_by_founder_cause[qi][d])
                        .sum::<u64>()
                })
                .sum();
            println!(
                "veilhound,{arm},{name},kills={},pack_chases={},pack_chase_kills={},carcasses_all={carcasses},carrion_eaten={:.5},carrion_bites={},bites={},assimilated={:.6}",
                balance::kills(la, lb, Founder::Chorister),
                lb.pack_chases - la.pack_chases,
                lb.pack_chase_kills - la.pack_chase_kills,
                lb.eaten_carrion_by_founder[c] - la.eaten_carrion_by_founder[c],
                lb.bites_by_founder_food[c][Food::Carrion.index()] - la.bites_by_founder_food[c][Food::Carrion.index()],
                lb.bites_by_founder[c] - la.bites_by_founder[c],
                lb.assimilated_by_founder[c] - la.assimilated_by_founder[c],
            );
        }
        let fl = sim.flora().view().ledger;
        for (name, a, b) in [("early", &self.fl0, &self.fl_mid), ("late", &self.fl_mid, fl)] {
            let sum = |x: &[f64; Species::COUNT], y: &[f64; Species::COUNT]| (0..Species::COUNT).map(|i| y[i] - x[i]).sum::<f64>();
            println!(
                "plantflux,{arm},{name},built={:.4},shed={:.4},overtop_pulse={:.4},foliage_taken={:.4},litter_taken={:.4},pad_to_turf={}",
                sum(&a.foliage_built, &b.foliage_built),
                sum(&a.shed_organic, &b.shed_organic),
                sum(&a.overtopped_organic, &b.overtopped_organic),
                sum(&a.foliage_taken, &b.foliage_taken),
                b.litter_taken - a.litter_taken,
                b.overtop_pairs[PAD.index()][TURF.index()] - a.overtop_pairs[PAD.index()][TURF.index()],
            );
        }
    }
}

// -------------------------------------------------------------------------- fauna

/// Every support face, sorted low to high (`habitat::support_sites`'s order).
fn support_sites(world: &World) -> Vec<Site> {
    let view = world.view();
    let c = world.config();
    let mut out: Vec<Site> = Vec::new();
    for z in 0..c.depth {
        for x in 0..c.width as i64 {
            for y in view.supports_in_column(x, z) {
                out.push(Site {
                    x: x.rem_euclid(i64::from(c.width)) as u32,
                    y,
                    z,
                });
            }
        }
    }
    out.sort_by_key(|s| (s.y, s.x, s.z));
    out
}

/// One readout of the fauna's food: edible foliage (photosynthetic stands), litter, dead
/// wood, film, nectar and fruit, world totals.
fn stocks(sim: &Sim) -> String {
    let fv = sim.flora().view();
    let foliage: f64 = fv
        .stands
        .iter()
        .filter(|s| fv.config.species(s.species).trophic == Trophic::Photo)
        .map(|s| edible(&fv, s))
        .sum();
    let litter: f64 = fv.ground.iter().map(|g| g.litter).sum();
    let wood: f64 = fv.ground.iter().map(|g| g.dead_wood).sum();
    let film: f64 = fv.film_sites().map(|(_, f)| f).sum();
    let nectar: f64 = fv.stands.iter().map(|s| s.nectar).sum::<f64>() + fv.cover.faces().iter().map(|c| c.nectar).sum::<f64>();
    let fruit: f64 = fv.stands.iter().map(|s| s.fruit).sum::<f64>() + fv.cover.faces().iter().map(|c| c.fruit).sum::<f64>();
    format!("edible_foliage={foliage:.4} litter={litter:.4} dead_wood={wood:.4} film={film:.4} nectar={nectar:.4} fruit={fruit:.4}")
}

/// Mean reserve fraction and mean body per lineage, and the count.
fn bodies(sim: &Sim, params: &Params) -> String {
    let v = sim.fauna().view();
    let mut parts = Vec::new();
    for f in Founder::ALL {
        let p = params.fauna.founder(f);
        let a: Vec<_> = v.animals.iter().filter(|a| a.founder == Some(f)).collect();
        if a.is_empty() {
            parts.push(format!("{}=0", f.name()));
            continue;
        }
        let n = a.len() as f64;
        let res = a.iter().map(|a| a.reserve / p.core.reserve_of(a.body).max(1e-300)).sum::<f64>() / n;
        let body = a.iter().map(|a| a.body).sum::<f64>() / n;
        parts.push(format!("{}={}/{res:.3}/{body:.4}", f.name(), a.len()));
    }
    parts.join(" ")
}

fn fauna(args: &[String]) {
    let hours: f64 = arg(args, "hours=").map_or(2.0, |h| h.parse().unwrap());
    let arm = arg(args, "arm=").unwrap_or("arm").to_string();
    let params = resolve(args);
    let want = params.flora_for_world();
    let dir = Path::new(arg(args, "load=").expect("load=DIR"));
    let (mut world, mut flora, mut fauna, minute0, _) = load(dir, &want);
    // A resumed arm already holds its introduced animals.
    if !staged::resumed(args) {
        introduce(&arm, minute0, &params, &mut world, &mut flora, &mut fauna);
    }
    let mut sim = Sim::ambient(world, flora, fauna, SimConfig::experiment());
    let mut runner = stage(args, &mut sim, minute0, minute0 + (hours * 60.0).round() as u64);
    let hours = runner.as_ref().map_or(hours, |r| (r.end_minute() - minute0) as f64 / 60.0);
    fauna_run(&arm, hours, &params, &mut sim, minute0, &mut runner);
}

/// The introduction: the seeder's own fauna placement on the world as it stands, with no
/// plant founders (every producer, glowcap, vine and film founding zeroed), then the
/// layer's own config back.
fn introduce(arm: &str, minute0: u64, params: &Params, world: &mut World, flora: &mut Flora, fauna: &mut Fauna) {
    let want = flora.config().clone();
    let mut founding = want.clone();
    for sp in Species::ALL {
        let sc = founding.species_mut(sp);
        sc.founder_per_m2 = 0.0;
        sc.founder_cap = 0;
    }
    founding.latticevine.founders = 0;
    founding.latticevine.founders_per_m2 = 0.0;
    founding.seed_film_at_founding = false;
    flora.replace_config(founding).expect("founding config");
    let sites = support_sites(&world);
    let wettest = {
        let view = world.view();
        sites.iter().map(|s| view.water_depth_m(i64::from(s.x), s.y, s.z)).collect()
    };
    let pre = cubarium_voxel_sim::habitat::PreRoll {
        sites,
        wettest,
        ..Default::default()
    };
    let stands_before = flora.view().stands.len();
    let seeded_before = flora.view().ledger.seeded_organic_in;
    let counts = params.fauna.founder_counts();
    let seeded = cubarium_voxel_sim::habitat::seed_pre_rolled(world, flora, fauna, &pre, counts);
    assert_eq!(flora.view().stands.len(), stands_before, "no plant was founded");
    flora.replace_config(want.clone()).expect("the arm's config");
    println!(
        "introduced,{arm},{minute0},asked={:?},placed={:?},shortfall={:?},founding_eggs={},litter_tiles={},seeded_organic={:.4},imported_organic={:.4}",
        counts,
        seeded.founders,
        seeded.shortfall,
        seeded.founding_eggs,
        seeded.litter_tiles,
        flora.view().ledger.seeded_organic_in - seeded_before,
        seeded.imported_organic
    );
}

/// The earlier study's fauna window: `hours` with the heuristics, one line per lineage per
/// window (first and second half) and over the run.
fn fauna_run(arm: &str, hours: f64, params: &Params, sim: &mut Sim, minute0: u64, runner: &mut Option<staged::Runner>) {
    let hz = u64::from(TICK_HZ);
    let total = (hours * 3600.0).round() as u64 * hz;
    let half = total / 2;
    let mut seen: std::collections::HashSet<u64> = sim.fauna().view().animals.iter().map(|a| a.id).collect();
    let mut born = [[0u64; Founder::COUNT]; 2];
    let mut adult_ticks = [[0u64; Founder::COUNT]; 2];
    let mut body_ticks = [[0u64; Founder::COUNT]; 2];
    let books0: Vec<balance::Books> = Founder::ALL.iter().map(|&f| balance::Books::of(sim.fauna().view().ledger, f)).collect();
    let mut books_mid = books0.clone();
    let l0 = sim.fauna().view().ledger.clone();
    let mut l_mid = l0.clone();
    let fl0 = sim.flora().view().ledger.clone();
    let mut fl_mid = fl0.clone();
    println!("stocks,{arm},0,{}", stocks(&sim));
    println!("bodies,{arm},0,{}", bodies(&sim, &params));
    let t0 = std::time::Instant::now();
    let per_min = 60 * hz;
    for tick in 1..=total {
        sim.step();
        if let Some(r) = runner.as_mut() {
            r.tick(sim);
        }
        if tick % per_min == 0 && stop_at(runner, sim, minute0 + tick / per_min) {
            break;
        }
        let w = usize::from(tick > half);
        for a in sim.fauna().view().animals {
            let Some(f) = a.founder else { continue };
            body_ticks[w][f.index()] += 1;
            if balance::is_adult(a, &params.fauna) {
                adult_ticks[w][f.index()] += 1;
            }
            if seen.insert(a.id) {
                born[w][f.index()] += 1;
            }
        }
        if tick == half {
            books_mid = Founder::ALL.iter().map(|&f| balance::Books::of(sim.fauna().view().ledger, f)).collect();
            l_mid = sim.fauna().view().ledger.clone();
            fl_mid = sim.flora().view().ledger.clone();
            println!("stocks,{arm},{},{}", tick / hz / 60, stocks(&sim));
            println!("bodies,{arm},{},{}", tick / hz / 60, bodies(&sim, &params));
        }
        if tick % (600 * hz) == 0 {
            eprintln!("fauna minute {} ({:.0} s wall)", tick / hz / 60, t0.elapsed().as_secs_f64());
        }
    }
    println!("stocks,{arm},{},{}", total / hz / 60, stocks(&sim));
    println!("bodies,{arm},{},{}", total / hz / 60, bodies(&sim, &params));
    let v = sim.fauna().view();
    let l = v.ledger;
    let books_end: Vec<balance::Books> = Founder::ALL.iter().map(|&f| balance::Books::of(l, f)).collect();
    let hzf = f64::from(TICK_HZ);
    // Windows: early (first half), late (second half), all.
    let spans: [(&str, &Vec<balance::Books>, &Vec<balance::Books>, &cubarium_voxel_fauna::FaunaLedger, &cubarium_voxel_fauna::FaunaLedger, f64, [usize; 2]); 3] = [
        ("early", &books0, &books_mid, &l0, &l_mid, half as f64 / hzf, [0, 1]),
        ("late", &books_mid, &books_end, &l_mid, l, (total - half) as f64 / hzf, [1, 2]),
        ("all", &books0, &books_end, &l0, l, total as f64 / hzf, [0, 2]),
    ];
    for (name, b0, b1, la, lb, secs, [w0, w1]) in spans {
        let days = balance::days(secs, v.config);
        for f in Founder::ALL {
            let i = f.index();
            let bt: u64 = (w0..w1).map(|w| body_ticks[w][i]).sum();
            let br: u64 = (w0..w1).map(|w| born[w][i]).sum();
            if bt == 0 && br == 0 {
                continue;
            }
            let at: u64 = (w0..w1).map(|w| adult_ticks[w][i]).sum();
            let books = b1[i].since(&b0[i]);
            let adult_days = balance::days(at as f64 / hzf, v.config);
            let ctx = balance::context(v.animals, v.config, f, adult_days, days);
            let m = balance::measures(&books, &ctx);
            let deaths: Vec<String> = Departure::ALL
                .iter()
                .filter_map(|d| {
                    let n = lb.deaths_by_founder_cause[i][d.index()] - la.deaths_by_founder_cause[i][d.index()];
                    (n > 0).then(|| format!("{}={n}", d.name()))
                })
                .collect();
            let bites: Vec<String> = Food::ALL
                .iter()
                .filter_map(|food| {
                    let n = lb.bites_by_founder_food[i][food.index()] - la.bites_by_founder_food[i][food.index()];
                    (n > 0).then(|| format!("{}={n}", food.name()))
                })
                .collect();
            println!(
                "balance,{arm},{name},{},B={:.3},r_hat={:.3},access={:.3},sated={:.3},assim={:.6},spend={:.6},born={br},eggs_laid={},eggs_hatched={},deaths=[{}],bites=[{}],body_days={:.3}",
                f.name(),
                m.energy_balance,
                m.r_hat,
                m.access,
                m.sated_share,
                books.assimilated,
                books.spend(),
                lb.eggs_laid_by_founder[i] - la.eggs_laid_by_founder[i],
                lb.eggs_hatched_by_founder[i] - la.eggs_hatched_by_founder[i],
                deaths.join(" "),
                bites.join(" "),
                balance::days(bt as f64 / hzf, v.config),
            );
        }
    }
    let fl = sim.flora().view().ledger;
    for (name, a, b) in [("early", &fl0, &fl_mid), ("late", &fl_mid, fl)] {
        let sum = |x: &[f64; Species::COUNT], y: &[f64; Species::COUNT]| (0..Species::COUNT).map(|i| y[i] - x[i]).sum::<f64>();
        println!(
            "plantflux,{arm},{name},built={:.4},shed={:.4},overtop_pulse={:.4},foliage_taken={:.4},litter_taken={:.4},pad_to_turf={}",
            sum(&a.foliage_built, &b.foliage_built),
            sum(&a.shed_organic, &b.shed_organic),
            sum(&a.overtopped_organic, &b.overtopped_organic),
            sum(&a.foliage_taken, &b.foliage_taken),
            b.litter_taken - a.litter_taken,
            b.overtop_pairs[PAD.index()][TURF.index()] - a.overtop_pairs[PAD.index()][TURF.index()],
        );
    }
}

// --------------------------------------------------------------------------- fate

fn read_save(dir: &Path) -> (World, Flora) {
    let read = |n: &str| std::fs::read(dir.join(n)).unwrap_or_else(|e| panic!("{}/{n}: {e}", dir.display()));
    let world = World::load(&read("world.bin")).expect("world");
    let flora = cubarium_voxel_flora::snapshot::decode(&read("flora.bin")).expect("flora");
    (world, flora)
}

/// Each soil face's wetness on `world` by `reference`'s springturf and velvetpad boxes:
/// (site, turf's aeration or depth gate fails, velvetpad's box saturated >= 0.6), on the
/// faces under no more standing water than drowns a velvetpad.
fn wetness(world: &World, reference: &FloraConfig) -> Vec<(Site, bool, bool)> {
    let view = world.view();
    let turf = reference.species(TURF);
    let pad = reference.species(PAD);
    soil_faces(world)
        .into_iter()
        .map(|site| {
            let sky = view.sky_visibility(site.x as i64, site.y, site.z);
            let t = establishment_gates_with_sky(&view, site, turf, sky, 0.0, 0.0);
            let p = establishment_gates_with_sky(&view, site, pad, sky, 0.0, 0.0);
            (site, !(t.aeration_ok && t.depth_ok), p.saturated_fraction >= 0.6, p.water_depth_m <= pad.drown_depth_m)
        })
        // Damp, not flooded: a face under more water than drowns a velvetpad is the lake.
        .filter(|w| w.3)
        .map(|(s, a, b, _)| (s, a, b))
        .collect()
}

fn fate(args: &[String]) {
    let from = Path::new(arg(args, "from=").expect("from=DIR"));
    let to = Path::new(arg(args, "to=").expect("to=DIR"));
    let (w0, f0) = read_save(from);
    let (w1, f1) = read_save(to);
    let reference = f0.config().clone();
    let occ = |f: &Flora, site: Site| occ_index(f.view().stand_at(site).map(|s| s.species));
    let tag = format!("{}->{}", from.display(), to.display());
    for (name, pick) in [("wet", 0usize), ("soggy", 1)] {
        let mut m: HashMap<(usize, usize), u64> = HashMap::new();
        for (site, wet, soggy) in wetness(&w0, &reference) {
            if [wet, soggy][pick] {
                *m.entry((occ(&f0, site), occ(&f1, site))).or_default() += 1;
            }
        }
        for from_occ in [TURF.index(), PAD.index()] {
            let total: u64 = m.iter().filter(|((a, _), _)| *a == from_occ).map(|(_, v)| v).sum();
            let mut parts: Vec<(usize, u64)> = m.iter().filter(|((a, _), _)| *a == from_occ).map(|((_, b), v)| (*b, *v)).collect();
            parts.sort();
            println!(
                "fate,{tag},{name},from={},n={total},{}",
                occ_name(from_occ),
                parts.iter().map(|(b, v)| format!("{}={v}", occ_name(*b))).collect::<Vec<_>>().join(",")
            );
        }
        // The same class on the `to` save's own water.
        let mut v = vec![0u64; Species::COUNT + 1];
        let mut n = 0;
        for (site, wet, soggy) in wetness(&w1, &reference) {
            if [wet, soggy][pick] {
                n += 1;
                v[occ(&f1, site)] += 1;
            }
        }
        println!(
            "wetnow,{tag},{name},n={n},springturf={},velvetpad={},vacant={},other={}",
            v[TURF.index()],
            v[PAD.index()],
            v[Species::COUNT],
            n as u64 - v[TURF.index()] - v[PAD.index()] - v[Species::COUNT]
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// **A loaded save's animals are driven.** Controllers are session state and the
    /// snapshot skips them, so a layer read back from bytes has bodies with nothing
    /// driving them: every one rests where it stands, never steps onto a new face, and
    /// so never meets a burr donor or a bite. The study's loader puts the founding's
    /// heuristics back on every standing founder body (and on births, through the
    /// lineage recipes).
    #[test]
    fn a_loaded_save_drives_every_founder_body() {
        let mut world = cubarium_voxel_sim::scene::authored(cubarium_voxel::Config {
            width: 64,
            height: 24,
            depth: 6,
            ..cubarium_voxel::Config::default()
        });
        let mut flora = Flora::new(Default::default());
        let mut fauna = Fauna::new(cubarium_voxel_fauna::FaunaConfig::placeholder());
        cubarium_voxel_sim::habitat::seed(&mut world, &mut flora, &mut fauna);
        let dir = std::env::temp_dir().join(format!("succession-study-load-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("world.bin"), world.save()).unwrap();
        std::fs::write(dir.join("flora.bin"), cubarium_voxel_flora::snapshot::encode(&flora)).unwrap();
        std::fs::write(dir.join("fauna.bin"), fauna.save()).unwrap();
        std::fs::write(dir.join("minute"), "0").unwrap();
        let (_, _, mut loaded, _) = load_raw(&dir);
        std::fs::remove_dir_all(&dir).ok();
        let bodies: Vec<(u64, Founder)> =
            loaded.view().animals.iter().filter_map(|a| a.founder.map(|f| (a.id, f))).collect();
        assert!(!bodies.is_empty(), "the seeded habitat has founder bodies");
        for (id, founder) in &bodies {
            assert!(loaded.has_founder_factory(*founder), "{founder:?} births are driven");
            assert!(loaded.take_controller(*id).is_some(), "body {id} ({founder:?}) is driven");
        }
    }
}
