//! **Package N: the lanternberry browse line**
//! (`design/handoffs/voxel-new-plants-2026-09-23.md`, "Ecology notes" and "Tests").
//! Written before the rule, by a separate pass. The rest of package N's tests are in
//! `crates/cubarium-voxel-flora/tests/new_plants.rs`.
//!
//! The brief: the browser eats all vascular foliage in its band, `[0, 0.5 m]`; a
//! lanternberry's foliage starts at 0.3 of its height, below 0.5 m at every size, so an
//! adult keeps a **browse line** — cropped below 0.5 m, full above. The anatomy table
//! gives the adult one foliage layer `0.3–1.0`, and today a layer is reached by its one
//! disc cell at the band's **top**, so as written the whole of an adult's foliage would
//! sit above the mouth and nothing would be cropped. How the model expresses the line
//! (splitting the foliage at the line, or a reach that reads the band) is the
//! implementer's call; these tests ask only for the behaviour.
//!
//! ## The interface these tests assume
//!
//! - `cubarium_voxel_flora::Species::Lanternberry` (package N), seedable with
//!   `Command::Seed` like any plant.
//! - Everything else is today's: [`Body::mouth_layers`] (the adult browser's band as the
//!   cell range over the face it stands on), `FloraView::layers_at` (foliage layers with
//!   `cell`, `stock` and `band_m`), `Flora::take_foliage_in_layers` (the bite) and
//!   [`browser_mouth_foliage`] (what the live mouth is offered).
//!
//! "Below the line" is read the way the live mouth reads it: a foliage layer is in reach
//! when its cell is in the mouth band. The tests add the physical claim that what is in
//! reach starts below 0.5 m and what is not reaches above it, on both shipped grids.
//!
//! ## Status before the implementation
//!
//! **Does not compile** today: `Species::Lanternberry` does not exist. With it and the
//! anatomy table taken literally (one `0.3–1.0` foliage layer), the test fails at run
//! time: nothing of an adult is in reach.

use cubarium_voxel::{Command as WorldCommand, Config as VoxelConfig, Material, World};
use cubarium_voxel_fauna::{
    Actions, Body, Command as FaunaCommand, Fauna, FaunaConfig, Founder, Scripted, StartingStores,
    browser_mouth_foliage,
};
use cubarium_voxel_flora::{Command as FloraCommand, Flora, FloraConfig, Site, Species};

/// Support face of the flat fixture.
const FACE: u32 = 2;

/// The browse line, metres above the face: the brief's band `[0, 0.5 m]`.
const LINE_M: f64 = 0.5;

/// A flat world at `voxel_m`: 8 × 6 columns, soil `1..=2`, tall enough for a 1.125 m
/// shrub.
fn flat_world(voxel_m: f64) -> World {
    let height = (1.5 / voxel_m).ceil() as u32 + FACE + 3;
    let mut world = World::empty(VoxelConfig {
        width: 8,
        height,
        depth: 6,
        voxel_m,
        ..VoxelConfig::default()
    });
    for z in 0..6 {
        for x in 0..8 {
            for y in 1..=FACE {
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

/// A full-grown browser at `(x, z)`, facing `+z`, feeding.
fn adult_browser(fauna: &mut Fauna, world: &World, x: i64, z: u32) -> u64 {
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

fn adult_body() -> Body {
    FaunaConfig::default()
        .founder(Founder::Browser)
        .adult_body()
}

/// An adult lanternberry taller than the line: foliage below 0.5 m is in the adult
/// browser's band and holds stock; foliage above it is out of the band and holds stock;
/// what is in reach starts below the line and what is not reaches above it. The live
/// mouth is offered exactly the in-band stock, and a bite of everything it can take
/// leaves the upper foliage untouched. At full size on both grids, and at 0.7 of
/// `wood_max` (0.75 m tall) on the 0.25 m grid.
#[test]
fn an_adult_lanternberry_is_cropped_below_half_a_metre_and_full_above() {
    let body = adult_body();
    assert!(
        body.mouth_ceiling_m <= LINE_M && body.mouth_ceiling_m > 0.45,
        "fixture: the adult browser's band is [0, 0.5 m], got {}",
        body.mouth_ceiling_m
    );
    for (voxel_m, wf) in [(0.25, 1.0), (0.25, 0.7), (0.125, 1.0)] {
        let world = flat_world(voxel_m);
        let mut flora = Flora::new(FloraConfig::for_voxel_size(voxel_m));
        let sc = flora.config().species(Species::Lanternberry).clone();
        let wood = wf * sc.wood_max;
        assert!(sc.profile_index(wood) >= 1, "wf {wf}: an adult");
        assert!(
            sc.crown_height_m_at(wood) > LINE_M,
            "wf {wf}: the fixture shrub must stand above the line"
        );
        assert!(flora.apply(
            &world,
            FloraCommand::Seed {
                x: 2,
                z: 2,
                species: Species::Lanternberry,
                wood,
            },
        ));
        let s = Site {
            x: 2,
            y: FACE,
            z: 2,
        };
        let what = format!("wf {wf} on {voxel_m} m");
        let base_m = f64::from(FACE) * voxel_m;
        let band = body.mouth_layers(FACE, voxel_m);

        let layers: Vec<_> = flora.view().layers_at(s).collect();
        let (mut low, mut high) = (0.0, 0.0);
        for l in &layers {
            if band.contains(&l.cell) {
                low += l.stock;
                assert!(
                    l.band_m[0] - base_m < LINE_M,
                    "{what}: an in-reach layer starts above the line: {l:?}"
                );
            } else {
                high += l.stock;
                assert!(
                    l.band_m[1] - base_m > LINE_M,
                    "{what}: an out-of-reach layer lies below the line: {l:?}"
                );
            }
        }
        assert!(
            low > 0.0,
            "{what}: nothing below the line is in reach: {layers:?}"
        );
        assert!(
            high > 0.0,
            "{what}: nothing above the line is out of reach: {layers:?}"
        );

        let mut fauna = Fauna::new(FaunaConfig::default());
        let id = adult_browser(&mut fauna, &world, 2, 2);
        let offered = {
            let av = fauna.view();
            browser_mouth_foliage(
                &world.view(),
                &flora.view(),
                av.config,
                av.animal(id).expect("the placed browser"),
            )
        };
        let (site, got) = offered.expect("the live mouth reaches the low foliage");
        assert_eq!(site, s);
        assert!(
            (got - low).abs() < 1e-12,
            "{what}: the mouth was offered {got}, the in-reach layers hold {low}"
        );

        assert!(flora.take_foliage_in_layers(s, 1e9, &band).is_some());
        let after: Vec<_> = flora.view().layers_at(s).collect();
        assert_eq!(after.len(), layers.len());
        for (b, a) in layers.iter().zip(&after) {
            if band.contains(&b.cell) {
                assert!(a.stock <= 0.0, "{what}: cropped to nothing below the line");
            } else {
                assert_eq!(
                    a.stock, b.stock,
                    "{what}: the foliage above the line is full"
                );
            }
        }
        let left = flora.view().stand_at(s).expect("the shrub lives").foliage;
        assert!(
            (left - high).abs() < 1e-12,
            "{what}: {left} left, {high} above the line"
        );
    }
}
