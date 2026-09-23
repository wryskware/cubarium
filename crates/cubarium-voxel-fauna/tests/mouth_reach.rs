//! The browser's **vertical mouth reach**, now a physical band
//! (`design/handoffs/voxel-body-anchors-2026-09-22.md`, decisions §2): the mouth accepts
//! the crown layers the interval `[0, 1.33 × body height]` over the standing surface
//! overlaps. The whole-voxel `mouth_reach_up_voxels` rule these tests were first written
//! for (`design/handoffs/voxel-browser-reach-2026-09-21.md`) is gone; the field survives
//! on the `Manifest` as the trained contract's record and is not read.
//!
//! The arithmetic on this fixture: the browser here is 0.8 of `body_max`, so its height
//! is `0.1875 · 0.8^(1/3) = 0.174` m and its ceiling `1.33 ×` that, 0.2315 m — inside
//! one 0.25 m cell. On a 0.25 m world the mouth therefore takes its own head layer and
//! nothing above it, and a crown one voxel up is 0.25 m of air away from a 0.23 m
//! mouth. That is the succession story decisions §2 asks for and the basal rosette of
//! the layers package is the answer to it.
//!
//! Short function tests on a flat 0.25 m world whose ground support face is `y = 2`, so
//! a body stands in layer 3 (`head`), `head + 1` is layer 4 and `head + 2` is layer 5.
//! Crown layers are made with **bloomcrown** wood, whose `crown_height_voxels` spans
//! `[1, 3]`: wood 0.12 rounds to one voxel, 0.30 to two and 0.60 to three. Nothing here
//! pins a trajectory; the amounts asserted are the frozen bite rate and the plant
//! layer's own stock.

use cubarium_voxel::{Command as WorldCommand, Config as VoxelConfig, Material, World};
use cubarium_voxel_flora::{
    Command as FloraCommand, Deposit, DepositKind, Flora, FloraConfig, Site, Species as Plant,
};

use cubarium_voxel_fauna::{
    Actions, Command as FaunaCommand, Fauna, FaunaConfig, Founder, Scripted, StartingStores,
};

/// The blind founder's manifest digest as recorded **before** the vertical-reach change
/// (`cargo test -p cubarium-voxel-fauna`, 2026-09-21, at ffac8a1). The blind lineage's
/// behaviour is identical under the new field, so its digest must not move and its
/// trained centres must keep loading. The browser's digest is deliberately not pinned:
/// it changes, and every browser centre trained before this build is refused.
const BLIND_DIGEST_BEFORE_THE_VERTICAL_REACH: u64 = 6_080_287_729_887_670_217;

// Since the layers package these fixtures use `one_layer_species()`: every plant is
// the one-disc lollipop the model was when these claims were written, because the
// subject here is the **mouth's physical band** and not the anatomy under it. An adult
// bloomcrown's basal rosette — the anatomy that now answers the reach these tests
// measure — is `tests/plant_layers.rs`'s subject
// (`design/handoffs/voxel-plant-layers-2026-09-22.md`).

/// A flat world: 8 × 6 × 6 voxels at 0.25 m, soil 1..=2, ground support face at y = 2.
fn flat_world() -> World {
    let mut world = World::empty(VoxelConfig {
        width: 8,
        height: 6,
        depth: 6,
        voxel_m: 0.25,
        ..VoxelConfig::default()
    });
    for z in 0..6 {
        for x in 0..8 {
            for y in 1..=2 {
                world.apply(WorldCommand::SetMaterial {
                    x,
                    y,
                    z,
                    material: Material::Soil,
                });
            }
        }
    }
    world
}

fn site(x: u32, z: u32) -> Site {
    Site { x, y: 2, z }
}

/// Seed a bloomcrown of `wood` at one column and return the crown layer it stands at,
/// asserting it is the `rise` the caller asked for above the body's head layer.
fn crown_at(flora: &mut Flora, world: &World, x: i64, z: u32, wood: f64, rise: i64) -> f64 {
    assert!(flora.apply(
        world,
        FloraCommand::Seed {
            x,
            z,
            species: Plant::Bloomcrown,
            wood,
        },
    ));
    let s = site(x as u32, z);
    let stand = flora.view().stand_at(s).expect("the seeded bloomcrown");
    let layer = i64::from(stand.site.y)
        + i64::from(
            flora
                .config()
                .species(Plant::Bloomcrown)
                .crown_voxels(stand.wood, flora.config().voxel_m),
        );
    // The body stands in `site.y + 1`; `rise` is how far above that the crown sits.
    assert_eq!(
        layer,
        i64::from(s.y) + 1 + rise,
        "wood {wood} did not put the crown {rise} above the head layer"
    );
    stand.foliage
}

