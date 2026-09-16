//! The per-tick intake diagnostic's measurement, checked against the fixture's own independent
//! columns and against the world's per-organism ledger, plus the experiment itself
//! (`design/handoffs/ecology-v1-intake-opus-2026-09-16.md`, deliverables 2 and 3).
//!
//! The diagnostic's whole value is that it reads what the tick decided, at the site it decided
//! it. That is worth nothing if switching the trace on changes what the episode does, and very
//! little if the trace and the ledger disagree without anybody noticing — they are accumulated
//! on different sides of the same pass, so agreeing is evidence and disagreeing is a bug in one
//! of them.
//!
//! The experiment at the bottom is `#[ignore]`d: it runs 36 episodes of 36,000 ticks and writes
//! to `runs/`, which is a measurement, not a check. Run it by name:
//!
//! ```bash
//! cargo test -p cubarium-search --release --test intake_diagnostic -- --ignored --nocapture
//! ```

use std::sync::atomic::AtomicBool;

use cubarium_core::{CARRION, CHANNELS, FOLIAGE, FRUIT, IntakeLimit, LITTER, MOUTHS, MOUTH_GRAZE};
use cubarium_search::es::budget::NamedDriver;
use cubarium_search::es::episode::{Control, Driver, Limits};
use cubarium_search::es::fixture::{self, Ecology};
use cubarium_search::es::intake::{self, LIMITS, OPEN};

const TICKS: u64 = 3_000;

fn first_training_layout() -> (Ecology, fixture::Layout) {
    let eco = Ecology::defaults();
    let layout = fixture::training_layouts_on(&eco).remove(0);
    (eco, layout)
}

/// The trace and the ledger are two accumulations of the same transfers, taken at different
/// points of the same pass: the per-tick rows must sum to the life the ledger booked.
#[test]
fn the_trace_sums_to_the_ledger() {
    let (eco, layout) = first_training_layout();
    let cancel = AtomicBool::new(false);
    let e_r = eco.base.organism.reserve_energy_density;
    let eta_ox = eco.base.organism.oxidation_efficiency;
    let named = NamedDriver {
        name: "mobile-script".into(),
        driver: Driver::Control(Control::MobileScript),
    };
    let (_, row, kept) =
        intake::measure(&layout, &named, TICKS, Limits::new(&cancel), e_r, eta_ox, true)
            .expect("measured");
    let kept = kept.expect("the per-tick rows were asked for");

    assert_eq!(row.ticks, TICKS, "one row per tick the body was alive");
    assert_eq!(kept.len() as u64, row.ticks);

    for c in 0..CHANNELS {
        let summed: f64 = kept.iter().map(|r| r.served[c]).sum();
        assert_eq!(
            summed, row.served_total[c],
            "channel {c}: the fold and the rows disagree"
        );
        // Float association is the only difference allowed against the ledger: both are sums of
        // the same per-bite values, added in the same order, so they agree to a few ULP.
        let ledger = row.budget.served[c];
        assert!(
            (summed - ledger).abs() <= 1e-12 * ledger.abs().max(1e-6),
            "channel {c}: trace {summed} against ledger {ledger}"
        );
    }
    let billed: f64 = kept.iter().map(|r| r.bill_total).sum();
    assert!(
        (billed - row.budget.bill_total).abs() <= 1e-9 * row.budget.bill_total.abs(),
        "trace bill {billed} against ledger {}",
        row.budget.bill_total
    );
    assert!(row.served_total[FOLIAGE] > 0.0, "the mobile script eats foliage");
    assert_eq!(row.served_total[FRUIT], 0.0, "the script's mouth is grazing only");
    assert_eq!(row.served_total[LITTER] + row.served_total[CARRION], 0.0);
}

