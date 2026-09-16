//! The two falsification checks' integration surface
//! (`design/handoffs/ecology-v1-score-checks-opus-2026-09-16.md`, deliverables 1 and 2).
//!
//! The checks' own arithmetic — the observation variants, the forward pass and the adapter, the
//! dwell rule's residence, the auxiliary's clip and its post-death charge — is pinned next to
//! the code in `es::scorecheck`. What is pinned here is the part a reader of the result note
//! has to be able to trust from outside: that check (b) really scores with the **trainer's own**
//! score and not a copy of it, that the proposed auxiliary cannot reorder a survival difference
//! the brief called material, and that the dwell rungs differ from the disclosed mobile script
//! in residence and in nothing else.
//!
//! The two experiments at the bottom are `#[ignore]`d: they run 96 and 24 episodes of 36,000
//! ticks and write to `runs/`, which is a measurement, not a check. Run them by name:
//!
//! ```bash
//! cargo test -p cubarium-search --release --test score_checks -- --ignored --nocapture
//! ```
//!
//! They are `#[ignore]`d tests rather than `es-*` subcommands for the same reason workstream H's
//! was: three other workers held `crates/cubarium-search/src/main.rs` open in worktrees, and a
//! subcommand is the one edit that would have collided. `scorecheck::run_sweep` and
//! `scorecheck::run_ladder` are public, so promoting them later is a one-liner.

use std::path::PathBuf;
use std::sync::atomic::AtomicBool;

use cubarium_search::es::episode::{self, Control, Driver, Limits};
use cubarium_search::es::fixture::{self, Ecology};
use cubarium_search::es::scorecheck::{
    self, AUX_LAMBDA, LadderEpisode, Stay, auxiliary, proposed_score,
};
use cubarium_search::es::trainer;

fn first_training_layout() -> (Ecology, fixture::Layout) {
    let eco = Ecology::defaults();
    let layout = fixture::training_layouts_on(&eco).remove(0);
    (eco, layout)
}

/// Check (b) is only worth anything if the number it prints is the number the trainer would
/// have computed. `reduce` must therefore agree with `trainer::score` on the same episodes,
/// reached independently here, and with the `t_min + 0.25 · stores` the brief names.
#[test]
fn a_rungs_current_score_is_the_trainers_own_function_of_its_episodes() {
    let (eco, layout) = first_training_layout();
    let e_r = eco.base.organism.reserve_energy_density;
    let eta_ox = eco.base.organism.oxidation_efficiency;
    let cancel = AtomicBool::new(false);

    let mut rows: Vec<LadderEpisode> = Vec::new();
    for d in [40u32, 400] {
        let named = cubarium_search::es::budget::NamedDriver {
            name: Control::Dwell(d).name(),
            driver: Driver::Control(Control::Dwell(d)),
        };
        rows.push(
            scorecheck::measure_rung(&layout, &named, 2_500, Limits::new(&cancel), e_r, eta_ox)
                .expect("measured"),
        );
    }
    let refs: Vec<&LadderEpisode> = rows.iter().collect();
    let reduced = scorecheck::reduce(&refs);
    let episodes: Vec<_> = rows.iter().map(|r| r.episode.clone()).collect();

    assert_eq!(reduced.current_score, trainer::score(&episodes));
    let t_min = episodes.iter().map(|e| e.ticks).min().expect("nonempty") as f64;
    let stores: f64 = episodes
        .iter()
        .map(cubarium_search::es::Episode::normalized_stores)
        .sum::<f64>()
        / episodes.len() as f64;
    assert!(
        (reduced.current_score - (t_min + trainer::STORE_WEIGHT * stores)).abs() < 1e-12,
        "the current score is not t_min + {} * stores",
        trainer::STORE_WEIGHT
    );
    assert_eq!(reduced.t_min as f64, t_min);
}

/// `λ` has to be small enough that the auxiliary cannot erase a material survival difference,
/// which the proposal fixes at 10 s — 200 ticks. Swept over the whole reachable range of `A`
/// rather than argued: no pair whose survival differs by 200 ticks or more may be reordered,
/// and a pair whose survival is equal must be ordered by `A` alone.
#[test]
fn the_auxiliary_cannot_reorder_a_material_survival_difference() {
    let steps = 41;
    for i in 0..steps {
        for j in 0..steps {
            let a_lo = -1.0 + 2.0 * i as f64 / (steps - 1) as f64;
            let a_hi = -1.0 + 2.0 * j as f64 / (steps - 1) as f64;
            // A 200-tick survival gap can at worst be tied by the extremes, never inverted;
            // one tick more and it is preserved strictly.
            assert!(
                proposed_score(1_000, a_hi) <= proposed_score(1_200, a_lo),
                "A {a_hi} over {a_lo} inverted a 200-tick gap"
            );
            assert!(
                proposed_score(1_000, a_hi) < proposed_score(1_201, a_lo),
                "A {a_hi} over {a_lo} inverted a 201-tick gap"
            );
            // And at equal survival the auxiliary is the whole ordering.
            if a_hi > a_lo {
                assert!(proposed_score(1_000, a_hi) > proposed_score(1_000, a_lo));
            }
        }
    }
    // The stated size of the term, so a reader can check the claim in the proposal.
    assert_eq!(AUX_LAMBDA, 100.0);
    assert_eq!(
        proposed_score(0, 1.0) - proposed_score(0, -1.0),
        200.0,
        "the whole auxiliary is worth 200 ticks"
    );
    // Dying at the horizon's quarter point costs at least three quarters of the range, whatever
    // the body earned while it lived: that is the post-death charge doing its job.
    let lived = 9_000u64;
    let best = auxiliary(lived as f64, lived, 36_000);
    assert!(best < 0.0, "a body that dies at a quarter of the horizon cannot score positive");
}

