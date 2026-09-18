//! The ecology v1 presentation contact sheet: one PNG showing what the three stocks draw.
//!
//! ```text
//! cargo run --release -p cubarium --example ecology_sheet -- \
//!     --out design/7_Research/assets/ecology-v1-presentation-2026-09-15.png
//! ```
//!
//! Four rows, every panel drawn through the **real** [`ArtPresenter`] with the shipped
//! `assets/atelier` pack at the real 64 × 64 face resolution, then cropped and nearest-
//! neighbour magnified. Nothing here is an overlay: it is the ordinary display, cropped.
//!
//! * **Rows 1–3** are the five states of one subject, left to right: healthy, half-grazed,
//!   stripped-but-living, dead wood, empty. Row 1 is a side-face **foliage** slot, row 2 a
//!   top-face **canopy** slot, row 3 a **tall column**. The stocks are the implementation
//!   note's measured bright stand (Run 3, B0: `P = 0.4789, W = 0.3934`); "dead wood" moves
//!   that `W` into `Wd`.
//! * **Row 4** is a recovery sequence sampled from a real `World`: a bright mature stand
//!   with one pinned grazer on it, stepped until the stand is stripped, the grazer removed,
//!   and then left to reflush and regrow. The trajectory it sampled is printed to stdout so
//!   the picture can be read against the numbers.

use cubarium_surface::{Scale, Topology};
use std::path::PathBuf;

use cubarium::art::{ArtPack, Band};
use cubarium::art_present::{ArtPresenter, band_of, rank_cap_of, tall_columns};
use cubarium_core::config::WorldConfig;
use cubarium_core::genome::{Genome, decode};
use cubarium_core::organism::{Mode, Organism, Origin};
use cubarium_core::rng::Counter;
use cubarium_core::view::RenderView;
use cubarium_core::{DT, World};
use cubarium_render::Canvas;
use cubarium_surface::{CUBE_CELL_COUNT, CellId, SurfacePoint, Vec2, cell_of};
use cube_proto::{FACE_SIZE, Face, Frame};

/// B0's measured bright stand (Run 3): the reference every synthetic panel is drawn at.
const BRIGHT_P: f64 = 0.4789;
const BRIGHT_W: f64 = 0.3934;
const BRIGHT_Q: f64 = 0.1967;
const BRIGHT_F: f64 = 0.0090;
const W_MAX: f64 = 0.6;

/// The tick every synthetic panel is drawn at. Fixed so the sway phase and the wind are the
/// same in every panel of a row and only the stocks differ.
const PANEL_TICK: u64 = 41;

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
                self.set(
                    at.0 + x,
                    at.1 + y,
                    [src.rgb[o], src.rgb[o + 1], src.rgb[o + 2]],
                );
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
// the synthetic states
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

/// The five states, applied to whichever cells `cells` names.
fn state_view(state: usize, cells: &[CellId]) -> RenderView {
    let mut v = empty_view(PANEL_TICK);
    for cell in cells {
        let i = cell.index();
        let (w, p, wd) = match state {
            0 => (BRIGHT_W, BRIGHT_P, 0.0),
            1 => (BRIGHT_W, 0.5 * BRIGHT_P, 0.0),
            2 => (BRIGHT_W, 0.0, 0.0),
            3 => (0.0, 0.0, BRIGHT_W),
            _ => (0.0, 0.0, 0.0),
        };
        v.wood[i] = w;
        v.producer[i] = p;
        v.plant_reserve[i] = BRIGHT_Q * w / BRIGHT_W;
        v.dead_wood[i] = wd;
        v.fruit[i] = if state == 0 { BRIGHT_F } else { 0.0 };
    }
    v
}

