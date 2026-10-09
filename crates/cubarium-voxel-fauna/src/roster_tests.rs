//! **Roster R1, written before the rule** (`design/handoffs/voxel-roster-2026-10-01.md`,
//! Brief R1 "Tests first"): the shredder loses glowcap caps, the capgnawer owns them and
//! smells them on a fungal field, and the loftstrider browses 0.8–3.5 m up, spills a share
//! of every bite as litter, and sees only the foliage in its band.
//!
//! In-crate because the `Chem` and `Cone` claims read the observation vector
//! (`body::observation`) and the cone occupancy, which are crate-private. Every test is a
//! handful of ticks on a flat soil plain at 0.25 m: soil `y = 1..=2`, the support face
//! `y = 2`, so a layer at cell `c` stands `(c − 3) · 0.25` m over the surface.

use cubarium_voxel::{Command as WorldCommand, Config as VoxelConfig, Material, World};
use cubarium_voxel_flora::{
    Command as FloraCommand, Deposit, DepositKind, Flora, FloraConfig, Site, Species as Plant,
};

use crate::{
    Actions, BirthMode, Command, Diet, Fauna, FaunaConfig, Food, Founder, FounderPhysiology,
    Founding, FoundingSite, Hunter, Scripted, StartingStores,
};

const V: f64 = 0.25;
/// Facing `+x`: `Pose::forward` is `(sin h, cos h)`.
const EAST: f64 = std::f64::consts::FRAC_PI_2;
/// Hungry enough to bite, with room in the reserve.
const HUNGRY_ADULT: StartingStores = StartingStores {
    body: 1.0,
    reserve: 0.5,
};

