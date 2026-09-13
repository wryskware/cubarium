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
//! Nothing here is wall-clock driven. Presentation time is [`present_seconds`]`(tick, f)`
//! — the *simulated* instant the frame shows, `(tick − 1 + f) · DT`, which is the same
//! interval the bodies are interpolated along — and every looping clip, the water shimmer,
//! the ground breath and the rain read it. So `--speed 8` animates eight times faster, a
//! paused world holds its pose, and the motion is continuous across a tick boundary
//! instead of stepping once per tick. The bud clip still advances on the organism's own
//! gestation progress.
//!
//! Growth is *paced*, and paced only in [`ArtPresenter::observe`]: one call per completed
//! tick moves each cell's [`Growth`] and each column's [`TallGrowth`] toward the target
//! its field warrants ([`advance_growth`], [`advance_tall`]), by the simulated time the
//! tick took. The first view, or a view whose tick went backwards, *snaps* instead, so a
//! mature world is not replayed from bare ground. [`ArtPresenter::draw`] is then a pure
//! function of (presenter state, view, `f`): it snap-initializes if the presenter has
//! never been observed and after that mutates nothing, so two draws of the same inputs
//! give the same image and a thousand draws advance nothing.

use std::sync::LazyLock;

use cube_proto::{FACE_SIZE, Face};
use cubarium_core::OrganismId;
use cubarium_core::hunter::{FixedHunterProfile, HunterEvent, HunterPhase, HunterView};
use cubarium_core::organism::Mode;
use cubarium_core::view::{OrganismView, RenderView};
use cubarium_render::{
    Bend, Canvas, Mask, Pose, draw_field, stamp_layers, stamp_layers_bent, stamp_pose,
};
use cubarium_surface::{
    CELL_COUNT, CELLS_PER_FACE_EDGE, CellId, Edge, PixelImage, ScalarField, SurfacePoint, Vec2, cell_of,
    pixel_neighbor,
};

pub use crate::art::Band;
use crate::art::{ArtPack, Clip, GroundTile, Plant, TallPlant};
use crate::clock::DT;
use crate::hunter_present::{HunterFrame, HunterMemory, validate_profile, validate_view};
use crate::lanternjaw::{Lanternjaw, Part};
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

// --- Pacing constants --------------------------------------------------------------
//
// How fast the *visual* follows the field: the fields decide what a cell warrants, these
// decide how long the picture takes to get there. Every one of them is review-tunable
// from a viewing session and none is visible to the simulation.

/// Simulated seconds one stage step upward takes ([`advance_growth`]). Review-tunable.
pub const STAGE_GROW_SECONDS: f64 = 4.0;
/// Simulated seconds one stage step downward takes: wilting is quicker than growing.
/// Review-tunable.
pub const STAGE_WILT_SECONDS: f64 = 2.0;
/// Simulated seconds the fruit accent takes to fade in once a full-grown plant's cell holds
/// fruit. It leaves *at once* when the fruit does: the accent is the food signal, so it
/// never shows food the world no longer holds (a bite is an event, and reads as one).
/// Review-tunable.
pub const FRUIT_FADE_SECONDS: f64 = 2.0;
/// How fast a tall column extends, in face pixels per simulated second (a segment is 4 px,
/// so a segment takes `4 / TALL_GROW_PX_PER_S` seconds). Review-tunable.
pub const TALL_GROW_PX_PER_S: f64 = 1.5;
/// How fast a tall column declines, in face pixels per simulated second. Review-tunable.
pub const TALL_WILT_PX_PER_S: f64 = 3.0;
/// Segments over which a column's base, cap and vine fade in from bare ground, so a new
/// column does not pop its ends in whole. Review-tunable.
pub const TALL_BASE_FADE: f64 = 0.25;
/// Opacity of a tall column's parts. Every row of a column is composited exactly once
/// (each tile paints only the rows it owns, see [`tall_grown_px`]), so this is the density
/// the trunk actually shows; before single ownership the twelve-row overlaps stacked three
/// stamps of [`MOTIF_OPACITY`] and read as nearly opaque, which this keeps close to.
/// Review-tunable.
pub const TALL_OPACITY: f32 = 0.95;
/// Rows of a 16-row tile.
pub const TILE_ROWS: f64 = 16.0;
/// The tile row (counted from the bottom edge, in tile pixels) from which a trunk tile
/// above another trunk owns its rows: the trunk is 4-periodic and the tile below already
/// paints everything under this line. Review-tunable only together with the art.
pub const TALL_JOIN: f64 = 12.0;
/// [`TALL_JOIN`] for the first trunk, which stands over the base whose join rows reach
/// 5 px above the horizon center (the base paints the trunk pattern up to there).
pub const TALL_FIRST_JOIN: f64 = 9.0;
/// The rows (from the bottom) a vine tile owns: vine tiles sit on every second trunk
/// position, eight px apart, so eight rows each tile the column without a gap or an
/// overlap; the topmost vine tile owns up to the tile's top instead.
pub const TALL_VINE_FLOOR: f64 = 4.0;
pub const TALL_VINE_TOP: f64 = 12.0;
/// The [`Mask::Axial`] reveal at which a 16-row tile is wholly shown (16 rows plus the
/// mask's half-pixel ramp). Review-tunable.
pub const PLANT_REVEAL_PX: f64 = 16.5;
/// Simulated seconds of cross-clip fade when a body changes state. Review-tunable.
pub const BODY_FADE_SECONDS: f64 = 0.3;
/// Cap, in simulated seconds, on how much one [`ArtPresenter::observe`] may advance the
/// growth: a host that fell behind catches its fields up in one tick without fast-forwarding
/// every plant through a visible jump. Review-tunable.
pub const MAX_STEP_SECONDS: f64 = 1.0;

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
/// 0.55 made the fully charged starting litter (D ≈ 1.2 on the floor) a bright mauve
/// carpet in the viewer; 0.32 keeps it dark ground that richer litter warms.
pub const SOIL_MAX_BRIGHTNESS: f32 = 0.32;

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
    /// This slot's own share of the shared breeze, in `[1 − `[`WIND_SLOT_VARIATION`]`, 1 +
    /// WIND_SLOT_VARIATION)`: the amplitude of every stamp of this slot is the family's
    /// admitted amplitude ([`effective_tip`]) times this. Fixed for the life of the
    /// presenter, and a *scale*, never a phase.
    pub wind: f64,
}

