//! The browser's **vertical mouth reach**, now a physical band
//! (`design/handoffs/voxel-body-anchors-2026-09-22.md`, decisions §2): the mouth accepts
//! the crown layers the interval `[0, 1.33 × body height]` over the standing surface
//! overlaps. The whole-voxel `mouth_reach_up_voxels` rule these tests were first written
//! for (`design/handoffs/voxel-browser-reach-2026-09-21.md`) is gone; the field survives
//! on the `Manifest` as the trained contract's record and is not read.
//!
//! The arithmetic on this fixture: the browser here is 0.8 of `body_max`, so on package
//! L's ladder its height is `0.375 · 0.8^(1/3) = 0.348` m and its ceiling `1.33 ×` that,
//! 0.463 m — into the second 0.25 m cell. On a 0.25 m world the mouth therefore takes
//! its own head layer and the one above it, and a crown two voxels up is 0.5 m of air
//! away from a 0.46 m mouth. That is the succession story decisions §2 asks for and the
//! basal rosette of the layers package is the answer to it.
//!
//! Short function tests on a flat 0.25 m world whose ground support face is `y = 2`, so
//! a body stands in layer 3 (`head`), `head + 1` is layer 4 and `head + 2` is layer 5.
//! Crown layers are made with a half-grown **springturf** (one voxel) and **bloomcrown**
//! wood, whose one-layer crown spans `[0.375, 1.0]` m: wood 0.12 rounds to two 0.25 m
//! voxels and 0.30 to three. Nothing here pins a trajectory; the amounts asserted are the
//! frozen bite rate and the plant layer's own stock.

use cubarium_voxel::{Command as WorldCommand, Config as VoxelConfig, Material, World};
use cubarium_voxel_flora::{
    Command as FloraCommand, Deposit, DepositKind, Flora, FloraConfig, Site, Species as Plant,
};

use cubarium_voxel_fauna::{
    Actions, Command as FaunaCommand, Fauna, FaunaConfig, Founder, Scripted, StartingStores,
};

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

/// The plant and wood whose one-layer crown stands `rise` voxels above the head layer
/// on this 0.25 m world: a half-grown springturf at the head, then bloomcrown.
fn crown_for(rise: i64) -> (Plant, f64) {
    match rise {
        0 => (
            Plant::Springturf,
            0.5 * FloraConfig::default().springturf.wood_max,
        ),
        1 => (Plant::Bloomcrown, 0.12),
        _ => (Plant::Bloomcrown, 0.30),
    }
}

/// Seed the crown of [`crown_for`]`(rise)` at one column and return its foliage,
/// asserting its crown layer is the `rise` the caller asked for above the body's head
/// layer.
fn crown_at(flora: &mut Flora, world: &World, x: i64, z: u32, rise: i64) -> f64 {
    let (species, wood) = crown_for(rise);
    assert!(flora.apply(
        world,
        FloraCommand::Seed {
            x,
            z,
            species,
            wood,
        },
    ));
    let s = site(x as u32, z);
    let stand = flora.view().stand_at(s).expect("the seeded crown");
    let layer = i64::from(stand.site.y)
        + i64::from(
            flora
                .config()
                .species(species)
                .crown_voxels(stand.wood, flora.config().voxel_m),
        );
    // The body stands in `site.y + 1`; `rise` is how far above that the crown sits.
    assert_eq!(
        layer,
        i64::from(s.y) + 1 + rise,
        "{species:?} wood {wood} did not put the crown {rise} above the head layer"
    );
    stand.foliage
}

/// The shipped founders with **no diminishing bite** (`bite_half_stock` 0): these cases
/// measure one whole bite of the frozen rate, which package G's `E / (E + K)` would
/// shrink by whatever stock is at the mouth. Their bodies start with an empty reserve,
/// so package G's satiety asks for the whole bite too.
fn exact_bites() -> FaunaConfig {
    let mut c = FaunaConfig::default();
    for f in Founder::ALL {
        c.founders[f.index()].core.bite_half_stock = 0.0;
    }
    c
}

