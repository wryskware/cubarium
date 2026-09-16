//! Workstream Z's opening frames: what a **coupled grazed** opening looks like, beside the
//! status quo and the plant-only one, through the **real** presenter with the shipped assets
//! at the real 64 × 64.
//!
//! This file lives under `design/7_Research/assets/` because workstream Z owns that directory
//! and does not own `crates/cubarium/`, which it may read but must not change. To run it, copy
//! it into `crates/cubarium/examples/` and remove it again afterwards, leaving that crate
//! exactly as it was:
//!
//! ```text
//! cp design/7_Research/assets/ecology-v1-grazed-opening-frames.rs \
//!    crates/cubarium/examples/grazed_opening_frames.rs
//! CARGO_TARGET_DIR=.../target cargo run --release -p cubarium --example grazed_opening_frames -- \
//!    --configs runs/ecology-v1-grazed-opening/configs \
//!    --seed 1001 --age 48000 \
//!    --out design/7_Research/assets/ecology-v1-grazed-opening-2026-09-16.png
//! rm crates/cubarium/examples/grazed_opening_frames.rs
//! ```
//!
//! Three columns — status quo, plant-only 48,000, coupled-grazed 48,000. Four rows: each
//! configuration at the founding and one simulated hour (72,000 ticks) later. Every panel is
//! the **Front** face of one real world, drawn by [`ArtPresenter`] from the world's own
//! `RenderView` with `assets/atelier`, then magnified with no filtering. Nothing is an overlay
//! and nothing is synthetic.
//!
//! Each opening is the procedure the campaign used, written out here against `cubarium-core`
//! alone:
//!
//! * **status quo** — `World::new` and nothing else;
//! * **plant-only** — roster emptied, stepped to the age with nothing eating, roster written
//!   back, `World::found_roster` (workstream S's operator);
//! * **coupled-grazed** — the *ordinary* world stepped to the age with its roster in it,
//!   `World::remove_all_animals`, then `World::found_roster` (workstream Z's).

use std::path::PathBuf;

use cubarium::art::ArtPack;
use cubarium::art_present::ArtPresenter;
use cubarium_core::{World, WorldConfig};
use cubarium_render::Canvas;
use cube_proto::{FACE_SIZE, Face, Frame};

/// One simulated hour at 20 ticks per second.
const HOUR: u64 = 72_000;
/// Nearest-neighbour magnification of each 64 × 64 face.
const SCALE: usize = 3;
/// Pixels of gutter between panels, and the gutter colour.
const GUTTER: usize = 4;
const INK: [u8; 3] = [18, 18, 24];

struct Image {
    w: usize,
    h: usize,
    rgb: Vec<u8>,
}

impl Image {
    fn filled(w: usize, h: usize, c: [u8; 3]) -> Image {
        Image { w, h, rgb: c.iter().cycle().take(w * h * 3).copied().collect() }
    }
    fn set(&mut self, x: usize, y: usize, c: [u8; 3]) {
        if x < self.w && y < self.h {
            let o = (y * self.w + x) * 3;
            self.rgb[o..o + 3].copy_from_slice(&c);
        }
    }
    fn blit(&mut self, other: &Image, x0: usize, y0: usize) {
        for y in 0..other.h {
            for x in 0..other.w {
                let o = (y * other.w + x) * 3;
                self.set(x0 + x, y0 + y, [other.rgb[o], other.rgb[o + 1], other.rgb[o + 2]]);
            }
        }
    }
}

fn face_image(frame: &Frame, face: Face, scale: usize) -> Image {
    let n = FACE_SIZE;
    let mut out = Image::filled(n * scale, n * scale, [0, 0, 0]);
    for y in 0..n {
        for x in 0..n {
            let c = frame.get(face, x, y);
            for dy in 0..scale {
                for dx in 0..scale {
                    out.set(x * scale + dx, y * scale + dy, c);
                }
            }
        }
    }
    out
}

fn write_png(path: &PathBuf, img: &Image) {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).expect("the sheet's directory");
    }
    let file = std::fs::File::create(path).expect("creating the sheet");
    let mut enc = png::Encoder::new(std::io::BufWriter::new(file), img.w as u32, img.h as u32);
    enc.set_color(png::ColorType::Rgb);
    enc.set_depth(png::BitDepth::Eight);
    enc.write_header().expect("PNG header").write_image_data(&img.rgb).expect("PNG data");
}

