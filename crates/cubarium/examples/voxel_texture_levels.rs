//! `voxel_texture_levels`: derive every level of the GPU renderer's face textures from
//! their 48 px masters (`cubarium::voxel::textures`).
//!
//! ```text
//! cargo run --release -p cubarium --example voxel_texture_levels -- \
//!     [--root assets/voxel-textures] [--tilt 30] [--levels 4,6,8,12,16,24]
//! ```
//!
//! Writes `<root>/lod/<px>/<stem>-<variant>.png`: side faces `px × px`, top faces
//! `px × rise` at this tilt. A hand-fixed level goes in `<root>/override/<px>/` under the
//! same name and wins at load; this tool never touches `override/`. A level the renderer
//! is asked for that is not here (`px_per_voxel = "auto"` can pick any) is derived the
//! same way at start-up.

use std::path::PathBuf;

use anyhow::{Context, Result, bail};
use cubarium::voxel::textures::{STANDARD_LEVELS, write_levels};

fn main() -> Result<()> {
    let mut root = PathBuf::from("assets/voxel-textures");
    let mut tilt = 30.0;
    let mut levels = STANDARD_LEVELS.to_vec();
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        match a.as_str() {
            "--root" => root = PathBuf::from(args.next().context("--root DIR")?),
            "--tilt" => tilt = args.next().context("--tilt DEGREES")?.parse()?,
            "--levels" => {
                levels = args
                    .next()
                    .context("--levels 4,6,…")?
                    .split(',')
                    .map(|s| s.trim().parse::<u32>())
                    .collect::<Result<_, _>>()?
            }
            _ => bail!("unexpected argument {a}"),
        }
    }
    let n = write_levels(&root, &levels, tilt)?;
    println!(
        "{n} face images at levels {levels:?} (tilt {tilt}°) -> {}/lod",
        root.display()
    );
    Ok(())
}