/// A flat plain `width × height × depth` at 0.25 m with the day clock off.
fn plain(width: u32, height: u32, depth: u32) -> World {
    let mut world = World::empty(VoxelConfig {
        width,
        height,
        depth,
        voxel_m: V,
        day: cubarium_voxel::weather::DayClock {
            mode: cubarium_voxel::weather::DayMode::Off,
            ..Default::default()
        },
        ..VoxelConfig::default()
    });
    for z in 0..depth {
        for x in 0..i64::from(width) {
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

/// The shredder-sized plain of the diets fixture.
fn small() -> World {
    plain(16, 8, 6)
}

/// Room for a 2.2 m-wide, 3.4 m-tall body and a vaulttree.
fn tall() -> World {
    plain(32, 40, 24)
}

fn site(x: u32, z: u32) -> Site {
    Site { x, y: 2, z }
}

/// A full-grown stand of `species` at `(x, z)`.
fn seed(flora: &mut Flora, world: &World, x: u32, z: u32, species: Plant) -> Site {
    let wood = flora.config().species(species).wood_max;
    assert!(
        flora.apply(
            world,
            FloraCommand::Seed {
                x: i64::from(x),
                z,
                species,
                wood,
            },
        ),
        "the fixture seeds a {species:?}"
    );
    let s = site(x, z);
    assert!(flora.view().stands_on(s).first().expect("the stand").foliage > 0.0);
    s
}

fn deposit(flora: &mut Flora, s: Site, kind: DepositKind, organic: f64) {
    assert!(flora.deposit(
        s,
        Deposit {
            kind,
            organic,
            mineral: organic * 0.02,
            energy: organic * 2.0,
        },
    ));
}

/// Introduce a founder at a column; returns its id.
fn introduce(
    fauna: &mut Fauna,
    world: &World,
    founder: Founder,
    x: i64,
    z: u32,
    heading_rad: f64,
    stores: StartingStores,
) -> u64 {
    assert!(
        fauna.apply(
            world,
            Command::IntroduceFounder {
                x,
                z,
                founder,
                stores,
                heading_rad,
            },
        ),
        "the fixture places a {founder:?} at ({x}, {z})"
    );
    fauna.view().ledger.births - 1
}

/// Introduce a founder holding a full feed action every tick.
fn feeding(fauna: &mut Fauna, world: &World, founder: Founder, x: i64, z: u32) -> u64 {
    let id = introduce(fauna, world, founder, x, z, 0.0, HUNGRY_ADULT);
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

fn steps(fauna: &mut Fauna, world: &World, flora: &mut Flora, n: usize) {
    for _ in 0..n {
        fauna.step(world, flora);
    }
}

fn foliage(flora: &Flora, s: Site) -> f64 {
    flora.view().stands_on(s).first().expect("the stand").foliage
}

fn litter_at(flora: &Flora, s: Site) -> f64 {
    flora.view().ground_at(s).map_or(0.0, |g| g.litter)
}

/// Both layers' books close, and what the plant layer lost is what the animal booked
/// eating minus what it put back.
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
    let lost = flora_organic_before - fv.organic();
    let net = av.ledger.eaten_organic_in - av.ledger.deposited_organic_out;
    assert!(
        (lost - net).abs() <= 1e-12 * lost.abs().max(1.0),
        "the plant layer lost {lost} and the animal booked {net}"
    );
}

/// The founder body's index in `fauna.animals`.
fn index_of(fauna: &Fauna, id: u64) -> usize {
    fauna
        .animals
        .iter()
        .position(|a| a.id == id)
        .expect("the body is alive")
}

fn module(founder: Founder, prefix: &str) -> crate::Module {
    *founder
        .manifest()
        .modules
        .iter()
        .find(|m| m.name.starts_with(prefix))
        .unwrap_or_else(|| panic!("{founder:?} has a {prefix} module"))
}

// ------------------------------------------------------------------ the decisions

/// The R1 tables' structural choices (brief R1): the shredder's diet without caps; the
/// capgnawer's diet, reproduction mode and founding; the loftstrider's diet, spill, birth
/// mode and founding; the capgnawer on the lanternjaw's list. No tuned number is pinned.
#[test]
fn the_r1_physiology_tables_have_the_briefs_shape() {
    let shredder = FounderPhysiology::frozen(Founder::Blind);
    let grazer = FounderPhysiology::frozen(Founder::Browser);
    let gnawer = FounderPhysiology::frozen(Founder::Capgnawer);
    let strider = FounderPhysiology::frozen(Founder::Loftstrider);

    // The shredder: litter and carrion, no caps.
    assert_eq!(shredder.diet, Diet::of(&[Food::Litter, Food::Carrion]));

    // The capgnawer.
    assert_eq!(gnawer.diet, Diet::of(&[Food::CapTissue, Food::Litter]));
    assert_eq!(gnawer.core.reproduction.mode, shredder.core.reproduction.mode);
    assert_eq!((gnawer.founding, gnawer.founding_site), (Founding::Bodies, FoundingSite::AtFood));
    assert!(gnawer.hunter.is_none());

    // The loftstrider: the only sloppy feeder.
    assert_eq!(strider.diet, Diet::only(Food::Foliage));
    assert!(strider.bite_spill_fraction > 0.0, "the sloppy feeder spills");
    assert_eq!(shredder.bite_spill_fraction, 0.0, "nobody else spills");
    assert_eq!(grazer.bite_spill_fraction, 0.0);
    assert_eq!(gnawer.bite_spill_fraction, 0.0);
    assert_eq!(strider.core.reproduction.mode, BirthMode::Gestation, "one young per birth");
    assert_eq!(
        (strider.founding, strider.founding_site),
        (Founding::Bodies, FoundingSite::AtFood)
    );
    assert!(strider.hunter.is_none() && strider.counter.is_none(), "no defence counter yet");

    // Predation: the capgnawer is on the lanternjaw's list; the loftstrider on none.
    assert!(Hunter::LANTERNJAW.hunts(Founder::Capgnawer));
    assert!(!Hunter::LANTERNJAW.hunts(Founder::Loftstrider));
    let lj = FounderPhysiology::frozen(Founder::Lanternjaw)
        .hunter
        .expect("the lanternjaw hunts");
    assert!(lj.hunts(Founder::Capgnawer) && !lj.hunts(Founder::Loftstrider));

    // Each is its own manifest, its template's v2 layout first.
    assert_eq!(Founder::Capgnawer.manifest().tail_offset(), Founder::Blind.manifest().tail_offset());
    assert_eq!(Founder::Loftstrider.manifest().tail_offset(), Founder::Browser.manifest().tail_offset());
    assert_ne!(Founder::Capgnawer.manifest().digest(), Founder::Blind.manifest().digest());
    assert_ne!(
        Founder::Loftstrider.manifest().digest(),
        Founder::Browser.manifest().digest()
    );
}

// ------------------------------------------------------------------ the shredder

/// **The shredder never takes a cap, and still takes litter and carrion.** A glowcap
/// whose cap stands in its band at its own column: on the default config it bites
/// nothing; litter laid at its face it eats; carrion laid there it eats; the cap is
/// untouched throughout.
#[test]
fn the_shredder_never_takes_a_cap_and_still_takes_litter_and_carrion() {
    let world = small();
    let mut flora = Flora::new(FloraConfig::default());
    let s = seed(&mut flora, &world, 2, 2, Plant::Glowcap);
    let cap = foliage(&flora, s);
    let mut fauna = Fauna::new(FaunaConfig::default());
    let _ = feeding(&mut fauna, &world, Founder::Blind, 2, 2);

    steps(&mut fauna, &world, &mut flora, 20);
    let l = &fauna.view().ledger;
    assert_eq!(l.bites, 0, "a cap is not its food: {:?}", l.bites_by_food);
    assert_eq!(foliage(&flora, s), cap, "the cap is untouched");

    deposit(&mut flora, s, DepositKind::Litter, 1e-3);
    steps(&mut fauna, &world, &mut flora, 20);
    let l = &fauna.view().ledger;
    assert!(l.bites_by_food[Food::Litter.index()] >= 1, "litter: {:?}", l.bites_by_food);
    assert!(flora.view().ground_at(s).unwrap().fragmented_s > 0.0,
        "a successful shredder litter bite fragments the feeding face");

    deposit(&mut flora, s, DepositKind::Carrion, 0.01);
    steps(&mut fauna, &world, &mut flora, 20);
    let l = &fauna.view().ledger;
    assert!(l.bites_by_food[Food::Carrion.index()] >= 1, "carrion: {:?}", l.bites_by_food);
    assert_eq!(l.bites_by_food[Food::CapTissue.index()], 0, "never a cap");
    assert_eq!(l.bites_by_plant[Plant::Glowcap.index()], 0);
    assert_eq!(foliage(&flora, s), cap, "the cap is still untouched");
}

// ------------------------------------------------------------------ the capgnawer

/// **A capgnawer takes a cap in its band, and litter at its own yield.** One glowcap at
/// its column and no litter: the bite is cap tissue off the glowcap. On a bare face with
/// litter only it eats the litter, and what it assimilates of it is at most its litter
/// yield of what it took.
#[test]
fn a_capgnawer_takes_a_cap_in_its_band_and_litter_at_its_own_yield() {
    let world = small();
    let mut flora = Flora::new(FloraConfig::default());
    let s = seed(&mut flora, &world, 2, 2, Plant::Glowcap);
    let cap = foliage(&flora, s);
    let organic_before = flora.view().organic();
    let mut fauna = Fauna::new(FaunaConfig::default());
    let _ = feeding(&mut fauna, &world, Founder::Capgnawer, 2, 2);
    steps(&mut fauna, &world, &mut flora, 20);
    let l = &fauna.view().ledger;
    assert!(l.bites_by_food[Food::CapTissue.index()] >= 1, "a cap bite: {:?}", l.bites_by_food);
    assert!(l.bites_by_plant[Plant::Glowcap.index()] >= 1, "off the glowcap");
    assert!(l.bites_by_founder[Founder::Capgnawer.index()] >= 1);
    assert!(foliage(&flora, s) < cap, "the cap lost tissue");
    ledgers_close(&fauna, &flora, organic_before);

    // Litter only.
    let mut flora = Flora::new(FloraConfig::default());
    let face = site(6, 2);
    deposit(&mut flora, face, DepositKind::Litter, 1e-3);
    let mut fauna = Fauna::new(FaunaConfig::default());
    let _ = feeding(&mut fauna, &world, Founder::Capgnawer, 6, 2);
    steps(&mut fauna, &world, &mut flora, 20);
    let l = &fauna.view().ledger;
    let took = l.eaten_by_food[Food::Litter.index()];
    assert!(l.bites_by_food[Food::Litter.index()] >= 1 && took > 0.0, "it eats litter");
    let got = l.assimilated_by_founder[Founder::Capgnawer.index()];
    let yield_litter = fauna.config().founder(Founder::Capgnawer).yield_for(Food::Litter);
    assert!(
        got > 0.0 && got <= yield_litter * took + 1e-15,
        "it assimilated {got} of {took} at a yield of {yield_litter}"
    );
}

/// The capgnawer's `Chem` response (validity 1) at body `i`, after the field settled.
fn chem(fauna: &Fauna, i: usize, world: &World, flora: &Flora, senses: &mut crate::Senses) -> f64 {
    let founder = fauna.animals[i].founder.expect("a founder");
    let m = module(founder, "Chem");
    let manifest = fauna.config().manifest(founder);
    let (view, fv) = (world.view(), flora.view());
    let obs = crate::body::observation(fauna, i, &view, &fv, &manifest, Some(senses), None);
    assert_eq!(obs[m.offset + 2], 1.0, "{founder:?}: the chem receptor is valid");
    obs[m.offset]
}

/// **A capgnawer's `Chem` rises toward a cap and does not answer litter alone.** Its
/// manifest is the shredder's layout with `Chem(fungal)` in the `Chem(detritus)` slot. On a
/// plain with one glowcap, a capgnawer one column from it reads more than one four
/// columns off, and more than zero. On a plain with only a litter pile, a capgnawer next
/// to it reads zero where a shredder in the same place reads the pile.
#[test]
fn a_capgnawer_chem_rises_toward_a_cap_and_ignores_litter_alone() {
    let blind = module(Founder::Blind, "Chem");
    let fungal = module(Founder::Capgnawer, "Chem");
    assert_eq!(fungal.name, "Chem(fungal)");
    assert_eq!((fungal.offset, fungal.width), (blind.offset, blind.width), "the same slot");

    let world = small();
    let mut flora = Flora::new(FloraConfig::default());
    let _ = seed(&mut flora, &world, 10, 2, Plant::Glowcap);
    let mut fauna = Fauna::new(FaunaConfig::default());
    let near = introduce(&mut fauna, &world, Founder::Capgnawer, 9, 2, EAST, HUNGRY_ADULT);
    let far = introduce(&mut fauna, &world, Founder::Capgnawer, 6, 2, EAST, HUNGRY_ADULT);
    let mut senses = crate::Senses::new();
    let (updates, converged) = senses.settle(&world.view(), &flora.view());
    assert!(converged, "the fields settled in {updates} updates");
    let (i_near, i_far) = (index_of(&fauna, near), index_of(&fauna, far));
    let q_near = chem(&fauna, i_near, &world, &flora, &mut senses);
    let q_far = chem(&fauna, i_far, &world, &flora, &mut senses);
    assert!(q_near > 0.0, "a cap one column away is smelled: {q_near}");
    assert!(q_near > q_far, "it rises toward the cap: near {q_near}, far {q_far}");

    // Litter alone: nothing on the fungal field.
    let mut flora = Flora::new(FloraConfig::default());
    deposit(&mut flora, site(10, 2), DepositKind::Litter, 0.2);
    let mut fauna = Fauna::new(FaunaConfig::default());
    let gnawer = introduce(&mut fauna, &world, Founder::Capgnawer, 9, 2, EAST, HUNGRY_ADULT);
    let mut senses = crate::Senses::new();
    let (updates, converged) = senses.settle(&world.view(), &flora.view());
    assert!(converged, "the fields settled in {updates} updates");
    let i = index_of(&fauna, gnawer);
    assert_eq!(chem(&fauna, i, &world, &flora, &mut senses), 0.0, "litter is not a cap");
    let mut shredders = Fauna::new(FaunaConfig::default());
    let shredder = introduce(&mut shredders, &world, Founder::Blind, 9, 2, EAST, HUNGRY_ADULT);
    let i = index_of(&shredders, shredder);
    assert!(
        chem(&shredders, i, &world, &flora, &mut senses) > 0.0,
        "the fixture's pile is on the detritus field"
    );
}

// ------------------------------------------------------------------ the loftstrider

/// **A loftstrider takes foliage 2 m up and none at 0.3 m.** A vaulttree at its column
/// holds foliage only 1.75 m and more over the surface; a velvetpad's holds it only in the
/// first 0.25 m. It bites the tree and never the pad.
#[test]
fn a_loftstrider_takes_foliage_at_two_metres_and_none_at_ground_level() {
    let world = tall();
    let base = 3; // the first cell over the support face y = 2
    let over = |cell: i64| (cell - base) as f64 * V;

    let mut flora = Flora::new(FloraConfig::default());
    let tree = seed(&mut flora, &world, 12, 12, Plant::Vaulttree);
    let layers: Vec<_> = flora.view().layers_at(tree).collect();
    assert!(layers.iter().all(|l| over(l.cell) >= 0.8), "nothing of the tree below the floor");
    assert!(
        layers.iter().any(|l| l.stock > 0.0 &&
            FounderPhysiology::frozen(Founder::Loftstrider).adult_body().mouth_layers(tree.y, V).contains(&l.cell)),
        "stocked foliage within the loftstrider's mouth band"
    );
    let mut fauna = Fauna::new(FaunaConfig::default());
    let _ = feeding(&mut fauna, &world, Founder::Loftstrider, 12, 12);
    let before = foliage(&flora, tree);
    steps(&mut fauna, &world, &mut flora, 20);
    let l = &fauna.view().ledger;
    assert!(l.bites_by_plant[Plant::Vaulttree.index()] >= 1, "it browses the crown: {:?}", l.bites_by_food);
    assert!(l.bites_by_food[Food::Foliage.index()] >= 1);
    assert!(foliage(&flora, tree) < before);

    let mut flora = Flora::new(FloraConfig::default());
    let pad = seed(&mut flora, &world, 12, 12, Plant::Velvetpad);
    assert!(
        flora.view().layers_at(pad).all(|l| over(l.cell) + V <= 0.3),
        "the pad's foliage is within 0.3 m of the ground"
    );
    let mut fauna = Fauna::new(FaunaConfig::default());
    let _ = feeding(&mut fauna, &world, Founder::Loftstrider, 12, 12);
    let before = foliage(&flora, pad);
    steps(&mut fauna, &world, &mut flora, 20);
    assert_eq!(fauna.view().ledger.bites, 0, "the floor keeps it off ground cover");
    assert_eq!(foliage(&flora, pad), before, "the pad is untouched");
}

/// **A bite's spill lands as litter at its face, and the ledgers close.** The gut never
/// voids in the window, so all litter on the standing face is spill: exactly
/// `bite_spill_fraction` of what the tree lost.
#[test]
fn a_loftstrider_bite_spills_its_share_as_litter_at_its_face_and_the_ledgers_close() {
    let world = tall();
    let mut flora = Flora::new(FloraConfig::default());
    let tree = seed(&mut flora, &world, 12, 12, Plant::Vaulttree);
    let mut config = FaunaConfig::default();
    config.life.gut_void_s = 1e9; // no dung in the window: every grain of litter is spill
    let spill = config.founder(Founder::Loftstrider).bite_spill_fraction;
    assert!(spill > 0.0, "the loftstrider spills");
    let mut fauna = Fauna::new(config);
    let id = feeding(&mut fauna, &world, Founder::Loftstrider, 12, 12);
    let face = fauna.view().animal(id).expect("the loftstrider").site;
    let (crown, litter, organic) = (foliage(&flora, tree), litter_at(&flora, face), flora.view().organic());

    steps(&mut fauna, &world, &mut flora, 20);
    assert_eq!(fauna.view().animal(id).expect("alive").site, face, "it stood still");
    let lost = crown - foliage(&flora, tree);
    let spilt = litter_at(&flora, face) - litter;
    assert!(lost > 0.0, "it browsed");
    assert!(spilt > 0.0, "and spilt");
    assert!(
        (spilt - spill * lost).abs() <= 1e-12 * lost.max(1.0),
        "the face gained {spilt}, {spill} of the {lost} the crown lost"
    );
    assert_eq!(fauna.view().ledger.bites_by_food[Food::Litter.index()], 0, "it does not eat it");
    ledgers_close(&fauna, &flora, organic);
}

/// The loftstrider's cone foliage channels (fraction, proximity) per sector.
fn cone_foliage(fauna: &Fauna, i: usize, world: &World, flora: &Flora) -> Vec<(f64, f64)> {
    let m = module(Founder::Loftstrider, "Cone");
    let manifest = fauna.config().manifest(Founder::Loftstrider);
    let (view, fv) = (world.view(), flora.view());
    let occupancy = crate::senses::cone_occupancy(&view, &fv, &fauna.view());
    let obs = crate::body::observation(fauna, i, &view, &fv, &manifest, None, Some((&occupancy).into()));
    assert_eq!(obs[m.offset + m.width - 1], 1.0, "the cone is valid");
    (0..3).map(|k| (obs[m.offset + k * 6 + 2], obs[m.offset + k * 6 + 3])).collect()
}

/// **A loftstrider's cone reads foliage in its band and none below its floor.** An
/// umbrellafrond (crowns 0.75–2.0 m up) a metre ahead reads as foliage. A velvetpad
/// (0–0.25 m) placed where the fan's steepest ray reaches the ground cover reads as
/// foliage only when the mouth floor is taken away (so the rays do reach it), and as none
/// at the loftstrider's own floor.
#[test]
fn a_loftstrider_cone_reads_foliage_in_its_band_and_none_below_its_floor() {
    let world = tall();
    let (x, z) = (8u32, 12u32);

    let mut flora = Flora::new(FloraConfig::default());
    let _ = seed(&mut flora, &world, x + 4, z, Plant::Umbrellafrond);
    let mut fauna = Fauna::new(FaunaConfig::default());
    let id = introduce(&mut fauna, &world, Founder::Loftstrider, i64::from(x), z, EAST, HUNGRY_ADULT);
    let seen = cone_foliage(&fauna, index_of(&fauna, id), &world, &flora);
    assert!(seen.iter().any(|&(f, _)| f > 0.0), "the crown in its band is seen: {seen:?}");

    // Ground cover where the steepest ray enters the top of the first cell.
    let ground = |floor: Option<f64>| {
        let mut config = FaunaConfig::default();
        if let Some(f) = floor {
            config.founders[Founder::Loftstrider.index()].mouth_floor_fraction = f;
        }
        let mut fauna = Fauna::new(config);
        let id = introduce(&mut fauna, &world, Founder::Loftstrider, i64::from(x), z, EAST, HUNGRY_ADULT);
        let i = index_of(&fauna, id);
        let eye = fauna.config().founder(Founder::Loftstrider).body_at(fauna.animals[i].body).eye_m;
        let manifest = fauna.config().manifest(Founder::Loftstrider);
        let steepest = manifest
            .ray_pitch_offsets_deg
            .iter()
            .fold(0.0f64, |a, &p| a.min(p))
            .abs()
            .to_radians();
        let ahead = ((eye - V) / steepest.tan() / V).round().max(1.0) as u32;
        let mut flora = Flora::new(FloraConfig::default());
        let _ = seed(&mut flora, &world, x + ahead, z, Plant::Velvetpad);
        cone_foliage(&fauna, i, &world, &flora)
    };
    let unbanded = ground(Some(0.0));
    assert!(
        unbanded.iter().any(|&(f, _)| f > 0.0),
        "with no floor the fan reaches the pad (the fixture is not vacuous): {unbanded:?}"
    );
    let banded = ground(None);
    assert!(
        banded.iter().all(|&(f, p)| f == 0.0 && p == 0.0),
        "below its floor is not its food: {banded:?}"
    );
}
