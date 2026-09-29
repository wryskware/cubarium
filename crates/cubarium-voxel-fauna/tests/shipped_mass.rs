//! The shipped org-unit mass (Wrysk, 2026-09-29): both lineages are built x30 heavier
//! against the plants than the placeholder tables — the nine absolute quantities, and
//! nothing per unit. No ticks.

use cubarium_voxel_fauna::{FaunaConfig, Founder, FounderPhysiology, SHIPPED_MASS_SCALE, SpeciesConfig};

#[test]
fn both_lineages_ship_thirty_times_heavier() {
    assert_eq!(SHIPPED_MASS_SCALE, 30.0);
    let c = FaunaConfig::default();
    c.validate().expect("the shipped config validates");
    for f in Founder::ALL {
        let p = c.founder(f);
        assert_eq!(p.mass_scale, 30.0, "{f:?}");
        assert_eq!(f.manifest().mass_scale, 30.0, "{f:?}: the contract carries it");
    }
    let b = c.founder(Founder::Browser).core;
    assert!((b.body_max - 30.0 * 0.05).abs() < 1e-12);
    assert!((b.bite_per_s - 30.0 * 0.002).abs() < 1e-12);
    assert_eq!(b.maintenance_per_s, 0.001, "per-unit rates stay");
    let s = c.founder(Founder::Blind).core;
    assert!((s.body_max - 30.0 * 0.0125).abs() < 1e-12);
    assert!((s.reproduction.egg_organic - 30.0 * 0.004).abs() < 1e-12);
    assert!((c.frondgrazer.body_max - 30.0 * 0.05).abs() < 1e-12, "the placeholder species too");
    // The unscaled placeholder tables stay nameable.
    assert_eq!(SpeciesConfig::frondgrazer().body_max, 0.05);
    assert_eq!(FounderPhysiology::placeholder(Founder::Browser).mass_scale, 1.0);
}