/// The fixed slot of a cell.
///
/// **Normative**: one `SplitMix64` stream seeded from the cell index, consumed in the
/// order pick, jitter `u`, jitter `v`, free heading angle, rank, heading jitter, wind
/// variation — the same order whatever band the cell is in, so moving the horizon or
/// flooding a cell never moves the plants that stay, and the wind draw is *appended* so
/// that adding it moved no plant at all. The jitter keeps the anchor within ±1 px of the
/// cell center, which is 2 px from every cell edge, so a plant never anchors in a
/// neighbouring cell. Rank: `cap = 2` if `r < `[`RANK_FULL`], `1` if `r < `[`RANK_MID`],
/// else `0`. Heading: [`stalk_heading`] of [`up_of`] on a side face, plus the jitter; free
/// on top. Wind: uniform in `[1 − `[`WIND_SLOT_VARIATION`]`, 1 + WIND_SLOT_VARIATION)`.
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
    let wind = hash.range(1.0 - WIND_SLOT_VARIATION, 1.0 + WIND_SLOT_VARIATION);
    let rank_cap = if rank < RANK_FULL {
        2
    } else if rank < RANK_MID {
        1
    } else {
        0
    };
    Slot { at, heading, pick, rank_cap, wind }
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
/// **Normative**: a sprout-only slot (rank cap 0, about half of all cells) grows nothing
/// in any band. Drawing a stage-0 sprout in every such slot turned the foliage into a
/// field of magenta speckles that swallowed the rain and the creatures, and lined a
/// flooded floor with a cyan fence of reeds; leaving those slots bare gives the wall
/// breathing room and lets ground cover carry the fill. Slots with cap 1 or 2 keep their
/// cap. Review-tunable by changing this rule.
pub fn plant_cap(_band: Band, cell: CellId) -> Option<u8> {
    match rank_cap_of(cell) {
        0 => None,
        cap => Some(cap),
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

/// The simulated instant a frame shows: `(tick − 1 + clamp(f, 0, 1)) · DT`.
///
/// **Normative**, and the one presentation-time helper in this module: every looping clip,
/// the water shimmer, the ground breath and the rain read it. The interval is `tick − 1 →
/// tick` because that is the path the bodies are interpolated along, so the whole scene
/// shares one clock; it is continuous at every tick boundary (`f → 1` of tick `T` meets
/// `f = 0` of `T + 1`); it never reads wall time, `--speed` scales it because the ticks
/// do, and a paused world holds it. Tick 0 has no completed interval behind it and reads 0
/// whatever `f` is (so the clock does not run through tick 0 and start over at tick 1);
/// tick 1 at `f = 0` is also 0. A non-finite `f` reads as 0.
pub fn present_seconds(tick: u64, f: f64) -> f64 {
    if tick == 0 {
        return 0.0;
    }
    let f = if f.is_finite() { f.clamp(0.0, 1.0) } else { 0.0 };
    ((tick - 1) as f64 + f) * DT
}

/// Where in a clip to sample this frame, from the presentation seconds of
/// [`present_seconds`].
///
/// **Normative**: the non-looping `bud` clip is driven by the organism's own gestation
/// progress — `progress × clip.seconds`, so the clip's last frame lands exactly when the
/// world commits the birth. Every looping clip is driven by *simulated* time, `seconds +
/// phase`, never by wall time: `--speed` and pauses then stay honest, and two frames of
/// the same (tick, `f`) show the same pose.
///
/// A non-looping clip with no gestation cannot arise from [`state_of`]; it yields 0.
pub fn clip_time(clip: &Clip, seconds: f64, phase: f64, gestation: Option<f32>) -> f64 {
    if clip.looping {
        return seconds + phase;
    }
    match gestation {
        Some(progress) => f64::from(progress) * clip.seconds,
        None => 0.0,
    }
}

// --- Wind ----------------------------------------------------------------------------
//
// One shared breeze, a pure function of presentation seconds and position: no simulated
// weather, no wall clock, no state. It is evaluated **once per plant slot and once per tall
// column per frame** — never per destination pixel, where the only work is the bend's own
// `D`. Every constant here is a presentation choice, review-tunable from a viewing session,
// and none of them is visible to the simulation.
//
// The shape is a *packet*: a rising gust, a while of mild movement carrying a flutter, a
// fall, and then a stretch of exact calm. The calm is the point. A cube that sways all the
// time reads as a machine; a cube that rests and then stirs reads as weather.

/// Seconds from the start of one wind packet to the start of the next. Review-tunable.
pub const WIND_PERIOD: f64 = 30.0;
/// Seconds the packet eases in over, with zero slope at the start. Review-tunable.
pub const WIND_RISE: f64 = 5.0;
/// Seconds the packet holds at full envelope, carrying the flutter. Review-tunable.
pub const WIND_HOLD: f64 = 8.0;
/// Seconds the packet eases out over, with zero slope at the end. Review-tunable.
pub const WIND_FALL: f64 = 5.0;
/// How deeply the flutter dips the packet: the flutter factor runs over
/// `[1 − WIND_FLUTTER, 1]`. Review-tunable.
pub const WIND_FLUTTER: f64 = 0.3;
/// Period of the flutter in simulated seconds — the breath inside a gust. Review-tunable.
pub const WIND_FLUTTER_SECONDS: f64 = 2.3;
/// Period of the slow secondary modulation of a packet's peak, chosen well away from a
/// multiple of [`WIND_PERIOD`] so consecutive packets are not the same gust twice.
/// Review-tunable.
pub const WIND_PEAK_SECONDS: f64 = 97.0;
/// How far the secondary modulation may pull a packet's peak *down* from 1: the peak runs
/// over `[1 − WIND_PEAK_VARY, 1]`. It never pulls it up, because 1 is the strength the
/// measured amplitude budgets are sized for. Review-tunable.
pub const WIND_PEAK_VARY: f64 = 0.15;
/// Seconds of lag per unit of the embedded spatial phase, so the gust crosses the cube
/// instead of arriving everywhere at once. Review-tunable.
pub const WIND_TRAVEL_SECONDS: f64 = 0.6;
/// The largest `|`[`wind_chart`]`|` anywhere on the surface. Exactly 1: on a side face
/// `|W| = 1 − a²`, largest at the chart's vertical center line, and on Top `|W|² = b²(1 −
/// a²)² + a²(1 − b²)²`, which reaches 1 at the middle of each edge and less everywhere
/// else. It is the divisor that turns a chart magnitude into a fraction of full wind.
pub const WIND_CHART_MAX: f64 = 1.0;

/// Seconds of exact calm at the end of every packet: [`WIND_PERIOD`] less the rise, hold
/// and fall. Negative constants would mean a packet that never rests, which
/// [`wind_strength`] would clip rather than honour.
pub const WIND_QUIET_SECONDS: f64 = WIND_PERIOD - WIND_RISE - WIND_HOLD - WIND_FALL;

/// The clamped Hermite smoothstep `t²(3 − 2t)` on `[0, 1]`, with `NaN` mapped to 0.
fn hermite(t: f64) -> f64 {
    let t = if t.is_nan() { 0.0 } else { t.clamp(0.0, 1.0) };
    t * t * (3.0 - 2.0 * t)
}

/// How hard the shared breeze blows at a presentation instant ([`present_seconds`]), in
/// `[0, 1]`.
///
/// **Normative**. With `u = seconds mod `[`WIND_PERIOD`] the *envelope* is
///
/// * `smoothstep(u / `[`WIND_RISE`]`)` while `u < WIND_RISE`,
/// * 1 while `u < WIND_RISE + `[`WIND_HOLD`],
/// * `1 − smoothstep((u − WIND_RISE − WIND_HOLD) / `[`WIND_FALL`]`)` while `u <
///   WIND_RISE + WIND_HOLD + WIND_FALL`,
/// * and **exactly 0** for the remaining [`WIND_QUIET_SECONDS`] of the period,
///
/// with `smoothstep` the Hermite `t²(3 − 2t)`, so the value *and the slope* are 0 at both
/// edges of the packet and the gust neither starts nor stops with a jerk. The envelope is
/// multiplied by two slow factors, each running over `[1 − v, 1]` and each a pure function
/// of the same clock:
///
/// * a flutter, `1 − `[`WIND_FLUTTER`]` + WIND_FLUTTER · ½ · (1 + sin(2π · seconds /
///   `[`WIND_FLUTTER_SECONDS`]`))` — the breath inside a gust, which only shows where the
///   envelope is 1 but multiplies the whole packet so no edge gains a step;
/// * a peak modulation on [`WIND_PEAK_SECONDS`] with depth [`WIND_PEAK_VARY`], written the
///   same way, so consecutive packets differ and the cube does not tick like a metronome.
///
/// The result is therefore in `[0, 1]` and reaches 1 only when both slow factors are at
/// their own maxima: 1 is the strength every measured amplitude budget is sized for, which
/// is why neither factor is allowed to exceed it. A non-finite time is 0, and so is the
/// whole quiet interval — **exactly** 0, so that a resting cube draws the windless image
/// bit for bit and at the windless cost.
pub fn wind_strength(seconds: f64) -> f64 {
    if !seconds.is_finite() {
        return 0.0;
    }
    let u = seconds.rem_euclid(WIND_PERIOD);
    let envelope = if u < WIND_RISE {
        hermite(u / WIND_RISE)
    } else if u < WIND_RISE + WIND_HOLD {
        1.0
    } else if u < WIND_RISE + WIND_HOLD + WIND_FALL {
        1.0 - hermite((u - WIND_RISE - WIND_HOLD) / WIND_FALL)
    } else {
        return 0.0;
    };
    if envelope <= 0.0 {
        return 0.0;
    }
    let wave = |period: f64, depth: f64| {
        1.0 - depth + depth * 0.5 * (1.0 + (std::f64::consts::TAU * seconds / period).sin())
    };
    let strength = envelope
        * wave(WIND_FLUTTER_SECONDS, WIND_FLUTTER)
        * wave(WIND_PEAK_SECONDS, WIND_PEAK_VARY);
    if strength.is_finite() { strength.clamp(0.0, 1.0) } else { 0.0 }
}

/// The direction the breeze blows at a chart position, as an **unnormalized** chart
/// tangent vector whose magnitude is how much of the full breeze reaches there.
///
/// **Normative** (Astra's seam-compatible circulation). With `a = u/32 − 1` and `b = v/32 −
/// 1`, the four side faces carry `W = (−(1 − a²), 0)` and Top carries `W = (−b(1 − a²),
/// a(1 − b²))`. A non-finite coordinate yields [`Vec2::ZERO`].
///
/// This field is chosen so that it **joins across every seam under the real tangent
/// transport**: at each side/Top seam the side face's vector, rotated by that seam's quarter
/// turns, is the Top vector at the same surface point, and at every side/side seam (`a =
/// ±1`) and at each of Top's four vertices it is exactly zero, so there is nothing to
/// disagree about. Top's center is calm as well. Those bounded calm regions are deliberate:
/// they are what a continuous circulation on a cube must have, and they are much better
/// than a direction that jumps at a seam. A naive 3D swirl projected face by face does not
/// join — on Top it keeps an edge-normal component the adjacent side face does not have.
///
/// It is never normalized and no per-face phase is seeded: both would break the join.
pub fn wind_chart(face: Face, u: f64, v: f64) -> Vec2 {
    if !(u.is_finite() && v.is_finite()) {
        return Vec2::ZERO;
    }
    let a = u / 32.0 - 1.0;
    let b = v / 32.0 - 1.0;
    if face == Face::Top {
        Vec2::new(-b * (1.0 - a * a), a * (1.0 - b * b))
    } else {
        Vec2::new(-(1.0 - a * a), 0.0)
    }
}

/// The spatial phase of a point, in `[-1, 1]`: `0.5 · (x + z)` of its embedded position
/// ([`SurfacePoint::embed`]).
///
/// Embedded coordinates directly, with a small coefficient, so the gust sweeps across the
/// cube as one front. There is deliberately no angle anywhere in it: an angular phase would
/// put a branch cut somewhere on the surface, and the plants either side of that cut would
/// lean in opposite directions.
pub fn wind_phase(point: SurfacePoint) -> f64 {
    let p = point.embed();
    let phase = 0.5 * (p[0] + p[2]);
    if phase.is_finite() { phase } else { 0.0 }
}

/// The breeze at a surface point at a presentation instant, for a part whose response lags
/// by `lag` seconds.
///
/// **Normative**: [`wind_chart`]`(point) · `[`wind_strength`]`(seconds − lag −
/// `[`WIND_TRAVEL_SECONDS`]` · `[`wind_phase`]`(point))`. A non-finite `lag` reads as 0. The
/// magnitude is therefore at most [`WIND_CHART_MAX`], and it is exactly [`Vec2::ZERO`]
/// wherever the chart field is calm and through the *shared* part of every quiet interval:
/// the spatial delay and the lag shift each root's clock by at most `WIND_TRAVEL_SECONDS +
/// lag`, so the interval in which every root on the cube is calm at once is
/// `WIND_QUIET_SECONDS − 2 · (WIND_TRAVEL_SECONDS + lag)` long, and a root at the edge of
/// the cube can still feel the very end of a packet's fall a fraction of a second after
/// the global sampler has gone quiet.
///
/// All structural parts of one plant share one sample: a column takes a single sample at its
/// base anchor for base, trunk, cap and vine, and a small plant one at its root. Sampling
/// per tile or per pixel would split one plant's motion at a tile join or a seam.
pub fn wind_at(point: SurfacePoint, seconds: f64, lag: f64) -> Vec2 {
    let lag = if lag.is_finite() { lag } else { 0.0 };
    let when = seconds - lag - WIND_TRAVEL_SECONDS * wind_phase(point);
    wind_chart(point.face, point.u, point.v) * wind_strength(when)
}

/// How one asset answers the shared breeze. Stiffness is expressed as displacement, not as
/// a different gust: every plant feels the same wind.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WindResponse {
    /// Desired tip travel in tile pixels at full wind, before the measured headroom caps
    /// it ([`effective_tip`]). 0 is an asset that does not move.
    pub tip_px: f64,
    /// Seconds this asset's whole structure lags the breeze — a soft head answers late.
    /// Every part of one plant shares it, so nothing inside a plant slides against itself.
    pub lag_seconds: f64,
    /// Degrees a *radial* (top-face) plant rotates about its stationary center at full
    /// wind, instead of bending. 0 for everything that bends.
    pub spin_deg: f64,
}

impl WindResponse {
    /// An asset the breeze does not move.
    pub const STILL: WindResponse = WindResponse { tip_px: 0.0, lag_seconds: 0.0, spin_deg: 0.0 };
}

/// The per-species wind response, by asset name. Review-tunable from a viewing session:
/// these are artistic starting points measured against what the cube actually shows, not
/// promises, and the measured headroom of the shipped pack may cap any of them
/// ([`ArtPresenter::bend_budget`]).
///
/// The two canopy species rotate rather than bend ([`canopy_heading`]): they are radial, a
/// horizontal shear would read as a smear, and a whole-plant translation would detach them
/// from the ground. `vinecoil` carries no response of its own — a vine shares its host
/// column's amplitude exactly, or it would slide against the trunk.
pub const WIND_RESPONSE: [(&str, WindResponse); 10] = [
    ("lanternstalk", WindResponse { tip_px: 0.45, lag_seconds: 0.10, spin_deg: 0.0 }),
    ("tendrilfan", WindResponse { tip_px: 0.55, lag_seconds: 0.15, spin_deg: 0.0 }),
    ("reedspire", WindResponse { tip_px: 0.70, lag_seconds: 0.05, spin_deg: 0.0 }),
    ("glowcap", WindResponse { tip_px: 0.12, lag_seconds: 0.0, spin_deg: 0.0 }),
    ("rootveil", WindResponse::STILL),
    ("umbrellafrond", WindResponse { tip_px: 0.0, lag_seconds: 0.0, spin_deg: 2.0 }),
    ("bloomcrown", WindResponse { tip_px: 0.0, lag_seconds: 0.0, spin_deg: 1.5 }),
    ("spiretree", WindResponse { tip_px: 0.9, lag_seconds: 0.2, spin_deg: 0.0 }),
    ("glasscane", WindResponse { tip_px: 0.5, lag_seconds: 0.1, spin_deg: 0.0 }),
    ("vinecoil", WindResponse::STILL),
];

/// The wind response of an asset name; [`WindResponse::STILL`] for a name the table does
/// not list, so a new or renamed asset stands still until it is given a response.
pub fn wind_response(name: &str) -> WindResponse {
    match WIND_RESPONSE.iter().find(|(n, _)| *n == name) {
        Some((_, r)) => *r,
        None => WindResponse::STILL,
    }
}

/// How much per-slot variation the amplitude carries, as a fraction: each slot's own
/// amplitude is the family's times a hashed factor in `[1 − WIND_SLOT_VARIATION, 1 +
/// WIND_SLOT_VARIATION)`, so a patch of one species reads as many plants rather than one
/// object — with no per-plant *phase* offset, which would destroy the shared breeze.
/// Review-tunable.
pub const WIND_SLOT_VARIATION: f64 = 0.10;

/// The tip travel one **family** is admitted to bend by at full wind, in tile pixels, before
/// a slot's own variation multiplies it.
///
/// **Normative**: `min(tip_px, budget / (1 + `[`WIND_SLOT_VARIATION`]`))`, never negative,
/// with a non-finite `tip_px` or a `NaN` budget reading as 0 and `budget =
/// `[`f64::INFINITY`] meaning "nothing measured bounds this", which leaves the desired tip.
///
/// The budget is divided by the largest variation a slot can draw, so that
/// `effective_tip(..) · variation ≤ budget` for **every** variation the hash can produce:
/// the nine-pixel footprint is a hard bound, and the slot that happened to hash a `+10 %`
/// must not be the one that clips. Because `|`[`wind_at`]`| ≤ `[`WIND_CHART_MAX`]` = 1` and
/// the wind is projected onto a *unit* heading, the amplitude a stamp actually receives
/// ([`plant_bend`]) never exceeds the varied tip, and therefore never exceeds the budget.
pub fn effective_tip(tip_px: f64, budget: f64) -> f64 {
    let want = if tip_px.is_finite() { tip_px } else { 0.0 };
    let budget = if budget.is_nan() { 0.0 } else { budget };
    want.min(budget / (1.0 + WIND_SLOT_VARIATION)).max(0.0)
}

/// Height above the root line, in tile pixels, of the bottom edge of a tall column's tile
/// `i` — the [`Bend::base`] every part of a column shares: `4i − 8`.
///
/// The column's tiles are stamped 4 px apart with the pivot at the tile center, so tile `i`
/// spans heights `4i − 8` to `4i + 8` and the whole column, base to cap, is one continuous
/// height coordinate. The cap's `i` is the *fractional* `height + 1`, so it rides the same
/// curve as the trunk it sits on while it glides.
pub fn tall_bend_base(i: f64) -> f64 {
    4.0 * i - 8.0
}

/// Height above a small plant's root line, in tile pixels, below which nothing moves at all.
///
/// The plants of `PLANTS.md` share a root contact at tile `(8.5, 15)` whose lowest painted
/// row is tile row 14 — the center of that row is exactly 1.5 px above the tile's bottom
/// edge. The root sits exactly there, so the painted root row's displacement is *exactly*
/// zero and the contact pixel is bit-identical windy or calm: a root that moved by a
/// hundredth of a pixel would still resample and skate. Review-tunable, but not below 1.5
/// without accepting that.
pub const PLANT_BEND_ROOT: f64 = 1.5;
/// The fixed mature bend length of a small plant, in tile pixels: root to tip of a
/// full-grown 16-row tile. Fixed, not the current growth height, so a stage change does not
/// slide the stem sideways. Review-tunable.
pub const PLANT_BEND_LENGTH: f64 = 13.0;
/// A tall column's root height: the horizon contact itself, so everything the base tile
/// paints at or below the anchor line is fixed. Review-tunable.
pub const TALL_BEND_ROOT: f64 = 0.0;
/// The fixed mature bend length of a tall column, in pixels above the horizon: about the
/// height of a full column, so the cap of a tall tree reaches the whole amplitude and a
/// young one gives only a little. Review-tunable.
pub const TALL_BEND_LENGTH: f64 = 48.0;

/// The bend a small plant's stamps carry: `amplitude` from the wind, rooted at
/// [`PLANT_BEND_ROOT`] over [`PLANT_BEND_LENGTH`], with the tile standing on the root line.
///
/// **Normative**: `Bend { amplitude: tip · dot(w, heading), base: 0, root: PLANT_BEND_ROOT,
/// length: PLANT_BEND_LENGTH }` — the breeze projected onto the tile's own horizontal axis
/// (which carries the slot's orientation jitter with it), so a plant turned a few degrees
/// answers a little less than its neighbour. Every stamp of that slot in that frame — the
/// idle stage, the fruit blend, and both the fading lower and revealing upper stamp of a
/// growth step — takes this one bend.
pub fn plant_bend(tip: f64, w: Vec2, heading: Vec2) -> Bend {
    Bend {
        amplitude: tip * w.dot(heading),
        base: 0.0,
        root: PLANT_BEND_ROOT,
        length: PLANT_BEND_LENGTH,
    }
}

/// The heading a *radial* (top-face) plant is stamped with: its own heading turned by
/// `θ = deg · |w| / `[`WIND_CHART_MAX`] radians about the stationary tile center.
///
/// **Normative**: `deg` is in degrees and `|w|` the magnitude of [`wind_at`] there, so the
/// rotation is a fraction of the species' full angle and reaches it only at full wind. The
/// plant is **not translated** — the pivot is the tile center and the anchor, so this turns
/// the crown in place and a radial reveal (which measures distance from that center) is
/// untouched. A `θ` of exactly 0 returns `heading` itself, bit for bit, so a calm cube is
/// the windless image rather than a rounded copy of it. A non-finite input is `heading`.
pub fn canopy_heading(heading: Vec2, deg: f64, w: Vec2) -> Vec2 {
    if !(deg.is_finite() && w.is_finite()) {
        return heading;
    }
    let theta = deg.to_radians() * w.length() / WIND_CHART_MAX;
    if theta == 0.0 {
        return heading;
    }
    Vec2::from_screen_angle(heading.screen_angle() + theta)
}

/// What the shared breeze does to one plant slot at a presentation instant: the [`Bend`]
/// every stamp of that slot takes, and the heading it is stamped with.
///
/// **Normative**, and the single rule [`ArtPresenter::draw`] follows for every small plant.
/// With `w = `[`wind_at`]`(slot.at, seconds, response.lag_seconds)` and `tip =
/// `[`effective_tip`]`(response.tip_px, budget) · slot.wind`:
///
/// * a species with no response ([`WindResponse::STILL`], e.g. `rootveil`) is
///   `(Bend::NONE, slot.heading)` without sampling the wind at all;
/// * a slot on the **top face** whose species is *radial* (`response.spin_deg > 0`) turns in
///   place: `(Bend::NONE, `[`canopy_heading`]`(slot.heading, response.spin_deg · slot.wind,
///   w))` — never bent or moved;
/// * any other slot bends: `(`[`plant_bend`]`(tip, w, slot.heading), slot.heading)`. This
///   includes a **reed standing in a flooded top-face cell** (`reedspire` has a tip and no
///   spin): its tile lies flat on the canopy face pointing along its own heading, and it
///   bends along that tile's horizontal axis exactly as it would on a side face, rooted at
///   its ripple row, so the root never skates and the breeze reads on the top face too.
///
/// It is evaluated **once per slot per frame** and applies unchanged to the idle stamp, the
/// fruit blend and both the fading lower and the revealing upper stamp of a growth step, so
/// nothing inside one plant moves differently from the rest of it.
pub fn slot_wind(slot: &Slot, name: &str, budget: f64, seconds: f64) -> (Bend, Vec2) {
    let response = wind_response(name);
    if response == WindResponse::STILL {
        return (Bend::NONE, slot.heading);
    }
    let w = wind_at(slot.at, seconds, response.lag_seconds);
    if slot.at.face == Face::Top && response.spin_deg > 0.0 {
        (Bend::NONE, canopy_heading(slot.heading, response.spin_deg * slot.wind, w))
    } else {
        let tip = effective_tip(response.tip_px, budget) * slot.wind;
        (plant_bend(tip, w, slot.heading), slot.heading)
    }
}

/// The one bend amplitude a whole tall column takes at a presentation instant, in tile pixels.
///
/// **Normative**: `effective_tip(response.tip_px, budget) · `[`tall_wind_of`]` ·
/// dot(`[`wind_at`]`(`[`tall_anchor`]`(face, cx, 0), seconds, response.lag_seconds),
/// `[`tall_heading`]`)` — one sample, at the column's base anchor, projected onto the column's
/// own horizontal axis, with `budget` the column's shared budget
/// ([`ArtPresenter::column_budget`]). Every part of the column is then stamped with this
/// amplitude and [`tall_bend_base`] of its own tile index.
pub fn tall_amplitude(column: &TallColumn, budget: f64, seconds: f64) -> f64 {
    let response = wind_response(TALL_PLANTS[column.pick]);
    let tip = effective_tip(response.tip_px, budget) * tall_wind_of(column.face, column.cx);
    if tip <= 0.0 {
        return 0.0;
    }
    let at = tall_anchor(column.face, column.cx, 0);
    tip * wind_at(at, seconds, response.lag_seconds).dot(tall_heading(column.face, column.cx))
}

/// The measured bend budget of one plant: the smallest
/// [`cubarium_render::Sprite::bend_headroom`] over every frame of every clip the plant can
/// draw — all three stages, the fruit clip, and every authored growth transition (pack v5) —
/// at the small-plant bend shape ([`PLANT_BEND_ROOT`], [`PLANT_BEND_LENGTH`], base 0).
///
/// **Normative**, and measured once per pack at [`ArtPresenter::new`], never per frame: a
/// budget that moved with the pose on screen would let a plant clip its own footprint the
/// frame it came into fruit, or the frame a growth clip reached its widest. A plant with no
/// bendable texel has an infinite budget, which [`effective_tip`] then leaves to the species'
/// desired tip.
pub fn plant_bend_budget(plant: &Plant) -> f64 {
    plant
        .stages
        .iter()
        .chain(plant.fruit.iter())
        .chain(plant.transitions.iter().map(|t| &t.clip))
        .flat_map(|clip| clip.frames.iter())
        .map(|frame| frame.bend_headroom(PLANT_BEND_ROOT, PLANT_BEND_LENGTH, 0.0))
        .fold(f64::INFINITY, f64::min)
}

/// The measured bend budget of one tall family: the smallest [`cubarium_render::Sprite::bend_headroom`] over
/// every frame of its base, trunk and **cap** (never the raw `crown`, which is not what is
/// drawn) at the worst [`tall_bend_base`] each part can be stamped at, with the column's
/// own bend shape.
///
/// **Normative**: the base tile at `tall_bend_base(0)` (`−8`), every trunk tile — and every
/// vine tile, which sits on a trunk position — at `tall_bend_base(`[`TALL_MAX_SEGMENTS`]`)`,
/// and the cap at its highest *continuous* placement `tall_bend_base(TALL_MAX_SEGMENTS + 1)`.
/// Those are the highest each part can be stamped at, and the highest placement bounds every
/// lower one: [`Bend::profile`] is monotone non-decreasing in `H`, and raising `base` raises
/// every texel's `H` by the same amount, so each texel's share of the amplitude can only grow
/// with `base` and its headroom can only shrink. Measuring the top placement therefore admits
/// every tile below it. A climber (a family with no base and no crown) is measured as a
/// trunk. One number per family, measured once: every part of a column bends by the same
/// amplitude, so they must all be inside the same budget.
pub fn tall_bend_budget(plant: &TallPlant) -> f64 {
    let worst = |clip: &Clip, i: f64| {
        clip.frames
            .iter()
            .map(|f| f.bend_headroom(TALL_BEND_ROOT, TALL_BEND_LENGTH, tall_bend_base(i)))
            .fold(f64::INFINITY, f64::min)
    };
    let top = f64::from(TALL_MAX_SEGMENTS);
    let mut budget = worst(&plant.trunk, top);
    if let Some(base) = &plant.base {
        budget = budget.min(worst(base, 0.0));
    }
    if let Some(cap) = &plant.cap {
        budget = budget.min(worst(cap, top + 1.0));
    }
    budget
}

/// A budget looked up by asset name; 0 for a name the table does not carry (an asset whose
/// room has not been measured must not move).
fn budget_in(budgets: &[(String, f64)], name: &str) -> f64 {
    match budgets.iter().find(|(n, _)| n == name) {
        Some((_, budget)) => *budget,
        None => 0.0,
    }
}

/// A presentation tick whose whole neighbourhood sits inside a wind packet's hold, so a
/// timing or capture fixture measures the cube at full wind. Fixture support only: nothing
/// in the presenter reads it.
pub const WIND_PEAK_TICK: u64 = 121;
/// A presentation tick whose whole neighbourhood sits in a quiet interval, so a fixture
/// measures the identity path. Fixture support only.
pub const WIND_QUIET_TICK: u64 = 401;
/// The tick a draw-cost fixture should start at: `CUBARIUM_WIND_TICK` when it is set and
/// parses, else [`WIND_PEAK_TICK`]. Fixture support only — this is how the crowded release
/// timings are taken at full wind and again at exact calm from one build, and nothing in the
/// drawn image depends on it.
pub fn wind_fixture_tick() -> u64 {
    match std::env::var("CUBARIUM_WIND_TICK").ok().and_then(|v| v.parse().ok()) {
        Some(tick) => tick,
        None => WIND_PEAK_TICK,
    }
}

// --- Paced growth --------------------------------------------------------------------
//
// The fields say what a cell warrants; these say how the picture gets there. A cell's
// visual walks one stage at a time, at [`STAGE_GROW_SECONDS`] up and
// [`STAGE_WILT_SECONDS`] down, and may turn round mid-step. Nothing here touches the
// simulation: the target is still exactly what [`next_stage`] decides.

/// The visual growth of one cell's plant slot.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Growth {
    /// The stage the visual has completed (`None` = bare).
    pub from: Option<u8>,
    /// The stage it is moving to; `== from` when idle.
    pub to: Option<u8>,
    /// Progress of `from → to` in [0, 1]; 1 when idle.
    pub g: f64,
    /// The resource-driven target with hysteresis (what the presenter's `stages` used to
    /// hold): what [`next_stage`] says the field warrants.
    pub target: Option<u8>,
    /// Fruit-accent blend in [0, 1].
    pub fruit: f64,
}

impl Growth {
    /// The growth of a slot that is already where the field wants it: no transition, and
    /// the fruit accent fully on iff a full-grown plant is in fruit.
    pub fn snapped(target: Option<u8>, in_fruit: bool) -> Growth {
        let fruit = if in_fruit && target == Some(2) { 1.0 } else { 0.0 };
        Growth { from: target, to: target, g: 1.0, target, fruit }
    }
}

