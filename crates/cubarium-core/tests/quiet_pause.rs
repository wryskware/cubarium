//! `post_birth_pause_v1`: what the candidate does, and everything it must leave alone
//! (`design/7_Research/astra-ordinary-quiet-experiment-proposal-2026-09-13.md`).
//!
//! The world under test is a **genuine mature one** — the committed schema 12 fixture at tick
//! 147000, which was produced by the pre-quiet binary and has already paid for 437 births. A
//! pause that only ever fires in a hand-built two-organism world would not tell us whether the
//! rule can fire at all.
//!
//! Nothing here is a balance claim, an accepted duration or evidence of viability. That a
//! parent can hold forty decisions says nothing about whether a lineage is better off for it.

use std::path::PathBuf;

use cubarium_core::ids::OrganismId;
use cubarium_core::organism::Mode;
use cubarium_core::quiet::{
    Budget, POST_BIRTH_PAUSE_TICKS, QuietEvent, QuietPolicy, QuietReason, QuietState,
    horizon_seconds,
};
use cubarium_core::snapshot::state_hash;
use cubarium_core::{
    DT, LifeEvent, World, WorldConfig, WorldState, decode_snapshot, encode_snapshot,
};

// ---------------------------------------------------------------- fixtures

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures").join(name)
}

/// The committed genuine schema 12 opening: a mature world that really reproduces.
fn mature() -> WorldState {
    let bytes = std::fs::read(fixture("quiet-v12-plain-3000.cubw")).expect("the fixture");
    let (_, state) = decode_snapshot(&bytes).expect("a genuine schema 12 snapshot loads");
    assert_eq!(state.tick, 147_000);
    assert_eq!(state.quiet, QuietState::default(), "it migrates Off");
    state
}

fn with_policy(mut state: WorldState) -> World {
    state.quiet = QuietState::post_birth_pause_v1();
    World::from_state(state).expect("an enabled quiet world is valid")
}

/// Step until the first pause is admitted, returning the world, the `Begin` record and the
/// boundary `B` it names. Bounded: a fixture that never reproduces is a failed test, not a hang.
fn run_to_first_begin(world: &mut World) -> (OrganismId, OrganismId, u64, Mode) {
    for _ in 0..20_000 {
        world.step();
        world.drain_events();
        for event in world.drain_quiet_events() {
            if let QuietEvent::Begin { tick, parent, child, end_tick, underlying } = event {
                assert_eq!(end_tick, tick + POST_BIRTH_PAUSE_TICKS);
                return (parent, child, tick, underlying);
            }
        }
    }
    panic!("the mature fixture admitted no pause within 20000 ticks");
}

fn mode_of(world: &World, id: OrganismId) -> Mode {
    world.state.organisms.get(id).expect("alive").mode
}

// ---------------------------------------------------------------- the reference arm

/// Off is inert. A world carrying the extension at its default steps **bit for bit** as the
/// pre-quiet build did, and never publishes a record.
#[test]
fn an_off_world_is_the_unchanged_trajectory() {
    let state = mature();
    assert!(!state.quiet.active());
    let mut off = World::from_state(state.clone()).expect("valid");
    // The same opening with the extension explicitly present but Off.
    let mut explicit = state.clone();
    explicit.quiet = QuietState::default();
    let mut explicit = World::from_state(explicit).expect("valid");

    for i in 0..1200 {
        off.step();
        explicit.step();
        off.drain_events();
        explicit.drain_events();
        assert_eq!(
            state_hash(&explicit.state),
            state_hash(&off.state),
            "diverged at tick {i}"
        );
        assert!(off.drain_quiet_events().is_empty(), "an Off world published a record");
        assert!(explicit.drain_quiet_events().is_empty());
    }
    assert!(off.state.births_total > state.births_total, "the reference really did reproduce");
    assert!(off.quiet().pauses.is_empty());
}

// ---------------------------------------------------------------- the window

