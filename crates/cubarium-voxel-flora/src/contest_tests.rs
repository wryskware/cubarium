//! **Woody plants win by shade, not at sprouting** (package CONTEST, Wrysk 2026-10-05: "i
//! thought woody plants just naturally win over grass because they start shading it.
//! seedlings deffo shouldnt just colonize and murder the grass").
//!
//! A woody seed that wins its draw on a ground cover's face comes up **beside** the cover:
//! a stand of its own on the face's second plant place ([`FaceLayer::Under`]) while it is
//! the shorter, its own budget, under the sward's shade by the light law. When its crown
//! rises above the sward it takes the face's top place ([`FaceLayer::Plant`]) and the
//! cover stays on under it, shaded by the shade law; the cover dies only when its own
//! budget fails.
//!
//! Hand-built: a one-deep soil strip at field capacity, day clock off, seeds checked every
//! tick. Nothing here reads a tuned number: every expectation is computed from the
//! config's own laws (crown heights, the shade exponent, the light response) on the stands
//! the fixture built.

use cubarium_voxel::weather::{DayClock, DayMode};
use cubarium_voxel::{Command as WorldCommand, Config as VoxelConfig, Material, World};

use crate::seeds::package_of;
use crate::{Command, DeathCause, FaceLayer, Flora, FloraConfig, Site, Species, Stand, Trophic};

const TURF: Species = Species::Springturf;
const PAD: Species = Species::Velvetpad;
const BLOOM: Species = Species::Bloomcrown;
const LANTERN: Species = Species::Lanternberry;

fn face(x: u32) -> Site {
    Site { x, y: 2, z: 0 }
}

/// `width` soil columns at field capacity, faces at `y = 2`, open sky, day clock off.
fn strip(width: u32) -> World {
    let mut w = World::empty(VoxelConfig {
        width,
        height: 12,
        depth: 1,
        voxel_m: 0.25,
        seed: 23,
        day: DayClock {
            mode: DayMode::Off,
            ..DayClock::default()
        },
        ..VoxelConfig::default()
    });
    let pore = cubarium_voxel::Materials::DEFAULT.field_capacity(Material::Soil);
    let cap = w.config().materials.pore_capacity(Material::Soil) * w.config().voxel_volume();
    for x in 0..i64::from(width) {
        for y in 1..=2u32 {
            w.apply(WorldCommand::AddWater { x, y, z: 0, volume_m3: pore * cap });
            w.apply(WorldCommand::SetMaterial { x, y, z: 0, material: Material::Soil });
        }
    }
    w
}

/// Nothing reproduces, no seed dies, every bank is checked each tick; the woody species
/// come up through any cover at every check, their gates and canopy gate open (neither is
/// this file's subject).
fn config() -> FloraConfig {
    let mut c = FloraConfig::default().drop_seeds_checked_each_tick();
    for sp in Species::ALL {
        let s = c.species_mut(sp);
        s.seed_attrition_per_s = 0.0;
        s.propagule_rate = 0.0;
        s.burr_pickup = 0.0;
        s.cure_after_s = 0.0;
    }
    for sp in [BLOOM, LANTERN] {
        let s = c.species_mut(sp);
        s.overtop_per_check = 1.0;
        s.overtop_covers.clear();
        s.establish_light_min = 0.0;
        s.establish_light_margin = 0.0;
    }
    c
}

fn plant(flora: &mut Flora, world: &World, site: Site, species: Species, wood: f64) -> u64 {
    assert!(
        flora.apply(world, Command::SeedOnFace { site, species, wood }),
        "{species:?} planted on {site:?}"
    );
    flora.view().stands.iter().find(|s| s.site == site && s.species == species).expect("planted").id
}

/// One seed of `species` landed on `site` through the public deposit path.
fn land(flora: &mut Flora, site: Site, species: Species) {
    let sc = flora.config().species(species).clone();
    let organic = package_of(&sc);
    assert!(flora.deposit_seeds(site, species, organic, sc.n_tissue * organic, sc.energy_density * organic));
}

fn find(flora: &Flora, site: Site, species: Species) -> Option<Stand> {
    flora.view().stands.iter().find(|s| s.site == site && s.species == species).copied()
}

