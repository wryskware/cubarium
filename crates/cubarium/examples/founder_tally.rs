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
//! An optional `bellwings=N` (F1's measure) does the same for the bellwing: its founding
//! count on the asked seed, so `bellwings=0` is the no-bellwing control on the same world.
//! The run then prints a `bellwing:` census line — body-hours, the shares of body-ticks
//! airborne / perched awake / asleep, assimilated intake and flight cost per body-hour,
//! sips per hour by source, landings by perch kind, takeoffs and drops, the sheltered share
//! of perched body-ticks in downpours — and a `bloomcrown:` line (seed output: propagule
//! organic funded and seeds landed, stands, nectar standing) and the wall time per tick.
//!
//! `trace=S` lines cover bellwings, seedporters and loftstriders (with the water at the site).
//!
//! An optional `seedporters=N` (F3's measure) does the same for the seedporter (its count
//! on the asked seed). The run then prints a `seedporter:` census line — body-hours, the
//! shares of body-ticks on the ground / climbing / gliding / perched (and asleep, a share
//! of perched), fruit eaten by source (organic and bites, `[lanternberry stand, latticevine
//! face]`), glides started (and forced), climbs, seeds voided by source — and a
//! `seed_dispersal:` line for the lanternberry seeds voided: how many voids, how many
//! landed more than `GROVE_M` from their source stand (`away`), how many of those within
//! `GROVE_M` of another adult lanternberry (`other_grove`), and the median and max
//! distance of the away ones.
//!
//! An optional `choristers=N` (roster R3's measure) sets the chorister's founding count —
//! **packs** — on the asked seed. The run prints a `packs@0` line (each pack's den choice:
//! covered, most_overhead or founding_point, and its members) and, at the end, a
//! `chorister:` line — pack chases started and those that ended in a kill (the success
//! rate), strikes and hits, kills by prey lineage, litters and pups born, provisioning
//! given and received, and lanternjaws displaced from carcasses (the yield).
//!
//! At t = 0 it reports the roster R1 check: the glowcap caps standing (count and stock)
//! and the foliage standing in a loftstrider's mouth band over each stand's support face
//! (the adult band, `mouth_floor..mouth_ceiling × height`).

use std::collections::HashSet;

use cubarium::voxel::{LayerConfig, Seeds, found, load_config, params_of};
use cubarium_voxel_fauna::{Departure, Food, Founder};
use cubarium_voxel_sim::{Sim, SimConfig};

