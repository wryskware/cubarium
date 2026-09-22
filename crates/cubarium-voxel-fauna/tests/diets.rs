//! **The diets decisions §3 chose** (`design/handoffs/voxel-diets-2026-09-22.md`,
//! deliverable 1). Written before the rule.
//!
//! Today the browser bites glowcap caps as if they were leaves, the shredder eats
//! litter only, and carrion decomposes with no consumer (audit §2). The decision:
//!
//! - the browser eats **every vascular species'** foliage in its band and nothing
//!   fungal — a glowcap cap is not browser food;
//! - the shredder is a detritivore with **three** foods — litter at the standing face,
//!   glowcap cap tissue taken as a foliage bite from a stand whose layer intersects its
//!   band, and carrion at the standing face;
//! - the yield per food class is an authored placeholder on the shredder's physiology
//!   and the assimilation rule is unchanged.
//!
//! The claims here are the feeding edges and their conservation. The detritus **cue**
//! is a field property and is tested where the field lives
//! (`crates/cubarium-voxel-fauna/src/senses.rs`, `mod tests`), because `sample_cue` is
//! crate-private. Every test is at most a handful of ticks.

use cubarium_voxel::{Command as WorldCommand, Config as VoxelConfig, Material, World};
use cubarium_voxel_flora::{
    Command as FloraCommand, Deposit, DepositKind, Flora, FloraConfig, Site, Species as Plant,
    Trophic,
};

use cubarium_voxel_fauna::{
    Actions, Command as FaunaCommand, Fauna, FaunaConfig, Food, Founder, Manifest, Scripted,
    StartingStores, browser_mouth_foliage,
};

/// A flat world: 8 × 6 × 8 voxels at 0.25 m, soil 1..=2, ground support face at y = 2.
/// The same fixture `plant_layers.rs` and `mouth_reach.rs` use.
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
    let s = site(x as u32, z);
    assert!(
        flora.view().stand_at(s).expect("the seeded stand").foliage > 0.0,
        "the fixture needs a stand with foliage on it"
    );
    s
}

