//! Plant placement, bands, species selection, and presentation timing.

use super::*;
use cubarium_surface::{Scale, Topology};

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

// --- Ecology v1: structure from wood, foliage as fullness on it ---------------------
//
// `design/ecology-v1-contract.md` §3 gives every cell three plant stocks the presenter can
// read: living wood `W` (persistent structure), living foliage `P`, and dead wood `Wd`.
// These constants are the whole mapping from those stocks to the picture, and every one of
// them is review-tunable from a viewing session.

/// Shape of the structural read: the stage a foliage or canopy cell shows is driven by
/// `(W / wood_max)^WOOD_SHAPE` through the band's ordinary [`stage_thresholds`] and
/// hysteresis. Review-tunable.
///
/// **Why a cube root and not the raw fraction.** `W` spans 0.02 m (`W_min`, barely alive)
/// to 0.6 m (`W_max`) — a thirty-fold range — while the three stage thresholds span 0.25 to
/// 0.70, under threefold. No linear normalisation can put a just-alive stand at stage 0 and
/// still separate an average-light stand from a bright one. A cube root is the honest
/// compression: wood is the stand's *bulk* and the sprite is its *size*, and size goes as
/// the cube root of bulk. With the shipped foliage thresholds it puts the stage steps at
/// `W = 0.0094` (bare to stage 0, below `W_min`, so **every living cell shows a plant**),
/// `W = 0.055` (stage 1) and `W = 0.206` (stage 2), so the implementation note's measured
/// stands — average `W = 0.105`, bright `W = 0.393` — read stage 1 and stage 2.
pub const WOOD_SHAPE: f64 = 1.0 / 3.0;

/// `k` of the fullness rule `f = clamp(P / (k * W), 0, 1)`: the foliage per unit of wood a
/// stand carries when its canopy is "whole". Review-tunable.
///
/// 1 m of foliage per m of wood is exactly half the structural cap `P_cap = alpha*W`
/// (`alpha = 2`, §11), and both measured ungrazed classes sit above it — average
/// `P/W = 0.93`, bright `1.22` — so an ungrazed stand of either class reads as a full
/// canopy.
pub const FOLIAGE_PER_WOOD: f64 = 1.0;

/// The fullness at which the canopy is drawn whole: [`foliage_ramp`] is 1 at and above it.
/// Review-tunable.
///
/// The shoulder is not slack, it is leaf overlap: the last sixth of a canopy hides behind
/// the rest of it and adding it changes no pixel. It also buys the two measured ungrazed
/// classes a margin (average `f = 0.93`, 9 % above the shoulder) so an ordinary steady-state
/// wobble cannot make the foliage layer breathe, and it is what lets a healthy stand draw
/// **exactly** the image it drew before this feature existed, in one stamp.
pub const FOLIAGE_FULL: f64 = 0.85;

/// The one environment variable that moves the foliage shoulder for a viewing session, read
/// **once per process** when the first [`ArtPresenter`](super::ArtPresenter) is built.
///
/// It exists because the shoulder's cost — how far a bright stand can be depleted before the
/// picture moves — is a judgement about a picture, and Wrysk judges pictures on the cube.
/// Unset, the display draws at [`FOLIAGE_FULL`] exactly, which is what ships; there is no
/// CLI flag and the default is not changed by anything here.
pub const FOLIAGE_FULL_ENV: &str = "CUBARIUM_FOLIAGE_FULL";

/// The range a shoulder read from [`FOLIAGE_FULL_ENV`] is clamped into. Below 0.5 a stand
/// would be drawing bare wood while still carrying half its leaves; above 1 the fullness
/// ratio cannot reach the shoulder at all and no canopy would ever be whole.
pub const FOLIAGE_FULL_RANGE: std::ops::RangeInclusive<f64> = 0.5..=1.0;

/// One process-wide reading of [`FOLIAGE_FULL_ENV`], so the value cannot change under a
/// running presenter and the complaint about a bad one is printed once.
static FOLIAGE_FULL_ENV_READING: std::sync::OnceLock<f64> = std::sync::OnceLock::new();

/// The shoulder a presenter is built at when nothing asks for another one: [`FOLIAGE_FULL`],
/// or [`FOLIAGE_FULL_ENV`]'s value if the process was started with it.
pub fn foliage_full_default() -> f64 {
    *FOLIAGE_FULL_ENV_READING.get_or_init(|| {
        let (full, complaint) =
            foliage_full_from_env(std::env::var(FOLIAGE_FULL_ENV).ok().as_deref());
        if let Some(complaint) = complaint {
            eprintln!("{complaint}");
        } else if full != FOLIAGE_FULL {
            // Provenance for a viewing session: the build id does not say which shoulder the
            // panels are drawn at, so the log does.
            eprintln!(
                "cubarium: drawing the foliage shoulder at {full} from {FOLIAGE_FULL_ENV} \
                 (shipped {FOLIAGE_FULL})"
            );
        }
        full
    })
}