fn assert_conserved(flora: &Flora, when: &str) {
    let v = flora.view();
    for (held, expected, what) in [
        (v.organic(), v.ledger.expected_organic(), "organic"),
        (v.mineral(), v.ledger.expected_mineral(), "mineral"),
        (v.energy(), v.ledger.expected_energy(), "energy"),
    ] {
        assert!(
            (held - expected).abs() <= 1e-9 * held.abs().max(1.0),
            "{when}: {what} holds {held}, the ledger expects {expected}"
        );
    }
}

/// `stand` copied onto `site` in `place`, a new identity, booked as seeded material: a
/// fixture's recruit or understorey, built where no seed lottery would put it on the first
/// tick.
fn put(flora: &mut Flora, mut stand: Stand, site: Site, place: FaceLayer) {
    stand.site = site;
    stand.place = place;
    stand.id = flora.ledger.births;
    flora.ledger.births += 1;
    let sc = flora.config.species(stand.species);
    let organic = stand.material();
    flora.ledger.seeded_organic_in += organic;
    flora.ledger.seeded_mineral_in += stand.mineral;
    flora.ledger.seeded_energy_in += sc.energy_density * organic;
    if let Err(g) = flora.ground.binary_search_by_key(&site, |g| g.site) {
        flora.ledger.seeded_mineral_in += flora.config.initial_mineral;
        flora.ground.insert(g, crate::Ground::new(site, flora.config.initial_mineral));
    }
    crate::step::insert_sorted(&flora.config.clone(), &mut flora.stands, vec![stand]);
}

/// The light law's factor for `receiver`'s top layer under `over`'s foliage layers: every
/// foliage layer of `over` multiplies the light by `exp(−share · optical_depth)`, the share
/// by [`crate::layers::shade_share`] (`step::shade_layers_into`).
fn shade_over(config: &FloraConfig, over: &Stand, receiver: &Stand) -> f64 {
    let band = crate::layers_of(config, receiver, config.voxel_m)
        .into_iter()
        .filter(|l| l.kind.bears_foliage())
        .map(|l| l.band_v)
        .fold([f64::NEG_INFINITY; 2], |a, b| if b[1] > a[1] { b } else { a });
    crate::layers_of(config, over, config.voxel_m)
        .into_iter()
        .filter(|l| l.kind.bears_foliage())
        .map(|l| (-crate::layers::shade_share(l.band_v, band) * l.optical_depth).exp())
        .product()
}

/// The raw light `L` a species' response turned into `l_eff` ([`crate::step::light_response`]
/// inverted).
fn raw_light(config: &FloraConfig, species: Species, l_eff: f64) -> f64 {
    let h = config.species(species).light_half.max(0.0);
    l_eff * h / (1.0 + h - l_eff)
}

/// **A woody seed that wins on a turf face comes up beside the turf**: the turf keeps its
/// identity and its face, nothing is overtopped, the newcomer is a recruit of its own,
/// in the place its crown height gives it, and every ledger closes.
#[test]
fn a_woody_seed_that_wins_on_a_turf_face_comes_up_beside_the_turf() {
    for (cover, woody) in [(TURF, BLOOM), (TURF, LANTERN), (PAD, LANTERN)] {
        let mut world = strip(3);
        let mut flora = Flora::new(config());
        let wood = flora.config().species(cover).wood_max;
        let cover_id = plant(&mut flora, &world, face(1), cover, wood);
        land(&mut flora, face(1), woody);
        for _ in 0..3 {
            flora.step(&mut world);
        }
        let held = find(&flora, face(1), cover).unwrap_or_else(|| panic!("{cover:?} still stands"));
        assert_eq!(held.id, cover_id, "{cover:?}: the same stand");
        let recruit = find(&flora, face(1), woody).unwrap_or_else(|| panic!("{woody:?} came up"));
        let l = flora.view().ledger;
        assert_eq!(l.overtopped, [0; Species::COUNT], "{woody:?} through {cover:?}: nothing overtopped");
        assert_eq!(l.deaths_by_cause[DeathCause::Overtopped.index()], [0; Species::COUNT]);
        assert_eq!(l.recruits[woody.index()], 1, "{woody:?} booked as a recruit");
        // The place is the crown heights' (config-derived): the taller of the two is the
        // face's top plant.
        let c = flora.config();
        let (h_cover, h_recruit) = (
            c.species(cover).crown_height_m_at(held.wood),
            c.species(woody).crown_height_m_at(recruit.wood),
        );
        let (top, under) = if h_recruit > h_cover { (woody, cover) } else { (cover, woody) };
        let v = flora.view();
        assert_eq!(v.stand_at(face(1)).map(|s| s.species), Some(top), "{woody:?}/{cover:?}: the top plant");
        assert_eq!(v.understorey_at(face(1)).map(|s| s.species), Some(under), "{woody:?}/{cover:?}: beneath");
        assert_conserved(&flora, &format!("{woody:?} through {cover:?}"));
    }
}

