//! Exercise emitted hits and a real corpse transfer without stepping a world.

use super::*;
use crate::size_not_mass_tests::{close, get, rescale, scene};

fn strike_damage(hf: Founder, pf: Founder, scaled: Founder, k: f64) -> f64 {
    let (world, mut fauna, hid, pid) = scene(hf, pf);
    let phys = &mut fauna.config.founders[hf.index()];
    let h = phys.hunter.as_mut().unwrap();
    h.strike_power = 0.2;
    h.jaw_forward_length_fraction = 0.0;
    h.strike_reach_length_fraction = 0.0;
    h.jaw_band_height_fraction = 20.0;
    for id in [hid, pid] {
        let a = fauna.animals.iter_mut().find(|a| a.id == id).unwrap();
        a.pose.x = 2.0;
        a.pose.z = 1.5;
        a.pose.y_m = 0.75;
        a.pose.heading_rad = 0.0;
    }
    let i = fauna.animals.binary_search_by_key(&hid, |a| a.id).unwrap();
    fauna.animals[i].mobility.mode = crate::MobilityMode::Perched;
    fauna.animals[i].strike = Some(Strike {
        phase: StrikePhase::Lunge,
        ticks_left: 1,
        prey: pid,
        from_x: 2.0,
        from_z: 1.5,
        lure: false,
    });
    rescale(&mut fauna, scaled, k);
    let phys = *fauna.config.founder(hf);
    let mut hits = Vec::new();
    strike_tick(
        &mut fauna,
        i,
        &world.view(),
        &phys,
        phys.hunter.unwrap(),
        913,
        1,
        &mut hits,
    );
    let hit = hits
        .iter()
        .find(|h| h.prey == pid && !h.counter)
        .expect("fixture strike contacts prey");
    assert!(hit.damage > 0.0);
    hit.damage
}

#[test]
fn strike_damage_ignores_both_lineages_demand_scales() {
    for (hf, pf) in [
        (Founder::Lanternjaw, Founder::Chorister),
        (Founder::Chorister, Founder::Lanternjaw),
    ] {
        let before = strike_damage(hf, pf, pf, 1.0);
        for scaled in [hf, pf] {
            for k in [0.2, 5.0] {
                close(strike_damage(hf, pf, scaled, k), before);
            }
        }
    }
}

#[test]
fn grown_lanternjaw_counter_ignores_both_lineages_demand_scales() {
    for grown in [false, true] {
        for scaled in [Founder::Chorister, Founder::Lanternjaw] {
            let (world, mut fauna, hid, pid) = scene(Founder::Chorister, Founder::Lanternjaw);
            let birth = fauna.config.founder(Founder::Lanternjaw).core.birth_body;
            fauna.animals.iter_mut().find(|a| a.id == pid).unwrap().body =
                if grown { 1.25 * birth } else { 0.75 * birth };
            let jaw = (get(&fauna, pid).pose.x, get(&fauna, pid).pose.z);
            let before = counter_answer(
                &fauna.config,
                &world.view(),
                &get(&fauna, hid),
                jaw,
                &get(&fauna, pid),
            );
            assert_eq!(before.is_some(), grown, "only a grown body counters");
            rescale(&mut fauna, scaled, 6.0);
            let after = counter_answer(
                &fauna.config,
                &world.view(),
                &get(&fauna, hid),
                jaw,
                &get(&fauna, pid),
            );
            assert_eq!(after.is_some(), before.is_some());
            if let (Some(after), Some(before)) = (after, before) {
                close(after.damage, before.damage);
            }
        }
    }
}

fn kill_meal_energy(k: f64) -> f64 {
    let (_, mut fauna, hid, pid) = scene(Founder::Lanternjaw, Founder::Blind);
    let mut flora = Flora::new(cubarium_voxel_flora::FloraConfig::default());
    let i = fauna.animals.binary_search_by_key(&hid, |a| a.id).unwrap();
    fauna.animals[i].reserve = 0.0;
    let prey = fauna.animals.iter_mut().find(|a| a.id == pid).unwrap();
    prey.reserve = 0.0;
    prey.energy = 2.0 * prey.organic();
    rescale(&mut fauna, Founder::Blind, k);
    let prey = get(&fauna, pid);
    apply_hit(&mut fauna, &mut flora, pid, 1.0, Founder::Lanternjaw, None);
    assert!(fauna.view().animal(pid).is_none());
    let t = flora
        .take_carrion(prey.site, 2.0 * prey.stored_organic())
        .expect("killed prey leaves food");
    close(t.energy, prey.stored_energy());
    let i = fauna.animals.binary_search_by_key(&hid, |a| a.id).unwrap();
    let before = fauna.animals[i].energy;
    let sc = fauna.config.founder(Founder::Lanternjaw).core;
    let t = fauna.book_eaten(t, Food::Carrion);
    assimilate(&mut fauna, i, &sc, t, 0.5);
    let gained = fauna.animals[i].energy - before;
    assert!(gained > 0.0, "the hunter assimilates the killed prey");
    gained
}

#[test]
fn killing_and_eating_prey_gains_energy_proportional_to_its_demand_scale() {
    let before = kill_meal_energy(1.0);
    for k in [0.25, 4.0] {
        close(kill_meal_energy(k), k * before);
    }
}
