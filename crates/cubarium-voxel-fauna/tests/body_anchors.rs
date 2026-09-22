//! **Bodies, eyes and mouths in metres** (`design/handoffs/voxel-body-anchors-2026-09-22.md`,
//! deliverable 1; decisions §1, §2, §6).
//!
//! These tests are the brief's, written before the rule. They say what a founder body
//! *is* — adult dimensions in metres on its physiology, every dimension scaling with
//! `(body / body_max)^(1/3)` — and where its anchors sit: the eye at `0.8 × height`
//! over the standing surface, the mouth band `[0, 1.33 × height]` over it, the
//! horizontal reach `0.25 × length` ahead of the footprint.
//!
//! The point of every "both grids" assertion is the encounter contract's invariance
//! claim (`design/voxel-encounter-contract-2026-09-21.md`, §4): the same animal on a
//! 0.125 m and a 0.25 m world is the same animal, and the discretisation is a
//! conversion at the consumer. Nothing here runs more than 200 ticks and nothing pins a
//! hash.

use cubarium_voxel::{Command as WorldCommand, Config as VoxelConfig, Material, World};
use cubarium_voxel_flora::{
    Command as FloraCommand, Flora, FloraConfig, Site, Species as Plant, SpeciesConfig as PlantCfg,
};

use cubarium_voxel_fauna::{
    Actions, Command as FaunaCommand, Fauna, FaunaConfig, Founder, FounderPhysiology, Pose,
    Scripted, StartingStores, surface_m,
};

// ---------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------