/// **A face holding two plants takes no third**: a second woody seed on a turf face that
/// already holds a recruit waits in the bank.
#[test]
fn a_face_with_a_recruit_takes_no_second() {
    let mut world = strip(3);
    let mut flora = Flora::new(config());
    let wood = flora.config().species(TURF).wood_max;
    plant(&mut flora, &world, face(1), TURF, wood);
    land(&mut flora, face(1), BLOOM);
    for _ in 0..3 {
        flora.step(&mut world);
    }
    assert!(find(&flora, face(1), BLOOM).is_some());
    land(&mut flora, face(1), LANTERN);
    for _ in 0..5 {
        flora.step(&mut world);
    }
    assert!(find(&flora, face(1), LANTERN).is_none(), "no third plant");
    assert_eq!(flora.view().stands_on(face(1)).len(), 2);
    assert!(flora.view().ground_at(face(1)).is_some_and(|g| g.seed_organic(LANTERN) > 0.0), "the seed waits");
}

/// **The sward shades its recruit by the light law**: a recruit under a dense turf gets
/// the light a recruit on bare ground gets times the turf's layer transmission, and over
/// the same ticks it grows less.
#[test]
fn a_recruit_under_a_dense_sward_gets_less_light_and_grows_slower() {
    let mut world = strip(9);
    let mut flora = Flora::new(config());
    let tw = flora.config().species(TURF).wood_max;
    plant(&mut flora, &world, face(6), TURF, tw);
    let alive_min = flora.config().species(BLOOM).alive_min;
    plant(&mut flora, &world, face(1), BLOOM, 2.0 * alive_min);
    // A recruit with an empty reserve, so its gain is its income's (a founder arrives with
    // a full one, and growth at its rate caps would hide the light).
    let i = flora.stands.iter().position(|s| s.site == face(1)).unwrap();
    let q = std::mem::take(&mut flora.stands[i].reserve);
    flora.ledger.removed_organic_out += q;
    flora.ledger.removed_energy_out += flora.config.species(BLOOM).energy_density * q;
    let bare = find(&flora, face(1), BLOOM).expect("the bare recruit");
    put(&mut flora, bare, face(6), FaceLayer::Under);
    let turf = find(&flora, face(6), TURF).expect("turf");
    let under = find(&flora, face(6), BLOOM).expect("the recruit in the turf");
    let c = flora.config().clone();
    assert!(
        c.species(TURF).crown_height_m_at(turf.wood) > c.species(BLOOM).crown_height_m_at(under.wood),
        "fixture: the sward stands above the recruit"
    );
    assert_eq!(flora.view().understorey_at(face(6)).map(|s| s.id), Some(under.id));
    let t = shade_over(&c, &turf, &under);
    assert!(t < 1.0, "fixture: the sward has foliage above the recruit ({t})");
    let before = [bare.material(), under.material()];
    flora.step(&mut world);
    let (a, b) = (find(&flora, face(1), BLOOM).unwrap(), find(&flora, face(6), BLOOM).unwrap());
    let (la, lb) = (raw_light(&c, BLOOM, a.light), raw_light(&c, BLOOM, b.light));
    assert!(lb < la, "shaded {lb} against bare {la}");
    assert!((lb - la * t).abs() <= 1e-9 * la, "the light law: {lb} = {la} x {t}");
    for _ in 0..200 {
        flora.step(&mut world);
    }
    let (a, b) = (find(&flora, face(1), BLOOM).unwrap(), find(&flora, face(6), BLOOM).unwrap());
    assert!(
        b.material() - before[1] < a.material() - before[0],
        "the shaded recruit grew {} against {}",
        b.material() - before[1],
        a.material() - before[0]
    );
    assert_conserved(&flora, "after the race");
}

