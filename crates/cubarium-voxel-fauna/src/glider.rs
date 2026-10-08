//! **Gliders** (F3, the seedporter; `design/predation-and-flight-2026-09-28.md` §3.5, §3.7,
//! §3.9; Brief F3 in `design/handoffs/voxel-roster-2026-10-01.md`): a walker that climbs
//! the stems of tall stands and latticevine-covered faces, perches in crowns, glides down
//! from height, eats fruit and carries the seeds in its gut. A child of `step` (it shares
//! the tick's private helpers); `flight` answers its questions of the air.
//!
//! The modes ([`MobilityMode`]) and what drives each:
//!
//! - **`Ground`**: the walker's own act ([`founder_act`], fruit aside). Walking into the
//!   stem of a stand at least `climb_min_stand_m` tall (its centre within its radius and
//!   half a voxel of the stem, the stem ahead) starts a **stem climb**; blocked against a
//!   wall with a covered face in its body's rows ahead, a **vine climb**; blocked at an
//!   edge with at least `glide_min_height_m` of drop beyond it, a **glide** (the body is
//!   put just past the edge, clear of it).
//! - **`Climb`**: on a stand's stem ([`PerchAnchor::Stand`]) or a covered face
//!   ([`PerchAnchor::Vine`]), held in [`Mobility::perch`]. Up at `climb_speed_fraction` ×
//!   the walking pace at the held forward effort, costing `wall_climb_cost_factor` × the
//!   walking cost (the shredder's wall rule); [`Mobility::falling`] marks a climb **down**
//!   (full effort, the same cost). A stem climb ends perched on the crown's top layer
//!   (`CrownTop`), or in a downpour under its lowest layer (`Underside`, sheltered); a
//!   vine climb ends perched on the face where the cover stops (`VineFace`); a climb
//!   down ends on the ground.
//! - **`Perched`**: basal upkeep only; it may turn (on a crown) and eat. Forward effort
//!   glides off when the floor below is at least `glide_min_height_m` down (off a vine
//!   face, away from the wall), else climbs: down from a crown top or a vine, on up from
//!   under a crown. A downpour sends a crown-top perch down under its crown. A lost perch
//!   (the crown eaten or grown, the face uncovered) is a forced glide.
//! - **`Glide`**: no thrust; `glide_speed_fraction` × its ground cruise a second across,
//!   steered by `turn`, falling `1 / glide_ratio` of that; `glide_cost_factor` × basal in
//!   all (the upkeep included, F1's hover convention). It lands on the first crown top of
//!   a stand it could climb that it crosses at its centre column (perched), the floor of
//!   its centre column (walking), or a covered face it is pressed against (climbing).
//!
//! **Fruit** (§3.7): on the due tick with `feed`, the sated bite from the stand it holds
//! (its stem or crown) or else the nearest fruit in the mouth's reach — a fruiting crown
//! cell ([`Flora::take_stand_fruit`]) or a latticevine face in fruit
//! ([`Flora::take_fruit`]) — at fruit's yield. `seeds_per_fruit_organic` of the bite (all
//! three currencies, pro rata) goes into a **gut seed lot** instead of the gut; the lot is
//! voided at the body's support face no sooner than `gut_transit_s` after its last meal:
//! a stand's seeds into that face's seed bank ([`Flora::deposit_seeds`]), a latticevine's
//! into its vine seed bank ([`Flora::deposit_vine_seeds`]), which founds a vine at the
//! site's bank check where one can root.
//!
//! **A glider sleeps only perched** (step's `circadian`), and the roost reflex
//! ([`roost_reflex`]) takes a heuristic-driven one to a crown at its rest hour and in a
//! downpour.

use super::*;

use crate::flight::{self, EPS, Shape};
use crate::{Bloom, Glider, GutSeedLot, MobilityMode, Perch, PerchAnchor, PerchKind, SeedPlant, SeedVoid};
use cubarium_voxel_flora::{Face as VineFace, FaceDir, FloraView, StandLayer};

/// How near the wall a pressed body's centre is beyond its radius, metres, for a vine
/// climb to start: a sweep stops within float dust of it.
const PRESSED_M: f64 = 0.02;

/// A glider's collision body: its footprint disc (half its width) and its height.
pub(crate) fn shape(phys: &crate::FounderPhysiology, body: f64) -> Shape {
    let g = phys.body_at(body);
    Shape {
        r: g.footprint_radius(),
        h: g.height_m.max(1e-6),
    }
}

/// Whether a stand's stem is one a glider climbs: foliage on it and a crown at least
/// `climb_min_stand_m` tall.
fn climbable(fv: &FloraView<'_>, stand: &cubarium_voxel_flora::Stand, glider: &Glider) -> bool {
    stand.foliage > 0.0
        && fv.config.species(stand.species).crown_height_m_at(stand.wood) >= glider.climb_min_stand_m - EPS
}

/// A stand's foliage layers holding foliage, lowest and highest.
/// The face's plant, or its fungus where it holds no plant (`FaceLayer`).
fn crown_span(fv: &FloraView<'_>, site: Site) -> Option<(StandLayer, StandLayer)> {
    let stand = fv.stands_on(site).first()?;
    let layers: Vec<StandLayer> = fv
        .profile_layers(stand)
        .into_iter()
        .filter(|l| l.kind.bears_foliage() && l.stock > 0.0)
        .collect();
    let low = *layers.iter().min_by_key(|l| l.cell)?;
    let high = *layers.iter().max_by_key(|l| l.cell)?;
    Some((low, high))
}

/// The top of the support face under `(x, z)` at or below `y_m`, and that face.
fn floor_at(view: &VoxelView<'_>, x: f64, z: f64, y_m: f64) -> Option<(f64, Site)> {
    let site = flight::support_below(view, x, z, y_m + EPS)?;
    Some((crate::surface_m(site.y, view.config.voxel_m), site))
}

/// **The fruit a glider's mouth reaches** (F3). A stand's fruit hangs in its crown: it is
/// reached only from the stand the body holds — all of it perched on the crown, and while
/// climbing the stem a fruiting crown cell within its radius plus its mouth reach of its
/// middle. A vine face in fruit is reached from anywhere within that reach (the beads hang
/// on the wall down to its foot). `(the source, its stock)`.
pub(crate) fn glider_fruit_in_reach(
    view: &VoxelView<'_>,
    fv: &FloraView<'_>,
    a: &Animal,
    phys: &crate::FounderPhysiology,
) -> Option<(Bloom, f64)> {
    let held = match (a.mobility.mode, a.mobility.perch) {
        (
            MobilityMode::Perched | MobilityMode::Climb,
            Some(Perch {
                anchor: PerchAnchor::Stand { site, .. },
                ..
            }),
        ) => Some(site),
        _ => None,
    };
    if a.mobility.mode == MobilityMode::Perched
        && let Some(site) = held
        && let Some(stand) = fv.stand_at(site)
        && stand.fruit > 0.0
    {
        return Some((Bloom::Stand(site), stand.fruit));
    }
    let s = shape(phys, a.body);
    let reach = s.r + phys.body_at(a.body).mouth_reach_m;
    flight::fruit_in_reach(view, fv, flight::mouth_point(a, s), reach, held.map(Some))
}

/// The nearest climbable stand the body walks in under: a stand rooted within a voxel of
/// its standing layer whose stem is within the body's radius and half a voxel of its
/// centre, or under the stand's lowest foliage layer (its radius), and not behind it. It
/// climbs into the crown from there.
fn stem_touched(view: &VoxelView<'_>, fv: &FloraView<'_>, a: &Animal, shape: Shape, glider: &Glider) -> Option<Site> {
    let c = view.config;
    let v = c.voxel_m;
    let ring = f64::from(c.width) * v;
    let (fx, fz) = a.pose.forward();
    let (cx, cz) = a.pose.column(v, c.depth)?;
    let mut best: Option<(f64, Site)> = None;
    let mut consider = |stand: &cubarium_voxel_flora::Stand| {
        if stand.site.y.abs_diff(a.site.y) > 1 || !climbable(fv, stand, glider) {
            return;
        }
        let Some((low, _)) = crown_span(fv, stand.site) else {
            return;
        };
        let reach = (shape.r + 0.5 * v).max(low.radius_m);
        let sx = (f64::from(stand.site.x) + 0.5) * v;
        let sz = (f64::from(stand.site.z) + 0.5) * v;
        let (dx, dz) = crate::pose::wrapped_delta(a.pose.x, a.pose.z, sx, sz, ring);
        let d = (dx * dx + dz * dz).sqrt();
        if d > reach || dx * fx + dz * fz < -EPS {
            return;
        }
        if best.is_none_or(|(b, s)| d < b || (d == b && stand.site < s)) {
            best = Some((d, stand.site));
        }
    };
    match fv.stands_near(cx.rem_euclid(i64::from(c.width)), cz, c.width, c.depth) {
        Some(near) => {
            for &k in near {
                consider(&fv.stands[k as usize]);
            }
        }
        None => {
            for s in fv.stands {
                consider(s);
            }
        }
    }
    best.map(|(_, s)| s)
}

/// The covered face of the wall the body is pressed against along its heading's axis, in
/// the rows its body spans: the lowest one.
fn vine_ahead(view: &VoxelView<'_>, fv: &FloraView<'_>, a: &Animal, shape: Shape) -> Option<VineFace> {
    if fv.cover.face_count() == 0 {
        return None;
    }
    let c = view.config;
    let v = c.voxel_m;
    let (cx, cz) = a.pose.column(v, c.depth)?;
    let (dir, _) = axis_of(a.pose.heading_rad);
    let (wx, wz) = (cx + dir.0, i64::from(cz) + dir.1);
    if wz < 0 || wz >= i64::from(c.depth) {
        return None;
    }
    let wx = wx.rem_euclid(i64::from(c.width));
    // Pressed: the centre within its radius of the plane it faces.
    let s = if dir.0 != 0 { dir.0 } else { dir.1 };
    let col = if dir.0 != 0 { cx } else { i64::from(cz) };
    let plane = beside_plane(col, s, v, 0.0, 0.0);
    let along = if dir.0 != 0 { a.pose.x } else { a.pose.z };
    let gap = if dir.0 != 0 {
        let w = f64::from(c.width) * v;
        let d = (plane - along).rem_euclid(w);
        d.min(w - d)
    } else {
        (plane - along).abs()
    };
    if gap > shape.r + PRESSED_M {
        return None;
    }
    let face_dir = match dir {
        (1, 0) => FaceDir::NegX,
        (-1, 0) => FaceDir::PosX,
        (0, 1) => FaceDir::NegZ,
        _ => FaceDir::PosZ,
    };
    let (lo, hi) = body_rows(a.pose.y_m, shape, v);
    (lo..=hi).find_map(|y| {
        if y < 0 || y >= i64::from(c.height) || !view.is_solid(wx, y as u32, wz as u32) {
            return None;
        }
        let face = VineFace::new(wx as u32, y as u32, wz as u32, face_dir);
        fv.cover.face(face).map(|_| face)
    })
}

/// The cell rows a body standing on `base` spans.
fn body_rows(base: f64, shape: Shape, v: f64) -> (i64, i64) {
    let lo = flight::row_at(base, v);
    let hi = (((base + shape.h - EPS) / v).floor() as i64).max(lo);
    (lo, hi)
}

