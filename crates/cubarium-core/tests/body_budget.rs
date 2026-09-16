//! The per-organism store budget and the apex mating-opportunity counters, tested from their
//! **definitions** rather than from what the instrumentation happens to produce
//! (`design/handoffs/ecology-v1-budget-opus-2026-09-16.md`, deliverables 1 and 2).
//!
//! Three things have to be true before any measurement made with these is worth reading:
//!
//! 1. Recording moves nothing. Not the trajectory, not a draw, not a stored value — the same
//!    seeded world must end on the same `state_hash` whether nobody records, somebody records
//!    and never looks, or somebody records and reads and drains every single tick.
//! 2. The ledger closes. `Σ credits − Σ debits = Δ(stores)`, in material and in energy,
//!    separately, for every body over its whole recorded life.
//! 3. The opportunity counters count the predicate the world actually evaluates: a pair that
//!    mates, a pair refused by the radius, and a pair refused by readiness each land in their
//!    own bin, and the census sees the readiness overlap.

use cubarium_core::encounter::{ApexEncounterState, MATING_RADIUS_PX};
use cubarium_core::genome::decode;
use cubarium_core::hunter::{FixedHunterProfile, HunterMember, HunterTarget};
use cubarium_core::organism::Origin;
use cubarium_core::rng::Counter;
use cubarium_core::snapshot::state_hash;
use cubarium_core::{DT, OrganismId, World, WorldConfig};
use cubarium_surface::{Face, SurfacePoint};

/// A tolerance one order of magnitude tighter than the contract's own `1e-9` acceptance, on
/// stocks of order 1 m and 1 e.
const IDENTITY_TOLERANCE: f64 = 1e-10;

fn ordinary_world(seed: u64) -> World {
    let config = WorldConfig { seed, ..WorldConfig::default() };
    World::new(config).expect("the shipped defaults build a world")
}

// --- 1. recording is inert --------------------------------------------------------------

/// The same seeded world, stepped the same number of ticks, three ways: never recording,
/// recording untouched, and recording read and drained on every tick. If any of the three
/// hashes differ, a diagnostic has become a dynamic and nothing measured with it means
/// anything.
#[test]
fn recording_a_body_budget_moves_no_state_hash() {
    // Long enough that the shipped defaults actually end a life: the first starvation in this
    // fixture is around tick 6,200, and a hash test over a world where nothing died would
    // never exercise the closing path at all.
    const TICKS: u64 = 9_000;

    let mut quiet = ordinary_world(4_242);
    for _ in 0..TICKS {
        quiet.step();
    }

    let mut recorded = ordinary_world(4_242);
    recorded.record_body_budgets(true);
    for _ in 0..TICKS {
        recorded.step();
    }

    let mut watched = ordinary_world(4_242);
    watched.record_body_budgets(true);
    let mut closed_seen = 0usize;
    for _ in 0..TICKS {
        watched.step();
        // Read every live record and drain every closed one, every tick: the most intrusive
        // thing a caller can legitimately do with this API.
        let ids: Vec<OrganismId> = watched.state.organisms.iter().map(|(id, _)| id).collect();
        for id in ids {
            let _ = watched.body_budget(id).map(|b| b.served_total());
        }
        let (closed, dropped) = watched.drain_body_budgets();
        assert_eq!(dropped, 0, "a per-tick drain can never overflow the cap");
        closed_seen += closed.len();
    }

    let (a, b, c) = (
        state_hash(&quiet.state),
        state_hash(&recorded.state),
        state_hash(&watched.state),
    );
    assert_eq!(a, b, "recording moved the world");
    assert_eq!(a, c, "reading and draining the records moved the world");
    assert_eq!(quiet.population(), watched.population());
    assert!(
        closed_seen > 0,
        "the fixture is meant to kill something in {TICKS} ticks; it closed no record"
    );
}

/// Recording and *then* switching it off mid-run must also leave the world where it was.
#[test]
fn switching_recording_on_and_off_moves_no_state_hash() {
    const TICKS: u64 = 1_500;
    let mut quiet = ordinary_world(77);
    let mut toggled = ordinary_world(77);
    for tick in 0..TICKS {
        quiet.step();
        toggled.record_body_budgets(tick % 3 == 0);
        toggled.step();
    }
    assert_eq!(state_hash(&quiet.state), state_hash(&toggled.state));
}

// --- 2. the ledger closes ---------------------------------------------------------------