/// [`FOLIAGE_FULL_ENV`]'s reading, as `(shoulder, one complaint to print)`.
///
/// **Normative**: nothing set is [`FOLIAGE_FULL`] and no complaint; a finite float is
/// clamped into [`FOLIAGE_FULL_RANGE`], and complains only if the clamp moved it; anything
/// else is [`FOLIAGE_FULL`] and complains. It never panics and never yields a value outside
/// the range.
///
/// Pure in its argument so the behaviour can be tested without a process environment.
pub fn foliage_full_from_env(value: Option<&str>) -> (f64, Option<String>) {
    let Some(raw) = value else {
        return (FOLIAGE_FULL, None);
    };
    let text = raw.trim();
    match text.parse::<f64>() {
        Ok(v) if v.is_finite() => {
            let full = v.clamp(*FOLIAGE_FULL_RANGE.start(), *FOLIAGE_FULL_RANGE.end());
            let complaint = (full != v).then(|| {
                format!(
                    "{FOLIAGE_FULL_ENV}={text} is outside {:?}; drawing the foliage shoulder at {full}",
                    FOLIAGE_FULL_RANGE
                )
            });
            (full, complaint)
        }
        _ => (
            FOLIAGE_FULL,
            Some(format!(
                "{FOLIAGE_FULL_ENV}={text} is not a number; drawing the foliage shoulder at \
                 the decided {FOLIAGE_FULL}"
            )),
        ),
    }
}

/// Living structure: a dim warm ember, the one warm value in a cube whose ground ramps
/// indigo to cyan and whose soil ramps dark plum to violet-mauve ([`SOIL_LOW_SRGB`],
/// [`SOIL_HIGH_SRGB`]). Review-tunable *within the Outrun family* of
/// `design/appearance.md` "Palette" — a stripped stand must read as a plant standing on the
/// ground, not as a hole in it, and hue is the only cue a 64 x 64 face reliably carries.
pub const LIVING_WOOD_SRGB: u32 = 0x009B_4633;

/// Dead structure: cool ash, desaturated and a long way round the wheel from
/// [`LIVING_WOOD_SRGB`]. Review-tunable. It is deliberately the least saturated thing on
/// the cube: dead wood is the only stock with no life in it.
pub const DEAD_WOOD_SRGB: u32 = 0x005A_5E6E;

/// How a wood tone carries the art's own light ([`cubarium_render::Shade`]).
/// Review-tunable, and the difference between "a plant with no leaves on it" and "a hole in
/// the ground".
///
/// A silhouette taken from alpha alone is a solid blob — a stripped bloomcrown becomes a
/// filled disc — because the pack's plants are drawn with their structure *inside* their
/// outline, not around it. Scaling the tone by the texel's own relative luminance keeps that
/// structure. `reference` 0.30 is about the luminance of the pack's lit neon at full alpha,
/// so a lit texel takes the whole tone; `floor` 0.42 is what an unlit one keeps, which is
/// enough that the outline never breaks up at 64 x 64.
pub fn wood_shade() -> cubarium_render::Shade {
    cubarium_render::Shade {
        floor: 0.42,
        reference: 0.30,
    }
}

/// How loud a dead silhouette is against a living one at the same stage. Review-tunable:
/// dead wood is scenery that is on its way out, and at 1 a field of standing dead wood
/// reads as busy as a living forest.
pub const DEAD_WOOD_OPACITY: f32 = 0.70;

/// How loud the **soil band's** dead-wood mark is against the band's own scenery, as a share
/// of [`band_opacity`]`(Band::Soil)`. Review-tunable.
///
/// The same factor the standing dead silhouette takes above the horizon: below it the mark
/// is the only thing dead wood draws, and it should read as a remnant rather than as a
/// second plant.
pub const SOIL_SNAG_OPACITY: f32 = DEAD_WOOD_OPACITY;

/// How many of the soil plant's own tile rows the mark keeps, counted up from the tile's
/// bottom edge ([`cubarium_render::Mask::Axial`], against [`PLANT_REVEAL_PX`] for the whole
/// plant). Review-tunable.
///
/// Two rows and the fade of a third: a *stub*, half a cell tall, which no living plant in
/// the band can be mistaken for even at stage 0 — and small enough that a field of dead
/// stands below the horizon reads as stubble rather than as a second litter layer.
pub const SOIL_SNAG_PX: f64 = 2.5;

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
/// The tile row (counted from the bottom edge, in tile pixels) up to which the trunk below a
/// trunk tile has already painted the 4-periodic pattern: the column's grown height advances
/// [`tall_grown_px`] by one such segment per unit of height. Review-tunable only together
/// with the art.
pub const TALL_JOIN: f64 = 12.0;
/// The strip of tile rows a trunk tile above another trunk **owns and draws** when its family
/// **opts in** by leaving the tile's row 0 unpainted ([`trunk_strip`]), from the bottom edge:
/// `TALL_STRIP_FLOOR..TALL_STRIP_TOP`, i.e. tile rows 1–4 from the top rather than the
/// original 0–3 (`TALL_JOIN..TILE_ROWS`). A trunk tile is 4-periodic, so the drawn column is
/// the same set of painted heights either way — the strips still partition the column
/// exactly, each row composited once, meeting at the same heights — but a tile's **top row
/// is drawn only by the last possible segment** (whose cap sits over it) and its bottom row
/// by none, which is what lets the family leave those two rows unpainted: they sit 7.5 px
/// from the tile pivot, where the 9 px footprint circle is only ±1.96 px wide, and a 4-wide
/// trunk painting them is bound to 0.47 px of wind. The spiretree's trunk leaves them clear
/// (2026-09-13) and is bound by its dome instead; glasscane and the vine paint them and keep
/// the original strips, so their images are untouched, growth included. The first trunk
/// keeps [`TALL_FIRST_JOIN`] as its floor. Review-tunable only together with the art.
pub const TALL_STRIP_FLOOR: f64 = TALL_JOIN - 1.0;
/// See [`TALL_STRIP_FLOOR`]: the row an opted-in trunk strip is cut at, one below the tile's
/// top, for every segment but the last possible one (`TALL_MAX_SEGMENTS`), cut at the top.
pub const TALL_STRIP_TOP: f64 = TILE_ROWS - 1.0;

