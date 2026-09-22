//! **The phase-one founder body**: paid heading motion on the continuous pose, the
//! contact/taste receptors of the manifest, and the geometry a local bite is allowed to
//! touch (`design/voxel-senses-phase1-plan.md`, "Phase-one body and action contract").
//!
//! **The body is in metres** (`design/handoffs/voxel-body-anchors-2026-09-22.md`,
//! decisions §1, §2, §6). Its dimensions are its lineage's adult dimensions on the
//! [`FounderPhysiology`] times `(body / body_max)^(1/3)`, resolved into one [`Body`]
//! value that every consumer takes. It is an upright disc of radius `width / 2` on the
//! support layer, centred on the [`Pose`](crate::Pose)'s `(x, z)`, standing on the
//! support face one layer below its centre column, and it needs its own **height** of
//! void over that face. The forward mouth is the footprint plus a reach of
//! `0.25 × length` beyond it, and it takes food from the physical band
//! `[0, 1.33 × height]` over the standing surface — not from a whole-voxel layer count.
//! The [`Manifest`](crate::Manifest)'s `body_length_m`, `body_width_m` and
//! `mouth_reach_up_voxels` are the **recorded contract** the shipped centres were
//! trained against and are no longer read for geometry.
//!
//! # What movement is allowed to do
//!
//! Forward effort sweeps the disc along the heading in bounded sub-steps of at most one
//! footprint radius, so a wall (a solid voxel at the body layer is at least one voxel
//! thick) cannot be tunnelled between endpoint checks. `x` wraps; the strip's `z` ends
//! are hard walls.
//!
//! **The body steps ledges.** A lineage has a climb height in metres on its
//! [`FounderPhysiology`], converted to whole voxels once at the consumer by
//! [`climb_voxels`], and a sub-step may put the centre column on a support face whose
//! standing layer is within that many voxels of the body's own, up or down
//! (`design/handoffs/voxel-founder-step-2026-09-22.md`). A candidate is refused when the
//! centre column carries no such face — a wall, or a cliff edge: there is still no
//! falling — when the disc would overlap a solid in the layers the body needs, or when
//! the standing water there is deeper than the founder can wade. The standing layer, and
//! with it `site.y` and the pose's height, follow the destination. A refused step stops
//! the sweep: movement is constrained, the delivered motion is what actually happened,
//! and the requested equivalent displacement is paid for anyway. There is no graph
//! search, no waypoint, no nearest-food target and no turn-to-target anywhere in this
//! path — the controller's turn effort is the only yaw input there is.
//!
//! # Receptors
//!
//! Front/left/right contact receptors probe the footprint's boundary arc in their
//! direction (the arc centre and ±45°, at the footprint radius) for a solid voxel at the
//! layer a body *one climb higher* would occupy — the cell holding `0.5 × height` over
//! the standing surface, plus `climb`. **A solid the
//! body could step onto is not a wall**: a steppable ledge reads as open ground and a
//! cliff face or a taller wall reads exactly as it did before (the receptor is still one
//! layer, and the observation vector's shape is unchanged). The bodies are smaller than a
//! voxel, so a touching solid always covers one of the samples. The underside receptor reads the support face under the centre.
//! `Wet` is the standing water at the foot, a valid zero when dry. `Taste` reads only
//! what the mouth region physically contacts: the browser tastes the foliage of a stand
//! whose crown cells reach the mouth region; the blind founder tastes actual litter stock
//! under its ground-level mouth, or valid-zero bare ground. Never a remote field query,
//! never a neighbour outside the reach.
//!
//! The modules P1-C owns (`Chem`, `Light`, `Cone`) are left **zero with validity 0** in
//! the observation: this module fabricates nothing it does not measure.

use cubarium_voxel::{DT, VoxelView};
use cubarium_voxel_flora::{FloraView, Reach, Site};
use serde::{Deserialize, Serialize};

use crate::controller::Actions;
use crate::manifest::{Founder, Manifest};
use crate::{Animal, Fauna, Reproduction, SpeciesConfig};

/// The cue-unit reference the plan fixes for the litter cue (`M_emit`, "Initial cue
/// field settings"): the field emits `min(litter / M_EMIT, 1)` cue units per second. The
/// contact Taste response uses the same material reference while reading actual stock;
/// the remote Chem channel reads the independently diffused concentration.
pub(crate) const M_EMIT: f64 = 0.05;

/// Below this much requested equivalent displacement an interval counts as "none
/// requested" for the motor-delivery channel, which then reads 1 by contract.
const NONE_REQUESTED: f64 = 1e-12;

// The anchor fractions, shared by both founders and listed as placeholders in
// `design/backlog.md` §1 (decisions §2 and §6; the brief
// `design/handoffs/voxel-body-anchors-2026-09-22.md`). They live on the physiology, not
// on the `Manifest`, because manifest geometry is part of the trained-policy digest.

/// The eye, as a fraction of body height over the standing surface.
const EYE_HEIGHT_FRACTION: f64 = 0.8;
/// The mouth band's ceiling, as a fraction of body height over the standing surface.
const MOUTH_CEILING_FRACTION: f64 = 1.33;
/// The contact receptors, as a fraction of body height over the standing surface.
const CONTACT_HEIGHT_FRACTION: f64 = 0.5;
/// The mouth's horizontal reach, as a fraction of body length ahead of the footprint.
const MOUTH_REACH_LENGTH_FRACTION: f64 = 0.25;

/// **A living body's physical geometry**, in metres, resolved from its lineage's
/// [`FounderPhysiology`] and its current structure.
///
/// One value, handed to every consumer, so the animal is the same animal at the tick, in
/// the presenter, in the seeder and in a diagnostic, and the grid appears only where a
/// length is turned into cells (`design/voxel-encounter-contract-2026-09-21.md` §4).
/// Every field is an absolute length: the dimensions are the adult's times
/// `(body / body_max)^(1/3)` (decisions §1) and the anchors are the physiology's
/// fractions of those.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Body {
    /// Nose to tail.
    pub length_m: f64,
    /// Across; the footprint is a disc of half this.
    pub width_m: f64,
    /// Standing surface to the top of the back. The void a body needs is this tall.
    pub height_m: f64,
    /// The eye, metres over the standing surface.
    pub eye_m: f64,
    /// The mouth band is `[0, mouth_ceiling_m]` over the standing surface.
    pub mouth_ceiling_m: f64,
    /// The contact receptors, metres over the standing surface.
    pub contact_m: f64,
    /// How far ahead of the footprint the mouth reaches, in metres.
    pub mouth_reach_m: f64,
}

impl Body {
    /// The footprint disc's radius: half the body's width.
    pub fn footprint_radius(&self) -> f64 {
        self.width_m / 2.0
    }

    /// The void this body needs over the face it stands on, in whole voxels:
    /// `ceil(height / voxel)`, never below one. The clearance is the **body**, not the
    /// mouth: `mouth_reach_up_voxels` is no longer read for geometry.
    pub fn headroom_voxels(&self, voxel_m: f64) -> u32 {
        if !(voxel_m > 0.0) || !self.height_m.is_finite() || self.height_m <= 0.0 {
            return 1;
        }
        let cells = (self.height_m / voxel_m).ceil();
        if !cells.is_finite() || cells < 1.0 {
            return 1;
        }
        (cells as u32).max(1)
    }

    /// The crown layers the mouth band reaches from `standing_y`: the cells whose slab
    /// overlaps `[surface, surface + mouth_ceiling_m]` by a positive amount. A touch at
    /// exactly the ceiling contributes nothing.
    pub fn mouth_layers(&self, standing_y: u32, voxel_m: f64) -> std::ops::RangeInclusive<i64> {
        crate::encounter::band_crown_layers(standing_y, voxel_m, self.mouth_ceiling_m)
    }

    /// The layer the contact receptors probe: the cell holding `contact_m` over the
    /// standing surface. For either founder on either shipped grid this is the body's
    /// own layer, which is where the receptors always were — it is now *derived* from
    /// the body rather than assumed.
    pub fn contact_layer(&self, standing_y: u32, voxel_m: f64) -> u32 {
        if !(voxel_m > 0.0) || !self.contact_m.is_finite() || self.contact_m < 0.0 {
            return standing_y + 1;
        }
        let up = (self.contact_m / voxel_m).floor();
        if !up.is_finite() || up < 0.0 {
            return standing_y + 1;
        }
        standing_y + 1 + up.min(f64::from(u32::MAX / 2)) as u32
    }
}

/// A founder's own physiology: the digestive and upkeep numbers in the ordinary
/// [`SpeciesConfig`] shape, plus the two phase-one cost settings the manifest's schema
/// deliberately does not carry (cost coefficients change how good a policy can be, not
/// what a slot means, so they stay out of the digest).
///
/// The physiology table is **frozen** for the first ES pilots
/// (`design/voxel-senses-phase1-plan.md`): genetic variation and cost optimisation are
/// later experiments.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct FounderPhysiology {
    /// The numbers the ordinary animal rules run on when the body carries this
    /// founder's marker: upkeep, bite rate, assimilation, thresholds and densities.
    pub core: SpeciesConfig,
    /// The named configurable motor respiration coefficient: organic matter respired
    /// per unit of body per second at **full cruise** (equivalent displacement equal to
    /// `cruise`). Below full cruise the cost scales with the requested equivalent
    /// displacement `|v| + r·|yaw rate|`, so an attempt a wall blocks still pays, and
    /// turning while stopped pays. The frozen default sets full cruise at the same rate
    /// as basal upkeep.
    pub motor_respiration_per_s: f64,
    /// The lineage's **climb height in metres**: how far up or down a body may put its
    /// feet in one sub-step, and therefore what counts as a ledge rather than a wall
    /// (`design/handoffs/voxel-founder-step-2026-09-22.md`).
    ///
    /// It lives here and not on the [`Manifest`](crate::Manifest) deliberately: manifest
    /// geometry is part of the trained-policy digest, and the shipped centres must keep
    /// loading. Converted to whole voxels once at the consumer by [`climb_voxels`].
    /// Placeholders, in `design/backlog.md` §1: browser 0.25 m, shredder 0.125 m.
    pub climb_m: f64,
    /// The **adult** body, in metres: length, width and height
    /// (`design/handoffs/voxel-body-anchors-2026-09-22.md`; decisions §1). The ladder's
    /// animal — browser 0.375 × 0.1875 × 0.1875 m, shredder 0.19 × 0.0625 × 0.0625 m —
    /// and *not* the [`Manifest`](crate::Manifest)'s `body_length_m` / `body_width_m`,
    /// which are part of the trained-policy digest and must not move. A living body's
    /// dimensions are these times `(body / body_max)^(1/3)`
    /// ([`FounderPhysiology::body_at`]). Placeholders, `design/backlog.md` §1.
    pub adult_length_m: f64,
    pub adult_width_m: f64,
    pub adult_height_m: f64,
    /// The eye, as a fraction of body height over the standing surface (decisions §6:
    /// 0.8). A placeholder, `design/backlog.md` §1.
    pub eye_height_fraction: f64,
    /// The mouth band's ceiling, as a fraction of body height over the standing surface
    /// (decisions §2: 1.33, the dossier's raised neck). A placeholder.
    pub mouth_ceiling_fraction: f64,
    /// The contact receptors' height, as a fraction of body height over the standing
    /// surface (0.5). A placeholder.
    pub contact_height_fraction: f64,
    /// The mouth's horizontal reach ahead of the footprint, as a fraction of body
    /// length (decisions §2: 0.25, "as today"). A placeholder.
    pub mouth_reach_length_fraction: f64,
    /// `f`, the declared share of the body's **total** structure that is sensor/organ
    /// tissue: 5% for the blind founder, 10% for the browser. The body's structure
    /// stock is the total; the sensor share is counted within it once — no second
    /// ledger — and the body's usual maintenance applies to the total. Adding organs
    /// raised paid structure when the founder was sized from a core budget; it came
    /// with no free reserve or energy.
    pub organ_structure_fraction: f64,
}

