//! The plant succession study (`design/handoffs/plant-succession-study-2026-10-03.md`,
//! Astra's review `design/7_Research/astra-plant-succession-review-2026-10-03.md`).
//!
//! ```text
//! succession_study found  seed=N save=DIR [minutes=120] [threads=T] [config=FILE] [set=P=V ...]
//! succession_study plants load=DIR/mM arm=NAME hours=H [every=30] [save=DIR save_min=M,..] [set=...]
//! succession_study fauna  load=DIR/mM arm=NAME hours=H [set=...]
//! ```
//!
//! Every subcommand resolves the layers as `velvet_niche` does: code defaults, the file's
//! `[flora]`/`[fauna]` (default `config/desktop/terrarium.toml`), then `set=` lines and a
//! `sets=` file. `load=` **replaces** the saved plant config with the resolved one.
//!
//! - `found`: the plants-only terrarium (no fauna) from founding to `minutes`, saved at the
//!   end (`DIR/mM`). One per seed: the shared baseline save every arm branches from.
//! - `plants`: branch a save under the resolved config. The cohorts are **fixed at the
//!   load**, from the save alone, by the rule below, before any step: the same sites in
//!   every arm. Samples every `every` minutes; a summary at the end.
//! - `fauna`: branch a save, introduce the code-default founding table through the
//!   seeder's own fauna placement (`habitat::seed_pre_rolled`, with every plant founder
//!   zeroed so nothing is planted), and run with the heuristics, one line per lineage per
//!   window (first and second half) and over the run.
//!
//! **Cohorts** (on soil support faces, read at the load; springturf's own gates):
//! - `S1` (succession): velvetpad-held, springturf's water, aeration and standing-water
//!   gates pass, its light gate (terrain sky ≥ `establish_light_min`) passes, and the raw
//!   light at a turf crown's top `L = sky · canopy ≥ l_min` (0.5: the r2 grid's adult and
//!   newborn viability for springturf).
//! - `S2` (refuge): velvetpad-held and **wet** (turf's aeration or standing-water gate
//!   fails) or **shaded** (turf's water gates pass, its sky gate or the `l_min` band
//!   fails). A velvetpad site failing only turf's water minimum is **dry**, reported apart.
//! - `D` (drained ground, S3): every soil face, any occupant, where turf's water, aeration
//!   and standing-water gates pass.

use std::collections::HashMap;
use std::path::Path;

use cubarium_search::es::voxel::live::balance;
use cubarium_voxel::{Material, World};
use cubarium_voxel_fauna::{Departure, Fauna, Food, Founder, TICK_HZ};
use cubarium_voxel_flora::study::canopy_at;
use cubarium_voxel_flora::{Flora, FloraConfig, FloraLedger, Site, Species, Trophic, establishment_gates_with_sky};
use cubarium_voxel_sim::params::{Layers, Params};
use cubarium_voxel_sim::{Sim, SimConfig};

