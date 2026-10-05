//! Glowcap yield (2026-10-05, Wrysk: "glowcaps shouldnt be outcompeting other plants where
//! they are feeding on litter. and its okay for them in the sun"):
//! [`crate::SpeciesConfig::yields_to_plants`]. On a lit face a photosynthetic seed or
//! runner whose own gates pass takes a glowcap's face as if it were bare, and the glowcap
//! dies a booked death; on a face where the plant's light gate fails the glowcap holds; a
//! glowcap never takes a plant's face.
//!
//! Hand-built: a three-column soil slab at field capacity, litter laid on the faces, every
//! seed's attrition and every stand's propagule rate zero unless a fixture says otherwise,
//! so the only births are the ones a fixture sets up. Nothing here reads a tuned number:
//! each fixture asks the config's own gates whether a plant passes before it expects one.

use cubarium_voxel::{Command as WorldCommand, Config as VoxelConfig, Material, World};

use crate::seeds::{check_period, check_phase, package_of};
use crate::{Command, DeathCause, Deposit, DepositKind, Flora, FloraConfig, Site, Species, Trophic};

const GLOW: Species = Species::Glowcap;
const TURF: Species = Species::Springturf;
const PAD: Species = Species::Velvetpad;

fn face(x: u32) -> Site {
    Site { x, y: 2, z: 0 }
}

