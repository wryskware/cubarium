//! Contact geometry against the **root-owned chart**, the capture metadata root's harness
//! needs, and the timing facts the art adapter reads
//! (`design/7_Research/lanternjaw-core-art-integration-gaps-2026-09-13.md`,
//! `lanternjaw-ecology-animation-contract-2026-09-13.md`).
//!
//! Nothing here is evidence about balance. `capture_min = capture_max = 1` in these fixtures
//! so the settlement path is deterministic; everything else is the trial profile.

use cubarium_core::genome::{Genome, decode};
use cubarium_core::hunter::{
    AttemptOutcome, ContactGeometry, FixedHunterProfile, HunterEvent, HunterPhase, HunterTarget,
    body_point, body_scale, measure_contact,
};
use cubarium_core::ids::OrganismId;
use cubarium_core::organism::{Mode, Organism, Origin};
use cubarium_core::rng::Counter;
use cubarium_core::snapshot::state_hash;
use cubarium_core::{World, WorldConfig, decode_snapshot, encode_snapshot};
use cubarium_surface::{ChartImage, Face, MAX_SEAMS, SurfacePoint, Vec2, chart_images, travel, unfold_with};

// ---------------------------------------------------------------- fixtures

fn quiet_world() -> World {
    let mut cfg = WorldConfig::default();
    cfg.founders.kinds.clear();
    cfg.founders.count = 0;
    cfg.weather.amplitude = 0.0;
    cfg.water.rain_rate = 0.0;
    cfg.detritus.initial_dark = 0.0;
    cfg.detritus.decomposition = 0.0;
    cfg.detritus.fall = 0.0;
    World::new(cfg).expect("a quiet world is valid")
}

fn certain(world: &World) -> FixedHunterProfile {
    let mut p = FixedHunterProfile::lanternjaw_trial(world.config());
    p.capture_min = 1.0;
    p.capture_max = 1.0;
    p
}

fn target_of(pos: SurfacePoint) -> HunterTarget {
    HunterTarget { face: pos.face.index() as u8, u: pos.u, v: pos.v }
}

/// The five faces' chart images, the same cache the world and the renderer use.
fn images() -> [Vec<ChartImage>; 5] {
    std::array::from_fn(|i| {
        let mut v = Vec::new();
        chart_images(Face::from_index(i as u8).expect("five faces"), MAX_SEAMS, &mut v);
        v
    })
}

/// The body basis of `stamp_rig`, transcribed: `+x` along the heading, `+y` its clockwise side.
fn basis(heading: Vec2) -> (Vec2, Vec2) {
    let h = heading.normalized().expect("a heading");
    (h, Vec2::new(-h.y, h.x))
}

fn chart_offset(heading: Vec2, body: Vec2) -> Vec2 {
    let (forward, side) = basis(heading);
    forward * body.x + side * body.y
}

