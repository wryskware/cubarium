//! **Springturf seedling trace**: why young turf starves. Loads a save (as
//! `succession_study plants` does), applies the arm's config, runs, and prints
//!
//! - `cfg,...`: springturf's carbon knobs and the day clock;
//! - `trace,...`: the first `k=` springturf stands born after the load (and their
//!   neighbours in time), every `every_s=` seconds: wood, foliage, reserve, light, μ,
//!   daylight, night scale, until death;
//! - `death,...`: every springturf death after the load: minute, day phase, daylight,
//!   age, cause, and its last sampled state;
//! - `phase,...`: springturf starvation deaths and births by day-phase bin.
//!
//! `cargo run --release -p cubarium --example turf_trace -- load=DIR config=... sets=... hours=H k=N`

use std::collections::HashMap;
use std::path::Path;

use cubarium_voxel_fauna::TICK_HZ;
use cubarium_voxel_flora::{DeathCause, Species};
use cubarium_voxel_sim::params::{Layers, Params};
use cubarium_voxel_sim::{Sim, SimConfig};

const TURF: Species = Species::Springturf;
const BINS: usize = 12;

fn arg<'a>(args: &'a [String], k: &str) -> Option<&'a str> {
    args.iter().find_map(|a| a.strip_prefix(k))
}

