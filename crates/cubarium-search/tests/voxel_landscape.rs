//! P5-B item 4: landscape episodes — whole frozen worlds founded by the moved founding
//! loop (D5), water frozen in the state the pre-roll captured (D6), every founder the
//! seeder placed running the candidate (D7), bodies sampled per episode (D11).
//!
//! The landscape here is a short staged ring founded on an asked-for seed with a shower
//! of a few seconds, so founding it costs a small fraction of a second; the episodes run
//! at most 200 ticks. The shipped presets are the same code at a larger size.

use std::sync::atomic::AtomicBool;

use cubarium_search::es::voxel::driver::{self, Limits};
use cubarium_search::es::voxel::imitate;
use cubarium_search::es::voxel::landscape::{Landscape, WaterState};
use cubarium_search::es::voxel::task::Prepared;
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
        assert_eq!(
            recorded, bare,
            "{founder:?}: the recording wrapper is transparent"
        );

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

/// P5-C (C1, C2): a mixed run trains on the arena half plus that generation's draw of
/// landscapes, and scores the centre on the held-out fixtures at update 0 and every
/// `held_out_every` updates, writing a policy file holding exactly the weights scored.
/// Plumbing only: one pair, two updates, 60-tick episodes on the tiny ring.
#[test]
fn a_mixed_run_draws_landscapes_and_writes_held_out_checkpoints() {
    use cubarium_search::es::voxel::store::VoxelPolicyFile;
    use cubarium_search::es::voxel::task::{Band, Stage};
    use cubarium_search::es::voxel::trainer::{self, LandscapeMix, TrainSpec};

    let land = landscape();
    let fixture = |founder, water| {
        Prepared::Landscape(Box::new(
            land.prepare(founder, water)
                .expect("the tiny ring rains")
                .with_horizon(60),
        ))
    };
    let founder = Founder::Blind;
    let pool = vec![
        fixture(founder, WaterState::Drained),
        fixture(founder, WaterState::MidShower),
    ];
    let held_out = vec![fixture(founder, WaterState::Drained)];
    let out = std::env::temp_dir().join(format!("cubarium-p5c-mix-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&out);
    let spec = TrainSpec {
        founder,
        stage: Stage::B,
        band: Band::Landed,
        init_center: None,
        pairs: 1,
        layouts: 1,
        updates: 2,
        horizon: 60,
        workers: 2,
        wall_seconds: 120,
        episode_limit: u64::MAX,
        train_seed: 5,
        evaluate_center: true,
        out: out.clone(),
        mix: Some(LandscapeMix {
            pool,
            per_generation: 1,
            held_out,
            held_out_every: 1,
            collapse_checkpoints: 4,
            plateau: None,
        }),
        remotes: None,
    };
    let cancel = AtomicBool::new(false);
    let report = trainer::train(&spec, &cancel).expect("the mixed run runs");
    assert_eq!(report.generations_completed, 2);
    // Initial centre on 1 arena + 1 landscape; then (2 signs + centre) × 2 fixtures × 2.
    assert_eq!(report.episodes_attempted, 2 + 2 * 3 * 2);
    let updates: Vec<u32> = report.held_out.iter().map(|h| h.updates).collect();
    assert_eq!(
        updates,
        vec![0, 1, 2],
        "held-out at 0 and after every update"
    );
    let cp = trainer::load_checkpoint(&out.join("checkpoint.json"), None).expect("checkpoint");
    let mix = cp
        .protocol
        .mix
        .as_ref()
        .expect("the mix is in the protocol");
    assert_eq!(mix.pool.len(), 2);
    assert_eq!(mix.arena_grids, vec!["0.25m".to_string()]);
    for h in &report.held_out {
        let file = VoxelPolicyFile::load(&out.join(&h.file)).expect("a held-out policy file");
        assert_eq!(file.score, Some(h.score));
        let d = file.driver().expect("drives");
        let episodes = trainer::evaluate_fixtures(
            &d,
            &spec.mix.as_ref().expect("mix").held_out,
            1,
            Limits::new(&cancel),
            "recheck",
        )
        .expect("re-evaluated");
        assert_eq!(
            episodes[0].score.score, h.score,
            "the file holds exactly the weights that were scored"
        );
    }
    let _ = std::fs::remove_dir_all(&out);
}

/// P5-C (coordinator's decision): a landscape's acting bodies start on faces drawn from
/// the standable faces of the lineage's judged components, not on the seeder's founder
/// faces — deterministic per episode seed, each start standable and in one of those
/// components, distinct, and (mid-shower) still standable in that water.
#[test]
fn acting_bodies_start_on_drawn_faces_of_the_judged_components() {
    use cubarium_voxel_fauna::RouteMap;

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
    let cfg = FaunaConfig::default();
    for founder in Founder::ALL {
        let map = RouteMap::for_founder(&founded.world.view(), cfg.founder(founder));
        let verdict = &founded.seeded.acceptance.lineages[founder.index()];
        let habitable: Vec<usize> = verdict
            .components
            .iter()
            .filter(|c| c.habitable)
            .map(|c| c.component)
            .collect();
        let judged: Vec<usize> = if habitable.is_empty() {
            verdict.components.iter().map(|c| c.component).collect()
        } else {
            habitable
        };
        assert!(
            !judged.is_empty(),
            "{founder:?}: the seeder judged a component"
        );
        let land_faces =
            cubarium_search::es::voxel::landscape::start_faces_of(&founded, &cfg, founder);
        assert!(
            land_faces.len() > 3,
            "{founder:?}: a pool to draw from, or the test proves nothing"
        );
        for f in &land_faces {
            let c = map.component_of(*f).expect("every start face is standable");
            assert!(
                judged.contains(&c),
                "{founder:?}: {f:?} is in a judged component"
            );
        }
    }

    let land = Landscape::from_founded("tiny", TINY_SEED, founded);
    for founder in Founder::ALL {
        for water in WaterState::ALL {
            let prepared = land.prepare(founder, water).expect("the tiny ring rains");
            let a = prepared.acting_starts(11);
            assert_eq!(a, prepared.acting_starts(11), "a pure function of the seed");
            assert_ne!(a, prepared.acting_starts(12), "another seed, other starts");
            assert_eq!(a.len(), prepared.acting().count(), "the seeder's count");
            let mut sites: Vec<_> = a.iter().map(|p| (p.site.x, p.site.y, p.site.z)).collect();
            sites.sort_unstable();
            sites.dedup();
            assert_eq!(sites.len(), a.len(), "distinct faces");
            for p in &a {
                assert_eq!(p.founder, founder);
                assert!(prepared.start_faces().contains(&p.site));
                assert!((0.0..std::f64::consts::TAU).contains(&p.heading_rad));
            }
            let seeder: Vec<_> = prepared.acting().map(|p| p.site).collect();
            assert!(
                a.iter().any(|p| !seeder.contains(&p.site)),
                "{founder:?}: the starts are drawn, not the seeder's faces"
            );
            if water == WaterState::MidShower {
                let standable =
                    RouteMap::for_founder(&prepared.world().view(), cfg.founder(founder));
                for p in &a {
                    assert!(
                        standable.face(p.site).is_some(),
                        "{founder:?}: a mid-shower start is standable in that water"
                    );
                }
            }
            // The episode runs them all.
            let e = run(
                &Prepared::Landscape(Box::new(prepared)),
                &EpisodeDriver::control(VoxelControl::StationaryFeeding, founder),
                11,
            );
            assert_eq!(e.bodies.len(), a.len());
        }
    }
}

/// The diagnostic `without_bystanders` leaves the other lineage out of the episode.
#[test]
fn a_fixture_without_bystanders_holds_only_the_acting_lineage() {
    let land = landscape();
    let cancel = AtomicBool::new(false);
    for founder in Founder::ALL {
        let prepared = land.prepare(founder, WaterState::Drained).expect("drained");
        let acting = prepared.acting().count();
        let others = prepared.bystanders().count();
        assert!(others > 0, "{founder:?}: the ring places the other lineage");
        let d = EpisodeDriver::control(VoxelControl::StationaryFeeding, founder);
        let (_, with) =
            driver::run_landscape_sim(&prepared, &d, 2, Limits::new(&cancel), "with", 1)
                .expect("runs");
        let alone = prepared.without_bystanders();
        assert_eq!(alone.bystanders().count(), 0);
        let (_, without) =
            driver::run_landscape_sim(&alone, &d, 2, Limits::new(&cancel), "without", 1)
                .expect("runs");
        assert_eq!(with.fauna().view().animals.len(), acting + others);
        assert_eq!(without.fauna().view().animals.len(), acting);
    }
}

/// S1 (replay): recorded production, replayed onto the frozen world tick by tick with
/// the dead pools decomposing at the model's rates, reproduces the live run's litter on
/// every face; and an episode hands the plant layer exactly the recorded deposits, with
/// the plant ledger closing.
#[test]
fn replayed_production_matches_the_live_litter_and_is_booked() {
    use cubarium_search::es::voxel::landscape::record_production;
    let land = landscape();
    let prepared = land
        .prepare(Founder::Blind, WaterState::Drained)
        .expect("drained");
    let ticks = 150;
    let p = record_production(prepared.world(), prepared.flora(), ticks, 1);
    assert!(
        p.litter_total() > 0.0,
        "the tiny ring sheds litter in 150 ticks"
    );

    // The live run, again, for its final litter.
    let mut live = cubarium_voxel_sim::Sim::new(
        prepared.world().clone(),
        prepared.flora().clone(),
        Fauna::new(FaunaConfig::default()),
        cubarium_voxel_sim::SimConfig { threads: 1 },
        None,
    );
    for _ in 0..ticks {
        live.step();
    }
    // The replay on the frozen plant layer.
    let mut replay = prepared.flora().clone();
    for t in 0..ticks {
        p.apply(&mut replay, t);
    }
    let (lv, rv) = (live.flora().view(), replay.view());
    for g in lv.ground {
        let r = rv.ground_at(g.site).map_or(0.0, |r| r.litter);
        assert!(
            (g.litter - r).abs() <= 1e-12 * (1.0 + g.litter),
            "face {:?}: live litter {} vs replayed {}",
            g.site,
            g.litter,
            r
        );
    }
    assert!(
        (rv.organic() - rv.ledger.expected_organic()).abs() < 1e-9,
        "replay ledger closes"
    );

    // An episode receives exactly the bucketed deposits.
    let p = record_production(prepared.world(), prepared.flora(), ticks, 50);
    let want: f64 = p.buckets.iter().flatten().map(|(_, d)| d.organic).sum();
    assert!(
        p.buckets
            .iter()
            .all(|b| b.iter().all(|(_, d)| d.organic > 0.0))
    );
    let fixture = prepared
        .clone()
        .with_horizon(ticks)
        .with_production(std::sync::Arc::new(p));
    let cancel = AtomicBool::new(false);
    let d = EpisodeDriver::control(VoxelControl::NoIntake, Founder::Blind);
    let before = fixture.flora().view().ledger.deposited_organic_in;
    let (_, sim) =
        driver::run_landscape_sim(&fixture, &d, ticks, Limits::new(&cancel), "replay", 3)
            .expect("runs");
    let fv = sim.flora().view();
    let deposited = fv.ledger.deposited_organic_in - before;
    // Bodies deposit nothing in 150 ticks without eating or dying; the rest is the replay.
    assert!(
        (deposited - want).abs() < 1e-12,
        "deposited {deposited} vs recorded {want}"
    );
    assert!(
        (fv.organic() - fv.ledger.expected_organic()).abs() < 1e-9,
        "episode ledger closes"
    );
}
