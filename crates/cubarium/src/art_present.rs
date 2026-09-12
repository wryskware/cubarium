//! The opt-in art image for the live world (`cubarium run --art <dir>`).
//!
//! Same contract as [`crate::present::Presenter`] — [`ArtPresenter::observe`] once per
//! completed tick, [`ArtPresenter::draw`] once per rendered frame — but the bodies are
//! the authored sprite clips of an [`ArtPack`] instead of procedural discs, and the
//! fields grow the authored **plants** of `art/PLANTS.md` where they are rich.
//!
//! What this presenter keeps from the decided M2 image above the horizon: the night
//! floor, the producer ramp with its saturation point and squared brightness, and the
//! detritus flecks, drawn with exactly the arguments [`crate::present::Presenter::draw`]
//! uses. What it drops: trails, disc lobes, and the warm feeding flash. The clips carry
//! that expression, and stacking both reads as two creatures on top of each other.
//!
//! What it adds is the **stratification** of `design/stratified-world.md`: the cube is
//! drawn as three places rather than one top-down field. By the embedded height `h` of a
//! point ([`SurfacePoint::embed`]`()[1]`, Top = 1, the open rim = −1):
//!
//! | Band | Where | Ground | Plants (by hash, per cell) | Driven by |
//! | --- | --- | --- | --- | --- |
//! | Soil | `h < `[`SOIL_TOP`] | detritus ramp, dark plum → violet-mauve, no flecks | glowcap / rootveil, dim | detritus `D` |
//! | Foliage | [`SOIL_TOP`]` ≤ h < 1` | the decided producer ramp and flecks | lanternstalk / tendrilfan | producer `P` |
//! | Canopy | the top face (`h = 1`) | the decided producer ramp and flecks | umbrellafrond / bloomcrown, eager | producer `P` |
//! | Water | any cell deeper than [`REED_DEPTH`] | (the band's own ground) | reedspire | water depth |
//!
//! Each cell owns one plant slot at a hashed placement. The plant grows through three
//! authored stages as its field rises ([`stage_thresholds`]), with hysteresis so an
//! oscillating field does not flicker ([`next_stage`]), and a hashed **rank** caps how
//! far each slot may grow ([`rank_cap_of`]) so a rich patch shows a few full plants,
//! more mid ones and many sprouts rather than a wall of 16-pixel sprites. On the side
//! faces stalks stand *up* toward the canopy; on the top face plants are radial and face
//! wherever their hash says. A full-grown plant with a `fruit` clip plays it while the
//! cell holds fruit ([`fruit_stage`]); until the world publishes fruit, it never does.
//!
//! The horizon between soil and foliage is a *per-pixel* blend ([`w_soil`], half-width
//! [`HORIZON`] in `h`), evaluated at each pixel center's own height rather than at its
//! cell's, so it reads as one horizontal line all the way round the cube instead of a
//! staircase of cell edges. Plant choice, which is per cell, uses the cell center's band
//! with no blending.
//!
//! **The simulation does not know about bands or plants.** [`SOIL_TOP`] lives here and
//! nowhere else; the world publishes the same `P`, `D` and `w` it always did, and the
//! bands are a consequence of the height-dependent light, the downhill detritus
//! transport and the water's flow.
//!
//! Nothing here is wall-clock driven. Looping clips advance on *simulated* time
//! (`view.tick × DT`), so `--speed 8` animates eight times faster and a paused world
//! holds its pose; the bud clip advances on the organism's own gestation progress; and
//! plant stages change in [`ArtPresenter::observe`], once per tick.

use std::sync::LazyLock;

use cube_proto::{FACE_SIZE, Face};
use cubarium_core::OrganismId;
use cubarium_core::organism::Mode;
use cubarium_core::view::{OrganismView, RenderView};
use cubarium_render::{Canvas, Sprite, draw_field, stamp_sprite};
use cubarium_surface::{
    CELL_COUNT, CELLS_PER_FACE_EDGE, CellId, Edge, PixelImage, ScalarField, SurfacePoint, Vec2, cell_of,
    pixel_neighbor,
};

pub use crate::art::Band;
use crate::art::{ArtPack, Clip, GroundTile, Plant, TallPlant};
use crate::clock::DT;
use crate::present::{
    self, DETRITUS_SCALE, DETRITUS_THRESHOLD, JUVENILE_SCALE, PALETTE, PRODUCER_SATURATION,
};
use crate::rng::SplitMix64;

// --- Plant constants ---------------------------------------------------------------
//
// Every constant here is review-tunable from a viewing session: they set how much of a
// field it takes before a cell shows a sprout, a mid plant or a full one, how loud the
// plants are, and how many of them a rich patch may grow. Change them from what the cube
// says, not from what a test prefers.

/// The opacity a full-grown plant reaches — under 1 so authored dark outlines never read
/// as a hard cutout over the substrate. Review-tunable.
pub const MOTIF_OPACITY: f32 = 0.85;
/// Soil plants top out lower than the rest: scenery in the dark, not a lawn.
/// Review-tunable.
pub const SOIL_PLANT_OPACITY: f32 = 0.7 * MOTIF_OPACITY;

/// The two soil plants; a cell's hash picks one. Asset names from `art/PLANTS.md`.
pub const SOIL_PLANTS: [&str; 2] = ["glowcap", "rootveil"];
/// The two foliage plants.
pub const FOLIAGE_PLANTS: [&str; 2] = ["lanternstalk", "tendrilfan"];
/// The two canopy plants.
pub const CANOPY_PLANTS: [&str; 2] = ["umbrellafrond", "bloomcrown"];
/// The one water plant, grown wherever a cell is deeper than [`REED_DEPTH`].
pub const WATER_PLANT: &str = "reedspire";

/// Water depth (d) above which a cell grows reeds instead of its band's plant, whatever
/// band it is in. Review-tunable.
pub const REED_DEPTH: f64 = 0.5;
/// Water depth (d) that counts as density 1 for the reeds' stage thresholds.
/// Review-tunable.
pub const REED_SCALE: f64 = 1.0;

/// Producer density (as a fraction of the ramp's saturation point) at which a foliage
/// cell's plant reaches stage 0, 1 and 2. Review-tunable.
pub const FOLIAGE_STAGES: [f64; 3] = [0.25, 0.45, 0.70];
/// The canopy's thresholds, lower than the foliage's so a healthy top reads as covered
/// rather than as a few islands. Review-tunable.
pub const CANOPY_STAGES: [f64; 3] = [0.15, 0.35, 0.55];
/// The soil's thresholds, on detritus as a fraction of [`SOIL_SCALE`]. Review-tunable.
pub const SOIL_STAGES: [f64; 3] = [0.30, 0.50, 0.75];
/// The reeds' thresholds, on water depth as a fraction of [`REED_SCALE`]. The first sits
/// *above* [`REED_DEPTH`] on purpose: a cell that counts as water is drawn as water, but
/// reeds stand only in the deeper pools (the whole floor row is about 0.4 deep after a
/// shower, and reeds in every wet cell read as a cyan fence). Review-tunable.
pub const REED_STAGES: [f64; 3] = [0.6, 0.8, 1.0];

/// Hysteresis of the stage thresholds: a stage is entered when the density rises above
/// its threshold and left only when it falls below `threshold − STAGE_HYST`, so a field
/// oscillating around a threshold does not make the plant flicker. Review-tunable.
pub const STAGE_HYST: f64 = 0.04;

/// Fruit density (m per cell) above which a full-grown plant with a `fruit` clip shows
/// it. Low on purpose: with frugivores about, the fruit a rich cell holds at any moment
/// is what ripened since the last bite, 0.02–0.06 m in the fauna runs, so a threshold
/// near the ripening ceiling would never show fruit at all. Review-tunable.
pub const FRUIT_SHOW: f64 = 0.04;

/// Share of cells whose slot may grow to stage 2. Review-tunable. (0.35/0.70 read as a
/// wall of overlapping 8–9 px plants on a saturated face, then 0.30/0.60 still did once
/// the world was wired; breathing room beats coverage, and the ground cover fills the
/// gaps.)
pub const RANK_FULL: f64 = 0.22;
/// Share of cells whose slot may grow to at least stage 1 (the rest stop at a sprout).
/// Review-tunable.
pub const RANK_MID: f64 = 0.50;

/// On the side faces a plant stands up toward the canopy, turned by at most this many
/// degrees either way so a row of stalks is not a picket fence. Review-tunable.
pub const HEADING_JITTER_DEG: f64 = 12.0;

// --- Band constants ----------------------------------------------------------------
//
// The stratification of `design/stratified-world.md`. Every constant here is a
// *presentation* decision and is review-tunable from a viewing session; none of them is
// visible to the simulation, which never asks where the soil is.

/// Top of the soil band, as the embedded height `h` of a cell center (Top face = 1, the
/// open rim = −1). Review-tunable.
///
/// A side face's sixteen cell rows sit at `h = 1 − (cy + 0.5)/8`, so −0.33 puts rows
/// 11–15 — the bottom five of sixteen, just under a third — in the soil.
pub const SOIL_TOP: f64 = -0.33;

/// Half-width, in `h`, of the soft horizon between soil and foliage: the blend runs over
/// `SOIL_TOP ± HORIZON`. Review-tunable.
///
/// One pixel is 1/32 in `h` and one cell is 1/8, so the full `2 · HORIZON` transition is
/// about four pixels — roughly one cell row. Widen it towards 0.125 if the line reads as
/// hard on the cube.
pub const HORIZON: f64 = 0.06;

/// Detritus (m per cell) at which the soil ground reaches its full violet-mauve.
/// Review-tunable. Matches [`crate::present::DETRITUS_SCALE`] so a cell that saturates
/// the M2 flecks also saturates the soil.
pub const SOIL_SCALE: f64 = 1.5;

/// Bare soil: dark plum. Review-tunable *within the Outrun family* of
/// `design/appearance.md` "Palette" — the soil is the one large dark area in the image
/// and a warm or desaturated value here breaks the whole cube's mood.
pub const SOIL_LOW_SRGB: u32 = 0x0024_1033;
/// Soil at [`SOIL_SCALE`] detritus: violet-mauve. Review-tunable within the Outrun
/// family, see [`SOIL_LOW_SRGB`].
pub const SOIL_HIGH_SRGB: u32 = 0x007A_3B8F;
/// Brightness of the soil ground at zero detritus. Unlike the producer ramp this is a
/// *floor*, not a cutoff: bare soil still reads as ground rather than as an unlit panel.
pub const SOIL_MIN_BRIGHTNESS: f32 = 0.10;
/// Brightness of the soil ground at [`SOIL_SCALE`]. The ramp between the two is linear,
/// not squared like the producer ramp: soil should read as ground even when it is poor.
pub const SOIL_MAX_BRIGHTNESS: f32 = 0.55;