/// A founder at one column, facing `+z`, holding a full feed action for every tick.
fn feeding(fauna: &mut Fauna, world: &World, founder: Founder, x: i64, z: u32) -> u64 {
    assert!(fauna.apply(
        world,
        FaunaCommand::IntroduceFounder {
            x,
            z,
            founder,
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

fn pool(flora: &Flora, s: Site, kind: DepositKind) -> (f64, f64, f64) {
    let fv = flora.view();
    let g = fv.ground_at(s).expect("a provisioned ground entry");
    match kind {
        DepositKind::Carrion => (g.carrion, g.carrion_mineral, g.carrion_energy),
        DepositKind::Litter => (g.litter, g.litter_mineral, g.litter_energy),
        DepositKind::DeadWood => (g.dead_wood, g.dead_wood_mineral, g.dead_wood_energy),
    }
}

/// Both layers' books close on themselves, and what one lost is what the other gained.
fn ledgers_close(fauna: &Fauna, flora: &Flora, flora_organic_before: f64) {
    let av = fauna.view();
    let fv = flora.view();
    for (name, got, want) in [
        ("fauna organic", av.organic(), av.ledger.expected_organic()),
        ("fauna mineral", av.mineral(), av.ledger.expected_mineral()),
        ("fauna energy", av.energy(), av.ledger.expected_energy()),
        ("flora organic", fv.organic(), fv.ledger.expected_organic()),
        ("flora mineral", fv.mineral(), fv.ledger.expected_mineral()),
        ("flora energy", fv.energy(), fv.ledger.expected_energy()),
    ] {
        assert!(
            (got - want).abs() <= 1e-9 * got.abs().max(1.0),
            "the {name} ledger must close: {got} against {want}"
        );
    }
    // The plant layer lost exactly what the animal booked eating, minus what the
    // animal put back as dung.
    let lost = flora_organic_before - fv.organic();
    let net = av.ledger.eaten_organic_in - av.ledger.deposited_organic_out;
    assert!(
        (lost - net).abs() <= 1e-12 * lost.abs().max(1.0),
        "the plant layer lost {lost} and the animal booked {net}"
    );
}

// ------------------------------------------------------------------ the claims

/// (1) **A shredder eats a glowcap cap.** No litter and no carrion anywhere, one
/// glowcap whose cap sits in the shredder's band: the bite comes out of the cap's
/// foliage as a foliage withdrawal, is booked under the cap-tissue food class, and both
/// ledgers close.
#[test]
fn a_shredder_with_no_litter_eats_the_glowcap_cap_in_its_band() {
    let world = flat_world();
    let mut flora = Flora::new(FloraConfig::default());
    assert_eq!(
        flora.config().species(Plant::Glowcap).trophic,
        Trophic::Saprotroph,
        "the fixture's fungal species"
    );
    let wood_max = flora.config().species(Plant::Glowcap).wood_max;
    let s = seed(&mut flora, &world, 2, 2, Plant::Glowcap, wood_max);
    let foliage_before = flora.view().stand_at(s).expect("the stand").foliage;
    let organic_before = flora.view().organic();
    assert_eq!(
        flora.view().ground_at(s).map_or(0.0, |g| g.litter),
        0.0,
        "there must be no litter for this to be the cap's bite"
    );

    let mut fauna = Fauna::new(FaunaConfig::default());
    let _ = feeding(&mut fauna, &world, Founder::Blind, 2, 2);
    for _ in 0..5 {
        fauna.step(&world, &mut flora);
    }

    let l = &fauna.view().ledger;
    assert_eq!(l.bites, 1, "one attempt, one bite");
    assert_eq!(
        l.bites_by_food[Food::CapTissue.index()],
        1,
        "the bite is cap tissue, not litter or carrion: {:?}",
        l.bites_by_food
    );
    assert_eq!(l.bites_by_food[Food::Litter.index()], 0);
    assert_eq!(l.bites_by_food[Food::Carrion.index()], 0);
    assert_eq!(
        l.bites_by_plant[Plant::Glowcap.index()],
        1,
        "and it came off the glowcap"
    );
    let foliage_after = flora.view().stand_at(s).expect("the stand").foliage;
    assert!(
        foliage_after < foliage_before,
        "the cap lost tissue: {foliage_before} -> {foliage_after}"
    );
    assert!(
        (l.eaten_by_food[Food::CapTissue.index()] - (foliage_before - foliage_after)).abs()
            <= 1e-12,
        "what the food class booked is what the cap lost"
    );
    ledgers_close(&fauna, &flora, organic_before);
}

/// (2) **A shredder eats carrion.** A corpse on the face it stands on, nothing else:
/// the withdrawal takes the pool's organic, mineral and energy at the pool's own
/// density, and both ledgers close.
#[test]
fn a_shredder_standing_on_carrion_eats_it_and_the_pool_conserves() {
    let world = flat_world();
    let mut flora = Flora::new(FloraConfig::default());
    let s = site(2, 2);
    assert!(flora.deposit(
        s,
        Deposit {
            kind: DepositKind::Carrion,
            organic: 0.01,
            mineral: 0.01 * 0.05,
            energy: 0.01 * 2.0,
        },
    ));
    let (organic_before, mineral_before, energy_before) = pool(&flora, s, DepositKind::Carrion);
    let flora_organic_before = flora.view().organic();

    let mut fauna = Fauna::new(FaunaConfig::default());
    let _ = feeding(&mut fauna, &world, Founder::Blind, 2, 2);
    for _ in 0..5 {
        fauna.step(&world, &mut flora);
    }

    let l = &fauna.view().ledger;
    assert_eq!(
        l.bites_by_food[Food::Carrion.index()],
        1,
        "the bite is carrion: {:?}",
        l.bites_by_food
    );
    let (organic_after, mineral_after, energy_after) = pool(&flora, s, DepositKind::Carrion);
    let took = organic_before - organic_after;
    assert!(took > 0.0, "the corpse lost organic matter");
    assert!(
        (l.eaten_by_food[Food::Carrion.index()] - took).abs() <= 1e-12,
        "the food class booked {} and the pool lost {took}",
        l.eaten_by_food[Food::Carrion.index()]
    );
    // Pro rata at the pool's own density: the fractions left are the fractions it had.
    let share = took / organic_before;
    assert!(
        (mineral_before - mineral_after - share * mineral_before).abs() <= 1e-15,
        "the mineral left pro rata: {mineral_before} -> {mineral_after}"
    );
    assert!(
        (energy_before - energy_after - share * energy_before).abs() <= 1e-15,
        "the energy left pro rata: {energy_before} -> {energy_after}"
    );
    ledgers_close(&fauna, &flora, flora_organic_before);
}

/// (3) **A browser does not bite a glowcap, and does bite a bloomcrown rosette.** The
/// same mouth, the same band, the same face: the only difference is whose tissue it is.
#[test]
fn a_browser_refuses_the_glowcap_and_takes_the_rosette() {
    let world = flat_world();
    let mut fungal = Flora::new(FloraConfig::default());
    let g = seed(
        &mut fungal,
        &world,
        2,
        2,
        Plant::Glowcap,
        fungal.config().species(Plant::Glowcap).wood_max,
    );
    let mut fauna = Fauna::new(FaunaConfig::default());
    let id = feeding(&mut fauna, &world, Founder::Browser, 2, 2);
    assert_eq!(
        browser_mouth_foliage(
            &world.view(),
            &fungal.view(),
            fauna.view().config,
            fauna.view().animal(id).expect("the browser"),
        ),
        None,
        "fungal tissue is not in a browser's mouth"
    );
    let foliage_before = fungal.view().stand_at(g).expect("the stand").foliage;
    for _ in 0..5 {
        fauna.step(&world, &mut fungal);
    }
    assert_eq!(
        fauna.view().ledger.bites_by_plant[Plant::Glowcap.index()],
        0,
        "a browser never bites a glowcap"
    );
    assert_eq!(
        fungal.view().stand_at(g).expect("the stand").foliage,
        foliage_before,
        "and the cap is untouched"
    );

    // The same browser on a vascular stand of the same shape eats it.
    let mut vascular = Flora::new(FloraConfig::default());
    let b = seed(
        &mut vascular,
        &world,
        2,
        2,
        Plant::Bloomcrown,
        vascular.config().species(Plant::Bloomcrown).wood_max,
    );
    let mut fauna = Fauna::new(FaunaConfig::default());
    let _ = feeding(&mut fauna, &world, Founder::Browser, 2, 2);
    let rosette_before = vascular
        .view()
        .layers_at(b)
        .next()
        .expect("a rosette")
        .stock;
    for _ in 0..5 {
        fauna.step(&world, &mut vascular);
    }
    assert_eq!(
        fauna.view().ledger.bites_by_plant[Plant::Bloomcrown.index()],
        1,
        "the rosette is food"
    );
    assert_eq!(
        fauna.view().ledger.bites_by_food[Food::Foliage.index()],
        1,
        "and it is booked as foliage: {:?}",
        fauna.view().ledger.bites_by_food
    );
    assert!(
        vascular.view().layers_at(b).next().expect("a rosette").stock < rosette_before,
        "the rosette was cropped"
    );
}

/// (4) **The manifests did not move.** The two shipped centres validate against this
/// build by founder-manifest digest, so the diets must not touch a module id, a slot, a
/// width or a taste mapping. The digests are the ones the shipped files carry
/// (`crates/cubarium/assets/policies/*.json`); the retrain owns the *meaning* change on
/// `Chem(litter)`, which is not a schema change.
#[test]
fn the_founder_manifests_keep_the_digests_the_shipped_centres_were_trained_against() {
    assert_eq!(
        Manifest::blind().digest(),
        6_080_287_729_887_670_217,
        "the littershredder centre would stop loading"
    );
    assert_eq!(
        Manifest::browser().digest(),
        5_231_006_656_698_958_532,
        "the frondgrazer centre would stop loading"
    );
    let blind = Manifest::blind();
    let chem = blind
        .modules
        .iter()
        .find(|m| m.name == "Chem(litter)")
        .expect("the shredder's one cue channel keeps its id");
    assert_eq!((chem.offset, chem.width), (18, 3));
}
