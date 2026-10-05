//! The velvetpad niche study (`design/handoffs/velvetpad-study-2026-10-03.md`).
//!
//! ```text
//! velvet_niche config  [config=FILE] [set=PATH=V ...] [sets=FILE]
//! velvet_niche grid    [...] [mineral=0.3,1.0] [days=1]          > grid.csv
//! velvet_niche drought [...] [light=0.2] [wet=1.2] [dry=0.8,0.6,0.4] [dry_s=7200]
//! velvet_niche run     [...] seed=N hours=H [every=10] [threads=2] [save=DIR save_min=M,M2]  (DIR/mM)
//! velvet_niche run     load=DIR [...] hours=H                      (config replaced on load)
//! ```
//!
//! Every subcommand resolves the plant layer the way `voxel_census config=` does: the
//! code defaults, the file's `[flora]` table (default `config/desktop/terrarium.toml`),
//! then `set=` lines and the lines of a `sets=` file, rescaled to the world's voxel.
//!
//! - `config`: the resolved knobs of velvetpad and its competitors, and the reference
//!   break-even `L_eff · μ` under its stated assumptions.
//! - `grid`: single-stand probes ([`cubarium_voxel_flora::study::probe`]) on the preset's
//!   own day clock, across held root-box water, raw light (`sky · canopy`) and site
//!   mineral, from an adult and from a newborn. CSV on stdout, thresholds on stderr.
//! - `drought`: an adult and a newborn-grown stand at wet ground, a dry spell of `dry_s`,
//!   then wet again for a day.
//! - `run`: the plants-only terrarium (no fauna), sampled every `every` minutes: occupancy
//!   by water × terrain sky × canopy class, occupation transitions between samples,
//!   births, donors, deaths by cause and litter. `load=` continues a `save=` directory with
//!   the resolved config **replacing** the saved one (printed as a check).

use std::collections::HashMap;
use std::path::Path;

use cubarium_voxel::World;
use cubarium_voxel::Material;
use cubarium_voxel_fauna::{Founder, TICK_HZ};
use cubarium_voxel_flora::study::{ClimateSample, Held, Start, canopy_at, probe, start_stand};
use cubarium_voxel_flora::{Flora, FloraConfig, Site, Species, establishment_gates_with_sky};
use cubarium_voxel_sim::params::{Layers, Params};
use cubarium_voxel_sim::{Sim, SimConfig};

const STUDIED: [Species; 6] = [
    Species::Velvetpad,
    Species::Bloomcrown,
    Species::Springturf,
    Species::Stonecushion,
    Species::Umbrellafrond,
    Species::Lanternberry,
];

fn arg<'a>(args: &'a [String], k: &str) -> Option<&'a str> {
    args.iter().find_map(|a| a.strip_prefix(k))
}

fn list(s: &str) -> Vec<f64> {
    s.split(',').map(|v| v.parse().expect("a number")).collect()
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
        Some("config") => config(&args),
        Some("grid") => grid(&args),
        Some("drought") => drought(&args),
        Some("run") => run(&args),
        _ => panic!("usage: velvet_niche config|grid|drought|run [...]"),
    }
}

// ------------------------------------------------------------------------- config

