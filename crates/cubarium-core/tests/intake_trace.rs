//! The per-tick intake trace, tested from its **definitions** rather than from what the
//! instrumentation happens to produce
//! (`design/handoffs/ecology-v1-intake-opus-2026-09-16.md`, deliverables 1 and 2).
//!
//! The trace exists to separate three explanations for a controller that travels and does not
//! eat: its mouths are shut, it is never on food, or its bite is being clamped. Four things
//! have to be true before any of that is worth reading:
//!
//! 1. **Tracing moves nothing.** The same seeded world must end on the same `state_hash`
//!    whether nobody traces, somebody traces and never looks, or somebody traces and drains
//!    every single tick — and nothing about the trace may reach a snapshot.
//! 2. **A served bite is the world's own bite.** On a tick where the mouth is open, the cell
//!    holds food and a bite is served, the row's `served` is the same number the per-organism
//!    ledger booked, and it is the contract's `mouth_rate · effort · dt · S(food)` (§6.3–6.4).
//! 3. **A bare cell says so.** The limiting term on a cell below the world's own
//!    `drives.feed_min` is `StockBelowThreshold`, whether or not a crumb was still served —
//!    the threshold is a description of the cell, never a gate on the mouth.
//! 4. **A shut mouth says so**, and so does a body with no gut for the food and a body with no
//!    room left in its reserve.

use cubarium_core::diagnostic::ScriptedIntent;
use cubarium_core::genome::{Genome, decode};
use cubarium_core::ids::OrganismId;
use cubarium_core::organism::{Mode, Organism, Origin};
use cubarium_core::rng::Counter;
use cubarium_core::snapshot::state_hash;
use cubarium_core::{
    CARRION, DT, FOLIAGE, FRUIT, IntakeLimit, IntakeTick, LITTER, MOUTH_FRUIT, MOUTH_GRAZE,
    MOUTH_SCAVENGE, World, WorldConfig, encode_snapshot,
};
use cubarium_surface::{CellId, Face, Vec2, cell_of};

fn ordinary_world(seed: u64) -> World {
    let config = WorldConfig { seed, ..WorldConfig::default() };
    World::new(config).expect("the shipped defaults build a world")
}

// --- 1. tracing is inert ------------------------------------------------------------------

/// The same seeded world, stepped the same number of ticks, three ways: never tracing, tracing
/// untouched, and tracing drained on every tick. A diagnostic that moved the trajectory would
/// make every measurement taken through it worthless.
#[test]
fn tracing_intake_moves_no_state_hash() {
    // Long enough that the shipped defaults actually end lives, so the traced body's own
    // removal and the slot reuse behind it are both exercised.
    const TICKS: u64 = 9_000;

    let first = |w: &World| w.state.organisms.iter().next().map(|(id, _)| id).expect("a body");

    let mut quiet = ordinary_world(4_242);
    for _ in 0..TICKS {
        quiet.step();
    }

    let mut traced = ordinary_world(4_242);
    let target = first(&traced);
    traced.trace_intake(Some(target));
    for _ in 0..TICKS {
        traced.step();
    }
    let (rows, dropped) = traced.intake_trace();
    assert!(!rows.is_empty(), "the traced body recorded nothing at all");
    assert!(dropped > 0, "an undrained trace over {TICKS} ticks must have hit its cap");

    let mut watched = ordinary_world(4_242);
    watched.trace_intake(Some(first(&watched)));
    // The ledger on as well, because the experiment runs both together and the two must not
    // interfere with each other either.
    watched.record_body_budgets(true);
    let mut seen = 0usize;
    for _ in 0..TICKS {
        watched.step();
        let (rows, dropped) = watched.drain_intake_trace();
        assert_eq!(dropped, 0, "a per-tick drain can never overflow the cap");
        seen += rows.len();
        let _ = watched.drain_body_budgets();
    }
    assert!(seen > 0, "draining every tick saw no row");

    let (a, b, c) = (
        state_hash(&quiet.state),
        state_hash(&traced.state),
        state_hash(&watched.state),
    );
    assert_eq!(a, b, "tracing moved the world");
    assert_eq!(a, c, "tracing, recording and draining moved the world");
    assert_eq!(quiet.population(), watched.population());
}