/// Place a browser founder at one column, facing `+z`, and give it a full feed action.
fn browser_feeding(fauna: &mut Fauna, world: &World, x: i64, z: u32) -> u64 {
    assert!(fauna.apply(
        world,
        FaunaCommand::IntroduceFounder {
            x,
            z,
            founder: Founder::Browser,
            stores: StartingStores {
                body: 0.8,
                reserve: 1.0,
            },
            heading_rad: 0.0,
        },
    ));
    let id = fauna.view().ledger.births - 1;
    assert!(fauna.set_controller(
        id,
        Box::new(Scripted::new(vec![Actions {
            forward: 0.0,
            turn: 0.0,
            feed: 1.0,
        }])),
    ));
    id
}

/// One controller period of the frondgrazer's frozen bite rate.
const ONE_BITE: f64 = 0.002 * 0.25;

/// (a) A browser bites a crown at its **own head layer** — the whole of the band on a
/// 0.25 m world. The stock moves by exactly one bite of the frozen rate, and the mouth
/// diagnostic names the same stand the bite took from.
#[test]
fn a_browser_bites_a_crown_at_its_head_layer() {
    let world = flat_world();
    let mut flora = Flora::new(FloraConfig::default().one_layer_species());
    let before = crown_at(&mut flora, &world, 2, 2, 0.12, 0);

    let mut fauna = Fauna::new(FaunaConfig::default());
    let id = browser_feeding(&mut fauna, &world, 2, 2);
    let reached = {
        let av = fauna.view();
        cubarium_voxel_fauna::browser_mouth_foliage(
            &world.view(),
            &flora.view(),
            av.config,
            av.animal(id).expect("the placed browser"),
        )
    };
    assert_eq!(
        reached.map(|(s, _)| s),
        Some(site(2, 2)),
        "the mouth must reach the crown at its own layer"
    );

    for _ in 0..5 {
        fauna.step(&world, &mut flora);
    }
    let after = flora.view().stand_at(site(2, 2)).expect("alive").foliage;
    assert_eq!(fauna.view().ledger.bites, 1, "one attempt, one bite");
    assert!(
        ((before - after) - ONE_BITE).abs() < 1e-12,
        "foliage at the head moved by {}",
        before - after
    );
}

/// (a2) And the same browser no longer reaches a crown one 0.25 m voxel above its head:
/// its physical ceiling is 0.2315 m. This is the rule that changed on 2026-09-22 — the
/// manifest still records the old `mouth_reach_up_voxels` of one, and nothing reads it.
#[test]
fn a_crown_one_voxel_above_the_head_is_out_of_the_physical_band() {
    let world = flat_world();
    let mut flora = Flora::new(FloraConfig::default().one_layer_species());
    let before = crown_at(&mut flora, &world, 2, 2, 0.30, 1);

    let mut fauna = Fauna::new(FaunaConfig::default());
    let id = browser_feeding(&mut fauna, &world, 2, 2);
    let reached = {
        let av = fauna.view();
        cubarium_voxel_fauna::browser_mouth_foliage(
            &world.view(),
            &flora.view(),
            av.config,
            av.animal(id).expect("the placed browser"),
        )
    };
    assert_eq!(reached, None, "0.25 m of air is over a 0.2315 m mouth");

    for _ in 0..5 {
        fauna.step(&world, &mut flora);
    }
    let after = flora.view().stand_at(site(2, 2)).expect("alive").foliage;
    assert_eq!(fauna.view().ledger.bites, 0, "no contact, no bite");
    assert_eq!(before, after, "an unreachable crown is not cropped");
    assert_eq!(
        Founder::Browser.manifest().mouth_reach_up_voxels,
        1,
        "the recorded contract keeps the field the shipped centres were trained with"
    );
}

/// (b) The same browser still cannot bite a crown at `y + 3`: two voxels above its head
/// is past the declared reach, the mouth reads air, and the stock does not move.
#[test]
fn a_browser_cannot_bite_a_crown_two_voxels_above_its_head() {
    let world = flat_world();
    let mut flora = Flora::new(FloraConfig::default().one_layer_species());
    let before = crown_at(&mut flora, &world, 2, 2, 0.60, 2);

    let mut fauna = Fauna::new(FaunaConfig::default());
    let id = browser_feeding(&mut fauna, &world, 2, 2);
    let reached = {
        let av = fauna.view();
        cubarium_voxel_fauna::browser_mouth_foliage(
            &world.view(),
            &flora.view(),
            av.config,
            av.animal(id).expect("the placed browser"),
        )
    };
    assert_eq!(
        reached, None,
        "a crown two voxels above the head is out of the mouth's reach"
    );

    for _ in 0..5 {
        fauna.step(&world, &mut flora);
    }
    let after = flora.view().stand_at(site(2, 2)).expect("alive").foliage;
    assert_eq!(fauna.view().ledger.bites, 0, "no contact, no bite");
    assert_eq!(before, after, "an unreachable crown is not cropped");
}