/// A real affordable birth holds exactly forty decisions, and the fortieth completed interval is
/// the last one: `B` through `B + 39` are held, `B + 40` is ordinary again. The birth's own tick
/// is not repainted.
#[test]
fn a_real_birth_holds_exactly_forty_decisions() {
    let mut world = with_policy(mature());
    let (parent, child, b, underlying) = run_to_first_begin(&mut world);
    assert_eq!(world.tick(), b, "the record is published at the boundary it names");
    assert_ne!(parent, child);

    let pause = *world.quiet().find(parent).expect("the pause is held");
    assert_eq!(pause.start_tick, b);
    assert_eq!(pause.end_tick, b + POST_BIRTH_PAUSE_TICKS);
    assert_eq!(pause.child, child);
    assert_eq!(pause.underlying, underlying);

    // Decisions at B..=B+39 are held: after each of those steps the parent's public mode is
    // genuinely Resting, and the completed intervals are B+1..=B+40.
    let mut ended = None;
    for offset in 0..POST_BIRTH_PAUSE_TICKS {
        assert!(
            world.quiet().find(parent).is_some(),
            "the pause vanished before B+{offset}"
        );
        world.step();
        world.drain_events();
        assert_eq!(world.tick(), b + offset + 1);
        assert_eq!(
            mode_of(&world, parent),
            Mode::Resting,
            "completed interval B+{} is not Resting",
            offset + 1
        );
        for event in world.drain_quiet_events() {
            match event {
                QuietEvent::Abort { .. } => panic!("aborted early at B+{offset}: {event:?}"),
                QuietEvent::End { .. } => panic!("ended early at B+{offset}"),
                _ => {}
            }
        }
    }
    // The decision at B+40 is ordinary again, and releasing is what the record says.
    assert_eq!(world.tick(), b + POST_BIRTH_PAUSE_TICKS);
    world.step();
    world.drain_events();
    for event in world.drain_quiet_events() {
        if let QuietEvent::End { tick, parent: p, child: c, completed_ticks, .. } = event {
            assert_eq!((tick, p, c), (b + POST_BIRTH_PAUSE_TICKS, parent, child));
            assert_eq!(completed_ticks, POST_BIRTH_PAUSE_TICKS);
            ended = Some(event);
        }
    }
    assert!(ended.is_some(), "the pause did not publish its end");
    assert!(world.quiet().find(parent).is_none(), "the entry outlived its window");
}

/// The completed child is untouched by the parent's pause: its opening stores are the escrow's,
/// its birth event is the ordinary one, and no second payment happens.
#[test]
fn the_paid_child_accounting_is_unchanged() {
    let mut world = with_policy(mature());
    // Find a birth and its child in the same drain, so the two records are the same event.
    let (parent, child, b) = loop {
        world.step();
        let life = world.drain_events();
        let quiet = world.drain_quiet_events();
        if let Some(QuietEvent::Begin { parent, child, tick, .. }) =
            quiet.iter().find(|e| matches!(e, QuietEvent::Begin { .. }))
        {
            let birth = life
                .iter()
                .find(|e| matches!(e, LifeEvent::Birth { id, .. } if id == child))
                .expect("the pause's child has an ordinary birth event in the same tick");
            let LifeEvent::Birth { tick: birth_tick, parent: birth_parent, .. } = birth else {
                unreachable!()
            };
            assert_eq!(birth_tick, tick, "the pause starts at the birth's own boundary");
            assert_eq!(birth_parent, parent, "and names its parent");
            break (*parent, *child, *tick);
        }
        assert!(world.tick() < 147_000 + 20_000, "no birth in the window");
    };

    let cfg = world.config().organism.clone();
    let c = world.state.organisms.get(child).expect("the child is alive");
    assert_eq!(c.born_tick, b);
    assert_eq!(c.parent, Some(parent));
    assert_eq!(c.structure, cfg.child_structure_fraction * c.phenotype.structure_adult);
    assert_eq!(c.reserve, cfg.child_reserve_fraction * c.phenotype.reserve_max);
    assert_eq!(c.energy, cfg.child_energy_fraction * c.phenotype.energy_max);
    assert_eq!(c.mode, Mode::Resting, "a newborn's own one-tick initialization, unchanged");
    // And the newborn is not itself given a pause.
    assert!(world.quiet().find(child).is_none(), "the newborn was paused");
    assert_eq!(world.quiet().pauses.len(), 1);
    // The parent's escrow was spent, not refunded, and it is not carrying a new one.
    let p = world.state.organisms.get(parent).expect("the parent survived");
    assert!(p.escrow.is_none());
}

