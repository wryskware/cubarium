//! Workstream M's per-cell plant budget, tested from its **definitions** rather than from
//! what the instrumentation happens to produce
//! (`design/handoffs/ecology-v1-plant-budget-opus-2026-09-16.md`, deliverable 1).
//!
//! Astra's round-2 review, P2 on workstream I: the claim that nearly every depletion crossing
//! is an ungrazed local plant-budget failure rests on sampled visits, attributed bites and a
//! static `L·μ` proxy. The record under test replaces all three with the plant step's own
//! arithmetic and the withdrawal site's own bite. Four things have to be true before a
//! measurement made with it is worth reading:
//!
//! 1. **It moves nothing.** The same seeded world must end on the same `state_hash`,
//!    `ecology_hash` and snapshot bytes whether nobody records or somebody records, reads and
//!    checks it every tick — the same inertness proof workstreams E and H gave.
//! 2. **It closes.** Per cell, `P − P₀ = in − out − withdrawal` and the matching `Q` and `W`
//!    identities, to 1e-9, over 9,000 ticks. This is what makes it a measurement and not a
//!    proxy: every term §4 names is booked where it is computed, so a missing or
//!    double-counted term shows up as a residual.
//! 3. **The withdrawal is exact.** The per-cell withdrawal sums to the world's own independent
//!    intake counters, bite for bite, on all four §6.4 channels.
//! 4. **A plant-only world is reachable.** With `founders.kinds` empty and `founders.count`
//!    zero the world has no animals at all, so the herbivore-absent arm is a configuration
//!    and not a special code path — and its recorded withdrawal is exactly zero everywhere.

use cubarium_core::fields::PlantBudgetRecord;
use cubarium_core::snapshot::{ecology_hash, encode_snapshot, state_hash};
use cubarium_core::{World, WorldConfig};

/// The brief's tolerance for the closing identity.
const IDENTITY_TOLERANCE: f64 = 1e-9;

/// Long enough that stands die, propagules land and bodies starve: the record's death,
/// propagule and withdrawal paths are all exercised rather than assumed.
const TICKS: u64 = 9_000;

fn ordinary_world(seed: u64) -> World {
    let config = WorldConfig { seed, ..WorldConfig::default() };
    World::new(config).expect("the shipped defaults build a world")
}

/// The herbivore-absent arm's world: the same configuration with an empty founder roster.
/// `world/lifecycle.rs:52-63` — with `founders.kinds` empty the roster is `0..founders.count`,
/// so zero is an empty roster and no animal is ever placed.
fn plant_only_world(seed: u64) -> World {
    let mut config = WorldConfig { seed, ..WorldConfig::default() };
    config.founders.kinds.clear();
    config.founders.count = 0;
    World::new(config).expect("an empty founder roster is a valid world")
}

fn residual(world: &World) -> f64 {
    world.plant_budget_residual()
}

fn record(world: &World) -> &PlantBudgetRecord {
    world.plant_budget().expect("the record is open")
}

// --- 1. the record is inert -------------------------------------------------------------

/// The same seeded world three ways: never recording, recording untouched, and recording read
/// and identity-checked on every single tick. If any hash or snapshot byte differs, a
/// diagnostic has become a dynamic and nothing measured with it means anything.
#[test]
fn recording_a_plant_budget_moves_no_state_hash_no_ecology_hash_and_no_snapshot_byte() {
    let mut quiet = ordinary_world(4_242);
    for _ in 0..TICKS {
        quiet.step();
    }

    let mut recorded = ordinary_world(4_242);
    recorded.record_plant_budgets(true);
    for _ in 0..TICKS {
        recorded.step();
    }

    let mut watched = ordinary_world(4_242);
    watched.record_plant_budgets(true);
    let mut worst = 0.0f64;
    for _ in 0..TICKS {
        watched.step();
        // Read *and* check every tick: the harshest form of "somebody is looking".
        worst = worst.max(residual(&watched));
        let _ = record(&watched).cells.len();
    }

    assert_eq!(
        state_hash(&quiet.state),
        state_hash(&recorded.state),
        "the open record moved the state hash"
    );
    assert_eq!(
        state_hash(&quiet.state),
        state_hash(&watched.state),
        "reading the record every tick moved the state hash"
    );
    assert_eq!(
        ecology_hash(&quiet.state),
        ecology_hash(&watched.state),
        "the record moved the ecology hash"
    );
    assert_eq!(
        encode_snapshot(&quiet.state, "test"),
        encode_snapshot(&watched.state, "test"),
        "the record changed a snapshot byte"
    );
    assert!(
        worst < IDENTITY_TOLERANCE,
        "the per-tick identity residual reached {worst}"
    );
    // A watched world that never grew anything would prove nothing about inertness.
    assert!(
        record(&watched).cells.iter().map(|c| c.income).sum::<f64>() > 0.0,
        "the fixture recorded no plant income at all"
    );
}

