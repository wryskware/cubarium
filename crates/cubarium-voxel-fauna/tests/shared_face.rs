//! **One face, a plant and a fungus** (2026-10-05): a bite's food picks the stand. A
//! capgnawer's cap-tissue bite on a face holding a bloomcrown and a glowcap comes off the
//! glowcap; a browser's foliage bite on the same face comes off the bloomcrown. Each
//! leaves the other stand untouched, and both layers' books close. A handful of ticks on
//! the diets fixture's flat world; the stands are found by species.

use cubarium_voxel::{Command as WorldCommand, Config as VoxelConfig, Material, World};
use cubarium_voxel_fauna::{
    Actions, Command as FaunaCommand, Fauna, FaunaConfig, Food, Founder, Scripted, StartingStores,
};
use cubarium_voxel_flora::{Command as FloraCommand, Flora, FloraConfig, Site, Species as Plant};

/// 8 × 8 × 6 voxels at 0.25 m, soil `y = 1..=2`, the support face at `y = 2`, day clock off.
fn flat_world() -> World {
    let mut world = World::empty(VoxelConfig {
        width: 8,
        height: 8,
        depth: 6,
        voxel_m: 0.25,
        day: cubarium_voxel::weather::DayClock {
            mode: cubarium_voxel::weather::DayMode::Off,
            ..Default::default()
        },
        ..VoxelConfig::default()
    });
    for z in 0..6 {
        for x in 0..8 {
            for y in 1..=2 {
                world.apply(WorldCommand::SetMaterial { x, y, z, material: Material::Soil });
            }
        }
    }
    world
}

const FACE: Site = Site { x: 2, y: 2, z: 2 };

/// A full-grown bloomcrown and a full-grown glowcap on [`FACE`].
fn shared_face(world: &World) -> Flora {
    let mut flora = Flora::new(FloraConfig::default());
    for species in [Plant::Bloomcrown, Plant::Glowcap] {
        let wood = flora.config().species(species).wood_max;
        assert!(
            flora.apply(world, FloraCommand::SeedOnFace { site: FACE, species, wood }),
            "{species:?} on the shared face"
        );
    }
    flora
}

fn foliage(flora: &Flora, species: Plant) -> f64 {
    flora
        .view()
        .stands
        .iter()
        .find(|s| s.site == FACE && s.species == species)
        .unwrap_or_else(|| panic!("the {species:?}"))
        .foliage
}

/// A founder on [`FACE`]'s column, facing `+z`, feeding every tick.
fn feeding(fauna: &mut Fauna, world: &World, founder: Founder) {
    assert!(fauna.apply(
        world,
        FaunaCommand::IntroduceFounder {
            x: i64::from(FACE.x),
            z: FACE.z,
            founder,
            stores: StartingStores { body: 0.8, reserve: 0.5 },
            heading_rad: 0.0,
        },
    ));
    let id = fauna.view().ledger.births - 1;
    assert!(fauna.set_controller(
        id,
        Box::new(Scripted::new(vec![Actions { forward: 0.0, turn: 0.0, feed: 1.0 }])),
    ));
}

/// Both layers' books close, and the plant layer lost what the animal booked eating net of
/// what it put back.
fn ledgers_close(fauna: &Fauna, flora: &Flora, flora_organic_before: f64) {
    let (av, fv) = (fauna.view(), flora.view());
    for (name, got, want) in [
        ("fauna organic", av.organic(), av.ledger.expected_organic()),
        ("fauna mineral", av.mineral(), av.ledger.expected_mineral()),
        ("fauna energy", av.energy(), av.ledger.expected_energy()),
        ("flora organic", fv.organic(), fv.ledger.expected_organic()),
        ("flora mineral", fv.mineral(), fv.ledger.expected_mineral()),
        ("flora energy", fv.energy(), fv.ledger.expected_energy()),
    ] {
        assert!((got - want).abs() <= 1e-9 * got.abs().max(1.0), "{name}: {got} against {want}");
    }
    let lost = flora_organic_before - fv.organic();
    let net = av.ledger.eaten_organic_in - av.ledger.deposited_organic_out;
    assert!((lost - net).abs() <= 1e-12 * lost.abs().max(1.0), "lost {lost}, booked {net}");
}

/// Run `founder` feeding on the shared face for `ticks`; the foliage of both stands before
/// and after.
fn feed(founder: Founder, ticks: usize) -> (Fauna, [f64; 2], [f64; 2]) {
    let world = flat_world();
    let mut flora = shared_face(&world);
    let before = [foliage(&flora, Plant::Bloomcrown), foliage(&flora, Plant::Glowcap)];
    let organic_before = flora.view().organic();
    let mut fauna = Fauna::new(FaunaConfig::default());
    feeding(&mut fauna, &world, founder);
    for _ in 0..ticks {
        fauna.step(&world, &mut flora);
    }
    let after = [foliage(&flora, Plant::Bloomcrown), foliage(&flora, Plant::Glowcap)];
    ledgers_close(&fauna, &flora, organic_before);
    (fauna, before, after)
}

