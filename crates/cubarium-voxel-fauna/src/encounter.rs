//! The physical encounter query: what a body at a pose on a support face can stand on,
//! reach with its mouth and see.
//!
//! Read-only. Nothing here runs in a tick; every function is the *same rule* the live
//! path uses, exposed so a diagnostic can ask it of a face nobody is standing on and of
//! a body size nobody has yet. The contract these helpers serve is
//! `design/voxel-encounter-contract-2026-09-21.md`; the decisions they are measured
//! against are `design/handoffs/voxel-organism-decisions-2026-09-21.md`.
//!
//! Two things are deliberately *not* re-derived here:
//!
//! - **Mouth acceptance** is [`crate::body::foliage_stands_touching`], the scan behind
//!   the live [`crate::body::mouth_foliage_stand`]: a foliage-bearing stand whose one
//!   crown disc of cells sits at an accepted layer and covers a mouth column. Only the
//!   layer range is handed in, because the crown is one cell thick and any band over the
//!   standing surface therefore selects a contiguous run of layers.
//! - **Where a body may stand** is the predicate the seeder's `browser_faces` and the
//!   tick's `faces_in_column` both use: a support face, wadeable water, and
//!   [`Body::headroom_voxels`] voxels of void over it — the body's own height.
//!
//! A founder body changes its standing layer by at most its lineage's step up or down
//! ([`crate::FounderPhysiology::step_limits`]; `design/handoffs/voxel-founder-step-2026-09-22.md`),
//! or — a shredder — by climbing a terrain wall, so the walkable
//! neighbourhood of a founder is what [`route_components`] computes, from the one shared
//! rule in `cubarium_voxel::walk` plus the wall rule, for this crate, the seeder and the
//! observer alike.

use rustc_hash::FxHashMap;
use std::ops::RangeInclusive;

use cubarium_voxel::VoxelView;
use cubarium_voxel_flora::{FloraView, Site, Stand};

use crate::body::{Body, has_headroom, mouth_columns, mouth_crown_layers};
use crate::senses::{self, ConeOccupancy};
use crate::{ConeHit, Diet, FaunaView, Pose};

/// How many headings a face is asked about when the question is "from *some* pose here".
///
/// The mouth region's furthest probe is `2 · footprint_radius + reach` from the body's
/// centre — 0.5625 m for the adult browser, 0.22 m for the shredder — so a
/// five-degree turn moves it by at most 0.05 m, under half a 0.125 m voxel. Seventy-two
/// headings therefore enumerate the union of mouth columns a body standing on one face
/// could cover, short of a column the region only clips at a corner for less than five
/// degrees; the acceptance rule is a pure OR over headings, so the union may be tested
/// in one call.
pub const HEADING_SAMPLES: usize = 72;

/// The standing surface of a support face, in metres: `Site.y` indexes the **solid**
/// support cell, so the surface a body stands on is its top
/// (`crates/cubarium-voxel/src/world.rs`, `is_support`; audit §1).
pub fn surface_m(standing_y: u32, voxel_m: f64) -> f64 {
    (f64::from(standing_y) + 1.0) * voxel_m
}

/// Every face a body of these dimensions could stand on, in `(y, x, z)` order.
///
/// The predicate is the seeder's and the tick's, not a new one: a support face, no more
/// than `wade_depth_m` of standing water on it, and [`Body::headroom_voxels`] voxels of
/// void over it (`crates/cubarium/src/voxel/habitat.rs`, `browser_faces`;
/// `crates/cubarium-voxel-fauna/src/step.rs`, `faces_in_column`).
pub fn standable_faces(view: &VoxelView<'_>, body: &Body, wade_depth_m: f64) -> Vec<Site> {
    let c = view.config;
    let room = body.headroom_voxels(c.voxel_m);
    let mut out = Vec::new();
    for y in 0..c.height {
        for x in 0..c.width {
            for z in 0..c.depth {
                let xi = i64::from(x);
                if !view.is_support(xi, y, z) {
                    continue;
                }
                if view.water_depth_m(xi, y, z) > wade_depth_m {
                    continue;
                }
                if !has_headroom(view, xi, y, z, room) {
                    continue;
                }
                out.push(Site { x, y, z });
            }
        }
    }
    out
}

/// The mouth columns of one pose: [`crate::body::mouth_columns`], exposed.
pub fn mouth_columns_at(view: &VoxelView<'_>, pose: &Pose, body: &Body) -> Vec<(i64, u32)> {
    mouth_columns(view, pose, body)
}

/// The union of mouth columns over every heading a body standing on `face` could take.
///
/// The body is placed where the model places one on a face, [`Pose::at_site`], and the
/// heading swept in [`HEADING_SAMPLES`] equal steps.
pub fn mouth_columns_from_face(view: &VoxelView<'_>, face: Site, body: &Body) -> Vec<(i64, u32)> {
    let v = view.config.voxel_m;
    let width = i64::from(view.config.width);
    // Every mouth column lies within `2r + reach` of the face (the tip's far corner), so
    // the union is deduplicated on a small window around the face rather than by a scan
    // of the list so far (which was 70 % of a wide world's founding check). Same columns,
    // same first-seen order.
    let half = ((2.0 * body.footprint_radius() + body.mouth_reach_m) / v).ceil() as i64 + 2;
    let side = (2 * half + 1) as usize;
    let mut seen = vec![false; side * side];
    let (cx, cz) = (i64::from(face.x), i64::from(face.z));
    let mut cols: Vec<(i64, u32)> = Vec::new();
    for k in 0..HEADING_SAMPLES {
        let heading = std::f64::consts::TAU * (k as f64) / (HEADING_SAMPLES as f64);
        let mut pose = Pose::at_site(face, v);
        pose.heading_rad = heading;
        crate::body::mouth_columns_each(view, &pose, body, |entry| {
            let mut dx = crate::body::wrap_x(entry.0 - cx, width);
            if dx > width / 2 {
                dx -= width;
            }
            let dz = i64::from(entry.1) - cz;
            if dx.abs() <= half && dz.abs() <= half {
                let at = (dz + half) as usize * side + (dx + half) as usize;
                if !seen[at] {
                    seen[at] = true;
                    cols.push(entry);
                }
            } else if !cols.contains(&entry) {
                cols.push(entry);
            }
        });
    }
    cols
}

/// The crown layers the implemented mouth accepts from `standing_y`:
/// [`crate::body::mouth_crown_layers`], exposed. Since 2026-09-22 this **is** the metre
/// band of [`band_crown_layers`], so the two no longer name different arms.
pub fn mouth_crown_layers_at(standing_y: u32, body: &Body, voxel_m: f64) -> RangeInclusive<i64> {
    mouth_crown_layers(standing_y, body, voxel_m)
}

/// The crown layers a **metre band** `[0, ceiling_m]` over the standing surface reaches:
/// [`band_layers`] with no floor.
pub fn band_crown_layers(standing_y: u32, voxel_m: f64, ceiling_m: f64) -> RangeInclusive<i64> {
    band_layers(standing_y, voxel_m, 0.0, ceiling_m)
}

/// The crown layers a **metre band** `[floor_m, ceiling_m]` over the standing surface
/// reaches.
///
/// A crown is one cell thick at layer `L`, so its physical slab is
/// `[L·v, (L+1)·v]`; the band selects every `L` whose slab overlaps it by a positive
/// amount. A touch at exactly the floor or the ceiling is not an overlap (audit §2: "a
/// boundary touch alone contributes no stock"). A band that reaches nothing comes back
/// empty.
pub fn band_layers(
    standing_y: u32,
    voxel_m: f64,
    floor_m: f64,
    ceiling_m: f64,
) -> RangeInclusive<i64> {
    let base = i64::from(standing_y) + 1;
    let empty = base..=(base - 1);
    let q = ceiling_m / voxel_m;
    if !q.is_finite() || q <= 0.0 {
        return empty;
    }
    // The lowest slab `[k·v, (k+1)·v]` reaching above the floor: `k = floor(floor / v)`.
    let f = (floor_m / voxel_m).max(0.0);
    if !f.is_finite() {
        return empty;
    }
    let lo = base + f.floor() as i64;
    let hi = base + (q.ceil() as i64) - 1;
    if hi < lo { empty } else { lo..=hi }
}

/// Every foliage-bearing stand whose crown cells sit at a layer in `layers`, covers one
/// of `cols`, and whose tissue `diet` accepts — the live mouth's own scan, with the
/// layer range and the diet handed in.
///
/// An observer measuring what physically stands in a band asks with [`Diet::ANY`]; a
/// mouth asks with its own ([`Diet::of`]), because since decisions §3 reach and
/// permission are different questions and a glowcap cap answers them differently.
pub fn foliage_stands_in_layers(
    fv: &FloraView<'_>,
    view: &VoxelView<'_>,
    cols: &[(i64, u32)],
    layers: &RangeInclusive<i64>,
    diet: Diet,
) -> Vec<(Site, f64)> {
    crate::body::foliage_stands_touching(fv, view, cols, layers, diet)
}

/// What a mouth over `cols` whose band selects `layers` can take from one stand,
/// **per foliage layer**: `(index among the stand's foliage layers, stock)`, bottom-up.
///
/// This is what closes package 0's "could not measure 1" (within-stand shares): an
/// observer can now say that a browser reaches an adult bloomcrown's rosette and not
/// its crown, rather than that it reaches the stand or does not
/// (`design/handoffs/voxel-edible-stock-2026-09-21.md`, integration note).
pub fn reachable_layers_of(
    fv: &FloraView<'_>,
    view: &VoxelView<'_>,
    stand: &Stand,
    cols: &[(i64, u32)],
    layers: &RangeInclusive<i64>,
) -> Vec<(usize, f64)> {
    crate::body::reachable_layers(fv, view, stand, cols, layers)
}

/// The `(x, z)` columns one **layer** covers, by the same disc rule the mouth and the
/// cone both use — its own radius, which is a fraction of the crown's.
pub fn layer_columns(
    view: &VoxelView<'_>,
    stand: &Stand,
    layer: &cubarium_voxel_flora::StandLayer,
) -> Vec<(u32, u32)> {
    let c = view.config;
    let span = layer.radius_v.floor() as i64;
    let r2 = layer.radius_v * layer.radius_v;
    let mut out = Vec::new();
    for dz in -span..=span {
        for dx in -span..=span {
            if (dx * dx + dz * dz) as f64 > r2 {
                continue;
            }
            let z = i64::from(stand.site.z) + dz;
            if z < 0 || z >= i64::from(c.depth) {
                continue;
            }
            let x = (i64::from(stand.site.x) + dx).rem_euclid(i64::from(c.width));
            out.push((x as u32, z as u32));
        }
    }
    out
}

/// The layer a stand's crown disc of cells sits at: `site.y + crown_voxels(wood)`.
///
/// **Since plants have layers this is the stand's *topmost* foliage cell and not the
/// whole of where its foliage is** — an adult bloomcrown also holds a basal rosette
/// three cells below it. Kept because it is the pre-layer geometry the "Edible stock,
/// 2026-09-21" measurements were taken with; a caller that wants the whole plant asks
/// [`FloraView::layers`] or [`reachable_layers_of`]
/// (`design/voxel-encounter-contract-2026-09-21.md` §8).
pub fn crown_layer(fv: &FloraView<'_>, stand: &Stand) -> i64 {
    i64::from(stand.site.y)
        + i64::from(
            fv.config
                .species(stand.species)
                .crown_voxels(stand.wood, fv.config.voxel_m),
        )
}