/// The embedded height of a cell's center: `CellId::center().embed()[1]`, Top = 1 exactly
/// and the open rim = −0.984375 (the center of the bottom cell row).
pub fn height_of(cell: CellId) -> f64 {
    cell.center().embed()[1]
}

/// The band of a height.
///
/// **Normative**: soil is `h < `[`SOIL_TOP`], canopy is `h ≥ 1` (only the top face
/// reaches it — a side face's highest cell center is at 0.984375), everything between is
/// foliage. A `NaN` height is foliage, which is the band that changes nothing.
pub fn band_of_height(h: f64) -> Band {
    if h >= 1.0 {
        Band::Canopy
    } else if h < SOIL_TOP {
        Band::Soil
    } else {
        Band::Foliage
    }
}

/// The band of a cell, by the height of its center. No blending: a cell is wholly in one
/// band for the purpose of choosing its motif.
pub fn band_of(cell: CellId) -> Band {
    band_of_height(height_of(cell))
}

/// How much of a pixel at height `h` is soil.
///
/// **Normative**: `1 − smoothstep(SOIL_TOP − HORIZON, SOIL_TOP + HORIZON, h)` — 1 well
/// below the horizon, 0 well above it, and monotonically decreasing between. A `NaN`
/// height yields 0.
///
/// This is evaluated at each *pixel* center's own height, not at its cell's, which is
/// what turns the boundary into one smooth line around the cube instead of a staircase
/// four pixels tall.
pub fn w_soil(h: f64) -> f64 {
    1.0 - smoothstep(SOIL_TOP - HORIZON, SOIL_TOP + HORIZON, h)
}

