//! The reproduction transactions a hunter actually performs, recorded at the mutation that
//! performed them (`design/7_Research/hunter-experiment-contract-2026-09-13.md` section 6).
//!
//! Every identity below is recomputed here from the record's own reported numbers and from the
//! world's public state — never from the implementation's expressions. These are mechanism
//! tests; nothing here is evidence about balance.

use cubarium_core::hunter::{EscrowKey, FixedHunterProfile, FundingBlocked, HunterEvent, HunterTarget, Reproduction};
use cubarium_core::ids::OrganismId;
use cubarium_core::organism::{DeathCause, Mode, Organism, Origin};
use cubarium_core::rng::Counter;
use cubarium_core::snapshot::state_hash;
use cubarium_core::{LifeEvent, World, WorldConfig, decode_snapshot, encode_snapshot};
use cubarium_core::genome::{Genome, decode};
use cubarium_surface::{Face, SurfacePoint, Vec2};

// ---------------------------------------------------------------- fixtures

/// A world with nothing growing, nothing decomposing and no litter of its own, so a deposit
/// into the fields is visible on its own.
fn quiet_world() -> World {
    quiet_config_world(WorldConfig::default())
}

fn quiet_config_world(mut cfg: WorldConfig) -> World {
    cfg.founders.kinds.clear();
    cfg.founders.count = 0;
    cfg.weather.amplitude = 0.0;
    cfg.water.rain_rate = 0.0;
    cfg.habitat.light_base = 0.0;
    cfg.habitat.light_height_gain = 0.0;
    cfg.habitat.light_noise_gain = 0.0;
    cfg.detritus.initial_dark = 0.0;
    cfg.detritus.decomposition = 0.0;
    cfg.detritus.fall = 0.0;
    World::new(cfg).expect("a quiet world is valid")
}

/// The trial profile with its reproduction clocks wound down, so a test can watch a whole
/// gestation. No other value changes.
fn breeder(world: &World) -> FixedHunterProfile {
    let mut p = FixedHunterProfile::lanternjaw_trial(world.config());
    p.reproduce_min_age_seconds = 0.0;
    p.gestation_seconds = 1.0;
    p.reproduce_interval_seconds = 2.0;
    p
}

fn target_of(pos: SurfacePoint) -> HunterTarget {
    HunterTarget { face: pos.face.index() as u8, u: pos.u, v: pos.v }
}

const SPOT: SurfacePoint = SurfacePoint { face: Face::Top, u: 22.0, v: 34.0 };

/// Fill a hunter up so the profile's reproduction gate opens, booking what it gained.
fn feed_to_full(world: &mut World, id: OrganismId) {
    let o = world.state.organisms.get_mut(id).expect("alive");
    let before = o.reserve;
    o.reserve = o.phenotype.reserve_max;
    o.energy = o.phenotype.energy_max;
    world.state.external_material_in += o.reserve - before;
}

