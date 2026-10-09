//! Contract v3 P6: the reflexes' place queries, read at the remote-sense cadence.
//! Places never writes a walking goal, percept, action or ecological ledger. Static
//! places require sight (day/night, still-object range, FOV and terrain ray). A
//! bellwing uses its near-IR receptor range for vines; an eyeless ground body only
//! knows supports touched by its footprint. Sound/tremor reveal no static places.
//! The den is known at any distance.
//! Four last-used home places are saved; the sensed cache is derived, not saved.

use cubarium_voxel::VoxelView;
use cubarium_voxel_flora::{FloraView, Site};
use serde::{Deserialize, Serialize};

use super::{Sky, glider};

use crate::{Animal, Fauna, MobilityMode, PerchAnchor, State};

// Fixed storage bounds, not ecological tuning.
const KINDS: [&str; 8] = [
    "shore",
    "water",
    "shore_egg",
    "film",
    "roost",
    "fruit",
    "den",
    "nest",
];
const HOMES: [&str; 4] = ["roost", "water", "den", "nest"];

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub(crate) struct Point {
    pub x: f64,
    pub y_m: f64,
    pub z: f64,
}

impl Point {
    fn site(view: &VoxelView<'_>, site: Site) -> Self {
        let v = view.config.voxel_m;
        let (x, z) = crate::pack::centre(site, v);
        Self {
            x,
            y_m: crate::surface_m(site.y, v) + 0.5 * v,
            z,
        }
    }

    fn water(view: &VoxelView<'_>, site: Site) -> Self {
        let mut p = Self::site(view, site);
        // It sees the water surface, not a support face hidden below that surface.
        p.y_m = crate::surface_m(site.y, view.config.voxel_m)
            + view.standing_depth_m(i64::from(site.x), site.y, site.z);
        p
    }

