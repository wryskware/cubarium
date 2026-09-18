//! A synthetic ring world: the scene the example animates and the golden test pins.
//!
//! This is **not** the simulation and it is not Stage B's adapter. It is a hand-built
//! [`Scene`] producer that exercises every pipeline the way the world eventually will —
//! a producer field with pools, plants across all three growth stages in all four
//! bands, tall columns, a shower that walks round the ring, and a dozen bodies that
//! cross the seam — so the renderer can be judged and measured before the world's ring
//! exists.
//!
//! Where it transcribes a presentation rule from the CPU presenter it says so and
//! names the constant. Where it invents one (band thresholds on a ring, the wind), it
//! says that too: those are Stage B's to replace with the world's own.

use std::f64::consts::TAU;

use crate::atlas::{Atlas, Clip, PlantClip};
use crate::scene::{Layer, NO_MASK_FLOOR, NO_MASK_REVEAL, RingLayout, Scene, SpriteInstance};

/// The vertical stratification, on the ring, in `Topology::height` units.
///
/// `art_present::habitat` puts the horizon at `SOIL_TOP = −0.33` and calls `h ≥ 1` the
/// canopy — which on the cube is the top face and on a ring is a single pixel row. The
/// canopy line below is therefore this module's own choice, not a transcription: it
/// gives the ring roughly the cube's *proportions* (a third soil, a canopy band that
/// reads as a band). Stage B replaces it with whatever §5's stratification decides.
pub const SOIL_TOP: f64 = -0.33;
/// Heights at or above this are canopy.
pub const CANOPY_TOP: f64 = 0.55;
/// Water deeper than this grows reeds rather than the band's plant
/// (`habitat::REED_DEPTH`).
pub const REED_DEPTH: f64 = 0.5;

/// `habitat::band_opacity`: the ceiling a band's plant stamps reach.
const MOTIF_OPACITY: f32 = 0.85;
const SOIL_PLANT_OPACITY: f32 = 0.7 * MOTIF_OPACITY;
/// `habitat::TALL_OPACITY`, and the tall column's geometry constants.
const TALL_OPACITY: f32 = 0.95;
const TALL_MAX_SEGMENTS: u32 = 9;
const TALL_FIRST_JOIN: f32 = 9.0;
const TALL_STRIP_FLOOR: f32 = 11.0;
const TALL_STRIP_TOP: f32 = 15.0;
const TILE_ROWS: f32 = 16.0;
const TALL_JOIN: f64 = 12.0;
const TALL_BASE_FADE: f64 = 0.25;
/// `wind::PLANT_BEND_LENGTH` and the tall column's, in source texels.
const PLANT_BEND_LENGTH: f32 = 13.0;
const TALL_BEND_LENGTH: f32 = 48.0;
/// `environment::GROUND_OPACITY` and the lattice pitch, in source texels.
const GROUND_OPACITY: f32 = 0.30;
const GROUND_LATTICE: u32 = 8;
/// `environment::RAIN_*`.
const RAIN_OPACITY: f32 = 0.5;
const RAIN_SPEED: f64 = 20.0;
const RAIN_PERIOD: f64 = 0.4;

/// **This module's own wind**, not the presenter's. `art_present::wind` measures each
/// family's bend headroom against the cube's nine-pixel radial footprint and lands at
/// 0.3–1.3 source texels, which — once the displacement is quantised to whole texels,
/// as pixel art requires at `S > 1` — is *no visible bend at all* for most species.
///
/// That budget is a **cube** constraint: it exists because a stamp must fit inside the
/// footprint circle `unfold_pixels` unfolds across a seam. The ring has no seam to
/// unfold and the GPU quad simply grows with the amplitude, so the ceiling is the art's
/// own tile, not the chart's. The synthetic scene therefore asks for a breeze that
/// reads — and Stage B should re-measure the budgets on the ring rather than inherit
/// the cube's.
pub const PLANT_TIP_PX: f64 = 2.5;
/// The tall columns' tip, at the top of a full column.
pub const TALL_TIP_PX: f64 = 5.0;

/// The four bands a ring cell can be in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Band {
    Soil,
    Foliage,
    Canopy,
    Water,
}

