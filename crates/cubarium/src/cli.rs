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
    /// Render and output rate in frames per second; the simulation stays at 20 Hz.
    #[arg(long, default_value_t = crate::clock::RENDER_HZ)]
    pub fps: u32,
    /// Port for the `web` sink (a viewer page at http://127.0.0.1:<port>/).
    #[arg(long, default_value_t = 7393)]
    pub web_port: u16,
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
    /// Local HTTP viewer mapping the frame onto a rotatable cube.
    Web,
}

/// `run` adds a headless sink to the three `demo` sinks.
#[derive(ValueEnum, Clone, Copy, Debug, PartialEq, Eq)]
pub enum RunSinkArg {
    Preview,
    Shim,
    Png,
    /// Local HTTP viewer mapping the frame onto a rotatable cube.
    Web,
    /// Headless: no canvas, no encode, no frames.
    None,
}

impl RunSinkArg {
    /// True for the sinks that put pixels somewhere; `--speed 0` is refused for these.
    pub fn is_visual(self) -> bool {
        self != RunSinkArg::None
    }

    /// The name `/status` reports for the primary sink.
    pub fn name(self) -> &'static str {
        match self {
            RunSinkArg::Preview => "preview",
            RunSinkArg::Shim => "shim",
            RunSinkArg::Png => "png",
            RunSinkArg::Web => "web",
            RunSinkArg::None => "none",
        }
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
    /// Ignore existing snapshots and create a new world. Refused when `--state` already
    /// holds snapshots, an interrupted snapshot write, or a non-empty care journal.
    #[arg(long, default_value_t = false)]
    pub fresh: bool,
    /// Refuse to start unless an existing snapshot loads. Turns "no loadable snapshot,
    /// so here is a brand new world" into an error, for a run that must be a resume.
    #[arg(long, default_value_t = false)]
    pub require_resume: bool,
    /// Offer the optional care interaction (feed, rain, clean) on the viewer. Needs
    /// `--sink web` or `--mirror-web`: care is served over the same loopback HTTP server.
    #[arg(long, default_value_t = false)]
    pub care: bool,
    /// JSON-lines telemetry file (appended). Defaults to `<state>/telemetry.jsonl`.
    #[arg(long)]
    pub telemetry: Option<PathBuf>,
    /// JSON-lines field dump file (appended), written only when
    /// `capacity.field_dump_seconds > 0`. Defaults to `<state>/fields.jsonl`.
    #[arg(long)]
    pub fields: Option<PathBuf>,
    /// JSON-lines birth and death log (appended), written only when
    /// `capacity.event_log` is true. Defaults to `<state>/events.jsonl`.
    #[arg(long)]
    pub events: Option<PathBuf>,
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
    /// Render and output rate in frames per second; the simulation stays at 20 Hz.
    #[arg(long, default_value_t = crate::clock::RENDER_HZ)]
    pub fps: u32,
    /// Port for the `web` sink (a viewer page at http://127.0.0.1:<port>/).
    #[arg(long, default_value_t = 7393)]
    pub web_port: u16,
    /// Also serve the loopback viewer on `--web-port` while `--sink` keeps running. The
    /// same world, the same render, the same frame bytes: one encode reaches both.
    #[arg(long, default_value_t = false)]
    pub mirror_web: bool,
    /// Draw the world with the baked sprite art in this directory (`assets/atelier`)
    /// instead of the procedural bodies. Omit it and the image is unchanged.
    #[arg(long)]
    pub art: Option<PathBuf>,
}

/// `--fps` outside [`crate::clock::MIN_FPS`]..=[`crate::clock::MAX_FPS`] is a typo, not a
/// request: refuse it rather than silently clamping.
fn check_fps(fps: u32) -> anyhow::Result<()> {
    anyhow::ensure!(
        (crate::clock::MIN_FPS..=crate::clock::MAX_FPS).contains(&fps),
        "--fps must be between {} and {}",
        crate::clock::MIN_FPS,
        crate::clock::MAX_FPS
    );
    Ok(())
}

impl Run {
    /// The telemetry path after the documented default is applied.
    pub fn telemetry_path(&self) -> PathBuf {
        self.telemetry.clone().unwrap_or_else(|| self.state.join("telemetry.jsonl"))
    }

    /// The field dump path after the documented default is applied. Only consulted when
    /// the world's `capacity.field_dump_seconds` is nonzero.
    pub fn fields_path(&self) -> PathBuf {
        self.fields.clone().unwrap_or_else(|| self.state.join("fields.jsonl"))
    }

