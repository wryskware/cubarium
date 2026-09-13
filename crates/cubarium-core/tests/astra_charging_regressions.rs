//! Independent paid-charging review probes; no lowered reproduction gates.
use cubarium_core::{
    ChargingDiagnostics, FixedHunterProfile, HunterEvent, HunterTarget, Reproduction, World,
    WorldConfig,
};

fn quiet() -> WorldConfig {
    let mut c = WorldConfig::default();
    c.founders.kinds.clear();
    c.founders.count = 0;
    c.weather.amplitude = 0.0;
    c.water.rain_rate = 0.0;
    c.habitat.light_base = 0.0;
    c.habitat.light_height_gain = 0.0;
    c.habitat.light_noise_gain = 0.0;
    c.detritus.initial_dark = 0.0;
    c.detritus.decomposition = 0.0;
    c.detritus.fall = 0.0;
    c
}

#[test]
fn zero_oxidation_rate_does_not_report_a_paid_transaction() {
    let mut c = quiet();
    c.organism.oxidation_rate = 0.0;
    let mut w = World::new(c).unwrap();
    let p = FixedHunterProfile::lanternjaw_trial(w.config()).charge80();
    let id = w
        .start_hunter_trial(
            p,
            HunterTarget {
                face: 4,
                u: 32.0,
                v: 32.0,
            },
        )
        .unwrap()
        .id;
    let before = w.state.organisms.get(id).unwrap().reserve;
    w.step();
    assert_eq!(w.state.organisms.get(id).unwrap().reserve, before);
    assert_eq!(w.charging_diagnostics(), ChargingDiagnostics::default());
}

#[test]
fn real_default_age_and_stock_gates_fund_only_after_paid_charge() {
    for candidate in [false, true] {
        let mut w = World::new(quiet()).unwrap();
        let mut p = FixedHunterProfile::lanternjaw_trial(w.config());
        p.seek_reserve_fraction = 0.8;
        p.perch_reserve_fraction = 0.9;
        if candidate {
            p = p.charge80();
        }
        assert_eq!(p.reproduce_min_age_seconds, 1200.0);
        assert_eq!(p.reproduce_reserve_fraction, 0.8);
        assert_eq!(p.reproduce_energy_fraction, 0.75);
        let id = w
            .start_hunter_trial(
                p,
                HunterTarget {
                    face: 4,
                    u: 32.0,
                    v: 32.0,
                },
            )
            .unwrap()
            .id;
        // Fixture time/stocks, not an altered age gate or an unpaid production transition.
        w.state.tick = 24000;
        let o = w.state.organisms.get_mut(id).unwrap();
        o.reserve = 4.0;
        o.energy = 2.9996;
        w.state.external_material_in += 2.0;
        let mut w = World::from_state(w.state).unwrap();
        w.step();
        let events = w.drain_hunter_events();
        let funded = events.iter().find_map(|e| match e {
            HunterEvent::Reproduction {
                record:
                    Reproduction::Funded {
                        parent_reserve_before,
                        parent_reserve_after,
                        parent_energy_before,
                        parent_energy_after,
                        escrow_structure,
                        escrow_reserve,
                        escrow_energy,
                        build_heat,
                        ..
                    },
                ..
            } => Some((
                *parent_reserve_before,
                *parent_reserve_after,
                *parent_energy_before,
                *parent_energy_after,
                *escrow_structure,
                *escrow_reserve,
                *escrow_energy,
                *build_heat,
            )),
            _ => None,
        });
        assert_eq!(funded.is_some(), candidate);
        if let Some((rb, ra, eb, ea, s, r, e, heat)) = funded {
            assert!(rb >= 3.2 && eb >= 3.0);
            assert_eq!((s, r, e, heat), (0.8, 0.8, 0.6, 0.4));
            assert_eq!(ra, rb - 1.6);
            assert_eq!(ea, eb - 1.0);
            let parent = w.state.organisms.get(id).unwrap();
            assert_eq!((parent.reserve, parent.energy), (ra, ea));
            assert!(w.charging_diagnostics().extra_reserve_burned > 0.0);
        }
    }
}
