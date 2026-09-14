use cubarium_core::dormancy::{
    ApexDormancyEvent, ApexDormancyState, MAINTENANCE_PER_STRUCTURE_SECOND, PREY_REQUIRED,
    RECHECK_TICKS, SUSTAIN_TICKS,
};
use cubarium_core::genome::{Genome, decode};
use cubarium_core::hunter::{FixedHunterProfile, HunterEvent, HunterTarget};
use cubarium_core::organism::{DeathCause, Mode, Organism, Origin};
use cubarium_core::rng::Counter;
use cubarium_core::snapshot::{HEADER_FIXED_BYTES, MAGIC, SCHEMA_V13, v13};
use cubarium_core::{DT, LifeEvent, World, WorldConfig, decode_snapshot, encode_snapshot};
use cubarium_surface::{Face, SurfacePoint, Vec2};

const SPOT: SurfacePoint = SurfacePoint {
    face: Face::Top,
    u: 24.0,
    v: 24.0,
};

fn quiet_world() -> World {
    let mut cfg = WorldConfig::default();
    cfg.founders.kinds.clear();
    cfg.founders.count = 0;
    cfg.weather.amplitude = 0.0;
    cfg.water.rain_rate = 0.0;
    cfg.habitat.light_base = 0.0;
    cfg.habitat.light_height_gain = 0.0;
    cfg.habitat.light_noise_gain = 0.0;
    cfg.producer.growth = 0.0;
    cfg.producer.mortality = 0.0;
    cfg.detritus.decomposition = 0.0;
    cfg.detritus.fall = 0.0;
    cfg.organism.maintenance = 0.0;
    cfg.organism.move_cost = 0.0;
    cfg.organism.sense_cost = 0.0;
    cfg.organism.oxidation_rate = 0.0;
    cfg.organism.growth_rate = 0.0;
    World::new(cfg).expect("quiet world")
}

fn target(pos: SurfacePoint) -> HunterTarget {
    HunterTarget {
        face: pos.face.index() as u8,
        u: pos.u,
        v: pos.v,
    }
}

fn dormant_child() -> (World, cubarium_core::OrganismId) {
    let mut world = quiet_world();
    world.state.apex_dormancy = ApexDormancyState::underground_v1();
    let mut profile = FixedHunterProfile::lanternjaw_trial(world.config());
    profile.attacks_enabled = false;
    profile.reproduce_min_age_seconds = 0.0;
    profile.gestation_seconds = DT;
    profile.reproduce_interval_seconds = 10_000.0;
    let parent = world
        .start_hunter_trial(profile, target(SPOT))
        .expect("hunter")
        .id;
    let o = world.state.organisms.get_mut(parent).expect("parent");
    let before = o.reserve;
    o.reserve = o.phenotype.reserve_max;
    o.energy = o.phenotype.energy_max;
    world.state.external_material_in += o.reserve - before;

    for _ in 0..20 {
        world.step();
        if let Some(child) = world
            .drain_hunter_events()
            .into_iter()
            .find_map(|e| match e {
                HunterEvent::Offspring { child, .. } => Some(child),
                _ => None,
            })
        {
            return (world, child);
        }
    }
    panic!("paid child was not inserted")
}