/// The stand's foliage slab in **metres above the world floor**: the one cell-thick disc
/// the model actually holds, `[L·v, (L+1)·v]`.
pub fn crown_slab_m(fv: &FloraView<'_>, stand: &Stand, voxel_m: f64) -> (f64, f64) {
    let l = crown_layer(fv, stand) as f64;
    (l * voxel_m, (l + 1.0) * voxel_m)
}

/// The `(x, z)` columns a stand's crown disc covers, by the same disc rule the mouth and
/// the cone both use. **Pre-layer geometry**, as [`crown_layer`]: one layer's own
/// columns are [`layer_columns`].
pub fn crown_columns(fv: &FloraView<'_>, view: &VoxelView<'_>, stand: &Stand) -> Vec<(u32, u32)> {
    let c = view.config;
    let sc = fv.config.species(stand.species);
    let radius = sc.crown_radius(stand.wood, fv.config.voxel_m).max(0.0);
    let span = radius.floor() as i64;
    let r2 = radius * radius;
    let mut out = Vec::new();
    for dz in -span..=span {
        for dx in -span..=span {
            if (dx * dx + dz * dz) as f64 > r2 {
                continue;
            }
            let z = i64::from(stand.site.z) + dz;
            if z < 0 || z >= i64::from(c.depth) {
                continue;
            }
            let x = (i64::from(stand.site.x) + dx).rem_euclid(i64::from(c.width));
            out.push((x as u32, z as u32));
        }
    }
    out
}

/// Where the eye is, in metres: the body's column and `0.8 × height` over the standing
/// surface ([`crate::senses::cone_origin`], exposed).
pub fn eye_origin_m(
    view: &VoxelView<'_>,
    pose: &Pose,
    standing_y: u32,
    body: &Body,
) -> (f64, f64, f64) {
    senses::cone_origin(view, pose, standing_y, body)
}

/// An eye a stated height in **metres above the standing surface**, for a diagnostic
/// measuring an arm the model does not run. The live eye is [`eye_origin_m`].
pub fn eye_above_surface_m(
    pose: &Pose,
    standing_y: u32,
    voxel_m: f64,
    height_m: f64,
) -> (f64, f64, f64) {
    (pose.x, surface_m(standing_y, voxel_m) + height_m, pose.z)
}

/// One ray's unit direction from a heading, a yaw and a pitch in degrees
/// ([`crate::senses::ray_direction`], exposed).
pub fn ray_direction_deg(heading_rad: f64, yaw_deg: f64, pitch_deg: f64) -> (f64, f64, f64) {
    senses::ray_direction(heading_rad, yaw_deg, 0.0, pitch_deg)
}

/// The occlusion map a cone marches against, built once and marched many times.
pub struct SightMap {
    occupancy: ConeOccupancy,
}

impl SightMap {
    /// Build it. `pools_occlude` is `true` for the live rule — since contract v2 a ground
    /// pool blocks a ray only below its physical height (D3) — and `false` for a
    /// diagnostic arm with no pools at all.
    pub fn new(
        view: &VoxelView<'_>,
        fv: &FloraView<'_>,
        av: &FaunaView<'_>,
        pools_occlude: bool,
    ) -> SightMap {
        SightMap {
            occupancy: senses::cone_occupancy_with(view, fv, av, pools_occlude),
        }
    }

    /// March one ray and name its first hit, with the cell it struck. The march — the
    /// cell-exact traversal and the order of the occlusion tests — is the live cone's.
    pub fn first_hit(
        &self,
        view: &VoxelView<'_>,
        observer_id: u64,
        origin_m: (f64, f64, f64),
        dir: (f64, f64, f64),
        range_m: f64,
    ) -> Option<(f64, ConeHit, usize)> {
        senses::ray_first_hit_cell(view, &self.occupancy, observer_id, origin_m, dir, range_m)
            .map(|(distance, fine, cell)| (distance, crate::cone_hit_of(fine), cell))
    }
}

/// The connected components of a set of standing faces under a **symmetric step**.
///
/// One function, one rule: `cubarium_voxel::walk::components`, with `climb` a step in
/// whole voxels. A founder lineage's own components are [`route_components`]' (its step
/// up and down, and its walls). Face-neighbouring
/// columns join when their standing layers differ by at most that, `x` wrapping and the
/// strip's `z` ends walls. The observer (`voxel_edible_stock`), the seeder's
/// feeding-face adjacency (`crates/cubarium/src/voxel/habitat.rs`, `browser_faces`) and
/// the generator's ring gate all go through it.
///
/// Returns a component id per face, in the order of `faces`.
pub fn walkable_components(faces: &[Site], width: u32, climb: u32) -> Vec<usize> {
    let cells: Vec<(u32, u32, u32)> = faces.iter().map(|f| (f.x, f.y, f.z)).collect();
    cubarium_voxel::walk::components(&cells, width, climb)
}

/// How a lineage's standable faces join into walkable components.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RouteRule {
    /// Its step up and down, whole voxels.
    pub step: crate::StepLimits,
    /// Whether it goes up and down terrain walls.
    pub climbs_walls: bool,
}

/// The walkable components of `faces` under a lineage's [`RouteRule`], in the order of
/// `faces`.
///
/// **Undirected, by mutual reachability**: a component is a set of faces every one of
/// which a body can walk to from every other, so two neighbouring faces join only when
/// the rise between them is within both the step up and the step down
/// ([`crate::StepLimits::mutual`]) — a browser's 0.5 m riser, which it can step down but
/// not up, is two components. A climber's wall joins its foot to its top when
/// [`crate::wall_ascent`] says the face can be climbed from the foot; the wall rule's
/// descent is its exact mirror, so the edge goes both ways. The column rule alone: the live
/// body also checks its disc along the face, which a route map does not.
pub fn route_components(
    view: &VoxelView<'_>,
    faces: &[Site],
    rule: RouteRule,
    headroom: u32,
) -> Vec<usize> {
    let c = view.config;
    let cells: Vec<(u32, u32, u32)> = faces.iter().map(|f| (f.x, f.y, f.z)).collect();
    let mut links: Vec<(usize, usize)> = Vec::new();
    if rule.climbs_walls {
        let index: FxHashMap<Site, usize> =
            faces.iter().enumerate().map(|(i, f)| (*f, i)).collect();
        for (i, f) in faces.iter().enumerate() {
            for dir in [(1i64, 0i64), (-1, 0), (0, 1), (0, -1)] {
                let crate::Face::Climb(top) =
                    crate::wall_ascent(view, i64::from(f.x), f.y, f.z, dir, headroom, rule.step.up)
                else {
                    continue;
                };
                let to = Site {
                    x: (i64::from(f.x) + dir.0).rem_euclid(i64::from(c.width)) as u32,
                    y: top,
                    z: (i64::from(f.z) + dir.1) as u32,
                };
                if let Some(&j) = index.get(&to) {
                    links.push((i, j));
                }
            }
        }
    }
    cubarium_voxel::walk::components_linked(&cells, c.width, rule.step.mutual(), &links)
}

/// A founder lineage's **walking route graph** (bellwing3 item 1; on the disc rule since
/// bellwing4): the graph [`founding_faces`] builds for its adult body — a face is a node
/// when the disc (its footprint plus `margin_m`) stands clear at the face's centre, and two
/// neighbouring nodes within the mutual step are joined when the disc is also clear at the
/// midpoint, standing on either layer; a climber's walls join as in [`route_components`] —
/// with its edges kept so a route can be walked: route distances
/// ([`RouteGraph::distances`]), the next node toward a goal ([`RouteGraph::next_toward`])
/// and whether a straight line is walkable ([`RouteGraph::walk_line`]). The column rule
/// let a 1.4 m chorister's route run up stairs and gaps its body does not fit.
/// **DROP**: a trapped body's jump down ([`RouteGraph::drop_way`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DropWay {
    /// The pocket face it jumps from.
    pub from: Site,
    /// The face it lands on.
    pub land: Site,
    /// The column it jumps out over, next to `from` (on `from`'s layer).
    pub edge: Site,
    /// The first face of its walk to `from` (`from` itself when it stands there).
    pub first: Site,
    /// Where in `land`'s column it comes down ([`DROP_LANDINGS`], [`DropWay::landing`]).
    pub at: u8,
}

/// **SP-CRAVE: a jump into water** ([`RouteGraph::drop_way_into`]): the highest drop to the
/// water's surface it takes, metres, and whether a body landing on a face under water deeper
/// than it wades gets out of it ([`crate::RouteGraph::wade_way`], the wade-out's way).
pub struct WaterDrop<'a> {
    pub reach_m: f64,
    pub out: &'a dyn Fn(Site) -> bool,
}

/// SP-STUCK-2: the jumps after the first a trapped body's way down may take
/// ([`RouteGraph::drop_way`]): one, a jump in two. A cost bound, not a knob.
pub const DROP_HOPS: u32 = 1;

/// SP-STUCK: the points of its landing column a jump down may come down on, as offsets from
/// the column's centre to its sides (×½ voxel, less a hair): the centre, then each side, then
/// each corner. On a terraced slope every face is a voxel deep with a riser behind it, and a
/// disc wider than half a voxel centred on one stands in that riser; it lands pressed against
/// it instead, as a body walking up to a wall stands.
pub const DROP_LANDINGS: [(i8, i8); 9] = [(0, 0), (1, 0), (-1, 0), (0, 1), (0, -1), (1, 1), (1, -1), (-1, 1), (-1, -1)];

/// How deep in solid a landing point may leave the disc ([`DROP_LANDINGS`]), metres: the hair
/// it stops short of the column's side by, a float tolerance (the motor's own escape lets a
/// body step out of a wall it is caught in, no deeper), not a knob.
const LANDING_TOLERANCE_M: f64 = 1e-6;

/// The point in `land`'s column of [`DROP_LANDINGS`]`[at]`, at `voxel_m` on a ring `width`
/// columns round.
pub fn drop_landing(land: Site, at: u8, voxel_m: f64, width: u32) -> (f64, f64) {
    let (ox, oz) = DROP_LANDINGS.get(usize::from(at)).copied().unwrap_or((0, 0));
    let s = 0.5 * voxel_m - 1e-9;
    let x = (f64::from(land.x) + 0.5) * voxel_m + f64::from(ox) * s;
    (x.rem_euclid(f64::from(width) * voxel_m), (f64::from(land.z) + 0.5) * voxel_m + f64::from(oz) * s)
}

impl DropWay {
    /// The point it comes down on ([`drop_landing`]).
    pub fn landing(&self, voxel_m: f64, width: u32) -> (f64, f64) {
        drop_landing(self.land, self.at, voxel_m, width)
    }
}

#[derive(Debug)]
pub struct RouteGraph {
    /// The nodes: faces the disc stands clear on.
    pub faces: Vec<Site>,
    /// The walkable component of each node.
    pub components: Vec<usize>,
    index: FxHashMap<Site, usize>,
    /// Each node's neighbours (the step edges the disc passes, and a climber's walls).
    adj: Vec<Vec<usize>>,
    /// ROUTE-WALK: the edges a straight line may cross ([`RouteGraph::walk_line`]), when
    /// narrower than `adj`: between nodes where the disc grown by the line margin also
    /// stands clear, with that disc clear at the midpoint too. `None`: every edge.
    lines: Option<Vec<Vec<usize>>>,
    /// The node count of each walkable component, by its id (`components`' values).
    sizes: FxHashMap<usize, usize>,
    climb: u32,
    width: u32,
    depth: u32,
    voxel_m: f64,
}

impl RouteGraph {
    /// The route graph of a founder lineage's adult body, its disc grown by `margin_m`
    /// ([`crate::FoundingConfig::clearance_margin_m`], as the founding's): the founding's
    /// and the habitat verdict's graph, nodes, edges and lines alike on the grown disc.
    pub fn for_founder(view: &VoxelView<'_>, phys: &crate::FounderPhysiology, margin_m: f64) -> RouteGraph {
        Self::build(view, phys, margin_m, margin_m)
    }

