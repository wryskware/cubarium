// R1a review reproductions: failed at 693a300; verified passing at bf96ecb after repair.
// Copy to crates/cubarium-core/tests/review_r1a_temporary.rs and run that test target.
// Minimal fixture helpers reused from tests/hunter.rs; no simulation implementation changes.
use cubarium_core::{World, WorldConfig};
use cubarium_core::genome::{Genome, decode};
use cubarium_core::hunter::{FixedHunterProfile, HunterTarget};
use cubarium_core::ids::OrganismId;
use cubarium_core::organism::{Organism, Origin, Mode};
use cubarium_core::rng::Counter;
use cubarium_surface::{Face, SurfacePoint, Vec2, travel};

fn empty_world() -> World {
    let mut cfg = WorldConfig::default();
    cfg.founders.kinds.clear();
    cfg.founders.count = 0;
    cfg.weather.amplitude = 0.0;
    cfg.water.rain_rate = 0.0;
    World::new(cfg).expect("an empty world is valid")
}

fn trial(world: &World) -> FixedHunterProfile {
    FixedHunterProfile::lanternjaw_trial(world.config())
}

/// A profile whose capture roll always succeeds, so the settlement path is deterministic.
fn certain(mut p: FixedHunterProfile) -> FixedHunterProfile {
    p.capture_min = 1.0;
    p.capture_max = 1.0;
    p
}

fn target_of(pos: SurfacePoint) -> HunterTarget {
    HunterTarget {
        face: pos.face.index() as u8,
        u: pos.u,
        v: pos.v,
    }
}

fn place_prey(
    world: &mut World,
    pos: SurfacePoint,
    s: f64,
    r: f64,
    e: f64,
    frozen: bool,
) -> OrganismId {
    let cfg = world.config().clone();
    let mut genome = Genome::founder(0.5, &cfg.drives);
    genome.size = 0.5;
    genome.speed = 0.3;
    genome.clamp();
    let mut phenotype = decode(&genome, &cfg.organism);
    if frozen {
        phenotype.speed_max = 0.0;
        // Also adult on arrival, so it neither grows into nor out of the eligibility window
        // while the test is watching.
        phenotype.structure_adult = s;
    }
    let id = world.state.organisms.insert(Organism {
        pos: pos.canonicalize(),
        heading: Vec2::new(1.0, 0.0),
        ou: Vec2::ZERO,
        structure: s,
        reserve: r,
        energy: e,
        born_tick: world.tick(),
        hunger_memory: 0.0,
        mode: Mode::Resting,
        escrow: None,
        births: 0,
        genome,
        phenotype,
        parent: None,
        origin: Origin::Founder,
        turn_counter: Counter::default(),
        fed_this_tick: false,
    });
    world.state.external_material_in += s + r;
    id
}

/// Point a hunter at a target and make it hungry enough to hunt, booking the reserve it gave
/// up so the material box stays closed.
fn aim(world: &mut World, id: OrganismId, heading: Vec2, reserve: f64) {
    let o = world.state.organisms.get_mut(id).expect("alive");
    let before = o.reserve;
    o.heading = heading;
    o.reserve = reserve;
    world.state.external_material_in += reserve - before;
}

fn body_offset(heading: Vec2, offset: Vec2, scale: f64) -> Vec2 {
    let h = heading.normalized().expect("a heading");
    let side = Vec2::new(-h.y, h.x);
    (h * offset.x + side * offset.y) * scale
}

/// The surface point of a hunter's capture effector, swept the way any offset is swept.
fn effector_point(
    root: SurfacePoint,
    heading: Vec2,
    profile: &FixedHunterProfile,
    scale: f64,
) -> SurfacePoint {
    travel(
        root,
        body_offset(heading, profile.capture_offset_body, scale),
    )
    .end
}

/// A hunter and one frozen prey exactly inside its claws, both hungry enough to act.
fn staged(profile: FixedHunterProfile) -> (World, OrganismId, OrganismId) {
    staged_in(empty_world(), profile)
}