/// The standard Hermite smoothstep, clamped, with `NaN` mapped to 1 so [`w_soil`] treats
/// a nonsense height as "not soil".
fn smoothstep(edge0: f64, edge1: f64, x: f64) -> f64 {
    if x.is_nan() || !edge0.is_finite() || !edge1.is_finite() || edge1 <= edge0 {
        return 1.0;
    }
    let t = ((x - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Pixels on one face.
const FACE_PIXELS: usize = FACE_SIZE * FACE_SIZE;

/// [`w_soil`] at every pixel center, in `face.index() · 4096 + y · 64 + x` order, built
/// once. Five faces of 64 × 64 is 20,480 floats; recomputing an `embed` and a smoothstep
/// per pixel per frame would be the most expensive thing in [`ArtPresenter::draw`].
static SOIL_WEIGHT: LazyLock<Box<[f32]>> = LazyLock::new(|| {
    let mut w = vec![0.0f32; 5 * FACE_PIXELS];
    for face in Face::ALL {
        for y in 0..FACE_SIZE as u8 {
            for x in 0..FACE_SIZE as u8 {
                let h = SurfacePoint::pixel_center(face, x, y).embed()[1];
                w[weight_index(face, x, y)] = w_soil(h) as f32;
            }
        }
    }
    w.into_boxed_slice()
});

#[inline]
fn weight_index(face: Face, x: u8, y: u8) -> usize {
    face.index() * FACE_PIXELS + usize::from(y) * FACE_SIZE + usize::from(x)
}

/// The precomputed [`w_soil`] of one pixel center. 1 is wholly soil, 0 wholly
/// foliage/canopy.
pub fn soil_weight(face: Face, x: u8, y: u8) -> f32 {
    SOIL_WEIGHT[weight_index(face, x, y)]
}

/// The soil ground ramp in linear light, decoded once (see [`crate::present::PALETTE`]
/// for why the hex is decoded rather than mixed).
static SOIL_RAMP: LazyLock<([f32; 3], [f32; 3])> =
    LazyLock::new(|| (present::srgb_linear(SOIL_LOW_SRGB), present::srgb_linear(SOIL_HIGH_SRGB)));

/// Seeds the per-cell slot hash. Any fixed value works; this one keeps the placement
/// stream separate from every other `SplitMix64` stream in the host.
const MOTIF_SEED: u64 = 0x6D6F_7469_6600_0001;
/// Seeds the per-organism clip-phase hash, for the same reason.
const PHASE_SEED: u64 = 0x7068_6173_6500_0001;
/// Folded into a cell index before the phase hash so a cell and an organism that happen
/// to share a number never share a phase.
const CELL_PHASE_TAG: u64 = 1 << 40;

/// A cell's plant slot: where in the cell, facing where, which of its band's two plants,
/// and how far it may grow. Fixed for the life of the presenter — only the band (through
/// water) and the stage change.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Slot {
    /// Anchor, within ±1 px of the cell center.
    pub at: SurfacePoint,
    /// Unit heading, in the renderer's convention: the tile's `+x` points along it. The
    /// plant tiles are authored standing along tile `−y`, so on a side face this is
    /// "up toward the canopy" turned a quarter turn (`(−up.y, up.x)`), then by the
    /// hashed jitter of at most ±[`HEADING_JITTER_DEG`]; on the top face it is the free
    /// hashed heading.
    pub heading: Vec2,
    /// Which of the band's two plants (0 or 1); the water band ignores it.
    pub pick: usize,
    /// The highest stage this slot may reach (0, 1 or 2).
    pub rank_cap: u8,
}

/// The fixed slot of a cell.
///
/// **Normative**: one `SplitMix64` stream seeded from the cell index, consumed in the
/// order pick, jitter `u`, jitter `v`, free heading angle, rank, heading jitter — the same
/// order whatever band the cell is in, so moving the horizon or flooding a cell never
/// moves the plants that stay. The jitter keeps the anchor within ±1 px of the cell
/// center, which is 2 px from every cell edge, so a plant never anchors in a neighbouring
/// cell. Rank: `cap = 2` if `r < `[`RANK_FULL`], `1` if `r < `[`RANK_MID`], else `0`.
/// Heading: [`stalk_heading`] of [`up_of`] on a side face, plus the jitter; free on top.
pub fn slot_of(cell: CellId) -> Slot {
    let mut hash = SplitMix64::new(MOTIF_SEED ^ cell.index() as u64);
    let pick = (hash.next_u64() % 2) as usize;
    let center = cell.center();
    let at = SurfacePoint::new(
        center.face,
        center.u + hash.range(-1.0, 1.0),
        center.v + hash.range(-1.0, 1.0),
    );
    let free = Vec2::from_screen_angle(hash.range(0.0, std::f64::consts::TAU));
    let rank = hash.next_f64();
    let jitter = hash.range(-HEADING_JITTER_DEG, HEADING_JITTER_DEG).to_radians();
    let heading = match up_of(cell) {
        Some(up) => Vec2::from_screen_angle(stalk_heading(up).screen_angle() + jitter),
        None => free,
    };
    let rank_cap = if rank < RANK_FULL {
        2
    } else if rank < RANK_MID {
        1
    } else {
        0
    };
    Slot { at, heading, pick, rank_cap }
}

/// The renderer heading that makes a tile authored standing along its `−y` axis stand
/// along `up`: [`cubarium_render::stamp_sprite`] lays the tile's `+x` along the heading
/// and its `+y` along the heading turned a quarter turn, so the heading must be `up`
/// turned the other quarter: `(−up.y, up.x)`.
pub fn stalk_heading(up: Vec2) -> Vec2 {
    Vec2::new(-up.y, up.x)
}

/// The chart direction, at a cell's center, in which embedded height increases: "up"
/// toward the canopy on the four side faces, `None` on the level top face.
pub fn up_of(cell: CellId) -> Option<Vec2> {
    let center = cell.center();
    let du = center.embed_tangent(Vec2::new(1.0, 0.0))[1];
    let dv = center.embed_tangent(Vec2::new(0.0, 1.0))[1];
    Vec2::new(du, dv).normalized()
}

/// Where a cell's plant stands and which way it faces.
pub fn placement_of(cell: CellId) -> (SurfacePoint, Vec2) {
    let s = slot_of(cell);
    (s.at, s.heading)
}

/// The highest stage a cell's slot may reach (0, 1 or 2).
pub fn rank_cap_of(cell: CellId) -> u8 {
    slot_of(cell).rank_cap
}

/// The cap a slot has in a band, `None` for a slot that grows nothing there.
///
/// **Normative**: every band uses [`rank_cap_of`] as is, except water: a flooded floor
/// row is one to two units deep after a shower, so every wet cell would pass the reed
/// thresholds and a sprout in every slot read as a cyan fence along the whole floor.
/// In [`Band::Water`] the sprout-only slots (rank cap 0, about half of all cells) grow
/// nothing, so reeds stand in clumps with open water between them. Review-tunable by
/// changing this rule.
pub fn plant_cap(band: Band, cell: CellId) -> Option<u8> {
    let cap = rank_cap_of(cell);
    match band {
        Band::Water if cap == 0 => None,
        _ => Some(cap),
    }
}

/// The band a cell is drawn in this tick: [`Band::Water`] when its water depth exceeds
/// [`REED_DEPTH`], otherwise its geometric band ([`band_of`]). `None` water is dry.
pub fn cell_band(cell: CellId, water: Option<f64>) -> Band {
    match water {
        Some(w) if w > REED_DEPTH => Band::Water,
        _ => band_of(cell),
    }
}

/// The asset name of the plant a cell grows in a band.
///
/// **Normative**: soil → one of [`SOIL_PLANTS`], foliage → [`FOLIAGE_PLANTS`], canopy →
/// [`CANOPY_PLANTS`], each by the cell's hashed pick; water → [`WATER_PLANT`] always.
pub fn species_of(band: Band, cell: CellId) -> &'static str {
    let pick = slot_of(cell).pick;
    match band {
        Band::Soil => SOIL_PLANTS[pick],
        Band::Foliage => FOLIAGE_PLANTS[pick],
        Band::Canopy => CANOPY_PLANTS[pick],
        Band::Water => WATER_PLANT,
    }
}

/// The stage thresholds a band's plants grow by.
pub fn stage_thresholds(band: Band) -> [f64; 3] {
    match band {
        Band::Soil => SOIL_STAGES,
        Band::Foliage => FOLIAGE_STAGES,
        Band::Canopy => CANOPY_STAGES,
        Band::Water => REED_STAGES,
    }
}

/// The density that drives a cell's plant, as a fraction of its band's scale.
///
/// **Normative**: soil reads detritus over [`SOIL_SCALE`]; foliage and canopy read the
/// producer field over the ramp's saturation point (`producer_max ·
/// `[`PRODUCER_SATURATION`]); water reads depth over [`REED_SCALE`]. Not clamped: the
/// stage rules compare it against thresholds and the opacity ramp clamps on its own. A
/// missing cell, a non-positive saturation or a non-finite value is density 0.
pub fn plant_density(view: &RenderView, index: usize, band: Band) -> f64 {
    let value = match band {
        Band::Soil => view.detritus.get(index).copied().unwrap_or(0.0) / SOIL_SCALE,
        Band::Water => view.water.get(index).copied().unwrap_or(0.0) / REED_SCALE,
        Band::Foliage | Band::Canopy => {
            let saturation = view.producer_max * PRODUCER_SATURATION;
            if !(saturation.is_finite() && saturation > 0.0) {
                return 0.0;
            }
            view.producer.get(index).copied().unwrap_or(0.0) / saturation
        }
    };
    if value.is_finite() { value } else { 0.0 }
}

/// The stage a slot is in after this tick, from the stage it was in.
///
/// **Normative**: with `c` the number of stages currently entered (0 for none, `s + 1`
/// for stage `s`): while `c > 0` and `t < thresholds[c − 1] − `[`STAGE_HYST`], leave a
/// stage; then while `c < 3` and `t > thresholds[c]`, enter one; then `c ≤ cap + 1`.
/// `None` is no plant, `Some(s)` is stage `s`. Rising is strict, so a density exactly on
/// a threshold does not enter; a `NaN` density is bare ground. Applying the rule twice to
/// the same density changes nothing, which is what lets a frame re-derive what the tick
/// decided.
pub fn next_stage(current: Option<u8>, t: f64, thresholds: &[f64; 3], cap: u8) -> Option<u8> {
    if t.is_nan() {
        return None;
    }
    let mut count = current.map_or(0, |s| usize::from(s) + 1).min(3);
    while count > 0 && t < thresholds[count - 1] - STAGE_HYST {
        count -= 1;
    }
    while count < 3 && t > thresholds[count] {
        count += 1;
    }
    count = count.min(usize::from(cap) + 1).min(3);
    (count > 0).then(|| (count - 1) as u8)
}

/// A plant's opacity at its stage and density.
///
/// **Normative**: stage 0 fades in linearly from `thresholds[0] − `[`STAGE_HYST`] (so a
/// sprout held by hysteresis is still faintly there) to `thresholds[1]`, where it is
/// `ceiling`; stages 1 and 2 are `ceiling`. The ceiling is [`SOIL_PLANT_OPACITY`] in the
/// soil and [`MOTIF_OPACITY`] everywhere else.
pub fn stage_opacity(stage: u8, t: f64, thresholds: &[f64; 3], ceiling: f32) -> f32 {
    if stage > 0 {
        return ceiling;
    }
    let start = thresholds[0] - STAGE_HYST;
    let end = thresholds[1];
    if t.is_nan() || end <= start {
        return 0.0;
    }
    (((t - start) / (end - start)).clamp(0.0, 1.0) as f32) * ceiling
}

/// The opacity ceiling of a band's plants.
pub fn band_opacity(band: Band) -> f32 {
    match band {
        Band::Soil => SOIL_PLANT_OPACITY,
        _ => MOTIF_OPACITY,
    }
}

/// Whether a cell's fruit is dense enough to show: `f > `[`FRUIT_SHOW`]. `None` (the world
/// publishes no fruit yet) is never in fruit.
pub fn fruit_stage(fruit: Option<f64>) -> bool {
    matches!(fruit, Some(f) if f > FRUIT_SHOW)
}

/// A stable per-cell offset into a plant's sway clip, in `[0, seconds)`, so a patch of
/// the same plant does not sway in lockstep. A non-finite or non-positive `seconds`
/// yields 0.
pub fn plant_phase_of(cell: CellId, seconds: f64) -> f64 {
    if !(seconds.is_finite() && seconds > 0.0) {
        return 0.0;
    }
    let mut hash = SplitMix64::new(PHASE_SEED ^ (cell.index() as u64 | CELL_PHASE_TAG));
    hash.next_f64() * seconds
}

/// Which rig draws an organism, from its inherited cosmetic hue.
///
/// **Normative**: `form = min(2, floor(hue × 3))`, and a `NaN` hue is form 0.
///
/// `hue` is a cosmetic gene copied exactly at birth, so a lineage keeps its rig for as
/// long as it survives and a bud looks like its parent. This is a *rig per hue tercile*
/// and nothing more: it is not a species, not a diet, not a capability, and the world
/// does not know it exists. Two organisms with the same rig differ in every way the
/// ecology actually models.
pub fn form_of(hue: f32) -> usize {
    if hue.is_nan() {
        return 0;
    }
    // A hue outside `[0, 1]` is not something the genome produces; clamping rather than
    // wrapping keeps an out-of-range value on an end rig instead of panicking on a cast.
    ((hue * 3.0).floor() as i64).clamp(0, 2) as usize
}

/// Which rig draws an organism, from its heritable `form` gene with the hue tercile as
/// the fallback.
///
/// **Normative** (`design/fauna-v2.md` "Render view and presentation"): `form` names the
/// rig when `form < count` (the pack's creature count); otherwise, including the v1
/// `FORM_UNSET` marker and a pack with fewer rigs than the world knows, the rig is
/// [`form_of`]`(hue)` clamped into the pack. Founder kinds set `form` to their rig, so a
/// burrower is drawn as the mossback and a skimmer as the skimmer whatever their hue.
pub fn rig_of(form: u8, hue: f32, count: usize) -> usize {
    let count = count.max(1);
    if usize::from(form) < count {
        usize::from(form)
    } else {
        form_of(hue).min(count - 1)
    }
}

/// Which clip an organism's state selects.
///
/// **Normative**, in this order: an organism holding an escrow is budding
/// (`bud`, state 3) whatever else it is doing; otherwise [`Mode::Feeding`] is `feed`
/// (2), [`Mode::Seeking`] is `move` (1), and [`Mode::Resting`] is `rest` (0). Gestation
/// beats mode because a birth is the rarer and more legible event.
pub fn state_of(o: &OrganismView) -> usize {
    if o.gestation.is_some() {
        return 3;
    }
    match o.mode {
        Mode::Resting => 0,
        Mode::Seeking => 1,
        Mode::Feeding => 2,
    }
}

/// A stable per-organism offset into a looping clip, in `[0, seconds)`.
///
/// **Normative**: a hash of the organism's `OrganismId` (slot *and* generation, so a
/// recycled slot is a new body with a new phase). Without it every organism on the cube
/// would breathe in lockstep, which reads as a machine rather than as a population.
/// A non-finite or non-positive `seconds` yields 0.
pub fn phase_of(id: OrganismId, seconds: f64) -> f64 {
    if !(seconds.is_finite() && seconds > 0.0) {
        return 0.0;
    }
    let mut hash = SplitMix64::new(
        PHASE_SEED ^ (u64::from(id.slot) << 32) ^ u64::from(id.generation).wrapping_mul(GENERATION_ODD),
    );
    // `next_f64` is in `[0, 1)`, so the phase never lands exactly on `seconds`.
    hash.next_f64() * seconds
}

/// An odd multiplier so `slot` and `generation` cannot cancel each other out in the
/// phase hash.
const GENERATION_ODD: u64 = 0x9E37_79B9_7F4A_7C15;

/// Where in a clip to sample this frame.
///
/// **Normative**: the non-looping `bud` clip is driven by the organism's own gestation
/// progress — `progress × clip.seconds`, so the clip's last frame lands exactly when the
/// world commits the birth. Every looping clip is driven by *simulated* time,
/// `tick × DT + phase`, never by wall time: `--speed` and pauses then stay honest, and
/// two frames of the same tick show the same pose.
///
/// A non-looping clip with no gestation cannot arise from [`state_of`]; it yields 0.
pub fn clip_time(clip: &Clip, tick: u64, phase: f64, gestation: Option<f32>) -> f64 {
    if clip.looping {
        return tick as f64 * DT + phase;
    }
    match gestation {
        Some(progress) => f64::from(progress) * clip.seconds,
        None => 0.0,
    }
}


// --- Ground cover --------------------------------------------------------------------
//
// Between the plants the ground is not bare glow: each band has a tileable 8×8 texture
// (`art/PLANTS.md` "Ground cover") laid on an 8-pixel lattice, fading in with the same
// density that grows the band's plants. Review-tunable from a viewing session.

/// Opacity of a ground tile where its band's density is 1. Review-tunable; drop it first
/// (toward 0.18) if the lattices read as a grid at 1×.
pub const GROUND_OPACITY: f32 = 0.30;
/// Lattice pitch of the ground cover in pixels; the tiles are 8×8 with pivot (4, 4), so
/// pitch 8 tiles the face without gaps.
pub const GROUND_LATTICE: u8 = 8;
/// Seeds the per-lattice-point phase hash.
const GROUND_SEED: u64 = 0x6772_6F75_6E64_0001;

/// The opacity of a band's ground cover at density `t`.
///
/// **Normative**: `GROUND_OPACITY · clamp((t − t₀) / (1 − t₀), 0, 1)` with `t₀` the band's
/// first stage threshold, so the texture appears exactly where the band's plants begin
/// to grow and a quiet world (every cell under its first threshold) shows none — which
/// keeps the decided M2 image intact above the horizon in a quiet world. `NaN` is 0.
pub fn ground_opacity(t: f64, band: Band) -> f32 {
    let t0 = stage_thresholds(band)[0];
    if t.is_nan() || t0 >= 1.0 {
        return 0.0;
    }
    (((t - t0) / (1.0 - t0)).clamp(0.0, 1.0) as f32) * GROUND_OPACITY
}

/// The pixel centers of the ground lattice on one face: `x ≡ 4 (mod 8)`, `y ≡ 4 (mod 8)`,
/// 64 points.
pub fn ground_points(face: Face) -> impl Iterator<Item = (Face, u8, u8)> {
    let half = GROUND_LATTICE / 2;
    (0..FACE_SIZE as u8 / GROUND_LATTICE).flat_map(move |j| {
        (0..FACE_SIZE as u8 / GROUND_LATTICE)
            .map(move |i| (face, i * GROUND_LATTICE + half, j * GROUND_LATTICE + half))
    })
}

/// How much of a lattice point belongs to the band whose tile it draws: the soil weight
/// for the soil tile, its complement for the foliage and canopy tiles, so the texture
/// cross-fades through the horizon with the ground under it.
pub fn ground_weight(face: Face, x: u8, y: u8, band: Band) -> f32 {
    let w = soil_weight(face, x, y);
    match band {
        Band::Soil => w,
        _ => 1.0 - w,
    }
}

/// A stable per-lattice-point offset into the tile's breath, in `[0, seconds)`, so a face
/// of texture does not blink in lockstep.
pub fn ground_phase_of(face: Face, x: u8, y: u8, seconds: f64) -> f64 {
    if !(seconds.is_finite() && seconds > 0.0) {
        return 0.0;
    }
    let key = (face.index() as u64) << 16 | u64::from(x) << 8 | u64::from(y);
    let mut hash = SplitMix64::new(GROUND_SEED ^ key);
    hash.next_f64() * seconds
}

/// The frame of a ground tile at a simulated time (looping).
pub fn ground_frame(tile: &GroundTile, seconds: f64) -> &Sprite {
    let n = tile.frames.len().max(1);
    let phase = if tile.seconds.is_finite() && tile.seconds > 0.0 && seconds.is_finite() {
        seconds.rem_euclid(tile.seconds) / tile.seconds
    } else {
        0.0
    };
    &tile.frames[((phase * n as f64).floor() as usize).min(n - 1)]
}

// --- Water ---------------------------------------------------------------------------
//
// The pools, streams and rain of `design/water.md`, drawn from the world's per-cell depth
// and this tick's rain. Water is drawn source-over the ground and under the plants, so
// reeds stand in it and bodies wade over it. Every constant is review-tunable.

/// Shallow water: electric blue. Review-tunable within the Outrun family.
pub const WATER_LOW_SRGB: u32 = 0x001E_9BF2;
/// Deep water (depth ≥ 1): electric cyan. Review-tunable within the Outrun family.
pub const WATER_HIGH_SRGB: u32 = 0x0042_C5F8;
/// Depth (d) at which the water film covers `1 − 1/e` of the ground: a 0.05 film is
/// faint, a pool of 1 is solid. Review-tunable.
pub const WATER_FILM: f64 = 0.6;
/// Brightness of the water color at coverage 1. Review-tunable.
pub const WATER_BRIGHT: f32 = 0.55;
/// Shimmer amplitude, as a fraction of [`WATER_BRIGHT`]. Review-tunable.
pub const WATER_SHIMMER: f32 = 0.08;
/// Shimmer period in simulated seconds. Review-tunable.
pub const WATER_SHIMMER_SECONDS: f64 = 2.5;
/// Seeds the per-pixel shimmer phase.
const WATER_SEED: u64 = 0x7761_7465_7200_0001;

/// The coverage of the water layer at depth `w`: `1 − exp(−w / WATER_FILM)`, 0 for a dry
/// or nonsense depth.
pub fn water_coverage(w: f64) -> f32 {
    if !(w.is_finite() && w > 0.0) {
        return 0.0;
    }
    (1.0 - (-w / WATER_FILM).exp()) as f32
}

/// The water color at depth `w`: blue at the surface film, cyan at depth 1 and beyond.
pub fn water_color(w: f64) -> [f32; 3] {
    let (low, high) = *WATER_RAMP;
    let t = if w.is_nan() { 0.0 } else { w.clamp(0.0, 1.0) as f32 };
    present::mix(low, high, t)
}

static WATER_RAMP: LazyLock<([f32; 3], [f32; 3])> =
    LazyLock::new(|| (present::srgb_linear(WATER_LOW_SRGB), present::srgb_linear(WATER_HIGH_SRGB)));

/// The shimmer phase of a pixel, in `[0, 2π)`, from a hash of its position: a fixed
/// pattern that the time term slides through.
pub fn water_phase(face: Face, x: u8, y: u8) -> f64 {
    WATER_PHASE[weight_index(face, x, y)]
}

static WATER_PHASE: LazyLock<Box<[f64]>> = LazyLock::new(|| {
    let mut phases = vec![0.0f64; 5 * FACE_PIXELS];
    for face in Face::ALL {
        for y in 0..FACE_SIZE as u8 {
            for x in 0..FACE_SIZE as u8 {
                let key = (face.index() as u64) << 16 | u64::from(x) << 8 | u64::from(y);
                phases[weight_index(face, x, y)] =
                    SplitMix64::new(WATER_SEED ^ key).next_f64() * std::f64::consts::TAU;
            }
        }
    }
    phases.into_boxed_slice()
});

/// The water brightness at a simulated tick for a pixel of shimmer phase `phase`:
/// `WATER_BRIGHT · (1 + WATER_SHIMMER · sin(2π · tick·DT / WATER_SHIMMER_SECONDS + phase))`.
/// Simulated time only: a paused world holds its glints, and the pattern repeats exactly
/// every `WATER_SHIMMER_SECONDS / DT` ticks.
pub fn water_brightness(tick: u64, phase: f64) -> f32 {
    let cycle = (tick as f64 * DT / WATER_SHIMMER_SECONDS).fract();
    WATER_BRIGHT * (1.0 + WATER_SHIMMER * (std::f64::consts::TAU * cycle + phase).sin() as f32)
}

/// The seam-aware one-pixel box filter of a cell field at a pixel: the own cell's value
/// weighted 4, each existing pixel neighbor's cell weighted 1, normalized over what is
/// present (so the open rim does not darken). Exactly `cubarium_render::draw_field`'s
/// filter.
fn filtered_at(field: &ScalarField, face: Face, x: u8, y: u8) -> f64 {
    let mut sum = field.get(cell_of(&SurfacePoint::pixel_center(face, x, y))) * 4.0;
    let mut divisor = 4.0;
    for edge in Edge::ALL {
        if let Some((nf, nx, ny)) = pixel_neighbor(face, x, y, edge) {
            sum += field.get(cell_of(&SurfacePoint::pixel_center(nf, nx, ny)));
            divisor += 1.0;
        }
    }
    sum / divisor
}

/// The water layer: for every pixel with filtered depth `w > 0`, `px = color(w) · b · a +
/// px · (1 − a)` with `a = `[`water_coverage`]`(w)` and `b = `[`water_brightness`].
fn draw_water(canvas: &mut Canvas, water: &ScalarField, tick: u64) {
    for face in Face::ALL {
        for y in 0..FACE_SIZE as u8 {
            for x in 0..FACE_SIZE as u8 {
                let w = filtered_at(water, face, x, y);
                let a = water_coverage(w);
                if a <= 0.0 {
                    continue;
                }
                let c = water_color(w);
                let b = water_brightness(tick, water_phase(face, x, y));
                let under = canvas.get(face, x, y);
                canvas.set(
                    face,
                    x,
                    y,
                    std::array::from_fn(|i| c[i] * b * a + under[i] * (1.0 - a)),
                );
            }
        }
    }
}

// --- Rain ----------------------------------------------------------------------------

/// Rain streak color: near-white cyan. Review-tunable.
pub const RAIN_SRGB: u32 = 0x00B8_F0FF;
/// A streak's opacity at rain rate ≥ 1 d/s. Review-tunable.
pub const RAIN_OPACITY: f32 = 0.5;
/// Streaks per unit of rain rate in a cell (`ceil(rain · RAIN_DENSITY)`). Review-tunable.
pub const RAIN_DENSITY: f64 = 3.0;
/// Most streaks one cell may show at once.
pub const RAIN_MAX_STREAKS: usize = 6;
/// How fast a streak falls down a side face, in pixels per simulated second.
pub const RAIN_SPEED: f64 = 20.0;
/// A streak's fall wraps within its cell every this many simulated seconds.
pub const RAIN_PERIOD: f64 = 0.4;
/// On the level top face a streak is a sparkle, on for this long of each period.
pub const RAIN_BLINK: f64 = 0.15;
/// Seeds the per-streak origin hash.
const RAIN_SEED: u64 = 0x7261_696E_0000_0001;

/// How many streaks a cell shows at rain rate `rain`: `ceil(rain · RAIN_DENSITY)`, capped
/// at [`RAIN_MAX_STREAKS`], 0 for no rain.
pub fn rain_streaks(rain: f32) -> usize {
    if !(rain.is_finite() && rain > 0.0) {
        return 0;
    }
    ((f64::from(rain) * RAIN_DENSITY).ceil() as usize).clamp(1, RAIN_MAX_STREAKS)
}

/// The stable sub-cell origin `(dx, dy) ∈ [0, 4)²` of streak `k` of a cell.
pub fn rain_origin(cell: CellId, k: usize) -> (u8, u8) {
    let mut hash = SplitMix64::new(RAIN_SEED ^ (cell.index() as u64) << 8 ^ k as u64);
    ((hash.next_u64() % 4) as u8, (hash.next_u64() % 4) as u8)
}

/// How far a streak has fallen at simulated time `seconds`: `RAIN_SPEED · (seconds mod
/// RAIN_PERIOD)` pixels.
pub fn rain_fall(seconds: f64) -> f64 {
    if !seconds.is_finite() {
        return 0.0;
    }
    RAIN_SPEED * seconds.rem_euclid(RAIN_PERIOD)
}

/// Whether a top-face sparkle is lit at simulated time `seconds`:
/// `(seconds mod RAIN_PERIOD) < RAIN_BLINK`.
pub fn rain_blink_on(seconds: f64) -> bool {
    seconds.is_finite() && seconds.rem_euclid(RAIN_PERIOD) < RAIN_BLINK
}

/// The pixels streak `k` of a cell marks at simulated time `seconds` (face-local).
///
/// **Normative**: on a side face the streak is a 1×2 mark along the downhill chart axis
/// (`−`[`up_of`]), its head at the origin moved by `floor(`[`rain_fall`]`)` pixels
/// downhill and wrapped within the cell's four pixels, its tail one pixel further
/// downhill (dropped if that leaves the face). On the top face it is a single pixel at
/// the origin, present only while [`rain_blink_on`]. The time is `(tick + f) · DT`, so a
/// frame between ticks shows the streak part-way down without extrapolating state.
pub fn rain_marks(cell: CellId, k: usize, seconds: f64) -> Vec<(u8, u8)> {
    let (dx, dy) = rain_origin(cell, k);
    let x0 = i32::from(cell.cx()) * 4;
    let y0 = i32::from(cell.cy()) * 4;
    let Some(up) = up_of(cell) else {
        return if rain_blink_on(seconds) {
            vec![((x0 + i32::from(dx)) as u8, (y0 + i32::from(dy)) as u8)]
        } else {
            Vec::new()
        };
    };
    let down = Vec2::new(-up.x, -up.y);
    let fall = rain_fall(seconds).floor() as i32;
    let in_face = |v: i32| (0..FACE_SIZE as i32).contains(&v);
    let mut marks = Vec::with_capacity(2);
    if down.y.abs() >= down.x.abs() {
        let sign = if down.y >= 0.0 { 1 } else { -1 };
        let head = (i32::from(dy) + sign * fall).rem_euclid(4);
        let (x, y) = (x0 + i32::from(dx), y0 + head);
        marks.push((x as u8, y as u8));
        let tail = y + sign;
        if in_face(tail) {
            marks.push((x as u8, tail as u8));
        }
    } else {
        let sign = if down.x >= 0.0 { 1 } else { -1 };
        let head = (i32::from(dx) + sign * fall).rem_euclid(4);
        let (x, y) = (x0 + head, y0 + i32::from(dy));
        marks.push((x as u8, y as u8));
        let tail = x + sign;
        if in_face(tail) {
            marks.push((tail as u8, y as u8));
        }
    }
    marks
}

static RAIN_COLOR: LazyLock<[f32; 3]> = LazyLock::new(|| present::srgb_linear(RAIN_SRGB));

/// Rain: for every cell with rain, its streaks source-over the image at
/// `RAIN_OPACITY · min(rain, 1)`.
fn draw_rain(canvas: &mut Canvas, rain: &[f32], seconds: f64) {
    let color = *RAIN_COLOR;
    for (index, cell) in CellId::all().enumerate() {
        let rate = rain.get(index).copied().unwrap_or(0.0);
        let n = rain_streaks(rate);
        if n == 0 {
            continue;
        }
        let alpha = RAIN_OPACITY * rate.min(1.0);
        let face = cell.face();
        for k in 0..n {
            for (x, y) in rain_marks(cell, k, seconds) {
                let under = canvas.get(face, x, y);
                canvas.set(
                    face,
                    x,
                    y,
                    std::array::from_fn(|i| color[i] * alpha + under[i] * (1.0 - alpha)),
                );
            }
        }
    }
}

// --- Tall plants ---------------------------------------------------------------------
//
// The foliage band is eleven cells tall and a 16-px plant cannot fill it. Tall plants
// (`art/PLANTS.md` "Tall plants") live in *columns*: a base at the horizon, trunk
// segments stacked 4 px apart up the side face, and a crown. A column's height follows
// the producer biomass of its foliage cells, and a crown that reaches the rim is stamped
// at the rim cell's center so the shared surface carries it onto the top face: the tree
// tops *are* the canopy. Review-tunable throughout.

/// The two tall species; a column's hash picks one.
pub const TALL_PLANTS: [&str; 2] = ["spiretree", "glasscane"];
/// The climber that wraps a tall column's trunk.
pub const VINE_PLANT: &str = "vinecoil";
/// Share of side-face cell columns that carry a tall plant. Review-tunable.
pub const TALL_COLUMN_P: f64 = 0.22;
/// Share of tall columns that also carry a vine. Review-tunable.
pub const TALL_VINE_P: f64 = 0.4;
/// Column density above the foliage's first stage threshold per trunk segment: the
/// segment count is `round((t_col − t₀) / TALL_STEP)`. Review-tunable. (The brief said
/// 0.07 on the raw density; measured from the foliage threshold so a column grows only
/// where its foliage does, 0.08 reaches the rim at `t_col = 1`.)
pub const TALL_STEP: f64 = 0.08;
/// Most trunk segments: base + 9 trunks + crown is eleven tiles, one per foliage cell,
/// which puts the crown exactly at the rim cell's center.
pub const TALL_MAX_SEGMENTS: u8 = 9;
/// Seeds the per-column hash.
const TALL_SEED: u64 = 0x7461_6C6C_0000_0001;

/// A tall plant's column on a side face.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TallColumn {
    pub face: Face,
    pub cx: u8,
    /// Which of [`TALL_PLANTS`].
    pub pick: usize,
    /// Whether a [`VINE_PLANT`] climbs it.
    pub vine: bool,
}

