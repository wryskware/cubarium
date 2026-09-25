//! **Independent** checks of the phase-one model rules as they run in the *real* static
//! arena (package P2-T), written from `design/voxel-senses-phase1-plan.md` and
//! `design/voxel-senses-phase1-tests.md` §1 by an author who did not write the code under
//! test.
//!
//! The companion file `cubarium-voxel-fauna/tests/independent_rules.rs` checks the same
//! rules on hand-built fixtures, where the geometry is under the test's control. These
//! two check that the rules survive the built arena and the bevy schedule: the same
//! conservation residuals through `Arena::build` + `into_sim_prepared` + `step_static`,
//! and the policy boundary as the schedule actually presents it.
//!
//! Nothing here asserts the Stage-A start heading — package P2-B is changing it.

use std::sync::{Arc, Mutex};

use cubarium_voxel_fauna::{Actions, Controller, Founder, Response};
use cubarium_voxel_flora::{Deposit, DepositKind};
use cubarium_voxel_sim::{Arena, ScheduleMode, SimConfig};

/// Records every observation the schedule hands it and answers with one fixed action.
struct Recorder {
    log: Arc<Mutex<Vec<Vec<f64>>>>,
    action: Actions,
}

impl Recorder {
    fn new(action: Actions) -> (Recorder, Arc<Mutex<Vec<Vec<f64>>>>) {
        let log = Arc::new(Mutex::new(Vec::new()));
        (
            Recorder {
                log: Arc::clone(&log),
                action,
            },
            log,
        )
    }
}

impl Controller for Recorder {
    fn drive(&mut self, observation: &[f64]) -> Response {
        self.log
            .lock()
            .expect("the recorder's log")
            .push(observation.to_vec());
        Response::Bounded(self.action)
    }

    fn reset(&mut self) {}
}

fn close(a: f64, b: f64, tol: f64) -> bool {
    (a - b).abs() <= tol * (1.0f64).max(a.abs().max(b.abs()))
}

/// Every unit of litter organic matter, mineral and energy on the ground, whichever site
/// it sits on: in a static arena nothing but a bite and its dung can move these.
fn litter_pools(flora: &cubarium_voxel_flora::Flora) -> (f64, f64, f64, f64) {
    let v = flora.view();
    (
        v.ground.iter().map(|g| g.litter).sum(),
        v.ground.iter().map(|g| g.litter_mineral).sum(),
        v.ground.iter().map(|g| g.litter_energy).sum(),
        v.ground.iter().map(|g| g.mineral).sum(),
    )
}