/// The covered face of `face`'s wall nearest the body's middle in the rows it spans, if
/// any: what a vine climber holds.
fn vine_hold(view: &VoxelView<'_>, fv: &FloraView<'_>, face: VineFace, base: f64, shape: Shape) -> Option<VineFace> {
    let v = view.config.voxel_m;
    let (lo, hi) = body_rows(base, shape, v);
    let mid = ((base + 0.5 * shape.h) / v).floor() as i64;
    let mut best: Option<(i64, VineFace)> = None;
    for y in lo..=hi {
        if y < 0 || y >= i64::from(view.config.height) || !view.is_solid(i64::from(face.x), y as u32, face.z) {
            continue;
        }
        let f = VineFace::new(face.x, y as u32, face.z, face.dir);
        if fv.cover.face(f).is_some() && best.is_none_or(|(d, _)| (y - mid).abs() < d) {
            best = Some(((y - mid).abs(), f));
        }
    }
    best.map(|(_, f)| f)
}

/// Whether the column under `(x, z)` is clear of solid over a body standing on `base`.
fn room_over(view: &VoxelView<'_>, x: f64, z: f64, base: f64, shape: Shape) -> bool {
    let c = view.config;
    let v = c.voxel_m;
    let cz = (z / v).floor();
    if !(0.0..f64::from(c.depth)).contains(&cz) {
        return false;
    }
    let cx = ((x / v).floor() as i64).rem_euclid(i64::from(c.width));
    let (lo, hi) = body_rows(base, shape, v);
    if lo < 0 || hi >= i64::from(c.height) {
        return false;
    }
    !view.solid_in(cx, cz as u32, lo as u32, hi as u32)
}

/// The heading pointing out of a wall face, away from it.
fn away_from(face: VineFace) -> f64 {
    match face.dir {
        FaceDir::NegX => heading_of((-1, 0)),
        FaceDir::PosX => heading_of((1, 0)),
        FaceDir::NegZ => heading_of((0, -1)),
        _ => heading_of((0, 1)),
    }
}

/// **Where a glide off a covered face starts** (PACK-TUNE): the body moved straight out from
/// the face until its footprint clears the wall by [`PRESSED_M`], heading away from it; `None`
/// when that point is itself blocked (the body stays and climbs down).
fn off_face(view: &VoxelView<'_>, s: Shape, a: &Animal, face: VineFace) -> Option<crate::Pose> {
    let v = view.config.voxel_m;
    let ring = f64::from(view.config.width) * v;
    let want = s.r + PRESSED_M;
    let (mut x, mut z) = (a.pose.x, a.pose.z);
    match face.dir {
        FaceDir::NegX => {
            let wall = face.x as f64 * v;
            let gap = crate::pose::wrapped_delta(x, 0.0, wall, 0.0, ring).0;
            x = (x - (want - gap).max(0.0)).rem_euclid(ring);
        }
        FaceDir::PosX => {
            let wall = (face.x + 1) as f64 * v;
            let gap = -crate::pose::wrapped_delta(x, 0.0, wall, 0.0, ring).0;
            x = (x + (want - gap).max(0.0)).rem_euclid(ring);
        }
        FaceDir::NegZ => z = z.min(face.z as f64 * v - want),
        _ => z = z.max((face.z + 1) as f64 * v + want),
    }
    let from = crate::Pose { x, z, heading_rad: away_from(face), ..a.pose };
    (!flight::blocked(view, s, x, z, a.pose.y_m)).then_some(from)
}

/// Whether two covered faces are on the same wall column and side (any height).
fn same_wall(f: VineFace, g: VineFace) -> bool {
    f.x == g.x && f.z == g.z && f.dir == g.dir
}

/// The heading pointing into a wall face.
fn into_face(face: VineFace) -> f64 {
    (away_from(face) + std::f64::consts::PI).rem_euclid(std::f64::consts::TAU)
}

fn set_mode(a: &mut Animal, mode: MobilityMode, perch: Option<Perch>, down: bool) {
    a.mobility.mode = mode;
    a.mobility.perch = perch;
    a.mobility.falling = down;
    a.mobility.climb = None;
}

fn start_glide(fauna: &mut Fauna, i: usize, forced: bool) {
    set_mode(&mut fauna.animals[i], MobilityMode::Glide, None, false);
    if forced {
        fauna.ledger.glides_forced += 1;
    } else {
        fauna.ledger.glides += 1;
    }
}

fn stand_perch(kind: PerchKind, site: Site, layer: &StandLayer) -> Perch {
    Perch {
        kind,
        anchor: PerchAnchor::Stand {
            site,
            layer: flight::foliage_layer_index(layer),
        },
    }
}

/// **One glider's tick** (F3), in place of a walker's act. The module doc has the rules.
#[allow(clippy::too_many_arguments)]
pub(super) fn glider_act(
    fauna: &mut Fauna,
    view: &VoxelView<'_>,
    flora: &mut Flora,
    seed: u64,
    tick: u64,
    sky: &Sky,
    hits: &mut Vec<Hit>,
    glider: Glider,
    i: usize,
) {
    let founder = fauna.animals[i].founder.expect("a founder body");
    let asleep = fauna.animals[i].state == State::Asleep;
    let mode = fauna.animals[i].mobility.mode;
    if mode == MobilityMode::Ground {
        if asleep {
            fauna.ledger.founder_ticks_by_founder[founder.index()] += 1;
            crate::gait::rest(fauna, i, founder);
            return;
        }
        ground_tick(fauna, view, flora, seed, tick, glider, i, hits);
        return;
    }
    fauna.ledger.founder_ticks_by_founder[founder.index()] += 1;
    // GAIT: off its feet it does not sprint, and rests.
    crate::gait::rest(fauna, i, founder);
    let phys = *fauna.config.founder(founder);
    let manifest = fauna.config.manifest(founder);
    let due = fauna.animals[i].age_ticks % founder.manifest().cadence_ticks() == 0;
    let mut held = fauna.animals[i].founder_state.held;
    if mode == MobilityMode::Perched && !asleep && phys.catch_bellwings {
        if let Some(h) = phys.active_hunter() {
            match fauna.animals[i].strike {
                Some(s) if s.phase != super::StrikePhase::Recovery => {
                    super::strike_tick(fauna, i, view, &phys, h, seed, tick, hits);
                    return;
                }
                Some(mut s) => {
                    s.ticks_left = s.ticks_left.saturating_sub(1);
                    fauna.animals[i].strike = (s.ticks_left > 0).then_some(s);
                }
                None => {}
            }
            let g = phys.body_at(fauna.animals[i].body);
            if due
                && held.feed > 0.0
                && fauna.animals[i].eating
                && fauna.animals[i].strike.is_none()
            {
                if let Some(prey) = super::prey_in_capsule(fauna, i, view, &g, &h) {
                    super::start_strike(fauna, i, prey);
                    return;
                }
            }
        }
    }
    {
        let pc = &fauna.config.predation;
        held.forward *= (1.0 - pc.wound_slow * fauna.animals[i].wound).clamp(0.0, 1.0);
    }
    match mode {
        MobilityMode::Glide => glide_tick(fauna, view, flora, &phys, &manifest, glider, held, i),
        MobilityMode::Climb => climb_tick(fauna, view, flora, sky, &phys, &manifest, held, due, i),
        MobilityMode::Perched => perched_tick(fauna, view, flora, sky, &phys, &manifest, glider, held, due, asleep, i),
        // A glider has no other mode; anything else walks.
        _ => set_mode(&mut fauna.animals[i], MobilityMode::Ground, None, false),
    }
    // Off the ground its support face is the one below it.
    let a = &mut fauna.animals[i];
    if a.mobility.mode != MobilityMode::Ground
        && let Some(site) = flight::support_below(view, a.pose.x, a.pose.z, a.pose.y_m + EPS)
    {
        a.site = site;
    }
}

/// On the ground: the walker's act, then a stem, a vine or an edge.
#[allow(clippy::too_many_arguments)]
fn ground_tick(
    fauna: &mut Fauna,
    view: &VoxelView<'_>,
    flora: &mut Flora,
    seed: u64,
    tick: u64,
    glider: Glider,
    i: usize,
    hits: &mut Vec<Hit>,
) {
    let founder = fauna.animals[i].founder.expect("a founder body");
    let phys = *fauna.config.founder(founder);
    let manifest = fauna.config.manifest(founder);
    let due = fauna.animals[i].age_ticks % founder.manifest().cadence_ticks() == 0;
    let held = fauna.animals[i].founder_state.held;
    let before = fauna.animals[i].pose;
    // Fruit remains this module's bite; only a reachable catch or carcass goes through
    // the walker's existing strike/meal machinery.
    let geometry = phys.body_at(fauna.animals[i].body);
    let catch = phys.catch_bellwings
        && phys.active_hunter().is_some_and(|h| {
            super::prey_in_capsule(fauna, i, view, &geometry, &h).is_some()
                || crate::hunt::mouth_on_carcass(&flora.view(), view, &fauna.animals[i], &geometry)
        });
    fauna.animals[i].founder_state.held.feed = if catch { held.feed } else { 0.0};
    founder_act(fauna, i, view, flora, seed, tick, hits);
    fauna.animals[i].founder_state.held.feed = held.feed;
    if fauna.animals[i]
        .strike
        .is_some_and(|s| s.phase != super::StrikePhase::Recovery)
    {
        return;
    }
    let v = view.config.voxel_m;
    {
        let a = &mut fauna.animals[i];
        a.pose.y_m = crate::surface_m(a.site.y, v);
    }
    if held.forward > 0.0 {
        let fv = flora.view();
        let a = fauna.animals[i];
        let s = shape(&phys, a.body);
        if let Some(site) = stem_touched(view, &fv, &a, s, &glider) {
            let Some((_, high)) = crown_span(&fv, site) else {
                return;
            };
            set_mode(
                &mut fauna.animals[i],
                MobilityMode::Climb,
                Some(stand_perch(PerchKind::CrownTop, site, &high)),
                false,
            );
            fauna.animals[i].state = State::Climbing;
            fauna.ledger.climbs += 1;
            return;
        }
        let ring = f64::from(view.config.width) * v;
        let (dx, dz) = crate::pose::wrapped_delta(before.x, before.z, a.pose.x, a.pose.z, ring);
        // At most cruise: a sprint the gate refused (winded) is not a blocked walk.
        let asked = held.forward.min(1.0) * manifest.cruise_m_per_s * DT;
        if (dx * dx + dz * dz).sqrt() < 0.5 * asked {
            if let Some(face) = vine_ahead(view, &fv, &a, s) {
                let a = &mut fauna.animals[i];
                a.pose.heading_rad = into_face(face);
                set_mode(
                    a,
                    MobilityMode::Climb,
                    Some(Perch {
                        kind: PerchKind::VineFace,
                        anchor: PerchAnchor::Vine(face),
                    }),
                    false,
                );
                a.state = State::Climbing;
                fauna.ledger.climbs += 1;
                return;
            }
            if let Some((x, z)) = glide_off_edge(view, &fv, &a, s, &glider, &phys) {
                let a = &mut fauna.animals[i];
                a.pose.x = x;
                a.pose.z = z;
                start_glide(fauna, i, false);
                fauna.animals[i].state = State::Walking;
                return;
            }
        }
    }
    if due && held.feed > 0.0 && glider_feed(fauna, i, view, flora, &manifest, &phys, held.feed) {
        fauna.animals[i].state = State::Cropping;
    }
}

