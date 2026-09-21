//! Greyscale maps of what erosion did to a preset's heightfield.
//!
//! ```text
//! cargo run -p cubarium-voxel --example erosion_map -- <preset> <seed> <out-dir>
//! ```
//!
//! Writes `bedrock.png`, `sediment.png`, `discharge.png` and `change.png`: one sample
//! column per pixel, magnified so a person can look at it, `x` across and depth down,
//! black at the low end of each map's own range and white at the high end. Discharge
//! gets a log ramp, spanning the whole ring in one number as it does. A development
//! tool, not a test and not a capture the repo keeps; the ranges and the budget's totals
//! are printed, so a picture is never read as a number.

use std::path::PathBuf;

use cubarium_voxel::generate::{Heightfield, heightfield};
use cubarium_voxel::{Config, Landform, PRESETS, Preset};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (name, seed, dir) = match args.as_slice() {
        [name, seed, dir] => (
            name.clone(),
            seed.parse::<u64>().expect("a seed is a number"),
            PathBuf::from(dir),
        ),
        _ => {
            let known: Vec<&str> = PRESETS.iter().map(|p| p.name).collect();
            eprintln!("usage: erosion_map <preset> <seed> <out-dir>    presets: {known:?}");
            std::process::exit(2);
        }
    };
    let preset = Preset::find(&name).unwrap_or_else(|| {
        eprintln!("no preset is called {name:?}");
        std::process::exit(2);
    });
    let config = Config {
        seed,
        ..preset.config()
    };
    let Landform::Staged(recipe) = config.landform.clone() else {
        unreachable!("a preset is staged")
    };

    let before = heightfield(&config, &recipe);
    let mut field = before.clone();
    let (circumference_m, cell) = (field.circumference_m, field.cell_m);
    let started = std::time::Instant::now();
    cubarium_voxel::erosion::erode(&mut field, &recipe.erosion, |x, z, bedrock_m| {
        recipe.hardness_at(
            (x as f64 + 0.5) * cell,
            bedrock_m,
            (z as f64 + 0.5) * cell,
            circumference_m,
            seed,
        )
    });
    let elapsed = started.elapsed();

    std::fs::create_dir_all(&dir).expect("the output directory");
    write(&dir, "bedrock", &field, &field.bedrock_m, Stretch::Linear);
    write(&dir, "sediment", &field, &field.sediment_m, Stretch::Linear);
    // Discharge spans the whole ring in one number: linear, every channel is one white
    // pixel in a black field.
    write(&dir, "discharge", &field, &field.discharge, Stretch::Log);
    let cut: Vec<f64> = (0..field.samples())
        .map(|i| before.surface_m(i) - field.surface_m(i))
        .collect();
    write(&dir, "change", &field, &cut, Stretch::Linear);

    let b = field.budget;
    println!(
        "{name} seed {seed}: {} x {} samples at {:.3} m, {} iterations in {:.1} ms",
        field.width,
        field.depth,
        field.cell_m,
        recipe.erosion.iterations,
        elapsed.as_secs_f64() * 1e3
    );
    println!(
        "  cut {:.2} m of bedrock and {:.2} m of sediment, deposited {:.2} m, {:.2} m in transport (imbalance {:.2e})",
        b.removed_bedrock_m,
        b.removed_sediment_m,
        b.deposited_m,
        b.in_transport_m,
        b.imbalance_m()
    );
    let caps = field.hard_cap.iter().filter(|c| **c).count();
    let bare = field.sediment_m.iter().filter(|s| **s < cell * 0.5).count();
    println!(
        "  {bare} of {} columns are bare rock, {caps} carry a hard cap",
        field.samples()
    );
    println!("  maps in {}", dir.display());
}

/// How a map's values are spread over the grey ramp.
#[derive(Clone, Copy)]
enum Stretch {
    Linear,
    /// For a quantity that spans the whole ring in one number.
    Log,
}

/// Whole-pixel magnification, so a `128 x 24` map is something a person can look at.
const ZOOM: usize = 4;

/// One greyscale PNG, stretched over the map's own range.
fn write(dir: &std::path::Path, name: &str, field: &Heightfield, values: &[f64], how: Stretch) {
    let (lo, hi) = values
        .iter()
        .fold((f64::MAX, f64::MIN), |(l, h), v| (l.min(*v), h.max(*v)));
    let shade = |v: f64| -> u8 {
        let t = match how {
            _ if hi <= lo => 0.0,
            Stretch::Linear => (v - lo) / (hi - lo),
            Stretch::Log => (1.0 + (v - lo)).ln() / (1.0 + (hi - lo)).ln(),
        };
        (t * 255.0).round().clamp(0.0, 255.0) as u8
    };
    let (w, d) = (field.width, field.depth);
    let mut pixels = vec![0u8; w * ZOOM * d * ZOOM];
    for z in 0..d * ZOOM {
        for x in 0..w * ZOOM {
            pixels[z * w * ZOOM + x] = shade(values[(z / ZOOM) * w + x / ZOOM]);
        }
    }
    let path = dir.join(format!("{name}.png"));
    let file = std::fs::File::create(&path).expect("the map file");
    let mut encoder = png::Encoder::new(
        std::io::BufWriter::new(file),
        (w * ZOOM) as u32,
        (d * ZOOM) as u32,
    );
    encoder.set_color(png::ColorType::Grayscale);
    encoder.set_depth(png::BitDepth::Eight);
    encoder
        .write_header()
        .expect("a header")
        .write_image_data(&pixels)
        .expect("the pixels");
    println!("  {name}: {lo:.3} .. {hi:.3}");
}
