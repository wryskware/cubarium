//! The **pursuit stopping predicate**, paired: the forward half-space that shipped until
//! 2026-09-16 against the reach envelope its own comment names, which is the shipped rule now
//! (`crates/cubarium-core/tests/pursuit_predicate_adoption.rs`).
//!
//! The strike record (`design/7_Research/ecology-v1-apex-reach-2026-09-16.md`) measured the
//! rule in `crate::world::step`'s hunt-intent pass — `body.x < capture_offset_body.x ·
//! scale + tolerance` — true at the burst's start on 408 of 449 paid attempts, which both drops
//! the member to its `rest_effort` and suppresses the burst it has just paid 0.08 e for. The
//! comment above that line, and every other use of "inside" in the file, mean
//! `ContactMeasure::in_contact()`. These tests fix the contract of the opt-in switch that lets
//! one paired experiment run both readings
//! (`design/handoffs/ecology-v1-apex-predicate-opus-2026-09-16.md`, deliverable 1):
//!
//! - the two rules **are** the two written predicates, and they differ exactly where the note
//!   says they do — for a prey short of the claws and out of reach;
//! - naming either rule is **not vacuous**: the same world diverges, and every record it makes
//!   carries the rule it was made under;
//! - a hand-built prey **ahead but outside reach** no longer satisfies the hold under the
//!   variant, and the burst is delivered — measured as the hunter's own realised speed over its
//!   paid burst, not as a re-reading of the predicate;
//! - a hand-built prey **inside reach** still holds under both.
//!
//! Every geometry asserted here is recomputed from `capture_offset_body`, `capture_reach_px`
//! and the prey's own extent, never read back from the record it is checking.

use cubarium_core::genome::{Genome, decode};
use cubarium_core::hunter::{
    AttemptOutcome, ContactGeometry, ContactMeasure, FixedHunterProfile, HunterTarget,
    PursuitStop,
};
use cubarium_core::ids::OrganismId;
use cubarium_core::organism::{Mode, Organism, Origin};
use cubarium_core::rng::Counter;
use cubarium_core::snapshot::state_hash;
use cubarium_core::{World, WorldConfig};
use cubarium_surface::{Face, SurfacePoint, Topology, Vec2, travel};

// ---------------------------------------------------------------- fixtures
// The same fixtures `hunter_strike_record.rs` stages an attempt with, so the two files measure
// the same hand-built geometry and a reader can compare them line for line.

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
/// heading, `+y` its clockwise side.
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

fn aim(world: &mut World, id: OrganismId, heading: Vec2, reserve: f64) {
    let o = world.state.organisms.get_mut(id).expect("alive");
    let before = o.reserve;
    o.heading = heading;
    o.reserve = reserve;
    world.state.external_material_in += reserve - before;
}

/// A hunter at a known spot, facing `+u`, hungry, with the recorder on and one named rule.
///
/// The hunter is deliberately **not** frozen: what these tests measure is whether its own paid
/// burst moved it, which is the only non-circular evidence that the burst was requested.
fn staged(profile: &FixedHunterProfile, stop: PursuitStop) -> (World, OrganismId, SurfacePoint) {
    let mut world = empty_world();
    let spot = SurfacePoint::new(Face::Front, 20.0, 32.0);
    let receipt = world
        .start_hunter_trial(profile.clone(), target_of(spot))
        .expect("the trial starts");
    aim(&mut world, receipt.id, Vec2::new(1.0, 0.0), 1.0);
    world.record_strike_attempts(true);
    world.set_pursuit_stop(stop);
    (world, receipt.id, spot)
}

fn run_for_record(world: &mut World, ticks: u64) -> cubarium_core::hunter::StrikeRecord {
    for _ in 0..ticks {
        world.step();
        world.drain_hunter_events();
        let (records, dropped, unreadable) = world.drain_strike_records();
        assert_eq!((dropped, unreadable), (0, 0), "no record was lost or unreadable");
        if let Some(first) = records.first() {
            return *first;
        }
    }
    panic!(
        "no attempt resolved in {ticks} ticks: phases {:?}",
        world.hunters().members.iter().map(|m| (m.phase, m.target)).collect::<Vec<_>>()
    );
}