impl FounderPhysiology {
    /// The frozen phase-one table, one entry per founder.
    ///
    /// The blind littershredder's digestive configuration is set **deliberately, once,
    /// from the real `Taken` composition** of the litter it eats (`voxel-senses-phase1-tests.md`
    /// §2): arena litter is deposited at a 0.02 mineral fraction and
    /// [`take_litter`](cubarium_voxel_flora::Flora::take_litter) returns mineral pro rata
    /// at that fraction with the litter's own retained energy density of 2.0. A tissue
    /// mineral content of 0.02 therefore never binds the mineral budget — the feeder
    /// converts its whole `yield_fraction` share of what it takes — and nothing was
    /// added to the litter and nothing was converted from the carrion or wood pools to
    /// make that so. The bite rate keeps the frondgrazer's placeholder ratio of 0.04
    /// body per second at the founder's smaller body.
    pub fn frozen(founder: Founder) -> FounderPhysiology {
        match founder {
            Founder::Blind => FounderPhysiology {
                core: SpeciesConfig {
                    maintenance_per_s: 0.001,
                    bite_per_s: 0.0005,
                    yield_fraction: 0.5,
                    n_tissue: 0.02,
                    body_max: 0.0125,
                    body_min: 0.003125,
                    birth_body: 0.0125,
                    birth_cost: 0.00625,
                    reserve_cap: 0.5,
                    // Unused by the founder path: feeding is local and there is no
                    // target search. Recorded at the frondgrazer's placeholders so a
                    // founder body is a complete animal under the same validators.
                    reach: Reach {
                        horizontal: 1,
                        up: 1,
                    },
                    climb: 0,
                    wade_depth_m: 0.05,
                    drown_depth_m: 0.2,
                    step_period_s: 1.0,
                    sense_radius: 8,
                    energy_density: 2.0,
                    // The littershredder lays a clutch on the litter it lives in.
                    reproduction: Reproduction::EGGS_PLACEHOLDER,
                },
                motor_respiration_per_s: 0.001,
                // A ground feeder's placeholder climb: half a browser's, one voxel on
                // either shipped grid (`design/backlog.md` §1).
                climb_m: 0.125,
                // The ladder's littershredder: 0.19 m long, a third of that in section.
                adult_length_m: 0.19,
                adult_width_m: 0.0625,
                adult_height_m: 0.0625,
                eye_height_fraction: EYE_HEIGHT_FRACTION,
                mouth_ceiling_fraction: MOUTH_CEILING_FRACTION,
                contact_height_fraction: CONTACT_HEIGHT_FRACTION,
                mouth_reach_length_fraction: MOUTH_REACH_LENGTH_FRACTION,
                organ_structure_fraction: 0.05,
            },
            Founder::Browser => FounderPhysiology {
                // The browser founder *is* the frondgrazer: the landed round-4 numbers
                // stand, including their deliberately mineral-hungry tissue against
                // foliage. Only the cost settings are new.
                core: SpeciesConfig::frondgrazer(),
                // The frondgrazer gives live birth out of a gestation escrow, which is
                // in `SpeciesConfig::frondgrazer`'s own table.
                motor_respiration_per_s: 0.001,
                // The browser's placeholder climb, one body length's worth of leg:
                // two voxels on `small`, one on `default` and `wide`.
                climb_m: 0.25,
                // The ladder's frondgrazer: 6 x 3 x 3 cells of 0.125 m.
                adult_length_m: 0.375,
                adult_width_m: 0.1875,
                adult_height_m: 0.1875,
                eye_height_fraction: EYE_HEIGHT_FRACTION,
                mouth_ceiling_fraction: MOUTH_CEILING_FRACTION,
                contact_height_fraction: CONTACT_HEIGHT_FRACTION,
                mouth_reach_length_fraction: MOUTH_REACH_LENGTH_FRACTION,
                organ_structure_fraction: 0.10,
            },
        }
    }

    /// Total structure a `core` non-sensory budget buys, with sensor allocation `f`:
    /// `core / (1 − f)`. A caller sizing a founder body from a core budget introduces
    /// this much; the sensor share is inside it, counted once.
    pub fn total_structure_for_core(core: f64, fraction: f64) -> f64 {
        debug_assert!(fraction.is_finite() && fraction >= 0.0 && fraction < 1.0);
        core / (1.0 - fraction)
    }

    /// The sensor/organ share of a total structure stock — a view onto the one body
    /// ledger, never a second stock.
    pub fn sensor_structure(&self, total_body: f64) -> f64 {
        self.organ_structure_fraction * total_body.max(0.0)
    }

    /// The adult's geometry: the authored dimensions, and the anchors as fractions of
    /// them.
    pub fn adult_body(&self) -> Body {
        self.body_scaled(1.0)
    }

    /// The geometry of a body holding `body_organic` of structure: the adult's
    /// dimensions times `(body / body_max)^(1/3)` (decisions §1), so a newborn browser
    /// at `body_min` is 0.46 of the adult's length. Growth changes geometry only; no
    /// rate reads this.
    pub fn body_at(&self, body_organic: f64) -> Body {
        let max = self.core.body_max;
        let fraction = if max.is_finite() && max > 0.0 && body_organic.is_finite() {
            (body_organic / max).clamp(0.0, 1.0)
        } else {
            1.0
        };
        self.body_scaled(fraction.cbrt())
    }

    fn body_scaled(&self, scale: f64) -> Body {
        let scale = if scale.is_finite() && scale > 0.0 {
            scale
        } else {
            0.0
        };
        let length_m = self.adult_length_m * scale;
        let width_m = self.adult_width_m * scale;
        let height_m = self.adult_height_m * scale;
        Body {
            length_m,
            width_m,
            height_m,
            eye_m: self.eye_height_fraction * height_m,
            mouth_ceiling_m: self.mouth_ceiling_fraction * height_m,
            contact_m: self.contact_height_fraction * height_m,
            mouth_reach_m: self.mouth_reach_length_fraction * length_m,
        }
    }
}

/// One tick's resolved motion, for the interval feedback. Attempted and delivered are
/// recorded separately: a wall-constrained attempt is attempted in full and delivered
/// short.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct Motion {
    /// Requested equivalent displacement this tick: `(|v| + r·|yaw rate|) · dt`.
    pub attempted_equivalent: f64,
    /// Delivered equivalent displacement this tick: the distance actually covered plus
    /// the footprint radius times the turn actually made.
    pub delivered_equivalent: f64,
    /// Metres actually covered along the heading (unsigned; forward effort has no
    /// reverse).
    pub delivered_forward: f64,
    /// Radians actually turned, signed.
    pub delivered_turn: f64,
    /// A sub-step was refused (wall, drop, water, the strip's ends) or a clamp at the
    /// `z` ends delivered less than its step asked for.
    pub blocked: bool,
}

/// Resolve one tick of held actions against the continuous pose: turn, then sweep the
/// footprint forward in bounded sub-steps. `pose` is advanced in place; nothing here
/// reads a target, a stock or a route.
#[allow(clippy::too_many_arguments)]
pub(crate) fn resolve_motion(
    view: &VoxelView<'_>,
    pose: &mut crate::Pose,
    standing_y: &mut u32,
    manifest: &Manifest,
    body: &Body,
    wade_depth_m: f64,
    climb: u32,
    held: Actions,
) -> Motion {
    let r = body.footprint_radius();
    let headroom = body.headroom_voxels(view.config.voxel_m);
    let v_req = held.forward * manifest.cruise_m_per_s;
    let yaw_rate = held.turn * manifest.yaw_cap_rad_per_s;
    let attempted = (v_req.abs() + r * yaw_rate.abs()) * DT;

    // Turn first; the sweep then runs along the heading the body now faces.
    let dyaw = yaw_rate * DT;
    pose.heading_rad = (pose.heading_rad + dyaw).rem_euclid(std::f64::consts::TAU);

    // Bounded sub-steps of at most one footprint radius: the overlap window of a
    // one-voxel wall is `voxel + 2r` wide and the sub-step is `r`, so no endpoint can
    // skip past it. A refused step is followed by a bisection that presses the body up
    // to the obstruction — a body stands against a wall at the wall's face, not a
    // sub-step short of it — and the sweep stops there: the rest of the request was
    // attempted and is paid for, but is not delivered.
    let mut moved = 0.0;
    let mut blocked = false;
    let mut left = v_req * DT;
    while left > 1e-12 {
        let step = left.min(r);
        match step_advance(
            view,
            pose,
            standing_y,
            step,
            r,
            wade_depth_m,
            headroom,
            climb,
        ) {
            Some(actual) => {
                moved += actual;
                left -= step;
                if actual < step - 1e-9 {
                    // A clamp at the strip's `z` ends: the world edge is a wall too.
                    blocked = true;
                }
            }
            None => {
                blocked = true;
                // Press the body up to the obstruction: bisect the largest still-valid
                // advance from where the sweep stopped, without committing the probes,
                // then take it once. The gap left under a wall is a few hundredths of
                // the footprint radius.
                let (px, pz, h) = (pose.x, pose.z, pose.heading_rad);
                let y0 = *standing_y;
                let mut lo = 0.0;
                let mut hi = step;
                for _ in 0..24 {
                    let mid = (lo + hi) / 2.0;
                    if mid <= 1e-12 {
                        break;
                    }
                    if advance_candidate(view, px, pz, h, y0, mid, r, wade_depth_m, headroom, climb)
                        .is_some()
                    {
                        lo = mid;
                    } else {
                        hi = mid;
                    }
                }
                if lo > 1e-12 {
                    let (nx, nz, actual, ny) = advance_candidate(
                        view,
                        px,
                        pz,
                        h,
                        y0,
                        lo,
                        r,
                        wade_depth_m,
                        headroom,
                        climb,
                    )
                    .expect("the bisection's best advance is valid");
                    pose.x = nx;
                    pose.z = nz;
                    *standing_y = ny;
                    moved += actual;
                }
                break;
            }
        }
    }
    Motion {
        attempted_equivalent: attempted,
        delivered_equivalent: moved + r * dyaw.abs(),
        delivered_forward: moved,
        delivered_turn: dyaw,
        blocked,
    }
}

/// One bounded sub-step. `Some(actual)` hands back the distance actually covered — a
/// clamp at the strip's `z` ends delivers the short move it is — and `None` leaves the
/// pose untouched.
#[allow(clippy::too_many_arguments)]
fn step_advance(
    view: &VoxelView<'_>,
    pose: &mut crate::Pose,
    standing_y: &mut u32,
    step: f64,
    r: f64,
    wade_depth_m: f64,
    headroom: u32,
    climb: u32,
) -> Option<f64> {
    let (nx, nz, actual, ny) = advance_candidate(
        view,
        pose.x,
        pose.z,
        pose.heading_rad,
        *standing_y,
        step,
        r,
        wade_depth_m,
        headroom,
        climb,
    )?;
    pose.x = nx;
    pose.z = nz;
    *standing_y = ny;
    Some(actual)
}