impl Band {
    fn name(self) -> &'static str {
        match self {
            Band::Soil => "soil",
            Band::Foliage => "foliage",
            Band::Canopy => "canopy",
            Band::Water => "water",
        }
    }

    /// `habitat::stage_thresholds`: the density at which this band's plant reaches
    /// stages 0, 1 and 2.
    fn thresholds(self) -> [f64; 3] {
        match self {
            Band::Soil => [0.30, 0.50, 0.75],
            Band::Foliage => [0.25, 0.45, 0.70],
            Band::Canopy => [0.15, 0.35, 0.55],
            Band::Water => [0.6, 0.8, 1.0],
        }
    }

    fn ceiling(self) -> f32 {
        match self {
            Band::Soil => SOIL_PLANT_OPACITY,
            _ => MOTIF_OPACITY,
        }
    }

    /// The band's two plants, as `habitat::SOIL_PLANTS` and friends name them.
    fn plants(self) -> [&'static str; 2] {
        match self {
            Band::Soil => ["glowcap", "rootveil"],
            Band::Foliage => ["lanternstalk", "tendrilfan"],
            Band::Canopy => ["umbrellafrond", "bloomcrown"],
            Band::Water => ["reedspire", "reedspire"],
        }
    }
}

/// One cell's fixed plant slot, laid out once — `habitat::slot_of`, transcribed.
#[derive(Clone, Copy, Debug)]
struct Slot {
    /// Anchor offset from the cell centre, within ±1 raster pixel at `S = 1`.
    jitter: [f32; 2],
    /// Heading, already jittered by at most ±12° (`HEADING_JITTER_DEG`).
    heading: [f32; 2],
    /// Which of the band's two plants.
    pick: usize,
    /// The highest stage this slot may reach (`RANK_FULL` 0.22, `RANK_MID` 0.50).
    rank_cap: u8,
    /// This slot's share of the shared breeze.
    wind: f64,
    /// Its own phase into the sway clip, so a field of stalks does not breathe in step.
    phase: f64,
}

/// One tall column standing on the horizon row.
#[derive(Clone, Copy, Debug)]
struct Column {
    /// Cell column it stands in.
    cx: u32,
    /// Which tall family.
    pick: usize,
    /// A phase into its own sway, and into its height cycle.
    phase: f64,
}

/// One organism walking round the ring.
#[derive(Clone, Copy, Debug)]
struct Body {
    /// Which creature rig.
    rig: usize,
    /// Raster pixels per simulated second along `+x`.
    speed: f64,
    /// Where it started, in raster pixels.
    start: [f64; 2],
    /// How far it bobs vertically, and how fast.
    bob: [f64; 2],
    /// A phase into its clip.
    phase: f64,
    /// Which state clip it holds (`atlas::STATES` order).
    state: usize,
}

/// The synthetic ring.
pub struct SyntheticWorld {
    layout: RingLayout,
    slots: Vec<Slot>,
    columns: Vec<Column>,
    bodies: Vec<Body>,
    /// The cell row the horizon runs through.
    horizon_row: u32,
    /// Where the shower's centre is, in cell columns; it walks round the ring.
    shower_speed: f64,
    scene: Scene,
    tick: u64,
}

impl SyntheticWorld {
    /// Lay out a ring: one slot per cell, a handful of columns, a dozen bodies.
    pub fn new(layout: RingLayout, seed: u64) -> SyntheticWorld {
        assert!(layout.is_valid(), "{layout:?}");
        let (cx, cy) = (layout.cells_x(), layout.cells_y());
        let mut slots = Vec::with_capacity((cx * cy) as usize);
        for index in 0..cx * cy {
            let mut hash =
                SplitMix64::new(seed ^ u64::from(index).wrapping_mul(0x9E37_79B9_7F4A_7C15));
            let pick = (hash.next_u64() % 2) as usize;
            let jitter = [hash.range(-1.0, 1.0) as f32, hash.range(-1.0, 1.0) as f32];
            let rank = hash.next_f64();
            let angle = hash.range(-12.0f64.to_radians(), 12.0f64.to_radians());
            slots.push(Slot {
                jitter,
                // The tile is authored standing along its −y, so a stalk's heading is
                // `stalk_heading(up) = (−up.y, up.x)` with up = (0, −1): `(1, 0)`.
                heading: [angle.cos() as f32, angle.sin() as f32],
                pick,
                rank_cap: if rank < 0.22 {
                    2
                } else if rank < 0.50 {
                    1
                } else {
                    0
                },
                wind: hash.range(0.85, 1.15),
                phase: hash.range(0.0, 3.0),
            });
        }

        let mut hash = SplitMix64::new(seed ^ 0x7461_6C6C);
        let columns = (0..12)
            .map(|i| Column {
                cx: (i * cx / 12 + (hash.next_u64() % 4) as u32) % cx,
                pick: (hash.next_u64() % 2) as usize,
                phase: hash.range(0.0, 3.0),
            })
            .collect();

        let bodies = (0..12)
            .map(|_| Body {
                rig: (hash.next_u64() % 4) as usize,
                speed: hash.range(6.0, 16.0),
                start: [
                    hash.range(0.0, f64::from(layout.w)),
                    hash.range(f64::from(layout.h) * 0.30, f64::from(layout.h) * 0.80),
                ],
                bob: [hash.range(1.0, 4.0), hash.range(0.3, 0.9)],
                phase: hash.range(0.0, 4.0),
                // rest, move or feed; never bud, which is a one-shot clip.
                state: (hash.next_u64() % 3) as usize,
            })
            .collect();

        // The horizon is the cell row whose centre height first falls below SOIL_TOP.
        let horizon_row = (0..cy)
            .find(|row| cell_height(*row, cy) < SOIL_TOP)
            .unwrap_or(cy - 1);

        SyntheticWorld {
            layout,
            slots,
            columns,
            bodies,
            horizon_row,
            shower_speed: 3.2,
            scene: Scene::new(layout),
            tick: 0,
        }
    }