/// Turning the record off frees it and leaves the world it was watching untouched, and turning
/// it on again re-anchors the identity on the stocks as they stand then.
#[test]
fn the_record_can_be_closed_and_reopened_and_the_identity_re_anchors() {
    let mut world = ordinary_world(9_001);
    for _ in 0..1_000 {
        world.step();
    }
    assert!(!world.records_plant_budgets());
    assert_eq!(world.plant_budget_residual(), 0.0, "a closed record has no residual");

    world.record_plant_budgets(true);
    assert!(world.records_plant_budgets());
    assert_eq!(record(&world).opened_at, world.tick());
    for _ in 0..1_000 {
        world.step();
    }
    assert!(residual(&world) < IDENTITY_TOLERANCE);

    // Re-opening restarts from the stocks of this tick, not from tick 0.
    world.record_plant_budgets(true);
    assert_eq!(record(&world).ticks, 0);
    assert_eq!(record(&world).p_open, world.state.fields.p);
    for _ in 0..500 {
        world.step();
    }
    assert!(residual(&world) < IDENTITY_TOLERANCE);

    world.record_plant_budgets(false);
    assert!(!world.records_plant_budgets());
    assert!(world.plant_budget().is_none());
}

// --- 2. the identities close ------------------------------------------------------------

/// `P − P₀ = (income-fed growth + reflush + propagule) − (senescence + ripening + death)
/// − withdrawal`, per cell, and the matching `Q` and `W` identities, over 9,000 ticks of an
/// ordinary world. Recomputed here from the record's own published terms rather than trusting
/// `World::plant_budget_residual`, so the test does not check the implementation against
/// itself.
#[test]
fn the_three_per_cell_identities_close_to_one_part_in_a_billion() {
    let mut world = ordinary_world(4_242);
    world.record_plant_budgets(true);
    for _ in 0..TICKS {
        world.step();
    }

    let build = world.config().plant.build;
    let rec = record(&world);
    let (p, q, w) =
        (&world.state.fields.p, &world.state.ecology.plant_reserve, &world.state.ecology.wood);
    let mut worst = 0.0f64;
    let (mut grown, mut withdrawn, mut died) = (0.0, 0.0, 0u32);
    for (i, c) in rec.cells.iter().enumerate() {
        let foliage = (p[i] - rec.p_open[i])
            - (c.foliage_from_income + c.foliage_from_reserve + c.foliage_from_propagule
                - c.senescence
                - c.ripened
                - c.death_foliage
                - c.withdrawal_foliage);
        let reserve = (q[i] - rec.q_open[i])
            - (c.reserve_share + c.reserve_refill + c.reserve_from_propagule
                - c.maintenance_from_reserve
                - (1.0 + build) * c.foliage_from_reserve
                - c.reserve_to_propagule
                - c.death_reserve);
        let wood = (w[i] - rec.w_open[i])
            - (c.wood_growth + c.wood_from_propagule - c.dieback_wood - c.death_wood);
        for (name, r) in [("P", foliage), ("Q", reserve), ("W", wood)] {
            assert!(
                r.abs() < IDENTITY_TOLERANCE,
                "cell {i}: the {name} identity is off by {r}"
            );
            worst = worst.max(r.abs());
        }
        grown += c.foliage_in();
        withdrawn += c.withdrawal_foliage;
        died += c.deaths;
    }
    assert_eq!(rec.ticks, TICKS);
    assert!(worst < IDENTITY_TOLERANCE);
    // The identity is only worth something if every branch of it actually ran.
    assert!(grown > 0.0, "no foliage was grown");
    assert!(withdrawn > 0.0, "no foliage was withdrawn");
    // Measured, and reported in the note: at the shipped defaults nothing goes unpaid, no
    // stand dies, no reflush opens and no propagule lands in 9,000 ticks. The three branches
    // this fixture cannot reach are covered by the two fixtures below rather than assumed.
    assert_eq!(died, 0, "the shipped defaults killed a stand; re-read the note's coverage claim");
}

