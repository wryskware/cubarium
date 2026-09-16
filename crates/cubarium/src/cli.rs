//! The `cubarium demo` and `cubarium run` command contracts from
//! `crates/cubarium/README.md`.

use std::path::PathBuf;

use clap::{Parser, Subcommand, ValueEnum};
use cubarium_surface::{Scale, Topology};

use crate::scene::SceneKind;

/// `--topology`: `cube`, or `ring:WxH`.
///
/// Spelled on the command line the way a panel is spelled — `ring:320x180` — rather than
/// as the config file's `topology = { Ring = { w = 320, h = 180 } }`, which is serde's
/// shape for the same value. Both reach the same [`Topology`]; the config file is the
/// place to write one down, and this flag is the place to try one.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TopologyArg(pub Topology);

impl std::str::FromStr for TopologyArg {
    type Err = String;

    fn from_str(text: &str) -> Result<TopologyArg, String> {
        let text = text.trim();
        if text.eq_ignore_ascii_case("cube") {
            return Ok(TopologyArg(Topology::Cube));
        }
        let rest = text
            .strip_prefix("ring:")
            .or_else(|| text.strip_prefix("Ring:"))
            .ok_or_else(|| {
                format!("`{text}` is not a topology: write `cube` or `ring:WxH`, e.g. ring:320x180")
            })?;
        let (w, h) = rest
            .split_once(['x', 'X'])
            .ok_or_else(|| format!("`{text}`: a ring is spelled ring:WxH, e.g. ring:320x180"))?;
        let parse = |s: &str, which: &str| -> Result<u16, String> {
            s.trim()
                .parse::<u16>()
                .map_err(|_| format!("`{text}`: the ring's {which} must be 1..=65535"))
        };
        Ok(TopologyArg(Topology::Ring { w: parse(w, "width")?, h: parse(h, "height")? }))
    }
}

impl std::fmt::Display for TopologyArg {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.0 {
            Topology::Cube => write!(f, "cube"),
            Topology::Ring { w, h } => write!(f, "ring:{w}x{h}"),
        }
    }
}

/// The word `/status`, the resume refusal and every message use for a topology: `cube` or
/// `ring`, never the Rust spelling of the variant.
pub fn topology_name(topo: Topology) -> &'static str {
    match topo {
        Topology::Cube => "cube",
        Topology::Ring { .. } => "ring",
    }
}

#[derive(Parser, Debug)]
#[command(name = "cubarium", about = "Cubarium host: clock, fixture scenes, output sinks")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

// One parsed command line exists once per process and is moved once, so the 208-byte
// difference between the two variants costs nothing worth an indirection — and the
// indirection clippy suggests is not available here: `#[derive(Subcommand)]` requires each
// variant's field to implement `clap::Args`, which `Box<Run>` does not.
#[allow(clippy::large_enum_variant)]
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
    /// The surface the fixtures are drawn on: `cube`, or `ring:WxH` (e.g. `ring:320x180`).
    #[arg(long, default_value_t = TopologyArg(Topology::Cube))]
    pub topology: TopologyArg,
    /// The world scale `S`. Ring only; a cube is pinned to 1.
    #[arg(long, default_value_t = 1.0)]
    pub world_scale: f64,
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

/// How `--sink gpu` samples a sprite.
#[derive(ValueEnum, Clone, Copy, Debug, PartialEq, Eq)]
pub enum GpuFilterArg {
    /// One source texel per `S x S` block, nothing filtered.
    Nearest,
    /// `cubarium_render::Sprite::sample`'s four taps at the un-snapped anchor: the CPU
    /// presenter's stamp, for a like-for-like comparison.
    Bilinear,
}