/// The `(floor, top)` in tile rows from the bottom edge that a family's trunk tiles above the
/// first own: `(TALL_STRIP_FLOOR, TALL_STRIP_TOP)` when **every** trunk frame leaves its top
/// row (tile row 0) wholly unpainted — the family has opted into the shifted strips to earn
/// wind room — and the original `(TALL_JOIN, TILE_ROWS)` otherwise. **Normative**; pure in
/// the art, so a pack decides it once. The last possible segment is always cut at the top.
pub fn trunk_strip(plant: &TallPlant) -> (f64, f64) {
    let clear = plant
        .trunk
        .frames
        .iter()
        .all(|frame| (0..frame.width() as i32).all(|x| frame.texel(x, 0)[3] <= 0.0));
    if clear {
        (TALL_STRIP_FLOOR, TALL_STRIP_TOP)
    } else {
        (TALL_JOIN, TILE_ROWS)
    }
}
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

/// Bottom of the canopy band on a **ring**, as the height `h` of a cell center (the top
/// row = +1, the bottom rim = −1). Review-tunable.
///
/// `design/flat-world-plan-2026-09-16.md` §5 proposes 0.67 as an explicit new default,
/// not a derived number: on a ring `height(p) = 1 − 2v/h`, so 0.67 puts the top **16.5 %**
/// of the rows in the canopy — 7 of 45 cell rows at 320×180 — and leaves 50 % of the
/// world for the foliage above [`SOIL_TOP`]'s bottom 33.5 %. It is a *presentation*
/// constant and lives here, beside [`SOIL_TOP`], for the same reason that one does: the
/// simulation never asks where the canopy is (`design/stratified-world.md`, "Nothing in
/// the world reads `soil_top`; only the presenter does").
pub const CANOPY_TOP: f64 = 0.67;

/// The canopy threshold on a **cube**: only the level Top face reaches `h = 1`, so the
/// cube's canopy is exactly its Top face and nothing else, which is the rule
/// `art_present` has always drawn and the one the cube's frames are pinned to.
pub const CUBE_CANOPY_TOP: f64 = 1.0;

/// The raster a presenter draws on, as the art needs to see it: the world's topology and
/// scale, plus the one band threshold that differs between them.
///
/// Everything in this module that used to name `Topology::Cube` and `Scale::ONE` is a
/// method here; the free functions below are that method on [`ArtGeometry::CUBE`] and are
/// therefore unchanged, to the bit, for every cube caller.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ArtGeometry {
    topology: Topology,
    scale: Scale,
    canopy_top: f64,
}

impl Default for ArtGeometry {
    fn default() -> ArtGeometry {
        ArtGeometry::CUBE
    }
}

impl ArtGeometry {
    /// The five-chart cube at `S = 1`, with the canopy exactly its Top face.
    pub const CUBE: ArtGeometry = ArtGeometry {
        topology: Topology::Cube,
        scale: Scale::ONE,
        canopy_top: CUBE_CANOPY_TOP,
    };

    /// The geometry of a world: [`CUBE_CANOPY_TOP`] on a cube, [`CANOPY_TOP`] on a ring.
    pub fn new(topology: Topology, scale: Scale) -> ArtGeometry {
        let canopy_top = match topology {
            Topology::Cube => CUBE_CANOPY_TOP,
            Topology::Ring { .. } => CANOPY_TOP,
        };
        ArtGeometry {
            topology,
            scale,
            canopy_top,
        }
    }

    /// This geometry with the canopy threshold moved, for a review session.
    ///
    /// **Normative**: a value outside `0..=1`, or a non-finite one, is refused and the
    /// geometry keeps the threshold it had — a canopy at `h ≥ 2` would be no canopy at
    /// all and a canopy at `h ≥ −1` would be the whole world.
    pub fn with_canopy_top(self, canopy_top: f64) -> ArtGeometry {
        if canopy_top.is_finite() && (0.0..=1.0).contains(&canopy_top) {
            ArtGeometry { canopy_top, ..self }
        } else {
            self
        }
    }

    #[inline]
    pub fn topology(self) -> Topology {
        self.topology
    }

    #[inline]
    pub fn scale(self) -> Scale {
        self.scale
    }

    /// The height at or above which a cell is [`Band::Canopy`].
    #[inline]
    pub fn canopy_top(self) -> f64 {
        self.canopy_top
    }

    /// Cells over the whole surface.
    #[inline]
    pub fn cell_count(self) -> usize {
        self.topology.cell_count(self.scale)
    }

    /// Cells across and down one chart.
    #[inline]
    pub fn cells(self, face: Face) -> (u16, u16) {
        self.topology.cells(self.scale, face)
    }

