//! **The phase-one founder body**: paid heading motion on the continuous pose, the
//! contact/taste receptors of the manifest, and the geometry a local bite is allowed to
//! touch (`design/voxel-senses-phase1-plan.md`, "Phase-one body and action contract").
//!
//! The body is an upright disc on the support layer: footprint radius
//! `body_width / 2` (width is half the length, so the disc fits the length), centred on
//! the [`Pose`](crate::Pose)'s `(x, z)`, standing on the support face one layer below
//! its centre column. The forward mouth is the footprint plus a short reach of
//! `mouth_reach_body_lengths` body lengths beyond it.
//!
//! # What movement is allowed to do
//!
//! Forward effort sweeps the disc along the heading in bounded sub-steps of at most one
//! footprint radius, so a wall (a solid voxel at the body layer is at least one voxel
//! thick) cannot be tunnelled between endpoint checks. `x` wraps; the strip's `z` ends
//! are hard walls; a candidate position is refused when its disc would overlap a solid
//! at the body layer, when the centre column has no support face at the standing layer
//! (a drop or a pit — **no step to a higher support and no walking off a drop**), or
//! when the standing water there is deeper than the founder can wade. A refused step
//! stops the sweep: movement is constrained, the delivered motion is what actually
//! happened, and the requested equivalent displacement is paid for anyway. There is no
//! graph search, no waypoint, no nearest-food target and no turn-to-target anywhere in
//! this path — the controller's turn effort is the only yaw input there is.
//!
//! # Receptors
//!
//! Front/left/right contact receptors probe the footprint's boundary arc in their
//! direction (the arc centre and ±45°, at the footprint radius) for a solid voxel at the
//! body layer; the bodies are smaller than a voxel, so a touching solid always covers
//! one of the samples. The underside receptor reads the support face under the centre.
//! `Wet` is the standing water at the foot, a valid zero when dry. `Taste` reads only
//! what the mouth region physically overlaps: the ground stocks under the footprint and
//! forward reach for the blind founder, a stand's crown cells at the body layer for the
//! browser — never a remote query, never a neighbour outside the reach.
//!
//! The modules P1-C owns (`Chem`, `Light`, `Cone`) are left **zero with validity 0** in
//! the observation: this module fabricates nothing it does not measure.

use cubarium_voxel::{VoxelView, DT};
use cubarium_voxel_flora::{FloraView, Reach, Site};
use serde::{Deserialize, Serialize};

use crate::controller::Actions;
use crate::manifest::{Founder, Manifest};
use crate::{Animal, Fauna, SpeciesConfig};

/// The cue-unit reference the plan fixes for the litter cue (`M_emit`, "Initial cue
/// field settings"): `M_EMIT` organic-material units of a contacted material read at
/// full cue strength. The taste probe uses the same reference the P1-C litter field
/// will emit with, so a taste and a `Chem` response at the same spot agree.
pub(crate) const M_EMIT: f64 = 0.05;

/// Below this much requested equivalent displacement an interval counts as "none
/// requested" for the motor-delivery channel, which then reads 1 by contract.
const NONE_REQUESTED: f64 = 1e-12;

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
                },
                motor_respiration_per_s: 0.001,
                organ_structure_fraction: 0.05,
            },
            Founder::Browser => FounderPhysiology {
                // The browser founder *is* the frondgrazer: the landed round-4 numbers
                // stand, including their deliberately mineral-hungry tissue against
                // foliage. Only the cost settings are new.
                core: SpeciesConfig::frondgrazer(),
                motor_respiration_per_s: 0.001,
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
}