/// Both identities, over a whole ordinary world: for every body that died and every body still
/// alive, what went into its stores minus what came out equals how much its stores changed.
///
/// ```text
/// material:  Σ reserve_credit + gut_reserve_credit
///              − oxidation_reserve_burned − reproduction_material − injury_structure
///            = Δ(structure + reserve)
/// energy:    Σ battery_credit + gut_battery_credit + oxidation_battery_credit
///              − bill_paid − other_energy_paid − growth_energy − reproduction_energy
///            = Δ(energy)
/// ```
#[test]
fn the_store_identity_holds_for_every_body_over_its_life() {
    const TICKS: u64 = 9_000;
    let mut world = ordinary_world(31_337);
    world.record_body_budgets(true);

    let mut closed = Vec::new();
    for _ in 0..TICKS {
        world.step();
        let (batch, dropped) = world.drain_body_budgets();
        assert_eq!(dropped, 0);
        closed.extend(batch);
    }

    assert!(
        closed.len() >= 2,
        "this fixture should end at least two lives in {TICKS} ticks, it ended {}",
        closed.len()
    );
    let mut checked = 0usize;
    let mut with_intake = 0usize;
    let mut with_growth = 0usize;
    let mut with_reproduction = 0usize;
    for b in closed.iter().chain(
        world
            .state
            .organisms
            .iter()
            .filter_map(|(id, _)| world.body_budget(id)),
    ) {
        assert!(
            b.material_residual().abs() < IDENTITY_TOLERANCE,
            "body {:?} material identity off by {:e}",
            b.id,
            b.material_residual()
        );
        assert!(
            b.energy_residual().abs() < IDENTITY_TOLERANCE,
            "body {:?} energy identity off by {:e}",
            b.id,
            b.energy_residual()
        );
        checked += 1;
        with_intake += usize::from(b.served_total() > 0.0);
        with_growth += usize::from(b.growth_material > 0.0);
        with_reproduction += usize::from(b.reproduction_material.abs() > 0.0);
    }
    assert!(checked >= 3, "too few records to be a test: {checked}");
    // An identity that only ever holds for bodies that did nothing is not evidence.
    assert!(with_intake > 0, "no recorded body ever ate");
    assert!(with_growth > 0, "no recorded body ever grew");
    assert!(with_reproduction > 0, "no recorded body ever conceived");
}

/// Every closed record carries the cause the world booked, terminal stores that are the ones
/// the body actually held, and — because recording was on from tick 0 — an opening that is the
/// body's own birth, so the ledger covers the whole life rather than a suffix of it.
#[test]
fn a_closed_record_names_the_death_and_covers_the_whole_life() {
    let mut world = ordinary_world(31_337);
    world.record_body_budgets(true);
    let mut closed = Vec::new();
    for _ in 0..9_000 {
        world.step();
        closed.extend(world.drain_body_budgets().0);
    }
    assert!(!closed.is_empty(), "nothing died inside the run");
    let mut ever_ate = false;
    for b in &closed {
        assert!(b.closed_tick.is_some());
        assert!(b.death_cause.is_some());
        assert_eq!(b.opened_tick, b.born_tick, "record {:?} missed part of a life", b.id);
        assert!(b.billed_ticks > 0, "a body that lived was never billed");
        assert!(b.bill_paid <= b.bill_total + IDENTITY_TOLERANCE);
        assert!(b.end_structure >= 0.0 && b.end_reserve >= 0.0);
        ever_ate |= b.served_total() > 0.0;
    }
    assert!(ever_ate, "no closed record ever recorded a bite");
}

/// The three billed terms are the bill's own three terms: upkeep plus translation plus turning
/// reproduces `MotorBill::total_cost` to floating-point association, over a whole life.
#[test]
fn the_billed_terms_reconstruct_the_bill_the_world_booked() {
    let mut world = ordinary_world(5_150);
    world.record_body_budgets(true);
    for _ in 0..2_000 {
        world.step();
    }
    let mut seen = 0;
    for (id, _) in world.state.organisms.iter() {
        let Some(b) = world.body_budget(id) else { continue };
        let split = b.upkeep_billed + b.motor_translation_billed + b.motor_turn_billed;
        let scale = b.bill_total.abs().max(1.0);
        assert!(
            (split - b.bill_total).abs() < 1e-12 * scale * b.billed_ticks as f64,
            "split {split:e} against booked {:e}",
            b.bill_total
        );
        seen += 1;
    }
    assert!(seen > 0);
}

// --- 3. the apex opportunity counters ---------------------------------------------------

const A: SurfacePoint = SurfacePoint { face: Face::Top, u: 24.0, v: 24.0 };

/// The quiet two-apex arena the encounter tests use: no weather, no plants, no upkeep, so the
/// only thing that happens is the encounter under test.
fn apex_world(interval_ticks: u64) -> World {
    let mut cfg = WorldConfig::default();
    cfg.capacity.max_organisms = 16;
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
    let mut world = World::new(cfg).expect("quiet world");
    world.state.apex_encounters = ApexEncounterState::paired_v1();
    let mut profile = FixedHunterProfile::lanternjaw_trial(world.config());
    profile.attacks_enabled = false;
    profile.reproduce_min_age_seconds = 0.0;
    profile.gestation_seconds = 400.0 * DT;
    profile.reproduce_interval_seconds = interval_ticks as f64 * DT;
    world
        .start_hunter_trial(
            profile,
            HunterTarget { face: A.face.index() as u8, u: A.u, v: A.v },
        )
        .expect("the first adult");
    world
}

