//! **Where mature springturf's ground goes** (grass-shrink, 2026-10-05). Loads a save as
//! `succession_study plants` does, applies the arm's config, runs `hours=` staged
//! (`run=DIR segment_min= check_min=`), and prints one block per `window_min=`:
//!
//! - `win`: turf stands, mature (wood ≥ `donor_min`), births, promotions, mature stands
//!   that fell below `donor_min` alive (demoted), and deaths by cause for mature and young
//!   stands, mature ones split edge/interior (interior: ≥ 6 of the 8 neighbour columns
//!   hold turf);
//! - `state`: the mature stands' means (light, μ, aeration stress, the site's mineral and
//!   its Monod, the site's litter, foliage / reflush cap, reserve / reserve cap, wood),
//!   and the share whose wood fell over the window, edge and interior;
//! - `env`: day phase, daylight, animals by founder, the flora ledger's consumed organic
//!   over the window, woody stands, litter and site mineral summed over the turf's sites;
//! - `fate`: every mature-turf death's site, `fate_after_min=` later: who holds it now;
//! - `dead`: each mature death's last sampled state (the first `k=`);
//! - `ystarved` / `yborn` / `yalive`: young (below `donor_min`) stands that starved, were
//!   born, or are alive, with their last sampled light, sky, μ, Monod, age, whether a
//!   mature turf stands within 2 columns or a woody stand within 3, and `t_turf` /
//!   `t_other`: the transmission to the stand's top through every taller turf layer and
//!   through every other species' taller layers (`light_per_stand`'s attenuation, split);
//!   `ex_mature` counts stands that had been donor size;
//! - `yfate`: who holds a dead young stand's site `fate_after_min=` later.
//!
//! The shade split walks 9 × 9 columns per turf stand every sample: about 4× slower
//! than the run without it.
//!
//! `cargo run --release -p cubarium --example turf_shrink -- load=DIR config=... sets=... hours=H`

use std::collections::HashMap;
use std::path::Path;

use cubarium_voxel_fauna::TICK_HZ;
use cubarium_voxel_flora::{DeathCause, Site, Species, Trophic};
use cubarium_voxel_sim::params::{Layers, Params};
use cubarium_voxel_sim::{Sim, SimConfig, staged};

const TURF: Species = Species::Springturf;

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

#[derive(Clone, Copy, Default)]
struct Rec {
    mature: bool,
    edge: bool,
    wood: f64,
    foliage: f64,
    reserve: f64,
    light: f64,
    mu: f64,
    stress: f64,
    mineral: f64,
    litter: f64,
    born_min: f64,
    wood_at_window: f64,
    ever_mature: bool,
    sampled: bool,
    sky: f64,
    nb_mature: u32,
    nb_woody: u32,
    /// Transmission to this stand's top from taller turf and from every other species.
    t_turf: f64,
    t_other: f64,
}

/// Young-stand deaths and births in a window, with their last sampled context.
#[derive(Default)]
struct Young {
    n: u64,
    unsampled: u64,
    ex_mature: u64,
    light: f64,
    sky: f64,
    mu: f64,
    monod: f64,
    age: f64,
    near_mature: u64,
    near_woody: u64,
    shaded: u64,
    t_turf: f64,
    t_other: f64,
}

impl Young {
    fn add(&mut self, r: &Rec, age: f64, nhalf: f64) {
        self.n += 1;
        if !r.sampled {
            self.unsampled += 1;
            return;
        }
        self.ex_mature += u64::from(r.ever_mature);
        self.light += r.light;
        self.sky += r.sky;
        self.mu += r.mu;
        self.monod += r.mineral / (r.mineral + nhalf);
        if age.is_finite() {
            self.age += age;
        }
        self.near_mature += u64::from(r.nb_mature > 0);
        self.near_woody += u64::from(r.nb_woody > 0);
        self.shaded += u64::from(r.light < 0.5 * r.sky);
        self.t_turf += r.t_turf;
        self.t_other += r.t_other;
    }
    fn line(&self, what: &str, minute: f64) -> String {
        let k = (self.n - self.unsampled).max(1) as f64;
        format!(
            "{what},min={minute:.0},n={},unsampled={},ex_mature={},light={:.3},sky={:.3},mu={:.3},monod={:.3},age_min={:.1},near_mature={:.3},near_woody={:.3},light_below_half_sky={:.3},t_turf={:.3},t_other={:.3}",
            self.n,
            self.unsampled,
            self.ex_mature,
            self.light / k,
            self.sky / k,
            self.mu / k,
            self.monod / k,
            self.age / k,
            self.near_mature as f64 / k,
            self.near_woody as f64 / k,
            self.shaded as f64 / k,
            self.t_turf / k,
            self.t_other / k
        )
    }
}

