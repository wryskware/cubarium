//! Tall-plant placement, growth geometry, and drawing.

use super::*;

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
/// The face-local `v` of a near-corner cap's centre below which (i.e. nearer the top edge)
/// an opted-in cap's pixels are owned by its final position's unfolding: tile row 10, the
/// centre's `v` at `height = 7`, so the handoff happens before the cap's visible support
/// reaches the corner. Studied, not tunable without re-running the corner handoff sweep.
pub const CORNER_CAP_HANDOFF_V: f64 = 10.0;
/// The face-local `v` of a full column's cap centre (`height = `[`TALL_MAX_SEGMENTS`]): the
/// retained owner. At maturity the owner and the centre coincide and the ordinary stamp is
/// used, so the mature image is the original one.
pub const CORNER_CAP_FINAL_V: f64 = 2.0;
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
    (select < TALL_COLUMN_P).then_some(TallColumn {
        face,
        cx,
        pick,
        vine,
    })
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
        .flat_map(|face| {
            (0..CELLS_PER_FACE_EDGE as u8).filter_map(move |cx| tall_column_of(face, cx))
        })
        .collect()
}

/// The mean producer density (clamped to `[0, 1]`) over a column's foliage cells.
pub fn column_density(view: &RenderView, face: Face, cx: u8) -> f64 {
    let Some((top, horizon)) = foliage_rows(face) else {
        return 0.0;
    };
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
    let mut hash =
        SplitMix64::new(PHASE_SEED ^ TALL_SEED ^ (face.index() as u64) << 8 ^ u64::from(cx));
    hash.next_f64() * seconds
}

/// The pack's tall species resolved once.
#[derive(Clone, Copy, Debug, Default)]
pub(super) struct TallSpecies {
    pub(super) plants: [Option<usize>; 2],
    pub(super) vine: Option<usize>,
}