    /// Advance the fields to a tick. Called at 20 Hz; [`SyntheticWorld::frame`] is
    /// called at 60.
    pub fn tick(&mut self, tick: u64, seconds: f64) {
        self.tick = tick;
        let (cx, cy) = (self.layout.cells_x(), self.layout.cells_y());
        let n = (cx * cy) as usize;
        let f = &mut self.scene.fields;
        f.producer.resize(n, 0.0);
        f.water.resize(n, 0.0);
        f.detritus.resize(n, 0.0);
        f.growth.resize(n, 0.0);
        f.tall.resize(n, 0.0);
        f.rain.resize(n, 0.0);
        f.producer_max = 1.0;
        f.revision = tick;

        // The shower: a cell of rain that walks round the ring and wraps, over the
        // foliage band only, so the seam carries a *moving* thing across it.
        let centre = (seconds * self.shower_speed).rem_euclid(f64::from(cx));
        for row in 0..cy {
            let h = cell_height(row, cy);
            for col in 0..cx {
                let i = (row * cx + col) as usize;
                let u = f64::from(col) / f64::from(cx);
                let v = f64::from(row) / f64::from(cy);

                // Producer: a wrapping sum of sines, pushed through a hard shoulder so
                // the world is *patchy* rather than uniformly overgrown. `appearance.md`
                // asks for negative space and "occasional brighter moving forms"; a
                // field that clears every stage threshold everywhere gives a hedge.
                let patch = smoothstep(0.44, 0.86, wrapped_noise(u, v, seconds * 0.02, 7));
                // Richest through the foliage band, thinner in the canopy, thinnest at
                // the rims.
                let band = if h >= CANOPY_TOP {
                    0.55 * (1.0 - (h - CANOPY_TOP) / (1.0 - CANOPY_TOP)).clamp(0.0, 1.0) + 0.30
                } else if h >= SOIL_TOP {
                    1.0 - 0.35 * ((h - 0.1) / 0.7).abs().min(1.0)
                } else {
                    (0.5 + (h - SOIL_TOP) * 2.0).clamp(0.0, 0.5)
                };
                f.producer[i] = (patch * band).clamp(0.0, 1.0) as f32;

                // Water: pools in the soil band, from a second noise field, plus a
                // shallow film that follows the shower.
                let pool = wrapped_noise(u + 0.37, v + 0.19, seconds * 0.01, 4);
                let below = ((SOIL_TOP - h) / 0.30).clamp(0.0, 1.0);
                let depth = smoothstep(0.62, 0.95, pool) * 1.4 * below;
                f.water[i] = depth as f32;

                // Detritus: litter gathers below the horizon and under rich foliage. It
                // is the soil band's own density, so its patches decide where mushrooms
                // stand; the same shoulder keeps bare ground bare.
                let litter = smoothstep(
                    0.30,
                    0.90,
                    wrapped_noise(u + 0.11, v + 0.5, seconds * 0.015, 5),
                );
                f.detritus[i] = ((0.10 + 1.10 * litter) * (0.25 + 1.05 * below) * 1.5) as f32;

                // Rain over a band of columns around the shower's centre, wrapping.
                let mut d = (f64::from(col) - centre).abs();
                if d > f64::from(cx) / 2.0 {
                    d = f64::from(cx) - d;
                }
                let over = (h - SOIL_TOP).clamp(0.0, 1.0).min(1.0);
                f.rain[i] = if d < 7.0 {
                    ((1.0 - d / 7.0) * over) as f32
                } else {
                    0.0
                };
            }
        }
        // Growth and the column heights read the fields just written, so they are a
        // second pass rather than a term in the first.
        for row in 0..cy {
            for col in 0..cx {
                let i = (row * cx + col) as usize;
                let stage = self.stage_at(i, row, cy);
                self.scene.fields.growth[i] = stage as f32;
            }
        }
        for column in &self.columns.clone() {
            let i = (self.horizon_row * cx + column.cx) as usize;
            self.scene.fields.tall[i] = self.column_height(column, seconds) as f32;
        }
    }

