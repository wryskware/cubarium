//! An ordinary **neural** body is legitimate prey for a legacy hunter, and a capture is the
//! second boundary in the world that removes an organism.
//!
//! The first (the physiology pass's death commit) took the private recurrent state with the
//! body; the capture settlement did not, and the world was then failing its own `validate`
//! with "neural animal has no organism" after a perfectly ordinary interaction. Refusing a
//! neural *apex* says nothing about this combination.
//!
//! The staging below is Astra's review fixture (`design/7_Research/assets/`), kept because it
//! produces a certain capture rather than waiting for a favourable roll.

use cubarium_surface::Topology;
use cubarium_core::genome::{Genome, decode};
use cubarium_core::hunter::{FixedHunterProfile, HunterTarget};
use cubarium_core::ids::OrganismId;
use cubarium_core::neural::Policy;
use cubarium_core::neural::gru::Gru32;
use cubarium_core::organism::{Mode, Organism, Origin};
use cubarium_core::rng::Counter;
use cubarium_core::{AnimalState, World, WorldConfig, decode_snapshot, encode_snapshot};
use cubarium_surface::{Face, SurfacePoint, Vec2, travel};

/// A motionless, non-feeding policy: the prey is a body with private state, nothing more.
fn still_policy() -> Policy {
    let mut w = Gru32::zeros();
    w.b_o = vec![-8.0, 0.0, -8.0, -8.0, -8.0, -8.0, -8.0];
    Policy::new(w)
}


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
        pos: pos.canonicalize(Topology::Cube),
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
    travel(Topology::Cube, 
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



#[test]
fn a_captured_neural_prey_leaves_no_private_state_behind() {
    let (mut world, hunter, prey) = staged(certain(trial(&empty_world())));
    world
        .attach_neural_policy(prey, still_policy())
        .expect("an ordinary body may be neural even with a legacy hunter in the world");
    assert!(world.neural().contains(prey));
    let hidden_before = world.neural().get(prey).expect("attached").hidden.clone();
    assert_eq!(hidden_before.len(), 32);

    let ticks = run_until(&mut world, 2000, |w| w.state.organisms.get(prey).is_none());
    assert!(
        world.hunters().captures_total > 0,
        "the fixture must actually capture, not merely lose the prey (after {ticks} ticks)"
    );

    // 1. The entry went at the same boundary the body did.
    assert!(
        !world.neural().contains(prey),
        "the captured prey kept its private state"
    );
    // 2. The world is still valid, which is what a continuation load checks.
    world
        .state
        .validate()
        .expect("a world that just ate a neural body is still a valid world");
    world.check_invariants().unwrap();

    // 3. The post-capture snapshot round-trips and resumes.
    let bytes = encode_snapshot(&world.state, "r1a-predation");
    let (_, reloaded) = decode_snapshot(&bytes).expect("the post-capture snapshot decodes");
    assert_eq!(reloaded, world.state, "the round trip is not lossless");
    let mut resumed = World::from_state(reloaded).expect("and it resumes");
    for _ in 0..40 {
        resumed.step();
        resumed.drain_events();
    }
    resumed.check_invariants().unwrap();

    // 4. The freed slot, reused by a later generation, inherits nothing.
    let reused = OrganismId {
        slot: prey.slot,
        generation: prey.generation + 1,
    };
    assert!(!world.neural().contains(reused));
    let index = world.state.neural.intern(still_policy());
    world
        .state
        .neural
        .insert(reused, AnimalState::fresh(world.tick(), index));
    assert!(
        world
            .neural()
            .get(reused)
            .expect("inserted")
            .hidden
            .iter()
            .all(|h| *h == 0.0),
        "a reused slot starts from zero hidden state"
    );
    // The hunter is untouched by any of this.
    assert!(world.state.organisms.get(hunter).is_some());
}