/// A bystander that is never worth hunting, to occupy a slot against the cap.
fn place_bystander(world: &mut World, pos: SurfacePoint) -> OrganismId {
    let cfg = world.config().clone();
    let mut genome = Genome::founder(0.5, &cfg.drives);
    genome.size = 2.0;
    genome.clamp();
    let mut phenotype = decode(&genome, &cfg.organism);
    phenotype.speed_max = 0.0;
    phenotype.structure_adult = 1.8;
    let id = world.state.organisms.insert(Organism {
        pos: pos.canonicalize(),
        heading: Vec2::new(1.0, 0.0),
        ou: Vec2::ZERO,
        structure: 1.8,
        reserve: 0.4,
        energy: 0.5,
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
    world.state.external_material_in += 1.8 + 0.4;
    id
}

/// Step, draining both streams, until `done` is satisfied by the hunter records seen so far.
fn run_for(world: &mut World, ticks: u64, mut done: impl FnMut(&[HunterEvent]) -> bool) -> (Vec<HunterEvent>, Vec<LifeEvent>) {
    let (mut hunter, mut life) = (Vec::new(), Vec::new());
    for _ in 0..ticks {
        world.step();
        hunter.extend(world.drain_hunter_events());
        life.extend(world.drain_events());
        if done(&hunter) {
            return (hunter, life);
        }
    }
    panic!("nothing matched in {ticks} ticks: {hunter:?}")
}

fn records(events: &[HunterEvent]) -> Vec<(u64, OrganismId, Reproduction)> {
    events
        .iter()
        .filter_map(|e| match e {
            HunterEvent::Reproduction { tick, hunter, record } => Some((*tick, *hunter, *record)),
            _ => None,
        })
        .collect()
}

fn is_funded(r: &Reproduction) -> bool {
    matches!(r, Reproduction::Funded { .. })
}

// ---------------------------------------------------------------- funded, then born

#[test]
fn a_funded_gestation_reports_its_debits_and_its_birth_reports_the_child_it_became() {
    let mut world = quiet_world();
    let profile = breeder(&world);
    let parent = world.start_hunter_trial(profile.clone(), target_of(SPOT)).expect("started").id;
    feed_to_full(&mut world, parent);
    let cfg = world.config().organism.clone();
    let e_r = cfg.reserve_energy_density;

    // --- the funding transaction
    let (seen, _) = run_for(&mut world, 60, |seen| records(seen).iter().any(|(_, _, r)| is_funded(r)));
    let (tick, hunter, funded) = records(&seen).into_iter().find(|(_, _, r)| is_funded(r)).expect("a funding");
    assert_eq!(hunter, parent);
    assert_eq!(tick, world.tick(), "the record belongs to the tick that completed it");
    let Reproduction::Funded {
        key,
        parent_reserve_before,
        parent_reserve_after,
        parent_energy_before,
        parent_energy_after,
        escrow_structure,
        escrow_reserve,
        escrow_energy,
        build_heat,
    } = funded
    else {
        panic!("{funded:?}")
    };
    assert_eq!(key, EscrowKey { parent, started_tick: tick - 1 }, "the escrow started before the tick closed");

    // Material: what left the parent's reserve is exactly what the escrow holds.
    assert!(
        (parent_reserve_before - parent_reserve_after - (escrow_structure + escrow_reserve)).abs() < 1e-15,
        "{parent_reserve_before} -> {parent_reserve_after} against {escrow_structure} + {escrow_reserve}"
    );
    // Energy: what left the parent is what the escrow holds plus what the build burned.
    let parent_lost = (e_r * parent_reserve_before + parent_energy_before)
        - (e_r * parent_reserve_after + parent_energy_after);
    let escrow_holds = e_r * (escrow_structure + escrow_reserve) + escrow_energy;
    assert!((parent_lost - (escrow_holds + build_heat)).abs() < 1e-12, "{parent_lost} vs {escrow_holds} + {build_heat}");
    // The build heat is the world's own build cost on that structure, and nothing else.
    assert!((build_heat - cfg.build_cost * escrow_structure).abs() < 1e-15);
    // The child inventory is the world's own fractions.
    let phenotype = decode(&profile.genome, &cfg);
    assert!((escrow_structure - cfg.child_structure_fraction * phenotype.structure_adult).abs() < 1e-15);
    assert!((escrow_reserve - cfg.child_reserve_fraction * phenotype.reserve_max).abs() < 1e-15);
    assert!((escrow_energy - cfg.child_energy_fraction * phenotype.energy_max).abs() < 1e-15);
    // Nothing else touched the parent that tick, so the mutation-site record is also what the
    // post-step world shows — the reverse is not true in general, which is why it is recorded.
    let o = world.state.organisms.get(parent).expect("alive");
    assert_eq!(o.reserve, parent_reserve_after);
    assert_eq!(o.energy, parent_energy_after);
    let held = o.escrow.as_ref().expect("the escrow exists");
    assert_eq!((held.structure, held.reserve, held.energy), (escrow_structure, escrow_reserve, escrow_energy));
    assert_eq!(held.started_tick, key.started_tick);

    // --- the birth
    let (seen, life) = run_for(&mut world, 200, |seen| {
        records(seen).iter().any(|(_, _, r)| matches!(r, Reproduction::Born { .. }))
    });
    let (born_tick, _, born) = records(&seen)
        .into_iter()
        .find(|(_, _, r)| matches!(r, Reproduction::Born { .. }))
        .expect("a birth");
    let Reproduction::Born { key: born_key, child, child_structure, child_reserve, child_energy, birth_heat } = born
    else {
        panic!("{born:?}")
    };
    assert_eq!(born_key, key, "the birth closes the gestation the funding opened");
    assert_eq!((child_structure, child_reserve, child_energy), (escrow_structure, escrow_reserve, escrow_energy));
    assert!((birth_heat - e_r * escrow_structure).abs() < 1e-15, "the structure gave up its reserve energy");
    // The child really is that animal.
    let o = world.state.organisms.get(child).expect("the child is alive");
    assert_eq!((o.structure, o.reserve, o.energy), (child_structure, child_reserve, child_energy));
    assert_eq!(o.born_tick, born_tick);

    // One-for-one with the existing identity records, which are unchanged.
    let offspring: Vec<_> = seen
        .iter()
        .filter(|e| matches!(e, HunterEvent::Offspring { child: c, .. } if *c == child))
        .collect();
    assert_eq!(offspring.len(), 1, "{offspring:?}");
    let births: Vec<_> = life
        .iter()
        .filter(|e| matches!(e, LifeEvent::Birth { id, .. } if *id == child))
        .collect();
    assert_eq!(births.len(), 1, "{births:?}");
    assert_eq!(births[0].tick(), born_tick);
    // And exactly one transaction closed this key.
    let closings = records(&seen)
        .into_iter()
        .filter(|(_, _, r)| {
            r.key() == Some(key)
                && matches!(r, Reproduction::Born { .. } | Reproduction::Refunded { .. } | Reproduction::Miscarried { .. })
        })
        .count();
    assert_eq!(closings, 1, "a gestation ends exactly once");
}

// ---------------------------------------------------------------- refused at the cap

#[test]
fn a_birth_the_cap_refuses_reports_the_refund_and_not_a_loss() {
    let mut cfg = WorldConfig::default();
    // Room for the hunter, one bystander and one more arrival: the escrow starts with room and
    // the world fills before the birth is due.
    cfg.capacity.max_organisms = 3;
    let mut world = quiet_config_world(cfg);
    let profile = breeder(&world);
    let parent = world.start_hunter_trial(profile, target_of(SPOT)).expect("started").id;
    feed_to_full(&mut world, parent);
    place_bystander(&mut world, SurfacePoint::new(Face::Back, 8.0, 8.0));

    let (seen, _) = run_for(&mut world, 60, |seen| records(seen).iter().any(|(_, _, r)| is_funded(r)));
    let (_, _, funded) = records(&seen).into_iter().find(|(_, _, r)| is_funded(r)).expect("a funding");
    let key = funded.key().expect("a transaction");

    // Fill the last slot before the birth is due.
    place_bystander(&mut world, SurfacePoint::new(Face::Left, 8.0, 8.0));
    assert_eq!(world.state.organisms.len(), 3, "the world is at its cap");

    let (seen, life) = run_for(&mut world, 200, |seen| {
        records(seen).iter().any(|(_, _, r)| matches!(r, Reproduction::Refunded { .. }))
    });
    let (tick, hunter, refund) = records(&seen)
        .into_iter()
        .find(|(_, _, r)| matches!(r, Reproduction::Refunded { .. }))
        .expect("a refund");
    let Reproduction::Refunded {
        key: refund_key,
        refunded_structure,
        refunded_reserve,
        refunded_energy,
        parent_reserve_before,
        parent_reserve_after,
        parent_energy_before,
        parent_energy_after,
    } = refund
    else {
        panic!("{refund:?}")
    };
    assert_eq!(hunter, parent);
    assert_eq!(refund_key, key, "the refund closes the gestation the funding opened");
    assert_eq!(tick, world.tick());

    // Everything went back, and nothing was burned: no heat term exists on a refund.
    let Reproduction::Funded { escrow_structure, escrow_reserve, escrow_energy, .. } = funded else {
        panic!("{funded:?}")
    };
    assert_eq!((refunded_structure, refunded_reserve, refunded_energy), (escrow_structure, escrow_reserve, escrow_energy));
    assert!(
        (parent_reserve_after - parent_reserve_before - (refunded_structure + refunded_reserve)).abs() < 1e-15,
        "the material did not come back"
    );
    assert!((parent_energy_after - parent_energy_before - refunded_energy).abs() < 1e-15);
    let o = world.state.organisms.get(parent).expect("alive");
    assert!(o.escrow.is_none(), "the escrow was returned, not still held");
    assert_eq!(o.reserve, parent_reserve_after);
    assert_eq!(o.energy, parent_energy_after);

    // A refund is not a birth and not a miscarriage, and no child was invented.
    assert!(!records(&seen).iter().any(|(_, _, r)| matches!(r, Reproduction::Born { .. } | Reproduction::Miscarried { .. })));
    assert!(!seen.iter().any(|e| matches!(e, HunterEvent::Offspring { .. })));
    assert!(!life.iter().any(|e| matches!(e, LifeEvent::Birth { .. })));
    assert_eq!(world.hunters().hunter_births_total, 0);
}

/// A cap that refuses the **funding** is a different fact from a cap that refuses a due birth:
/// no escrow was ever created, so there is no transaction and nothing to close.
#[test]
fn funding_blocked_by_the_cap_is_reported_as_a_non_transaction() {
    let mut cfg = WorldConfig::default();
    cfg.capacity.max_organisms = 2;
    let mut world = quiet_config_world(cfg);
    let profile = breeder(&world);
    let parent = world.start_hunter_trial(profile, target_of(SPOT)).expect("started").id;
    feed_to_full(&mut world, parent);
    place_bystander(&mut world, SurfacePoint::new(Face::Back, 8.0, 8.0));
    let rejections = world.state.cap_rejections_total;

    let (seen, life) = run_for(&mut world, 60, |seen| {
        records(seen).iter().any(|(_, _, r)| matches!(r, Reproduction::NotFunded { .. }))
    });
    let (tick, hunter, blocked) = records(&seen)
        .into_iter()
        .find(|(_, _, r)| matches!(r, Reproduction::NotFunded { .. }))
        .expect("a blocked funding");
    assert_eq!(hunter, parent);
    assert_eq!(tick, world.tick());
    assert_eq!(blocked, Reproduction::NotFunded { parent, reason: FundingBlocked::Cap });
    assert_eq!(blocked.key(), None, "a non-transaction has no escrow key");
    assert_eq!(blocked.parent(), parent);

    // Nothing moved: no escrow, no funding record, no child, and the world's own counter agrees.
    assert!(world.state.organisms.get(parent).expect("alive").escrow.is_none());
    assert!(!records(&seen).iter().any(|(_, _, r)| is_funded(r)), "an escrow was funded anyway");
    assert!(!life.iter().any(|e| matches!(e, LifeEvent::Birth { .. })));
    assert!(world.state.cap_rejections_total > rejections);
}

// ---------------------------------------------------------------- lost with its parent

#[test]
fn a_parent_that_dies_gestating_reports_the_escrow_it_exported_and_nothing_else() {
    let mut world = quiet_world();
    let profile = breeder(&world);
    let parent = world.start_hunter_trial(profile, target_of(SPOT)).expect("started").id;
    feed_to_full(&mut world, parent);
    let e_r = world.config().organism.reserve_energy_density;
    let cap = world.config().detritus.energy_cap;

    let (seen, _) = run_for(&mut world, 60, |seen| records(seen).iter().any(|(_, _, r)| is_funded(r)));
    let (_, _, funded) = records(&seen).into_iter().find(|(_, _, r)| is_funded(r)).expect("a funding");
    let key = funded.key().expect("a transaction");
    let Reproduction::Funded { escrow_structure, escrow_reserve, escrow_energy, .. } = funded else {
        panic!("{funded:?}")
    };

    // Starve it while it gestates.
    let (body_material, body_energy) = {
        let o = world.state.organisms.get_mut(parent).expect("alive");
        let material = o.reserve;
        o.energy = 0.0;
        o.reserve = 0.0;
        world.state.external_material_in -= material;
        (o.structure, 0.0)
    };
    let detritus_before: f64 = world.state.fields.d.iter().sum();
    let detritus_energy_before: f64 = world.state.fields.de.iter().sum();

    let (seen, life) = run_for(&mut world, 60, |seen| {
        records(seen).iter().any(|(_, _, r)| matches!(r, Reproduction::Miscarried { .. }))
    });
    let (tick, hunter, lost) = records(&seen)
        .into_iter()
        .find(|(_, _, r)| matches!(r, Reproduction::Miscarried { .. }))
        .expect("a miscarriage");
    let Reproduction::Miscarried { key: lost_key, cause, material, energy, energy_stored, energy_heat } = lost
    else {
        panic!("{lost:?}")
    };
    assert_eq!(hunter, parent);
    assert_eq!(lost_key, key, "the loss closes the gestation the funding opened");
    assert_eq!(cause, DeathCause::Starvation);
    assert_eq!(tick, world.tick());

    // The escrow's own terms, and only those: the corpse is not counted as escrow.
    assert!((material - (escrow_structure + escrow_reserve)).abs() < 1e-15, "{material}");
    assert!((energy - (e_r * material + escrow_energy)).abs() < 1e-12, "{energy}");
    assert!((energy_stored + energy_heat - energy).abs() < 1e-12, "the exported energy is unaccounted");
    assert!(energy_stored <= cap * material + 1e-12, "the detritus cap was exceeded");
    assert!(energy_heat >= 0.0);
    assert!(material < body_material, "the escrow is not the whole corpse");
    let _ = body_energy;

    // The body and the escrow both landed, and the litter gained exactly their sum: in this
    // quiet world nothing else moves detritus.
    let detritus_after: f64 = world.state.fields.d.iter().sum();
    assert!(
        (detritus_after - detritus_before - (body_material + material)).abs() < 1e-12,
        "litter moved by {} against body {body_material} + escrow {material}",
        detritus_after - detritus_before
    );
    let detritus_energy_after: f64 = world.state.fields.de.iter().sum();
    assert!(detritus_energy_after - detritus_energy_before >= energy_stored - 1e-12);

    // The hunter's own death record is a separate fact, with its own gut terms.
    let death = seen
        .iter()
        .find(|e| matches!(e, HunterEvent::Death { .. }))
        .expect("the member's death record");
    match death {
        HunterEvent::Death { id, gut_material, gut_energy, .. } => {
            assert_eq!(*id, parent);
            assert_eq!((*gut_material, *gut_energy), (0.0, 0.0), "it was carrying no meal");
        }
        other => panic!("{other:?}"),
    }
    assert!(!seen.iter().any(|e| matches!(e, HunterEvent::Offspring { .. })), "no child was born");
    assert!(life.iter().any(|e| matches!(e, LifeEvent::Death { id, .. } if *id == parent)));
    assert_eq!(world.hunters().hunter_births_total, 0);
}

/// An escrow can be funded and lost inside one tick. Both facts are emitted, for the same key,
/// at the same tick: a reader that only saw the loss would count a gestation it never saw begin.
#[test]
fn a_gestation_funded_and_lost_in_one_tick_emits_both_facts() {
    let mut cfg = WorldConfig::default();
    // Two seconds of life: the founder reaches its age limit at tick 40.
    cfg.organism.max_age_seconds = 2.0;
    let mut world = quiet_config_world(cfg);
    let profile = breeder(&world);
    let parent = world.start_hunter_trial(profile, target_of(SPOT)).expect("started").id;
    feed_to_full(&mut world, parent);
    // Hold the gate shut until the tick it dies on, so the funding and the death land together.
    world.state.hunters.member_mut(parent).expect("a member").next_reproduction_tick = 40;

    let (seen, life) = run_for(&mut world, 80, |seen| {
        records(seen).iter().any(|(_, _, r)| matches!(r, Reproduction::Miscarried { .. }))
    });
    let all = records(&seen);
    let (funded_tick, _, funded) = all.iter().copied().find(|(_, _, r)| is_funded(r)).expect("a funding");
    let (lost_tick, _, lost) = all
        .iter()
        .copied()
        .find(|(_, _, r)| matches!(r, Reproduction::Miscarried { .. }))
        .expect("a loss");
    assert_eq!(funded_tick, lost_tick, "this fixture needs both in one tick");
    assert_eq!(funded.key(), lost.key(), "and they must be the same gestation");
    let Reproduction::Miscarried { cause, material, .. } = lost else { panic!("{lost:?}") };
    assert_eq!(cause, DeathCause::Age, "it reached its age limit on the tick it paid");
    let Reproduction::Funded { escrow_structure, escrow_reserve, .. } = funded else { panic!("{funded:?}") };
    assert!((material - (escrow_structure + escrow_reserve)).abs() < 1e-15);
    assert!(world.state.organisms.get(parent).is_none(), "the parent died");
    assert!(life.iter().any(|e| matches!(e, LifeEvent::Death { id, .. } if *id == parent)));
    assert!(!seen.iter().any(|e| matches!(e, HunterEvent::Offspring { .. })));
}

// ---------------------------------------------------------------- replay and accessors

/// A gestation checkpointed mid-flight resumes into the **same records**, not merely the same
/// world: the transaction stream a deferred observer reads is reproducible across a restart.
#[test]
fn a_saved_gestation_replays_the_same_transaction_stream() {
    let mut world = quiet_world();
    let profile = breeder(&world);
    let parent = world.start_hunter_trial(profile, target_of(SPOT)).expect("started").id;
    feed_to_full(&mut world, parent);

    // Run into the gestation, then checkpoint with the escrow held.
    run_for(&mut world, 60, |seen| records(seen).iter().any(|(_, _, r)| is_funded(r)));
    assert!(world.state.organisms.get(parent).expect("alive").escrow.is_some(), "mid-gestation");
    let bytes = encode_snapshot(&world.state, "gestation-replay");
    let (_, state) = decode_snapshot(&bytes).expect("round trip");
    let mut reloaded = World::from_state(state).expect("valid");
    assert_eq!(state_hash(&reloaded.state), state_hash(&world.state));

    let (mut original, mut resumed) = (Vec::new(), Vec::new());
    for _ in 0..300 {
        world.step();
        reloaded.step();
        original.extend(world.drain_hunter_events());
        resumed.extend(reloaded.drain_hunter_events());
        assert_eq!(state_hash(&reloaded.state), state_hash(&world.state), "diverged at {}", world.tick());
    }
    assert_eq!(resumed, original, "the resumed run reported different transactions");
    let closings: Vec<_> = records(&original)
        .into_iter()
        .filter(|(_, _, r)| matches!(r, Reproduction::Born { .. }))
        .collect();
    assert_eq!(closings.len(), 1, "the saved gestation must have ended in a birth: {closings:?}");
    assert_eq!(world.hunters().hunter_births_total, 1);
}

/// `tick()` and `hunter()` answer for every variant, so an observer does not carry its own
/// exhaustive match just to bucket a record by tick or by member.
#[test]
fn every_hunter_event_reports_its_tick_and_its_member() {
    let mut world = quiet_world();
    let profile = breeder(&world);
    let parent = world.start_hunter_trial(profile, target_of(SPOT)).expect("started").id;
    feed_to_full(&mut world, parent);

    let mut seen = Vec::new();
    let mut ticks = Vec::new();
    for _ in 0..400 {
        world.step();
        for event in world.drain_hunter_events() {
            // The accessor agrees with the variant's own field, whichever variant it is.
            let by_hand = match &event {
                HunterEvent::Attempt { tick, hunter, .. } => (*tick, *hunter),
                HunterEvent::Capture { tick, hunter, .. } => (*tick, *hunter),
                HunterEvent::Offspring { tick, parent, .. } => (*tick, *parent),
                HunterEvent::Reproduction { tick, hunter, .. } => (*tick, *hunter),
                HunterEvent::Death { tick, id, .. } => (*tick, *id),
            };
            assert_eq!((event.tick(), event.hunter()), by_hand, "{event:?}");
            assert_eq!(event.tick(), world.tick(), "a record belongs to the tick that closed it");
            ticks.push(event.tick());
            seen.push(event);
        }
    }
    assert!(seen.len() >= 3, "this fixture should produce a funding, a birth and more: {seen:?}");
    assert!(ticks.windows(2).all(|w| w[0] <= w[1]), "the stream is in commit order");
    // The variants this fixture is expected to exercise are all present.
    assert!(seen.iter().any(|e| matches!(e, HunterEvent::Offspring { .. })));
    assert!(records(&seen).iter().any(|(_, _, r)| is_funded(r)));
    assert!(records(&seen).iter().any(|(_, _, r)| matches!(r, Reproduction::Born { .. })));
    // Every record is about a member of the lineage, by full ID.
    for event in &seen {
        let id = event.hunter();
        assert!(id == parent || world.hunters().contains(id) || world.state.organisms.get(id).is_some(), "{event:?}");
    }
}