    /// Pixels along a cell edge: 4 on the cube, `4·S` on a ring.
    #[inline]
    pub fn cell_pixels(self) -> f64 {
        match self.topology {
            Topology::Cube => cubarium_surface::CELL_PIXELS,
            Topology::Ring { .. } => self.scale.cell_pixels(),
        }
    }

    /// Every cell of this world, in index order.
    pub fn all_cells(self) -> impl Iterator<Item = CellId> {
        CellId::all(self.topology, self.scale)
    }

    /// The cell at `(cx, cy)` of a chart.
    #[inline]
    pub fn cell(self, face: Face, cx: u16, cy: u16) -> CellId {
        CellId::new(self.topology, self.scale, face, cx, cy)
    }

    /// The chart a cell belongs to.
    #[inline]
    pub fn face_of(self, cell: CellId) -> Face {
        cell.face(self.topology, self.scale)
    }

    /// A cell's center as a surface point.
    #[inline]
    pub fn center_of(self, cell: CellId) -> SurfacePoint {
        cell.center(self.topology, self.scale)
    }

    /// The height of a cell's center ([`Topology::height`]): 1 at the top of the world,
    /// −1 at the bottom. On the cube this *is* `embed(..)[1]` and the Top face is exactly
    /// 1; on a ring it is `1 − 2v/h`.
    pub fn height_of(self, cell: CellId) -> f64 {
        self.topology.height(&self.center_of(cell))
    }

    /// The band of a height, against this geometry's [`ArtGeometry::canopy_top`].
    ///
    /// **Normative**: canopy is `h ≥ canopy_top`, soil is `h < `[`SOIL_TOP`], everything
    /// between is foliage, and a `NaN` height is foliage. With the cube's `canopy_top = 1`
    /// this is exactly the `h >= 1.0` rule, so only the Top face is canopy there.
    pub fn band_of_height(self, h: f64) -> Band {
        if h >= self.canopy_top {
            Band::Canopy
        } else if h < SOIL_TOP {
            Band::Soil
        } else {
            Band::Foliage
        }
    }

    /// The band of a cell, by the height of its center. No blending.
    pub fn band_of(self, cell: CellId) -> Band {
        self.band_of_height(self.height_of(cell))
    }

    /// The band a cell is drawn in this tick: [`Band::Water`] over [`REED_DEPTH`],
    /// otherwise its geometric band.
    pub fn cell_band(self, cell: CellId, water: Option<f64>) -> Band {
        match water {
            Some(w) if w > REED_DEPTH => Band::Water,
            _ => self.band_of(cell),
        }
    }

    /// Whether a cell's plant is drawn radially: its *geometric* band is [`Band::Canopy`].
    ///
    /// The band is geometric, so flooding a cell cannot turn a radial plant into a stalk
    /// halfway through a run. On the cube this is exactly `up_of(cell).is_none()` and
    /// exactly `face == Face::Top`, so nothing about the cube moves.
    pub fn is_radial(self, cell: CellId) -> bool {
        self.band_of(cell) == Band::Canopy
    }

    /// The chart direction, at a cell's center, in which height increases.
    ///
    /// `None` on the cube's level Top face, where there is no uphill and plants are
    /// drawn radially. A ring has no such chart: `height = 1 − 2v/h` falls with `v`
    /// everywhere, so this is the constant `(0, −1)` and every ring plant is a stalk.
    pub fn up_of(self, cell: CellId) -> Option<Vec2> {
        let center = self.center_of(cell);
        let du = self
            .topology
            .embed_tangent(self.scale, &center, Vec2::new(1.0, 0.0))[1];
        let dv = self
            .topology
            .embed_tangent(self.scale, &center, Vec2::new(0.0, 1.0))[1];
        Vec2::new(du, dv).normalized()
    }

    /// The fixed slot of a cell — see [`slot_of`] for the normative draw order.
    pub fn slot_of(self, cell: CellId) -> Slot {
        let mut hash = SplitMix64::new(MOTIF_SEED ^ cell.index() as u64);
        let pick = (hash.next_u64() % 2) as usize;
        let center = self.center_of(cell);
        let at = SurfacePoint::new(
            center.face,
            center.u + hash.range(-1.0, 1.0),
            center.v + hash.range(-1.0, 1.0),
        );
        let free = Vec2::from_screen_angle(hash.range(0.0, std::f64::consts::TAU));
        let rank = hash.next_f64();
        let jitter = hash
            .range(-HEADING_JITTER_DEG, HEADING_JITTER_DEG)
            .to_radians();
        let radial = self.is_radial(cell);
        let heading = match (radial, self.up_of(cell)) {
            (false, Some(up)) => Vec2::from_screen_angle(stalk_heading(up).screen_angle() + jitter),
            _ => free,
        };
        let wind = hash.range(1.0 - WIND_SLOT_VARIATION, 1.0 + WIND_SLOT_VARIATION);
        let rank_cap = if rank < RANK_FULL {
            2
        } else if rank < RANK_MID {
            1
        } else {
            0
        };
        Slot {
            at,
            heading,
            pick,
            rank_cap,
            wind,
            radial,
        }
    }

    /// Where a cell's plant stands and which way it faces.
    pub fn placement_of(self, cell: CellId) -> (SurfacePoint, Vec2) {
        let s = self.slot_of(cell);
        (s.at, s.heading)
    }

    /// The highest stage a cell's slot may reach (0, 1 or 2).
    pub fn rank_cap_of(self, cell: CellId) -> u8 {
        self.slot_of(cell).rank_cap
    }

