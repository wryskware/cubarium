//! **The budget sheet**: between two [`Snap`]s, per world day — each stock's production,
//! each consumer's take and each loss (with the stock's own residual where its books close);
//! each species' births, maturation, deaths by cause and net change; the loop's ledger.

use std::fmt::Write;

use cubarium_voxel_fauna::{Food, Founder};
use cubarium_voxel_flora::Species;

use super::Snap;
use crate::flora::model::NS;

const GLOW: usize = 5;

/// The display name of a lineage (prose names; the code keeps `chorister`).
pub fn lineage_name(f: Founder) -> &'static str {
    match f {
        Founder::Chorister => "veilhound",
        f => f.name(),
    }
}

pub fn food_name(f: Food) -> &'static str {
    match f {
        Food::Litter => "litter",
        Food::CapTissue => "caps",
        Food::Carrion => "carrion",
        Food::Foliage => "foliage",
        Food::Film => "film",
        Food::Nectar => "nectar",
        Food::Fruit => "fruit",
    }
}

fn fmt(v: f64) -> String {
    let a = v.abs();
    if a == 0.0 {
        "0".into()
    } else if a >= 1000.0 {
        format!("{v:.0}")
    } else if a >= 10.0 {
        format!("{v:.1}")
    } else if a >= 0.1 {
        format!("{v:.2}")
    } else {
        format!("{v:.2e}")
    }
}

fn parts(v: &[(String, f64)]) -> String {
    let v: Vec<String> = v.iter().filter(|(_, x)| x.abs() > 1e-12).map(|(k, x)| format!("{k} {}", fmt(*x))).collect();
    if v.is_empty() { "-".into() } else { v.join(", ") }
}

/// Swallowed of `food` by lineage between the snaps, per day.
fn eaten(a: &Snap, b: &Snap, food: Food, per: f64) -> Vec<(String, f64)> {
    Founder::ALL
        .iter()
        .map(|&f| (lineage_name(f).to_string(), (b.fauna[f.index()].swallowed[food.index()] - a.fauna[f.index()].swallowed[food.index()]) * per))
        .collect()
}

