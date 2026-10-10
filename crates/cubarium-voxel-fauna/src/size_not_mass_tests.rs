//! Demand rescaling changes held matter, not an animal's physical size.

#[allow(dead_code)]
#[path = "predation_fixture.rs"]
mod fixture;

use crate::{Animal, Fauna, FaunaConfig, Founder, StartingStores};
use cubarium_voxel::World;
use fixture::*;

pub(crate) fn close(got: f64, want: f64) {
    assert!(
        (got - want).abs() <= 1e-11 * want.abs().max(1.0),
        "{got} != {want}"
    );
}

pub(crate) fn scene(hf: Founder, pf: Founder) -> (World, Fauna, u64, u64) {
    let world = clock_off(913);
    let mut config = FaunaConfig::default();
    for p in &mut config.founders {
        p.scale_mass(1.0 / p.mass_scale);
        p.catch_bellwings = true;
        p.catch_gain_min = 0.0;
        p.adult_size_window_factor = 1.0;
        p.agility = 0.0;
        p.hunker.enabled = false;
        p.core.reserve_cap = 100.0;
    }
    if let Some(h) = config.founders[hf.index()].hunter.as_mut() {
        h.prey = [true; Founder::COUNT];
        h.prey_juveniles_only = [false; Founder::COUNT];
        h.prey_ashore_only = [false; Founder::COUNT];
        h.prey_max_ratio = 1.0;
        h.prey_weight = [1.0; Founder::COUNT];
        h.strike_cost_s = 0.0;
    }
    let mut fauna = Fauna::new(config);
    fauna.set_births_enabled(false);
    let h = introduce(&mut fauna, &world, hf, StartingStores::FULL);
    let p = introduce(&mut fauna, &world, pf, StartingStores::FULL);
    for id in [h, p] {
        place(&mut fauna, id, 2.0, LANE_Z, EAST);
        let a = animal_mut(&mut fauna, id);
        a.mobility.mode = crate::MobilityMode::Ground;
        a.mobility.falling = false;
        a.mobility.climb = None;
        a.attached = None;
    }
    (world, fauna, h, p)
}

pub(crate) fn rescale(fauna: &mut Fauna, founder: Founder, k: f64) {
    let mut config = fauna.config.clone();
    config.founders[founder.index()].scale_mass(k);
    fauna
        .replace_config(config)
        .expect("valid demand rescaling");
}

pub(crate) fn get(fauna: &Fauna, id: u64) -> Animal {
    *fauna.view().animal(id).expect("fixture animal")
}