/// `run` adds a headless sink to the three `demo` sinks.
#[derive(ValueEnum, Clone, Copy, Debug, PartialEq, Eq)]
pub enum RunSinkArg {
    Preview,
    Shim,
    Png,
    /// Local HTTP viewer mapping the frame onto a rotatable cube.
    Web,
    /// The GPU renderer: the ring drawn by `cubarium-gpu` instead of by the canvas, onto
    /// the panel through the display daemon's socket or into a development window.
    Gpu,
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
            RunSinkArg::Gpu => "gpu",
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
    /// Where `--sink gpu` puts its frames. Omitted, it is `shim` where the display
    /// daemon's frame socket exists and `window` where it does not.
    #[arg(long)]
    pub gpu_target: Option<crate::sink::GpuTargetKind>,
    /// How `--sink gpu` samples a sprite. `nearest` is the pixel-art rule: one source
    /// texel on an exact S x S block. `bilinear` is the CPU presenter's own sampler at
    /// its own sub-pixel anchor, for comparing the two renderers with nothing but the
    /// sampler between them.
    #[arg(long, value_enum, default_value_t = GpuFilterArg::Nearest)]
    pub gpu_filter: GpuFilterArg,
    /// How many raster pixels one authored source texel covers in `--sink gpu`.
    ///
    /// The default, 1, is what the CPU presenter does at every `--world-scale`: `S`
    /// scales the cell grid and leaves the art the size it was authored. The ring-world
    /// plan §6 instead has `S` multiply the sprite tile too, which is `--gpu-art-scale`
    /// equal to the world scale. The two have not been reconciled; this shows both.
    #[arg(long)]
    pub gpu_art_scale: Option<f32>,
    /// Let the wind's displacement land between source texels instead of rounding it to
    /// a whole one.
    ///
    /// **On by default on a ring at `--world-scale` 2 and above**, where Wrysk asked for
    /// a smoother sway than whole-texel steps, and where the bend budgets a ring's
    /// topology admits are wide enough for a whole texel to be a jump. At `S = 1` the two
    /// are the same picture. `--no-gpu-bend-substep` turns it off; naming either flag
    /// overrides the default.
    #[arg(long, default_value_t = false, conflicts_with = "no_gpu_bend_substep")]
    pub gpu_bend_substep: bool,
    /// Round the wind's displacement to a whole source texel, whatever the world scale:
    /// the `S x S` block rule applied to the bend as well. See `--gpu-bend-substep`.
    #[arg(long, default_value_t = false)]
    pub no_gpu_bend_substep: bool,
    /// Measure where `--sink gpu`'s sprite fill goes — quad area against the art that
    /// can actually paint in it — and print it at exit. A diagnostic: it walks the
    /// frame's instances on the CPU, which is the adapter's own scarce resource.
    #[arg(long, default_value_t = false)]
    pub gpu_fill_profile: bool,
    /// Write a PNG of every `--sink gpu` frame into this directory. Captures and the
    /// fidelity comparison; not for a run anyone is watching.
    #[arg(long)]
    pub gpu_capture: Option<PathBuf>,
    /// Seed a **new** world with trained neural animals running this exported policy file
    /// (`cubarium-search es-export`). A seeding control, so it applies only when this run
    /// creates the world: on a resume it is refused rather than seeding a second cohort
    /// into a world that already has one.
    #[arg(long)]
    pub neural: Option<PathBuf>,
    /// How many copies of the training animal `--neural` seeds. Must be at least 1.
    #[arg(long, default_value_t = 4)]
    pub neural_count: usize,
    /// The surface a **new** world is created on: `cube`, or `ring:WxH` (e.g.
    /// `ring:320x180`). Overrides the config file's `topology`. A resume keeps the
    /// snapshot's own topology and refuses a different one by name, like every other
    /// schema refusal — a world's shape is not an operational override.
    #[arg(long)]
    pub topology: Option<TopologyArg>,
    /// The world scale `S` of a **new** world: every length in pixels is multiplied by it,
    /// so `ring:640x360 --world-scale 2` is the same 3,600-cell world as `ring:320x180` at
    /// twice the resolution. Ring only; a cube is pinned to 1. Overrides the config file's
    /// `world_scale`.
    #[arg(long)]
    pub world_scale: Option<f64>,
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

/// `--sink preview` cannot show a ring: both halves of the window are cube pictures.
/// Refused here, at argument validation, as well as at the sink's own construction — this
/// catches the spelling the operator actually typed, and names it.
fn check_preview_topology(topology: Option<Topology>, preview: bool) -> anyhow::Result<()> {
    if let (true, Some(topo @ Topology::Ring { .. })) = (preview, topology) {
        let (w, h) = match topo {
            Topology::Ring { w, h } => (w, h),
            Topology::Cube => unreachable!(),
        };
        anyhow::bail!(
            "--sink preview cannot show a ring world ({w}x{h}): the preview window is the \
             unfolded cube net beside a ray-cast cube, neither of which means anything on a \
             flat world. Use --sink web for the loopback viewer, or --sink png to capture \
             the raster."
        );
    }
    Ok(())
}

/// `--world-scale` must be a positive finite number, and a cube is pinned to 1 — the
/// cube's 32-pixel local radius and 9-pixel stamp budget are completeness proofs, not
/// tunables. `Topology::validate` refuses the pair again when the world is built; this is
/// the early, named refusal for the value the operator typed.
fn check_world_scale(topology: Option<Topology>, world_scale: Option<f64>) -> anyhow::Result<()> {
    let Some(s) = world_scale else { return Ok(()) };
    anyhow::ensure!(
        s.is_finite() && s > 0.0,
        "--world-scale must be a positive finite number, not {s}"
    );
    if matches!(topology, Some(Topology::Cube) | None) && s != 1.0 {
        anyhow::bail!(
            "--world-scale {s} needs a ring: a cube world is pinned to scale 1, because its \
             32-pixel local radius and 9-pixel stamp budget are proofs about a 64-pixel \
             chart. Add --topology ring:WxH, or drop --world-scale."
        );
    }
    Ok(())
}

impl Run {
    /// What the two bend-substep flags say, or `None` for "the sink's own default for
    /// this world" (`GpuSink::substep_default`). Naming neither leaves the choice where
    /// it belongs: with the world's scale, which the command line does not always know.
    pub fn bend_substep(&self) -> Option<bool> {
        match (self.gpu_bend_substep, self.no_gpu_bend_substep) {
            (true, _) => Some(true),
            (_, true) => Some(false),
            _ => None,
        }
    }