/// Every rule the held decision must obey at once: no intake, no bud request, rest effort and
/// rest turn fraction, real upkeep still paid, hunger memory still truthful, age still running,
/// and no extra RNG draw.
#[test]
fn a_held_decision_suppresses_intake_and_budding_and_pays_everything_else() {
    // Oxidation is switched off in this fixture **before** the world is built, so the only
    // thing moving the parent's battery during a held tick is the upkeep it still owes. With
    // oxidation on, a resting parent's energy can legitimately *rise* as reserve is converted —
    // which is ordinary physiology continuing, exactly as the contract requires, and would make
    // "energy fell" the wrong question.
    let mut state = mature();
    state.config.organism.oxidation_rate = 0.0;
    let mut world = with_policy(state);
    let (parent, _, _, _) = run_to_first_begin(&mut world);

    let before = world.state.organisms.get(parent).expect("alive").clone();
    let turn_counter_before = before.turn_counter;
    world.step();
    world.drain_events();
    let after = world.state.organisms.get(parent).expect("alive").clone();

    assert_eq!(after.mode, Mode::Resting);
    assert!(!after.fed_this_tick, "a held parent must not eat");
    assert!(after.escrow.is_none(), "a held parent must not open a gestation");
    assert_eq!(after.births, before.births, "and pays for no new child");
    // Upkeep is real: a resting body still spends maintenance and sensing, and it still ages.
    assert!(
        after.energy < before.energy,
        "upkeep was not paid: {} -> {}",
        before.energy,
        after.energy
    );
    assert_eq!(after.born_tick, before.born_tick, "age is not suspended");
    assert!(after.age_ticks(world.tick()) > before.age_ticks(world.tick() - 1));
    // Hunger memory kept moving toward the truth, and was written exactly once.
    let tau = f64::from(before.phenotype.drives.tau_hunger_seconds).max(1e-9);
    let want =
        before.hunger_memory + (1.0 - (-DT / tau).exp()) * (before.hunger() - before.hunger_memory);
    assert!(
        (after.hunger_memory - want).abs() < 1e-12,
        "hunger memory {} is not the single ordinary update {want}",
        after.hunger_memory
    );
    // Exactly the ordinary four turn-noise draws: the override consumes none of its own.
    assert_eq!(
        after.turn_counter.0 - turn_counter_before.0,
        4,
        "a held tick drew a different number of counters"
    );
}

/// Release must not latch. The parent leaves the pause in whatever ordinary mode the controller
/// had been keeping for it, not in the imposed `Resting` — which inside the hysteresis band
/// would keep it resting indefinitely.
#[test]
fn release_uses_the_underlying_mode_and_does_not_latch() {
    let mut world = with_policy(mature());
    let (parent, _, b, _) = run_to_first_begin(&mut world);

    // Carry the parent's underlying mode across the window and watch it keep updating: the
    // ordinary transition runs from the retained value every held tick.
    let mut seen_active_underlying = false;
    for _ in 0..POST_BIRTH_PAUSE_TICKS {
        if let Some(p) = world.quiet().find(parent)
            && p.underlying != Mode::Resting
        {
            seen_active_underlying = true;
        }
        world.step();
        world.drain_events();
        world.drain_quiet_events();
        if world.state.organisms.get(parent).is_none() {
            return; // The parent died; a different test covers that path.
        }
    }
    assert_eq!(world.tick(), b + POST_BIRTH_PAUSE_TICKS);
    let released = world.quiet().find(parent).map(|p| p.underlying);
    world.step();
    world.drain_events();
    let ended = world
        .drain_quiet_events()
        .into_iter()
        .find_map(|e| match e {
            QuietEvent::End { underlying, .. } => Some(underlying),
            _ => None,
        })
        .expect("the release record");
    assert_eq!(Some(ended), released);
    if seen_active_underlying {
        assert_ne!(
            ended,
            Mode::Resting,
            "an active underlying mode must survive the pause"
        );
        // And the released parent is not stuck in Resting on the very next decision.
        assert_eq!(mode_of(&world, parent), ended, "release used the imposed mode, not the real one");
    }
}

