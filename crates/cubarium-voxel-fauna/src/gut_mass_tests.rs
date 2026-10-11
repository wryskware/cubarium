//! **GUT-MASS** (Wrysk 2026-10-09, training plan Q10): raising a lineage's `mass_scale`
//! scales body, reserves, gut and upkeep together, so the gut's ceiling over upkeep and the
//! time a body takes to starve do not move. A child of `gut_tests`, on its cadence fixture
//! (each lineage's own gut, upkeep and rhythm on a walker's carrion). Mechanisms only: the
//! same body at x1 and xK, compared with itself; no shipped mass is read.

use super::*;

/// The heavier copy of `config`: lineage `f` built `k` times heavier.
fn heavier(config: &FaunaConfig, f: Founder, k: f64) -> FaunaConfig {
    let mut c = config.clone();
    c.founders[f.index()].scale_mass(k);
    c
}

/// One tick of digestion from a full stomach with an empty reserve (room for all of it),
/// over the body's maintenance for that tick: the gut ceiling in multiples of upkeep.
fn ceiling_over_upkeep(config: FaunaConfig, f: Founder) -> f64 {
    let world = clock_off(3);
    let mut flora = Flora::new(FloraConfig::default());
    let mut fauna = Fauna::new(config);
    fauna.set_births_enabled(false);
    let id = introduce(&mut fauna, &world, f, StartingStores { body: 1.0, reserve: 0.0 });
    fill_stomach(&mut fauna, id, 1.0);
    fauna.controllers.set(id, stand_and_feed());
    let body = animal(&fauna, id).body;
    let before = fauna.ledger.digested.organic;
    fauna.step_with(&world, &mut flora, 1);
    let digested = fauna.ledger.digested.organic - before;
    let upkeep = fauna.config.founder(f).core.maintenance_per_s * body * DT;
    assert!(digested > 0.0 && upkeep > 0.0, "{f:?}: a full stomach digests");
    digested / upkeep
}

/// Every walking lineage (the fixture's walker carrion reads a walker's senses; the flyer's
/// gut is the same rule on its own numbers).
#[test]
fn the_gut_ceiling_over_upkeep_is_unchanged_when_mass_scales() {
    for f in Founder::ALL.into_iter().filter(|&f| FaunaConfig::default().founder(f).flight.is_none()) {
        let base = carrion_config(f);
        let light = ceiling_over_upkeep(base.clone(), f);
        let heavy = ceiling_over_upkeep(heavier(&base, f, 40.0), f);
        assert!((light - heavy).abs() <= 1e-9 * light, "{f:?}: x1 {light} against x40 {heavy}");
    }
}

/// Ticks until a fed body on a bare plain dies of hunger, upkeep sped up (both copies
/// alike) so the case is short.
fn ticks_to_starve(config: FaunaConfig, f: Founder) -> u64 {
    let world = clock_off(3);
    let mut flora = Flora::new(FloraConfig::default());
    let mut fauna = Fauna::new(config);
    fauna.set_births_enabled(false);
    let id = introduce(&mut fauna, &world, f, StartingStores { body: 1.0, reserve: 1.0 });
    fill_stomach(&mut fauna, id, 0.0);
    fauna.controllers.set(id, stand_and_feed());
    for t in 1..=40_000u64 {
        fauna.step_with(&world, &mut flora, 1);
        if !fauna.view().animals.iter().any(|a| a.id == id) {
            return t;
        }
    }
    panic!("{f:?}: still alive after 40,000 ticks");
}

#[test]
fn starvation_time_is_unchanged_when_mass_scales() {
    let f = Founder::Browser;
    let mut base = carrion_config(f);
    let p = &mut base.founders[f.index()];
    p.core.maintenance_per_s *= 1000.0;
    p.motor_respiration_per_s *= 1000.0;
    let light = ticks_to_starve(base.clone(), f);
    let heavy = ticks_to_starve(heavier(&base, f, 40.0), f);
    assert!(light > 20, "the reserve carries it a while: {light} ticks");
    assert!(light.abs_diff(heavy) <= 1, "x1 starves in {light} ticks, x40 in {heavy}");
}