/// The same staging in a world the caller chose.
fn staged_in(mut world: World, profile: FixedHunterProfile) -> (World, OrganismId, OrganismId) {
    let spot = SurfacePoint::new(Face::Front, 20.0, 32.0);
    let receipt = world
        .start_hunter_trial(profile.clone(), target_of(spot))
        .expect("the trial starts");
    let hunter = receipt.id;
    aim(&mut world, hunter, Vec2::new(1.0, 0.0), 1.0);
    // In the grasp, not in front of the thorax: an adult's claws close 13.28 px ahead and
    // 1.16 px to its clockwise side.
    let grasp = effector_point(spot, Vec2::new(1.0, 0.0), &profile, 1.0);
    let prey = place_prey(&mut world, grasp, 0.5, 0.3, 0.4, true);
    (world, hunter, prey)
}

/// Step until a predicate holds, or panic with what the hunter was doing.
fn run_until(world: &mut World, ticks: u64, mut done: impl FnMut(&World) -> bool) -> u64 {
    for t in 1..=ticks {
        world.step();
        if done(world) {
            return t;
        }
    }
    panic!(
        "nothing happened in {ticks} ticks: phases {:?}",
        world
            .hunters()
            .members
            .iter()
            .map(|m| (m.phase, m.target, m.gut_material))
            .collect::<Vec<_>>()
    );
}


fn review_r1a_policy(reproduce: bool) -> cubarium_core::neural::Policy {
    let mut w = cubarium_core::neural::gru::Gru32::zeros();
    w.b_o = vec![-8.0, 0.0, -8.0, -8.0, -8.0, -8.0, if reproduce { 8.0 } else { -8.0 }];
    cubarium_core::neural::Policy::new(w)
}

#[test]
fn review_r1a_capture_removes_private_state() {
    let (mut world, _, prey) = staged(certain(trial(&empty_world())));
    world.attach_neural_policy(prey, review_r1a_policy(false)).unwrap();
    run_until(&mut world, 2000, |w| w.state.organisms.get(prey).is_none());
    let valid = world.state.validate();
    assert!(!world.neural().contains(prey), "captured prey kept neural state; validation: {valid:?}");
}

#[test]
fn review_r1a_underage_body_cannot_start_gestation() {
    let mut world = empty_world();
    let id = place_prey(&mut world, SurfacePoint::new(Face::Top, 26.0, 26.0), 0.5, 0.3, 0.4, true);
    {
        let o = world.state.organisms.get_mut(id).unwrap();
        let delta = o.phenotype.reserve_max - o.reserve;
        o.reserve = o.phenotype.reserve_max;
        o.energy = o.phenotype.energy_max;
        world.state.external_material_in += delta;
    }
    world.attach_neural_policy(id, review_r1a_policy(true)).unwrap();
    let age_gate = world.state.organisms.get(id).unwrap().phenotype.drives.bud_min_age_seconds;
    assert!(age_gate > 0.0);
    world.step();
    let o = world.state.organisms.get(id).unwrap();
    assert!(o.escrow.is_none(), "age 0 started gestation despite age gate {age_gate}s");
}

#[test]
fn review_r1a_reserve_funded_body_actually_pays_upkeep() {
    let mut world = empty_world();
    let id = place_prey(&mut world, SurfacePoint::new(Face::Top, 26.0, 26.0), 0.5, 0.05, 0.0, true);
    world.attach_neural_policy(id, review_r1a_policy(false)).unwrap();
    let cfg = world.config().clone();
    let before = world.state.organisms.get(id).unwrap().clone();
    let upkeep = cubarium_core::motor::MotorBill::of(&before, &cfg).upkeep(cubarium_core::DT);
    assert!(before.raisable_energy(&cfg.organism, cubarium_core::DT) >= upkeep);
    world.step();
    let after = world.state.organisms.get(id).unwrap();
    let oxidized = (before.reserve - after.reserve) * cfg.organism.reserve_energy_density * cfg.organism.oxidation_efficiency;
    let paid = before.energy + oxidized - after.energy;
    assert!((paid-upkeep).abs() < 1e-12, "owed upkeep {upkeep:e}, actual payment {paid:e}, oxidation credit {oxidized:e}, final energy {:e}", after.energy);
}
