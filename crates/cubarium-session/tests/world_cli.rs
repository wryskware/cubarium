//! CLI mechanisms on tiny worlds, with short clocks and no ecological assertions.
use std::{
    path::{Path, PathBuf},
    process::{Command, Output},
    sync::atomic::{AtomicU64, Ordering},
};

use cubarium_observatory::query;
use cubarium_session::store;
use cubarium_voxel::DT;

static NEXT: AtomicU64 = AtomicU64::new(0);
struct Scratch(PathBuf);
impl Scratch {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "cubarium-world-test-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&path).unwrap();
        Self(path)
    }
    fn config(&self) -> PathBuf {
        let path = self.0.join("tiny.toml");
        std::fs::write(&path, "threads = 1\n[world]\nwidth = 32\nheight = 24\ndepth = 4\nvoxel_m = 0.25\n[world.landform]\npreset = 'default'\n[world.landform.water]\nmin_lake_m2 = 0.0\nmin_tier_pools = 0\n[world.day]\nmode = 'cycle'\nlength_min = 0.1\nstart_hour = 23.5\ndaylight = 0.5\n[world.climate]\nopening_min_s = 1.0\n").unwrap();
        path
    }
    fn found(&self) -> PathBuf {
        let path = self.0.join("initial");
        success(&[
            "found",
            "--config",
            text(&self.config()),
            "--seed",
            "7",
            "--empty",
            "--out",
            text(&path),
        ]);
        path
    }
}
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
fn text(path: &Path) -> &str {
    path.to_str().unwrap()
}
fn run(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_cubarium-world"))
        .args(["--threads", "1"])
        .args(args)
        .output()
        .unwrap()
}
fn success(args: &[&str]) -> Output {
    let output = run(args);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    output
}

#[test]
fn found_is_immediately_loadable_and_tick_stop_saves_the_requested_tick() {
    let dir = Scratch::new();
    let initial = dir.found();
    let meta = store::read_meta(&initial).unwrap();
    let loaded = store::load_save(&initial).unwrap();
    assert_eq!(loaded.world.tick(), meta.tick);
    let target = meta.tick + 7;
    let prefix = dir.0.join("advanced");
    success(&[
        "advance",
        "--load",
        text(&initial),
        "--stop",
        &format!("tick:{target}"),
        "--out-prefix",
        text(&prefix),
    ]);
    let save = dir.0.join(format!("advanced-tick-{target}"));
    assert_eq!(store::read_meta(&save).unwrap().tick, target);
    assert_eq!(store::load_save(&save).unwrap().world.tick(), target);
}

#[test]
fn clock_stop_wraps_midnight_and_lands_within_one_tick() {
    let dir = Scratch::new();
    let initial = dir.found();
    let prefix = dir.0.join("clock");
    success(&[
        "advance",
        "--load",
        text(&initial),
        "--stop",
        "00:05",
        "--out-prefix",
        text(&prefix),
    ]);
    let loaded = store::load_save(&dir.0.join("clock-00-05")).unwrap();
    let clock = query::clock(&loaded.world);
    let requested = 5.0 / 1440.0;
    let overshoot = (clock.day_phase - requested).rem_euclid(1.0);
    assert!(overshoot <= DT / clock.day_length_s + 1e-12, "{clock:?}");
    assert!(loaded.world.tick() > store::read_meta(&initial).unwrap().tick);
}

