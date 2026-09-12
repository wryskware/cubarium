//! The `cubarium demo` and `cubarium run` command contracts from
//! `crates/cubarium/README.md`.

use std::path::PathBuf;

use clap::{Parser, Subcommand, ValueEnum};

use crate::scene::SceneKind;

#[derive(Parser, Debug)]
#[command(name = "cubarium", about = "Cubarium host: clock, fixture scenes, output sinks")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand, Debug)]
pub enum Command {
    /// Run the M1 geometry fixtures.
    Demo(Demo),
    /// Run the persistent M2 world.
    Run(Run),
}

#[derive(Parser, Debug, Clone)]
pub struct Demo {
    /// Which fixture(s) to run.
    #[arg(long, value_enum, default_value_t = SceneArg::All)]
    pub scene: SceneArg,
    /// Where frames go.
    #[arg(long, value_enum, default_value_t = SinkArg::Preview)]
    pub sink: SinkArg,
    /// Stop after this much wall time; 0 runs until closed. Required for `png`.
    #[arg(long, default_value_t = 0.0)]
    pub seconds: f64,
    /// Seed for the fixture's own deterministic PRNG.
    #[arg(long, default_value_t = 1)]
    pub seed: u64,
    /// Shim daemon address.
    #[arg(long, default_value = "127.0.0.1:7392")]
    pub addr: String,
    /// Directory for PNG captures.
    #[arg(long, default_value = "captures")]
    pub out: PathBuf,
    /// With `png`, save one capture every N rendered frames.
    #[arg(long, default_value_t = 30)]
    pub every: u64,
    /// Preview window pixel scale.
    #[arg(long, default_value_t = 4)]
    pub scale: usize,
}

#[derive(ValueEnum, Clone, Copy, Debug, PartialEq, Eq)]
pub enum SceneArg {
    Body,
    Vertex,
    Patch,
    All,
}

impl From<SceneArg> for SceneKind {
    fn from(a: SceneArg) -> SceneKind {
        match a {
            SceneArg::Body => SceneKind::Body,
            SceneArg::Vertex => SceneKind::Vertex,
            SceneArg::Patch => SceneKind::Patch,
            SceneArg::All => SceneKind::All,
        }
    }
}

#[derive(ValueEnum, Clone, Copy, Debug, PartialEq, Eq)]
pub enum SinkArg {
    Preview,
    Shim,
    Png,
}

/// `run` adds a headless sink to the three `demo` sinks.
#[derive(ValueEnum, Clone, Copy, Debug, PartialEq, Eq)]
pub enum RunSinkArg {
    Preview,
    Shim,
    Png,
    /// Headless: no canvas, no encode, no frames.
    None,
}

impl RunSinkArg {
    /// True for the sinks that put pixels somewhere; `--speed 0` is refused for these.
    pub fn is_visual(self) -> bool {
        self != RunSinkArg::None
    }
}

/// `cubarium run`: the persistent world, its snapshots, and its telemetry.
#[derive(Parser, Debug, Clone)]
pub struct Run {
    /// TOML `WorldConfig`; missing fields take defaults, unknown fields are errors.
    #[arg(long)]
    pub config: Option<PathBuf>,
    /// Directory for snapshots and the journal.
    #[arg(long, default_value = "state")]
    pub state: PathBuf,
    /// Where frames go; `none` runs headless.
    #[arg(long, value_enum, default_value_t = RunSinkArg::Preview)]
    pub sink: RunSinkArg,
    /// Simulated seconds per wall second; 0 means as fast as possible (headless only).
    #[arg(long, default_value_t = 1.0)]
    pub speed: f64,
    /// Stop after this much simulated time; 0 runs until closed.
    #[arg(long, default_value_t = 0.0)]
    pub seconds: f64,
    /// Overrides `config.seed` when creating a fresh world.
    #[arg(long)]
    pub seed: Option<u64>,
    /// Ignore existing snapshots and create a new world.
    #[arg(long, default_value_t = false)]
    pub fresh: bool,
    /// JSON-lines telemetry file (appended). Defaults to `<state>/telemetry.jsonl`.
    #[arg(long)]
    pub telemetry: Option<PathBuf>,
    /// Shim daemon address.
    #[arg(long, default_value = "127.0.0.1:7392")]
    pub addr: String,
    /// Directory for PNG captures.
    #[arg(long, default_value = "captures")]
    pub out: PathBuf,
    /// With `png`, save one capture every N rendered frames.
    #[arg(long, default_value_t = 30)]
    pub every: u64,
    /// Preview window pixel scale.
    #[arg(long, default_value_t = 4)]
    pub scale: usize,
}