/// The candidate one sub-step would move the body to, as a pure query: the new
/// position, the distance actually covered and the **standing layer it would leave the
/// body on**, or `None` when the position is refused.
///
/// The centre column must carry a support face within `climb` voxels of `standing_y`,
/// up or down — the nearest one, ties to the higher — and that face is the standing
/// layer the body ends on. It is the *centre* column's, not a neighbour's, because
/// `site` has to stay a support face: `step::terrain` removes an animal whose site is
/// not one.
///
/// The disc's clearance is checked at the `headroom` layers ([`headroom_voxels`]) over
/// the **highest ground under the disc** — the greatest within-climb support face of any
/// column the disc actually overlaps, never below `standing_y` or the destination. A
/// disc body straddling a riser rests on the upper surface, and this is not a nicety:
/// checking the clearance at the body's own layer alone deadlocks both the ascent and
/// the descent, because the centre has to cross the column boundary while the disc still
/// overlaps the riser, and a sub-step is capped at one footprint radius. A column whose
/// only faces are outside the climb — a wall, a cliff — contributes nothing, so its
/// solid is checked and refuses exactly as before.
///
/// The standing water at the destination must be within the founder's wade depth.
#[allow(clippy::too_many_arguments)]
fn advance_candidate(
    view: &VoxelView<'_>,
    px: f64,
    pz: f64,
    heading: f64,
    standing_y: u32,
    step: f64,
    r: f64,
    wade_depth_m: f64,
    headroom: u32,
    climb: u32,
) -> Option<(f64, f64, f64, u32)> {
    let c = view.config;
    let v = c.voxel_m;
    let width_m = f64::from(c.width) * v;
    let depth_m = f64::from(c.depth) * v;
    let (fx, fz) = (heading.sin(), heading.cos());
    let mut nx = px + fx * step;
    let mut nz = pz + fz * step;
    // Wrap on x; the strip's z ends are walls, not wraps.
    nx = nx.rem_euclid(width_m);
    if nz < r {
        nz = r;
    } else if nz > depth_m - r {
        nz = depth_m - r;
    }
    let cz = (nz / v).floor();
    if cz < 0.0 || cz >= f64::from(c.depth) {
        return None;
    }
    let cx = ((nx / v).floor() as i64).rem_euclid(i64::from(c.width));
    // The step rule: the centre column's support face nearest the body's own standing
    // layer, within the climb. No such face is a wall or a cliff edge, and refuses.
    let ny = step_target_layer(view, cx, cz as u32, standing_y, climb)?;
    // The body rests on the highest ground under its disc while it straddles a riser,
    // so the clearance is checked from there.
    let clearance = disc_ground_layer(view, nx, nz, r, standing_y, climb).max(standing_y.max(ny));
    if (1..=headroom).any(|d| disc_hits_solid(view, nx, nz, clearance + d, r)) {
        return None;
    }
    if view.water_depth_m(cx, ny, cz as u32) > wade_depth_m {
        return None;
    }
    // The projection of the move onto the heading, wrap-aware, so a clamp at the z
    // ends is delivered as the short move it is.
    let mut dx = nx - px;
    if dx > width_m / 2.0 {
        dx -= width_m;
    } else if dx < -width_m / 2.0 {
        dx += width_m;
    }
    Some((nx, nz, (dx * fx + (nz - pz) * fz).max(0.0), ny))
}

/// The support face of column `(x, z)` a body standing on `standing_y` could put its
/// feet on: the one nearest its own layer within `climb` voxels, ties to the higher.
///
/// A column cannot hold two support faces one layer apart — a support face is a solid
/// with void over it — so ties only arise for a climb of two or more, and the higher
/// face is the ground a body walking over the terrain meets first.
fn step_target_layer(
    view: &VoxelView<'_>,
    x: i64,
    z: u32,
    standing_y: u32,
    climb: u32,
) -> Option<u32> {
    let lo = standing_y.saturating_sub(climb);
    let hi = (standing_y + climb).min(view.config.height.saturating_sub(1));
    let mut best: Option<u32> = None;
    for y in lo..=hi {
        if !view.is_support(x, y, z) {
            continue;
        }
        let d = |a: u32| (i64::from(a) - i64::from(standing_y)).abs();
        best = match best {
            None => Some(y),
            Some(b) if d(y) < d(b) || (d(y) == d(b) && y > b) => Some(y),
            keep => keep,
        };
    }
    best
}

/// The highest within-climb support face under the disc at `(cx, cz)`: the surface a
/// disc body straddling a riser actually rests on. `standing_y` when nothing under the
/// disc is higher and reachable.
///
/// The disc's overlap test is [`disc_hits_solid`]'s, to the same strictness, so a column
/// the body merely grazes at exactly the radius does not lift it.
fn disc_ground_layer(
    view: &VoxelView<'_>,
    cx: f64,
    cz: f64,
    r: f64,
    standing_y: u32,
    climb: u32,
) -> u32 {
    if climb == 0 {
        return standing_y;
    }
    let c = view.config;
    let v = c.voxel_m;
    let x0 = ((cx - r) / v).floor() as i64;
    let x1 = ((cx + r) / v).floor() as i64;
    let z0 = ((cz - r) / v).floor() as i64;
    let z1 = ((cz + r) / v).floor() as i64;
    let mut best = standing_y;
    for x in x0..=x1 {
        for z in z0..=z1 {
            if z < 0 || z >= i64::from(c.depth) {
                continue;
            }
            let qx = cx.clamp(x as f64 * v, (x as f64 + 1.0) * v);
            let qz = cz.clamp(z as f64 * v, (z as f64 + 1.0) * v);
            let (ddx, ddz) = (cx - qx, cz - qz);
            if ddx * ddx + ddz * ddz >= r * r {
                continue;
            }
            let wx = x.rem_euclid(i64::from(c.width));
            if let Some(y) = step_target_layer(view, wx, z as u32, standing_y, climb) {
                best = best.max(y);
            }
        }
    }
    best
}

/// Whether the disc at `(cx, cz)` overlaps any solid voxel at `layer`. Strict: a
/// grazing touch at exactly the radius does not block, it contacts.
fn disc_hits_solid(view: &VoxelView<'_>, cx: f64, cz: f64, layer: u32, r: f64) -> bool {
    let c = view.config;
    let v = c.voxel_m;
    if layer >= c.height {
        return false;
    }
    let x0 = ((cx - r) / v).floor() as i64;
    let x1 = ((cx + r) / v).floor() as i64;
    let z0 = ((cz - r) / v).floor() as i64;
    let z1 = ((cz + r) / v).floor() as i64;
    for x in x0..=x1 {
        for z in z0..=z1 {
            if z < 0 || z >= i64::from(c.depth) {
                continue;
            }
            let wx = x.rem_euclid(i64::from(c.width));
            if !view.material_at(wx, layer, z as u32).is_solid() {
                continue;
            }
            // Nearest point of the voxel's square to the disc centre.
            let qx = cx.clamp(x as f64 * v, (x as f64 + 1.0) * v);
            let qz = cz.clamp(z as f64 * v, (z as f64 + 1.0) * v);
            let (ddx, ddz) = (cx - qx, cz - qz);
            if ddx * ddx + ddz * ddz < r * r {
                return true;
            }
        }
    }
    false
}

/// The contact and wet readings of one body, from its geometry right now. `resolved`
/// is false only when the pose cannot be mapped onto the strip at all; every channel
/// that was evaluated is a valid reading, a zero included.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct ContactReading {
    pub front: f64,
    pub left: f64,
    pub right: f64,
    pub underside: f64,
    pub wet: f64,
    pub resolved: bool,
}

impl ContactReading {
    const UNRESOLVED: ContactReading = ContactReading {
        front: 0.0,
        left: 0.0,
        right: 0.0,
        underside: 0.0,
        wet: 0.0,
        resolved: false,
    };
}

/// Read the four contact receptors and the wet response from the body's actual
/// geometry. Front/left/right probe the footprint's boundary arc in their direction
/// (centre and ±45°); the underside reads the support face under the centre; wet is the
/// standing water at the foot, a valid zero when dry.
///
/// **Semantic change, 2026-09-22** (`design/handoffs/voxel-founder-step-2026-09-22.md`):
/// the arcs are probed at `standing_y + 1 + climb`, not at the body layer, because a
/// solid the body could step onto is not a wall. A ledge within the lineage's climb now
/// reads as open ground; a cliff face, a taller rise and a wall read exactly as they did.
/// The receptor is still one layer and one channel — the observation vector's shape and
/// its five Contact slots are untouched — and `climb = 0` reproduces the old reading
/// exactly. A body that stands still while embedded in the riser it just stepped off is
/// the one case the receptor does not describe (see `advance_candidate`); it reads clear
/// there, which is what a body that can step away from it should read.
pub(crate) fn contact_readings(
    view: &VoxelView<'_>,
    pose: &crate::Pose,
    standing_y: u32,
    body: &Body,
    climb: u32,
) -> ContactReading {
    let c = view.config;
    let Some((cx, cz)) = pose.column(c.voxel_m, c.depth) else {
        return ContactReading::UNRESOLVED;
    };
    if standing_y + 1 >= c.height {
        return ContactReading::UNRESOLVED;
    }
    // A probe above the world is open sky, not a wall.
    let layer = body.contact_layer(standing_y, c.voxel_m) + climb;
    let solid_above = layer < c.height;
    let r = body.footprint_radius();
    let wx = cx.rem_euclid(i64::from(c.width));
    let arc = |centre: f64| {
        if solid_above {
            boundary_arc(view, pose, r, layer, centre)
        } else {
            0.0
        }
    };
    ContactReading {
        front: arc(pose.heading_rad),
        left: arc(pose.heading_rad - std::f64::consts::FRAC_PI_2),
        right: arc(pose.heading_rad + std::f64::consts::FRAC_PI_2),
        underside: f64::from(view.is_support(wx, standing_y, cz)),
        wet: f64::from(view.water_depth_m(wx, standing_y, cz) > 0.0),
        resolved: true,
    }
}

/// One boundary arc: probe at the arc's centre and ±45°, a hair beyond the footprint
/// radius so a body pressed against a wall by the sweep's snap reads the wall it is
/// standing on. A body smaller than a voxel that touches a solid covers at least one
/// probe.
fn boundary_arc(view: &VoxelView<'_>, pose: &crate::Pose, r: f64, layer: u32, centre: f64) -> f64 {
    let c = view.config;
    let probe = r * (1.0 + 1e-6);
    for da in [
        0.0,
        -std::f64::consts::FRAC_PI_4,
        std::f64::consts::FRAC_PI_4,
    ] {
        let a = centre + da;
        let px = pose.x + probe * a.sin();
        let pz = pose.z + probe * a.cos();
        let z = (pz / c.voxel_m).floor();
        if z < 0.0 || z >= f64::from(c.depth) {
            continue;
        }
        let wx = ((px / c.voxel_m).floor() as i64).rem_euclid(i64::from(c.width));
        if view.material_at(wx, layer, z as u32).is_solid() {
            return 1.0;
        }
    }
    0.0
}

/// The columns the mouth region covers: the footprint and its short forward reach,
/// sampled at the capsule's extreme points (centre, reach tip, and the tip's forward
/// and lateral extremes). At most the four columns around the body can be involved at
/// these body sizes, and the five probes reach all of them.
pub(crate) fn mouth_columns(
    view: &VoxelView<'_>,
    pose: &crate::Pose,
    body: &Body,
) -> Vec<(i64, u32)> {
    let c = view.config;
    let v = c.voxel_m;
    let r = body.footprint_radius();
    let reach = body.mouth_reach_m;
    let (fx, fz) = pose.forward();
    let (rx, rz) = (fz, -fx);
    let tip = (pose.x + (r + reach) * fx, pose.z + (r + reach) * fz);
    let probes = [
        (pose.x, pose.z),
        tip,
        (tip.0 + r * fx, tip.1 + r * fz),
        (tip.0 + r * rx, tip.1 + r * rz),
        (tip.0 - r * rx, tip.1 - r * rz),
    ];
    let mut cols: Vec<(i64, u32)> = Vec::with_capacity(probes.len());
    for &(px, pz) in &probes {
        let z = (pz / v).floor();
        if z < 0.0 || z >= f64::from(c.depth) {
            continue;
        }
        let x = ((px / v).floor() as i64).rem_euclid(i64::from(c.width));
        let entry = (x, z as u32);
        if !cols.contains(&entry) {
            cols.push(entry);
        }
    }
    cols
}