/// Where a glide off the edge ahead starts: the first point along the heading, up to two
/// radii and a voxel on, where the disc is clear and every floor under it is at least
/// `glide_min_height_m` below the base. `None` when there is no such drop, or the glide
/// from there would end in water deeper than it wades ([`glide_ends_deep`]; roster
/// mechanics fix 1: the end cell decides, not the water it starts out over).
#[allow(clippy::too_many_arguments)]
fn glide_off_edge(
    view: &VoxelView<'_>,
    fv: &FloraView<'_>,
    a: &Animal,
    shape: Shape,
    glider: &Glider,
    phys: &crate::FounderPhysiology,
) -> Option<(f64, f64)> {
    let v = view.config.voxel_m;
    let base = a.pose.y_m;
    let (fx, fz) = a.pose.forward();
    let ring = f64::from(view.config.width) * v;
    let n = (((2.0 * shape.r + v) / (0.125 * v)).ceil() as usize).max(1);
    for k in 1..=n {
        let d = k as f64 * 0.125 * v;
        let (x, z) = ((a.pose.x + fx * d).rem_euclid(ring), a.pose.z + fz * d);
        if flight::blocked(view, shape, x, z, base) {
            return None;
        }
        let Some((top, _, water)) = flight::floor_under(view, shape, x, z, base) else {
            continue;
        };
        if base - (top - water) >= glider.glide_min_height_m - EPS {
            let from = crate::Pose { x, z, ..a.pose };
            return (!glide_ends_deep(view, fv, glider, phys, a.body, from)).then_some((x, z));
        }
    }
    None
}

/// Where a glide comes down ([`glide_motion`]).
#[derive(Clone, Copy, Debug)]
enum GlideEnd {
    /// On the top of a climbable stand's crown layer: `(the stand, the layer)`.
    Crown(Site, StandLayer),
    /// On the floor of its centre column: the face.
    Floor(Site),
    /// Against a covered wall face.
    Vine(VineFace),
}

/// **One gliding tick's motion** on `pose` (its heading already turned): `distance`
/// along the heading through the air, then `fall` down its centre column onto the first
/// crown top of a climbable stand, the floor, or a covered face it is pressed against.
/// The metres it covered and where it came down, if it did. Pure: the glide itself and its
/// prediction ([`glide_end`]) are this one rule.
#[allow(clippy::too_many_arguments)]
fn glide_motion(
    view: &VoxelView<'_>,
    fv: &FloraView<'_>,
    glider: &Glider,
    s: Shape,
    puddle: f64,
    distance: f64,
    fall: f64,
    pose: &mut crate::Pose,
) -> (f64, Option<GlideEnd>) {
    let v = view.config.voxel_m;
    let moved = flight::move_through_air(view, fv, s, f64::INFINITY, puddle, pose, distance, 0.0);
    // The descent, against its centre column.
    let y0 = pose.y_m;
    let floor = floor_at(view, pose.x, pose.z, y0);
    let floor_top = floor.map_or(0.0, |(t, _)| t);
    let y = (y0 - fall).max(floor_top);
    let cz = (pose.z / v).floor().clamp(0.0, f64::from(view.config.depth) - 1.0) as u32;
    let cx = (pose.x / v).floor() as i64;
    let crown = flight::crown_layers_at(view, fv, cx, cz)
        .into_iter()
        .filter(|(site, _)| fv.stand_at(*site).is_some_and(|st| climbable(fv, st, glider)))
        .map(|(site, l)| ((l.cell + 1) as f64 * v, site, l))
        .filter(|&(top, _, _)| top < y0 - EPS && top >= y - EPS && top >= floor_top - EPS)
        .max_by(|p, q| p.0.total_cmp(&q.0));
    if let Some((top, site, l)) = crown {
        pose.y_m = top;
        return (moved.distance_m, Some(GlideEnd::Crown(site, l)));
    }
    if let Some((top, site)) = floor.filter(|&(t, _)| y <= t + EPS) {
        pose.y_m = top;
        return (moved.distance_m, Some(GlideEnd::Floor(site)));
    }
    pose.y_m = y;
    let end = flight::vine_contact(view, fv, s, pose.x, pose.z, y).map(GlideEnd::Vine);
    (moved.distance_m, end)
}

/// A glide's per-tick `(distance, fall)` for a lineage cruising at `cruise_m_per_s` (GAIT:
/// `glide_speed_fraction` × the ground cruise, the walk's own reference at every size).
fn glide_pace(glider: &Glider, cruise_m_per_s: f64) -> (f64, f64) {
    let distance = glider.glide_speed_fraction * cruise_m_per_s * DT;
    let fall = if glider.glide_ratio > 0.0 {
        distance / glider.glide_ratio
    } else {
        distance
    };
    (distance, fall)
}

/// **Where a glide from `from` comes down** if its heading holds: [`glide_motion`] tick by
/// tick until it lands. `None` when it never does (a bottomless column).
fn glide_end(
    view: &VoxelView<'_>,
    fv: &FloraView<'_>,
    glider: &Glider,
    phys: &crate::FounderPhysiology,
    body: f64,
    mut from: crate::Pose,
) -> Option<GlideEnd> {
    let s = shape(phys, body);
    let puddle = phys.drown_depth_m(&phys.body_at(body));
    let (distance, fall) = glide_pace(glider, phys.cruise_body_lengths_per_s * phys.adult_length_m);
    if !(fall > 0.0) {
        return None;
    }
    // It falls `fall` a tick until it meets something: never more ticks than its height.
    let cap = (from.y_m.max(0.0) / fall).ceil() as usize + 2;
    for _ in 0..cap {
        if let (_, Some(end)) = glide_motion(view, fv, glider, s, puddle, distance, fall, &mut from) {
            return Some(end);
        }
    }
    None
}

/// **Whether a glide from `from` ends in water deeper than the body wades** (roster
/// mechanics fix 1): only its end cell decides — the floor it comes down on — not the water
/// it crosses. A crown or a covered face is dry.
fn glide_ends_deep(
    view: &VoxelView<'_>,
    fv: &FloraView<'_>,
    glider: &Glider,
    phys: &crate::FounderPhysiology,
    body: f64,
    from: crate::Pose,
) -> bool {
    let wade = phys.wade_depth_m(&phys.body_at(body));
    match glide_end(view, fv, glider, phys, body, from) {
        Some(GlideEnd::Floor(site)) => view.water_depth_m(i64::from(site.x), site.y, site.z) > wade,
        _ => false,
    }
}

/// The heading a glider in the air steers by (roster mechanics fix 1): the one it asks for
/// when that glide ends dry; else the one it holds when that does; else, of 16 headings
/// round the compass, the dry one nearest what it asked for, turned toward at its yaw cap;
/// with none, what it asked for.
#[allow(clippy::too_many_arguments)]
fn steer_dry(
    view: &VoxelView<'_>,
    fv: &FloraView<'_>,
    glider: &Glider,
    phys: &crate::FounderPhysiology,
    a: &Animal,
    asked: f64,
    yaw_step: f64,
) -> f64 {
    let dry = |h: f64| !glide_ends_deep(view, fv, glider, phys, a.body, crate::Pose { heading_rad: h, ..a.pose });
    if dry(asked) {
        return asked;
    }
    let held = a.pose.heading_rad;
    if dry(held) {
        return held;
    }
    let tau = std::f64::consts::TAU;
    let gap = |h: f64| {
        let d = (h - asked).rem_euclid(tau);
        d.min(tau - d)
    };
    let mut best: Option<f64> = None;
    for k in 0..16 {
        let h = f64::from(k) * tau / 16.0;
        if dry(h) && best.is_none_or(|b| gap(h) < gap(b)) {
            best = Some(h);
        }
    }
    let Some(target) = best else {
        return asked;
    };
    let d = (target - held + std::f64::consts::PI).rem_euclid(tau) - std::f64::consts::PI;
    (held + d.clamp(-yaw_step, yaw_step)).rem_euclid(tau)
}

/// One gliding tick.
#[allow(clippy::too_many_arguments)]
fn glide_tick(
    fauna: &mut Fauna,
    view: &VoxelView<'_>,
    flora: &mut Flora,
    phys: &crate::FounderPhysiology,
    manifest: &crate::Manifest,
    glider: Glider,
    held: Actions,
    i: usize,
) {
    let v = view.config.voxel_m;
    // An introduced glider's perch is resolved on its first tick: a crown top its base
    // rests on at its centre column holds it.
    if fauna.animals[i].mobility.falling {
        fauna.animals[i].mobility.falling = false;
        let fv = flora.view();
        let a = fauna.animals[i];
        let cz = (a.pose.z / v).floor().clamp(0.0, f64::from(view.config.depth) - 1.0) as u32;
        let rest = flight::crown_layers_at(view, &fv, (a.pose.x / v).floor() as i64, cz)
            .into_iter()
            .filter(|(site, _)| fv.stand_at(*site).is_some_and(|st| climbable(&fv, st, &glider)))
            .find(|(_, l)| (((l.cell + 1) as f64 * v) - a.pose.y_m).abs() <= 1e-6);
        if let Some((site, l)) = rest {
            let a = &mut fauna.animals[i];
            set_mode(a, MobilityMode::Perched, Some(stand_perch(PerchKind::CrownTop, site, &l)), false);
            a.state = State::Resting;
            return;
        }
    }
    let basal = phys.core.maintenance_per_s * fauna.animals[i].body;
    let cost = (glider.glide_cost_factor - 1.0).max(0.0) * basal * DT;
    let (paid, body) = respire(fauna, i, cost, Respiration::Motor);
    fauna.ledger.respired_flight_out += paid;
    let s = shape(phys, fauna.animals[i].body);
    let (distance, fall) = glide_pace(&glider, manifest.cruise_m_per_s);
    let puddle = phys.drown_depth_m(&phys.body_at(fauna.animals[i].body));
    let fv = flora.view();
    let asked = (fauna.animals[i].pose.heading_rad + held.turn * manifest.yaw_cap_rad_per_s * DT)
        .rem_euclid(std::f64::consts::TAU);
    // It steers clear of an end in water deeper than it wades (roster mechanics fix 1).
    let heading = steer_dry(view, &fv, &glider, phys, &fauna.animals[i], asked, manifest.yaw_cap_rad_per_s * DT);
    let a = &mut fauna.animals[i];
    a.pose.heading_rad = heading;
    let (moved, end) = glide_motion(view, &fv, &glider, s, puddle, distance, fall, &mut a.pose);
    let landed = end.is_some();
    match end {
        Some(GlideEnd::Crown(site, l)) => {
            set_mode(a, MobilityMode::Perched, Some(stand_perch(PerchKind::CrownTop, site, &l)), false);
            fauna.ledger.landings_by_perch[PerchKind::CrownTop.index()] += 1;
        }
        Some(GlideEnd::Floor(site)) => {
            a.site = site;
            set_mode(a, MobilityMode::Ground, None, false);
            settle(view, a, phys);
            fauna.ledger.landings_by_perch[PerchKind::Face.index()] += 1;
        }
        Some(GlideEnd::Vine(face)) => {
            a.pose.heading_rad = into_face(face);
            set_mode(
                a,
                MobilityMode::Climb,
                Some(Perch {
                    kind: PerchKind::VineFace,
                    anchor: PerchAnchor::Vine(face),
                }),
                false,
            );
            fauna.ledger.landings_by_perch[PerchKind::VineFace.index()] += 1;
        }
        None => {}
    }
    let fb = &mut a.founder_state.feedback;
    fb.attempted_equivalent += distance;
    fb.delivered_equivalent += moved;
    fb.delivered_forward += moved;
    fb.motor_respiration += paid;
    fb.structural_loss += body;
    a.state = if landed && a.mobility.mode == MobilityMode::Perched {
        State::Resting
    } else {
        State::Walking
    };
}