fn config(args: &[String]) {
    let params = resolve(args);
    let f = params.flora_for_world();
    let day = &params.world.day;
    println!(
        "world: voxel_m {} day {:?} length_min {} daylight {} plant_night_factor {} upkeep_q10 {}",
        params.world.voxel_m, day.mode, day.length_min, day.daylight, day.plant_night_factor, f.upkeep_q10
    );
    type Col = (&'static str, fn(&cubarium_voxel_flora::SpeciesConfig) -> String);
    let cols: Vec<Col> = vec![
        ("alpha", |s| format!("{:.3}", s.alpha)),
        ("assim g", |s| format!("{:.5}", s.assimilation)),
        ("maint m", |s| format!("{:.6}", s.maintenance)),
        ("senesc s", |s| format!("{:.6}", s.senescence)),
        ("build", |s| format!("{}", s.build)),
        ("light_half", |s| format!("{}", s.light_half)),
        ("K_N", |s| format!("{}", s.nutrient_half)),
        ("f_max", |s| format!("{}", s.nutrient_draw_max)),
        ("wilt/full", |s| format!("{}/{}", s.wilt_water, s.full_water)),
        ("est water", |s| format!("{}", s.establish_water_min)),
        ("est light", |s| format!("{}", s.establish_light_min)),
        ("sat_max", |s| format!("{}", s.establish_saturated_max)),
        ("stress/relax", |s| format!("{}/{}", s.stress_rate_per_s, s.relax_rate_per_s)),
        ("drown m", |s| format!("{}", s.drown_depth_m)),
        ("root d/r", |s| format!("{}/{}", s.rooting_depth, s.rooting_radius)),
        ("W max/alive/donor", |s| format!("{}/{}/{}", s.wood_max, s.alive_min, s.donor_min)),
        ("reserve cap/floor", |s| format!("{}/{}", s.reserve_cap, s.donor_reserve_floor)),
        ("foliage/wood rate", |s| format!("{:.5}/{:.5}", s.foliage_rate, s.wood_rate)),
        ("propagule rate", |s| format!("{:.6}", s.propagule_rate)),
        ("dispersal", |s| format!("{:?} hop {} tries {}", s.dispersal, s.hop, s.spore_tries)),
        ("clonal/reach", |s| format!("{}/{}", s.clonal_share, s.runner_reach)),
        ("seed age s", |s| format!("{:.0}", s.seed_max_age_s)),
        ("ground cover", |s| format!("{}", s.ground_cover)),
        ("overtop", |s| format!("{}", s.overtop_per_check)),
        ("graze refuge", |s| format!("{}", s.graze_refuge)),
        ("crown h/r m", |s| format!("{:?}/{:?}", s.crown_height_m, s.crown_radius_m)),
    ];
    print!("{:<18}", "knob");
    for s in STUDIED {
        print!("{:>22}", s.name());
    }
    println!();
    for (name, f_) in &cols {
        print!("{name:<18}");
        for s in STUDIED {
            print!("{:>22}", f_(f.species(s)));
        }
        println!();
    }
    // The reference break-even: full foliage (P = α W), mineral N = 1 (Monod N/(N+K_N)),
    // no aeration stress, daylight 1, upkeep factor 1, no growth and no propagules:
    // income g α W L_eff μ monod = maintenance m W + (1 + c_g) s α W (replacing shed
    // foliage). L_eff·μ = (m + (1 + c_g) s α) / (g α monod).
    print!("{:<18}", "BE L_eff*mu ref");
    for s in STUDIED {
        let c = f.species(s);
        let monod = 1.0 / (1.0 + c.nutrient_half);
        let be = (c.maintenance + (1.0 + c.build) * c.senescence * c.alpha) / (c.assimilation * c.alpha * monod);
        print!("{be:>22.4}");
    }
    println!();
    // The same with the day clock's means: income × mean daylight d̄, upkeep and shedding ×
    // mean night scale n̄ (d + (1 − d) · plant_night_factor).
    let climate = climate_series(&params, &f, 1.0);
    let n = climate.len() as f64;
    let d = climate.iter().map(|c| c.daylight).sum::<f64>() / n;
    let ns = climate.iter().map(|c| c.night).sum::<f64>() / n;
    let up = climate.iter().map(|c| c.upkeep * c.night).sum::<f64>() / n;
    print!("{:<18}", "BE day-mean");
    for s in STUDIED {
        let c = f.species(s);
        let monod = 1.0 / (1.0 + c.nutrient_half);
        let be = (c.maintenance * up + (1.0 + c.build) * c.senescence * ns * c.alpha) / (c.assimilation * c.alpha * monod * d);
        print!("{be:>22.4}");
    }
    println!();
    println!("day means: daylight {d:.4} night-scale {ns:.4} upkeep×night {up:.4}");
}

/// One day (`days` of them) of the preset's clock as a plant reads it, from a small world
/// on the preset's `[world.day]` and climate, stepped from its start.
fn climate_series(params: &Params, flora: &FloraConfig, days: f64) -> Vec<ClimateSample> {
    let mut wc = params.world.clone();
    wc.width = 8;
    wc.depth = 8;
    wc.height = 16;
    let mut w = World::empty(wc);
    let ticks = (days * params.world.day.day_s() * f64::from(TICK_HZ)).round() as usize;
    let mut out = Vec::with_capacity(ticks);
    for _ in 0..ticks {
        w.step_with(1);
        out.push(ClimateSample::of(&w, flora));
    }
    out
}

// --------------------------------------------------------------------------- grid

const WATERS: [(f64, f64, &str); 9] = [
    (0.4, 0.0, "0.4"),
    (0.6, 0.0, "0.6"),
    (0.8, 0.0, "0.8"),
    (0.9, 0.0, "0.9"),
    (1.0, 0.0, "1.0"),
    (1.2, 0.0, "1.2"),
    (1.4, 0.0, "1.4"),
    // A soggy box: the deeper half saturated (pore ≥ 0.95, available ≥ 1.81), the upper
    // half at 1.4; and a wholly saturated one.
    (1.6, 0.5, "1.6s50"),
    (1.9, 1.0, "1.9s100"),
];
const LIGHTS: [f64; 13] = [0.02, 0.05, 0.08, 0.1, 0.15, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8, 1.0];
const SKIES: [f64; 7] = [0.05, 0.1, 0.2, 0.4, 0.6, 0.8, 1.0];
const CANOPIES: [f64; 4] = [1.0, 0.5, 0.25, 0.1];

fn grid(args: &[String]) {
    let params = resolve(args);
    let f = params.flora_for_world();
    let days: f64 = arg(args, "days=").map_or(1.0, |d| d.parse().unwrap());
    let minerals = arg(args, "mineral=").map_or(vec![1.0], list);
    let climate = climate_series(&params, &f, days);
    let species: Vec<Species> = arg(args, "species=").map_or(STUDIED.to_vec(), |s| {
        s.split(',').map(|n| Species::parse(n).expect("species")).collect()
    });
    // Physiology depends on the product L = sky · canopy alone; the establishment gate on
    // sky, water and saturation. Probe each (species, water, L, mineral, start) once.
    let mut memo: HashMap<(usize, usize, u64, u64, bool), cubarium_voxel_flora::study::Outcome> = HashMap::new();
    let mut run_one = |sp: Species, wi: usize, l: f64, n: f64, adult: bool| {
        *memo
            .entry((sp.index(), wi, l.to_bits(), n.to_bits(), adult))
            .or_insert_with(|| {
                let (water, sat, _) = WATERS[wi];
                let stand = start_stand(&f, sp, if adult { Start::Adult } else { Start::Newborn });
                probe(&f, stand, &climate, |_| Held {
                    water,
                    saturated: sat,
                    light: l,
                    mineral: n,
                })
            })
    };
    println!(
        "species,mineral,water,saturated,sky,canopy,light,est_gate,adult_alive,adult_net,adult_foliage_ratio,adult_wood_ratio,adult_packages,new_alive,new_donor_s,new_wood_end,new_packages"
    );
    for &sp in &species {
        let sc = f.species(sp);
        for &n in &minerals {
            for (wi, &(water, sat, wname)) in WATERS.iter().enumerate() {
                for &sky in &SKIES {
                    for &can in &CANOPIES {
                        let l = sky * can;
                        let a = run_one(sp, wi, l, n, true);
                        let b = run_one(sp, wi, l, n, false);
                        let gate = water >= sc.establish_water_min
                            && sat <= sc.establish_saturated_max
                            && sky >= sc.establish_light_min;
                        println!(
                            "{},{n},{wname},{sat},{sky},{can},{l:.4},{},{},{:.5},{:.3},{:.3},{},{},{},{:.4},{}",
                            sp.name(),
                            u8::from(gate),
                            u8::from(a.alive),
                            a.net(),
                            a.foliage[1] / a.foliage[0],
                            a.wood[1] / a.wood[0],
                            a.packages,
                            u8::from(b.alive),
                            b.donor_s.map_or("-".into(), |s| format!("{s:.0}")),
                            b.wood[1],
                            b.packages
                        );
                    }
                }
            }
        }
    }
    // Thresholds: per species, mineral and water, the least L at which an adult's net is
    // ≥ 0, an adult funds ≥ 1 package a day, and a newborn reaches donor size in a day.
    eprintln!("species mineral water | est_gate(water,sat) est_light_min | L*: adult_net>=0 adult_pkg>=1/day newborn_donor<=1day newborn_alive");
    for &sp in &species {
        let sc = f.species(sp);
        for &n in &minerals {
            for (wi, &(water, sat, wname)) in WATERS.iter().enumerate() {
                let first = |pred: &dyn Fn(&cubarium_voxel_flora::study::Outcome, &cubarium_voxel_flora::study::Outcome) -> bool,
                             run_one: &mut dyn FnMut(Species, usize, f64, f64, bool) -> cubarium_voxel_flora::study::Outcome| {
                    LIGHTS
                        .iter()
                        .copied()
                        .find(|&l| {
                            let a = run_one(sp, wi, l, n, true);
                            let b = run_one(sp, wi, l, n, false);
                            pred(&a, &b)
                        })
                        .map_or("none".to_string(), |l| format!("{l}"))
                };
                let net = first(&|a, _| a.alive && a.net() >= 0.0, &mut run_one);
                let pkg = first(&|a, _| a.alive && a.packages as f64 >= days, &mut run_one);
                let donor = first(&|_, b| b.alive && b.donor_s.is_some(), &mut run_one);
                let alive = first(&|_, b| b.alive, &mut run_one);
                eprintln!(
                    "{:<13} {n:<4} {wname:<8}| {} {:<5} | {net:<5} {pkg:<5} {donor:<5} {alive:<5}",
                    sp.name(),
                    if water >= sc.establish_water_min && sat <= sc.establish_saturated_max { "pass" } else { "shut" },
                    sc.establish_light_min,
                );
            }
        }
    }
}

// ------------------------------------------------------------------------ drought

fn drought(args: &[String]) {
    let params = resolve(args);
    let f = params.flora_for_world();
    let light: f64 = arg(args, "light=").map_or(0.2, |v| v.parse().unwrap());
    let wet: f64 = arg(args, "wet=").map_or(1.2, |v| v.parse().unwrap());
    let mineral: f64 = arg(args, "mineral=").map_or(1.0, |v| v.parse().unwrap());
    let dry_s: f64 = arg(args, "dry_s=").map_or(7200.0, |v| v.parse().unwrap());
    let drys = arg(args, "dry=").map_or(vec![0.8, 0.6, 0.4, 0.2], list);
    let species: Vec<Species> = arg(args, "species=").map_or(vec![Species::Velvetpad], |s| {
        s.split(',').map(|n| Species::parse(n).expect("species")).collect()
    });
    // A day wet (the newborn grows up in it), the dry spell, a day wet again.
    let day = params.world.day.day_s();
    let total = 2.0 * day + dry_s;
    let climate = climate_series(&params, &f, total / day);
    let hz = f64::from(TICK_HZ);
    let (d0, d1) = ((day * hz) as usize, ((day + dry_s) * hz) as usize);
    println!("species,start,light,wet,dry,dry_s,alive,died_s,min_wood,wood_end,foliage_end,packages_total,packages_after");
    for &sp in &species {
        for start in [Start::Adult, Start::Newborn] {
            for &dry in &drys {
                let held = |t: usize| Held {
                    water: if (d0..d1).contains(&t) { dry } else { wet },
                    saturated: 0.0,
                    light,
                    mineral,
                };
                let all = probe(&f, start_stand(&f, sp, start), &climate, held);
                let before = probe(&f, start_stand(&f, sp, start), &climate[..d1], held);
                println!(
                    "{},{:?},{light},{wet},{dry},{dry_s},{},{},{:.4},{:.4},{:.4},{},{}",
                    sp.name(),
                    start,
                    u8::from(all.alive),
                    all.died_s.map_or("-".into(), |s| format!("{s:.0}")),
                    all.min_wood,
                    all.wood[1],
                    all.foliage[1],
                    all.packages,
                    all.packages.saturating_sub(before.packages)
                );
            }
        }
    }
}

// ---------------------------------------------------------------------------- run

/// Water classes on velvetpad's root box at today's rooting: bins of mean available
/// water (1 = field capacity), and soggy (saturated fraction ≥ 0.6) ahead of them.
const WATER_CLASSES: [&str; 8] = ["a<0.4", "a0.4-0.8", "a0.8-1.0", "a1.0-1.2", "a1.2-1.4", "a1.4-1.6", "a>=1.6", "soggy"];
const CLASSES: usize = WATER_CLASSES.len() * 4;

fn water_class(mean: f64, saturated: f64) -> usize {
    if saturated >= 0.6 {
        7
    } else {
        [0.4, 0.8, 1.0, 1.2, 1.4, 1.6].iter().filter(|&&b| mean >= b).count()
    }
}

struct Census {
    sky: HashMap<Site, f64>,
    sky_version: u64,
    /// Each soil support face's occupant at the last sample: species index, or COUNT for
    /// vacant.
    last: HashMap<Site, usize>,
    /// Each face's water class and occupant at the first sample: the fate rows' origin.
    first: HashMap<Site, (usize, usize)>,
    /// Live stands at the last minute: id → (species, site).
    alive: HashMap<u64, (Species, Site)>,
    deaths_drown: [u64; Species::COUNT],
    deaths_starve: [u64; Species::COUNT],
    overtopped0: [u64; Species::COUNT],
}

fn classify(world: &World, flora: &Flora, census: &mut Census) -> Vec<(Site, usize, usize)> {
    let view = world.view();
    let c = view.config;
    if census.sky_version != view.terrain_version {
        census.sky.clear();
        census.sky_version = view.terrain_version;
    }
    let fv = flora.view();
    // The box is today's velvetpad box (rooting 2 / 1 at the reference voxel, rescaled):
    // classes do not move with a candidate's water knobs.
    let probe_sc = {
        let mut s = fv.config.species(Species::Velvetpad).clone();
        s.establish_saturated_max = 0.6;
        s
    };
    let pad_top = fv.config.species(Species::Velvetpad).crown_height(0.0, c.voxel_m);
    let mut out = Vec::new();
    for z in 0..c.depth {
        for x in 0..c.width {
            for y in view.supports_in_column(x as i64, z) {
                if view.material_at(x as i64, y, z) != Material::Soil {
                    continue;
                }
                let site = Site { x, y, z };
                let sky = *census
                    .sky
                    .entry(site)
                    .or_insert_with(|| view.sky_visibility(x as i64, y, z));
                let g = establishment_gates_with_sky(&view, site, &probe_sc, sky, 0.0, 0.0);
                let w = g.mean_water.unwrap_or(0.0);
                let wc = water_class(w, g.saturated_fraction);
                let canopy = canopy_at(&fv, c.width, c.depth, site, f64::from(y) + pad_top);
                // class = water * 4 + lit(sky ≥ 0.6) * 2 + open(canopy ≥ 0.5)
                let class = wc * 4 + usize::from(sky >= 0.6) * 2 + usize::from(canopy >= 0.5);
                let occ = fv.stand_at(site).map_or(Species::COUNT, |s| s.species.index());
                out.push((site, class, occ));
            }
        }
    }
    out
}

fn class_name(k: usize) -> String {
    format!(
        "{}/{}/{}",
        WATER_CLASSES[k / 4],
        if k & 2 != 0 { "lit" } else { "shade" },
        if k & 1 != 0 { "open" } else { "canopy" }
    )
}

fn occ_name(i: usize) -> &'static str {
    if i == Species::COUNT { "vacant" } else { Species::ALL[i].name() }
}