/// A grove's reach for the seed census, metres: a void this far from its source stand is
/// "away", and within it of another adult lanternberry it is in another grove.
const GROVE_M: f64 = 2.0;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let path = args
        .get(1)
        .map_or("config/desktop/terrarium.toml", String::as_str);
    let ticks: u64 = args.get(2).map_or(2000, |a| a.parse().expect("TICKS"));
    let seed: u64 = args.get(3).map_or(1, |a| a.parse().expect("SEED"));
    let mut snails: Option<usize> = None;
    let mut bellwings: Option<usize> = None;
    let mut seedporters: Option<usize> = None;
    let mut choristers: Option<usize> = None;
    // `trace=S`: every S seconds, one line per bellwing (a diagnostic).
    let mut trace: Option<u64> = None;
    for a in args.iter().skip(4) {
        if let Some(n) = a.strip_prefix("snails=") {
            snails = Some(n.parse().expect("snails=N"));
        } else if let Some(n) = a.strip_prefix("choristers=") {
            choristers = Some(n.parse().expect("choristers=N"));
        } else if let Some(n) = a.strip_prefix("seedporters=") {
            seedporters = Some(n.parse().expect("seedporters=N"));
        } else if let Some(n) = a.strip_prefix("bellwings=") {
            bellwings = Some(n.parse().expect("bellwings=N"));
        } else if let Some(n) = a.strip_prefix("trace=") {
            trace = Some(n.parse().expect("trace=SECONDS"));
        } else {
            panic!("unknown argument {a}");
        }
    }
    cubarium_voxel::set_thread_override(1);
    let cfg = load_config(std::path::Path::new(path)).expect("config");
    let params = params_of(&cfg, &[]).expect("params");
    let world_cfg = params.world.clone();
    let mut counts = founding(&params);
    if let Some(n) = snails {
        counts[Founder::RippleSnail.index()] = n;
    }
    if let Some(n) = bellwings {
        counts[Founder::Bellwing.index()] = n;
    }
    if let Some(n) = seedporters {
        counts[Founder::Seedporter.index()] = n;
    }
    if let Some(n) = choristers {
        counts[Founder::Chorister.index()] = n;
    }
    let asked = snails.is_some() || bellwings.is_some() || seedporters.is_some() || choristers.is_some();
    let founded = found(
        &world_cfg,
        &LayerConfig::new(&params, world_cfg.voxel_m),
        counts,
        if asked { Seeds::Asked(seed) } else { Seeds::from(seed) },
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
    {
        let f = sim.fauna();
        let packs: Vec<String> = f
            .packs()
            .iter()
            .map(|p| {
                let members = f.view().animals.iter().filter(|a| a.pack.is_some_and(|m| m.pack == p.id)).count();
                let how = match p.den_choice {
                    Some(cubarium_voxel_fauna::DenChoice::Covered) => "covered",
                    Some(cubarium_voxel_fauna::DenChoice::MostOverhead) => "most_overhead",
                    Some(cubarium_voxel_fauna::DenChoice::FoundingPoint) => "founding_point",
                    None => "given",
                };
                format!("pack{}:den={},{},{}:{how}:members={members}", p.id, p.den.x, p.den.y, p.den.z)
            })
            .collect();
        println!("packs@0 n={} {}", packs.len(), packs.join(" "));
    }
    println!("film@0 {}", film_line(&sim));
    println!("fruit@0 {}", fruit_line(&sim));
    let mut seen: HashSet<u64> = sim.fauna().view().animals.iter().map(|a| a.id).collect();
    let mut born = [0u64; Founder::COUNT];
    let quarter = 15 * 60 * u64::from(cubarium_voxel::TICK_HZ);
    let bw = Founder::Bellwing;
    let band = params.fauna.founder(bw).flight.map_or(2.5, |f| f.band_max_m);
    // Bellwing body-ticks: aloft, perched awake, asleep; perched in a downpour, sheltered.
    let (mut aloft, mut perched, mut asleep, mut pour_perched, mut pour_sheltered) =
        (0u64, 0u64, 0u64, 0u64, 0u64);
    // Seedporter body-ticks by mode (ground, climb, glide, perched) and asleep; its
    // lanternberry seed voids as (distance from source, in another grove).
    let sp = Founder::Seedporter;
    let mut sp_modes = [0u64; 4];
    let mut sp_asleep = 0u64;
    let mut voids: Vec<(f64, bool)> = Vec::new();
    let started = std::time::Instant::now();
    for tick in 1..=ticks {
        sim.step();
        {
            use cubarium_voxel_fauna::MobilityMode;
            for a in sim.fauna().view().animals.iter().filter(|a| a.founder == Some(sp)) {
                let k = match a.mobility.mode {
                    MobilityMode::Climb => 1,
                    MobilityMode::Glide => 2,
                    MobilityMode::Perched => 3,
                    _ => 0,
                };
                sp_modes[k] += 1;
                if a.state == cubarium_voxel_fauna::State::Asleep {
                    sp_asleep += 1;
                }
            }
            let events = sim.fauna_mut().take_seed_voids();
            if !events.is_empty() {
                let fv = sim.flora().view();
                let v = sim.world().config().voxel_m;
                let ring = f64::from(sim.world().config().width) * v;
                let centre = |s: cubarium_voxel_flora::Site| ((f64::from(s.x) + 0.5) * v, (f64::from(s.z) + 0.5) * v);
                let dist = |a: (f64, f64), b: (f64, f64)| {
                    let dx = (a.0 - b.0).rem_euclid(ring);
                    let dx = dx.min(ring - dx);
                    (dx * dx + (a.1 - b.1).powi(2)).sqrt()
                };
                for e in events {
                    if !matches!(e.plant, cubarium_voxel_fauna::SeedPlant::Stand(_)) {
                        continue;
                    }
                    let (from, to) = (centre(e.source), centre(e.landed));
                    let d = dist(from, to);
                    let other = d > GROVE_M
                        && fv.stands.iter().any(|s| {
                            s.species == cubarium_voxel_flora::Species::Lanternberry
                                && s.profile_stage >= 1
                                && s.site != e.source
                                && dist(centre(s.site), to) <= GROVE_M
                        });
                    voids.push((d, other));
                }
            }
        }
        if trace.is_some() {
            // Every tick a body is under water deeper than it drowns in: the drown count.
            let wv = sim.world().view();
            for a in sim.fauna().view().animals.iter().filter(|a| a.mobility.submerged_ticks > 0) {
                println!(
                    "submerged tick={tick} id={} {} n={} water={:.2} site=({},{},{}) z={:.2} climb={:?} mode={:?}",
                    a.id,
                    a.founder.map_or("-", |f| f.name()),
                    a.mobility.submerged_ticks,
                    wv.water_depth_m(i64::from(a.site.x), a.site.y, a.site.z),
                    a.site.x,
                    a.site.y,
                    a.site.z,
                    a.pose.z,
                    a.mobility.climb,
                    a.mobility.mode,
                );
            }
        }
        if let Some(every) = trace {
            if tick % (every * u64::from(cubarium_voxel::TICK_HZ)) == 0 {
                let fv = sim.flora().view();
                let v = sim.world().config().voxel_m;
                let ring = f64::from(sim.world().config().width) * v;
                for a in sim.fauna().view().animals.iter().filter(|a| a.founder == Some(bw) || a.founder == Some(Founder::Seedporter) || a.founder == Some(Founder::Loftstrider)) {
                    let near = fv
                        .stands
                        .iter()
                        .filter(|s| s.nectar > 0.0)
                        .map(|s| {
                            let dx = ((f64::from(s.site.x) + 0.5) * v - a.pose.x).rem_euclid(ring);
                            let dx = dx.min(ring - dx);
                            let dz = (f64::from(s.site.z) + 0.5) * v - a.pose.z;
                            (dx * dx + dz * dz).sqrt()
                        })
                        .fold(f64::INFINITY, f64::min);
                    let fan = cubarium_voxel_fauna::flyer_fan_census(
                        &sim.world().view(),
                        &fv,
                        &sim.fauna().view(),
                        a,
                        &[],
                    )
                    .map_or(0, |c| {
                        c.rays.iter().filter(|r| r.hit == cubarium_voxel_fauna::ConeHit::Bloom).count()
                    });
                    print!("fan_blooms={fan} ");
                    println!(
                        "trace t={} id={} {} water={:.2} site=({},{},{}) x={:.2} z={:.2} y={:.2} over={:.2} mode={:?} perch={:?} state={:?} held=({:.2},{:.2},{:.2}) lift={:.2} reserve={:.5} body={:.5} bloom_m={:.2}",
                        tick / u64::from(cubarium_voxel::TICK_HZ),
                        a.id,
                        a.founder.map_or("-", |f| f.name()),
                        sim.world().view().water_depth_m(i64::from(a.site.x), a.site.y, a.site.z),
                        a.site.x,
                        a.site.y,
                        a.site.z,
                        a.pose.x,
                        a.pose.z,
                        a.pose.y_m,
                        a.pose.y_m - (f64::from(a.site.y) + 1.0) * v,
                        a.mobility.mode,
                        a.mobility.perch.map(|p| p.kind),
                        a.state,
                        a.founder_state.held.forward,
                        a.founder_state.held.turn,
                        a.founder_state.held.feed,
                        a.founder_state.lift,
                        a.reserve,
                        a.body,
                        near
                    );
                }
            }
        }
        {
            let view = sim.world().view();
            let fv = sim.flora().view();
            let downpour = view.weather.mode == cubarium_voxel::weather::RainMode::Downpour;
            for a in sim.fauna().view().animals.iter().filter(|a| a.founder == Some(bw)) {
                use cubarium_voxel_fauna::MobilityMode;
                if a.state == cubarium_voxel_fauna::State::Asleep {
                    asleep += 1;
                } else if a.mobility.mode == MobilityMode::Perched {
                    perched += 1;
                } else {
                    aloft += 1;
                }
                if downpour && a.mobility.mode == MobilityMode::Perched {
                    pour_perched += 1;
                    if cubarium_voxel_fauna::flyer_sheltered(&view, &fv, a, band) {
                        pour_sheltered += 1;
                    }
                }
            }
        }
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
            println!("fruit@{minute} {}", fruit_line(&sim));
        }
        for a in sim.fauna().view().animals {
            if seen.insert(a.id) {
                if let Some(f) = a.founder {
                    born[f.index()] += 1;
                }
            }
        }
    }
    let wall = started.elapsed().as_secs_f64();
    let view = sim.fauna().view();
    let l = view.ledger;
    {
        let hz = f64::from(cubarium_voxel::TICK_HZ);
        let body_ticks = (aloft + perched + asleep) as f64;
        let body_h = body_ticks / hz / 3600.0;
        let share = |n: u64| if body_ticks > 0.0 { n as f64 / body_ticks } else { 0.0 };
        let per_bh = |x: f64| if body_h > 0.0 { x / body_h } else { 0.0 };
        let landings: Vec<String> = cubarium_voxel_fauna::PerchKind::ALL
            .iter()
            .map(|k| format!("{}={}", k.name(), l.landings_by_perch[k.index()]))
            .collect();
        println!(
            "bellwing: body_h={:.3} aloft={:.3} perched={:.3} asleep={:.3} intake_per_bh={:.6} flight_cost_per_bh={:.6} sips_per_bh=[stand={:.1} vine={:.1}] landings=[{}] takeoffs={} drops={} downpour_perched_ticks={} sheltered={:.3}",
            body_h,
            share(aloft),
            share(perched),
            share(asleep),
            per_bh(l.assimilated_by_founder[bw.index()]),
            per_bh(l.respired_flight_out),
            per_bh(l.sips_by_bloom[0] as f64),
            per_bh(l.sips_by_bloom[1] as f64),
            landings.join(" "),
            l.takeoffs,
            l.drops,
            pour_perched,
            if pour_perched > 0 { pour_sheltered as f64 / pour_perched as f64 } else { f64::NAN },
        );
        let fv = sim.flora().view();
        let b = cubarium_voxel_flora::Species::Bloomcrown;
        let stands: Vec<_> = fv.stands.iter().filter(|s| s.species == b).collect();
        println!(
            "bloomcrown: propagule_funded={:.6} seeds_landed={} seeds_germinated={} stands={} adults={} wood={:.4} nectar_standing={:.6}",
            fv.ledger.propagule_funded[b.index()],
            fv.ledger.seeds_landed[b.index()],
            fv.ledger.seeds_germinated[b.index()],
            stands.len(),
            stands.iter().filter(|s| s.profile_stage >= 1).count(),
            stands.iter().map(|s| s.wood).sum::<f64>(),
            stands.iter().map(|s| s.nectar).sum::<f64>(),
        );
        let c = Founder::Chorister;
        let kills: Vec<String> = Founder::ALL
            .iter()
            .filter(|&&f| params.fauna.founder(c).hunter.is_some_and(|h| h.hunts(f)))
            .map(|&f| format!("{}={}", f.name(), l.departed_founder(f, Departure::Killed { by: c })))
            .collect();
        println!(
            "chorister: pack_chases={} chase_kills={} chase_success={:.3} strikes={} hits={} kills=[{}] litters={} pups_born={} provision_given={:.6} provision_received={:.6} lanternjaws_displaced={} packs_left={}",
            l.pack_chases,
            l.pack_chase_kills,
            if l.pack_chases > 0 { l.pack_chase_kills as f64 / l.pack_chases as f64 } else { f64::NAN },
            l.strikes_by_founder[c.index()],
            l.hits_by_founder[c.index()],
            kills.join(" "),
            l.litters,
            l.pups_born,
            l.provision_given_organic,
            l.provision_received_organic,
            l.carcass_yields,
            sim.fauna().packs().len(),
        );
        let sp_ticks: u64 = sp_modes.iter().sum();
        let sp_share = |n: u64| if sp_ticks > 0 { n as f64 / sp_ticks as f64 } else { 0.0 };
        println!(
            "seedporter: body_h={:.3} ground={:.3} climb={:.3} glide={:.3} perched={:.3} asleep={:.3} fruit_eaten=[stand={:.6} vine={:.6}] fruit_bites=[stand={} vine={}] glides={} glides_forced={} climbs={} seeds_voided=[stand={:.6} vine={:.6}] seed_voids={}",
            sp_ticks as f64 / f64::from(cubarium_voxel::TICK_HZ) / 3600.0,
            sp_share(sp_modes[0]),
            sp_share(sp_modes[1]),
            sp_share(sp_modes[2]),
            sp_share(sp_modes[3]),
            sp_share(sp_asleep),
            l.fruit_eaten_by_source[0],
            l.fruit_eaten_by_source[1],
            l.fruit_bites_by_source[0],
            l.fruit_bites_by_source[1],
            l.glides,
            l.glides_forced,
            l.climbs,
            l.seeds_voided_by_source[0],
            l.seeds_voided_by_source[1],
            l.seed_voids,
        );
        let mut away: Vec<f64> = voids.iter().filter(|(d, _)| *d > GROVE_M).map(|(d, _)| *d).collect();
        away.sort_by(f64::total_cmp);
        println!(
            "seed_dispersal: lanternberry_voids={} away={} other_grove={} away_median_m={:.2} away_max_m={:.2}",
            voids.len(),
            away.len(),
            voids.iter().filter(|(_, o)| *o).count(),
            away.get(away.len() / 2).copied().unwrap_or(f64::NAN),
            away.last().copied().unwrap_or(f64::NAN),
        );
        println!("step_cost: wall_s={wall:.1} ms_per_tick={:.4}", 1000.0 * wall / ticks as f64);
    }
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

/// F3: the lanternberries (adults, fruiting) and the fruit hanging on stands and vines,
/// and the daylight.
fn fruit_line(sim: &Sim) -> String {
    let fv = sim.flora().view();
    let lb = cubarium_voxel_flora::Species::Lanternberry;
    let adults = fv.stands.iter().filter(|s| s.species == lb && s.profile_stage >= 1).count();
    let fruiting = fv.stands.iter().filter(|s| s.fruit > 0.0).count();
    let stand_fruit: f64 = fv.stands.iter().map(|s| s.fruit).sum();
    let vine_faces = fv.cover.faces().iter().filter(|c| c.fruit > 0.0).count();
    let vine_fruit: f64 = fv.cover.faces().iter().map(|c| c.fruit).sum();
    format!(
        "lanternberry_adults={adults} fruiting_stands={fruiting} stand_fruit={stand_fruit:.4} vine_faces_in_fruit={vine_faces} vine_fruit={vine_fruit:.4} daylight={:.2}",
        sim.world().life_daylight()
    )
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
            Some(Founder::Capgnawer | Founder::Loftstrider | Founder::RippleSnail | Founder::Bellwing | Founder::Seedporter)
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