    /// **ROUTE-WALK, the walking graph** the live animals route on (pack routes, the patrol,
    /// hunting; [`crate::FaunaConfig::route_clearance_m`],
    /// [`crate::FaunaConfig::route_line_clearance_m`]): nodes and edges where the adult's
    /// disc grown by `clearance_m` stands and steps clear, as [`RouteGraph::for_founder`]
    /// judges them; straight lines ([`RouteGraph::walk_line`]) only over the nodes and
    /// edges where the disc grown by `line_clearance_m` does too. The graph on the
    /// founding's margin left out ground the motor walks: in seed 9 of the Veilhound clone
    /// world a strip a voxel deep between the depth edge and a wall, 2.5 cm wider than the
    /// adult's disc and walked both ways, had no node, cutting the den off from the basin
    /// its hounds rested in.
    pub fn for_walking(
        view: &VoxelView<'_>,
        phys: &crate::FounderPhysiology,
        clearance_m: f64,
        line_clearance_m: f64,
    ) -> RouteGraph {
        Self::build(view, phys, clearance_m, line_clearance_m)
    }

    fn build(view: &VoxelView<'_>, phys: &crate::FounderPhysiology, margin_m: f64, line_margin_m: f64) -> RouteGraph {
        let c = view.config;
        let v = c.voxel_m;
        let body = phys.adult_body();
        let step = phys.step_limits(v);
        let room = body.headroom_voxels(v);
        let margin_m = margin_m.max(0.0);
        let r = body.footprint_radius() + margin_m;
        let clear = |x: f64, z: f64, y: u32| crate::body::pose_clear(view, x, z, y, r, room, step);
        // The lines' disc, when wider than the nodes' (else the lines are every edge).
        let r_line = (line_margin_m > margin_m).then(|| body.footprint_radius() + line_margin_m);
        let clear_line = |x: f64, z: f64, y: u32| r_line.is_none_or(|rl| crate::body::pose_clear(view, x, z, y, rl, room, step));
        let wade = phys.wade_depth_m(&body);
        let faces: Vec<Site> = standable_faces(view, &body, wade)
            .into_iter()
            .filter(|f| {
                let z = (f64::from(f.z) + 0.5) * v;
                z + 1e-9 >= body.footprint_radius() && z <= f64::from(c.depth) * v - body.footprint_radius() + 1e-9
            })
            .filter(|f| clear((f64::from(f.x) + 0.5) * v, (f64::from(f.z) + 0.5) * v, f.y))
            .collect();
        let index: FxHashMap<Site, usize> = faces.iter().enumerate().map(|(i, f)| (*f, i)).collect();
        let mut adj: Vec<Vec<usize>> = vec![Vec::new(); faces.len()];
        let wide: Vec<bool> = match r_line {
            Some(_) => faces.iter().map(|f| clear_line((f64::from(f.x) + 0.5) * v, (f64::from(f.z) + 0.5) * v, f.y)).collect(),
            None => Vec::new(),
        };
        let mut lines: Option<Vec<Vec<usize>>> = r_line.map(|_| vec![Vec::new(); faces.len()]);
        let climb = step.mutual();
        for (i, f) in faces.iter().enumerate() {
            for (dx, dz) in [(1i64, 0i64), (0, 1)] {
                let nz = i64::from(f.z) + dz;
                if nz >= i64::from(c.depth) {
                    continue;
                }
                let nx = (i64::from(f.x) + dx).rem_euclid(i64::from(c.width)) as u32;
                let (mx, mz) = ((f64::from(f.x) + 0.5 + dx as f64 / 2.0) * v, (f64::from(f.z) + 0.5 + dz as f64 / 2.0) * v);
                for ny in f.y.saturating_sub(climb)..=f.y + climb {
                    let Some(&j) = index.get(&Site { x: nx, y: ny, z: nz as u32 }) else {
                        continue;
                    };
                    if !(clear(mx, mz, f.y) && clear(mx, mz, ny)) {
                        continue;
                    }
                    if let Some(lines) = lines.as_mut() {
                        if wide[i] && wide[j] && clear_line(mx, mz, f.y) && clear_line(mx, mz, ny) {
                            lines[i].push(j);
                            lines[j].push(i);
                        } else {
                            // A tight edge (the disc clears with less than the lines' slack):
                            // kept only where the motor's own walk takes it, both ways. The
                            // centres' clearance admits a step up a one-voxel shelf from its
                            // riser's foot that the motor refuses from anywhere it can stand
                            // (SP-STUCK); an edge it refuses strands a body mid-route.
                            let a = ((f64::from(f.x) + 0.5) * v, (f64::from(f.z) + 0.5) * v);
                            let b = (a.0 + dx as f64 * v, a.1 + dz as f64 * v);
                            let walks = |from: (f64, f64), to: (f64, f64), y0: u32, y1: u32| {
                                crate::body::motor_walk(view, from, to, y0, r, wade, room, step) == Some(y1)
                            };
                            if !(walks(a, b, f.y, ny) && walks(b, a, ny, f.y)) {
                                continue;
                            }
                        }
                    }
                    adj[i].push(j);
                    adj[j].push(i);
                }
            }
        }
        if phys.climbs_walls {
            for (i, f) in faces.iter().enumerate() {
                for dir in [(1i64, 0i64), (-1, 0), (0, 1), (0, -1)] {
                    let crate::Face::Climb(top) = crate::wall_ascent(view, i64::from(f.x), f.y, f.z, dir, room, step.up)
                    else {
                        continue;
                    };
                    let to = Site {
                        x: (i64::from(f.x) + dir.0).rem_euclid(i64::from(c.width)) as u32,
                        y: top,
                        z: (i64::from(f.z) + dir.1) as u32,
                    };
                    if let Some(&j) = index.get(&to) {
                        adj[i].push(j);
                        adj[j].push(i);
                        if let Some(lines) = lines.as_mut() {
                            if wide[i] && wide[j] {
                                lines[i].push(j);
                                lines[j].push(i);
                            }
                        }
                    }
                }
            }
        }
        // Components over the edges.
        let mut components: Vec<usize> = vec![usize::MAX; faces.len()];
        for s0 in 0..faces.len() {
            if components[s0] != usize::MAX {
                continue;
            }
            components[s0] = s0;
            let mut stack = vec![s0];
            while let Some(i) = stack.pop() {
                for &j in &adj[i] {
                    if components[j] == usize::MAX {
                        components[j] = s0;
                        stack.push(j);
                    }
                }
            }
        }
        let mut sizes: FxHashMap<usize, usize> = FxHashMap::default();
        for &k in &components {
            *sizes.entry(k).or_default() += 1;
        }
        RouteGraph { faces, components, index, adj, lines, sizes, climb, width: c.width, depth: c.depth, voxel_m: v }
    }

    /// **ROUTE-WALK, a diagnostic**: for each support face in column `(x, z)` (layers
    /// `ys`), which of [`RouteGraph::for_founder`]'s tests it passes: standable (support,
    /// wade, headroom), inside the depth edges, the disc of radius + `margin_m` clear at its
    /// centre, the bare disc clear at its centre, and the deepest any solid reaches into each
    /// disc there (metres). Read only.
    pub fn face_tests(
        view: &VoxelView<'_>,
        phys: &crate::FounderPhysiology,
        margin_m: f64,
        x: u32,
        z: u32,
        ys: std::ops::RangeInclusive<u32>,
    ) -> Vec<String> {
        let c = view.config;
        let v = c.voxel_m;
        let body = phys.adult_body();
        let step = phys.step_limits(v);
        let room = body.headroom_voxels(v);
        let r0 = body.footprint_radius();
        let r = r0 + margin_m.max(0.0);
        let (cx, cz) = ((f64::from(x) + 0.5) * v, (f64::from(z) + 0.5) * v);
        let mut out = Vec::new();
        for y in ys {
            if !view.is_support(i64::from(x), y, z) {
                continue;
            }
            let wade = view.water_depth_m(i64::from(x), y, z) <= phys.wade_depth_m(&body);
            let head = has_headroom(view, i64::from(x), y, z, room);
            let edge = cz + 1e-9 >= r0 && cz <= f64::from(c.depth) * v - r0 + 1e-9;
            let clear_m = crate::body::pose_clear(view, cx, cz, y, r, room, step);
            let clear_0 = crate::body::pose_clear(view, cx, cz, y, r0, room, step);
            let pen_m = crate::body::pose_penetration(view, cx, cz, y, r, room, step);
            let pen_0 = crate::body::pose_penetration(view, cx, cz, y, r0, room, step);
            out.push(format!(
                "({x},{y},{z}) wade={wade} headroom={head} edge={edge} clear_r+m={clear_m} clear_r={clear_0} pen_r+m={pen_m:.4} pen_r={pen_0:.4} r={r0:.4} m={margin_m:.3} v={v}"
            ));
        }
        out
    }

    /// The node count of node `i`'s walkable component (WADE-OUT's pocket test).
    pub fn component_len(&self, i: usize) -> usize {
        self.components.get(i).and_then(|k| self.sizes.get(k)).copied().unwrap_or(0)
    }

    /// The node on `site`, if the disc stands clear there.
    pub fn face(&self, site: Site) -> Option<usize> {
        self.index.get(&site).copied()
    }

    /// The walkable component of the node on `site`.
    pub fn component_of(&self, site: Site) -> Option<usize> {
        self.face(site).map(|i| self.components[i])
    }

    /// The node nearest `(x, z)` within `radius` columns of `site`'s column, within the
    /// step of its layer: where a body standing (or a prey caught) at `site` is on the
    /// graph when the disc does not stand clear on its own face.
    pub fn node_near(&self, site: Site, xz: (f64, f64), radius: u32) -> Option<usize> {
        self.node_near_where(site, xz, radius, |_| true)
    }

    /// The side, in columns, of the square an island fits in ([`RouteGraph::island`]) for a
    /// body whose disc reaches `own_cols` columns from its centre: its footprint, and its own
    /// width of walking to either side of it (`4 · own_cols + 1`, at least 5). A component
    /// that small takes the body nowhere: a shelf a voxel deep and a few long on a terraced
    /// slope, or a lone clear spot.
    pub fn island_side(own_cols: u32) -> u32 {
        4 * own_cols.max(1) + 1
    }

    /// **ISLAND** (SP-STUCK): whether node `i`'s whole walkable component fits in a square
    /// `side` columns on a side. On a steep terraced vine slope the seedporter's graph held
    /// hundreds of components of one to four nodes (shelves a voxel deep); a body on one
    /// counted as on its graph ([`RouteGraph::node_near`]), so neither DROP nor a reflex walk
    /// found it stranded, and seedporters stood on such shelves for hours, starving (branch
    /// s3, 2026-10-11). Exact: the walk over the component stops at the first node that widens
    /// it past `side`.
    pub fn island(&self, i: usize, side: u32) -> bool {
        let Some(&origin) = self.faces.get(i) else { return false };
        let w = i64::from(self.width);
        let side = i64::from(side);
        let (mut x0, mut x1, mut z0, mut z1) = (0i64, 0i64, 0i64, 0i64);
        let mut seen = rustc_hash::FxHashSet::default();
        seen.insert(i);
        let mut stack = vec![i];
        while let Some(j) = stack.pop() {
            let f = self.faces[j];
            let dx = (i64::from(f.x) - i64::from(origin.x)).rem_euclid(w);
            let dx = if dx > w / 2 { dx - w } else { dx };
            let dz = i64::from(f.z) - i64::from(origin.z);
            (x0, x1, z0, z1) = (x0.min(dx), x1.max(dx), z0.min(dz), z1.max(dz));
            if x1 - x0 >= side || z1 - z0 >= side {
                return false;
            }
            for &k in &self.adj[j] {
                if seen.insert(k) {
                    stack.push(k);
                }
            }
        }
        true
    }

