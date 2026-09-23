//! **The grazing refuge** (`design/handoffs/voxel-plant-viability-2026-09-23.md` §G.3),
//! written before the rule.
//!
//! A stand keeps `graze_refuge ×` the foliage its wood carries — per layer, that layer's
//! share of it — and no bite goes below it. Every consumer reads edible foliage through
//! one flora function, [`cubarium_voxel_flora::StandLayer::edible`], which reports only
//! what stands above the floor. A handful of withdrawals, no ticks.

use cubarium_voxel::{Command as WorldCommand, Config as VoxelConfig, Material, World};
use cubarium_voxel_flora::{Command, Flora, FloraConfig, Profile, Site, Species, SpeciesConfig};

/// A one-row soil plain at 0.25 m, support face at `support`, open sky.
fn plain(width: u32, support: u32) -> World {
    let mut w = World::empty(VoxelConfig {
        width,
        height: 10,
        depth: 1,
        voxel_m: 0.25,
        seed: 29,
        ..VoxelConfig::default()
    });
    for x in 0..width as i64 {
        for y in 1..=support {
            w.apply(WorldCommand::SetMaterial {
                x,
                y,
                z: 0,
                material: Material::Soil,
            });
        }
    }
    w
}

fn at(x: u32, support: u32) -> Site {
    Site {
        x,
        y: support,
        z: 0,
    }
}

fn assert_residuals(flora: &Flora, when: &str) {
    let v = flora.view();
    for (name, got) in [
        ("organic", v.organic() - v.ledger.expected_organic()),
        ("mineral", v.mineral() - v.ledger.expected_mineral()),
        ("energy", v.energy() - v.ledger.expected_energy()),
    ] {
        assert!(
            got.abs() <= 1e-12 * v.organic().abs().max(1.0),
            "{when}: flora {name} residual {got:e}"
        );
    }
}

/// Test 4: `withdraw_foliage` never goes below the floor, and the reader reports zero
/// edible there. A full springturf is stripped as hard as a mouth can ask; what is left
/// is exactly `graze_refuge · alpha · wood`, every layer reads zero edible, and the next
/// bite gets nothing at all.
#[test]
fn a_bite_never_takes_a_stand_below_its_refuge_and_the_floor_reads_zero_edible() {
    let support = 2;
    let world = plain(6, support);
    let mut flora = Flora::new(FloraConfig::default());
    let sc = flora.config().species(Species::Springturf).clone();
    assert!(
        sc.graze_refuge > 0.0 && sc.graze_refuge < 1.0,
        "a refuge is a fraction"
    );
    assert!(flora.apply(
        &world,
        Command::Seed {
            x: 2,
            z: 0,
            species: Species::Springturf,
            wood: sc.wood_max,
        }
    ));
    let site = at(2, support);
    let stand = *flora.view().stand_at(site).expect("seeded");
    let floor = sc.graze_refuge * sc.alpha * stand.wood;

    // The reader: what stands above the floor, layer by layer, and nothing else.
    let edible: f64 = flora.view().layers(&stand).map(|l| l.edible()).sum();
    assert!(
        (edible - (stand.foliage - floor)).abs() < 1e-12,
        "a full stand offers its foliage less the floor: {edible} vs {}",
        stand.foliage - floor
    );

    let taken = flora.take_foliage(site, 1e9).expect("a full stand feeds");
    assert!(
        (taken.organic - (stand.foliage - floor)).abs() < 1e-12,
        "the bite took everything above the floor and nothing below: {}",
        taken.organic
    );
    let left = *flora.view().stand_at(site).expect("still standing");
    assert!(
        (left.foliage - floor).abs() < 1e-12,
        "the stand is at its floor: {} vs {floor}",
        left.foliage
    );
    for l in flora.view().layers(&left) {
        assert_eq!(l.edible(), 0.0, "a layer at its floor reads zero edible");
    }
    assert!(
        flora.take_foliage(site, 1e9).is_none(),
        "a stand at its floor gives nothing"
    );
    let every = i64::MIN..=i64::MAX;
    assert!(
        flora.take_foliage_in_layers(site, 1e9, &every).is_none(),
        "and nothing through the layered bite either"
    );
    assert_residuals(&flora, "after stripping to the floor");
}

/// The floor is per layer: a bite from below stops at the rosette's own share of the
/// refuge and leaves the crown's untouched, so the floors sum to the stand's.
#[test]
fn a_bite_from_below_stops_at_that_layers_own_floor() {
    let support = 2;
    let world = plain(8, support);
    let mut config = FloraConfig::default();
    config.species_mut(Species::Bloomcrown).profile = vec![Profile {
        wood_fraction_max: f64::INFINITY,
        height_m_max: None,
        layers: vec![
            SpeciesConfig::foliage_layer([0.0, 0.34], 1.0, 0.25, 0.0),
            SpeciesConfig::foliage_layer([0.34, 1.0], 1.0, 0.75, 0.0),
        ],
    }];
    let sc = config.species(Species::Bloomcrown).clone();
    let mut flora = Flora::new(config);
    let wood = 1.0 / sc.alpha;
    assert!(flora.apply(
        &world,
        Command::Seed {
            x: 4,
            z: 0,
            species: Species::Bloomcrown,
            wood,
        }
    ));
    let site = at(4, support);
    let cells: Vec<i64> = flora.view().layers_at(site).map(|l| l.cell).collect();
    assert!(cells[1] > cells[0], "two distinct discs: {cells:?}");

    let taken = flora
        .take_foliage_in_layers(site, 1.0, &(cells[0]..=cells[0]))
        .expect("the rosette");
    let rosette_floor = sc.graze_refuge * 0.25;
    assert!(
        (taken.taken.organic - (0.25 - rosette_floor)).abs() < 1e-12,
        "the rosette gave all but its own floor: {taken:?}"
    );
    let after: Vec<f64> = flora.view().layers_at(site).map(|l| l.stock).collect();
    assert!(
        (after[0] - rosette_floor).abs() < 1e-12 && (after[1] - 0.75).abs() < 1e-12,
        "{after:?}"
    );
    let floors: f64 = flora
        .view()
        .layers_at(site)
        .map(|l| l.stock - l.edible())
        .sum();
    assert!(
        (floors - sc.graze_refuge * sc.alpha * wood).abs() < 1e-12,
        "the layers' floors sum to the stand's: {floors}"
    );
    assert_residuals(&flora, "after a bite from below");
}