/// **Escape**: a recruit whose crown has grown above the sward takes the face's top place
/// and the cover stays on under it; when the canopy dies the cover holds the face again.
#[test]
fn a_recruit_above_the_sward_becomes_the_canopy_and_the_cover_stays() {
    let mut world = strip(3);
    let mut flora = Flora::new(config());
    let tw = flora.config().species(TURF).wood_max;
    let turf_id = plant(&mut flora, &world, face(1), TURF, tw);
    let lw = flora.config().species(LANTERN).wood_max;
    plant(&mut flora, &world, face(0), LANTERN, lw);
    let tall = find(&flora, face(0), LANTERN).unwrap();
    assert!(flora.apply(&world, Command::Clear { x: 0, z: 0 }));
    put(&mut flora, tall, face(1), FaceLayer::Under);
    let c = flora.config().clone();
    assert!(
        c.species(LANTERN).crown_height_m_at(tall.wood) > c.species(TURF).crown_height_m_at(tw),
        "fixture: the lanternberry stands above the turf"
    );
    flora.step(&mut world);
    let v = flora.view();
    assert_eq!(v.stand_at(face(1)).map(|s| s.species), Some(LANTERN), "the crown holds the face");
    assert_eq!(v.understorey_at(face(1)).map(|s| s.id), Some(turf_id), "the turf stays under it");
    assert_eq!(v.ledger.escapes[LANTERN.index()], 1);
    assert_eq!(v.ledger.overtopped, [0; Species::COUNT]);
    // The canopy goes: the cover is the face's plant again.
    let canopy = v.stand_at(face(1)).unwrap().id;
    let i = flora.stands.iter().position(|s| s.id == canopy).unwrap();
    flora.stands[i].wood = 0.0;
    flora.stands[i].reserve = 0.0;
    flora.stands[i].foliage = 0.0;
    flora.stands[i].layer_stock = [0.0; crate::MAX_FOLIAGE_LAYERS];
    let lost = tall.material() - flora.stands[i].material();
    flora.ledger.removed_organic_out += lost;
    flora.ledger.removed_energy_out += c.species(LANTERN).energy_density * lost;
    flora.step(&mut world);
    let v = flora.view();
    assert!(find(&flora, face(1), LANTERN).is_none(), "the canopy died");
    assert_eq!(v.stand_at(face(1)).map(|s| s.id), Some(turf_id), "the turf holds the face again");
    assert!(v.understorey_at(face(1)).is_none());
}

/// **A grown canopy shades its understorey by the shade law**, and the understorey dies
/// only when its own budget fails: one whose budget cannot pay dies starved under the
/// canopy (booked as an understorey death, never overtopped), one whose budget pays stays.
#[test]
fn a_canopy_shades_its_understorey_which_dies_only_when_its_budget_fails() {
    for pays in [false, true] {
        let mut c = config();
        let s = c.species_mut(PAD);
        if pays {
            // An income ten times the preset's: whether it pays in the crown's shade is
            // still read off the config's own break-even below.
            s.assimilation *= 10.0;
        } else {
            // Its budget fails: no income, no reserve, an upkeep that spends it in ticks.
            s.assimilation = 0.0;
            s.reserve_cap = 0.0;
            s.maintenance = 1.0;
        }
        let mut world = strip(9);
        let mut flora = Flora::new(c.clone());
        let lw = c.species(LANTERN).wood_max;
        plant(&mut flora, &world, face(6), LANTERN, lw);
        let pw = c.species(PAD).wood_max;
        plant(&mut flora, &world, face(1), PAD, pw);
        let mut pad = find(&flora, face(1), PAD).unwrap();
        pad.reserve = c.species(PAD).reserve_cap * pad.wood;
        put(&mut flora, pad, face(6), FaceLayer::Under);
        let tree = find(&flora, face(6), LANTERN).unwrap();
        let under = find(&flora, face(6), PAD).unwrap();
        let t = shade_over(&c, &tree, &under);
        assert!(t < 1.0, "fixture: the crown has foliage above the pad");
        flora.step(&mut world);
        if pays {
            let (open, shaded) = (find(&flora, face(1), PAD).unwrap(), find(&flora, face(6), PAD).unwrap());
            let (lo, ls) = (raw_light(&c, PAD, open.light), raw_light(&c, PAD, shaded.light));
            assert!((ls - lo * t).abs() <= 1e-9 * lo, "the shade law: {ls} = {lo} x {t}");
            // Its budget in that light, by the config's own break-even: it pays.
            let sc = c.species(PAD);
            let n = flora.view().ground_at(face(6)).unwrap().mineral;
            let be = crate::step::break_even_cover(
                sc,
                shaded.wood,
                shaded.foliage,
                shaded.light,
                shaded.moisture,
                crate::step::monod(sc, n),
            );
            assert!(be >= 1.0, "fixture: the pad's budget holds in the crown's shade ({be})");
        }
        for _ in 0..300 {
            flora.step(&mut world);
        }
        let l = flora.view().ledger;
        assert_eq!(l.overtopped, [0; Species::COUNT], "never overtopped");
        let starved_under = l.under_deaths[DeathCause::Starved.index()][PAD.index()];
        if pays {
            assert!(find(&flora, face(6), PAD).is_some(), "a pad whose budget holds persists under the crown");
            assert_eq!(starved_under, 0);
        } else {
            assert!(find(&flora, face(6), PAD).is_none(), "a pad whose budget fails dies");
            assert_eq!(starved_under, 1, "booked as starved under the canopy");
        }
        assert!(find(&flora, face(6), LANTERN).is_some(), "the crown stands");
        assert_conserved(&flora, &format!("pays {pays}"));
    }
}