    /// **The node a body is on** (SP-STUCK), for a body of radius `r` standing on `site` with
    /// its centre at `xz`: `site`'s own node, else the nearest node within `radius` columns
    /// and the step ([`RouteGraph::node_near`]) to whose centre the walker's motor walks from
    /// `xz` ([`crate::body::motor_walk`]) — never a node on an island
    /// ([`RouteGraph::island`], [`RouteGraph::island_side`] of `radius`). The plain
    /// [`RouteGraph::node_near`] counted a body on a shelf as on its graph by the node on
    /// the next shelf up, a step its motor refuses from where it stands, or by its shelf's
    /// own component of a node or two: it was on no graph it could walk.
    #[allow(clippy::too_many_arguments)]
    pub fn node_under(
        &self,
        view: &VoxelView<'_>,
        site: Site,
        xz: (f64, f64),
        r: f64,
        headroom: u32,
        step: crate::StepLimits,
        wade_m: f64,
        radius: u32,
    ) -> Option<usize> {
        let side = Self::island_side(radius);
        let ring = f64::from(self.width) * self.voxel_m;
        self.node_near_where(site, xz, radius, |f| {
            let Some(j) = self.face(f) else { return false };
            if self.island(j, side) {
                return false;
            }
            if f == site {
                return true;
            }
            let (cx, cz) = self.centre(j);
            let (dx, dz) = crate::pose::wrapped_delta(xz.0, xz.1, cx, cz, ring);
            let deep = view.water_depth_m(i64::from(f.x), f.y, f.z);
            crate::body::motor_walk(view, xz, (xz.0 + dx, xz.1 + dz), site.y, r, wade_m.max(deep), headroom, step) == Some(f.y)
        })
    }

    /// [`RouteGraph::node_near`] among the nodes whose face `keep` accepts (STRIKE: a face
    /// from which a hunter's jaw band meets the prey's body).
    pub fn node_near_where(&self, site: Site, (x, z): (f64, f64), radius: u32, keep: impl Fn(Site) -> bool) -> Option<usize> {
        if let Some(i) = self.face(site).filter(|&i| keep(self.faces[i])) {
            return Some(i);
        }
        let v = self.voxel_m;
        let ring = f64::from(self.width) * v;
        let (r, climb) = (i64::from(radius), i64::from(self.climb));
        let mut best: Option<(f64, usize)> = None;
        for dz in -r..=r {
            let nz = i64::from(site.z) + dz;
            if nz < 0 || nz >= i64::from(self.depth) {
                continue;
            }
            for dx in -r..=r {
                let nx = (i64::from(site.x) + dx).rem_euclid(i64::from(self.width)) as u32;
                for dy in -climb..=climb {
                    let ny = i64::from(site.y) + dy;
                    if ny < 0 {
                        continue;
                    }
                    let Some(j) = self.face(Site { x: nx, y: ny as u32, z: nz as u32 }).filter(|&j| keep(self.faces[j])) else {
                        continue;
                    };
                    let (cx, cz) = self.centre(j);
                    let (ddx, ddz) = crate::pose::wrapped_delta(x, z, cx, cz, ring);
                    let d = ddx * ddx + ddz * ddz;
                    if best.is_none_or(|(b, bj)| d < b || (d == b && j < bj)) {
                        best = Some((d, j));
                    }
                }
            }
        }
        best.map(|(_, j)| j)
    }

    /// Route distances in steps from node `from`, out to `max_steps`: a step is one node
    /// to a neighbouring column (one voxel across).
    pub fn distances(&self, from: usize, max_steps: u32) -> FxHashMap<usize, u32> {
        let mut dist: FxHashMap<usize, u32> = FxHashMap::default();
        let mut queue = std::collections::VecDeque::new();
        dist.insert(from, 0);
        queue.push_back(from);
        while let Some(i) = queue.pop_front() {
            let d = dist[&i];
            if d >= max_steps {
                continue;
            }
            for &j in &self.adj[i] {
                if let std::collections::hash_map::Entry::Vacant(e) = dist.entry(j) {
                    e.insert(d + 1);
                    queue.push_back(j);
                }
            }
        }
        dist
    }

    /// The neighbour of node `i` one step nearer the goal of `field` (a
    /// [`RouteGraph::distances`] from the goal), the lowest index among equals; `None` at
    /// the goal or off the field.
    pub fn next_toward(&self, field: &FxHashMap<usize, u32>, i: usize) -> Option<usize> {
        let d = *field.get(&i)?;
        self.adj[i]
            .iter()
            .filter_map(|&j| field.get(&j).map(|&dj| (dj, j)))
            .filter(|&(dj, _)| dj < d)
            .min()
            .map(|(_, j)| j)
    }

    /// The voxel's size the graph was built at, metres.
    pub fn voxel_m(&self) -> f64 {
        self.voxel_m
    }

    /// **PIT-2, the point it steers at on its route**: from node `i` (where a body standing at
    /// `(x, z)` is on the graph), the farthest node of `field`'s route within `max_steps`
    /// steps of `i` to which the straight line from `(x, z)` is walkable on the graph
    /// ([`RouteGraph::walk_line`]), else the route's next node. A body steering at the next
    /// node, one column ahead, ran past it at its pace (two or three columns a controller
    /// period) and off the graph at a bend; steering further along a straight stretch, it has
    /// graph under its line the whole way. `None` at the goal or off the field.
    pub fn pursue(&self, field: &FxHashMap<usize, u32>, i: usize, (x, z): (f64, f64), max_steps: u32) -> Option<usize> {
        let mut at = self.next_toward(field, i)?;
        for _ in 1..max_steps {
            let Some(j) = self.next_toward(field, at) else { break };
            if !self.walk_line_to(i, (x, z), self.centre(j), j) {
                break;
            }
            at = j;
        }
        Some(at)
    }

    /// **PIT, the way out**: for a body standing on `site` off the graph with its centre at
    /// `xz`, the first face of the shortest way **its own body** can step back onto it — its
    /// own disc of radius `r` clear over `headroom` layers at each face's centre and at the
    /// midpoint of each step ([`crate::body::pose_clear`], the graph's own rule without the
    /// margin), each step to the neighbouring column's support face nearest its layer within
    /// `step` ([`VoxelView::nearest_support`], the motor's rule) and one the walker's motor
    /// takes ([`crate::body::motor_walk`]; SP-STUCK: the clearance rule alone admitted a step
    /// up onto the next shelf of a terraced slope that the motor refuses), into water no
    /// deeper than `wade_m` or than the water it steps from — out to `max_cols` columns round
    /// `site`. The first step is walked from `xz`, where the body stands, not from its face's
    /// centre (a body on a shelf narrower than its disc never stands there); when that fails
    /// but the body walks to its own face's centre and on from there, the way starts with
    /// `site` itself (walk to the centre first). The way ends on a face from which the graph
    /// is reached ([`RouteGraph::node_under`] within `own_cols`: never an island). `None`
    /// when the body is already on the graph or no such way exists in reach.
    #[allow(clippy::too_many_arguments)]
    pub fn way_on(
        &self,
        view: &VoxelView<'_>,
        site: Site,
        xz: (f64, f64),
        r: f64,
        headroom: u32,
        step: crate::StepLimits,
        wade_m: f64,
        own_cols: u32,
        max_cols: u32,
    ) -> Option<Site> {
        self.way_on_path(view, site, xz, r, headroom, step, wade_m, own_cols, max_cols).map(|p| p.0)
    }

    /// The same recovery walk, with its entry node and step count for a travel budget.
    #[allow(clippy::too_many_arguments)]
    pub fn way_on_path(
        &self,
        view: &VoxelView<'_>,
        site: Site,
        xz: (f64, f64),
        r: f64,
        headroom: u32,
        step: crate::StepLimits,
        wade_m: f64,
        own_cols: u32,
        max_cols: u32,
    ) -> Option<(Site, usize, u32)> {
        let v = self.voxel_m;
        let centre = |f: Site| ((f64::from(f.x) + 0.5) * v, (f64::from(f.z) + 0.5) * v);
        let entry = |f: Site, at: (f64, f64)| self.node_under(view, f, at, r, headroom, step, wade_m, own_cols);
        if max_cols == 0 || entry(site, xz).is_some() {
            return None;
        }
        let clear = |x: f64, z: f64, y: u32| crate::body::pose_clear(view, x, z, y, r, headroom, step);
        let water = |f: Site| view.water_depth_m(i64::from(f.x), f.y, f.z);
        let recentre = self.recentres(view, site, xz, r, headroom, step, wade_m);
        let w = i64::from(self.width);
        let span = |a: i64, b: i64| {
            let d = (a - b).rem_euclid(w);
            d.min(w - d)
        };
        // Breadth first from `site`; each face remembers the first step of the way to it.
        let mut first: FxHashMap<Site, (Site, u32)> = FxHashMap::default();
        let mut queue = std::collections::VecDeque::new();
        first.insert(site, (site, 0));
        queue.push_back(site);
        while let Some(f) = queue.pop_front() {
            let (fx, fz) = centre(f);
            for (dx, dz) in [(1i64, 0i64), (-1, 0), (0, 1), (0, -1)] {
                let nz = i64::from(f.z) + dz;
                if nz < 0 || nz >= i64::from(self.depth) {
                    continue;
                }
                let nx = (i64::from(f.x) + dx).rem_euclid(w);
                if span(nx, i64::from(site.x)) > i64::from(max_cols) || (nz - i64::from(site.z)).abs() > i64::from(max_cols) {
                    continue;
                }
                let Some(ny) = view.nearest_support(nx, nz as u32, f.y, step.down, step.up) else {
                    continue;
                };
                let n = Site { x: nx as u32, y: ny, z: nz as u32 };
                if first.contains_key(&n) {
                    continue;
                }
                let deep = water(n);
                if deep > wade_m && deep > water(f) + crate::body::LEVEL_TOLERANCE_M {
                    continue;
                }
                let (cx, cz) = centre(n);
                let (mx, mz) = (fx + 0.5 * dx as f64 * v, fz + 0.5 * dz as f64 * v);
                if !(clear(cx, cz, ny) && clear(mx, mz, f.y) && clear(mx, mz, ny)) {
                    continue;
                }
                // The motor's own walk: from where the body stands for the first step (else
                // via its face's centre, the way then starting there), centre to centre after.
                let walk = |from: (f64, f64)| {
                    let to = self.toward(from, (fx + dx as f64 * v, fz + dz as f64 * v));
                    crate::body::motor_walk(view, from, to, f.y, r, wade_m.max(deep), headroom, step) == Some(ny)
                };
                let way = if f == site {
                    if walk(xz) {
                        n
                    } else if recentre && walk((fx, fz)) {
                        site
                    } else {
                        continue;
                    }
                } else if walk((fx, fz)) {
                    first[&f].0
                } else {
                    continue;
                };
                let steps = first[&f].1 + 1;
                if let Some(entry) = entry(n, centre(n)) {
                    return Some((way, entry, steps));
                }
                first.insert(n, (way, steps));
                queue.push_back(n);
            }
        }
        None
    }

