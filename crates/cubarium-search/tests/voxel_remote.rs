//! Package S item 3: remote episode workers over a pipe
//! (`design/handoffs/voxel-training-speed-2026-09-23.md`).
//!
//! The "remote" here is the loopback: this build's own `voxel-eval-worker`, started
//! locally through `sh -c` rather than through ssh, so the tests exercise the protocol,
//! the pair-per-unit dispatch, the fixture check and the re-queue without a network.
//! Every episode is 40 ticks on the first two Stage-A arena layouts.

use std::sync::atomic::AtomicBool;
use std::time::Duration;

use cubarium_search::es::optimizer::Adam;
use cubarium_search::es::tensor::initial_center_shape;
use cubarium_search::es::voxel::remote::{FixtureSpec, RemoteOptions, RemotePool, RemoteSpec};
use cubarium_search::es::voxel::task::{self, Band, Stage};
use cubarium_search::es::voxel::trainer::{
    GenerationPlan, GenerationReport, RemotePlan, VoxelProtocol, run_generation,
};
use cubarium_voxel_fauna::Founder;

const BIN: &str = env!("CARGO_BIN_EXE_cubarium-search");
const HORIZON: u64 = 40;
const PAIRS: usize = 2;

fn fixtures() -> FixtureSpec {
    FixtureSpec {
        founder: Founder::Blind.name().into(),
        stage: Stage::A.as_str().into(),
        band: Band::Landed.as_str().into(),
        layouts: 2,
        p5: false,
    }
}

/// The worker, started locally: `sh -c 'exec <this build> voxel-eval-worker ...'`.
fn loopback(extra: &str) -> RemoteSpec {
    RemoteSpec::command(
        "loopback",
        vec![
            "sh".into(),
            "-c".into(),
            format!("exec '{BIN}' voxel-eval-worker --threads 2 --heartbeat-ms 100 {extra}"),
        ],
        2,
    )
}

fn quick() -> RemoteOptions {
    RemoteOptions {
        hello_timeout: Duration::from_secs(20),
        stall: Duration::from_millis(600),
        unit_timeout: Duration::from_secs(20),
    }
}

fn protocol() -> VoxelProtocol {
    VoxelProtocol::new(
        Founder::Blind,
        Stage::A,
        Band::Landed,
        PAIRS,
        HORIZON,
        20_260_918,
        &task::TRAINING_LAYOUT_SEEDS[..2],
    )
}

/// One generation from the seeded centre, on one local worker plus `pool` when given.
fn generation(pool: Option<&RemotePool>) -> (Vec<f64>, Adam, GenerationReport) {
    let protocol = protocol();
    let layouts = fixtures().build(1).expect("the arenas build");
    let ids: Vec<usize> = (0..layouts.len()).collect();
    let cancel = AtomicBool::new(false);
    let mut theta = initial_center_shape::<23, 3>(protocol.train_seed);
    let mut adam = Adam::new(theta.len());
    let plan = GenerationPlan {
        layouts: &layouts,
        horizon: HORIZON,
        workers: 1,
        evaluate_center: true,
        deadline: None,
        remote: pool.map(|pool| RemotePlan {
            pool,
            fixture_ids: &ids,
        }),
    };
    let report = run_generation(&mut theta, &mut adam, &protocol, 0, &plan, &cancel)
        .expect("the generation completes");
    (theta, adam, report)
}

fn connected(extra: &str) -> RemotePool {
    let pool = RemotePool::connect(&[loopback(extra)], &fixtures(), quick());
    let local = fixtures().build(1).expect("the arenas build");
    pool.verify(&local, Duration::from_secs(20));
    assert_eq!(pool.active(), 1, "the loopback joined: {:?}", pool.status());
    pool
}

fn remote_units(report: &GenerationReport) -> u64 {
    report
        .shares
        .iter()
        .filter(|s| s.executor != "local")
        .map(|s| s.units)
        .sum()
}