/// The foliage rows of a side face as `(top cy, horizon cy)`: the cell rows whose centers
/// are foliage, top to bottom. `None` on the top face (all canopy) — and on any face with
/// no foliage row.
pub fn foliage_rows(face: Face) -> Option<(u8, u8)> {
    let rows: Vec<u8> = (0..CELLS_PER_FACE_EDGE as u8)
        .filter(|&cy| band_of(CellId::new(face, 0, cy)) == Band::Foliage)
        .collect();
    Some((*rows.first()?, *rows.last()?))
}

/// The tall column of a face's cell column, if its hash selects one.
///
/// **Normative**: one `SplitMix64` stream from the face and column, consumed as select,
/// pick, vine. Never on the top face.
pub fn tall_column_of(face: Face, cx: u8) -> Option<TallColumn> {
    foliage_rows(face)?;
    let mut hash = SplitMix64::new(TALL_SEED ^ (face.index() as u64) << 8 ^ u64::from(cx));
    let select = hash.next_f64();
    let pick = (hash.next_u64() % 2) as usize;
    let vine = hash.next_f64() < TALL_VINE_P;
    (select < TALL_COLUMN_P).then_some(TallColumn { face, cx, pick, vine })
}

/// Every tall column on the cube, side faces in `Face::ALL` order.
pub fn tall_columns() -> Vec<TallColumn> {
    Face::ALL
        .into_iter()
        .flat_map(|face| (0..CELLS_PER_FACE_EDGE as u8).filter_map(move |cx| tall_column_of(face, cx)))
        .collect()
}