fn pack() -> ArtPack {
    ArtPack::load(&PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets/atelier"))
        .expect("the baked pack at assets/atelier must load")
}

/// One still of a world's Front face: a fresh presenter, snapped to this view, drawn at `f = 0`
/// — the same way `shoulder_sheet::shot` takes a panel.
fn shot(world: &World) -> Image {
    let view = world.render_view();
    let mut p = ArtPresenter::new(pack());
    p.observe(&view);
    let mut canvas = Canvas::new();
    p.draw(&view, 0.0, &mut canvas);
    let mut frame = Frame::black();
    canvas.encode(&mut frame);
    face_image(&frame, Face::Front, SCALE)
}

fn arg(name: &str) -> Option<String> {
    let mut it = std::env::args();
    while let Some(a) = it.next() {
        if a == name {
            return it.next();
        }
    }
    None
}

fn step(world: &mut World, ticks: u64) {
    for _ in 0..ticks {
        world.step();
        world.drain_events();
        world.drain_hunter_events();
        world.drain_apex_dormancy_events();
        world.drain_apex_encounter_events();
        world.drain_quiet_events();
    }
}

/// The three openings, each built the way its campaign builds it.
fn opening(base: &WorldConfig, seed: u64, which: &str, age: u64) -> World {
    let config = WorldConfig { seed, ..base.clone() };
    match which {
        "status quo" => World::new(config).expect("the shipped defaults build a world"),
        "plant-only" => {
            let roster = config.founders.clone();
            let mut bare = config;
            bare.founders.kinds.clear();
            bare.founders.count = 0;
            let mut world = World::new(bare).expect("a world with no founders");
            step(&mut world, age);
            world.state.config.founders = roster;
            world.found_roster().expect("the roster is founded into the grown field");
            world
        }
        "coupled-grazed" => {
            let mut world = World::new(config).expect("the shipped defaults build a world");
            step(&mut world, age);
            world.remove_all_animals().expect("the burn-in population is removed");
            world.found_roster().expect("the identical fresh roster is founded");
            world
        }
        other => panic!("unknown opening {other}"),
    }
}

fn main() {
    let configs = PathBuf::from(arg("--configs").expect("--configs DIR"));
    let seed: u64 = arg("--seed").map_or(1_001, |s| s.parse().expect("--seed"));
    let age: u64 = arg("--age").map_or(48_000, |s| s.parse().expect("--age"));
    let out = PathBuf::from(arg("--out").expect("--out PATH.png"));
    let names = ["baseline", "fast-leaf"];
    let openings = ["status quo", "plant-only", "coupled-grazed"];

    let cols = openings.len();
    let rows = names.len() * 2;
    let cell = FACE_SIZE * SCALE;
    let mut sheet = Image::filled(
        cols * cell + (cols + 1) * GUTTER,
        rows * cell + (rows + 1) * GUTTER,
        INK,
    );

    println!("| config | opening | moment | Σ P | Σ W | population |");
    println!("| --- | --- | --- | ---: | ---: | ---: |");
    for (n, name) in names.iter().enumerate() {
        let text = std::fs::read_to_string(configs.join(format!("{name}.toml")))
            .expect("the exported candidate config");
        let base: WorldConfig = toml::from_str(&text).expect("a candidate config parses");
        for (c, which) in openings.iter().enumerate() {
            let mut world = opening(&base, seed, which, age);
            let at_founding = shot(&world);
            println!(
                "| {name} | {which} | founding | {:.1} | {:.1} | {} |",
                world.state.fields.p.iter().sum::<f64>(),
                world.state.ecology.wood.iter().sum::<f64>(),
                world.population()
            );
            step(&mut world, HOUR);
            let an_hour_later = shot(&world);
            println!(
                "| {name} | {which} | +1 h | {:.1} | {:.1} | {} |",
                world.state.fields.p.iter().sum::<f64>(),
                world.state.ecology.wood.iter().sum::<f64>(),
                world.population()
            );

            let x = GUTTER + c * (cell + GUTTER);
            sheet.blit(&at_founding, x, GUTTER + (n * 2) * (cell + GUTTER));
            sheet.blit(&an_hour_later, x, GUTTER + (n * 2 + 1) * (cell + GUTTER));
        }
    }
    write_png(&out, &sheet);
    println!("\nwrote {}", out.display());
    println!(
        "rows, top to bottom: baseline at founding, baseline +1 h, fast-leaf at founding, \
         fast-leaf +1 h. Columns: status quo, plant-only {age}, coupled-grazed {age}; \
         seed {seed}, Front face."
    );
}