/// The ground site with the most litter under the mouth region, or `None` when the
/// mouth touches no litter at all. Ties go to the smallest site, so the answer is a
/// pure function of the state and never of storage order.
pub(crate) fn mouth_litter_site(
    fv: &FloraView<'_>,
    cols: &[(i64, u32)],
    standing_y: u32,
) -> Option<Site> {
    let mut best: Option<Site> = None;
    let mut best_amount = 0.0;
    for &(x, z) in cols {
        let site = Site {
            x: x as u32,
            y: standing_y,
            z,
        };
        let litter = fv.ground_at(site).map_or(0.0, |g| g.litter);
        if litter > best_amount
            || (litter > 0.0 && litter == best_amount && best.is_some_and(|b| site < b))
        {
            best = Some(site);
            best_amount = litter;
        }
    }
    best
}

/// A founder's authored climb height expressed on `voxel_m`: the one conversion, in
/// `cubarium_voxel::walk::climb_voxels`, applied to the lineage's own
/// [`FounderPhysiology::climb_m`].
///
/// It is the same kind of thing as every other length on the physiology — a physical
/// distance the consumer discretises once — and since 2026-09-22 it is not the odd one
/// out: the mouth's reach and the body's clearance are metres too
/// (`design/handoffs/voxel-body-anchors-2026-09-22.md`). The browser's 0.25 m climb is
/// two voxels on `small` and one on `default`/`wide`; the shredder's 0.125 m is one on
/// both.
pub fn climb_voxels(phys: &FounderPhysiology, voxel_m: f64) -> u32 {
    cubarium_voxel::walk::climb_voxels(phys.climb_m, voxel_m)
}

/// Whether `headroom` whole voxels of void stand over the face `(x, y, z)`.
pub fn has_headroom(view: &VoxelView<'_>, x: i64, y: u32, z: u32, headroom: u32) -> bool {
    let c = view.config;
    (1..=headroom).all(|d| {
        let yy = y + d;
        yy < c.height && !view.material_at(x, yy, z).is_solid()
    })
}

/// The crown layers one mouth can take food from, standing on `standing_y`: the cells
/// the physical band `[0, 1.33 × body height]` over the standing surface overlaps
/// (decisions §2). [`Body::mouth_layers`], named here because this is where the live
/// mouth asks for it.
pub(crate) fn mouth_crown_layers(
    standing_y: u32,
    body: &Body,
    voxel_m: f64,
) -> std::ops::RangeInclusive<i64> {
    body.mouth_layers(standing_y, voxel_m)
}

/// The stand whose crown cells the mouth region physically touches: a crown cell of
/// `stand.site` in a mouth column, at a layer inside the physical mouth band
/// ([`Body::mouth_layers`]). The most foliage wins, ties to the smallest root site.
/// `None` when the mouth is in air — a neighbouring stand whose crown does not reach the
/// mouth is not mouth input.
pub(crate) fn mouth_foliage_stand(
    fv: &FloraView<'_>,
    view: &VoxelView<'_>,
    cols: &[(i64, u32)],
    standing_y: u32,
    body: &Body,
) -> Option<(Site, f64)> {
    let layers = mouth_crown_layers(standing_y, body, view.config.voxel_m);
    let mut best: Option<(Site, f64)> = None;
    for stand in fv.stands.iter().filter(|s| s.foliage > 0.0) {
        let reachable = reachable_layer_stock(fv, view, stand, cols, &layers);
        if !(reachable > 0.0) {
            continue;
        }
        let better = match best {
            None => true,
            Some((site, stock)) => reachable > stock || (reachable == stock && stand.site < site),
        };
        if better {
            best = Some((stand.site, reachable));
        }
    }
    best
}

/// What a mouth over `cols` whose band selects the cell range `layers` can take from
/// **one stand**: the sum of the stocks of its foliage layers whose disc cell is in
/// range and whose own disc — its own radius, which is a fraction of the crown's —
/// covers one of the columns.
///
/// This is the whole of what layers changed about reach. Before, a stand was in reach
/// or it was not and the answer was its whole `foliage`; now an adult bloomcrown offers
/// a low mouth its basal rosette and keeps its crown three cells up.
pub(crate) fn reachable_layer_stock(
    fv: &FloraView<'_>,
    view: &VoxelView<'_>,
    stand: &cubarium_voxel_flora::Stand,
    cols: &[(i64, u32)],
    layers: &std::ops::RangeInclusive<i64>,
) -> f64 {
    reachable_layers(fv, view, stand, cols, layers)
        .iter()
        .map(|(_, stock)| *stock)
        .sum()
}

/// The same, **per layer**: `(index among the stand's foliage layers, stock)`, which is
/// what an observer reporting a per-layer split reads.
pub(crate) fn reachable_layers(
    fv: &FloraView<'_>,
    view: &VoxelView<'_>,
    stand: &cubarium_voxel_flora::Stand,
    cols: &[(i64, u32)],
    layers: &std::ops::RangeInclusive<i64>,
) -> Vec<(usize, f64)> {
    let width = i64::from(view.config.width);
    let depth = i64::from(view.config.depth);
    let mut out = Vec::new();
    for layer in fv.layers(stand) {
        if !(layer.stock > 0.0) || !layers.contains(&layer.cell) {
            continue;
        }
        let span = layer.radius_v.floor() as i64;
        let r2 = layer.radius_v * layer.radius_v;
        let mut touching = false;
        'cells: for dz in -span..=span {
            for dx in -span..=span {
                if (dx * dx + dz * dz) as f64 > r2 {
                    continue;
                }
                let cx = (i64::from(stand.site.x) + dx).rem_euclid(width);
                let cz = i64::from(stand.site.z) + dz;
                if cz < 0 || cz >= depth {
                    continue;
                }
                if cols.contains(&(cx, cz as u32)) {
                    touching = true;
                    break 'cells;
                }
            }
        }
        if touching {
            out.push((layer.foliage_index.unwrap_or(0), layer.stock));
        }
    }
    out
}

/// Every foliage-bearing stand whose crown touches the actual mouth probe columns.
/// The public fauna wrapper exposes this only as a read-only autopsy diagnostic.
pub(crate) fn mouth_foliage_stands(
    fv: &FloraView<'_>,
    view: &VoxelView<'_>,
    cols: &[(i64, u32)],
    standing_y: u32,
    body: &Body,
) -> Vec<(Site, f64)> {
    foliage_stands_touching(
        fv,
        view,
        cols,
        &mouth_crown_layers(standing_y, body, view.config.voxel_m),
    )
}

/// Every foliage-bearing stand whose crown **cells** sit at a layer in `layers` and
/// intersect `cols`.
///
/// This is [`mouth_foliage_stands`]' own scan with the layer range handed in instead of
/// derived from the manifest, because the layer range is the *only* thing that differs
/// between today's whole-voxel mouth and a metre band measured from the standing surface
/// (`design/voxel-encounter-contract-2026-09-21.md`, "Discretisation"): a crown is one
/// cell thick, so any band over the surface selects a contiguous run of layers. Nothing
/// in the live tick calls this with anything but the mouth's own range.
pub(crate) fn foliage_stands_touching(
    fv: &FloraView<'_>,
    view: &VoxelView<'_>,
    cols: &[(i64, u32)],
    layers: &std::ops::RangeInclusive<i64>,
) -> Vec<(Site, f64)> {
    let mut out = Vec::new();
    for stand in fv.stands.iter().filter(|s| s.foliage > 0.0) {
        let reachable = reachable_layer_stock(fv, view, stand, cols, layers);
        if reachable > 0.0 {
            out.push((stand.site, reachable));
        }
    }
    out
}

/// A taste reading: the contact's cue response, the manifest's fixed resistance for the
/// material actually touched, and whether the mouth contacted anything at all.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct TasteReading {
    pub cue: f64,
    pub resistance: f64,
    pub valid: bool,
}

impl TasteReading {
    /// Zero data, validity zero: no contact, or no channel to read.
    const INVALID: TasteReading = TasteReading {
        cue: 0.0,
        resistance: 0.0,
        valid: false,
    };
}

/// Read chemistry and resistance at the mouth's actual contact. The blind founder's
/// mouth roots at the ground: litter under its mouth produces a litter response, while
/// bare ground is a valid ground contact with zero litter response. The diffused
/// `Chem(litter)` field is deliberately not consulted. The browser's mouth is at body
/// height and is invalid in air.
pub(crate) fn taste_reading(
    fv: &FloraView<'_>,
    view: &VoxelView<'_>,
    pose: &crate::Pose,
    standing_y: u32,
    manifest: &Manifest,
    body: &Body,
    founder: Founder,
) -> TasteReading {
    let sat = manifest.tunings.chem_saturation;
    // The contact-material response: `amount / M_EMIT` cue units through the same curve
    // the field's concentration reads through. The browser's foliage cue is still this —
    // there is no foliage field to sample.
    let response = |amount: f64| {
        let c = (amount / M_EMIT).min(1.0);
        c / (c + sat)
    };
    let cols = mouth_columns(view, pose, body);
    match founder {
        Founder::Blind => match mouth_litter_site(fv, &cols, standing_y) {
            Some(site) => {
                let litter = fv.ground_at(site).map_or(0.0, |g| g.litter);
                TasteReading {
                    cue: response(litter),
                    resistance: resistance_of(manifest, "litter"),
                    valid: true,
                }
            }
            None => TasteReading {
                cue: 0.0,
                resistance: resistance_of(manifest, "ground"),
                valid: true,
            },
        },
        Founder::Browser => match mouth_foliage_stand(fv, view, &cols, standing_y, body) {
            Some((_, foliage)) => TasteReading {
                cue: response(foliage),
                resistance: resistance_of(manifest, "foliage"),
                valid: true,
            },
            // The mouth is in air: no contact, no taste.
            None => TasteReading::INVALID,
        },
    }
}

/// The manifest's fixed resistance for a material class; a class the schema does not
/// map reads 0 rather than an invented number (the manifest's own tests pin the
/// classes each founder maps).
fn resistance_of(manifest: &Manifest, class: &str) -> f64 {
    manifest
        .taste_resistances
        .iter()
        .find(|(c, _)| *c == class)
        .map_or(0.0, |&(_, r)| r)
}