const WOODY: [Species; 3] = [Species::Bloomcrown, Species::Lanternberry, Species::Vaulttree];
const TURF: Species = Species::Springturf;
const PAD: Species = Species::Velvetpad;

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

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if let Some(n) = arg(&args, "threads=").and_then(|s| s.parse().ok()) {
        cubarium_voxel::set_thread_override(n);
    }
    match args.get(1).map(String::as_str) {
        Some("found") => found(&args),
        Some("plants") => plants(&args),
        Some("fauna") => fauna(&args),
        _ => panic!("usage: succession_study found|plants|fauna [...]"),
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

/// A save, its plant config replaced by `want`.
fn load(dir: &Path, want: &FloraConfig) -> (World, Flora, Fauna, u64) {
    let read = |n: &str| std::fs::read(dir.join(n)).unwrap_or_else(|e| panic!("{}/{n}: {e}", dir.display()));
    let world = World::load(&read("world.bin")).expect("world");
    let mut flora = cubarium_voxel_flora::snapshot::decode(&read("flora.bin")).expect("flora");
    let fauna = Fauna::load(&read("fauna.bin")).expect("fauna");
    let minute: u64 = String::from_utf8(read("minute")).unwrap().trim().parse().unwrap();
    flora.replace_config(want.clone()).expect("the resolved config validates");
    assert!(flora.config() == want, "the configuration replacement took");
    let t = flora.config().species(TURF);
    eprintln!(
        "loaded {} at minute {minute}; springturf overtop {} covers {:?}; woody overtop {:?}",
        dir.display(),
        t.overtop_per_check,
        t.overtop_covers,
        WOODY.map(|w| flora.config().species(w).overtop_per_check)
    );
    (world, flora, fauna, minute)
}

// -------------------------------------------------------------------------- found

fn found(args: &[String]) {
    let params = resolve(args);
    let seed: u64 = arg(args, "seed=").map_or(1, |s| s.parse().unwrap());
    let minutes: u64 = arg(args, "minutes=").map_or(120, |m| m.parse().unwrap());
    let dir = Path::new(arg(args, "save=").expect("save=DIR"));
    let wc = params.world.clone();
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
        founded.seeded.stands
    );
    let mut sim = Sim::ambient(founded.world, founded.flora, founded.fauna, SimConfig::default());
    let per_min = 60 * u64::from(TICK_HZ);
    let t0 = std::time::Instant::now();
    for minute in 1..=minutes {
        for _ in 0..per_min {
            sim.step();
        }
        if minute % 10 == 0 {
            eprintln!("minute {minute} ({:.0} s wall)", t0.elapsed().as_secs_f64());
        }
    }
    save(&dir.join(format!("m{minutes}")), &sim, minutes);
}

// ------------------------------------------------------------------------ cohorts

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
enum Class {
    S1,
    Wet,
    Shaded,
    Dry,
}

struct Cohorts {
    /// Velvetpad-held sites at the load, by class.
    pad: Vec<(Site, Class)>,
    /// Drained ground (any occupant): turf's water gates pass.
    drained: Vec<Site>,
    /// Soil faces in all.
    soil: usize,
    /// Each S1 site's distance to the nearest springturf donor at the load, voxels
    /// (horizontal, the ring wrapped).
    reach: HashMap<Site, f64>,
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

fn cohorts(world: &World, flora: &Flora, l_min: f64) -> Cohorts {
    let view = world.view();
    let c = view.config;
    let fv = flora.view();
    let sc = fv.config.species(TURF);
    let top = sc.crown_height(sc.wood_max, c.voxel_m);
    let mut out = Cohorts {
        pad: Vec::new(),
        drained: Vec::new(),
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
                .map(|d| {
                    let dx = (f64::from(d.x) - f64::from(site.x)).rem_euclid(w);
                    let dx = dx.min(w - dx);
                    let dz = f64::from(d.z) - f64::from(site.z);
                    (dx * dx + dz * dz).sqrt()
                })
                .fold(f64::INFINITY, f64::min);
            out.reach.insert(site, d);
        }
    }
    out
}

// ------------------------------------------------------------------------- plants

/// What each S1 site has been through, from the per-minute occupancy.
#[derive(Clone, Copy, Default)]
struct Track {
    turf_seen: bool,
    turf_donor_seen: bool,
    /// A woody stand seen after turf was.
    woody_after_turf: bool,
    /// A woody stand seen before any turf.
    woody_direct: bool,
    /// Turf births seen (a new turf stand id).
    turf_ids: u32,
    last_id: Option<u64>,
}

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