    /// The cap a slot has in a band, `None` for a slot that grows nothing there.
    pub fn plant_cap(self, _band: Band, cell: CellId) -> Option<u8> {
        match self.rank_cap_of(cell) {
            0 => None,
            cap => Some(cap),
        }
    }

    /// The asset name of the plant a cell grows in a band.
    pub fn species_of(self, band: Band, cell: CellId) -> &'static str {
        let pick = self.slot_of(cell).pick;
        match band {
            Band::Soil => SOIL_PLANTS[pick],
            Band::Foliage => FOLIAGE_PLANTS[pick],
            Band::Canopy => CANOPY_PLANTS[pick],
            Band::Water => WATER_PLANT,
        }
    }

    /// A hash key that separates one pixel of this raster from every other.
    ///
    /// **Normative**: the cube keeps `face << 16 | x << 8 | y`, which is what every
    /// per-pixel phase in this presenter was seeded with and is injective there because
    /// `x` and `y` are both below 64. A ring's pixels run to 65,535 on a side, so that
    /// packing would collide; it uses `x << 32 | y` instead, which cannot.
    #[inline]
    pub fn pixel_key(self, face: Face, x: u16, y: u16) -> u64 {
        match self.topology {
            Topology::Cube => (face.index() as u64) << 16 | u64::from(x) << 8 | u64::from(y),
            Topology::Ring { .. } => u64::from(x) << 32 | u64::from(y),
        }
    }

    /// The per-pixel tables of this raster: the soil weight and the shimmer phase.
    /// Shared, so building two presenters on the cube costs one copy.
    pub(super) fn tables(self) -> std::sync::Arc<PixelTables> {
        if self == ArtGeometry::CUBE {
            return CUBE_TABLES.clone();
        }
        std::sync::Arc::new(PixelTables::build(self))
    }
}

/// [`w_soil`] and the water shimmer phase at every pixel centre of one raster, built once.
///
/// Five faces of 64 × 64 is 20,480 entries; a 320×180 ring is 57,600 and a 640×360 one
/// 230,400. Recomputing a height and a smoothstep per pixel per frame would be the most
/// expensive thing in [`ArtPresenter::draw`], which is why this is a table and not a
/// function.
pub(super) struct PixelTables {
    topology: Topology,
    width: u16,
    height: u16,
    soil: Box<[f32]>,
    shimmer: Box<[f64]>,
}

impl PixelTables {
    fn build(geom: ArtGeometry) -> PixelTables {
        let topology = geom.topology();
        let (w, h) = topology.extent(Face::Front);
        let (width, height) = (w as u16, h as u16);
        let n = topology.charts().len() * usize::from(width) * usize::from(height);
        let mut soil = vec![0.0f32; n];
        let mut shimmer = vec![0.0f64; n];
        let mut table = PixelTables {
            topology,
            width,
            height,
            soil: Box::new([]),
            shimmer: Box::new([]),
        };
        for &face in topology.charts() {
            for y in 0..height {
                for x in 0..width {
                    let i = table.index(face, x, y);
                    let p = SurfacePoint::pixel_center(topology, face, x, y);
                    soil[i] = w_soil(topology.height(&p)) as f32;
                    shimmer[i] = SplitMix64::new(WATER_SEED ^ geom.pixel_key(face, x, y))
                        .next_f64()
                        * std::f64::consts::TAU;
                }
            }
        }
        debug_assert_eq!(soil.len(), n);
        table.soil = soil.into_boxed_slice();
        table.shimmer = shimmer.into_boxed_slice();
        table
    }

    /// The canvas's own storage order — charts outermost, then `y`, then `x` — so a pass
    /// walks the table and the image together.
    #[inline]
    pub(super) fn index(&self, face: Face, x: u16, y: u16) -> usize {
        self.topology.chart_index(face) * usize::from(self.width) * usize::from(self.height)
            + usize::from(y) * usize::from(self.width)
            + usize::from(x)
    }

    /// [`w_soil`] of one pixel centre: 1 wholly soil, 0 wholly foliage/canopy.
    #[inline]
    pub(super) fn soil(&self, face: Face, x: u16, y: u16) -> f32 {
        self.soil[self.index(face, x, y)]
    }

    /// The water shimmer phase of one pixel, in `[0, 2π)`.
    #[inline]
    pub(super) fn shimmer(&self, face: Face, x: u16, y: u16) -> f64 {
        self.shimmer[self.index(face, x, y)]
    }
}

/// The cube's tables, shared by every cube presenter and by the free functions below.
pub(super) static CUBE_TABLES: LazyLock<std::sync::Arc<PixelTables>> =
    LazyLock::new(|| std::sync::Arc::new(PixelTables::build(ArtGeometry::CUBE)));

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

/// The embedded height of a cell's center: Topology::Cube.embed(Scale::ONE, &`CellId::center())[1]`, Top = 1 exactly
/// and the open rim = −0.984375 (the center of the bottom cell row).
pub fn height_of(cell: CellId) -> f64 {
    ArtGeometry::CUBE.height_of(cell)
}

/// The band of a height.
///
/// **Normative**: soil is `h < `[`SOIL_TOP`], canopy is `h ≥ 1` (only the top face
/// reaches it — a side face's highest cell center is at 0.984375), everything between is
/// foliage. A `NaN` height is foliage, which is the band that changes nothing.
pub fn band_of_height(h: f64) -> Band {
    ArtGeometry::CUBE.band_of_height(h)
}

