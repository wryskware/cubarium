//! P5-B item 4: landscape episodes — whole frozen worlds founded by the moved founding
//! loop (D5), water frozen in the state the pre-roll captured (D6), every founder the
//! seeder placed running the candidate (D7), bodies sampled per episode (D11).
//!
//! The landscape here is a short staged ring founded on an asked-for seed with a shower
//! of a few seconds, so founding it costs a small fraction of a second; the episodes run
//! at most 200 ticks. The shipped presets are the same code at a larger size.

use std::sync::atomic::AtomicBool;

use cubarium_search::es::voxel::driver::{self, Limits};
use cubarium_search::es::voxel::landscape::{Landscape, WaterState};
use cubarium_search::es::voxel::task::Prepared;
use cubarium_search::es::voxel::imitate;
use cubarium_search::es::voxel::{EpisodeDriver, VoxelControl, teacher_sink};
use cubarium_voxel::{Landform, World};
use cubarium_voxel_fauna::{Fauna, FaunaConfig, Founder};
use cubarium_voxel_flora::{Flora, FloraConfig};
use cubarium_voxel_sim::found;

const TICKS: u64 = 160;

/// A short staged ring whose opening shower lasts a few seconds.
fn tiny() -> cubarium_voxel::Config {
    let mut recipe = cubarium_voxel::Recipe::DEFAULT;
    recipe.water.min_lake_m2 = 0.0;
    recipe.water.min_tier_pools = 0;
    recipe.water.shower_volume_m3 = 0.005;
    cubarium_voxel::Config {
        width: 64,
        height: 24,
        depth: 8,
        voxel_m: 0.25,
        landform: Landform::Staged(recipe),
        ..cubarium_voxel::Config::default()
    }
}

/// The tiny ring founded on an asked-for seed, mid-shower water captured.
fn landscape() -> Landscape {
    let founded = found::found_a_landscape(
        &tiny(),
        Some(TINY_SEED),
        1,
        1,
        || unreachable!("an asked-for seed draws nothing"),
        |w: &World| {
            (
                Flora::new(FloraConfig::for_voxel_size(w.config().voxel_m)),
                Fauna::new(FaunaConfig::default()),
            )
        },
        [3, 3],
        |_| true,
    );
    Landscape::from_founded("tiny", TINY_SEED, founded)
}

/// A seed whose tiny ring places founders of both lineages.
const TINY_SEED: u64 = 6;

fn run(prepared: &Prepared, driver: &EpisodeDriver, seed: u64) -> driver::Episode {
    let cancel = AtomicBool::new(false);
    driver::run_prepared_seeded(prepared, driver, TICKS, Limits::new(&cancel), "t", seed)
        .expect("a landscape episode completes")
}

#[test]
fn a_landscape_episode_is_deterministic_given_its_seed() {
    let land = landscape();
    for founder in Founder::ALL {
        let prepared = Prepared::Landscape(Box::new(
            land.prepare(founder, WaterState::Drained)
                .expect("the drained state always exists"),
        ));
        let heuristic = EpisodeDriver::control(VoxelControl::Heuristic, founder);
        let a = run(&prepared, &heuristic, 11);
        let b = run(&prepared, &heuristic, 11);
        assert_eq!(a, b, "{founder:?}: the same seed is the same episode");
        let c = run(&prepared, &heuristic, 12);
        let sizes =
            |e: &driver::Episode| -> Vec<f64> { e.bodies.iter().map(|b| b.start_body).collect() };
        assert_ne!(
            sizes(&a),
            sizes(&c),
            "{founder:?}: another seed samples other body sizes (D11)"
        );
    }
}