/// The mean producer density (clamped to `[0, 1]`) over a column's foliage cells.
pub fn column_density(view: &RenderView, face: Face, cx: u8) -> f64 {
    let Some((top, horizon)) = foliage_rows(face) else { return 0.0 };
    let mut sum = 0.0;
    let mut n = 0.0;
    for cy in top..=horizon {
        let cell = CellId::new(face, cx, cy);
        sum += plant_density(view, cell.index(), Band::Foliage).clamp(0.0, 1.0);
        n += 1.0;
    }
    if n > 0.0 { sum / n } else { 0.0 }
}

/// The rising threshold of segment `n ≥ 1`: the column density at which a column from
/// bare ground first shows `n` trunks, `t₀ + (n − ½) · TALL_STEP` with `t₀` the foliage's
/// first stage threshold.
pub fn tall_rise(n: u8) -> f64 {
    FOLIAGE_STAGES[0] + (f64::from(n) - 0.5) * TALL_STEP
}

/// The trunk segments a column density warrants from bare ground: the largest `n ≤
/// `[`TALL_MAX_SEGMENTS`] with `t_col ≥ `[`tall_rise`]`(n)`, i.e. `round((t_col − t₀) /
/// TALL_STEP)` for `t_col > t₀`; 0 at or below `t₀` and for a nonsense density.
pub fn tall_target(t_col: f64) -> u8 {
    next_tall(0, t_col)
}

/// Hysteresis of the column height, in density: a column keeps a segment until `t_col`
/// falls more than this below that segment's rising threshold (0.6 of a step, so a
/// density one whole segment below still holds and two below lets go). Review-tunable.
pub const TALL_HYST: f64 = 0.6 * TALL_STEP;

/// The segments a column shows after this tick, from what it showed and its density.
///
/// **Normative**, like [`next_stage`]: while `n > 0` and `t_col < `[`tall_rise`]`(n) −
/// `[`TALL_HYST`], lose a segment; then while `n < `[`TALL_MAX_SEGMENTS`] and `t_col ≥
/// tall_rise(n + 1)`, gain one. Rising is immediate, falling lags by the hysteresis, a
/// bare column goes to 0 in one tick, and applying the rule twice to the same density
/// changes nothing. A `NaN` density is a bare column.
pub fn next_tall(current: u8, t_col: f64) -> u8 {
    if t_col.is_nan() {
        return 0;
    }
    let mut n = current.min(TALL_MAX_SEGMENTS);
    while n > 0 && t_col < tall_rise(n) - TALL_HYST {
        n -= 1;
    }
    while n < TALL_MAX_SEGMENTS && t_col >= tall_rise(n + 1) {
        n += 1;
    }
    n
}

/// Where tile `i` of a column stands: the horizon cell's center moved `4 · i` pixels up
/// the face. `i = 0` is the base, `1..=n` the trunks, `n + 1` the crown; with
/// `n = `[`TALL_MAX_SEGMENTS`] the crown lands on the rim cell's center.
pub fn tall_anchor(face: Face, cx: u8, i: u8) -> SurfacePoint {
    let (_, horizon) = foliage_rows(face).unwrap_or((0, 0));
    let cell = CellId::new(face, cx, horizon);
    let center = cell.center();
    let up = up_of(cell).unwrap_or(Vec2::new(0.0, -1.0));
    let step = up * (4.0 * f64::from(i));
    SurfacePoint::new(face, center.u + step.x, center.v + step.y)
}

/// The heading tall tiles are stamped with: [`stalk_heading`] of the column's up, with no
/// jitter so the segments stack straight.
pub fn tall_heading(face: Face, cx: u8) -> Vec2 {
    let (_, horizon) = foliage_rows(face).unwrap_or((0, 0));
    stalk_heading(up_of(CellId::new(face, cx, horizon)).unwrap_or(Vec2::new(0.0, -1.0)))
}

/// A stable per-column offset into a tall clip, in `[0, seconds)`.
pub fn tall_phase_of(face: Face, cx: u8, seconds: f64) -> f64 {
    if !(seconds.is_finite() && seconds > 0.0) {
        return 0.0;
    }
    let mut hash = SplitMix64::new(PHASE_SEED ^ TALL_SEED ^ (face.index() as u64) << 8 ^ u64::from(cx));
    hash.next_f64() * seconds
}

/// The pack's tall species resolved once.
#[derive(Clone, Copy, Debug, Default)]
struct TallSpecies {
    plants: [Option<usize>; 2],
    vine: Option<usize>,
}

impl TallSpecies {
    fn resolve(pack: &ArtPack) -> TallSpecies {
        let find = |name: &str| pack.tall.iter().position(|p| p.name == name);
        TallSpecies { plants: [find(TALL_PLANTS[0]), find(TALL_PLANTS[1])], vine: find(VINE_PLANT) }
    }
}

/// Stamp one column: base, `n` trunks, crown, then the vine's trunks at every second
/// trunk position, all at the column's heading and simulated time.
fn draw_column(
    canvas: &mut Canvas,
    column: &TallColumn,
    n: u8,
    plant: &TallPlant,
    vine: Option<&TallPlant>,
    seconds: f64,
    scratch: &mut Vec<PixelImage>,
) {
    if n == 0 {
        return;
    }
    let heading = tall_heading(column.face, column.cx);
    let mut stamp = |clip: &Clip, i: u8| {
        let sprite = clip.at(seconds + tall_phase_of(column.face, column.cx, clip.seconds));
        let at = tall_anchor(column.face, column.cx, i);
        stamp_sprite(canvas, at, heading, sprite, 1.0, MOTIF_OPACITY, scratch);
    };
    if let Some(base) = &plant.base {
        stamp(base, 0);
    }
    for i in 1..=n {
        stamp(&plant.trunk, i);
    }
    if let Some(crown) = &plant.crown {
        stamp(crown, n + 1);
    }
    if let (true, Some(vine)) = (column.vine, vine) {
        for i in (1..=n).filter(|i| i % 2 == 1) {
            stamp(&vine.trunk, i);
        }
    }
}

/// The pack's plants resolved to indices once, per band and pick. A pack without a plant
/// (a v1 pack, or a repaint that dropped one) simply grows nothing in that slot.
#[derive(Clone, Copy, Debug, Default)]
struct Species {
    soil: [Option<usize>; 2],
    foliage: [Option<usize>; 2],
    canopy: [Option<usize>; 2],
    water: Option<usize>,
}