    /// Whether a body standing on `site` at `xz` may begin a walk by going to its face's
    /// centre (SP-STUCK): its disc stands clear there and its motor walks it there.
    #[allow(clippy::too_many_arguments)]
    fn recentres(
        &self,
        view: &VoxelView<'_>,
        site: Site,
        xz: (f64, f64),
        r: f64,
        headroom: u32,
        step: crate::StepLimits,
        wade_m: f64,
    ) -> bool {
        let v = self.voxel_m;
        let c = ((f64::from(site.x) + 0.5) * v, (f64::from(site.z) + 0.5) * v);
        let to = self.toward(xz, c);
        let deep = view.water_depth_m(i64::from(site.x), site.y, site.z);
        crate::body::pose_clear(view, c.0, c.1, site.y, r, headroom, step)
            && crate::body::motor_walk(view, xz, to, site.y, r, wade_m.max(deep), headroom, step) == Some(site.y)
    }

    /// `to` brought within half the ring of `from` (the motor's walk does not wrap).
    fn toward(&self, from: (f64, f64), to: (f64, f64)) -> (f64, f64) {
        let ring = f64::from(self.width) * self.voxel_m;
        let (dx, dz) = crate::pose::wrapped_delta(from.0, from.1, to.0, to.1, ring);
        (from.0 + dx, from.1 + dz)
    }

    /// **DROP, the jump down**: for a body standing on `site` off the graph, its centre at
    /// `xz`, with no walking way back on, the lowest jump down that lands where the graph is
    /// reachable — `None` when there is none. The pocket is the faces its own body walks to
    /// from `site` out to `max_cols` columns, each step one the motor itself admits
    /// ([`crate::body::motor_walk`]: stricter than the clearance rule alone), the first from
    /// `xz` where it stands (SP-STUCK: walked from its face's centre, which a body on a shelf
    /// narrower than its disc never reaches, the pocket ran up the slope to faces it could
    /// not reach, and it jumped from none), or via its face's centre when it walks there. A
    /// jump leaves a pocket face `f` over a neighbouring column with no support face within
    /// its `step` of `f`'s layer, the motor walking it to that edge (from `xz` on `site`), and
    /// lands on the support face nearest under `f` in that column or one further out (to
    /// `ceil(r / voxel) + 1` columns: a wide disc cannot land against the riser), past
    /// `step.down` and at most `reach` layers below `f`, its column open from the landing up
    /// to `f`'s headroom and every column passed over open from a step over the landing up
    /// (SP-STUCK: the next stair down a terraced slope, a voxel deep, is jumped over). The disc of radius `r` must stand clear over
    /// `headroom` layers at the landing's centre, else pressed to a side or corner of its
    /// column ([`DROP_LANDINGS`]; SP-STUCK: on a terraced slope each face has a riser behind
    /// it), the landing's water be no deeper than
    /// `wade_m`, and the graph be reached from it ([`RouteGraph::node_under`] within
    /// `own_cols`, else [`RouteGraph::way_on`] out to `max_cols`): never onto an island, nor
    /// back into its own pocket. Of those, the shallowest drop, then the nearest pocket face.
    /// Never a climb, and never a route: the graph's own edges stay within the step.
    ///
    /// **SP-STUCK-2, a jump in two**: when no landing reaches the graph, a landing in a lower
    /// pocket from which a jump of its own does ([`DROP_HOPS`]) is taken: the shallowest
    /// first jump, then the nearest pocket face. A seedporter on a shelf a voxel deep in a
    /// cliff of such shelves (s5, 2026-10-11) had its next shelf five layers down, itself
    /// cornered with the floor ten layers under it, both within its reach; no single jump
    /// reached its graph, so it never jumped, and starved there for hours.
    #[allow(clippy::too_many_arguments)]
    pub fn drop_way(
        &self,
        view: &VoxelView<'_>,
        site: Site,
        xz: (f64, f64),
        r: f64,
        headroom: u32,
        step: crate::StepLimits,
        reach: u32,
        wade_m: f64,
        own_cols: u32,
        max_cols: u32,
    ) -> Option<DropWay> {
        self.drop_way_hops(view, site, xz, r, headroom, step, reach, wade_m, own_cols, max_cols, DROP_HOPS, None)
    }

    /// **SP-CRAVE: [`RouteGraph::drop_way`], or else a jump into water** (Wrysk's DROP rule,
    /// 2026-10-06: "a trapped walker may jump down further than its normal step-down … escape
    /// only", extended to water): with no dry jump on offer (one jump or two), the lowest
    /// drop off the pocket's edge into water deeper than it wades, at most `water.reach_m`
    /// from its face down to the water's surface, onto a face clear for its body under that
    /// water, from which it gets out (`water.out`): never into water with no way out. The
    /// pocket, the edge and the columns passed over are the dry jump's.
    #[allow(clippy::too_many_arguments)]
    pub fn drop_way_into(
        &self,
        view: &VoxelView<'_>,
        site: Site,
        xz: (f64, f64),
        r: f64,
        headroom: u32,
        step: crate::StepLimits,
        reach: u32,
        wade_m: f64,
        own_cols: u32,
        max_cols: u32,
        water: &WaterDrop<'_>,
    ) -> Option<DropWay> {
        self.drop_way_hops(view, site, xz, r, headroom, step, reach, wade_m, own_cols, max_cols, DROP_HOPS, Some(water))
    }

    /// [`RouteGraph::drop_way`], with `hops` further jumps allowed after this one, and a jump
    /// into water last when `water` allows one.
    #[allow(clippy::too_many_arguments)]
    fn drop_way_hops(
        &self,
        view: &VoxelView<'_>,
        site: Site,
        xz: (f64, f64),
        r: f64,
        headroom: u32,
        step: crate::StepLimits,
        reach: u32,
        wade_m: f64,
        own_cols: u32,
        max_cols: u32,
        hops: u32,
        water_drop: Option<&WaterDrop<'_>>,
    ) -> Option<DropWay> {
        let wet_ok = water_drop.is_some_and(|w| w.reach_m > 0.0);
        if reach <= step.down && !wet_ok {
            return None;
        }
        let v = self.voxel_m;
        let centre = |f: Site| ((f64::from(f.x) + 0.5) * v, (f64::from(f.z) + 0.5) * v);
        let on = |f: Site, p: (f64, f64)| self.node_under(view, f, p, r, headroom, step, wade_m, own_cols).is_some();
        let water = |f: Site| view.water_depth_m(i64::from(f.x), f.y, f.z);
        let recentre = self.recentres(view, site, xz, r, headroom, step, wade_m);
        let w = i64::from(self.width);
        let span = |a: i64, b: i64| {
            let d = (a - b).rem_euclid(w);
            d.min(w - d)
        };
        let dirs = [(1i64, 0i64), (-1, 0), (0, 1), (0, -1)];
        // The pocket, breadth first from `site` (way_on's walking rule); each face remembers
        // the first step of the walk to it.
        let mut pocket: Vec<(Site, Site)> = vec![(site, site)];
        let mut seen: FxHashMap<Site, ()> = FxHashMap::default();
        seen.insert(site, ());
        let mut k = 0;
        while k < pocket.len() {
            let (f, first) = pocket[k];
            k += 1;
            let (fx, fz) = centre(f);
            for (dx, dz) in dirs {
                let nz = i64::from(f.z) + dz;
                if nz < 0 || nz >= i64::from(self.depth) {
                    continue;
                }
                let nx = (i64::from(f.x) + dx).rem_euclid(w);
                if span(nx, i64::from(site.x)) > i64::from(max_cols) || (nz - i64::from(site.z)).abs() > i64::from(max_cols) {
                    continue;
                }
                let Some(ny) = view.nearest_support(nx, nz as u32, f.y, step.down, step.up) else {
                    continue;
                };
                let n = Site { x: nx as u32, y: ny, z: nz as u32 };
                if seen.contains_key(&n) {
                    continue;
                }
                let deep = water(n);
                if deep > wade_m && deep > water(f) + crate::body::LEVEL_TOLERANCE_M {
                    continue;
                }
                // The motor's own walk (it is stricter than the clearance rule): from where
                // it stands for the first step, else via its face's centre; centre to centre
                // after.
                let walk = |from: (f64, f64)| {
                    let to = self.toward(from, (fx + dx as f64 * v, fz + dz as f64 * v));
                    crate::body::motor_walk(view, from, to, f.y, r, wade_m.max(deep), headroom, step) == Some(ny)
                };
                let first_step = if f == site {
                    if walk(xz) {
                        n
                    } else if recentre && walk((fx, fz)) {
                        site
                    } else {
                        continue;
                    }
                } else if walk((fx, fz)) {
                    first
                } else {
                    continue;
                };
                seen.insert(n, ());
                pocket.push((n, first_step));
            }
        }
        // The jumps off its edges: (layers, pocket order, from, landing, edge). A jump goes out
        // over the edge column (no face within the step of `f`) and lands on the support face
        // nearest under `f` in that column or one further out, up to the columns its disc spans
        // and one: a disc wider than half a voxel cannot land against the ledge's riser, so it
        // jumps out. The landing's column is open from the landing up to its headroom, and
        // every column it passes over from a step over the landing up.
        let out = (r / v).ceil().max(0.0) as i64 + 1;
        let mut jumps: Vec<(u32, usize, Site, Site, Site, u8)> = Vec::new();
        // SP-CRAVE: the jumps into water, as `jumps` (their layers counted to the surface).
        let mut wet: Vec<(u32, usize, Site, Site, Site, u8)> = Vec::new();
        for (order, &(f, _)) in pocket.iter().enumerate() {
            let (fx, fz) = centre(f);
            for (dx, dz) in dirs {
                let col = |k: i64| -> Option<(i64, u32)> {
                    let z = i64::from(f.z) + dz * k;
                    (z >= 0 && z < i64::from(self.depth)).then(|| ((i64::from(f.x) + dx * k).rem_euclid(w), z as u32))
                };
                let Some((ex, ez)) = col(1) else { continue };
                if view.nearest_support(ex, ez, f.y, step.down, step.up).is_some() {
                    continue;
                }
                // The motor walks it to the edge: its centre to the column's boundary, less a
                // hair — from where it stands on `site` (along the edge's axis only), else
                // from its face's centre.
                let edge_walk = |from: (f64, f64)| {
                    let to = if dx != 0 {
                        (fx + (0.5 - 1e-3) * dx as f64 * v, from.1)
                    } else {
                        (from.0, fz + (0.5 - 1e-3) * dz as f64 * v)
                    };
                    let to = self.toward(from, to);
                    // SP-PROTEIN: a centre already at or past that point along the edge's axis
                    // is at the edge (its disc reaches over it, as the jump's own test reads).
                    // A body pressed back into the corner of a one-column perch stands there,
                    // and its walk "to the edge" went back into the wall behind it, so it found
                    // no jump and stayed (a seedporter on one column for hours, 2026-10-13).
                    let ahead = if dx != 0 { (to.0 - from.0) * dx as f64 } else { (to.1 - from.1) * dz as f64 };
                    if ahead <= 0.0 {
                        return true;
                    }
                    crate::body::motor_walk(view, from, to, f.y, r, wade_m.max(water(f)), headroom, step) == Some(f.y)
                };
                let reached = if f == site {
                    edge_walk(xz) || (recentre && edge_walk((fx, fz)))
                } else {
                    edge_walk((fx, fz))
                };
                if !reached {
                    continue;
                }
                let edge = Site { x: ex as u32, y: f.y, z: ez };
                for k in 1..=out {
                    let Some((nx, nz)) = col(k) else { break };
                    // A dry landing lies within `reach`; one under water may lie deeper.
                    let Some(ly) = view.nearest_support(nx, nz, f.y, if wet_ok { f.y } else { reach }, 0) else {
                        continue;
                    };
                    if ly + step.down >= f.y {
                        continue;
                    }
                    let depth = water(Site { x: nx as u32, y: ly, z: nz });
                    let into_water = depth > wade_m;
                    if into_water {
                        // Its drop to the surface, metres, within its reach into water.
                        let over = crate::surface_m(f.y, v) - (crate::surface_m(ly, v) + depth);
                        if !water_drop.is_some_and(|w| over <= w.reach_m) {
                            continue;
                        }
                    } else if f.y - ly > reach {
                        continue;
                    }
                    // SP-STUCK: a column it passes over may hold a step within its step up of
                    // the landing (the next stair of a terraced slope, which its landed disc
                    // straddles); only a rise past that stands in its way.
                    let open = |m: i64, lo: u32| col(m).is_some_and(|(cx, cz)| !view.solid_in(cx, cz, lo, f.y + headroom.max(1)));
                    if !open(k, ly + 1) || (1..k).any(|m| !open(m, ly + step.up + 1)) {
                        continue;
                    }
                    let land = Site { x: nx as u32, y: ly, z: nz };
                    // Its disc clear at the column's centre, else pressed to a side of it.
                    let Some(at) = (0..DROP_LANDINGS.len() as u8).find(|&at| {
                        let (x, z) = drop_landing(land, at, v, self.width);
                        crate::body::pose_penetration(view, x, z, ly, r, headroom, step) <= LANDING_TOLERANCE_M
                    }) else {
                        continue;
                    };
                    if into_water {
                        let over = crate::surface_m(f.y, v) - (crate::surface_m(ly, v) + depth);
                        wet.push(((over / v).ceil().max(0.0) as u32, order, f, land, edge, at));
                    } else {
                        jumps.push((f.y - ly, order, f, land, edge, at));
                    }
                }
            }
        }
        jumps.sort_by_key(|&(layers, order, _, _, _, at)| (layers, order, at));
        jumps.retain(|j| !seen.contains_key(&j.3));
        let way = |&(_, order, from, land, edge, at): &(u32, usize, Site, Site, Site, u8)| DropWay { from, land, edge, first: pocket[order].1, at };
        let direct = jumps.iter().find(|&&(_, _, _, land, _, at)| {
            let p = drop_landing(land, at, v, self.width);
            on(land, p) || self.way_on(view, land, p, r, headroom, step, wade_m, own_cols, max_cols).is_some()
        });
        if let Some(j) = direct {
            return Some(way(j));
        }
        // A jump in two: into a lower pocket with a jump on of its own. Each landing face is
        // asked once, from its first landing point.
        if hops > 0 && reach > step.down {
            let mut asked: FxHashMap<Site, ()> = FxHashMap::default();
            let two = jumps.iter().find(|&&(_, _, _, land, _, at)| {
                if asked.insert(land, ()).is_some() {
                    return false;
                }
                let p = drop_landing(land, at, v, self.width);
                self.drop_way_hops(view, land, p, r, headroom, step, reach, wade_m, own_cols, max_cols, hops - 1, None).is_some()
            });
            if let Some(j) = two {
                return Some(way(j));
            }
        }
        // SP-CRAVE: last, into water it gets out of: the lowest drop to the surface, then the
        // nearest pocket face. Each landing face is asked once.
        let Some(w) = water_drop else { return None };
        wet.sort_by_key(|&(layers, order, _, _, _, at)| (layers, order, at));
        wet.retain(|j| !seen.contains_key(&j.3));
        let mut asked: FxHashMap<Site, bool> = FxHashMap::default();
        wet.iter().find(|&&(_, _, _, land, _, _)| *asked.entry(land).or_insert_with(|| (w.out)(land))).map(way)
    }

