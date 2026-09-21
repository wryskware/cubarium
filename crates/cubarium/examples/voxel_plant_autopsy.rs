//! Voxel plant autopsy: what kills every seeded stand in the authored habitat.
//!
//! ```text
//! cargo run --release -p cubarium --example voxel_plant_autopsy -- 6 > runs/voxel-plant-autopsy.csv
//! ```
//!
//! Read-only diagnosis for `design/handoffs/voxel-collapse-diagnosis-2026-09-20.md` D1.
//! Nothing here changes a rule, a constant, a seeder line or the terrain: it seeds the
//! authored habitat exactly as `habitat::seed` does and then reports, per stand, the
//! quantities the stand's own rules read.
//!
//! **Plant only.** The world and the plant layer are stepped, and the fauna the seeder
//! introduces is left standing but never stepped, so nothing grazes. That is the control
//! the census (`design/7_Research/voxel-census-2026-09-20.md`) says is worth having: run A
//! (legacy grazers) and run B (live founders) collapse to the same minute, so if this
//! plant-only run reproduces those minutes the cause is not consumption. The coupled form
//! is `Sim::new(world, flora, fauna, SimConfig::default(), None)`; it is not used here
//! because a grazed stand would confound the income arithmetic below.
//!
//! Output:
//!
//! - stdout, one CSV row per living stand per simulated minute: site, the water depth and
//!   root-box wetness its predicate reads, sky visibility, its light response, `μ`,
//!   aeration stress, `W/P/Q`, the site's mineral pool, this tick's income against this
//!   tick's maintenance, and which clause of its own survival rule is failing;
//! - stderr, the seeding report (with each founder's own establishment gates at the site
//!   the seeder chose), one line per death with the readings it died on, and a closing
//!   summary of the reproduction path — requested, funded and landed propagules, the
//!   largest parcel each species ever saved against its package size, the banks, and the
//!   skyline's gate-by-gate eligibility at the start and at the end.
//!
//! `--wet` runs the **counterfactual probe**, and it changes nothing in the repository:
//! the same authored terrain and the same seeder, under the study arena's own water
//! supply — `rain_m_per_s` 2e-4 (`cubarium-voxel-flora/examples/harness`'s
//! `HARNESS_RAIN_M_PER_S`) and an aquifer charged to a metre above the lowest support
//! face. It is the experiment condition that tells a dry world from a dry plant model,
//! and it is here so the proposal at the end of the diagnosis has a measurement under it.

use cubarium::voxel::VoxelConfig;
use cubarium::voxel::habitat;
use cubarium::voxel::scene;
use cubarium_voxel::{VoxelView, World};
use cubarium_voxel_fauna::{Fauna, FaunaConfig, TICK_HZ};
use cubarium_voxel_flora::{
    DT, Flora, FloraConfig, Gates, Site, SkyCache, Species, SpeciesConfig, Stand, Trophic,
    highest_support,
};

/// One simulated minute, in ticks.
const TICKS_PER_MIN: u64 = 60 * TICK_HZ as u64;