/// The counts are the counts the column names claim, on a driver whose behaviour is disclosed:
/// the stationary grazer never leaves its opening cell, holds `graze_effort = 1` forever, and
/// crops that cell until it falls below the world's own threshold.
#[test]
fn the_counts_are_what_the_columns_say_on_a_disclosed_driver() {
    let (eco, layout) = first_training_layout();
    let cancel = AtomicBool::new(false);
    let e_r = eco.base.organism.reserve_energy_density;
    let eta_ox = eco.base.organism.oxidation_efficiency;
    let named = NamedDriver {
        name: "stationary-grazing".into(),
        driver: Driver::Control(Control::StationaryGrazing),
    };
    let (_, row, kept) =
        intake::measure(&layout, &named, TICKS, Limits::new(&cancel), e_r, eta_ox, true)
            .expect("measured");
    let kept = kept.expect("rows");

    // One cell, for the whole episode.
    assert_eq!(row.episode.distinct_cells, 1);
    assert!(kept.windows(2).all(|w| w[0].cell == w[1].cell), "the grazer moved");

    // The script's mouth is open on every tick, and only the grazing one.
    assert_eq!(row.open_ticks, row.ticks, "a scripted `graze_effort = 1` is open every tick");
    assert_eq!(row.wide_open_ticks, row.ticks);
    assert!((row.mean_effort[MOUTH_GRAZE] - 1.0).abs() < 1e-15);
    assert_eq!(row.mean_effort[1], 0.0);
    assert_eq!(row.mean_effort[2], 0.0);

    // It opens on food, so every tick on food has the matching mouth open: the fractions the
    // verdict is read off are 1.0 by construction for this driver, which is the control that
    // makes them legible for the others.
    assert!(row.on_food > 0, "the opening cell is food");
    assert_eq!(row.open_on_food, row.on_food);
    assert_eq!(row.wide_open_on_food, row.on_food);
    assert!((row.open_given_food() - 1.0).abs() < 1e-15);
    // And whatever it does off food, it is never on a cell it did not start on.
    assert_eq!(row.open_off_food, row.ticks - row.on_food);

    // Hand-recount the whole fold from the rows it folded, so the reduction is checked against
    // its own inputs rather than trusted.
    let on_food = kept.iter().filter(|r| r.on_food()).count() as u64;
    assert_eq!(on_food, row.on_food);
    let open_on_food = kept
        .iter()
        .filter(|r| r.on_food() && (0..CHANNELS).any(|c| r.above_threshold[c] && r.mouth_open(c, OPEN)))
        .count() as u64;
    assert_eq!(open_on_food, row.open_on_food);
    let mut limits = [[0u64; 6]; MOUTHS];
    for r in &kept {
        for (m, bin) in limits.iter_mut().enumerate() {
            let i = LIMITS.iter().position(|x| *x == r.limit[m]).expect("a known term");
            bin[i] += 1;
        }
    }
    assert_eq!(limits, row.limits);

    // The one cell is cropped from food to bare over the episode, so both terms appear and
    // nothing else does for the grazing mouth: it is never shut, never without a gut, and its
    // reserve fills only at the very end if at all.
    let counts = row.limits[MOUTH_GRAZE];
    let i = |l: IntakeLimit| LIMITS.iter().position(|x| *x == l).expect("known");
    assert_eq!(counts[i(IntakeLimit::EffortZero)], 0, "the script never shuts the mouth");
    assert_eq!(counts[i(IntakeLimit::CapabilityZero)], 0, "the fixture body grazes");
    assert!(counts[i(IntakeLimit::MouthRate)] > 0, "it starts on food and eats at its own rate");
    assert!(
        counts[i(IntakeLimit::StockBelowThreshold)] > 0,
        "it crops the cell below `feed_min` within {TICKS} ticks"
    );
}

/// A policy with every mouth channel driven hard negative is the one arm that makes branch (a)
/// visible: it stands wherever it stands and never asks for anything, and the diagnostic must
/// say `effort_zero` rather than blaming the cell.
#[test]
fn a_shut_mouthed_policy_reads_as_the_effort_and_not_the_cell() {
    use cubarium_core::neural::Policy;
    use cubarium_core::neural::gru::Gru32;

    let (eco, layout) = first_training_layout();
    let cancel = AtomicBool::new(false);
    let mut w = Gru32::zeros();
    // Channels 2..5 are the three mouths; σ(−8) is far inside the adapter's deadband.
    for i in 2..5 {
        w.b_o[i] = -8.0;
    }
    let named =
        NamedDriver { name: "shut".into(), driver: Driver::Policy(Box::new(Policy::new(w))) };
    let (_, row, _) = intake::measure(
        &layout,
        &named,
        500,
        Limits::new(&cancel),
        eco.base.organism.reserve_energy_density,
        eco.base.organism.oxidation_efficiency,
        false,
    )
    .expect("measured");

    assert_eq!(row.open_ticks, 0, "every mouth is inside the deadband");
    assert_eq!(row.served_total.iter().sum::<f64>(), 0.0, "a shut mouth ate something");
    assert!(row.on_food > 0, "the fixture starts the body on its opening patch");
    assert_eq!(row.open_on_food, 0);
    let i = LIMITS.iter().position(|x| *x == IntakeLimit::EffortZero).expect("known");
    assert_eq!(
        row.limits_on_its_food[MOUTH_GRAZE][i], row.limits_on_its_food[MOUTH_GRAZE].iter().sum::<u64>(),
        "standing on food with a shut mouth must read as the effort"
    );
}

// --- the experiment -----------------------------------------------------------------------
