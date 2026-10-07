//! Bounded validation of the current model against already-paid hunter_study logs.
//! `found=DIR log=FILE mode=closed|fauna|flora [founders=1] [legacy_taste=1] set=...`
//! No simulation ticks are run. `fauna` clamps producer stocks and non-focal fauna;
//! absent nectar telemetry is explicitly replaced by a measured intake clamp.
use cubarium_voxel_fauna::{Food, Founder};
use cubarium_voxel_flora::Species;
use cubarium_webmodel::{
    exchange::{Supply, Take},
    fauna::{Cohort, EggCohort},
    web::{start, Web},
};
use std::{collections::BTreeMap, path::Path};

#[derive(Clone, Default)]
struct Observation {
    n: BTreeMap<String, f64>,
    food: BTreeMap<String, f64>,
}
fn field(line: &str, key: &str) -> Option<f64> {
    line.split([',', ' ']).find_map(|p| {
        p.strip_prefix(&format!("{key}="))
            .and_then(|s| s.parse().ok())
    })
}
fn read_log(path: &Path) -> (BTreeMap<u32, Observation>, [f64; Founder::COUNT]) {
    let mut rows = BTreeMap::new();
    let mut intake = [0.0; Founder::COUNT];
    for line in std::fs::read_to_string(path).unwrap().lines() {
        let cols: Vec<_> = line.split(',').collect();
        if cols.len() < 3 {
            continue;
        }
        if cols[0] == "stocks" {
            let minute: u32 = cols[2].parse().unwrap();
            let row = rows.entry(minute).or_insert_with(Observation::default);
            for f in Founder::ALL {
                if let Some(v) = field(line, f.name()) {
                    row.n.insert(f.name().into(), v);
                }
            }
            for k in ["litter", "film", "cap_tissue", "glowcap_stands"] {
                if let Some(v) = field(line, k) {
                    row.food.insert(k.into(), v);
                }
            }
        }
        if cols[0] == "snail" {
            let minute: u32 = cols[2].parse().unwrap();
            if let Some(v) = field(line, "land_film") {
                rows.entry(minute)
                    .or_insert_with(Observation::default)
                    .food
                    .insert("land_film".into(), v);
            }
        }
        if ["snail", "bwl"].contains(&cols[0]) {
            let minute: u32 = cols[2].parse().unwrap();
            let (rate, hours, key) = if cols[0] == "snail" {
                ("intake_per_h", "snail_h", "film_assimilated")
            } else {
                ("nectar_per_bwh", "bw_hours", "nectar_taken")
            };
            let amount = field(line, rate).unwrap_or(0.0) * field(line, hours).unwrap_or(0.0);
            rows.entry(minute)
                .or_insert_with(Observation::default)
                .food
                .insert(key.into(), if amount.is_finite() { amount } else { 0.0 });
        }
        if cols[0] == "lj" || cols[0] == "vh" {
            let minute: u32 = cols[2].parse().unwrap();
            let (field_key, key) = if cols[0] == "lj" {
                ("riders", "lanternjaw_riders")
            } else {
                ("pups", "chorister_pups")
            };
            rows.entry(minute)
                .or_insert_with(Observation::default)
                .food
                .insert(key.into(), field(line, field_key).unwrap_or(0.0));
        }
        if cols[0] == "sumsnail" {
            intake[Founder::RippleSnail.index()] =
                field(line, "intake_per_h").unwrap_or(0.0) / 3600.0;
        }
        if cols[0] == "sumbwl" {
            intake[Founder::Bellwing.index()] =
                field(line, "nectar_per_bwh").unwrap_or(0.0) / 3600.0;
        }
    }
    (rows, intake)
}
fn interpolate(rows: &BTreeMap<u32, Observation>, m: f64) -> Observation {
    let minute = m.floor().max(0.0) as u32;
    let (t, a) = rows.range(..=minute).next_back().unwrap();
    let (u, b) = rows.range((minute + 1)..).next().unwrap_or((t, a));
    let w = if u > t {
        ((m - *t as f64) / (*u - *t) as f64).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let mix = |x: &BTreeMap<String, f64>, y: &BTreeMap<String, f64>| {
        x.iter()
            .map(|(k, v)| (k.clone(), v + (y.get(k).unwrap_or(v) - v) * w))
            .collect()
    };
    Observation {
        n: mix(&a.n, &b.n),
        food: mix(&a.food, &b.food),
    }
}
fn print(w: &Web) {
    let minute = w.minute();
    for f in Founder::ALL {
        let l = &w.fauna.ledger[f.index()];
        let c = w.fauna.census(f);
        let hours = l.alive_s / 3600.0;
        let refuge = w.fauna.pops[f.index()]
            .iter()
            .filter(|c| c.film_patch.is_some_and(|v| v <= f64::EPSILON) && c.film_travel_s == 0.0)
            .map(|c| c.n)
            .sum::<f64>()
            / c.n.max(f64::MIN_POSITIVE);
        println!("animal,{minute:.0},{},n={},born={},intake_h={},assim_h={},nectar_h={},hunker={},flight={},juvenile_kills={},adult_kills={},lure_visits={},lure_kills={},killed_lj={},killed_vh={},eggs_drowned={},eggs_dried={},refuge={},refuge_time={}",
            f.name(),c.n,l.born+l.hatched,l.swallowed.iter().sum::<f64>()/hours.max(f64::MIN_POSITIVE),l.flows.assimilated/hours.max(f64::MIN_POSITIVE),l.swallowed[Food::Nectar.index()]/(l.awake_s/3600.0).max(f64::MIN_POSITIVE),l.hunker_s/l.awake_s.max(f64::MIN_POSITIVE),l.flight_s/l.awake_s.max(f64::MIN_POSITIVE),l.kills_juvenile,l.kills_adult,l.lure_visits,l.lure_kills,l.killed_by[Founder::Lanternjaw.index()],l.killed_by[Founder::Chorister.index()],l.eggs_drowned,l.eggs_dried,refuge,l.refuge_s/l.awake_s.max(f64::MIN_POSITIVE));
    }
    let s = w.flora.supply();
    let stands = w.flora.stands();
    let land: f64 = w
        .flora
        .film_beds
        .iter()
        .filter(|b| b.land)
        .map(|b| b.stock)
        .sum();
    let caps: f64 = w
        .flora
        .z
        .iter()
        .flat_map(|z| z.cohorts[Species::Glowcap.index()].iter())
        .map(|c| c.n * c.st.foliage)
        .sum();
    println!("producer,{minute:.0},litter={},film={},land_film={land},cap_tissue={caps},glowcap_stands={},nectar={},fruit={}",s.litter,w.flora.film,stands[Species::Glowcap.index()].0,s.nectar,s.fruit);
    let c = w.closure();
    println!(
        "closure,{minute:.0},organic={},mineral={}",
        c.organic_held - c.organic_expected,
        c.mineral_held - c.mineral_expected
    );
}

fn clamp_fauna(m: &mut cubarium_webmodel::fauna::FaunaModel, o: &Observation) {
    for f in Founder::ALL {
        if [Founder::Browser, Founder::RippleSnail, Founder::Bellwing].contains(&f) {
            continue;
        }
        let phys = *m.phys(f);
        let special = match f {
            Founder::Lanternjaw => o.food.get("lanternjaw_riders").copied().unwrap_or(0.0),
            Founder::Chorister => o.food.get("chorister_pups").copied().unwrap_or(0.0),
            _ => 0.0,
        };
        let n = o.n.get(f.name()).copied().unwrap_or(0.0);
        m.pops[f.index()].retain(|c| c.n > 0.0);
        let group = |c: &Cohort| match f {
            Founder::Lanternjaw => c.host.is_some(),
            Founder::Chorister => {
                c.native && phys.pack.is_some_and(|p| c.age_s < p.independence_age_s)
            }
            _ => false,
        };
        for young in [false, true] {
            let target = if young {
                special
            } else {
                (n - special).max(0.0)
            };
            let old: f64 = m.pops[f.index()]
                .iter()
                .filter(|c| group(c) == young)
                .map(|c| c.n)
                .sum();
            if old > 0.0 {
                for c in m.pops[f.index()].iter_mut().filter(|c| group(c) == young) {
                    c.n *= target / old;
                }
            } else if target > 0.0 {
                let mut c = if young {
                    Cohort::new(
                        target,
                        phys.core.body_min,
                        (phys.core.reproduction.egg_organic - phys.core.body_min).max(0.0),
                    )
                } else {
                    Cohort::founders(&m.config, f, target)
                };
                if young {
                    c.native = true;
                    c.host = if f == Founder::Lanternjaw {
                        Some(Founder::Browser)
                    } else {
                        None
                    };
                }
                m.add(f, c);
            }
        }
    }
}
fn main() {
    let args: Vec<String> = std::env::args().collect();
    let arg = |k: &str| args.iter().find_map(|s| s.strip_prefix(k));
    let found = Path::new(arg("found=").expect("found=DIR"));
    let (rows, intake) = read_log(Path::new(arg("log=").expect("log=FILE")));
    let mode = arg("mode=").unwrap_or("closed");
    let sets: Vec<_> = args
        .iter()
        .filter_map(|s| s.strip_prefix("set=").map(str::to_owned))
        .collect();
    let p = start::params(
        Path::new(arg("config=").unwrap_or("config/desktop/terrarium.toml")),
        &sets,
    )
    .unwrap();
    let flora = start::flora(&p, found, &[], 1).unwrap();
    let mut census = start::FaunaCensus::default();
    let exact = cubarium_voxel_sim::saved::load(found, &[]);
    if let Ok(save) = exact {
        let fv = save.fauna.view();
        census.tick = fv.tick;
        for a in fv.animals {
            if let Some(f) = a.founder {
                let host = a
                    .attached
                    .and_then(|r| fv.animal(r.host))
                    .and_then(|h| h.founder);
                census
                    .bodies
                    .push((f, a.body, a.reserve, a.age_ticks, host));
            }
        }
        for c in fv.clutches {
            census.clutches.push((
                c.lineage,
                c.count,
                c.organic,
                c.laid_tick,
                c.dormancy.is_some(),
            ));
        }
    } else {
        assert_eq!(
            arg("founders="),
            Some("1"),
            "fauna save is unavailable/incompatible; founders=1 explicitly permits reconstruction"
        );
        eprintln!("START LIMIT: fauna reconstructed from the m0 counts and resolved arrival stores; old-schema save not decoded");
    }
    let mut fauna = start::fauna(&p, start::closed_fits(), &census, &Founder::ALL, 1.0);
    cubarium_webmodel::web::to_whole(&mut fauna);
    fauna.area_scale = flora.zones.zones.iter().map(|z| z.area).sum::<f64>() / 4748.0;
    fauna.ref_area = [74.19, 30.66, 74.19, 32.69, 15.09, 0.0, 0.0];
    let zero = rows.values().next().unwrap();
    for f in Founder::ALL {
        let n = zero.n.get(f.name()).copied().unwrap_or(0.0);
        let current = fauna.census(f).n;
        if current != n {
            fauna.pops[f.index()] = if n > 0.0 {
                vec![Cohort::founders(&p.fauna, f, n)]
            } else {
                Vec::new()
            };
        }
    }
    if census.clutches.is_empty() {
        let bank = std::fs::read_to_string(arg("log=").unwrap())
            .unwrap()
            .lines()
            .find(|l| l.starts_with("lj,") && l.split(',').nth(2) == Some("0"))
            .and_then(|l| field(l, "bank"))
            .unwrap_or(0.0);
        fauna.eggs.push(EggCohort {
            lineage: Founder::Lanternjaw,
            n: bank,
            organic: p
                .fauna
                .founder(Founder::Lanternjaw)
                .core
                .reproduction
                .egg_organic,
            dormant: true,
            ..Default::default()
        });
    }
    fauna.edible_perception = arg("legacy_taste=") != Some("1");
    let mut w = Web::new(flora, fauna);
    let h = w.fauna.habitat.as_ref().unwrap();
    eprintln!(
        "HABITAT area={} film_faces={} film_rich={} reference={} refuge={} shore={} film_offer={}",
        h.area,
        h.film_faces,
        h.film_rich,
        h.film_reference,
        h.film_refuge,
        h.shore.iter().map(|s| s.faces).sum::<f64>(),
        w.flora.offer().film
    );
    if mode == "fauna" {
        eprintln!("OPEN: film/litter/caps and non-focal fauna counts clamped; nectar intake clamped because stocks logs omit nectar. The intake clamp is not an independent feeding-law test.");
        let mut clamp = [[None; Food::COUNT]; Founder::COUNT];
        clamp[Founder::Bellwing.index()][Food::Nectar.index()] =
            Some(intake[Founder::Bellwing.index()]);
        // Only nectar is unavailable as a stock; film still exercises the new foraging law.
        w.fauna.clamped_intake = Some(clamp);
    }
    if mode == "flora" {
        eprintln!("OPEN PRODUCER LIMIT: nectar take is measured per segment; film take is inferred from assimilation / configured yield (gut inventory delay unmeasured). Other grazing/returns absent from paid logs are held at zero; only film validation is independent of the fauna response.");
    }
    print(&w);
    let hours: u32 = arg("hours=").unwrap_or("6").parse().unwrap();
    for hour in 1..=hours {
        let target = hour as f64 * 60.0;
        if mode == "closed" {
            w.run_to(target);
        } else if mode == "flora" {
            while w.minute() < target {
                let minute = w.minute().floor() as u32;
                let (t, _) = rows.range(..=minute).next_back().unwrap();
                let (u, o) = rows
                    .range((minute + 1)..)
                    .next()
                    .unwrap_or_else(|| rows.last_key_value().unwrap());
                let span = (u - t).max(1) as f64 * 60.0;
                let yield_f = p.fauna.founder(Founder::RippleSnail).yield_for(Food::Film);
                w.flora.take = Take {
                    film: o.food.get("film_assimilated").copied().unwrap_or(0.0)
                        / yield_f.max(f64::MIN_POSITIVE)
                        * w.flora.p.dt
                        / span,
                    nectar: o.food.get("nectar_taken").copied().unwrap_or(0.0) * w.flora.p.dt
                        / span,
                    ..Default::default()
                };
                w.flora.step();
            }
        } else {
            while w.minute() < target {
                let o = interpolate(&rows, w.minute());
                // Preserve the model's focal grazer/snail/bellwing trajectories. Predators
                // and competing fauna are forced to the paid run's census at this minute.
                w.flora.step();
                // Retain the census' face distribution while clamping measured film totals,
                // separately on land and under water. No whole-world refuge subtraction.
                let film = o.food.get("film").copied().unwrap_or(w.flora.film);
                let land = o.food.get("land_film").copied().unwrap_or_else(|| {
                    w.flora
                        .film_beds
                        .iter()
                        .filter(|b| b.land)
                        .map(|b| b.stock)
                        .sum::<f64>()
                        .min(film)
                });
                for is_land in [false, true] {
                    let total: f64 = w
                        .flora
                        .film_beds
                        .iter()
                        .filter(|b| b.land == is_land)
                        .map(|b| b.stock)
                        .sum();
                    let target = if is_land {
                        land
                    } else {
                        (film - land).max(0.0)
                    };
                    if total > 0.0 {
                        for b in w.flora.film_beds.iter_mut().filter(|b| b.land == is_land) {
                            b.stock *= target / total;
                        }
                    }
                }
                w.flora.film = w.flora.film_beds.iter().map(|b| b.stock).sum();
                w.fauna.habitat = Some(w.flora.habitat(w.fauna.phys(Founder::RippleSnail)));
                let mut s: Supply = w.flora.offer();
                s.litter = o.food.get("litter").copied().unwrap_or(0.0);
                s.foliage[Species::Glowcap.index()] =
                    o.food.get("cap_tissue").copied().unwrap_or(0.0)
                        * (1.0 - p.flora.glowcap.graze_refuge);
                s.film = w.flora.edible_film();
                s.nectar = 1e6; // External supply for the explicitly measured intake clamp.
                for _ in 0..w.sub {
                    clamp_fauna(&mut w.fauna, &o);
                    w.fauna.step(&s);
                }
            }
        }
        print(&w);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn a_logged_rider_is_clamped_as_a_rider() {
        let p = cubarium_voxel_sim::params::Params::defaults();
        let mut m = cubarium_webmodel::fauna::FaunaModel::new(&p, start::closed_fits(), 1.0);
        let mut o = Observation::default();
        o.n.insert(Founder::Lanternjaw.name().into(), 3.0);
        o.food.insert("lanternjaw_riders".into(), 3.0);
        clamp_fauna(&mut m, &o);
        let c = m.census(Founder::Lanternjaw);
        assert_eq!(c.n, c.riders);
        assert_eq!(c.riders, 3.0);
        assert_eq!(c.adults, 0.0);
    }
}