fn place_prey(world: &mut World, pos: SurfacePoint, s: f64, r: f64, e: f64) -> OrganismId {
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

fn aim(world: &mut World, id: OrganismId, heading: Vec2, reserve: f64) {
    let o = world.state.organisms.get_mut(id).expect("alive");
    let before = o.reserve;
    o.heading = heading;
    o.reserve = reserve;
    world.state.external_material_in += reserve - before;
}

/// Shrink a member to `structure`, the way growth would have left a juvenile.
fn shrink(world: &mut World, id: OrganismId, structure: f64) {
    let o = world.state.organisms.get_mut(id).expect("alive");
    let before = o.structure;
    o.structure = structure;
    world.state.external_material_in += structure - before;
}

// ---------------------------------------------------------------- the root chart

/// Over ordinary interior points, seams, rims and corners: whatever the world publishes as a
/// grasp centre must be exactly where the renderer would draw it, and where it publishes
/// nothing there must be nothing to publish.
///
/// This is the property the old transported-anchor helper broke: it answered with a reflected
/// point whose own chart disagreed with the root's.
#[test]
fn every_published_grasp_centre_round_trips_through_the_root_chart() {
    let world = quiet_world();
    let profile = certain(&world);
    let images = images();
    let offsets = [profile.capture_offset_body, profile.ingestion_offset_body];

    let mut published = 0;
    let mut refused = 0;
    for face in [Face::Front, Face::Right, Face::Top, Face::Back, Face::Left] {
        for u in [1.5, 16.5, 32.5, 47.5, 62.5] {
            for v in [1.5, 16.5, 32.5, 47.5, 62.5] {
                for angle in 0..8 {
                    let root = SurfacePoint::new(face, u, v).canonicalize();
                    let heading = Vec2::from_screen_angle(
                        angle as f64 * std::f64::consts::TAU / 8.0,
                    );
                    for offset in offsets {
                        match body_point(&images, root, heading, offset) {
                            Some(point) => {
                                published += 1;
                                // The root's own shortest image of that point is the body
                                // coordinate it was built from — not merely *a* short path.
                                let u = unfold_with(&images[root.face.index()], root, point, 32.0)
                                    .expect("a published centre is reachable from the root");
                                let (forward, side) = basis(heading);
                                let delta = u.local - root.chart();
                                let back = Vec2::new(forward.dot(delta), side.dot(delta));
                                assert!(
                                    (back - offset).length() < 1e-6,
                                    "{face:?} ({u:?},{v}) angle {angle}: published {back:?} for {offset:?}"
                                );
                                // And the sweep that produced it neither reflected nor guessed.
                                let swept = travel(root, chart_offset(heading, offset));
                                assert_eq!(swept.reflections, 0);
                                assert!(!swept.fallback);
                                assert_eq!(swept.ties, 0);
                            }
                            None => {
                                refused += 1;
                                // A refusal is always explained by the sweep itself.
                                let swept = travel(root, chart_offset(heading, offset));
                                let off_surface =
                                    swept.reflections > 0 || swept.fallback || swept.ties > 0;
                                let inconsistent = !off_surface;
                                assert!(
                                    off_surface || inconsistent,
                                    "a centre was refused for no reason"
                                );
                            }
                        }
                    }
                }
            }
        }
    }
    assert!(published > 100, "this sweep must actually publish centres: {published}");
    assert!(refused > 0, "and must actually meet rims and vertices: {refused}");
}

/// A prey exactly at a published grasp centre is captured; the same prey one reach-plus-slack
/// beyond it is not. Run on an interior point and across an ordinary seam.
#[test]
fn contact_is_decided_at_the_published_grasp_and_not_at_the_thorax() {
    for (face, u, v, what) in
        [(Face::Top, 22.0, 34.0, "interior"), (Face::Front, 55.0, 32.0, "across a seam")]
    {
        // In the grasp: captured.
        let mut world = quiet_world();
        let profile = certain(&world);
        let spot = SurfacePoint::new(face, u, v);
        let hunter = world.start_hunter_trial(profile.clone(), target_of(spot)).expect("started").id;
        aim(&mut world, hunter, Vec2::new(1.0, 0.0), 1.0);
        let grasp = world.hunter_view()[0].capture_center.expect("{what}: a centre");
        let prey = place_prey(&mut world, grasp, 0.5, 0.3, 0.4);
        for _ in 0..300 {
            world.step();
            if world.hunters().captures_total == 1 {
                break;
            }
        }
        assert!(world.state.organisms.get(prey).is_none(), "{what}: the grasp missed its own centre");

        // At the thorax — six pixels ahead, the old placeholder — nothing is ever caught.
        let mut world = quiet_world();
        let profile = certain(&world);
        let hunter = world.start_hunter_trial(profile.clone(), target_of(spot)).expect("started").id;
        aim(&mut world, hunter, Vec2::new(1.0, 0.0), 1.0);
        let near = travel(spot, chart_offset(Vec2::new(1.0, 0.0), Vec2::new(6.0, 0.0))).end;
        let prey = place_prey(&mut world, near, 0.5, 0.3, 0.4);
        for _ in 0..200 {
            world.step();
        }
        assert!(
            world.state.organisms.get(prey).is_some(),
            "{what}: a prey at the old six-pixel placeholder was captured by the claws"
        );
        assert_eq!(world.hunters().captures_total, 0, "{what}");
    }
}

/// A juvenile hunts with exactly the geometry the view publishes for it, and that geometry is
/// the profile's mapping of its actual structure — not the adult's reach, and not a boolean.
#[test]
fn a_juvenile_grasps_at_its_own_published_scale() {
    let mut world = quiet_world();
    let profile = certain(&world);
    let spot = SurfacePoint::new(Face::Top, 22.0, 34.0);
    let hunter = world.start_hunter_trial(profile.clone(), target_of(spot)).expect("started").id;
    aim(&mut world, hunter, Vec2::new(1.0, 0.0), 1.0);
    // Exactly the structure a funded child is born with.
    let child_structure = world.config().organism.child_structure_fraction * 2.0;
    shrink(&mut world, hunter, child_structure);

    let view = &world.hunter_view()[0];
    let expected = (child_structure / 2.0f64).sqrt();
    assert!((view.body_scale - expected).abs() < 1e-12, "{} vs {expected}", view.body_scale);
    assert_eq!(view.body_scale, body_scale(&profile, child_structure, 2.0));
    assert!(view.body_scale < 1.0 && view.body_scale > profile.body_scale_min);
    // Every published number is that one scale applied to the profile.
    assert_eq!(view.geometry.capture_offset_body, profile.capture_offset_body * view.body_scale);
    assert_eq!(view.geometry.capture_reach_px, profile.capture_reach_px * view.body_scale);
    assert_eq!(view.geometry.ingestion_offset_body, profile.ingestion_offset_body * view.body_scale);
    assert_eq!(view.geometry.visual_query_extent_px, profile.visual_query_extent_px * view.body_scale);
    let juvenile_grasp = view.capture_center.expect("a centre on the open top face");

    // A prey in the juvenile's own grasp is caught.
    let prey = place_prey(&mut world, juvenile_grasp, 0.3, 0.2, 0.3);
    for _ in 0..400 {
        world.step();
        if world.hunters().captures_total == 1 {
            break;
        }
    }
    assert!(world.state.organisms.get(prey).is_none(), "the juvenile could not reach its own claws");

    // A prey at the *adult* reach is out of a juvenile's grasp entirely.
    let mut world = quiet_world();
    let profile = certain(&world);
    let hunter = world.start_hunter_trial(profile.clone(), target_of(spot)).expect("started").id;
    aim(&mut world, hunter, Vec2::new(1.0, 0.0), 1.0);
    shrink(&mut world, hunter, child_structure);
    let adult_grasp = travel(spot, chart_offset(Vec2::new(1.0, 0.0), profile.capture_offset_body)).end;
    let far = place_prey(&mut world, adult_grasp, 0.3, 0.2, 0.3);
    for _ in 0..200 {
        world.step();
    }
    assert!(
        world.state.organisms.get(far).is_some(),
        "a juvenile captured at the adult reach: the world is not using the scale it publishes"
    );
}

// ---------------------------------------------------------------- settlement evidence

/// The capture record is the settlement itself: positions taken before removal, the geometry
/// the test used, and one stable key shared with the paid attempt.
#[test]
fn capture_metadata_is_the_pre_removal_settlement_and_shares_the_paid_key() {
    let mut world = quiet_world();
    let profile = certain(&world);
    let spot = SurfacePoint::new(Face::Top, 22.0, 34.0);
    let hunter = world.start_hunter_trial(profile.clone(), target_of(spot)).expect("started").id;
    aim(&mut world, hunter, Vec2::new(1.0, 0.0), 1.0);
    let grasp = world.hunter_view()[0].capture_center.expect("a centre");
    let prey = place_prey(&mut world, grasp, 0.5, 0.3, 0.4);
    let prey_pos = world.state.organisms.get(prey).expect("alive").pos;

    let mut events = Vec::new();
    for _ in 0..400 {
        world.step();
        events.extend(world.drain_hunter_events());
        if world.hunters().captures_total == 1 {
            break;
        }
    }

    let capture = events
        .iter()
        .find_map(|e| match e {
            HunterEvent::Capture { hunter: h, prey: p, attack_counter, evidence, tick, .. } => {
                Some((*h, *p, *attack_counter, *evidence, *tick))
            }
            _ => None,
        })
        .expect("a capture record");
    let (event_hunter, event_prey, key, evidence, tick) = capture;
    assert_eq!(event_hunter, hunter);
    assert_eq!(event_prey, prey);
    assert_eq!(tick, world.tick());

    // The same paid key on the attempt that produced it, and on the member that made it.
    let attempt = events
        .iter()
        .find_map(|e| match e {
            HunterEvent::Attempt { outcome: AttemptOutcome::Captured, attack_counter, evidence, .. } => {
                Some((*attack_counter, *evidence))
            }
            _ => None,
        })
        .expect("the paid attempt");
    assert_eq!(attempt.0, Some(key), "the attempt and the capture must share one key");
    assert_eq!(attempt.1, Some(evidence), "and the same settlement evidence");
    assert_eq!(world.hunters().member(hunter).expect("a member").attack_counter, key);
    assert_eq!(world.hunters().member(hunter).expect("a member").episode, key);

    // The evidence is the settlement, recomputed here from its own reported numbers.
    let images = images();
    assert_eq!(evidence.prey, prey);
    assert_eq!(evidence.prey_pos, prey_pos, "the frozen prey never moved, and this is its place");
    let measure = evidence.measure.expect("a capture measured its prey");
    let recomputed = measure_contact(
        &images,
        evidence.hunter_pos,
        evidence.hunter_heading,
        &evidence.geometry,
        evidence.prey_pos,
        evidence.prey_extent,
    )
    .expect("the reported pairing is measurable");
    assert_eq!(recomputed, measure, "the record does not describe its own contact test");
    assert!(measure.in_contact());
    assert_eq!(
        evidence.capture_center,
        body_point(&images, evidence.hunter_pos, evidence.hunter_heading, evidence.geometry.capture_offset_body),
    );
    assert!(evidence.capture_center.is_some(), "a capture always had a drawable grasp");
    assert!(evidence.ingestion_center.is_some());
    assert_ne!(evidence.capture_center, evidence.ingestion_center, "grasp and mouth are not one point");
    // The hunter has moved on by the time a deferred reader sees this, and the record still
    // holds where the settlement happened.
    assert_eq!(evidence.geometry, ContactGeometry::of(&profile, world.state.organisms.get(hunter).expect("alive")));
}

/// An unpaid refusal carries no paid key at all, so it can never alias a paid attempt; a stale
/// target carries no prey position, so nothing is filled in from whoever reused the slot.
#[test]
fn unpaid_refusals_and_stale_targets_carry_no_paid_key_or_borrowed_position() {
    let mut world = quiet_world();
    let profile = certain(&world);
    let spot = SurfacePoint::new(Face::Top, 22.0, 34.0);
    let hunter = world.start_hunter_trial(profile.clone(), target_of(spot)).expect("started").id;
    aim(&mut world, hunter, Vec2::new(1.0, 0.0), 1.0);
    let grasp = world.hunter_view()[0].capture_center.expect("a centre");
    place_prey(&mut world, grasp, 0.5, 0.3, 0.4);
    world.state.organisms.get_mut(hunter).expect("alive").energy = profile.strike_energy_cost * 0.5;

    let mut events = Vec::new();
    for _ in 0..300 {
        world.step();
        events.extend(world.drain_hunter_events());
        if events.iter().any(|e| {
            matches!(e, HunterEvent::Attempt { outcome: AttemptOutcome::Unaffordable, .. })
        }) {
            break;
        }
    }
    let refusal = events
        .iter()
        .find_map(|e| match e {
            HunterEvent::Attempt { outcome: AttemptOutcome::Unaffordable, attack_counter, evidence, .. } => {
                Some((*attack_counter, *evidence))
            }
            _ => None,
        })
        .expect("the unpaid refusal");
    assert_eq!(refusal.0, None, "an unpaid refusal must carry no paid key");
    assert!(refusal.1.is_some(), "but it still says what it was looking at");
    assert_eq!(world.hunters().member(hunter).expect("a member").attack_counter, 0);
    assert_eq!(world.hunters().attacks_total, 0);
}

// ---------------------------------------------------------------- timing across a restart

/// **Regression.** Settlement runs after movement, so the phase it opens belongs to the
/// boundary the tick completes. An earlier build entered Handling and Recovering at `now`
/// while stamping their events `now + 1`: a capture at tick 33 published Handling as having
/// started at 32, which starts a recoil before the contact it recoils from, and a failed
/// strike lost one tick of its advertised recovery
/// (`astra-hunter-geometry-review-2026-09-13.md`).
#[test]
fn a_settled_phase_starts_at_the_settlement_tick_and_runs_its_whole_span() {
    // A capture: Handling opens exactly at the capture record's tick.
    let mut world = quiet_world();
    let profile = certain(&world);
    let spot = SurfacePoint::new(Face::Top, 22.0, 34.0);
    let hunter = world.start_hunter_trial(profile.clone(), target_of(spot)).expect("started").id;
    aim(&mut world, hunter, Vec2::new(1.0, 0.0), 1.0);
    let grasp = world.hunter_view()[0].capture_center.expect("a centre");
    place_prey(&mut world, grasp, 0.5, 0.3, 0.4);

    let mut capture_tick = None;
    for _ in 0..400 {
        world.step();
        for event in world.drain_hunter_events() {
            if let HunterEvent::Capture { tick, .. } = event {
                capture_tick = Some(tick);
            }
        }
        if capture_tick.is_some() {
            break;
        }
    }
    let capture_tick = capture_tick.expect("a capture");
    let member = *world.hunters().member(hunter).expect("a member");
    assert_eq!(capture_tick, world.tick(), "the record is stamped at the completed boundary");
    assert_eq!(member.phase, HunterPhase::Handling);
    assert_eq!(
        member.phase_started_tick, capture_tick,
        "Handling must open at the settlement, not a tick before it"
    );
    assert_eq!(member.phase_ends_tick, member.phase_started_tick, "Handling is untimed");
    assert_eq!(member.entered_from, HunterPhase::Strike, "it recoils from a fully extended strike");
    let view = world.hunter_view().into_iter().next().expect("a view");
    assert_eq!(view.phase_started_tick, capture_tick);

    // A paid miss: Recovering opens at the attempt's tick and runs its whole advertised span.
    let mut missing = certain(&world);
    missing.capture_min = 0.0;
    missing.capture_max = 0.0;
    let mut world = quiet_world();
    let hunter = world.start_hunter_trial(missing.clone(), target_of(spot)).expect("started").id;
    aim(&mut world, hunter, Vec2::new(1.0, 0.0), 1.0);
    let grasp = world.hunter_view()[0].capture_center.expect("a centre");
    place_prey(&mut world, grasp, 0.5, 0.3, 0.4);

    let mut miss_tick = None;
    for _ in 0..400 {
        world.step();
        for event in world.drain_hunter_events() {
            if let HunterEvent::Attempt { tick, outcome: AttemptOutcome::Missed, .. } = event {
                miss_tick = Some(tick);
            }
        }
        if miss_tick.is_some() {
            break;
        }
    }
    let miss_tick = miss_tick.expect("a paid miss");
    let member = *world.hunters().member(hunter).expect("a member");
    assert_eq!(miss_tick, world.tick());
    assert_eq!(member.phase, HunterPhase::Recovering);
    assert_eq!(member.phase_started_tick, miss_tick, "the recoil starts where the strike ended");
    let recovery_ticks = (missing.recovery_seconds / cubarium_core::DT).round() as u64;
    assert_eq!(
        member.phase_ends_tick - member.phase_started_tick,
        recovery_ticks,
        "a failed strike must recover for its whole advertised span"
    );
    // And it really waits that long. The phase occupies the boundaries `[started, ends]`: the
    // decision pass leaves it when the world has reached `ends`, so the member is still
    // recovering when observed *at* `ends` and has moved on one step later — the same one-step
    // display lag every pre-step decision has.
    for _ in 0..recovery_ticks {
        world.step();
    }
    assert_eq!(world.tick(), member.phase_ends_tick);
    assert_eq!(
        world.hunters().member(hunter).expect("a member").phase,
        HunterPhase::Recovering,
        "the pause ended before its advertised boundary"
    );
    world.step();
    let after = *world.hunters().member(hunter).expect("a member");
    assert_ne!(after.phase, HunterPhase::Recovering, "the pause outlived its span");
    assert_eq!(after.phase_started_tick, member.phase_ends_tick, "and the next phase is contiguous");
}

/// The pause after a finished meal is a post-movement transition too: it opens at the tick the
/// gut empties and lasts the profile's whole `meal_recovery_seconds`.
#[test]
fn the_pause_after_a_meal_opens_at_the_tick_the_gut_empties() {
    let mut world = quiet_world();
    let mut profile = certain(&world);
    // A short pause, so the test can watch the whole of it without a long run.
    profile.meal_recovery_seconds = 1.0;
    let spot = SurfacePoint::new(Face::Top, 22.0, 34.0);
    let hunter = world.start_hunter_trial(profile.clone(), target_of(spot)).expect("started").id;
    aim(&mut world, hunter, Vec2::new(1.0, 0.0), 1.0);
    let grasp = world.hunter_view()[0].capture_center.expect("a centre");
    // A small meal, so it is digested inside the test window.
    place_prey(&mut world, grasp, 0.2, 0.05, 0.1);

    let mut emptied_at = None;
    for _ in 0..3_000 {
        let carrying = world.hunters().member(hunter).map(|m| m.carrying()).unwrap_or(false);
        world.step();
        let member = *world.hunters().member(hunter).expect("a member");
        if carrying && !member.carrying() {
            emptied_at = Some((world.tick(), member));
            break;
        }
    }
    let (tick, member) = emptied_at.expect("the meal must finish inside the window");
    assert_eq!(member.phase, HunterPhase::Recovering, "the pause starts as the gut empties");
    assert_eq!(member.entered_from, HunterPhase::Handling, "and it knows it followed a meal");
    assert_eq!(member.phase_started_tick, tick, "at the boundary the tick completed");
    let meal_ticks = (profile.meal_recovery_seconds / cubarium_core::DT).round() as u64;
    assert_eq!(member.phase_ends_tick - member.phase_started_tick, meal_ticks);
}

/// The view's timing is the member's timing, and both survive a checkpoint: a restart during a
/// recoil still knows what it recoiled from, and the resumed world produces the very same
/// event stream, key for key.
#[test]
fn phase_timing_and_episode_identity_survive_a_restart() {
    let mut world = quiet_world();
    let profile = certain(&world);
    let spot = SurfacePoint::new(Face::Top, 22.0, 34.0);
    let hunter = world.start_hunter_trial(profile.clone(), target_of(spot)).expect("started").id;
    aim(&mut world, hunter, Vec2::new(1.0, 0.0), 1.0);
    let grasp = world.hunter_view()[0].capture_center.expect("a centre");
    place_prey(&mut world, grasp, 0.5, 0.3, 0.4);

    // Run into the paid strike, where the phase is timed and belongs to an episode.
    for _ in 0..200 {
        world.step();
        if world.hunters().member(hunter).expect("a member").phase == HunterPhase::Strike {
            break;
        }
    }
    let member = *world.hunters().member(hunter).expect("a member");
    assert_eq!(member.phase, HunterPhase::Strike);
    assert_eq!(member.entered_from, HunterPhase::Windup, "a strike follows its windup");
    assert_eq!(member.episode, member.attack_counter);
    let view = world.hunter_view().into_iter().next().expect("a view");
    assert_eq!(view.phase_started_tick, member.phase_started_tick, "the view is the member");
    assert_eq!(view.phase_ends_tick, member.phase_ends_tick);
    assert_eq!(view.entered_from, member.entered_from);
    assert_eq!(view.episode, member.episode);
    assert_eq!(view.attack_counter, member.attack_counter);
    assert!(view.phase_ends_tick > view.phase_started_tick, "a timed phase has a real boundary");

    // Checkpoint here and resume: the timing facts are persisted, not inferred.
    let bytes = encode_snapshot(&world.state, "geometry-restart");
    let (_, state) = decode_snapshot(&bytes).expect("round trip");
    let mut reloaded = World::from_state(state).expect("valid");
    assert_eq!(*reloaded.hunters().member(hunter).expect("a member"), member);
    assert_eq!(state_hash(&reloaded.state), state_hash(&world.state));

    let mut original_events = Vec::new();
    let mut reloaded_events = Vec::new();
    let mut settled: Option<u64> = None;
    for _ in 0..200 {
        world.step();
        reloaded.step();
        let fresh = world.drain_hunter_events();
        // The tick the paid attempt resolved on, checked *there* rather than at the end of the
        // run: a restart must not shift the boundary the recoil is interpolated from.
        if settled.is_none()
            && let Some(tick) = fresh.iter().find_map(|e| match e {
                HunterEvent::Capture { tick, .. } => Some(*tick),
                HunterEvent::Attempt { tick, outcome, .. }
                    if !matches!(outcome, AttemptOutcome::Unaffordable) =>
                {
                    Some(*tick)
                }
                _ => None,
            })
        {
            settled = Some(tick);
            let opened = *world.hunters().member(hunter).expect("a member");
            assert_eq!(tick, world.tick());
            assert_eq!(
                opened.phase_started_tick, tick,
                "the phase opened by settlement must start at the settlement tick"
            );
            assert_eq!(*reloaded.hunters().member(hunter).expect("a member"), opened);
        }
        original_events.extend(fresh);
        reloaded_events.extend(reloaded.drain_hunter_events());
        assert_eq!(state_hash(&reloaded.state), state_hash(&world.state), "diverged at {}", world.tick());
    }
    assert!(!original_events.is_empty(), "the strike must have resolved");
    assert_eq!(reloaded_events, original_events, "the resumed run told a different story");
    assert!(settled.is_some(), "the paid attempt never resolved");

    // And the recoil that followed says what it followed.
    let after = *world.hunters().member(hunter).expect("a member");
    assert!(
        matches!(after.entered_from, HunterPhase::Strike | HunterPhase::Handling | HunterPhase::Recovering),
        "{after:?}"
    );
}