/// The observation vector for one founder body, built from the **pre-action state of
/// this tick**: the body's own stocks and prior-interval feedback, and the receptors'
/// geometry as it stands before anything moves or eats. Everything is clamped; a
/// non-finite input is rejected to zero rather than trained through. `senses` carries the
/// per-arena litter field and trend stores; without it (the live schedule) `Chem` reads
/// zero with validity 0. Taste remains contact-local, while `Light` and the `Cone` are
/// pure geometry. `cone_occupancy` is prepared once for all observations in a controller
/// stage so each browser does not rescan the world.
pub(crate) fn observation(
    fauna: &Fauna,
    i: usize,
    view: &VoxelView<'_>,
    fv: &FloraView<'_>,
    manifest: &Manifest,
    mut senses: Option<&mut crate::Senses>,
    cone_occupancy: Option<&crate::senses::ConeOccupancy>,
) -> Vec<f64> {
    let a = &fauna.animals[i];
    let founder = a
        .founder
        .expect("an observation is built for a founder body");
    let phys = fauna.config.founder(founder);
    // The body this animal actually has right now: the anchors below are fractions of
    // it, so a growing founder's eye, mouth and receptors rise with it (decisions §1).
    let body = phys.body_at(a.body);
    let mut obs = vec![0.0; manifest.inputs()];

    // Self: real body state against the manifest's fixed references.
    let fb = &a.founder_state.feedback;
    obs[0] = clamp01(a.energy / manifest.adult_energy_reference);
    obs[1] = clamp01(a.reserve / manifest.adult_reserve_reference);
    obs[2] = f64::from(
        phys.core.birth_cost > 0.0
            && a.body >= phys.core.birth_body
            && a.reserve >= phys.core.birth_cost,
    );
    obs[3] = clamp01(fb.structural_loss / manifest.structural_reference);
    obs[4] = clamp01(fb.intake / manifest.body_reference);
    obs[5] = clamp01(fb.delivered_forward / manifest.forward_reference_m);
    obs[6] = clamp_sym(fb.delivered_turn / manifest.turn_reference_rad);
    obs[7] = if fb.attempted_equivalent > NONE_REQUESTED {
        clamp01(fb.delivered_equivalent / fb.attempted_equivalent)
    } else {
        1.0
    };

    // Contact(4), Wet and Taste: the body's geometry right now.
    let cm = module(manifest, "Contact(4)");
    let wm = module(manifest, "Wet");
    let tm = module(manifest, "Taste(1)");
    let reading = contact_readings(
        view,
        &a.pose,
        a.site.y,
        &body,
        climb_voxels(phys, view.config.voxel_m),
    );
    let valid = f64::from(reading.resolved);
    obs[cm.offset] = reading.front;
    obs[cm.offset + 1] = reading.left;
    obs[cm.offset + 2] = reading.right;
    obs[cm.offset + 3] = reading.underside;
    obs[cm.offset + 4] = valid;
    obs[wm.offset] = reading.wet;
    obs[wm.offset + 1] = valid;
    let taste = taste_reading(fv, view, &a.pose, a.site.y, manifest, &body, founder);
    obs[tm.offset] = taste.cue;
    obs[tm.offset + 1] = taste.resistance;
    obs[tm.offset + 2] = f64::from(taste.valid);

    // Chem(litter): the arena's litter field at the receptor, response then trend then
    // validity. Blind founder only. Without a senses handle the module reads zero with
    // validity 0.
    if let Some(cm) = module_opt(manifest, "Chem(litter)") {
        if let Some(senses) = senses.as_deref_mut() {
            if let Some(cue) = senses.sample_cue(view, &a.pose, a.site.y) {
                let sat = manifest.tunings.chem_saturation;
                let q = cue / (cue + sat);
                obs[cm.offset] = clamp01(q);
                obs[cm.offset + 1] = senses.advance_chem_trend(a.id, q, manifest);
                obs[cm.offset + 2] = 1.0;
            }
        }
    }

    // Light: uniform sky illumination × terrain exposure. Canopy shading and emission are
    // deferred, so a glowcap is invisible. Valid when the pose resolves to a support.
    if let Some(lm) = module_opt(manifest, "Light") {
        if let Some((cx, cz)) = a.pose.column(view.config.voxel_m, view.config.depth) {
            let wx = cx.rem_euclid(i64::from(view.config.width));
            if view.is_support(wx, a.site.y, cz) {
                obs[lm.offset] = clamp01(view.sky_visibility(wx, a.site.y, cz));
                obs[lm.offset + 1] = 1.0;
            }
        }
    }

    // Cone(3, foliage/body): the fixed ray fan, a fresh reading each observation. No
    // memory, no expansion, no body identity. Geometry alone, so it reads with or without
    // a senses handle.
    if let Some(cn) = module_opt(manifest, "Cone(3, foliage/body)") {
        let occupancy = cone_occupancy.expect("browser observations prepare cone occupancy");
        let cone =
            crate::senses::cone_readings(view, occupancy, a.id, &a.pose, a.site.y, manifest, &body);
        let base = cn.offset;
        for (k, sec) in cone.sectors.iter().enumerate() {
            let o = base + k * 6;
            obs[o] = sec.clear;
            obs[o + 1] = sec.all_proximity;
            obs[o + 2] = sec.foliage_fraction;
            obs[o + 3] = sec.foliage_proximity;
            obs[o + 4] = sec.body_fraction;
            obs[o + 5] = sec.body_proximity;
        }
        obs[base + 18] = f64::from(cone.valid);
    }

    obs
}

/// [`module`] without the panic: `None` when this manifest has no such module.
fn module_opt(manifest: &Manifest, name: &str) -> Option<crate::manifest::Module> {
    manifest.modules.iter().find(|m| m.name == name).copied()
}

fn module(manifest: &Manifest, name: &str) -> crate::manifest::Module {
    *manifest
        .modules
        .iter()
        .find(|m| m.name == name)
        .unwrap_or_else(|| panic!("manifest has no {name} module"))
}

fn clamp01(v: f64) -> f64 {
    if v.is_finite() {
        v.clamp(0.0, 1.0)
    } else {
        0.0
    }
}

fn clamp_sym(v: f64) -> f64 {
    if v.is_finite() {
        v.clamp(-1.0, 1.0)
    } else {
        0.0
    }
}