/// Switching tracing on and off mid-run must also leave the world where it was, and must never
/// hand back a trace with two bodies' ticks in it.
#[test]
fn switching_tracing_on_and_off_moves_no_state_hash() {
    const TICKS: u64 = 1_500;
    let mut quiet = ordinary_world(77);
    let mut toggled = ordinary_world(77);
    let ids: Vec<OrganismId> = toggled.state.organisms.iter().map(|(id, _)| id).take(2).collect();
    for tick in 0..TICKS {
        quiet.step();
        toggled.trace_intake(match tick % 3 {
            0 => None,
            1 => Some(ids[0]),
            _ => Some(ids[1]),
        });
        toggled.step();
    }
    assert_eq!(state_hash(&quiet.state), state_hash(&toggled.state));
    // Changing the target discards the rows, so whatever is left is one body's.
    let (rows, _) = toggled.intake_trace();
    assert!(rows.len() <= 1, "a target change kept another body's rows");
}

/// Nothing about the trace reaches the persisted state, and a reloaded world starts empty.
#[test]
fn the_trace_is_never_persisted() {
    let mut plain = ordinary_world(9);
    let mut traced = ordinary_world(9);
    let target = traced.state.organisms.iter().next().expect("a body").0;
    traced.trace_intake(Some(target));
    for _ in 0..200 {
        plain.step();
        plain.drain_events();
        traced.step();
        traced.drain_events();
    }
    assert_eq!(
        encode_snapshot(&traced.state, "trace"),
        encode_snapshot(&plain.state, "trace"),
        "the intake trace leaked into the persisted state"
    );
    let reloaded = World::from_state(traced.state.clone()).expect("valid");
    assert_eq!(reloaded.traced_intake(), None, "a reloaded world traces somebody");
    assert!(reloaded.intake_trace().0.is_empty(), "a reloaded world carries rows");
}

// --- a probe: one body alone on an emptied face --------------------------------------------

fn calm(diet: f32) -> WorldConfig {
    let mut c = WorldConfig::default();
    c.weather.amplitude = 0.0;
    c.water.rain_rate = 0.0;
    c.founders.kinds.clear();
    c.founders.count = 0;
    // Oxidation off, so a hand-built tick's stores move only by what the mouth did.
    c.organism.oxidation_rate = 0.0;
    let _ = diet;
    c
}