#[test]
fn every_hunters_size_window_ignores_individual_demand_scales() {
    let hunters: Vec<_> = Founder::ALL
        .into_iter()
        .filter(|f| FaunaConfig::default().founder(*f).hunter.is_some())
        .collect();
    assert!(!hunters.is_empty());
    for hf in hunters {
        for pf in Founder::ALL {
            for inside in [true, false] {
                for scaled in [hf, pf] {
                    let (_, mut fauna, hid, pid) = scene(hf, pf);
                    let hb = get(&fauna, hid).body;
                    animal_mut(&mut fauna, pid).body = if inside { 0.75 * hb } else { 1.25 * hb };
                    let verdict =
                        crate::is_prey_of(&fauna.config, &get(&fauna, hid), &get(&fauna, pid));
                    assert_eq!(verdict, inside, "hand-built window for {hf:?}");
                    for k in [8.0, 0.125] {
                        rescale(&mut fauna, scaled, k);
                        assert_eq!(
                            crate::is_prey_of(&fauna.config, &get(&fauna, hid), &get(&fauna, pid)),
                            verdict,
                            "{hf:?}, rescaling {scaled:?} by {k}"
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn grown_lanternjaw_window_factor_ignores_demand_scales() {
    for grown in [false, true] {
        for scaled in [Founder::Chorister, Founder::Lanternjaw] {
            let (_, mut fauna, hid, pid) = scene(Founder::Chorister, Founder::Lanternjaw);
            fauna.config.founders[Founder::Lanternjaw.index()].adult_size_window_factor = 4.0;
            let birth = fauna.config.founder(Founder::Lanternjaw).core.birth_body;
            animal_mut(&mut fauna, pid).body = if grown { 1.25 * birth } else { 0.75 * birth };
            let pb = get(&fauna, pid).body;
            animal_mut(&mut fauna, hid).body = 2.0 * pb;
            let before = crate::predation::window_size(&fauna.config, &get(&fauna, pid));
            close(before, pb * if grown { 4.0 } else { 1.0 });
            let verdict = crate::is_prey_of(&fauna.config, &get(&fauna, hid), &get(&fauna, pid));
            assert_eq!(
                verdict, !grown,
                "the configured adult factor excludes a grown prey"
            );
            rescale(&mut fauna, scaled, 7.0);
            close(
                crate::predation::window_size(&fauna.config, &get(&fauna, pid)),
                before,
            );
            assert_eq!(
                crate::is_prey_of(&fauna.config, &get(&fauna, hid), &get(&fauna, pid)),
                verdict
            );
        }
    }
}

#[test]
fn pack_window_sums_physical_sizes_after_demand_rescaling() {
    let (world, mut fauna, hid, pid) = scene(Founder::Chorister, Founder::Browser);
    let pack = 37;
    for id in [hid, pid] {
        let a = animal_mut(&mut fauna, id);
        a.pack = Some(crate::PackMember {
            pack,
            pup: false,
            calling_ticks: 0,
        });
        a.eating = true;
        a.state = crate::State::Resting;
        place(&mut fauna, id, 2.0, LANE_Z, EAST);
    }
    // A mixed fixture makes each participant's own demand scale observable.
    let i = fauna.animals.binary_search_by_key(&hid, |a| a.id).unwrap();
    let view = world.view();
    let ring = f64::from(view.config.width) * view.config.voxel_m;
    let before = crate::pack::window_body(&fauna, i, ring);
    close(before, get(&fauna, hid).body + get(&fauna, pid).body);
    for f in [Founder::Chorister, Founder::Browser] {
        rescale(&mut fauna, f, 5.0);
        close(crate::pack::window_body(&fauna, i, ring), before);
    }
}

#[test]
fn attack_profitability_uses_physical_sizes_after_demand_rescaling() {
    for scaled in [Founder::Lanternjaw, Founder::Browser] {
        let (_, mut fauna, hid, pid) = scene(Founder::Lanternjaw, Founder::Browser);
        let hb = get(&fauna, hid).body;
        animal_mut(&mut fauna, pid).body = 2.0 * hb;
        fauna.config.founders[Founder::Lanternjaw.index()]
            .hunter
            .as_mut()
            .unwrap()
            .strike_power = 0.25;
        let before = crate::attack_chance(&fauna.config, &get(&fauna, hid), &get(&fauna, pid));
        assert!(
            before > 0.0 && before < 1.0,
            "fixture has an unsaturated strike probability"
        );
        rescale(&mut fauna, scaled, 6.0);
        close(
            crate::attack_chance(&fauna.config, &get(&fauna, hid), &get(&fauna, pid)),
            before,
        );
    }
}

#[test]
fn catch_profit_budget_keeps_strike_count_when_hunter_demand_scales() {
    let (_, mut fauna, hid, pid) = scene(Founder::Seedporter, Founder::Blind);
    let hb = get(&fauna, hid).body;
    animal_mut(&mut fauna, pid).body = 2.0 * hb;
    animal_mut(&mut fauna, pid).reserve = 10.0 - 2.0 * hb;
    fauna.config.founders[Founder::Blind.index()].armour = 0.0;
    let p = &mut fauna.config.founders[Founder::Seedporter.index()];
    p.yield_by_food[crate::Food::Carrion.index()] = 1.0;
    let h = p.hunter.as_mut().unwrap();
    h.strike_power = 0.25;
    h.strike_cost_s = 1.0 / (p.core.maintenance_per_s * hb);
    assert!(
        crate::predation::catch_worthwhile(&fauna.config, &get(&fauna, hid), &get(&fauna, pid)),
        "ten units of food pay for eight strikes costing one unit each"
    );
    rescale(&mut fauna, Founder::Seedporter, 4.0);
    assert!(
        !crate::predation::catch_worthwhile(&fauna.config, &get(&fauna, hid), &get(&fauna, pid)),
        "the same eight strikes now cost four units each"
    );
}

#[test]
fn every_founders_physical_dimensions_ignore_demand_rescaling() {
    for f in Founder::ALL {
        let (_, mut fauna, id, _) = scene(f, Founder::Browser);
        let body_max = fauna.config.founder(f).core.body_max;
        animal_mut(&mut fauna, id).body = 0.4 * body_max;
        let dimensions = |fauna: &Fauna| {
            let b = crate::body_of(&fauna.config, &get(fauna, id)).unwrap();
            [
                b.length_m,
                b.width_m,
                b.height_m,
                b.eye_m,
                b.mouth_floor_m,
                b.mouth_ceiling_m,
                b.contact_m,
                b.mouth_reach_m,
                b.footprint_radius(),
            ]
        };
        let before = dimensions(&fauna);
        rescale(&mut fauna, f, 9.0);
        for (after, before) in dimensions(&fauna).into_iter().zip(before) {
            close(after, before);
        }
    }
}

#[test]
fn parasite_host_bearing_remains_a_daily_energy_budget() {
    let mut config = FaunaConfig::default();
    config.parasite_take_body_per_day = 0.5;
    config.parasite_host_bear_per_day = 0.25;
    let (rider, host) = (2.0, 6.0);
    assert!(crate::host_bears(&config, rider, host));
    assert!(!crate::host_bears(&config, 2.0 * rider, host));
    assert!(crate::host_bears(&config, 2.0 * rider, 2.0 * host));
}