/// A dwell rung is the disclosed mobile script with one rule replaced, and the replacement is
/// visible from outside: the stays are the counter's, the script's are the threshold's, and
/// nothing else about the route, the heading or the paid motion differs.
#[test]
fn the_dwell_rungs_differ_from_the_mobile_script_only_in_residence() {
    let (_, layout) = first_training_layout();
    let cancel = AtomicBool::new(false);
    let limits = Limits::new(&cancel);

    let script_stays = scorecheck::stays(
        &layout,
        &Driver::Control(Control::MobileScript),
        6_000,
    )
    .expect("ok");
    let completed = |v: &[Stay]| -> Vec<u64> {
        v.iter().filter(|s| s.completed).map(|s| s.ticks_on_cell).collect()
    };
    let script = completed(&script_stays);
    assert!(!script.is_empty());
    assert!(
        script.iter().any(|t| *t > 400),
        "the script's own residence is the threshold's, not a counter's: {script:?}"
    );

    let mut last_distinct = usize::MAX;
    for d in [20u32, 100, 300, 1_000] {
        let stays =
            scorecheck::stays(&layout, &Driver::Control(Control::Dwell(d)), 6_000).expect("ok");
        for t in completed(&stays) {
            assert_eq!(t, u64::from(d), "d {d}: a completed stay was {t} ticks");
        }
        // More residence per visit means fewer cells reached in a fixed window.
        let e = episode::run(&layout, &Driver::Control(Control::Dwell(d)), 6_000, limits, "t")
            .expect("ok");
        assert!(
            e.distinct_cells <= last_distinct,
            "d {d}: {} cells is not fewer than the shorter rung's {last_distinct}",
            e.distinct_cells
        );
        last_distinct = e.distinct_cells;
    }
}

// --- the experiments ----------------------------------------------------------------------

fn var(name: &str, default: &str) -> String {
    std::env::var(name).unwrap_or_else(|_| default.to_string())
}

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// Check (a): generation 9 and the untrained centre on the twelve `fast-leaf` layouts, with the
/// observation sweep run over the recorded ticks.
///
/// Overridable for a re-run of one slice: `CUBARIUM_SCORECHECK_CONFIG`,
/// `CUBARIUM_SCORECHECK_POLICY`, `CUBARIUM_SCORECHECK_SWEEP_OUT`,
/// `CUBARIUM_SCORECHECK_HORIZON`, `CUBARIUM_SCORECHECK_WORKERS`,
/// `CUBARIUM_SCORECHECK_STRATUM`.
#[test]
#[ignore = "workstream L's check (a), not a check"]
fn the_food_response_sweep() {
    let config = root().join(var(
        "CUBARIUM_SCORECHECK_CONFIG",
        "runs/ecology-v1-calibration/selected/fast-leaf.toml",
    ));
    let policy = root().join(var(
        "CUBARIUM_SCORECHECK_POLICY",
        "runs/es-eco-v1-fastleaf/selected/center-00009-policy.json",
    ));
    let out = root().join(var(
        "CUBARIUM_SCORECHECK_SWEEP_OUT",
        "runs/ecology-v1-score-checks/sweep.json",
    ));
    let horizon: u64 = var("CUBARIUM_SCORECHECK_HORIZON", "36000").parse().expect("a horizon");
    let workers: usize = var("CUBARIUM_SCORECHECK_WORKERS", "8").parse().expect("a worker count");
    let stratum: usize = var("CUBARIUM_SCORECHECK_STRATUM", "32").parse().expect("a sample size");

    scorecheck::run_sweep(policy, config, horizon, 20_260_915, workers, 240, stratum, out)
        .expect("the sweep ran");
}

/// Check (b): the dwell ladder under the current score, with the proposed auxiliary computed
/// from the same episodes.
///
/// Overridable: `CUBARIUM_SCORECHECK_CONFIG`, `CUBARIUM_SCORECHECK_POLICY`,
/// `CUBARIUM_SCORECHECK_LADDER_OUT`, `CUBARIUM_SCORECHECK_HORIZON`,
/// `CUBARIUM_SCORECHECK_WORKERS`, `CUBARIUM_SCORECHECK_DWELLS` (a comma-separated list).
#[test]
#[ignore = "workstream L's check (b), not a check"]
fn the_dwell_ladder() {
    let config = root().join(var(
        "CUBARIUM_SCORECHECK_CONFIG",
        "runs/ecology-v1-calibration/selected/fast-leaf.toml",
    ));
    let policy = root().join(var(
        "CUBARIUM_SCORECHECK_POLICY",
        "runs/es-eco-v1-fastleaf/selected/center-00009-policy.json",
    ));
    let out = root().join(var(
        "CUBARIUM_SCORECHECK_LADDER_OUT",
        "runs/ecology-v1-score-checks/ladder.json",
    ));
    let horizon: u64 = var("CUBARIUM_SCORECHECK_HORIZON", "36000").parse().expect("a horizon");
    let workers: usize = var("CUBARIUM_SCORECHECK_WORKERS", "8").parse().expect("a worker count");
    let dwells: Vec<u32> = var("CUBARIUM_SCORECHECK_DWELLS", "20,100,300,1000,3000")
        .split(',')
        .filter(|s| !s.is_empty())
        .map(|s| s.trim().parse().expect("a dwell"))
        .collect();

    scorecheck::run_ladder(Some(policy), config, horizon, &dwells, workers, 240, out)
        .expect("the ladder ran");
}