/// The study arena's rain tap, for the `--wet` probe: `HARNESS_RAIN_M_PER_S` of
/// `crates/cubarium-voxel-flora/examples/harness`, restated rather than depended on.
const PROBE_RAIN_M_PER_S: f64 = 0.0002;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let wet = args.iter().any(|a| a == "--wet");
    let hours: f64 = args.iter().find(|a| !a.starts_with("--")).map_or(6.0, |a| {
        a.parse()
            .expect("usage: voxel_plant_autopsy [HOURS] [--wet]")
    });

    let cfg = VoxelConfig::default();
    let mut world_cfg = cfg.world.clone();
    if wet {
        // A metre of head above the lowest support face of this very terrain, which is
        // how the arena charges its own aquifer.
        let probe = scene::authored(cfg.world.clone());
        let low = skyline_of(&probe).first().map_or(0, |s| s.y);
        world_cfg.rain_m_per_s = PROBE_RAIN_M_PER_S;
        world_cfg.initial_aquifer_head_m = f64::from(low) * cfg.world.voxel_m + 1.0;
        eprintln!(
            "--wet: the counterfactual probe — rain {} m/s, aquifer head {:.3} m. \
             The terrain, the seeder and every preset are untouched.",
            world_cfg.rain_m_per_s, world_cfg.initial_aquifer_head_m
        );
    }
    let mut world = scene::authored(world_cfg.clone());
    let mut flora = Flora::new(FloraConfig::default());
    let mut fauna = Fauna::new(FaunaConfig::default());
    let seeded = habitat::seed(&mut world, &mut flora, &mut fauna);
    eprintln!(
        "seeded: stands={} logs={} litter_tiles={} founders={:?}",
        seeded.stands, seeded.logs, seeded.litter_tiles, seeded.founders
    );
    eprintln!(
        "world: {}x{}x{} voxel_m {} rain_m_per_s {} evaporation_m_per_s {} \
         initial_aquifer_head_m {} outlet_m3_per_s {}",
        world_cfg.width,
        world_cfg.height,
        world_cfg.depth,
        world_cfg.voxel_m,
        world_cfg.rain_m_per_s,
        world_cfg.evaporation_m_per_s,
        world_cfg.initial_aquifer_head_m,
        world_cfg.outlet_m3_per_s,
    );

    // ---- at seeding: every founder, and its own establishment predicate at the site the
    // seeder's environment proxy gave it. The predicate is germination's test and a
    // founder is not held to it; a founder standing where its own seeds could never start
    // is still a fact about the placement.
    eprintln!("\n--- at seeding: every founder against its own establishment predicate ---");
    {
        let fv = flora.view();
        let view = world.view();
        for stand in fv.stands.iter() {
            let sc = fv.config.species(stand.species);
            let g = fv.establishment_gates(&view, stand.site, stand.species);
            eprintln!(
                "seed id={:<3} {:<14} site=({:>3},{:>2},{:>2}) water={:.4} m soil_vox={} \
                 mean_pore={} sat_frac={:.3} sky={:.3} | mu={:.3} | gates {} [{}]",
                stand.id,
                stand.species.name(),
                stand.site.x,
                stand.site.y,
                stand.site.z,
                g.water_depth_m,
                g.soil_voxels,
                g.mean_pore
                    .map_or("none".to_string(), |p| format!("{p:.4}")),
                g.saturated_fraction,
                g.sky_visibility,
                mu_of(g.mean_pore, sc),
                if g.passes() { "pass" } else { "REFUSE" },
                failed_gates(&g, sc),
            );
        }
    }

    // ---- the skyline's eligibility at the start, per species.
    let skyline = skyline_of(&world);
    let mut sky = SkyCache::default();
    eprintln!(
        "\n--- skyline eligibility at t = 0 ({} columns) ---",
        skyline.len()
    );
    report_eligibility(&flora, &world, &skyline, &mut sky);

    // ---- the run.
    print_header();
    let total_ticks = (hours * 3600.0 * f64::from(TICK_HZ)) as u64;
    let mut prev: Vec<(u64, Stand)> = sorted_by_id(&flora);
    let mut max_parcel = [0.0f64; Species::COUNT];
    let mut max_bank = [0.0f64; Species::COUNT];

    print_minute(0, 0, &flora, &world);
    for tick in 1..=total_ticks {
        world.step();
        flora.step(&mut world);

        {
            let fv = flora.view();
            for s in fv.stands.iter() {
                let i = s.species.index();
                max_parcel[i] = max_parcel[i].max(s.parcel);
            }
            for g in fv.ground.iter() {
                for species in Species::ALL {
                    let o = g.seed_organic(species);
                    let i = species.index();
                    max_bank[i] = max_bank[i].max(o);
                }
            }
        }

        let now = sorted_by_id(&flora);
        if now.len() != prev.len() {
            report_deaths(tick, &prev, &now, &flora, &world);
        }
        prev = now;

        if tick % TICKS_PER_MIN == 0 {
            print_minute(tick / TICKS_PER_MIN, tick, &flora, &world);
        }
    }

    // ---- the reproduction path, which is the germination question.
    eprintln!("\n--- reproduction, whole run ---");
    {
        let fv = flora.view();
        eprintln!(
            "establishments={} deaths={} births={} (births includes the {} seeded founders)",
            fv.ledger.establishments, fv.ledger.deaths, fv.ledger.births, seeded.stands
        );
        for species in Species::ALL {
            let sc = fv.config.species(species);
            let i = species.index();
            let package = sc.alive_min / sc.propagule_split[0];
            // What a founder can ever pay: its reserve above its own donor floor, net of
            // construction. A founder is seeded at `reserve_cap · W` with `W = donor_min`.
            let w = 0.5 * sc.wood_max;
            let spendable = (sc.reserve_cap * w - sc.donor_reserve_floor * sc.reserve_cap * w)
                / (1.0 + sc.build);
            eprintln!(
                "{:<14} package={:.5} | requested={:.5} funded={:.5} landed={:.5} \
                 | max parcel seen={:.5} ({:>5.1}% of a package) | max bank seen={:.5} \
                 | a founder's whole spendable reserve buys {:.1}% of a package",
                species.name(),
                package,
                fv.ledger.propagule_requested[i],
                fv.ledger.propagule_funded[i],
                fv.ledger.propagule_landed[i],
                max_parcel[i],
                100.0 * max_parcel[i] / package,
                max_bank[i],
                100.0 * spendable / package,
            );
        }
    }

    eprintln!("\n--- skyline eligibility at the end ---");
    let mut sky = SkyCache::default();
    report_eligibility(&flora, &world, &skyline, &mut sky);
}

