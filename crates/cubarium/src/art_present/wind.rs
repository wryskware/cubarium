//! Shared wind field and bend budgets.

use super::*;

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
pub(super) fn hermite(t: f64) -> f64 {
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
    if strength.is_finite() {
        strength.clamp(0.0, 1.0)
    } else {
        0.0
    }
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
    pub const STILL: WindResponse = WindResponse {
        tip_px: 0.0,
        lag_seconds: 0.0,
        spin_deg: 0.0,
    };
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
    (
        "lanternstalk",
        WindResponse {
            tip_px: 0.45,
            lag_seconds: 0.10,
            spin_deg: 0.0,
        },
    ),
    (
        "tendrilfan",
        WindResponse {
            tip_px: 0.55,
            lag_seconds: 0.15,
            spin_deg: 0.0,
        },
    ),
    (
        "reedspire",
        WindResponse {
            tip_px: 0.70,
            lag_seconds: 0.05,
            spin_deg: 0.0,
        },
    ),
    (
        "glowcap",
        WindResponse {
            tip_px: 0.12,
            lag_seconds: 0.0,
            spin_deg: 0.0,
        },
    ),
    ("rootveil", WindResponse::STILL),
    (
        "umbrellafrond",
        WindResponse {
            tip_px: 0.0,
            lag_seconds: 0.0,
            spin_deg: 2.0,
        },
    ),
    (
        "bloomcrown",
        WindResponse {
            tip_px: 0.0,
            lag_seconds: 0.0,
            spin_deg: 1.5,
        },
    ),
    (
        "spiretree",
        WindResponse {
            tip_px: 0.9,
            lag_seconds: 0.2,
            spin_deg: 0.0,
        },
    ),
    (
        "glasscane",
        WindResponse {
            tip_px: 0.5,
            lag_seconds: 0.1,
            spin_deg: 0.0,
        },
    ),
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
        (
            Bend::NONE,
            canopy_heading(slot.heading, response.spin_deg * slot.wind, w),
        )
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
/// every tile below it. An opted-in vine is measured from its derived trunk at tile 9 and
/// endpoint at tile 10, instead of its unrendered original trunk. An unflagged climber
/// retains its original trunk budget. One number per family, measured once: every part
/// of a column bends by the same amplitude, so they must all be inside the same budget.
pub fn tall_bend_budget(plant: &TallPlant) -> f64 {
    let worst = |clip: &Clip, i: f64| {
        clip.frames
            .iter()
            .map(|f| f.bend_headroom(TALL_BEND_ROOT, TALL_BEND_LENGTH, tall_bend_base(i)))
            .fold(f64::INFINITY, f64::min)
    };
    let top = f64::from(TALL_MAX_SEGMENTS);
    if let Some(vine) = &plant.vine_strips {
        // The original authored trunk is not drawn on this path and must not continue
        // to impose its old narrow endpoint budget on the derived, recentered pieces.
        return worst(&vine.trunk, top).min(worst(&vine.endpoint, top + 1.0));
    }
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
pub(super) fn budget_in(budgets: &[(String, f64)], name: &str) -> f64 {
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
    match std::env::var("CUBARIUM_WIND_TICK")
        .ok()
        .and_then(|v| v.parse().ok())
    {
        Some(tick) => tick,
        None => WIND_PEAK_TICK,
    }
}