/// A presenter snapped to `v`, drawn at `f = 0`, encoded.
fn shot(v: &RenderView) -> Frame {
    let mut p = ArtPresenter::new(pack());
    p.observe(v);
    let mut canvas = Canvas::cube();
    p.draw(v, 0.0, &mut canvas);
    let mut frame = Frame::black();
    canvas.encode(&mut frame);
    frame
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

fn canopy_cell() -> CellId {
    CellId::all(Topology::Cube, Scale::ONE)
        .find(|&c| {
            band_of(c) == Band::Canopy
                && rank_cap_of(c) == 2
                && (5..=10).contains(&c.cx(Topology::Cube, Scale::ONE))
                && (5..=10).contains(&c.cy(Topology::Cube, Scale::ONE))
        })
        .expect("a rank-2 canopy slot")
}

// ---------------------------------------------------------------------------
// the recovery sequence, from a real world
// ---------------------------------------------------------------------------

/// The staged config `design/ecology-v1-contract.md` §13 uses: a flat bright habitat, no
/// weather, no rain, no founders, a bare surface, and every body pinned where it stands.
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

/// The recovery run: returns the sampled `(tick, frame, P, W, Q, Wd)` rows in order.
#[allow(clippy::type_complexity)]
fn recovery(cell: CellId, samples: &[u64], horizon: u64) -> Vec<(u64, Frame, [f64; 4])> {
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

    // One pinned grazer standing in the stand's own cell.
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

    let mut presenter = ArtPresenter::new(pack());
    let mut canvas = Canvas::cube();
    let mut rows = Vec::new();
    let mut removed_at = None;
    // The grazer comes off as soon as the stand is stripped to a seventh of its foliage, so
    // what follows is reflush and regrowth rather than a slower strip.
    let strip_to = BRIGHT_P / 7.0;
    println!("# row 4 — the recovery sequence");
    println!(
        "{:>7} {:>9} {:>8} {:>8} {:>8} {:>8}",
        "tick", "seconds", "P", "W", "Q", "Wd"
    );
    for tick in 0..=horizon {
        let view = world.render_view();
        presenter.observe(&view);
        if samples.contains(&tick) {
            presenter.draw(&view, 0.0, &mut canvas);
            let mut frame = Frame::black();
            canvas.encode(&mut frame);
            let s = [
                world.state.fields.p[i],
                world.state.ecology.wood[i],
                world.state.ecology.plant_reserve[i],
                world.state.ecology.dead_wood[i],
            ];
            println!(
                "{tick:>7} {:>9.1} {:>8.4} {:>8.4} {:>8.4} {:>8.4}",
                tick as f64 * DT,
                s[0],
                s[1],
                s[2],
                s[3]
            );
            rows.push((tick, frame, s));
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
    world
        .check_invariants()
        .expect("the recovery run ends consistent");
    println!(
        "- the grazer was removed at tick {} ({:.0} s), with Q = {:.4}",
        removed_at.map_or("never".into(), |t| t.to_string()),
        removed_at.map_or(f64::NAN, |t| t as f64 * DT),
        rows.first()
            .map_or(f64::NAN, |_| world.state.ecology.plant_reserve[i])
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
        .unwrap_or_else(|| PathBuf::from("ecology-sheet.png"));

    let foliage = foliage_cell();
    let canopy = canopy_cell();
    let column = tall_columns()
        .into_iter()
        .find(|c| c.face == Face::Front && (4..=11).contains(&c.cx))
        .expect("a front tall column away from the face edges");
    println!("# rows 1-3 — the five states");
    println!("- foliage slot {foliage:?}, canopy slot {canopy:?}, column {column:?}");

    // A tall column reads the mean structure of its whole foliage column, so the synthetic
    // column panels fill that column's cells rather than one of them.
    let column_cells: Vec<CellId> = CellId::all(Topology::Cube, Scale::ONE)
        .filter(|c| {
            c.face(Topology::Cube, Scale::ONE) == column.face
                && c.cx(Topology::Cube, Scale::ONE) == column.cx
                && band_of(*c) == Band::Foliage
        })
        .collect();

    const CELL_CROP: usize = 22;
    const CELL_SCALE: usize = 3;
    const COL_W: usize = 16;
    const COL_H: usize = 50;
    const COL_SCALE: usize = 2;
    const GAP: usize = 6;
    const MARGIN: usize = 8;
    let background = [10, 6, 24];

    let mut rows: Vec<Vec<Image>> = Vec::new();
    for (cells, face, anchor) in [
        (vec![foliage], Face::Front, Some(foliage)),
        (vec![canopy], Face::Top, Some(canopy)),
        (column_cells.clone(), column.face, None),
    ] {
        let mut row = Vec::new();
        for state in 0..5 {
            let frame = shot(&state_view(state, &cells));
            row.push(match anchor {
                Some(cell) => crop(
                    &frame,
                    face,
                    i32::from(cell.cx(Topology::Cube, Scale::ONE)) * 4 + 2 - (CELL_CROP as i32) / 2,
                    i32::from(cell.cy(Topology::Cube, Scale::ONE)) * 4 + 2 - (CELL_CROP as i32) / 2,
                    CELL_CROP,
                    CELL_CROP,
                    CELL_SCALE,
                ),
                None => crop(
                    &frame,
                    face,
                    i32::from(column.cx) * 4 + 2 - (COL_W as i32) / 2,
                    0,
                    COL_W,
                    COL_H,
                    COL_SCALE,
                ),
            });
        }
        rows.push(row);
    }

    // Row 4: eight frames of one real trajectory, 30 simulated minutes.
    let samples = [0u64, 600, 1_500, 3_000, 6_000, 12_000, 24_000, 36_000];
    let mut row = Vec::new();
    for (_, frame, _) in recovery(foliage, &samples, 36_000) {
        row.push(crop(
            &frame,
            Face::Front,
            i32::from(foliage.cx(Topology::Cube, Scale::ONE)) * 4 + 2 - (CELL_CROP as i32) / 2,
            i32::from(foliage.cy(Topology::Cube, Scale::ONE)) * 4 + 2 - (CELL_CROP as i32) / 2,
            CELL_CROP,
            CELL_CROP,
            CELL_SCALE,
        ));
    }
    rows.push(row);

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
        + 2 * MARGIN;
    let mut sheet = Image::filled(width, height, background);
    let mut y = MARGIN;
    for row in &rows {
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