/// A stage as a rank: `None` is −1, `Some(s)` is `s`.
fn rank(stage: Option<u8>) -> i32 {
    stage.map_or(-1, i32::from)
}

/// The stage one rank step from `stage` in direction `dir`, clamped to the three stages.
fn stage_at(stage: Option<u8>, dir: i32) -> Option<u8> {
    match (rank(stage) + dir).clamp(-1, 2) {
        -1 => None,
        r => Some(r as u8),
    }
}

/// One tick of a cell's paced growth.
///
/// **Normative**, pure, and called once per simulation tick with `dt` the simulated
/// seconds the tick took (`dt = 0` is legal: only the retarget happens). With `rank(None)
/// = −1` and `rank(Some(s)) = s`:
///
/// 1. `target` is recorded. A non-finite or negative `dt` is 0.
/// 2. Idle (`from == to`) with `target != to` **and `dt > 0`**: one step starts, `to = from
///    ± 1` toward `target` and `g = 0` — one stage at a time, so `None → 2` runs `None → 0
///    → 1 → 2`. With `dt = 0` (a repeated observe of the same tick) nothing starts, so a
///    step that has just completed is not immediately followed by the next one's
///    bookkeeping on a call that represents no time.
/// 3. In flight (`from != to`) with `target != to`: if `target` lies on the `from` side of
///    `to` (including `target == from`) the step **reverses** — `from` and `to` swap and
///    `g` becomes `1 − g`, so a plant that starts wilting and is fed again grows back out
///    of exactly the pose it had reached. A target beyond `to` in the same direction
///    finishes this step first.
/// 4. `g` advances by `dt / `[`STAGE_GROW_SECONDS`] rising or `dt /
///    `[`STAGE_WILT_SECONDS`] falling, and the step completes (`from = to`) at 1. At most
///    one step starts per call.
/// 5. The fruit accent moves toward 1 over [`FRUIT_FADE_SECONDS`] while the plant is
///    full-grown (`from == to == Some(2)`) and in fruit, never overshooting, and is 0 the
///    moment either stops being true: it is the food signal and does not linger.
pub fn advance_growth(g: Growth, target: Option<u8>, in_fruit: bool, dt: f64) -> Growth {
    let dt = if dt.is_finite() && dt > 0.0 { dt } else { 0.0 };
    let mut g = Growth { target, ..g };
    if g.from == g.to {
        if target != g.to && dt > 0.0 {
            g.to = stage_at(g.from, (rank(target) - rank(g.from)).signum());
            g.g = 0.0;
        }
    } else if target != g.to
        && (rank(target) - rank(g.to)).signum() != (rank(g.to) - rank(g.from)).signum()
    {
        std::mem::swap(&mut g.from, &mut g.to);
        g.g = 1.0 - g.g;
    }
    if g.from != g.to {
        let seconds =
            if rank(g.to) > rank(g.from) { STAGE_GROW_SECONDS } else { STAGE_WILT_SECONDS };
        let step = if seconds > 0.0 { dt / seconds } else { 1.0 };
        g.g = (g.g + step).min(1.0);
        if g.g >= 1.0 {
            g.from = g.to;
            g.g = 1.0;
        }
    }
    g.fruit = if in_fruit && g.from == Some(2) && g.to == Some(2) {
        let step = if FRUIT_FADE_SECONDS > 0.0 { dt / FRUIT_FADE_SECONDS } else { 1.0 };
        (g.fruit + step).min(1.0)
    } else {
        0.0
    };
    g
}

/// The growth drawn a fraction `f` of a tick after `prev` (the state the previous
/// `observe` left) on the way to `cur` (the state this tick's `observe` left).
///
/// **Normative**: `f` is clamped to `[0, 1]` (non-finite → 0), `target` is `cur`'s and
/// `fruit` is the linear mix. The pair and progress follow the one thing that can have
/// happened in a tick: the same step in flight (`g` mixed linearly); a step that started
/// from idle (`g` from 0); a step that completed into idle (`g` to 1); a reversal (drawn in
/// `prev`'s orientation, `g` mixed toward `1 − cur.g`); a reversal that completed back
/// where it started (`g` toward 0). Anything else — a snap, a band change — is `cur`.
/// Every case is continuous at `f = 0` with `prev` and at `f = 1` with `cur`, so growth
/// drawn at 60 fps is as continuous as the sway.
pub fn growth_between(prev: Growth, cur: Growth, f: f64) -> Growth {
    let f = if f.is_finite() { f.clamp(0.0, 1.0) } else { 0.0 };
    let mix = |a: f64, b: f64| a + (b - a) * f;
    let fruit = mix(prev.fruit, cur.fruit);
    let prev_idle = prev.from == prev.to;
    let cur_idle = cur.from == cur.to;
    let (from, to, g) = if prev.from == cur.from && prev.to == cur.to {
        (cur.from, cur.to, mix(prev.g, cur.g))
    } else if prev_idle && !cur_idle && prev.to == cur.from {
        (cur.from, cur.to, mix(0.0, cur.g))
    } else if !prev_idle && cur_idle && cur.to == prev.to {
        (prev.from, prev.to, mix(prev.g, 1.0))
    } else if !prev_idle && !cur_idle && prev.from == cur.to && prev.to == cur.from {
        (prev.from, prev.to, mix(prev.g, 1.0 - cur.g))
    } else if !prev_idle && cur_idle && cur.to == prev.from {
        (prev.from, prev.to, mix(prev.g, 0.0))
    } else {
        (cur.from, cur.to, cur.g)
    };
    // A pair drawn at progress 1 is that pair's `to`, idle; keep the invariant so callers
    // can test `from == to` for idleness.
    if from != to && g >= 1.0 {
        return Growth { from: to, to, g: 1.0, target: cur.target, fruit };
    }
    if from != to && g <= 0.0 {
        return Growth { from, to: from, g: 1.0, target: cur.target, fruit };
    }
    Growth { from, to, g, target: cur.target, fruit }
}

/// One stage step as the *drawing* rules see it: the two stages it runs between, ordered by
/// rank, and how far along the upper one it is.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GrowthStep {
    /// The lower-ranked of the two stages; `None` is bare ground (the `None ↔ 0` step).
    pub lower: Option<u8>,
    /// The higher-ranked stage, which is always a real stage.
    pub upper: u8,
    /// The **upper stage's progress** in `[0, 1]`: 0 is the lower stage alone, 1 the upper
    /// stage alone, whichever way the step is travelling.
    pub t: f64,
}

/// The step a frame's [`Growth`] draws, or `None` for an idle growth (which draws one stage
/// whole).
///
/// **Normative**: with `rank(None) = −1`, the pair is ordered by rank — `(lower, upper) =
/// (from, to)` while rising and `(to, from)` while falling — and `t = g` rising, `1 − g`
/// falling. So `t` always runs from the lower stage to the upper one **whichever direction
/// the step is going**, and a reversal is drawn by exactly the same rule at exactly the same
/// `t`: a plant that starts to wilt and is fed again retraces the very pictures it came
/// through, in reverse, rather than restarting anything.
pub fn growth_step(growth: Growth) -> Option<GrowthStep> {
    if growth.from == growth.to {
        return None;
    }
    let rising = rank(growth.to) > rank(growth.from);
    let (lower, upper) = if rising { (growth.from, growth.to) } else { (growth.to, growth.from) };
    // The higher-ranked of two *different* stages is never bare ground; the `?` is a
    // safeguard, not a case.
    let upper = upper?;
    Some(GrowthStep { lower, upper, t: if rising { growth.g } else { 1.0 - growth.g } })
}

/// The share of an authored growth clip's progress ([`GrowthStep::t`]) spent blending into
/// the idle stage clip at each end of the step. Review-tunable.
///
/// A growth clip is baked from its plant's *neutral* pose, but the stage clips it grows out
/// of and into are sway loops running at the slot's own phase and, for a sprout, pulsing
/// their whole sprite. Cutting straight to and from the clip would therefore step the
/// brightness and the lean at both ends of every step. Blending over the first and last
/// `GROW_BLEND` of the progress — 0.48 s of the pilot's 4 s clip — hides both without
/// stretching the authored motion. 0 disables the blends (the clip alone, endpoint cuts
/// included); it must stay below 0.5 or the two blends would overlap.
pub const GROW_BLEND: f64 = 0.12;

/// The three layer weights an authored growth stamp carries at a step's progress `t`:
/// `[w_from, w_grow, w_to]` — the lower stage's idle sway clip, the growth clip, and the
/// upper stage's idle sway clip.
///
/// **Normative**, with `smoothstep` the clamped Hermite `t²(3 − 2t)`:
///
/// ```text
/// w_from = 1 − smoothstep(t / GROW_BLEND)                  (exactly 0 once t ≥ GROW_BLEND)
/// w_to   = smoothstep((t − (1 − GROW_BLEND)) / GROW_BLEND)  (exactly 0 until t > 1 − GROW_BLEND)
/// w_grow = 1 − w_from − w_to
/// ```
///
/// The three weights sum to 1, so the stamp is an exact lerp and a pixel opaque in every
/// layer stays opaque. Because [`GROW_BLEND`] is below ½ the two edge blends never overlap:
/// `w_from` and `w_to` are never both positive, `w_grow` is 1 through the middle of the
/// step, and each end of the step is the neighbouring idle clip *alone* — `t = 0` is the
/// lower stage's own idle image and `t = 1` the upper stage's, so a step neither enters nor
/// leaves with a cut. Both edges have zero slope in `t`. A `NaN` `t` reads as the lower
/// stage held still.
pub fn growth_weights(t: f64) -> [f32; 3] {
    let w_from = 1.0 - hermite(t / GROW_BLEND);
    let w_to = hermite((t - (1.0 - GROW_BLEND)) / GROW_BLEND);
    [w_from as f32, (1.0 - w_from - w_to) as f32, w_to as f32]
}

/// The visual height of one tall column.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TallGrowth {
    /// Trunk segments shown, continuous, in `0..=`[`TALL_MAX_SEGMENTS`].
    pub height: f64,
    /// The segments the column's density warrants ([`next_tall`], with hysteresis).
    pub target: u8,
}

/// One tick of a column's paced height.
///
/// **Normative**: `height` moves toward `target` by `dt · `[`TALL_GROW_PX_PER_S`]` / 4`
/// rising and `dt · `[`TALL_WILT_PX_PER_S`]` / 4` falling (a segment is 4 px), never
/// overshooting. A non-finite or negative `dt` is 0; a non-finite height snaps.
pub fn advance_tall(g: TallGrowth, target: u8, dt: f64) -> TallGrowth {
    let dt = if dt.is_finite() && dt > 0.0 { dt } else { 0.0 };
    let want = f64::from(target);
    if !g.height.is_finite() {
        return TallGrowth { height: want, target };
    }
    let rate = if want > g.height { TALL_GROW_PX_PER_S } else { TALL_WILT_PX_PER_S } / 4.0;
    let height = if want > g.height {
        (g.height + dt * rate).min(want)
    } else {
        (g.height - dt * rate).max(want)
    };
    TallGrowth { height, target }
}