/// Place a hungry browser founder at one column, facing `+z`, and give it a full feed
/// action.
fn browser_feeding(fauna: &mut Fauna, world: &World, x: i64, z: u32) -> u64 {
    assert!(fauna.apply(
        world,
        FaunaCommand::IntroduceFounder {
            x,
            z,
            founder: Founder::Browser,
            stores: StartingStores {
                body: 0.8,
                reserve: 0.0,
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
    let before = crown_at(&mut flora, &world, 2, 2, 0);

    let mut fauna = Fauna::new(exact_bites());
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

/// (a2) The same browser reaches a crown one 0.25 m voxel above its head because its
/// physical ceiling, 0.463 m, overlaps that cell — and not because of a voxel count:
/// the manifest still records the old `mouth_reach_up_voxels` of one, and nothing
/// reads it. (Before package L's ladder the ceiling was 0.2315 m and this crown was out
/// of the band; the band did not change, the body did.)
#[test]
fn a_crown_one_voxel_above_the_head_is_inside_the_physical_band() {
    let world = flat_world();
    let mut flora = Flora::new(FloraConfig::default().one_layer_species());
    let before = crown_at(&mut flora, &world, 2, 2, 1);

    let mut fauna = Fauna::new(exact_bites());
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
        "a cell starting 0.25 m up is under a 0.463 m mouth"
    );

    for _ in 0..5 {
        fauna.step(&world, &mut flora);
    }
    let after = flora.view().stand_at(site(2, 2)).expect("alive").foliage;
    assert_eq!(fauna.view().ledger.bites, 1, "one attempt, one bite");
    assert!(
        ((before - after) - ONE_BITE).abs() < 1e-12,
        "foliage one voxel up moved by {}",
        before - after
    );
    assert_eq!(
        Founder::Browser.manifest().mouth_reach_up_voxels,
        1,
        "the recorded contract keeps the field the shipped centres were trained with"
    );
}

/// (b) The same browser still cannot bite a crown two voxels above its head: that cell
/// starts 0.5 m over the surface, past the 0.463 m band, the mouth reads air, and the
/// stock does not move.
#[test]
fn a_browser_cannot_bite_a_crown_two_voxels_above_its_head() {
    let world = flat_world();
    let mut flora = Flora::new(FloraConfig::default().one_layer_species());
    let before = crown_at(&mut flora, &world, 2, 2, 2);

    let mut fauna = Fauna::new(exact_bites());
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

    let mut fauna = Fauna::new(exact_bites());
    assert!(fauna.apply(
        &world,
        FaunaCommand::IntroduceFounder {
            x: 2,
            z: 2,
            founder: Founder::Blind,
            stores: StartingStores {
                body: 0.8,
                reserve: 0.0,
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
}

/// (d) The read-only D3 diagnostic and the stepping rule are the same rule. On one
/// fixture holding crowns at `head`, `head + 1` and `head + 2`, a browser placed on each
/// column reports a mouth stand exactly when a real feed tick takes foliage from it.
#[test]
fn the_mouth_diagnostic_agrees_with_the_stepping_rule_at_three_crown_heights() {
    let world = flat_world();
    // Three columns two apart, so no crown disc reaches its neighbour's mouth probes.
    let cases = [(1i64, 0i64), (3, 1), (5, 2)];
    let mut flora = Flora::new(FloraConfig::default().one_layer_species());
    let mut before = Vec::new();
    for (x, rise) in cases {
        before.push(crown_at(&mut flora, &world, x, 2, rise));
    }

    for (i, (x, rise)) in cases.into_iter().enumerate() {
        let mut flora = flora.clone();
        let mut fauna = Fauna::new(exact_bites());
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
        // The band, not a voxel count: this body's ceiling is into the second 0.25 m
        // cell, so the head layer and the one above it are in reach.
        assert_eq!(
            bit,
            rise <= 1,
            "crown at head + {rise} against a 0.463 m physical mouth band"
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