#[derive(Default)]
struct Win {
    births: u64,
    promoted: u64,
    demoted: [u64; 2],
    dead_mature: [[u64; DeathCause::COUNT]; 2],
    dead_young: [u64; DeathCause::COUNT],
    starved: Young,
    born: Young,
    young_alive: Young,
}

struct Lost {
    min: f64,
    site: Site,
    cause: DeathCause,
    edge: bool,
}

fn causes(v: &[u64; DeathCause::COUNT]) -> String {
    DeathCause::ALL
        .iter()
        .filter(|c| v[c.index()] > 0)
        .map(|c| format!("{}={}", c.name(), v[c.index()]))
        .collect::<Vec<_>>()
        .join(" ")
}

fn main() {
    let raw: Vec<String> = std::env::args().collect();
    let args = staged::resume_args(&raw, &[]).unwrap_or_else(|e| panic!("{e}"));
    cubarium_voxel_sim::experiment_args(&args);
    let hours: f64 = arg(&args, "hours=").map_or(4.0, |h| h.parse().unwrap());
    let window_min: u64 = arg(&args, "window_min=").map_or(10, |v| v.parse().unwrap());
    let fate_after: f64 = arg(&args, "fate_after_min=").map_or(10.0, |v| v.parse().unwrap());
    let k: usize = arg(&args, "k=").map_or(40, |v| v.parse().unwrap());
    let params = resolve(&args);
    let want = params.flora_for_world();
    let dir = Path::new(arg(&args, "load=").expect("load=DIR"));
    let s = cubarium_voxel_sim::saved::load(dir, &[]).unwrap_or_else(|e| panic!("load: {e}"));
    let (world, mut flora, fauna, minute0) = (s.world, s.flora, s.fauna, s.minute);
    flora.replace_config(want.clone()).expect("the resolved config validates");
    let mut sim = Sim::ambient(world, flora, fauna, SimConfig::experiment());
    let end = minute0 + (hours * 60.0).round() as u64;
    let mut runner = staged::StageConfig::from_args(&args)
        .unwrap_or_else(|e| panic!("{e}"))
        .map(|cfg| {
            staged::Runner::start(cfg, &args, &mut sim, minute0, end).unwrap_or_else(|e| {
                eprintln!("staged: the load check failed — {e}");
                std::process::exit(2)
            })
        });
    let (donor_min, alpha, reserve_cap, nhalf) = {
        let sc = sim.flora().config().species(TURF);
        println!(
            "cfg,minute0={minute0},alpha={},donor_min={},alive_min={},wood_max={},maintenance={},senescence={},assimilation={},nutrient_half={},n_tissue={},light_half={}",
            sc.alpha, sc.donor_min, sc.alive_min, sc.wood_max, sc.maintenance, sc.senescence, sc.assimilation, sc.nutrient_half, sc.n_tissue, sc.light_half
        );
        (sc.donor_min, sc.alpha, sc.reserve_cap, sc.nutrient_half)
    };
    let _ = alpha;

    let hz = u64::from(TICK_HZ);
    let total = (hours * 3600.0).round() as u64 * hz;
    let sample = 30 * hz;
    let per_min = 60 * hz;
    let mut known: HashMap<u64, Rec> = HashMap::new();
    let mut win = Win::default();
    let mut lost: Vec<Lost> = Vec::new();
    let mut fate_done = 0usize;
    let mut young_lost: Vec<Lost> = Vec::new();
    let mut young_done = 0usize;
    let mut young_fate: HashMap<(&'static str, &'static str), u64> = HashMap::new();
    let mut fate: HashMap<(&'static str, &'static str), u64> = HashMap::new();
    let mut fate_total: HashMap<(&'static str, &'static str), u64> = HashMap::new();
    let mut dead_printed = 0usize;
    let mut consumed0 = sim.flora().view().ledger.consumed_organic_out;

    let sample_all = |sim: &Sim, known: &mut HashMap<u64, Rec>, win: &mut Win, minute: f64, first: bool| {
        let fv = sim.flora().view();
        let cfg = fv.config;
        let cols: std::collections::HashSet<(u32, u32)> =
            fv.stands.iter().filter(|s| s.species == TURF).map(|s| (s.site.x, s.site.z)).collect();
        let sc = cfg.species(TURF);
        let mature_cols: std::collections::HashSet<(i64, i64)> = fv
            .stands
            .iter()
            .filter(|s| s.species == TURF && s.wood >= donor_min)
            .map(|s| (s.site.x as i64, s.site.z as i64))
            .collect();
        let woody_cols: std::collections::HashSet<(i64, i64)> = fv
            .stands
            .iter()
            .filter(|s| {
                let c = cfg.species(s.species);
                !c.ground_cover && c.trophic == Trophic::Photo
            })
            .map(|s| (s.site.x as i64, s.site.z as i64))
            .collect();
        let wv = sim.world().view();
        let voxel_m = wv.config.voxel_m;
        let mut by_col: HashMap<(i64, i64), Vec<usize>> = HashMap::new();
        for (j, s) in fv.stands.iter().enumerate() {
            by_col.entry((s.site.x as i64, s.site.z as i64)).or_default().push(j);
        }
        let width = wv.config.width as i64;
        let shade_of = |i: usize| -> (f64, f64) {
            let me = &fv.stands[i];
            let top = cubarium_voxel_flora::layer_iter(cfg, me, voxel_m)
                .filter(|l| l.kind.bears_foliage())
                .map(|l| l.band_v[1])
                .fold(f64::NEG_INFINITY, f64::max);
            let (mut tt, mut to) = (1.0, 1.0);
            for dx in -4i64..=4 {
                for dz in -4i64..=4 {
                    let key = ((me.site.x as i64 + dx).rem_euclid(width), me.site.z as i64 + dz);
                    let Some(js) = by_col.get(&key) else { continue };
                    for &j in js {
                        if j == i {
                            continue;
                        }
                        let o = &fv.stands[j];
                        for l in cubarium_voxel_flora::layer_iter(cfg, o, voxel_m).filter(|l| l.kind.bears_foliage()) {
                            let f = cubarium_voxel_flora::shade_share(l.band_v, [top, top]);
                            if !(f > 0.0) || (dx * dx + dz * dz) as f64 > l.radius_v * l.radius_v {
                                continue;
                            }
                            let a = (-f * l.optical_depth).exp();
                            if o.species == TURF {
                                tt *= a;
                            } else {
                                to *= a;
                            }
                        }
                    }
                }
            }
            (tt, to)
        };
        for (si, st) in fv.stands.iter().enumerate().filter(|(_, s)| s.species == TURF) {
            let (x, z) = (st.site.x as i64, st.site.z as i64);
            let (t_turf, t_other) = shade_of(si);
            let near = |set: &std::collections::HashSet<(i64, i64)>, r: i64| {
                let mut n = 0u32;
                for dx in -r..=r {
                    for dz in -r..=r {
                        if (dx, dz) != (0, 0) && set.contains(&(x + dx, z + dz)) {
                            n += 1;
                        }
                    }
                }
                n
            };
            let nb_mature = near(&mature_cols, 2);
            let nb_woody = near(&woody_cols, 3);
            let sky = wv.sky_visibility(st.site.x as i64, st.site.y, st.site.z);
            let mut nb = 0;
            for dx in [-1i64, 0, 1] {
                for dz in [-1i64, 0, 1] {
                    if (dx, dz) != (0, 0) && cols.contains(&((st.site.x as i64 + dx) as u32, (st.site.z as i64 + dz) as u32)) {
                        nb += 1;
                    }
                }
            }
            let g = fv.ground_at(st.site);
            let mature = st.wood >= donor_min;
            let edge = nb < 6;
            let e = known.entry(st.id).or_insert_with(|| {
                if !first {
                    win.births += 1;
                }
                Rec {
                    born_min: if first { -1.0 } else { minute },
                    mature,
                    wood_at_window: st.wood,
                    ..Rec::default()
                }
            });
            if mature && !e.mature {
                win.promoted += 1;
            } else if !mature && e.mature {
                win.demoted[usize::from(!e.edge)] += 1;
            }
            *e = Rec {
                mature,
                edge,
                wood: st.wood,
                foliage: st.foliage,
                reserve: st.reserve,
                light: st.light,
                mu: st.moisture,
                stress: st.aeration_stress,
                mineral: g.map_or(0.0, |g| g.mineral),
                litter: g.map_or(0.0, |g| g.litter),
                born_min: e.born_min,
                wood_at_window: e.wood_at_window,
                ever_mature: e.ever_mature || mature,
                sampled: true,
                sky,
                nb_mature,
                nb_woody,
                t_turf,
                t_other,
            };
            if !first && e.born_min == minute {
                let r = *e;
                win.born.add(&r, 0.0, nhalf);
            }
            let _ = sc;
        }
    };
    sample_all(&sim, &mut known, &mut win, minute0 as f64, true);
    win = Win::default();

    for tick in 1..=total {
        sim.step();
        if let Some(r) = runner.as_mut() {
            r.tick(&sim);
        }
        let minute = minute0 as f64 + tick as f64 / per_min as f64;
        for d in sim.flora().death_log().iter().filter(|d| d.species == TURF) {
            let Some(last) = known.remove(&d.id) else {
                win.dead_young[d.cause.index()] += 1;
                if d.cause == DeathCause::Starved {
                    win.starved.add(&Rec::default(), 0.0, nhalf);
                }
                continue;
            };
            if last.mature {
                win.dead_mature[usize::from(!last.edge)][d.cause.index()] += 1;
                lost.push(Lost {
                    min: minute,
                    site: d.site,
                    cause: d.cause,
                    edge: last.edge,
                });
                if dead_printed < k {
                    dead_printed += 1;
                    println!(
                        "dead,min={minute:.1},id={},cause={},edge={},age_min={:.0},wood={:.4},foliage={:.4},reserve={:.4},light={:.3},mu={:.3},stress={:.3},mineral={:.5},monod={:.3},litter={:.4}",
                        d.id,
                        d.cause.name(),
                        last.edge,
                        if last.born_min < 0.0 { -1.0 } else { minute - last.born_min },
                        last.wood,
                        last.foliage,
                        last.reserve,
                        last.light,
                        last.mu,
                        last.stress,
                        last.mineral,
                        last.mineral / (last.mineral + nhalf),
                        last.litter
                    );
                }
            } else {
                win.dead_young[d.cause.index()] += 1;
                if d.cause == DeathCause::Starved {
                    win.starved.add(&last, if last.born_min < 0.0 { f64::NAN } else { minute - last.born_min }, nhalf);
                }
                young_lost.push(Lost {
                    min: minute,
                    site: d.site,
                    cause: d.cause,
                    edge: last.ever_mature,
                });
            }
        }
        if tick % sample == 0 {
            sample_all(&sim, &mut known, &mut win, minute, false);
        }
        if tick % (window_min * per_min) == 0 {
            let fv = sim.flora().view();
            let cfg = fv.config;
            let turf: Vec<_> = fv.stands.iter().filter(|s| s.species == TURF).collect();
            let mature: Vec<&Rec> = known.values().filter(|r| r.mature).collect();
            println!(
                "win,min={minute:.0},turf={},mature={},births={},promoted={},demoted_edge={},demoted_interior={},dead_mature_edge=[{}],dead_mature_interior=[{}],dead_young=[{}]",
                turf.len(),
                mature.len(),
                win.births,
                win.promoted,
                win.demoted[0],
                win.demoted[1],
                causes(&win.dead_mature[0]),
                causes(&win.dead_mature[1]),
                causes(&win.dead_young)
            );
            for (label, edge) in [("edge", true), ("interior", false)] {
                let m: Vec<&&Rec> = mature.iter().filter(|r| r.edge == edge).collect();
                let n = m.len().max(1) as f64;
                let mean = |f: &dyn Fn(&Rec) -> f64| m.iter().map(|r| f(r)).sum::<f64>() / n;
                let fell = m.iter().filter(|r| r.wood < r.wood_at_window).count();
                println!(
                    "state,min={minute:.0},{label},n={},light={:.3},mu={:.3},stress={:.3},mineral={:.5},monod={:.3},litter={:.4},fol_over_cap={:.3},res_over_cap={:.3},wood={:.4},wood_fell={:.3}",
                    m.len(),
                    mean(&|r| r.light),
                    mean(&|r| r.mu),
                    mean(&|r| r.stress),
                    mean(&|r| r.mineral),
                    mean(&|r| r.mineral / (r.mineral + nhalf)),
                    mean(&|r| r.litter),
                    mean(&|r| r.foliage / (alpha * r.wood).max(1e-12)),
                    mean(&|r| r.reserve / (reserve_cap * r.wood).max(1e-12)),
                    mean(&|r| r.wood),
                    fell as f64 / n
                );
            }
            for r in known.values().filter(|r| !r.mature) {
                let age = if r.born_min < 0.0 { f64::NAN } else { minute - r.born_min };
                win.young_alive.add(r, age, nhalf);
            }
            println!("{}", win.starved.line("ystarved", minute));
            println!("{}", win.born.line("yborn", minute));
            println!("{}", win.young_alive.line("yalive", minute));
            for r in known.values_mut() {
                r.wood_at_window = r.wood;
            }
            let mut animals: HashMap<String, u64> = HashMap::new();
            for a in sim.fauna().view().animals {
                if let Some(f) = a.founder {
                    *animals.entry(format!("{f:?}")).or_default() += 1;
                }
            }
            let mut animals: Vec<_> = animals.into_iter().collect();
            animals.sort();
            let woody = fv
                .stands
                .iter()
                .filter(|s| {
                    let sc = cfg.species(s.species);
                    !sc.ground_cover && sc.trophic == Trophic::Photo
                })
                .count();
            let consumed = fv.ledger.consumed_organic_out;
            let w = sim.world();
            println!(
                "env,min={minute:.0},phase={:.3},daylight={:.3},consumed={:.3},woody={woody},glowcap={},velvetpad={},animals={:?}",
                w.day_phase(),
                w.life_daylight(),
                consumed - consumed0,
                fv.stands.iter().filter(|s| s.species == Species::Glowcap).count(),
                fv.stands.iter().filter(|s| s.species == Species::Velvetpad).count(),
                animals
            );
            consumed0 = consumed;
            // Fates of the lost ground `fate_after` minutes on.
            let occ: HashMap<Site, Species> = fv.stands.iter().map(|s| (s.site, s.species)).collect();
            while fate_done < lost.len() && minute - lost[fate_done].min >= fate_after {
                let l = &lost[fate_done];
                let now = occ.get(&l.site).map_or("vacant", |s| s.name());
                *fate.entry((l.cause.name(), now)).or_default() += 1;
                *fate_total.entry((if l.edge { "edge" } else { "interior" }, now)).or_default() += 1;
                fate_done += 1;
            }
            while young_done < young_lost.len() && minute - young_lost[young_done].min >= fate_after {
                let l = &young_lost[young_done];
                let now = occ.get(&l.site).map_or("vacant", |s| s.name());
                *young_fate.entry((if l.edge { "ex_mature" } else { "never_mature" }, now)).or_default() += 1;
                young_done += 1;
            }
            let mut f: Vec<_> = young_fate.drain().collect();
            f.sort();
            println!("yfate,min={minute:.0},{:?}", f);
            let mut f: Vec<_> = fate.drain().collect();
            f.sort();
            println!("fate,min={minute:.0},{:?}", f);
            win = Win::default();
        }
        if tick % per_min == 0 {
            if let Some(r) = runner.as_mut() {
                if let Some(ended) = r.minute(&sim, minute0 + tick / per_min) {
                    eprintln!("staged: {ended:?} at minute {}", minute0 + tick / per_min);
                    if ended != staged::Ended::Done {
                        break;
                    }
                }
            }
        }
    }
    let mut f: Vec<_> = fate_total.into_iter().collect();
    f.sort();
    println!("fate_by_place,{:?}", f);
}