#[test]
fn several_stops_save_in_order_including_absolute_sim_minutes() {
    let dir = Scratch::new();
    let initial = dir.found();
    let prefix = dir.0.join("ordered");
    let output = success(&[
        "advance",
        "--load",
        text(&initial),
        "--stop",
        "tick:2",
        "--stop",
        "minute:0.01",
        "--stop",
        "00:05",
        "--out-prefix",
        text(&prefix),
    ]);
    let names = ["tick-2", "minute-0.01", "00-05"];
    let mut previous = 0;
    for name in names {
        let path = dir.0.join(format!("ordered-{name}"));
        let meta = store::read_meta(&path).unwrap();
        assert!(meta.tick > previous);
        assert_eq!(store::load_save(&path).unwrap().world.tick(), meta.tick);
        previous = meta.tick;
    }
    assert_eq!(
        store::read_meta(&dir.0.join("ordered-minute-0.01"))
            .unwrap()
            .tick,
        (0.01 * 60.0 / DT).ceil() as u64
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    let positions = names.map(|name| stdout.find(&format!("ordered-{name}")).unwrap());
    assert!(positions.windows(2).all(|w| w[0] < w[1]));
}

#[test]
fn named_stops_follow_the_worlds_sunrise_and_sunset() {
    let dir = Scratch::new();
    let initial = dir.found();
    let clock = query::clock(&store::load_save(&initial).unwrap().world);
    let daylight = (clock.sunset_phase - clock.sunrise_phase).rem_euclid(1.0);
    let expected = [
        clock.sunrise_phase,
        clock.sunrise_phase + daylight / 2.0,
        clock.sunset_phase,
        clock.sunset_phase + (1.0 - daylight) / 2.0,
    ];
    success(&[
        "advance",
        "--load",
        text(&initial),
        "--stop",
        "dawn",
        "--stop",
        "midday",
        "--stop",
        "dusk",
        "--stop",
        "night",
        "--out-prefix",
        text(&dir.0.join("named")),
    ]);
    let mut previous = 0;
    for (name, phase) in ["dawn", "midday", "dusk", "night"]
        .into_iter()
        .zip(expected)
    {
        let loaded = store::load_save(&dir.0.join(format!("named-{name}"))).unwrap();
        let at = query::clock(&loaded.world);
        assert!((at.day_phase - phase).rem_euclid(1.0) <= DT / at.day_length_s + 1e-12);
        assert!(at.tick > previous);
        previous = at.tick;
    }
}

#[test]
fn toml_and_json_specs_work_and_unknown_fields_are_refused() {
    let dir = Scratch::new();
    let config = dir.config();
    for extension in ["toml", "json"] {
        let spec = dir.0.join(format!("spec.{extension}"));
        let value = cubarium_session::WorldSpec {
            config: Some(config.clone()),
            seed: Some(7),
            empty: true,
            ..Default::default()
        };
        let mut contents = if extension == "toml" {
            toml::to_string(&value).unwrap()
        } else {
            serde_json::to_string(&value).unwrap()
        };
        std::fs::write(&spec, &contents).unwrap();
        let save = dir.0.join(extension);
        success(&["found", "--spec", text(&spec), "--out", text(&save)]);
        store::load_save(&save).unwrap();
        if extension == "toml" {
            contents.push_str("\nseeeed = 4\n");
        } else {
            contents.pop();
            contents.push_str(",\"seeeed\":4}");
        }
        std::fs::write(&spec, contents).unwrap();
        let refused = dir.0.join(format!("refused-{extension}"));
        let output = run(&["found", "--spec", text(&spec), "--out", text(&refused)]);
        assert!(!output.status.success());
        assert!(String::from_utf8_lossy(&output.stderr).contains("unknown field"));
        assert!(!refused.exists());
    }
}

#[test]
fn invalid_and_duplicate_stops_are_refused() {
    let dir = Scratch::new();
    let initial = dir.found();
    for stop in [
        "25:00",
        "minute:NaN",
        "minute:-1",
        "minute:-0.000001",
        "tick:-1",
        "noon",
    ] {
        assert!(
            !run(&[
                "advance",
                "--load",
                text(&initial),
                "--stop",
                stop,
                "--out-prefix",
                text(&dir.0.join("bad"))
            ])
            .status
            .success()
        );
    }
    let output = run(&[
        "advance",
        "--load",
        text(&initial),
        "--stop",
        "dawn",
        "--stop",
        "dawn",
        "--out-prefix",
        text(&dir.0.join("bad")),
    ]);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("duplicate output name"));
}

#[test]
fn a_clock_time_equal_to_the_loaded_clock_means_the_next_day() {
    let dir = Scratch::new();
    // Noon has an exactly representable phase and an unambiguous HH:MM floor.
    let config = dir.config();
    let contents = std::fs::read_to_string(&config)
        .unwrap()
        .replace("start_hour = 23.5", "start_hour = 12.0");
    std::fs::write(&config, contents).unwrap();
    let initial = dir.0.join("initial");
    success(&[
        "found",
        "--config",
        text(&config),
        "--seed",
        "7",
        "--empty",
        "--out",
        text(&initial),
    ]);
    let before = query::clock(&store::load_save(&initial).unwrap().world);
    success(&[
        "advance",
        "--load",
        text(&initial),
        "--stop",
        &before.time_of_day,
        "--out-prefix",
        text(&dir.0.join("next-day")),
    ]);
    let save = dir
        .0
        .join(format!("next-day-{}", before.time_of_day.replace(':', "-")));
    let loaded = store::load_save(&save).unwrap();
    let elapsed = (loaded.world.tick() - before.tick) as f64 * DT;
    assert!(elapsed >= before.day_length_s && elapsed <= before.day_length_s + DT + 1e-12);
}

#[test]
fn clock_stops_refuse_modes_that_do_not_follow_simulated_time() {
    let dir = Scratch::new();
    for mode in ["off", "local"] {
        let config = dir.config();
        let contents = std::fs::read_to_string(&config)
            .unwrap()
            .replace("mode = 'cycle'", &format!("mode = '{mode}'"));
        std::fs::write(&config, contents).unwrap();
        let initial = dir.0.join(mode);
        success(&[
            "found",
            "--config",
            text(&config),
            "--empty",
            "--seed",
            "7",
            "--out",
            text(&initial),
        ]);
        let output = run(&[
            "advance",
            "--load",
            text(&initial),
            "--stop",
            "dawn",
            "--out-prefix",
            text(&dir.0.join("refused")),
        ]);
        assert!(!output.status.success());
        assert!(String::from_utf8_lossy(&output.stderr).contains("require cycle mode"));
    }
}