    /// The life event log path after the documented default is applied. Only consulted
    /// when the world's `capacity.event_log` is true.
    pub fn events_path(&self) -> PathBuf {
        self.events.clone().unwrap_or_else(|| self.state.join("events.jsonl"))
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
        if self.mirror_web {
            // Both refusals are typos, not requests: a headless run renders nothing to
            // mirror, and `--sink web` already *is* the viewer, so mirroring it would ask
            // for the same port twice.
            anyhow::ensure!(
                self.sink != RunSinkArg::None,
                "--mirror-web needs a rendering sink: with --sink none there is nothing to mirror"
            );
            anyhow::ensure!(
                self.sink != RunSinkArg::Web,
                "--mirror-web is redundant with --sink web, which is already the viewer"
            );
        }
        if self.care {
            // Care arrives over the viewer's own HTTP server; without one there is no
            // route to offer it on, and silently enabling nothing would be a lie.
            anyhow::ensure!(
                self.sink == RunSinkArg::Web || self.mirror_web,
                "--care needs the viewer: use --sink web, or --mirror-web beside another sink"
            );
        }
        anyhow::ensure!(
            !(self.fresh && self.require_resume),
            "--fresh and --require-resume ask for opposite things: one demands a new world, \
             the other demands an old one"
        );
        anyhow::ensure!(self.every >= 1, "--every must be at least 1");
        anyhow::ensure!(self.scale >= 1, "--scale must be at least 1");
        check_fps(self.fps)?;
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
        check_fps(self.fps)?;
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
        assert_eq!(d.fps, 60);
    }

    #[test]
    fn every_documented_option_parses() {
        let d = demo([
            "cubarium", "demo", "--scene", "vertex", "--sink", "shim", "--seconds", "20",
            "--seed", "9", "--addr", "10.0.0.4:1234", "--out", "/tmp/c", "--every", "5",
            "--scale", "2", "--fps", "24",
        ]);
        assert_eq!(d.scene, SceneArg::Vertex);
        assert_eq!(d.sink, SinkArg::Shim);
        assert_eq!(d.seconds, 20.0);
        assert_eq!(d.seed, 9);
        assert_eq!(d.addr, "10.0.0.4:1234");
        assert_eq!(d.out, PathBuf::from("/tmp/c"));
        assert_eq!(d.every, 5);
        assert_eq!(d.scale, 2);
        assert_eq!(d.fps, 24);
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
        assert_eq!(r.fields, None);
        assert_eq!(r.fields_path(), PathBuf::from("state/fields.jsonl"));
        assert_eq!(r.events, None);
        assert_eq!(r.events_path(), PathBuf::from("state/events.jsonl"));
        assert_eq!(r.addr, "127.0.0.1:7392");
        assert_eq!(r.out, PathBuf::from("captures"));
        assert_eq!(r.every, 30);
        assert_eq!(r.scale, 4);
        assert_eq!(r.fps, 60);
        assert_eq!(r.web_port, 7393);
        assert!(!r.mirror_web, "the mirrored viewer is opt-in");
        assert_eq!(r.art, None, "the art image is opt-in");
        assert!(!r.care, "care is opt-in");
        assert!(!r.require_resume, "a run may still create a new world by default");
        r.validate().unwrap();
    }

    #[test]
    fn care_needs_the_viewer_and_is_accepted_with_either_spelling_of_it() {
        parse_run(["cubarium", "run", "--sink", "web", "--care"]).validate().unwrap();
        parse_run(["cubarium", "run", "--sink", "shim", "--mirror-web", "--care"])
            .validate()
            .unwrap();
        for sink in ["shim", "preview"] {
            let r = parse_run(["cubarium", "run", "--sink", sink, "--care"]);
            let err = r.validate().unwrap_err().to_string();
            assert!(err.contains("--care needs the viewer"), "{sink}: {err}");
        }
        let r = parse_run(["cubarium", "run", "--sink", "none", "--speed", "0", "--care"]);
        assert!(r.validate().is_err(), "a headless run has no viewer to offer care on");
    }

    #[test]
    fn a_fresh_world_and_a_required_resume_cannot_be_asked_for_together() {
        let r = parse_run(["cubarium", "run", "--fresh", "--require-resume"]);
        let err = r.validate().unwrap_err().to_string();
        assert!(err.contains("opposite things"), "{err}");
        parse_run(["cubarium", "run", "--require-resume"]).validate().unwrap();
        parse_run(["cubarium", "run", "--fresh"]).validate().unwrap();
    }