fn assert_same_update(
    (t_local, a_local, r_local): &(Vec<f64>, Adam, GenerationReport),
    (t_remote, a_remote, r_remote): &(Vec<f64>, Adam, GenerationReport),
) {
    assert_eq!(
        r_local.candidate_scores.len(),
        r_remote.candidate_scores.len()
    );
    for (a, b) in r_local
        .candidate_scores
        .iter()
        .zip(&r_remote.candidate_scores)
    {
        assert!((a - b).abs() <= 1e-12 * a.abs().max(1.0), "{a} vs {b}");
    }
    assert_eq!(t_local.len(), t_remote.len());
    for (a, b) in t_local.iter().zip(t_remote) {
        assert!((a - b).abs() <= 1e-12 * a.abs().max(1.0), "{a} vs {b}");
    }
    assert_eq!(a_local.step, a_remote.step);
}

/// A generation whose pairs partly ran on the loopback worker gives the same update as
/// a local-only one: same candidate scores, same centre, same Adam step.
#[test]
fn a_loopback_generation_gives_the_local_update() {
    let local = generation(None);
    let pool = connected("");
    let remote = generation(Some(&pool));
    assert!(
        remote_units(&remote.2) > 0,
        "the loopback ran some of the pairs: {:?}",
        remote.2.shares
    );
    assert_eq!(remote.2.requeued, 0);
    assert_same_update(&local, &remote);
}

/// Cache study B: a worker told `--pin auto` (what `voxel-train --remote` passes under
/// the coordinator's own `--pin`) pins its threads and still gives the local update.
#[test]
fn a_pinned_loopback_worker_gives_the_local_update() {
    let local = generation(None);
    let pool = connected("--pin auto");
    let remote = generation(Some(&pool));
    assert!(remote_units(&remote.2) > 0, "{:?}", remote.2.shares);
    assert_eq!(remote.2.requeued, 0);
    assert_same_update(&local, &remote);
}

/// A worker that dies mid-generation — it exits after answering one unit, with another
/// still in hand — has its outstanding work re-queued locally, and the generation
/// completes with the local update.
#[test]
fn a_remote_killed_mid_generation_still_yields_a_complete_generation() {
    let local = generation(None);
    let pool = connected("--die-after 1");
    let remote = generation(Some(&pool));
    assert!(remote.2.requeued >= 1, "the lost unit was re-queued");
    assert_eq!(pool.active(), 0, "the dead worker is out of the run");
    assert_same_update(&local, &remote);
    // The next generation runs local-only without complaint.
    let again = generation(Some(&pool));
    assert_eq!(remote_units(&again.2), 0);
    assert_same_update(&local, &again);
}

/// A worker that stops answering — no results and no heartbeats, pipe still open — is
/// declared stalled after the stall window, and its units are re-queued locally.
#[test]
fn a_stalled_remote_is_dropped_and_its_units_run_locally() {
    let local = generation(None);
    let pool = connected("--hang-after 1");
    let remote = generation(Some(&pool));
    assert!(remote.2.requeued >= 1, "the stalled units were re-queued");
    assert_eq!(pool.active(), 0);
    assert_same_update(&local, &remote);
}

/// A worker whose fixtures do not match the coordinator's is refused before it is
/// given any work: here the coordinator trains Stage B while the worker built Stage A.
#[test]
fn a_remote_with_other_worlds_is_refused() {
    let pool = RemotePool::connect(&[loopback("")], &fixtures(), quick());
    let other = FixtureSpec {
        stage: Stage::B.as_str().into(),
        ..fixtures()
    }
    .build(1)
    .expect("stage-B arenas build");
    pool.verify(&other, Duration::from_secs(20));
    assert_eq!(pool.active(), 0, "{:?}", pool.status());
    let status = pool.status();
    assert!(
        status.iter().any(|(_, s)| s.contains("refused")),
        "{status:?}"
    );
}