/// A second adult member, a clone of the first, placed at `pos`.
fn second_adult(world: &mut World, pos: SurfacePoint) -> OrganismId {
    let cfg = world.config().clone();
    let first = world.hunters().members[0].id;
    let mut o = world.state.organisms.get(first).expect("the first adult").clone();
    o.pos = pos;
    o.genome.clamp();
    o.phenotype = decode(&o.genome, &cfg.organism);
    o.phenotype.extent = world.hunters().profile().expect("profile").body_extent_px;
    o.structure = o.phenotype.structure_adult;
    o.reserve = o.phenotype.reserve_max;
    o.energy = o.phenotype.energy_max;
    o.born_tick = world.tick();
    o.escrow = None;
    o.births = 0;
    o.parent = None;
    o.origin = Origin::Founder;
    o.turn_counter = Counter::default();
    let imported = o.structure + o.reserve;
    let id = world.state.organisms.insert(o);
    assert!(world.state.hunters.insert_member(HunterMember::new(id, world.tick())));
    world.state.external_material_in += imported;
    id
}

fn fill(world: &mut World, id: OrganismId) {
    let o = world.state.organisms.get_mut(id).expect("a live adult");
    let before = o.reserve;
    o.reserve = o.phenotype.reserve_max;
    o.energy = o.phenotype.energy_max;
    world.state.external_material_in += o.reserve - before;
}

/// Two ready adults half a pixel apart conceive, and that is what the counters say: one
/// candidate, one mating, no failure bin touched, and a census that saw the readiness overlap
/// and measured the pair's separation.
#[test]
fn two_ready_adults_within_the_radius_are_counted_as_a_mating() {
    let mut world = apex_world(20);
    let first = world.hunters().members[0].id;
    fill(&mut world, first);
    let second = second_adult(&mut world, SurfacePoint { face: Face::Top, u: 24.5, v: 24.0 });
    world.step();

    let o = world.apex_opportunity();
    assert_eq!(o.matings, 1, "the pair conceived but the counter says {}", o.matings);
    assert_eq!(o.pair_candidates, 1);
    assert_eq!(o.fail_radius + o.fail_ready_a + o.fail_ready_b, 0);
    assert_eq!(o.fail_capacity + o.fail_contribution + o.fail_partner_used, 0);
    assert_eq!(o.ticks_sampled, 1);
    assert_eq!(o.max_adults_alive, 2);
    assert_eq!(o.max_ready, 2);
    assert_eq!(o.ticks_two_ready, 1);
    assert_eq!(o.ticks_ready_pair_within_radius, 1);
    let d = o.min_ready_distance_px.expect("two ready adults have a distance");
    assert!((d - 0.5).abs() < 1e-6, "min ready distance {d}");
    assert!(world.state.organisms.get(second).is_some());
    // And a drain resets it.
    let drained = world.drain_apex_opportunity();
    assert_eq!(drained, o);
    assert_eq!(world.apex_opportunity().ticks_sampled, 0);
    assert_eq!(world.apex_opportunity().min_ready_distance_px, None);
}

/// The same two ready adults, too far apart to mate but still inside each other's sensing:
/// the candidate pair is formed and the radius is the first thing it fails. The census records
/// the readiness overlap anyway, which is the distinction the audit exists to make.
#[test]
fn two_ready_adults_beyond_the_radius_fail_on_the_radius_alone() {
    let far = 3.0 * MATING_RADIUS_PX;
    let mut world = apex_world(20);
    let first = world.hunters().members[0].id;
    fill(&mut world, first);
    let _ = second_adult(&mut world, SurfacePoint { face: Face::Top, u: A.u + far, v: A.v });
    world.step();

    let o = world.apex_opportunity();
    assert_eq!(o.matings, 0);
    assert_eq!(o.pair_candidates, 1, "the two sense each other, so a candidate is formed");
    assert_eq!(o.fail_radius, 1);
    assert_eq!(o.fail_ready_a + o.fail_ready_b + o.fail_contribution, 0);
    assert_eq!(o.ticks_two_ready, 1, "both were ready; only the distance stopped them");
    assert_eq!(o.ticks_ready_pair_within_radius, 0);
    let d = o.min_ready_distance_px.expect("measurable within one local unfolding");
    assert!((d - far).abs() < 1e-6, "min ready distance {d} against {far}");
}

