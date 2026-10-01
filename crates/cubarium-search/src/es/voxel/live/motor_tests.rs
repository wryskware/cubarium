//! Motor cost tied to upkeep (`design/handoffs/voxel-audit-round-2026-09-28.md`, 1; the
//! audit's §1.1, §8.1). No ticks.

use cubarium_voxel_fauna::{FaunaConfig, Founder};

use super::ecology::EcologyParams;

fn motor(c: &FaunaConfig, f: Founder) -> f64 {
    c.founder(f).motor_respiration_per_s
}
fn upkeep(c: &FaunaConfig, f: Founder) -> f64 {
    c.founder(f).core.maintenance_per_s
}

/// An upkeep multiplier moves the walking price with it: walking keeps its shipped
/// ratio to upkeep unless `motor_factor` says otherwise.
#[test]
fn an_upkeep_multiplier_moves_the_walking_price_with_it() {
    let base = FaunaConfig::default();
    let mut e = EcologyParams::defaults();
    e.set("founder.maintenance_per_s", 0.25).expect("in range");
    let c = e.fauna_config();
    for f in Founder::ALL {
        let ratio = motor(&base, f) / upkeep(&base, f);
        assert!(
            (motor(&c, f) / upkeep(&c, f) - ratio).abs() < 1e-12,
            "{f:?}: walking stays {ratio}x upkeep"
        );
        assert!((motor(&c, f) - 0.25 * motor(&base, f)).abs() < 1e-18);
    }
}

/// `motor_factor` sets walking as a multiple of upkeep, per lineage, shared and
/// per-lineage multiplying; the defaults are the shipped prices.
#[test]
fn motor_factor_prices_walking_per_lineage() {
    let base = FaunaConfig::default();
    assert_eq!(EcologyParams::defaults().fauna_config(), base);
    let mut e = EcologyParams::defaults();
    e.set("founder.motor_factor@frondgrazer", 2.0)
        .expect("in range");
    let c = e.fauna_config();
    assert!((motor(&c, Founder::Browser) - 2.0 * motor(&base, Founder::Browser)).abs() < 1e-18);
    assert_eq!(motor(&c, Founder::Blind), motor(&base, Founder::Blind));
    assert_eq!(
        upkeep(&c, Founder::Browser),
        upkeep(&base, Founder::Browser),
        "upkeep untouched"
    );
    e.set("founder.motor_factor", 0.5).expect("in range");
    let c = e.fauna_config();
    assert!((motor(&c, Founder::Browser) - motor(&base, Founder::Browser)).abs() < 1e-18);
    assert!(
        e.set("founder.motor_factor", 0.4).is_err(),
        "log range [0.5, 4]"
    );
    assert!(
        EcologyParams::search_names().contains(&"founder.motor_factor@littershredder".to_string())
    );
}