    /// Build the frame's instances. `seconds` is presentation seconds and `f` the tick
    /// phase, exactly as [`Scene`] documents them.
    pub fn frame(&mut self, atlas: &Atlas, seconds: f64, f: f32) -> &Scene {
        self.scene.clear_instances();
        self.scene.seconds = seconds;
        self.scene.tick = self.tick;
        self.scene.f = f;
        self.ground_cover(atlas, seconds);
        self.plants(atlas, seconds);
        self.tall(atlas, seconds);
        self.rain(atlas, seconds);
        self.bodies(atlas, seconds);
        &self.scene
    }

    /// The scene as it stands, without rebuilding it.
    pub fn scene(&self) -> &Scene {
        &self.scene
    }

    // --- the passes ---------------------------------------------------------------

    /// Pass 5: each band's tileable 8×8 texture on the 8-texel lattice, fading in with
    /// the same density that grows the band's plants and cross-fading through the
    /// horizon — `environment::ground_opacity` and `ground_weight`.
    fn ground_cover(&mut self, atlas: &Atlas, seconds: f64) {
        let s = self.layout.scale;
        let pitch = GROUND_LATTICE * s;
        let half = pitch / 2;
        let (nx, ny) = (self.layout.w / pitch, self.layout.h / pitch);
        for j in 0..ny {
            for i in 0..nx {
                let (px, py) = (i * pitch + half, j * pitch + half);
                let cell = self.cell_at(px, py);
                let band = self.band_of(cell, py);
                // The soil tile takes the soil weight and the others its complement, so
                // the texture cross-fades through the horizon with the ground under it.
                let soil = soil_weight(f64::from(py) + 0.5, f64::from(self.layout.h));
                let weight = if band == Band::Soil { soil } else { 1.0 - soil };
                let tile_band = if band == Band::Water {
                    Band::Soil
                } else {
                    band
                };
                let Some(clip) = atlas.ground(tile_band.name()) else {
                    continue;
                };
                let t = self.density_at(cell, tile_band);
                let t0 = tile_band.thresholds()[0];
                let opacity = (((t - t0) / (1.0 - t0)).clamp(0.0, 1.0) as f32)
                    * GROUND_OPACITY
                    * weight as f32;
                if opacity <= 0.0 {
                    continue;
                }
                let phase = hash01(u64::from(i) << 20 ^ u64::from(j) ^ 0x6772_6F75) * clip.seconds;
                self.scene.push(
                    Layer::GroundCover,
                    instance(
                        atlas,
                        clip,
                        seconds + phase,
                        [px as f32 + 0.5, py as f32 + 0.5],
                        [1.0, 0.0],
                        opacity,
                    ),
                );
            }
        }
    }

    /// Passes 7 and 9's small half: one plant per cell that warrants one, at the stage
    /// its density gives, with the slot's shared breeze.
    fn plants(&mut self, atlas: &Atlas, seconds: f64) {
        let (cx, cy) = (self.layout.cells_x(), self.layout.cells_y());
        let cell_px = 4 * self.layout.scale;
        for row in 0..cy {
            for col in 0..cx {
                let index = (row * cx + col) as usize;
                let band = self.band_of(index, row * cell_px + cell_px / 2);
                let slot = self.slots[index];
                let stage = self.stage_at(index, row, cy);
                if stage < 0.0 {
                    continue;
                }
                let capped = stage.min(f64::from(slot.rank_cap));
                let name = band.plants()[slot.pick];
                let t = self.density_at(index, band);
                let thresholds = band.thresholds();
                let ceiling = band.ceiling();
                let lower = capped.floor() as u8;
                let upper = (lower + 1).min(2);
                let step = capped - f64::from(lower);
                let opacity = stage_opacity(lower, t, &thresholds, ceiling);
                if opacity <= 0.0 {
                    continue;
                }
                let anchor = [
                    (col * cell_px) as f32 + cell_px as f32 * 0.5 + slot.jitter[0],
                    (row * cell_px) as f32 + cell_px as f32 * 0.5 + slot.jitter[1],
                ];
                let bend = self.plant_bend(&slot, anchor, seconds);
                // A step in flight with an authored growth clip is one stamp of that
                // clip; otherwise the idle stage. `art_present` also blends the two
                // idle clips into the ends of the step — the synthetic scene keeps the
                // clip alone, which is the same image at both ends.
                let (clip, phase) = if step > 1e-3 && lower < 2 {
                    match atlas.plant(name, PlantClip::Grow(lower, upper)) {
                        Some(grow) => (grow, step * grow.seconds - seconds),
                        None => (
                            atlas.plant(name, PlantClip::Stage(lower)).unwrap(),
                            slot.phase,
                        ),
                    }
                } else {
                    let Some(idle) = atlas.plant(name, PlantClip::Stage(lower)) else {
                        continue;
                    };
                    (idle, slot.phase)
                };
                let mut sprite =
                    instance(atlas, clip, seconds + phase, anchor, slot.heading, opacity);
                sprite.bend = [bend as f32, 0.0, 0.0, PLANT_BEND_LENGTH];
                self.scene.push(Layer::Plants, sprite);
            }
        }
    }