impl Species {
    fn resolve(pack: &ArtPack) -> Species {
        let find = |name: &str| pack.plants.iter().position(|p| p.name == name);
        Species {
            soil: [find(SOIL_PLANTS[0]), find(SOIL_PLANTS[1])],
            foliage: [find(FOLIAGE_PLANTS[0]), find(FOLIAGE_PLANTS[1])],
            canopy: [find(CANOPY_PLANTS[0]), find(CANOPY_PLANTS[1])],
            water: find(WATER_PLANT),
        }
    }

    fn index(&self, band: Band, pick: usize) -> Option<usize> {
        match band {
            Band::Soil => self.soil[pick],
            Band::Foliage => self.foliage[pick],
            Band::Canopy => self.canopy[pick],
            Band::Water => self.water,
        }
    }
}

/// The live world drawn with the baked art. Holds the pack, the scratch buffers the
/// field and sprite paths need, the fixed per-cell slots, and the one piece of
/// renderer-side state the art image keeps: which stage each cell's plant is in.
pub struct ArtPresenter {
    pack: ArtPack,
    species: Species,
    producer: ScalarField,
    detritus: ScalarField,
    /// The raw detritus field, which the soil ground ramps against. Separate from
    /// `detritus`, which the flecks threshold: the soil is a wash, not flecks.
    soil: ScalarField,
    /// One layer of the foliage/canopy ground, drawn on its own so it can be faded out
    /// per pixel by the horizon before it reaches the image.
    layer: Canvas,
    scratch: Vec<PixelImage>,
    /// One slot per field cell, in `CellId` index order. Placement never changes.
    slots: Vec<Slot>,
    /// The band each cell was last drawn in; water can move a cell in and out of
    /// [`Band::Water`], and a cell that changes band starts over from bare ground.
    bands: Vec<Band>,
    /// The stage each cell's plant is in, `None` for bare ground. Updated once per tick
    /// in [`ArtPresenter::observe`] by [`next_stage`]; [`ArtPresenter::draw`] re-applies
    /// the same rule to the same view, which changes nothing, so a frame drawn without a
    /// preceding `observe` still shows the tick's plants.
    stages: Vec<Option<u8>>,
    /// The water field, seam-filtered per pixel when drawn.
    water: ScalarField,
    /// The pack's tall species resolved once.
    tall_species: TallSpecies,
    /// The tall columns the hash selected, side faces in `Face::ALL` order.
    columns: Vec<TallColumn>,
    /// Trunk segments each column currently shows; updated once per tick by
    /// [`next_tall`], re-applied by `draw` (a no-op on an observed view).
    segments: Vec<u8>,
}

impl ArtPresenter {
    /// Build the presenter and lay out every cell's slot once.
    pub fn new(pack: ArtPack) -> ArtPresenter {
        let species = Species::resolve(&pack);
        let slots: Vec<Slot> = CellId::all().map(slot_of).collect();
        let bands = CellId::all().map(band_of).collect();
        let tall_species = TallSpecies::resolve(&pack);
        let columns = tall_columns();
        let segments = vec![0; columns.len()];
        ArtPresenter {
            pack,
            species,
            water: ScalarField::zeros(),
            tall_species,
            columns,
            segments,
            producer: ScalarField::zeros(),
            detritus: ScalarField::zeros(),
            soil: ScalarField::zeros(),
            layer: Canvas::new(),
            scratch: Vec::new(),
            slots,
            bands,
            stages: vec![None; CELL_COUNT],
        }
    }

    /// The loaded pack, for tests that want to compare a drawn body or plant against its
    /// sprite.
    pub fn pack(&self) -> &ArtPack {
        &self.pack
    }

    /// The stage a cell's plant is in after the last `observe`/`draw`, `None` for bare
    /// ground.
    pub fn stage_of(&self, cell: CellId) -> Option<u8> {
        self.stages[cell.index()]
    }

    /// The band a cell was last drawn in.
    pub fn band_at(&self, cell: CellId) -> Band {
        self.bands[cell.index()]
    }

    /// The plant a cell grows in a band, if the pack has it.
    pub fn plant_for(&self, band: Band, cell: CellId) -> Option<&Plant> {
        let pick = self.slots[cell.index()].pick;
        self.species.index(band, pick).map(|i| &self.pack.plants[i])
    }

    /// Record one completed tick: move every cell's plant to the stage its field now
    /// warrants ([`next_stage`], with hysteresis), after deciding the cell's band from
    /// its water. This is the only renderer-side history the art image keeps.
    pub fn observe(&mut self, view: &RenderView) {
        self.update_stages(view);
        self.update_tall(view);
    }

    /// The trunk segments a tall column shows after the last `observe`/`draw` (0 when
    /// nothing tall stands there), by its index in [`tall_columns`].
    pub fn segments_of(&self, column: usize) -> u8 {
        self.segments.get(column).copied().unwrap_or(0)
    }

    /// The tall columns this presenter draws, in the order [`segments_of`] indexes.
    pub fn columns(&self) -> &[TallColumn] {
        &self.columns
    }

    fn update_tall(&mut self, view: &RenderView) {
        for (i, column) in self.columns.iter().enumerate() {
            let t_col = column_density(view, column.face, column.cx);
            self.segments[i] = next_tall(self.segments[i], t_col);
        }
    }

    fn update_stages(&mut self, view: &RenderView) {
        for (index, cell) in CellId::all().enumerate() {
            let band = cell_band(cell, view.water.get(index).copied());
            if band != self.bands[index] {
                self.bands[index] = band;
                self.stages[index] = None;
            }
            let t = plant_density(view, index, band);
            self.stages[index] = match plant_cap(band, cell) {
                Some(cap) => next_stage(self.stages[index], t, &stage_thresholds(band), cap),
                None => None,
            };
        }
    }

    /// Draw the art image: floor, foliage/canopy ground, soil ground, ground cover,
    /// water, plants, tall plants, rain, bodies, in that order. `f` is the clock's interpolation fraction, used exactly as
    /// [`crate::present::Presenter::draw`] uses it. Full-grown plants with a `fruit` clip
    /// play it where the view's fruit field says the cell is in fruit; see
    /// [`ArtPresenter::draw_with_fruit`] for drawing with a different fruit field.
    ///
    /// The two grounds are complementary per pixel — the foliage ground is scaled by
    /// `1 − `[`w_soil`] and the soil ground by [`w_soil`] — so above the horizon the
    /// image is exactly the decided one, below it there is no producer lawn and no
    /// fleck, and across the horizon the two cross-fade over about a cell.
    pub fn draw(&mut self, view: &RenderView, f: f64, canvas: &mut Canvas) {
        self.draw_with_fruit(view, f, canvas, Some(&view.fruit));
    }

    /// [`ArtPresenter::draw`] with the world's fruit field (one value per cell, the same
    /// order as the other fields): a full-grown plant whose species has a `fruit` clip
    /// plays it where [`fruit_stage`] says the cell is in fruit. [`ArtPresenter::draw`]
    /// passes the view's own `fruit`; tests pass `None` or a synthetic field.
    pub fn draw_with_fruit(
        &mut self,
        view: &RenderView,
        f: f64,
        canvas: &mut Canvas,
        fruit: Option<&[f64]>,
    ) {
        // The same rules the tick applied, re-applied: a no-op on a view already
        // observed, and the right answer for a frame drawn without one.
        self.update_stages(view);
        self.update_tall(view);
        let seconds = view.tick as f64 * DT;

        canvas.clear();
        present::draw_floor(canvas);

        // The decided producer ramp, with the same arguments the M2 presenter passes,
        // drawn into a scratch layer and then faded out through the horizon. Drawing it
        // into a layer rather than straight onto the canvas is what lets the weight be
        // per pixel while `present::draw_ramp_field` stays untouched — and at weight 1
        // the multiply is exact, so a foliage pixel is bit-for-bit the decided image.
        present::copy_field(&mut self.producer, &view.producer);
        let saturation = view.producer_max * PRODUCER_SATURATION;
        self.layer.clear();
        present::draw_ramp_field(
            &mut self.layer,
            &self.producer,
            saturation,
            PALETTE.producer_low,
            PALETTE.producer_high,
            true,
        );
        add_above_horizon(canvas, &self.layer);

        // Detritus flecks, exactly as the M2 presenter draws them — and likewise only
        // above the horizon. In the soil, detritus is the ground itself, not a fleck.
        present::threshold_field(&mut self.detritus, &view.detritus, DETRITUS_THRESHOLD);
        self.layer.clear();
        draw_field(&mut self.layer, &self.detritus, DETRITUS_SCALE, PALETTE.detritus, false);
        add_above_horizon(canvas, &self.layer);

        // The soil ground: dark plum to violet-mauve by raw detritus, seam-filtered like
        // every other field layer, faded in through the same horizon.
        present::copy_field(&mut self.soil, &view.detritus);
        draw_soil_ground(canvas, &self.soil);

        // Ground cover: each band's tileable texture on the 8-px lattice, fading in with
        // the density that grows the band's plants and cross-fading through the horizon.
        {
            let pack = &self.pack;
            let scratch = &mut self.scratch;
            for face in Face::ALL {
                for (face, x, y) in ground_points(face) {
                    let point = SurfacePoint::pixel_center(face, x, y);
                    let cell = cell_of(&point);
                    let band = band_of(cell);
                    let Some(tile) = pack.ground_for(band) else { continue };
                    let t = plant_density(view, cell.index(), band);
                    let opacity = ground_opacity(t, band) * ground_weight(face, x, y, band);
                    if opacity <= 0.0 {
                        continue;
                    }
                    let frame = ground_frame(tile, seconds + ground_phase_of(face, x, y, tile.seconds));
                    stamp_sprite(canvas, point, Vec2::new(1.0, 0.0), frame, 1.0, opacity, scratch);
                }
            }
        }

        // Water: pools and streams source-over the ground, under the plants.
        if !view.water.is_empty() {
            present::copy_field(&mut self.water, &view.water);
            draw_water(canvas, &self.water, view.tick);
        }

        // Plants: scenery that follows the fields. These are not organisms — nothing in
        // the world knows about them, they never move, and they are not eaten. They are
        // how a rich cell reads as overgrown rather than as merely brighter.
        let pack = &self.pack;
        let species = &self.species;
        let scratch = &mut self.scratch;
        for (index, cell) in CellId::all().enumerate() {
            let Some(stage) = self.stages[index] else { continue };
            let band = self.bands[index];
            let slot = &self.slots[index];
            let Some(plant) = species.index(band, slot.pick).map(|i| &pack.plants[i]) else {
                continue;
            };
            let t = plant_density(view, index, band);
            let opacity = stage_opacity(stage, t, &stage_thresholds(band), band_opacity(band));
            if opacity <= 0.0 {
                continue;
            }
            let in_fruit = stage == 2 && fruit_stage(fruit.and_then(|f| f.get(index).copied()));
            let clip: &Clip = match (&plant.fruit, in_fruit) {
                (Some(fruit_clip), true) => fruit_clip,
                _ => &plant.stages[usize::from(stage)],
            };
            let sprite = clip.at(view.tick as f64 * DT + plant_phase_of(cell, clip.seconds));
            stamp_sprite(canvas, slot.at, slot.heading, sprite, 1.0, opacity, scratch);
        }

        // Tall plants: columns of base, trunks and crown up the side faces, the crown of a
        // full column carried onto the top face by the shared surface.
        {
            let tall = &self.tall_species;
            for (i, column) in self.columns.iter().enumerate() {
                let Some(plant) = tall.plants[column.pick].map(|p| &pack.tall[p]) else { continue };
                let vine = tall.vine.map(|v| &pack.tall[v]);
                draw_column(canvas, column, self.segments[i], plant, vine, seconds, scratch);
            }
        }

        // Rain: this tick's streaks, part-way down at the frame fraction.
        if !view.rain.is_empty() {
            draw_rain(canvas, &view.rain, (view.tick as f64 + f.clamp(0.0, 1.0)) * DT);
        }

        // Bodies: one clip frame each, driven by the organism's real state.
        for o in &view.organisms {
            let form = rig_of(o.form, o.hue, self.pack.creature_count());
            let state = state_of(o);
            let clip = &self.pack.clips[form * 4 + state];
            let phase = phase_of(o.id, clip.seconds);
            let sprite = clip.at(clip_time(clip, view.tick, phase, o.gestation));
            let (anchor, heading) = present::interpolate(&o.moved, o.pos, o.heading, f);
            let scale = if o.juvenile { JUVENILE_SCALE } else { 1.0 };
            stamp_sprite(canvas, anchor, heading, sprite, scale, 1.0, &mut self.scratch);
        }
    }
}