    #[test]
    fn mirroring_the_viewer_is_allowed_beside_every_rendering_sink() {
        for sink in ["shim", "preview", "png"] {
            let r = parse_run([
                "cubarium", "run", "--sink", sink, "--mirror-web", "--web-port", "7393",
                "--seconds", "5",
            ]);
            assert!(r.mirror_web, "{sink}");
            assert_eq!(r.web_port, 7393);
            r.validate().unwrap_or_else(|e| panic!("--mirror-web with --sink {sink}: {e}"));
        }
    }

    #[test]
    fn mirroring_a_headless_run_is_refused() {
        let r = parse_run(["cubarium", "run", "--sink", "none", "--speed", "0", "--mirror-web"]);
        let err = r.validate().unwrap_err().to_string();
        assert!(err.contains("nothing to mirror"), "{err}");
    }

    #[test]
    fn mirroring_the_web_sink_onto_itself_is_refused() {
        let r = parse_run(["cubarium", "run", "--sink", "web", "--mirror-web"]);
        let err = r.validate().unwrap_err().to_string();
        assert!(err.contains("already the viewer"), "{err}");
        // …and `--sink web` on its own is untouched.
        parse_run(["cubarium", "run", "--sink", "web"]).validate().unwrap();
    }

    #[test]
    fn every_run_sink_has_a_status_name() {
        assert_eq!(RunSinkArg::Preview.name(), "preview");
        assert_eq!(RunSinkArg::Shim.name(), "shim");
        assert_eq!(RunSinkArg::Png.name(), "png");
        assert_eq!(RunSinkArg::Web.name(), "web");
        assert_eq!(RunSinkArg::None.name(), "none");
    }

    #[test]
    fn every_documented_run_option_parses() {
        let r = parse_run([
            "cubarium", "run", "--config", "w.toml", "--state", "/tmp/s", "--sink", "none",
            "--speed", "0", "--seconds", "600", "--seed", "7", "--fresh", "--telemetry",
            "/tmp/t.jsonl", "--fields", "/tmp/f.jsonl", "--events", "/tmp/e.jsonl",
            "--addr", "10.0.0.4:1", "--out", "/tmp/c", "--every", "5", "--scale", "2",
            "--fps", "120", "--art", "assets/atelier",
        ]);
        assert_eq!(r.config, Some(PathBuf::from("w.toml")));
        assert_eq!(r.state, PathBuf::from("/tmp/s"));
        assert_eq!(r.sink, RunSinkArg::None);
        assert_eq!(r.speed, 0.0);
        assert_eq!(r.seconds, 600.0);
        assert_eq!(r.seed, Some(7));
        assert!(r.fresh);
        assert_eq!(r.telemetry_path(), PathBuf::from("/tmp/t.jsonl"));
        assert_eq!(r.fields_path(), PathBuf::from("/tmp/f.jsonl"));
        assert_eq!(r.events_path(), PathBuf::from("/tmp/e.jsonl"));
        assert_eq!(r.addr, "10.0.0.4:1");
        assert_eq!(r.out, PathBuf::from("/tmp/c"));
        assert_eq!(r.every, 5);
        assert_eq!(r.scale, 2);
        assert_eq!(r.fps, 120);
        assert_eq!(r.art, Some(PathBuf::from("assets/atelier")));
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
    fn the_frame_rate_must_be_inside_the_documented_range() {
        for fps in ["0", "241", "10000"] {
            let err = parse_run(["cubarium", "run", "--fps", fps]).validate().unwrap_err();
            assert!(err.to_string().contains("--fps must be"), "{fps}: {err}");
            let err = demo(["cubarium", "demo", "--fps", fps]).validate().unwrap_err();
            assert!(err.to_string().contains("--fps must be"), "{fps}: {err}");
        }
        for fps in ["1", "30", "60", "144", "240"] {
            parse_run(["cubarium", "run", "--fps", fps]).validate().unwrap();
            demo(["cubarium", "demo", "--fps", fps]).validate().unwrap();
        }
    }

    #[test]
    fn scene_names_map_onto_the_fixtures() {
        assert_eq!(SceneKind::from(SceneArg::Body), SceneKind::Body);
        assert_eq!(SceneKind::from(SceneArg::Vertex), SceneKind::Vertex);
        assert_eq!(SceneKind::from(SceneArg::Patch), SceneKind::Patch);
        assert_eq!(SceneKind::from(SceneArg::All), SceneKind::All);
    }
}
