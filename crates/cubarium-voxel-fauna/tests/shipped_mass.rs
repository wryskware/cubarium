//! The shipped org-unit mass (Wrysk, 2026-09-29): both lineages are built
//! `SHIPPED_MASS_SCALE` heavier against the plants than the placeholder tables — the
//! absolute quantities scale, and nothing per unit. The scale itself is a tuned number and
//! is not pinned here. No ticks.

use cubarium_voxel_fauna::{FaunaConfig, Founder, FounderPhysiology, SHIPPED_MASS_SCALE, SpeciesConfig};

#[test]
fn the_shipped_config_is_the_placeholder_scaled_by_the_shipped_mass() {
    let k = SHIPPED_MASS_SCALE;
    let close = |a: f64, b: f64| (a - b).abs() <= 1e-12 * b.abs().max(1e-12);
    let c = FaunaConfig::default();
    c.validate().expect("the shipped config validates");
    for f in Founder::ALL {
        let p = c.founder(f);
        let x1 = FounderPhysiology::placeholder(f);
        assert_eq!(p.mass_scale, k, "{f:?}");
        assert_eq!(f.manifest().mass_scale, k, "{f:?}: the contract carries it");
        assert_eq!(x1.mass_scale, 1.0, "{f:?}: the placeholder is unscaled");
        // Absolute quantities scale.
        assert!(close(p.core.body_max, k * x1.core.body_max), "{f:?}: body_max");
        assert!(
            close(p.core.reproduction.egg_organic, k * x1.core.reproduction.egg_organic),
            "{f:?}: egg_organic"
        );
        // Per-unit rates stay.
        assert_eq!(p.core.maintenance_per_s, x1.core.maintenance_per_s, "{f:?}: per-unit rates stay");
    }
    let x1 = SpeciesConfig::frondgrazer();
    assert!(close(c.frondgrazer.body_max, k * x1.body_max), "the placeholder species too");
}