// ------------------------------------------------------------------ the readings

/// `μ`, the species' moisture ramp over its root box's mean pore fraction. An empty box
/// wilts, which is `moisture_of`'s own rule.
fn mu_of(mean_pore: Option<f64>, sc: &SpeciesConfig) -> f64 {
    match mean_pore {
        None => 0.0,
        Some(mean) => {
            if sc.sat_pore <= sc.wilt_pore {
                if mean >= sc.sat_pore { 1.0 } else { 0.0 }
            } else {
                ((mean - sc.wilt_pore) / (sc.sat_pore - sc.wilt_pore)).clamp(0.0, 1.0)
            }
        }
    }
}

/// This tick's income and maintenance for one stand, in organic-matter units: the
/// arithmetic of `step::grow`'s §4.1–4.3, read rather than run.
fn income_and_maintenance(
    fv: &cubarium_voxel_flora::FloraView<'_>,
    view: &VoxelView<'_>,
    stand: &Stand,
) -> (f64, f64, f64) {
    let sc = fv.config.species(stand.species);
    let n0 = fv.ground_at(stand.site).map_or(0.0, |g| g.mineral);
    let maintenance = sc.maintenance * stand.wood * DT;
    let substrate = fv.dead_wood_in_box(view, stand.site, sc);
    let income = match sc.trophic {
        Trophic::Photo => {
            let monod = if n0 + sc.nutrient_half > 0.0 {
                n0 / (n0 + sc.nutrient_half)
            } else {
                0.0
            };
            let mineral_cap = if sc.n_tissue > 0.0 {
                n0 / sc.n_tissue
            } else {
                f64::INFINITY
            };
            (sc.assimilation
                * stand.light
                * stand.moisture
                * (1.0 - stand.aeration_stress)
                * stand.foliage
                * monod
                * DT)
                .min(sc.nutrient_draw_max * n0 * DT)
                .min(mineral_cap)
                .max(0.0)
        }
        Trophic::Saprotroph => {
            let demand = sc.substrate_uptake_per_s * stand.wood * stand.moisture * DT;
            sc.substrate_yield * demand.min(substrate).max(0.0)
        }
    };
    (income, maintenance, substrate)
}

