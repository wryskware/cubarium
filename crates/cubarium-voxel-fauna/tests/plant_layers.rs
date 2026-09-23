//! **What a low browser can reach once plants have layers**
//! (`design/handoffs/voxel-plant-layers-2026-09-22.md`, deliverable 1). Written before
//! the rule.
//!
//! Package 1b put the mouth in metres and reach fell: a 0.23 m mouth cannot eat a
//! one-cell crown that sits above 0.25 m, which was every adult bloomcrown on the
//! 0.25 m presets (`design/handoffs/voxel-body-anchors-2026-09-22.md`, integration
//! note). Decisions §5's answer is anatomy, not a taller animal: an adult bloomcrown
//! keeps a renewable basal rosette holding a quarter of its foliage, woody seedlings
//! are ground rosettes, and the canopy escapes at the juvenile transition.
//!
//! The claims, on the same flat 0.25 m fixture `mouth_reach.rs` uses (support face at
//! `y = 2`, a body standing in layer 3, a browser at 0.8 of `body_max` whose ceiling is
//! 0.463 m on package L's ladder — into the second cell):
//!
//! 1. An adult bloomcrown's reachable stock is **its rosette's stock**, not its whole
//!    foliage and not zero.
//! 2. A seedling bloomcrown is wholly reachable.
//! 3. A juvenile umbrellafrond offers its lowest tier (package L's accepted
//!    consequence); a grown one offers nothing.
//! 4. A bite takes from the reached layer and leaves the crown alone, and the fauna
//!    ledger books exactly what left the stand.
//! 5. A cone ray fired at a rosette cell reads foliage.

use cubarium_voxel::{Command as WorldCommand, Config as VoxelConfig, Material, World};
use cubarium_voxel_flora::{Command as FloraCommand, Flora, FloraConfig, Site, Species as Plant};

use cubarium_voxel_fauna::{
    Actions, Command as FaunaCommand, Fauna, FaunaConfig, Founder, Scripted, StartingStores,
    browser_mouth_foliage,
};