/// (c) The blind founder is untouched: its mouth still roots at the ground, litter under
/// it is still one bite of its own frozen rate, and its manifest digest is exactly the
/// value recorded before the vertical reach existed — so its trained centres still load.
#[test]
fn the_blind_founders_litter_mouth_and_digest_are_unchanged() {
    let world = flat_world();
    let mut flora = Flora::new(FloraConfig::default().one_layer_species());
    assert!(flora.deposit(
        site(2, 2),
        Deposit {
            kind: DepositKind::Litter,
            organic: 0.05,
            mineral: 0.001,
            energy: 0.1,
        },
    ));
    let before = flora.view().ground_at(site(2, 2)).expect("ground").litter;

    let mut fauna = Fauna::new(FaunaConfig::default());
    assert!(fauna.apply(
        &world,
        FaunaCommand::IntroduceFounder {
            x: 2,
            z: 2,
            founder: Founder::Blind,
            stores: StartingStores {
                body: 0.8,
                reserve: 1.0,
            },
            heading_rad: 0.0,
        },
    ));
    let id = fauna.view().ledger.births - 1;
    assert!(fauna.set_controller(
        id,
        Box::new(Scripted::new(vec![Actions {
            forward: 0.0,
            turn: 0.0,
            feed: 1.0,
        }])),
    ));
    for _ in 0..5 {
        fauna.step(&world, &mut flora);
    }
    let after = flora.view().ground_at(site(2, 2)).expect("ground").litter;
    assert_eq!(fauna.view().ledger.bites, 1);
    assert!(
        ((before - after) - 0.0005 * 0.25).abs() < 1e-12,
        "the blind bite moved {}",
        before - after
    );
    assert!(fauna.view().animal(id).is_some(), "the blind founder lives");

    let blind = Founder::Blind.manifest();
    assert_eq!(blind.mouth_reach_up_voxels, 0, "the blind mouth stays flat");
    assert_eq!(
        blind.digest(),
        BLIND_DIGEST_BEFORE_THE_VERTICAL_REACH,
        "the blind manifest digest moved: its centres would be refused for nothing"
    );
}

/// (d) The read-only D3 diagnostic and the stepping rule are the same rule. On one
/// fixture holding crowns at `head`, `head + 1` and `head + 2`, a browser placed on each
/// column reports a mouth stand exactly when a real feed tick takes foliage from it.
#[test]
fn the_mouth_diagnostic_agrees_with_the_stepping_rule_at_three_crown_heights() {
    let world = flat_world();
    // Three columns two apart, so no crown disc reaches its neighbour's mouth probes.
    let cases = [(1i64, 0.12, 0i64), (3, 0.30, 1), (5, 0.60, 2)];
    let mut flora = Flora::new(FloraConfig::default().one_layer_species());
    let mut before = Vec::new();
    for (x, wood, rise) in cases {
        before.push(crown_at(&mut flora, &world, x, 2, wood, rise));
    }

    for (i, (x, _, rise)) in cases.into_iter().enumerate() {
        let mut flora = flora.clone();
        let mut fauna = Fauna::new(FaunaConfig::default());
        let id = browser_feeding(&mut fauna, &world, x, 2);
        let (diagnosed, candidates) = {
            let av = fauna.view();
            let animal = av.animal(id).expect("the placed browser");
            (
                cubarium_voxel_fauna::browser_mouth_foliage(
                    &world.view(),
                    &flora.view(),
                    av.config,
                    animal,
                )
                .map(|(s, _)| s),
                cubarium_voxel_fauna::browser_mouth_candidates(
                    &world.view(),
                    &flora.view(),
                    av.config,
                    animal,
                )
                .expect("a browser has mouth candidates"),
            )
        };

        for _ in 0..5 {
            fauna.step(&world, &mut flora);
        }
        let after = flora
            .view()
            .stand_at(site(x as u32, 2))
            .expect("alive")
            .foliage;
        let bit = fauna.view().ledger.bites == 1;
        assert_eq!(
            diagnosed.is_some(),
            bit,
            "crown at head + {rise}: the diagnostic said {diagnosed:?} and the tick bit {bit}"
        );
        assert_eq!(
            candidates.iter().any(|(s, _)| *s == site(x as u32, 2)),
            bit,
            "crown at head + {rise}: the candidate list disagrees with the tick"
        );
        // The band, not a voxel count: this body's ceiling is under one 0.25 m cell,
        // so only the head layer is in reach.
        assert_eq!(
            bit,
            rise == 0,
            "crown at head + {rise} against a 0.2315 m physical mouth band"
        );
        if bit {
            assert!(
                ((before[i] - after) - ONE_BITE).abs() < 1e-12,
                "crown at head + {rise} moved by {}",
                before[i] - after
            );
        } else {
            assert_eq!(before[i], after, "crown at head + {rise} was not cropped");
        }
    }
}
