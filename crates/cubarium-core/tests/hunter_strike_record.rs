//! The **per-attempt strike record**: what one paid attempt was aimed at, and what it hit.
//!
//! The apex eligibility audit (`design/7_Research/ecology-v1-apex-eligibility-2026-09-16.md`)
//! localised an introduced apex's starvation to intake, and intake to *reach*: 402 of 449 paid
//! attempts ended `OutOfReach`. It could not say why, because the only geometry the world
//! published was the settlement's, by which time both bodies had moved twice.
//! `crate::hunter::StrikeRecord` is the missing measurement and these tests fix its contract
//! (`design/handoffs/ecology-v1-apex-reach-opus-2026-09-16.md`, deliverable 1):
//!
//! - a hand-built attempt on a stationary prey **inside** the grasp captures, and all three
//!   frames say it was in reach the whole time;
//! - the same attempt with the prey placed a stated distance **beyond** the grasp resolves
//!   `OutOfReach`, and every frame's separation is the one the body geometry predicts, to
//!   floating point;
//! - a windup that never pays for a burst leaves no record at all;
//! - recording is **inert**: a two-apex world hashes identically, tick for tick, over 9,000
//!   ticks with the recorder on and off.
//!
//! Every separation asserted here is recomputed from `capture_offset_body`, `capture_reach_px`
//! and the prey's own extent, never read back from the record it is checking.

use cubarium_core::genome::{Genome, decode};
use cubarium_core::hunter::{
    AttemptOutcome, FixedHunterProfile, HunterEvent, HunterTarget, StrikeClass,
};
use cubarium_core::ids::OrganismId;
use cubarium_core::organism::{Mode, Organism, Origin};
use cubarium_core::rng::Counter;
use cubarium_core::snapshot::state_hash;
use cubarium_core::{DT, World, WorldConfig};
use cubarium_surface::{Face, SurfacePoint, Vec2, travel};

// ---------------------------------------------------------------- fixtures

/// An empty, quiet world: no founders, no weather motion, no rain. Only what a test places.
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
    HunterTarget { face: pos.face.index() as u8, u: pos.u, v: pos.v }
}

/// The chart offset of a body-local offset, in the same basis `stamp_rig` uses: `+x` along the
/// heading, `+y` its clockwise side. Transcribed here rather than imported, so a test places
/// prey where the *renderer* would draw the claws.
fn body_offset(heading: Vec2, offset: Vec2, scale: f64) -> Vec2 {
    let h = heading.normalized().expect("a heading");
    let side = Vec2::new(-h.y, h.x);
    (h * offset.x + side * offset.y) * scale
}

