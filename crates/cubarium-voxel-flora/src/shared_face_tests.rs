//! **A face holds one plant and one fungus** (2026-10-05, Wrysk: "i feel like glowcaps and
//! plants can coincide ... like normal mushrooms grow under trees").
//!
//! A stand's place on its face is its trophic mode's: photosynthetic stands share one
//! place and saprotrophs another. Every establishment path — the seed lottery, a runner,
//! a founder — asks only its own place, so a plant comes up on a glowcap's face and a
//! glowcap under a plant, and neither displaces the other. Overtopping and runner
//! displacement are plant against plant.
//!
//! Hand-built: a three-column soil slab at field capacity, litter laid on the faces, every
//! seed's attrition and every stand's propagule rate zero unless a fixture says otherwise.
//! Nothing here reads a tuned number: a fixture asks the config's own gates before it
//! expects a birth, and the stands are found by species, so the file compiles against a
//! layer that allowed one stand per face (where every claim here fails).

use cubarium_voxel::{Command as WorldCommand, Config as VoxelConfig, Material, World};

use crate::seeds::{check_period, check_phase, package_of};
use crate::{Command, Deposit, DepositKind, Flora, FloraConfig, Site, Species, Stand, Trophic};

const GLOW: Species = Species::Glowcap;
const TURF: Species = Species::Springturf;
const PAD: Species = Species::Velvetpad;

fn face(x: u32) -> Site {
    Site { x, y: 2, z: 0 }
}

