//! Headless study for ROOSTS-RAIN (backlog §12, `design/handoffs/roosts-and-rain-2026-10-08.md`).
//! No renderer or window.
//!
//! Usage: roost_rain_study CONFIG SEED MINUTES OUTPUT_DIR [start_hour=H] [rain_at=M,M,..]
//!        [arm=before|after] [check-only] [threads=N] [water_every=N]
//!
//! The day clock starts at `start_hour` (default 15: dusk about 25 sim-minutes in on the 2 h
//! day), the dry season is off, and a shower is forced at each `rain_at` minute (default 2,12)
//! when nothing is falling. Writes `summary.json`: seedporter night positions (per sample,
//! asleep or not), bellwing airborne fraction by day in rain and dry, bellwing upkeep+motor a
//! second per body in rain and dry (day), and deaths by founder.

use std::{fs, path::Path};

use anyhow::Result;
use cubarium_voxel::weather::RainMode;
use cubarium_voxel_fauna::{Founder, MobilityMode, PerchAnchor, State, TICK_HZ};
use cubarium_voxel_flora::Species;
use cubarium_voxel_sim::{
    Sim, SimConfig,
    found::{LayerConfig, Seeds, found},
};
use serde_json::json;

fn arg<'a>(args: &'a [String], key: &str) -> Option<&'a str> {
    args.iter().find_map(|s| s.strip_prefix(key))
}

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();
    anyhow::ensure!(args.len() >= 5, "usage: roost_rain_study CONFIG SEED MINUTES OUTPUT_DIR [start_hour=H] [rain_at=M,..]");
    cubarium_voxel_sim::experiment_args(&args);
    let start_hour: f64 = arg(&args, "start_hour=").map_or(Ok(15.0), str::parse)?;
    let rain_at: Vec<u64> = arg(&args, "rain_at=")
        .unwrap_or("2,12")
        .split(',')
        .filter(|s| !s.is_empty())
        .map(str::parse)
        .collect::<Result<_, _>>()?;
    // `arm=before` turns the two new behaviours off by their knobs: today's nearest-stand
    // roost (`roost_budget_m = 0`) and downpour-only grounding (a rain threshold of 1 m/s,
    // which no rain reaches).
    let arm = arg(&args, "arm=").unwrap_or("after").to_string();
    let mut cfg = cubarium::voxel::load_config(Path::new(&args[1]))?;
    cfg.sim.world.day.start_hour = start_hour;
    cfg.sim.world.climate.dry_season = false;
    let mut sets: Vec<String> = Vec::new();
    if arm == "before" {
        sets.push("fauna.founders.seedporter.glider.roost_budget_m=0".into());
        sets.push("fauna.founders.bellwing.flight.rain_ground_m_per_s=1".into());
    }
    let p = cubarium::voxel::params_of(&cfg, &sets)?;
    eprintln!(
        "arm {arm}: R0 {} m/s, shower x{:?} drizzle x{:?}; roost_budget_m {:?} rain_ground_m_per_s {:?} start_hour {} dry_season {}",
        p.world.rain_m_per_s,
        p.world.climate.mode(RainMode::Shower).map(|m| m.rate_r0),
        p.world.climate.mode(RainMode::Drizzle).map(|m| m.rate_r0),
        p.fauna.founder(Founder::Seedporter).glider.map(|g| g.roost_budget_m),
        p.fauna.founder(Founder::Bellwing).flight.map(|f| f.rain_ground_m_per_s),
        p.world.day.start_hour,
        p.world.climate.dry_season
    );
    if args.iter().any(|s| s == "check-only") {
        return Ok(());
    }
    let seed: u64 = args[2].parse()?;
    let minutes: u64 = args[3].parse()?;
    let dir = Path::new(&args[4]);
    fs::create_dir_all(dir)?;
    let start = std::time::Instant::now();
    let f = found(&p.world, &LayerConfig::new(&p, p.world.voxel_m), p.fauna.founder_counts(), Seeds::from(seed), false);
    eprintln!("founded terrain seed {} in {:.0} s", f.seed, start.elapsed().as_secs_f64());
    let mut fauna = f.fauna;
    cubarium::voxel::install_default_founders(&mut fauna)?;
    let mut sim = Sim::ambient(f.world, f.flora, fauna, SimConfig::experiment());
    let day_line = sim.fauna().config().life.day_line;
    let count = |sim: &Sim, who: Founder| sim.fauna().view().animals.iter().filter(|a| a.founder == Some(who)).count();
    let founded = json!({"seedporters": count(&sim, Founder::Seedporter), "bellwings": count(&sim, Founder::Bellwing),
        "vaulttrees": sim.flora().view().stands.iter().filter(|s| s.species == Species::Vaulttree).count(),
        "vine_faces": sim.flora().view().cover.face_count()});
    let deaths0 = sim.fauna().view().ledger.deaths_by_founder_cause;
    // Night positions: [vaulttree crown, ledge top (perched on a face), other crown, vine
    // face, ground, other], all samples and asleep samples.
    let mut night = [0u64; 6];
    let mut night_asleep = [0u64; 6];
    // Bellwing day samples: [rain, dry] × (aloft, total).
    let mut bw = [[0u64; 2]; 2];
    // Bellwing day spend (upkeep + motor) and body-seconds, [rain, dry].
    let mut spend = [0.0f64; 2];
    let mut body_s = [0.0f64; 2];
    let mut rain_ticks = 0u64;
    let every = u64::from(TICK_HZ);
    let bi = Founder::Bellwing.index();
    let total = minutes * 60 * u64::from(TICK_HZ);
    for tick in 1..=total {
        if rain_at.iter().any(|&m| m * 60 * u64::from(TICK_HZ) == tick) && sim.world().weather().mode() == RainMode::Clear {
            sim.world_mut().force_rain(RainMode::Shower);
        }
        let before = {
            let l = &sim.fauna().view().ledger;
            l.upkeep_by_founder[bi] + l.motor_by_founder[bi]
        };
        let bellwings = count(&sim, Founder::Bellwing) as f64;
        sim.step();
        let view = sim.world().view();
        let raining = view.weather.rain_m_per_s > 0.0;
        rain_ticks += u64::from(raining);
        let day = sim.world().life_daylight() >= day_line;
        if day {
            let l = &sim.fauna().view().ledger;
            let k = usize::from(!raining);
            spend[k] += l.upkeep_by_founder[bi] + l.motor_by_founder[bi] - before;
            body_s[k] += bellwings / f64::from(TICK_HZ);
        }
        if tick % every != 0 {
            continue;
        }
        let fv = sim.flora().view();
        for a in sim.fauna().view().animals {
            match a.founder {
                Some(Founder::Bellwing) if day => {
                    let k = usize::from(!raining);
                    bw[k][1] += 1;
                    bw[k][0] += u64::from(a.mobility.mode == MobilityMode::Fly);
                }
                Some(Founder::Seedporter) if !day => {
                    let class = match (a.mobility.mode, a.mobility.perch.map(|p| p.anchor)) {
                        (MobilityMode::Perched, Some(PerchAnchor::Stand { site, .. })) => {
                            if fv.stands_on(site).iter().any(|s| s.species == Species::Vaulttree) { 0 } else { 2 }
                        }
                        (MobilityMode::Perched, Some(PerchAnchor::Face(_))) => 1,
                        (MobilityMode::Perched, Some(PerchAnchor::Vine(_))) => 3,
                        (MobilityMode::Ground, _) => 4,
                        _ => 5,
                    };
                    night[class] += 1;
                    if a.state == State::Asleep {
                        night_asleep[class] += 1;
                    }
                }
                _ => {}
            }
        }
    }
    let deaths = sim.fauna().view().ledger.deaths_by_founder_cause;
    let died = |who: Founder| -> u64 {
        deaths[who.index()].iter().sum::<u64>() - deaths0[who.index()].iter().sum::<u64>()
    };
    let names = ["vaulttree", "ledge_top", "other_crown", "vine_face", "ground", "other"];
    let table = |v: &[u64; 6]| -> serde_json::Value {
        names.iter().zip(v.iter()).map(|(n, c)| ((*n).to_string(), json!(c))).collect::<serde_json::Map<_, _>>().into()
    };
    let frac = |k: usize| if bw[k][1] > 0 { bw[k][0] as f64 / bw[k][1] as f64 } else { f64::NAN };
    let per = |k: usize| if body_s[k] > 0.0 { spend[k] / body_s[k] } else { f64::NAN };
    let summary = json!({
        "seed": seed, "arm": arm, "minutes": minutes, "start_hour": start_hour, "rain_at": rain_at,
        "terrain_seed": f.seed, "founded": founded,
        "rain_minutes": rain_ticks as f64 / f64::from(TICK_HZ) / 60.0,
        "seedporter_night_samples": table(&night),
        "seedporter_night_asleep_samples": table(&night_asleep),
        "bellwing_day_samples_rain": bw[0][1], "bellwing_day_samples_dry": bw[1][1],
        "bellwing_airborne_rain": frac(0), "bellwing_airborne_dry": frac(1),
        "bellwing_spend_per_body_s_rain": per(0), "bellwing_spend_per_body_s_dry": per(1),
        "deaths": {"seedporter": died(Founder::Seedporter), "bellwing": died(Founder::Bellwing)},
        "alive_end": {"seedporters": count(&sim, Founder::Seedporter), "bellwings": count(&sim, Founder::Bellwing)},
        "wall_s": start.elapsed().as_secs_f64(),
    });
    fs::write(dir.join("summary.json"), serde_json::to_vec_pretty(&summary)?)?;
    println!("{summary}");
    Ok(())
}