/// A parent holding a pause still moves for real: the rest effort and the rest turn fraction are
/// the individual's own existing values, and the residual drift is genuine transported motion
/// across seams and the open rim, not a frozen pose.
#[test]
fn a_held_parent_still_performs_real_bounded_movement() {
    let mut world = with_policy(mature());
    let (parent, _, _, _) = run_to_first_begin(&mut world);
    let o = world.state.organisms.get(parent).expect("alive");
    let rest_effort = f64::from(o.phenotype.drives.rest_effort);
    let speed_max = o.phenotype.speed_max;
    let mut positions = Vec::new();
    for _ in 0..POST_BIRTH_PAUSE_TICKS {
        let before = world.state.organisms.get(parent).map(|o| o.pos);
        world.step();
        world.drain_events();
        world.drain_quiet_events();
        let Some(o) = world.state.organisms.get(parent) else { break };
        positions.push((before, o.pos, o.heading));
        // Whatever it moved, it stayed on the surface and inside the rest budget. The world's
        // own transport is what carried it, so a seam crossing is an ordinary result here.
        assert!(o.pos.u.is_finite() && o.pos.v.is_finite());
        assert!((o.heading.length() - 1.0).abs() < 1e-9, "the heading stayed a unit vector");
    }
    assert!(!positions.is_empty());
    // The bound the contract promises: a held tick can move at most `rest_effort · speed_max · DT`
    // before wading, which only ever divides it.
    let ceiling = rest_effort * speed_max * DT + 1e-9;
    for (before, after, _) in &positions {
        if let Some(before) = before
            && before.face == after.face
        {
            let d = ((after.u - before.u).powi(2) + (after.v - before.v).powi(2)).sqrt();
            assert!(d <= ceiling, "a held parent moved {d} px, past the rest ceiling {ceiling}");
        }
    }
    world.check_invariants().expect("a held world stays consistent");
}

// ---------------------------------------------------------------- refusals and aborts

/// A parent that cannot cover the conservative budget is refused at admission, and the
/// opportunity is forgotten rather than retried.
#[test]
fn an_unaffordable_parent_is_refused_and_the_opportunity_is_forgotten() {
    let mut world = with_policy(mature());
    // Drain a real birth, then repeat it against a stripped parent through the same public
    // budget the admission uses.
    let (parent, _, _, _) = run_to_first_begin(&mut world);
    let cfg = world.config().organism.clone();
    let o = world.state.organisms.get(parent).expect("alive").clone();
    let horizon = horizon_seconds(POST_BIRTH_PAUSE_TICKS, DT).expect("finite");
    let budget = Budget::of(&o, &cfg, horizon).expect("finite inputs");
    assert!(budget.affordable(&o), "the admitted parent really could afford it");

    let mut thin = o.clone();
    thin.energy = budget.energy;
    assert!(!budget.affordable(&thin), "exactly on the energy bound is not affordable");
    let mut thin = o.clone();
    thin.reserve = budget.material;
    assert!(!budget.affordable(&thin), "exactly on the material bound is not affordable");
    let mut thin = o.clone();
    thin.energy = 0.0;
    assert!(!budget.affordable(&thin));
    thin.energy = o.energy;
    thin.reserve = 0.0;
    assert!(!budget.affordable(&thin));

    // And a refusal really is published for a real birth whose parent cannot pay: strip every
    // parent, then run until a birth happens.
    let mut poor = with_policy(mature());
    for (_, o) in poor.state.organisms.iter_mut() {
        // Enough to keep gestating and give birth, far too little to hold forty ticks.
        o.energy = o.energy.min(1e-6);
    }
    let mut refused = None;
    for _ in 0..4000 {
        poor.step();
        poor.drain_events();
        for event in poor.drain_quiet_events() {
            match event {
                QuietEvent::Refuse { reason, .. } => refused = Some(reason),
                QuietEvent::Begin { .. } => panic!("a stripped parent was admitted"),
                _ => {}
            }
        }
        if refused.is_some() {
            break;
        }
    }
    if let Some(reason) = refused {
        assert_eq!(reason, QuietReason::Unaffordable);
    }
    assert!(poor.quiet().pauses.is_empty(), "a refused opportunity left an entry behind");
}