/// Tests plan §1, "Paid food", in the real arena: *"Litter and foliage bites debit actual
/// stocks and transfer organic material, mineral and energy once."* The three currencies
/// must close as residuals against the animal layer's own boundary ledger, and the plant
/// layer's `consumed_*_out` must be the same numbers as the animal layer's `eaten_*_in`.
///
/// The founder is built off the food by the arena, so this fixture puts one litter tile
/// under the body it built and re-settles the field for the changed source layout — the
/// only way to exercise feeding at a built arena's own start without moving the body.
///
/// A failure means the ES score's intake term is measuring something that does not come
/// out of a stock, which would make every arena result unfalsifiable.
#[test]
fn an_arena_bite_closes_the_three_currencies_across_stock_animal_and_respiration() {
    let mut arena = Arena::build(Founder::Blind, 1);
    // The founder alone: the ledger below is the layer's, so the bystanders go.
    arena.bystanders.clear();
    arena.refound(cubarium_voxel_sim::FOUNDER_START);
    let id = arena.animal_id.expect("the arena placed its founder");
    let stand_on = arena
        .fauna
        .view()
        .animal(id)
        .expect("the placed founder")
        .site;
    assert!(
        arena
            .flora
            .view()
            .ground_at(stand_on)
            .map_or(0.0, |g| g.litter)
            == 0.0,
        "the arena started the founder on food, so this fixture would not be a bite test"
    );
    assert!(arena.flora.deposit(
        stand_on,
        Deposit {
            kind: DepositKind::Litter,
            organic: 0.2,
            mineral: 0.004,
            energy: 0.4,
        },
    ));

    let before_pools = litter_pools(&arena.flora);
    let before_animal = *arena.fauna.view().animal(id).expect("the founder");
    let before_consumed = {
        let l = arena.flora.view().ledger;
        (
            l.consumed_organic_out,
            l.consumed_mineral_out,
            l.consumed_energy_out,
        )
    };

    let senses = arena.prepare_senses();
    let mut sim = arena.into_sim_prepared(SimConfig::default(), senses);
    assert_eq!(sim.mode(), ScheduleMode::Static);
    assert!(
        sim.fauna_mut().set_controller(
            id,
            Box::new(
                Recorder::new(Actions {
                    forward: 0.0,
                    turn: 0.0,
                    feed: 1.0,
                })
                .0
            ),
        )
    );
    // 60 ticks: twelve controller intervals, well inside the short-test budget.
    for _ in 0..60 {
        sim.step_static();
    }

    let after_pools = litter_pools(sim.flora());
    let view = sim.fauna().view();
    let ledger = view.ledger;
    let after_animal = *view.animal(id).expect("the founder survived 60 ticks");

    assert_eq!(
        ledger.bites, 12,
        "twelve intervals produced {} bites",
        ledger.bites
    );
    assert!(
        ledger.eaten_organic_in > 0.0,
        "the arena bite transferred nothing"
    );

    // The stock fell by exactly what was booked, on both sides of the boundary.
    assert!(
        close(
            before_pools.0 - after_pools.0,
            ledger.eaten_organic_in,
            1e-12
        ),
        "litter fell by {} against an eaten_organic_in of {}",
        before_pools.0 - after_pools.0,
        ledger.eaten_organic_in
    );
    let consumed = sim.flora().view().ledger;
    assert!(
        close(
            consumed.consumed_organic_out - before_consumed.0,
            ledger.eaten_organic_in,
            1e-12
        ) && close(
            consumed.consumed_mineral_out - before_consumed.1,
            ledger.eaten_mineral_in,
            1e-12
        ) && close(
            consumed.consumed_energy_out - before_consumed.2,
            ledger.eaten_energy_in,
            1e-12
        ),
        "the plant layer's consumed_* and the animal layer's eaten_* are different numbers"
    );

    // What the animal holds (its gut included) is what came in, less what it respired and
    // put back.
    assert!(
        close(
            after_animal.stored_organic() - before_animal.stored_organic(),
            ledger.eaten_organic_in - ledger.respired_out - ledger.deposited_organic_out,
            1e-12
        ),
        "organic residual does not close"
    );
    assert!(
        close(
            after_animal.stored_mineral() - before_animal.stored_mineral(),
            ledger.eaten_mineral_in - ledger.deposited_mineral_out,
            1e-12
        ),
        "mineral residual does not close"
    );
    assert!(
        close(
            after_animal.stored_energy() - before_animal.stored_energy(),
            ledger.eaten_energy_in - ledger.heat_out - ledger.deposited_energy_out,
            1e-12
        ),
        "energy residual does not close"
    );
    // Mineral is conserved across the two layers: none is respired and none is created.
    // What respiration sheds and what a bite brings beyond its tissue waits in the gut.
    let mineral_moved = (before_pools.1 - after_pools.1) + (before_pools.3 - after_pools.3);
    let mineral_gained = after_animal.stored_mineral() - before_animal.stored_mineral();
    assert!(
        close(mineral_moved, mineral_gained, 1e-12),
        "the ground lost {mineral_moved} of mineral and the animal gained {mineral_gained}"
    );
    // The three respiration splits sum to the total.
    assert!(
        close(
            ledger.respired_maintenance_out
                + ledger.respired_motor_out
                + ledger.respired_digestion_out,
            ledger.respired_out,
            1e-12
        ),
        "the respiration splits do not sum to the total"
    );
}