/// **The sheet** from `a` to `b` (`day_s` the world day): one table a person can read.
pub fn sheet(a: &Snap, b: &Snap, day_s: f64) -> String {
    let days = ((b.minute - a.minute) * 60.0 / day_s).max(1e-9);
    let per = 1.0 / days;
    let (fa, fb) = (&a.flora, &b.flora);
    let mut s = String::new();
    let _ = writeln!(s, "BUDGET SHEET  minute {:.0} -> {:.0}  ({:.2} world days; flows per world day)", a.minute, b.minute, days);

    // ---- Stocks.
    let _ = writeln!(s, "\nSTOCKS  stock: start -> end (change/day) | in | taken by | lost | residual");
    let row = |s: &mut String, name: &str, s0: f64, s1: f64, inn: Vec<(String, f64)>, take: Vec<(String, f64)>, lost: Vec<(String, f64)>, closes: bool| {
        let d = (s1 - s0) * per;
        let resid = if closes {
            let i: f64 = inn.iter().map(|x| x.1).sum();
            let o: f64 = take.iter().chain(lost.iter()).map(|x| x.1).sum();
            format!(" | residual {}", fmt(d - (i - o)))
        } else {
            String::new()
        };
        let _ = writeln!(s, "  {name:<14} {} -> {} ({}) | in: {} | taken: {} | lost: {}{}", fmt(s0), fmt(s1), fmt(d), parts(&inn), parts(&take), parts(&lost), resid);
    };
    // Litter and dead wood together (the fungi feed on both; decomposition is booked together).
    let lit_in: f64 = fb.prod_z.iter().map(|p| p[0] + p[1]).sum::<f64>() - fa.prod_z.iter().map(|p| p[0] + p[1]).sum::<f64>();
    let dung = b.web.returned.litter - a.web.returned.litter;
    let dec_all = fb.decomposed - fa.decomposed;
    let dec_car = fb.decomposed_carrion - fa.decomposed_carrion;
    let fungi = fb.substrate - fa.substrate;
    let mut take = eaten(a, b, Food::Litter, per);
    take.push(("fungi".into(), fungi * per));
    row(&mut s, "litter+wood", a.litter, b.litter, vec![("plants".into(), lit_in * per), ("dung".into(), dung * per)], take, vec![("decomposition".into(), (dec_all - dec_car) * per)], true);
    // Carrion.
    let (out_a, out_b) = (a.carcass.3, b.carcass.3);
    let (left_a, left_b) = (a.carcass.2, b.carcass.2);
    let car_in = b.web.returned.carrion - a.web.returned.carrion;
    let beyond = out_b - out_a;
    let left = left_b - left_a;
    row(
        &mut s,
        "carrion",
        a.carrion,
        b.carrion,
        vec![("kills beyond reach".into(), beyond * per), ("kills left".into(), left * per), ("corpses+eggs".into(), (car_in - beyond - left) * per)],
        eaten(a, b, Food::Carrion, per),
        vec![("decomposition".into(), dec_car * per)],
        true,
    );
    // Film.
    row(
        &mut s,
        "film",
        a.film,
        b.film,
        vec![("grown".into(), (fb.film_grown - fa.film_grown) * per)],
        eaten(a, b, Food::Film, per),
        vec![("upkeep".into(), (fb.film_upkeep - fa.film_upkeep) * per)],
        true,
    );
    // The animals' fungal food class includes mycelium after caps are cropped.
    let mycelium = (fb.mycelium_taken-fa.mycelium_taken)*per;
    let tissue = (fb.taken.foliage[GLOW]-fa.taken.foliage[GLOW])*per;
    row(&mut s, "glowcap caps", a.caps_total, b.caps_total, vec![("built".into(), (fb.built[GLOW] - fa.built[GLOW]) * per)], vec![("all consumers".into(), (tissue-mycelium).max(0.0))], vec![("shed".into(), (fb.shed[GLOW] - fa.shed[GLOW]) * per)], false);
    let _ = writeln!(s, "  fungal tissue taken: caps {}, mycelium {} (per day; lineage intake below includes both)", fmt((tissue-mycelium).max(0.0)), fmt(mycelium));
    // Foliage per photosynthetic species: the takeable stock; built, shed, eaten (by share).
    let _ = writeln!(s, "  foliage (takeable stock; built | eaten | shed):");
    for sp in Species::ALL {
        let k = sp.index();
        if k == GLOW {
            continue;
        }
        let got = (b.web.got.foliage[k] - a.web.got.foliage[k]) * per;
        let built = (fb.built[k] - fa.built[k]) * per;
        if a.offer.foliage[k] + b.offer.foliage[k] + built <= 1e-9 {
            continue;
        }
        let _ = writeln!(s, "    {:<14} {} -> {} | built {} | eaten {} | shed {}", sp.name(), fmt(a.offer.foliage[k]), fmt(b.offer.foliage[k]), fmt(built), fmt(got), fmt((fb.shed[k] - fa.shed[k]) * per));
    }
    let _ = writeln!(s, "    eaten by: {}", parts(&eaten(a, b, Food::Foliage, per)));
    row(&mut s, "nectar", a.offer.nectar, b.offer.nectar, vec![], eaten(a, b, Food::Nectar, per), vec![], false);
    row(&mut s, "fruit", a.offer.fruit, b.offer.fruit, vec![], eaten(a, b, Food::Fruit, per), vec![], false);

    // ---- Animals.
    let _ = writeln!(s, "\nANIMALS  bodies start -> end (adults/juveniles/riders, eggs+bank at end) | per day: births, matured, deaths by cause, net | intake per day");
    for f in Founder::ALL {
        let i = f.index();
        let (c0, c1) = (&a.census[i], &b.census[i]);
        if !b.enabled[i] {
            if c1.n > 0.0 {
                let _ = writeln!(s, "  {:<14} {} held (not stepped: no fitted rates)", lineage_name(f), fmt(c1.n));
            }
            continue;
        }
        let (l0, l1) = (&a.fauna[i], &b.fauna[i]);
        let births = (l1.hatched + l1.born - l0.hatched - l0.born) * per;
        let matured = (l1.matured - l0.matured) * per;
        let mut deaths = vec![("starved".to_string(), (l1.starved - l0.starved) * per), ("aged".to_string(), (l1.aged - l0.aged) * per), ("drowned".to_string(), (l1.drowned - l0.drowned) * per)];
        for h in Founder::ALL {
            deaths.push((format!("killed by {}", lineage_name(h)), (l1.killed_by[h.index()] - l0.killed_by[h.index()]) * per));
        }
        let lost = (l1.eggs_lost + l1.bank_died - l0.eggs_lost - l0.bank_died) * per;
        let intake: Vec<(String, f64)> = Food::ALL.iter().map(|&fd| (food_name(fd).to_string(), (l1.swallowed[fd.index()] - l0.swallowed[fd.index()]) * per)).collect();
        let _ = writeln!(
            s,
            "  {:<14} {} -> {} ({}/{}/{}, eggs {} bank {}) | births {} matured {} | deaths: {} | eggs lost {} | net {} | eats {} | assim {} resp {}",
            lineage_name(f),
            fmt(c0.n),
            fmt(c1.n),
            fmt(c1.adults),
            fmt(c1.juveniles),
            fmt(c1.riders),
            fmt(c1.eggs),
            fmt(c1.bank),
            fmt(births),
            fmt(matured),
            parts(&deaths),
            fmt(lost),
            fmt((c1.n - c0.n) * per),
            parts(&intake),
            fmt((l1.flows.assimilated - l0.flows.assimilated) * per),
            fmt((super::respired(l1) - super::respired(l0)) * per),
        );
    }

    // ---- Plants.
    let _ = writeln!(s, "\nPLANTS  stands start -> end | per day: births seed/runner | deaths starved, drought, overtopped, displaced, browsed | net");
    for sp in Species::ALL {
        let k = sp.index();
        if a.stands[k] + b.stands[k] <= 0.0 {
            continue;
        }
        let d = |x: &[f64; NS], y: &[f64; NS]| (y[k] - x[k]) * per;
        let _ = writeln!(
            s,
            "  {:<14} {} -> {} | births {} / {} | deaths {} {} {} {} {} | net {}",
            sp.name(),
            fmt(a.stands[k]),
            fmt(b.stands[k]),
            fmt(d(&fa.births_seed, &fb.births_seed)),
            fmt(d(&fa.births_runner, &fb.births_runner)),
            fmt(d(&fa.starved, &fb.starved)),
            fmt(d(&fa.drought, &fb.drought)),
            fmt(d(&fa.overtopped, &fb.overtopped)),
            fmt(d(&fa.displaced, &fb.displaced)),
            fmt(d(&fa.browsed, &fb.browsed)),
            fmt((b.stands[k] - a.stands[k]) * per),
        );
    }

    // ---- The loop's ledger.
    let cl = &b.closure;
    let _ = writeln!(
        s,
        "\nLEDGER  organic held {} (books say {}, residual {:.2e}; plant half {:.2e}, animal half {:.2e}) | fixed {} seep {} plants resp {} decomposed {} animals resp {} browsed seedlings {} unmet {} (per day)",
        fmt(cl.organic_held),
        fmt(cl.organic_expected),
        (cl.organic_held - cl.organic_expected) / cl.organic_held.abs().max(1.0),
        cl.flora_gap,
        cl.fauna_gap,
        fmt((fb.fixed - fa.fixed) * per),
        fmt((fb.seep - fa.seep) * per),
        fmt((fb.respired - fa.respired) * per),
        fmt((fb.decomposed - fa.decomposed) * per),
        fmt(b.fauna.iter().zip(a.fauna.iter()).map(|(x, y)| super::respired(x) - super::respired(y)).sum::<f64>() * per),
        fmt((fb.browsed_organic - fa.browsed_organic) * per),
        fmt((b.web.unmet - a.web.unmet) * per),
    );
    let _ = writeln!(
        s,
        "        mineral held {} (books say {}, residual {:.2e}; plant half {:.2e}) | out in bites {} back in returns {} caps' mineral in {} (per day) | animals hold {} of the {} their tissue wants",
        fmt(cl.mineral_held),
        fmt(cl.mineral_expected),
        (cl.mineral_held - cl.mineral_expected) / cl.mineral_held.abs().max(1.0),
        cl.flora_mineral_gap,
        fmt((fb.taken_mineral - fa.taken_mineral) * per),
        fmt((b.web.returned.mineral - a.web.returned.mineral) * per),
        fmt((fb.cap_mineral_taken - fa.cap_mineral_taken) * per),
        fmt(b.fauna_mineral.0),
        fmt(b.fauna_mineral.1),
    );
    s
}