/// One of the two is not ready — its reserve is below the profile's fraction — so the pair
/// fails on readiness, not on distance, although it is standing half a pixel away. A campaign
/// that recorded only "no mating" could not tell this run from the one above.
#[test]
fn a_pair_that_is_close_but_unready_fails_on_readiness_not_the_radius() {
    let mut world = apex_world(20);
    let first = world.hunters().members[0].id;
    fill(&mut world, first);
    let second = second_adult(&mut world, SurfacePoint { face: Face::Top, u: 24.5, v: 24.0 });
    {
        // Strip the *higher* id's reserve: the pass tests `a_ready` for the lower id first.
        let (low, high) = if first < second { (first, second) } else { (second, first) };
        let _ = low;
        let o = world.state.organisms.get_mut(high).expect("a live adult");
        let before = o.reserve;
        o.reserve = 0.0;
        world.state.external_material_in -= before;
    }
    world.step();

    let o = world.apex_opportunity();
    assert_eq!(o.matings, 0);
    assert_eq!(o.pair_candidates, 1);
    assert_eq!(o.fail_radius, 0, "they were well inside the radius");
    assert_eq!(o.fail_ready_b, 1, "the second of the ordered pair is the unready one");
    assert_eq!(o.fail_ready_a, 0);
    assert_eq!(o.ticks_two_ready, 0, "there was never a readiness overlap");
    assert_eq!(o.max_ready, 1);
    assert_eq!(o.max_adults_alive, 2, "both were adult and perched all the same");
    assert_eq!(o.min_ready_distance_px, None);
}

/// The counters are as inert as the budgets: a world that is asked for them every tick ends on
/// the same hash as one that never is.
#[test]
fn reading_the_opportunity_counters_moves_no_state_hash() {
    const TICKS: u64 = 600;
    let mut quiet = apex_world(20);
    let first = quiet.hunters().members[0].id;
    fill(&mut quiet, first);
    let _ = second_adult(&mut quiet, SurfacePoint { face: Face::Top, u: 30.0, v: 24.0 });

    let mut watched = apex_world(20);
    let first = watched.hunters().members[0].id;
    fill(&mut watched, first);
    let _ = second_adult(&mut watched, SurfacePoint { face: Face::Top, u: 30.0, v: 24.0 });

    for _ in 0..TICKS {
        quiet.step();
        watched.step();
        let _ = watched.drain_apex_opportunity();
    }
    assert_eq!(state_hash(&quiet.state), state_hash(&watched.state));
}

// --- 4. what recording costs ------------------------------------------------------------

/// The per-tick price of recording, measured rather than asserted.
///
/// Ignored because it is a timing study, not a check: run it with
/// `cargo test -p cubarium-core --release --test body_budget -- --ignored --nocapture`.
///
/// Two shapes, because the one the brief names is the cheap one. The **plant-only** world is
/// the `ecology_v1_scenarios` B0 shape — a landscape with no animals in it at all — where
/// recording is by construction two empty loops and a boolean. The **whole default world**
/// carries 46–255 bodies and is where the two per-tick passes and the per-bite writes actually
/// cost something; that is the number to quote.
#[test]
#[ignore = "timing study, not a check"]
fn how_much_recording_costs() {
    fn plant_only(seed: u64) -> World {
        let mut config = WorldConfig { seed, ..WorldConfig::default() };
        config.founders.kinds.clear();
        config.founders.count = 0;
        World::new(config).expect("a world with no animals")
    }

    fn ticks_per_second(mut world: World, on: bool, ticks: u64) -> f64 {
        world.record_body_budgets(on);
        // Warm the caches on a tenth of the run before the clock starts.
        for _ in 0..ticks / 10 {
            world.step();
        }
        let start = std::time::Instant::now();
        for _ in 0..ticks {
            world.step();
            if on {
                let _ = world.drain_body_budgets();
            }
        }
        ticks as f64 / start.elapsed().as_secs_f64()
    }

    // Alternating A/B repeats, best-of: this machine runs three workers and one shared
    // `target/`, so a single pair of timings is dominated by whatever else is compiling. The
    // best time each arm achieved is the one least contaminated by that.
    for (name, build) in [
        ("plant-only (B0 shape)", plant_only as fn(u64) -> World),
        ("whole default world", ordinary_world as fn(u64) -> World),
    ] {
        let ticks = 10_000;
        let (mut best_off, mut best_on) = (0.0f64, 0.0f64);
        for _ in 0..5 {
            best_off = best_off.max(ticks_per_second(build(31_337), false, ticks));
            best_on = best_on.max(ticks_per_second(build(31_337), true, ticks));
        }
        println!(
            "{name}: {best_off:.0} ticks/s off, {best_on:.0} ticks/s on, {:+.2} %",
            100.0 * (best_on / best_off - 1.0)
        );
    }
}
