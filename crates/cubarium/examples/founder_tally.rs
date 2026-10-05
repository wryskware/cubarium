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
//! `trace=S` lines cover bellwings, seedporters and loftstriders (with the water at the site);
//! a bellwing's line also gives the nearest nectar-holding flower head (horizontal distance,
//! height over the body, height over its own root) and its fan's first hits by class.
//!
//! `reach` (bellwing-finding, 2026-10-03) prints at t = 0 and every 15 minutes which
//! nectar-holding flower heads a bellwing can fly to through air under its band ceiling
//! (and with no ceiling): a point body, so an upper bound. `packtrace=S` prints, every S
//! seconds, each pack's target and feast (carrion left, carcass record) and each member's
//! state, latch and distances. `bwlife` tracks every bellwing from founding or hatching to
//! death and prints one `bwdeath:` line per death (founder or hatchling, cause, age, size,
//! endowment and the cruise seconds it pays, sips, aloft share) and a `bwlife:` summary.
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
//! **Night survival** (brief `design/handoffs/night-survival-2026-10-02.md`): one `sleep:`
//! line per lineage and phase (`day` while the world's life daylight is at least 0.5,
//! `night` below) with the minutes asleep (summed over bodies, the sleep rest only, not a
//! carnivore's satiated torpor), the sleep onsets and the mean reserve fraction at onset
//! (`f0`, and its minimum), the wakes by cause (`alert`: a hit's alert running; `hunger`:
//! at or past the lineage's `wake_hunger`; `other`: the phase turning, a wall, water, a den
//! walk) and the mean reserve fraction at waking, and the deaths in that phase by cause.
//! One `night_spend:` line per plant species: the night budget's reserve spent on the
//! reflush (with its construction respiration), nectar and fruit, organic units.
//!
//! **Balance** (the search rebase's addendum): one `balance:` line per lineage over the
//! run — `B`, days to empty, surplus per adult per world day, `R̂` (optimistic), access,
//! the sated share (awake with the eating latch off),
//! the books behind them, and a hunter's kills per adult-day against its prey's births.
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
    // Training, search and experiment runs take the coarse water (`water_every=`).
    cubarium_voxel_sim::experiment_args(&args);
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
    // `packtrace=S`: every S seconds, one line per pack and per member (a diagnostic).
    let mut packtrace: Option<u64> = None;
    // `bwlife`: one line per bellwing death (age, size, endowment, sips), and a summary.
    let mut bwlife_on = false;
    // `reach`: at t = 0 and every 15 minutes, which bloom heads a bellwing can fly to.
    let mut reach = false;
    for a in args.iter().skip(4) {
        if let Some(n) = a.strip_prefix("snails=") {
            snails = Some(n.parse().expect("snails=N"));
        } else if let Some(n) = a.strip_prefix("choristers=") {
            choristers = Some(n.parse().expect("choristers=N"));
        } else if let Some(n) = a.strip_prefix("seedporters=") {
            seedporters = Some(n.parse().expect("seedporters=N"));
        } else if let Some(n) = a.strip_prefix("bellwings=") {
            bellwings = Some(n.parse().expect("bellwings=N"));
        } else if a == "bwlife" {
            bwlife_on = true;
        } else if a == "reach" {
            reach = true;
        } else if let Some(n) = a.strip_prefix("packtrace=") {
            packtrace = Some(n.parse().expect("packtrace=SECONDS"));
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
        SimConfig::experiment(),
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
    if reach {
        print_reach(&sim, &params, 0);
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
    // Night survival readouts, per lineage and phase (0 day, 1 night).
    let mut night = NightTally::default();
    let mut prev_asleep: std::collections::HashMap<u64, bool> = sim
        .fauna()
        .view()
        .animals
        .iter()
        .map(|a| (a.id, is_sleeping(a)))
        .collect();
    let mut prev_deaths = sim.fauna().view().ledger.deaths_by_founder_cause;
    let started = std::time::Instant::now();
    // The minimal gut's readouts: body-ticks per lineage, and the stomach's fill `S / K`
    // summed per lineage over six bins of the clock day (four clock hours each).
    let mut gut = GutTally::default();
    gut.bellwing_reserve0 = bellwing_reserve(&sim, &params);
    let mut pack_events: Vec<cubarium_voxel_fauna::PackEvent> = Vec::new();
    let mut bwlife = BwLife::new(&sim);
    let start_hour = params.world.day.start_hour;
    let day_s = params.world.day.day_s();
    for tick in 1..=ticks {
        sim.step();
        gut.observe(&sim, &params, start_hour, day_s, tick);
        pack_events.extend(sim.fauna_mut().take_pack_events());
        if bwlife_on {
            bwlife.observe(&sim, &params, tick);
        }
        night.observe(&sim, &params, &mut prev_asleep, &mut prev_deaths);
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
        if let Some(every) = packtrace {
            if tick % (every * u64::from(cubarium_voxel::TICK_HZ)) == 0 {
                print_pack_trace(&sim, tick);
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
                    if a.founder == Some(bw) {
                        // The nearest nectar stand's flower head in 3-D: its layer's band
                        // and radius against the body's height, and the fan's hits by class.
                        let head = fv
                            .stands
                            .iter()
                            .filter(|s| s.nectar > 0.0)
                            .filter_map(|s| {
                                let l = fv.profile_layers(s).into_iter().filter(|l| l.kind.bears_foliage()).max_by_key(|l| l.cell)?;
                                let dx = ((f64::from(s.site.x) + 0.5) * v - a.pose.x).rem_euclid(ring);
                                let dx = dx.min(ring - dx);
                                let dz = (f64::from(s.site.z) + 0.5) * v - a.pose.z;
                                let h = (dx * dx + dz * dz).sqrt();
                                let cy = (l.cell as f64 + 0.5) * v;
                                Some((h, cy - a.pose.y_m, l.radius_m, s.nectar, s.species, (l.cell - i64::from(s.site.y)) as f64 * v, s.profile_stage))
                            })
                            .min_by(|x, y| x.0.total_cmp(&y.0));
                        let census = cubarium_voxel_fauna::flyer_fan_census(&sim.world().view(), &fv, &sim.fauna().view(), a, &[]);
                        let mut classes = std::collections::BTreeMap::<String, u32>::new();
                        if let Some(c) = &census {
                            for r in &c.rays {
                                *classes.entry(format!("{:?}", r.hit)).or_default() += 1;
                            }
                        }
                        if let Some((h, dy, r, n, sp, above, stage)) = head {
                            print!("head=[h={h:.2} dy={dy:.2} r={r:.2} nectar={n:.4} {sp:?} above_root={above:.2} stage={stage}] ");
                        }
                        print!("fan={classes:?} ");
                    }
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
            if reach {
                print_reach(&sim, &params, minute);
            }
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
    night.print(&sim);
    gut.print(&sim, &params, &born);
    print_packs(&sim, &pack_events);
    if bwlife_on {
        bwlife.summary(&sim, &params);
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

/// Whether a body is in the sleep rest (not a carnivore's satiated torpor).
/// **The minimal gut's readouts** (`design/handoffs/gut-meals-2026-10-03.md`), one `gut:`
/// line per lineage: meals per world day (completed bouts over body-days of the
/// 7,200-s ecological day), the bouts by kind and their mean length, the stomach's fill
/// over the clock day (mean `S / K` in six four-hour bins from midnight), the share of
/// body-ticks with the eating latch on, emergency wakes, sleep flips per body-hour, the
/// measured maturations (count and mean, world days), births, kills by this lineage, and
/// deaths by cause. A `riders:` line: attached, dropped off, and the organic riders took.
#[derive(Default)]
struct GutTally {
    /// Mean bellwing reserve fraction at t = 0 (repair 3's measure).
    bellwing_reserve0: f64,
    body_ticks: [u64; Founder::COUNT],
    eating_ticks: [u64; Founder::COUNT],
    fill: [[f64; 6]; Founder::COUNT],
    fill_n: [[u64; 6]; Founder::COUNT],
    /// Adult body-ticks (free, at `birth_body` or more): the balance line's adult-days.
    adult_ticks: [u64; Founder::COUNT],
}

impl GutTally {
    fn observe(&mut self, sim: &Sim, params: &cubarium_voxel_sim::params::Params, start_hour: f64, day_s: f64, tick: u64) {
        let t = tick as f64 / f64::from(cubarium_voxel::TICK_HZ);
        let hour = (start_hour + 24.0 * t / day_s).rem_euclid(24.0);
        let bin = ((hour / 4.0) as usize).min(5);
        for a in sim.fauna().view().animals {
            let Some(f) = a.founder else { continue };
            let i = f.index();
            self.body_ticks[i] += 1;
            if cubarium_search::es::voxel::live::balance::is_adult(a, &params.fauna) {
                self.adult_ticks[i] += 1;
            }
            if a.eating {
                self.eating_ticks[i] += 1;
            }
            let k = params.fauna.founder(f).gut.capacity(a.body);
            if k > 0.0 {
                self.fill[i][bin] += a.stomach.organic() / k;
                self.fill_n[i][bin] += 1;
            }
        }
    }

    fn print(&self, sim: &Sim, params: &cubarium_voxel_sim::params::Params, born: &[u64; Founder::COUNT]) {
        let l = sim.fauna().view().ledger;
        let hz = f64::from(cubarium_voxel::TICK_HZ);
        let eco_day = params.fauna.life.ecological_day_s;
        for f in Founder::ALL {
            let i = f.index();
            if self.body_ticks[i] == 0 && born[i] == 0 {
                continue;
            }
            let body_s = self.body_ticks[i] as f64 / hz;
            let meals: u64 = l.meals_by_founder_kind[i].iter().sum();
            let kinds: Vec<String> = cubarium_voxel_fauna::BoutKind::ALL
                .iter()
                .map(|k| {
                    let n = l.meals_by_founder_kind[i][k.index()];
                    let s = l.meal_ticks_by_founder_kind[i][k.index()] as f64 / hz;
                    format!("{}={n}/{:.0}s", k.name(), if n > 0 { s / n as f64 } else { 0.0 })
                })
                .collect();
            let fill: Vec<String> = (0..6)
                .map(|b| {
                    let n = self.fill_n[i][b];
                    if n > 0 { format!("{:.2}", self.fill[i][b] / n as f64) } else { "-".into() }
                })
                .collect();
            let kills: u64 = Founder::ALL
                .iter()
                .map(|&p| l.departed_founder(p, Departure::Killed { by: f }))
                .sum();
            let matured = l.matured_by_founder[i];
            let mat_days = if matured > 0 {
                l.maturation_ticks_by_founder[i] as f64 / hz / eco_day / matured as f64
            } else {
                f64::NAN
            };
            println!(
                "gut: {:15} body_days={:.3} meals={} meals_per_day={:.3} bouts=[{}] fill_by_4h=[{}] eating_share={:.3} emergency_wakes={} sleep_flips_per_h={:.2} matured={} maturation_days={:.3} born={} kills={}",
                f.name(),
                body_s / eco_day,
                meals,
                if body_s > 0.0 { meals as f64 / (body_s / eco_day) } else { f64::NAN },
                kinds.join(" "),
                fill.join(" "),
                if self.body_ticks[i] > 0 { self.eating_ticks[i] as f64 / self.body_ticks[i] as f64 } else { 0.0 },
                l.emergency_wakes_by_founder[i],
                if body_s > 0.0 { l.sleep_flips_by_founder[i] as f64 / (body_s / 3600.0) } else { f64::NAN },
                matured,
                mat_days,
                born[i],
                kills,
            );
        }
        // The continuous balance measures over the run (the search rebase's addendum):
        // B, days to empty (at B < 1), surplus per adult per world day, R-hat (optimistic:
        // every juvenile survives), access; a hunter's kills per adult-day against its
        // prey's births per world day.
        {
            use cubarium_search::es::voxel::live::balance;
            let v = sim.fauna().view();
            let run_s = sim.fauna().view().tick as f64 / hz;
            let days = balance::days(run_s, v.config);
            for f in Founder::ALL {
                let i = f.index();
                if self.body_ticks[i] == 0 && born[i] == 0 {
                    continue;
                }
                let books = balance::Books::of(l, f);
                let adult_days = balance::days(self.adult_ticks[i] as f64 / hz, v.config);
                let ctx = balance::context(v.animals, v.config, f, adult_days, days);
                let m = balance::measures(&books, &ctx);
                let predator = v.config.founder(f).hunter.map(|h| {
                    let kills: u64 = Founder::ALL
                        .iter()
                        .map(|&p| l.departed_founder(p, Departure::Killed { by: f }))
                        .sum();
                    let prey: u64 = Founder::ALL.iter().filter(|q| h.prey[q.index()]).map(|q| born[q.index()]).sum();
                    format!(
                        " kills_per_adult_day={:.3} prey_births_per_day={:.3}",
                        if adult_days > 0.0 { kills as f64 / adult_days } else { f64::NAN },
                        if days > 0.0 { prey as f64 / days } else { f64::NAN },
                    )
                });
                println!(
                    "balance: {:15} B={:.3} days_to_empty={} surplus_per_adult_day={:.5} r_hat={:.3} access={:.3} sated_share={:.3} assimilated={:.6} upkeep={:.6} motor={:.6} rider_take_lost={:.6} growth={:.6} reproduction={:.6} provision={:.6} stores={:.6} adult_days={:.3}{}",
                    f.name(),
                    m.energy_balance,
                    m.days_to_empty.map_or("-".to_string(), |d| format!("{d:.2}")),
                    m.surplus_per_adult_day,
                    m.r_hat,
                    m.access,
                    m.sated_share,
                    books.assimilated,
                    books.upkeep,
                    books.motor,
                    books.rider_take_lost,
                    books.growth,
                    books.reproduction,
                    books.provision,
                    ctx.stores,
                    adult_days,
                    predator.unwrap_or_default(),
                );
            }
        }
        let bw = Founder::Bellwing.index();
        let dig = l.digested_by_founder[bw];
        println!(
            "bellwing_gut: swallowed={:.6} digested={:.6} assimilated={:.6} y_eff={:.3} sip_misses={} reserve_frac={:.3}->{:.3}",
            l.swallowed_by_founder[bw],
            dig,
            l.assimilated_by_founder[bw],
            if dig > 0.0 { l.assimilated_by_founder[bw] / dig } else { f64::NAN },
            l.sip_misses_by_founder[bw],
            self.bellwing_reserve0,
            bellwing_reserve(sim, params),
        );
        println!(
            "riders: attached={} dropoffs={} freed={} taken_organic={:.6}",
            l.riders_attached, l.rider_dropoffs, l.riders_freed, l.parasite_taken_organic
        );
    }
}

/// The mean reserve fraction of the living bellwings.
fn bellwing_reserve(sim: &Sim, params: &cubarium_voxel_sim::params::Params) -> f64 {
    let p = params.fauna.founder(Founder::Bellwing);
    let v: Vec<f64> = sim
        .fauna()
        .view()
        .animals
        .iter()
        .filter(|a| a.founder == Some(Founder::Bellwing))
        .map(|a| a.reserve / p.core.reserve_of(a.body).max(1e-300))
        .collect();
    if v.is_empty() { f64::NAN } else { v.iter().sum::<f64>() / v.len() as f64 }
}

/// Per pack (repair 3's measure): chases started with their participant counts, chase
/// ends by reason, and each feast's carrion eaten and why it ended; a feast still open at
/// the end is listed as `eaten:Open(carrion left)`.
fn print_packs(sim: &Sim, events: &[cubarium_voxel_fauna::PackEvent]) {
    use cubarium_voxel_fauna::PackEvent;
    let mut packs: Vec<u64> = events
        .iter()
        .map(|e| match e {
            PackEvent::ChaseStart { pack, .. } | PackEvent::ChaseEnd { pack, .. } | PackEvent::FeastEnd { pack, .. } => *pack,
        })
        .collect();
    packs.extend(sim.fauna().packs().iter().filter(|p| p.feast.is_some()).map(|p| p.id));
    packs.sort_unstable();
    packs.dedup();
    for p in packs {
        let parts: Vec<String> = events
            .iter()
            .filter_map(|e| match e {
                PackEvent::ChaseStart { pack, participants, .. } if *pack == p => Some(participants.to_string()),
                _ => None,
            })
            .collect();
        let mut ends = std::collections::BTreeMap::<String, u32>::new();
        let mut feasts: Vec<String> = Vec::new();
        for e in events {
            match e {
                PackEvent::ChaseEnd { pack, why, .. } if *pack == p => *ends.entry(format!("{why:?}")).or_default() += 1,
                PackEvent::FeastEnd { pack, eaten, why, .. } if *pack == p => feasts.push(format!("{eaten:.3}:{why:?}")),
                _ => {}
            }
        }
        if let Some(open) = sim.fauna().packs().iter().find(|r| r.id == p) {
            if let Some(fs) = open.feast {
                let left = sim.flora().view().ground_at(fs).map_or(0.0, |g| g.carrion);
                feasts.push(format!("{:.3}:Open({left:.3})", open.feast_eaten));
            }
        }
        let ends: Vec<String> = ends.iter().map(|(k, v)| format!("{k}={v}")).collect();
        println!(
            "pack{p}: chases={} participants=[{}] chase_ends=[{}] feasts=[{}]",
            parts.len(),
            parts.join(","),
            ends.join(" "),
            feasts.join(" ")
        );
    }
}

/// `reach`: the air a bellwing can fly through — cells that are not solid, not under
/// standing water, and no higher than `band` over the support face below them (§3.2's
/// ceiling) — split into 6-connected components, a point body (so an upper bound on the
/// real disc's reach). Prints, for the band and for a band without a ceiling: the
/// bloomcrown flower heads holding nectar, those with a reachable cell beside or over
/// them, and per bellwing its component's share of the heads; plus the head heights over
/// the bellwings.
fn print_reach(sim: &Sim, params: &cubarium_voxel_sim::params::Params, minute: u64) {
    let view = sim.world().view();
    let fv = sim.flora().view();
    let c = view.config;
    let v = c.voxel_m;
    let (w, d, h) = (c.width as usize, c.depth as usize, c.height as usize);
    let idx = |x: usize, y: usize, z: usize| (z * h + y) * w + x;
    let bw = Founder::Bellwing;
    let band = params.fauna.founder(bw).flight.map_or(2.5, |f| f.band_max_m);
    // The flower heads: (cells of the head disc's row, the stand).
    let heads: Vec<Vec<(usize, usize, usize)>> = fv
        .stands
        .iter()
        .filter(|s| s.nectar > 0.0)
        .filter_map(|s| {
            let l = fv.profile_layers(s).into_iter().filter(|l| l.kind.bears_foliage()).max_by_key(|l| l.cell)?;
            let span = l.radius_v.floor() as i64;
            let mut cells = Vec::new();
            for dz in -span..=span {
                for dx in -span..=span {
                    if ((dx * dx + dz * dz) as f64) > l.radius_v * l.radius_v {
                        continue;
                    }
                    let z = i64::from(s.site.z) + dz;
                    if z < 0 || z >= d as i64 || l.cell < 0 || l.cell >= h as i64 {
                        continue;
                    }
                    let x = (i64::from(s.site.x) + dx).rem_euclid(w as i64) as usize;
                    cells.push((x, l.cell as usize, z as usize));
                }
            }
            Some(cells)
        })
        .collect();
    let birds: Vec<(usize, usize, usize)> = sim
        .fauna()
        .view()
        .animals
        .iter()
        .filter(|a| a.founder == Some(bw))
        .map(|a| {
            (
                ((a.pose.x / v).floor() as i64).rem_euclid(w as i64) as usize,
                ((a.pose.y_m / v).floor().max(0.0) as usize).min(h - 1),
                ((a.pose.z / v).floor().max(0.0) as usize).min(d - 1),
            )
        })
        .collect();
    for (label, ceiling) in [("band", band), ("no_ceiling", f64::INFINITY)] {
        let air = |x: usize, y: usize, z: usize| -> bool {
            if y == 0 || view.is_solid(x as i64, y as u32, z as u32) {
                return false;
            }
            let Some(s) = view.highest_support_in(x as i64, z as u32, 0, (y - 1) as u32) else {
                return false;
            };
            let top = (f64::from(s) + 1.0) * v;
            let water = view.standing_depth_m(x as i64, s, z as u32);
            let base = y as f64 * v;
            base <= top + ceiling + 1e-9 && base + 1e-9 >= top + water
        };
        let mut comp = vec![u32::MAX; w * h * d];
        let mut next = 0u32;
        let mut stack = Vec::new();
        let mut sizes = Vec::new();
        for z in 0..d {
            for y in 0..h {
                for x in 0..w {
                    if comp[idx(x, y, z)] != u32::MAX || !air(x, y, z) {
                        continue;
                    }
                    comp[idx(x, y, z)] = next;
                    stack.push((x, y, z));
                    let mut n = 0u64;
                    while let Some((x, y, z)) = stack.pop() {
                        n += 1;
                        let mut nb = vec![((x + 1) % w, y, z), ((x + w - 1) % w, y, z)];
                        if y + 1 < h {
                            nb.push((x, y + 1, z));
                        }
                        if y > 0 {
                            nb.push((x, y - 1, z));
                        }
                        if z + 1 < d {
                            nb.push((x, y, z + 1));
                        }
                        if z > 0 {
                            nb.push((x, y, z - 1));
                        }
                        for (a, b, cc) in nb {
                            if comp[idx(a, b, cc)] == u32::MAX && air(a, b, cc) {
                                comp[idx(a, b, cc)] = next;
                                stack.push((a, b, cc));
                            }
                        }
                    }
                    sizes.push(n);
                    next += 1;
                }
            }
        }
        // Each head's components: any air cell in the head's row band (one row under to
        // two over) within one cell of its disc.
        let head_comps: Vec<Vec<u32>> = heads
            .iter()
            .map(|cells| {
                let mut out = Vec::new();
                for &(x, y, z) in cells {
                    for dy in -1i64..=2 {
                        for dz in -1i64..=1 {
                            for dx in -1i64..=1 {
                                let (yy, zz) = (y as i64 + dy, z as i64 + dz);
                                if yy < 0 || yy >= h as i64 || zz < 0 || zz >= d as i64 {
                                    continue;
                                }
                                let xx = (x as i64 + dx).rem_euclid(w as i64) as usize;
                                let k = comp[idx(xx, yy as usize, zz as usize)];
                                if k != u32::MAX && !out.contains(&k) {
                                    out.push(k);
                                }
                            }
                        }
                    }
                }
                out
            })
            .collect();
        let per_bird: Vec<String> = birds
            .iter()
            .map(|&(x, y, z)| {
                // The bird's cell, or the nearest air cell above it.
                let k = (y..h).map(|yy| comp[idx(x, yy, z)]).find(|&k| k != u32::MAX);
                match k {
                    Some(k) => format!(
                        "{}/{}@{}",
                        head_comps.iter().filter(|hc| hc.contains(&k)).count(),
                        heads.len(),
                        sizes[k as usize]
                    ),
                    None => "-".into(),
                }
            })
            .collect();
        let reachable_any = head_comps
            .iter()
            .filter(|hc| hc.iter().any(|k| birds.iter().any(|&(x, y, z)| (y..h).map(|yy| comp[idx(x, yy, z)]).find(|&q| q != u32::MAX) == Some(*k))))
            .count();
        println!(
            "reach@{minute} {label} heads={} reachable_by_some_bellwing={} per_bellwing(heads/total@cells)=[{}]",
            heads.len(),
            reachable_any,
            per_bird.join(" ")
        );
    }
}

/// One bellwing's life so far (`bwlife`).
struct BwRecord {
    born_tick: u64,
    founder: bool,
    body0: f64,
    reserve0: f64,
    nectar_last: f64,
    sips: u64,
    first_sip_tick: Option<u64>,
    aloft: u64,
    ticks: u64,
    last: cubarium_voxel_fauna::Animal,
}

/// `bwlife`: every bellwing tracked from birth (or founding) to death.
struct BwLife {
    live: std::collections::HashMap<u64, BwRecord>,
    prev_deaths: [u64; 8],
    dead: Vec<(BwRecord, u64, String)>,
}

impl BwLife {
    fn new(sim: &Sim) -> BwLife {
        let mut live = std::collections::HashMap::new();
        for a in sim.fauna().view().animals.iter().filter(|a| a.founder == Some(Founder::Bellwing)) {
            live.insert(a.id, BwLife::record(a, 0, true));
        }
        let mut prev = [0u64; 8];
        let d = sim.fauna().view().ledger.deaths_by_founder_cause[Founder::Bellwing.index()];
        for (k, v) in d.iter().enumerate().take(8) {
            prev[k] = *v;
        }
        BwLife { live, prev_deaths: prev, dead: Vec::new() }
    }

    fn record(a: &cubarium_voxel_fauna::Animal, tick: u64, founder: bool) -> BwRecord {
        BwRecord {
            born_tick: tick,
            founder,
            body0: a.body,
            reserve0: a.reserve,
            nectar_last: a.stomach.pools[Food::Nectar.index()].organic,
            sips: 0,
            first_sip_tick: None,
            aloft: 0,
            ticks: 0,
            last: *a,
        }
    }

    fn observe(&mut self, sim: &Sim, _params: &cubarium_voxel_sim::params::Params, tick: u64) {
        let view = sim.fauna().view();
        let mut seen: HashSet<u64> = HashSet::new();
        for a in view.animals.iter().filter(|a| a.founder == Some(Founder::Bellwing)) {
            seen.insert(a.id);
            let r = self.live.entry(a.id).or_insert_with(|| BwLife::record(a, tick, false));
            let n = a.stomach.pools[Food::Nectar.index()].organic;
            if n > r.nectar_last + 1e-15 {
                r.sips += 1;
                r.first_sip_tick.get_or_insert(tick);
            }
            r.nectar_last = n;
            r.ticks += 1;
            if a.mobility.mode == cubarium_voxel_fauna::MobilityMode::Fly {
                r.aloft += 1;
            }
            r.last = *a;
        }
        let gone: Vec<u64> = self.live.keys().copied().filter(|id| !seen.contains(id)).collect();
        if gone.is_empty() {
            return;
        }
        let d = view.ledger.deaths_by_founder_cause[Founder::Bellwing.index()];
        let mut causes: Vec<String> = Vec::new();
        for (k, v) in d.iter().enumerate().take(8) {
            for _ in self.prev_deaths[k]..*v {
                causes.push(format!("{:?}", Departure::ALL.get(k).copied()));
            }
            self.prev_deaths[k] = *v;
        }
        let cause = if causes.len() == gone.len() && causes.windows(2).all(|w| w[0] == w[1]) {
            causes.first().cloned().unwrap_or_else(|| "?".into())
        } else {
            format!("{causes:?}")
        };
        for id in gone {
            let r = self.live.remove(&id).expect("tracked");
            self.dead.push((r, tick, cause.clone()));
        }
    }

    fn summary(&self, sim: &Sim, params: &cubarium_voxel_sim::params::Params) {
        let p = params.fauna.founder(Founder::Bellwing);
        let flight = p.flight.expect("a flyer");
        let hz = f64::from(cubarium_voxel::TICK_HZ);
        // Seconds of cruise its reserve pays at its body (flight_factor x basal).
        let cruise_s = |reserve: f64, body: f64| reserve / (flight.flight_factor * p.core.maintenance_per_s * body);
        println!(
            "bwlife_table: body_min={:.6} body_max={:.6} birth_body={:.6} birth_cost={:.6} egg_organic={:.6} clutch={} reserve_cap={:.3} cruise_s_of_hatchling_reserve={:.0}",
            p.core.body_min,
            p.core.body_max,
            p.core.birth_body,
            p.core.birth_cost,
            p.core.reproduction.egg_organic,
            p.core.reproduction.clutch_size,
            p.core.reserve_cap,
            cruise_s(p.core.reproduction.egg_organic - p.core.body_min, p.core.body_min)
        );
        for (r, tick, cause) in &self.dead {
            let a = &r.last;
            println!(
                "bwdeath: {} cause={} born_min={:.1} age_s={:.0} body={:.6} body/max={:.3} body0={:.6} reserve0={:.6} reserve0_cruise_s={:.0} reserve_end={:.6} sips={} first_sip_age_s={} aloft={:.2} site=({},{},{}) mode={:?}",
                if r.founder { "founder" } else { "hatchling" },
                cause,
                r.born_tick as f64 / hz / 60.0,
                (tick - r.born_tick) as f64 / hz,
                a.body,
                a.body / p.core.body_max,
                r.body0,
                r.reserve0,
                cruise_s(r.reserve0, r.body0),
                a.reserve,
                r.sips,
                r.first_sip_tick.map_or("-".into(), |t| format!("{:.0}", (t - r.born_tick) as f64 / hz)),
                r.aloft as f64 / r.ticks.max(1) as f64,
                a.site.x,
                a.site.y,
                a.site.z,
                a.mobility.mode,
            );
        }
        let live = |founder: bool| self.live.values().filter(move |r| r.founder == founder);
        let dead = |founder: bool| self.dead.iter().filter(move |d| d.0.founder == founder);
        for (label, f) in [("founder", true), ("hatchling", false)] {
            println!(
                "bwlife: {label} n={} alive={} dead={} ever_sipped_alive={} ever_sipped_dead={} mean_body/max_alive={:.3}",
                live(f).count() + dead(f).count(),
                live(f).count(),
                dead(f).count(),
                live(f).filter(|r| r.sips > 0).count(),
                dead(f).filter(|d| d.0.sips > 0).count(),
                live(f).map(|r| r.last.body / p.core.body_max).sum::<f64>() / live(f).count().max(1) as f64,
            );
        }
        let _ = sim;
    }
}

/// `packtrace=S`: each pack's target and feast, and each member's state.
fn print_pack_trace(sim: &Sim, tick: u64) {
    let f = sim.fauna();
    let fv = sim.flora().view();
    let v = sim.world().config().voxel_m;
    let ring = f64::from(sim.world().config().width) * v;
    let dist = |a: &cubarium_voxel_fauna::Animal, s: cubarium_voxel_flora::Site| {
        let dx = ((f64::from(s.x) + 0.5) * v - a.pose.x).rem_euclid(ring);
        let dx = dx.min(ring - dx);
        let dz = (f64::from(s.z) + 0.5) * v - a.pose.z;
        (dx * dx + dz * dz).sqrt()
    };
    let t = tick / u64::from(cubarium_voxel::TICK_HZ);
    for p in f.packs() {
        let feast = p.feast.map(|s| {
            let g = fv.ground_at(s);
            format!(
                "({},{},{}) carrion={:.5} carcass={}",
                s.x,
                s.y,
                s.z,
                g.map_or(0.0, |g| g.carrion),
                g.is_some_and(|g| g.carcass.is_some())
            )
        });
        let target = p.target.and_then(|id| f.view().animal(id).copied());
        println!(
            "packtrace t={t} pack{} target={:?} {} since={}s target_site={:?} feast={} eaten={:.4} abandoned={:?}",
            p.id,
            p.target,
            target.map_or(String::new(), |o| format!(
                "{} site=({},{},{}) mode={:?} state={:?}",
                o.founder.map_or("-", |f| f.name()),
                o.site.x,
                o.site.y,
                o.site.z,
                o.mobility.mode,
                o.state
            )),
            tick.saturating_sub(p.chase_since_tick) / u64::from(cubarium_voxel::TICK_HZ),
            p.target_site.map(|s| (s.x, s.y, s.z)),
            feast.unwrap_or_else(|| "-".into()),
            p.feast_eaten,
            p.abandoned,
        );
        // The route from each member to the target (bellwing3): steps on the lineage's
        // route graph (out to 400) and whether the straight line is walkable.
        let graph = target.map(|_| {
            cubarium_voxel_fauna::RouteGraph::for_founder(
                &sim.world().view(),
                f.config().founder(Founder::Chorister),
                f.config().founding.clearance_margin_m,
            )
        });
        let field = target
            .zip(graph.as_ref())
            .and_then(|(o, g)| g.node_near(o.site, (o.pose.x, o.pose.z), 8).map(|j| g.distances(j, 400)));
        for a in f.view().animals.iter().filter(|a| a.pack.is_some_and(|m| m.pack == p.id)) {
            if let (Some(o), Some(g)) = (target, graph.as_ref()) {
                let node = g.node_near(a.site, (a.pose.x, a.pose.z), 2);
                let steps = node.and_then(|i| field.as_ref().and_then(|fl| fl.get(&i).copied()));
                print!(
                    "  route id={} steps={:?} straight={} ",
                    a.id,
                    steps,
                    node.is_some_and(|i| g.walk_line(i, (a.pose.x, a.pose.z), (o.pose.x, o.pose.z)))
                );
            }
            let pup = a.pack.is_some_and(|m| m.pup);
            println!(
                "  member id={} pup={pup} site=({},{},{}) state={:?} eating={} emergency={} reserve={:.4} held=({:.2},{:.2},{:.2}) d_feast={} d_target={}",
                a.id,
                a.site.x,
                a.site.y,
                a.site.z,
                a.state,
                a.eating,
                a.emergency,
                a.reserve,
                a.founder_state.held.forward,
                a.founder_state.held.turn,
                a.founder_state.held.feed,
                p.feast.map_or("-".into(), |s| format!("{:.2}", dist(a, s))),
                target.map_or("-".into(), |o| format!("{:.2}", dist(a, o.site))),
            );
        }
    }
}

fn is_sleeping(a: &cubarium_voxel_fauna::Animal) -> bool {
    a.state == cubarium_voxel_fauna::State::Asleep && a.rest == Some(cubarium_voxel_fauna::Rest::Sleep)
}

/// The night survival readouts, per lineage and per phase (0 day, 1 night).
#[derive(Default)]
struct NightTally {
    sleep_ticks: [[u64; 2]; Founder::COUNT],
    onsets: [[u64; 2]; Founder::COUNT],
    f0_sum: [[f64; 2]; Founder::COUNT],
    f0_min: [[f64; 2]; Founder::COUNT],
    /// Wakes by cause: alert, hunger, other.
    wakes: [[[u64; 3]; 2]; Founder::COUNT],
    wake_sum: [[f64; 2]; Founder::COUNT],
    deaths: [[[u64; cubarium_voxel_fauna::Departure::COUNT]; 2]; Founder::COUNT],
}

impl NightTally {
    fn observe(
        &mut self,
        sim: &Sim,
        params: &cubarium_voxel_sim::params::Params,
        prev_asleep: &mut std::collections::HashMap<u64, bool>,
        prev_deaths: &mut [[u64; cubarium_voxel_fauna::Departure::COUNT]; Founder::COUNT],
    ) {
        let phase = usize::from(sim.world().life_daylight() < 0.5);
        let view = sim.fauna().view();
        let mut seen: std::collections::HashMap<u64, bool> = std::collections::HashMap::with_capacity(view.animals.len());
        for a in view.animals {
            let Some(f) = a.founder else { continue };
            let k = f.index();
            let phys = params.fauna.founder(f);
            let cap = phys.core.reserve_of(a.body);
            let frac = if cap > 0.0 { a.reserve / cap } else { 0.0 };
            let now = is_sleeping(a);
            let was = prev_asleep.get(&a.id).copied().unwrap_or(false);
            if now {
                self.sleep_ticks[k][phase] += 1;
            }
            if now && !was {
                if self.onsets[k][phase] == 0 {
                    self.f0_min[k][phase] = f64::INFINITY;
                }
                self.onsets[k][phase] += 1;
                self.f0_sum[k][phase] += frac;
                self.f0_min[k][phase] = self.f0_min[k][phase].min(frac);
            }
            if was && !now {
                let cause = if a.alert_ticks > 0 {
                    0
                } else if 1.0 - frac >= phys.wake_hunger - 1e-9 {
                    1
                } else {
                    2
                };
                self.wakes[k][phase][cause] += 1;
                self.wake_sum[k][phase] += frac;
            }
            seen.insert(a.id, now);
        }
        *prev_asleep = seen;
        let deaths = view.ledger.deaths_by_founder_cause;
        for f in 0..Founder::COUNT {
            for d in 0..cubarium_voxel_fauna::Departure::COUNT {
                self.deaths[f][phase][d] += deaths[f][d] - prev_deaths[f][d];
            }
        }
        *prev_deaths = deaths;
    }

    fn print(&self, sim: &Sim) {
        let hz = f64::from(cubarium_voxel::TICK_HZ);
        for f in Founder::ALL {
            let k = f.index();
            for (p, name) in [(0, "day"), (1, "night")] {
                let n = self.onsets[k][p];
                let w: u64 = self.wakes[k][p].iter().sum();
                let deaths: Vec<String> = cubarium_voxel_fauna::Departure::ALL
                    .iter()
                    .filter_map(|d| {
                        let c = self.deaths[k][p][d.index()];
                        (c > 0).then(|| format!("{}={c}", d.name()))
                    })
                    .collect();
                println!(
                    "sleep: {:15} {:5} sleep_min={:.1} onsets={} f0_mean={:.3} f0_min={:.3} wakes=[alert={} hunger={} other={}] wake_frac_mean={:.3} deaths=[{}]",
                    f.name(),
                    name,
                    self.sleep_ticks[k][p] as f64 / hz / 60.0,
                    n,
                    if n > 0 { self.f0_sum[k][p] / n as f64 } else { f64::NAN },
                    if n > 0 { self.f0_min[k][p] } else { f64::NAN },
                    self.wakes[k][p][0],
                    self.wakes[k][p][1],
                    self.wakes[k][p][2],
                    if w > 0 { self.wake_sum[k][p] / w as f64 } else { f64::NAN },
                    deaths.join(" "),
                );
            }
        }
        let l = &sim.flora().view().ledger;
        for s in cubarium_voxel_flora::Species::ALL {
            let [r, n, fr] = l.night_spend[s.index()];
            if r + n + fr > 0.0 {
                println!("night_spend: {:13} reflush={r:.6} nectar={n:.6} fruit={fr:.6}", s.name());
            }
        }
    }
}