/// A flat world: 8 × 6 × 6 voxels at 0.25 m, soil 1..=2, ground support face at y = 2.
fn flat_world() -> World {
    let mut world = World::empty(VoxelConfig {
        width: 8,
        height: 8,
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

fn seed(flora: &mut Flora, world: &World, x: i64, z: u32, species: Plant, wood: f64) -> Site {
    assert!(flora.apply(
        world,
        FloraCommand::Seed {
            x,
            z,
            species,
            wood,
        },
    ));
    site(x as u32, z)
}

/// Place a browser founder at one column, facing `+z`, with a full feed action.
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

/// What the live mouth offers at one column, and what the stand actually holds.
fn reached(fauna: &Fauna, world: &World, flora: &Flora, id: u64) -> Option<(Site, f64)> {
    let av = fauna.view();
    browser_mouth_foliage(
        &world.view(),
        &flora.view(),
        av.config,
        av.animal(id).expect("the placed browser"),
    )
}

/// The per-layer stocks of the stand at `s`.
fn stocks(flora: &Flora, s: Site) -> Vec<f64> {
    flora.view().layers_at(s).map(|l| l.stock).collect()
}

// ------------------------------------------------------------------ the claims

/// (1) An adult bloomcrown: the mouth reaches its **rosette** and nothing else. The
/// crown of an adult on this grid sits three cells up; the rosette sits in the first.
#[test]
fn an_adult_bloomcrowns_reachable_stock_is_its_rosette() {
    let world = flat_world();
    let mut flora = Flora::new(FloraConfig::default());
    let wood_max = flora.config().species(Plant::Bloomcrown).wood_max;
    let s = seed(&mut flora, &world, 2, 2, Plant::Bloomcrown, wood_max);

    let layers: Vec<(i64, f64)> = flora
        .view()
        .layers_at(s)
        .map(|l| (l.cell, l.stock))
        .collect();
    assert_eq!(
        layers.len(),
        2,
        "an adult bloomcrown has a rosette and a crown"
    );
    assert!(
        layers[1].0 > layers[0].0,
        "and they sit at different cells: {layers:?}"
    );
    let stand = flora.view().stand_at(s).expect("the stand").clone();
    assert!(
        (layers[0].1 / stand.foliage - 0.25).abs() < 1e-9,
        "the rosette holds {} of {}",
        layers[0].1,
        stand.foliage
    );

    let mut fauna = Fauna::new(FaunaConfig::default());
    let id = browser_feeding(&mut fauna, &world, 2, 2);
    let got = reached(&fauna, &world, &flora, id).expect("a reachable rosette");
    assert_eq!(got.0, s);
    assert!(
        (got.1 - layers[0].1).abs() < 1e-12,
        "the mouth was offered {} and the rosette holds {}",
        got.1,
        layers[0].1
    );
    assert!(
        got.1 < stand.foliage,
        "and it was not offered the whole plant"
    );
}

/// (2) A seedling bloomcrown is a ground rosette no taller than 0.125 m (decisions §5),
/// so all of it is reachable.
#[test]
fn a_seedling_bloomcrown_is_wholly_reachable() {
    let world = flat_world();
    let mut flora = Flora::new(FloraConfig::default());
    let sc = flora.config().species(Plant::Bloomcrown).clone();
    let wood = sc.alive_min.max(0.05 * sc.wood_max);
    assert!(
        sc.profile_index(wood) == 0,
        "the fixture must be in the seedling stage"
    );
    let s = seed(&mut flora, &world, 2, 2, Plant::Bloomcrown, wood);
    let stand = flora.view().stand_at(s).expect("the stand").clone();
    assert_eq!(stocks(&flora, s).len(), 1, "one rosette layer");

    let mut fauna = Fauna::new(FaunaConfig::default());
    let id = browser_feeding(&mut fauna, &world, 2, 2);
    let got = reached(&fauna, &world, &flora, id).expect("a reachable seedling");
    assert!(
        (got.1 - stand.foliage).abs() < 1e-12,
        "a seedling offered {} of its {}",
        got.1,
        stand.foliage
    );
}

/// (3) Package L's accepted consequence (`design/handoffs/voxel-ladder-growth-2026-09-23.md`
/// §2): a young umbrellafrond's lowest tier stays inside the browser's band until the
/// frond is about 1 m tall, so a juvenile at 0.4 of `wood_max` (0.68 m) offers its
/// lowest tier and nothing else; a full-grown 2 m frond has escaped, and the mouth
/// finds nothing. (Before the ladder the 0.23 m mouth reached no juvenile at all.)
#[test]
fn a_juvenile_umbrellafrond_offers_its_lowest_tier_and_an_adult_nothing() {
    let world = flat_world();
    let mut flora = Flora::new(FloraConfig::default());
    let sc = flora.config().species(Plant::Umbrellafrond).clone();
    let wood = sc.wood_max * 0.4;
    assert!(
        sc.profile_index(wood) == 1,
        "the fixture must be in the juvenile stage"
    );
    let s = seed(&mut flora, &world, 2, 2, Plant::Umbrellafrond, wood);
    let stand = flora.view().stand_at(s).expect("the stand").clone();
    assert!(stand.foliage > 0.0);
    let layers = stocks(&flora, s);
    assert_eq!(layers.len(), 2, "a juvenile frond has two tiers");

    let mut fauna = Fauna::new(FaunaConfig::default());
    let id = browser_feeding(&mut fauna, &world, 2, 2);
    let got = reached(&fauna, &world, &flora, id).expect("the lowest tier is in band");
    assert_eq!(got.0, s);
    assert!(
        (got.1 - layers[0]).abs() < 1e-12,
        "the mouth was offered {} and the lowest tier holds {}",
        got.1,
        layers[0]
    );
    assert!(got.1 < stand.foliage, "and not the upper tier");

    let mut grown = Flora::new(FloraConfig::default());
    seed(&mut grown, &world, 2, 2, Plant::Umbrellafrond, sc.wood_max);
    assert_eq!(
        reached(&fauna, &world, &grown, id),
        None,
        "a 2 m frond's lowest tier is over a 0.463 m mouth"
    );
}

/// (4) The bite takes from the layer the mouth reached and leaves the crown exactly
/// where it was, and what the fauna ledger ate is what the stand lost.
#[test]
fn a_bite_empties_the_rosette_and_never_the_crown() {
    let world = flat_world();
    let mut flora = Flora::new(FloraConfig::default());
    let wood_max = flora.config().species(Plant::Bloomcrown).wood_max;
    let s = seed(&mut flora, &world, 2, 2, Plant::Bloomcrown, wood_max);
    let before = stocks(&flora, s);

    let mut fauna = Fauna::new(FaunaConfig::default());
    let _ = browser_feeding(&mut fauna, &world, 2, 2);
    let organic_before = flora.view().organic();
    for _ in 0..5 {
        fauna.step(&world, &mut flora);
    }
    let after = stocks(&flora, s);
    assert_eq!(fauna.view().ledger.bites, 1, "one attempt, one bite");
    assert!(
        after[0] < before[0],
        "the rosette was cropped: {before:?} -> {after:?}"
    );
    assert_eq!(
        after[1], before[1],
        "the crown must be untouched: {before:?} -> {after:?}"
    );
    let lost = organic_before - flora.view().organic();
    let eaten = fauna.view().ledger.eaten_organic_in;
    assert!(
        (lost - eaten).abs() <= 1e-12 * lost.abs().max(1.0),
        "the plant lost {lost} and the animal booked {eaten}"
    );
    assert!(
        (fauna.view().organic() - fauna.view().ledger.expected_organic()).abs()
            <= 1e-9 * fauna.view().organic().abs().max(1.0),
        "the fauna ledger must close"
    );
}

/// (5) Porosity is a light property only: a cone ray that meets a rosette cell reads
/// **foliage**, not air, however porous the layer is. The known simplification is
/// recorded in `design/voxel-encounter-contract-2026-09-21.md` §8.
#[test]
fn a_cone_ray_reads_a_rosette_cell_as_foliage() {
    let mut world = flat_world();
    let mut flora = Flora::new(FloraConfig::default());
    let wood_max = flora.config().species(Plant::Bloomcrown).wood_max;
    let s = seed(&mut flora, &world, 2, 4, Plant::Bloomcrown, wood_max);
    let rosette_cell = flora.view().layers_at(s).next().expect("a rosette").cell;
    assert_eq!(
        rosette_cell,
        i64::from(s.y) + 1,
        "the rosette must be in the first cell over the face"
    );

    let mut fauna = Fauna::new(FaunaConfig::default());
    // A browser three columns away in `z`, facing `+z` — straight at the stand.
    let id = browser_feeding(&mut fauna, &world, 2, 1);
    let av = fauna.view();
    let sectors = cubarium_voxel_fauna::browser_cone_readings(
        &world.view(),
        &flora.view(),
        &av,
        av.animal(id).expect("the browser"),
    )
    .expect("a browser cone");
    assert!(
        sectors.iter().any(|(fraction, _)| *fraction > 0.0),
        "no sector saw the rosette: {sectors:?}"
    );
    let _ = &mut world;
}

/// The layers a stand reports always sum to its scalar `foliage` — the invariant every
/// consumer in this crate relies on when it books what it took.
#[test]
fn the_layers_a_stand_reports_sum_to_its_foliage() {
    let world = flat_world();
    let mut flora = Flora::new(FloraConfig::default());
    for (i, species) in Plant::ALL.iter().enumerate() {
        let sc = flora.config().species(*species).clone();
        for (j, fraction) in [0.1f64, 0.4, 0.9].iter().enumerate() {
            let wood = sc.alive_min + fraction * (sc.wood_max - sc.alive_min);
            let x = (i * 3 + j) as i64 % 8;
            let z = (i + j) as u32 % 6;
            if flora.view().stand_at(site(x as u32, z)).is_some() {
                continue;
            }
            let s = seed(&mut flora, &world, x, z, *species, wood);
            let stand = flora.view().stand_at(s).expect("the stand").clone();
            let sum: f64 = stocks(&flora, s).iter().sum();
            assert!(
                (sum - stand.foliage).abs() <= 1e-12 * stand.foliage.max(1.0),
                "{species:?} at wood {wood}: layers sum to {sum}, foliage is {}",
                stand.foliage
            );
        }
    }
}