/// A glider come down on the ground stands where a walker can: when its footprint overlaps
/// solid over its face (it came down beside a ledge or in a slot), it steps to the nearest
/// point within two radii, in eight directions, where the footprint is clear over a face
/// at the same height; with none it stays (a walker's own rules then apply).
fn settle(view: &VoxelView<'_>, a: &mut Animal, phys: &crate::FounderPhysiology) {
    let c = view.config;
    let v = c.voxel_m;
    let g = phys.body_at(a.body);
    let r = g.footprint_radius();
    let headroom = g.headroom_voxels(v);
    let y = a.site.y;
    let rows = y + 1..=(y + headroom).min(c.height - 1);
    if body::disc_clear(view, a.pose.x, a.pose.z, r, rows.clone()) {
        return;
    }
    let ring = f64::from(c.width) * v;
    for k in 1..=8 {
        let d = 0.25 * r * f64::from(k);
        for j in 0..8 {
            let t = f64::from(j) * std::f64::consts::FRAC_PI_4;
            let (x, z) = ((a.pose.x + d * t.sin()).rem_euclid(ring), a.pose.z + d * t.cos());
            if !(z > r && z < f64::from(c.depth) * v - r) {
                continue;
            }
            let Some((top, site)) = floor_at(view, x, z, a.pose.y_m) else {
                continue;
            };
            if site.y == y && body::disc_clear(view, x, z, r, rows.clone()) {
                a.pose.x = x;
                a.pose.z = z;
                a.pose.y_m = top;
                a.site = site;
                return;
            }
        }
    }
}

/// One climbing tick: up or down a stem or a vine face.
#[allow(clippy::too_many_arguments)]
fn climb_tick(
    fauna: &mut Fauna,
    view: &VoxelView<'_>,
    flora: &mut Flora,
    sky: &Sky,
    phys: &crate::FounderPhysiology,
    manifest: &crate::Manifest,
    held: Actions,
    due: bool,
    i: usize,
) {
    let v = view.config.voxel_m;
    let down = fauna.animals[i].mobility.falling;
    let effort = if down { 1.0 } else { held.forward.clamp(0.0, 1.0) };
    let s = shape(phys, fauna.animals[i].body);
    let Some(perch) = fauna.animals[i].mobility.perch else {
        start_glide(fauna, i, true);
        return;
    };
    // The hold first: a stem with no crown left, or a vine with no cover in reach, lets go.
    let fv = flora.view();
    let a = fauna.animals[i];
    let hold = match perch.anchor {
        PerchAnchor::Stand { site, .. } => crown_span(&fv, site).map(|span| (Some((site, span)), None)),
        PerchAnchor::Vine(face) => {
            let f = vine_hold(view, &fv, face, a.pose.y_m, s);
            // Going down it may slide below the cover to the ground.
            if f.is_some() || down { Some((None, f.or(Some(face)))) } else { None }
        }
        _ => None,
    };
    let Some(hold) = hold else {
        start_glide(fauna, i, true);
        return;
    };
    // Metres along the stem or face this tick: a body holding on without going anywhere
    // (stopped to eat, or sated at a fruiting crown) is not climbing (LOAD-CLIMB).
    let mut went = 0.0;
    if effort > 0.0 {
        let cost = phys.motor_respiration_per_s * fauna.animals[i].body * effort * phys.wall_climb_cost_factor * DT;
        let (paid, body) = respire(fauna, i, cost, Respiration::Motor);
        let rate = phys.climb_speed_fraction * effort * manifest.cruise_m_per_s;
        let step = rate * DT;
        let y0 = fauna.animals[i].pose.y_m;
        let floor = floor_at(view, a.pose.x, a.pose.z, y0).map_or(0.0, |(t, _)| t);
        let fv = flora.view();
        let a = &mut fauna.animals[i];
        let mut moved = 0.0;
        match hold {
            (Some((site, (low, high))), _) => {
                let top = (high.cell + 1) as f64 * v;
                let under = low.cell as f64 * v - s.h;
                if down {
                    let shelter = sky.downpour && y0 > under + EPS && under >= floor - EPS;
                    let target = if shelter { under } else { floor };
                    let y = (y0 - step).max(target);
                    moved = y0 - y;
                    a.pose.y_m = y;
                    if y <= target + EPS {
                        if shelter {
                            set_mode(a, MobilityMode::Perched, Some(stand_perch(PerchKind::Underside, site, &low)), false);
                        } else {
                            set_mode(a, MobilityMode::Ground, None, false);
                        }
                    }
                } else {
                    let shelter = sky.downpour && y0 <= under + EPS;
                    let (target, kind, layer) = if shelter {
                        (under, PerchKind::Underside, low)
                    } else {
                        (top, PerchKind::CrownTop, high)
                    };
                    let y = (y0 + step).min(target.max(y0));
                    moved = y - y0;
                    a.pose.y_m = y;
                    if y >= target - EPS {
                        a.pose.y_m = target.max(floor);
                        set_mode(a, MobilityMode::Perched, Some(stand_perch(kind, site, &layer)), false);
                    }
                }
            }
            (None, Some(face)) => {
                if down {
                    let y = (y0 - step).max(floor);
                    moved = y0 - y;
                    a.pose.y_m = y;
                    if y <= floor + EPS {
                        a.pose.heading_rad = away_from(face);
                        set_mode(a, MobilityMode::Ground, None, false);
                    } else if let Some(f) = vine_hold(view, &fv, face, y, s) {
                        a.mobility.perch = Some(Perch {
                            kind: PerchKind::VineFace,
                            anchor: PerchAnchor::Vine(f),
                        });
                    }
                } else {
                    let y = y0 + step;
                    match vine_hold(view, &fv, face, y, s) {
                        // Room overhead in its own column (an overhang stops it).
                        Some(f) if room_over(view, a.pose.x, a.pose.z, y, s) => {
                            moved = step;
                            a.pose.y_m = y;
                            a.mobility.perch = Some(Perch {
                                kind: PerchKind::VineFace,
                                anchor: PerchAnchor::Vine(f),
                            });
                        }
                        // The cover (or the room) stops at a lip it reaches: it pulls
                        // itself over and perches on the top (ROOSTS, backlog §12).
                        _ if let Some((x, z, top, site)) = mount(view, &fv, *a, vine_hold(view, &fv, face, y0, s).unwrap_or(face), s) => {
                            moved = (top - y0).max(0.0);
                            a.pose.x = x;
                            a.pose.z = z;
                            a.pose.y_m = top;
                            set_mode(
                                a,
                                MobilityMode::Perched,
                                Some(Perch {
                                    kind: PerchKind::Face,
                                    anchor: PerchAnchor::Face(site),
                                }),
                                false,
                            );
                        }
                        // The cover (or the room) stops: it clings where it is.
                        _ => {
                            let f = vine_hold(view, &fv, face, y0, s).unwrap_or(face);
                            set_mode(
                                a,
                                MobilityMode::Perched,
                                Some(Perch {
                                    kind: PerchKind::VineFace,
                                    anchor: PerchAnchor::Vine(f),
                                }),
                                false,
                            );
                        }
                    }
                }
            }
            _ => {}
        }
        let fb = &mut a.founder_state.feedback;
        fb.attempted_equivalent += step;
        fb.delivered_equivalent += moved;
        fb.delivered_forward += moved;
        fb.structural_loss += body;
        fb.motor_respiration += paid;
        went = moved;
    }
    let fed = due && held.feed > 0.0 && glider_feed(fauna, i, view, flora, manifest, phys, held.feed);
    let a = &mut fauna.animals[i];
    // As a walker's: Climbing or Walking only for a tick it went somewhere, else Resting
    // (the stand still counts as climbing time by its mode).
    a.state = match (fed, a.mobility.mode) {
        (true, _) => State::Cropping,
        (false, _) if went <= 0.0 => State::Resting,
        (false, MobilityMode::Climb) => State::Climbing,
        (false, MobilityMode::Ground) => State::Walking,
        _ => State::Resting,
    };
}

/// One perched tick.
#[allow(clippy::too_many_arguments)]
fn perched_tick(
    fauna: &mut Fauna,
    view: &VoxelView<'_>,
    flora: &mut Flora,
    sky: &Sky,
    phys: &crate::FounderPhysiology,
    manifest: &crate::Manifest,
    glider: Glider,
    held: Actions,
    due: bool,
    asleep: bool,
    i: usize,
) {
    let s = shape(phys, fauna.animals[i].body);
    let puddle = phys.drown_depth_m(&phys.body_at(fauna.animals[i].body));
    let a = fauna.animals[i];
    if !flight::perch_holds(view, &flora.view(), &a, s, puddle) {
        start_glide(fauna, i, true);
        return;
    }
    if asleep {
        return;
    }
    let Some(perch) = a.mobility.perch else {
        start_glide(fauna, i, true);
        return;
    };
    // A downpour sends a crown-top perch down under its crown.
    if sky.downpour
        && perch.kind == PerchKind::CrownTop
        && let PerchAnchor::Stand { site, .. } = perch.anchor
        && let Some((low, _)) = crown_span(&flora.view(), site)
    {
        let under = low.cell as f64 * view.config.voxel_m - s.h;
        let floor = floor_at(view, a.pose.x, a.pose.z, a.pose.y_m).map_or(0.0, |(t, _)| t);
        if under >= floor - EPS && under < a.pose.y_m - EPS {
            fauna.animals[i].mobility.mode = MobilityMode::Climb;
            fauna.animals[i].mobility.falling = true;
            fauna.animals[i].state = State::Climbing;
            return;
        }
    }
    if held.forward > 0.0 {
        let floor = floor_at(view, a.pose.x, a.pose.z, a.pose.y_m).map_or(0.0, |(t, _)| t);
        let high = a.pose.y_m - floor >= glider.glide_min_height_m - EPS;
        // A glide that would end in water deeper than it wades is refused, as a refused
        // walk is (roster mechanics fix 1): nothing delivered of what it asked, so its
        // heuristic's escape turns it; a wall face's perch climbs down instead.
        let refused = |fauna: &mut Fauna| {
            let asked = held.forward * manifest.cruise_m_per_s * DT;
            let a = &mut fauna.animals[i];
            a.founder_state.feedback.attempted_equivalent += asked;
            a.state = State::Resting;
        };
        match perch.anchor {
            PerchAnchor::Vine(face) => {
                // PACK-TUNE, the glide loop: a body on a face hangs with its footprint in the
                // wall's columns, so a glide begun where it hangs is blocked in place, falls,
                // is pressed against the same face and climbs it again (49,704 glides in
                // GAIT's s4). It leaves from a point clear of the wall, as a glide off an
                // edge does, and only for a glide that does not come back to this face.
                let fv = flora.view();
                let from = off_face(view, s, &a, face).filter(|&from| {
                    !glide_ends_deep(view, &fv, &glider, phys, a.body, from)
                        && !matches!(glide_end(view, &fv, &glider, phys, a.body, from),
                            Some(GlideEnd::Vine(f)) if same_wall(f, face))
                });
                if high && let Some(from) = from {
                    fauna.animals[i].pose = from;
                    start_glide(fauna, i, false);
                } else {
                    if high {
                        refused(fauna);
                    }
                    let m = &mut fauna.animals[i].mobility;
                    m.mode = MobilityMode::Climb;
                    m.falling = true;
                }
            }
            PerchAnchor::Stand { .. } => {
                // On a crown it turns as it asks, then goes.
                let heading = (a.pose.heading_rad + held.turn * manifest.yaw_cap_rad_per_s * DT)
                    .rem_euclid(std::f64::consts::TAU);
                fauna.animals[i].pose.heading_rad = heading;
                let from = crate::Pose {
                    heading_rad: heading,
                    ..a.pose
                };
                if high {
                    if glide_ends_deep(view, &flora.view(), &glider, phys, a.body, from) {
                        refused(fauna);
                        return;
                    }
                    start_glide(fauna, i, false);
                } else {
                    // Down from a crown top; on up from under a crown.
                    let m = &mut fauna.animals[i].mobility;
                    m.mode = MobilityMode::Climb;
                    m.falling = perch.kind != PerchKind::Underside;
                }
            }
            // On a ledge top (ROOSTS) it stands up and walks.
            PerchAnchor::Face(_) => set_mode(&mut fauna.animals[i], MobilityMode::Ground, None, false),
            _ => start_glide(fauna, i, true),
        }
        if fauna.animals[i].state != State::Resting || fauna.animals[i].mobility.mode != MobilityMode::Perched {
            fauna.animals[i].state = State::Walking;
        }
        return;
    }
    // At rest it may turn on a crown (a wall face holds its heading), and eat.
    if !matches!(perch.anchor, PerchAnchor::Vine(_)) {
        let a = &mut fauna.animals[i];
        a.pose.heading_rad =
            (a.pose.heading_rad + held.turn * manifest.yaw_cap_rad_per_s * DT).rem_euclid(std::f64::consts::TAU);
    }
    let fed = due && held.feed > 0.0 && glider_feed(fauna, i, view, flora, manifest, phys, held.feed);
    fauna.animals[i].state = if fed { State::Cropping } else { State::Resting };
}

