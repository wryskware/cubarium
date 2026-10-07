//! **TEST-PASS-4 (independent), GLOWCAP through a capgnawer's bite with its switch off**:
//! with `mycelium_graze_fraction = 0` a bite finishes the regrown caps and stops there (the
//! old caps-only bite), and a glowcap cropped to its refuge feeds nothing; the same scene
//! with a fraction grazes on into the mycelium (the control). Both layers' books close.
//!
//! A child of `step` beside the predation files, sharing their fixture. Every number a case
//! asserts is read off the config it built.

use super::predation_fixture::*;
use crate::{Fauna, FaunaConfig, Food, Founder, StartingStores};
use cubarium_voxel_flora::{Command as FloraCommand, FaceLayer, Flora, FloraConfig, Site, Species};

const GNAWER: Founder = Founder::Capgnawer;

/// A hungry capgnawer feeding in place over a glowcap cropped to its refuge, whose caps
/// regrow `regrow` × a full bite in one plant step (0: none). Returns the layers, the
/// gnawer, the fungus site and a full bite.
fn scene(fraction: f64, regrow: f64) -> (cubarium_voxel::World, Flora, Fauna, Site, f64) {
    let mut world = clock_off(701);
    let mut c = FaunaConfig::default();
    let p = &mut c.founders[GNAWER.index()];
    p.diet = crate::Diet::only(Food::CapTissue);
    p.gut.s0 = 0.0;
    p.gut.stomach_k = 1.0;
    p.gut.bite_per_body_s = 1.0;
    p.gut.mass_exponent = 1.0;
    p.core.bite_half_stock = 0.0;
    p.bite_spill_fraction = 0.0;
    let full_bite = p.gut.bite_per_body_s * p.core.body_max * GNAWER.manifest().controller_period_s;
    let mut fc = FloraConfig::default();
    fc.step_every = 1;
    fc.glowcap.mycelium_graze_fraction = fraction;
    fc.glowcap.defence_per_bite = 0.0;
    fc.glowcap.maintenance = 0.0;
    fc.glowcap.wood_rate = 0.0;
    fc.glowcap.senescence = 0.0;
    fc.glowcap.propagule_rate = 0.0;
    fc.glowcap.dark_uptake_per_s = 0.0;
    let wood = fc.glowcap.wood_max;
    let reserve = fc.glowcap.reserve_cap * wood;
    let regrown = (regrow * full_bite).min(0.25 * reserve / (1.0 + fc.glowcap.build));
    fc.glowcap.foliage_rate = regrown / (wood * cubarium_voxel_flora::DT);
    let mut flora = Flora::new(fc);
    let mut fauna = Fauna::new(c);
    let id = introduce(&mut fauna, &world, GNAWER, StartingStores { body: 1.0, reserve: 0.0 });
    drive(&mut fauna, id, 0.0, true);
    let at = animal(&fauna, id).site;
    assert!(flora.apply(&world, FloraCommand::Seed { x: i64::from(at.x), z: at.z, species: Species::Glowcap, wood }));
    let fungus = Site { x: at.x, y: SURFACE_Y, z: at.z };
    flora.take_foliage_of(fungus, FaceLayer::Fungus, f64::MAX, &(i64::MIN..=i64::MAX)).expect("crop to the refuge");
    flora.step(&mut world);
    (world, flora, fauna, fungus, full_bite)
}

fn bite(world: &cubarium_voxel::World, flora: &mut Flora, fauna: &mut Fauna) -> bool {
    for _ in 0..20 {
        fauna.step_with(world, flora, 1);
        crate::testpass3_support::ledgers(fauna, flora);
        if fauna.ledger.bites_by_food[Food::CapTissue.index()] > 0 {
            return true;
        }
    }
    false
}

#[test]
fn with_no_mycelium_offered_a_bite_stops_at_the_regrown_caps() {
    for fraction in [0.5, 0.0] {
        let (world, mut flora, mut fauna, fungus, full_bite) = scene(fraction, 0.25);
        let before = *flora.view().fungus_at(fungus).unwrap();
        let caps = flora.view().edible_foliage(&before);
        assert!(caps > 0.0 && caps < full_bite, "fixture: regrown caps {caps} leave room in a bite {full_bite}");
        assert!(bite(&world, &mut flora, &mut fauna), "fraction {fraction}: it bites the caps");
        let after = *flora.view().fungus_at(fungus).unwrap();
        let (cap_loss, wood_loss) = (before.foliage - after.foliage, before.wood - after.wood);
        assert!((cap_loss - caps).abs() < 1e-12, "fraction {fraction}: the caps go first");
        let eaten = fauna.ledger.eaten_mycelium_by_founder[GNAWER.index()];
        if fraction > 0.0 {
            assert!(wood_loss > 0.0 && (eaten - wood_loss).abs() < 1e-12, "the control grazes on into mycelium");
        } else {
            assert_eq!((wood_loss, eaten), (0.0, 0.0), "no offer: the bite ends at the caps");
            assert!((fauna.ledger.eaten_by_food[Food::CapTissue.index()] - caps).abs() < 1e-12);
        }
    }
}

#[test]
fn with_no_mycelium_offered_a_cropped_glowcap_feeds_nothing() {
    let (world, mut flora, mut fauna, fungus, _) = scene(0.0, 0.0);
    let s = *flora.view().fungus_at(fungus).unwrap();
    assert!(flora.view().edible_foliage(&s) <= 1e-12, "fixture: no caps regrew");
    assert!(!bite(&world, &mut flora, &mut fauna), "nothing edible at its mouth");
    assert_eq!(flora.view().fungus_at(fungus).unwrap().wood, s.wood);
    assert_eq!(fauna.ledger.eaten_mycelium_by_founder[GNAWER.index()], 0.0);
}