/// `width` soil columns at field capacity, faces at `y = 2`; with `roof`, a rock lid over
/// every column at `y = 4`, so the faces see (almost) no sky.
fn slab(width: u32, roof: bool) -> World {
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
        if roof {
            w.apply(WorldCommand::SetMaterial { x, y: 4, z: 0, material: Material::Rock });
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

fn plant(flora: &mut Flora, world: &World, x: u32, species: Species) {
    let wood = flora.config.species(species).wood_max;
    assert!(
        flora.apply(world, Command::SeedOnFace { site: face(x), species, wood }),
        "{species:?} planted at {x}"
    );
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

/// **Lit litter**: for every photosynthetic species whose own gates pass on a lit glowcap
/// face (and there is at least one), one banked seed of it comes up there — with its
/// `overtop_per_check` zero, so it is not the overtopping draw — and the glowcap dies a
/// booked overtopped death, its material counted, the ledgers closed.
#[test]
fn a_plant_seed_that_passes_its_gates_takes_a_lit_glowcap_face() {
    let mut tried = 0;
    for sp in Species::ALL {
        let mut c = quiet();
        if c.species(sp).trophic != Trophic::Photo {
            continue;
        }
        c.species_mut(sp).overtop_per_check = 0.0;
        let mut world = slab(3, false);
        let mut flora = Flora::new(c);
        litter(&mut flora, 1);
        plant(&mut flora, &world, 1, GLOW);
        flora.step(&mut world);
        if !flora.view().establishment_gates(&world.view(), face(1), sp).passes() {
            continue;
        }
        tried += 1;
        let glow = flora.view().stand_at(face(1)).expect("glowcap").material();
        land(&mut flora, 1, sp);
        checks(&mut flora, &mut world, 1, 3);
        let v = flora.view();
        assert_eq!(v.stand_at(face(1)).map(|s| s.species), Some(sp), "{sp:?} took the face");
        let l = v.ledger;
        assert_eq!(l.overtopped[GLOW.index()], 1, "{sp:?}");
        assert_eq!(l.overtop_pairs[GLOW.index()][sp.index()], 1, "{sp:?}");
        assert_eq!(l.deaths_by_cause[DeathCause::Overtopped.index()][GLOW.index()], 1, "{sp:?}");
        let booked = l.overtopped_organic[GLOW.index()];
        assert!(booked > 0.0 && booked <= glow * (1.0 + 1e-9), "{sp:?}: booked {booked}, planted {glow}");
        assert_conserved(&flora, &format!("{sp:?}"));
    }
    assert!(tried > 0, "no photosynthetic species passes its gates on the lit slab");
}

/// **Runners**: a springturf donor beside two lit glowcap faces roots its runners on them
/// — with `runners_overtop` off, so it is not the cover rule — and each glowcap it lands on
/// dies a booked displaced death.
#[test]
fn a_plant_runner_roots_on_a_lit_glowcap_face() {
    let mut c = quiet();
    let t = &mut c.springturf;
    t.propagule_rate = FloraConfig::default().springturf.propagule_rate.max(1e-3);
    t.clonal_share = 1.0;
    t.runner_reach = 1;
    t.runners_overtop = false;
    let mut world = slab(3, false);
    let mut flora = Flora::new(c);
    plant(&mut flora, &world, 0, TURF);
    for x in [1, 2] {
        litter(&mut flora, x);
        plant(&mut flora, &world, x, GLOW);
    }
    flora.step(&mut world);
    assert!(flora.view().establishment_gates(&world.view(), face(1), TURF).passes());
    let sc = &flora.config.springturf;
    let per_tick = sc.propagule_rate * crate::DT / (1.0 + sc.build);
    let ticks = (3.0 * package_of(sc) / per_tick).ceil() as u64 + 20;
    for _ in 0..ticks {
        flora.step(&mut world);
        if flora.view().ledger.runner_displaced[GLOW.index()] > 0 {
            break;
        }
    }
    let l = flora.view().ledger;
    let took = l.runner_displaced[GLOW.index()];
    assert!(took > 0, "no runner rooted on a glowcap face");
    assert_eq!(l.deaths_by_cause[DeathCause::Displaced.index()][GLOW.index()], took);
    assert_eq!(l.overtop_pairs[GLOW.index()][TURF.index()], took);
    let turf_on_glow_faces = [1, 2]
        .iter()
        .filter(|&&x| flora.view().stand_at(face(x)).is_some_and(|s| s.species == TURF))
        .count() as u64;
    assert_eq!(turf_on_glow_faces, took);
    assert_conserved(&flora, "runner");
}

/// **Dark ground**: under a rock lid, with the plant's light gate set above the sky the
/// face sees, a plant seed fails its light gate and the glowcap holds; the seed stays
/// banked. And a glowcap seed under a plant never takes the plant's face.
#[test]
fn the_glowcap_holds_where_the_plant_fails_its_light_gate_and_never_takes_a_plant() {
    let mut world = slab(3, true);
    let sky = world.view().sky_visibility(1, 2, 0);
    let mut c = quiet();
    c.velvetpad.overtop_per_check = 0.0;
    c.velvetpad.establish_light_min = 0.5 * (1.0 + sky);
    assert!(sky < c.velvetpad.establish_light_min);
    let mut flora = Flora::new(c);
    litter(&mut flora, 1);
    plant(&mut flora, &world, 1, GLOW);
    flora.step(&mut world);
    let g = flora.view().establishment_gates(&world.view(), face(1), PAD);
    assert!(!g.light_ok, "{g:?}");
    let id = flora.view().stand_at(face(1)).expect("glowcap").id;
    land(&mut flora, 1, PAD);
    checks(&mut flora, &mut world, 1, 3);
    let v = flora.view();
    assert_eq!(v.stand_at(face(1)).map(|s| (s.species, s.id)), Some((GLOW, id)), "the glowcap holds");
    assert!(v.ground_at(face(1)).is_some_and(|g| g.seed_organic(PAD) > 0.0), "the seed stays banked");
    assert_eq!(v.ledger.overtopped[GLOW.index()], 0);
    assert_conserved(&flora, "dark");

    // A glowcap spore under a lit velvetpad: the pad holds.
    let mut world = slab(3, false);
    let mut flora = Flora::new(quiet());
    litter(&mut flora, 1);
    plant(&mut flora, &world, 1, PAD);
    flora.step(&mut world);
    let id = flora.view().stand_at(face(1)).expect("pad").id;
    land(&mut flora, 1, GLOW);
    checks(&mut flora, &mut world, 1, 3);
    let v = flora.view();
    assert_eq!(v.stand_at(face(1)).map(|s| (s.species, s.id)), Some((PAD, id)), "the pad holds");
    assert_eq!(v.ledger.overtopped[PAD.index()], 0);
    assert_conserved(&flora, "glowcap under a pad");
}