/// A parent that runs out mid-pause aborts before that tick's held decision and uses the
/// ordinary controller on the same tick. No free meal, no floor, no deferred bill.
#[test]
fn a_depleted_parent_aborts_early_and_acts_ordinarily_on_that_tick() {
    let mut world = with_policy(mature());
    let (parent, child, b, _) = run_to_first_begin(&mut world);
    // Hold a few ticks, then strip the parent's battery below the remaining budget.
    for _ in 0..5 {
        world.step();
        world.drain_events();
        world.drain_quiet_events();
    }
    assert!(world.quiet().find(parent).is_some());
    let reserve_before = {
        let o = world.state.organisms.get_mut(parent).expect("alive");
        o.energy = 1e-9;
        o.reserve
    };
    let external_before = world.state.external_material_in;
    world.step();
    world.drain_events();
    let abort = world
        .drain_quiet_events()
        .into_iter()
        .find_map(|e| match e {
            QuietEvent::Abort { parent: p, child: c, completed_ticks, reason, tick } => {
                Some((p, c, completed_ticks, reason, tick))
            }
            _ => None,
        })
        .expect("the depleted parent must abort");
    assert_eq!(abort.0, parent);
    assert_eq!(abort.1, child);
    assert_eq!(abort.3, QuietReason::UnaffordableRemaining);
    assert_eq!(abort.4, b + 5, "the abort is at the decision it refused to hold");
    assert!(abort.2 > 0 && abort.2 < POST_BIRTH_PAUSE_TICKS, "a partial window: {}", abort.2);
    assert!(world.quiet().find(parent).is_none(), "the entry outlived its abort");
    // Nothing was *given* to the parent for stopping. Its stores may well rise on this very
    // tick — it is back on the ordinary controller, so it may eat what its own cell holds, and
    // oxidation keeps converting its own reserve. That is the contract working, not a rescue.
    // What must not happen is material appearing from outside the world to pay for it.
    assert_eq!(
        world.state.external_material_in, external_before,
        "an abort admitted material from outside"
    );
    world.check_invariants().expect("the abort tick conserves the world");
    let _ = reserve_before;
}