/// A probe standing alone on an emptied world with `foliage` painted as a live stand under it,
/// so nothing but the script and the cell drive what it eats.
fn probe(config: WorldConfig, cell: CellId, diet: f32, foliage: f64) -> (World, OrganismId) {
    let mut world = World::new(config).expect("valid");
    let mut removed = 0.0;
    for c in CellId::all() {
        let i = c.index();
        let f = &mut world.state.fields;
        removed += f.p[i] + f.f[i] + f.d[i];
        f.p[i] = 0.0;
        f.f[i] = 0.0;
        f.d[i] = 0.0;
        f.de[i] = 0.0;
        let e = &mut world.state.ecology;
        removed += e.wood[i] + e.plant_reserve[i] + e.dead_wood[i] + e.carrion[i];
        e.wood[i] = 0.0;
        e.plant_reserve[i] = 0.0;
        e.dead_wood[i] = 0.0;
        e.carrion[i] = 0.0;
        e.carrion_energy[i] = 0.0;
    }
    world.state.external_material_in -= removed;

    let cfg = world.config().clone();
    if foliage > 0.0 {
        // A live stand, exactly as the ES fixture paints one: the foliage, the wood that can
        // carry it and the reserve that wood holds.
        let i = cell.index();
        let w = foliage / cfg.plant.alpha;
        let q = cfg.plant.reserve_cap * w;
        world.state.fields.p[i] = foliage;
        world.state.ecology.wood[i] = w;
        world.state.ecology.plant_reserve[i] = q;
        world.state.external_material_in += foliage + w + q;
    }

    let mut genome = Genome::founder(0.5, &cfg.drives);
    genome.diet = diet;
    let phenotype = decode(&genome, &cfg.organism);
    let structure = phenotype.structure_adult;
    let reserve = 0.5 * phenotype.reserve_max;
    let id = world.state.organisms.insert(Organism {
        pos: cell.center(),
        heading: Vec2::new(1.0, 0.0),
        ou: Vec2::ZERO,
        structure,
        reserve,
        energy: 0.75 * phenotype.energy_max,
        born_tick: 0,
        hunger_memory: 1.0,
        mode: Mode::Seeking,
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
    let mut world = World::from_state(world.state).expect("the staged state is valid");
    world.check_invariants().expect("staged world is consistent");
    world.record_body_budgets(true);
    world.trace_intake(Some(id));
    (world, id)
}

/// One scripted tick with the three efforts named, and the row it produced.
fn one_tick(world: &mut World, id: OrganismId, graze: f64, fruit: f64, scavenge: f64) -> IntakeTick {
    let heading = world.state.organisms.get(id).expect("alive").heading;
    world.set_scripted_intents(vec![(id, ScriptedIntent {
        heading: Some(heading),
        effort: Some(0.0),
        graze_effort: Some(graze),
        fruit_effort: Some(fruit),
        scavenge_effort: Some(scavenge),
        mode: None,
        bud: Some(false),
    })]);
    world.step();
    world.drain_events();
    let (rows, dropped) = world.drain_intake_trace();
    assert_eq!(dropped, 0);
    assert_eq!(rows.len(), 1, "one tick, one row");
    rows[0]
}

// --- 2. a served bite is the world's own bite ---------------------------------------------

/// The mouth is open, the cell holds food well above the threshold, a bite is served: the row's
/// served material is the ledger's served material, it is the contract's type-II mouth bite,
/// and the limiting term is the mouth rate.
#[test]
fn an_open_mouth_on_a_fed_cell_is_served_the_mouth_rate_and_the_ledger_agrees() {
    let cell = CellId::new(Face::Top, 8, 8);
    let (mut world, id) = probe(calm(0.5), cell, 0.5, 1.0);
    let (rate, k_p) = {
        let o = world.state.organisms.get(id).expect("placed");
        (o.phenotype.mouth_rate, world.config().organism.intake_half_saturation)
    };

    let row = one_tick(&mut world, id, 1.0, 0.0, 0.0);

    assert_eq!(cell_of(&world.state.organisms.get(id).expect("alive").pos), cell);
    assert_eq!(row.cell, cell.0, "the row names the cell the body fed in");
    assert!(row.above_threshold[FOLIAGE], "a 1.0 stand is above `feed_min`");
    assert_eq!(row.effort[MOUTH_GRAZE], 1.0);
    assert_eq!(row.effort_normalised[MOUTH_GRAZE], 1.0, "one open mouth is not normalised");
    assert_eq!(row.raw_head, None, "a scripted body has no network head");

    // §6.3–6.4, from the row's own recorded stock: one mouth, one rate, type-II saturation.
    let food = row.stock[FOLIAGE];
    let expected = rate * 1.0 * DT * (food / (food + k_p));
    assert!(
        (row.mouth_bite[MOUTH_GRAZE] - expected).abs() < 1e-15,
        "mouth bite {} against the contract's {expected}",
        row.mouth_bite[MOUTH_GRAZE]
    );
    assert!(row.served[FOLIAGE] > 0.0, "an open mouth on a fed cell ate nothing");
    assert!(
        (row.served[FOLIAGE] - expected).abs() < 1e-15,
        "served {} against the requested {expected}",
        row.served[FOLIAGE]
    );
    assert_eq!(row.limit[MOUTH_GRAZE], IntakeLimit::MouthRate);

    // The row and the per-organism ledger are two records of the same transfer.
    let ledger = world.body_budget(id).expect("recording").served[FOLIAGE];
    assert_eq!(row.served[FOLIAGE], ledger, "the row and the ledger disagree on the bite");
    let ledger = *world.body_budget(id).expect("recording");
    assert_eq!(row.digestible[FOLIAGE], ledger.digestible[FOLIAGE]);
    assert_eq!(row.reserve_credit[FOLIAGE], ledger.reserve_credit[FOLIAGE]);
    assert_eq!(row.battery_credit[FOLIAGE], ledger.battery_credit[FOLIAGE]);
    assert_eq!(row.bill_total, ledger.bill_total, "the row and the ledger disagree on the bill");

    // The untouched channels are named for what they are, not left blank.
    assert_eq!(row.limit[MOUTH_FRUIT], IntakeLimit::EffortZero);
    assert_eq!(row.limit[MOUTH_SCAVENGE], IntakeLimit::EffortZero);
    assert_eq!(row.served[FRUIT], 0.0);
    assert_eq!(row.served[LITTER] + row.served[CARRION], 0.0);
}

// --- 3. a bare cell says so ----------------------------------------------------------------

/// A mouth wide open on a cell with nothing in it: the limiting term is the stock, not the
/// mouth.
#[test]
fn an_open_mouth_on_a_bare_cell_names_the_stock() {
    let cell = CellId::new(Face::Top, 8, 8);
    let (mut world, id) = probe(calm(0.5), cell, 0.5, 0.0);
    let row = one_tick(&mut world, id, 1.0, 0.0, 0.0);
    assert_eq!(row.stock[FOLIAGE], 0.0, "the probe's cell is bare");
    assert!(!row.above_threshold[FOLIAGE]);
    assert_eq!(row.effort[MOUTH_GRAZE], 1.0, "the mouth was open");
    assert_eq!(row.served[FOLIAGE], 0.0);
    assert_eq!(row.limit[MOUTH_GRAZE], IntakeLimit::StockBelowThreshold);
}

/// The threshold describes the cell; it does not gate the mouth. A stand *below* `feed_min` but
/// above zero still serves a crumb, and the term still names the stock — which is the honest
/// reading, because at that density the mouth's own rate is not what is costing the body.
#[test]
fn a_thin_cell_names_the_stock_even_though_it_still_serves() {
    let cell = CellId::new(Face::Top, 4, 4);
    let feed_min = WorldConfig::default().drives.feed_min;
    let (mut world, id) = probe(calm(0.5), cell, 0.5, 0.5 * feed_min);
    let row = one_tick(&mut world, id, 1.0, 0.0, 0.0);
    assert!(row.stock[FOLIAGE] > 0.0 && row.stock[FOLIAGE] < feed_min);
    assert!(!row.above_threshold[FOLIAGE]);
    assert!(row.served[FOLIAGE] > 0.0, "a thin cell still serves what it has");
    assert_eq!(row.limit[MOUTH_GRAZE], IntakeLimit::StockBelowThreshold);
    assert_eq!(row.feed_threshold, feed_min);
}

// --- 4. a shut mouth, an absent gut and a full reserve all say so --------------------------

/// The cell is full, the mouth is shut: the limiting term is the effort, and the row still
/// records that the body was standing on food — which is exactly the tick the experiment is
/// looking for.
#[test]
fn a_shut_mouth_on_a_fed_cell_names_the_effort() {
    let cell = CellId::new(Face::Top, 8, 8);
    let (mut world, id) = probe(calm(0.5), cell, 0.5, 1.0);
    let row = one_tick(&mut world, id, 0.0, 0.0, 0.0);
    assert!(row.above_threshold[FOLIAGE], "the body was standing on food");
    assert_eq!(row.effort[MOUTH_GRAZE], 0.0);
    assert_eq!(row.mouth_bite[MOUTH_GRAZE], 0.0);
    assert_eq!(row.served[FOLIAGE], 0.0);
    assert_eq!(row.limit[MOUTH_GRAZE], IntakeLimit::EffortZero);
    assert!(row.on_food(), "the row must still say the cell was food");
    assert!(!row.mouth_open(FOLIAGE, 0.05));
    // The bill is still levied on a tick nothing was eaten.
    assert!(row.bill_total > 0.0, "a living body is billed whether or not it ate");
}

/// A body with no detrital machinery asking to scavenge: the world refuses the channel, and
/// the term names the body rather than the cell or the controller.
#[test]
fn a_channel_with_no_machinery_names_the_capability() {
    let cell = CellId::new(Face::Top, 8, 8);
    // `capability_gate = 0.2`: at `diet = 0.9` the detrital side is 0.1 and is closed outright.
    let (mut world, id) = probe(calm(0.9), cell, 0.9, 1.0);
    assert_eq!(world.state.organisms.get(id).expect("placed").phenotype.cap_detrital, 0.0);
    let row = one_tick(&mut world, id, 0.0, 0.0, 1.0);
    assert_eq!(row.effort[MOUTH_SCAVENGE], 1.0, "the controller did ask");
    assert_eq!(row.effort_normalised[MOUTH_SCAVENGE], 0.0, "the world refused it");
    assert_eq!(row.limit[MOUTH_SCAVENGE], IntakeLimit::CapabilityZero);
    assert_eq!(row.served[LITTER] + row.served[CARRION], 0.0);
}

/// A full reserve is the body's own refusal, not the cell's: the term names the headroom.
#[test]
fn a_full_reserve_names_the_headroom() {
    let cell = CellId::new(Face::Top, 8, 8);
    let (mut world, id) = probe(calm(0.5), cell, 0.5, 1.0);
    {
        let o = world.state.organisms.get_mut(id).expect("placed");
        let full = o.phenotype.reserve_max;
        world.state.external_material_in += full - o.reserve;
        o.reserve = full;
    }
    let row = one_tick(&mut world, id, 1.0, 0.0, 0.0);
    assert_eq!(row.reserve_headroom, 0.0);
    assert!(row.mouth_bite[MOUTH_GRAZE] > 0.0, "the mouth still asked");
    assert_eq!(row.requested[MOUTH_GRAZE], 0.0, "and the headroom clamped it to nothing");
    assert_eq!(row.served[FOLIAGE], 0.0);
    assert_eq!(row.limit[MOUTH_GRAZE], IntakeLimit::ReserveHeadroom);
}

/// Two bodies on one thin cell: neither is stopped by its own mouth, and the term says the cell
/// could not serve what was asked.
#[test]
fn a_cell_that_cannot_serve_the_request_names_the_share() {
    let cell = CellId::new(Face::Top, 8, 8);
    let feed_min = WorldConfig::default().drives.feed_min;
    // Above the threshold, and far below one mouth-tick: the share, not the threshold, binds.
    let stand = feed_min * 1.05;
    let (mut world, id) = probe(calm(0.5), cell, 0.5, stand);
    // A mouth large enough to ask for more than the cell holds and still small enough that
    // the reserve's own room is not what stops it: at `K_P = 0.45` and `dt = 0.05` this asks
    // for about 0.32 m against a 0.21 m stand and a 0.5 m headroom, so the cell's share is the
    // only term that can bind.
    {
        let o = world.state.organisms.get_mut(id).expect("placed");
        o.phenotype.mouth_rate = 20.0;
    }
    let row = one_tick(&mut world, id, 1.0, 0.0, 0.0);
    assert!(row.above_threshold[FOLIAGE], "the cell is above the threshold");
    assert!(row.requested[MOUTH_GRAZE] > row.served[FOLIAGE], "the request was not met");
    assert!(row.served[FOLIAGE] > 0.0, "the cell served what it had");
    assert_eq!(row.limit[MOUTH_GRAZE], IntakeLimit::StockShare);
}

// --- 5. a neural body's raw action, before the adapter touched it --------------------------

/// For a neural body the row carries the linear head **before** squashing, deadband, masking
/// and the shared-mouth normalisation — which is the only place the difference between "the
/// policy asked for nothing" and "the adapter refused what it asked for" is visible.
///
/// The cadence is the animal's own: the head appears on the ticks its controller actually ran
/// and is absent on the ticks that reused the held action.
#[test]
fn a_neural_bodys_row_carries_the_head_before_the_adapter() {
    use cubarium_core::neural::action::{Action7, Capability, GRAZE};
    use cubarium_core::neural::gru::Gru32;
    use cubarium_core::neural::Policy;

    let cell = CellId::new(Face::Top, 8, 8);
    let (mut world, id) = probe(calm(0.5), cell, 0.5, 1.0);
    // A bias-only head: every weight is zero, so the head is the bias whatever it observes.
    let mut w = Gru32::zeros();
    let bias: [f64; 7] = [-8.0, 0.0, 2.0, -8.0, -8.0, -8.0, -8.0];
    w.b_o[..7].copy_from_slice(&bias);
    world.attach_neural_policy(id, Policy::new(w)).expect("attach");

    let (cap_foliage, cap_detrital) = {
        let o = world.state.organisms.get(id).expect("placed");
        (o.phenotype.cap_foliage, o.phenotype.cap_detrital)
    };
    let mechanisms = world.config().mechanisms.clone();
    let capability =
        Capability::ordinary(cap_foliage, cap_detrital, mechanisms.grazing, mechanisms.scavenging);
    let head: [f64; 7] = bias;
    let expected = Action7::squash(&head, &capability);

    let mut with_head = 0;
    let mut without = 0;
    for _ in 0..8 {
        world.step();
        world.drain_events();
        let (rows, _) = world.drain_intake_trace();
        let row = rows[0];
        match row.raw_head {
            Some(h) => {
                assert_eq!(h, head, "the row carries a head the policy did not produce");
                with_head += 1;
            }
            None => without += 1,
        }
        // Whether or not the controller ran this tick, the efforts in force are the adapter's.
        assert!(
            (row.effort[MOUTH_GRAZE] - expected.0[GRAZE]).abs() < 1e-15,
            "decoded graze {} against the adapter's {}",
            row.effort[MOUTH_GRAZE],
            expected.0[GRAZE]
        );
        assert_eq!(row.effort[MOUTH_FRUIT], 0.0, "σ(−8) is inside the deadband");
        assert_eq!(row.effort[MOUTH_SCAVENGE], 0.0);
    }
    assert!(with_head > 0, "no tick recorded a head");
    assert!(without > 0, "every tick ran the controller; the cadence is not being shown");
}

// --- 6. what the trace costs ---------------------------------------------------------------

/// What tracing costs, measured rather than assumed. Not a check — run it by name:
/// `cargo test -p cubarium-core --release --test intake_trace -- --ignored --nocapture`.
///
/// The whole default world is the number to quote: 46–255 bodies, of which exactly one is
/// traced, which is what the experiment does.
#[test]
#[ignore = "timing study, not a check"]
fn how_much_tracing_costs() {
    fn ticks_per_second(mut world: World, on: bool, ticks: u64) -> f64 {
        if on {
            let id = world.state.organisms.iter().next().expect("a body").0;
            world.record_body_budgets(true);
            world.trace_intake(Some(id));
        }
        for _ in 0..ticks / 10 {
            world.step();
        }
        let start = std::time::Instant::now();
        for _ in 0..ticks {
            world.step();
            if on {
                let _ = world.drain_intake_trace();
                let _ = world.drain_body_budgets();
            }
        }
        ticks as f64 / start.elapsed().as_secs_f64()
    }

    // Alternating A/B repeats, best-of: this machine runs several workers against one shared
    // `target/`, so a single pair of timings is dominated by whatever else is compiling.
    let ticks = 10_000;
    let (mut off, mut on) = (0.0f64, 0.0f64);
    for _ in 0..5 {
        off = off.max(ticks_per_second(ordinary_world(4_242), false, ticks));
        on = on.max(ticks_per_second(ordinary_world(4_242), true, ticks));
    }
    println!(
        "whole default world: {off:.0} ticks/s off, {on:.0} ticks/s with the trace and the \
         ledger on and drained every tick ({:+.2} %)",
        100.0 * (on - off) / off
    );
}