fn plants(args: &[String]) {
    let hours: f64 = arg(args, "hours=").map_or(4.0, |h| h.parse().unwrap());
    let every: u64 = arg(args, "every=").map_or(30, |m| m.parse().unwrap());
    let l_min: f64 = arg(args, "l_min=").map_or(0.5, |v| v.parse().unwrap());
    let arm = arg(args, "arm=").unwrap_or("arm").to_string();
    let params = resolve(args);
    let want = params.flora_for_world();
    let dir = Path::new(arg(args, "load=").expect("load=DIR"));
    let (world, mut flora, fauna, minute0) = load(dir, &want);
    // The cohorts, from the save alone, before any step.
    let co = cohorts(&world, &flora, l_min);
    let count = |k: Class| co.pad.iter().filter(|(_, c)| *c == k).count();
    let pads = co.pad.len();
    println!(
        "cohort,{arm},{minute0},soil={},pad_sites={pads},s1={},wet={},shaded={},dry={},drained={},s1_share_of_pad={:.4}",
        co.soil,
        count(Class::S1),
        count(Class::Wet),
        count(Class::Shaded),
        count(Class::Dry),
        co.drained.len(),
        count(Class::S1) as f64 / pads.max(1) as f64
    );
    flora.trace_attempts(Some(TURF));
    let mut sim = Sim::ambient(world, flora, fauna, SimConfig::default());
    let save_dir = arg(args, "save=");
    let save_min: Vec<u64> = arg(args, "save_min=").map_or(Vec::new(), |m| m.split(',').map(|v| v.parse().unwrap()).collect());
    let s1: Vec<Site> = co.pad.iter().filter(|(_, c)| *c == Class::S1).map(|(s, _)| *s).collect();
    let refuge: Vec<(Site, Class)> = co.pad.iter().filter(|(_, c)| matches!(c, Class::Wet | Class::Shaded)).copied().collect();
    let mut track: HashMap<Site, Track> = s1.iter().map(|&s| (s, Track::default())).collect();
    // Refuge velvetpad births and deaths per two-hour window from the load, wet and shaded.
    let mut refuge_last: HashMap<Site, Option<(Species, u64)>> = HashMap::new();
    let mut refuge_bd: HashMap<(u64, Class), (u64, u64)> = HashMap::new();
    {
        let fv = sim.flora().view();
        for &(site, _) in &refuge {
            refuge_last.insert(site, fv.stand_at(site).map(|s| (s.species, s.id)));
        }
    }
    let start = Window {
        ledger: sim.flora().view().ledger.clone(),
        minute: minute0,
    };
    let mut windows: Vec<Window> = vec![start];
    let per_min = 60 * u64::from(TICK_HZ);
    let end = minute0 + (hours * 60.0).round() as u64;
    let t0 = std::time::Instant::now();
    sample(&sim, &arm, minute0, &co, &s1, &refuge, &track, &windows[0]);
    for minute in minute0 + 1..=end {
        for _ in 0..per_min {
            sim.step();
        }
        let fv = sim.flora().view();
        let sc = fv.config.species(TURF);
        for &site in &s1 {
            let t = track.get_mut(&site).unwrap();
            if let Some(s) = fv.stand_at(site) {
                if s.species == TURF {
                    t.turf_seen = true;
                    if s.wood >= sc.donor_min {
                        t.turf_donor_seen = true;
                    }
                    if t.last_id != Some(s.id) {
                        t.turf_ids += 1;
                    }
                } else if is_woody(s.species) {
                    if t.turf_seen {
                        t.woody_after_turf = true;
                    } else {
                        t.woody_direct = true;
                    }
                }
                t.last_id = Some(s.id);
            } else {
                t.last_id = None;
            }
        }
        let win = (minute - minute0 - 1) / 120;
        for &(site, class) in &refuge {
            let now = fv.stand_at(site).map(|s| (s.species, s.id));
            let was = refuge_last.insert(site, now).flatten();
            if was.map(|w| w.1) != now.map(|n| n.1) {
                let e = refuge_bd.entry((win, class)).or_default();
                if was.is_some_and(|w| w.0 == PAD) {
                    e.1 += 1;
                }
                if now.is_some_and(|n| n.0 == PAD) {
                    e.0 += 1;
                }
            }
        }
        if (minute - minute0) % every == 0 || minute == end {
            if (minute - minute0) % 120 == 0 {
                windows.push(Window {
                    ledger: sim.flora().view().ledger.clone(),
                    minute,
                });
            }
            sample(&sim, &arm, minute, &co, &s1, &refuge, &track, &windows[0]);
            eprintln!("minute {minute} ({:.0} s wall)", t0.elapsed().as_secs_f64());
        }
        if let Some(d) = save_dir
            && save_min.contains(&minute)
        {
            save(&Path::new(d).join(format!("m{minute}")), &sim, minute);
        }
    }
    // Flux windows: the ledger's food-side lines over each two hours.
    for pair in windows.windows(2) {
        flux_line(&arm, &pair[0], &pair[1], sim.flora().view().config);
    }
    for ((win, class), (b, d)) in {
        let mut v: Vec<_> = refuge_bd.iter().collect();
        v.sort_by_key(|((w, c), _)| (*w, format!("{c:?}")));
        v
    } {
        println!(
            "refuge_bd,{arm},{}-{},{class:?},births={b},deaths={d}",
            minute0 + win * 120,
            minute0 + (win + 1) * 120
        );
    }
    attempts(&sim, &arm, &co, &s1, &track);
}