/// **A fruit bite** (F3, §3.7): the gut's bite from [`glider_fruit_in_reach`], booked
/// eaten whole, its seed share into the gut seed lots and the rest into the stomach's
/// fruit pool. Only while the eating latch is on.
fn glider_feed(
    fauna: &mut Fauna,
    i: usize,
    view: &VoxelView<'_>,
    flora: &mut Flora,
    manifest: &crate::Manifest,
    phys: &crate::FounderPhysiology,
    effort: f64,
) -> bool {
    if !fauna.animals[i].eating {
        return false;
    }
    let founder = fauna.animals[i].founder.expect("a founder body");
    let Some((source, stock)) = glider_fruit_in_reach(view, &flora.view(), &fauna.animals[i], phys) else {
        return false;
    };
    let bite = gut_bite(&fauna.animals[i], phys, manifest.controller_period_s, effort, stock);
    if !(bite > 0.0) {
        return false;
    }
    let (plant, src_site) = match source {
        Bloom::Stand(site) => (
            flora.view().stand_at(site).map(|s| SeedPlant::Stand(s.species)),
            site,
        ),
        Bloom::Vine(face) => (
            Some(SeedPlant::Latticevine),
            flight::support_below(view, (f64::from(face.x) + 0.5) * view.config.voxel_m, (f64::from(face.z) + 0.5) * view.config.voxel_m, f64::from(face.y) * view.config.voxel_m)
                .unwrap_or(fauna.animals[i].site),
        ),
    };
    let taken = match source {
        Bloom::Stand(site) => flora.take_stand_fruit(site, bite),
        Bloom::Vine(face) => flora.take_fruit(face, bite),
    };
    let Some(taken) = taken else {
        return false;
    };
    let taken = fauna.book_eaten(taken, Food::Fruit);
    let k = match source {
        Bloom::Stand(_) => 0,
        Bloom::Vine(_) => 1,
    };
    fauna.ledger.fruit_eaten_by_source[k] += taken.organic;
    fauna.ledger.fruit_bites_by_source[k] += 1;
    if let Some(SeedPlant::Stand(sp)) = plant {
        fauna.ledger.bites_by_plant[sp.index()] += 1;
        fauna.ledger.eaten_by_plant[sp.index()] += taken.organic;
    }
    // The seeds ride in the gut: their share of all three currencies.
    let rest = match (phys.gut_seeds, plant) {
        (Some(gut), Some(plant)) if gut.seeds_per_fruit_organic > 0.0 => {
            let f = gut.seeds_per_fruit_organic.clamp(0.0, 1.0);
            let seeds = Taken {
                organic: f * taken.organic,
                mineral: f * taken.mineral,
                energy: f * taken.energy,
            };
            let transit = cubarium_voxel::ticks_at_least_one(gut.gut_transit_s);
            stow_seeds(&mut fauna.animals[i], plant, seeds, src_site, fauna.tick, transit);
            Taken {
                organic: taken.organic - seeds.organic,
                mineral: taken.mineral - seeds.mineral,
                energy: taken.energy - seeds.energy,
            }
        }
        _ => taken,
    };
    let l = &mut fauna.ledger;
    l.bites_by_founder[founder.index()] += 1;
    l.bites_by_founder_food[founder.index()][Food::Fruit.index()] += 1;
    swallow(fauna, i, Food::Fruit, rest);
    true
}

/// Put a meal's seeds in a lot: the newest lot of the same plant and source opened within
/// half the transit, else an empty slot (at most half the slots for one plant), else the
/// newest lot of the same plant. Its due tick moves to this meal's plus the transit.
fn stow_seeds(a: &mut Animal, plant: SeedPlant, seeds: Taken, source: Site, now: u64, transit: u64) {
    let lots = &mut a.gut_seeds.lots;
    let same = |l: &GutSeedLot| l.plant == Some(plant);
    let newest = |lots: &[GutSeedLot], pred: &dyn Fn(&GutSeedLot) -> bool| {
        lots.iter()
            .enumerate()
            .filter(|(_, l)| pred(l))
            .max_by_key(|(_, l)| l.opened_tick)
            .map(|(k, _)| k)
    };
    let joined = newest(lots, &|l| same(l) && l.source == source && now.saturating_sub(l.opened_tick) <= transit / 2);
    let held = lots.iter().filter(|l| same(l)).count();
    let k = joined
        .or_else(|| (held < crate::GutSeedLots::SLOTS / 2).then(|| lots.iter().position(|l| l.plant.is_none())).flatten())
        .or_else(|| newest(lots, &same))
        .or_else(|| lots.iter().position(|l| l.plant.is_none()));
    let Some(k) = k else {
        // Every slot is the other plant's: fold into the newest lot whatever it is (cannot
        // happen with half the slots per plant; kept total so nothing is lost).
        let k = newest(lots, &|_| true).expect("a lot");
        let l = &mut lots[k];
        l.organic += seeds.organic;
        l.mineral += seeds.mineral;
        l.energy += seeds.energy;
        l.due_tick = now + transit;
        return;
    };
    let l = &mut lots[k];
    if l.plant.is_none() {
        *l = GutSeedLot {
            plant: Some(plant),
            organic: 0.0,
            mineral: 0.0,
            energy: 0.0,
            source,
            opened_tick: now,
            due_tick: now + transit,
        };
    }
    l.organic += seeds.organic;
    l.mineral += seeds.mineral;
    l.energy += seeds.energy;
    l.due_tick = now + transit;
}

/// Step 2, after the gut: **every due gut seed lot is voided** at the body's support face
/// — a stand's seeds into its seed bank, a latticevine's into the vine seed bank
/// ([`Flora::deposit_vine_seeds`]) — booked `deposited_*`.
pub(super) fn void_gut_seeds(fauna: &mut Fauna, flora: &mut Flora) {
    let now = fauna.tick;
    for i in 0..fauna.animals.len() {
        if fauna.animals[i].gut_seeds.is_empty() {
            continue;
        }
        for k in 0..crate::GutSeedLots::SLOTS {
            let lot = fauna.animals[i].gut_seeds.lots[k];
            let Some(plant) = lot.plant else {
                continue;
            };
            if lot.due_tick > now {
                continue;
            }
            fauna.animals[i].gut_seeds.lots[k] = GutSeedLot::default();
            let (site, id) = (fauna.animals[i].site, fauna.animals[i].id);
            let seeds = Taken {
                organic: lot.organic,
                mineral: lot.mineral,
                energy: lot.energy,
            };
            let banked = match plant {
                SeedPlant::Stand(species) => {
                    if flora.deposit_seeds(site, species, seeds.organic, seeds.mineral, seeds.energy) {
                        fauna.ledger.deposited_organic_out += seeds.organic;
                        fauna.ledger.deposited_mineral_out += seeds.mineral;
                        fauna.ledger.deposited_energy_out += seeds.energy;
                        true
                    } else {
                        fauna.book_deposit(flora, site, DepositKind::Litter, seeds);
                        false
                    }
                }
                // Roster mechanics fix 3: the vine's own seed bank, by the same path.
                SeedPlant::Latticevine => {
                    if flora.deposit_vine_seeds(site, seeds.organic, seeds.mineral, seeds.energy) {
                        fauna.ledger.deposited_organic_out += seeds.organic;
                        fauna.ledger.deposited_mineral_out += seeds.mineral;
                        fauna.ledger.deposited_energy_out += seeds.energy;
                        true
                    } else {
                        fauna.book_deposit(flora, site, DepositKind::Litter, seeds);
                        false
                    }
                }
            };
            fauna.ledger.seeds_voided_by_source[plant.source_index()] += seeds.organic;
            fauna.ledger.seed_voids += 1;
            fauna.log_seed_void(SeedVoid {
                tick: now,
                animal: id,
                plant,
                organic: seeds.organic,
                source: lot.source,
                landed: site,
                banked,
            });
        }
    }
}

/// **The roost reflex** (F3; Fable's call on the brief's night and downpour lines): a
/// glider driven by its own heuristic, at its rest hour (and not hungry) or in a downpour,
/// walks to the nearest stand it can climb within `roost_search_m`, climbs it (on to the
/// crown top at night, under the crown in a downpour) and stays perched; `None` leaves its
/// heuristic's actions (nothing to climb in reach, or already perched and not asked to
/// move, or gliding).
pub(super) fn roost_reflex(
    fauna: &mut Fauna,
    i: usize,
    view: &VoxelView<'_>,
    fv: &FloraView<'_>,
    sky: &Sky,
) -> Option<Actions> {
    let a = fauna.animals[i];
    let founder = a.founder?;
    let phys = *fauna.config.founder(founder);
    let glider = phys.glider?;
    let sleepy = !phys.active.awake_in(sky.clock_on, sky.daylight, 1.0, fauna.night, phys.active_window, &fauna.config.life)
        && !a.emergency;
    if !(sleepy || sky.downpour) {
        return None;
    }
    let still = Actions {
        forward: 0.0,
        turn: 0.0,
        feed: 0.0,
    };
    // ROOSTS (backlog §12): at its rest hour, a vaulttree crown, else a vine-climbed ledge
    // top, within its budget, over where it is; on a ledge top it settles there.
    if sleepy && !sky.downpour && matches!(a.mobility.mode, MobilityMode::Ground | MobilityMode::Perched) {
        if let Some(roost) = better_roost(fauna, i, view, fv) {
            if let Some(go) = go_roost(fauna, i, view, fv, roost) {
                return Some(go);
            }
        } else if a.mobility.mode == MobilityMode::Ground && on_ledge_top(fauna, i, view, fv) {
            let site = a.site;
            set_mode(
                &mut fauna.animals[i],
                MobilityMode::Perched,
                Some(Perch {
                    kind: PerchKind::Face,
                    anchor: PerchAnchor::Face(site),
                }),
                false,
            );
            return Some(still);
        }
    }
    match a.mobility.mode {
        MobilityMode::Perched => Some(still),
        MobilityMode::Climb if !a.mobility.falling => Some(Actions {
            forward: 1.0,
            ..still
        }),
        MobilityMode::Ground => {
            let cands = stands_within(view, fv, &a, glider.roost_search_m, |s| climbable(fv, s, &glider));
            walk_to_stand(fauna, i, view, fv, &cands, glider.roost_search_m, 0.0, crate::Steer::Roost).map(|(go, _)| go)
        }
        _ => None,
    }
}