/// Which clause of the stand's own survival rule is failing, named by the quantity that
/// is zero or short. The rule itself: income has to cover `maintenance · W · dt`, or the
/// shortfall comes out of the reserve and then diebacks `κ` of itself out of the wood,
/// and the stand dies when `W < alive_min`.
fn clause(
    sc: &SpeciesConfig,
    stand: &Stand,
    g: &Gates,
    income: f64,
    maintenance: f64,
    substrate: f64,
) -> String {
    if g.water_depth_m > sc.drown_depth_m {
        return format!(
            "drowning: water {:.4} m > drown_depth_m {:.2}",
            g.water_depth_m, sc.drown_depth_m
        );
    }
    if stand.moisture <= 0.0 {
        return match g.mean_pore {
            None => format!(
                "moisture: no soil voxel in the root box ({} of them), mu = 0",
                g.soil_voxels
            ),
            Some(p) => format!(
                "moisture: mean_pore {:.4} <= wilt_pore {:.2}, mu = 0",
                p, sc.wilt_pore
            ),
        };
    }
    if sc.trophic == Trophic::Saprotroph && substrate <= 0.0 {
        return "substrate: no dead wood left in the mycelium box".to_string();
    }
    if stand.light <= 0.0 && sc.trophic == Trophic::Photo {
        return format!("light: sky visibility {:.3}, response 0", g.sky_visibility);
    }
    if stand.foliage <= 0.0 && sc.trophic == Trophic::Photo {
        return "foliage: P = 0, nothing to fix with".to_string();
    }
    if income < maintenance {
        return format!(
            "deficit: income {income:.3e} < maintenance {maintenance:.3e} ({:.1}% covered)",
            100.0 * income / maintenance.max(f64::MIN_POSITIVE)
        );
    }
    "solvent".to_string()
}

/// Which establishment gates are shut, with the numbers they shut on.
fn failed_gates(g: &Gates, sc: &SpeciesConfig) -> String {
    let mut out: Vec<String> = Vec::new();
    if !g.pore_ok {
        out.push(format!(
            "pore {} < {:.2}",
            g.mean_pore
                .map_or("none".to_string(), |p| format!("{p:.4}")),
            sc.establish_pore_min
        ));
    }
    if !g.aeration_ok {
        out.push(format!(
            "saturated {:.3} > {:.2}",
            g.saturated_fraction, sc.establish_saturated_max
        ));
    }
    if !g.depth_ok {
        out.push(format!(
            "water {:.4} m > {:.2}",
            g.water_depth_m, sc.drown_depth_m
        ));
    }
    if !g.light_ok {
        out.push(format!(
            "sky {:.3} < {:.2}",
            g.sky_visibility, sc.establish_light_min
        ));
    }
    if !g.substrate_ok {
        out.push(format!(
            "dead wood {:.4} < {:.3}",
            g.dead_wood, sc.establish_substrate_min
        ));
    }
    if out.is_empty() {
        "all gates open".to_string()
    } else {
        out.join("; ")
    }
}

// ------------------------------------------------------------------ the reports

fn print_header() {
    println!(
        "sim_min,tick,id,species,x,y,z,water_m,soil_voxels,mean_pore,saturated_fraction,\
         sky,light_response,mu,aeration_stress,wood,foliage,reserve,parcel,site_mineral,\
         litter,dead_wood_in_box,income,maintenance,covered,clause"
    );
}

fn print_minute(minute: u64, tick: u64, flora: &Flora, world: &World) {
    let fv = flora.view();
    let view = world.view();
    for stand in fv.stands.iter() {
        let sc = fv.config.species(stand.species);
        let g = fv.establishment_gates(&view, stand.site, stand.species);
        let (income, maintenance, substrate) = income_and_maintenance(&fv, &view, stand);
        let ground = fv.ground_at(stand.site);
        println!(
            "{minute},{tick},{},{},{},{},{},{:.5},{},{},{:.4},{:.4},{:.4},{:.4},{:.4},\
             {:.6},{:.6},{:.6},{:.6},{:.5},{:.5},{:.5},{:.4e},{:.4e},{:.4},\"{}\"",
            stand.id,
            stand.species.name(),
            stand.site.x,
            stand.site.y,
            stand.site.z,
            g.water_depth_m,
            g.soil_voxels,
            g.mean_pore.map_or("".to_string(), |p| format!("{p:.5}")),
            g.saturated_fraction,
            g.sky_visibility,
            stand.light,
            stand.moisture,
            stand.aeration_stress,
            stand.wood,
            stand.foliage,
            stand.reserve,
            stand.parcel,
            ground.map_or(0.0, |x| x.mineral),
            ground.map_or(0.0, |x| x.litter),
            substrate,
            income,
            maintenance,
            income / maintenance.max(f64::MIN_POSITIVE),
            clause(sc, stand, &g, income, maintenance, substrate),
        );
    }
}