fn sample(
    sim: &Sim,
    arm: &str,
    minute: u64,
    co: &Cohorts,
    s1: &[Site],
    refuge: &[(Site, Class)],
    track: &HashMap<Site, Track>,
    start: &Window,
) {
    let fv = sim.flora().view();
    let config = fv.config;
    let area = sim.world().config().cell_area();
    let occ = |sites: &mut dyn Iterator<Item = Site>| {
        let mut v = vec![0u64; Species::COUNT + 1];
        let mut food = 0.0;
        let mut litter = 0.0;
        let mut n = 0usize;
        // Velvetpad and springturf donors on these sites.
        let mut donors = [0u64; 2];
        for site in sites {
            n += 1;
            let st = fv.stand_at(site);
            v[occ_index(st.map(|s| s.species))] += 1;
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
        (v, food, litter, n, donors)
    };
    let line = |name: &str, (v, food, litter, n, donors): (Vec<u64>, f64, f64, usize, [u64; 2])| {
        let photo: u64 = Species::ALL
            .iter()
            .filter(|s| config.species(**s).trophic == Trophic::Photo)
            .map(|s| v[s.index()])
            .sum();
        let parts: Vec<String> = (0..=Species::COUNT).filter(|&i| v[i] > 0).map(|i| format!("{}={}", occ_name(i), v[i])).collect();
        println!(
            "occ,{arm},{minute},{name},n={n},photo={photo},fungal={},pad_donors={},turf_donors={},edible={food:.4},edible_per_m2={:.5},litter={litter:.4},{}",
            v[Species::Glowcap.index()],
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
    // S1 progress: success (turf reached donor size and is turf or gave way to woody).
    let (mut success, mut turf_now, mut direct_woody) = (0usize, 0usize, 0usize);
    for &site in s1 {
        let t = track[&site];
        let now = fv.stand_at(site).map(|s| s.species);
        if t.turf_donor_seen && now.is_some_and(|s| s == TURF || is_woody(s)) {
            success += 1;
        }
        if now == Some(TURF) {
            turf_now += 1;
        }
        if t.woody_direct {
            direct_woody += 1;
        }
    }
    println!(
        "s1,{arm},{minute},n={},success={success},share={:.4},turf_now={turf_now},direct_woody={direct_woody}",
        s1.len(),
        success as f64 / s1.len().max(1) as f64
    );
    // The world: stands, donors and edible foliage per species; litter and dead wood.
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
    let litter: f64 = fv.ground.iter().map(|g| g.litter).sum();
    let wood: f64 = fv.ground.iter().map(|g| g.dead_wood).sum();
    let food: f64 = fv
        .stands
        .iter()
        .filter(|s| config.species(s.species).trophic == Trophic::Photo)
        .map(|s| edible(&fv, s))
        .sum();
    let l = fv.ledger;
    let p = |a: Species, b: Species| l.overtop_pairs[a.index()][b.index()] - start.ledger.overtop_pairs[a.index()][b.index()];
    let woody_from = |a: Species| WOODY.iter().map(|&w| p(a, w)).sum::<u64>();
    println!(
        "world,{arm},{minute},edible={food:.4},litter={litter:.4},dead_wood={wood:.4},pad_to_turf={},pad_to_woody={},turf_to_woody={},stands/donors/edible: {}",
        p(PAD, TURF),
        woody_from(PAD),
        woody_from(TURF),
        parts.join(" ")
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
        "flux,{arm},{}-{},built={:.4},shed={:.4},overtop_pulse={:.4},foliage_taken={:.4},litter_taken={:.4} | built: {} | shed: {} | pulse: {}",
        a.minute,
        b.minute,
        sum(&la.foliage_built, &lb.foliage_built),
        sum(&la.shed_organic, &lb.shed_organic),
        Species::ALL.iter().map(|&s| d(&la.overtopped_organic, &lb.overtopped_organic, s)).sum::<f64>(),
        sum(&la.foliage_taken, &lb.foliage_taken),
        lb.litter_taken - la.litter_taken,
        per(&la.foliage_built, &lb.foliage_built),
        per(&la.shed_organic, &lb.shed_organic),
        per(&la.overtopped_organic, &lb.overtopped_organic),
    );
}

/// The S1 attempt classification at the end, by reach to the nearest turf donor at the
/// load, with the trace's totals behind each class.
fn attempts(sim: &Sim, arm: &str, co: &Cohorts, s1: &[Site], track: &HashMap<Site, Track>) {
    let fv = sim.flora().view();
    let trace = sim.flora().attempts().expect("tracing");
    const BINS: [(f64, &str); 4] = [(6.0, "<=6"), (12.0, "6-12"), (24.0, "12-24"), (f64::INFINITY, ">24")];
    const CLASSES: [&str; 9] = [
        "success",
        "turf_juvenile",
        "post_birth_loss",
        "direct_woody",
        "failed_arm",
        "failed_gate",
        "cover_changed",
        "no_arrival",
        "other",
    ];
    let mut table = vec![[0u64; CLASSES.len()]; BINS.len()];
    let mut totals = cubarium_voxel_flora::Attempts::default();
    for &site in s1 {
        let t = track[&site];
        let a = trace.sites.get(&site).copied().unwrap_or_default();
        totals.bank_checks += a.bank_checks;
        totals.cover_forbidden += a.cover_forbidden;
        totals.gate_fail += a.gate_fail;
        totals.gate_pass += a.gate_pass;
        totals.lottery_lost += a.lottery_lost;
        totals.draw_failed += a.draw_failed;
        totals.arms += a.arms;
        totals.arm_lost += a.arm_lost;
        totals.births_bare += a.births_bare;
        totals.births_through_cover += a.births_through_cover;
        let now = fv.stand_at(site).map(|s| s.species);
        let born = a.births_bare + a.births_through_cover > 0 || t.turf_seen;
        let class = if t.turf_donor_seen && now.is_some_and(|s| s == TURF || is_woody(s)) {
            0
        } else if now == Some(TURF) {
            1
        } else if born {
            2
        } else if t.woody_direct {
            3
        } else if a.gate_pass > 0 {
            4
        } else if a.gate_fail > 0 {
            5
        } else if a.cover_forbidden > 0 {
            6
        } else if a.bank_checks == 0 {
            7
        } else {
            8
        };
        let r = co.reach[&site];
        let bin = BINS.iter().position(|(hi, _)| r <= *hi).unwrap();
        table[bin][class] += 1;
    }
    println!("attempts,{arm},reach,{}", CLASSES.join(","));
    for (b, row) in table.iter().enumerate() {
        println!(
            "attempts,{arm},{},{}",
            BINS[b].1,
            row.iter().map(|v| v.to_string()).collect::<Vec<_>>().join(",")
        );
    }
    println!(
        "trace,{arm},bank_checks={},cover_forbidden={},gate_fail={},gate_pass={},lottery_lost={},draw_failed={},arms={},arm_lost={},births_bare={},births_through_cover={}",
        totals.bank_checks,
        totals.cover_forbidden,
        totals.gate_fail,
        totals.gate_pass,
        totals.lottery_lost,
        totals.draw_failed,
        totals.arms,
        totals.arm_lost,
        totals.births_bare,
        totals.births_through_cover
    );
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
    let (mut world, mut flora, mut fauna, minute0) = load(dir, &want);
    // The introduction: the seeder's own fauna placement on the world as it stands, with
    // no plant founders (every producer, glowcap, vine and film founding zeroed).
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
    let seeded = cubarium_voxel_sim::habitat::seed_pre_rolled(&mut world, &mut flora, &mut fauna, &pre, counts);
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
    let mut sim = Sim::ambient(world, flora, fauna, SimConfig::default());
    sim.set_overlap(false);
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
    for tick in 1..=total {
        sim.step();
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