/// **Where a vine climber pulls itself over the lip** (ROOSTS, backlog §12): the wall
/// column of `face` tops out ([`ledge_lip`]) within the body's height of its highest covered
/// row there, and its disc stands clear on the top somewhere along its heading into the
/// face, within two radii and a voxel. `(x, z, the top's height, the top face)`.
fn mount(view: &VoxelView<'_>, fv: &FloraView<'_>, a: Animal, face: VineFace, s: Shape) -> Option<(f64, f64, f64, Site)> {
    let c = view.config;
    let v = c.voxel_m;
    let (y1, high) = ledge_lip(view, fv, face)?;
    if !mountable(y1, high, s.h, v) {
        return None;
    }
    let site = Site { x: face.x, y: y1 - 1, z: face.z };
    let top = crate::surface_m(site.y, v);
    let ring = f64::from(c.width) * v;
    let h = into_face(face);
    let (fx, fz) = (h.sin(), h.cos());
    let n = (((2.0 * s.r + v) / (0.125 * v)).ceil() as usize).max(1);
    (1..=n).find_map(|k| {
        let d = k as f64 * 0.125 * v;
        let (x, z) = ((a.pose.x + fx * d).rem_euclid(ring), a.pose.z + fz * d);
        if flight::blocked(view, s, x, z, top) {
            return None;
        }
        // Its centre over the top's column.
        let (floor, under) = floor_at(view, x, z, top)?;
        ((floor - top).abs() <= EPS && under == site).then_some((x, z, top, site))
    })
}

/// The lip of `face`'s wall column: the first open row over it, and the highest covered row
/// on that side under it. `None` when the column has no open row over the face, or the top
/// is no support face.
fn ledge_lip(view: &VoxelView<'_>, fv: &FloraView<'_>, face: VineFace) -> Option<(u32, u32)> {
    let c = view.config;
    let x = i64::from(face.x);
    let mut high = face.y;
    let mut y = face.y;
    loop {
        if fv.cover.face(VineFace::new(face.x, y, face.z, face.dir)).is_some() {
            high = y;
        }
        y += 1;
        if y >= c.height {
            return None;
        }
        if !view.is_solid(x, y, face.z) {
            break;
        }
    }
    view.is_support(x, y - 1, face.z).then_some((y, high))
}

/// Whether a body `h` tall clinging at the top of cover row `high` reaches over the lip row
/// `y1`: the lip no higher than its top.
fn mountable(y1: u32, high: u32, h: f64, v: f64) -> bool {
    f64::from(y1) * v <= f64::from(high + 1) * v + h + EPS
}

/// **The roost ledges** (ROOSTS): every wall column side whose latticevine cover runs up to
/// its lip's reach, a support face on top and a foot in front, with no overhang over the
/// front column below the lip; re-read at the teachers' `route_refresh_s`. The
/// body-specific test is [`ledge_fits`].
fn ledges(fauna: &mut Fauna, view: &VoxelView<'_>, fv: &FloraView<'_>) -> Vec<crate::Ledge> {
    let every = cubarium_voxel::ticks_at_least_one(fauna.config.teachers.route_refresh_s);
    if let (Some(t), list) = &fauna.ledges
        && fauna.tick.saturating_sub(*t) < every
    {
        return list.clone();
    }
    let mut faces: Vec<VineFace> = fv.cover.faces().into_iter().map(|f| f.face).filter(|f| !f.dir.is_underside()).collect();
    faces.sort_unstable_by_key(|f| (f.x, f.z, f.dir, f.y));
    let mut out = Vec::new();
    let mut k = 0;
    while k < faces.len() {
        let f0 = faces[k];
        let mut j = k;
        let mut gap = 0;
        while j + 1 < faces.len() && (faces[j + 1].x, faces[j + 1].z, faces[j + 1].dir) == (f0.x, f0.z, f0.dir) {
            gap = gap.max(faces[j + 1].y - faces[j].y - 1);
            j += 1;
        }
        let high = faces[j].y;
        k = j + 1;
        let Some((y1, lip_high)) = ledge_lip(view, fv, f0) else { continue };
        if lip_high != high {
            continue;
        }
        let Some((fx, _, fz)) = f0.front(view) else { continue };
        // The foot: the first solid down the front column from the lowest cover.
        let Some(fy) = (0..f0.y).rev().find(|&y| view.is_solid(i64::from(fx), y, fz)) else { continue };
        if !view.is_support(i64::from(fx), fy, fz) || view.solid_in(i64::from(fx), fz, fy + 1, y1) {
            continue;
        }
        out.push(crate::Ledge {
            face: f0,
            foot: Site { x: fx, y: fy, z: fz },
            top: Site { x: f0.x, y: y1 - 1, z: f0.z },
            low: f0.y,
            high,
            gap,
        });
    }
    fauna.ledges = (Some(fauna.tick), out.clone());
    out
}

/// Whether body shape `s` climbs `l` from its foot onto its top: cover in the rows it spans
/// standing at the foot, no run of uncovered rows it cannot span, the lip within its reach,
/// and the top at least `min_m` over the foot.
fn ledge_fits(l: &crate::Ledge, s: Shape, v: f64, min_m: f64) -> bool {
    let foot = crate::surface_m(l.foot.y, v);
    let (lo, hi) = body_rows(foot, s, v);
    let span = (hi - lo + 1).max(1);
    i64::from(l.low) <= hi
        && i64::from(l.gap) < span
        && mountable(l.top.y + 1, l.high, s.h, v)
        && crate::surface_m(l.top.y, v) - foot >= min_m - EPS
}

/// The key a walk to ledge `l` is shunned by: apart from every stand id.
fn ledge_key(l: &crate::Ledge) -> u64 {
    let d = match l.face.dir {
        FaceDir::PosX => 0,
        FaceDir::NegX => 1,
        FaceDir::PosZ => 2,
        FaceDir::NegZ => 3,
        FaceDir::Down => 4,
    };
    (1 << 63) | (u64::from(l.face.x) << 24) | (u64::from(l.face.z) << 4) | d
}

/// A preferred roost ([`better_roost`]).
#[derive(Clone, Copy, Debug)]
pub(super) enum Roost {
    /// A vaulttree: `(id, its site)`, walked to by its stem.
    Crown(u64, Site),
    /// A ledge at this distance, `true` walked to on top, `false` climbed from its foot.
    Ledge(f64, crate::Ledge, bool),
}

/// Whether body `i` stands on (or by) the lip of a ledge it could have climbed.
fn on_ledge_top(fauna: &mut Fauna, i: usize, view: &VoxelView<'_>, fv: &FloraView<'_>) -> bool {
    let a = fauna.animals[i];
    let Some(founder) = a.founder else { return false };
    let phys = *fauna.config.founder(founder);
    let Some(glider) = phys.glider else { return false };
    if glider.roost_budget_m <= 0.0 {
        return false;
    }
    let v = view.config.voxel_m;
    let ring = f64::from(view.config.width) * v;
    let s = shape(&phys, a.body);
    ledges(fauna, view, fv).iter().any(|l| {
        let (cx, cz) = crate::pack::centre(l.top, v);
        let (dx, dz) = crate::pose::wrapped_delta(a.pose.x, a.pose.z, cx, cz, ring);
        l.top.y == a.site.y && (dx * dx + dz * dz).sqrt() <= s.r + v && ledge_fits(l, s, v, glider.roost_ledge_min_m)
    })
}

/// **ROOSTS: a better roost than where it is** (backlog §12, Wrysk 2026-10-08), for a
/// glider at its rest hour: perched in a vaulttree crown it has the best; else the nearest
/// vaulttree it can climb within `roost_budget_m`; else, unless it already perches on a
/// ledge top, the nearest ledge within it its body climbs ([`ledge_fits`]). Each must be on
/// its route graph's component (walked to on top, or to the foot), not given up for
/// pressing, and — den rest's guard — cost the walk at cruise and the climb at the climbing
/// pace no more than `roost_reserve_share` of the reserve over the emergency line. `None`:
/// it stays where it is (or takes today's nearest stand).
pub(super) fn better_roost(fauna: &mut Fauna, i: usize, view: &VoxelView<'_>, fv: &FloraView<'_>) -> Option<Roost> {
    let a = fauna.animals[i];
    let founder = a.founder?;
    let phys = *fauna.config.founder(founder);
    let glider = phys.glider?;
    let budget = glider.roost_budget_m;
    if budget <= 0.0 {
        return None;
    }
    let perched = a.mobility.mode == MobilityMode::Perched;
    let class = match a.mobility.perch.map(|p| p.anchor) {
        Some(PerchAnchor::Stand { site, .. })
            if perched && fv.stands_on(site).iter().any(|st| st.species == cubarium_voxel_flora::Species::Vaulttree) =>
        {
            0
        }
        Some(PerchAnchor::Face(_)) if perched => 1,
        _ => 2,
    };
    if class == 0 {
        return None;
    }
    let v = view.config.voxel_m;
    let ring = f64::from(view.config.width) * v;
    let s = shape(&phys, a.body);
    let g = phys.body_at(a.body);
    // Den rest's guard: the trip's cost against a share of the reserve over the emergency
    // line.
    let cruise = (phys.cruise_body_lengths_per_s * g.length_m).max(f64::EPSILON);
    let climb = (phys.climb_speed_fraction * cruise).max(f64::EPSILON);
    let cap = phys.core.reserve_of(a.body);
    let usable = (a.reserve - (1.0 - phys.wake_hunger) * cap).max(0.0);
    let allowance = glider.roost_reserve_share.clamp(0.0, 1.0) * usable;
    let cost = |walk_m: f64, climb_m: f64| {
        a.body
            * (walk_m / cruise * (phys.core.maintenance_per_s + phys.motor_respiration_per_s)
                + climb_m.max(0.0) / climb
                    * (phys.core.maintenance_per_s + phys.motor_respiration_per_s * phys.wall_climb_cost_factor))
    };
    let tc = fauna.config.teachers;
    let shunned = fauna.walk_shunned(a.id, founder);
    let graph = fauna.routes(view, founder, tc.route_refresh_s);
    let own = (s.r / v).ceil().max(0.0) as u32;
    let from = graph.node_near(a.site, (a.pose.x, a.pose.z), own)?;
    let reach = ((s.r + 0.5 * v) / v).floor().max(1.0) as u32;
    let reachable = |site: Site| {
        graph
            .node_near_where(site, crate::pack::centre(site, v), reach, |f| f.y.abs_diff(site.y) <= 1)
            .is_some_and(|goal| graph.components[goal] == graph.components[from])
    };
    let vault = stands_within(view, fv, &a, budget, |st| {
        st.species == cubarium_voxel_flora::Species::Vaulttree && climbable(fv, st, &glider)
    });
    for (d, id, site) in vault {
        if shunned.contains(&id) {
            continue;
        }
        let Some((_, high)) = crown_span(fv, site) else { continue };
        let rise = (high.cell + 1) as f64 * v - crate::surface_m(site.y, v);
        if cost(d, rise) <= allowance && reachable(site) {
            return Some(Roost::Crown(id, site));
        }
    }
    if class == 1 {
        return None;
    }
    let mut best: Option<Roost> = None;
    for l in ledges(fauna, view, fv) {
        if shunned.contains(&ledge_key(&l)) || !ledge_fits(&l, s, v, glider.roost_ledge_min_m) {
            continue;
        }
        let (cx, cz) = crate::pack::centre(l.foot, v);
        let (dx, dz) = crate::pose::wrapped_delta(a.pose.x, a.pose.z, cx, cz, ring);
        let d = (dx * dx + dz * dz).sqrt();
        if d > budget || best.is_some_and(|b| matches!(b, Roost::Ledge(bd, _, _) if bd <= d)) {
            continue;
        }
        let rise = crate::surface_m(l.top.y, v) - crate::surface_m(l.foot.y, v);
        if reachable(l.top) && cost(d, 0.0) <= allowance {
            best = Some(Roost::Ledge(d, l, true));
        } else if reachable(l.foot) && cost(d, rise) <= allowance {
            best = Some(Roost::Ledge(d, l, false));
        }
    }
    best
}

