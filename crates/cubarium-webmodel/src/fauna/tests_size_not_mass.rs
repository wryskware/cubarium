//! The cohort laws compare physical sizes, while budgets keep organic units.

use super::laws;
use cubarium_voxel_fauna::{FaunaConfig, Founder};

fn close(got: f64, want: f64) {
    assert!(
        (got - want).abs() <= 1e-11 * want.abs().max(1.0),
        "{got} != {want}"
    );
}

#[test]
fn every_hunters_cohort_window_ignores_individual_demand_scales() {
    for f in Founder::ALL {
        let config = FaunaConfig::default();
        let mut hunter = *config.founder(f);
        if hunter.hunter.is_none() {
            continue;
        }
        hunter.catch_bellwings = true;
        hunter.hunter.as_mut().unwrap().prey_max_ratio = 1.0;
        let mut prey = *config.founder(Founder::Lanternjaw);
        hunter.scale_mass(1.0 / hunter.mass_scale);
        prey.scale_mass(1.0 / prey.mass_scale);
        prey.adult_size_window_factor = 3.0;
        for grown in [false, true] {
            let pb = prey.core.birth_body * if grown { 1.25 } else { 0.75 };
            for inside in [false, true] {
                let hb = pb * if grown { 3.0 } else { 1.0 } * if inside { 1.25 } else { 0.75 };
                assert_eq!(
                    laws::in_size_window(&hunter, hb, &prey, pb),
                    inside,
                    "{f:?} fixture"
                );
                for k in [0.2, 5.0] {
                    let mut scaled_hunter = hunter;
                    scaled_hunter.scale_mass(k);
                    assert_eq!(
                        laws::in_size_window(&scaled_hunter, k * hb, &prey, pb),
                        inside,
                        "{f:?} hunter scale"
                    );
                    let mut scaled_prey = prey;
                    scaled_prey.scale_mass(k);
                    assert_eq!(
                        laws::in_size_window(&hunter, hb, &scaled_prey, k * pb),
                        inside,
                        "{f:?} prey scale"
                    );
                }
            }
        }
    }
}

#[test]
fn cohort_strike_share_ignores_both_lineages_demand_scales() {
    let config = FaunaConfig::default();
    let mut hunter = *config.founder(Founder::Lanternjaw);
    let mut prey = *config.founder(Founder::Browser);
    hunter.hunter.as_mut().unwrap().strike_power = 0.25;
    prey.armour = 0.25;
    let hb = hunter.core.body_max;
    let pb = 2.0 * hb;
    let before = laws::strike_share(&hunter, hb, &prey, pb);
    assert!(before > 0.0 && before < 1.0);
    for k in [0.2, 5.0] {
        let mut h = hunter;
        h.scale_mass(k);
        close(laws::strike_share(&h, k * hb, &prey, pb), before);
        let mut p = prey;
        p.scale_mass(k);
        close(laws::strike_share(&hunter, hb, &p, k * pb), before);
    }
}
