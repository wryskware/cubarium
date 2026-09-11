//! The `cubarium demo` command contract from `crates/cubarium/README.md`.

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

    #[test]
    fn the_command_line_matches_the_documented_defaults() {
        Cli::command().debug_assert();
        let Command::Demo(d) = Cli::parse_from(["cubarium", "demo"]).command;
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
        let Command::Demo(d) = Cli::parse_from([
            "cubarium", "demo", "--scene", "vertex", "--sink", "shim", "--seconds", "20",
            "--seed", "9", "--addr", "10.0.0.4:1234", "--out", "/tmp/c", "--every", "5",
            "--scale", "2",
        ])
        .command;
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
        let Command::Demo(d) =
            Cli::parse_from(["cubarium", "demo", "--sink", "png"]).command;
        assert!(d.validate().is_err());
        let Command::Demo(d) =
            Cli::parse_from(["cubarium", "demo", "--sink", "png", "--seconds", "1"]).command;
        d.validate().unwrap();
    }

    #[test]
    fn scene_names_map_onto_the_fixtures() {
        assert_eq!(SceneKind::from(SceneArg::Body), SceneKind::Body);
        assert_eq!(SceneKind::from(SceneArg::Vertex), SceneKind::Vertex);
        assert_eq!(SceneKind::from(SceneArg::Patch), SceneKind::Patch);
        assert_eq!(SceneKind::from(SceneArg::All), SceneKind::All);
    }
}
