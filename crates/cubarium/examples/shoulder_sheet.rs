//! The foliage-shoulder study: what [`FOLIAGE_FULL`] costs, in one PNG and four numbers.
//!
//! ```text
//! cargo run --release -p cubarium --example shoulder_sheet -- \
//!     --out design/7_Research/assets/ecology-v1-shoulder-2026-09-16.png
//! ```
//!
//! Astra's finding 7 on `design/7_Research/ecology-v1-presentation-2026-09-15.md`: the 0.85
//! shoulder holds a bright canopy unchanged through its first ~30 % of foliage loss, which
//! masks depletion. The shoulder is the fullness at which [`foliage_ramp`] reaches 1, so
//! raising it makes the picture start moving sooner — and costs margin at the other end,
//! where an ungrazed *average-light* stand (`P/W = 0.93`) would already be drawing bare wood.
//!
//! Six rows, every panel drawn through the **real** [`ArtPresenter`] with the shipped
//! `assets/atelier` pack at the real 64 x 64 face resolution, then cropped and
//! nearest-neighbour magnified. Nothing is an overlay.
//!
//! * **Rows 1-3**: a foliage-loss ladder on B0's measured bright stand (`W` fixed, `P` from
//!   whole down to nothing), at shoulder 0.85, 0.95 and 1.0.
//! * **Rows 4-6**: the *same* strip-then-reflush sequence row 4 of B's sheet shows — one
//!   pinned grazer on a real stepped `World`, removed once the stand is stripped — at the
//!   same three shoulders, sampled at the same eight ticks.
//!
//! Printed to stdout, per shoulder: the worst per-frame step of a strip-and-reflush against a
//! standing control (B's flicker bound), the frame of a controlled 30 % loss at which the
//! stand's pixels first move, the foliage loss that first moves them, and what an ungrazed
//! average-light stand draws.

use cubarium_surface::{Scale, Topology};
use std::path::PathBuf;

use cubarium::art::{ArtPack, Band};
use cubarium::art_present::{
    ArtPresenter, FOLIAGE_PER_WOOD, band_of, foliage_ramp_at, rank_cap_of,
};
use cubarium_core::config::WorldConfig;
use cubarium_core::genome::{Genome, decode};
use cubarium_core::organism::{Mode, Organism, Origin};
use cubarium_core::rng::Counter;
use cubarium_core::view::RenderView;
use cubarium_core::{DT, World};
use cubarium_render::Canvas;
use cubarium_surface::{CUBE_CELL_COUNT, CellId, SurfacePoint, Vec2, cell_of};
use cube_proto::{FACE_SIZE, Face, Frame};

/// The three shoulders the sheet compares. 0.85 is what ships.
const SHOULDERS: [f64; 3] = [0.85, 0.95, 1.0];

/// B0's measured bright stand (Run 3): `P / W = 1.217`, a whole canopy at every shoulder.
const BRIGHT_P: f64 = 0.4789;
const BRIGHT_W: f64 = 0.3934;
const BRIGHT_Q: f64 = 0.1967;
const BRIGHT_F: f64 = 0.0090;
/// B0's measured average-light stand: `P / W = 0.930`, between 0.85 and 0.95.
const AVERAGE_P: f64 = 0.0977;
const AVERAGE_W: f64 = 0.1050;
const W_MAX: f64 = 0.6;

/// The tick every synthetic panel is drawn at, so only the stocks differ across a row.
const PANEL_TICK: u64 = 41;

/// The loss ladder the first three rows show, as the share of `P` the stand has lost.
const LADDER: [f64; 8] = [0.0, 0.1, 0.2, 0.3, 0.4, 0.6, 0.8, 1.0];

/// The eight ticks B's row 4 samples.
const SAMPLES: [u64; 8] = [0, 600, 1_500, 3_000, 6_000, 12_000, 24_000, 36_000];