    /// Pass 9: base, the trunk strips the grown height reaches, and the crown — every
    /// row composited exactly once, as `tall::draw_column` arranges it.
    fn tall(&mut self, atlas: &Atlas, seconds: f64) {
        let s = self.layout.scale as f32;
        let cell_px = 4 * self.layout.scale;
        let names = ["spiretree", "glasscane"];
        for column in &self.columns.clone() {
            let name = names[column.pick];
            let height = self.column_height(column, seconds);
            if !(height > 0.0) {
                continue;
            }
            let fade = (height / TALL_BASE_FADE).clamp(0.0, 1.0) as f32;
            let grown = tall_grown_px(height) as f32;
            let anchor_x = (column.cx * cell_px) as f32 + cell_px as f32 * 0.5;
            let anchor_y = (self.horizon_row * cell_px) as f32 + cell_px as f32 * 0.5;
            // One wind sample at the column's base, one amplitude for every part of it,
            // so the whole column is one continuous curve of one height coordinate.
            let amplitude = TALL_TIP_PX
                * wind_at(
                    f64::from(anchor_x),
                    f64::from(anchor_y),
                    seconds,
                    f64::from(self.layout.w),
                );
            let at = |i: f32| [anchor_x, anchor_y - 4.0 * i * s];
            let stamp =
                |world: &mut Scene, clip: Clip, i: f32, floor: f32, reveal: f32, opacity: f32| {
                    let mut sprite = instance(
                        atlas,
                        clip,
                        seconds + column.phase,
                        at(i),
                        [1.0, 0.0],
                        opacity,
                    );
                    // `tall_bend_base(i) = 4i − 8`: the tile's bottom edge above the root.
                    sprite.bend = [amplitude as f32, 4.0 * i - 8.0, 0.0, TALL_BEND_LENGTH];
                    sprite.mask_floor = floor;
                    sprite.mask_reveal = reveal;
                    world.push(Layer::Tall, sprite);
                };
            if let Some(base) = atlas.tall(name, "base") {
                stamp(
                    &mut self.scene,
                    base,
                    0.0,
                    NO_MASK_FLOOR,
                    NO_MASK_REVEAL,
                    TALL_OPACITY * fade,
                );
            }
            if let Some(trunk) = atlas.tall(name, "trunk") {
                for i in 1..=TALL_MAX_SEGMENTS {
                    let floor = if i == 1 {
                        TALL_FIRST_JOIN
                    } else {
                        TALL_STRIP_FLOOR
                    };
                    let top = if i == TALL_MAX_SEGMENTS {
                        TILE_ROWS
                    } else {
                        TALL_STRIP_TOP
                    };
                    let local = grown - (4.0 * i as f32 - 8.0);
                    let reveal = local.min(top);
                    if reveal <= floor {
                        break;
                    }
                    stamp(
                        &mut self.scene,
                        trunk,
                        i as f32,
                        floor,
                        reveal,
                        TALL_OPACITY,
                    );
                }
            }
            if let Some(crown) = atlas.tall(name, "crown") {
                stamp(
                    &mut self.scene,
                    crown,
                    height as f32 + 1.0,
                    NO_MASK_FLOOR,
                    NO_MASK_REVEAL,
                    TALL_OPACITY * fade,
                );
            }
        }
    }

