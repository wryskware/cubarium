use super::care_runtime::*;
use super::logging::*;
use super::*;
use crate::care_effects;
use clap::Parser;
use cubarium_core::Telemetry;
use cubarium_core::care::{CareCommand, CareKind, CareTarget};

#[test]
fn apex_commands_apply_and_replay_exactly_at_the_journaled_locations() {
    let command = care::PlannedCommand {
        seq: 1,
        apply_after_tick: 0,
        kind: care::CareKind::SpawnApex,
        target: care::CareTarget {
            face: 0,
            u: 4,
            v: 5,
        },
        second_target: Some(care::CareTarget {
            face: 3,
            u: 60,
            v: 42,
        }),
        dose: care::CareDose::STANDARD,
        client: "test.1".into(),
        request: 1,
    };
    let mut live = World::new(WorldConfig::default()).unwrap();
    let mut replay = World::new(WorldConfig::default()).unwrap();
    assert_eq!(
        apply_planned(&mut live, &command),
        apply_planned(&mut replay, &command)
    );
    assert_eq!(live.hunters().founders_placed, 2);
    assert_eq!(
        cubarium_core::snapshot::state_hash(&live.state),
        cubarium_core::snapshot::state_hash(&replay.state)
    );
    let repeated = care::PlannedCommand {
        seq: 2,
        second_target: None,
        target: care::CareTarget {
            face: 4,
            u: 20,
            v: 21,
        },
        request: 2,
        ..command
    };
    let repeated_receipt = apply_planned(&mut live, &repeated);
    assert_eq!(repeated_receipt.outcome.as_str(), "applied");
    assert_eq!(repeated_receipt, apply_planned(&mut replay, &repeated));
    assert_eq!(live.hunters().founders_placed, 3);
    let feed = care::PlannedCommand {
        seq: 3,
        kind: care::CareKind::Feed,
        request: 3,
        ..repeated
    };
    let feed_receipt = apply_planned(&mut live, &feed);
    assert_eq!(feed_receipt.outcome.as_str(), "applied");
    assert_eq!(feed_receipt, apply_planned(&mut replay, &feed));
    assert_eq!(live.care().admitted_seq, 3);
    assert_eq!(replay.care().admitted_seq, 3);
    assert_eq!(
        cubarium_core::snapshot::state_hash(&live.state),
        cubarium_core::snapshot::state_hash(&replay.state)
    );
}

/// Exercise the runner's receipt hook, not just the effect renderer. These
/// scratch journals have explicit private hooks, independent of process-wide
/// failure injection used by other tests.
#[test]
fn care_flourishes_follow_durable_application_and_boundary_replay_only() {
    for route in ["durable", "replay", "uncertain"] {
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!(
            "cubarium-receipt-visual-{}-{nonce}-{route}",
            std::process::id()
        ));
        std::fs::create_dir(&dir).unwrap();
        let hooks = care::JournalHooks::default();
        if route == "uncertain" {
            hooks.fail_after_sync.store(1, Ordering::Relaxed);
        }
        let journal = care::Journal::open_with_hooks(
            &dir,
            "visual-test",
            "test",
            cubarium_surface::Topology::Cube,
            hooks,
        ).unwrap();
        let service = care::CareService::new("visual-test".to_string(), journal.status());
        let mut rt = CareRuntime {
            service,
            worker: care::JournalWorker::spawn(journal),
            effects: care_effects::CareEffects::default(),
            next_seq: 2,
            replay: Default::default(),
            inflight: Vec::new(),
            holding_at: None,
            failed: false,
            intake_allowed: false,
            released: false,
        };
        let mut world = World::new(WorldConfig::default()).unwrap();
        world.step();
        let opening_hash = cubarium_core::snapshot::state_hash(&world.state);
        let command = care::PlannedCommand::standard(
            1,
            world.tick(),
            care::CareKind::Feed,
            care::CareTarget {
                face: 0,
                u: 32,
                v: 32,
            },
            "visual-test",
            1,
        );
        let sample = |rt: &mut CareRuntime| {
            let mut canvas = Canvas::cube();
            rt.effects
                .draw(command.apply_after_tick + 11, 0.5, &mut canvas);
            let mut frame = Frame::black();
            canvas.encode(&mut frame);
            frame
        };
        assert!(sample(&mut rt).as_bytes().iter().all(|&b| b == 0));
        if route == "replay" {
            rt.replay.push_back(command.clone());
            assert!(rt.apply_replay(&mut world).unwrap());
        } else {
            rt.holding_at = Some(world.tick());
            rt.inflight.push(command.clone());
            assert!(
                rt.worker
                    .submit(care::JournalJob::Accept(vec![command.clone()]))
            );
            // Durable bytes alone do not paint: only the runner consuming a
            // successful acknowledgement may apply the command and its visual.
            assert!(sample(&mut rt).as_bytes().iter().all(|&b| b == 0));
            let deadline = Instant::now() + Duration::from_secs(5);
            while rt.holding_at.is_some() && !rt.failed {
                assert!(
                    Instant::now() < deadline,
                    "journal acknowledgement timed out"
                );
                rt.drain_acks(&mut world).unwrap();
                std::thread::sleep(Duration::from_millis(1));
            }
        }
        let after_application = cubarium_core::snapshot::state_hash(&world.state);
        let frame = sample(&mut rt);
        if route == "uncertain" {
            assert!(rt.failed);
            assert_eq!(after_application, opening_hash);
            assert!(frame.as_bytes().iter().all(|&b| b == 0));
        } else {
            assert_ne!(after_application, opening_hash);
            assert!(frame.as_bytes().iter().any(|&b| b != 0));
            assert_eq!(
                frame.as_bytes(),
                sample(&mut rt).as_bytes(),
                "held visual changed"
            );
        }
        assert_eq!(
            cubarium_core::snapshot::state_hash(&world.state),
            after_application
        );
        rt.worker.shutdown().unwrap();
        // Only this test's unique scratch journal is removed; no live state.
        std::fs::remove_dir_all(&dir).unwrap();
    }
}