    /// **WADE-OUT, the way out through the water**: for a body held on `site` — out of its
    /// depth, or on a pocket of its graph (a component under `open_min` nodes) — the walk its
    /// own body takes, out to `max_cols` columns round `site`, to a face whose water it wades
    /// (`wade_m`) and from which an open part of its graph is reached
    /// ([`RouteGraph::node_near`] within `own_cols`, on a component of at least `open_min`
    /// nodes). Water past its wade depth does not stop it; water past `under_m` (the depth it
    /// is under, and drowns in if it stays) it crosses on at most `max_under` faces. Of those
    /// ways, the one with the fewest faces under, then the fewest faces, whose goal `accept`
    /// takes. Each step is to the
    /// neighbouring column's support face nearest its layer within `step`
    /// ([`VoxelView::nearest_support`]), its disc clear at the face's centre and the step's
    /// midpoint, and the motor's own walk centre to centre ([`crate::body::motor_walk`]) at
    /// any depth. The faces from `site` to the goal, the deepest water on them and the number
    /// under; `None` when there is no such way in reach. An escape only, as DROP's jump is: no
    /// route ever wades past its body's depth.
    #[allow(clippy::too_many_arguments)]
    pub fn wade_way(
        &self,
        view: &VoxelView<'_>,
        site: Site,
        r: f64,
        headroom: u32,
        step: crate::StepLimits,
        (wade_m, under_m, max_under): (f64, f64, u32),
        own_cols: u32,
        open_min: usize,
        max_cols: u32,
        accept: &mut dyn FnMut(Site) -> bool,
    ) -> Option<(Vec<Site>, f64, u32)> {
        if max_cols == 0 {
            return None;
        }
        let v = self.voxel_m;
        let centre = |f: Site| ((f64::from(f.x) + 0.5) * v, (f64::from(f.z) + 0.5) * v);
        let open = |f: Site| self.node_near(f, centre(f), own_cols).is_some_and(|n| self.component_len(n) >= open_min);
        let clear = |x: f64, z: f64, y: u32| crate::body::pose_clear(view, x, z, y, r, headroom, step);
        let water = |f: Site| view.water_depth_m(i64::from(f.x), f.y, f.z);
        let w = i64::from(self.width);
        let span = |a: i64, b: i64| {
            let d = (a - b).rem_euclid(w);
            d.min(w - d)
        };
        // Fewest faces under, then fewest faces: one face under outweighs every walk in the
        // window.
        let side = 2 * u64::from(max_cols) + 1;
        let heavy = side * side;
        let mut best: FxHashMap<Site, (u64, u32, Site)> = FxHashMap::default();
        let mut heap = std::collections::BinaryHeap::new();
        best.insert(site, (0, 0, site));
        heap.push(std::cmp::Reverse((0u64, site.x, site.y, site.z)));
        while let Some(std::cmp::Reverse((cost, x, y, z))) = heap.pop() {
            let f = Site { x, y, z };
            let (c, under, _) = best[&f];
            if cost > c {
                continue;
            }
            if f != site && water(f) <= wade_m && open(f) && accept(f) {
                let mut faces = vec![f];
                let mut at = f;
                while at != site {
                    at = best[&at].2;
                    faces.push(at);
                }
                faces.reverse();
                let deepest = faces.iter().map(|&f| water(f)).fold(0.0, f64::max);
                return Some((faces, deepest, under));
            }
            let (fx, fz) = centre(f);
            for (dx, dz) in [(1i64, 0i64), (-1, 0), (0, 1), (0, -1)] {
                let nz = i64::from(f.z) + dz;
                if nz < 0 || nz >= i64::from(self.depth) {
                    continue;
                }
                let nx = (i64::from(f.x) + dx).rem_euclid(w);
                if span(nx, i64::from(site.x)) > i64::from(max_cols) || (nz - i64::from(site.z)).abs() > i64::from(max_cols) {
                    continue;
                }
                let Some(ny) = view.nearest_support(nx, nz as u32, f.y, step.down, step.up) else {
                    continue;
                };
                let n = Site { x: nx as u32, y: ny, z: nz as u32 };
                let is_under = water(n) > under_m;
                let n_under = under + u32::from(is_under);
                if n_under > max_under {
                    continue;
                }
                let n_cost = cost + 1 + if is_under { heavy } else { 0 };
                if best.get(&n).is_some_and(|&(c, _, _)| c <= n_cost) {
                    continue;
                }
                let (cx, cz) = centre(n);
                let (mx, mz) = (fx + 0.5 * dx as f64 * v, fz + 0.5 * dz as f64 * v);
                if !(clear(cx, cz, ny) && clear(mx, mz, f.y) && clear(mx, mz, ny)) {
                    continue;
                }
                if crate::body::motor_walk(view, (fx, fz), (fx + dx as f64 * v, fz + dz as f64 * v), f.y, r, f64::INFINITY, headroom, step) != Some(ny) {
                    continue;
                }
                best.insert(n, (n_cost, n_under, f));
                heap.push(std::cmp::Reverse((n_cost, n.x, n.y, n.z)));
            }
        }
        None
    }

    /// **WADE-OUT, a body's own wading pocket**: the faces a body of radius `r` walks to from
    /// `site` through water it wades (`wade_m`; the step and clearance rule of
    /// [`RouteGraph::way_on`]), and whether they all lie within `max_cols` columns of `site`
    /// (`true`: a pocket it cannot walk out of). When its walk reaches that far it stops
    /// there (`false`, the faces found so far); `site` deeper than it wades is no pocket
    /// (`false`, none). A juvenile wades shallower water than the adult its lineage's graph is
    /// built for, so a shallow on the graph's open water can be a pocket to it.
    #[allow(clippy::too_many_arguments)]
    pub fn wade_pocket(
        &self,
        view: &VoxelView<'_>,
        site: Site,
        r: f64,
        headroom: u32,
        step: crate::StepLimits,
        wade_m: f64,
        max_cols: u32,
    ) -> (bool, Vec<Site>) {
        let v = self.voxel_m;
        let water = |f: Site| view.water_depth_m(i64::from(f.x), f.y, f.z);
        if water(site) > wade_m {
            return (false, Vec::new());
        }
        let centre = |f: Site| ((f64::from(f.x) + 0.5) * v, (f64::from(f.z) + 0.5) * v);
        let clear = |x: f64, z: f64, y: u32| crate::body::pose_clear(view, x, z, y, r, headroom, step);
        let w = i64::from(self.width);
        let span = |a: i64, b: i64| {
            let d = (a - b).rem_euclid(w);
            d.min(w - d)
        };
        let mut seen: FxHashMap<Site, ()> = FxHashMap::default();
        seen.insert(site, ());
        let mut pocket = vec![site];
        let mut k = 0;
        while k < pocket.len() {
            let f = pocket[k];
            k += 1;
            let (fx, fz) = centre(f);
            for (dx, dz) in [(1i64, 0i64), (-1, 0), (0, 1), (0, -1)] {
                let nz = i64::from(f.z) + dz;
                if nz < 0 || nz >= i64::from(self.depth) {
                    continue;
                }
                let nx = (i64::from(f.x) + dx).rem_euclid(w);
                let Some(ny) = view.nearest_support(nx, nz as u32, f.y, step.down, step.up) else {
                    continue;
                };
                let n = Site { x: nx as u32, y: ny, z: nz as u32 };
                if seen.contains_key(&n) || water(n) > wade_m {
                    continue;
                }
                let (cx, cz) = centre(n);
                let (mx, mz) = (fx + 0.5 * dx as f64 * v, fz + 0.5 * dz as f64 * v);
                if !(clear(cx, cz, ny) && clear(mx, mz, f.y) && clear(mx, mz, ny)) {
                    continue;
                }
                if span(nx, i64::from(site.x)) > i64::from(max_cols) || (nz - i64::from(site.z)).abs() > i64::from(max_cols) {
                    return (false, pocket);
                }
                seen.insert(n, ());
                pocket.push(n);
            }
        }
        (true, pocket)
    }

    /// Node `i`'s neighbours on the graph.
    pub fn neighbours(&self, i: usize) -> &[usize] {
        &self.adj[i]
    }