/// The speed cap a **held** member runs its paid burst at: `rest_effort · speed_max`, which the
/// note measured 80.4 % of held bursts at or below. A little slack is allowed above it because
/// the cap bounds `|v| + r·|ω|` rather than `|v|` alone, so a held member can never exceed it.
fn held_speed_cap(world: &World, hunter: OrganismId) -> f64 {
    let o = world.state.organisms.get(hunter).expect("alive");
    f64::from(o.phenotype.drives.rest_effort) * o.phenotype.speed_max
}

// ---------------------------------------------------------------- the two written rules

/// The two rules, on hand-built geometry, at the four places they can disagree. Nothing here
/// runs a world: this is the predicate itself against the two inequalities as written.
#[test]
fn the_two_rules_are_the_two_written_predicates_and_differ_where_the_note_says() {
    let profile = trial(&empty_world());
    let geometry = ContactGeometry {
        scale: 1.0,
        capture_offset_body: profile.capture_offset_body,
        capture_reach_px: profile.capture_reach_px,
        ingestion_offset_body: profile.ingestion_offset_body,
        visual_query_extent_px: profile.visual_query_extent_px,
    };
    let forward = geometry.capture_offset_body.x;
    let tolerance = geometry.capture_reach_px + 0.5;
    // A measure whose prey sits `along` the heading axis of the grasp centre: the separation
    // from the grasp is |along| and the forward coordinate is `forward + along`.
    let on_axis = |along: f64| ContactMeasure {
        body: geometry.capture_offset_body + Vec2::new(along, 0.0),
        root_distance: (geometry.capture_offset_body + Vec2::new(along, 0.0)).length(),
        effector_distance: along.abs(),
        tolerance,
    };

    // 1. In the grasp: both rules hold. This is the case the comment is about.
    let inside = on_axis(0.0);
    assert!(geometry.pursuit_holds(PursuitStop::ForwardHalfSpace, &inside));
    assert!(geometry.pursuit_holds(PursuitStop::ReachEnvelope, &inside));

    // 2. **Short of the claws and out of reach.** The whole disagreement: the half-space holds
    //    a member that is 6 px from closing on its prey, the envelope does not. This is the
    //    geometry of 93 % of the death arm's reach failures (`fwd@int` 7.36 px against a grasp
    //    at 13.28 px).
    let short = on_axis(-(tolerance + 2.0));
    assert!(short.body.x < forward, "the prey is short of the claws");
    assert!(!short.in_contact(), "and out of reach");
    assert!(
        geometry.pursuit_holds(PursuitStop::ForwardHalfSpace, &short),
        "the shipped rule holds a member whose prey is 6 px short of its own claws"
    );
    assert!(
        !geometry.pursuit_holds(PursuitStop::ReachEnvelope, &short),
        "the envelope does not: this prey is exactly what the burst exists to close on"
    );

    // 3. Past the claws and out of reach: neither rule holds, so both push the burst. The
    //    shipped rule's stated purpose — not walking onto a prey and putting it behind the
    //    claws — is the one case it and the envelope already agree on.
    let past = on_axis(tolerance + 2.0);
    assert!(past.body.x > forward + tolerance);
    assert!(!geometry.pursuit_holds(PursuitStop::ForwardHalfSpace, &past));
    assert!(!geometry.pursuit_holds(PursuitStop::ReachEnvelope, &past));

    // 4. The two boundaries, as written: `<` is strict and `<=` is not.
    let at_half_space = ContactMeasure { body: Vec2::new(forward + tolerance, 0.0), ..on_axis(0.0) };
    assert!(
        !geometry.pursuit_holds(PursuitStop::ForwardHalfSpace, &at_half_space),
        "`body.x < forward + tolerance` is strict"
    );
    let at_envelope = ContactMeasure { effector_distance: tolerance, ..on_axis(0.0) };
    assert!(
        geometry.pursuit_holds(PursuitStop::ReachEnvelope, &at_envelope),
        "`effector_distance <= tolerance` is not"
    );

    // The shipped rule — the default, everywhere it is read from — is the envelope from
    // 2026-09-16. `pursuit_predicate_adoption.rs` is where the adoption itself is fixed.
    assert_eq!(PursuitStop::default(), PursuitStop::ReachEnvelope);
    assert_eq!(empty_world().pursuit_stop(), PursuitStop::ReachEnvelope);
}

