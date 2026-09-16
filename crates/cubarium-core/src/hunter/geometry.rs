use cubarium_surface::Topology;
use serde::{Deserialize, Serialize};

use cubarium_surface::{ChartImage, SurfacePoint, Vec2, travel, unfold_with};

use crate::ids::OrganismId;
use crate::organism::Organism;

use super::*;

/// The **capture effector**: Fable's authored `Lanternjaw::effectors(1.0).near_claw`, the near
/// raptorial claw's painted centre at full extension, in adult body pixels.
///
/// Recomputed here from the same expressions in `crates/cubarium/src/lanternjaw.rs`
/// (`LIMB_STRIKE.1 = [12.3, 0.6]`, `CELL_CENTRE = 0.5`), read-only and never edited by this
/// crate:
///
/// ```text
/// head_dx = -0.3 · (1 - 13/17) + 1.1 · clamp((13 - 9)/8, 0, 1)     // the clamp is a no-op: 0.5
/// near_claw = (12.3 + head_dx + 0.5,  0.6 + 0.5) = (13.2794117647…, 1.1)
/// ```
///
/// The art is never shortened to fit a core constant; the constant follows the art. The `y`
/// moved by 0.062368 px against the profile's version 2 value — well inside the trial reach,
/// but a frozen experimental value, so [`PROFILE_VERSION`] moved with it.
pub const CAPTURE_OFFSET_BODY: Vec2 = {
    let head_dx = -0.3 * (1.0 - 13.0 / 17.0) + 1.1 * 0.5;
    Vec2::new(12.3 + head_dx + 0.5, 0.6 + 0.5)
};

/// The **ingestion mouth**: Fable's authored `effectors(1.0).mouth`, the outer jaw column's
/// centre at full lunge, `(8 + 1.1 + 0.5, 0)`. Folded it sits at `(8.5, 0)`; this is the
/// extended position, the one that exists at a settlement.
pub const INGESTION_OFFSET_BODY: Vec2 = Vec2::new(8.0 + 1.1 + 0.5, 0.0);

/// How exactly a transported grasp centre must round-trip back to its intended body
/// coordinate before it is published or allowed to capture. Transport rounding is far below
/// this; a different shortest image is far above it.
pub const GRASP_EPS: f64 = 1e-6;

/// Everything the settlement actually used, recorded from the **common post-movement state
/// before the prey is removed or any target is cleared**. The later render view cannot
/// recover it: the prey is gone and the hunter has moved on.
#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
pub struct ContactEvidence {
    pub prey: OrganismId,
    /// The prey's own post-movement position, before removal.
    pub prey_pos: SurfacePoint,
    pub prey_extent: f64,
    /// The hunter's post-movement root and heading, the ones the contact test used.
    pub hunter_pos: SurfacePoint,
    pub hunter_heading: Vec2,
    /// The scaled geometry the test used.
    pub geometry: ContactGeometry,
    /// The validated physical grasp centre, when one exists — never a reflected point.
    pub capture_center: Option<SurfacePoint>,
    /// The validated ingestion mouth, named separately from the grasp.
    pub ingestion_center: Option<SurfacePoint>,
    /// The measurement itself; `None` when the prey could not be unfolded from the root at all.
    pub measure: Option<ContactMeasure>,
}

impl ContactEvidence {
    /// Gather the evidence for this pairing. Pure, and safe to call before or instead of a
    /// capture: it never mutates anything.
    pub fn gather(
        topo: Topology,
        images: &[Vec<ChartImage>],
        profile: &FixedHunterProfile,
        hunter: &Organism,
        prey_id: OrganismId,
        prey: &Organism,
    ) -> ContactEvidence {
        let geometry = ContactGeometry::of(profile, hunter);
        ContactEvidence {
            prey: prey_id,
            prey_pos: prey.pos,
            prey_extent: prey.phenotype.extent,
            hunter_pos: hunter.pos,
            hunter_heading: hunter.heading,
            geometry,
            capture_center: body_point(
                topo,
                images,
                hunter.pos,
                hunter.heading,
                geometry.capture_offset_body,
            ),
            ingestion_center: body_point(
                topo,
                images,
                hunter.pos,
                hunter.heading,
                geometry.ingestion_offset_body,
            ),
            measure: measure_contact(
                topo,
                images,
                hunter.pos,
                hunter.heading,
                &geometry,
                prey.pos,
                prey.phenotype.extent,
            ),
        }
    }

    /// The bounded first policy: a capture needs the prey inside the scaled grasp **and** a
    /// grasp centre the renderer can actually draw there.
    pub fn grants_capture(&self) -> bool {
        self.capture_center.is_some() && self.measure.is_some_and(|m| m.in_contact())
    }
}

/// The surface distance from `from` to `to` through the shortest valid unfolding, or `None`
/// when the target is past `max_distance` or not reachable without leaving the surface.
///
/// This is the same local unfolding the pair pass and the controller use: it crosses seams,
/// never a face-local straight line that would miss one, and never the open rim.
pub fn surface_reach(
    topo: Topology,
    images: &[Vec<ChartImage>],
    from: SurfacePoint,
    to: SurfacePoint,
    max_distance: f64,
) -> Option<f64> {
    let max = max_distance.min(topo.max_local_radius());
    if max <= 0.0 {
        return None;
    }
    unfold_with(topo, &images[topo.chart_index(from.face)], from, to, max).map(|u| u.distance)
}