/// Reproducible native-resolution review images, using real applied care and
/// the actual art presenter. No HTTP, live state or display transport.
#[test]
#[ignore = "writes isolated native-resolution care review captures"]
fn capture_care_flourishes_on_the_authored_world() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let pack = ArtPack::load(&root.join("assets/atelier")).unwrap();
    let mut presenter = ArtPresenter::new(pack);
    let mut effects = care_effects::CareEffects::default();
    let mut world = World::new(WorldConfig::default()).unwrap();
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let dir = std::env::temp_dir().join(format!("cubarium-care-flourishes-{nonce}"));
    std::fs::create_dir(&dir).unwrap();
    let mut receipt_log = Vec::new();
    for _ in 0..510 {
        if world.tick() == 400 || world.tick() == 470 {
            let command = CareCommand::standard(
                if world.tick() == 400 { 1 } else { 2 },
                world.tick(),
                if world.tick() == 400 {
                    CareKind::Feed
                } else {
                    CareKind::Clean
                },
                CareTarget {
                    face: 0,
                    u: 32.0,
                    v: 48.0,
                },
            );
            let receipt = world.apply_care(&command);
            effects.observe(&command, &receipt);
            receipt_log.push(serde_json::json!({
                    "seq":receipt.seq,"tick":receipt.tick,"kind":command.kind.as_str(),
                    "outcome":receipt.outcome.as_str(),"applied":receipt.outcome.applied().map(care_applied_json),
                }));
        }
        world.step();
        world.drain_events();
        let view = world.render_view();
        presenter.observe(&view);
        if ![
            401, 403, 407, 411, 419, 431, 447, 455, 471, 475, 483, 495, 507,
        ]
        .contains(&world.tick())
        {
            continue;
        }
        let hash = cubarium_core::snapshot::state_hash(&world.state);
        let mut canvas = Canvas::cube();
        presenter.draw(&view, 0.5, &mut canvas);
        for variant in ["base", "flourish"] {
            if variant == "flourish" {
                effects.draw(view.tick, 0.5, &mut canvas);
            }
            let mut frame = Frame::black();
            canvas.encode(&mut frame);
            let mut rgb = Vec::new();
            crate::net::net_rgb8(&frame, &mut rgb);
            crate::sink::png::write_net_png(
                &dir.join(format!("{}-{variant}.png", world.tick())),
                &rgb,
            )
            .unwrap();
        }
        assert_eq!(cubarium_core::snapshot::state_hash(&world.state), hash);
    }
    std::fs::write(
        dir.join("receipts.json"),
        serde_json::to_vec_pretty(&receipt_log).unwrap(),
    )
    .unwrap();
    eprintln!(
        "care-flourish captures: {} (Front32,48; base and flourish share ecology)",
        dir.display()
    );
}

#[test]
fn the_viewer_note_prints_the_speed_without_trailing_zeros() {
    assert_eq!(speed_note(1.0), "1× time");
    assert_eq!(speed_note(8.0), "8× time");
    assert_eq!(speed_note(0.5), "0.5× time");
    assert_eq!(speed_note(2.5), "2.5× time");
    assert_eq!(speed_note(20.0), "20× time");
}

