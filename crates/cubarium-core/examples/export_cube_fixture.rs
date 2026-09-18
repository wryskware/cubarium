//! Export one cube-world snapshot after a fixed-seed headless run.
//!
//! The `CubeProjection` comparator (`design/flat-world-plan-2026-09-16.md` §4) needs two
//! runs of the same world from two different builds: one from `main` before the topology
//! work, one from this build. This example is that exporter, and it is deliberately the
//! whole recipe — seed, tick count, build id — so the *same file* can be dropped into a
//! pre-change checkout and produce the pre-change half of the evidence.
//!
//! Usage: `cargo run --release --example export_cube_fixture -- <out-path> [ticks] [seed]`

use cubarium_core::{World, WorldConfig, snapshot::encode_snapshot};

/// Ticks the fixture run advances. Five simulated minutes at `TICK_HZ = 20`, so the weather
/// random walk has fired four times and the world has recorded births by the time it is written.
pub const FIXTURE_TICKS: u64 = 6000;
/// The fixed seed. Nothing else about the world is non-default.
pub const FIXTURE_SEED: u64 = 20_260_916;
/// The build id written into the header. Fixed, so the header bytes are reproducible.
pub const FIXTURE_BUILD_ID: &str = "cube-projection-fixture";

fn main() {
    let mut args = std::env::args().skip(1);
    let out = args
        .next()
        .expect("usage: export_cube_fixture <out-path> [ticks] [seed]");
    let ticks: u64 = args
        .next()
        .map_or(FIXTURE_TICKS, |s| s.parse().expect("ticks"));
    let seed: u64 = args
        .next()
        .map_or(FIXTURE_SEED, |s| s.parse().expect("seed"));

    let config = WorldConfig {
        seed,
        ..WorldConfig::default()
    };
    let mut world = World::new(config).expect("the default cube world is valid");
    for _ in 0..ticks {
        world.step();
    }
    let bytes = encode_snapshot(&world.state, FIXTURE_BUILD_ID);
    std::fs::write(&out, &bytes).expect("write the fixture");
    eprintln!(
        "wrote {out}: {} bytes, tick {}, {} organisms",
        bytes.len(),
        world.state.tick,
        world.state.organisms.len()
    );
}