    /// Pass 10: `environment::rain_marks`, on the ring. Each streak is a 1×2 mark at a
    /// continuous fall position, its light shared between the head and its two trailing
    /// pixels so the fall reads as smooth rather than as a pixel step.
    fn rain(&mut self, atlas: &Atlas, seconds: f64) {
        let (cx, cy) = (self.layout.cells_x(), self.layout.cells_y());
        let solid = atlas.rect(atlas.solid());
        let cell_px = 4 * self.layout.scale;
        let fall = RAIN_SPEED * seconds.rem_euclid(RAIN_PERIOD);
        let phi = (fall - fall.floor()) as f32;
        let steps = fall.floor() as i32;
        for row in 0..cy {
            for col in 0..cx {
                let index = (row * cx + col) as usize;
                let rate = self.scene.fields.rain.get(index).copied().unwrap_or(0.0);
                if !(rate > 0.0) {
                    continue;
                }
                let streaks = ((f64::from(rate) * 3.0).ceil() as usize).clamp(1, 6);
                let scale = RAIN_OPACITY * rate.min(1.0);
                for k in 0..streaks {
                    let h = SplitMix64::new(0x7261_696E ^ (index as u64) << 8 ^ k as u64);
                    let mut h = h;
                    let dx = (h.next_u64() % 4) as i32;
                    let dy = (h.next_u64() % 4) as i32;
                    let head = (dy + steps).rem_euclid(4);
                    for (offset, weight) in [(0, 1.0 - phi), (1, 1.0), (2, phi)] {
                        let y = (row * cell_px) as i32 + (head + offset) * self.layout.scale as i32;
                        if y < 0 || y >= self.layout.h as i32 {
                            continue;
                        }
                        let alpha = (scale * weight).min(1.0);
                        if alpha <= 0.0 {
                            continue;
                        }
                        let x = (col * cell_px) as i32 + dx * self.layout.scale as i32;
                        self.scene
                            .push(Layer::Rain, rain_mark(solid, [x as f32, y as f32], alpha));
                    }
                }
            }
        }
    }

    /// Passes 11 and 12: a dozen organisms crossing the ring and its seam.
    fn bodies(&mut self, atlas: &Atlas, seconds: f64) {
        let names: Vec<String> = atlas.creature_names().to_vec();
        for body in &self.bodies.clone() {
            let Some(name) = names.get(body.rig % names.len()) else {
                continue;
            };
            let Some(clip) = atlas.creature(name, body.state) else {
                continue;
            };
            let x = (body.start[0] + body.speed * seconds).rem_euclid(f64::from(self.layout.w));
            let y = body.start[1] + body.bob[0] * (TAU * seconds * body.bob[1]).sin();
            // The tile is authored facing +x (`pack.json`'s `facing`), so the heading is
            // the direction of travel; a bobbing body tilts with its path.
            // The tilt is damped: a body that bobs a couple of pixels should lean, not
            // swim on its side, and a nearest-neighbour rotation of a 16-px tile past
            // about 20 degrees starts to break its own outline.
            let dy = 0.2 * body.bob[0] * body.bob[1] * TAU * (TAU * seconds * body.bob[1]).cos();
            let len = (body.speed * body.speed + dy * dy).sqrt().max(1e-6);
            self.scene.push(
                Layer::Bodies,
                instance(
                    atlas,
                    clip,
                    seconds + body.phase,
                    [x as f32, y as f32],
                    [(body.speed / len) as f32, (dy / len) as f32],
                    1.0,
                ),
            );
        }
    }

    // --- the fields, read back ------------------------------------------------------

    fn cell_at(&self, px: u32, py: u32) -> usize {
        let cell = 4 * self.layout.scale;
        let (col, row) = (px / cell, (py / cell).min(self.layout.cells_y() - 1));
        (row * self.layout.cells_x() + col.min(self.layout.cells_x() - 1)) as usize
    }

    fn band_of(&self, index: usize, py: u32) -> Band {
        if self.scene.fields.water.get(index).copied().unwrap_or(0.0) > REED_DEPTH as f32 {
            return Band::Water;
        }
        let h = 1.0 - 2.0 * (f64::from(py) + 0.5) / f64::from(self.layout.h);
        if h >= CANOPY_TOP {
            Band::Canopy
        } else if h < SOIL_TOP {
            Band::Soil
        } else {
            Band::Foliage
        }
    }

    /// The density a band's plants read: producer over the ramp's saturation for the
    /// living bands, detritus over `SOIL_SCALE` in the soil, depth in water.
    fn density_at(&self, index: usize, band: Band) -> f64 {
        let f = &self.scene.fields;
        let get = |v: &Vec<f32>| f64::from(v.get(index).copied().unwrap_or(0.0));
        match band {
            Band::Soil => get(&f.detritus) / 1.5,
            Band::Water => get(&f.water),
            _ => get(&f.producer) / (f64::from(f.producer_max) * 0.6),
        }
    }