    /// The topology this run asks a **new** world to have, if it named one.
    pub fn topology(&self) -> Option<Topology> {
        self.topology.map(|t| t.0)
    }

    /// The scale this run asks a **new** world to have, if it named one.
    pub fn scale(&self) -> Option<Scale> {
        self.world_scale.map(Scale::new)
    }

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
        // A count of zero asks for a seeding that seeds nothing; that is a typo, not a
        // request. `--neural-count` without `--neural` is harmless and stays accepted.
        anyhow::ensure!(self.neural_count >= 1, "--neural-count must be at least 1");
        check_fps(self.fps)?;
        // A world's shape belongs to the world, and `--fresh` is the only moment a run
        // chooses one. Refused rather than ignored on a resume: silently dropping it would
        // leave the operator believing they had asked for a ring and got one. Checked
        // before the two shape checks below, because "you cannot ask for a shape here at
        // all" is the more useful thing to be told than which shape is wrong.
        if (self.topology.is_some() || self.world_scale.is_some()) && self.require_resume {
            anyhow::bail!(
                "--topology and --world-scale describe a world to create, and \
                 --require-resume demands an existing one. A resumed world keeps the shape \
                 its snapshot records."
            );
        }
        check_preview_topology(self.topology(), self.sink == RunSinkArg::Preview)?;
        check_world_scale(self.topology(), self.world_scale)?;
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
        check_preview_topology(Some(self.topology.0), self.sink == SinkArg::Preview)?;
        check_world_scale(Some(self.topology.0), Some(self.world_scale))?;
        // FW-4 refused `--scene patch` and `--scene all` on a ring here, because
        // `Scenes::new` built the patch scene's `ScalarField` at the cube's 1,280 cells
        // while the substrate pass read the cells of the canvas it drew on, and on a ring
        // those disagree and the pass indexes past the field. FW-5 built all four
        // fixtures from a `(topology, scale)` (`Scenes::on`) and `run_demo` now names the
        // demo's own shape, so there is nothing left to refuse: every scene draws on
        // every surface the contract accepts.
        self.topology
            .0
            .validate(Scale::new(self.world_scale))
            .map_err(|e| anyhow::anyhow!("--topology {}: {e}", self.topology))?;
        Ok(())
    }