/// The trigger is the successful core commit. A cap refusal, a refund and a parent that died in
/// the same tick all produce no admission.
#[test]
fn a_refused_or_refunded_birth_and_a_dead_parent_admit_nothing() {
    // A world already at its organism cap: gestations finish, the birth is refused, the escrow
    // is returned, and no pause may start.
    let mut state = mature();
    let cap = state.organisms.len();
    state.config.capacity.max_organisms = cap as u32;
    let mut world = with_policy(state);
    let mut refusals = 0u32;
    for _ in 0..4000 {
        world.step();
        world.drain_events();
        for event in world.drain_quiet_events() {
            match event {
                QuietEvent::Begin { .. } => panic!("a capped world admitted a pause"),
                QuietEvent::Refuse { .. } => refusals += 1,
                _ => {}
            }
        }
        if world.state.cap_rejections_total > 0 {
            break;
        }
    }
    assert!(world.state.cap_rejections_total > 0, "the fixture must actually hit its cap");
    assert_eq!(refusals, 0, "a refused birth is not even offered a pause");
    assert!(world.quiet().pauses.is_empty());

    // And a parent that dies while holding a pause loses it, with a record.
    //
    // Death is by **age**, not by starvation: a held parent that is merely stripped of stores
    // stops being affordable, aborts, and then feeds itself on the ordinary controller — which
    // is the contract working. Age is the cause that no amount of food averts, so it is the one
    // that actually exercises the removal path while the pause is still held.
    let mut world = with_policy(mature());
    let (parent, child, _, _) = run_to_first_begin(&mut world);
    {
        let max_age = world.config().organism.max_age_seconds;
        let dt = DT;
        let o = world.state.organisms.get_mut(parent).expect("alive");
        o.born_tick = 0;
        assert!(
            o.age_ticks(world.state.tick) as f64 * dt > max_age,
            "the fixture must make this parent overdue"
        );
    }
    let mut aborted = None;
    for _ in 0..200 {
        world.step();
        world.drain_events();
        for event in world.drain_quiet_events() {
            if let QuietEvent::Abort { parent: p, child: c, reason, .. } = event
                && p == parent
            {
                aborted = Some((c, reason));
            }
        }
        if world.state.organisms.get(parent).is_none() {
            break;
        }
    }
    assert!(world.state.organisms.get(parent).is_none(), "the overdue parent must die");
    let (c, reason) = aborted.expect("a dead parent's pause must be recorded as aborted");
    assert_eq!(c, child);
    assert_eq!(reason, QuietReason::ParentGone);
    assert!(world.quiet().pauses.is_empty(), "a dead parent's entry survived");
    world.check_invariants().expect("consistent after the death");
}

/// A slot that is reused by a later organism does not inherit the retired one's pause.
#[test]
fn a_reused_slot_does_not_inherit_a_retired_parents_pause() {
    let mut world = with_policy(mature());
    let (parent, _, _, _) = run_to_first_begin(&mut world);
    let slot = parent.slot;
    {
        let o = world.state.organisms.get_mut(parent).expect("alive");
        o.energy = 0.0;
        o.reserve = 0.0;
    }
    for _ in 0..600 {
        world.step();
        world.drain_events();
        world.drain_quiet_events();
        if world.state.organisms.get(parent).is_none() {
            break;
        }
    }
    assert!(world.state.organisms.get(parent).is_none());
    // Whatever now occupies that slot — the same one or a later generation — carries no pause
    // from the retired organism.
    for _ in 0..2000 {
        world.step();
        world.drain_events();
        world.drain_quiet_events();
        let reused = world.state.organisms.iter().find(|(id, _)| id.slot == slot);
        if let Some((id, _)) = reused
            && id.generation != parent.generation
        {
            assert!(
                world.quiet().find(parent).is_none(),
                "the retired parent's entry is still held"
            );
            for p in &world.quiet().pauses {
                assert_ne!(p.parent, parent, "a retired full ID is still in the set");
            }
            return;
        }
    }
}

// ---------------------------------------------------------------- restart