fn sample(minute: u64, world: &World, flora: &Flora, census: &mut Census, last: bool) {
    let rows = classify(world, flora, census);
    if census.first.is_empty() {
        census.first = rows.iter().map(|&(site, class, o)| (site, (class / 4, o))).collect();
    }
    if last {
        // Fate since the first sample: the water class then, the occupant then and now.
        let n = Species::COUNT + 1;
        let mut fate = vec![vec![vec![0u64; n]; n]; WATER_CLASSES.len()];
        for &(site, _, o) in &rows {
            if let Some(&(wc, was)) = census.first.get(&site) {
                fate[wc][was][o] += 1;
            }
        }
        for (g, m) in fate.iter().enumerate() {
            for (a, row) in m.iter().enumerate() {
                for (b, &v) in row.iter().enumerate() {
                    if v > 0 {
                        println!("fate,{minute},{},{},{},{v}", WATER_CLASSES[g], occ_name(a), occ_name(b));
                    }
                }
            }
        }
    }
    let n = Species::COUNT + 1;
    let mut occ = vec![vec![0u64; n]; CLASSES];
    // Transitions since the last sample, by the water class of the site now: from × to.
    let mut tr = vec![vec![vec![0u64; n]; n]; WATER_CLASSES.len()];
    for &(site, class, o) in &rows {
        occ[class][o] += 1;
        if let Some(&was) = census.last.get(&site)
            && was != o
        {
            tr[class / 4][was][o] += 1;
        }
        census.last.insert(site, o);
    }
    for (k, row) in occ.iter().enumerate() {
        let total: u64 = row.iter().sum();
        if total == 0 {
            continue;
        }
        print!("occ,{minute},{}", class_name(k));
        for v in row {
            print!(",{v}");
        }
        println!();
    }
    for (g, m) in tr.iter().enumerate() {
        for (a, row) in m.iter().enumerate() {
            for (b, &v) in row.iter().enumerate() {
                if v > 0 {
                    println!("tr,{minute},{},{},{},{v}", WATER_CLASSES[g], occ_name(a), occ_name(b));
                }
            }
        }
    }
    let fv = flora.view();
    let l = fv.ledger;
    let litter: f64 = fv.ground.iter().map(|g| g.litter).sum();
    let mineral_under = |sp: Species| {
        let mut v: Vec<f64> = fv
            .stands
            .iter()
            .filter(|s| s.species == sp)
            .filter_map(|s| fv.ground_at(s.site).map(|g| g.mineral))
            .collect();
        v.sort_by(f64::total_cmp);
        v.get(v.len() / 2).copied().unwrap_or(f64::NAN)
    };
    for sp in STUDIED {
        let i = sp.index();
        let sc = fv.config.species(sp);
        let st: Vec<_> = fv.stands.iter().filter(|s| s.species == sp).collect();
        let donors = st.iter().filter(|s| s.wood >= sc.donor_min).count();
        println!(
            "sp,{minute},{},{},{donors},{},{},{},{},{},{},{:.4},{:.4},{:.3}",
            sp.name(),
            st.len(),
            l.seeds_germinated[i],
            l.clonal_births[i],
            l.overtopped[i] - census.overtopped0[i],
            census.deaths_drown[i],
            census.deaths_starve[i],
            l.seeds_landed[i],
            l.propagule_funded[i],
            mineral_under(sp),
            litter
        );
    }
}