/// **A bite reaches a recruit**: a mouth that names the recruit's place takes its foliage,
/// and the cover beside it is untouched.
#[test]
fn a_bite_can_take_a_recruits_foliage() {
    let mut world = strip(3);
    let mut flora = Flora::new(config());
    let tw = flora.config().species(TURF).wood_max;
    plant(&mut flora, &world, face(1), TURF, tw);
    land(&mut flora, face(1), BLOOM);
    for _ in 0..3 {
        flora.step(&mut world);
    }
    let recruit = find(&flora, face(1), BLOOM).expect("the recruit");
    let turf = find(&flora, face(1), TURF).unwrap();
    assert_eq!(flora.config().species(BLOOM).trophic, Trophic::Photo);
    let edible: f64 = crate::layers_of(flora.config(), &recruit, flora.config().voxel_m)
        .iter()
        .map(|l| l.edible())
        .sum();
    assert!(edible > 0.0, "fixture: the recruit holds food");
    let every = 0..=i64::from(world.config().height);
    let taken = flora
        .take_foliage_of(face(1), recruit.place, edible, &every)
        .expect("a bite off the recruit");
    assert!(taken.taken.organic > 0.0);
    assert!(find(&flora, face(1), BLOOM).unwrap().foliage < recruit.foliage, "the recruit lost foliage");
    assert_eq!(find(&flora, face(1), TURF).unwrap().foliage, turf.foliage, "the turf untouched");
    assert_eq!(flora.view().ledger.foliage_taken[BLOOM.index()], taken.taken.organic);
    assert_conserved(&flora, "after the bite");
}

/// **Snapshots carry both plants of a face**, each in its place.
#[test]
fn a_snapshot_round_trips_a_face_with_two_plants() {
    let mut world = strip(3);
    let mut flora = Flora::new(config());
    let tw = flora.config().species(TURF).wood_max;
    plant(&mut flora, &world, face(1), TURF, tw);
    land(&mut flora, face(1), BLOOM);
    for _ in 0..3 {
        flora.step(&mut world);
    }
    assert_eq!(flora.view().stands_on(face(1)).len(), 2, "fixture: two plants on the face");
    let bytes = crate::snapshot::encode(&flora);
    let back = crate::snapshot::decode(&bytes).expect("decodes");
    let places = |f: &Flora| -> Vec<(Species, FaceLayer, u64)> {
        f.view().stands_on(face(1)).iter().map(|s| (s.species, s.place, s.id)).collect()
    };
    assert_eq!(places(&back), places(&flora));
    assert_eq!(back.view().understorey_at(face(1)).map(|s| s.id), flora.view().understorey_at(face(1)).map(|s| s.id));
}