/// The band of a cell, by the height of its center. No blending: a cell is wholly in one
/// band for the purpose of choosing its motif.
pub fn band_of(cell: CellId) -> Band {
    ArtGeometry::CUBE.band_of(cell)
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

/// The precomputed [`w_soil`] of one **cube** pixel center. 1 is wholly soil, 0 wholly
/// foliage/canopy. A ring's weights live in the presenter's own [`PixelTables`], because
/// they depend on the raster's height.
pub fn soil_weight(face: Face, x: u16, y: u16) -> f32 {
    CUBE_TABLES.soil(face, x, y)
}

/// The soil ground ramp in linear light, decoded once (see [`crate::present::PALETTE`]
/// for why the hex is decoded rather than mixed).
pub(super) static SOIL_RAMP: LazyLock<([f32; 3], [f32; 3])> = LazyLock::new(|| {
    (
        present::srgb_linear(SOIL_LOW_SRGB),
        present::srgb_linear(SOIL_HIGH_SRGB),
    )
});

/// [`LIVING_WOOD_SRGB`] and [`DEAD_WOOD_SRGB`] decoded to linear light once, the way
/// [`SOIL_RAMP`] decodes the soil. `srgb_decode` is not a `const fn`.
pub(super) static WOOD_TONES: LazyLock<([f32; 3], [f32; 3])> = LazyLock::new(|| {
    (
        present::srgb_linear(LIVING_WOOD_SRGB),
        present::srgb_linear(DEAD_WOOD_SRGB),
    )
});

/// The living-structure tint a silhouette is stamped with, in linear light.
pub fn living_wood_tone() -> [f32; 3] {
    WOOD_TONES.0
}

/// The dead-structure tint a silhouette is stamped with, in linear light.
pub fn dead_wood_tone() -> [f32; 3] {
    WOOD_TONES.1
}

/// Seeds the per-cell slot hash. Any fixed value works; this one keeps the placement
/// stream separate from every other `SplitMix64` stream in the host.
const MOTIF_SEED: u64 = 0x6D6F_7469_6600_0001;
/// Seeds the per-organism clip-phase hash, for the same reason.
pub(super) const PHASE_SEED: u64 = 0x7068_6173_6500_0001;
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
    /// Whether this slot's plant is drawn **radially** — seen from above, free to face
    /// anywhere, turning in place in the wind and opening outward from its own centre —
    /// rather than as a stalk standing up the wall.
    ///
    /// **Normative**: a slot is radial exactly where its cell's *geometric* band is
    /// [`Band::Canopy`] ([`ArtGeometry::is_radial`]). On the cube that is the Top face and
    /// nothing else, which is the rule this presenter has always drawn — Top is the one
    /// chart with no uphill. A ring's canopy is a band of rows rather than a chart, and
    /// its crowns are the same radial art, so they keep the same treatment.
    pub radial: bool,
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
/// else `0`. Heading: [`stalk_heading`] of [`up_of`] on a stalk slot, plus the jitter;
/// free on a radial one ([`ArtGeometry::is_radial`] — the cube's Top face, a ring's canopy
/// band). Wind: uniform in `[1 − `[`WIND_SLOT_VARIATION`]`, 1 + WIND_SLOT_VARIATION)`.
pub fn slot_of(cell: CellId) -> Slot {
    ArtGeometry::CUBE.slot_of(cell)
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
    ArtGeometry::CUBE.up_of(cell)
}

/// Where a cell's plant stands and which way it faces.
pub fn placement_of(cell: CellId) -> (SurfacePoint, Vec2) {
    ArtGeometry::CUBE.placement_of(cell)
}

/// The highest stage a cell's slot may reach (0, 1 or 2).
pub fn rank_cap_of(cell: CellId) -> u8 {
    ArtGeometry::CUBE.rank_cap_of(cell)
}

/// The cap a slot has in a band, `None` for a slot that grows nothing there.
///
/// **Normative**: a sprout-only slot (rank cap 0, about half of all cells) grows nothing
/// in any band. Drawing a stage-0 sprout in every such slot turned the foliage into a
/// field of magenta speckles that swallowed the rain and the creatures, and lined a
/// flooded floor with a cyan fence of reeds; leaving those slots bare gives the wall
/// breathing room and lets ground cover carry the fill. Slots with cap 1 or 2 keep their
/// cap. Review-tunable by changing this rule.
pub fn plant_cap(band: Band, cell: CellId) -> Option<u8> {
    ArtGeometry::CUBE.plant_cap(band, cell)
}

/// The band a cell is drawn in this tick: [`Band::Water`] when its water depth exceeds
/// [`REED_DEPTH`], otherwise its geometric band ([`band_of`]). `None` water is dry.
pub fn cell_band(cell: CellId, water: Option<f64>) -> Band {
    ArtGeometry::CUBE.cell_band(cell, water)
}

/// The asset name of the plant a cell grows in a band.
///
/// **Normative**: soil → one of [`SOIL_PLANTS`], foliage → [`FOLIAGE_PLANTS`], canopy →
/// [`CANOPY_PLANTS`], each by the cell's hashed pick; water → [`WATER_PLANT`] always.
pub fn species_of(band: Band, cell: CellId) -> &'static str {
    ArtGeometry::CUBE.species_of(band, cell)
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

/// The structural density that drives a cell's plant, as a fraction of its band's scale.
///
/// **Normative**: soil reads **litter plus remains** (`detritus + carrion`) over
/// [`SOIL_SCALE`]; foliage and canopy read [`wood_density`] — the cube root of the living
/// wood fraction, not the producer field; water reads depth over [`REED_SCALE`]. Not
/// clamped above (the stage rules compare it against thresholds and the opacity ramp clamps
/// on its own); a missing cell or a non-finite value is density 0.
///
/// This is the ecology v1 presentation boundary (contract §12): what a cell *is* comes from
/// its persistent structure, so stripping a stand's foliage does not take its plant away.
/// [`ground_density`] keeps the old producer read for the ground-cover texture, which is
/// low cover rather than structure.
pub fn plant_density(view: &RenderView, index: usize, band: Band) -> f64 {
    let value = match band {
        Band::Soil => litter_density(view, index),
        Band::Water => view.water.get(index).copied().unwrap_or(0.0) / REED_SCALE,
        Band::Foliage | Band::Canopy => return wood_density(view, index),
    };
    if value.is_finite() { value } else { 0.0 }
}

/// Litter plus remains over [`SOIL_SCALE`]: what the soil band's plants, its flecks and its
/// ground wash all read (contract §3.1 `D` and `C`; §12 allows the presenter to sum them
/// until a carcass look exists, and there is none).
pub fn litter_density(view: &RenderView, index: usize) -> f64 {
    let d = view.detritus.get(index).copied().unwrap_or(0.0);
    let c = view.carrion.get(index).copied().unwrap_or(0.0);
    let value = (d + c) / SOIL_SCALE;
    if value.is_finite() { value } else { 0.0 }
}

/// The density that drives the **ground-cover texture**, unchanged from before ecology v1:
/// soil reads litter plus remains, foliage and canopy the producer field over the ramp's
/// saturation point, water its depth. Ground cover is low cover, not structure, so it
/// follows the foliage field and thins out under grazing with it.
pub fn ground_density(view: &RenderView, index: usize, band: Band) -> f64 {
    let value = match band {
        Band::Soil => litter_density(view, index),
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

/// One wood stock as a stage-driving density: `(value / wood_max)^`[`WOOD_SHAPE`], with the
/// fraction clamped to `[0, 1]` first. A non-positive or non-finite `wood_max`, or a
/// non-finite value, is density 0.
pub fn wood_fraction(value: f64, wood_max: f64) -> f64 {
    if !(wood_max.is_finite() && wood_max > 0.0 && value.is_finite() && value > 0.0) {
        return 0.0;
    }
    (value / wood_max).clamp(0.0, 1.0).powf(WOOD_SHAPE)
}

/// The living structure of a cell: [`wood_fraction`] of its `wood`.
pub fn wood_density(view: &RenderView, index: usize) -> f64 {
    wood_fraction(view.wood.get(index).copied().unwrap_or(0.0), view.wood_max)
}

/// The dead structure of a cell: [`wood_fraction`] of its `dead_wood`, read through exactly
/// the same mapping so a stand that dies keeps the size it had.
pub fn dead_wood_density(view: &RenderView, index: usize) -> f64 {
    wood_fraction(
        view.dead_wood.get(index).copied().unwrap_or(0.0),
        view.wood_max,
    )
}

/// How strongly the **soil band** marks a stand that died in it:
/// `clamp(`[`dead_wood_density`]` − `[`wood_density`]`, 0, 1)`.
///
/// **Normative**, and 0 in every band but [`Band::Soil`] as far as the drawing is concerned
/// — the structural bands carry a whole dead silhouette instead ([`structural`]).
///
/// The rule the brief states is "`Wd > 0` and `W = 0`", and that is exactly what this reads
/// at both ends: full strength where a stand is wholly dead, nothing where there is no dead
/// wood, and nothing where a living stand at least as large is standing in the same cell —
/// which is what keeps the soil band's *living* image the one it has always had, wood or no
/// wood. Between those ends it is the same difference read through the same cube root
/// [`wood_fraction`] uses, so a stand that dies below the horizon has its mark **fade in over
/// the dieback** instead of cutting in at the instant `W` reaches zero. A cut is the one
/// thing this presentation is not allowed (contract §12), and `W` reaching zero is precisely
/// when `Wd` is largest, so a hard gate would be the loudest cut on the cube.
///
/// It is deliberately *not* litter: `D + C` still drive the band's plants, its flecks and its
/// ground wash on their own ([`litter_density`]), and a cell's litter neither creates this
/// mark nor hides it.
pub fn soil_snag(view: &RenderView, index: usize) -> f64 {
    let dead = dead_wood_density(view, index);
    // A cell with no dead wood cannot be marked whatever its living stand is, and that is
    // almost every cell of almost every world: taking it before the living read means the
    // whole pass costs one field compare per soil cell in a world where nothing has died.
    if dead <= 0.0 {
        return 0.0;
    }
    (dead - wood_density(view, index)).clamp(0.0, 1.0)
}

/// The opacity the soil band's dead-wood mark is stamped at:
/// [`band_opacity`]`(Band::Soil) · `[`SOIL_SNAG_OPACITY`]` · `[`soil_snag`].
pub fn soil_snag_opacity(view: &RenderView, index: usize) -> f32 {
    (band_opacity(Band::Soil) * SOIL_SNAG_OPACITY * soil_snag(view, index) as f32).clamp(0.0, 1.0)
}

/// How full a living stand's canopy is: `f = clamp(P / (`[`FOLIAGE_PER_WOOD`]` * W), 0, 1)`,
/// and 0 where there is no wood to carry foliage.
///
/// **Normative**, and the one continuous quantity ecology v1 adds to the picture. It is a
/// *ratio*, not a stock: a stand that loses half its wood and half its foliage is still
/// full, and a stand that keeps its wood and loses its foliage empties out. That is what
/// makes grazing legible on a structure that does not move.
pub fn foliage_fullness(view: &RenderView, index: usize) -> f64 {
    let w = view.wood.get(index).copied().unwrap_or(0.0);
    let p = view.producer.get(index).copied().unwrap_or(0.0);
    if !(w.is_finite() && w > 0.0 && p.is_finite() && p > 0.0) {
        return 0.0;
    }
    (p / (FOLIAGE_PER_WOOD * w)).clamp(0.0, 1.0)
}

/// The share of a stand's stage sprite drawn as **foliage** rather than as bare structure:
/// the clamped Hermite of the fullness over [`FOLIAGE_FULL`].
///
/// **Normative**: 0 at `f = 0`, 1 at and above `f = `[`FOLIAGE_FULL`], monotone
/// non-decreasing in between, with zero slope at both ends — so neither a full canopy nor a
/// stripped one can flicker, and a grazed stand thins and refills continuously. A `NaN`
/// fullness is a stripped stand.
pub fn foliage_ramp(f: f64) -> f32 {
    foliage_ramp_at(f, FOLIAGE_FULL)
}

/// [`foliage_ramp`] at a chosen shoulder, for the shoulder study
/// (`design/7_Research/ecology-v1-presentation-2-2026-09-16.md`) and for nothing else.
///
/// **Normative**: identical to [`foliage_ramp`] at `full = `[`FOLIAGE_FULL`], non-increasing
/// in `full` at every fullness (a higher shoulder asks a stand to be fuller before it draws
/// a whole canopy), and a stripped stand for a non-finite or non-positive `full`.
///
/// This exists because the shoulder is the one mapping constant whose cost — how far a
/// bright stand can be depleted before the picture moves — can only be judged from a picture.
/// It is reachable from [`ArtPresenter::with_foliage_full`] at construction, and the shipped
/// display draws at [`foliage_full_default`]: [`FOLIAGE_FULL`] unless the process was started
/// with [`FOLIAGE_FULL_ENV`], the one runtime override, read once.
pub fn foliage_ramp_at(f: f64, full: f64) -> f32 {
    if f.is_nan() || !(full.is_finite() && full > 0.0) {
        return 0.0;
    }
    hermite(f / full) as f32
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

/// The living wood a cell needs to read as structural `density`: the inverse of
/// [`wood_fraction`], `wood_max * density³`.
pub fn wood_for_density(density: f64, wood_max: f64) -> f64 {
    if !(wood_max.is_finite() && wood_max > 0.0 && density.is_finite() && density > 0.0) {
        return 0.0;
    }
    wood_max * density.min(1.0).powi(3)
}

/// Fixture and capture support: give every cell of a view the living wood whose structural
/// density ([`wood_density`]) equals the **producer** density that cell already carries
/// ([`ground_density`]), plus a reserve at half of it.
///
/// Fixtures and studies written before ecology v1 said "how grown is this cell" as a
/// fraction of the producer ramp's saturation point, which is what drove the stage then.
/// This translates such a view into the stocks that drive it now, exactly, and leaves
/// `producer` alone — so the ground cover, which still reads the producer field, is
/// untouched, and every cell's canopy is full (`P / W = 1.5 / d²`, never below 1.5), which
/// is the case those fixtures were written for. It is not part of the drawn contract: the
/// world fills both fields itself.
pub fn wood_from_producer(view: &mut RenderView) {
    for i in 0..view.wood.len() {
        let w = wood_for_density(ground_density(view, i, Band::Foliage), view.wood_max);
        view.wood[i] = w;
        if i < view.plant_reserve.len() {
            view.plant_reserve[i] = 0.5 * w;
        }
    }
}

/// Whether a band's plants are **structural**: driven by living wood, carrying a
/// living-wood silhouette under their foliage and a dead-wood silhouette of their own.
///
/// Only the foliage and canopy bands are. The soil band's plants are litter scenery — their
/// stage comes from `D + C`, not from `W`, and painting a wood silhouette under a mushroom
/// whose size is set by litter would say something the stocks do not. The water band's reeds
/// stand by depth. Both keep exactly the image they had before ecology v1 **for every living
/// stand**; the soil band additionally carries [`soil_snag`], a stub in the dead tone where a
/// stand has died, so that a stand dying below the horizon is not invisible.
pub fn structural(band: Band) -> bool {
    matches!(band, Band::Foliage | Band::Canopy)
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
        PHASE_SEED
            ^ (u64::from(id.slot) << 32)
            ^ u64::from(id.generation).wrapping_mul(GENERATION_ODD),
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
    let f = if f.is_finite() {
        f.clamp(0.0, 1.0)
    } else {
        0.0
    };
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