/// A flat world of `voxel_m` cells whose ground support face is the highest soil layer.
/// Soil fills `1..=support`, so a body stands on the face at `y = support` and occupies
/// the layer over it.
fn flat_world(voxel_m: f64, support: u32, height: u32) -> World {
    let mut world = World::empty(VoxelConfig {
        width: 16,
        height,
        depth: 8,
        voxel_m,
        ..VoxelConfig::default()
    });
    for z in 0..8 {
        for x in 0..16 {
            for y in 1..=support {
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

/// The `wood` that puts a bloomcrown's one-cell crown exactly `voxels` cells above its
/// own support face, on the grid `config` was built for. Crown heights scale with the
/// grid (`FloraConfig::for_voxel_size`), so the same physical crown needs a different
/// wood on each preset and this searches for it rather than hard-coding one.
fn wood_for_crown_voxels(config: &FloraConfig, voxels: u32) -> f64 {
    let sc: &PlantCfg = config.species(Plant::Bloomcrown);
    let mut wood = 0.02;
    while wood < sc.wood_max {
        if sc.crown_voxels(wood) == voxels {
            return wood;
        }
        wood += 0.005;
    }
    panic!("no bloomcrown wood puts the crown {voxels} cells up on this grid");
}

/// Seed a bloomcrown whose crown cell is the one **containing** `height_m` above the
/// standing surface of a body on the same face, and return its foliage.
fn crown_containing(
    flora: &mut Flora,
    world: &World,
    x: i64,
    z: u32,
    support: u32,
    height_m: f64,
) -> f64 {
    let v = world.config().voxel_m;
    // The surface is `(support + 1) · v`; the cell holding `surface + height_m` is
    // `support + 1 + floor(height_m / v)`, which is `floor(height_m / v) + 1` cells
    // above the support face — and that is exactly `crown_voxels`.
    let cells = (height_m / v).floor() as u32 + 1;
    let wood = wood_for_crown_voxels(flora.config(), cells);
    assert!(flora.apply(
        world,
        FloraCommand::Seed {
            x,
            z,
            species: Plant::Bloomcrown,
            wood,
        },
    ));
    let site = Site {
        x: x as u32,
        y: support,
        z,
    };
    let stand = flora.view().stand_at(site).expect("the seeded bloomcrown");
    assert_eq!(
        flora
            .config()
            .species(Plant::Bloomcrown)
            .crown_voxels(stand.wood),
        cells,
        "the seeded wood moved the crown off the cell holding {height_m} m"
    );
    stand.foliage
}

/// One adult browser founder on a column, holding a full feed action and nothing else.
fn adult_browser_feeding(fauna: &mut Fauna, world: &World, x: i64, z: u32) -> u64 {
    assert!(fauna.apply(
        world,
        FaunaCommand::IntroduceFounder {
            x,
            z,
            founder: Founder::Browser,
            stores: StartingStores::FULL,
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

// ---------------------------------------------------------------------------
// 1. The body
// ---------------------------------------------------------------------------

/// Decisions §1: the adult dimensions are on the founder's physiology, in metres, and a
/// living body's are the adult's times `(body / body_max)^(1/3)`. A newborn browser —
/// a body at `body_min`, a tenth of `body_max` — is 0.46 of the adult's length.
#[test]
fn adult_and_newborn_dimensions_are_metres_on_the_physiology() {
    let browser = FounderPhysiology::frozen(Founder::Browser);
    let adult = browser.adult_body();
    assert_eq!(
        (adult.length_m, adult.width_m, adult.height_m),
        (0.375, 0.1875, 0.1875),
        "the browser adult is the ladder's 6 × 3 × 3 animal"
    );

    let shredder = FounderPhysiology::frozen(Founder::Blind);
    let adult_s = shredder.adult_body();
    assert_eq!(
        (adult_s.length_m, adult_s.width_m, adult_s.height_m),
        (0.19, 0.0625, 0.0625),
        "the shredder adult"
    );

    let core = browser.core;
    let newborn = browser.body_at(core.body_min);
    let scale = (core.body_min / core.body_max).cbrt();
    assert!(
        (scale - 0.4642).abs() < 1e-3,
        "the cube-root convention: {scale}"
    );
    for (got, want) in [
        (newborn.length_m, 0.375 * scale),
        (newborn.width_m, 0.1875 * scale),
        (newborn.height_m, 0.1875 * scale),
    ] {
        assert!((got - want).abs() < 1e-12, "{got} against {want}");
    }
    // Every anchor is a fraction of the living dimensions, so they grow with it.
    assert!((newborn.eye_m - 0.8 * newborn.height_m).abs() < 1e-12);
    assert!((newborn.mouth_ceiling_m - 1.33 * newborn.height_m).abs() < 1e-12);
    assert!((newborn.contact_m - 0.5 * newborn.height_m).abs() < 1e-12);
    assert!((newborn.mouth_reach_m - 0.25 * newborn.length_m).abs() < 1e-12);

    // A body at its full structure is the adult, and the footprint is half the width.
    let full = browser.body_at(core.body_max);
    assert_eq!(full, adult);
    assert!((adult.footprint_radius() - 0.09375).abs() < 1e-12);
}

/// Decisions §6: the eye is `0.8 × body height` above the standing surface, in metres.
/// The same animal standing at the same physical height on a 0.125 m and a 0.25 m world
/// has its eye at the same place — which is exactly what the `standing_y + 1.5` cells
/// origin could not do (the contract's §2, "twice as high on a coarser grid").
#[test]
fn the_eye_is_the_same_height_in_metres_on_both_grids() {
    let browser = FounderPhysiology::frozen(Founder::Browser);
    let adult = browser.adult_body();
    let want = 0.8 * 0.1875;

    // Support faces chosen so the standing surface is 0.75 m on both worlds.
    for (v, support) in [(0.125, 5u32), (0.25, 2u32)] {
        let world = flat_world(v, support, 12);
        let view = world.view();
        let pose = Pose {
            x: 4.0 * v,
            z: 4.0 * v,
            heading_rad: 0.0,
        };
        let surface = surface_m(support, v);
        assert!((surface - 0.75).abs() < 1e-12, "fixture: surface {surface}");
        let (_, eye_y, _) = cubarium_voxel_fauna::eye_origin_m(&view, &pose, support, &adult);
        assert!(
            (eye_y - surface - want).abs() < 1e-12,
            "at {v} m the eye sits {} m over the surface, not {want}",
            eye_y - surface
        );
    }
}

// ---------------------------------------------------------------------------
// 2. The mouth
// ---------------------------------------------------------------------------

/// Decisions §2: the mouth band is `[0, 1.33 × height]` over the standing surface —
/// 0.249375 m for the adult browser. A crown slab holding 0.20 m over the surface is
/// inside it on **both** cell sizes; one holding 0.30 m is outside on both. The bite is
/// the judge, not a helper: the stock moves only where the band reaches.
#[test]
fn a_crown_at_0_20_m_is_reachable_on_both_grids_and_one_at_0_30_m_is_not() {
    let adult = FounderPhysiology::frozen(Founder::Browser).adult_body();
    assert!(
        (adult.mouth_ceiling_m - 0.249375).abs() < 1e-12,
        "the adult ceiling is 1.33 × 0.1875"
    );

    for (v, support, height) in [(0.125f64, 5u32, 14u32), (0.25, 2, 10)] {
        for (target, reachable) in [(0.20f64, true), (0.30, false)] {
            let world = flat_world(v, support, height);
            let mut flora = Flora::new(FloraConfig::for_voxel_size(v));
            let before = crown_containing(&mut flora, &world, 4, 4, support, target);

            let mut fauna = Fauna::new(FaunaConfig::default());
            let id = adult_browser_feeding(&mut fauna, &world, 4, 4);
            let seen = {
                let av = fauna.view();
                cubarium_voxel_fauna::browser_mouth_foliage(
                    &world.view(),
                    &flora.view(),
                    av.config,
                    av.animal(id).expect("the placed browser"),
                )
            };
            assert_eq!(
                seen.is_some(),
                reachable,
                "a crown holding {target} m over the surface on a {v} m grid: the mouth \
                 said {seen:?}"
            );

            for _ in 0..5 {
                fauna.step(&world, &mut flora);
            }
            let after = flora
                .view()
                .stand_at(Site {
                    x: 4,
                    y: support,
                    z: 4,
                })
                .expect("alive")
                .foliage;
            assert_eq!(
                fauna.view().ledger.bites,
                u64::from(reachable),
                "a crown at {target} m on a {v} m grid: bites"
            );
            assert_eq!(
                after < before,
                reachable,
                "a crown at {target} m on a {v} m grid: stock moved by {}",
                before - after
            );
        }
    }
}

/// Decisions §2: the horizontal reach is `0.25 × body length` ahead of the footprint,
/// so an adult browser's mouth region is the disc of radius `W/2` swept out to
/// `0.25 × L` — 0.09375 m each — and reaches 0.28125 m ahead of its centre and no
/// further. The old animal (0.25 m long, 0.125 m wide) reached 0.1875 m, so the column
/// this test names is one the adult reaches and the old body did not.
#[test]
fn the_horizontal_reach_is_a_quarter_of_the_body_length() {
    let adult = FounderPhysiology::frozen(Founder::Browser).adult_body();
    assert!((adult.mouth_reach_m - 0.09375).abs() < 1e-12);

    let v = 0.25;
    let world = flat_world(v, 2, 10);
    let view = world.view();
    // Just inside column 3, facing +x. The mouth's furthest probe is
    // `2 · r + reach = 0.28125` m ahead of the centre, so it crosses into column 4 and
    // stops well short of column 5.
    let pose = Pose {
        x: 3.0 * v + 0.01,
        z: 4.5 * v,
        heading_rad: std::f64::consts::FRAC_PI_2,
    };
    let cols = cubarium_voxel_fauna::mouth_columns_at(&view, &pose, &adult);
    assert!(
        cols.contains(&(3, 4)) && cols.contains(&(4, 4)),
        "the adult's mouth covers its own column and the one ahead: {cols:?}"
    );
    assert!(
        !cols.iter().any(|&(x, _)| x == 5),
        "and nothing two columns ahead: {cols:?}"
    );
}

// ---------------------------------------------------------------------------
// 3. Clearance
// ---------------------------------------------------------------------------

/// The body needs its own **height** of void over the face it stands on, so a void
/// shorter than the body is not a place to stand. On the 0.125 m grid the adult
/// browser's 0.1875 m needs two cells: a one-cell slot refuses it and a two-cell slot
/// admits it. The clearance no longer comes from the mouth's voxel reach.
#[test]
fn a_body_cannot_enter_a_void_shorter_than_its_height() {
    let adult = FounderPhysiology::frozen(Founder::Browser).adult_body();
    assert_eq!(adult.headroom_voxels(0.125), 2, "0.1875 m of body at 0.125 m");
    assert_eq!(adult.headroom_voxels(0.25), 1, "and one cell at 0.25 m");
    let newborn = FounderPhysiology::frozen(Founder::Browser).body_at(0.005);
    assert_eq!(
        newborn.headroom_voxels(0.125),
        1,
        "a 0.087 m newborn fits under one 0.125 m cell"
    );

    // A roofed corridor east of x = 8, one cell of void over the ground, then the same
    // fixture with two.
    for (void, admitted) in [(1u32, false), (2, true)] {
        let mut world = flat_world(0.125, 5, 14);
        for z in 0..8 {
            for x in 8..16 {
                world.apply(WorldCommand::SetMaterial {
                    x,
                    y: 6 + void,
                    z,
                    material: Material::Rock,
                });
            }
        }
        let mut fauna = Fauna::new(FaunaConfig::default());
        assert!(fauna.apply(
            &world,
            FaunaCommand::IntroduceFounder {
                x: 4,
                z: 4,
                founder: Founder::Browser,
                stores: StartingStores::FULL,
                heading_rad: std::f64::consts::FRAC_PI_2,
            },
        ));
        let id = fauna.view().ledger.births - 1;
        assert!(fauna.set_controller(
            id,
            Box::new(Scripted::new(vec![Actions {
                forward: 1.0,
                turn: 0.0,
                feed: 0.0,
            }])),
        ));
        let mut flora = Flora::new(FloraConfig::for_voxel_size(0.125));
        for _ in 0..200 {
            fauna.step(&world, &mut flora);
        }
        let x = fauna.view().animal(id).expect("alive").pose.x;
        assert_eq!(
            x > 8.0 * 0.125,
            admitted,
            "a {void}-cell void: the body walked to {x}"
        );
    }
}

// ---------------------------------------------------------------------------
// 4. Books
// ---------------------------------------------------------------------------

/// The ledgers still close across a bite taken under the band: nothing about measuring
/// the mouth in metres creates or destroys organic matter, mineral or energy.
#[test]
fn the_ledgers_conserve_across_a_bite_under_the_band() {
    let mut world = flat_world(0.25, 2, 10);
    let mut flora = Flora::new(FloraConfig::default());
    crown_containing(&mut flora, &world, 4, 4, 2, 0.20);
    let mut fauna = Fauna::new(FaunaConfig::default());
    adult_browser_feeding(&mut fauna, &world, 4, 4);

    for _ in 0..40 {
        world.step();
        flora.step(&mut world);
        fauna.step(&world, &mut flora);
    }
    assert!(fauna.view().ledger.bites > 0, "the body did eat");

    let fv = flora.view();
    let av = fauna.view();
    for (residual, what) in [
        (fv.organic() - fv.ledger.expected_organic(), "flora organic"),
        (fv.mineral() - fv.ledger.expected_mineral(), "flora mineral"),
        (fv.energy() - fv.ledger.expected_energy(), "flora energy"),
        (av.organic() - av.ledger.expected_organic(), "fauna organic"),
        (av.mineral() - av.ledger.expected_mineral(), "fauna mineral"),
        (av.energy() - av.ledger.expected_energy(), "fauna energy"),
    ] {
        assert!(residual.abs() < 1e-9, "{what} residual {residual:e}");
    }
}

// ---------------------------------------------------------------------------
// 5. The trained contract
// ---------------------------------------------------------------------------

/// The observation vector and its digest do not move. Every number this package
/// changes lives on the **physiology**, never on the `Manifest`, whose geometry fields
/// are in the trained-policy digest — so the shipped centres keep loading and the
/// browser keeps its 37 inputs. Recorded at f1f3f1f, before the anchors changed.
#[test]
fn the_manifest_digests_and_the_input_widths_are_unchanged() {
    let blind = Founder::Blind.manifest();
    let browser = Founder::Browser.manifest();
    assert_eq!(blind.digest(), 6_080_287_729_887_670_217, "littershredder");
    assert_eq!(browser.digest(), 5_231_006_656_698_958_532, "frondgrazer");
    assert_eq!(browser.inputs(), 37, "the browser's observation width");
    assert_eq!(blind.inputs(), 23, "the shredder's observation width");
    // The recorded contract's geometry fields are still there, still what they were.
    assert_eq!((browser.body_length_m, browser.body_width_m), (0.25, 0.125));
    assert_eq!(browser.mouth_reach_up_voxels, 1);
    assert_eq!(browser.mouth_reach_body_lengths, 0.25);
    assert_eq!(browser.ray_pitch_offsets_deg, &[-20.0, 0.0, 20.0]);
}
