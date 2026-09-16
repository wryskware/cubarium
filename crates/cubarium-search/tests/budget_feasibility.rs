//! The feasibility experiment's measurement, checked against the ES fixture's own independent
//! columns, and checked for not having changed the fixture.
//!
//! The experiment's whole value is that it replaces a reconstruction with the world's own
//! ledger. That is worth nothing if switching the ledger on changes what the episode does, and
//! it is worth very little if the ledger and the reconstruction disagree without anybody
//! noticing — the two are computed from different sides of the tick, so agreeing is evidence
//! and disagreeing is a bug in one of them.

use std::sync::atomic::AtomicBool;

use cubarium_core::{CARRION, FOLIAGE, FRUIT, LITTER};
use cubarium_search::es::budget::{self, WINDOW_TICKS};
use cubarium_search::es::episode::{self, Control, Driver, Limits};
use cubarium_search::es::fixture::{self, Ecology};

const TICKS: u64 = 3_000;

fn first_training_layout() -> (Ecology, fixture::Layout) {
    let eco = Ecology::defaults();
    let layout = fixture::training_layouts_on(&eco).remove(0);
    (eco, layout)
}

/// Turning the recorder on before the first tick must leave the episode bit-for-bit what the
/// trainer would have scored. Every field, not just the score: a diagnostic that moved the
/// travelled distance would be as bad as one that moved the survival time.
#[test]
fn recording_the_ledger_leaves_the_episode_identical() {
    let (eco, layout) = first_training_layout();
    let cancel = AtomicBool::new(false);
    let limits = Limits::new(&cancel);
    let e_r = eco.base.organism.reserve_energy_density;
    let eta_ox = eco.base.organism.oxidation_efficiency;

    for (name, driver) in [
        ("stationary-grazing", Driver::Control(Control::StationaryGrazing)),
        ("mobile-script", Driver::Control(Control::MobileScript)),
    ] {
        let plain = episode::run(&layout, &driver, TICKS, limits, "plain").expect("not cancelled");
        let named = budget::NamedDriver { name: name.into(), driver };
        let (measured, _, _) =
            budget::measure(&layout, &named, TICKS, limits, e_r, eta_ox).expect("measured");
        assert_eq!(plain, measured, "{name}: recording changed the episode");
    }
}

/// The ledger and the episode's own reconstruction are two independent measurements of the
/// same run: the served material per channel against the world totals the episode reads, and
/// the upkeep the bill levied against the upkeep the episode re-derived from `MotorBill`.
#[test]
fn the_ledger_agrees_with_the_episodes_own_columns() {
    let (eco, layout) = first_training_layout();
    let cancel = AtomicBool::new(false);
    let limits = Limits::new(&cancel);
    let e_r = eco.base.organism.reserve_energy_density;
    let eta_ox = eco.base.organism.oxidation_efficiency;
    let named = budget::NamedDriver {
        name: "stationary-grazing".into(),
        driver: Driver::Control(Control::StationaryGrazing),
    };
    let (episode, ledger, trace) =
        budget::measure(&layout, &named, TICKS, limits, e_r, eta_ox).expect("measured");

    // One body in the arena, so the world's totals are this body's.
    assert_eq!(ledger.served[FOLIAGE], episode.intake_producer);
    assert_eq!(ledger.served[FRUIT], episode.intake_fruit);
    assert_eq!(
        ledger.served[LITTER] + ledger.served[CARRION],
        episode.intake_detritus
    );
    assert!(ledger.served[FOLIAGE] > 0.0, "a stationary grazer on its patch ate nothing");

    // The episode reconstructs the upkeep price tick by tick from the same `MotorBill`; the
    // ledger books it where the world levies it. They must be the same number.
    assert!(
        (ledger.upkeep_billed - episode.upkeep_billed).abs()
            < 1e-12 * episode.upkeep_billed.abs().max(1.0),
        "ledger upkeep {} against episode upkeep {}",
        ledger.upkeep_billed,
        episode.upkeep_billed
    );

    // And the identities close for this body over this life.
    assert!(ledger.material_residual().abs() < 1e-10, "{:e}", ledger.material_residual());
    assert!(ledger.energy_residual().abs() < 1e-10, "{:e}", ledger.energy_residual());

    // One trace entry per simulated tick, both series non-decreasing.
    assert_eq!(trace.len() as u64, episode.ticks);
    for pair in trace.windows(2) {
        assert!(pair[1].0 >= pair[0].0, "income went backwards");
        assert!(pair[1].1 >= pair[0].1, "the bill went backwards");
    }
}

/// The windowed ratio is the cumulative trace differenced over the window, not an average of
/// per-tick ratios: computing it by hand from the trace must reproduce what the command
/// reports.
#[test]
fn the_reported_window_is_the_trace_differenced_by_hand() {
    let (eco, layout) = first_training_layout();
    let cancel = AtomicBool::new(false);
    let limits = Limits::new(&cancel);
    let e_r = eco.base.organism.reserve_energy_density;
    let eta_ox = eco.base.organism.oxidation_efficiency;
    let named = budget::NamedDriver {
        name: "mobile-script".into(),
        driver: Driver::Control(Control::MobileScript),
    };
    let (_, _, trace) =
        budget::measure(&layout, &named, TICKS, limits, e_r, eta_ox).expect("measured");
    let w = (WINDOW_TICKS as usize).min(trace.len());
    let (last, best, window) = budget::ratios(&trace, WINDOW_TICKS as usize);
    assert_eq!(window as usize, w);

    let (i1, b1) = trace[trace.len() - 1];
    // `ratios` differences from the tick *before* the window opens; reproduce that.
    let (i0, b0) = if trace.len() == w { (0.0, 0.0) } else { trace[trace.len() - w - 1] };
    let expected = (i1 - i0) / (b1 - b0);
    assert!((last - expected).abs() < 1e-12, "{last} against {expected}");
    assert!(best >= last - 1e-12, "the best window cannot be worse than the last one");
}