#[test]
fn an_art_directory_that_will_not_load_is_fatal_and_names_itself() {
    let mut run = Run::parse_from(["cubarium"]);
    assert!(
        matches!(open_show(&run), Ok(Show::Plain(_))),
        "no --art is the M2 image"
    );
    run.art = Some(PathBuf::from("/nonexistent/atelier"));
    let err = match open_show(&run) {
        Err(e) => e,
        Ok(_) => panic!("a missing pack must not fall back to discs"),
    };
    assert!(
        format!("{err:#}").contains("/nonexistent/atelier"),
        "the error must name the directory: {err:#}"
    );
}

#[test]
fn pacing_follows_the_documented_rounding() {
    assert_eq!(Pace::of(0.0), Pace::Unlimited);
    assert_eq!(Pace::of(1.0), Pace::TicksPerWallTick(1));
    assert_eq!(Pace::of(20.0), Pace::TicksPerWallTick(20));
    assert_eq!(Pace::of(2.4), Pace::TicksPerWallTick(2));
    assert_eq!(Pace::of(2.6), Pace::TicksPerWallTick(3));
    assert_eq!(Pace::of(0.5), Pace::TicksPerWallTick(1));
    // Below half speed the wall tick slows instead of the simulation stalling.
    assert_eq!(Pace::of(0.25), Pace::WallTicksPerTick(4));
    assert_eq!(Pace::of(0.1), Pace::WallTicksPerTick(10));
}

#[test]
fn cadences_never_round_to_zero_ticks() {
    assert_eq!(ticks_of(60.0), 1200);
    assert_eq!(ticks_of(5.0), 100);
    assert_eq!(ticks_of(0.01), 1);
    assert_eq!(ticks_of(0.0), 1);
    assert_eq!(ticks_of(f64::NAN), 1);
}

#[test]
fn only_capacity_and_moving_weather_come_from_the_file() {
    let mut loaded = WorldConfig {
        seed: 99,
        ..WorldConfig::default()
    };
    loaded.producer.growth = 0.123;
    let mut file = WorldConfig::default();
    file.capacity.max_organisms = 64;
    file.capacity.checkpoint_seconds = 5.0;
    file.weather.moving = false;

    merge_operational(&mut loaded, &file);
    assert_eq!(loaded.seed, 99, "the loaded world keeps its own seed");
    assert_eq!(
        loaded.producer.growth, 0.123,
        "the loaded world keeps its own rates"
    );
    assert_eq!(loaded.capacity.max_organisms, 64);
    assert_eq!(loaded.capacity.checkpoint_seconds, 5.0);
    assert!(!loaded.weather.moving);
}

#[test]
fn a_config_with_an_unknown_field_is_rejected() {
    let dir = std::env::temp_dir().join(format!("cubarium-cfg-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("bad.toml");
    std::fs::write(&path, "seed = 7\nnot_a_field = 1\n").unwrap();
    let err = load_config(&path).unwrap_err().to_string();
    assert!(err.contains("parsing"), "{err}");

    std::fs::write(&path, "seed = 7\n").unwrap();
    let cfg = load_config(&path).unwrap();
    assert_eq!(cfg.seed, 7);
    // Everything unmentioned is the default.
    assert_eq!(cfg.producer.growth, WorldConfig::default().producer.growth);

    // Values the world cannot use are rejected before any state is touched.
    std::fs::write(&path, "[capacity]\nmax_organisms = 0\n").unwrap();
    assert!(load_config(&path).is_err());
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn field_values_are_four_decimals_and_non_finite_values_are_null() {
    let v = rounded(&[
        0.0,
        1.0 / 3.0,
        0.123_449,
        0.123_45,
        -2.5,
        f64::NAN,
        f64::INFINITY,
    ]);
    let n = |i: usize| v[i].as_f64();
    assert_eq!(n(0), Some(0.0));
    assert_eq!(n(1), Some(0.3333));
    assert_eq!(n(2), Some(0.1234));
    assert_eq!(n(3), Some(0.1235));
    assert_eq!(n(4), Some(-2.5));
    assert!(
        v[5].is_null(),
        "NaN must not be written as a number: {:?}",
        v[5]
    );
    assert!(
        v[6].is_null(),
        "an infinity must not be written as a number: {:?}",
        v[6]
    );
}

#[test]
fn the_headless_digest_names_the_numbers_the_contract_asks_for() {
    let sample = Telemetry {
        tick: 1200,
        population: 84,
        births: 3,
        deaths_age: 1,
        mass_residual: 1.5e-9,
        state_hash: 0xdead_beef,
        ..Telemetry::default()
    };
    let line = headless_line(&sample);
    for part in [
        "tick 1200",
        "pop 84",
        "births 3",
        "deaths 1",
        "residual",
        "hash",
    ] {
        assert!(line.contains(part), "{line}");
    }
}