#[test]
fn frozen_water_does_not_move_during_a_landscape_episode() {
    let land = landscape();
    let prepared = land
        .prepare(Founder::Blind, WaterState::MidShower)
        .expect("the tiny ring rains, so a mid-shower state was captured");
    // The captured state is water in transit: the live schedule would move it.
    let mut live = prepared.world().clone();
    live.step();
    assert_ne!(
        live.view().free,
        prepared.world().view().free,
        "the mid-shower water is moving water, or this test proves nothing"
    );
    let cancel = AtomicBool::new(false);
    let driver = EpisodeDriver::control(VoxelControl::Cruise, Founder::Blind);
    let (_, sim) =
        driver::run_landscape_sim(&prepared, &driver, TICKS, Limits::new(&cancel), "frozen", 3)
            .expect("the episode completes");
    let (before, after) = (prepared.world().view(), sim.world().view());
    assert_eq!(after.free, before.free, "no free water moved");
    assert_eq!(after.pore, before.pore, "no pore water moved");
}

#[test]
fn every_founder_in_a_landscape_episode_runs_the_candidate() {
    let land = landscape();
    for founder in Founder::ALL {
        let prepared = land
            .prepare(founder, WaterState::Drained)
            .expect("the drained state always exists");
        let placed: Vec<_> = land
            .placements()
            .iter()
            .filter(|p| p.founder == founder)
            .collect();
        assert!(
            !placed.is_empty(),
            "{founder:?}: the tiny ring places this lineage, or the test is empty"
        );
        let cancel = AtomicBool::new(false);
        let driver = EpisodeDriver::control(VoxelControl::StationaryFeeding, founder);
        let e = driver::run_prepared_seeded(
            &Prepared::Landscape(Box::new(prepared)),
            &driver,
            TICKS,
            Limits::new(&cancel),
            "every",
            5,
        )
        .expect("the episode completes");
        assert_eq!(
            e.bodies.len(),
            placed.len(),
            "{founder:?}: one acting body per placed founder of the lineage"
        );
        for b in &e.bodies {
            assert!(
                b.samples > 0,
                "{founder:?}: body {} never sampled the candidate",
                b.id
            );
        }
    }
}

/// P5-C: teacher recording on a landscape keeps **one buffer per acting body**. The
/// shared buffer P5-B found interleaved the bodies' samples into one stream and every
/// body's `reset()` wiped the others'. Each body's buffer holds exactly the samples that
/// body's controller took, the recorded episode is the unrecorded one, and
/// [`imitate::record_fixtures`] turns it into one valid stream per body.
#[test]
fn landscape_teacher_recording_keeps_one_stream_per_body() {
    let land = landscape();
    for founder in Founder::ALL {
        let prepared = Prepared::Landscape(Box::new(
            land.prepare(founder, WaterState::Drained)
                .expect("the drained state always exists"),
        ));
        let plain = EpisodeDriver::control(VoxelControl::Heuristic, founder);
        let bare = run(&prepared, &plain, 9);
        assert!(
            bare.bodies.len() >= 2,
            "{founder:?}: the tiny ring must place several bodies of this lineage, or the \
             test proves nothing"
        );

        let sink = teacher_sink();
        let recorded = run(&prepared, &plain.clone().recording(sink.clone()), 9);
        assert_eq!(recorded, bare, "{founder:?}: the recording wrapper is transparent");

        let buffers = sink.lock().expect("sink").clone();
        assert_eq!(
            buffers.len(),
            recorded.bodies.len(),
            "{founder:?}: one buffer per acting body"
        );
        for (buffer, body) in buffers.iter().zip(&recorded.bodies) {
            assert_eq!(
                buffer.len() as u64,
                body.samples,
                "{founder:?}: body {}'s buffer holds exactly its own samples",
                body.id
            );
        }
        assert_ne!(
            buffers[0][0].observation, buffers[1][0].observation,
            "{founder:?}: two bodies' streams are two bodies' observations"
        );

        let streams = imitate::record_fixtures(founder, &[prepared], Some(TICKS), 1)
            .expect("the landscape records");
        assert_eq!(streams.len(), recorded.bodies.len(), "{founder:?}");
        for (k, s) in streams.iter().enumerate() {
            s.validate("landscape stream", founder).expect("valid");
            assert_eq!(s.body, k);
            assert_eq!(s.stage, imitate::LANDSCAPE_STAGE);
        }
    }
}