/// The actions that take body `i` to `roost`: off a perch, toward it (a glide or a climb
/// down); on the ground, its route to the vaulttree's stem, the ledge's top, or the
/// ledge's foot and then into its wall (which climbs it). `None` when it cannot walk there.
fn go_roost(fauna: &mut Fauna, i: usize, view: &VoxelView<'_>, fv: &FloraView<'_>, roost: Roost) -> Option<Actions> {
    let a = fauna.animals[i];
    let founder = a.founder?;
    let phys = *fauna.config.founder(founder);
    let v = view.config.voxel_m;
    let ring = f64::from(view.config.width) * v;
    let (key, target) = match roost {
        Roost::Crown(id, site) => (id, site),
        Roost::Ledge(_, l, true) => (ledge_key(&l), l.top),
        Roost::Ledge(_, l, false) => (ledge_key(&l), l.foot),
    };
    let (tx, tz) = crate::pack::centre(target, v);
    let (dx, dz) = crate::pose::wrapped_delta(a.pose.x, a.pose.z, tx, tz, ring);
    if a.mobility.mode == MobilityMode::Perched {
        fauna.walking_to(a.id, key, crate::Steer::Roost);
        return Some(walk_toward(&a, dx, dz, 0.0));
    }
    let s = shape(&phys, a.body);
    if let Roost::Ledge(_, l, false) = roost
        && a.site.y == l.foot.y
        && (dx * dx + dz * dz).sqrt() <= s.r + v
    {
        // At the foot: into the vine's wall column from the foot's own column (a vine climb
        // starts only from the column in front of the covered face); beside the foot, onto
        // it first. It steps forward only while its aim lies within half a voxel of its
        // heading, so turning in place never carries it off the foot along the wall.
        fauna.walking_to(a.id, key, crate::Steer::Roost);
        let (ax, az) = if (a.site.x, a.site.z) == (l.foot.x, l.foot.z) {
            let (wx, wz) = crate::pack::centre(Site { x: l.face.x, y: l.face.y, z: l.face.z }, v);
            crate::pose::wrapped_delta(a.pose.x, a.pose.z, wx, wz, ring)
        } else {
            (dx, dz)
        };
        return Some(aim_toward(&a, ax, az, 0.5 * v));
    }
    let d = (dx * dx + dz * dz).sqrt();
    let budget = phys.glider?.roost_budget_m;
    walk_to_stand(fauna, i, view, fv, &[(d, key, target)], budget, 0.0, crate::Steer::Roost).map(|(go, _)| go)
}

/// Actions that walk a body at `(dx, dz)` from it: turn toward it and go forward when it
/// is ahead (the roost reflex's steering).
pub(super) fn walk_toward(a: &Animal, dx: f64, dz: f64, feed: f64) -> Actions {
    let (fx, fz) = a.pose.forward();
    // Signed angle from the heading to the target: `forward` is (sin h, cos h), so a
    // positive turn (heading up) swings toward +x from +z.
    let cross = fz * dx - fx * dz;
    let dot = fx * dx + fz * dz;
    let angle = cross.atan2(dot);
    Actions {
        forward: if angle.abs() < std::f64::consts::FRAC_PI_2 { 1.0 } else { 0.0 },
        turn: (angle * 2.0).clamp(-1.0, 1.0),
        feed,
    }
}

/// [`walk_toward`] that steps forward only while the target at `(dx, dz)` lies ahead and
/// within `lateral` metres of the heading's line; else it turns in place.
fn aim_toward(a: &Animal, dx: f64, dz: f64, lateral: f64) -> Actions {
    let (fx, fz) = a.pose.forward();
    let cross = fz * dx - fx * dz;
    let dot = fx * dx + fz * dz;
    let mut go = walk_toward(a, dx, dz, 0.0);
    go.forward = if dot > 0.0 && cross.abs() <= lateral { 1.0 } else { 0.0 };
    go
}

/// **The fruit search** (roster mechanics fix 4; Fable's call, 2026-10-02): a glider
/// driven by its own heuristic, on the ground, wanting food (its eating latch on; the
/// old reserve line `seedporter_full_at` is gone) and with no fruit at its
/// mouth, walks to the stem of the nearest stand it can climb that hangs fruit within
/// [`Glider::fruit_search_m`] — the roost reflex's world query over the plant layer, not a
/// sense — and walking into it climbs it. `None` leaves its heuristic's actions. No
/// observation channel carries fruit beyond the cone's 2 m (`design/voxel-senses.md` is
/// unchanged).
///
/// **S2**: a glider whose table is live finds fruit by sight instead ([`fruit_by_sight`]).
pub(super) fn fruit_reflex(fauna: &mut Fauna, i: usize, view: &VoxelView<'_>, fv: &FloraView<'_>, day: bool) -> Option<Actions> {
    let a = fauna.animals[i];
    let founder = a.founder?;
    let phys = *fauna.config.founder(founder);
    let glider = phys.glider?;
    if a.mobility.mode != MobilityMode::Ground || a.state == State::Asleep {
        return None;
    }
    // It searches while it wants food (the eating latch; since the minimal gut a fed
    // body's reserve stays near full, so the old reserve line no longer tells).
    if !a.eating {
        return None;
    }
    if glider_fruit_in_reach(view, fv, &a, &phys).is_some() {
        return None;
    }
    if phys.senses.live {
        return fruit_by_sight(fauna, i, view, fv, day);
    }
    let cands = stands_within(view, fv, &a, glider.fruit_search_m, |s| s.fruit > 0.0 && climbable(fv, s, &glider));
    walk_to_stand(fauna, i, view, fv, &cands, glider.fruit_search_m, 1.0, crate::Steer::Fruit).map(|(go, _)| go)
}

/// **Fruit by sight** (S2, review §1: the seedporter's still-object recognition): the nearest
/// fruiting stand it can climb that it sees — within its sight range for a still body at this
/// light, inside its field, the line from its eye to the crown's middle open of terrain — goes
/// into its record as its food ([`crate::perception::class::FOOD`]); with none in sight, the
/// one it last saw, for the pursuit's memory while that stand still fruits. It walks to it;
/// `None` (its heuristic wanders) when it knows of none.
fn fruit_by_sight(fauna: &mut Fauna, i: usize, view: &VoxelView<'_>, fv: &FloraView<'_>, day: bool) -> Option<Actions> {
    let a = fauna.animals[i];
    let founder = a.founder?;
    let phys = *fauna.config.founder(founder);
    let glider = phys.glider?;
    let table = phys.senses;
    let c = view.config;
    let v = c.voxel_m;
    let ring = f64::from(c.width) * v;
    let now = fauna.tick;
    let range = table.sight_range_m(day) * table.sight_still_fraction.max(0.0);
    let geometry = crate::body_of(&fauna.config, &a)?;
    let origin = crate::senses::cone_origin(view, &a.pose, a.sense_layer(v), &geometry);
    let cos_half = table.sight_half_angle_deg.to_radians().cos() - 1e-12;
    let (fx, fz) = a.pose.forward();
    let mut seen: Vec<(f64, u64, Site, (f64, f64, f64))> = Vec::new();
    if range > 0.0 {
        for stand in fv.stands.iter().filter(|s| s.fruit > 0.0 && climbable(fv, s, &glider)) {
            let sx = (f64::from(stand.site.x) + 0.5) * v;
            let sz = (f64::from(stand.site.z) + 0.5) * v;
            let (dx, dz) = crate::pose::wrapped_delta(a.pose.x, a.pose.z, sx, sz, ring);
            let dh = (dx * dx + dz * dz).sqrt();
            let my = crate::surface_m(stand.site.y, v) + 0.5 * fv.config.species(stand.species).crown_height_m_at(stand.wood);
            let d3 = (dh * dh + (my - origin.1).powi(2)).sqrt();
            if d3 > range || (dh > 0.0 && (dx * fx + dz * fz) / dh < cos_half) {
                continue;
            }
            seen.push((d3, stand.id, stand.site, (sx, my, sz)));
        }
    }
    seen.sort_by(|p, q| p.0.total_cmp(&q.0).then(p.1.cmp(&q.1)));
    // EDGE: of those it sees, the nearest it can walk to on its route graph.
    let clear: Vec<(f64, u64, Site, (f64, f64, f64))> = seen.into_iter().filter(|s| crate::perception::line_clear(view, a.id, origin, s.3)).collect();
    let cands: Vec<(f64, u64, Site)> = clear.iter().map(|s| (s.0, s.1, s.2)).collect();
    if let Some((go, k)) = walk_to_stand(fauna, i, view, fv, &cands, range, 1.0, crate::Steer::Fruit) {
        let (d, id, site, (sx, _, sz)) = clear[k];
        let p = fauna.percepts.entry(a.id).or_default();
        let food = crate::perception::class::FOOD as usize;
        p.best[food] = crate::Track {
            known: true,
            id,
            class: crate::perception::class::FOOD,
            modes: crate::perception::SIGHT,
            x: sx,
            z: sz,
            y_m: crate::surface_m(site.y, v),
            site: (site.x, site.y, site.z),
            d_m: d,
            tick: now,
            ..crate::Track::default()
        };
        return Some(go);
    }
    // None it sees and can reach: the one it last saw, for the pursuit's memory while that
    // stand still fruits.
    let food = crate::perception::class::FOOD as usize;
    let m = fauna.percepts.get(&a.id).map(|p| p.best[food]).unwrap_or_default();
    let memory = cubarium_voxel::ticks_at_least_one(fauna.config.perception.give_up_s);
    let fruits = |id: u64| fv.stands.iter().any(|s| s.id == id && s.fruit > 0.0);
    if !(m.known && now.saturating_sub(m.tick) <= memory && fruits(m.id)) {
        return None;
    }
    let (dx, dz) = crate::pose::wrapped_delta(a.pose.x, a.pose.z, m.x, m.z, ring);
    let site = Site { x: m.site.0, y: m.site.1, z: m.site.2 };
    walk_to_stand(fauna, i, view, fv, &[((dx * dx + dz * dz).sqrt(), m.id, site)], range, 1.0, crate::Steer::Fruit).map(|(go, _)| go)
}