/// The branches an ordinary world does not reach in 9,000 ticks, forced one fixture at a time,
/// because an identity that never ran through the death, dieback and reflush terms has not
/// been tested on them. Wood maintenance is raised twentyfold so stands run a deficit, and a
/// third of the cells open with no foliage so the §4.4 emergency reflush opens.
#[test]
fn a_dying_stand_a_dieback_and_a_reflush_all_close_the_identity() {
    let mut config = WorldConfig { seed: 4_242, ..WorldConfig::default() };
    config.plant.maintenance *= 20.0;
    let mut world = World::new(config).expect("a harsher maintenance is still a valid world");
    for i in 0..world.state.fields.p.len() {
        if i % 3 == 0 {
            world.state.fields.p[i] = 0.0;
        }
    }
    // Opened *after* the fixture is arranged, so `p_open` is the state the identity starts at.
    world.record_plant_budgets(true);
    for _ in 0..TICKS {
        world.step();
    }
    let rec = record(&world);
    let any = |f: fn(&cubarium_core::fields::PlantCellBudget) -> bool| rec.cells.iter().any(f);
    assert!(rec.cells.iter().map(|c| c.deaths).sum::<u32>() > 0, "no stand died");
    assert!(any(|c| c.death_foliage > 0.0), "no stand dropped foliage");
    assert!(any(|c| c.maintenance_unpaid > 0.0), "nothing went unpaid");
    assert!(any(|c| c.dieback_wood > 0.0), "nothing died back");
    assert!(any(|c| c.foliage_from_reserve > 0.0), "the §4.4 reflush never opened");
    assert!(residual(&world) < IDENTITY_TOLERANCE, "residual {}", residual(&world));
}

/// The §4.8 propagule terms: a third of the cells open bare, donation is made cheap enough to
/// happen inside the horizon, and both sides of the transfer — the donor's spent reserve and
/// the recipient's starter wood, foliage and reserve — have to close the identity.
#[test]
fn a_landed_propagule_closes_the_identity_on_both_sides() {
    let mut config = WorldConfig { seed: 4_242, ..WorldConfig::default() };
    config.plant.propagule_rate *= 200.0;
    config.plant.donor_min *= 0.2;
    config.plant.donor_reserve_floor = 0.05;
    let mut world = World::new(config).expect("a cheaper propagule is still a valid world");
    for i in 0..world.state.fields.p.len() {
        if i % 3 == 0 {
            world.state.ecology.wood[i] = 0.0;
            world.state.fields.p[i] = 0.0;
            world.state.ecology.plant_reserve[i] = 0.0;
        }
    }
    world.record_plant_budgets(true);
    for _ in 0..TICKS {
        world.step();
    }
    let rec = record(&world);
    let any = |f: fn(&cubarium_core::fields::PlantCellBudget) -> bool| rec.cells.iter().any(f);
    assert!(any(|c| c.foliage_from_propagule > 0.0), "no propagule landed");
    assert!(any(|c| c.wood_from_propagule > 0.0), "no propagule built wood");
    assert!(any(|c| c.reserve_to_propagule > 0.0), "nobody donated");
    assert!(any(|c| c.withdrawal_foliage > 0.0), "nothing grazed in this fixture");
    assert!(residual(&world) < IDENTITY_TOLERANCE, "residual {}", residual(&world));
    // §4.8 conserves: what donors spent is `(1 + c_g)` times what recipients received.
    let build = 1.0 + world.config().plant.build;
    let sent: f64 = rec.cells.iter().map(|c| c.reserve_to_propagule).sum();
    let landed: f64 = rec
        .cells
        .iter()
        .map(|c| c.wood_from_propagule + c.foliage_from_propagule + c.reserve_from_propagule)
        .sum();
    assert!(
        (sent - build * landed).abs() < IDENTITY_TOLERANCE * sent.max(1.0),
        "donors spent {sent}, recipients received {landed} net of construction"
    );
}

// --- 3. the withdrawal is exact ---------------------------------------------------------

