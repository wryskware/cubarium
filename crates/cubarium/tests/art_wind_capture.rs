//! Review captures for animation slice 2 (wind, rooted bend, growth pilot). Not
//! correctness tests: `#[ignore]`d fixtures that write native 64×64 net PNGs for a human
//! to look at. Run with
//! `cargo test --release -p cubarium --test art_wind_capture -- --ignored --nocapture`
//! and look in `$CUBARIUM_CAPTURE_DIR` (default `/tmp/cubarium-wind`).
//!
//! The sparse scene: one full-grown plant of every side species on Front, a column with a
//! vine grown to the rim, a reed in a pool, both canopy species on Top, and one lanternstalk
//! that is fed at 6 s (so its authored sprout → stalk growth plays inside the first gust)
//! and starved at 30 s. After 600 warm-up ticks, the capture runs 36 simulated seconds
//! from tick 601 (presentation time 30 s): one packet's rise, hold and fall (capture
//! seconds 0–18), its quiet interval (18–30), then the next packet's rise. Spatial delays
//! shift those boundaries slightly at each root.

use std::path::PathBuf;

use cube_proto::Frame;
use cubarium::art::{ArtPack, Band};
use cubarium::art_present::{
    ArtPresenter, FRUIT_SHOW, SOIL_SCALE, TALL_PLANTS, TallColumn, band_of, plant_cap,
    species_of, tall_columns,
};
use cubarium::net::net_rgb8;
use cubarium::present::PRODUCER_SATURATION;
use cubarium::sink::png::write_net_png;
use cubarium_core::view::RenderView;
use cubarium_render::Canvas;
use cubarium_surface::{CELL_COUNT, CellId, Face};

const PRODUCER_MAX: f64 = 10.0;
const FRAMES_PER_TICK: u64 = 3;

fn pack() -> ArtPack {
    ArtPack::load(&PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets/atelier"))
        .expect("the baked pack")
}

fn saturation() -> f64 {
    PRODUCER_MAX * PRODUCER_SATURATION
}

fn bare_view(tick: u64) -> RenderView {
    RenderView {
        tick,
        producer: vec![0.0; CELL_COUNT],
        detritus: vec![0.0; CELL_COUNT],
        fruit: vec![0.0; CELL_COUNT],
        water: vec![0.0; CELL_COUNT],
        rain: vec![0.0; CELL_COUNT],
        producer_max: PRODUCER_MAX,
        organisms: Vec::new(),
    }
}

/// The first rank-2 slot of `species` on `face`, away from the face edges.
fn slot_for(face: Face, species: &str, band: Band) -> CellId {
    CellId::all()
        .find(|&c| {
            c.face() == face
                && band_of(c) == band
                && plant_cap(band, c) == Some(2)
                && species_of(band, c) == species
                && (2..=13).contains(&c.cx())
                && (2..=13).contains(&c.cy())
        })
        .unwrap_or_else(|| panic!("a rank-2 {species} slot on {face:?}"))
}

fn a_column(face: Face, vine: bool) -> TallColumn {
    tall_columns()
        .into_iter()
        .find(|c| c.face == face && c.vine == vine)
        .expect("a column")
}

#[test]
#[ignore = "review capture, not behaviour"]
fn wind_and_growth_capture() {
    let dir = PathBuf::from(
        std::env::var("CUBARIUM_CAPTURE_DIR").unwrap_or_else(|_| "/tmp/cubarium-wind".into()),
    );
    std::fs::create_dir_all(&dir).expect("writable capture dir");

    let lantern = slot_for(Face::Front, "lanternstalk", Band::Foliage);
    let tendril = slot_for(Face::Front, "tendrilfan", Band::Foliage);
    let glow = slot_for(Face::Front, "glowcap", Band::Soil);
    let veil = slot_for(Face::Front, "rootveil", Band::Soil);
    let umbrella = slot_for(Face::Top, "umbrellafrond", Band::Canopy);
    let bloom = slot_for(Face::Top, "bloomcrown", Band::Canopy);
    // A second lanternstalk on Right, fed late so its authored growth clip plays in wind.
    let pilot = slot_for(Face::Right, "lanternstalk", Band::Foliage);
    // A reed: a deep pool in a bottom foliage cell of Left.
    let reed = CellId::all()
        .find(|&c| c.face() == Face::Left && plant_cap(Band::Water, c) == Some(2) && c.cy() == 10)
        .expect("a reed slot");
    let column = a_column(Face::Front, true);
    let column2 = a_column(Face::Back, false);
    println!(
        "columns: {} ({}), {} ({})",
        column.cx, TALL_PLANTS[column.pick], column2.cx, TALL_PLANTS[column2.pick]
    );

    let view_at = |tick: u64, elapsed: f64| {
        let mut v = bare_view(tick);
        for cell in [lantern, tendril, umbrella, bloom] {
            v.producer[cell.index()] = saturation();
        }
        v.fruit[tendril.index()] = FRUIT_SHOW * 2.0;
        for cell in [glow, veil] {
            v.detritus[cell.index()] = SOIL_SCALE;
        }
        v.water[reed.index()] = 1.2;
        for c in [column, column2] {
            for cell in CellId::all() {
                if cell.face() == c.face && cell.cx() == c.cx && band_of(cell) == Band::Foliage {
                    v.producer[cell.index()] = saturation();
                }
            }
        }
        if (6.0..30.0).contains(&elapsed) {
            // Enough for stage 1 only, so the authored 0 → 1 clip is what plays.
            v.producer[pilot.index()] = saturation() * 0.55;
        }
        v
    };

    let mut p = ArtPresenter::new(pack());
    for (name, budget) in p.bend_budgets() {
        println!("budget {name}: {budget:.3}");
    }
    // Let the scene grow in before the capture: 30 s of simulated time with no wind
    // recorded (the columns take 24 s to reach the rim).
    let warm_ticks: u64 = 600;
    for tick in 1..=warm_ticks {
        p.observe(&view_at(tick, -1.0));
    }
    let mut canvas = Canvas::new();
    let mut frame = Frame::black();
    let mut rgb = Vec::new();
    let seconds = 36.0;
    let frames = (seconds * 60.0) as u64;
    let mut written = 0;
    for i in 0..frames {
        let tick = warm_ticks + i / FRAMES_PER_TICK + 1;
        let f = (i % FRAMES_PER_TICK) as f64 / FRAMES_PER_TICK as f64;
        let elapsed = i as f64 / 60.0;
        let v = view_at(tick, elapsed);
        if i % FRAMES_PER_TICK == 0 {
            p.observe(&v);
        }
        canvas.clear();
        p.draw(&v, f, &mut canvas);
        if i % 2 == 0 {
            canvas.encode(&mut frame);
            rgb.clear();
            net_rgb8(&frame, &mut rgb);
            write_net_png(&dir.join(format!("frame_{i:05}.png")), &rgb).expect("write png");
            written += 1;
        }
    }
    println!("wrote {written} net PNGs to {}", dir.display());
}