    /// The surface the fixtures draw on.
    pub fn shape(&self) -> (Topology, Scale) {
        (self.topology.0, Scale::new(self.world_scale))
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

    /// `--neural` is the display's seeding door. The parser takes it on its own — the
    /// runtime is what refuses it on a resume — and a count of zero is a typo.
    #[test]
    fn the_neural_seeding_controls_parse_and_refuse_a_zero_count() {
        let r = parse_run(["cubarium", "run"]);
        assert_eq!(r.neural, None, "no seeding unless it is asked for");
        assert_eq!(r.neural_count, 4, "the documented default cohort");

        let r = parse_run([
            "cubarium", "run", "--fresh", "--neural", "/tmp/center-00059.json",
            "--neural-count", "7",
        ]);
        assert_eq!(r.neural, Some(PathBuf::from("/tmp/center-00059.json")));
        assert_eq!(r.neural_count, 7);
        r.validate().expect("a seeded fresh run is well formed");

        // Accepted by the parser without `--fresh`: the refusal belongs to the runtime,
        // which is the only thing that knows whether this run created the world.
        let r = parse_run(["cubarium", "run", "--neural", "/tmp/center-00059.json"]);
        assert_eq!(r.neural, Some(PathBuf::from("/tmp/center-00059.json")));
        assert!(!r.fresh);
        r.validate().expect("the parser does not decide fresh-versus-resume");

        let err = parse_run([
            "cubarium", "run", "--fresh", "--neural", "/tmp/p.json", "--neural-count", "0",
        ])
        .validate()
        .expect_err("a cohort of zero seeds nothing");
        assert!(format!("{err}").contains("--neural-count must be at least 1"), "{err}");
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

#[cfg(test)]
mod topology_tests {
    use super::*;
    use clap::Parser;
    use std::str::FromStr;

    fn parse_run<const N: usize>(args: [&str; N]) -> Run {
        match Cli::parse_from(args).command {
            Command::Run(r) => r,
            other => panic!("expected a run command, got {other:?}"),
        }
    }

    fn parse_demo<const N: usize>(args: [&str; N]) -> Demo {
        match Cli::parse_from(args).command {
            Command::Demo(d) => d,
            other => panic!("expected a demo command, got {other:?}"),
        }
    }

    /// `ring:WxH` on the command line, `{ Ring = { w, h } }` in the config file: two
    /// spellings of one value, and the flag's own spelling round-trips through `Display`
    /// so a refusal can quote back what was typed.
    #[test]
    fn the_topology_flag_spells_a_ring_as_the_panel_is_spelled() {
        assert_eq!(TopologyArg::from_str("cube").unwrap().0, Topology::Cube);
        assert_eq!(TopologyArg::from_str("CUBE").unwrap().0, Topology::Cube);
        assert_eq!(
            TopologyArg::from_str("ring:320x180").unwrap().0,
            Topology::Ring { w: 320, h: 180 }
        );
        assert_eq!(
            TopologyArg::from_str(" ring:1920X1080 ").unwrap().0,
            Topology::Ring { w: 1920, h: 1080 }
        );
        for bad in ["", "ring", "ring:320", "ring:320x", "ring:0x180 extra", "sphere:1x1",
                    "ring:65536x180", "ring:-1x180", "ring:320x180x2"] {
            assert!(TopologyArg::from_str(bad).is_err(), "`{bad}` must not parse");
        }
        for text in ["cube", "ring:320x180", "ring:640x360"] {
            assert_eq!(TopologyArg::from_str(text).unwrap().to_string(), text);
        }
    }

    /// The flags reach `Run`, and a run that names neither leaves the world's shape to the
    /// config file exactly as before.
    #[test]
    fn a_fresh_run_takes_the_topology_and_the_scale_from_the_command_line() {
        let r = parse_run(["cubarium", "run"]);
        assert_eq!(r.topology(), None, "no --topology means the config file decides");
        assert_eq!(r.scale(), None);

        let r = parse_run([
            "cubarium", "run", "--fresh", "--sink", "none", "--speed", "0",
            "--topology", "ring:640x360", "--world-scale", "2",
        ]);
        assert_eq!(r.topology(), Some(Topology::Ring { w: 640, h: 360 }));
        assert_eq!(r.scale(), Some(Scale::new(2.0)));
        r.validate().expect("a fresh ring at S = 2 is well formed");
    }

    /// The preview window is two pictures of a cube. Refused at argument validation as
    /// well as at the sink's own construction, so the operator is told about the spelling
    /// they typed rather than about a sink they did not name.
    #[test]
    fn the_preview_window_is_refused_for_a_ring_on_both_commands() {
        let err = parse_run([
            "cubarium", "run", "--fresh", "--sink", "preview", "--topology", "ring:320x180",
        ])
        .validate()
        .unwrap_err()
        .to_string();
        assert!(err.contains("cannot show a ring world (320x180)"), "{err}");
        assert!(err.contains("--sink web"), "the refusal must name what to use instead: {err}");
        assert!(err.contains("--sink png"), "{err}");

        let err = parse_demo(["cubarium", "demo", "--topology", "ring:320x180"])
            .validate()
            .unwrap_err()
            .to_string();
        assert!(err.contains("cannot show a ring world"), "preview is the demo default: {err}");

        // And a cube preview is untouched on both.
        parse_run(["cubarium", "run", "--sink", "preview"]).validate().unwrap();
        parse_demo(["cubarium", "demo"]).validate().unwrap();
    }

    /// The cube is pinned to `S = 1`: its 32-pixel local radius and 9-pixel stamp budget
    /// are completeness proofs about a 64-pixel chart, not tunables.
    #[test]
    fn a_scale_other_than_one_needs_a_ring() {
        let err = parse_run(["cubarium", "run", "--fresh", "--world-scale", "2"])
            .validate()
            .unwrap_err()
            .to_string();
        assert!(err.contains("needs a ring"), "{err}");
        assert!(err.contains("pinned to scale 1"), "{err}");

        let err = parse_run([
            "cubarium", "run", "--fresh", "--topology", "cube", "--world-scale", "1.5",
        ])
        .validate()
        .unwrap_err()
        .to_string();
        assert!(err.contains("needs a ring"), "{err}");

        // Spelled with `=`, as the contract's other negative values are: bare `-1` reads
        // as a flag.
        for bad in ["--world-scale=0", "--world-scale=-1", "--world-scale=nan",
                    "--world-scale=inf"] {
            let r = parse_run([
                "cubarium", "run", "--fresh", "--sink", "none", "--speed", "0",
                "--topology", "ring:320x180", bad,
            ]);
            assert!(r.validate().is_err(), "{bad}");
        }
        // The documented pair does parse and validate. `--sink none --speed 0` because
        // `run`'s default sink is the preview window, which a ring is refused from.
        parse_run([
            "cubarium", "run", "--fresh", "--sink", "none", "--speed", "0",
            "--topology", "ring:320x180", "--world-scale", "1",
        ])
        .validate()
        .unwrap();
    }

    /// A world's shape is chosen when it is created. Asking for one *and* demanding a
    /// resume asks for two different things, so it is refused rather than one of them
    /// being quietly dropped.
    #[test]
    fn a_shape_cannot_be_asked_for_alongside_a_required_resume() {
        let err = parse_run([
            "cubarium", "run", "--require-resume", "--sink", "none", "--speed", "0",
            "--topology", "ring:320x180",
        ])
        .validate()
        .unwrap_err()
        .to_string();
        assert!(err.contains("--require-resume"), "{err}");
        let err = parse_run(["cubarium", "run", "--require-resume", "--world-scale", "2"])
            .validate()
            .unwrap_err()
            .to_string();
        assert!(err.contains("--require-resume"), "{err}");
    }

    /// FW-4's guard is lifted: FW-5 built every M1 fixture from a `(topology, scale)`,
    /// so all four scenes are accepted on a ring as they always were on a cube. The
    /// fixtures themselves are exercised by `scene.rs`'s own ring tests; what this pins
    /// is that the command line no longer refuses them, and at both ladder rungs.
    #[test]
    fn every_demo_fixture_is_accepted_on_a_ring() {
        for scene in ["body", "vertex", "patch", "all"] {
            for (ring, s) in [("ring:320x180", "1"), ("ring:640x360", "2")] {
                parse_demo([
                    "cubarium", "demo", "--sink", "png", "--seconds", "1", "--scene", scene,
                    "--topology", ring, "--world-scale", s,
                ])
                .validate()
                .unwrap_or_else(|e| panic!("{scene} on {ring} S={s}: {e}"));
            }
        }
        // And every scene is still fine on a cube.
        for scene in ["body", "vertex", "patch", "all"] {
            parse_demo(["cubarium", "demo", "--sink", "png", "--seconds", "1", "--scene", scene])
                .validate()
                .unwrap();
        }
    }

    /// A ring the surface contract cannot accept is refused with the contract's own
    /// message rather than panicking when the canvas is built.
    #[test]
    fn a_ring_the_geometry_refuses_is_refused_on_the_command_line() {
        // Not a whole number of 4-pixel cells.
        let err = parse_demo(["cubarium", "demo", "--topology", "ring:321x180", "--sink", "png",
                              "--seconds", "1", "--scene", "body"])
            .validate()
            .unwrap_err()
            .to_string();
        assert!(err.contains("--topology ring:321x180"), "{err}");
    }
}