// ---------------------------------------------------------------------------
// a tiny image and the sheet it is pasted into
// ---------------------------------------------------------------------------

struct Image {
    w: usize,
    h: usize,
    rgb: Vec<u8>,
}

impl Image {
    fn filled(w: usize, h: usize, c: [u8; 3]) -> Image {
        Image {
            w,
            h,
            rgb: c.iter().cycle().take(w * h * 3).copied().collect(),
        }
    }

    fn set(&mut self, x: usize, y: usize, c: [u8; 3]) {
        if x < self.w && y < self.h {
            let o = (y * self.w + x) * 3;
            self.rgb[o..o + 3].copy_from_slice(&c);
        }
    }

    fn blit(&mut self, at: (usize, usize), src: &Image) {
        for y in 0..src.h {
            for x in 0..src.w {
                let o = (y * src.w + x) * 3;
                self.set(at.0 + x, at.1 + y, [src.rgb[o], src.rgb[o + 1], src.rgb[o + 2]]);
            }
        }
    }
}

/// One face's rectangle out of a drawn frame, magnified by `scale` with no filtering.
fn crop(frame: &Frame, face: Face, x0: i32, y0: i32, w: usize, h: usize, scale: usize) -> Image {
    let mut out = Image::filled(w * scale, h * scale, [0, 0, 0]);
    for y in 0..h {
        for x in 0..w {
            let (sx, sy) = (x0 + x as i32, y0 + y as i32);
            let c = if (0..FACE_SIZE as i32).contains(&sx) && (0..FACE_SIZE as i32).contains(&sy) {
                frame.get(face, sx as usize, sy as usize)
            } else {
                [0, 0, 0]
            };
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
    enc.write_header()
        .expect("PNG header")
        .write_image_data(&img.rgb)
        .expect("PNG data");
}

// ---------------------------------------------------------------------------
// views
// ---------------------------------------------------------------------------

fn pack() -> ArtPack {
    ArtPack::load(&PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets/atelier"))
        .expect("the baked pack at assets/atelier must load")
}

fn empty_view(tick: u64) -> RenderView {
    RenderView {
        topology: cubarium_surface::Topology::Cube,
        scale: cubarium_surface::Scale::ONE,
        tick,
        producer: vec![0.0; CUBE_CELL_COUNT],
        detritus: vec![0.0; CUBE_CELL_COUNT],
        fruit: vec![0.0; CUBE_CELL_COUNT],
        wood: vec![0.0; CUBE_CELL_COUNT],
        plant_reserve: vec![0.0; CUBE_CELL_COUNT],
        dead_wood: vec![0.0; CUBE_CELL_COUNT],
        carrion: vec![0.0; CUBE_CELL_COUNT],
        water: vec![0.0; CUBE_CELL_COUNT],
        rain: vec![0.0; CUBE_CELL_COUNT],
        producer_max: WorldConfig::default().producer.max,
        wood_max: W_MAX,
        organisms: Vec::new(),
    }
}

/// One cell holding a stand of living wood `w` and foliage `p`.
fn stand(tick: u64, cell: CellId, w: f64, p: f64) -> RenderView {
    let mut v = empty_view(tick);
    let i = cell.index();
    v.wood[i] = w;
    v.producer[i] = p;
    v.plant_reserve[i] = BRIGHT_Q * w / BRIGHT_W;
    v.fruit[i] = 0.0;
    v
}

fn foliage_cell() -> CellId {
    CellId::all(Topology::Cube, Scale::ONE)
        .find(|&c| {
            c.face(Topology::Cube, Scale::ONE) == Face::Front
                && band_of(c) == Band::Foliage
                && rank_cap_of(c) == 2
                && (3..=7).contains(&c.cy(Topology::Cube, Scale::ONE))
                && (4..=11).contains(&c.cx(Topology::Cube, Scale::ONE))
        })
        .expect("a rank-2 front foliage slot")
}

/// A presenter at `full`, snapped to `v`, drawn at `f = 0`.
fn shot(v: &RenderView, full: f64) -> Frame {
    let mut p = ArtPresenter::new(pack()).with_foliage_full(full);
    p.observe(v);
    let mut canvas = Canvas::cube();
    p.draw(v, 0.0, &mut canvas);
    let mut frame = Frame::black();
    canvas.encode(&mut frame);
    frame
}

// ---------------------------------------------------------------------------
// the measurements
// ---------------------------------------------------------------------------

fn every_pixel() -> impl Iterator<Item = (Face, u16, u16)> {
    Face::ALL.into_iter().flat_map(|f| {
        (0..FACE_SIZE as u16).flat_map(move |y| (0..FACE_SIZE as u16).map(move |x| (f, x, y)))
    })
}

fn max_diff(a: &Canvas, b: &Canvas) -> f32 {
    every_pixel()
        .flat_map(|(f, x, y)| {
            let (p, q) = (a.get(f, x, y), b.get(f, x, y));
            (0..3).map(move |c| (p[c] - q[c]).abs())
        })
        .fold(0.0, f32::max)
}

/// A shoulder low enough that every stand with any foliage at all draws a whole canopy: the
/// image this presenter drew *before* ecology v1, read off the same stocks. Differencing
/// against it isolates the shoulder's own effect — the ground cover, the producer wash and
/// the detritus flecks all read `P` directly and move from the first frame of any depletion
/// whatever the shoulder is, so an undifferenced comparison measures those instead.
const WHOLE: f64 = 1e-9;

/// Whether two frames are identical **as the cube shows them**: 8-bit encoded, not the f32
/// canvas, so a tone travel too small to change a displayed pixel counts as invisible.
fn same_frame(a: &Frame, b: &Frame) -> bool {
    Face::ALL.into_iter().all(|f| {
        (0..FACE_SIZE).all(|y| (0..FACE_SIZE).all(|x| a.get(f, x, y) == b.get(f, x, y)))
    })
}

fn encode(canvas: &Canvas) -> Frame {
    let mut frame = Frame::black();
    canvas.encode(&mut frame);
    frame
}

/// B's flicker measurement, at a chosen shoulder: a tick-by-tick sequence run at three frames
/// a tick, returning the largest per-channel step between consecutive frames.
fn worst_frame_step(full: f64, states: &dyn Fn(u64) -> RenderView, ticks: u64) -> f32 {
    let mut p = ArtPresenter::new(pack()).with_foliage_full(full);
    let mut canvas = Canvas::cube();
    let mut previous: Option<Canvas> = None;
    let mut worst = 0.0f32;
    for tick in 0..ticks {
        let v = states(tick);
        p.observe(&v);
        for frame in 0..3 {
            p.draw(&v, f64::from(frame) / 3.0, &mut canvas);
            if let Some(prev) = &previous {
                worst = worst.max(max_diff(prev, &canvas));
            }
            previous = Some(canvas.clone());
        }
    }
    worst
}

/// The first frame of a controlled foliage loss at which the **stand** moves, and the loss
/// that had accumulated by then.
///
/// The loss runs over 60 ticks at three frames a tick — 180 frames, 3 simulated seconds — and
/// each frame is drawn twice from the *same view*: once at the shoulder under test, once at
/// [`WHOLE`]. Both presenters see the same stocks, so the sway clip, the wind, the growth
/// pacing, the ground cover, the producer wash and the flecks are identical between them, and
/// the first frame whose **encoded** image differs is the first frame on which the shoulder
/// changes a pixel the cube would show.
fn first_visible_loss(full: f64, cell: CellId, loss: f64) -> Option<(usize, f64)> {
    const TICKS: u64 = 60;
    let mut at = ArtPresenter::new(pack()).with_foliage_full(full);
    let mut whole = ArtPresenter::new(pack()).with_foliage_full(WHOLE);
    let (mut a, mut b) = (Canvas::cube(), Canvas::cube());
    for tick in 0..TICKS {
        let share = loss * f64::from(u32::try_from(tick).unwrap()) / f64::from(TICKS as u32 - 1);
        let v = stand(tick + 1, cell, BRIGHT_W, BRIGHT_P * (1.0 - share));
        at.observe(&v);
        whole.observe(&v);
        for frame in 0..3u32 {
            let f = f64::from(frame) / 3.0;
            at.draw(&v, f, &mut a);
            whole.draw(&v, f, &mut b);
            if !same_frame(&encode(&a), &encode(&b)) {
                return Some((tick as usize * 3 + frame as usize, share));
            }
        }
    }
    None
}

/// The first frame of the same loss at which **anything at all** moves in the picture,
/// whatever the shoulder is: the same sequence compared against the *undepleted* stand, which
/// is what the ground cover, the producer wash and the detritus flecks all read.
fn first_visible_anywhere(cell: CellId, loss: f64) -> Option<(usize, f64)> {
    const TICKS: u64 = 60;
    let mut moving = ArtPresenter::new(pack());
    let mut held = ArtPresenter::new(pack());
    let (mut a, mut b) = (Canvas::cube(), Canvas::cube());
    for tick in 0..TICKS {
        let share = loss * f64::from(u32::try_from(tick).unwrap()) / f64::from(TICKS as u32 - 1);
        let moved = stand(tick + 1, cell, BRIGHT_W, BRIGHT_P * (1.0 - share));
        let still = stand(tick + 1, cell, BRIGHT_W, BRIGHT_P);
        moving.observe(&moved);
        held.observe(&still);
        for frame in 0..3u32 {
            let f = f64::from(frame) / 3.0;
            moving.draw(&moved, f, &mut a);
            held.draw(&still, f, &mut b);
            if !same_frame(&encode(&a), &encode(&b)) {
                return Some((tick as usize * 3 + frame as usize, share));
            }
        }
    }
    None
}

// ---------------------------------------------------------------------------
// the recovery sequence, from a real world (B's row 4, stepped once)
// ---------------------------------------------------------------------------

/// The staged config `design/ecology-v1-contract.md` §13 uses, exactly as B's sheet builds it.
fn staged_bright() -> WorldConfig {
    let mut c = WorldConfig::default();
    c.founders.kinds.clear();
    c.founders.count = 0;
    c.weather.amplitude = 0.0;
    c.water.rain_rate = 0.0;
    c.mechanisms.mutation = false;
    c.habitat.light_base = 0.8;
    c.habitat.light_height_gain = 0.0;
    c.habitat.light_noise_gain = 0.0;
    c.habitat.moisture_base = 0.75;
    c.habitat.moisture_height_gain = 0.0;
    c.habitat.moisture_noise_gain = 0.0;
    c.plant.initial_wood = 0.0;
    c.producer.initial_fraction = 0.0;
    c.detritus.initial_dark = 0.0;
    c.organism.speed_max = 0.0;
    c.drives.turn_rate_max_deg = 0.0;
    c.drives.feed_min = 0.001;
    c
}

fn material(world: &World) -> f64 {
    let s = &world.state;
    s.fields.n.iter().sum::<f64>()
        + s.fields.p.iter().sum::<f64>()
        + s.fields.d.iter().sum::<f64>()
        + s.fields.f.iter().sum::<f64>()
        + s.ecology.total_material()
        + s.organisms.iter().map(|(_, o)| o.material()).sum::<f64>()
        + s.hunters.gut_material_total()
}

/// B's row 4, stepped once and drawn through one presenter per shoulder: the same world, the
/// same ticks, three readings of it. Returns, per shoulder, the sampled frames in order.
fn recovery(cell: CellId, horizon: u64) -> [Vec<Frame>; SHOULDERS.len()] {
    let mut world = World::new(staged_bright()).expect("a valid staged world");
    let before = material(&world);
    for v in world.state.fields.n.iter_mut() {
        *v = 0.4;
    }
    let i = cell.index();
    world.state.fields.p[i] = BRIGHT_P;
    world.state.fields.f[i] = BRIGHT_F;
    world.state.ecology.wood[i] = BRIGHT_W;
    world.state.ecology.plant_reserve[i] = BRIGHT_Q;
    let after = material(&world);
    world.state.external_material_in += after - before;
    let mut world = World::from_state(world.state).expect("the staged state is a valid world");

    let cfg = world.config().clone();
    let mut genome = Genome::founder(0.5, &cfg.drives);
    genome.diet = 0.85;
    genome.clamp();
    let phenotype = decode(&genome, &cfg.organism);
    let pos = cell.center(Topology::Cube, Scale::ONE);
    assert_eq!(cell_of(Topology::Cube, Scale::ONE, &pos), cell);
    let grazer = world.state.organisms.insert(Organism {
        pos: SurfacePoint::new(pos.face, pos.u, pos.v),
        heading: Vec2::new(1.0, 0.0),
        ou: Vec2::ZERO,
        structure: phenotype.structure_adult,
        reserve: 0.5 * phenotype.reserve_max,
        energy: 0.75 * phenotype.energy_max,
        born_tick: world.tick(),
        hunger_memory: 1.0,
        mode: Mode::Seeking,
        escrow: None,
        births: 0,
        genome,
        phenotype,
        parent: None,
        origin: Origin::Founder,
        turn_counter: Counter::default(),
        fed_this_tick: false,
    });
    {
        let o = world.state.organisms.get(grazer).expect("placed");
        world.state.external_material_in += o.structure + o.reserve;
    }

    let mut presenters: Vec<ArtPresenter> = SHOULDERS
        .iter()
        .map(|&full| ArtPresenter::new(pack()).with_foliage_full(full))
        .collect();
    let mut canvas = Canvas::cube();
    let mut rows: [Vec<Frame>; SHOULDERS.len()] = Default::default();
    let mut removed_at = None;
    let strip_to = BRIGHT_P / 7.0;
    println!("## rows 4-6 — the strip-then-reflush sequence (B's row 4, one world)");
    println!("| tick | seconds | P | W | P/W |");
    println!("| --- | --- | --- | --- | --- |");
    for tick in 0..=horizon {
        let view = world.render_view();
        for p in presenters.iter_mut() {
            p.observe(&view);
        }
        if SAMPLES.contains(&tick) {
            let (p_now, w_now) = (world.state.fields.p[i], world.state.ecology.wood[i]);
            println!(
                "| {tick} | {:.0} | {p_now:.4} | {w_now:.4} | {:.3} |",
                tick as f64 * DT,
                p_now / w_now
            );
            for (n, presenter) in presenters.iter_mut().enumerate() {
                presenter.draw(&view, 0.0, &mut canvas);
                let mut frame = Frame::black();
                canvas.encode(&mut frame);
                rows[n].push(frame);
            }
        }
        if removed_at.is_none() && world.state.fields.p[i] <= strip_to {
            let before = material(&world);
            world.state.organisms.remove(grazer);
            let after = material(&world);
            world.state.external_material_in += after - before;
            removed_at = Some(tick);
        }
        if tick < horizon {
            world.step();
            world.drain_events();
        }
    }
    world.check_invariants().expect("the recovery run ends consistent");
    println!();
    println!(
        "- the grazer was removed at tick {} ({:.0} s)",
        removed_at.map_or("never".into(), |t| t.to_string()),
        removed_at.map_or(f64::NAN, |t| t as f64 * DT)
    );
    rows
}

// ---------------------------------------------------------------------------

fn main() {
    let out: PathBuf = std::env::args()
        .collect::<Vec<_>>()
        .windows(2)
        .find(|w| w[0] == "--out")
        .map(|w| PathBuf::from(&w[1]))
        .unwrap_or_else(|| PathBuf::from("ecology-shoulder.png"));

    let cell = foliage_cell();
    println!("# the foliage shoulder");
    println!();
    println!("- foliage slot {cell:?}");
    println!(
        "- B0 bright: P = {BRIGHT_P}, W = {BRIGHT_W}, P/W = {:.4}",
        BRIGHT_P / BRIGHT_W
    );
    println!(
        "- B0 average light: P = {AVERAGE_P}, W = {AVERAGE_W}, P/W = {:.4}",
        AVERAGE_P / AVERAGE_W
    );
    println!();

    // The table.
    let anywhere = first_visible_anywhere(cell, 0.30);
    println!("## the shoulder table");
    println!(
        "Flicker is B's bound: the worst per-channel step between two consecutive frames of a \
         strip-and-reflush at three frames a tick, against a standing control, allowed \
         0.05 of excess. The first-visible columns run a controlled 30 % loss over 180 \
         frames."
    );
    println!();
    println!(
        "| shoulder | worst frame step | standing control | excess | first frame the stand \
         moves | loss then | ramp leaves 1 at | ungrazed average stand | its worst wobble step \
         | wobble excess |"
    );
    println!("| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |");
    for &full in &SHOULDERS {
        let w = BRIGHT_W;
        let whole = FOLIAGE_PER_WOOD * w;
        let worst = worst_frame_step(
            full,
            &|tick| {
                let phase = f64::from(u32::try_from(tick).unwrap());
                let p_now = if tick < 60 {
                    whole * (1.0 - phase / 59.0)
                } else {
                    whole * ((phase - 60.0) / 59.0)
                };
                stand(tick + 1, cell, w, p_now)
            },
            120,
        );
        let control = worst_frame_step(full, &|tick| stand(tick + 1, cell, w, whole), 120);
        let first = first_visible_loss(full, cell, 0.30);
        // Where the ramp analytically leaves 1: the loss at which `P / W` first falls below
        // the shoulder. This is the shoulder's own cost, independent of any pixel threshold.
        let leaves = 1.0 - full * BRIGHT_W / BRIGHT_P;
        // What a whole *ungrazed* average-light stand draws: 0 is the image this presenter
        // drew before ecology v1, anything above it is bare wood showing on a stand nothing
        // has eaten. And whether its ordinary steady-state wobble can make that breathe:
        // +/- 5 % of `P` over 120 ticks, against the same standing control.
        let average_mix = 1.0 - foliage_ramp_at(AVERAGE_P / AVERAGE_W, full);
        let wobble = worst_frame_step(
            full,
            &|tick| {
                let phase = f64::from(u32::try_from(tick).unwrap()) / 20.0;
                stand(
                    tick + 1,
                    cell,
                    AVERAGE_W,
                    AVERAGE_P * (1.0 + 0.05 * (phase * std::f64::consts::TAU).sin()),
                )
            },
            120,
        );
        let steady = worst_frame_step(full, &|tick| stand(tick + 1, cell, AVERAGE_W, AVERAGE_P), 120);
        // And whether that mix is actually on screen: the same ungrazed average stand drawn
        // at this shoulder and at `WHOLE`, encoded, compared pixel for pixel.
        let average = stand(PANEL_TICK, cell, AVERAGE_W, AVERAGE_P);
        let tinted = !same_frame(&shot(&average, full), &shot(&average, WHOLE));
        println!(
            "| {full} | {worst:.4} | {control:.4} | {:.4} | {} | {} | {:.1} % | mix \
             {average_mix:.3}{} | {wobble:.4} | {:.4} |",
            worst - control,
            first.map_or("never".to_string(), |(f, _)| f.to_string()),
            first.map_or("never".to_string(), |(_, s)| format!("{:.1} %", 100.0 * s)),
            100.0 * leaves,
            if tinted { ", on screen" } else { ", not on screen" },
            wobble - steady,
        );
    }
    println!();
    println!(
        "- whatever the shoulder, the *picture* first moves at frame {} ({}): the ground \
         cover, the producer wash and the detritus flecks read `P` directly and none of them \
         has a shoulder.",
        anywhere.map_or("never".to_string(), |(f, _)| f.to_string()),
        anywhere.map_or("never".to_string(), |(_, s)| format!("{:.2} % lost", 100.0 * s)),
    );
    println!();

    // The picture.
    const CELL_CROP: usize = 22;
    const CELL_SCALE: usize = 3;
    const GAP: usize = 6;
    const BAND: usize = 10;
    const MARGIN: usize = 8;
    let background = [10, 6, 24];
    let crop_at = |frame: &Frame| {
        crop(
            frame,
            Face::Front,
            i32::from(cell.cx(Topology::Cube, Scale::ONE)) * 4 + 2 - (CELL_CROP as i32) / 2,
            i32::from(cell.cy(Topology::Cube, Scale::ONE)) * 4 + 2 - (CELL_CROP as i32) / 2,
            CELL_CROP,
            CELL_CROP,
            CELL_SCALE,
        )
    };

    println!("## rows 1-3 — the foliage-loss ladder on the bright stand");
    println!("| lost | P | P/W | mix at 0.85 | mix at 0.95 | mix at 1.0 |");
    println!("| --- | --- | --- | --- | --- | --- |");
    for lost in LADDER {
        let p = BRIGHT_P * (1.0 - lost);
        let f = p / BRIGHT_W;
        println!(
            "| {:.0} % | {p:.4} | {f:.3} | {:.3} | {:.3} | {:.3} |",
            100.0 * lost,
            1.0 - foliage_ramp_at(f, SHOULDERS[0]),
            1.0 - foliage_ramp_at(f, SHOULDERS[1]),
            1.0 - foliage_ramp_at(f, SHOULDERS[2]),
        );
    }
    println!();

    let mut rows: Vec<Vec<Image>> = Vec::new();
    for &full in &SHOULDERS {
        rows.push(
            LADDER
                .iter()
                .map(|&lost| {
                    crop_at(&shot(
                        &stand(PANEL_TICK, cell, BRIGHT_W, BRIGHT_P * (1.0 - lost)),
                        full,
                    ))
                })
                .collect(),
        );
    }
    for frames in recovery(cell, *SAMPLES.last().expect("eight samples")) {
        rows.push(frames.iter().map(crop_at).collect());
    }

    let width = rows
        .iter()
        .map(|r| r.iter().map(|i| i.w + GAP).sum::<usize>() - GAP)
        .max()
        .unwrap_or(0)
        + 2 * MARGIN;
    let height = rows
        .iter()
        .map(|r| r.iter().map(|i| i.h).max().unwrap_or(0) + GAP)
        .sum::<usize>()
        - GAP
        + 2 * MARGIN
        + BAND;
    let mut sheet = Image::filled(width, height, background);
    let mut y = MARGIN;
    for (n, row) in rows.iter().enumerate() {
        // A hairline between the ladder block and the sequence block, so the two groups of
        // three shoulders are not read as six of one thing.
        if n == SHOULDERS.len() {
            for x in MARGIN..(width - MARGIN) {
                sheet.set(x, y + BAND / 2, [60, 40, 90]);
            }
            y += BAND;
        }
        let mut x = MARGIN;
        let tall = row.iter().map(|i| i.h).max().unwrap_or(0);
        for panel in row {
            sheet.blit((x, y + (tall - panel.h) / 2), panel);
            x += panel.w + GAP;
        }
        y += tall + GAP;
    }
    write_png(&out, &sheet);
    println!();
    println!("wrote {} ({width} x {height})", out.display());
}