/// Deaths by cause since the last minute: a stand gone from the layer is overtopped (the
/// ledger's per-species count moves), drowned (water over its face deeper than it takes),
/// or else starved (dieback below `alive_min`).
fn deaths(world: &World, flora: &Flora, census: &mut Census, overtopped_before: &[u64; Species::COUNT]) {
    let fv = flora.view();
    let view = world.view();
    let now: HashMap<u64, (Species, Site)> = fv.stands.iter().map(|s| (s.id, (s.species, s.site))).collect();
    let mut over = [0u64; Species::COUNT];
    for i in 0..Species::COUNT {
        over[i] = fv.ledger.overtopped[i] - overtopped_before[i];
    }
    for (id, &(sp, site)) in &census.alive {
        if now.contains_key(id) {
            continue;
        }
        let i = sp.index();
        if over[i] > 0 {
            over[i] -= 1;
        } else if view.water_depth_m(site.x as i64, site.y, site.z) > fv.config.species(sp).drown_depth_m {
            census.deaths_drown[i] += 1;
        } else {
            census.deaths_starve[i] += 1;
        }
    }
    census.alive = now;
}

fn run(args: &[String]) {
    let hours: f64 = arg(args, "hours=").map_or(6.0, |h| h.parse().unwrap());
    let every: u64 = arg(args, "every=").map_or(10, |m| m.parse().unwrap());
    let params = resolve(args);
    let (mut sim, minute0) = if let Some(dir) = arg(args, "load=") {
        let dir = Path::new(dir);
        // The one safe load: any animals in the save get their heuristics back.
        let s = cubarium_voxel_sim::saved::load(dir, &[]).unwrap_or_else(|e| panic!("load: {e}"));
        let (world, mut flora, fauna, minute) = (s.world, s.flora, s.fauna, s.minute);
        let want = params.flora_for_world();
        let before = flora.config().species(Species::Velvetpad).clone();
        flora.replace_config(want.clone()).expect("the resolved config validates");
        assert!(*flora.config() == want, "the configuration replacement took");
        let after = flora.config().species(Species::Velvetpad);
        eprintln!(
            "loaded {} at minute {minute}; velvetpad wilt/full/est {}/{}/{} -> {}/{}/{}, senescence {} -> {}",
            dir.display(),
            before.wilt_water,
            before.full_water,
            before.establish_water_min,
            after.wilt_water,
            after.full_water,
            after.establish_water_min,
            before.senescence,
            after.senescence
        );
        (Sim::ambient(world, flora, fauna, SimConfig::default()), minute)
    } else {
        let seed: u64 = arg(args, "seed=").map_or(1, |s| s.parse().unwrap());
        let world_cfg = params.world.clone();
        let founded = cubarium::voxel::found(
            &world_cfg,
            &cubarium::voxel::LayerConfig::new(&params, world_cfg.voxel_m),
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
        (Sim::ambient(founded.world, founded.flora, founded.fauna, SimConfig::default()), 0)
    };
    let save = arg(args, "save=");
    let save_min: Vec<u64> = arg(args, "save_min=").map_or(Vec::new(), |m| m.split(',').map(|v| v.parse().unwrap()).collect());
    let ticks_per_min = 60 * u64::from(TICK_HZ);
    let mut census = Census {
        sky: HashMap::new(),
        sky_version: u64::MAX,
        last: HashMap::new(),
        first: HashMap::new(),
        alive: sim.flora().view().stands.iter().map(|s| (s.id, (s.species, s.site))).collect(),
        deaths_drown: [0; Species::COUNT],
        deaths_starve: [0; Species::COUNT],
        overtopped0: sim.flora().view().ledger.overtopped,
    };
    println!("# occ,minute,class,{}", (0..=Species::COUNT).map(occ_name).collect::<Vec<_>>().join(","));
    println!("# tr,minute,group,from,to,count");
    println!("# fate,minute,water_class_at_first_sample,occupant_then,occupant_now,count");
    println!(
        "# sp,minute,species,stands,donors,germinated,clonal,overtopped,drowned,starved,seeds_landed,funded,median_site_mineral,litter_total"
    );
    let (world, flora, _) = sim.layers();
    sample(minute0, world, flora, &mut census, false);
    let end = minute0 + (hours * 60.0).round() as u64;
    let t0 = std::time::Instant::now();
    for minute in minute0 + 1..=end {
        let over = sim.flora().view().ledger.overtopped;
        for _ in 0..ticks_per_min {
            sim.step();
        }
        let (world, flora, _) = sim.layers();
        deaths(world, flora, &mut census, &over);
        if minute % every == 0 || minute == end {
            sample(minute, world, flora, &mut census, minute == end);
            eprintln!("minute {minute} ({:.0} s wall)", t0.elapsed().as_secs_f64());
        }
        if let Some(dir) = save
            && save_min.contains(&minute)
        {
            let d = Path::new(dir).join(format!("m{minute}"));
            let d = d.as_path();
            std::fs::create_dir_all(d).unwrap();
            let (world, flora, fauna) = sim.layers();
            std::fs::write(d.join("world.bin"), world.save()).unwrap();
            std::fs::write(d.join("flora.bin"), cubarium_voxel_flora::snapshot::encode(flora)).unwrap();
            std::fs::write(d.join("fauna.bin"), fauna.save()).unwrap();
            std::fs::write(d.join("minute"), minute.to_string()).unwrap();
            eprintln!("saved {} at minute {minute}", d.display());
        }
    }
}