/// Add one ground layer to the image, each pixel scaled by `1 − `[`w_soil`].
///
/// At a pixel wholly above the horizon the scale is exactly 1, so the multiply and the
/// add reproduce the decided image bit for bit; at a pixel wholly in the soil the layer
/// is skipped entirely.
fn add_above_horizon(canvas: &mut Canvas, layer: &Canvas) {
    let weight = &*SOIL_WEIGHT;
    for face in Face::ALL {
        for y in 0..FACE_SIZE as u8 {
            for x in 0..FACE_SIZE as u8 {
                let k = 1.0 - weight[weight_index(face, x, y)];
                if k <= 0.0 {
                    continue;
                }
                let c = layer.get(face, x, y);
                if c == [0.0; 3] {
                    continue;
                }
                canvas.add(face, x, y, [c[0] * k, c[1] * k, c[2] * k]);
            }
        }
    }
}

/// The soil ground: for every pixel with any soil in it, the detritus field ramped from
/// [`SOIL_LOW_SRGB`] to [`SOIL_HIGH_SRGB`] against [`SOIL_SCALE`], at brightness
/// `SOIL_MIN_BRIGHTNESS + (SOIL_MAX_BRIGHTNESS − SOIL_MIN_BRIGHTNESS) · t`, scaled by
/// [`w_soil`].
///
/// Two things differ from [`crate::present::draw_ramp_field`] on purpose. The brightness
/// is linear in `t` rather than squared, because soil is ground rather than a highlight
/// that should only appear when rich. And `t = 0` still paints: bare soil is the dark
/// plum at [`SOIL_MIN_BRIGHTNESS`], which is what makes the band read as a *place* below
/// the horizon instead of as an unlit strip. The seam-aware one-pixel box filter is
/// exactly `cubarium_render::draw_field`'s, rim normalization included.
fn draw_soil_ground(canvas: &mut Canvas, detritus: &ScalarField) {
    let (low, high) = *SOIL_RAMP;
    let weight = &*SOIL_WEIGHT;
    for face in Face::ALL {
        for y in 0..FACE_SIZE as u8 {
            for x in 0..FACE_SIZE as u8 {
                let w = weight[weight_index(face, x, y)];
                if w <= 0.0 {
                    continue;
                }
                let t = filtered_at(detritus, face, x, y) / SOIL_SCALE;
                // A `NaN` cell paints bare soil rather than nothing or a panic.
                let t = if t.is_nan() { 0.0 } else { t.clamp(0.0, 1.0) as f32 };
                let c = present::mix(low, high, t);
                let b = (SOIL_MIN_BRIGHTNESS + (SOIL_MAX_BRIGHTNESS - SOIL_MIN_BRIGHTNESS) * t) * w;
                canvas.add(face, x, y, [c[0] * b, c[1] * b, c[2] * b]);
            }
        }
    }
}

/// Compile-time reminder that the slot table is one entry per field cell.
const _: () = assert!(CELL_COUNT == 1280);

#[cfg(test)]
mod tests {
    use super::*;
    use crate::present::Presenter;
    use cube_proto::Face;
    use std::path::Path;

