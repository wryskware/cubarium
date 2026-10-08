//! Headless, uninterrupted founding study for DEN-REST. No renderer or window.
//! Usage: den_rest_study CONFIG SEED MINUTES OUTPUT_DIR [threads=N] [water_every=N]
//! Optional `baseline=CONFIG` records the old clock's analytic birth ceiling at t=0.

use std::{
    collections::BTreeMap,
    fs::{self, File},
    io::{BufWriter, Write},
    path::Path,
};

use anyhow::{Context, Result};
use cubarium_voxel_fauna::{Animal, Founder, State, TICK_HZ, den_quality, pack_centre};
use cubarium_voxel_flora::Species;
use cubarium_voxel_sim::{
    Sim, SimConfig,
    found::{LayerConfig, Seeds, found},
    params::{Layers, Params},
};
use serde_json::json;

fn parameters(path: &Path) -> Result<Params> {
    let file = fs::read_to_string(path)?.parse::<toml::Table>()?;
    Params::resolve(&Layers {
        file: Some(file),
        ..Layers::default()
    })
    .map_err(anyhow::Error::msg)
}

fn birth_clock(p: &Params) -> serde_json::Value {
    let phys = p.fauna.founder(Founder::Loftstrider);
    let r = phys.core.reproduction;
    let day_s = p.world.day.length_min * 60.0;
    let hold_s = r.hold_required() as f64 / f64::from(TICK_HZ);
    let cycle_s = hold_s + r.gestation_s + r.birth_interval_s;
    json!({"world_day_s":day_s, "surplus_hold_s":r.surplus_hold_s,
        "required_surplus_s":hold_s, "gestation_s":r.gestation_s,
        "refractory_s":r.birth_interval_s, "calves_per_gestation":1,
        "earliest_first_birth_min":(hold_s+r.gestation_s)/60.0,
        "fed_pair_calves_per_world_day":2.0*day_s/cycle_s,
        "fastest_newborn_to_breeding_size_s":(phys.core.birth_body-phys.core.body_min)/phys.core.growth_max_per_s,
        "qualification":"analytic ceiling for two continuously eligible surviving founders; descendants excluded"})
}