/// The body basis `stamp_rig` uses: `+x` along the heading, `+y` its clockwise side.
/// Returns `None` for a heading that is not a direction.
pub fn body_basis(heading: Vec2) -> Option<(Vec2, Vec2)> {
    let h = heading.normalized()?;
    Some((h, Vec2::new(-h.y, h.x)))
}

/// One member's contact geometry at one instant, in the renderer's own body basis. Core owns
/// this: the art adapter is handed the same numbers rather than deriving its own.
#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
pub struct ContactGeometry {
    /// `max(body_scale_min, (S / S_adult)^body_scale_exponent)`, the authoritative whole-rig
    /// scale. Everything below is already multiplied by it.
    pub scale: f64,
    /// Scaled body-local centre of the grasp, `(forward, side)`.
    pub capture_offset_body: Vec2,
    /// Scaled contact tolerance around that centre.
    pub capture_reach_px: f64,
    /// Scaled body-local ingestion mouth, named separately from the capture effector.
    pub ingestion_offset_body: Vec2,
    /// Scaled query support the renderer needs around the root.
    pub visual_query_extent_px: f64,
}

impl ContactGeometry {
    /// The geometry of this member right now: the profile's measured offsets scaled by the
    /// body scale its actual structure implies.
    pub fn of(profile: &FixedHunterProfile, organism: &Organism) -> ContactGeometry {
        let scale = body_scale(
            profile,
            organism.structure,
            organism.phenotype.structure_adult,
        );
        ContactGeometry {
            scale,
            capture_offset_body: profile.capture_offset_body * scale,
            capture_reach_px: profile.capture_reach_px * scale,
            ingestion_offset_body: profile.ingestion_offset_body * scale,
            visual_query_extent_px: profile.visual_query_extent_px * scale,
        }
    }

    /// How far the local unfolding has to reach to decide contact with a body of `extent`,
    /// never past the topology's own query radius.
    fn window(&self, topo: Topology, extent: f64) -> f64 {
        (self.capture_offset_body.length() + self.capture_reach_px + extent + 1.0)
            .min(topo.max_local_radius())
    }
}

/// The whole-rig scale of a member: `max(min, (S / S_adult)^exponent)`, the mapping the
/// profile selected. A juvenile is the same rig, every part in the same relative place,
/// `scale` times smaller — which is exactly what `stamp_rig_scaled` does with the same number.
pub fn body_scale(profile: &FixedHunterProfile, structure: f64, structure_adult: f64) -> f64 {
    if !structure_adult.is_finite()
        || structure_adult <= 0.0
        || !structure.is_finite()
        || structure <= 0.0
    {
        return profile.body_scale_min;
    }
    let ratio = (structure / structure_adult).clamp(0.0, 1.0);
    let scale = ratio.powf(profile.body_scale_exponent);
    if scale.is_finite() {
        scale.max(profile.body_scale_min)
    } else {
        profile.body_scale_min
    }
}

/// Where a prey actually sits relative to the claws, measured the way the renderer draws them.
#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
pub struct ContactMeasure {
    /// The prey's position in the hunter's body basis, unfolded from the hunter **root**.
    pub body: Vec2,
    /// Surface distance from the root to the prey.
    pub root_distance: f64,
    /// Distance from the scaled capture effector centre to the prey.
    pub effector_distance: f64,
    /// `capture_reach_px · scale + prey extent`: what `effector_distance` must not exceed.
    pub tolerance: f64,
}

impl ContactMeasure {
    pub fn in_contact(&self) -> bool {
        self.effector_distance <= self.tolerance
    }
}

/// Which rule the pursuit's stopping predicate runs under.
///
/// The hunt-intent pass in `crate::world::step` holds a member still — at its `rest_effort`,
/// with no strike burst pushed — for a prey the predicate calls "inside". The two variants are
/// the two readings the source has always contained: what the line tested, and what its own
/// comment said.
///
/// **The shipped rule is [`PursuitStop::ReachEnvelope`] from 2026-09-16.** The rule before
/// that date was the forward half-space, which
/// `design/7_Research/ecology-v1-apex-reach-2026-09-16.md` measured true at the burst's start
/// on 408 of 449 paid attempts — dropping the member to `rest_effort` and suppressing the
/// burst it had just paid for — and which the paired intervention in
/// `design/7_Research/ecology-v1-apex-predicate-2026-09-16.md` then corrected: held at the
/// burst's start 89.4 % → 5.4 %, contacts 88 → 140, captures 38 → 67 over 32 lives. It stays
/// reachable as an opt-in so the rows retained under it remain reproducible.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PursuitStop {
    /// **The rule before 2026-09-16**: `body.x < capture_offset_body.x · scale + tolerance`, a
    /// one-sided **forward half-space**. A prey short of the claws satisfies it as readily as
    /// one inside them, so an apex whose 12 px sense radius is shorter than its own 13.28 px
    /// grasp held for essentially every prey it could hunt at all. No longer the default: opt
    /// in per `World` (`crate::World::set_pursuit_stop`) to reproduce a retained row.
    ForwardHalfSpace,
    /// **The shipped rule**: [`ContactMeasure::in_contact`], the **reach envelope** the
    /// predicate's own comment names and the test the rest of the file means by "inside". This
    /// is the default and what an ordinary world runs without being told.
    #[default]
    ReachEnvelope,
}