fn resolve(args: &[String]) -> Params {
    let path = arg(args, "config=").unwrap_or("config/desktop/terrarium.toml");
    let cfg = cubarium::voxel::load_config(Path::new(path)).unwrap_or_else(|e| panic!("config {path}: {e}"));
    let mut file = toml::Table::new();
    if !cfg.sim.flora.is_empty() {
        file.insert("flora".into(), toml::Value::Table(cfg.sim.flora));
    }
    if !cfg.sim.fauna.is_empty() {
        file.insert("fauna".into(), toml::Value::Table(cfg.sim.fauna));
    }
    let mut sets: Vec<String> = args.iter().filter_map(|a| a.strip_prefix("set=").map(str::to_string)).collect();
    if let Some(f) = arg(args, "sets=") {
        let text = std::fs::read_to_string(f).unwrap_or_else(|e| panic!("sets {f}: {e}"));
        sets.extend(text.lines().map(str::trim).filter(|l| !l.is_empty() && !l.starts_with('#')).map(str::to_string));
    }
    Params::resolve_over(
        Params {
            world: cfg.sim.world,
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

/// Where the glowcap stands: on dark terrain (darkness > 0) or lit, and whether the ground
/// under it holds the substrate its establishment gate asks for.
fn glowcap_line(sim: &Sim, when: &str) {
    let fv = sim.flora().view();
    let sc = fv.config.species(Species::Glowcap);
    let w = sim.world();
    let view = w.view();
    let (mut n, mut dark, mut lit_sub, mut lit_nosub) = (0, 0, 0, 0);
    for st in fv.stands.iter().filter(|s| s.species == Species::Glowcap) {
        n += 1;
        let sky = view.sky_visibility(st.site.x as i64, st.site.y, st.site.z);
        let sub = fv.ground_at(st.site).map_or(0.0, |g| g.litter + g.dead_wood);
        if sc.darkness(sky) > 0.0 {
            dark += 1;
        } else if sub >= sc.establish_substrate_min {
            lit_sub += 1;
        } else {
            lit_nosub += 1;
        }
    }
    println!("glowcap,{when},n={n},dark_terrain={dark},lit_on_substrate={lit_sub},lit_below_substrate_min={lit_nosub}");
}

#[derive(Clone, Copy, Default)]
struct Last {
    born_s: f64,
    wood: f64,
    foliage: f64,
    reserve: f64,
    light: f64,
    moisture: f64,
    min_reserve: f64,
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let hours: f64 = arg(&args, "hours=").map_or(2.0, |h| h.parse().unwrap());
    let k: usize = arg(&args, "k=").map_or(8, |v| v.parse().unwrap());
    let every_s: f64 = arg(&args, "every_s=").map_or(60.0, |v| v.parse().unwrap());
    let trace_from_s: f64 = arg(&args, "trace_from_s=").map_or(0.0, |v| v.parse().unwrap());
    let young_s: f64 = arg(&args, "young_s=").map_or(40.0, |v| v.parse().unwrap());
    let params = resolve(&args);
    let want = params.flora_for_world();
    let dir = Path::new(arg(&args, "load=").expect("load=DIR"));
    let s = cubarium_voxel_sim::saved::load(dir, &[]).unwrap_or_else(|e| panic!("load: {e}"));
    let (world, mut flora, fauna, minute0) = (s.world, s.flora, s.fauna, s.minute);
    flora.replace_config(want.clone()).expect("the resolved config validates");
    let mut sim = Sim::ambient(world, flora, fauna, SimConfig::experiment());

    {
        let sc = sim.flora().config().species(TURF);
        let day = &sim.world().config().day;
        println!(
            "cfg,alpha={},assimilation={},light_half={},maintenance={},senescence={},alive_min={},wood_max={},reserve_cap={},reserve_share={},reflush_below={},foliage_rate={},wood_rate={},build={},dieback={},night_floor={},night_min_s={},propagule_split={:?},seed_mass={},donor_min={},clonal_share={},day_s={},plant_night_factor={},step_every={}",
            sc.alpha,
            sc.assimilation,
            sc.light_half,
            sc.maintenance,
            sc.senescence,
            sc.alive_min,
            sc.wood_max,
            sc.reserve_cap,
            sc.reserve_share,
            sc.reflush_below,
            sc.foliage_rate,
            sc.wood_rate,
            sc.build,
            sc.dieback,
            sc.night_floor,
            sc.night_min_s,
            sc.propagule_split,
            sc.seed_mass,
            sc.donor_min,
            sc.clonal_share,
            day.day_s(),
            day.plant_night_factor,
            sim.flora().config().step_every,
        );
    }

    glowcap_line(&sim, "start");
    let hz = f64::from(TICK_HZ);
    let total = (hours * 3600.0 * hz).round() as u64;
    let sample = ((every_s * hz).round() as u64).max(1);
    let mut known: HashMap<u64, Last> = HashMap::new();
    for st in sim.flora().view().stands.iter().filter(|s| s.species == TURF) {
        known.insert(st.id, Last { born_s: -1.0, ..Last::default() });
    }
    let mut traced: Vec<u64> = Vec::new();
    let mut starved_bins = [0u64; BINS];
    let mut born_bins = [0u64; BINS];
    let mut ages: Vec<f64> = Vec::new();
    let mut causes: HashMap<&'static str, u64> = HashMap::new();
    for tick in 1..=total {
        sim.step();
        let t_s = tick as f64 / hz;
        let w = sim.world();
        let phase = w.day_phase();
        let bin = ((phase * BINS as f64) as usize).min(BINS - 1);
        let daylight = w.life_daylight();
        let night = w.plant_night_scale();
        let to_dawn = w.life_to_dawn_s();
        let flora = sim.flora();
        for d in flora.death_log().iter().filter(|d| d.species == TURF) {
            let last = known.get(&d.id).copied().unwrap_or_default();
            *causes.entry(d.cause.name()).or_default() += 1;
            if d.cause == DeathCause::Starved && last.born_s >= 0.0 {
                starved_bins[bin] += 1;
                ages.push(t_s - last.born_s);
            }
            if traced.contains(&d.id) || last.born_s >= 0.0 && ages.len() <= 60 {
                println!(
                    "death,id={},t_s={t_s:.0},min={:.1},phase={phase:.3},daylight={daylight:.3},to_dawn={},cause={},age_s={:.0},last_wood={:.5},last_foliage={:.5},last_reserve={:.5},min_reserve={:.5},last_light={:.3},last_mu={:.3}",
                    d.id,
                    minute0 as f64 + t_s / 60.0,
                    to_dawn.map_or("-".into(), |v| format!("{v:.0}")),
                    d.cause.name(),
                    if last.born_s >= 0.0 { t_s - last.born_s } else { -1.0 },
                    last.wood,
                    last.foliage,
                    last.reserve,
                    last.min_reserve,
                    last.light,
                    last.moisture,
                );
            }
        }
        // Births, every tick (a stand can live less than a sample).
        let view = flora.view();
        for st in view.stands.iter().filter(|s| s.species == TURF) {
            let e = known.entry(st.id).or_insert_with(|| {
                born_bins[bin] += 1;
                if traced.len() < k && t_s >= trace_from_s {
                    traced.push(st.id);
                }
                Last {
                    born_s: t_s,
                    min_reserve: f64::INFINITY,
                    ..Last::default()
                }
            });
            e.wood = st.wood;
            e.foliage = st.foliage;
            e.reserve = st.reserve;
            e.light = st.light;
            e.moisture = st.moisture;
            e.min_reserve = e.min_reserve.min(st.reserve);
        }
        // The first `young_s` seconds of the first `k` seedlings, every tick they change.
        for st in view.stands.iter().filter(|s| traced.contains(&s.id)) {
            let born = known[&st.id].born_s;
            if t_s - born <= young_s && tick % 5 == 0 {
                let n0 = view.ground_at(st.site).map_or(f64::NAN, |g| g.mineral);
                println!(
                    "young,id={},age_s={:.2},daylight={daylight:.3},wood={:.6},foliage={:.6},reserve={:.6},light={:.3},mu={:.3},mineral={n0:.5},monod={:.3}",
                    st.id,
                    t_s - born,
                    st.wood,
                    st.foliage,
                    st.reserve,
                    st.light,
                    st.moisture,
                    n0 / (n0 + view.config.species(TURF).nutrient_half)
                );
            }
        }
        if tick % sample == 0 {
            for st in view.stands.iter().filter(|s| traced.contains(&s.id)) {
                println!(
                    "trace,id={},t_s={t_s:.0},phase={phase:.3},daylight={daylight:.3},night_scale={night:.3},wood={:.5},foliage={:.5},reserve={:.5},light={:.3},mu={:.3},site={},{},{}",
                    st.id, st.wood, st.foliage, st.reserve, st.light, st.moisture, st.site.x, st.site.y, st.site.z
                );
            }
        }
        if tick % (600 * TICK_HZ as u64) == 0 {
            eprintln!("t={:.0} min", t_s / 60.0);
        }
    }
    glowcap_line(&sim, "end");
    ages.sort_by(f64::total_cmp);
    let q = |p: f64| ages.get(((ages.len() as f64 - 1.0) * p).round() as usize).copied().unwrap_or(f64::NAN);
    println!(
        "ages,starved_born_here={},p10={:.0},p50={:.0},p90={:.0}",
        ages.len(),
        q(0.1),
        q(0.5),
        q(0.9)
    );
    println!("causes,{:?}", causes);
    for b in 0..BINS {
        println!(
            "phase,{:.3}-{:.3},starved={},born={}",
            b as f64 / BINS as f64,
            (b + 1) as f64 / BINS as f64,
            starved_bins[b],
            born_bins[b]
        );
    }
}