    fn pose(p: crate::Pose) -> Self {
        Self {
            x: p.x,
            y_m: p.y_m,
            z: p.z,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub(crate) struct Remembered {
    pub point: Point,
    pub visited: u64,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub(crate) struct Memory {
    homes: [Option<Remembered>; 4],
}

impl Memory {
    fn visit(&mut self, kind: &str, point: Point, tick: u64) {
        if let Some(k) = HOMES.iter().position(|&s| s == kind) {
            self.homes[k] = Some(Remembered {
                point,
                visited: tick,
            });
        }
    }

    fn get(&self, kind: &str) -> Option<Remembered> {
        self.homes[HOMES.iter().position(|&s| s == kind)?]
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct Cache {
    pub tick: u64,
    pub range_m: f64,
    sensed: [Option<Point>; 8],
}

/// Querying early must not move the heuristic caches' refresh timestamps. Keep
/// separate copies, borrowing the existing query implementations by swapping only
/// their derived caches for the duration of the observation pass.
#[derive(Clone, Debug, Default)]
pub(crate) struct QueryCaches {
    routes: rustc_hash::FxHashMap<usize, (u64, std::sync::Arc<crate::encounter::RouteGraph>)>,
    air: Option<(u64, std::sync::Arc<crate::flight::OpenAir>)>,
    ledges: (Option<u64>, Vec<crate::Ledge>),
}

impl QueryCaches {
    fn swap(&mut self, fauna: &mut Fauna) {
        std::mem::swap(&mut self.routes, &mut fauna.pack_routes);
        std::mem::swap(&mut self.air, &mut fauna.open_air);
        std::mem::swap(&mut self.ledges, &mut fauna.ledges);
    }
}

#[derive(Clone, Copy)]
struct Sight {
    id: u64,
    origin: (f64, f64, f64),
    heading: (f64, f64),
    range_m: f64,
    cos_half: f64,
    contact_radius_m: Option<f64>,
}

impl Sight {
    fn of(fauna: &Fauna, a: &Animal, view: &VoxelView<'_>, sky: &Sky) -> Option<Self> {
        let phys = fauna.config.founder(a.founder?);
        let table = phys.senses.sensing(a.still_ticks);
        let day = !sky.clock_on || sky.daylight >= fauna.config.life.day_line;
        let down = fauna
            .percepts
            .get(&a.id)
            .is_some_and(|p| p.vigil.head_down(fauna.tick));
        let scale = if down {
            table.feeding_sight_fraction
        } else {
            1.0
        };
        let body = crate::body_of(&fauna.config, a)?;
        let visual = (table.sight_range_m(day) * table.sight_still_fraction * scale).max(0.0);
        let infrared = phys.egg_site == crate::EggSite::Vine && table.nir;
        let contact = !infrared && visual <= 0.0 && phys.flight.is_none();
        let r = body.footprint_radius();
        let range_m = if infrared {
            table.light_m.max(0.0)
        } else if contact {
            r + std::f64::consts::SQRT_2 * 0.5 * view.config.voxel_m
        } else {
            visual
        };
        let origin = if contact {
            (
                a.pose.x,
                crate::surface_m(a.site.y, view.config.voxel_m) + 0.5 * view.config.voxel_m,
                a.pose.z,
            )
        } else {
            crate::senses::cone_origin(view, &a.pose, a.sense_layer(view.config.voxel_m), &body)
        };
        Some(Self {
            id: a.id,
            origin,
            heading: a.pose.forward(),
            range_m,
            cos_half: if infrared || contact {
                -1.0
            } else {
                table.sight_half_angle_deg.to_radians().cos() - 1e-12
            },
            contact_radius_m: contact.then_some(r),
        })
    }

    fn sees(&self, view: &VoxelView<'_>, p: Point) -> bool {
        if !(self.range_m > 0.0) {
            return false;
        }
        let ring = f64::from(view.config.width) * view.config.voxel_m;
        let (dx, dz) = crate::pose::wrapped_delta(self.origin.0, self.origin.2, p.x, p.z, ring);
        if let Some(r) = self.contact_radius_m {
            // Exact overlap with the support column under the footprint, not vision
            // borrowed from a tremor/hearing range. Only its own support height is felt.
            let half = 0.5 * view.config.voxel_m;
            return (dx.abs() - half).max(0.0).hypot((dz.abs() - half).max(0.0)) <= r
                && (p.y_m - self.origin.1).abs() < 1e-12;
        }
        let dh = dx.hypot(dz);
        dh.hypot(p.y_m - self.origin.1) <= self.range_m
            && (dh == 0.0 || (dx * self.heading.0 + dz * self.heading.1) / dh >= self.cos_half)
            && crate::perception::line_clear(view, self.id, self.origin, (p.x, p.y_m, p.z))
    }
}

/// Cheap contact visits for every body, including heuristic bodies and sleepers.
/// Merely seeing a place never learns or refreshes a home. A water visit is actual
/// standing water; roost/nest visits use physical perch anchors; a den visit requires
/// the body's footprint to reach its den face at the same support height.
fn visits(fauna: &mut Fauna, view: &VoxelView<'_>) {
    for a in &fauna.animals {
        let Some(f) = a.founder else { continue };
        let phys = fauna.config.founder(f);
        let mut used = [None; 4];
        if a.attached.is_none() {
            if phys.glider.is_some() && a.mobility.mode == MobilityMode::Perched {
                used[0] = a.mobility.perch.map(|p| match p.anchor {
                    PerchAnchor::Stand { site, .. } | PerchAnchor::Face(site) => {
                        Point::site(view, site)
                    }
                    PerchAnchor::Vine(_) | PerchAnchor::Rock { .. } => Point::pose(a.pose),
                });
            }
            if phys.amphibious.is_some()
                && a.mobility.mode == MobilityMode::Ground
                && view.standing_depth_m(i64::from(a.site.x), a.site.y, a.site.z) > 0.0
            {
                used[1] = Some(Point::water(view, a.site));
            }
            if let Some(den) = a
                .pack
                .and_then(|m| fauna.packs.iter().find(|p| p.id == m.pack))
                .map(|p| p.den)
            {
                let p = Point::site(view, den);
                let ring = f64::from(view.config.width) * view.config.voxel_m;
                let (dx, dz) = crate::pose::wrapped_delta(a.pose.x, a.pose.z, p.x, p.z, ring);
                if a.site.y == den.y
                    && dx.hypot(dz)
                        <= phys.body_at(a.body).footprint_radius() + 0.5 * view.config.voxel_m
                {
                    used[2] = Some(p);
                }
            }
            if phys.egg_site == crate::EggSite::Vine
                && a.mobility.mode == MobilityMode::Perched
                && matches!(
                    a.mobility.perch.map(|p| p.anchor),
                    Some(PerchAnchor::Vine(_))
                )
            {
                used[3] = Some(Point::pose(a.pose));
            }
        }
        if used.iter().any(Option::is_some) {
            let m = fauna.place_memory.entry(a.id).or_default();
            for (kind, p) in HOMES.into_iter().zip(used) {
                if let Some(p) = p {
                    m.visit(kind, p, fauna.tick);
                }
            }
        }
    }
}

/// Runs once per tick; queries only active brain/tapped bodies on their staggered
/// remote tick. Cache hits encode bearings from the body's *current* frame.
pub(crate) fn refresh(fauna: &mut Fauna, view: &VoxelView<'_>, fv: &FloraView<'_>, sky: &Sky) {
    visits(fauna, view);
    if fauna.tick % 64 == 0 {
        let alive: rustc_hash::FxHashSet<_> = fauna.animals.iter().map(|a| a.id).collect();
        fauna.place_memory.retain(|id, _| alive.contains(id));
        fauna.place_cache.retain(|id, _| alive.contains(id));
    }
    let period = fauna.config.perception.stagger_ticks();
    let mut queries = std::mem::take(&mut fauna.place_queries);
    queries.swap(fauna);
    for i in 0..fauna.animals.len() {
        let a = fauna.animals[i];
        if a.state == State::Asleep
            || a.attached.is_some()
            || a.id % period != fauna.tick % period
            || !(fauna.tap.is_some() || fauna.controllers.reads_tail(a.id))
        {
            continue;
        }
        let Some(f) = a.founder else { continue };
        let phys = *fauna.config.founder(f);
        let Some(sight) = Sight::of(fauna, &a, view, sky) else {
            continue;
        };
        let mut c = Cache {
            tick: fauna.tick,
            range_m: sight.range_m,
            ..Cache::default()
        };
        let site_seen = |s| sight.sees(view, Point::site(view, s));
        let water_seen = |s| sight.sees(view, Point::water(view, s));
        for module in fauna
            .config
            .manifest(f)
            .modules
            .iter()
            .filter(|m| m.name.starts_with("Place("))
        {
            let kind = module
                .name
                .strip_prefix("Place(")
                .and_then(|s| s.strip_suffix(')'))
                .expect("place module");
            if kind != "den" && !(sight.range_m > 0.0) {
                continue;
            }
            let point = match kind {
                "shore" => crate::step::shallows_target(
                    fauna,
                    i,
                    view,
                    fauna.config.life.shallows_search_m.min(sight.range_m),
                    site_seen,
                )
                .map(|s| Point::site(view, s)),
                "water" => phys
                    .amphibious
                    .and_then(|am| {
                        crate::step::water_target(
                            fauna,
                            i,
                            view,
                            am.water_search_m.min(sight.range_m),
                            water_seen,
                        )
                    })
                    .map(|s| Point::water(view, s)),
                "shore_egg" => phys
                    .amphibious
                    .and_then(|am| {
                        crate::step::shore_target(
                            fauna,
                            i,
                            view,
                            sky,
                            am.egg_search_m.min(sight.range_m),
                            site_seen,
                        )
                    })
                    .map(|s| Point::site(view, s)),
                "film" => phys
                    .amphibious
                    .and_then(|mut am| {
                        am.graze_search_m = am.graze_search_m.min(sight.range_m);
                        let others: Vec<_> = fauna
                            .animals
                            .iter()
                            .filter(|b| b.id != a.id && b.founder == a.founder)
                            .map(|b| b.site)
                            .collect();
                        let roam = am.on_land && crate::step::roam_hours(fauna, &am, view, sky);
                        let k = fv.config.film_capacity_per_m2 * view.config.voxel_m.powi(2);
                        crate::step::film_target(
                            view,
                            &am,
                            &a,
                            roam,
                            k,
                            &|s| fv.edible_film_at(s),
                            &others,
                            site_seen,
                        )
                    })
                    .map(|s| Point::site(view, s)),
                "roost" => phys
                    .glider
                    .and_then(|g| {
                        glider::roost_target(
                            fauna,
                            i,
                            view,
                            fv,
                            g.roost_budget_m.min(sight.range_m),
                            true,
                            site_seen,
                        )
                    })
                    .map(|r| {
                        let site = match r {
                            glider::Roost::Crown(_, s) => s,
                            glider::Roost::Ledge(_, l, true) => l.top,
                            glider::Roost::Ledge(_, l, false) => l.foot,
                        };
                        Point::site(view, site)
                    }),
                "fruit" => {
                    glider::fruit_candidates(fauna, i, view, fv, sight.range_m).and_then(|seen| {
                        let seen: Vec<_> = seen
                            .into_iter()
                            .filter(|s| {
                                sight.sees(
                                    view,
                                    Point {
                                        x: s.3.0,
                                        y_m: s.3.1,
                                        z: s.3.2,
                                    },
                                )
                            })
                            .collect();
                        let cands: Vec<_> = seen.iter().map(|s| (s.0, s.1, s.2)).collect();
                        glider::walk_target(fauna, i, view, fv, &cands, sight.range_m, 1.0)
                            .map(|(_, k)| Point::site(view, cands[k].2))
                    })
                }
                "den" => a
                    .pack
                    .and_then(|m| fauna.packs.iter().find(|p| p.id == m.pack))
                    .map(|p| Point::site(view, p.den)),
                "nest" => phys
                    .flight
                    .and_then(|flight| {
                        crate::step::nest_target(
                            fauna,
                            i,
                            view,
                            fv,
                            flight.nest_search_m.min(sight.range_m),
                            |p| {
                                let h = crate::flight::Shape::of(&phys, &flight, a.body).h;
                                sight.sees(
                                    view,
                                    Point {
                                        y_m: p.y_m + 0.5 * h,
                                        ..Point::pose(p)
                                    },
                                )
                            },
                        )
                    })
                    .map(Point::pose),
                _ => None,
            };
            c.sensed[KINDS
                .iter()
                .position(|&s| s == kind)
                .expect("declared kind")] = point;
        }
        fauna.place_cache.insert(a.id, c);
    }
    queries.swap(fauna);
    fauna.place_queries = queries;
}

pub(crate) fn encode(
    fauna: &Fauna,
    a: &Animal,
    kind: &str,
    frame: &crate::observe_v3::Frame,
    v: f64,
) -> [f64; 5] {
    let Some(k) = KINDS.iter().position(|&s| s == kind) else {
        return [0.0; 5];
    };
    let cache = fauna
        .place_cache
        .get(&a.id)
        .filter(|c| c.tick <= fauna.tick);
    let range = cache.map_or_else(
        || {
            a.founder
                .map_or(0.0, |f| fauna.config.manifest(f).slot_range_m)
        },
        |c| c.range_m,
    );
    // The current pack's den is always known, even before the first remote tick.
    let den = (kind == "den")
        .then(|| {
            a.pack
                .and_then(|m| fauna.packs.iter().find(|p| p.id == m.pack))
        })
        .flatten()
        .map(|p| {
            let (x, z) = crate::pack::centre(p.den, v);
            Point {
                x,
                z,
                y_m: crate::surface_m(p.den.y, v),
            }
        });
    let sensed = if kind == "den" {
        den
    } else {
        cache.and_then(|c| c.sensed[k])
    };
    let (point, fresh) = if let Some(p) = sensed {
        (p, 1.0)
    } else if let Some(m) = fauna.place_memory.get(&a.id).and_then(|m| m.get(kind)) {
        let fade = fauna.config.perception.place_memory_fade_s;
        let age = fauna.tick.saturating_sub(m.visited) as f64 * crate::DT;
        let fresh = if fade > 0.0 {
            (1.0 - age / fade).clamp(0.0, 1.0)
        } else {
            0.0
        };
        if fresh <= 0.0 {
            return [0.0; 5];
        }
        (m.point, fresh)
    } else {
        return [0.0; 5];
    };
    let (sin, cos, d) = frame.toward(point.x, point.z);
    [1.0, sin, cos, crate::observe_v3::proximity(d, range), fresh]
}

#[cfg(test)]
#[path = "places_tests.rs"]
mod tests;