#[derive(Default)]
struct RestSamples {
    ticks: u64,
    at_den: u64,
    roofed: u64,
    max_distance_m: f64,
}

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();
    anyhow::ensure!(
        args.len() >= 5,
        "usage: den_rest_study CONFIG SEED MINUTES OUTPUT_DIR [threads=N] [water_every=N] [baseline=CONFIG]"
    );
    cubarium_voxel_sim::experiment_args(&args);
    let p = parameters(Path::new(&args[1]))?;
    let seed: u64 = args[2].parse()?;
    let minutes: u64 = args[3].parse()?;
    let dir = Path::new(&args[4]);
    fs::create_dir_all(dir)?;
    fs::write(
        dir.join("params.toml"),
        p.to_toml().map_err(anyhow::Error::msg)?,
    )?;
    let before = args
        .iter()
        .find_map(|s| s.strip_prefix("baseline="))
        .map(|s| parameters(Path::new(s)).map(|p| birth_clock(&p)))
        .transpose()?;
    fs::write(
        dir.join("birth-clocks.json"),
        serde_json::to_vec_pretty(&json!({"before":before,"after":birth_clock(&p)}))?,
    )?;
    if args.iter().any(|s| s == "validate-only") {
        println!("{}", birth_clock(&p));
        return Ok(());
    }
    let start = std::time::Instant::now();
    let f = found(
        &p.world,
        &LayerConfig::new(&p, p.world.voxel_m),
        p.fauna.founder_counts(),
        Seeds::from(seed),
        false,
    );
    let metadata = json!({"requested_seed":seed,"terrain_seed":f.seed,"accepted":f.accepted,
        "habitat_rejected":f.habitat_rejected,"lake_rejected":f.lake_rejected,
        "founders":f.seeded.founders,"shortfall":f.seeded.shortfall,"warnings":f.warnings});
    fs::write(
        dir.join("founding.json"),
        serde_json::to_vec_pretty(&metadata)?,
    )?;
    let mut fauna = f.fauna;
    cubarium::voxel::install_default_founders(&mut fauna)?;
    let initial_lofts: Vec<u64> = fauna
        .view()
        .animals
        .iter()
        .filter(|a| a.founder == Some(Founder::Loftstrider))
        .map(|a| a.id)
        .collect();
    let initial: Vec<_> = fauna.view().animals.iter().filter(|a| a.founder==Some(Founder::Loftstrider)).map(|a|
        json!({"id":a.id,"body":a.body,"age_ticks":a.age_ticks,"gravid":a.reproduction.escrow.is_some(),
            "surplus_ticks":a.reproduction.surplus_ticks,"refractory_ticks":a.reproduction.refractory_ticks})).collect();
    fs::write(
        dir.join("loftstriders-t0.json"),
        serde_json::to_vec_pretty(&initial)?,
    )?;
    let mut sim = Sim::ambient(f.world, f.flora, fauna, SimConfig::experiment());
    let mut rest_log = BufWriter::new(File::create(dir.join("rest-positions.csv"))?);
    let mut births = BufWriter::new(File::create(dir.join("loftstrider-births.jsonl"))?);
    writeln!(
        rest_log,
        "tick,animal,pack,state,x,y,z,den_x,den_y,den_z,distance_m,roofed,den_roofed,at_den,reserve_fraction,homing_ticks"
    )?;
    let mut prior_rest: BTreeMap<u64, (State, cubarium_voxel_flora::Site)> = BTreeMap::new();
    let mut samples: BTreeMap<u64, RestSamples> = BTreeMap::new();
    let mut shelter = BTreeMap::new();
    let mut born = 0u64;
    let mut founder_calves = 0u64;
    let total_ticks = minutes * 60 * u64::from(TICK_HZ);
    for tick in 1..=total_ticks {
        let old_lofts: BTreeMap<u64, Animal> = sim
            .fauna()
            .view()
            .animals
            .iter()
            .filter(|a| a.founder == Some(Founder::Loftstrider))
            .map(|a| (a.id, *a))
            .collect();
        sim.step();
        if tick % (60 * u64::from(TICK_HZ)) == 0 {
            shelter.clear();
        }
        let fauna = sim.fauna();
        let view = sim.world().view();
        let fv = fauna.view();
        let mut calves = fv
            .animals
            .iter()
            .filter(|a| a.founder == Some(Founder::Loftstrider) && !old_lofts.contains_key(&a.id));
        for a in fv
            .animals
            .iter()
            .filter(|a| a.founder == Some(Founder::Loftstrider))
        {
            if old_lofts
                .get(&a.id)
                .is_some_and(|old| a.reproduction.given > old.reproduction.given)
            {
                let calf = calves
                    .next()
                    .context("a paid loftstrider birth must have a newborn")?;
                born += 1;
                founder_calves += u64::from(initial_lofts.contains(&a.id));
                writeln!(
                    births,
                    "{}",
                    json!({"tick":tick,"sim_min":tick as f64/(60.0*f64::from(TICK_HZ)),
                    "parent":a.id,"calf":calf.id,"founding_parent":initial_lofts.contains(&a.id),
                    "refractory_ticks":a.reproduction.refractory_ticks,"site":a.site})
                )?;
            }
        }
        for a in fv
            .animals
            .iter()
            .filter(|a| a.founder == Some(Founder::Chorister))
        {
            if !matches!(a.state, State::Resting | State::Asleep) {
                prior_rest.remove(&a.id);
                continue;
            }
            let Some(m) = a.pack else { continue };
            let Some(rec) = fauna.pack(m.pack) else {
                continue;
            };
            let phys = fauna.config().founder(Founder::Chorister);
            let t = phys.pack.context("a Veilhound pack table")?;
            let (x, z) = pack_centre(rec.den, view.config.voxel_m);
            let ring = f64::from(view.config.width) * view.config.voxel_m;
            let dx = (a.pose.x - x).rem_euclid(ring);
            let dx = dx.min(ring - dx);
            let distance = (dx * dx + (a.pose.z - z).powi(2)).sqrt();
            let glowcap = sim.flora().config().species(Species::Glowcap);
            let roofed = shelter
                .entry(a.site)
                .or_insert_with(|| den_quality(&view, glowcap, a.site, &t))
                .roofed();
            let den_roofed = shelter
                .entry(rec.den)
                .or_insert_with(|| den_quality(&view, glowcap, rec.den, &t))
                .roofed();
            let at_den = a.site.y == rec.den.y && distance <= t.rest_radius_m.min(t.den_radius_m);
            let s = samples.entry(a.id).or_default();
            s.ticks += 1;
            s.at_den += u64::from(at_den);
            s.roofed += u64::from(roofed);
            s.max_distance_m = s.max_distance_m.max(distance);
            // Every rest onset/site change plus a minute sample of ongoing spells.
            if prior_rest.insert(a.id, (a.state, a.site)) != Some((a.state, a.site))
                || tick % (60 * u64::from(TICK_HZ)) == 0
            {
                writeln!(
                    rest_log,
                    "{tick},{},{},{:?},{:.4},{:.4},{:.4},{},{},{},{distance:.4},{roofed},{den_roofed},{at_den},{:.6},{}",
                    a.id,
                    m.pack,
                    a.state,
                    a.pose.x,
                    a.pose.y_m,
                    a.pose.z,
                    rec.den.x,
                    rec.den.y,
                    rec.den.z,
                    a.reserve / phys.core.reserve_of(a.body),
                    a.homing_ticks
                )?;
            }
        }
    }
    rest_log.flush()?;
    births.flush()?;
    let mut rest_table = BufWriter::new(File::create(dir.join("rest-summary.csv"))?);
    writeln!(
        rest_table,
        "animal,rest_ticks,at_den_ticks,roofed_ticks,max_distance_m"
    )?;
    for (id, s) in &samples {
        writeln!(
            rest_table,
            "{id},{},{},{},{:.4}",
            s.ticks, s.at_den, s.roofed, s.max_distance_m
        )?;
    }
    rest_table.flush()?;
    let fauna = sim.fauna().view();
    let summary = json!({"seed":seed,"terrain_seed":metadata["terrain_seed"],"minutes":minutes,
        "completed_ticks":total_ticks,"accepted":metadata["accepted"],"loftstrider_births":born,
        "founder_calves":founder_calves,"founding_loftstriders":initial_lofts.len(),
        "founder_calves_per_pair_per_world_day_observed":if initial_lofts.len()==2 { Some(founder_calves as f64/(minutes as f64/p.world.day.length_min)) } else { None },
        "loftstriders_alive":fauna.animals.iter().filter(|a| a.founder==Some(Founder::Loftstrider)).count(),
        "veilhounds_logged":samples.len(),"rest_ticks":samples.values().map(|s| s.ticks).sum::<u64>(),
        "at_den_ticks":samples.values().map(|s| s.at_den).sum::<u64>(),
        "roofed_ticks":samples.values().map(|s| s.roofed).sum::<u64>(),
        "wall_seconds":start.elapsed().as_secs_f64()});
    fs::write(
        dir.join("summary.json"),
        serde_json::to_vec_pretty(&summary)?,
    )?;
    println!("{summary}");
    Ok(())
}
