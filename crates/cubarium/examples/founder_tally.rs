//! Founder tally: a short heuristic run, per-lineage bites, births and deaths.
//!
//! ```text
//! cargo run --release -p cubarium --example founder_tally -- config/desktop/terrarium.toml 2000 1
//! ```
//!
//! Founds the TOML's world exactly as `voxel_census config=` does (seed `SEED`, the
//! code-default founding table, heuristics, the chained tick on one thread), steps it
//! `TICKS` ticks, and prints one line per lineage: alive at the end, bodies born or
//! hatched during the run, deaths by cause, bites, bites by food, eggs laid and hatched.
//! Every 15 simulated minutes it prints the living count per lineage (`alive@<min>`), and
//! the line per lineage carries its own bites by food (roster R1's measure).
//!
//! An optional fourth argument `snails=N` (roster R2b's measure) sets the ripple-snail's
//! founding count and founds the asked seed itself (no redraw on a refused habitat), so a
//! `snails=0` control runs on the same world as `snails=8`. The film total (glassfilm
//! stock summed over every site) and the film site count are printed at t = 0, every 15
//! minutes and at the end, with the plant layer's film consumed.
//!
//! At t = 0 it reports the roster R1 check: the glowcap caps standing (count and stock)
//! and the foliage standing in a loftstrider's mouth band over each stand's support face
//! (the adult band, `mouth_floor..mouth_ceiling × height`).

use std::collections::HashSet;

use cubarium::voxel::{LayerConfig, Seeds, found, load_config, params_of};
use cubarium_voxel_fauna::{Departure, Food, Founder};
use cubarium_voxel_sim::{Sim, SimConfig};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let path = args
        .get(1)
        .map_or("config/desktop/terrarium.toml", String::as_str);
    let ticks: u64 = args.get(2).map_or(2000, |a| a.parse().expect("TICKS"));
    let seed: u64 = args.get(3).map_or(1, |a| a.parse().expect("SEED"));
    let snails: Option<usize> = args
        .get(4)
        .map(|a| a.strip_prefix("snails=").expect("snails=N").parse().expect("snails=N"));
    cubarium_voxel::set_thread_override(1);
    let cfg = load_config(std::path::Path::new(path)).expect("config");
    let params = params_of(&cfg, &[]).expect("params");
    let world_cfg = params.world.clone();
    let mut counts = founding(&params);
    if let Some(n) = snails {
        counts[Founder::RippleSnail.index()] = n;
    }
    let founded = found(
        &world_cfg,
        &LayerConfig::new(&params, world_cfg.voxel_m),
        counts,
        if snails.is_some() { Seeds::Asked(seed) } else { Seeds::from(seed) },
        false,
    );
    eprintln!(
        "seed {} accepted {} founders {:?}",
        founded.seed, founded.accepted, founded.seeded.founders
    );
    println!(
        "founded: seed {} bodies {:?} shortfall {:?} founding_eggs {} litter_tiles {} film_sites {}",
        founded.seed,
        founded.seeded.founders,
        founded.seeded.shortfall,
        founded.seeded.founding_eggs,
        founded.seeded.litter_tiles,
        founded.seeded.film_sites
    );
    let mut sim = Sim::ambient(
        founded.world,
        founded.flora,
        founded.fauna,
        SimConfig::default(),
    );
    sim.set_overlap(false);
    t0_report(&sim, &params);
    println!("film@0 {}", film_line(&sim));
    let mut seen: HashSet<u64> = sim.fauna().view().animals.iter().map(|a| a.id).collect();
    let mut born = [0u64; Founder::COUNT];
    let quarter = 15 * 60 * u64::from(cubarium_voxel::TICK_HZ);
    for tick in 1..=ticks {
        sim.step();
        if tick % quarter == 0 {
            let v = sim.fauna().view();
            let alive: Vec<String> = Founder::ALL
                .iter()
                .map(|f| {
                    let n = v.animals.iter().filter(|a| a.founder == Some(*f)).count();
                    format!("{}={n}", f.name())
                })
                .collect();
            let minute = tick / (60 * u64::from(cubarium_voxel::TICK_HZ));
            println!("alive@{minute} {}", alive.join(" "));
            println!("film@{minute} {}", film_line(&sim));
        }
        for a in sim.fauna().view().animals {
            if seen.insert(a.id) {
                if let Some(f) = a.founder {
                    born[f.index()] += 1;
                }
            }
        }
    }
    let view = sim.fauna().view();
    let l = view.ledger;
    for f in Founder::ALL {
        let alive = view.animals.iter().filter(|a| a.founder == Some(f)).count();
        let deaths: Vec<String> = Departure::ALL
            .iter()
            .filter_map(|d| {
                let n = l.deaths_by_founder_cause[f.index()][d.index()];
                (n > 0).then(|| format!("{}={n}", d.name()))
            })
            .collect();
        let by_food: Vec<String> = Food::ALL
            .iter()
            .filter_map(|food| {
                let n = l.bites_by_founder_food[f.index()][food.index()];
                (n > 0).then(|| format!("{}={n}", food.name()))
            })
            .collect();
        println!(
            "{:15} alive={:3} born={:3} bites={:6} food=[{}] assim={:.6} eggs_laid={} eggs_hatched={} eggs_now={} deaths=[{}]",
            f.name(),
            alive,
            born[f.index()],
            l.bites_by_founder[f.index()],
            by_food.join(" "),
            l.assimilated_by_founder[f.index()],
            l.eggs_laid_by_founder[f.index()],
            l.eggs_hatched_by_founder[f.index()],
            view.eggs_by_founder(f),
            deaths.join(" "),
        );
    }
    let foods: Vec<String> = Food::ALL
        .iter()
        .map(|f| format!("{}={}", f.name(), l.bites_by_food[f.index()]))
        .collect();
    println!("bites_by_food {}", foods.join(" "));
    println!(
        "births={} deaths={} hatched={}",
        l.births, l.deaths, l.hatched
    );
}