    /// **EDGE: the route from node `from` to node `goal`**, at most `max_steps` steps (one
    /// node to a neighbouring column each): the nodes in order, `from` first and `goal`
    /// last; `None` when the two are in different components or no route is that short.
    /// A* on the columns walked, the wrapped x plus z span to the goal being a lower bound.
    pub fn path(&self, from: usize, goal: usize, max_steps: u32) -> Option<Vec<usize>> {
        if self.components[from] != self.components[goal] {
            return None;
        }
        let w = i64::from(self.width);
        let g = self.faces[goal];
        let h = |i: usize| -> u32 {
            let f = self.faces[i];
            let dx = (i64::from(f.x) - i64::from(g.x)).rem_euclid(w);
            (dx.min(w - dx) + (i64::from(f.z) - i64::from(g.z)).abs()) as u32
        };
        if h(from) > max_steps {
            return None;
        }
        let mut best: FxHashMap<usize, (u32, usize)> = FxHashMap::default();
        let mut open = std::collections::BinaryHeap::new();
        best.insert(from, (0, from));
        open.push(std::cmp::Reverse((h(from), 0u32, from)));
        while let Some(std::cmp::Reverse((_, d, i))) = open.pop() {
            if best.get(&i).is_some_and(|&(b, _)| b < d) {
                continue;
            }
            if i == goal {
                let mut out = vec![goal];
                let mut k = goal;
                while k != from {
                    k = best[&k].1;
                    out.push(k);
                }
                out.reverse();
                return Some(out);
            }
            for &j in &self.adj[i] {
                let dj = d + 1;
                if dj + h(j) > max_steps || best.get(&j).is_some_and(|&(b, _)| b <= dj) {
                    continue;
                }
                best.insert(j, (dj, i));
                open.push(std::cmp::Reverse((dj + h(j), dj, j)));
            }
        }
        None
    }

    /// EDGE: where a body standing on node `path[0]` at `xz` heads on `path` (a
    /// [`RouteGraph::path`]): the farthest of its next `lookahead` nodes it can walk straight
    /// to ([`RouteGraph::walk_line`]), else the next node.
    pub fn waypoint(&self, path: &[usize], xz: (f64, f64), lookahead: usize) -> Option<(f64, f64)> {
        let next = *path.get(1)?;
        let far = path.iter().skip(2).take(lookahead.saturating_sub(1)).rev().find(|&&j| self.walk_line_to(path[0], xz, self.centre(j), j));
        Some(self.centre(*far.unwrap_or(&next)))
    }

    /// The centre of node `i`, metres `(x, z)`.
    pub fn centre(&self, i: usize) -> (f64, f64) {
        let f = self.faces[i];
        ((f64::from(f.x) + 0.5) * self.voxel_m, (f64::from(f.z) + 0.5) * self.voxel_m)
    }

    /// Whether the straight line from node `from` at `(x0, z0)` to `(x1, z1)` is walkable on
    /// the graph: sampled each half voxel, every column it enters holds a node joined to the
    /// last one by an edge (a diagonal crossing by two). On the walking graph
    /// ([`RouteGraph::for_walking`]) only its line edges count: a line steers a body off its
    /// route's centres, so it keeps the lines' slack.
    pub fn walk_line(&self, from: usize, xz: (f64, f64), end: (f64, f64)) -> bool {
        self.line_end(from, xz, end).is_some()
    }

    /// [`RouteGraph::walk_line`] to node `goal`'s level (DEN-FLOOD): the line is walkable
    /// **and** the node it ends on is `goal` itself, or, ending in another column than
    /// `goal`'s, a node within the mutual step of `goal`'s layer. The plain line test only
    /// asks for a node in each column, on any layer: a pack standing on the roof straight
    /// above its cave den had a "walkable" line of length zero to the den and stood there,
    /// homing, for good.
    pub fn walk_line_to(&self, from: usize, xz: (f64, f64), end: (f64, f64), goal: usize) -> bool {
        self.line_end(from, xz, end).is_some_and(|j| {
            let (f, g) = (self.faces[j], self.faces[goal]);
            j == goal || ((f.x, f.z) != (g.x, g.z) && f.y.abs_diff(g.y) <= self.climb)
        })
    }

    /// The node a walkable straight line from node `from` at `(x0, z0)` to `(x1, z1)` ends
    /// on ([`RouteGraph::walk_line`]); `None` when it is not walkable.
    fn line_end(&self, from: usize, (x0, z0): (f64, f64), (x1, z1): (f64, f64)) -> Option<usize> {
        let v = self.voxel_m;
        let ring = f64::from(self.width) * v;
        let (dx, dz) = crate::pose::wrapped_delta(x0, z0, x1, z1, ring);
        let len = (dx * dx + dz * dz).sqrt();
        let n = ((len / (0.5 * v)).ceil() as usize).max(1);
        let adj = self.lines.as_ref().unwrap_or(&self.adj);
        let mut cur = from;
        for k in 1..=n {
            let t = k as f64 / n as f64;
            let (x, z) = (x0 + dx * t, z0 + dz * t);
            let cz = (z / v).floor();
            if !(0.0..f64::from(self.depth)).contains(&cz) {
                return None;
            }
            let cx = ((x / v).floor() as i64).rem_euclid(i64::from(self.width)) as u32;
            let f = self.faces[cur];
            if f.x == cx && f.z == cz as u32 {
                continue;
            }
            let next = adj[cur]
                .iter()
                .copied()
                .find(|&j| self.faces[j].x == cx && self.faces[j].z == cz as u32)
                .or_else(|| {
                    adj[cur].iter().find_map(|&k| {
                        adj[k].iter().copied().find(|&j| self.faces[j].x == cx && self.faces[j].z == cz as u32)
                    })
                });
            match next {
                Some(j) => cur = j,
                None => return None,
            }
        }
        Some(cur)
    }
}

/// The faces a lineage's founders may be placed on (fix 2,
/// `design/handoffs/browser-stuck-in-crags-2026-10-01.md`): standable for its adult body,
/// in the **largest** component of the graph below. A founder with its disc in a wall can
/// move nowhere; one in a pocket of a few square metres eats it bare and cannot leave.
///
/// The graph is the disc's, not the column's, with `margin_m`
/// ([`crate::FoundingConfig::clearance_margin_m`]) added to the radius:
/// - a face is a node when the disc at its centre ([`crate::Pose::at_site`]) is clear of
///   solid over its headroom ([`crate::body::pose_clear`], the motion's own clearance);
/// - two neighbouring nodes within the lineage's mutual step are joined when the disc is
///   also clear at the midpoint between their centres, standing on either layer, so a
///   corner the swept disc meets between two clear centres cuts the edge;
/// - a climber's wall links ([`crate::wall_ascent`]) join as in [`route_components`].
///
/// The margin is why a corridor exactly as wide as the disc is not a way out: the live
/// body passes one only on its exact centre line.
pub fn founding_faces(
    view: &VoxelView<'_>,
    phys: &crate::FounderPhysiology,
    margin_m: f64,
) -> Vec<Site> {
    let c = view.config;
    let v = c.voxel_m;
    let body = phys.adult_body();
    let step = phys.step_limits(v);
    let room = body.headroom_voxels(v);
    let r = body.footprint_radius() + margin_m.max(0.0);
    let clear = |x: f64, z: f64, y: u32| crate::body::pose_clear(view, x, z, y, r, room, step);
    let faces: Vec<Site> = standable_faces(view, &body, phys.wade_depth_m(&body))
        .into_iter()
        // Its centre where the motor can hold it: no nearer the strip's `z` ends than its
        // radius (roster mechanics: a disc founded over the end could not move).
        .filter(|f| {
            let z = (f64::from(f.z) + 0.5) * v;
            z + 1e-9 >= body.footprint_radius() && z <= f64::from(c.depth) * v - body.footprint_radius() + 1e-9
        })
        .filter(|f| clear((f64::from(f.x) + 0.5) * v, (f64::from(f.z) + 0.5) * v, f.y))
        .collect();
    let index: FxHashMap<Site, usize> = faces.iter().enumerate().map(|(i, f)| (*f, i)).collect();
    let mut parent: Vec<usize> = (0..faces.len()).collect();
    fn root(parent: &mut [usize], mut i: usize) -> usize {
        while parent[i] != i {
            parent[i] = parent[parent[i]];
            i = parent[i];
        }
        i
    }
    let join = |parent: &mut Vec<usize>, a: usize, b: usize| {
        let (ra, rb) = (root(parent, a), root(parent, b));
        if ra != rb {
            parent[ra.max(rb)] = ra.min(rb);
        }
    };
    let climb = step.mutual();
    for (i, f) in faces.iter().enumerate() {
        for (dx, dz) in [(1i64, 0i64), (0, 1)] {
            let nz = i64::from(f.z) + dz;
            if nz >= i64::from(c.depth) {
                continue;
            }
            let nx = (i64::from(f.x) + dx).rem_euclid(i64::from(c.width)) as u32;
            // The midpoint between the two centres, unwrapped on x: the disc's lookups wrap.
            let (mx, mz) = (
                (f64::from(f.x) + 0.5 + dx as f64 / 2.0) * v,
                (f64::from(f.z) + 0.5 + dz as f64 / 2.0) * v,
            );
            for ny in f.y.saturating_sub(climb)..=f.y + climb {
                let Some(&j) = index.get(&Site { x: nx, y: ny, z: nz as u32 }) else {
                    continue;
                };
                if clear(mx, mz, f.y) && clear(mx, mz, ny) {
                    join(&mut parent, i, j);
                }
            }
        }
    }
    if phys.climbs_walls {
        for (i, f) in faces.iter().enumerate() {
            for dir in [(1i64, 0i64), (-1, 0), (0, 1), (0, -1)] {
                let crate::Face::Climb(top) =
                    crate::wall_ascent(view, i64::from(f.x), f.y, f.z, dir, room, step.up)
                else {
                    continue;
                };
                let to = Site {
                    x: (i64::from(f.x) + dir.0).rem_euclid(i64::from(c.width)) as u32,
                    y: top,
                    z: (i64::from(f.z) + dir.1) as u32,
                };
                if let Some(&j) = index.get(&to) {
                    join(&mut parent, i, j);
                }
            }
        }
    }
    let roots: Vec<usize> = (0..faces.len()).map(|i| root(&mut parent, i)).collect();
    let mut size: FxHashMap<usize, usize> = FxHashMap::default();
    for r in &roots {
        *size.entry(*r).or_default() += 1;
    }
    // The largest; a tie goes to the lowest root, so the answer is one component.
    let Some(big) = size
        .iter()
        .max_by(|a, b| a.1.cmp(b.1).then(b.0.cmp(a.0)))
        .map(|(r, _)| *r)
    else {
        return Vec::new();
    };
    faces
        .into_iter()
        .zip(roots)
        .filter(|(_, r)| *r == big)
        .map(|(f, _)| f)
        .collect()
}

/// One lineage's **route map** over a world: every face its adult body can stand on,
/// the walkable component of each under the lineage's climb, and the mouth columns a body
/// standing there could cover at any heading.
///
/// The observer (`voxel_edible_stock`) and the seeder's acceptance check ask the same
/// thing through it. Every rule in it is
/// one of the helpers above: [`standable_faces`], [`walkable_components`],
/// [`mouth_columns_from_face`], the mouth band and [`reachable_layers_of`].
pub struct RouteMap {
    /// The standable faces, in [`standable_faces`] order.
    pub faces: Vec<Site>,
    /// The walkable component of each face, in the order of `faces`.
    pub components: Vec<usize>,
    index: FxHashMap<Site, usize>,
    /// The union of mouth columns over every heading, per face.
    columns: Vec<Vec<(i64, u32)>>,
    /// From an `(x, z)` column back to every face whose mouth can cover it.
    by_column: FxHashMap<(u32, u32), Vec<usize>>,
    body: Body,
    voxel_m: f64,
}