impl Run {
    /// The telemetry path after the documented default is applied.
    pub fn telemetry_path(&self) -> PathBuf {
        self.telemetry.clone().unwrap_or_else(|| self.state.join("telemetry.jsonl"))
    }

    /// Reject combinations the contract forbids.
    pub fn validate(&self) -> anyhow::Result<()> {
        anyhow::ensure!(self.seconds >= 0.0 && self.seconds.is_finite(), "--seconds must be >= 0");
        anyhow::ensure!(self.speed >= 0.0 && self.speed.is_finite(), "--speed must be >= 0");
        anyhow::ensure!(
            self.speed > 0.0 || !self.sink.is_visual(),
            "--speed 0 is headless only; use --sink none"
        );
        // Same rule as `demo`: a capture run that never ends writes captures forever.
        if self.sink == RunSinkArg::Png {
            anyhow::ensure!(self.seconds > 0.0, "--seconds is required with --sink png");
        }
        anyhow::ensure!(self.every >= 1, "--every must be at least 1");
        anyhow::ensure!(self.scale >= 1, "--scale must be at least 1");
        Ok(())
    }
}

impl Demo {
    /// Reject combinations the contract forbids.
    pub fn validate(&self) -> anyhow::Result<()> {
        anyhow::ensure!(self.seconds >= 0.0 && self.seconds.is_finite(), "--seconds must be >= 0");
        if self.sink == SinkArg::Png {
            anyhow::ensure!(self.seconds > 0.0, "--seconds is required with --sink png");
        }
        anyhow::ensure!(self.every >= 1, "--every must be at least 1");
        anyhow::ensure!(self.scale >= 1, "--scale must be at least 1");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;

    /// Parse a `demo` command line, panicking if it is not one.
    fn demo<const N: usize>(args: [&str; N]) -> Demo {
        match Cli::parse_from(args).command {
            Command::Demo(d) => d,
            other => panic!("expected a demo command, got {other:?}"),
        }
    }

    /// Parse a `run` command line, panicking if it is not one.
    fn parse_run<const N: usize>(args: [&str; N]) -> Run {
        match Cli::parse_from(args).command {
            Command::Run(r) => r,
            other => panic!("expected a run command, got {other:?}"),
        }
    }

    #[test]
    fn the_command_line_matches_the_documented_defaults() {
        Cli::command().debug_assert();
        let d = demo(["cubarium", "demo"]);
        assert_eq!(d.scene, SceneArg::All);
        assert_eq!(d.sink, SinkArg::Preview);
        assert_eq!(d.seconds, 0.0);
        assert_eq!(d.seed, 1);
        assert_eq!(d.addr, "127.0.0.1:7392");
        assert_eq!(d.out, PathBuf::from("captures"));
        assert_eq!(d.every, 30);
        assert_eq!(d.scale, 4);
    }

    #[test]
    fn every_documented_option_parses() {
        let d = demo([
            "cubarium", "demo", "--scene", "vertex", "--sink", "shim", "--seconds", "20",
            "--seed", "9", "--addr", "10.0.0.4:1234", "--out", "/tmp/c", "--every", "5",
            "--scale", "2",
        ]);
        assert_eq!(d.scene, SceneArg::Vertex);
        assert_eq!(d.sink, SinkArg::Shim);
        assert_eq!(d.seconds, 20.0);
        assert_eq!(d.seed, 9);
        assert_eq!(d.addr, "10.0.0.4:1234");
        assert_eq!(d.out, PathBuf::from("/tmp/c"));
        assert_eq!(d.every, 5);
        assert_eq!(d.scale, 2);
        d.validate().unwrap();
    }

    #[test]
    fn png_requires_a_duration() {
        let d = demo(["cubarium", "demo", "--sink", "png"]);
        assert!(d.validate().is_err());
        let d = demo(["cubarium", "demo", "--sink", "png", "--seconds", "1"]);
        d.validate().unwrap();
    }

    #[test]
    fn the_run_command_line_matches_the_documented_defaults() {
        let r = parse_run(["cubarium", "run"]);
        assert_eq!(r.config, None);
        assert_eq!(r.state, PathBuf::from("state"));
        assert_eq!(r.sink, RunSinkArg::Preview);
        assert_eq!(r.speed, 1.0);
        assert_eq!(r.seconds, 0.0);
        assert_eq!(r.seed, None);
        assert!(!r.fresh);
        assert_eq!(r.telemetry, None);
        assert_eq!(r.telemetry_path(), PathBuf::from("state/telemetry.jsonl"));
        assert_eq!(r.addr, "127.0.0.1:7392");
        assert_eq!(r.out, PathBuf::from("captures"));
        assert_eq!(r.every, 30);
        assert_eq!(r.scale, 4);
        r.validate().unwrap();
    }

    #[test]
    fn every_documented_run_option_parses() {
        let r = parse_run([
            "cubarium", "run", "--config", "w.toml", "--state", "/tmp/s", "--sink", "none",
            "--speed", "0", "--seconds", "600", "--seed", "7", "--fresh", "--telemetry",
            "/tmp/t.jsonl", "--addr", "10.0.0.4:1", "--out", "/tmp/c", "--every", "5",
            "--scale", "2",
        ]);
        assert_eq!(r.config, Some(PathBuf::from("w.toml")));
        assert_eq!(r.state, PathBuf::from("/tmp/s"));
        assert_eq!(r.sink, RunSinkArg::None);
        assert_eq!(r.speed, 0.0);
        assert_eq!(r.seconds, 600.0);
        assert_eq!(r.seed, Some(7));
        assert!(r.fresh);
        assert_eq!(r.telemetry_path(), PathBuf::from("/tmp/t.jsonl"));
        assert_eq!(r.addr, "10.0.0.4:1");
        assert_eq!(r.out, PathBuf::from("/tmp/c"));
        assert_eq!(r.every, 5);
        assert_eq!(r.scale, 2);
        r.validate().unwrap();
    }

    #[test]
    fn run_png_requires_a_duration_like_demo_does() {
        let r = parse_run(["cubarium", "run", "--sink", "png"]);
        let err = r.validate().unwrap_err().to_string();
        assert!(err.contains("--seconds is required"), "{err}");
        parse_run(["cubarium", "run", "--sink", "png", "--seconds", "1"]).validate().unwrap();
    }

    #[test]
    fn speed_zero_is_refused_for_every_visual_sink() {
        for sink in ["preview", "shim", "png"] {
            let r = parse_run(["cubarium", "run", "--sink", sink, "--speed", "0"]);
            let err = r.validate().unwrap_err().to_string();
            assert!(err.contains("headless"), "{sink}: {err}");
        }
        parse_run(["cubarium", "run", "--sink", "none", "--speed", "0"]).validate().unwrap();
    }

    #[test]
    fn negative_durations_and_speeds_are_refused() {
        // `--seconds -1` would be read as a flag, so the contract's negative values are
        // spelled with `=`.
        assert!(parse_run(["cubarium", "run", "--seconds=-1"]).validate().is_err());
        assert!(parse_run(["cubarium", "run", "--speed=-1"]).validate().is_err());
        assert!(parse_run(["cubarium", "run", "--speed=nan"]).validate().is_err());
        assert!(parse_run(["cubarium", "run", "--every", "0"]).validate().is_err());
        assert!(parse_run(["cubarium", "run", "--scale", "0"]).validate().is_err());
    }

    #[test]
    fn scene_names_map_onto_the_fixtures() {
        assert_eq!(SceneKind::from(SceneArg::Body), SceneKind::Body);
        assert_eq!(SceneKind::from(SceneArg::Vertex), SceneKind::Vertex);
        assert_eq!(SceneKind::from(SceneArg::Patch), SceneKind::Patch);
        assert_eq!(SceneKind::from(SceneArg::All), SceneKind::All);
    }
}