/// **The arboreal refuge** (S2, [`crate::vigil`]): a grounded glider walks to the nearest stand
/// it can climb within `search_m` (walking into the stem climbs it); `None` with none.
pub(super) fn refuge_step(fauna: &mut Fauna, i: usize, view: &VoxelView<'_>, fv: &FloraView<'_>, search_m: f64) -> Option<Actions> {
    let a = fauna.animals[i];
    let glider = fauna.config.founder(a.founder?).glider?;
    if a.mobility.mode != MobilityMode::Ground {
        return None;
    }
    let cands = stands_within(view, fv, &a, search_m, |s| climbable(fv, s, &glider));
    walk_to_stand(fauna, i, view, fv, &cands, search_m, 0.0, crate::Steer::Flee).map(|(go, _)| go)
}

/// The stands `keep` accepts within `search_m` of body `a` (straight, across the ring),
/// nearest first: `(distance, id, site)`.
fn stands_within(
    view: &VoxelView<'_>,
    fv: &FloraView<'_>,
    a: &Animal,
    search_m: f64,
    keep: impl Fn(&cubarium_voxel_flora::Stand) -> bool,
) -> Vec<(f64, u64, Site)> {
    let v = view.config.voxel_m;
    let ring = f64::from(view.config.width) * v;
    let mut out: Vec<(f64, u64, Site)> = fv
        .stands
        .iter()
        .filter(|s| keep(s))
        .filter_map(|s| {
            let sx = (f64::from(s.site.x) + 0.5) * v;
            let sz = (f64::from(s.site.z) + 0.5) * v;
            let (dx, dz) = crate::pose::wrapped_delta(a.pose.x, a.pose.z, sx, sz, ring);
            let d = (dx * dx + dz * dz).sqrt();
            (d <= search_m).then_some((d, s.id, s.site))
        })
        .collect();
    out.sort_by(|p, q| p.0.total_cmp(&q.0).then(p.1.cmp(&q.1)));
    out
}

/// The nodes ahead on a route a walk's waypoint looks along: a cost bound, not a knob.
const ROUTE_LOOKAHEAD: usize = 12;

/// The headings a body off its route graph tries for a glide off its ledge
/// ([`off_graph_exit`]): a resolution, not a knob.
const GLIDE_HEADINGS: u32 = 16;

/// **EDGE-2: the way off a face off the route graph**, for a grounded body standing where its
/// lineage's route graph has no node within its own reach:
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum OffGraph {
    /// It is on its graph.
    On,
    /// The first face of its own body's walking way back onto it ([`crate::RouteGraph::way_on`],
    /// out to `teachers.way_on_m`).
    Way(Site),
    /// With no walking way, a glider's heading off its ledge: walking that way, the edge
    /// starts a glide ([`glide_off_edge`]); of those, the nearest to the direction it wants.
    Glide(f64),
    /// Neither: it is cornered, and holds still.
    Cornered,
}

/// [`OffGraph`] for body `a`, wanting to go along `(want_x, want_z)` (any length; zero for
/// no preference).
pub(crate) fn off_graph_exit(fauna: &mut Fauna, view: &VoxelView<'_>, fv: &FloraView<'_>, a: &Animal, want: (f64, f64)) -> OffGraph {
    let Some(founder) = a.founder else { return OffGraph::On };
    let tc = fauna.config.teachers;
    let phys = *fauna.config.founder(founder);
    let graph = fauna.routes(view, founder, tc.route_refresh_s);
    let v = view.config.voxel_m;
    let body = phys.body_at(a.body);
    let r = body.footprint_radius();
    let own = (r / v).ceil().max(0.0) as u32;
    if graph.node_near(a.site, (a.pose.x, a.pose.z), own).is_some() {
        return OffGraph::On;
    }
    let cols = if tc.way_on_m.is_finite() && tc.way_on_m > 0.0 { (tc.way_on_m / v).ceil() as u32 } else { 0 };
    if let Some(f) = graph.way_on(view, a.site, r, body.headroom_voxels(v), phys.step_limits(v), phys.wade_depth_m(&body), own, cols) {
        return OffGraph::Way(f);
    }
    if let Some(glider) = phys.glider {
        let s = shape(&phys, a.body);
        let want_h = if want.0 * want.0 + want.1 * want.1 > 0.0 { Some(want.0.atan2(want.1)) } else { None };
        let mut best: Option<(f64, f64)> = None;
        for k in 0..GLIDE_HEADINGS {
            let h = f64::from(k) * std::f64::consts::TAU / f64::from(GLIDE_HEADINGS);
            let mut b = *a;
            b.pose.heading_rad = h;
            if glide_off_edge(view, fv, &b, s, &glider, &phys).is_none() {
                continue;
            }
            let off = want_h.map_or(0.0, |w| {
                let d = (h - w).rem_euclid(std::f64::consts::TAU);
                d.min(std::f64::consts::TAU - d)
            });
            if best.is_none_or(|(o, _)| off < o) {
                best = Some((off, h));
            }
        }
        if let Some((_, h)) = best {
            return OffGraph::Glide(h);
        }
    }
    OffGraph::Cornered
}

/// **EDGE: a reflex's walk to a stand** (the refuge, the roost, the fruit search): of
/// `cands` (`(distance, id, site)`, nearest first), the first the body can walk to on its
/// lineage's route graph — a node within its stem reach (its radius and half a voxel) of the
/// stem, on the stand's layer or one either side, routed from the body's node in at most
/// `teachers.route_detour × search_m` — and the actions that walk it there: straight at the
/// stem when the line is walkable, else toward the farthest of the next route nodes it can
/// walk straight to. A body off its graph walks straight, as before.
///
/// **A pressed walk re-plans**: a target walked to for `teachers.press_replan_s` while the
/// body has pressed that long without a break (held forward, its motor delivering under
/// `teachers.motor_refused`) is given up for `teachers.press_shun_s`, and the next is taken. Returns the actions and the index in
/// `cands`; `None` when it can reach none of them.
#[allow(clippy::too_many_arguments)]
fn walk_to_stand(
    fauna: &mut Fauna,
    i: usize,
    view: &VoxelView<'_>,
    _fv: &FloraView<'_>,
    cands: &[(f64, u64, Site)],
    search_m: f64,
    feed: f64,
    kind: crate::Steer,
) -> Option<(Actions, usize)> {
    let a = fauna.animals[i];
    let founder = a.founder?;
    let phys = *fauna.config.founder(founder);
    let tc = fauna.config.teachers;
    let v = view.config.voxel_m;
    let ring = f64::from(view.config.width) * v;
    let shunned = fauna.walk_shunned(a.id, founder);
    if cands.is_empty() {
        return None;
    }
    let graph = fauna.routes(view, founder, tc.route_refresh_s);
    let r = phys.body_at(a.body).footprint_radius();
    let own = (r / v).ceil().max(0.0) as u32;
    let from = graph.node_near(a.site, (a.pose.x, a.pose.z), own);
    // EDGE-2: off its graph, it does not walk straight at a stem over a drop or into a wall:
    // it takes its own way back on, else glides off its ledge, else it can reach none.
    if from.is_none() {
        let &(_, id, site) = cands.iter().find(|c| !shunned.contains(&c.1))?;
        let stem = ((f64::from(site.x) + 0.5) * v, (f64::from(site.z) + 0.5) * v);
        let (sx, sz) = crate::pose::wrapped_delta(a.pose.x, a.pose.z, stem.0, stem.1, ring);
        let (dx, dz) = match off_graph_exit(fauna, view, _fv, &a, (sx, sz)) {
            OffGraph::On => (sx, sz),
            OffGraph::Way(f) => crate::pose::wrapped_delta(a.pose.x, a.pose.z, (f64::from(f.x) + 0.5) * v, (f64::from(f.z) + 0.5) * v, ring),
            OffGraph::Glide(h) => (h.sin(), h.cos()),
            OffGraph::Cornered => return None,
        };
        let k = cands.iter().position(|c| c.1 == id).expect("a candidate");
        fauna.walking_to(a.id, id, kind);
        return Some((walk_toward(&a, dx, dz, feed), k));
    }
    let reach = ((r + 0.5 * v) / v).floor().max(1.0) as u32;
    let steps = tc.route_detour.max(0.0) * search_m / v;
    let max_steps = if steps.is_finite() { steps.ceil().min(f64::from(u32::MAX)) as u32 } else { u32::MAX };
    for (k, &(_, id, site)) in cands.iter().enumerate() {
        if shunned.contains(&id) {
            continue;
        }
        let stem = ((f64::from(site.x) + 0.5) * v, (f64::from(site.z) + 0.5) * v);
        let aim = match from {
            None => unreachable!("handled above"),
            Some(from) => {
                let Some(goal) = graph.node_near_where(site, stem, reach, |f| f.y.abs_diff(site.y) <= 1) else {
                    continue;
                };
                if graph.components[goal] != graph.components[from] {
                    continue;
                }
                if goal == from || graph.walk_line(from, (a.pose.x, a.pose.z), stem) {
                    Some(stem)
                } else {
                    graph.path(from, goal, max_steps).and_then(|p| graph.waypoint(&p, (a.pose.x, a.pose.z), ROUTE_LOOKAHEAD))
                }
            }
        };
        let Some((tx, tz)) = aim else { continue };
        fauna.walking_to(a.id, id, kind);
        let (dx, dz) = crate::pose::wrapped_delta(a.pose.x, a.pose.z, tx, tz, ring);
        return Some((walk_toward(&a, dx, dz, feed), k));
    }
    None
}

impl Fauna {
    /// [`crate::Command::IntroduceAloft`] for a glider (F3). The command sees the world
    /// and not the plant layer, so a pose in the air is introduced **gliding, its perch
    /// unresolved** ([`Mobility::falling`] set in `Glide`): its first tick perches it on
    /// the crown top of a climbable stand its base meets at its centre column, or else it
    /// glides on. A base on the floor below walks; a base in the air under
    /// `glide_min_height_m` over the floor is refused, as is a body inside solid.
    pub(crate) fn introduce_glider_aloft(
        &mut self,
        world: &World,
        pose: crate::Pose,
        founder: Founder,
        stores: crate::StartingStores,
    ) -> bool {
        let phys = *self.config.founder(founder);
        let Some(glider) = phys.glider else {
            return false;
        };
        let Some((body, reserve)) = stores.resolve(&phys.core) else {
            return false;
        };
        if !pose.is_finite() || !pose.y_m.is_finite() {
            return false;
        }
        let view = world.view();
        let s = shape(&phys, body);
        if flight::blocked(&view, s, pose.x, pose.z, pose.y_m) {
            return false;
        }
        let Some((floor_top, site)) = floor_at(&view, pose.x, pose.z, pose.y_m) else {
            return false;
        };
        let on_floor = (pose.y_m - floor_top).abs() <= EPS;
        if !on_floor && pose.y_m - floor_top < glider.glide_min_height_m - EPS {
            return false;
        }
        if !self.introduce_body(
            world,
            site,
            crate::Species::Frondgrazer,
            Some(founder),
            body,
            Some(reserve),
            pose.heading_rad,
        ) {
            return false;
        }
        let id = self.ledger.births - 1;
        let i = self
            .animals
            .binary_search_by_key(&id, |a| a.id)
            .expect("just placed");
        let a = &mut self.animals[i];
        if on_floor {
            a.pose = crate::Pose { y_m: floor_top, ..pose };
            set_mode(a, MobilityMode::Ground, None, false);
        } else {
            a.pose = pose;
            set_mode(a, MobilityMode::Glide, None, true);
        }
        true
    }
}