fn place_juvenile_prey(world: &mut World, n: u32, pos: SurfacePoint) {
    let cfg = world.config().clone();
    for i in 0..n {
        let mut genome = Genome::founder(0.4, &cfg.drives);
        genome.clamp();
        let mut phenotype = decode(&genome, &cfg.organism);
        phenotype.speed_max = 0.0;
        phenotype.maintenance = 0.0;
        let structure = 0.2;
        let reserve = 0.02;
        let p = SurfacePoint::new(pos.face, pos.u + f64::from(i) * 0.1, pos.v).canonicalize();
        world.state.organisms.insert(Organism {
            pos: p,
            heading: Vec2::new(1.0, 0.0),
            ou: Vec2::ZERO,
            structure,
            reserve,
            energy: 0.2,
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
        world.state.external_material_in += structure + reserve;
    }
}

#[test]
fn paid_child_enters_as_the_same_concealed_inventory_and_only_pays_dormant_upkeep() {
    let (mut world, child) = dormant_child();
    let entry = *world.apex_dormancy().get(child).expect("dormant record");
    let before = world
        .state
        .organisms
        .get(child)
        .expect("real child")
        .clone();
    assert_eq!(
        world.population(),
        2,
        "dormancy still occupies real capacity"
    );
    assert!(
        world.hunters().contains(child),
        "identity remains in the lineage"
    );
    assert_eq!(before.parent, Some(entry.parent));
    assert!(!world.render_view().organisms.iter().any(|o| o.id == child));
    assert!(!world.hunter_view().iter().any(|o| o.id == child));
    let entered = world.drain_apex_dormancy_events();
    assert!(
        matches!(entered.as_slice(), [ApexDormancyEvent::Entered { child: id, structure, reserve, energy, .. }]
        if *id == child && *structure == before.structure && *reserve == before.reserve && *energy == before.energy)
    );

    let heat_before = world.state.heat_out_corrected();
    world.step();
    let after = world.state.organisms.get(child).expect("still alive");
    let paid = MAINTENANCE_PER_STRUCTURE_SECOND * before.structure * DT;
    assert_eq!(after.pos, before.pos);
    assert_eq!(
        after.structure, before.structure,
        "active maturation is paused"
    );
    assert_eq!(after.reserve, before.reserve);
    assert_eq!(after.born_tick, before.born_tick);
    assert_eq!(
        after.turn_counter, before.turn_counter,
        "no controller draw was consumed"
    );
    assert!((before.energy - after.energy - paid).abs() < 1e-15);
    assert!((world.apex_dormancy().maintenance_energy_paid_total - paid).abs() < 1e-15);
    assert!((world.state.heat_out_corrected() - heat_before - paid).abs() < 1e-12);
    assert!(world.moved_segments(child).is_empty());
    world.check_invariants().expect("accounting remains closed");
}

#[test]
fn emergence_requires_current_sustained_local_juvenile_prey_and_post_payment_stocks() {
    let (mut world, child) = dormant_child();
    let child_pos = world.state.organisms.get(child).unwrap().pos;
    place_juvenile_prey(&mut world, PREY_REQUIRED, child_pos);
    let check_tick = world.tick() + 1;
    {
        let d = world
            .state
            .apex_dormancy
            .dormant
            .iter_mut()
            .find(|d| d.id == child)
            .unwrap();
        d.suitable_prey_ticks = SUSTAIN_TICKS - 1;
        d.next_check_tick = check_tick;
    }
    let old_born = world.state.organisms.get(child).unwrap().born_tick;
    world.step();
    assert!(!world.apex_dormancy().contains(child));
    assert!(world.render_view().organisms.iter().any(|o| o.id == child));
    assert!(world.hunter_view().iter().any(|o| o.id == child));
    let now = world.tick();
    assert_eq!(world.state.organisms.get(child).unwrap().born_tick, now);
    assert!(
        now > old_born,
        "the active age clock was paused underground"
    );
    assert!(matches!(world.drain_apex_dormancy_events().as_slice(),
        [ApexDormancyEvent::Entered { .. }, ApexDormancyEvent::Emerged { id, suitable_prey, .. }]
        if *id == child && *suitable_prey == PREY_REQUIRED));
}

#[test]
fn a_failed_wake_rechecks_and_exhaustion_removes_the_real_organism_with_events() {
    let (mut world, child) = dormant_child();
    world.drain_apex_dormancy_events();
    let check_tick = world.tick() + 1;
    {
        let d = world
            .state
            .apex_dormancy
            .dormant
            .iter_mut()
            .find(|d| d.id == child)
            .unwrap();
        d.suitable_prey_ticks = SUSTAIN_TICKS;
        d.next_check_tick = check_tick;
    }
    // No local prey: current conditions override the persisted streak and schedule a recheck.
    world.step();
    let d = world.apex_dormancy().get(child).unwrap();
    assert_eq!(d.suitable_prey_ticks, 0);
    assert_eq!(d.next_check_tick, world.tick() + RECHECK_TICKS);

    let cost =
        MAINTENANCE_PER_STRUCTURE_SECOND * world.state.organisms.get(child).unwrap().structure * DT;
    world.state.organisms.get_mut(child).unwrap().energy = cost / 2.0;
    world.step();
    assert!(world.state.organisms.get(child).is_none());
    assert!(!world.hunters().contains(child));
    assert!(!world.apex_dormancy().contains(child));
    assert_eq!(world.apex_dormancy().exhausted_total, 1);
    assert!(
        world
            .drain_apex_dormancy_events()
            .iter()
            .any(|e| matches!(e, ApexDormancyEvent::Exhausted { id, .. } if *id == child))
    );
    assert!(world.drain_events().iter().any(
        |e| matches!(e, LifeEvent::Death { id, cause: DeathCause::Starvation, .. } if *id == child)
    ));
    assert!(world.drain_hunter_events().iter().any(|e|
        matches!(e, HunterEvent::Death { id, cause: DeathCause::Starvation, .. } if *id == child)));
    world
        .check_invariants()
        .expect("exhaustion settlement closes");
}

#[test]
fn dormant_state_round_trips_replays_and_schema_thirteen_migrates_off() {
    let plain = quiet_world().state;
    let old = v13::project(&plain).expect("an off state has an old image");
    let payload = postcard::to_allocvec(&old).unwrap();
    let mut framed = Vec::new();
    framed.extend_from_slice(&MAGIC);
    framed.extend_from_slice(&SCHEMA_V13.to_le_bytes());
    framed.extend_from_slice(&0u16.to_le_bytes());
    framed.extend_from_slice(&(payload.len() as u64).to_le_bytes());
    framed.extend_from_slice(&crc32fast::hash(&payload).to_le_bytes());
    framed.extend_from_slice(&payload);
    assert_eq!(framed.len(), HEADER_FIXED_BYTES + payload.len());
    let (_, migrated) = decode_snapshot(&framed).expect("schema 13 loads");
    assert_eq!(migrated, plain);
    assert_eq!(migrated.apex_dormancy, ApexDormancyState::default());

    let (mut original, child) = dormant_child();
    original.drain_apex_dormancy_events();
    original.drain_events();
    original.drain_hunter_events();
    original.step();
    original.drain_apex_dormancy_events();
    original.drain_events();
    original.drain_hunter_events();
    let (_, state) = decode_snapshot(&encode_snapshot(&original.state, "dormant"))
        .expect("current dormancy snapshot loads");
    assert_eq!(state, original.state);
    assert!(state.apex_dormancy.contains(child));
    let mut resumed = World::from_state(state).expect("resumed");
    original.step();
    resumed.step();
    assert_eq!(resumed.state, original.state);
    assert_eq!(
        resumed.drain_apex_dormancy_events(),
        original.drain_apex_dormancy_events()
    );
    assert_eq!(resumed.drain_events(), original.drain_events());
    assert_eq!(
        resumed.drain_hunter_events(),
        original.drain_hunter_events()
    );
}