/// Every stand that vanished between the two snapshots, with the readings it held on its
/// last tick alive and the reading of the site it died on.
fn report_deaths(
    tick: u64,
    prev: &[(u64, Stand)],
    now: &[(u64, Stand)],
    flora: &Flora,
    world: &World,
) {
    let fv = flora.view();
    let view = world.view();
    for (id, stand) in prev {
        if now.binary_search_by_key(id, |e| e.0).is_ok() {
            continue;
        }
        let sc = fv.config.species(stand.species);
        let g = fv.establishment_gates(&view, stand.site, stand.species);
        let (income, maintenance, substrate) = income_and_maintenance(&fv, &view, stand);
        let cause = if g.water_depth_m > sc.drown_depth_m {
            format!(
                "DROWNED (water {:.4} m > drown_depth_m {:.2})",
                g.water_depth_m, sc.drown_depth_m
            )
        } else {
            format!(
                "wood {:.6} fell below alive_min {:.4} on unpaid maintenance",
                stand.wood, sc.alive_min
            )
        };
        eprintln!(
            "DEATH min={:>4} tick={:<7} id={:<3} {:<14} site=({:>3},{:>2},{:>2}) {} | \
             last W={:.6} P={:.6} Q={:.6} parcel={:.6} | mean_pore={} mu={:.3} sky={:.3} \
             L={:.3} stress={:.3} | income={:.3e} maintenance={:.3e} | {}",
            tick / TICKS_PER_MIN,
            tick,
            id,
            stand.species.name(),
            stand.site.x,
            stand.site.y,
            stand.site.z,
            cause,
            stand.wood,
            stand.foliage,
            stand.reserve,
            stand.parcel,
            g.mean_pore
                .map_or("none".to_string(), |p| format!("{p:.5}")),
            stand.moisture,
            g.sky_visibility,
            stand.light,
            stand.aeration_stress,
            income,
            maintenance,
            clause(sc, stand, &g, income, maintenance, substrate),
        );
    }
}

/// How many skyline columns each species' establishment predicate admits, and which gate
/// refuses the rest — the germination question, read off the one predicate the tick runs.
fn report_eligibility(flora: &Flora, world: &World, skyline: &[Site], sky: &mut SkyCache) {
    let fv = flora.view();
    let view = world.view();
    for species in Species::ALL {
        let sc = fv.config.species(species);
        let gates = fv.establishment_gates_over(&view, skyline, species, sky);
        let mut eligible = 0usize;
        let (mut pore, mut aer, mut depth, mut light, mut sub) = (0usize, 0, 0, 0, 0);
        for g in &gates {
            if g.passes() {
                eligible += 1;
            }
            if !g.pore_ok {
                pore += 1;
            }
            if !g.aeration_ok {
                aer += 1;
            }
            if !g.depth_ok {
                depth += 1;
            }
            if !g.light_ok {
                light += 1;
            }
            if !g.substrate_ok {
                sub += 1;
            }
        }
        eprintln!(
            "{:<14} eligible {:>5} of {:>5} | shut: pore<{:.2} {:>5}, saturated>{:.2} {:>5}, \
             water>{:.2} m {:>5}, sky<{:.2} {:>5}, dead wood<{:.3} {:>5}",
            species.name(),
            eligible,
            skyline.len(),
            sc.establish_pore_min,
            pore,
            sc.establish_saturated_max,
            aer,
            sc.drown_depth_m,
            depth,
            sc.establish_light_min,
            light,
            sc.establish_substrate_min,
            sub,
        );
    }
}

// ------------------------------------------------------------------ small helpers

/// The stands by identity, sorted, so a disappearance is a merge and not a scan.
fn sorted_by_id(flora: &Flora) -> Vec<(u64, Stand)> {
    let mut out: Vec<(u64, Stand)> = flora.view().stands.iter().map(|s| (s.id, *s)).collect();
    out.sort_unstable_by_key(|e| e.0);
    out
}

/// Every column's highest support face, the seeder's own pool.
fn skyline_of(world: &World) -> Vec<Site> {
    let view = world.view();
    let (width, depth) = (world.config().width, world.config().depth);
    let mut out: Vec<Site> = Vec::new();
    for z in 0..depth {
        for x in 0..width as i64 {
            if let Some(site) = highest_support(&view, x, z) {
                out.push(site);
            }
        }
    }
    out.sort_by_key(|s| (s.y, s.x, s.z));
    out
}