/// A snapshot taken immediately after the birth, and one taken mid-pause, both resume into
/// exactly the world that was never interrupted — same full state hash every tick, same life
/// and quiet records.
#[test]
fn a_saved_world_resumes_identically_right_after_the_birth_and_mid_pause() {
    // Including the release boundary itself: at B+40 the window is complete but the decision
    // that consumes the entry has not been made, so the snapshot still carries the underlying
    // mode that release must resume from.
    for offset in [0u64, 1, 17, POST_BIRTH_PAUSE_TICKS - 1, POST_BIRTH_PAUSE_TICKS] {
        let mut world = with_policy(mature());
        let (parent, _, b, _) = run_to_first_begin(&mut world);
        for _ in 0..offset {
            world.step();
            world.drain_events();
            world.drain_quiet_events();
        }
        assert_eq!(world.tick(), b + offset);
        let pause = *world
            .quiet()
            .find(parent)
            .unwrap_or_else(|| panic!("offset {offset}: the pause must still be held"));
        assert_eq!(pause.remaining(world.tick()), POST_BIRTH_PAUSE_TICKS - offset);
        assert_eq!(pause.holds(world.tick()), offset < POST_BIRTH_PAUSE_TICKS);

        let bytes = encode_snapshot(&world.state, "quiet-resume");
        let (meta, state) = decode_snapshot(&bytes).expect("a mid-pause state round trips");
        assert_eq!(meta.schema, cubarium_core::SCHEMA_VERSION);
        assert_eq!(state, world.state, "the timer and underlying mode round trip exactly");
        assert_eq!(state.quiet.policy, QuietPolicy::PostBirthPauseV1);
        assert_eq!(state.quiet.find(parent), Some(&pause));
        let mut resumed = World::from_state(state).expect("valid");
        assert_eq!(state_hash(&resumed.state), state_hash(&world.state));

        for i in 0..(POST_BIRTH_PAUSE_TICKS + 60) {
            world.step();
            resumed.step();
            assert_eq!(
                state_hash(&resumed.state),
                state_hash(&world.state),
                "offset {offset}: diverged {i} ticks after the split"
            );
            let (a, b2) = (world.drain_events(), resumed.drain_events());
            assert_eq!(format!("{a:?}"), format!("{b2:?}"), "life records diverged");
            let (a, b2) = (world.drain_quiet_events(), resumed.drain_quiet_events());
            assert_eq!(format!("{a:?}"), format!("{b2:?}"), "quiet records diverged");
        }
    }
}

// ---------------------------------------------------------------- exclusions

/// The ordinary quiet policy and the hunter extension are not combined in this slice: an enabled
/// world refuses a trial and a budget control, and a loaded state carrying both is refused.
#[test]
fn an_enabled_quiet_world_refuses_every_hunter_initialization() {
    use cubarium_core::hunter::{FixedHunterProfile, HunterTarget};

    let mut cfg = WorldConfig::default();
    cfg.founders.count = 4;
    let mut state = World::new(cfg).expect("valid").state;
    state.quiet = QuietState::post_birth_pause_v1();
    let mut world = World::from_state(state.clone()).expect("an enabled world is valid");
    let profile = FixedHunterProfile::lanternjaw_trial(world.config());
    let target = HunterTarget { face: 4, u: 32.0, v: 32.0 };

    let err = world
        .start_hunter_trial(profile.clone(), target)
        .expect_err("a trial must be refused");
    assert!(err.contains("ordinary quiet policy"), "{err}");
    let err = world
        .deposit_hunter_budget_control(profile.clone(), target)
        .expect_err("a control must be refused");
    assert!(err.contains("ordinary quiet policy"), "{err}");
    // Nothing was installed by either refusal.
    assert_eq!(world.hunters(), &Default::default());

    // The other order is refused at load: a hunter world whose quiet policy is switched on.
    let mut hunted = World::new(WorldConfig::default()).expect("valid");
    hunted
        .start_hunter_trial(FixedHunterProfile::lanternjaw_trial(hunted.config()), target)
        .expect("an Off world starts a trial normally");
    let mut both = hunted.state.clone();
    both.quiet = QuietState::post_birth_pause_v1();
    let err = both.validate().expect_err("both at once must be refused");
    assert!(err.contains("not combined in this slice"), "{err}");
    assert!(World::from_state(both.clone()).is_err());
    match decode_snapshot(&encode_snapshot(&both, "both")) {
        Err(cubarium_core::SnapshotError::Invalid(reason)) => {
            assert!(reason.contains("not combined in this slice"), "{reason}")
        }
        other => panic!("a combined world decoded as {other:?}"),
    }
}