/// The film total, its site count and what the plant layer booked consumed.
fn film_line(sim: &Sim) -> String {
    let fv = sim.flora().view();
    let (mut total, mut sites) = (0.0, 0usize);
    for (_, film) in fv.film_sites() {
        total += film;
        sites += 1;
    }
    format!(
        "film_total={total:.6} film_sites={sites} film_consumed={:.6}",
        fv.ledger.film_consumed
    )
}

fn founding(params: &cubarium_voxel_sim::params::Params) -> [usize; Founder::COUNT] {
    params.fauna.founder_counts()
}

/// Roster R1's t = 0 check: glowcap caps, and foliage in the loftstrider's adult band.
fn t0_report(sim: &Sim, params: &cubarium_voxel_sim::params::Params) {
    use cubarium_voxel_flora::{Species, Trophic};
    let fv = sim.flora().view();
    let v = sim.world().config().voxel_m;
    let caps: Vec<f64> = fv
        .stands
        .iter()
        .filter(|s| s.species == Species::Glowcap && s.foliage > 0.0)
        .map(|s| s.foliage)
        .collect();
    let strider = params.fauna.founder(Founder::Loftstrider).adult_body();
    let (mut in_band, mut stands_in_band, mut all) = (0.0, 0usize, 0.0);
    let mut by_species = std::collections::BTreeMap::<&str, f64>::new();
    for s in fv.stands.iter() {
        if fv.config.species(s.species).trophic != Trophic::Photo {
            continue;
        }
        let band = strider.mouth_layers(s.site.y, v);
        let mut here = 0.0;
        for l in fv.layers(s) {
            all += l.stock;
            if band.contains(&l.cell) {
                here += l.stock;
            }
        }
        if here > 0.0 {
            stands_in_band += 1;
            in_band += here;
            *by_species.entry(s.species.name()).or_default() += here;
        }
    }
    for a in sim.fauna().view().animals {
        if matches!(
            a.founder,
            Some(Founder::Capgnawer | Founder::Loftstrider | Founder::RippleSnail | Founder::Bellwing)
        ) {
            println!(
                "t0: {} id={} site=({}, {}, {}) heading={:.2}",
                a.founder.map_or("", |f| f.name()),
                a.id,
                a.site.x,
                a.site.y,
                a.site.z,
                a.pose.heading_rad
            );
        }
    }
    println!(
        "t0: glowcap_caps={} cap_stock={:.4} | loftstrider band {:.2}..{:.2} m: foliage={:.4} of {:.4} on {} stands {:?}",
        caps.len(),
        caps.iter().sum::<f64>(),
        strider.mouth_floor_m,
        strider.mouth_ceiling_m,
        in_band,
        all,
        stands_in_band,
        by_species
    );
}