/// Place a frozen prey by hand and book its material as admitted from outside, so the closed
/// box still reads zero. `speed_max = 0` keeps the geometry of a test where the test put it.
fn place_frozen_prey(world: &mut World, pos: SurfacePoint, s: f64, r: f64, e: f64) -> OrganismId {
    let cfg = world.config().clone();
    let mut genome = Genome::founder(0.5, &cfg.drives);
    genome.size = 0.5;
    genome.speed = 0.3;
    genome.clamp();
    let mut phenotype = decode(&genome, &cfg.organism);
    phenotype.speed_max = 0.0;
    phenotype.structure_adult = s;
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

/// Point a hunter at a bearing and make it hungry enough to hunt, booking the reserve it gave
/// up so the material box stays closed.
fn aim(world: &mut World, id: OrganismId, heading: Vec2, reserve: f64) {
    let o = world.state.organisms.get_mut(id).expect("alive");
    let before = o.reserve;
    o.heading = heading;
    o.reserve = reserve;
    world.state.external_material_in += reserve - before;
}

/// Freeze the hunter itself, so a staged attempt measures the record and not the pursuit: with
/// `speed_max = 0` the motor envelope `|v| + r·|ω| ≤ speed_cap` is zero unless a burst lifts
/// it, and a burst is only pushed for a prey **outside** the reach envelope.
fn freeze_hunter(world: &mut World, id: OrganismId) {
    let o = world.state.organisms.get_mut(id).expect("alive");
    o.phenotype.speed_max = 0.0;
}

/// A hunter at a known spot, facing `+u`, hungry, with the recorder on.
fn staged(profile: &FixedHunterProfile) -> (World, OrganismId, SurfacePoint) {
    let mut world = empty_world();
    let spot = SurfacePoint::new(Face::Front, 20.0, 32.0);
    let receipt = world
        .start_hunter_trial(profile.clone(), target_of(spot))
        .expect("the trial starts");
    aim(&mut world, receipt.id, Vec2::new(1.0, 0.0), 1.0);
    world.record_strike_attempts(true);
    (world, receipt.id, spot)
}

/// Step until one strike record is drained, or panic with what the hunter was doing.
fn run_for_record(
    world: &mut World,
    ticks: u64,
) -> (cubarium_core::hunter::StrikeRecord, Vec<HunterEvent>) {
    let mut events = Vec::new();
    for _ in 0..ticks {
        world.step();
        events.extend(world.drain_hunter_events());
        let (records, dropped, unreadable) = world.drain_strike_records();
        assert_eq!((dropped, unreadable), (0, 0), "no record was lost or unreadable");
        if let Some(first) = records.first() {
            assert_eq!(records.len(), 1, "one staged hunter resolves one attempt at a time");
            return (*first, events);
        }
    }
    panic!(
        "no attempt resolved in {ticks} ticks: phases {:?}",
        world.hunters().members.iter().map(|m| (m.phase, m.target)).collect::<Vec<_>>()
    );
}

fn close(a: f64, b: f64, eps: f64, what: &str) {
    assert!((a - b).abs() <= eps, "{what}: {a} vs {b} (|Δ| = {})", (a - b).abs());
}

// ---------------------------------------------------------------- the two hand-built attempts

/// The prey sits exactly in the grasp and never moves. The attempt captures, and the record
/// says the separation was zero at the gesture, at the burst and at the settlement.
#[test]
fn a_stationary_prey_in_the_grasp_captures_and_all_three_frames_say_it_was_in_reach() {
    let base = trial(&empty_world());
    let profile = certain(base.clone());
    let (mut world, hunter, spot) = staged(&profile);
    // In the grasp, not in front of the thorax: an adult's claws close 13.28 px ahead and
    // 1.16 px to its clockwise side.
    let grasp = travel(spot, body_offset(Vec2::new(1.0, 0.0), profile.capture_offset_body, 1.0)).end;
    let prey = place_frozen_prey(&mut world, grasp, 0.5, 0.3, 0.4);
    freeze_hunter(&mut world, hunter);
    let extent = world.state.organisms.get(prey).expect("placed").phenotype.extent;
    let tolerance = profile.capture_reach_px + extent;

    let (record, events) = run_for_record(&mut world, 400);

    assert_eq!(record.outcome, AttemptOutcome::Captured);
    assert_eq!(record.class, StrikeClass::ResolvedInReach);
    assert_eq!(record.hunter, hunter);
    close(record.energy_paid, profile.strike_energy_cost, 0.0, "the strike cost as charged");

    let intent = record.intent.expect("the gesture was recorded");
    let strike = record.strike.expect("the burst was recorded");
    assert_eq!(intent.target, Some(prey));
    assert_eq!(strike.target, Some(prey));
    assert_eq!(record.resolution.target, Some(prey));
    for (name, frame) in [("intent", intent), ("strike", strike), ("resolution", record.resolution)]
    {
        assert!(frame.in_reach, "{name}: a prey in the grasp is in reach");
        assert!(frame.grasp_mapped, "{name}: the grasp is on the surface");
        close(frame.effector_distance.expect(name), 0.0, 1e-9, name);
        close(frame.tolerance.expect(name), tolerance, 1e-12, name);
        close(frame.scale, 1.0, 1e-12, name);
        close(
            frame.advertised_reach,
            profile.capture_offset_body.length() + profile.capture_reach_px,
            1e-12,
            name,
        );
    }
    // Neither body moved: the record's realised speeds are the measurement, not an assumption.
    for s in [
        record.target_speed_windup,
        record.target_speed_strike,
        record.hunter_speed_windup,
        record.hunter_speed_strike,
    ] {
        close(s.expect("both ends of both phases were recorded"), 0.0, 1e-9, "a frozen body");
    }
    assert!(!record.target_changed && !record.target_missing_at_resolution);
    assert!(!record.target_crossed_face && !record.hunter_crossed_face);

    // The three frames are exactly `windup_seconds` and `strike_seconds` apart, so a realised
    // speed is a displacement over a duration this record knows.
    let windup_ticks = (profile.windup_seconds / DT).round() as u64;
    let strike_ticks = (profile.strike_seconds / DT).round() as u64;
    assert_eq!(strike.tick - intent.tick, windup_ticks);
    assert_eq!(record.resolution.tick - strike.tick, strike_ticks);

    // The record and the world's own event reconcile on the attempt key.
    let attempt = events
        .iter()
        .find_map(|e| match e {
            HunterEvent::Attempt { outcome, attack_counter, .. } => Some((*outcome, *attack_counter)),
            _ => None,
        })
        .expect("the world published the attempt");
    assert_eq!(attempt, (AttemptOutcome::Captured, Some(record.attack_counter)));
}

/// The same attempt with the prey placed a stated distance beyond the grasp, to the claw's
/// clockwise side so the lunge is never pushed (a burst is only requested for a prey outside
/// the reach envelope, and a prey abreast of the claws is inside it). Nothing moves, so the
/// separation at every frame is the one the geometry predicts and the outcome is the reach
/// refusal the audit counted 402 of.
#[test]
fn a_prey_just_beyond_the_grasp_records_out_of_reach_with_the_separation_the_geometry_predicts() {
    let profile = certain(trial(&empty_world()));
    let (mut world, hunter, spot) = staged(&profile);
    // Place it once at the origin of the claw so its extent is known, then move it out.
    let grasp = travel(spot, body_offset(Vec2::new(1.0, 0.0), profile.capture_offset_body, 1.0)).end;
    let prey = place_frozen_prey(&mut world, grasp, 0.5, 0.3, 0.4);
    let extent = world.state.organisms.get(prey).expect("placed").phenotype.extent;
    let tolerance = profile.capture_reach_px + extent;
    // A stated gap past the contact tolerance: inside the admission window the stalk uses
    // (`tolerance + strike_speed_px_s · strike_seconds`), outside the grasp itself.
    let gap = 0.75;
    let beyond = travel(
        spot,
        body_offset(
            Vec2::new(1.0, 0.0),
            profile.capture_offset_body + Vec2::new(0.0, tolerance + gap),
            1.0,
        ),
    )
    .end;
    world.state.organisms.get_mut(prey).expect("placed").pos = beyond.canonicalize();
    freeze_hunter(&mut world, hunter);

    let (record, _) = run_for_record(&mut world, 400);

    assert_eq!(record.outcome, AttemptOutcome::OutOfReach);
    assert_eq!(record.class, StrikeClass::BeganOutOfReach);
    let intent = record.intent.expect("the gesture was recorded");
    let strike = record.strike.expect("the burst was recorded");
    for (name, frame) in [("intent", intent), ("strike", strike), ("resolution", record.resolution)]
    {
        assert!(!frame.in_reach, "{name}: a prey past the tolerance is not in reach");
        // The separation the body geometry predicts: the prey sits `tolerance + gap` from the
        // scaled grasp centre, along the claw's own clockwise side.
        close(frame.effector_distance.expect(name), tolerance + gap, 1e-9, name);
        close(frame.tolerance.expect(name), tolerance, 1e-12, name);
        close(frame.overshoot().expect(name), gap, 1e-9, name);
    }
    // Neither body moved, so the attempt neither closed nor lost ground.
    close(record.separation_change_over_strike().expect("both frames"), 0.0, 1e-9, "over the burst");
    close(record.separation_change_total().expect("both frames"), 0.0, 1e-9, "over the attempt");
    assert!(!record.target_changed && !record.target_missing_at_resolution);
}

/// A gesture the hunter cannot pay to finish is not an attempt: the world refuses it before
/// payment, and the recorder publishes nothing.
#[test]
fn a_windup_that_never_pays_for_a_burst_leaves_no_record() {
    let profile = certain(trial(&empty_world()));
    let (mut world, hunter, spot) = staged(&profile);
    let grasp = travel(spot, body_offset(Vec2::new(1.0, 0.0), profile.capture_offset_body, 1.0)).end;
    place_frozen_prey(&mut world, grasp, 0.5, 0.3, 0.4);
    freeze_hunter(&mut world, hunter);

    let mut refusals = 0;
    for _ in 0..400 {
        // Held below the strike cost every tick, so the windup always expires unaffordable.
        world.state.organisms.get_mut(hunter).expect("alive").energy = 0.0;
        world.step();
        for e in world.drain_hunter_events() {
            if let HunterEvent::Attempt { outcome: AttemptOutcome::Unaffordable, .. } = e {
                refusals += 1;
            }
        }
        let (records, dropped, unreadable) = world.drain_strike_records();
        assert!(records.is_empty(), "an unpaid refusal is not a strike record");
        assert_eq!((dropped, unreadable), (0, 0));
    }
    assert!(refusals > 0, "the fixture produced at least one unaffordable refusal to not record");
}

// ---------------------------------------------------------------- inertness

fn two_apex_world(record: bool) -> World {
    let mut world = World::new(WorldConfig::default()).expect("the default world is valid");
    let profile = FixedHunterProfile::lanternjaw_trial(world.config());
    for _ in 0..1_000 {
        world.step();
    }
    world
        .introduce_hunters(
            profile,
            &[
                HunterTarget { face: 1, u: 23.0, v: 31.0 },
                HunterTarget { face: 4, u: 55.0, v: 3.0 },
            ],
        )
        .expect("two adults are placed");
    world.record_strike_attempts(record);
    world
}

/// Recording changes nothing the world computes. The comparison is the state hash at every
/// 500-tick boundary of 9,000 ticks of a two-apex world, and the run is only evidence if the
/// recorder actually filled: the record count is asserted non-zero.
#[test]
fn the_recorder_is_inert_by_state_hash_over_nine_thousand_ticks_of_a_two_apex_world() {
    let mut off = two_apex_world(false);
    let mut on = two_apex_world(true);
    assert!(!off.records_strike_attempts() && on.records_strike_attempts());
    let mut recorded = 0usize;
    let mut outcomes = 0usize;
    for tick in 1..=9_000u64 {
        off.step();
        on.step();
        off.drain_events();
        on.drain_events();
        off.drain_hunter_events();
        for e in on.drain_hunter_events() {
            if matches!(e, HunterEvent::Attempt { attack_counter: Some(_), .. }) {
                outcomes += 1;
            }
        }
        let (records, dropped, unreadable) = on.drain_strike_records();
        assert_eq!((dropped, unreadable), (0, 0), "tick {tick}: nothing was lost");
        recorded += records.len();
        if tick % 500 == 0 {
            assert_eq!(
                state_hash(&off.state),
                state_hash(&on.state),
                "the state hash diverged at tick {tick} with the recorder on"
            );
        }
    }
    assert_eq!(
        state_hash(&off.state),
        state_hash(&on.state),
        "the final state hash differs with the recorder on"
    );
    assert!(recorded > 0, "the inertness run recorded nothing, so it is not evidence");
    assert_eq!(recorded, outcomes, "one record per paid attempt the world published");
}

// ---------------------------------------------------------------- the four-way classification

/// A frame built by hand, so the classification is tested on stated geometry rather than on
/// whatever a simulated attempt happened to produce.
fn frame(
    tick: u64,
    hunter: SurfacePoint,
    target: Option<(OrganismId, SurfacePoint, f64)>,
) -> cubarium_core::hunter::StrikeFrame {
    let tolerance = 2.6;
    cubarium_core::hunter::StrikeFrame {
        tick,
        hunter_pos: hunter,
        hunter_heading: Vec2::new(1.0, 0.0),
        scale: 1.0,
        advertised_reach: 14.83,
        target: target.map(|(id, _, _)| id),
        target_pos: target.map(|(_, p, _)| p),
        target_heading: target.map(|_| Vec2::new(1.0, 0.0)),
        target_extent: target.map(|_| 1.1),
        root_distance: target.map(|(_, _, d)| d),
        effector_distance: target.map(|(_, _, d)| d),
        tolerance: target.map(|_| tolerance),
        in_reach: target.is_some_and(|(_, _, d)| d <= tolerance),
        grasp_mapped: true,
    }
}

/// Run one hand-built attempt through the recorder and return the class it filed.
fn class_of(
    intent: f64,
    strike: f64,
    resolution: Option<f64>,
    target_at_resolution: Option<OrganismId>,
) -> StrikeClass {
    let mut recorder = cubarium_core::hunter::StrikeRecorder::default();
    recorder.set_enabled(true);
    let hunter = OrganismId { slot: 0, generation: 1 };
    let prey = OrganismId { slot: 1, generation: 1 };
    let spot = SurfacePoint::new(Face::Front, 20.0, 32.0);
    let at = |d: f64, id: Option<OrganismId>| id.map(|id| (id, spot, d));
    recorder.open_intent(hunter, frame(0, spot, at(intent, Some(prey))));
    recorder.begin_strike(hunter, 7, frame(12, spot, at(strike, Some(prey))));
    recorder.close(
        hunter,
        7,
        frame(32, spot, resolution.and_then(|d| at(d, target_at_resolution))),
        AttemptOutcome::OutOfReach,
        0.08,
        0.6,
        1.0,
        |_, _| Some(0.0),
    );
    let (records, dropped, unreadable) = recorder.drain();
    assert_eq!((dropped, unreadable), (0, 0));
    assert_eq!(records.len(), 1, "one closed attempt, one record");
    records[0].class
}

/// The five classes, each from the geometry that defines it, in the priority the recorder
/// documents: a lost target first, then a strike that arrived, then an in-reach start that
/// resolved out, then a growing gap, then a gap that was simply never closed.
#[test]
fn every_class_is_decided_by_the_geometry_that_defines_it() {
    let prey = OrganismId { slot: 1, generation: 1 };
    let other = OrganismId { slot: 2, generation: 1 };
    // In the grasp at settlement: the strike arrived, whatever the roll then did.
    assert_eq!(class_of(10.0, 6.0, Some(1.0), Some(prey)), StrikeClass::ResolvedInReach);
    // Began inside the grasp and resolved outside it: cadence or resolution.
    assert_eq!(class_of(1.0, 4.0, Some(9.0), Some(prey)), StrikeClass::BeganInReachResolvedOut);
    // Began outside and the gap grew across the paid burst: the escape envelope.
    assert_eq!(class_of(10.0, 9.0, Some(11.0), Some(prey)), StrikeClass::PreyOutran);
    // Began outside and the gap did not grow: selection or pursuit. A tie counts as not grown.
    assert_eq!(class_of(10.0, 9.0, Some(9.0), Some(prey)), StrikeClass::BeganOutOfReach);
    assert_eq!(class_of(10.0, 9.0, Some(4.0), Some(prey)), StrikeClass::BeganOutOfReach);
    // A different body at settlement, or none at all, is a lost target and outranks the rest —
    // including a resolution that would otherwise have read as in reach.
    assert_eq!(class_of(1.0, 1.0, Some(1.0), Some(other)), StrikeClass::TargetLost);
    assert_eq!(class_of(1.0, 1.0, None, None), StrikeClass::TargetLost);
}

/// An attempt already in flight when recording is switched on has no intent frame, and is
/// filed as unreadable rather than guessed at.
#[test]
fn an_attempt_with_no_intent_frame_is_unreadable_and_counted() {
    let mut recorder = cubarium_core::hunter::StrikeRecorder::default();
    recorder.set_enabled(true);
    let hunter = OrganismId { slot: 0, generation: 1 };
    let prey = OrganismId { slot: 1, generation: 1 };
    let spot = SurfacePoint::new(Face::Front, 20.0, 32.0);
    recorder.close(
        hunter,
        7,
        frame(32, spot, Some((prey, spot, 9.0))),
        AttemptOutcome::OutOfReach,
        0.08,
        0.6,
        1.0,
        |_, _| Some(0.0),
    );
    let (records, dropped, unreadable) = recorder.drain();
    assert_eq!((dropped, unreadable), (0, 1));
    assert_eq!(records[0].class, StrikeClass::Unreadable);
}