impl PursuitStop {
    pub fn as_str(self) -> &'static str {
        match self {
            PursuitStop::ForwardHalfSpace => "forward_half_space",
            PursuitStop::ReachEnvelope => "reach_envelope",
        }
    }
}

impl ContactMeasure {
    /// Does the pursuit's stopping rule hold a member still against this measure?
    ///
    /// **The one place either rule is written.** `crate::world::step`'s hunt-intent pass, the
    /// per-attempt record's own reading ([`crate::hunter::StrikeFrame::pursuit_holds`]) and the
    /// tests all evaluate it here, so a record can never transcribe a rule the world did not
    /// run. `capture_forward` is the scaled `capture_offset_body.x` of the member asking, i.e.
    /// [`ContactGeometry::capture_offset_body`]`.x`.
    ///
    /// The shipped rule is [`PursuitStop::ReachEnvelope`], so this call is
    /// [`ContactMeasure::in_contact`] in an ordinary world; the half-space arm is the one that
    /// has to be asked for.
    pub fn pursuit_holds(&self, stop: PursuitStop, capture_forward: f64) -> bool {
        match stop {
            PursuitStop::ForwardHalfSpace => self.body.x < capture_forward + self.tolerance,
            PursuitStop::ReachEnvelope => self.in_contact(),
        }
    }
}

impl ContactGeometry {
    /// [`ContactMeasure::pursuit_holds`] at this member's own scaled forward grasp coordinate.
    pub fn pursuit_holds(&self, stop: PursuitStop, measure: &ContactMeasure) -> bool {
        measure.pursuit_holds(stop, self.capture_offset_body.x)
    }
}

/// Measure a prey against a hunter's claws **from the hunter root**, with the same shortest
/// image the rig is drawn through, then rotated into the same body basis.
///
/// This is the contact authority. A separately transported mouth point has its own chart and
/// can pick a different shortest image at a vertex, so distance from *that* chart is not proof
/// of contact with the drawn claw — which is why the old `mouth_point` helper is gone.
/// `None` means the prey is not reachable inside the local unfolding at all.
pub fn measure_contact(
    topo: Topology,
    images: &[Vec<ChartImage>],
    root: SurfacePoint,
    heading: Vec2,
    geometry: &ContactGeometry,
    prey: SurfacePoint,
    prey_extent: f64,
) -> Option<ContactMeasure> {
    let (forward, side) = body_basis(heading)?;
    let window = geometry.window(topo, prey_extent);
    let u = unfold_with(topo, &images[topo.chart_index(root.face)], root, prey, window)?;
    let delta = u.local - root.chart();
    let body = Vec2::new(forward.dot(delta), side.dot(delta));
    Some(ContactMeasure {
        body,
        root_distance: u.distance,
        effector_distance: (body - geometry.capture_offset_body).length(),
        tolerance: geometry.capture_reach_px + prey_extent,
    })
}

/// The physical surface point of a scaled body-local offset, **or `None`**.
///
/// `None` whenever the point is not honestly on the surface where the artwork draws it: the
/// sweep reflected off the open rim (static art clips there — only actual root travel
/// reflects), hit the forward-progress fallback, resolved a vertex tie, or does not round-trip
/// through the root-owned unfolding back to the body coordinate it was built from. A
/// fabricated reflected point is never published and never captures.
pub fn body_point(
    topo: Topology,
    images: &[Vec<ChartImage>],
    root: SurfacePoint,
    heading: Vec2,
    offset_body: Vec2,
) -> Option<SurfacePoint> {
    let (forward, side) = body_basis(heading)?;
    if !offset_body.is_finite() {
        return None;
    }
    let chart_offset = forward * offset_body.x + side * offset_body.y;
    let reach = chart_offset.length();
    if reach <= GRASP_EPS {
        return Some(root);
    }
    if reach + 1.0 >= topo.max_local_radius() {
        return None;
    }
    let swept = travel(topo, root, chart_offset);
    if swept.reflections > 0 || swept.fallback || swept.ties > 0 {
        return None;
    }
    // The round trip: the root's own shortest image of that point must be the body coordinate
    // it was built from, or the renderer and the world disagree about where the claw is.
    let u = unfold_with(topo, &images[topo.chart_index(root.face)], root, swept.end, reach + 1.0)?;
    let delta = u.local - root.chart();
    let back = Vec2::new(forward.dot(delta), side.dot(delta));
    if (back - offset_body).length() > GRASP_EPS {
        return None;
    }
    Some(swept.end)
}