// ---------------------------------------------------------------- default byte-identity

fn two_apex_world(stop: Option<PursuitStop>) -> World {
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
    world.record_strike_attempts(true);
    if let Some(stop) = stop {
        world.set_pursuit_stop(stop);
    }
    world
}

// The "naming the default changes nothing over 9,000 ticks" run this file used to carry now
// lives in `pursuit_predicate_adoption.rs`, where it is checked against hashes pinned by the
// commit before the default was flipped — a strictly stronger claim on the same fixture, and
// one that has to name the rule the retained rows ran under. Duplicating a 9,000-tick two-apex
// run here would buy nothing.

/// The two rules are not the same world: the same seed, the same introductions, run
/// under the envelope, is a different world — and says so on every record it makes.
#[test]
fn the_variant_moves_the_same_two_apex_world_and_every_record_names_the_rule_it_ran_under() {
    let mut shipped = two_apex_world(Some(PursuitStop::ForwardHalfSpace));
    let mut variant = two_apex_world(Some(PursuitStop::ReachEnvelope));
    assert_eq!(variant.pursuit_stop(), PursuitStop::ReachEnvelope);
    let mut diverged_at = None;
    let (mut shipped_records, mut variant_records) = (0usize, 0usize);
    for tick in 1..=9_000u64 {
        shipped.step();
        variant.step();
        shipped.drain_events();
        variant.drain_events();
        shipped.drain_hunter_events();
        variant.drain_hunter_events();
        for r in shipped.drain_strike_records().0 {
            assert_eq!(r.stop, PursuitStop::ForwardHalfSpace);
            shipped_records += 1;
        }
        for r in variant.drain_strike_records().0 {
            assert_eq!(r.stop, PursuitStop::ReachEnvelope, "the record names the rule it ran under");
            variant_records += 1;
        }
        if diverged_at.is_none() && state_hash(&shipped.state) != state_hash(&variant.state) {
            diverged_at = Some(tick);
        }
    }
    assert!(shipped_records > 0 && variant_records > 0, "both arms made paid attempts");
    assert!(
        diverged_at.is_some(),
        "the envelope changed nothing in 9,000 ticks of a two-apex world, so the switch is \
         not wired into the predicate"
    );
}

// ---------------------------------------------------------------- the two hand-built holds

/// The deciding fixture. The prey sits on the grasp's own forward axis, `tolerance + 2 px`
/// **short** of the claws: the shipped rule holds, the envelope does not, and the difference
/// is measured as the hunter's realised speed over its own paid burst.
fn prey_short_of_the_claws(stop: PursuitStop) -> (f64, bool, f64, AttemptOutcome) {
    let profile = certain(trial(&empty_world()));
    let (mut world, hunter, spot) = staged(&profile, stop);
    // Placed at the claw first so its extent is known, then moved back along the grasp's axis.
    let grasp = travel(Topology::Cube, spot, body_offset(Vec2::new(1.0, 0.0), profile.capture_offset_body, 1.0)).end;
    let prey = place_frozen_prey(&mut world, grasp, 0.5, 0.3, 0.4);
    let extent = world.state.organisms.get(prey).expect("placed").phenotype.extent;
    let tolerance = profile.capture_reach_px + extent;
    let gap = tolerance + 2.0;
    let short = travel(
        Topology::Cube,
        spot,
        body_offset(
            Vec2::new(1.0, 0.0),
            profile.capture_offset_body - Vec2::new(gap, 0.0),
            1.0,
        ),
    )
    .end;
    world.state.organisms.get_mut(prey).expect("placed").pos = short.canonicalize(Topology::Cube);
    let cap = held_speed_cap(&world, hunter);

    let record = run_for_record(&mut world, 400);
    let strike = record.strike.expect("the burst was recorded");
    // The fixture is the one the test claims: short of the claws, and out of reach.
    assert!(
        strike.body_forward.expect("measured") < strike.capture_forward,
        "the prey is short of the claws at the burst's start"
    );
    assert!(!strike.in_reach, "and out of the grasp");
    assert_eq!(record.stop, stop);
    (
        record.hunter_speed_strike.expect("both ends of the burst"),
        record.held_at_strike().expect("the strike frame carried a measure"),
        cap,
        record.outcome,
    )
}