    /// The continuous stage a cell's density warrants, or `< 0` for bare ground.
    fn stage_at(&self, index: usize, row: u32, cy: u32) -> f64 {
        let py = row * 4 * self.layout.scale + 2 * self.layout.scale;
        let _ = cy;
        let band = self.band_of(index, py);
        let t = self.density_at(index, band);
        let th = band.thresholds();
        if t < th[0] {
            return -1.0;
        }
        if t >= th[2] {
            return 2.0;
        }
        if t >= th[1] {
            1.0 + (t - th[1]) / (th[2] - th[1])
        } else {
            (t - th[0]) / (th[1] - th[0])
        }
    }

    fn column_height(&self, column: &Column, seconds: f64) -> f64 {
        // A slow breath between a sapling and a full column, so the trunk strips and
        // the crown's glide are visible in a short capture.
        let t = 0.5 - 0.5 * (TAU * (seconds * 0.05 + column.phase / 6.0)).cos();
        0.35 + t * (f64::from(TALL_MAX_SEGMENTS) - 0.35)
    }

    fn plant_bend(&self, slot: &Slot, anchor: [f32; 2], seconds: f64) -> f64 {
        PLANT_TIP_PX
            * slot.wind
            * wind_at(
                f64::from(anchor[0]),
                f64::from(anchor[1]),
                seconds,
                f64::from(self.layout.w),
            )
    }
}

/// `habitat::stage_opacity`.
fn stage_opacity(stage: u8, t: f64, thresholds: &[f64; 3], ceiling: f32) -> f32 {
    if stage > 0 {
        return ceiling;
    }
    let start = thresholds[0] - 0.04;
    let end = thresholds[1];
    if end <= start {
        return 0.0;
    }
    (((t - start) / (end - start)).clamp(0.0, 1.0) as f32) * ceiling
}

/// `tall::tall_grown_px`: the column's grown height in tile rows above the horizon.
fn tall_grown_px(height: f64) -> f64 {
    let base_top = f64::from(TALL_FIRST_JOIN) - 4.0;
    if !(height > 0.0) {
        return base_top;
    }
    if height <= 1.0 {
        base_top + (TALL_JOIN - base_top) * height
    } else {
        TALL_JOIN + 4.0 * (height - 1.0)
    }
}

/// `habitat::soil_weight`, on a raster row.
fn soil_weight(y: f64, h: f64) -> f64 {
    let height = 1.0 - 2.0 * y / h;
    let t = ((height - (SOIL_TOP - 0.06)) / 0.12).clamp(0.0, 1.0);
    1.0 - t * t * (3.0 - 2.0 * t)
}