    fn pack() -> ArtPack {
        ArtPack::load(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/atelier"))
            .expect("the baked art pack")
    }

    fn organism(slot: u32, hue: f32, mode: Mode) -> OrganismView {
        OrganismView {
            id: OrganismId { slot, generation: 1 },
            pos: SurfacePoint::new(Face::Front, 32.0, 32.0),
            heading: Vec2::new(1.0, 0.0),
            lobes: vec![(0.0, 0.0, 1.4), (2.0, 0.0, 0.9)],
            hue,
            mode,
            fed: false,
            juvenile: false,
            gestation: None,
            // No `form`: the hue tercile decides, as these fixtures were written to.
            form: u8::MAX,
            moved: Vec::new(),
        }
    }

    fn empty_view() -> RenderView {
        RenderView {
            tick: 0,
            producer: vec![0.0; CELL_COUNT],
            detritus: vec![0.0; CELL_COUNT],
            fruit: vec![0.0; CELL_COUNT],
            water: vec![0.0; CELL_COUNT],
            rain: vec![0.0; CELL_COUNT],
            producer_max: 2.0,
            organisms: Vec::new(),
        }
    }

    /// Every pixel with no soil in it, compared exactly. The M2 image says nothing about
    /// the soil band, so comparisons against it are only meaningful above the horizon.
    fn identical_above_horizon(a: &Canvas, b: &Canvas) -> bool {
        Face::ALL.into_iter().all(|face| {
            (0..64u8).all(|y| {
                (0..64u8).all(|x| {
                    soil_weight(face, x, y) > 0.0 || a.get(face, x, y) == b.get(face, x, y)
                })
            })
        })
    }

    /// The art image of a view with no organisms: floor, both grounds, and whatever
    /// plants its fields grow.
    fn art_ground(view: &RenderView) -> Canvas {
        let mut canvas = Canvas::new();
        ArtPresenter::new(pack()).draw(view, 0.0, &mut canvas);
        canvas
    }

    /// Pixels of `after` that differ from `before`, as (face, x, y).
    fn added(before: &Canvas, after: &Canvas) -> Vec<(Face, u8, u8)> {
        let mut out = Vec::new();
        for face in Face::ALL {
            for y in 0..64u8 {
                for x in 0..64u8 {
                    if before.get(face, x, y) != after.get(face, x, y) {
                        out.push((face, x, y));
                    }
                }
            }
        }
        out
    }

    #[test]
    fn the_form_is_the_hue_tercile_and_never_leaves_the_three_rigs() {
        assert_eq!(form_of(0.0), 0);
        // The 0/1 boundary is exactly 1/3: just below is rig 0, at and above is rig 1.
        assert_eq!(form_of(0.333), 0);
        assert_eq!(form_of(0.34), 1);
        assert_eq!(form_of(0.99), 2);
        assert_eq!(form_of(1.0), 2, "min(2, floor(3)) is 2, not an out-of-range rig");
        assert_eq!(form_of(f32::NAN), 0);
        // Values the genome does not produce still land on a real rig.
        assert_eq!(form_of(-0.5), 0);
        assert_eq!(form_of(5.0), 2);
        assert_eq!(form_of(f32::INFINITY), 2);
        assert_eq!(form_of(f32::NEG_INFINITY), 0);
    }

    #[test]
    fn the_form_gene_names_the_rig_and_the_hue_is_only_the_fallback() {
        // In range: the gene wins whatever the hue says.
        assert_eq!(rig_of(3, 0.0, 4), 3);
        assert_eq!(rig_of(0, 0.99, 4), 0);
        // Out of range, including the v1 marker: the hue tercile, clamped into the pack.
        assert_eq!(rig_of(u8::MAX, 0.5, 4), 1);
        assert_eq!(rig_of(7, 0.99, 4), 2);
        assert_eq!(rig_of(4, 0.99, 2), 1, "a two-rig pack clamps tercile 2 to rig 1");
        assert_eq!(rig_of(0, 0.0, 0), 0, "an empty count never divides or indexes by zero");
    }

    #[test]
    fn a_body_is_drawn_with_the_rig_its_form_names() {
        let art = pack();
        let count = art.creature_count();
        assert!(count >= 4, "the pack has four rigs");
        let mut view = empty_view();
        let mut o = organism(0, 0.0, Mode::Resting);
        o.form = 3;
        view.organisms = vec![o.clone()];
        let mut presenter = ArtPresenter::new(pack());
        let mut drawn = Canvas::new();
        presenter.observe(&view);
        presenter.draw(&view, 0.0, &mut drawn);

        // The same frame stamped by hand with rig 3's rest clip on the same ground.
        let mut expected = Canvas::new();
        let mut bare = view.clone();
        bare.organisms.clear();
        presenter.observe(&bare);
        presenter.draw(&bare, 0.0, &mut expected);
        let clip = &art.clips[3 * 4];
        let sprite = clip.at(clip_time(clip, 0, phase_of(o.id, clip.seconds), None));
        stamp_sprite(&mut expected, o.pos, o.heading, sprite, 1.0, 1.0, &mut vec![]);
        let same = |a: &Canvas, b: &Canvas| {
            Face::ALL.into_iter().all(|face| {
                (0..64u8).all(|y| (0..64u8).all(|x| a.get(face, x, y) == b.get(face, x, y)))
            })
        };
        assert!(same(&drawn, &expected), "form 3 must draw rig 3's rest frame");

        // With the hue-tercile fallback the body is rig 0 instead, so the images differ.
        let mut fallback = Canvas::new();
        view.organisms[0].form = u8::MAX;
        presenter.observe(&view);
        presenter.draw(&view, 0.0, &mut fallback);
        assert!(!same(&drawn, &fallback), "the fallback rig differs from rig 3");
    }

    #[test]
    fn gestation_beats_mode_and_every_mode_maps_to_its_clip() {
        for (mode, want) in
            [(Mode::Resting, 0), (Mode::Seeking, 1), (Mode::Feeding, 2)]
        {
            let mut o = organism(0, 0.0, mode);
            assert_eq!(state_of(&o), want, "{mode:?}");
            o.gestation = Some(0.0);
            assert_eq!(state_of(&o), 3, "a budding {mode:?} organism draws the bud clip");
        }
    }

    #[test]
    fn the_clip_phase_is_stable_per_id_bounded_and_not_shared() {
        let a = OrganismId { slot: 3, generation: 1 };
        let b = OrganismId { slot: 4, generation: 1 };
        let recycled = OrganismId { slot: 3, generation: 2 };
        assert_eq!(phase_of(a, 2.0), phase_of(a, 2.0), "the phase is a pure hash");
        assert_ne!(phase_of(a, 2.0), phase_of(b, 2.0), "two slots must not animate together");
        assert_ne!(phase_of(a, 2.0), phase_of(recycled, 2.0), "a reused slot is a new body");
        for slot in 0..256u32 {
            let p = phase_of(OrganismId { slot, generation: 7 }, 2.5);
            assert!((0.0..2.5).contains(&p), "slot {slot}: {p}");
        }
        assert_eq!(phase_of(a, 0.0), 0.0);
        assert_eq!(phase_of(a, f64::NAN), 0.0);
    }

    #[test]
    fn the_bud_clip_follows_gestation_and_looping_clips_follow_simulated_time() {
        let art = pack();
        let bud = &art.clips[3];
        assert!(!bud.looping);
        assert_eq!(clip_time(bud, 0, 0.0, Some(0.0)), 0.0);
        assert_eq!(clip_time(bud, 12_345, 1.7, Some(0.5)), 0.5 * bud.seconds);
        assert_eq!(clip_time(bud, 12_345, 1.7, Some(1.0)), bud.seconds);
        // Wall time is nowhere in it: the tick and the phase are the only inputs.
        let walk = &art.clips[1];
        assert!(walk.looping);
        assert_eq!(clip_time(walk, 0, 0.25, None), 0.25);
        assert_eq!(clip_time(walk, 20, 0.25, None), 20.0 * DT + 0.25);
        // Gestation is ignored by a looping clip, and a doubled tick rate doubles the
        // clip's progress, which is what makes `--speed` honest.
        assert_eq!(clip_time(walk, 40, 0.0, Some(0.5)), 40.0 * DT);
    }

    #[test]
    fn a_quiet_world_draws_the_m2_image_above_the_horizon() {
        // No organisms, and every cell below its band's first stage threshold: above the
        // horizon the art image is then the decided M2 image with nothing added. Below
        // it the soil ground replaces the producer ramp and the flecks, which is the
        // whole point of the band and is checked in `tests/art_bands.rs`.
        let mut view = empty_view();
        let saturation = view.producer_max * PRODUCER_SATURATION;
        // Under the lowest threshold anywhere on the cube, which is the canopy's, and
        // with every soil cell under the soil's first threshold too.
        for (i, v) in view.producer.iter_mut().enumerate() {
            *v = saturation * CANOPY_STAGES[0] * (i % 7) as f64 / 7.0;
        }
        for (i, v) in view.detritus.iter_mut().enumerate() {
            *v = SOIL_SCALE * SOIL_STAGES[0] * (i % 5) as f64 / 5.0;
        }
        let mut plain = Canvas::new();
        let mut art = Canvas::new();
        Presenter::new().draw(&view, 0.0, &mut plain);
        let mut presenter = ArtPresenter::new(pack());
        presenter.observe(&view);
        presenter.draw(&view, 0.0, &mut art);
        assert!(
            identical_above_horizon(&plain, &art),
            "the art image added something below every threshold"
        );
        // Non-vacuity: the soil band really is a different image.
        assert!(
            !added(&plain, &art).is_empty(),
            "the soil band drew nothing at all, so the comparison above proves nothing"
        );

        // And a cell exactly at its band's first threshold is still bare: the rule is
        // strict in every band.
        for (cell, v) in CellId::all().zip(view.producer.iter_mut()) {
            *v = saturation * stage_thresholds(band_of(cell))[0];
        }
        Presenter::new().draw(&view, 0.0, &mut plain);
        let mut presenter = ArtPresenter::new(pack());
        presenter.observe(&view);
        presenter.draw(&view, 0.0, &mut art);
        assert!(
            identical_above_horizon(&plain, &art),
            "a cell exactly at its first threshold grew a plant"
        );
        for cell in CellId::all() {
            assert_eq!(presenter.stage_of(cell), None, "{cell:?} is not bare");
        }
    }

    #[test]
    fn each_state_draws_a_body_near_the_organism_and_bud_ends_on_the_last_frame() {
        let art = pack();
        for (mode, gestation) in [
            (Mode::Resting, None),
            (Mode::Seeking, None),
            (Mode::Feeding, None),
            (Mode::Resting, Some(1.0f32)),
        ] {
            let mut view = empty_view();
            let mut o = organism(0, 0.5, mode);
            o.gestation = gestation;
            view.organisms = vec![o];
            let mut canvas = Canvas::new();
            // The floor, the (empty) fields and the bare soil band alone, to subtract.
            let plain = art_ground(&empty_view());
            ArtPresenter::new(pack()).draw(&view, 0.0, &mut canvas);
            let lit = added(&plain, &canvas);
            assert!(!lit.is_empty(), "{mode:?}/{gestation:?} drew no body");
            for &(f, x, y) in &lit {
                assert_eq!(f, Face::Front, "{mode:?}: a body leaked onto {f:?}");
                let d = (f64::from(x) - 32.0).hypot(f64::from(y) - 32.0);
                assert!(d < 10.0, "{mode:?}: lit {x},{y}, {d:.1} px from the body");
            }
        }

        // A gestation of 1.0 is the bud clip's last frame, exactly.
        let bud = &art.clips[form_of(0.5) * 4 + 3];
        let at_birth = clip_time(bud, 0, 0.0, Some(1.0));
        assert!(std::ptr::eq(bud.at(at_birth), bud.frames.last().unwrap()));
        assert!(std::ptr::eq(
            bud.at(at_birth),
            art.creature(form_of(0.5), 3, bud.seconds),
        ));
        // And a gestation of 0 is its first frame, so the clip really does run.
        assert!(std::ptr::eq(bud.at(clip_time(bud, 0, 0.0, Some(0.0))), &bud.frames[0]));
    }

    #[test]
    fn a_juvenile_is_the_same_clip_drawn_smaller() {
        let mut view = empty_view();
        let mut o = organism(0, 0.9, Mode::Seeking);
        o.juvenile = true;
        view.organisms = vec![o.clone()];
        let mut small = Canvas::new();
        ArtPresenter::new(pack()).draw(&view, 0.0, &mut small);

        o.juvenile = false;
        view.organisms = vec![o];
        let mut full = Canvas::new();
        ArtPresenter::new(pack()).draw(&view, 0.0, &mut full);

        let floor = art_ground(&empty_view());
        let (a, b) = (added(&floor, &small).len(), added(&floor, &full).len());
        assert!(a > 0 && a < b, "a juvenile lit {a} pixels, an adult {b}");
        assert_eq!(JUVENILE_SCALE, 0.7);
    }

    #[test]
    fn the_stage_rule_rises_strictly_falls_with_hysteresis_and_respects_the_cap() {
        let th = FOLIAGE_STAGES;
        assert_eq!(next_stage(None, 0.0, &th, 2), None);
        assert_eq!(next_stage(None, th[0], &th, 2), None, "rising is strict");
        assert_eq!(next_stage(None, th[0] + 1e-9, &th, 2), Some(0));
        assert_eq!(next_stage(None, th[2] + 0.01, &th, 2), Some(2), "rises all the way at once");
        assert_eq!(next_stage(Some(2), th[2] - STAGE_HYST / 2.0, &th, 2), Some(2), "held");
        assert_eq!(next_stage(Some(2), th[2] - STAGE_HYST - 1e-9, &th, 2), Some(1), "falls");
        assert_eq!(next_stage(Some(2), 0.0, &th, 2), None, "falls all the way at once");
        assert_eq!(next_stage(None, 1.0, &th, 0), Some(0), "a cap of 0 is a sprout at most");
        assert_eq!(next_stage(Some(2), 1.0, &th, 1), Some(1), "the cap clips a held stage");
        assert_eq!(next_stage(Some(1), f64::NAN, &th, 2), None);
        // Idempotent: re-applying to the same density changes nothing.
        for t in [0.0, 0.2, 0.26, 0.44, 0.5, 0.69, 0.71, 1.0] {
            for cur in [None, Some(0), Some(1), Some(2)] {
                let once = next_stage(cur, t, &th, 2);
                assert_eq!(next_stage(once, t, &th, 2), once, "t {t} from {cur:?}");
            }
        }
    }

    /// Not a correctness test: the number the brief asks for. Run with
    /// `cargo test --release -p cubarium -- --ignored plant_and_body_draw_cost`.
    #[test]
    #[ignore = "timing, not behaviour"]
    fn plant_and_body_draw_cost() {
        let mut view = empty_view();
        // Every cell rich in every band, so every slot grows to its cap.
        for v in view.producer.iter_mut() {
            *v = view.producer_max;
        }
        for v in view.detritus.iter_mut() {
            *v = SOIL_SCALE;
        }
        let mut rng = SplitMix64::new(11);
        view.organisms = (0..200u32)
            .map(|slot| {
                let mut o = organism(slot, rng.next_f64() as f32, Mode::Seeking);
                let face = cube_proto::Face::ALL[(slot as usize) % 5];
                o.pos = SurfacePoint::new(face, rng.range(1.0, 63.0), rng.range(1.0, 63.0));
                o.juvenile = slot % 3 == 0;
                o.gestation = (slot % 5 == 0).then_some(0.5);
                o
            })
            .collect();

        let mut presenter = ArtPresenter::new(pack());
        let mut canvas = Canvas::new();
        presenter.observe(&view);
        // Warm the caches, then time a run of frames.
        for _ in 0..5 {
            presenter.draw(&view, 0.0, &mut canvas);
        }
        let grown = CellId::all().filter(|&c| presenter.stage_of(c).is_some()).count();
        let frames = 60;
        let t0 = std::time::Instant::now();
        for i in 0..frames {
            view.tick = i;
            presenter.draw(&view, 0.5, &mut canvas);
        }
        let per = t0.elapsed().as_secs_f64() / frames as f64;
        println!(
            "ArtPresenter::draw: {:.3} ms/frame ({grown} plants, 200 organisms) = {:.0}% of a 60 fps budget",
            per * 1e3,
            per / (1.0 / 60.0) * 100.0
        );
        assert!(per < 1.0 / 60.0, "draw took {:.3} ms, past the whole 60 fps budget", per * 1e3);
    }
}