impl RouteMap {
    /// The route map of a founder lineage's **adult** body: its wade depth (a fraction of
    /// its height), its step up and down, and — for a climber — the walls it goes up and
    /// down.
    pub fn for_founder(view: &VoxelView<'_>, phys: &crate::FounderPhysiology) -> RouteMap {
        let body = phys.adult_body();
        RouteMap::new(
            view,
            body,
            phys.wade_depth_m(&body),
            RouteRule {
                step: phys.step_limits(view.config.voxel_m),
                climbs_walls: phys.climbs_walls,
            },
        )
    }

    /// The route map of any body, wade depth and route rule.
    pub fn new(view: &VoxelView<'_>, body: Body, wade_depth_m: f64, rule: RouteRule) -> RouteMap {
        let c = view.config;
        let faces = standable_faces(view, &body, wade_depth_m);
        let components = route_components(view, &faces, rule, body.headroom_voxels(c.voxel_m));
        let index: FxHashMap<Site, usize> =
            faces.iter().enumerate().map(|(i, f)| (*f, i)).collect();
        // Every face's mouth columns are its own (the slow part on a wide world: 72
        // headings per face), so they are found in face ranges side by side on the compute
        // pool and come back in face order; the column index is then built serially in
        // that order, exactly as one pass would.
        let threads = cubarium_voxel::default_threads();
        let jobs: Vec<crate::par::Job<'_, Vec<Vec<(i64, u32)>>>> = crate::par::ranges(faces.len(), threads, 512)
            .into_iter()
            .map(|r| -> crate::par::Job<'_, Vec<Vec<(i64, u32)>>> {
                let (faces, body) = (&faces, &body);
                Box::new(move || faces[r].iter().map(|f| mouth_columns_from_face(view, *f, body)).collect())
            })
            .collect();
        let columns: Vec<Vec<(i64, u32)>> = crate::par::run(threads, jobs).into_iter().flatten().collect();
        let mut by_column: FxHashMap<(u32, u32), Vec<usize>> = FxHashMap::default();
        for (i, cols) in columns.iter().enumerate() {
            for &(cx, cz) in cols {
                by_column
                    .entry((cx.rem_euclid(i64::from(c.width)) as u32, cz))
                    .or_default()
                    .push(i);
            }
        }
        RouteMap {
            faces,
            components,
            index,
            columns,
            by_column,
            body,
            voxel_m: c.voxel_m,
        }
    }

    /// The index of `site` among [`RouteMap::faces`], if a body can stand there.
    pub fn face(&self, site: Site) -> Option<usize> {
        self.index.get(&site).copied()
    }

    /// The walkable component a body standing on `site` is in.
    pub fn component_of(&self, site: Site) -> Option<usize> {
        self.face(site).map(|i| self.components[i])
    }

    /// The mouth columns of face `i`, over every heading.
    pub fn mouth_columns(&self, i: usize) -> &[(i64, u32)] {
        &self.columns[i]
    }

    /// Every face whose mouth takes from this foliage layer of `stand`: the diet
    /// accepts the stand's tissue, the layer holds stock, the layer's cell is in the
    /// mouth band from that face, and the model's own per-layer scan
    /// ([`reachable_layers_of`]) offers it to a mouth over that face's columns.
    pub fn faces_reaching_layer(
        &self,
        fv: &FloraView<'_>,
        view: &VoxelView<'_>,
        stand: &Stand,
        layer: &cubarium_voxel_flora::StandLayer,
        diet: Diet,
    ) -> Vec<usize> {
        let mut out = Vec::new();
        if !diet.accepts(fv.config.species(stand.species).trophic) || !(layer.edible() > 0.0) {
            return out;
        }
        let li = layer.foliage_index.unwrap_or(0);
        // Faces already judged for this layer (taken or not): each is judged once, in
        // first-seen order, as the scan of the list so far did.
        let mut judged: rustc_hash::FxHashSet<usize> = rustc_hash::FxHashSet::default();
        for column in layer_columns(view, stand, layer) {
            for &i in self
                .by_column
                .get(&column)
                .map(Vec::as_slice)
                .unwrap_or(&[])
            {
                if !judged.insert(i) {
                    continue;
                }
                let band = mouth_crown_layers_at(self.faces[i].y, &self.body, self.voxel_m);
                if !band.contains(&layer.cell) {
                    continue;
                }
                if reachable_layers_of(fv, view, stand, &self.columns[i], &band)
                    .iter()
                    .any(|(index, _)| *index == li)
                {
                    out.push(i);
                }
            }
        }
        out
    }

    /// Every face whose mouth takes from a ground pool (litter, carrion) at `site`: the
    /// live mouth reads a pool at the standing face's own height under one of its
    /// columns (`body::mouth_pool_site`).
    pub fn faces_reaching_pool(&self, site: Site) -> Vec<usize> {
        self.by_column
            .get(&(site.x, site.z))
            .map(Vec::as_slice)
            .unwrap_or(&[])
            .iter()
            .copied()
            .filter(|&i| self.faces[i].y == site.y)
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::body::{mouth_foliage_stand, mouth_foliage_stands};
    use crate::manifest::Founder;
    use cubarium_voxel::{Command as WorldCommand, Config as VoxelConfig, Material, World};
    use cubarium_voxel_flora::{Command as FloraCommand, Flora, FloraConfig, Species as Plant};

    /// The same flat fixture `body`'s own tests use: 8 × 6 × 6 at 0.25 m, soil in
    /// 1..=2, so the ground is a support face at y = 2.
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

    /// The extracted scan **is** the live mouth: over every face and heading of a small
    /// planted world, the best stand `foliage_stands_in_layers` offers at the mouth's own
    /// layer range is exactly the stand `mouth_foliage_stand` picks, tie-break included.
    #[test]
    fn the_shared_scan_is_the_live_mouths_own_choice() {
        let world = flat_world();
        let view = world.view();
        let mut flora = Flora::new(FloraConfig::default());
        for (x, z, species, wood) in [
            (2i64, 2u32, Plant::Bloomcrown, 0.30),
            (5, 2, Plant::Bloomcrown, 0.12),
            (3, 3, Plant::Springturf, 0.05),
            (6, 4, Plant::Velvetpad, 0.05),
        ] {
            assert!(flora.apply(
                &world,
                FloraCommand::Seed {
                    x,
                    z,
                    species,
                    wood,
                },
            ));
        }
        let fv = flora.view();
        let body = crate::FounderPhysiology::frozen(Founder::Browser).adult_body();
        let layers = mouth_crown_layers_at(2, &body, view.config.voxel_m);
        let mut compared = 0usize;
        let mut hits = 0usize;
        for xi in 0..8 {
            for zi in 0..6 {
                for k in 0..HEADING_SAMPLES {
                    let heading = std::f64::consts::TAU * (k as f64) / (HEADING_SAMPLES as f64);
                    let pose = Pose {
                        x: (f64::from(xi) + 0.5) * 0.25,
                        z: (f64::from(zi) + 0.5) * 0.25,
                        heading_rad: heading,
                        y_m: 0.0,
                    };
                    let cols = mouth_columns_at(&view, &pose, &body);
                    let live = mouth_foliage_stand(&fv, &view, &cols, 2, &body, Diet::only(crate::Food::Foliage));
                    let shared =
                        foliage_stands_in_layers(&fv, &view, &cols, &layers, Diet::only(crate::Food::Foliage));
                    assert_eq!(
                        shared,
                        mouth_foliage_stands(&fv, &view, &cols, 2, &body, Diet::only(crate::Food::Foliage)),
                        "the diagnostic list is the shared scan"
                    );
                    let best =
                        shared
                            .iter()
                            .copied()
                            .fold(None, |best, (site, foliage)| match best {
                                None => Some((site, foliage)),
                                Some((bs, bf)) => {
                                    if foliage > bf || (foliage == bf && site < bs) {
                                        Some((site, foliage))
                                    } else {
                                        Some((bs, bf))
                                    }
                                }
                            });
                    assert_eq!(live, best, "at {pose:?}");
                    compared += 1;
                    hits += usize::from(live.is_some());
                }
            }
        }
        assert_eq!(compared, 8 * 6 * HEADING_SAMPLES);
        assert!(hits > 0, "the fixture has to put some crown in a mouth");
    }

    /// The band is a layer range, and it is the range the arithmetic in the contract
    /// says: two layers on a 0.125 m grid at a 0.25 m ceiling, one on a 0.25 m grid, and
    /// nothing at all for a ceiling of zero.
    #[test]
    fn a_metre_band_selects_the_layers_whose_slabs_it_overlaps() {
        assert_eq!(band_crown_layers(4, 0.125, 0.25), 5..=6);
        assert_eq!(band_crown_layers(4, 0.25, 0.25), 5..=5);
        assert_eq!(band_crown_layers(4, 0.25, 0.30), 5..=6);
        assert!(band_crown_layers(4, 0.25, 0.0).is_empty());
    }

    /// Two faces a column apart on one terrace are one component; a face a layer up is
    /// its own when the lineage cannot climb, and joins when it can.
    #[test]
    fn a_level_step_joins_and_a_rise_needs_the_climb() {
        let faces = [
            Site { x: 0, y: 3, z: 1 },
            Site { x: 1, y: 3, z: 1 },
            Site { x: 2, y: 4, z: 1 },
            Site { x: 7, y: 3, z: 1 },
        ];
        let comp = walkable_components(&faces, 8, 0);
        assert_eq!(comp[0], comp[1], "four-adjacent at one height");
        assert_ne!(comp[1], comp[2], "a rise is not a step without a climb");
        assert_eq!(comp[0], comp[3], "x wraps");
        let comp = walkable_components(&faces, 8, 1);
        assert_eq!(comp[1], comp[2], "one voxel of climb makes the rise a step");
    }

    /// Two terraces a voxel apart are one walkable component under the step rule and
    /// two without it: the observer's route question and the seeder's are the same
    /// question (`design/handoffs/voxel-founder-step-2026-09-22.md`, deliverable 2).
    #[test]
    fn the_step_rule_joins_two_terraces_and_a_climb_of_zero_does_not() {
        // A low shelf at y = 3 in columns 0..4 and a high one at y = 4 in 4..8, on one
        // row; the only way across is the riser between x = 3 and x = 4.
        let mut faces: Vec<Site> = Vec::new();
        for x in 0..4u32 {
            faces.push(Site { x, y: 3, z: 1 });
        }
        for x in 4..8u32 {
            faces.push(Site { x, y: 4, z: 1 });
        }
        let level = walkable_components(&faces, 8, 0);
        assert_eq!(
            level.iter().collect::<std::collections::HashSet<_>>().len(),
            2,
            "level adjacency leaves the two terraces apart"
        );
        let stepped = walkable_components(&faces, 8, 1);
        assert_eq!(
            stepped
                .iter()
                .collect::<std::collections::HashSet<_>>()
                .len(),
            1,
            "one voxel of climb makes the riser a route"
        );
        // Two voxels apart is still two components for a one-voxel climb.
        let mut apart = faces.clone();
        for f in apart.iter_mut().filter(|f| f.y == 4) {
            f.y = 5;
        }
        assert_eq!(
            walkable_components(&apart, 8, 1)
                .iter()
                .collect::<std::collections::HashSet<_>>()
                .len(),
            2,
            "a two-voxel riser is not a step for a one-voxel climb"
        );
    }
}