/// The founder physiology an animal actually runs: its own founder's when it carries
/// the marker, its species' placeholder otherwise. The live heuristic path is the
/// `None` branch and is unchanged.
pub fn effective_config(config: &crate::FaunaConfig, animal: &Animal) -> SpeciesConfig {
    match animal.founder {
        Some(founder) => config.founder(founder).core,
        None => *config.species(animal.species),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::manifest::Founder;
    use cubarium_voxel::{Command as WorldCommand, Config as VoxelConfig, Material, World};
    use cubarium_voxel_flora::{Deposit, DepositKind, Flora, FloraConfig};

    /// A small flat world: 8 × 6 × 6 voxels at 0.25 m, soil 1..=2, so the ground is a
    /// support face at y = 2 and bodies stand in layer 3.
    fn flat_world() -> World {
        let mut world = World::empty(VoxelConfig {
            width: 8,
            height: 6,
            depth: 6,
            voxel_m: 0.25,
            ..VoxelConfig::default()
        });
        for z in 0..6 {
            for x in 0..8 {
                for y in 1..=2 {
                    world.apply(WorldCommand::SetMaterial {
                        x,
                        y,
                        z,
                        material: Material::Soil,
                    });
                }
            }
        }
        world
    }

    fn site(x: u32, z: u32) -> Site {
        Site { x, y: 2, z }
    }

    fn pose_at(x_m: f64, z_m: f64, heading: f64) -> crate::Pose {
        crate::Pose {
            x: x_m,
            z: z_m,
            heading_rad: heading,
        }
    }

    const H: f64 = std::f64::consts::FRAC_PI_2; // heading east (+x)

    /// One lineage's adult geometry, the body every fixture below places.
    pub(super) fn adult(founder: Founder) -> Body {
        crate::FounderPhysiology::frozen(founder).adult_body()
    }

    /// Full cruise over one controller period (five ticks of held action) is exactly
    /// the manifest's forward reference, and the pose follows the heading it was given
    /// — including a wrap across the seam, which is an ordinary move and not a turn.
    #[test]
    fn full_cruise_moves_the_reference_distance_along_the_heading() {
        let world = flat_world();
        let view = world.view();
        let manifest = Founder::Blind.manifest();
        let body = adult(Founder::Blind);
        let mut standing = 2u32;
        let mut pose = pose_at(0.5, 0.5, H);
        let mut total = 0.0;
        for _ in 0..manifest.cadence_ticks() {
            let motion = resolve_motion(
                &view,
                &mut pose,
                &mut standing,
                &manifest,
                &body,
                0.05,
                1,
                Actions {
                    forward: 1.0,
                    turn: 0.0,
                    feed: 0.0,
                },
            );
            total += motion.delivered_forward;
        }
        assert!(
            (total - manifest.forward_reference_m).abs() < 1e-12,
            "one period at full cruise = {}",
            total
        );
        assert!((pose.x - (0.5 + 0.03125)).abs() < 1e-12);
        // Wrap on x: starting just before the seam, the same period lands past it.
        let mut seam = pose_at(8.0 * 0.25 - 0.01, 0.5, H);
        for _ in 0..manifest.cadence_ticks() {
            resolve_motion(
                &view,
                &mut seam,
                &mut standing,
                &manifest,
                &body,
                0.05,
                1,
                Actions {
                    forward: 1.0,
                    turn: 0.0,
                    feed: 0.0,
                },
            );
        }
        assert!(
            seam.x < 0.05,
            "the pose wrapped past the seam to {}, it did not turn or stop",
            seam.x
        );
    }

    /// Turning while stopped is permitted and paid, and the yaw cap is real: one
    /// controller period's turn is exactly the turn reference.
    #[test]
    fn turning_while_stopped_moves_only_the_heading() {
        let world = flat_world();
        let view = world.view();
        let manifest = Founder::Blind.manifest();
        let body = adult(Founder::Blind);
        let mut standing = 2u32;
        let mut pose = pose_at(0.5, 0.5, 0.0);
        let mut total = 0.0;
        let mut paid = 0.0;
        for _ in 0..manifest.cadence_ticks() {
            let motion = resolve_motion(
                &view,
                &mut pose,
                &mut standing,
                &manifest,
                &body,
                0.05,
                1,
                Actions {
                    forward: 0.0,
                    turn: 1.0,
                    feed: 0.0,
                },
            );
            total += motion.delivered_turn;
            paid += motion.delivered_equivalent;
        }
        assert!(
            (total - manifest.turn_reference_rad).abs() < 1e-12,
            "one period at the yaw cap = {}",
            total
        );
        assert!(
            (pose.heading_rad - manifest.turn_reference_rad).abs() < 1e-12,
            "the heading turned to {}",
            pose.heading_rad
        );
        assert_eq!(
            (pose.x, pose.z),
            (0.5, 0.5),
            "a stopped turn does not translate"
        );
        assert!(
            paid > 0.0,
            "the turn was paid in equivalent displacement: r·|yaw| over the period"
        );
    }

    /// A wall constrains the sweep: the delivered motion stops at the wall, the block
    /// is flagged, and the contact receptors report the wall on the right side.
    #[test]
    fn a_wall_constrains_motion_and_reports_on_the_contacted_side() {
        let mut world = flat_world();
        // A two-voxel wall on column x = 4, standing proud at y = 3 and y = 4 — over
        // the founder's one-voxel climb, so it is a wall and not a ledge.
        for y in 3..=4 {
            world.apply(WorldCommand::SetMaterial {
                x: 4,
                y,
                z: 3,
                material: Material::Soil,
            });
        }
        let view = world.view();
        let manifest = Founder::Blind.manifest();
        let body = adult(Founder::Blind);
        // Heading east at z row 3, close enough to the wall that the period would
        // cross into it.
        let mut standing = 2u32;
        let mut pose = pose_at(4.0 * 0.25 - 0.05, 3.0 * 0.25 + 0.125, H);
        let mut blocked = false;
        let mut attempted = 0.0;
        let mut delivered = 0.0;
        for _ in 0..manifest.cadence_ticks() {
            let motion = resolve_motion(
                &view,
                &mut pose,
                &mut standing,
                &manifest,
                &body,
                0.05,
                1,
                Actions {
                    forward: 1.0,
                    turn: 0.0,
                    feed: 0.0,
                },
            );
            blocked |= motion.blocked;
            attempted += motion.attempted_equivalent;
            delivered += motion.delivered_forward;
        }
        assert!(blocked, "the wall stopped the sweep within the period");
        assert!(
            delivered < attempted,
            "attempted {} against delivered {}",
            attempted,
            delivered
        );
        assert!(
            pose.x + body.footprint_radius() <= 4.0 * 0.25 + 1e-9,
            "the disc stopped at the wall's face, at x = {}",
            pose.x
        );

        // The wall is dead ahead: front contact, no side contact.
        let contacts = contact_readings(&view, &pose, 2, &body, 1);
        assert_eq!(contacts.front, 1.0);
        assert_eq!(contacts.left, 0.0);
        assert_eq!(contacts.right, 0.0);
        assert_eq!(contacts.underside, 1.0);
        assert_eq!(contacts.wet, 0.0, "dry is a valid zero");

        // Turn to face north (a −90° turn from east): the wall is now on the body's
        // right, and the contact moved with it — contact location follows the body,
        // not the world's axes.
        let mut turned = pose;
        turned.heading_rad =
            (turned.heading_rad - std::f64::consts::FRAC_PI_2).rem_euclid(std::f64::consts::TAU);
        let contacts = contact_readings(&view, &turned, 2, &body, 1);
        assert_eq!(
            contacts.right, 1.0,
            "the wall the body faces is its right after a −90° turn"
        );
        assert_eq!(contacts.front, 0.0);
    }

    /// A body needs **its own height** of void over the face it stands on
    /// (`design/handoffs/voxel-body-anchors-2026-09-22.md`), not the voxels its mouth
    /// lifts through: the 0.1875 m browser asks for one cell at 0.25 m and two at
    /// 0.125 m, and the 0.0625 m shredder asks for one at either. A void one cell
    /// shorter is not a place to stand and not a place to walk to.
    #[test]
    fn a_slot_shorter_than_the_body_is_not_a_place_to_stand() {
        let browser = adult(Founder::Browser);
        assert_eq!(browser.headroom_voxels(0.25), 1);
        assert_eq!(browser.headroom_voxels(0.125), 2);
        assert_eq!(
            adult(Founder::Blind).headroom_voxels(0.125),
            1,
            "a ground feeder asks for the voxel it stands in"
        );

        // A 0.125 m world: ground at y = 2, then a roof over two columns — x = 4 at
        // y = 5, leaving two cells of room, and x = 5 at y = 4, leaving one.
        let mut world = World::empty(VoxelConfig {
            width: 8,
            height: 8,
            depth: 6,
            voxel_m: 0.125,
            ..VoxelConfig::default()
        });
        for z in 0..6 {
            for x in 0..8 {
                for y in 1..=2 {
                    world.apply(WorldCommand::SetMaterial {
                        x,
                        y,
                        z,
                        material: Material::Soil,
                    });
                }
            }
        }
        for (x, roof) in [(4i64, 5u32), (5, 4)] {
            world.apply(WorldCommand::SetMaterial {
                x,
                y: roof,
                z: 2,
                material: Material::Soil,
            });
        }
        let view = world.view();
        let need = browser.headroom_voxels(view.config.voxel_m);
        assert_eq!(need, 2);
        assert!(
            has_headroom(&view, 4, 2, 2, need),
            "two cells under the roof is the browser's height"
        );
        assert!(
            !has_headroom(&view, 5, 2, 2, need),
            "one cell less is a slot, not a floor"
        );
        // The shredder is half the browser's height and the same slot is a corridor
        // for it: the clearance is the animal's, not the grid's.
        assert!(has_headroom(
            &view,
            5,
            2,
            2,
            adult(Founder::Blind).headroom_voxels(view.config.voxel_m)
        ));

        // And the walk agrees: from the open face at x = 3 the two-cell slot is
        // steppable and the one-cell slot is not.
        let sc = *crate::FaunaConfig::default().founder(Founder::Browser);
        let from = Site { x: 3, y: 2, z: 2 };
        assert_eq!(
            crate::steppable(&view, from, 4, 2, &sc.core, need).len(),
            1,
            "the body fits under the higher roof"
        );
        assert!(
            crate::steppable(&view, from, 5, 2, &sc.core, need).is_empty(),
            "the body does not fit under the lower one"
        );
    }

    /// A rise beyond the climb is a wall and a drop beyond it is a cliff edge: the
    /// founder's centre stays on its standing layer whatever the sweep asks for. The
    /// blind founder's climb is one voxel here, so the fixture is two.
    #[test]
    fn a_drop_and_a_higher_face_constrain_the_centre() {
        let mut world = flat_world();
        // A step up: column x = 5 carries two extra layers, so its support face is at
        // y = 4 — two voxels over the founder's one-voxel climb — and at y = 2 and
        // y = 3 it has no support at all.
        for y in 3..=4 {
            world.apply(WorldCommand::SetMaterial {
                x: 5,
                y,
                z: 2,
                material: Material::Soil,
            });
        }
        let view = world.view();
        let manifest = Founder::Blind.manifest();
        let body = adult(Founder::Blind);
        assert!(view.is_support(5, 4, 2));
        assert!(!view.is_support(5, 2, 2));
        assert!(!view.is_support(5, 3, 2));

        let mut pose = pose_at(5.0 * 0.25 - 0.06, 2.0 * 0.25 + 0.125, H);
        let mut blocked = false;
        let mut standing = 2u32;
        for _ in 0..manifest.cadence_ticks() {
            let motion = resolve_motion(
                &view,
                &mut pose,
                &mut standing,
                &manifest,
                &body,
                0.05,
                1,
                Actions {
                    forward: 1.0,
                    turn: 0.0,
                    feed: 0.0,
                },
            );
            blocked |= motion.blocked;
        }
        assert!(blocked, "the higher face is not steppable");
        let contacts = contact_readings(&view, &pose, 2, &body, 1);
        assert_eq!(
            contacts.front, 1.0,
            "the step-up's wall is at the body layer: contact"
        );
        assert_eq!(contacts.underside, 1.0);

        // A drop: dig column x = 3 at row 4 out entirely, so there is no support face
        // within the founder's one-voxel climb — a pit the body must not walk into.
        for y in 1..=2 {
            world.apply(WorldCommand::SetMaterial {
                x: 3,
                y,
                z: 4,
                material: Material::Air,
            });
        }
        let view = world.view();
        let body = adult(Founder::Blind);
        assert!(!view.is_support(3, 2, 4));
        assert!(!view.is_support(3, 1, 4));
        let mut pose = pose_at(3.0 * 0.25 - 0.025, 4.0 * 0.25 + 0.125, H);
        let mut blocked = false;
        let mut standing = 2u32;
        for _ in 0..manifest.cadence_ticks() {
            let motion = resolve_motion(
                &view,
                &mut pose,
                &mut standing,
                &manifest,
                &body,
                0.05,
                1,
                Actions {
                    forward: 1.0,
                    turn: 0.0,
                    feed: 0.0,
                },
            );
            blocked |= motion.blocked;
        }
        assert!(blocked, "the pit is not steppable either");
        let contacts = contact_readings(&view, &pose, 2, &body, 1);
        assert_eq!(
            contacts.front, 0.0,
            "a drop is open air at body level: the delivery ratio is what reports it"
        );
        assert_eq!(contacts.underside, 1.0);
    }

    /// Standing water deeper than the founder can wade stops the sweep before it.
    #[test]
    fn deep_water_stops_the_sweep() {
        let mut world = flat_world();
        // A pond on column x = 2, row 2: one voxel of free water on the face.
        let volume = 0.25 * 0.25 * 0.25;
        world.apply(WorldCommand::AddWater {
            x: 2,
            y: 3,
            z: 2,
            volume_m3: volume,
        });
        let view = world.view();
        assert!(
            view.water_depth_m(2, 2, 2) > 0.05,
            "the pond is {} deep",
            view.water_depth_m(2, 2, 2)
        );
        let manifest = Founder::Blind.manifest();
        let body = adult(Founder::Blind);
        let mut pose = pose_at(2.0 * 0.25 - 0.01, 2.0 * 0.25 + 0.125, H);
        let mut blocked = false;
        let mut standing = 2u32;
        for _ in 0..manifest.cadence_ticks() {
            let motion = resolve_motion(
                &view,
                &mut pose,
                &mut standing,
                &manifest,
                &body,
                0.05,
                1,
                Actions {
                    forward: 1.0,
                    turn: 0.0,
                    feed: 0.0,
                },
            );
            blocked |= motion.blocked;
        }
        assert!(blocked);
        assert!(
            pose.x < 2.0 * 0.25,
            "the body never entered the pond column"
        );
    }

    /// A remote stock may create a local `Chem` field, but it cannot become Taste until
    /// the mouth physically reaches litter.
    #[test]
    fn blind_taste_reads_contact_stock_not_the_diffused_field() {
        let world = flat_world();
        let view = world.view();
        let mut flora = Flora::new(FloraConfig::default());
        flora.deposit(
            site(4, 2),
            Deposit {
                kind: DepositKind::Litter,
                organic: 0.2,
                mineral: 0.2 * 0.02,
                energy: 0.2 * 2.0,
            },
        );
        let mut senses = crate::Senses::new();
        let (updates, converged) = senses.settle(&view, &flora.view());
        assert!(converged, "the field settled in {updates} updates");
        let manifest = Founder::Blind.manifest();
        let body = adult(Founder::Blind);
        let pose = pose_at(2.0 * 0.25 + 0.125, 2.0 * 0.25 + 0.125, -H);
        let field = senses.sample_cue(&view, &pose, 2).expect("supported");
        assert!(
            field > 0.0,
            "the remote litter produces a local diffused cue"
        );
        let t = taste_reading(
            &flora.view(),
            &view,
            &pose,
            2,
            &manifest,
            &body,
            Founder::Blind,
        );
        assert_eq!(t.cue, 0.0, "remote litter is not mouth chemistry");
        assert!(t.valid, "bare ground is still an actual mouth contact");
        assert!((t.resistance - 0.5).abs() < 1e-12);

        flora.deposit(
            site(2, 2),
            Deposit {
                kind: DepositKind::Litter,
                organic: 0.2,
                mineral: 0.2 * 0.02,
                energy: 0.2 * 2.0,
            },
        );
        let t = taste_reading(
            &flora.view(),
            &view,
            &pose,
            2,
            &manifest,
            &body,
            Founder::Blind,
        );
        assert!(
            t.cue > 0.2,
            "contact with litter reads its stock: {}",
            t.cue
        );
        assert!((t.resistance - 0.2).abs() < 1e-12, "the litter resistance");
    }

    /// Taste is a contact sensor and remains available without the remote cue field.
    #[test]
    fn blind_taste_on_bare_ground_is_valid_zero() {
        let world = flat_world();
        let view = world.view();
        let flora = Flora::new(FloraConfig::default());
        let manifest = Founder::Blind.manifest();
        let body = adult(Founder::Blind);
        let pose = pose_at(2.0 * 0.25 + 0.125, 2.0 * 0.25 + 0.125, 0.0);
        let t = taste_reading(
            &flora.view(),
            &view,
            &pose,
            2,
            &manifest,
            &body,
            Founder::Blind,
        );
        assert_eq!(t.cue, 0.0);
        assert_eq!(t.resistance, 0.5);
        assert!(t.valid);
    }

    /// The browser's mouth is at body height: a stand's crown cell in the mouth region
    /// is a valid contact, and the same stand one voxel beyond the reach is not mouth
    /// input.
    #[test]
    fn browser_taste_is_invalid_until_a_crown_reaches_the_mouth() {
        let world = flat_world();
        let view = world.view();
        let mut flora = Flora::new(FloraConfig::default());
        // A springturf on column (2, 2): its crown cell is one voxel above the face —
        // exactly the standing body's layer.
        let wood = 0.5
            * flora
                .config()
                .species(cubarium_voxel_flora::Species::Springturf)
                .wood_max;
        assert!(flora.apply(
            &world,
            cubarium_voxel_flora::Command::Seed {
                x: 2,
                z: 2,
                species: cubarium_voxel_flora::Species::Springturf,
                wood,
            },
        ));
        let fv = flora.view();
        let manifest = Founder::Browser.manifest();
        let body = adult(Founder::Browser);

        // Standing in the turf itself: contact.
        let pose = pose_at(2.0 * 0.25 + 0.125, 2.0 * 0.25 + 0.125, 0.0);
        let t = taste_reading(&fv, &view, &pose, 2, &manifest, &body, Founder::Browser);
        assert!(
            t.valid,
            "the crown cell is at the body's own column and layer"
        );
        assert!(t.cue > 0.0 && (t.resistance - 0.3).abs() < 1e-12);

        // Standing one column west facing east: the crown is within the mouth reach.
        let near = pose_at(1.0 * 0.25 + 0.125, 2.0 * 0.25 + 0.125, H);
        let t = taste_reading(&fv, &view, &near, 2, &manifest, &body, Founder::Browser);
        assert!(t.valid, "the reach reaches the neighbouring crown cell");

        // Facing away, the crown is behind the mouth: no contact, invalid.
        let mut away = near;
        away.heading_rad = (H + std::f64::consts::PI).rem_euclid(std::f64::consts::TAU);
        let t = taste_reading(&fv, &view, &away, 2, &manifest, &body, Founder::Browser);
        assert!(
            !t.valid,
            "a stock behind the mouth is unreachable and stays unmouthed"
        );
        assert_eq!(t.cue, 0.0);
    }

    /// A controller that records what it was shown and holds rest.
    struct Recorder(std::sync::Arc<std::sync::Mutex<Vec<Vec<f64>>>>);

    impl crate::Controller for Recorder {
        fn drive(&mut self, observation: &[f64]) -> crate::Response {
            self.0.lock().expect("log").push(observation.to_vec());
            crate::Response::Bounded(crate::Actions::REST)
        }

        fn reset(&mut self) {
            self.0.lock().expect("log").clear();
        }
    }

    /// "Initial intake/loss/motion feedback is zero", from a **depleted** start.
    ///
    /// The ticks between an introduction and the first sampling are not an interval the
    /// controller acted in, so channels 3..6 read zero and motor delivery reads 1 — even
    /// though a hungry founder has been paying upkeep out of its own structure since
    /// tick one. Before P2-C this held by accident: a full reserve absorbed that upkeep
    /// and `structural_loss` was never incremented (P2-T finding 3).
    #[test]
    fn the_first_sample_reports_no_prior_interval_from_a_depleted_start() {
        for founder in [Founder::Blind, Founder::Browser] {
            let world = flat_world();
            let mut fauna = Fauna::new(crate::FaunaConfig::default());
            assert!(fauna.apply(
                &world,
                crate::Command::IntroduceFounder {
                    x: 2,
                    z: 2,
                    founder,
                    stores: crate::StartingStores::HUNGRY,
                    heading_rad: H,
                },
            ));
            let id = fauna.view().ledger.births - 1;
            let start_body = fauna.view().animal(id).expect("placed").body;
            let log = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
            assert!(fauna.set_controller(id, Box::new(Recorder(log.clone()))));
            let mut flora = Flora::new(FloraConfig::default());
            let manifest = founder.manifest();
            for _ in 0..manifest.cadence_ticks() {
                fauna.step(&world, &mut flora);
            }
            let log = log.lock().expect("log");
            assert_eq!(log.len(), 1, "{founder:?}: exactly one sampling");
            let obs = &log[0];

            // The premise: with no reserve, those ticks' upkeep really did come out of
            // structure, so there was something for the old rule to leak.
            let a = *fauna.view().animal(id).expect("alive");
            assert_eq!(a.reserve, 0.0, "{founder:?}");
            assert!(
                a.body < start_body,
                "{founder:?}: upkeep must have eaten structure"
            );

            for c in 3..=6 {
                assert_eq!(
                    obs[c], 0.0,
                    "{founder:?}: channel {c} is not a prior interval"
                );
            }
            assert_eq!(
                obs[7], 1.0,
                "{founder:?}: nothing was requested, so delivery is 1"
            );
            // And the *stocks* are not zeroed: they are current state, and a hungry
            // founder's reserve channel says so.
            assert_eq!(obs[1], 0.0, "{founder:?}: an empty reserve reads empty");
            assert!(obs[0] > 0.0, "{founder:?}: it is still alive");
        }
    }

    /// The observation vector: real Self channels against the manifest's references,
    /// real contacts, and the P1-C spans left zero with validity 0.
    #[test]
    fn the_observation_is_real_body_state_with_p1c_spans_zero() {
        let mut world = flat_world();
        world.apply(WorldCommand::SetMaterial {
            x: 4,
            y: 3,
            z: 2,
            material: Material::Soil,
        });
        let mut fauna = Fauna::new(crate::FaunaConfig::default());
        assert!(fauna.apply(
            &world,
            crate::Command::IntroduceFounder {
                x: 2,
                z: 2,
                founder: Founder::Blind,
                stores: crate::StartingStores::FULL,
                heading_rad: H,
            },
        ));
        let mut flora = Flora::new(FloraConfig::default());
        // Litter one column east of the body, so the settled field has a gradient the
        // cue channels can read.
        flora.deposit(
            site(3, 2),
            Deposit {
                kind: DepositKind::Litter,
                organic: 0.2,
                mineral: 0.2 * 0.02,
                energy: 0.2 * 2.0,
            },
        );
        let view = world.view();
        let fv = flora.view();
        let manifest = Founder::Blind.manifest();
        let obs = observation(&fauna, 0, &view, &fv, &manifest, None, None);
        assert_eq!(obs.len(), 23);
        // Introduced at the adult reference: energy and reserve read 1, birth
        // readiness is the real threshold state (adult body, full adult reserve).
        assert!((obs[0] - 1.0).abs() < 1e-9, "energy {}", obs[0]);
        assert!((obs[1] - 1.0).abs() < 1e-9, "reserve {}", obs[1]);
        assert_eq!(obs[2], 1.0);
        // Initial feedback is zero, so no delivery was requested: motor delivery 1.
        assert_eq!(obs[3], 0.0);
        assert_eq!(obs[4], 0.0);
        assert_eq!(obs[5], 0.0);
        assert_eq!(obs[6], 0.0);
        assert_eq!(obs[7], 1.0);
        // The wall two columns east is beyond the footprint+reach: no contact.
        assert_eq!((obs[8], obs[9], obs[10]), (0.0, 0.0, 0.0));
        assert_eq!(obs[11], 1.0, "supported underside");
        assert_eq!(obs[12], 1.0, "contact validity");
        assert_eq!(obs[13], 0.0, "dry");
        assert_eq!(obs[14], 1.0, "wet validity");
        // The litter one column ahead is outside the short mouth reach: bare-ground
        // Taste is valid zero. Chem remains unavailable; Light reads the real sky.
        assert_eq!(obs[15], 0.0, "unreached litter is not taste");
        assert_eq!(obs[16], 0.5, "ground resistance");
        assert_eq!(obs[17], 1.0, "ground contact is valid");
        assert_eq!(&obs[18..21], &[0.0, 0.0, 0.0], "chem stays invalid");
        assert_eq!(obs[22], 1.0, "light validity");
        assert!(
            (0.0..=1.0).contains(&obs[21]),
            "light response is a sky fraction, got {}",
            obs[21]
        );

        // With the arena's field the blind observation also reads remote Chem. Taste
        // remains the independent contact-stock response.
        let mut senses = crate::Senses::new();
        let (updates, converged) = senses.settle(&view, &fv);
        assert!(converged, "settled in {updates}");
        let obs = observation(&fauna, 0, &view, &fv, &manifest, Some(&mut senses), None);
        assert_eq!(obs[20], 1.0, "chem validity");
        assert_eq!(obs[17], 1.0, "ground taste contact remains valid");
        assert_eq!(obs[15], 0.0, "the field does not leak into Taste");
        assert!(obs[18] > 0.0, "a cue in the field here, got {}", obs[18]);
        assert!((-1.0..=1.0).contains(&obs[19]), "trend in range");

        // The browser's observation is 37 wide and its Cone reads the real world: valid,
        // and the wall two columns east shades the front sector at the eye's layer.
        let mut browser = Fauna::new(crate::FaunaConfig::default());
        assert!(browser.apply(
            &world,
            crate::Command::IntroduceFounder {
                x: 2,
                z: 2,
                founder: Founder::Browser,
                stores: crate::StartingStores::FULL,
                heading_rad: H,
            },
        ));
        let cone_occupancy = crate::senses::cone_occupancy(&view, &fv, &browser.view());
        let obs = observation(
            &browser,
            0,
            &view,
            &fv,
            &Founder::Browser.manifest(),
            None,
            Some(&cone_occupancy),
        );
        assert_eq!(obs.len(), 37);
        assert_eq!(obs[36], 1.0, "the cone reads the real world");
        assert!(
            obs[24] < 1.0,
            "the front sector is occluded by the wall, clear = {}",
            obs[24]
        );
        assert!(obs[18..36].iter().all(|v| (-1.0..=1.0).contains(v)));
    }

    /// The organ-allocation rule: total structure for a core budget is core/(1−f), the
    /// sensor share is inside that total once, and the frozen table carries the plan's
    /// 5% and 10%.
    #[test]
    fn the_organ_allocation_counts_the_sensor_share_once() {
        let blind = FounderPhysiology::frozen(Founder::Blind);
        let browser = FounderPhysiology::frozen(Founder::Browser);
        assert_eq!(blind.organ_structure_fraction, 0.05);
        assert_eq!(browser.organ_structure_fraction, 0.10);
        let core = 0.011875;
        let total =
            FounderPhysiology::total_structure_for_core(core, blind.organ_structure_fraction);
        assert!((total - 0.0125).abs() < 1e-12);
        assert!((blind.sensor_structure(total) - 0.05 * total).abs() < 1e-15);
        // The rule does not create matter: sensor + core is the total, not more.
        assert!((blind.sensor_structure(total) + core - total).abs() < 1e-12);
    }

    /// The mouth band is a **length**, so the same animal selects the same physical
    /// layers on either grid: the adult browser's `1.33 × 0.1875 = 0.249375` m ceiling
    /// is one 0.25 m cell and two 0.125 m cells, which are the same 0.25 m of air. The
    /// shredder's 0.0831 m ceiling is the standing layer alone on both.
    #[test]
    fn the_mouth_band_selects_the_same_physical_layers_on_both_grids() {
        let browser = adult(Founder::Browser);
        let blind = adult(Founder::Blind);
        assert!((browser.mouth_ceiling_m - 0.249375).abs() < 1e-12);
        assert_eq!(mouth_crown_layers(2, &browser, 0.25), 3..=3);
        assert_eq!(mouth_crown_layers(2, &browser, 0.125), 3..=4);
        assert_eq!(mouth_crown_layers(2, &blind, 0.25), 3..=3);
        assert_eq!(mouth_crown_layers(2, &blind, 0.125), 3..=3);
        // A touch at exactly the ceiling contributes nothing: a body whose ceiling is
        // one whole cell reaches that cell and not the one starting there.
        let mut exact = browser;
        exact.mouth_ceiling_m = 0.25;
        assert_eq!(mouth_crown_layers(2, &exact, 0.25), 3..=3);
    }

    /// The counterfactual behind the band: the **same** fixture, the same mouth columns
    /// and the same two crowns, read with the adult browser's 0.249 m ceiling and again
    /// with a half-metre one. The crown one 0.25 m voxel above the head — which the old
    /// whole-voxel reach of one accepted — is out of the adult's physical band, and the
    /// crown at the head layer is in it either way. The band is what decides, and the
    /// manifest's `mouth_reach_up_voxels` is not read at all.
    #[test]
    fn the_band_and_not_a_voxel_count_decides_what_the_mouth_takes() {
        use cubarium_voxel_flora::{Command as FloraCommand, Species as Plant};

        let world = flat_world();
        let view = world.view();
        // The subject is the **band**, so the plants are the one-disc lollipops this
        // test was written against: what an adult bloomcrown's basal rosette does under
        // the same band is `tests/plant_layers.rs`'s claim.
        let mut flora = Flora::new(FloraConfig::default().one_layer_species());
        // bloomcrown wood 0.30 rounds to a two-voxel crown, so its crown layer is the
        // body's head layer plus one; wood 0.12 rounds to one and sits at the head.
        for (x, wood) in [(2i64, 0.30), (5, 0.12)] {
            assert!(flora.apply(
                &world,
                FloraCommand::Seed {
                    x,
                    z: 2,
                    species: Plant::Bloomcrown,
                    wood,
                },
            ));
        }
        let fv = flora.view();
        let browser = adult(Founder::Browser);

        let cols_over = mouth_columns(&view, &pose_at(2.5 * 0.25, 2.5 * 0.25, 0.0), &browser);
        let cols_head = mouth_columns(&view, &pose_at(5.5 * 0.25, 2.5 * 0.25, 0.0), &browser);

        assert_eq!(
            mouth_foliage_stand(&fv, &view, &cols_over, 2, &browser),
            None,
            "0.25 m of crown is above a 0.249375 m ceiling"
        );
        assert_eq!(
            mouth_foliage_stand(&fv, &view, &cols_head, 2, &browser).map(|(s, _)| s),
            Some(site(5, 2)),
            "the crown at the head layer is inside the band"
        );

        // A taller animal, same fixture: the band rises with the body and the crown
        // over the head comes into reach. Nothing else about the mouth changed.
        let mut tall = browser;
        tall.mouth_ceiling_m = 0.5;
        assert_eq!(
            mouth_foliage_stand(&fv, &view, &cols_over, 2, &tall).map(|(s, _)| s),
            Some(site(2, 2)),
            "a 0.5 m band reaches the crown one voxel up"
        );
        assert_eq!(
            mouth_foliage_stand(&fv, &view, &cols_head, 2, &tall).map(|(s, _)| s),
            Some(site(5, 2)),
            "and still takes the one at its feet"
        );
    }
}

#[cfg(test)]
mod step_rule_tests {
    //! The founder step rule (`design/handoffs/voxel-founder-step-2026-09-22.md`).
    //!
    //! Written from the brief before the implementation: a lineage has a climb height
    //! in metres, converted to whole voxels at the consumer, and a sub-step may move
    //! the centre column to a support face within that many layers of the body's own,
    //! up or down. Contact reports only solids the body could **not** step onto.
    //!
    //! Every case here is a handful of ticks on an 8-column fixture.

    use super::tests::adult;
    use super::*;
    use crate::manifest::Founder;
    use cubarium_voxel::{Command as WorldCommand, Config as VoxelConfig, Material, World};

    /// A terraced strip: 16 × `height` × 6 at `voxel_m`, soil in `1..=base`, and the
    /// columns in `raised` carrying `rise` layers more. The support face of the low
    /// ground is `base`; of the terrace, `base + rise`.
    fn terrace(
        voxel_m: f64,
        height: u32,
        base: u32,
        raised: std::ops::Range<i64>,
        rise: u32,
    ) -> World {
        let mut world = World::empty(VoxelConfig {
            width: 16,
            height,
            depth: 6,
            voxel_m,
            ..VoxelConfig::default()
        });
        for z in 0..6 {
            for x in 0..16 {
                for y in 1..=base {
                    world.apply(WorldCommand::SetMaterial {
                        x,
                        y,
                        z,
                        material: Material::Soil,
                    });
                }
            }
        }
        for z in 0..6 {
            for x in raised.clone() {
                for y in base + 1..=base + rise {
                    world.apply(WorldCommand::SetMaterial {
                        x,
                        y,
                        z,
                        material: Material::Soil,
                    });
                }
            }
        }
        world
    }

    /// A pit: soil in `1..=base` everywhere, then the columns in `sunk` dug down by
    /// `drop` layers, so their support face is `base - drop`.
    fn pit(voxel_m: f64, height: u32, base: u32, sunk: std::ops::Range<i64>, drop: u32) -> World {
        let mut world = terrace(voxel_m, height, base, 0..0, 0);
        for z in 0..6 {
            for x in sunk.clone() {
                for y in base - drop + 1..=base {
                    world.apply(WorldCommand::SetMaterial {
                        x,
                        y,
                        z,
                        material: Material::Air,
                    });
                }
            }
        }
        world
    }

    /// Walk a body east at full cruise and report where it got to: the standing layer
    /// after each tick, in order, and the final pose. `ticks` stays small.
    #[allow(clippy::too_many_arguments)]
    fn walk_east(
        world: &World,
        manifest: &Manifest,
        body: &Body,
        climb: u32,
        start_x_m: f64,
        y0: u32,
        ticks: u32,
    ) -> (crate::Pose, Vec<u32>) {
        let view = world.view();
        let v = view.config.voxel_m;
        let mut pose = crate::Pose {
            x: start_x_m,
            z: 2.5 * v,
            heading_rad: std::f64::consts::FRAC_PI_2,
        };
        let mut y = y0;
        let mut layers = Vec::new();
        for _ in 0..ticks {
            resolve_motion(
                &view,
                &mut pose,
                &mut y,
                manifest,
                body,
                0.05,
                climb,
                Actions {
                    forward: 1.0,
                    turn: 0.0,
                    feed: 0.0,
                },
            );
            layers.push(y);
        }
        (pose, layers)
    }

    /// The conversion is one rounding of a physical height, and it is the founders'
    /// authored placeholders that go through it: the browser's 0.25 m is two voxels on
    /// the 0.125 m preset and one on the 0.25 m presets; the shredder's 0.125 m is one
    /// on both.
    #[test]
    fn the_climb_height_converts_to_whole_voxels_once() {
        let browser = crate::FounderPhysiology::frozen(Founder::Browser);
        let shredder = crate::FounderPhysiology::frozen(Founder::Blind);
        assert_eq!(browser.climb_m, 0.25);
        assert_eq!(shredder.climb_m, 0.125);
        assert_eq!(climb_voxels(&browser, 0.125), 2);
        assert_eq!(climb_voxels(&browser, 0.25), 1);
        assert_eq!(climb_voxels(&shredder, 0.125), 1);
        assert_eq!(climb_voxels(&shredder, 0.25), 1);
    }

    /// One voxel up onto a terrace and one voxel back down off it, on the 0.25 m
    /// reference grid, for a browser whose 0.25 m climb is one voxel there. The
    /// standing layer follows the ground under the body.
    #[test]
    fn a_browser_steps_up_and_down_one_voxel_on_the_reference_grid() {
        let world = terrace(0.25, 10, 2, 5..8, 1);
        let manifest = Founder::Browser.manifest();
        let body = adult(Founder::Browser);
        let (pose, layers) = walk_east(&world, &manifest, &body, 1, 3.5 * 0.25, 2, 160);
        assert!(
            layers.contains(&3),
            "the body never got onto the terrace: {layers:?}"
        );
        assert_eq!(
            *layers.last().unwrap(),
            2,
            "and never got off it again: {layers:?}"
        );
        assert!(
            pose.x > 8.0 * 0.25,
            "the body is past the terrace at {}",
            pose.x
        );
    }

    /// The shredder's 0.125 m climb is one voxel at either scale, so a one-voxel
    /// terrace is a step on both presets.
    #[test]
    fn a_shredder_steps_one_voxel_at_either_scale() {
        for v in [0.125, 0.25] {
            let world = terrace(v, 12, 2, 5..8, 1);
            let manifest = Founder::Blind.manifest();
            let body = adult(Founder::Blind);
            let (_, layers) = walk_east(&world, &manifest, &body, 1, 3.5 * v, 2, 60);
            assert!(
                layers.contains(&3),
                "a 1-voxel terrace at {v} m is not a step: {layers:?}"
            );
        }
    }

    /// Two voxels is over the browser's climb on the 0.25 m grid (0.5 m of rise
    /// against a 0.25 m climb) and exactly at it on the 0.125 m grid. The same animal,
    /// the same physical rise of 0.25 m, refused on one grid and taken on the other is
    /// the discretisation working, not the rule changing.
    #[test]
    fn a_two_voxel_rise_refuses_on_the_reference_grid_and_passes_on_the_fine_one() {
        let manifest = Founder::Browser.manifest();
        let body = adult(Founder::Browser);

        let coarse = terrace(0.25, 10, 2, 5..8, 2);
        let (pose, layers) = walk_east(&coarse, &manifest, &body, 1, 3.5 * 0.25, 2, 40);
        assert!(
            layers.iter().all(|&y| y == 2),
            "a 0.5 m rise is a wall for a 0.25 m climb: {layers:?}"
        );
        assert!(
            pose.x < 5.0 * 0.25,
            "the body stopped at the wall, not on it: {}",
            pose.x
        );

        let fine = terrace(0.125, 14, 4, 5..8, 2);
        let (_, layers) = walk_east(&fine, &manifest, &body, 2, 3.5 * 0.125, 4, 60);
        assert!(
            layers.contains(&6),
            "the same 0.25 m rise is two voxels of climb here: {layers:?}"
        );
    }

    /// A drop deeper than the climb is a cliff edge: the body stops on it and does not
    /// fall. A drop within the climb is a step (the terrace test's descent).
    #[test]
    fn a_drop_beyond_the_climb_refuses() {
        let manifest = Founder::Browser.manifest();
        let body = adult(Founder::Browser);
        let world = pit(0.25, 10, 4, 5..9, 2);
        let (pose, layers) = walk_east(&world, &manifest, &body, 1, 3.5 * 0.25, 4, 40);
        assert!(
            layers.iter().all(|&y| y == 4),
            "the body walked off a 0.5 m cliff: {layers:?}"
        );
        assert!(
            pose.x < 5.0 * 0.25,
            "the body should stand on the rim at {}",
            pose.x
        );

        // One voxel of the same pit is a step down.
        let shallow = pit(0.25, 10, 4, 5..9, 1);
        let (_, layers) = walk_east(&shallow, &manifest, &body, 1, 3.5 * 0.25, 4, 40);
        assert!(
            layers.contains(&3),
            "a 0.25 m drop is within the climb: {layers:?}"
        );
    }

    /// Contact is about walls, not about ground: a ledge the body could step onto
    /// reads as open, a ledge it could not reads as solid. The channel count does not
    /// change and the underside still reads the face the body is standing on.
    #[test]
    fn contact_reads_a_steppable_ledge_as_clear_and_a_taller_one_as_solid() {
        let body = adult(Founder::Browser);
        // Standing in column 4, facing east, pressed up against the terrace at x = 5.
        let pose = crate::Pose {
            x: 5.0 * 0.25 - 0.0624,
            z: 2.5 * 0.25,
            heading_rad: std::f64::consts::FRAC_PI_2,
        };

        let ledge = terrace(0.25, 10, 2, 5..8, 1);
        let reading = contact_readings(&ledge.view(), &pose, 2, &body, 1);
        assert_eq!(
            reading.front, 0.0,
            "a one-voxel ledge within the climb is ground, not a wall"
        );
        assert_eq!(reading.underside, 1.0);
        assert!(reading.resolved);

        let wall = terrace(0.25, 10, 2, 5..8, 2);
        let reading = contact_readings(&wall.view(), &pose, 2, &body, 1);
        assert_eq!(
            reading.front, 1.0,
            "a two-voxel rise is over the climb: a wall, as before"
        );
        assert_eq!(reading.underside, 1.0);

        // A climb of zero is the old receptor exactly: the ledge is a wall again.
        let reading = contact_readings(&ledge.view(), &pose, 2, &body, 0);
        assert_eq!(reading.front, 1.0);
    }
}