/// Tests plan §1, "Policy boundary", through the schedule that actually trains: the
/// controller is handed a vector of exactly its manifest's width and nothing else, every
/// value finite and inside its channel's declared range, on every sampling of a real
/// arena episode. Both founders, both widths (23 and 37).
///
/// A failure means the ES trainer's tensor shape and the observation the body builds have
/// drifted apart, or a channel escapes its normalization and is trained through.
#[test]
fn every_arena_sampling_hands_the_controller_exactly_its_manifest_width() {
    for founder in Founder::ALL {
        let manifest = founder.manifest();
        let arena = Arena::build(founder, 2);
        let id = arena.animal_id.expect("the arena placed its founder");
        let senses = arena.prepare_senses();
        let mut sim = arena.into_sim_prepared(SimConfig::default(), senses);
        let (recorder, log) = Recorder::new(Actions {
            forward: 0.7,
            turn: -0.4,
            feed: 1.0,
        });
        assert!(sim.fauna_mut().set_controller(id, Box::new(recorder)));
        for _ in 0..60 {
            sim.step_static();
        }

        // The two channels the schema encodes as signed: a resolved turn and a smoothed
        // time derivative. Everything else is a normalized magnitude in [0, 1].
        let signed: Vec<usize> = manifest
            .modules
            .iter()
            .flat_map(|m| {
                m.channels
                    .iter()
                    .enumerate()
                    .filter(|(_, c)| **c == "resolved_turn" || **c == "trend")
                    .map(move |(k, _)| m.offset + k)
            })
            .collect();
        let samples = log.lock().expect("the log").clone();
        assert_eq!(samples.len(), 12, "{}: sampling cadence", founder.name());
        for (k, o) in samples.iter().enumerate() {
            assert_eq!(
                o.len(),
                manifest.inputs(),
                "{}: sample {k} was {} long, not {}",
                founder.name(),
                o.len(),
                manifest.inputs()
            );
            for (i, v) in o.iter().enumerate() {
                assert!(
                    v.is_finite(),
                    "{}: sample {k} channel {i} is {v}",
                    founder.name()
                );
                let low = if signed.contains(&i) { -1.0 } else { 0.0 };
                assert!(
                    *v >= low && *v <= 1.0,
                    "{}: sample {k} channel {i} is {v}, outside [{low}, 1]",
                    founder.name()
                );
            }
        }
    }
    assert_eq!(Founder::Blind.manifest().inputs(), 23);
    assert_eq!(Founder::Browser.manifest().inputs(), 37);
}

/// Plan, "Frozen arena contract": *"Resource depletion stops emission and updates sensory
/// occupancy. ... Neither smell nor visual snapshots may keep reporting the initial food
/// stock after it has been consumed."*
///
/// A blind founder eating the tile it stands on must see its own `Taste` cue fall as the
/// stock falls, and the field's `Chem` response must fall with it rather than keeping the
/// prepared layout's value.
///
/// A failure means a depleted patch still smells full, so Stage B's "leave and reacquire"
/// task has no signal to leave on.
#[test]
fn eating_the_tile_underfoot_lowers_both_the_taste_and_the_field_reading() {
    let mut arena = Arena::build(Founder::Blind, 3);
    let id = arena.animal_id.expect("the arena placed its founder");
    let stand_on = arena
        .fauna
        .view()
        .animal(id)
        .expect("the placed founder")
        .site;
    // A small tile, so twelve bites take a visible share of it.
    assert!(arena.flora.deposit(
        stand_on,
        Deposit {
            kind: DepositKind::Litter,
            organic: 0.004,
            mineral: 0.00008,
            energy: 0.008,
        },
    ));
    let start_stock = arena
        .flora
        .view()
        .ground_at(stand_on)
        .expect("the tile")
        .litter;

    let senses = arena.prepare_senses();
    let mut sim = arena.into_sim_prepared(SimConfig::default(), senses);
    let (recorder, log) = Recorder::new(Actions {
        forward: 0.0,
        turn: 0.0,
        feed: 1.0,
    });
    assert!(sim.fauna_mut().set_controller(id, Box::new(recorder)));
    for _ in 0..60 {
        sim.step_static();
    }

    let end_stock = sim
        .flora()
        .view()
        .ground_at(stand_on)
        .expect("the tile")
        .litter;
    // A tenth, not a fifth: since package G a bite shrinks with the stock at the mouth and
    // with the reserve's fill, so twelve bites of a small tile take less than they did.
    assert!(
        end_stock < start_stock * 0.9,
        "twelve bites only took the tile from {start_stock} to {end_stock}"
    );

    let manifest = Founder::Blind.manifest();
    let named = |module: &str, channel: &str| {
        let m = manifest
            .modules
            .iter()
            .find(|m| m.name == module)
            .expect("module");
        m.offset
            + m.channels
                .iter()
                .position(|c| *c == channel)
                .expect("channel")
    };
    let taste = named("Taste(1)", "cue");
    let taste_valid = named("Taste(1)", "valid");
    let chem = named("Chem(detritus)", "response");

    let samples = log.lock().expect("the log").clone();
    assert_eq!(samples.len(), 12);
    assert_eq!(
        samples[0][taste_valid], 1.0,
        "a mouth on litter read invalid"
    );
    assert!(
        samples[0][taste] > 0.0,
        "the mouth on a full tile tasted nothing"
    );
    assert!(
        samples[11][taste] < samples[0][taste] - 1e-9,
        "the taste cue did not fall as the stock fell: {} -> {}",
        samples[0][taste],
        samples[11][taste]
    );
    assert!(
        samples[11][chem] < samples[0][chem] - 1e-9,
        "the diffused cue kept reporting the prepared layout: {} -> {}",
        samples[0][chem],
        samples[11][chem]
    );
}