/// The footprint radius: half the manifest's body width, the upright disc the plan
/// starts these founders with.
pub(crate) fn footprint_radius(manifest: &Manifest) -> f64 {
    manifest.body_width_m / 2.0
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
pub(crate) fn resolve_motion(
    view: &VoxelView<'_>,
    pose: &mut crate::Pose,
    standing_y: u32,
    manifest: &Manifest,
    wade_depth_m: f64,
    held: Actions,
) -> Motion {
    let r = footprint_radius(manifest);
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
        match step_advance(view, pose, standing_y, step, r, wade_depth_m) {
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
                let mut lo = 0.0;
                let mut hi = step;
                for _ in 0..24 {
                    let mid = (lo + hi) / 2.0;
                    if mid <= 1e-12 {
                        break;
                    }
                    if advance_candidate(view, px, pz, h, standing_y, mid, r, wade_depth_m)
                        .is_some()
                    {
                        lo = mid;
                    } else {
                        hi = mid;
                    }
                }
                if lo > 1e-12 {
                    let (nx, nz, actual) = advance_candidate(
                        view,
                        px,
                        pz,
                        h,
                        standing_y,
                        lo,
                        r,
                        wade_depth_m,
                    )
                    .expect("the bisection's best advance is valid");
                    pose.x = nx;
                    pose.z = nz;
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
fn step_advance(
    view: &VoxelView<'_>,
    pose: &mut crate::Pose,
    standing_y: u32,
    step: f64,
    r: f64,
    wade_depth_m: f64,
) -> Option<f64> {
    let (nx, nz, actual) = advance_candidate(
        view,
        pose.x,
        pose.z,
        pose.heading_rad,
        standing_y,
        step,
        r,
        wade_depth_m,
    )?;
    pose.x = nx;
    pose.z = nz;
    Some(actual)
}

/// The candidate one sub-step would move the body to, as a pure query: the new
/// position and the distance actually covered, or `None` when the position is refused.
/// The candidate must keep the disc off every solid voxel at the body layer, the centre
/// column supported on the standing layer, and the standing water within the founder's
/// wade depth.
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
) -> Option<(f64, f64, f64)> {
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
    if disc_hits_solid(view, nx, nz, standing_y + 1, r) {
        return None;
    }
    let cz = (nz / v).floor();
    if cz < 0.0 || cz >= f64::from(c.depth) {
        return None;
    }
    let cx = ((nx / v).floor() as i64).rem_euclid(i64::from(c.width));
    // The centre column must still be a support face at the standing layer: no step to
    // a higher support, and no walking off a drop either.
    if !view.is_support(cx, standing_y, cz as u32) {
        return None;
    }
    if view.water_depth_m(cx, standing_y, cz as u32) > wade_depth_m {
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
    Some((nx, nz, (dx * fx + (nz - pz) * fz).max(0.0)))
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
/// (centre and ±45°) for a solid voxel at the body layer; the underside reads the
/// support face under the centre; wet is the standing water at the foot, a valid zero
/// when dry.
pub(crate) fn contact_readings(
    view: &VoxelView<'_>,
    pose: &crate::Pose,
    standing_y: u32,
    manifest: &Manifest,
) -> ContactReading {
    let c = view.config;
    let Some((cx, cz)) = pose.column(c.voxel_m, c.depth) else {
        return ContactReading::UNRESOLVED;
    };
    let layer = standing_y + 1;
    if layer >= c.height {
        return ContactReading::UNRESOLVED;
    }
    let r = footprint_radius(manifest);
    let wx = cx.rem_euclid(i64::from(c.width));
    ContactReading {
        front: boundary_arc(view, pose, r, layer, pose.heading_rad),
        left: boundary_arc(view, pose, r, layer, pose.heading_rad - std::f64::consts::FRAC_PI_2),
        right: boundary_arc(view, pose, r, layer, pose.heading_rad + std::f64::consts::FRAC_PI_2),
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
    for da in [0.0, -std::f64::consts::FRAC_PI_4, std::f64::consts::FRAC_PI_4] {
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
pub(crate) fn mouth_columns(view: &VoxelView<'_>, pose: &crate::Pose, manifest: &Manifest) -> Vec<(i64, u32)> {
    let c = view.config;
    let v = c.voxel_m;
    let r = footprint_radius(manifest);
    let reach = manifest.mouth_reach_body_lengths * manifest.body_length_m;
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
        if litter > best_amount || (litter > 0.0 && litter == best_amount && best.is_some_and(|b| site < b)) {
            best = Some(site);
            best_amount = litter;
        }
    }
    best
}

/// The stand whose crown cells the mouth region physically touches, at the body's own
/// layer: a crown cell of `stand.site` in a mouth column, at the standing layer plus
/// one. The most foliage wins, ties to the smallest root site. `None` when the mouth is
/// in air — a neighbouring stand whose crown does not reach the mouth is not mouth
/// input.
pub(crate) fn mouth_foliage_stand(
    fv: &FloraView<'_>,
    view: &VoxelView<'_>,
    cols: &[(i64, u32)],
    standing_y: u32,
) -> Option<(Site, f64)> {
    let body_layer = i64::from(standing_y) + 1;
    let width = i64::from(view.config.width);
    let depth = i64::from(view.config.depth);
    let mut best: Option<(Site, f64)> = None;
    for stand in fv.stands.iter().filter(|s| s.foliage > 0.0) {
        let sc = fv.config.species(stand.species);
        if i64::from(stand.site.y) + i64::from(sc.crown_voxels(stand.wood)) != body_layer {
            continue;
        }
        let radius = sc.crown_radius(stand.wood).max(0.0);
        let span = radius.floor() as i64;
        let r2 = radius * radius;
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
            let better = match best {
                None => true,
                Some((site, foliage)) => {
                    stand.foliage > foliage || (stand.foliage == foliage && stand.site < site)
                }
            };
            if better {
                best = Some((stand.site, stand.foliage));
            }
        }
    }
    best
}

/// A taste reading: the contact's cue response, the manifest's fixed resistance for the
/// material actually touched, and whether the mouth contacted anything at all.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct TasteReading {
    pub cue: f64,
    pub resistance: f64,
    pub valid: bool,
}

/// Read the mouth. The blind founder's mouth roots at the ground: it always contacts
/// the ground it stands on, so a bare patch is a **valid zero** cue at the ground's
/// resistance, and litter reads through the plan's response curve at the contact's own
/// amount. The browser's mouth is at body height: it contacts a stand's crown cells
/// when they reach the mouth region, and is **invalid** in air.
pub(crate) fn taste_reading(
    fv: &FloraView<'_>,
    view: &VoxelView<'_>,
    pose: &crate::Pose,
    standing_y: u32,
    manifest: &Manifest,
    founder: Founder,
) -> TasteReading {
    let sat = manifest.tunings.chem_saturation;
    let response = |amount: f64| {
        let c = (amount / M_EMIT).min(1.0);
        c / (c + sat)
    };
    let cols = mouth_columns(view, pose, manifest);
    match founder {
        Founder::Blind => match mouth_litter_site(fv, &cols, standing_y) {
            Some(site) => {
                let amount = fv.ground_at(site).map_or(0.0, |g| g.litter);
                TasteReading {
                    cue: response(amount),
                    resistance: resistance_of(manifest, "litter"),
                    valid: true,
                }
            }
            // No litter in reach: the mouth still contacts the ground itself — a valid
            // zero cue at the ground's resistance.
            None => TasteReading {
                cue: 0.0,
                resistance: resistance_of(manifest, "ground"),
                valid: true,
            },
        },
        Founder::Browser => match mouth_foliage_stand(fv, view, &cols, standing_y) {
            Some((_, foliage)) => TasteReading {
                cue: response(foliage),
                resistance: resistance_of(manifest, "foliage"),
                valid: true,
            },
            // The mouth is in air: no contact, no taste.
            None => TasteReading {
                cue: 0.0,
                resistance: 0.0,
                valid: false,
            },
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
/// non-finite input is rejected to zero rather than trained through. The modules P1-C
/// owns stay zero with validity 0.
pub(crate) fn observation(
    fauna: &Fauna,
    i: usize,
    view: &VoxelView<'_>,
    fv: &FloraView<'_>,
    manifest: &Manifest,
) -> Vec<f64> {
    let a = &fauna.animals[i];
    let founder = a
        .founder
        .expect("an observation is built for a founder body");
    let phys = fauna.config.founder(founder);
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
    let reading = contact_readings(view, &a.pose, a.site.y, manifest);
    let valid = f64::from(reading.resolved);
    obs[cm.offset] = reading.front;
    obs[cm.offset + 1] = reading.left;
    obs[cm.offset + 2] = reading.right;
    obs[cm.offset + 3] = reading.underside;
    obs[cm.offset + 4] = valid;
    obs[wm.offset] = reading.wet;
    obs[wm.offset + 1] = valid;
    let taste = taste_reading(fv, view, &a.pose, a.site.y, manifest, founder);
    obs[tm.offset] = taste.cue;
    obs[tm.offset + 1] = taste.resistance;
    obs[tm.offset + 2] = f64::from(taste.valid);

    // Everything after Taste belongs to P1-C: left at zero, validity included. The
    // taste probe shares M_EMIT with that field so the two will agree at one spot.
    obs
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
        Site {
            x,
            y: 2,
            z,
        }
    }

    fn pose_at(x_m: f64, z_m: f64, heading: f64) -> crate::Pose {
        crate::Pose {
            x: x_m,
            z: z_m,
            heading_rad: heading,
        }
    }

    const H: f64 = std::f64::consts::FRAC_PI_2; // heading east (+x)

    /// Full cruise over one controller period (five ticks of held action) is exactly
    /// the manifest's forward reference, and the pose follows the heading it was given
    /// — including a wrap across the seam, which is an ordinary move and not a turn.
    #[test]
    fn full_cruise_moves_the_reference_distance_along_the_heading() {
        let world = flat_world();
        let view = world.view();
        let manifest = Founder::Blind.manifest();
        let mut pose = pose_at(0.5, 0.5, H);
        let mut total = 0.0;
        for _ in 0..manifest.cadence_ticks() {
            let motion = resolve_motion(
                &view,
                &mut pose,
                2,
                &manifest,
                0.05,
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
                2,
                &manifest,
                0.05,
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
        let mut pose = pose_at(0.5, 0.5, 0.0);
        let mut total = 0.0;
        let mut paid = 0.0;
        for _ in 0..manifest.cadence_ticks() {
            let motion = resolve_motion(
                &view,
                &mut pose,
                2,
                &manifest,
                0.05,
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
        assert_eq!((pose.x, pose.z), (0.5, 0.5), "a stopped turn does not translate");
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
        // A one-voxel wall on column x = 4, standing proud at y = 3 (the body layer).
        world.apply(WorldCommand::SetMaterial {
            x: 4,
            y: 3,
            z: 3,
            material: Material::Soil,
        });
        let view = world.view();
        let manifest = Founder::Blind.manifest();
        // Heading east at z row 3, close enough to the wall that the period would
        // cross into it.
        let mut pose = pose_at(4.0 * 0.25 - 0.05, 3.0 * 0.25 + 0.125, H);
        let mut blocked = false;
        let mut attempted = 0.0;
        let mut delivered = 0.0;
        for _ in 0..manifest.cadence_ticks() {
            let motion = resolve_motion(
                &view,
                &mut pose,
                2,
                &manifest,
                0.05,
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
            pose.x + footprint_radius(&manifest) <= 4.0 * 0.25 + 1e-9,
            "the disc stopped at the wall's face, at x = {}",
            pose.x
        );

        // The wall is dead ahead: front contact, no side contact.
        let contacts = contact_readings(&view, &pose, 2, &manifest);
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
        let contacts = contact_readings(&view, &turned, 2, &manifest);
        assert_eq!(contacts.right, 1.0, "the wall the body faces is its right after a −90° turn");
        assert_eq!(contacts.front, 0.0);
    }

    /// No step to a higher support, and no walking off a drop: a founder's centre
    /// stays on its standing layer whatever the sweep asks for.
    #[test]
    fn a_drop_and_a_higher_face_constrain_the_centre() {
        let mut world = flat_world();
        // A step up: column x = 5 carries an extra layer, so its support face is at
        // y = 3 and its body layer would be 4 — a higher support the founder cannot
        // climb, and at y = 2 it has no support at all.
        world.apply(WorldCommand::SetMaterial {
            x: 5,
            y: 3,
            z: 2,
            material: Material::Soil,
        });
        let view = world.view();
        let manifest = Founder::Blind.manifest();
        assert!(view.is_support(5, 3, 2));
        assert!(!view.is_support(5, 2, 2));

        let mut pose = pose_at(5.0 * 0.25 - 0.06, 2.0 * 0.25 + 0.125, H);
        let mut blocked = false;
        for _ in 0..manifest.cadence_ticks() {
            let motion = resolve_motion(
                &view,
                &mut pose,
                2,
                &manifest,
                0.05,
                Actions {
                    forward: 1.0,
                    turn: 0.0,
                    feed: 0.0,
                },
            );
            blocked |= motion.blocked;
        }
        assert!(blocked, "the higher face is not steppable");
        let contacts = contact_readings(&view, &pose, 2, &manifest);
        assert_eq!(contacts.front, 1.0, "the step-up's wall is at the body layer: contact");
        assert_eq!(contacts.underside, 1.0);

        // A drop: dig column x = 3 at row 4 down to air, so its support face is at
        // y = 1 and there is nothing at y = 2 to stand on — a pit the body must not
        // walk into.
        world.apply(WorldCommand::SetMaterial {
            x: 3,
            y: 2,
            z: 4,
            material: Material::Air,
        });
        let view = world.view();
        assert!(!view.is_support(3, 2, 4));
        let mut pose = pose_at(3.0 * 0.25 - 0.025, 4.0 * 0.25 + 0.125, H);
        let mut blocked = false;
        for _ in 0..manifest.cadence_ticks() {
            let motion = resolve_motion(
                &view,
                &mut pose,
                2,
                &manifest,
                0.05,
                Actions {
                    forward: 1.0,
                    turn: 0.0,
                    feed: 0.0,
                },
            );
            blocked |= motion.blocked;
        }
        assert!(blocked, "the pit is not steppable either");
        let contacts = contact_readings(&view, &pose, 2, &manifest);
        assert_eq!(contacts.front, 0.0, "a drop is open air at body level: the delivery ratio is what reports it");
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
        assert!(view.water_depth_m(2, 2, 2) > 0.05, "the pond is {} deep", view.water_depth_m(2, 2, 2));
        let manifest = Founder::Blind.manifest();
        let mut pose = pose_at(2.0 * 0.25 - 0.01, 2.0 * 0.25 + 0.125, H);
        let mut blocked = false;
        for _ in 0..manifest.cadence_ticks() {
            let motion = resolve_motion(
                &view,
                &mut pose,
                2,
                &manifest,
                0.05,
                Actions {
                    forward: 1.0,
                    turn: 0.0,
                    feed: 0.0,
                },
            );
            blocked |= motion.blocked;
        }
        assert!(blocked);
        assert!(pose.x < 2.0 * 0.25, "the body never entered the pond column");
    }

    /// The blind founder's taste reads the litter its mouth region actually covers:
    /// underfoot litter gives a valid cue at the litter resistance, a bare patch a
    /// valid zero at the ground resistance, and a column beyond the reach is never
    /// input. A founder's mouth reach is well under a voxel, so only a body standing
    /// against the far edge of its column reaches into the next one.
    #[test]
    fn blind_taste_reads_only_the_mouth_region() {
        let world = flat_world();
        let view = world.view();
        let mut flora = Flora::new(FloraConfig::default());
        // Litter on the body's own site, and on a neighbour two columns east, beyond
        // the mouth reach of anything standing on the home site.
        for (x, z) in [(2u32, 2u32), (4, 2)] {
            flora.deposit(
                site(x, z),
                Deposit {
                    kind: DepositKind::Litter,
                    organic: 0.2,
                    mineral: 0.2 * 0.02,
                    energy: 0.2 * 2.0,
                },
            );
        }
        let fv = flora.view();
        let manifest = Founder::Blind.manifest();

        // Standing on the litter site: valid cue at the litter resistance, whatever
        // the heading — the mouth covers the footprint, and the footprint is on it.
        let pose = pose_at(2.0 * 0.25 + 0.125, 2.0 * 0.25 + 0.125, 0.0);
        let t = taste_reading(&fv, &view, &pose, 2, &manifest, Founder::Blind);
        assert!(t.valid);
        assert!(t.cue > 0.49, "a full tile reads at full cue strength, got {}", t.cue);
        assert!((t.resistance - 0.2).abs() < 1e-12, "the litter resistance");

        // Standing against the east edge of the bare column 3, facing east: the
        // mouth's reach just crosses into column 4, whose litter is real mouth input.
        let edge = pose_at(1.0 - 0.03, 2.0 * 0.25 + 0.125, H);
        let t = taste_reading(&fv, &view, &edge, 2, &manifest, Founder::Blind);
        assert!(t.valid && t.cue > 0.49, "the neighbour's litter is within the reach now");
        // The same body facing west: the reach covers bare column 3 only — the litter
        // two columns west is behind the mouth and never becomes input.
        let mut west = edge;
        west.heading_rad = -H;
        let t = taste_reading(&fv, &view, &west, 2, &manifest, Founder::Blind);
        assert!(t.valid, "contact with the ground itself is a valid reading");
        assert_eq!(t.cue, 0.0);
        assert!((t.resistance - 0.5).abs() < 1e-12, "the ground resistance, got {}", t.resistance);
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
        let wood = 0.5 * flora.config().species(cubarium_voxel_flora::Species::Springturf).wood_max;
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

        // Standing in the turf itself: contact.
        let pose = pose_at(2.0 * 0.25 + 0.125, 2.0 * 0.25 + 0.125, 0.0);
        let t = taste_reading(&fv, &view, &pose, 2, &manifest, Founder::Browser);
        assert!(t.valid, "the crown cell is at the body's own column and layer");
        assert!(t.cue > 0.0 && (t.resistance - 0.3).abs() < 1e-12);

        // Standing one column west facing east: the crown is within the mouth reach.
        let near = pose_at(1.0 * 0.25 + 0.125, 2.0 * 0.25 + 0.125, H);
        let t = taste_reading(&fv, &view, &near, 2, &manifest, Founder::Browser);
        assert!(t.valid, "the reach reaches the neighbouring crown cell");

        // Facing away, the crown is behind the mouth: no contact, invalid.
        let mut away = near;
        away.heading_rad = (H + std::f64::consts::PI).rem_euclid(std::f64::consts::TAU);
        let t = taste_reading(&fv, &view, &away, 2, &manifest, Founder::Browser);
        assert!(!t.valid, "a stock behind the mouth is unreachable and stays unmouthed");
        assert_eq!(t.cue, 0.0);
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
                body: 0.0125,
                heading_rad: H,
            },
        ));
        let flora = Flora::new(FloraConfig::default());
        let view = world.view();
        let fv = flora.view();
        let manifest = Founder::Blind.manifest();
        let obs = observation(&fauna, 0, &view, &fv, &manifest);
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
        assert_eq!(obs[15], 0.0, "no litter in the mouth");
        assert!((obs[16] - 0.5).abs() < 1e-12, "ground resistance");
        assert_eq!(obs[17], 1.0, "taste validity: the mouth contacts the ground");
        // Chem(litter) 18..21 and Light 21..23: P1-C's, zero with validity 0.
        assert!(obs[18..23].iter().all(|v| *v == 0.0));

        // The browser's observation is 37 wide and its Cone span is zero too.
        let mut browser = Fauna::new(crate::FaunaConfig::default());
        assert!(browser.apply(
            &world,
            crate::Command::IntroduceFounder {
                x: 2,
                z: 2,
                founder: Founder::Browser,
                body: 0.05,
                heading_rad: H,
            },
        ));
        let obs = observation(&browser, 0, &view, &fv, &Founder::Browser.manifest());
        assert_eq!(obs.len(), 37);
        assert!(obs[18..37].iter().all(|v| *v == 0.0), "Cone stays zero with validity 0");
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
        let total = FounderPhysiology::total_structure_for_core(core, blind.organ_structure_fraction);
        assert!((total - 0.0125).abs() < 1e-12);
        assert!((blind.sensor_structure(total) - 0.05 * total).abs() < 1e-15);
        // The rule does not create matter: sensor + core is the total, not more.
        assert!((blind.sensor_structure(total) + core - total).abs() < 1e-12);
    }
}