impl TallSpecies {
    pub(super) fn resolve(pack: &ArtPack) -> TallSpecies {
        let find = |name: &str| pack.tall.iter().position(|p| p.name == name);
        TallSpecies {
            plants: [find(TALL_PLANTS[0]), find(TALL_PLANTS[1])],
            vine: find(VINE_PLANT),
        }
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
/// - trunk tile `i ≥ 1` at [`tall_anchor`]`(i)` owns the rows from `floor` up to `top`,
///   `floor = `[`TALL_FIRST_JOIN`] for `i = 1` and the family's [`trunk_strip`] floor above
///   (below that line the tile below has already painted the 4-periodic pattern), `top` the
///   family's [`trunk_strip`] top except for the last possible segment `i =
///   TALL_MAX_SEGMENTS`, which owns up to the tile's top since nothing above can repaint it;
///   drawn with `Mask::Strip { floor, reveal }`, `reveal = min(top, grown − (4i − 8))` in
///   tile rows: the strip of rows it owns, cut at the grown height, so the newest segment
///   grows out of the one below one row-fraction at a time and a tile whose strip is empty
///   is not drawn. A tile's row 15 is never drawn; for a family on the shifted strips its
///   row 0 is drawn only by that last segment, where its cap's dome covers the trunk columns;
/// - the cap ([`TallPlant::cap`], never `crown`) glides at [`tall_anchor_at`]`(height + 1)`
///   at `TALL_OPACITY · fade`; at rest on a whole cell it sits where the crown used to. For
///   a cap that carries the explicit [`TallPlant::corner_cap_owner`] capability, on a
///   **near-corner** column (`cx` in {0, 1, 14, 15}) and only while the cap's centre is
///   above tile row 10 of its face (`height > 7`), the pixels are chosen by the surface
///   unfolding of the cap's *final* position `(u, 2)` ([`cubarium_render::stamp_pose_in_chart`])
///   while sampling stays about the actual centre within the same nine-pixel footprint;
///   at `height = 9` owner and centre coincide and the ordinary stamp is used, so the mature
///   endpoint is the original image. Everything else about the cap is unchanged, and an
///   unflagged cap never takes this path;
/// - vine tiles at odd `i` own rows [`TALL_VINE_FLOOR`]`..`[`TALL_VINE_TOP`] (the topmost
///   possible tile up to 16) with the same cut, at `TALL_OPACITY · fade`. An explicitly
///   opted-in vine uses its derived trunk through tile9 with top12, then its separately
///   cached endpoint at tile10 for global heights40..44. That patch retains tile9's
///   owning chart, tile10's support center and an Axial grown ceiling; its transparent
///   source rows supply the lower boundary without a second strip-start envelope.
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
pub(super) fn draw_column(
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
    // `hold_owner` is true only for the cap: with the plant's explicit capability, on a
    // near-corner column, while the cap centre is above tile row 10, its pixels are owned
    // by the unfolding of the cap's final position `(u, 2)`; otherwise, and for every other
    // part, the ordinary stamp about the actual anchor.
    let near_corner = column.cx < 2 || column.cx > 13;
    let mut stamp = |clip: &Clip, i: f64, mask: Mask, opacity: f32, hold_owner: bool| {
        let pose = clip.sample(seconds + tall_phase_of(column.face, column.cx, clip.seconds));
        let at = tall_anchor_at(column.face, column.cx, i);
        let bend = Bend {
            amplitude,
            base: tall_bend_base(i),
            root: TALL_BEND_ROOT,
            length: TALL_BEND_LENGTH,
        };
        let owner =
            if hold_owner && plant.corner_cap_owner && near_corner && at.v < CORNER_CAP_HANDOFF_V {
                SurfacePoint::new(at.face, at.u, CORNER_CAP_FINAL_V)
            } else {
                at
            };
        if owner == at {
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
        } else {
            cubarium_render::stamp_pose_in_chart(
                canvas, owner, at, heading, pose, opacity, mask, bend, scratch,
            );
        }
    };
    // The grown height in tile `i`'s own rows (its bottom edge is 4i − 8 px up the face).
    let local = |i: u8| grown - (4.0 * f64::from(i) - 8.0);
    if let Some(base) = &plant.base {
        stamp(base, 0.0, Mask::None, TALL_OPACITY * fade, false);
    }
    let (strip_floor, strip_top) = trunk_strip(plant);
    for i in 1..=TALL_MAX_SEGMENTS {
        let floor = if i == 1 { TALL_FIRST_JOIN } else { strip_floor };
        let top = if i == TALL_MAX_SEGMENTS {
            TILE_ROWS
        } else {
            strip_top
        };
        let reveal = local(i).min(top);
        if reveal <= floor {
            break;
        }
        stamp(
            &plant.trunk,
            f64::from(i),
            Mask::Strip { floor, reveal },
            TALL_OPACITY,
            false,
        );
    }
    if let Some(cap) = &plant.cap {
        stamp(cap, height + 1.0, Mask::None, TALL_OPACITY * fade, true);
    }
    if let (true, Some(vine)) = (column.vine, vine) {
        let strips = vine.vine_strips.as_ref();
        let trunk = strips.map_or(&vine.trunk, |v| &v.trunk);
        for i in (1..=TALL_MAX_SEGMENTS).step_by(2) {
            let top = if i + 2 > TALL_MAX_SEGMENTS && strips.is_none() {
                TILE_ROWS
            } else {
                TALL_VINE_TOP
            };
            let reveal = local(i).min(top);
            if reveal <= TALL_VINE_FLOOR {
                break;
            }
            stamp(
                trunk,
                f64::from(i),
                Mask::Strip {
                    floor: TALL_VINE_FLOOR,
                    reveal,
                },
                TALL_OPACITY * fade,
                false,
            );
        }
        if let Some(strips) = strips {
            let i = f64::from(TALL_MAX_SEGMENTS) + 1.0;
            let clip = &strips.endpoint;
            let reveal = grown - tall_bend_base(i);
            // Endpoint pixels own global heights40..44. Skip the empty patch below its
            // first bilinear support without adding a new Strip start envelope at40.
            if reveal > 7.0 {
                let pose =
                    clip.sample(seconds + tall_phase_of(column.face, column.cx, clip.seconds));
                cubarium_render::stamp_pose_in_chart(
                    canvas,
                    tall_anchor(column.face, column.cx, TALL_MAX_SEGMENTS),
                    tall_anchor_at(column.face, column.cx, i),
                    heading,
                    pose,
                    TALL_OPACITY * fade,
                    Mask::Axial { reveal },
                    Bend {
                        amplitude,
                        base: tall_bend_base(i),
                        root: TALL_BEND_ROOT,
                        length: TALL_BEND_LENGTH,
                    },
                    scratch,
                );
            }
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
pub(super) fn stage_layers(
    plant: &Plant,
    stage: u8,
    cell: CellId,
    s: f64,
    fruit: f64,
) -> [(Pose<'_>, f32); 2] {
    let at = |clip: &Clip| s + plant_phase_of(cell, clip.seconds);
    let q = if fruit.is_finite() {
        fruit.clamp(0.0, 1.0) as f32
    } else {
        0.0
    };
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
pub(super) fn stage_pose(plant: &Plant, stage: u8, cell: CellId, s: f64) -> Pose<'_> {
    let clip = &plant.stages[usize::from(stage)];
    clip.sample(s + plant_phase_of(cell, clip.seconds))
}

/// The largest extent among the layers that will be sampled (weight > 0).
pub(super) fn layers_extent(layers: &[(Pose<'_>, f32)]) -> f64 {
    layers
        .iter()
        .filter(|(_, w)| w.is_finite() && *w > 0.0)
        .map(|(p, _)| p.extent())
        .fold(0.0, f64::max)
}