/// The column height drawn a fraction `f` of a tick after `prev` on the way to `cur`: the
/// linear mix of the two heights (`f` clamped, non-finite → 0), with `cur`'s target.
pub fn tall_between(prev: TallGrowth, cur: TallGrowth, f: f64) -> TallGrowth {
    let f = if f.is_finite() { f.clamp(0.0, 1.0) } else { 0.0 };
    TallGrowth { height: prev.height + (cur.height - prev.height) * f, target: cur.target }
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

/// The pose of a ground tile at a presentation time ([`present_seconds`]).
///
/// **Normative**: exactly [`Clip::sample`]'s looping rule applied to the tile's frames —
/// `u = (seconds mod length) / length · n`, the samples `floor(u)` and `(floor(u) + 1) mod
/// n`, blended by `fract(u)`, so the breath wraps last → first without a step. A tile with
/// one frame, a nonsense length or a non-finite time is its first frame held still.
pub fn ground_pose(tile: &GroundTile, seconds: f64) -> Pose<'_> {
    let n = tile.frames.len();
    if n < 2 || !seconds.is_finite() || !(tile.seconds.is_finite() && tile.seconds > 0.0) {
        return Pose::still(&tile.frames[0]);
    }
    let u = seconds.rem_euclid(tile.seconds) / tile.seconds * n as f64;
    let i = (u.floor() as usize).min(n - 1);
    Pose { first: &tile.frames[i], second: &tile.frames[(i + 1) % n], mix: (u - i as f64) as f32 }
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
/// Algae in a pool: the water color leans toward this mint where the wet cell holds
/// producers (`design/water.md` "Algae"). Review-tunable within the flora family.
pub const ALGAE_SRGB: u32 = 0x007B_EBC9;
/// How far a fully grown mat pulls the water color toward [`ALGAE_SRGB`]. Review-tunable.
pub const ALGAE_TINT: f32 = 0.6;

/// The water color over a cell whose producer density (fraction of the ramp saturation)
/// is `p_t`: `mix(water_color(w), algae, min(p_t, 1) · ALGAE_TINT)`. The soil ground itself
/// still draws no lawn; the mat shows only as a tint on the pool.
pub fn algae_water_color(w: f64, p_t: f64) -> [f32; 3] {
    let base = water_color(w);
    let t = if p_t.is_nan() { 0.0 } else { p_t.clamp(0.0, 1.0) as f32 * ALGAE_TINT };
    present::mix(base, *ALGAE_COLOR, t)
}

static ALGAE_COLOR: LazyLock<[f32; 3]> = LazyLock::new(|| present::srgb_linear(ALGAE_SRGB));

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

/// The water brightness at a presentation time ([`present_seconds`]) for a pixel of
/// shimmer phase `phase`: `WATER_BRIGHT · (1 + WATER_SHIMMER · sin(2π · seconds /
/// WATER_SHIMMER_SECONDS + phase))`. Simulated time only: a paused world holds its glints,
/// the glints slide continuously between ticks, and the pattern repeats exactly every
/// `WATER_SHIMMER_SECONDS`.
pub fn water_brightness(seconds: f64, phase: f64) -> f32 {
    let cycle = (seconds / WATER_SHIMMER_SECONDS).fract();
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
/// px · (1 − a)` with `a = `[`water_coverage`]`(w)` and `b = `[`water_brightness`]; the
/// color is [`algae_water_color`] with the pixel's own cell's producer density over
/// `saturation`, so a pool with a mat reads mint rather than pure blue.
fn draw_water(
    canvas: &mut Canvas,
    water: &ScalarField,
    producer: &ScalarField,
    saturation: f64,
    seconds: f64,
) {
    for face in Face::ALL {
        for y in 0..FACE_SIZE as u8 {
            for x in 0..FACE_SIZE as u8 {
                let w = filtered_at(water, face, x, y);
                let a = water_coverage(w);
                if a <= 0.0 {
                    continue;
                }
                let p_t = if saturation.is_finite() && saturation > 0.0 {
                    producer.get(cell_of(&SurfacePoint::pixel_center(face, x, y))) / saturation
                } else {
                    0.0
                };
                let c = algae_water_color(w, p_t);
                let b = water_brightness(seconds, water_phase(face, x, y));
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

/// How bright a top-face sparkle is at a presentation time ([`present_seconds`]).
///
/// **Normative**: with `u = seconds mod `[`RAIN_PERIOD`], a raised cosine `0.5 · (1 −
/// cos(2π · u / RAIN_BLINK))` while `u < `[`RAIN_BLINK`], 0 after it; a non-finite time is
/// 0. It peaks at 1 halfway through the blink and is continuous everywhere, so a sparkle
/// swells and dies instead of switching on.
pub fn rain_blink(seconds: f64) -> f32 {
    if !seconds.is_finite() || !(RAIN_BLINK > 0.0) {
        return 0.0;
    }
    let u = seconds.rem_euclid(RAIN_PERIOD);
    if u >= RAIN_BLINK {
        return 0.0;
    }
    (0.5 * (1.0 - (std::f64::consts::TAU * u / RAIN_BLINK).cos())) as f32
}

/// Whether a top-face sparkle is lit at all: `rain_blink(seconds) > 0`.
pub fn rain_blink_on(seconds: f64) -> bool {
    rain_blink(seconds) > 0.0
}

/// The pixels streak `k` of a cell marks at a presentation time ([`present_seconds`]),
/// face-local, each with the weight of its coverage.
///
/// **Normative**: on a side face the streak is a 1×2 mark at the *continuous* downhill
/// position `s = origin + `[`rain_fall`]`(seconds)`. With `φ = fract(rain_fall)` and the
/// head pixel at the wrapped integer position (`(origin + floor(fall)).rem_euclid(4)`
/// within the cell's four pixels), the marks are the head at `1 − φ`, one pixel downhill
/// at 1 and two pixels downhill at `φ`, each dropped if it leaves the face (the two
/// trailing pixels are not wrapped). Their weights sum to 2 whenever all three are on the
/// face, so a streak's light is constant as it falls and the fall reads as smooth rather
/// than as a pixel step. On the top face it is a single pixel at the origin weighted by
/// [`rain_blink`], omitted when that is 0.
pub fn rain_marks(cell: CellId, k: usize, seconds: f64) -> Vec<((u8, u8), f32)> {
    let (dx, dy) = rain_origin(cell, k);
    let x0 = i32::from(cell.cx()) * 4;
    let y0 = i32::from(cell.cy()) * 4;
    let Some(up) = up_of(cell) else {
        let blink = rain_blink(seconds);
        return if blink > 0.0 {
            vec![(((x0 + i32::from(dx)) as u8, (y0 + i32::from(dy)) as u8), blink)]
        } else {
            Vec::new()
        };
    };
    let down = Vec2::new(-up.x, -up.y);
    let fall = rain_fall(seconds);
    let phi = (fall - fall.floor()) as f32;
    let steps = fall.floor() as i32;
    let in_face = |v: i32| (0..FACE_SIZE as i32).contains(&v);
    let mut marks = Vec::with_capacity(3);
    let (vertical, sign) = if down.y.abs() >= down.x.abs() {
        (true, if down.y >= 0.0 { 1 } else { -1 })
    } else {
        (false, if down.x >= 0.0 { 1 } else { -1 })
    };
    let along = if vertical { i32::from(dy) } else { i32::from(dx) };
    let head = (along + sign * steps).rem_euclid(4);
    for (offset, weight) in [(0, 1.0 - phi), (1, 1.0), (2, phi)] {
        let step = head + sign * offset;
        let (x, y) = if vertical {
            (x0 + i32::from(dx), y0 + step)
        } else {
            (x0 + step, y0 + i32::from(dy))
        };
        // The head is wrapped within its cell; the two trailing pixels are not, so a
        // streak that reaches the edge of the face simply loses its tail.
        if in_face(x) && in_face(y) {
            marks.push(((x as u8, y as u8), weight));
        }
    }
    marks
}

static RAIN_COLOR: LazyLock<[f32; 3]> = LazyLock::new(|| present::srgb_linear(RAIN_SRGB));

/// Rain: for every cell with rain, its streaks source-over the image, each mark at
/// `min(1, RAIN_OPACITY · min(rain, 1) · weight)` — so a streak sliding between two pixels
/// shares its light between them instead of jumping.
fn draw_rain(canvas: &mut Canvas, rain: &[f32], seconds: f64) {
    let color = *RAIN_COLOR;
    for (index, cell) in CellId::all().enumerate() {
        let rate = rain.get(index).copied().unwrap_or(0.0);
        let n = rain_streaks(rate);
        if n == 0 {
            continue;
        }
        let scale = RAIN_OPACITY * rate.min(1.0);
        let face = cell.face();
        for k in 0..n {
            for ((x, y), weight) in rain_marks(cell, k, seconds) {
                let alpha = (scale * weight).min(1.0);
                if alpha <= 0.0 {
                    continue;
                }
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

/// A column's own share of the shared breeze, in `[1 − `[`WIND_SLOT_VARIATION`]`, 1 +
/// WIND_SLOT_VARIATION)`, exactly like a small plant's [`Slot::wind`].
///
/// **Normative**: the fourth value of the column's own `SplitMix64` stream (select, pick,
/// vine, wind) — *appended*, so adding the wind moved no column, changed no species and
/// grew no vine that was not there before.
pub fn tall_wind_of(face: Face, cx: u8) -> f64 {
    let mut hash = SplitMix64::new(TALL_SEED ^ (face.index() as u64) << 8 ^ u64::from(cx));
    let _select = hash.next_f64();
    let _pick = hash.next_u64();
    let _vine = hash.next_f64();
    hash.range(1.0 - WIND_SLOT_VARIATION, 1.0 + WIND_SLOT_VARIATION)
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
    tall_anchor_at(face, cx, f64::from(i))
}

/// [`tall_anchor`] at a *continuous* tile index: the horizon cell's center moved `4 · i`
/// pixels up the face. This is what lets a crown glide with its column instead of stepping
/// a whole cell when a segment completes; `tall_anchor(face, cx, i)` is
/// `tall_anchor_at(face, cx, i as f64)`.
pub fn tall_anchor_at(face: Face, cx: u8, i: f64) -> SurfacePoint {
    let (_, horizon) = foliage_rows(face).unwrap_or((0, 0));
    let cell = CellId::new(face, cx, horizon);
    let center = cell.center();
    let up = up_of(cell).unwrap_or(Vec2::new(0.0, -1.0));
    let step = up * (4.0 * i);
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

/// How far up the face, in pixels above the horizon cell's center, a column of continuous
/// `height` (segments) has painted trunk.
///
/// **Normative**: the base paints the trunk pattern up to `TALL_FIRST_JOIN − 4` (5 px);
/// the first segment grows from there to [`TALL_JOIN`] (12 px) over `height ∈ [0, 1]`;
/// every further segment adds 4 px: `TALL_JOIN + 4 · (height − 1)` for `height ≥ 1`. So
/// `tall_grown_px(n)` for a whole `n ≥ 1` is `4n + 8`, the top of trunk tile `n`.
/// Non-positive or non-finite heights give the base's 5 px.
pub fn tall_grown_px(height: f64) -> f64 {
    let base_top = TALL_FIRST_JOIN - 4.0;
    if !(height > 0.0) {
        return base_top;
    }
    if height <= 1.0 {
        base_top + (TALL_JOIN - base_top) * height
    } else {
        TALL_JOIN + 4.0 * (height - 1.0)
    }
}

/// Stamp one column at a continuous `height` in segments: base, the trunk tiles the grown
/// height reaches, the cap, then the vine's tiles, all at the column's heading and
/// presentation time, **every row of the column composited exactly once**.
///
/// **Normative**: nothing at all when `height ≤ 0`. With `fade = clamp(height /
/// `[`TALL_BASE_FADE`]`, 0, 1)` and `grown = `[`tall_grown_px`]`(height)`:
///
/// - the base (tile 0) is drawn whole at `TALL_OPACITY · fade`;
/// - trunk tile `i ≥ 1` at [`tall_anchor`]`(i)` owns the rows from `floor` up to the tile's
///   top, `floor = `[`TALL_FIRST_JOIN`] for `i = 1` and [`TALL_JOIN`] above (below that
///   line the tile below has already painted the 4-periodic pattern), and is drawn with
///   `Mask::Strip { floor, reveal }`, `reveal = min(16, grown − (4i − 8))` in tile rows: the
///   strip of rows it owns, cut at the grown height, so the newest segment grows out of the
///   one below one row-fraction at a time and a tile whose strip is empty is not drawn;
/// - the cap ([`TallPlant::cap`], never `crown`) glides at [`tall_anchor_at`]`(height + 1)`
///   at `TALL_OPACITY · fade`; at rest on a whole cell it sits where the crown used to;
/// - vine tiles at odd `i` own rows [`TALL_VINE_FLOOR`]`..`[`TALL_VINE_TOP`] (the topmost
///   possible tile up to 16) with the same cut, at `TALL_OPACITY · fade`.
///
/// Because no row is painted twice, the image is continuous in `height`: at `height → 0`
/// everything fades to nothing, and at a whole `height = k` the tile `k + 1` enters with
/// an empty strip.
///
/// **Wind**: `amplitude` is the column's single bend amplitude, in tile pixels, taken from
/// one [`wind_at`] sample at the base anchor. Every part — the base, every trunk strip, the
/// cap at its fractional index, and every vine tile — is stamped with the *same* amplitude
/// and the same [`Bend`] shape ([`TALL_BEND_ROOT`], [`TALL_BEND_LENGTH`]), differing only in
/// [`tall_bend_base`]`(i)`, so the whole column is one continuous curve `D(H)` of one global
/// height coordinate: no tile join opens, the cap stays on the stem's curve even where its
/// pixels are owned by Top, and a vine cannot slide against its trunk. An amplitude of 0 is
/// [`Bend::NONE`] and the column is drawn exactly as it was before the wind existed.
fn draw_column(
    canvas: &mut Canvas,
    column: &TallColumn,
    height: f64,
    plant: &TallPlant,
    vine: Option<&TallPlant>,
    seconds: f64,
    amplitude: f64,
    scratch: &mut Vec<PixelImage>,
) {
    if !(height > 0.0) {
        return;
    }
    let height = height.min(f64::from(TALL_MAX_SEGMENTS));
    let fade = (height / TALL_BASE_FADE).clamp(0.0, 1.0) as f32;
    let grown = tall_grown_px(height);
    let heading = tall_heading(column.face, column.cx);
    let mut stamp = |clip: &Clip, i: f64, mask: Mask, opacity: f32| {
        let pose = clip.sample(seconds + tall_phase_of(column.face, column.cx, clip.seconds));
        let at = tall_anchor_at(column.face, column.cx, i);
        let bend = Bend {
            amplitude,
            base: tall_bend_base(i),
            root: TALL_BEND_ROOT,
            length: TALL_BEND_LENGTH,
        };
        stamp_layers_bent(
            canvas,
            at,
            heading,
            &[(pose, 1.0)],
            1.0,
            opacity,
            mask,
            bend,
            scratch,
        );
    };
    // The grown height in tile `i`'s own rows (its bottom edge is 4i − 8 px up the face).
    let local = |i: u8| grown - (4.0 * f64::from(i) - 8.0);
    if let Some(base) = &plant.base {
        stamp(base, 0.0, Mask::None, TALL_OPACITY * fade);
    }
    for i in 1..=TALL_MAX_SEGMENTS {
        let floor = if i == 1 { TALL_FIRST_JOIN } else { TALL_JOIN };
        let reveal = local(i).min(TILE_ROWS);
        if reveal <= floor {
            break;
        }
        stamp(&plant.trunk, f64::from(i), Mask::Strip { floor, reveal }, TALL_OPACITY);
    }
    if let Some(cap) = &plant.cap {
        stamp(cap, height + 1.0, Mask::None, TALL_OPACITY * fade);
    }
    if let (true, Some(vine)) = (column.vine, vine) {
        for i in (1..=TALL_MAX_SEGMENTS).step_by(2) {
            let top = if i + 2 > TALL_MAX_SEGMENTS { TILE_ROWS } else { TALL_VINE_TOP };
            let reveal = local(i).min(top);
            if reveal <= TALL_VINE_FLOOR {
                break;
            }
            stamp(
                &vine.trunk,
                f64::from(i),
                Mask::Strip { floor: TALL_VINE_FLOOR, reveal },
                TALL_OPACITY * fade,
            );
        }
    }
}

/// The layers one stage of a plant holds at presentation seconds `s`, fruit included, for
/// [`stamp_layers`].
///
/// **Normative**: the stage's own sway clip sampled at `s + `[`plant_phase_of`], blended
/// between its two bracketing samples ([`Clip::sample`]), at weight `1 − fruit`; and, for a
/// full-grown plant (`stage == 2`) whose species has a `fruit` clip, that clip sampled the
/// same way at weight `fruit`. Both clips keep their temporal blend through the fade, so a
/// plant coming into fruit never drops to held frames; at `fruit = 0` the fruit layer is
/// not sampled, at `fruit = 1` the stage layer is not. A plant without a fruit clip, or
/// below stage 2, is its stage clip alone whatever `fruit` says.
fn stage_layers(plant: &Plant, stage: u8, cell: CellId, s: f64, fruit: f64) -> [(Pose<'_>, f32); 2] {
    let at = |clip: &Clip| s + plant_phase_of(cell, clip.seconds);
    let q = if fruit.is_finite() { fruit.clamp(0.0, 1.0) as f32 } else { 0.0 };
    match &plant.fruit {
        Some(fruit_clip) if stage == 2 && q > 0.0 => [
            (stage_pose(plant, stage, cell, s), 1.0 - q),
            (fruit_clip.sample(at(fruit_clip)), q),
        ],
        _ => {
            let pose = stage_pose(plant, stage, cell, s);
            [(pose, 1.0), (pose, 0.0)]
        }
    }
}

/// The **idle sway pose** of one stage at presentation seconds `s`: that stage's own looping
/// clip sampled at `s + `[`plant_phase_of`]`(cell, clip.seconds)`, blended between its two
/// bracketing frames ([`Clip::sample`]).
///
/// Exactly the layer [`stage_layers`] carries at `fruit = 0`, and the pose an authored growth
/// stamp blends into at each end of its step, so entering and leaving a growth clip lands on
/// the very image the idle plant is showing at that instant — the slot's own phase included.
fn stage_pose(plant: &Plant, stage: u8, cell: CellId, s: f64) -> Pose<'_> {
    let clip = &plant.stages[usize::from(stage)];
    clip.sample(s + plant_phase_of(cell, clip.seconds))
}

/// The largest extent among the layers that will be sampled (weight > 0).
fn layers_extent(layers: &[(Pose<'_>, f32)]) -> f64 {
    layers
        .iter()
        .filter(|(_, w)| w.is_finite() && *w > 0.0)
        .map(|(p, _)| p.extent())
        .fold(0.0, f64::max)
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

/// What the presenter remembers about one organism's body, so a state change can cross-fade
/// instead of cutting — and a change that arrives *during* a fade starts from the blend
/// actually on screen rather than from the pose it was fading toward.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BodyMemory {
    /// The state it is in now ([`state_of`]).
    pub state: usize,
    /// What the body was showing the instant `state` began: at most two earlier states
    /// with weights summing to 1 (an unused entry has weight 0). A body that has never
    /// changed, or entered on a snap, holds `[(state, 1), (state, 0)]`.
    pub prev: [(usize, f32); 2],
    /// The presentation seconds at which `state` began; `−∞` for a body that entered on a
    /// snap, which therefore draws with no fade.
    pub switched_at: f64,
    /// The unit heading the body was drawn with at the end of the last observed tick
    /// ([`present::interpolate`] at `f = 1`): what the next tick's turn is measured from.
    pub end_heading: Vec2,
    /// The signed turn, in radians in `(−π, π]`, from `end_heading` of the previous tick to
    /// the heading this tick *starts* with. The world turns a body in one step at the
    /// tick; the presenter spends that step over the tick's frames ([`turn_heading`]), so
    /// a turning body rotates continuously instead of popping. 0 for a body just entered.
    pub turn: f64,
}

impl BodyMemory {
    /// A body first seen in `state`, drawn with no fade and no pending turn.
    pub fn entered(state: usize) -> BodyMemory {
        BodyMemory {
            state,
            prev: [(state, 1.0), (state, 0.0)],
            switched_at: f64::NEG_INFINITY,
            end_heading: Vec2::new(1.0, 0.0),
            turn: 0.0,
        }
    }

    /// How far the fade into `state` has got at presentation seconds `s`, in `[0, 1]`.
    pub fn fade_at(&self, s: f64) -> f32 {
        let w = (s - self.switched_at) / BODY_FADE_SECONDS;
        if w.is_nan() { 1.0 } else { w.clamp(0.0, 1.0) as f32 }
    }

    /// The states on screen at presentation seconds `s` with their weights (summing to 1):
    /// the earlier states at `prev · (1 − fade)` and `state` at `fade`, zero-weight entries
    /// dropped and equal states merged. Once the fade is complete this is `[(state, 1)]`.
    pub fn layers_at(&self, s: f64) -> Vec<(usize, f32)> {
        let w = self.fade_at(s);
        let mut out: Vec<(usize, f32)> = Vec::with_capacity(3);
        let mut push = |state: usize, weight: f32| {
            if weight <= 0.0 {
                return;
            }
            match out.iter_mut().find(|(st, _)| *st == state) {
                Some((_, acc)) => *acc += weight,
                None => out.push((state, weight)),
            }
        };
        for (state, weight) in self.prev {
            push(state, weight * (1.0 - w));
        }
        push(self.state, w);
        out
    }

    /// Record the headings of a newly observed tick: `start` is the heading the tick's
    /// path begins with, `end` the one it leaves the body with. The turn from the previous
    /// tick's `end_heading` to `start` is what the frames of this tick spend.
    pub fn observe_heading(&mut self, start: Vec2, end: Vec2) {
        self.turn = signed_turn(self.end_heading, start);
        self.end_heading = end;
    }

    /// Record a change to `state` at presentation seconds `now`.
    ///
    /// **Normative**: the new `prev` is the blend on screen at `now` ([`layers_at`]), which
    /// is what the next fade starts from; with three distinct states in it, the lightest is
    /// dropped and the other two renormalized (a step of at most that weight, which cannot
    /// exceed one third, on a body that changed state twice inside one fade). A change back
    /// to the state being faded out therefore resumes from exactly the current blend.
    pub fn switch_to(&mut self, state: usize, now: f64) {
        let mut layers = self.layers_at(now);
        layers.sort_by(|a, b| b.1.total_cmp(&a.1));
        layers.truncate(2);
        let total: f32 = layers.iter().map(|(_, w)| w).sum();
        let mut prev = [(state, 0.0f32); 2];
        for (slot, (st, w)) in prev.iter_mut().zip(layers) {
            *slot = (st, if total > 0.0 { w / total } else { 0.0 });
        }
        if prev.iter().all(|(_, w)| *w <= 0.0) {
            prev = [(self.state, 1.0), (self.state, 0.0)];
        }
        *self = BodyMemory { state, prev, switched_at: now, ..*self };
    }
}

/// The signed angle, in `(−π, π]`, that turns unit vector `from` onto `to` the short way;
/// 0 when either cannot be normalized.
pub fn signed_turn(from: Vec2, to: Vec2) -> f64 {
    let (Some(a), Some(b)) = (from.normalized(), to.normalized()) else { return 0.0 };
    let d = b.screen_angle() - a.screen_angle();
    let d = (d + std::f64::consts::PI).rem_euclid(std::f64::consts::TAU) - std::f64::consts::PI;
    if d.is_finite() { d } else { 0.0 }
}

/// The heading a body is drawn with a fraction `f` into its tick: the path's own direction
/// `dir` (from [`present::interpolate`], which already follows the chart across a seam)
/// turned back by the unspent part `(1 − f) · turn` of the turn the tick began with.
///
/// **Normative**: at `f = 0` this is the previous tick's end heading, at `f = 1` it is
/// `dir` itself, and in between it rotates the short way — so a body that the world turned
/// by 30° at a tick boundary sweeps those 30° over the tick's frames. `turn = 0` leaves
/// `dir` unchanged. A non-finite `f` reads as 0.
pub fn turn_heading(dir: Vec2, turn: f64, f: f64) -> Vec2 {
    let f = if f.is_finite() { f.clamp(0.0, 1.0) } else { 0.0 };
    if turn == 0.0 || !turn.is_finite() {
        return dir;
    }
    Vec2::from_screen_angle(dir.screen_angle() - (1.0 - f) * turn)
}

/// The live world drawn with the baked art. Holds the pack, the scratch buffers the
/// field and sprite paths need, the fixed per-cell slots, and the renderer-side history the
/// art image keeps: how far each cell's plant, each tall column and each body has got
/// toward what the world now says.
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
    /// How far each cell's plant has got toward the stage its field warrants: the target
    /// [`next_stage`] decides, the step in flight toward it, and the fruit blend. Advanced
    /// once per completed tick in [`ArtPresenter::observe`] and nowhere else;
    /// [`ArtPresenter::draw`] only reads it (a presenter that has never been observed snaps
    /// it to the view on its first draw).
    growth: Vec<Growth>,
    /// `growth` as the previous `observe` left it, so a frame at fraction `f` of the tick
    /// draws [`growth_between`] the two: growth is then continuous at the render rate, not
    /// stepped at 20 Hz. Equal to `growth` after a snap.
    growth_prev: Vec<Growth>,
    /// The water field, seam-filtered per pixel when drawn.
    water: ScalarField,
    /// The pack's tall species resolved once.
    tall_species: TallSpecies,
    /// The tall columns the hash selected, side faces in `Face::ALL` order.
    columns: Vec<TallColumn>,
    /// How tall each column has got, in segments, toward the target [`next_tall`] decides;
    /// advanced in `observe` exactly like `growth`.
    tall: Vec<TallGrowth>,
    /// `tall` as the previous `observe` left it, for [`tall_between`].
    tall_prev: Vec<TallGrowth>,
    /// The state each live body is in and when it last changed, for the cross-clip fade.
    /// Bodies the view no longer carries are dropped in `observe`.
    bodies: std::collections::HashMap<OrganismId, BodyMemory>,
    /// The measured bend budget of every plant and every tall family in the pack, by asset
    /// name, in pack order: plants first, then tall families. Measured once here
    /// ([`plant_bend_budget`], [`tall_bend_budget`]) and never per frame, so a pose that is
    /// wide only in one frame of one clip still cannot clip its footprint when the wind
    /// blows.
    budgets: Vec<(String, f64)>,
    /// The tick of the last [`ArtPresenter::observe`], `None` until the first one: what
    /// tells `observe` how much simulated time to advance, and what makes the first view a
    /// snap instead of a replay of a mature world's whole growth.
    last_tick: Option<u64>,
    /// The selected megafauna body, rasterized per frame for every hunter member.
    lanternjaw: Lanternjaw,
    /// The hunters the world's observer lists, by full generation-bearing id, in id order
    /// so two hunters always composite in the same order. Bounded by the membership list
    /// handed to [`ArtPresenter::observe_hunters`]; an id the list no longer carries is
    /// forgotten there. Never derived from `genome.form`.
    hunters: std::collections::BTreeMap<OrganismId, HunterMemory>,
    /// Scratch for the rig's parts, reused across frames.
    hunter_parts: Vec<Part>,
}

impl ArtPresenter {
    /// Build the presenter and lay out every cell's slot once.
    pub fn new(pack: ArtPack) -> ArtPresenter {
        let species = Species::resolve(&pack);
        let slots: Vec<Slot> = CellId::all().map(slot_of).collect();
        let bands = CellId::all().map(band_of).collect();
        let tall_species = TallSpecies::resolve(&pack);
        let columns = tall_columns();
        let tall = vec![TallGrowth { height: 0.0, target: 0 }; columns.len()];
        // The amplitude budgets of the shipped pack, measured once from its own pixels.
        let budgets: Vec<(String, f64)> = pack
            .plants
            .iter()
            .map(|p| (p.name.clone(), plant_bend_budget(p)))
            .chain(pack.tall.iter().map(|p| (p.name.clone(), tall_bend_budget(p))))
            .collect();
        ArtPresenter {
            pack,
            species,
            water: ScalarField::zeros(),
            tall_species,
            columns,
            tall_prev: tall.clone(),
            tall,
            producer: ScalarField::zeros(),
            detritus: ScalarField::zeros(),
            soil: ScalarField::zeros(),
            layer: Canvas::new(),
            scratch: Vec::new(),
            slots,
            bands,
            growth: vec![Growth::snapped(None, false); CELL_COUNT],
            growth_prev: vec![Growth::snapped(None, false); CELL_COUNT],
            bodies: std::collections::HashMap::new(),
            budgets,
            last_tick: None,
            lanternjaw: Lanternjaw::new(),
            hunters: std::collections::BTreeMap::new(),
            hunter_parts: Vec::new(),
        }
    }

    /// Whether this presenter can draw hunters of `profile`: [`validate_profile`], the
    /// renderer's capability stated before a world is stepped or drawn. A host asks this at
    /// load, with the saved world's own profile, and fails by name rather than clamping,
    /// substituting an ordinary body, or panicking mid-frame later.
    pub fn validate_hunter_profile(&self, profile: &FixedHunterProfile) -> Result<(), String> {
        validate_profile(profile)
    }

    /// Record this tick's hunter membership from the world's own observer, after
    /// [`ArtPresenter::observe`] of the same view, together with the hunter events the world
    /// committed at this tick.
    ///
    /// **Normative.** A member is a hunter by its full id in `hunters` and nothing else: an
    /// ordinary organism with the same `genome.form` is not one, and an entry whose id does
    /// not resolve to an organism of `view` (a stale generation) is ignored. A member whose
    /// published scale or contact geometry the rig cannot honour ([`validate_view`]) is an
    /// error, named: nothing is clamped and no ordinary body is substituted — a host that
    /// validated the profile at load never sees this. Every other member gets a
    /// [`HunterMemory`] — entered from its persisted `entered_from` when first seen, advanced
    /// by [`HunterMemory::observe`] after — and is drawn once, as the Lanternjaw, never also as
    /// its atelier rig. Memory of an id the list no longer carries is dropped, so the map is
    /// bounded by the membership. A target that leaves the view at the same boundary its
    /// hunter enters `Handling` is kept for that one tick's frames ([`HunterMemory::prey`]),
    /// carried to the `Capture` event's settlement position when the event names it, so a
    /// captured prey is not shown vanishing before the claws close; nothing is kept longer,
    /// and nothing is regenerated.
    ///
    /// **Idempotent and rewind-safe**: the same completed view observed again changes no
    /// memory and no frame; a view of an earlier tick (a rewind or a replaced world — the
    /// same rule [`ArtPresenter::observe`] snaps on) starts every memory over as first seen,
    /// so a later phase's reach never leaks into an earlier tick. With an empty list this
    /// changes nothing: the image is bit for bit the one a presenter never told about hunters
    /// draws.
    pub fn observe_hunters(
        &mut self,
        view: &RenderView,
        hunters: &[HunterView],
        events: &[HunterEvent],
    ) -> Result<(), String> {
        if self.hunters.values().any(|m| view.tick < m.cur.tick) {
            self.hunters.clear();
        }
        let live: std::collections::HashSet<OrganismId> =
            view.organisms.iter().map(|o| o.id).collect();
        let mut listed = Vec::with_capacity(hunters.len());
        for h in hunters {
            if !live.contains(&h.id) {
                continue;
            }
            validate_view(h)?;
            listed.push(h.id);
            let frame = HunterFrame::of(h, view.tick);
            let target_view = frame
                .target
                .and_then(|t| view.organisms.iter().find(|o| o.id == t))
                .cloned();
            match self.hunters.get_mut(&h.id) {
                Some(memory) if memory.cur.tick == view.tick => {
                    // The same completed view again: one observation, nothing moves.
                }
                Some(memory) => {
                    memory.observe(frame);
                    let captured = memory.prev.as_ref().is_some_and(|p| {
                        p.target.is_some_and(|t| !live.contains(&t))
                            && frame.phase == HunterPhase::Handling
                            && frame.started == view.tick
                    });
                    memory.prey = if captured { memory.target_view.take() } else { None };
                    memory.prey_at = None;
                    memory.target_view = target_view;
                }
                None => {
                    let mut memory = HunterMemory::enter(frame);
                    memory.target_view = target_view;
                    self.hunters.insert(h.id, memory);
                }
            }
        }
        self.hunters.retain(|id, _| listed.contains(id));
        for event in events {
            if let HunterEvent::Capture { tick, hunter, prey, evidence, .. } = event
                && *tick == view.tick
                && let Some(memory) = self.hunters.get_mut(hunter)
            {
                memory.note_capture(*prey, evidence.prey_pos);
            }
        }
        Ok(())
    }

    /// The adapter's memory of a hunter, if it is drawn as the Lanternjaw.
    pub fn hunter_of(&self, id: OrganismId) -> Option<&HunterMemory> {
        self.hunters.get(&id)
    }

    /// The hunters currently drawn as the Lanternjaw, in composite order.
    pub fn hunter_ids(&self) -> Vec<OrganismId> {
        self.hunters.keys().copied().collect()
    }

    /// The measured amplitude budget of an asset, in tile pixels: the largest `|amplitude|`
    /// every frame of every clip of that plant or tall family can be bent by without any
    /// painted texel leaving the nine-pixel stamp footprint ([`cubarium_render::Sprite::bend_headroom`],
    /// [`plant_bend_budget`], [`tall_bend_budget`]).
    ///
    /// **Normative**: measured once when the presenter is built and constant for its life;
    /// [`f64::INFINITY`] for an asset nothing bounds, and **0** for a name this pack does not
    /// carry, so an unknown asset stands still rather than moving on an unmeasured budget.
    pub fn bend_budget(&self, name: &str) -> f64 {
        budget_in(&self.budgets, name)
    }

    /// Every measured budget, in pack order (plants, then tall families): the table a review
    /// session reads to see which species the pack's own art is holding back.
    pub fn bend_budgets(&self) -> &[(String, f64)] {
        &self.budgets
    }

    /// The one budget a whole tall column shares: the smaller of its own family's and, when
    /// it carries a vine, the vine's. Every part of the column bends by the same amplitude,
    /// so the vine's art bounds the tree's motion as much as the tree's own does.
    pub fn column_budget(&self, column: &TallColumn) -> f64 {
        let own = self.bend_budget(TALL_PLANTS[column.pick]);
        if column.vine { own.min(self.bend_budget(VINE_PLANT)) } else { own }
    }

    /// The growth of a cell's plant as the previous `observe` left it (equal to
    /// [`ArtPresenter::growth_of`] after a snap): what a frame at fraction `f` interpolates
    /// from.
    pub fn growth_prev_of(&self, cell: CellId) -> Growth {
        self.growth_prev[cell.index()]
    }

    /// What the presenter remembers of a body, if it has seen it.
    pub fn body_of(&self, id: OrganismId) -> Option<BodyMemory> {
        self.bodies.get(&id).copied()
    }

    /// The loaded pack, for tests that want to compare a drawn body or plant against its
    /// sprite.
    pub fn pack(&self) -> &ArtPack {
        &self.pack
    }

    /// The stage a cell's plant is *headed for* after the last `observe`: what the field
    /// warrants once [`next_stage`]'s hysteresis has had its say, `None` for bare ground.
    /// The visual may still be part-way there — see [`ArtPresenter::growth_of`].
    pub fn stage_of(&self, cell: CellId) -> Option<u8> {
        self.growth[cell.index()].target
    }

    /// The whole paced growth of a cell's plant: where the visual is, where it is going and
    /// how far along it is.
    pub fn growth_of(&self, cell: CellId) -> Growth {
        self.growth[cell.index()]
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

    /// Record one completed tick: retarget every cell's plant and every column to what its
    /// field now warrants ([`next_stage`] and [`next_tall`], with their hysteresis), and
    /// move the visuals that much closer to it. This is the only place the art image's
    /// history advances.
    ///
    /// **Normative**: the step is `dt = (view.tick − last_tick) · DT` clamped into `[0,
    /// `[`MAX_STEP_SECONDS`]`]`. The presenter **snaps** instead — every visual set to its
    /// target, every body entered with no fade, the previous-state copies made equal — when
    /// this is the first view (a mature world must not replay its growth from bare ground)
    /// or when the tick went backwards (a replaced or rewound world). A snap derives its
    /// targets from the same empty baseline a new presenter would (`next_stage(None, ..)`,
    /// `next_tall(0, ..)`), so the old world's hysteresis does not leak into the new one and
    /// restoring a view into a used presenter draws what a fresh presenter would. A cell
    /// whose band changed starts over from bare ground and then grows paced.
    pub fn observe(&mut self, view: &RenderView) {
        self.observe_with_fruit(view, Some(&view.fruit));
    }

    /// [`ArtPresenter::observe`] against a fruit field other than the view's own, the way
    /// [`ArtPresenter::draw_with_fruit`] draws against one: the fruit accent is paced here,
    /// not at draw time, so a test that wants to see a plant in fruit must observe the
    /// fruit. `None` is "the world publishes no fruit yet".
    pub fn observe_with_fruit(&mut self, view: &RenderView, fruit: Option<&[f64]>) {
        let snap = match self.last_tick {
            None => true,
            Some(last) => view.tick < last,
        };
        if snap {
            // A rewind or a replaced world: the hunters' memory starts over with everything
            // else, so no later phase's reach can leak back in time.
            self.hunters.clear();
        }
        let dt = match self.last_tick {
            Some(last) if view.tick >= last => {
                (((view.tick - last) as f64) * DT).clamp(0.0, MAX_STEP_SECONDS)
            }
            _ => 0.0,
        };
        // The previous-state copies are "as the previous *tick* left it": a repeated
        // observe of the same tick retargets but must not fold them forward, or a frame
        // drawn mid-tick would stop interpolating from the tick before.
        let new_tick = self.last_tick != Some(view.tick);
        for (index, cell) in CellId::all().enumerate() {
            let band = cell_band(cell, view.water.get(index).copied());
            if band != self.bands[index] {
                self.bands[index] = band;
                // The new band's plant is a different plant: it starts from bare ground.
                self.growth[index] = Growth::snapped(None, false);
            }
            let t = plant_density(view, index, band);
            let from_target = if snap { None } else { self.growth[index].target };
            let target = plant_cap(band, cell)
                .and_then(|cap| next_stage(from_target, t, &stage_thresholds(band), cap));
            let in_fruit = fruit_stage(fruit.and_then(|f| f.get(index).copied()));
            if new_tick {
                self.growth_prev[index] = self.growth[index];
            }
            self.growth[index] = if snap {
                Growth::snapped(target, in_fruit)
            } else {
                advance_growth(self.growth[index], target, in_fruit, dt)
            };
            if snap {
                self.growth_prev[index] = self.growth[index];
            }
        }
        for (i, column) in self.columns.iter().enumerate() {
            let t_col = column_density(view, column.face, column.cx);
            let from_target = if snap { 0 } else { self.tall[i].target };
            let target = next_tall(from_target, t_col);
            if new_tick {
                self.tall_prev[i] = self.tall[i];
            }
            self.tall[i] = if snap {
                TallGrowth { height: f64::from(target), target }
            } else {
                advance_tall(self.tall[i], target, dt)
            };
            if snap {
                self.tall_prev[i] = self.tall[i];
            }
        }
        self.observe_bodies(view, snap);
        self.last_tick = Some(view.tick);
    }

    /// The body memory: a state change records when it happened and the blend it started
    /// from ([`BodyMemory::switch_to`]), an unknown id enters with no fade, and an id the
    /// view no longer carries is forgotten.
    fn observe_bodies(&mut self, view: &RenderView, snap: bool) {
        if snap {
            self.bodies.clear();
        }
        let now = present_seconds(view.tick, 0.0);
        for o in &view.organisms {
            let state = state_of(o);
            let start = present::interpolate(&o.moved, o.pos, o.heading, 0.0).1;
            let end = present::interpolate(&o.moved, o.pos, o.heading, 1.0).1;
            match self.bodies.get_mut(&o.id) {
                Some(memory) => {
                    if memory.state != state {
                        memory.switch_to(state, now);
                    }
                    memory.observe_heading(start, end);
                }
                None => {
                    let mut memory = BodyMemory::entered(state);
                    // A body just seen has no previous heading to turn from.
                    memory.end_heading = end;
                    self.bodies.insert(o.id, memory);
                }
            }
        }
        if self.bodies.len() > view.organisms.len() {
            let live: std::collections::HashSet<OrganismId> =
                view.organisms.iter().map(|o| o.id).collect();
            self.bodies.retain(|id, _| live.contains(id));
        }
    }

    /// The trunk segments a tall column is *headed for* after the last `observe` (0 when
    /// nothing tall stands there), by its index in [`tall_columns`]. The column may still be
    /// growing toward it — see [`ArtPresenter::tall_growth_of`].
    pub fn segments_of(&self, column: usize) -> u8 {
        self.tall.get(column).map_or(0, |g| g.target)
    }

    /// The paced height of a tall column: how tall it is drawn now and what it is headed
    /// for.
    pub fn tall_growth_of(&self, column: usize) -> TallGrowth {
        self.tall.get(column).copied().unwrap_or(TallGrowth { height: 0.0, target: 0 })
    }

    /// The tall columns this presenter draws, in the order [`segments_of`] indexes.
    pub fn columns(&self) -> &[TallColumn] {
        &self.columns
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
    ///
    /// The fruit field passed here does two things: it snap-initializes a presenter that
    /// has never been observed, and it **gates** the accent at draw time — a cell the field
    /// says is not in fruit ([`fruit_stage`]) draws no fruit whatever the paced
    /// [`Growth::fruit`] says, so `None` always suppresses the accent and the picture never
    /// shows food the field does not hold. The fade *in* is the paced value
    /// [`ArtPresenter::observe_with_fruit`] advanced.
    ///
    /// # A stage step in flight
    ///
    /// **Normative**, and the whole of the growth pilot. A cell whose [`Growth`] is in flight
    /// this frame is decomposed by [`growth_step`] into its `lower` stage, its `upper` stage
    /// and the upper stage's progress `t` — one number that runs 0 → 1 up the step whichever
    /// way the step is travelling. Then, with `plant` the cell's species:
    ///
    /// * When `lower` is a stage and [`Plant::transition`]`(lower, upper)` is `Some(clip)` —
    ///   an **authored growth clip**, pack v5 — the step is drawn as **one**
    ///   [`cubarium_render::stamp_layers_bent`] of three layers at the weights
    ///   [`growth_weights`]`(t)` gives: `[(stage_pose(lower), w_from), (clip.sample(t ·
    ///   clip.seconds), w_grow), (stage_pose(upper), w_to)]` — `stage_pose` being the stage's
    ///   own looping sway clip at the slot's phase, exactly the pose an idle plant shows at
    ///   that instant — with [`Mask::None`], opacity
    ///   `opacity_of(lower) + (opacity_of(upper) − opacity_of(lower)) · t`
    ///   ([`stage_opacity`] at the cell's own density), and the slot's wind — the same
    ///   `(`[`cubarium_render::Bend`]`, heading)` [`slot_wind`] gives every other stamp of
    ///   that slot this frame, so the breeze carries on right through the growth. Nothing
    ///   here is masked: the clip's own art says what a half-grown plant looks like. Top-face
    ///   (radial) slots with a clip follow exactly the same rule.
    /// * Otherwise — every pair the pack has no clip for, the bare-ground step (`None ↔ 0`,
    ///   which has no lower stage to blend from), and every plant of a pack before v5 — the
    ///   step keeps the **reveal masks**: the lower stage stamped whole at `opacity_of(lower)
    ///   · (1 − t)` and the upper stage over it at `opacity_of(upper)` through
    ///   [`Mask::Axial`]`{ reveal: t · `[`PLANT_REVEAL_PX`]` }` on a side face or
    ///   [`Mask::Radial`]`{ reveal: t · (extent + 0.5) }` outward from the pivot on the top
    ///   face, `extent` being the largest [`cubarium_render::Pose::extent`] of the upper
    ///   stage's layers — so `t = 1` is exactly [`Mask::None`] for that pose.
    ///
    /// The clip is a pure function of `t` and is therefore never restarted, resumed or
    /// advanced by anything a frame does: a repeated draw of the same (state, view, `f`) is
    /// the same image, a reversal replays the same `t` backwards ([`growth_step`]), a wind
    /// packet arriving mid-step changes only the bend, and pausing holds the pose. The fruit
    /// accent takes no part in an authored step: [`advance_growth`] holds it at 0 while a
    /// plant is in flight and the three-layer stamp has no fruit layer, so an authored
    /// growth stamp never reads the `fruit` clip. On the *mask* path a full-grown plant in
    /// fruit that turns round still carries the accent [`growth_between`] interpolates from
    /// the previous tick for the first frames of the step (about two frames at 60 fps, only
    /// for a fruiting species whose 1 → 2 step has no clip — none on the shipped pack since
    /// the canopy species were authored on 2026-09-13; a v1–v4 pack's bloomcrown).
    pub fn draw_with_fruit(
        &mut self,
        view: &RenderView,
        f: f64,
        canvas: &mut Canvas,
        fruit: Option<&[f64]>,
    ) {
        // A presenter that has never seen a tick snaps to this view, exactly as the first
        // `observe` would: a mature world is drawn as it is, not replayed from bare ground.
        // After that `draw` mutates nothing, so a thousand draws advance nothing.
        if self.last_tick.is_none() {
            self.observe_with_fruit(view, fruit);
        }
        let seconds = present_seconds(view.tick, f);

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
                    let pose = ground_pose(tile, seconds + ground_phase_of(face, x, y, tile.seconds));
                    stamp_pose(
                        canvas,
                        point,
                        Vec2::new(1.0, 0.0),
                        pose,
                        1.0,
                        opacity,
                        Mask::None,
                        scratch,
                    );
                }
            }
        }

        // Water: pools and streams source-over the ground, under the plants.
        if !view.water.is_empty() {
            present::copy_field(&mut self.water, &view.water);
            let saturation = view.producer_max * PRODUCER_SATURATION;
            draw_water(canvas, &self.water, &self.producer, saturation, seconds);
        }

        // Plants: scenery that follows the fields. These are not organisms — nothing in
        // the world knows about them, they never move, and they are not eaten. They are
        // how a rich cell reads as overgrown rather than as merely brighter.
        let pack = &self.pack;
        let species = &self.species;
        let budgets = &self.budgets;
        let scratch = &mut self.scratch;
        for (index, cell) in CellId::all().enumerate() {
            // The growth this frame shows: between the last two observed states, at `f`.
            let growth = growth_between(self.growth_prev[index], self.growth[index], f);
            if growth.from.is_none() && growth.to.is_none() {
                continue;
            }
            let band = self.bands[index];
            let slot = &self.slots[index];
            let Some(plant) = species.index(band, slot.pick).map(|i| &pack.plants[i]) else {
                continue;
            };
            let t = plant_density(view, index, band);
            let thresholds = stage_thresholds(band);
            let ceiling = band_opacity(band);
            let opacity_of =
                |stage: u8| stage_opacity(stage, t, &thresholds, ceiling);
            // The accent is gated by the field this frame is drawn against: the paced value
            // only ever fades it *in*.
            let fruit_now = if fruit_stage(fruit.and_then(|f| f.get(index).copied())) {
                growth.fruit
            } else {
                0.0
            };
            // The shared breeze, once for this slot this frame: a side-face plant bends
            // along its own tile's horizontal axis, a radial top-face plant turns in place,
            // and a species with no response takes neither.
            let (bend, heading) =
                slot_wind(slot, &plant.name, budget_in(budgets, &plant.name), seconds);
            if growth.from == growth.to {
                // Idle: one stage, whole, with the fruit accent blended in where it holds.
                let stage = growth.to.expect("an idle bare slot was skipped above");
                let opacity = opacity_of(stage);
                if opacity > 0.0 {
                    let layers = stage_layers(plant, stage, cell, seconds, fruit_now);
                    stamp_layers_bent(
                        canvas,
                        slot.at,
                        heading,
                        &layers,
                        1.0,
                        opacity,
                        Mask::None,
                        bend,
                        scratch,
                    );
                }
                continue;
            }
            // In flight. Where the pack carries an authored growth clip for this step that
            // clip *is* the picture, blended into the two idle stage clips at its ends;
            // otherwise the lower stage fades out under the higher one, which is revealed
            // along the stalk on a side face and outward from its centre on the top face.
            let Some(GrowthStep { lower, upper, t: gu }) = growth_step(growth) else { continue };
            if let Some((low, clip)) =
                lower.and_then(|low| plant.transition(low, upper).map(|clip| (low, clip)))
            {
                let under = opacity_of(low);
                let opacity = under + (opacity_of(upper) - under) * gu as f32;
                if opacity > 0.0 {
                    let [w_from, w_grow, w_to] = growth_weights(gu);
                    // One stamp: the step's own art, held between the two idle clips. A
                    // layer at weight 0 is not sampled, so each end of the step reads one
                    // clip and costs one.
                    let layers = [
                        (stage_pose(plant, low, cell, seconds), w_from),
                        (clip.sample(gu * clip.seconds), w_grow),
                        (stage_pose(plant, upper, cell, seconds), w_to),
                    ];
                    stamp_layers_bent(
                        canvas,
                        slot.at,
                        heading,
                        &layers,
                        1.0,
                        opacity,
                        Mask::None,
                        bend,
                        scratch,
                    );
                }
                continue;
            }
            if let Some(stage) = lower {
                let opacity = opacity_of(stage) * (1.0 - gu) as f32;
                if opacity > 0.0 {
                    let layers = stage_layers(plant, stage, cell, seconds, fruit_now);
                    stamp_layers_bent(
                        canvas,
                        slot.at,
                        heading,
                        &layers,
                        1.0,
                        opacity,
                        Mask::None,
                        bend,
                        scratch,
                    );
                }
            }
            {
                let stage = upper;
                let opacity = opacity_of(stage);
                if opacity > 0.0 {
                    let layers = stage_layers(plant, stage, cell, seconds, fruit_now);
                    let mask = match up_of(cell) {
                        // A stalk stands on the tile's bottom edge and grows upward.
                        Some(_) => Mask::Axial { reveal: gu * PLANT_REVEAL_PX },
                        // A radial top-face plant opens from its centre.
                        None => Mask::Radial { reveal: gu * (layers_extent(&layers) + 0.5) },
                    };
                    stamp_layers_bent(
                        canvas, slot.at, heading, &layers, 1.0, opacity, mask, bend, scratch,
                    );
                }
            }
        }

        // Tall plants: columns of base, trunks and crown up the side faces, the crown of a
        // full column carried onto the top face by the shared surface.
        {
            let tall = &self.tall_species;
            for (i, column) in self.columns.iter().enumerate() {
                let Some(plant) = tall.plants[column.pick].map(|p| &pack.tall[p]) else { continue };
                let vine = tall.vine.map(|v| &pack.tall[v]);
                let height = tall_between(self.tall_prev[i], self.tall[i], f).height;
                // One wind sample at the column's base anchor, one amplitude for every part
                // of it, bounded by the family's budget and the vine's together.
                let budget = {
                    let own = budget_in(budgets, TALL_PLANTS[column.pick]);
                    if column.vine { own.min(budget_in(budgets, VINE_PLANT)) } else { own }
                };
                let amplitude = tall_amplitude(column, budget, seconds);
                draw_column(canvas, column, height, plant, vine, seconds, amplitude, scratch);
            }
        }

        // Rain: the streaks at this frame's own instant, so they fall continuously.
        if !view.rain.is_empty() {
            draw_rain(canvas, &view.rain, seconds);
        }

        // Bodies: the organism's real state, cross-faded for `BODY_FADE_SECONDS` after a
        // change so a body does not cut from one clip to another. Every clip in the fade
        // keeps its own temporal blend ([`Clip::sample`]); the layers are mixed in one stamp.
        // A hunter member drawn as the Lanternjaw is skipped here: each body is drawn once.
        for o in &view.organisms {
            if self.hunters.contains_key(&o.id) {
                continue;
            }
            stamp_creature(&self.pack, &mut self.scratch, o, self.bodies.get(&o.id), seconds, f, canvas);
        }

        // A prey the world removed at this tick's boundary while its hunter entered
        // `Handling`: still on screen for the frames before that boundary, carried from its
        // last published pose to the settlement position the `Capture` event names
        // ([`HunterMemory::retained_prey_pose`]), gone at `f = 1`.
        if f < 1.0 {
            for memory in self.hunters.values() {
                if let (Some(prey), Some((pos, heading))) =
                    (&memory.prey, memory.retained_prey_pose(f))
                {
                    let held = OrganismView {
                        pos,
                        heading,
                        moved: Vec::new(),
                        ..prey.clone()
                    };
                    stamp_creature(&self.pack, &mut self.scratch, &held, None, seconds, 1.0, canvas);
                }
            }
        }

        // The hunters, in id order, over the ordinary bodies: the living pose from the
        // adapter's memory of the world's own phases, the root along the same interpolated
        // path and turn every body uses, the whole rig at the authoritative scale.
        for (id, memory) in &self.hunters {
            let Some(o) = view.organisms.iter().find(|o| o.id == *id) else { continue };
            let (anchor, dir) = present::interpolate(&o.moved, o.pos, o.heading, f);
            let heading = turn_heading(dir, self.bodies.get(id).map_or(0.0, |m| m.turn), f);
            let (pose, scale) = memory.living_pose(view.tick, f, &o.moved);
            self.lanternjaw.draw_living(
                canvas,
                anchor,
                heading,
                &pose,
                scale,
                1.0,
                &mut self.hunter_parts,
                &mut self.scratch,
            );
        }
    }
}

/// One ordinary creature at fraction `f` of its tick: exactly the stamp the body loop has
/// always made, with `memory` supplying the cross-fade and the turn (none for a body with
/// no memory).
fn stamp_creature(
    pack: &ArtPack,
    scratch: &mut Vec<PixelImage>,
    o: &OrganismView,
    memory: Option<&BodyMemory>,
    seconds: f64,
    f: f64,
    canvas: &mut Canvas,
) {
    let form = rig_of(o.form, o.hue, pack.creature_count());
    let state = state_of(o);
    let pose_of = |st: usize| {
        let clip = &pack.clips[form * 4 + st];
        // The bud clip's last frame *is* the birth, so a body that has just stopped
        // budding fades out of that frame rather than out of a half-grown bud.
        let gestation = if st == 3 && st != state { Some(1.0) } else { o.gestation };
        clip.sample(clip_time(clip, seconds, phase_of(o.id, clip.seconds), gestation))
    };
    let (anchor, dir) = present::interpolate(&o.moved, o.pos, o.heading, f);
    // The turn the tick began with is spent over the tick's frames.
    let heading = turn_heading(dir, memory.map_or(0.0, |m| m.turn), f);
    let scale = if o.juvenile { JUVENILE_SCALE } else { 1.0 };
    let states = match memory {
        Some(memory) if memory.fade_at(seconds) < 1.0 => memory.layers_at(seconds),
        _ => vec![(state, 1.0)],
    };
    let layers: Vec<(Pose<'_>, f32)> = states.iter().map(|&(st, w)| (pose_of(st), w)).collect();
    stamp_layers(canvas, anchor, heading, &layers, scale, 1.0, Mask::None, scratch);
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
        let seconds = present_seconds(view.tick, 0.0);
        let pose = clip.sample(clip_time(clip, seconds, phase_of(o.id, clip.seconds), None));
        stamp_pose(&mut expected, o.pos, o.heading, pose, 1.0, 1.0, Mask::None, &mut vec![]);
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
        assert_eq!(clip_time(bud, 0.0, 0.0, Some(0.0)), 0.0);
        assert_eq!(clip_time(bud, 617.25, 1.7, Some(0.5)), 0.5 * bud.seconds);
        assert_eq!(clip_time(bud, 617.25, 1.7, Some(1.0)), bud.seconds);
        // Wall time is nowhere in it: the presentation seconds and the phase are the only
        // inputs, and the presentation seconds are the tick's own.
        let walk = &art.clips[1];
        assert!(walk.looping);
        assert_eq!(clip_time(walk, 0.0, 0.25, None), 0.25);
        assert_eq!(present_seconds(21, 0.0), 20.0 * DT);
        assert_eq!(clip_time(walk, present_seconds(21, 0.0), 0.25, None), 20.0 * DT + 0.25);
        // Gestation is ignored by a looping clip, and a doubled tick rate doubles the
        // clip's progress, which is what makes `--speed` honest.
        assert_eq!(clip_time(walk, 40.0 * DT, 0.0, Some(0.5)), 40.0 * DT);
        // A frame between ticks lands between the two tick instants, continuously.
        assert_eq!(present_seconds(21, 0.5), 20.5 * DT);
        assert_eq!(present_seconds(21, 1.0), present_seconds(22, 0.0));
        assert_eq!(present_seconds(0, 0.0), 0.0, "tick 0 cannot go below zero");
        assert_eq!(present_seconds(9, f64::NAN), 8.0 * DT, "a nonsense fraction reads as 0");
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
        let at_birth = clip_time(bud, 0.0, 0.0, Some(1.0));
        assert!(std::ptr::eq(bud.at(at_birth), bud.frames.last().unwrap()));
        assert!(std::ptr::eq(
            bud.at(at_birth),
            art.creature(form_of(0.5), 3, bud.seconds),
        ));
        // And a gestation of 0 is its first frame, so the clip really does run.
        assert!(std::ptr::eq(bud.at(clip_time(bud, 0.0, 0.0, Some(0.0))), &bud.frames[0]));
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

    // --- Wind ------------------------------------------------------------------------
    //
    // These check the *global* sampler [`wind_strength`] (a pure function of presentation
    // seconds, the same everywhere on the cube), the *chart field* [`wind_chart`] (a pure
    // function of position), and the *delayed per-root sampler* [`wind_at`] (the product of
    // the two, at a time shifted by the part's lag and the point's own spatial phase, so its
    // quiet interval sits a fraction of a second away from the global one). Each test says
    // which.

    #[test]
    fn the_global_gust_is_bounded_rests_exactly_and_never_jerks() {
        // wind_strength, the global sampler.
        let mut peak = 0.0f64;
        let mut quiet = 0;
        let mut samples = 0;
        for i in 0..600_000 {
            let s = f64::from(i) * 0.001;
            let w = wind_strength(s);
            assert!((0.0..=1.0).contains(&w), "strength {w} at {s}");
            peak = peak.max(w);
            if w == 0.0 {
                quiet += 1;
            }
            samples += 1;
        }
        assert!(peak > 0.8, "the packet never reaches its peak: {peak}");
        // The quiet interval is exactly the period less the packet, to the sampling.
        let share = f64::from(quiet) / f64::from(samples);
        let want = WIND_QUIET_SECONDS / WIND_PERIOD;
        assert!((share - want).abs() < 0.01, "calm for {share:.3} of the time, not {want:.3}");
        // Exactly zero, not nearly: every instant of the quiet interval of ten packets.
        for packet in 0..10 {
            let start = f64::from(packet) * WIND_PERIOD + WIND_RISE + WIND_HOLD + WIND_FALL;
            for k in 0..=100 {
                let s = start + WIND_QUIET_SECONDS * f64::from(k) / 100.0;
                // The end of the quiet interval is the next packet's start, also zero.
                assert_eq!(wind_strength(s), 0.0, "not resting at {s}");
            }
        }
        assert_eq!(wind_strength(f64::NAN), 0.0);
        assert_eq!(wind_strength(f64::INFINITY), 0.0);
        // Continuous with a continuous slope at all four boundaries of the packet: the
        // one-sided differences agree, and the edges have (almost) no slope at all.
        let h = 1e-4;
        for packet in 0..3 {
            let base = f64::from(packet) * WIND_PERIOD;
            for edge in [0.0, WIND_RISE, WIND_RISE + WIND_HOLD, WIND_RISE + WIND_HOLD + WIND_FALL] {
                let t = base + edge;
                let (l, m, r) = (wind_strength(t - h), wind_strength(t), wind_strength(t + h));
                assert!((m - l).abs() < 1e-3 && (r - m).abs() < 1e-3, "a jump at {t}");
                let slope = |a: f64, b: f64| (b - a) / h;
                assert!(
                    (slope(l, m) - slope(m, r)).abs() < 0.1,
                    "a kink at {t}: {} vs {}",
                    slope(l, m),
                    slope(m, r)
                );
            }
            // The packet's own edges have zero value *and* zero slope.
            for edge in [0.0, WIND_RISE + WIND_HOLD + WIND_FALL] {
                let t = base + edge;
                assert_eq!(wind_strength(t), 0.0);
                assert!(wind_strength(t + h).abs() < 1e-6 && wind_strength(t - h).abs() < 1e-6);
            }
        }
        // A 60 fps step through a whole packet never moves the strength by much.
        let mut last = wind_strength(0.0);
        for frame in 1..=(WIND_PERIOD * 60.0) as i32 {
            let w = wind_strength(f64::from(frame) / 60.0);
            assert!((w - last).abs() < 0.02, "frame {frame}: {last} → {w}");
            last = w;
        }
    }

    #[test]
    fn the_chart_field_joins_across_every_seam_under_the_real_tangent_transport() {
        // wind_chart, verified against `travel`'s own tangent map rather than a second seam
        // table: step across each connected seam and compare the transported vector with the
        // field on the far side.
        let eps = 1e-7;
        let mut crossings = 0;
        for face in Face::ALL {
            for edge in cubarium_surface::Edge::ALL {
                if face != Face::Top && edge == cubarium_surface::Edge::Bottom {
                    continue; // the open rim reflects; it is not a seam
                }
                for k in 0..64 {
                    let along = f64::from(k) + 0.5;
                    let (u, v, step) = match edge {
                        cubarium_surface::Edge::Top => (along, eps, Vec2::new(0.0, -2.0 * eps)),
                        cubarium_surface::Edge::Right => {
                            (64.0 - eps, along, Vec2::new(2.0 * eps, 0.0))
                        }
                        cubarium_surface::Edge::Bottom => {
                            (along, 64.0 - eps, Vec2::new(0.0, 2.0 * eps))
                        }
                        cubarium_surface::Edge::Left => (eps, along, Vec2::new(-2.0 * eps, 0.0)),
                    };
                    let here = SurfacePoint::new(face, u, v);
                    let crossed = cubarium_surface::travel(here, step);
                    assert_eq!(crossed.crossings, 1, "{face:?} {edge:?} is not one seam");
                    let mine = wind_chart(here.face, here.u, here.v);
                    let theirs = wind_chart(crossed.end.face, crossed.end.u, crossed.end.v);
                    let transported = crossed.map.apply(mine);
                    assert!(
                        (transported - theirs).length() < 1e-6,
                        "{face:?} {edge:?} at {along}: {transported:?} vs {theirs:?}"
                    );
                    crossings += 1;
                }
            }
        }
        assert_eq!(crossings, 16 * 64, "sixteen connected half-edges, 64 positions each");

        // Exactly zero where the design says so: every side/side seam (a side face's whole
        // left and right edge) and each of Top's four vertices.
        for face in [Face::Front, Face::Right, Face::Back, Face::Left] {
            for k in 0..64 {
                let v = f64::from(k) + 0.5;
                assert_eq!(wind_chart(face, 0.0, v), Vec2::ZERO, "{face:?} left seam");
                assert_eq!(wind_chart(face, 64.0, v), Vec2::ZERO, "{face:?} right seam");
            }
        }
        for u in [0.0, 64.0] {
            for v in [0.0, 64.0] {
                assert_eq!(wind_chart(Face::Top, u, v), Vec2::ZERO, "Top vertex ({u},{v})");
            }
        }
        assert_eq!(wind_chart(Face::Top, 32.0, 32.0), Vec2::ZERO, "Top's centre is calm");
        // And nowhere does it exceed the documented maximum, which it attains.
        let mut worst = 0.0f64;
        for face in Face::ALL {
            for i in 0..=128 {
                for j in 0..=128 {
                    let (u, v) = (f64::from(i) * 0.5, f64::from(j) * 0.5);
                    worst = worst.max(wind_chart(face, u, v).length());
                }
            }
        }
        assert!(worst <= WIND_CHART_MAX + 1e-12, "|W| reached {worst}");
        assert!(worst > WIND_CHART_MAX - 1e-9, "|W| never reaches its maximum: {worst}");
        assert_eq!(wind_chart(Face::Front, f64::NAN, 3.0), Vec2::ZERO);
    }

    #[test]
    fn the_delayed_sampler_rests_with_the_packet_and_stays_inside_the_chart_maximum() {
        // wind_at: the chart field times the global strength at a *shifted* time, so its own
        // quiet interval is the global one moved by the lag and the point's spatial phase —
        // at most `WIND_TRAVEL_SECONDS` of the 12-second rest, which is why the middle of a
        // quiet interval is calm everywhere at once.
        let middle = WIND_RISE + WIND_HOLD + WIND_FALL + WIND_QUIET_SECONDS / 2.0;
        for face in Face::ALL {
            for i in 0..8 {
                for j in 0..8 {
                    let p = SurfacePoint::new(face, f64::from(i) * 8.0 + 0.5, f64::from(j) * 8.0 + 0.5);
                    assert!(wind_phase(p).abs() <= 1.0, "phase out of range at {p:?}");
                    for lag in [0.0, 0.05, 0.1, 0.15, 0.2] {
                        assert_eq!(wind_at(p, middle, lag), Vec2::ZERO, "{p:?} lag {lag}");
                        for k in 0..60 {
                            let s = f64::from(k) * 0.5;
                            let w = wind_at(p, s, lag);
                            assert!(w.length() <= WIND_CHART_MAX + 1e-12, "{p:?} at {s}");
                        }
                    }
                }
            }
        }
        // The lag really delays: a point in the middle of a face, during the rise, answers
        // later with a larger lag.
        let p = SurfacePoint::new(Face::Front, 32.0, 32.0);
        let during = WIND_RISE * 0.5;
        assert!(wind_at(p, during, 0.0).length() > wind_at(p, during, 0.2).length());
        // And the spatial phase really varies: two points on opposite sides of the cube
        // answer at different times.
        let far = SurfacePoint::new(Face::Back, 32.0, 32.0);
        assert!((wind_phase(p) - wind_phase(far)).abs() > 0.5);
        assert_eq!(wind_at(p, f64::NAN, 0.0), Vec2::ZERO);
        assert_eq!(wind_at(p, 6.0, f64::NAN).length(), wind_at(p, 6.0, 0.0).length());
    }

    /// Which clip and frame of a plant is the one holding its budget down — the art a
    /// review session would have to narrow to buy more movement.
    fn limiting_clip(art: &ArtPack, name: &str) -> String {
        let mut worst = (f64::INFINITY, String::from("nothing"));
        let mut consider = |label: String, clip: &Clip, base: f64, root: f64, length: f64| {
            for (i, frame) in clip.frames.iter().enumerate() {
                let room = frame.bend_headroom(root, length, base);
                if room < worst.0 {
                    worst = (room, format!("{label} frame {i}"));
                }
            }
        };
        if let Some(plant) = art.plant(name) {
            for (i, clip) in plant.stages.iter().enumerate() {
                consider(format!("stage {i}"), clip, 0.0, PLANT_BEND_ROOT, PLANT_BEND_LENGTH);
            }
            if let Some(clip) = &plant.fruit {
                consider("fruit".into(), clip, 0.0, PLANT_BEND_ROOT, PLANT_BEND_LENGTH);
            }
            for t in &plant.transitions {
                consider(
                    format!("grow {}→{}", t.from, t.to),
                    &t.clip,
                    0.0,
                    PLANT_BEND_ROOT,
                    PLANT_BEND_LENGTH,
                );
            }
        }
        if let Some(plant) = art.tall_plant(name) {
            let top = f64::from(TALL_MAX_SEGMENTS);
            consider("trunk".into(), &plant.trunk, tall_bend_base(top), TALL_BEND_ROOT, TALL_BEND_LENGTH);
            if let Some(clip) = &plant.base {
                consider("base".into(), clip, tall_bend_base(0.0), TALL_BEND_ROOT, TALL_BEND_LENGTH);
            }
            if let Some(clip) = &plant.cap {
                consider(
                    "cap".into(),
                    clip,
                    tall_bend_base(top + 1.0),
                    TALL_BEND_ROOT,
                    TALL_BEND_LENGTH,
                );
            }
        }
        worst.1
    }

    #[test]
    fn the_shipped_pack_admits_a_bend_for_every_species_that_wants_one() {
        let presenter = ArtPresenter::new(pack());
        let art = pack();
        for (name, budget) in presenter.bend_budgets() {
            println!("  {name:<14} budget {budget:>6.2}  held by its {}", limiting_clip(&art, name));
        }
        println!("\n  asset          budget   desired   effective (±10 %)");
        for (name, budget) in presenter.bend_budgets() {
            let r = wind_response(name);
            let eff = effective_tip(r.tip_px, *budget);
            println!(
                "  {name:<14} {budget:>6.2}   {:>7.2}   {eff:>6.3} .. {:>6.3}{}",
                r.tip_px,
                eff * (1.0 + WIND_SLOT_VARIATION),
                if r.spin_deg > 0.0 { format!("   (rotates {}°)", r.spin_deg) } else { String::new() },
            );
            assert!(*budget >= 0.0, "{name}: a negative budget");
            // The hard bound: no slot of this family, at full wind, can exceed the budget.
            assert!(
                eff * (1.0 + WIND_SLOT_VARIATION) <= *budget + 1e-12,
                "{name}: {eff} × 1.1 escapes the budget {budget}"
            );
            assert!(eff <= r.tip_px + 1e-12, "{name}: more than the species asked for");
        }
        // The clamp is real on this pack, not a theoretical guard: at least one species asks
        // for more tip travel than its own widest frame can afford, and gets less.
        let clamped: Vec<&str> = presenter
            .bend_budgets()
            .iter()
            .filter(|(name, budget)| {
                let want = wind_response(name).tip_px;
                want > 0.0 && effective_tip(want, *budget) < want - 1e-12
            })
            .map(|(name, _)| name.as_str())
            .collect();
        println!("  clamped by the pack's own art: {clamped:?}");
        assert!(!clamped.is_empty(), "no species is bounded by the pack, so the rule is untested");
        // Every side-face species that wants to move is admitted *some* movement, or the
        // slice has no visible effect and the art has to change instead.
        for name in ["lanternstalk", "tendrilfan", "reedspire", "glowcap", "spiretree", "glasscane"] {
            let eff = effective_tip(wind_response(name).tip_px, presenter.bend_budget(name));
            assert!(eff > 0.0, "{name} cannot move at all on this pack");
        }
        // An unmeasured name never moves.
        assert_eq!(presenter.bend_budget("nosuchplant"), 0.0);
        assert_eq!(effective_tip(0.9, 0.0), 0.0);
        // A column's budget is its own family's, tightened by the vine where it carries one.
        for column in presenter.columns() {
            let own = presenter.bend_budget(TALL_PLANTS[column.pick]);
            let want = if column.vine { own.min(presenter.bend_budget(VINE_PLANT)) } else { own };
            assert_eq!(presenter.column_budget(column), want);
        }
    }

    #[test]
    fn a_quiet_interval_draws_the_windless_image_and_a_packet_does_not() {
        // Both samplers, through the presenter: at a quiet instant every slot's bend is the
        // identity and every column's amplitude is exactly zero, so the drawn image is the
        // windless one by construction; inside a packet neither is.
        let presenter = ArtPresenter::new(pack());
        let quiet = present_seconds(WIND_QUIET_TICK, 0.0);
        let windy = present_seconds(WIND_PEAK_TICK, 0.0);
        assert_eq!(wind_strength(quiet), 0.0);
        assert!(wind_strength(windy) > 0.5, "the peak fixture tick is not windy");
        let mut moved = 0;
        let mut turned = 0;
        for cell in CellId::all() {
            let band = band_of(cell);
            let Some(plant) = presenter.plant_for(band, cell) else { continue };
            let slot = slot_of(cell);
            let budget = presenter.bend_budget(&plant.name);
            let (bend, heading) = slot_wind(&slot, &plant.name, budget, quiet);
            assert!(bend.is_identity(), "{cell:?} bends in a quiet interval: {bend:?}");
            assert_eq!(heading, slot.heading, "{cell:?} turns in a quiet interval");
            let (bend, heading) = slot_wind(&slot, &plant.name, budget, windy);
            if !bend.is_identity() {
                moved += 1;
                assert!(
                    bend.amplitude.abs() <= budget + 1e-12,
                    "{cell:?} bends {} past its budget {budget}",
                    bend.amplitude
                );
                assert_eq!(bend.root, PLANT_BEND_ROOT);
                assert_eq!(bend.length, PLANT_BEND_LENGTH);
                assert_eq!(bend.base, 0.0);
            }
            if heading != slot.heading {
                turned += 1;
                let deg = signed_turn(slot.heading, heading).to_degrees().abs();
                assert!(
                    deg <= wind_response(&plant.name).spin_deg * (1.0 + WIND_SLOT_VARIATION) + 1e-9,
                    "{cell:?} turned {deg}°"
                );
                assert_eq!(cell.face(), Face::Top, "only a radial plant turns");
            }
        }
        assert!(moved > 100, "only {moved} slots bend at the packet's peak");
        assert!(turned > 100, "only {turned} canopy slots turn at the packet's peak");
        for column in presenter.columns() {
            let budget = presenter.column_budget(column);
            assert_eq!(tall_amplitude(column, budget, quiet), 0.0, "a column bends when calm");
            assert!(tall_amplitude(column, budget, windy).abs() <= budget + 1e-12);
        }
        assert!(
            presenter.columns().iter().any(|c| tall_amplitude(c, presenter.column_budget(c), windy) != 0.0),
            "no column bends at the packet's peak"
        );
    }

    #[test]
    fn a_columns_parts_share_one_amplitude_on_one_continuous_curve() {
        // The bend base of every tile of a column: 4i − 8, so the tile bottoms meet and one
        // `D(H)` runs from the base's root to the cap.
        assert_eq!(tall_bend_base(0.0), -8.0);
        assert_eq!(tall_bend_base(1.0), -4.0);
        assert_eq!(tall_bend_base(f64::from(TALL_MAX_SEGMENTS) + 1.0), 32.0);
        // Tile i's top edge is tile i+1's bottom edge plus the 12-row overlap: a sample at
        // the same *height* has the same displacement whichever tile paints it, which is
        // what keeps a join from opening.
        let amplitude = 0.5;
        let bend_at = |i: f64| Bend {
            amplitude,
            base: tall_bend_base(i),
            root: TALL_BEND_ROOT,
            length: TALL_BEND_LENGTH,
        };
        for i in 1..TALL_MAX_SEGMENTS {
            let lower = bend_at(f64::from(i));
            let upper = bend_at(f64::from(i + 1));
            // Height h above the horizon is tile row `16 − (h − base)` in each tile.
            for h in [12.0, 13.5, 15.0] {
                let a = lower.displacement(TILE_ROWS, TILE_ROWS - (h - lower.base));
                let b = upper.displacement(TILE_ROWS, TILE_ROWS - (h - upper.base));
                assert!((a - b).abs() < 1e-12, "tile {i}/{} disagree at h={h}", i + 1);
            }
        }
        // The root of the column does not move, and the top of a full column moves fully.
        assert_eq!(bend_at(0.0).displacement(TILE_ROWS, 15.5), 0.0, "the base's root row");
        // The cap's top row of a full column sits at H = 47.5 of the 48-pixel bend length,
        // so it takes all but a thousandth of the amplitude.
        let top = bend_at(f64::from(TALL_MAX_SEGMENTS) + 1.0).displacement(TILE_ROWS, 0.5);
        assert!((top - amplitude).abs() < 1e-3, "the cap's top row moved {top}");
    }

    // --- Authored growth clips (pack v5) ----------------------------------------------
    //
    // The growth pilot: where the pack carries a `grow<from><to>` clip for the step a cell is
    // in, `draw` plays that clip instead of revealing the upper stage from behind a mask.
    // Every test here drives the presenter through `observe` only — paced growth, never
    // injected state — and rebuilds the picture the documented rule asks for by hand.

    /// Producer density, as a fraction of the ramp's saturation point, that warrants a sprout
    /// in the foliage band and no more.
    const AT_SPROUT: f64 = 0.30;
    /// Producer density that warrants stage 1 in the foliage band and no more.
    const AT_MID: f64 = 0.50;

    /// The one slot these tests use: a rank-2 lanternstalk in the middle of Front. The
    /// lanternstalk is the pilot species and the only plant of the shipped pack that carries
    /// an authored growth clip.
    fn growth_cell() -> CellId {
        CellId::all()
            .find(|&c| {
                c.face() == Face::Front
                    && band_of(c) == Band::Foliage
                    && plant_cap(Band::Foliage, c) == Some(2)
                    && species_of(Band::Foliage, c) == "lanternstalk"
                    && (4..=7).contains(&c.cy())
                    && (4..=11).contains(&c.cx())
            })
            .expect("a rank-2 lanternstalk slot in the middle of Front")
    }

    /// A view whose only rich cell is `cell`, at `density` as a fraction of the producer
    /// ramp's saturation point — which is exactly what [`plant_density`] measures. One cell
    /// is far too little to grow a column ([`column_density`] averages a whole face column),
    /// so the only thing on the canvas above the ground is that cell's plant.
    fn one_cell_view(tick: u64, cell: CellId, density: f64) -> RenderView {
        let mut view = empty_view();
        view.tick = tick;
        view.producer[cell.index()] = density * view.producer_max * PRODUCER_SATURATION;
        view
    }

    fn drawn_with(p: &mut ArtPresenter, view: &RenderView, f: f64) -> Canvas {
        let mut canvas = Canvas::new();
        p.draw(view, f, &mut canvas);
        canvas
    }

    /// The largest absolute per-channel difference between two images.
    fn worst_diff(a: &Canvas, b: &Canvas) -> f32 {
        let mut worst = 0.0f32;
        for face in Face::ALL {
            for y in 0..64u8 {
                for x in 0..64u8 {
                    let (p, q) = (a.get(face, x, y), b.get(face, x, y));
                    for c in 0..3 {
                        worst = worst.max((p[c] - q[c]).abs());
                    }
                }
            }
        }
        worst
    }

    /// The same image with the plants taken out of the pack: floor, both grounds, the ground
    /// cover and the water — the background a plant stamp lands on. The fixtures grow nothing
    /// tall (asserted), so this needs no history of its own.
    fn background(view: &RenderView, f: f64) -> Canvas {
        let mut art = pack();
        art.plants.clear();
        let mut p = ArtPresenter::new(art);
        p.observe(view);
        let canvas = drawn_with(&mut p, view, f);
        assert!(
            (0..p.columns().len()).all(|i| p.segments_of(i) == 0),
            "the one-cell fixture grew a tall column, so this is not just the ground"
        );
        canvas
    }

    /// One plant stamp at a cell's slot with that slot's own wind at `seconds`: the single
    /// [`stamp_layers_bent`] call the drawing rules make for a plant.
    fn stamp_at(
        canvas: &mut Canvas,
        cell: CellId,
        plant: &Plant,
        budget: f64,
        layers: &[(Pose<'_>, f32)],
        opacity: f32,
        mask: Mask,
        seconds: f64,
    ) {
        let slot = slot_of(cell);
        let (bend, heading) = slot_wind(&slot, &plant.name, budget, seconds);
        stamp_layers_bent(
            canvas,
            slot.at,
            heading,
            layers,
            1.0,
            opacity,
            mask,
            bend,
            &mut Vec::new(),
        );
    }

    /// A presenter idle at stage 0 in `cell`, reached the way a viewer joining a living world
    /// does: one snapping observe.
    fn idle_sprout(art: ArtPack, cell: CellId) -> ArtPresenter {
        let th = stage_thresholds(band_of(cell));
        assert!(
            th[0] < AT_SPROUT && AT_SPROUT < th[1] && th[1] < AT_MID && AT_MID < th[2],
            "the fixture densities no longer bracket the band's thresholds"
        );
        let mut p = ArtPresenter::new(art);
        p.observe(&one_cell_view(1, cell, AT_SPROUT));
        assert_eq!(p.growth_of(cell), Growth::snapped(Some(0), false), "{cell:?}");
        p
    }

    /// That presenter with the `0 → 1` step `quarters` quarters of the way along: each
    /// observe is 20 ticks (one simulated second, a quarter of [`STAGE_GROW_SECONDS`]) after
    /// the last, so `g = quarters / 4` exactly, and `quarters = 4` lands idle on stage 1.
    fn growing_from(art: ArtPack, cell: CellId, quarters: u64) -> ArtPresenter {
        let mut p = idle_sprout(art, cell);
        for k in 1..=quarters {
            p.observe(&one_cell_view(1 + 20 * k, cell, AT_MID));
        }
        p
    }

    fn growing(cell: CellId, quarters: u64) -> ArtPresenter {
        growing_from(pack(), cell, quarters)
    }

    #[test]
    fn the_growth_blend_weights_hold_the_two_idle_clips_at_the_ends() {
        assert!(GROW_BLEND > 0.0 && GROW_BLEND < 0.5, "the two edge blends would overlap");
        for k in 0..=1000 {
            let t = f64::from(k) / 1000.0;
            let [from, grow, to] = growth_weights(t);
            assert!(from >= 0.0 && grow >= 0.0 && to >= 0.0, "t = {t}: {from} {grow} {to}");
            assert!(((from + grow + to) - 1.0).abs() < 1e-6, "t = {t}: not an exact lerp");
            assert!(!(from > 0.0 && to > 0.0), "t = {t}: both edge blends are live at once");
            assert_eq!(from > 0.0, t < GROW_BLEND, "t = {t}: the entry blend's reach");
            assert_eq!(to > 0.0, t > 1.0 - GROW_BLEND, "t = {t}: the exit blend's reach");
        }
        // The ends are the neighbouring idle clip alone; the middle is the growth clip alone.
        assert_eq!(growth_weights(0.0), [1.0, 0.0, 0.0]);
        assert_eq!(growth_weights(GROW_BLEND), [0.0, 1.0, 0.0]);
        assert_eq!(growth_weights(0.5), [0.0, 1.0, 0.0]);
        assert_eq!(growth_weights(1.0 - GROW_BLEND), [0.0, 1.0, 0.0]);
        let end = growth_weights(1.0);
        assert!(end[0] == 0.0 && end[1].abs() < 1e-6 && (end[2] - 1.0).abs() < 1e-6, "{end:?}");
        assert_eq!(growth_weights(f64::NAN), [1.0, 0.0, 0.0], "nonsense holds the lower stage");
        // Zero slope at both edges: the first thousandth moves far less than the middle.
        let slope = |t: f64| (growth_weights(t)[0] - growth_weights(t + 0.001)[0]).abs();
        assert!(slope(0.0) < slope(GROW_BLEND / 2.0) / 10.0, "a kink at the entry");
        let rise = |t: f64| (growth_weights(t + 0.001)[2] - growth_weights(t)[2]).abs();
        assert!(rise(1.0 - 0.001) < rise(1.0 - GROW_BLEND / 2.0) / 10.0, "a kink at the exit");
    }

    #[test]
    fn an_authored_growth_clip_enters_from_the_lower_idle_stage_and_leaves_on_the_upper_one() {
        let cell = growth_cell();
        let art = pack();
        let plant = art.plant("lanternstalk").expect("the pilot species");
        assert!(plant.transition(0, 1).is_some(), "pack v5 must carry the 0 → 1 growth clip");
        let budget = ArtPresenter::new(pack()).bend_budget(&plant.name);
        // The frames are *drawn* against a sparse density, where the two stages' opacities
        // differ — so the endpoint opacity is the endpoint stage's, not a blend of the two.
        let th = stage_thresholds(Band::Foliage);
        let low = stage_opacity(0, AT_SPROUT, &th, MOTIF_OPACITY);
        let high = stage_opacity(1, AT_SPROUT, &th, MOTIF_OPACITY);
        assert!(low > 0.0 && low < high, "{low} vs {high}: the endpoints must be tellable apart");

        // t → 0: one tick into the step, a thousandth of the way through the frame. The
        // residual is the growth layer's own weight, `1 − smoothstep(t / GROW_BLEND) ≈ 3
        // (t / GROW_BLEND)²`, times at most one (premultiplied linear channels) times the
        // opacity — under 1e-7 here, so 1e-5 is a decade of slack over the arithmetic.
        let mut p = idle_sprout(pack(), cell);
        p.observe(&one_cell_view(2, cell, AT_MID));
        let f = 1e-3;
        let step = growth_step(growth_between(p.growth_prev_of(cell), p.growth_of(cell), f))
            .expect("the step is in flight");
        assert_eq!((step.lower, step.upper), (Some(0), 1));
        assert!(step.t > 0.0 && step.t < 1e-4, "t = {}", step.t);
        assert!(growth_weights(step.t)[1] < 1e-6, "the entry is not the idle sprout");
        let v = one_cell_view(2, cell, AT_SPROUT);
        let seconds = present_seconds(v.tick, f);
        let mut want = background(&v, f);
        stamp_at(
            &mut want,
            cell,
            plant,
            budget,
            &[(stage_pose(plant, 0, cell, seconds), 1.0)],
            low,
            Mask::None,
            seconds,
        );
        let entry = worst_diff(&drawn_with(&mut p, &v, f), &want);
        assert!(entry < 1e-5, "the growth clip entered with a cut: {entry}");

        // t → 1: the step completed on the last observe, drawn a ten-thousandth before the
        // frame's end. Here the residual is the exit blend's, of the same size.
        let mut p = growing(cell, 4);
        assert_eq!(p.growth_of(cell), Growth::snapped(Some(1), false));
        assert_eq!(p.growth_prev_of(cell).g, 0.75, "the last in-flight tick");
        let (tick, f) = (81, 1.0 - 1e-4);
        let step = growth_step(growth_between(p.growth_prev_of(cell), p.growth_of(cell), f))
            .expect("the last frame of the step is still in flight");
        assert!(step.t > 1.0 - 1e-4 && step.t < 1.0, "t = {}", step.t);
        assert!(growth_weights(step.t)[1] < 1e-5, "the exit is not the idle stage 1");
        let v = one_cell_view(tick, cell, AT_SPROUT);
        let seconds = present_seconds(tick, f);
        let mut want = background(&v, f);
        stamp_at(
            &mut want,
            cell,
            plant,
            budget,
            &[(stage_pose(plant, 1, cell, seconds), 1.0)],
            high,
            Mask::None,
            seconds,
        );
        let exit = worst_diff(&drawn_with(&mut p, &v, f), &want);
        assert!(exit < 1e-5, "the growth clip left with a cut: {exit}");
        // Non-vacuity: the plant is really on the canvas, and the two endpoints differ.
        assert!(worst_diff(&want, &background(&v, f)) > 0.01, "the fixture's plant is invisible");
    }

    #[test]
    fn a_reversed_step_draws_the_same_picture_at_the_same_progress() {
        // The rule first, as a pure function of the growth: a step and its reversal agree on
        // the pair *and* on the progress, so the clip simply runs backwards. (`1 − (1 − g)`
        // is `g` only up to one rounding of the complement, which is why the progress is
        // compared to the double's own precision and the pair exactly.)
        for k in 0..=20 {
            let g = f64::from(k) / 20.0;
            let up = growth_step(Growth { from: Some(0), to: Some(1), g, target: Some(1), fruit: 0.0 })
                .expect("in flight");
            let down = growth_step(Growth {
                from: Some(1),
                to: Some(0),
                g: 1.0 - g,
                target: Some(0),
                fruit: 0.0,
            })
            .expect("in flight");
            assert_eq!((up.lower, up.upper), (down.lower, down.upper), "g = {g}");
            assert_eq!(up.t, g);
            assert!((up.t - down.t).abs() <= f64::EPSILON, "g = {g}: {} vs {}", up.t, down.t);
        }
        assert_eq!(growth_step(Growth::snapped(Some(1), false)), None, "an idle plant is no step");
        assert_eq!(growth_step(Growth::snapped(None, false)), None);

        // And through the presenter: one plant growing 0 → 1 and one wilting 1 → 0, both
        // halfway through the step, drawn against the same view at the same instant.
        let cell = growth_cell();
        let mut up = growing(cell, 2);
        let mut down = ArtPresenter::new(pack());
        down.observe(&one_cell_view(1, cell, AT_MID));
        assert_eq!(down.growth_of(cell), Growth::snapped(Some(1), false), "snapped to stage 1");
        // One second of a two-second wilt is half of it, as one second is a quarter of a grow.
        down.observe(&one_cell_view(21, cell, AT_SPROUT));
        assert_eq!(
            up.growth_of(cell),
            Growth { from: Some(0), to: Some(1), g: 0.5, target: Some(1), fruit: 0.0 }
        );
        assert_eq!(
            down.growth_of(cell),
            Growth { from: Some(1), to: Some(0), g: 0.5, target: Some(0), fruit: 0.0 }
        );
        let v = one_cell_view(41, cell, AT_SPROUT);
        let forward = drawn_with(&mut up, &v, 1.0);
        let backward = drawn_with(&mut down, &v, 1.0);
        assert_eq!(worst_diff(&forward, &backward), 0.0, "a reversal drew a different picture");
        assert!(
            worst_diff(&forward, &background(&v, 1.0)) > 0.01,
            "neither presenter drew a plant, so the comparison proves nothing"
        );
    }

    #[test]
    fn a_growth_clip_is_never_advanced_or_restarted_by_a_draw() {
        let cell = growth_cell();
        let mut p = growing(cell, 2);
        let v = one_cell_view(41, cell, AT_MID);
        let before = (p.growth_of(cell), p.growth_prev_of(cell));
        let first = drawn_with(&mut p, &v, 0.37);
        for k in 0..3 {
            let again = drawn_with(&mut p, &v, 0.37);
            assert_eq!(worst_diff(&first, &again), 0.0, "draw {k} moved the growth clip");
        }
        assert_eq!(
            (p.growth_of(cell), p.growth_prev_of(cell)),
            before,
            "a draw advanced the paced growth"
        );
        // A different frame of the same tick *is* a different picture: the clip runs on the
        // frame's own instant rather than being held between ticks.
        assert!(worst_diff(&first, &drawn_with(&mut p, &v, 0.63)) > 0.0, "the clip is held");
    }

    #[test]
    fn a_pack_without_a_clip_for_the_step_keeps_the_reveal_masks() {
        let cell = growth_cell();
        // The shipped pack with every authored growth clip removed: a v4 pack, in effect.
        let mut v4 = pack();
        for plant in v4.plants.iter_mut() {
            plant.transitions.clear();
        }
        let mut p = growing_from(v4, cell, 2);
        assert_eq!(p.bend_budget("lanternstalk"), plant_bend_budget(
            pack().plant("lanternstalk").expect("the pilot species"),
        ), "clearing the transitions must not change the measured budget, or the bends differ");
        let art = pack();
        let plant = art.plant("lanternstalk").unwrap();
        let budget = p.bend_budget(&plant.name);
        let (v, f, t) = (one_cell_view(41, cell, AT_MID), 1.0, 0.5);
        let seconds = present_seconds(v.tick, f);
        let th = stage_thresholds(Band::Foliage);
        let mut want = background(&v, f);
        // The lower stage fades out whole...
        stamp_at(
            &mut want,
            cell,
            plant,
            budget,
            &[(stage_pose(plant, 0, cell, seconds), 1.0)],
            stage_opacity(0, AT_MID, &th, MOTIF_OPACITY) * (1.0 - t) as f32,
            Mask::None,
            seconds,
        );
        // ...under the upper stage, revealed up the stalk.
        stamp_at(
            &mut want,
            cell,
            plant,
            budget,
            &[(stage_pose(plant, 1, cell, seconds), 1.0)],
            stage_opacity(1, AT_MID, &th, MOTIF_OPACITY),
            Mask::Axial { reveal: t * PLANT_REVEAL_PX },
            seconds,
        );
        let fallback = drawn_with(&mut p, &v, f);
        assert_eq!(worst_diff(&fallback, &want), 0.0, "the fallback is not the reveal mask");
        // And the v5 pack plays the clip instead, which is a different picture.
        let with_clip = drawn_with(&mut growing(cell, 2), &v, f);
        assert!(worst_diff(&fallback, &with_clip) > 0.01, "the authored clip changed nothing");
    }

    #[test]
    fn a_growth_stamp_takes_the_slots_wind_and_still_never_moves_its_root() {
        let cell = growth_cell();
        // Halfway through the step, at a tick whose instant sits inside a wind packet.
        let mut p = idle_sprout(pack(), cell);
        p.observe(&one_cell_view(WIND_PEAK_TICK - 20, cell, AT_MID));
        p.observe(&one_cell_view(WIND_PEAK_TICK, cell, AT_MID));
        let (v, f) = (one_cell_view(WIND_PEAK_TICK, cell, AT_MID), 1.0);
        let seconds = present_seconds(v.tick, f);
        assert!(wind_strength(seconds) > 0.5, "the fixture instant is not windy");
        let art = pack();
        let plant = art.plant("lanternstalk").unwrap();
        let budget = p.bend_budget(&plant.name);
        let slot = slot_of(cell);
        let (bend, heading) = slot_wind(&slot, &plant.name, budget, seconds);
        assert!(!bend.is_identity(), "this slot does not answer the breeze: {bend:?}");
        assert_eq!(heading, slot.heading, "a side-face plant bends, it does not turn");

        let clip = plant.transition(0, 1).expect("the pilot's growth clip");
        let t = 0.5;
        let [w_from, w_grow, w_to] = growth_weights(t);
        let layers = [
            (stage_pose(plant, 0, cell, seconds), w_from),
            (clip.sample(t * clip.seconds), w_grow),
            (stage_pose(plant, 1, cell, seconds), w_to),
        ];
        let th = stage_thresholds(Band::Foliage);
        let under = stage_opacity(0, AT_MID, &th, MOTIF_OPACITY);
        let opacity = under + (stage_opacity(1, AT_MID, &th, MOTIF_OPACITY) - under) * t as f32;
        let mut want = background(&v, f);
        stamp_at(&mut want, cell, plant, budget, &layers, opacity, Mask::None, seconds);
        assert_eq!(
            worst_diff(&drawn_with(&mut p, &v, f), &want),
            0.0,
            "the growth stamp is not the slot's own bent stamp"
        );
        // The bend is really doing something: the same stamp without it is a different image.
        let mut calm = background(&v, f);
        let (calm_bend, _) = (Bend::NONE, ());
        stamp_layers_bent(
            &mut calm,
            slot.at,
            heading,
            &layers,
            1.0,
            opacity,
            Mask::None,
            calm_bend,
            &mut Vec::new(),
        );
        assert!(worst_diff(&want, &calm) > 0.0, "the wind did not bend the growth stamp");

        // And the root stays put. Stamped on the chart's own axes (heading (1, 0), so the
        // tile's rows are the face's rows), the shared root contact of `PLANTS.md` — tile row
        // 14, which this anchor puts on face row 38 — is bit-identical bent or calm, because
        // `PLANT_BEND_ROOT` holds the profile at exactly 0 there.
        let axis = SurfacePoint::new(Face::Front, 32.0, 32.0);
        let along = |bend: Bend| {
            let mut canvas = Canvas::new();
            stamp_layers_bent(
                &mut canvas,
                axis,
                Vec2::new(1.0, 0.0),
                &layers,
                1.0,
                1.0,
                Mask::None,
                bend,
                &mut Vec::new(),
            );
            canvas
        };
        let (still, windy) = (along(Bend::NONE), along(bend));
        assert!(
            (24..40).any(|x| still.get(Face::Front, x, 38).into_iter().any(|c| c > 0.0)),
            "the growth pose paints no root contact row, so the check is vacuous"
        );
        assert!(worst_diff(&still, &windy) > 0.0, "the bend moved nothing at all");
        for x in 23..41u8 {
            assert_eq!(
                still.get(Face::Front, x, 38),
                windy.get(Face::Front, x, 38),
                "the growth stamp's root contact skated at x = {x}"
            );
        }
    }

    #[test]
    fn a_radial_slot_with_a_clip_plays_it_unmasked_too() {
        // The canopy species carry their own clips (2026-09-13), so the top face's rule is
        // exercised on the shipped umbrellafrond's 0 → 1 and compared against the same pack
        // with that plant's clips stripped — the radial reveal it used before.
        let cell = CellId::all()
            .find(|&c| {
                c.face() == Face::Top
                    && plant_cap(Band::Canopy, c) == Some(2)
                    && species_of(Band::Canopy, c) == CANOPY_PLANTS[0]
                    && (4..=11).contains(&c.cx())
                    && (4..=11).contains(&c.cy())
            })
            .expect("a rank-2 umbrellafrond slot in the middle of Top");
        let stripped = || {
            let mut art = pack();
            let target =
                art.plants.iter_mut().find(|p| p.name == CANOPY_PLANTS[0]).expect("the canopy plant");
            assert!(!target.transitions.is_empty(), "the shipped canopy plant carries clips");
            target.transitions.clear();
            art
        };
        // The canopy's own thresholds bracket the same two fixture densities.
        let th = stage_thresholds(Band::Canopy);
        assert!(th[0] < AT_SPROUT && AT_SPROUT < th[1] && th[1] < AT_MID && AT_MID < th[2]);
        let mut p = growing(cell, 2);
        let (v, f, t) = (one_cell_view(41, cell, AT_MID), 1.0, 0.5);
        let seconds = present_seconds(v.tick, f);
        let art = pack();
        let plant = art.plant(CANOPY_PLANTS[0]).unwrap();
        let clip = plant.transition(0, 1).expect("the shipped canopy 0 → 1 clip");
        let budget = p.bend_budget(&plant.name);
        // A radial plant is never bent: it turns in place, and the reveal it no longer uses
        // measured from that same stationary centre.
        let (bend, heading) = slot_wind(&slot_of(cell), &plant.name, budget, seconds);
        assert!(bend.is_identity(), "a radial plant must not be bent: {bend:?}");
        let [w_from, w_grow, w_to] = growth_weights(t);
        let layers = [
            (stage_pose(plant, 0, cell, seconds), w_from),
            (clip.sample(t * clip.seconds), w_grow),
            (stage_pose(plant, 1, cell, seconds), w_to),
        ];
        let under = stage_opacity(0, AT_MID, &th, MOTIF_OPACITY);
        let opacity = under + (stage_opacity(1, AT_MID, &th, MOTIF_OPACITY) - under) * t as f32;
        let mut want = background(&v, f);
        stamp_layers_bent(
            &mut want,
            slot_of(cell).at,
            heading,
            &layers,
            1.0,
            opacity,
            Mask::None,
            bend,
            &mut Vec::new(),
        );
        let drawn = drawn_with(&mut p, &v, f);
        assert_eq!(worst_diff(&drawn, &want), 0.0, "a top-face clip is not drawn unmasked");
        // With the clips stripped the same slot keeps its radial reveal, which looks different.
        let masked = drawn_with(&mut growing_from(stripped(), cell, 2), &v, f);
        assert!(worst_diff(&drawn, &masked) > 0.01, "the radial reveal and the clip agree");
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
        // Inside a wind packet by default ([`wind_fixture_tick`]); `CUBARIUM_WIND_TICK` puts
        // the same fixture in a quiet interval to measure the identity path.
        let base = wind_fixture_tick();
        let t0 = std::time::Instant::now();
        for i in 0..frames {
            view.tick = base + i;
            presenter.draw(&view, 0.5, &mut canvas);
        }
        let per = t0.elapsed().as_secs_f64() / frames as f64;
        println!(
            "ArtPresenter::draw: {:.3} ms/frame ({grown} plants, 200 organisms, wind {:.3} at tick {base}) = {:.0}% of a 60 fps budget",
            per * 1e3,
            wind_strength(present_seconds(base, 0.5)),
            per / (1.0 / 60.0) * 100.0
        );
        assert!(per < 1.0 / 60.0, "draw took {:.3} ms, past the whole 60 fps budget", per * 1e3);
    }
}