/// `width` soil columns at field capacity, faces at `y = 2`, open sky.
fn slab(width: u32) -> World {
    let mut w = World::empty(VoxelConfig {
        width,
        height: 8,
        depth: 1,
        voxel_m: 1.0,
        seed: 5,
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

/// Nothing reproduces and no seed dies.
fn quiet() -> FloraConfig {
    let mut c = FloraConfig::default();
    for sp in Species::ALL {
        let s = c.species_mut(sp);
        s.seed_attrition_per_s = 0.0;
        s.propagule_rate = 0.0;
        s.burr_pickup = 0.0;
        s.cure_after_s = 0.0;
    }
    c
}

/// Plant a full-grown founder; true when the layer took it.
fn plant(flora: &mut Flora, world: &World, x: u32, species: Species) -> bool {
    let wood = flora.config.species(species).wood_max;
    flora.apply(world, Command::SeedOnFace { site: face(x), species, wood })
}

/// The stand of `species` on `face(x)`, found by species.
fn on(flora: &Flora, x: u32, species: Species) -> Option<Stand> {
    flora
        .view()
        .stands
        .iter()
        .find(|s| s.site == face(x) && s.species == species)
        .copied()
}

/// Litter on a face, booked as a deposit: the glowcap's food.
fn litter(flora: &mut Flora, x: u32) {
    let e = flora.config.species(GLOW).energy_density;
    assert!(flora.deposit(
        face(x),
        Deposit { kind: DepositKind::Litter, organic: 0.5, mineral: 0.01, energy: e * 0.5 }
    ));
}

/// One seed of `species` landed on `face(x)`, booked as seeded material.
fn land(flora: &mut Flora, x: u32, species: Species) {
    let sc = flora.config.species(species).clone();
    let organic = package_of(&sc);
    assert!(flora.deposit_seeds(face(x), species, organic, sc.n_tissue * organic, sc.energy_density * organic));
}

/// Step through `n` of `face(x)`'s seed-bank checks.
fn checks(flora: &mut Flora, world: &mut World, x: u32, n: usize) {
    let period = check_period(&flora.config);
    let phase = check_phase(face(x), period);
    for _ in 0..n {
        let after = flora.tick;
        let t = after - after % period + phase;
        let t = if t > after { t } else { t + period };
        while flora.tick < t {
            flora.step(world);
        }
    }
}

fn assert_conserved(flora: &Flora, when: &str) {
    let v = flora.view();
    let o = v.organic() - v.ledger.expected_organic();
    let n = v.mineral() - v.ledger.expected_mineral();
    let e = v.energy() - v.ledger.expected_energy();
    assert!(o.abs() <= 1e-9 * v.organic().max(1.0), "{when}: organic {o}");
    assert!(n.abs() <= 1e-9 * v.mineral().max(1.0), "{when}: mineral {n}");
    assert!(e.abs() <= 1e-9 * v.energy().max(1.0), "{when}: energy {e}");
}

/// Every photosynthetic species whose own gates pass on `face(1)` of a lit slab.
fn photo_passing(c: &FloraConfig) -> Vec<Species> {
    let mut world = slab(3);
    let mut flora = Flora::new(c.clone());
    litter(&mut flora, 1);
    flora.step(&mut world);
    let v = flora.view();
    Species::ALL
        .into_iter()
        .filter(|&sp| c.species(sp).trophic == Trophic::Photo)
        .filter(|&sp| v.establishment_gates(&world.view(), face(1), sp).passes())
        .collect()
}

/// **A plant comes up on a lit glowcap face, and the glowcap stays.** For every
/// photosynthetic species whose gates pass there (at least one does), one banked seed on a
/// face a glowcap holds on litter germinates, and the glowcap is the same stand after: no
/// overtopping, no death.
#[test]
fn a_plant_establishes_on_a_lit_glowcap_face_and_the_glowcap_stays() {
    let passing = photo_passing(&quiet());
    assert!(!passing.is_empty(), "no photosynthetic species passes its gates on the lit slab");
    for sp in passing {
        let mut world = slab(3);
        let mut flora = Flora::new(quiet());
        litter(&mut flora, 1);
        assert!(plant(&mut flora, &world, 1, GLOW), "the glowcap founder");
        flora.step(&mut world);
        let glow = on(&flora, 1, GLOW).expect("the glowcap").id;
        land(&mut flora, 1, sp);
        checks(&mut flora, &mut world, 1, 3);
        assert!(on(&flora, 1, sp).is_some(), "{sp:?} came up on the glowcap's face");
        assert_eq!(on(&flora, 1, GLOW).map(|s| s.id), Some(glow), "{sp:?}: the glowcap stays");
        let l = flora.view().ledger;
        assert_eq!(l.overtopped[GLOW.index()], 0, "{sp:?}");
        assert_conserved(&flora, &format!("{sp:?}"));
    }
}

/// **A glowcap comes up under a plant on litter, and the plant stays.** A spore banked on
/// a face a velvetpad holds, with litter there for the glowcap's substrate gate: the
/// glowcap germinates and the pad is the same stand after.
#[test]
fn a_glowcap_establishes_under_a_plant_on_litter_and_the_plant_stays() {
    let mut world = slab(3);
    let mut flora = Flora::new(quiet());
    litter(&mut flora, 1);
    assert!(plant(&mut flora, &world, 1, PAD), "the pad founder");
    flora.step(&mut world);
    let g = flora.view().establishment_gates(&world.view(), face(1), GLOW);
    assert!(g.passes(), "the glowcap's own gates pass on the pad's face: {g:?}");
    let pad = on(&flora, 1, PAD).expect("the pad").id;
    land(&mut flora, 1, GLOW);
    checks(&mut flora, &mut world, 1, 3);
    assert!(on(&flora, 1, GLOW).is_some(), "the glowcap came up under the pad");
    assert_eq!(on(&flora, 1, PAD).map(|s| s.id), Some(pad), "the pad stays");
    let l = flora.view().ledger;
    assert_eq!(l.overtopped[PAD.index()], 0);
    assert_conserved(&flora, "glowcap under a pad");
}

/// **Founders ask only their own place.** A glowcap founder goes onto a plant's face and a
/// plant founder onto a glowcap's; a second stand of the same place is refused.
#[test]
fn a_founder_shares_a_face_across_places_and_not_within_one() {
    let world = slab(3);
    let mut flora = Flora::new(quiet());
    assert!(plant(&mut flora, &world, 1, PAD));
    assert!(plant(&mut flora, &world, 1, GLOW), "a glowcap under the pad");
    assert!(!plant(&mut flora, &world, 1, TURF), "a second plant is refused");
    assert!(!plant(&mut flora, &world, 1, GLOW), "a second glowcap is refused");
    assert!(plant(&mut flora, &world, 2, GLOW));
    assert!(plant(&mut flora, &world, 2, TURF), "a plant over the glowcap");
    let v = flora.view();
    assert_eq!(v.stands.len(), 4);
    assert!(v.stands.windows(2).all(|w| w[0].site <= w[1].site), "site order");
    assert_conserved(&flora, "founders");
}

/// **Overtopping is plant against plant.** A face holding a velvetpad and a glowcap; a
/// seed of a photosynthetic species that passes its gates there and may overtop the pad
/// (its draw set to one, any cover) comes up through the pad: the pad dies a booked
/// overtopped death and the glowcap is the same stand after.
#[test]
fn overtopping_takes_the_plant_and_never_the_fungus() {
    let mut c = quiet();
    assert!(c.species(PAD).ground_cover, "the fixture's cover is a ground cover");
    let sp = photo_passing(&c)
        .into_iter()
        .find(|&s| s != PAD)
        .expect("a photosynthetic species other than the pad passes on the lit slab");
    c.species_mut(sp).overtop_per_check = 1.0;
    c.species_mut(sp).overtop_covers = Vec::new();
    let mut world = slab(3);
    let mut flora = Flora::new(c);
    litter(&mut flora, 1);
    assert!(plant(&mut flora, &world, 1, PAD));
    assert!(plant(&mut flora, &world, 1, GLOW), "a glowcap shares the pad's face");
    flora.step(&mut world);
    let glow = on(&flora, 1, GLOW).expect("the glowcap").id;
    land(&mut flora, 1, sp);
    checks(&mut flora, &mut world, 1, 3);
    assert!(on(&flora, 1, sp).is_some(), "{sp:?} came up through the pad");
    assert!(on(&flora, 1, PAD).is_none(), "the pad is gone");
    assert_eq!(on(&flora, 1, GLOW).map(|s| s.id), Some(glow), "the glowcap stays");
    let l = flora.view().ledger;
    assert_eq!(l.overtopped[PAD.index()], 1);
    assert_eq!(l.overtop_pairs[PAD.index()][sp.index()], 1);
    assert_eq!(l.overtopped[GLOW.index()], 0);
    assert_conserved(&flora, "overtop");
}

/// **A runner roots beside a fungus and displaces only a plant.** A springturf donor in
/// the middle column, rooting through covers; its two neighbours hold glowcaps on litter,
/// and one of them a velvetpad too. Its runners root on both faces: the pad dies a booked
/// displaced death and both glowcaps are the same stands after.
#[test]
fn a_runner_displaces_a_plant_and_never_the_fungus() {
    let mut c = quiet();
    let t = &mut c.springturf;
    t.propagule_rate = FloraConfig::default().springturf.propagule_rate.max(1e-3);
    t.clonal_share = 1.0;
    t.runner_reach = 1;
    t.runners_overtop = true;
    t.overtop_covers = Vec::new();
    let mut world = slab(3);
    let mut flora = Flora::new(c);
    assert!(plant(&mut flora, &world, 1, TURF));
    for x in [0, 2] {
        litter(&mut flora, x);
        assert!(plant(&mut flora, &world, x, GLOW));
    }
    assert!(plant(&mut flora, &world, 2, PAD), "a pad shares the second glowcap's face");
    flora.step(&mut world);
    for x in [0, 2] {
        assert!(flora.view().establishment_gates(&world.view(), face(x), TURF).passes(), "turf passes at {x}");
    }
    let glows = [0, 2].map(|x| on(&flora, x, GLOW).expect("a glowcap").id);
    let sc = &flora.config.springturf;
    let per_tick = sc.propagule_rate * crate::DT / (1.0 + sc.build);
    let ticks = (4.0 * package_of(sc) / per_tick).ceil() as u64 + 20;
    for _ in 0..ticks {
        flora.step(&mut world);
        if on(&flora, 0, TURF).is_some() && on(&flora, 2, TURF).is_some() {
            break;
        }
    }
    assert!(on(&flora, 0, TURF).is_some(), "a runner rooted beside the first glowcap");
    assert!(on(&flora, 2, TURF).is_some(), "a runner took the pad's place");
    assert!(on(&flora, 2, PAD).is_none());
    assert_eq!([0, 2].map(|x| on(&flora, x, GLOW).map(|s| s.id)), glows.map(Some), "both glowcaps stay");
    let l = flora.view().ledger;
    assert_eq!(l.runner_displaced[PAD.index()], 1);
    assert_eq!(l.runner_displaced[GLOW.index()], 0);
    assert_conserved(&flora, "runner");
}

/// **A snapshot keeps both places.** A face holding a pad and a glowcap round-trips
/// through the codec with both stands, by id, and the ledgers still close.
#[test]
fn a_snapshot_round_trip_keeps_both_stands_on_a_face() {
    let mut world = slab(3);
    let mut flora = Flora::new(quiet());
    litter(&mut flora, 1);
    assert!(plant(&mut flora, &world, 1, PAD));
    assert!(plant(&mut flora, &world, 1, GLOW));
    for _ in 0..5 {
        flora.step(&mut world);
    }
    let back = crate::snapshot::decode(&crate::snapshot::encode(&flora)).expect("decodes");
    for sp in [PAD, GLOW] {
        assert_eq!(on(&back, 1, sp).map(|s| s.id), on(&flora, 1, sp).map(|s| s.id), "{sp:?}");
        assert!(on(&back, 1, sp).is_some(), "{sp:?} survived the round trip");
    }
    assert_eq!(back.view().stands, flora.view().stands);
    assert_conserved(&back, "decoded");
}