/// Under the shipped rule the member is held and its paid burst moves it no further than its
/// resting effort allows; under the envelope the hold is gone and the burst is delivered.
#[test]
fn a_prey_ahead_but_outside_reach_holds_under_the_half_space_and_bursts_under_the_envelope() {
    let (held_speed, held, cap, held_outcome) =
        prey_short_of_the_claws(PursuitStop::ForwardHalfSpace);
    assert!(held, "the shipped rule holds this member");
    assert!(
        held_speed <= cap,
        "a held burst cannot exceed rest_effort · speed_max = {cap:.4} px/s, got {held_speed:.4}"
    );
    assert_eq!(
        held_outcome,
        AttemptOutcome::OutOfReach,
        "a burst that is never requested cannot arrive"
    );

    let (burst_speed, still_held, _, _) = prey_short_of_the_claws(PursuitStop::ReachEnvelope);
    assert!(!still_held, "the envelope does not hold a prey that is out of reach");
    assert!(
        burst_speed > 10.0 * cap.max(1e-9),
        "the burst was requested but not delivered: {burst_speed:.4} px/s against a held cap of \
         {cap:.4} px/s"
    );
}

/// A prey already in the grasp holds under **both** rules — the case the comment is about, and
/// the one the correction must not change.
#[test]
fn a_prey_inside_reach_still_holds_under_both_rules() {
    for stop in [PursuitStop::ForwardHalfSpace, PursuitStop::ReachEnvelope] {
        let profile = certain(trial(&empty_world()));
        let (mut world, hunter, spot) = staged(&profile, stop);
        let grasp =
            travel(Topology::Cube, spot, body_offset(Vec2::new(1.0, 0.0), profile.capture_offset_body, 1.0)).end;
        place_frozen_prey(&mut world, grasp, 0.5, 0.3, 0.4);
        let cap = held_speed_cap(&world, hunter);

        let record = run_for_record(&mut world, 400);
        let strike = record.strike.expect("the burst was recorded");
        assert!(strike.in_reach, "{stop:?}: the prey is in the grasp");
        assert_eq!(record.held_at_strike(), Some(true), "{stop:?}: the pursuit holds");
        assert_eq!(record.outcome, AttemptOutcome::Captured, "{stop:?}: and it captures");
        let speed = record.hunter_speed_strike.expect("both ends of the burst");
        assert!(
            speed <= cap,
            "{stop:?}: a held burst cannot exceed {cap:.4} px/s, got {speed:.4}"
        );
    }
}

/// A record read under the other rule says what the other rule *would* have done at the same
/// instant — which is how the paired arms are compared without re-running either.
#[test]
fn a_frame_can_be_read_under_either_rule_and_the_record_reads_under_its_own() {
    let profile = certain(trial(&empty_world()));
    let (mut world, _hunter, spot) = staged(&profile, PursuitStop::ForwardHalfSpace);
    let grasp = travel(Topology::Cube, spot, body_offset(Vec2::new(1.0, 0.0), profile.capture_offset_body, 1.0)).end;
    let prey = place_frozen_prey(&mut world, grasp, 0.5, 0.3, 0.4);
    let extent = world.state.organisms.get(prey).expect("placed").phenotype.extent;
    let tolerance = profile.capture_reach_px + extent;
    let short = travel(
        Topology::Cube,
        spot,
        body_offset(
            Vec2::new(1.0, 0.0),
            profile.capture_offset_body - Vec2::new(tolerance + 2.0, 0.0),
            1.0,
        ),
    )
    .end;
    world.state.organisms.get_mut(prey).expect("placed").pos = short.canonicalize(Topology::Cube);

    let record = run_for_record(&mut world, 400);
    let strike = record.strike.expect("the burst was recorded");
    assert_eq!(record.stop, PursuitStop::ForwardHalfSpace);
    assert_eq!(record.held_at_strike(), Some(true));
    assert_eq!(strike.pursuit_holds(PursuitStop::ForwardHalfSpace), Some(true));
    assert_eq!(
        strike.pursuit_holds(PursuitStop::ReachEnvelope),
        Some(false),
        "the same instant, read under the other rule"
    );
    assert_eq!(record.held_at_intent(), record.intent.unwrap().pursuit_holds(record.stop));
}