#[test]
fn a_cap_bite_on_a_shared_face_takes_the_fungus() {
    let (fauna, before, after) = feed(Founder::Capgnawer, 20);
    let l = &fauna.view().ledger;
    assert!(l.bites_by_food[Food::CapTissue.index()] >= 1, "a cap bite: {:?}", l.bites_by_food);
    assert!(l.bites_by_plant[Plant::Glowcap.index()] >= 1, "booked to the glowcap");
    assert_eq!(l.bites_by_plant[Plant::Bloomcrown.index()], 0, "never to the bloomcrown");
    assert!(after[1] < before[1], "the cap lost tissue");
    assert_eq!(after[0], before[0], "the bloomcrown is untouched");
}

#[test]
fn a_foliage_bite_on_a_shared_face_takes_the_plant() {
    let (fauna, before, after) = feed(Founder::Browser, 5);
    let l = &fauna.view().ledger;
    assert!(l.bites_by_food[Food::Foliage.index()] >= 1, "a foliage bite: {:?}", l.bites_by_food);
    assert!(l.bites_by_plant[Plant::Bloomcrown.index()] >= 1, "booked to the bloomcrown");
    assert_eq!(l.bites_by_plant[Plant::Glowcap.index()], 0, "never to the glowcap");
    assert!(after[0] < before[0], "the rosette was cropped");
    assert_eq!(after[1], before[1], "the cap is untouched");
}

/// **A woody recruit in its sward is food** (CONTEST, 2026-10-05): a seedling that came up
/// beside a springturf holds its own place on the face (`FaceLayer::Under` while it is the
/// shorter). With the turf cropped to its grazing floor, a browser feeding there bites the
/// recruit — the bite names the recruit's place — and the turf is not touched again.
#[test]
fn a_browser_bites_a_recruit_beside_its_turf() {
    use cubarium_voxel_flora::FaceLayer;
    // The flat world's soil at field capacity: the water first, then the soil around it.
    let mut world = World::empty(VoxelConfig {
        width: 8,
        height: 8,
        depth: 6,
        voxel_m: 0.25,
        day: cubarium_voxel::weather::DayClock {
            mode: cubarium_voxel::weather::DayMode::Off,
            ..Default::default()
        },
        ..VoxelConfig::default()
    });
    let pore = cubarium_voxel::Materials::DEFAULT.field_capacity(Material::Soil);
    let cap = world.config().materials.pore_capacity(Material::Soil) * world.config().voxel_volume();
    for z in 0..6 {
        for x in 0..8 {
            for y in 1..=2 {
                world.apply(WorldCommand::AddWater { x, y, z, volume_m3: pore * cap });
                world.apply(WorldCommand::SetMaterial { x, y, z, material: Material::Soil });
            }
        }
    }
    let mut c = FloraConfig::default().drop_seeds_checked_each_tick();
    for sc in [&mut c.bloomcrown, &mut c.springturf] {
        sc.seed_attrition_per_s = 0.0;
        sc.propagule_rate = 0.0;
    }
    c.bloomcrown.overtop_per_check = 1.0;
    c.bloomcrown.establish_light_min = 0.0;
    c.bloomcrown.establish_light_margin = 0.0;
    let mut flora = Flora::new(c);
    let wood = flora.config().species(Plant::Springturf).wood_max;
    assert!(flora.apply(&world, FloraCommand::SeedOnFace { site: FACE, species: Plant::Springturf, wood }));
    let sc = flora.config().species(Plant::Bloomcrown).clone();
    let organic = sc.alive_min / sc.propagule_split[0] * f64::from(sc.seed_mass.max(1));
    assert!(flora.deposit_seeds(FACE, Plant::Bloomcrown, organic, sc.n_tissue * organic, sc.energy_density * organic));
    for _ in 0..4 {
        flora.step(&mut world);
    }
    let stand = |f: &Flora, sp: Plant| f.view().stands.iter().find(|s| s.site == FACE && s.species == sp).copied();
    let recruit = stand(&flora, Plant::Bloomcrown).expect("fixture: the recruit came up beside the turf");
    let turf = stand(&flora, Plant::Springturf).expect("the turf stays");
    assert_ne!(recruit.place, turf.place, "two plants, two places");
    // Crop the turf to its grazing floor: no food left in it.
    let every = 0..=i64::from(world.config().height);
    flora.take_foliage_of(FACE, turf.place, 1e9, &every);
    let turf = stand(&flora, Plant::Springturf).unwrap();
    assert!(flora.view().stands_on(FACE).iter().all(|s| s.place != FaceLayer::Fungus));
    let organic_before = flora.view().organic();
    let mut fauna = Fauna::new(FaunaConfig::default());
    feeding(&mut fauna, &world, Founder::Browser);
    for _ in 0..5 {
        fauna.step(&world, &mut flora);
    }
    let l = &fauna.view().ledger;
    assert!(l.bites_by_plant[Plant::Bloomcrown.index()] >= 1, "booked to the recruit: {:?}", l.bites_by_plant);
    assert!(stand(&flora, Plant::Bloomcrown).unwrap().foliage < recruit.foliage, "the recruit was cropped");
    assert_eq!(stand(&flora, Plant::Springturf).unwrap().foliage, turf.foliage, "the bare turf untouched");
    ledgers_close(&fauna, &flora, organic_before);
}