/// The four §6.4 channels, summed over cells, equal the world's own independent intake
/// counters. Those counters are written at the same sites but into different variables and are
/// not derived from the record, so agreement to the last bit is evidence that the record books
/// every bite and only bites — which is exactly what I's attributed per-cell bites could not
/// claim.
#[test]
fn the_exact_per_cell_withdrawal_equals_the_worlds_own_intake_counters() {
    let mut world = ordinary_world(4_242);
    world.record_plant_budgets(true);
    for _ in 0..TICKS {
        world.step();
    }
    let diag = world.intake_diagnostics();
    let rec = record(&world);
    let sum = |f: fn(&cubarium_core::fields::PlantCellBudget) -> f64| -> f64 {
        rec.cells.iter().map(f).sum()
    };
    for (name, recorded, counted) in [
        ("foliage", sum(|c| c.withdrawal_foliage), diag.producer_eaten),
        ("fruit", sum(|c| c.withdrawal_fruit), diag.fruit_eaten),
        ("litter", sum(|c| c.withdrawal_litter), diag.litter_eaten),
        ("carrion", sum(|c| c.withdrawal_carrion), diag.carrion_eaten),
    ] {
        assert!(
            (recorded - counted).abs() <= 1e-12 * counted.abs().max(1.0),
            "{name}: the record has {recorded}, the world counted {counted}"
        );
    }
    assert!(diag.producer_eaten > 0.0, "nothing grazed, so the check is empty");
}

// --- 4. the plant-only arm is a configuration -------------------------------------------

/// With `founders.kinds` empty and `founders.count` zero, no animal is ever placed, so no
/// withdrawal is ever booked — while the plant step keeps running and the identities keep
/// closing. This is the herbivore-absent arm: a config, not a code path.
#[test]
fn an_empty_founder_roster_is_a_world_with_no_animals_and_no_withdrawal() {
    let mut world = plant_only_world(4_242);
    assert_eq!(world.population(), 0, "an empty roster placed an animal");
    world.record_plant_budgets(true);
    for _ in 0..TICKS {
        world.step();
    }
    assert_eq!(world.population(), 0, "an animal appeared in a plant-only world");

    let rec = record(&world);
    for (i, c) in rec.cells.iter().enumerate() {
        assert_eq!(
            (c.withdrawal_foliage, c.withdrawal_fruit, c.withdrawal_litter, c.withdrawal_carrion),
            (0.0, 0.0, 0.0, 0.0),
            "cell {i} recorded a withdrawal in a world with no mouths"
        );
    }
    assert!(residual(&world) < IDENTITY_TOLERANCE, "the identity broke without herbivores");
    assert!(
        rec.cells.iter().map(|c| c.income).sum::<f64>() > 0.0,
        "the plant step did not run without animals"
    );
    let diag = world.intake_diagnostics();
    assert_eq!(diag.producer_eaten, 0.0);
    assert_eq!(diag.request_ticks, 0, "something asked for food in a world with no mouths");
}

/// A cell's `plant_budget()` is the plant's own net, with no consumer term, and it is the
/// quantity the verdict turns on. Checked against the same cell's published parts so the
/// helper cannot drift from what the note reports.
#[test]
fn the_plant_budget_helper_is_the_plant_terms_with_no_consumer_term() {
    let mut world = plant_only_world(7);
    world.record_plant_budgets(true);
    for _ in 0..2_000 {
        world.step();
    }
    let rec = record(&world);
    let p = &world.state.fields.p;
    for (i, c) in rec.cells.iter().enumerate() {
        assert_eq!(
            c.plant_budget(),
            c.foliage_in() - c.foliage_out(),
            "cell {i}: the helper is not in − out"
        );
        // With no mouths, the plant budget *is* the change in standing foliage.
        let change = p[i] - rec.p_open[i];
        assert!(
            (c.plant_budget() - change).abs() < IDENTITY_TOLERANCE,
            "cell {i}: budget {} against ΔP {change}",
            c.plant_budget()
        );
    }
}

/// The means the record reports are means over the ticks the cell was *alive*, which is the
/// only window in which the plant step read anything at all. A cell that was never alive
/// reports zero rather than dividing by zero.
#[test]
fn the_state_means_are_over_the_ticks_the_cell_was_alive() {
    let mut world = plant_only_world(4_242);
    world.record_plant_budgets(true);
    for _ in 0..1_200 {
        world.step();
    }
    let rec = record(&world);
    let mut alive_cells = 0;
    for c in rec.cells.iter() {
        if c.ticks_alive == 0 {
            assert_eq!(c.mean(c.light_effective_sum), 0.0);
            assert_eq!(c.income, 0.0, "a cell that was never alive drew income");
            continue;
        }
        alive_cells += 1;
        assert!(c.ticks_alive <= 1_200);
        let light = c.mean(c.light_effective_sum);
        let monod = c.mean(c.monod_sum);
        assert!((0.0..=1.0).contains(&monod), "the Monod mean {monod} is not a fraction");
        assert!(light >= 0.0 && light.is_finite());
    }
    assert!(alive_cells > 0, "no cell was ever alive");
}