/// The Hermite smoothstep, as every ramp in the presenter uses it.
fn smoothstep(lo: f64, hi: f64, v: f64) -> f64 {
    let t = ((v - lo) / (hi - lo)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// A cell row's centre height.
fn cell_height(row: u32, rows: u32) -> f64 {
    1.0 - 2.0 * (f64::from(row) + 0.5) / f64::from(rows)
}

/// A travelling breeze: one number in `[−1, 1]`, wrapping exactly round the ring
/// because every `u` harmonic is an integer multiple of `2π`.
fn wind_at(x: f64, y: f64, seconds: f64, w: f64) -> f64 {
    let u = x / w;
    (0.65 * (TAU * (2.0 * u - seconds * 0.11) + y * 0.02).sin()
        + 0.35 * (TAU * (5.0 * u - seconds * 0.19)).sin())
    .clamp(-1.0, 1.0)
}

/// A wrapping field in `[0, 1]`: a handful of separable harmonics whose `u`
/// frequencies are whole numbers of turns, so `u = 0` and `u = 1` are the same point
/// and the seam is invisible by construction — the same argument §5a makes for the
/// cylinder embedding. The `v` frequencies are free, and the phases are irrational
/// multiples of each other, so the terms do not line up into stripes.
fn wrapped_noise(u: f64, v: f64, t: f64, harmonics: u32) -> f64 {
    // (turns round the ring, rows down the raster, phase u, phase v, amplitude)
    const TERMS: [(f64, f64, f64, f64, f64); 7] = [
        (1.0, 1.9, 0.00, 0.71, 1.00),
        (2.0, 3.3, 1.37, 2.11, 0.62),
        (3.0, 1.1, 2.94, 0.43, 0.46),
        (4.0, 5.2, 0.61, 3.31, 0.33),
        (6.0, 2.7, 2.18, 1.07, 0.25),
        (8.0, 7.4, 1.05, 2.63, 0.17),
        (11.0, 4.6, 3.02, 0.29, 0.12),
    ];
    let mut sum = 0.0;
    let mut weight = 0.0;
    for (i, (ku, kv, pu, pv, amplitude)) in TERMS.iter().enumerate() {
        if i as u32 >= harmonics {
            break;
        }
        sum += amplitude
            * (TAU * ku * u + pu + t * (i as f64 * 0.21 + 0.3)).sin()
            * (TAU * kv * v + pv - t * 0.17).cos();
        weight += amplitude;
    }
    (0.5 + 0.5 * sum / weight).clamp(0.0, 1.0)
}

/// One stamp of a clip at a presentation time.
fn instance(
    atlas: &Atlas,
    clip: Clip,
    seconds: f64,
    anchor: [f32; 2],
    heading: [f32; 2],
    opacity: f32,
) -> SpriteInstance {
    let (a, b, mix) = clip.pose(seconds);
    let mut sprite = SpriteInstance {
        anchor,
        heading,
        opacity,
        ..SpriteInstance::empty()
    };
    assert!(
        sprite.push_pose(a, b, mix, 1.0, atlas),
        "one pose always fits four slots"
    );
    sprite
}

/// A rain streak: one source texel of the atlas's white pixel, in the rain colour, so
/// rain costs no second pipeline and no second texture.
fn rain_mark(solid: crate::atlas::FrameRect, anchor: [f32; 2], alpha: f32) -> SpriteInstance {
    let rain = crate::palette::srgb_linear(crate::palette::RAIN_SRGB);
    SpriteInstance {
        anchor,
        heading: [1.0, 0.0],
        // A 1×1 tile with a pivot at its corner is exactly one S × S block.
        frames: [[solid.x, solid.y], [0, 0], [0, 0], [0, 0]],
        size: [1, 1],
        pivot: [0, 0],
        weights: [1.0, 0.0, 0.0, 0.0],
        opacity: alpha,
        tone_colour: rain,
        tone_mix: 1.0,
        ..Default::default()
    }
}

fn hash01(key: u64) -> f64 {
    SplitMix64::new(key).next_f64()
}

/// SplitMix64, the same generator `art_present` seeds its slot hashes with.
#[derive(Clone, Copy)]
struct SplitMix64(u64);

impl SplitMix64 {
    fn new(seed: u64) -> SplitMix64 {
        SplitMix64(seed)
    }

    fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    fn next_f64(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 / (1u64 << 53) as f64
    }

    fn range(&mut self, lo: f64, hi: f64) -> f64 {
        lo + (hi - lo) * self.next_f64()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_tick_fills_every_field_and_the_seam_is_continuous() {
        let mut world = SyntheticWorld::new(RingLayout::RING_320, 1);
        world.tick(1, 1.0);
        let f = &world.scene().fields;
        let n = RingLayout::RING_320.cell_count();
        for field in [
            &f.producer,
            &f.water,
            &f.detritus,
            &f.rain,
            &f.growth,
            &f.tall,
        ] {
            assert_eq!(field.len(), n);
        }
        assert!(f.producer.iter().any(|v| *v > 0.3), "no producer anywhere");
        assert!(f.water.iter().any(|v| *v > 0.2), "no pools anywhere");
        assert!(f.rain.iter().any(|v| *v > 0.0), "no shower anywhere");
        // Column 0 and column 79 are neighbours on the ring: the wrapping noise must
        // not step between them any harder than any other adjacent pair.
        let cx = RingLayout::RING_320.cells_x() as usize;
        for row in 0..RingLayout::RING_320.cells_y() as usize {
            let seam = (f.producer[row * cx] - f.producer[row * cx + cx - 1]).abs();
            let inner = (f.producer[row * cx + 1] - f.producer[row * cx]).abs();
            assert!(
                seam <= inner.max(0.02) * 3.0 + 0.02,
                "row {row}: seam {seam} vs {inner}"
            );
        }
    }

    #[test]
    fn a_frame_fills_every_layer_and_stays_inside_the_instance_budget() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/atelier");
        let atlas = Atlas::load(&root).unwrap();
        let mut world = SyntheticWorld::new(RingLayout::RING_320, 1);
        world.tick(20, 1.0);
        let scene = world.frame(&atlas, 1.0, 0.0);
        for layer in crate::scene::LAYERS {
            assert!(
                !scene.layers[layer as usize].is_empty(),
                "{layer:?} drew nothing"
            );
        }
        assert!(
            scene.instance_count() < 8192,
            "{} instances",
            scene.instance_count()
        );
        assert!(
            scene.layers[Layer::Plants as usize].len() > 200,
            "only {} plants",
            scene.layers[Layer::Plants as usize].len()
        );
    }
}
