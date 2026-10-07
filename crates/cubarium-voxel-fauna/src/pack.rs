//! **Packs** (roster R3, `design/handoffs/voxel-roster-2026-10-01.md` Brief R3;
//! predation-and-flight §2.10, §2.14; organism-large-animals §5): the chorister's den,
//! breeder, pups, provisioning, territory and its scripted pack tactic, plus the
//! lanternjaw's carcass yield (Fable's decision on the test author's open point).
//!
//! What lives here and where the tick calls it:
//! - [`upkeep`] (after maintenance): calls count down, pups come of age, a dead breeder's
//!   place goes to the oldest adult, empty packs are dropped, and adults at the den
//!   provision the hungriest pup there.
//! - [`must_walk_home`] (the circadian stage): a pack adult under its heuristic rests and
//!   sleeps only within `den_radius_m` of its den, or once `home_give_up_s` has passed.
//! - [`steer`] (the controller stage, for a body whose controller opts in): the tactic —
//!   a pup stays at the den; an adult walks home to rest, drives or flanks a pack target,
//!   gives a chase up, eats the kill, keeps near the breeder, and otherwise hunts alone
//!   as the lanternjaw does; nothing it does paths into another pack's territory.
//! - [`window_body`] (the strike rule): the pack seam — the size window measures prey
//!   against the summed body of the striker's adult packmates within `pack_radius_m`.
//! - [`carcass_yield`] (the controller stage, the lanternjaw's heuristic): leave a carcass
//!   to a body that hunts it, walk directly away until it is clear, resume.
//!
//! Distances are a body centre's to a point on the ring, in metres; a den's point is its
//! support face's column centre.

use std::f64::consts::{FRAC_PI_3, FRAC_PI_4, PI, TAU};

use cubarium_voxel::{DT, TICK_HZ, VoxelView};
use cubarium_voxel_flora::{FloraView, Site};

use crate::controller::Actions;
use crate::perception::{Track, class};
use crate::{Animal, Fauna, Pack};

/// A den's (or any face's) column centre, metres.
pub fn centre(site: Site, voxel_m: f64) -> (f64, f64) {
    (
        (f64::from(site.x) + 0.5) * voxel_m,
        (f64::from(site.z) + 0.5) * voxel_m,
    )
}

/// The ring delta from `a` to the point and its length.
fn delta(a: &Animal, x: f64, z: f64, ring: f64) -> (f64, f64, f64) {
    let (dx, dz) = crate::pose::wrapped_delta(a.pose.x, a.pose.z, x, z, ring);
    (dx, dz, (dx * dx + dz * dz).sqrt())
}

fn distance(a: &Animal, x: f64, z: f64, ring: f64) -> f64 {
    delta(a, x, z, ring).2
}

/// The pack table of a body's lineage.
fn table(fauna: &Fauna, a: &Animal) -> Option<Pack> {
    fauna.config.founder(a.founder?).pack
}

/// A body's reserve as a fraction of its cap (`reserve_cap · body`).
fn reserve_fraction(fauna: &Fauna, a: &Animal) -> f64 {
    let cap = crate::body::effective_config(&fauna.config, a).reserve_of(a.body);
    if cap > 0.0 { a.reserve / cap } else { 0.0 }
}

/// The ticks of a call ([`Pack::call_s`]).
fn call_ticks(t: &Pack) -> u32 {
    (t.call_s * f64::from(TICK_HZ)).round().max(0.0) as u32
}

/// Set a member's call (the target changed, or it struck).
pub(crate) fn call(fauna: &mut Fauna, i: usize) {
    let Some(t) = table(fauna, &fauna.animals[i]) else {
        return;
    };
    if let Some(m) = fauna.animals[i].pack.as_mut() {
        m.calling_ticks = call_ticks(&t);
    }
}

/// **The pack seam** (Brief R3): the body a striker's size window is measured against —
/// its own, plus every adult (not a pup) of its own pack within `pack_radius_m` of it. A
/// body outside a pack is measured against its own body alone.
pub(crate) fn window_body(fauna: &Fauna, i: usize, ring: f64) -> f64 {
    let a = &fauna.animals[i];
    let (Some(m), Some(t)) = (a.pack, table(fauna, a)) else {
        return a.body;
    };
    if m.pup {
        return a.body;
    }
    fauna
        .animals
        .iter()
        .filter(|o| {
            o.id == a.id
                || participates(o, m.pack)
                    && distance(a, o.pose.x, o.pose.z, ring) <= t.pack_radius_m
        })
        .map(|o| o.body)
        .sum()
}

/// **A hunt participant** (repair 3 item 5): an adult of pack `pack` that is awake and
/// wants food or is in an emergency. The one set the driver and flankers are picked from
/// and the body window sums (within `pack_radius_m`).
pub(crate) fn participates(o: &Animal, pack: u64) -> bool {
    o.pack.is_some_and(|om| om.pack == pack && !om.pup)
        && o.state != crate::State::Asleep
        && (o.eating || o.emergency)
}

/// After maintenance: calls count down, pups come of age, succession, empty packs dropped,
/// and provisioning at the den.
pub(crate) fn upkeep(fauna: &mut Fauna, view: &VoxelView<'_>) {
    if fauna.packs().is_empty() && fauna.animals.iter().all(|a| a.pack.is_none()) {
        return;
    }
    // Calls count down; a pup at its independence age joins the pack.
    for i in 0..fauna.animals.len() {
        let t = table(fauna, &fauna.animals[i]);
        let a = &mut fauna.animals[i];
        let Some(m) = a.pack.as_mut() else { continue };
        m.calling_ticks = m.calling_ticks.saturating_sub(1);
        if m.pup && t.is_some_and(|t| a.age_ticks as f64 * DT >= t.independence_age_s) {
            m.pup = false;
            fauna.ledger.pups_independent += 1;
        }
    }
    // Succession: a breeder that is gone (or no adult of the pack) gives way to the oldest
    // adult left, the lowest id on a tie. A pack with no member left is dropped.
    let ids: Vec<u64> = fauna.packs().iter().map(|p| p.id).collect();
    for id in ids {
        let mut members = 0usize;
        let mut breeder_ok = false;
        let mut oldest: Option<(u64, u64)> = None;
        let breeder = fauna.pack(id).and_then(|p| p.breeder);
        for a in &fauna.animals {
            let Some(m) = a.pack.filter(|m| m.pack == id) else {
                continue;
            };
            members += 1;
            if m.pup {
                continue;
            }
            if Some(a.id) == breeder {
                breeder_ok = true;
            }
            let older = oldest.is_none_or(|(age, oid)| {
                a.age_ticks > age || (a.age_ticks == age && a.id < oid)
            });
            if older {
                oldest = Some((a.age_ticks, a.id));
            }
        }
        if members == 0 {
            fauna.packs_mut().retain(|p| p.id != id);
            continue;
        }
        if !breeder_ok {
            if let Some(p) = fauna.pack_mut(id) {
                p.breeder = oldest.map(|(_, oid)| oid);
            }
        }
    }
    provision(fauna, view);
}

/// **Provisioning** (Brief R3; the minimal gut's repair 2 item 6): every adult within
/// `den_radius_m` of its den feeds a pup of its pack there, at most
/// `provision_rate_per_s` × its body per second.
///
/// 1. **Regurgitation first**: while its stomach holds raw food it passes that, raw, into
///    the stomach of the hungriest pup that **wants food** (its eating latch on) with
///    stomach room (lowest reserve fraction,
///    lowest id on a tie), capped by the pup's room `K − S`. Every food class moves by the
///    same fraction, so the food keeps its composition and its yield applies once, at the
///    pup's digestion. The giver's reserve floor does not apply. Booked `regurgitated`.
/// 2. **The reserve fallback**: with an empty stomach, an adult whose reserve is above
///    `provision_reserve_fraction` of its cap passes reserve to the hungriest pup with
///    reserve room (the old transfer, unchanged: no eating latch needed, repair 3 item 6), never below the fraction and never more than the pup can take.
///    The pup receives `provision_efficiency` of it — organic matter, mineral and energy
///    alike — and the rest is respired: its organic matter booked respired, its energy as
///    heat, its mineral shed to the giver's gut, as every respiration sheds it.
fn provision(fauna: &mut Fauna, view: &VoxelView<'_>) {
    let v = view.config.voxel_m;
    let ring = f64::from(view.config.width) * v;
    for g in 0..fauna.animals.len() {
        let giver = fauna.animals[g];
        let (Some(m), Some(t)) = (giver.pack, table(fauna, &giver)) else {
            continue;
        };
        if m.pup || !(t.provision_rate_per_s > 0.0) {
            continue;
        }
        let Some(den) = fauna.pack(m.pack).map(|p| p.den) else {
            continue;
        };
        let (cx, cz) = centre(den, v);
        if distance(&giver, cx, cz, ring) > t.den_radius_m {
            continue;
        }
        let rate = t.provision_rate_per_s * giver.body * DT;
        // The hungriest pup of its pack at the den that wants food, with room by `room`.
        let hungriest = |fauna: &Fauna, room: &dyn Fn(&Fauna, &Animal) -> f64, wanting: bool| -> Option<usize> {
            let mut best: Option<(f64, usize)> = None;
            for (k, p) in fauna.animals.iter().enumerate() {
                if !p.pack.is_some_and(|pm| pm.pack == m.pack && pm.pup)
                    || (wanting && !p.eating)
                    || distance(p, cx, cz, ring) > t.den_radius_m
                    || !(room(fauna, p) > 0.0)
                {
                    continue;
                }
                let f = reserve_fraction(fauna, p);
                if best.is_none_or(|(bf, _)| f < bf) {
                    best = Some((f, k));
                }
            }
            best.map(|(_, k)| k)
        };
        let stomach_room = |fauna: &Fauna, p: &Animal| -> f64 {
            let gut = p.founder.map(|f| fauna.config.founder(f).gut);
            gut.map_or(0.0, |gut| gut.capacity(p.body) - p.stomach.organic())
        };
        let held = giver.stomach.organic();
        if held > 0.0 {
            let Some(k) = hungriest(fauna, &stomach_room, true) else {
                continue;
            };
            let amount = rate.min(held).min(stomach_room(fauna, &fauna.animals[k]));
            if !(amount > 0.0) {
                continue;
            }
            let share = (amount / held).min(1.0);
            let mut moved = cubarium_voxel_flora::Taken::default();
            for food in crate::Food::ALL {
                let pool = &mut fauna.animals[g].stomach.pools[food.index()];
                let want = if share >= 1.0 { pool.organic } else { pool.organic * share };
                let part = crate::step::take_from_pool(pool, want);
                let to = &mut fauna.animals[k].stomach.pools[food.index()];
                to.organic += part.organic;
                to.mineral += part.mineral;
                to.energy += part.energy;
                moved.organic += part.organic;
                moved.mineral += part.mineral;
                moved.energy += part.energy;
            }
            let l = &mut fauna.ledger;
            l.regurgitated.organic += moved.organic;
            l.regurgitated.mineral += moved.mineral;
            l.regurgitated.energy += moved.energy;
            if let Some(f) = fauna.animals[g].founder {
                l.provision_by_founder[f.index()] += moved.organic;
            }
            if let Some(b) = fauna.animals[g].bout.as_mut() {
                b.kind = b.kind.max(crate::BoutKind::Provisioning);
            }
            crate::step::settle_full(fauna, k);
            continue;
        }
        // The reserve fallback: the old transfer, unchanged (repair 3 item 6) — pup reserve
        // room and the giver's floor; it does not need the pup's eating latch.
        let sc = crate::body::effective_config(&fauna.config, &giver);
        let floor = t.provision_reserve_fraction * sc.reserve_of(giver.body);
        if !(giver.reserve > floor) {
            continue;
        }
        let reserve_room = |fauna: &Fauna, p: &Animal| -> f64 {
            crate::body::effective_config(&fauna.config, p).reserve_of(p.body) - p.reserve
        };
        let Some(k) = hungriest(fauna, &reserve_room, false) else {
            continue;
        };
        let eff = t.provision_efficiency.clamp(0.0, 1.0);
        if !(eff > 0.0) {
            continue;
        }
        let room = reserve_room(fauna, &fauna.animals[k]);
        let give = rate.min(giver.reserve - floor).min(room / eff);
        if !(give > 0.0) {
            continue;
        }
        let taken = crate::step::take_from_reserve(&mut fauna.animals[g], give);
        let lost_organic = taken.organic * (1.0 - eff);
        let lost_mineral = taken.mineral * (1.0 - eff);
        let lost_energy = taken.energy * (1.0 - eff);
        {
            let p = &mut fauna.animals[k];
            p.reserve += taken.organic - lost_organic;
            p.mineral += taken.mineral - lost_mineral;
            p.energy += taken.energy - lost_energy;
        }
        fauna.animals[g].gut.mineral += lost_mineral;
        crate::step::book_provision_loss(fauna, lost_organic, lost_energy);
        fauna.ledger.provision_given_organic += taken.organic;
        if let Some(f) = fauna.animals[g].founder {
            fauna.ledger.provision_by_founder[f.index()] += taken.organic;
        }
        fauna.ledger.provision_received_organic += taken.organic - lost_organic;
    }
}

/// Whether body `i` wants to sleep now: out of its activity phase and not in an emergency
/// — the circadian stage's own reading for an awake body (the satiated rest went with the
/// minimal gut).
pub(crate) fn wants_rest(fauna: &Fauna, i: usize, clock_on: bool, daylight: f64) -> bool {
    let a = &fauna.animals[i];
    let Some(founder) = a.founder else {
        return false;
    };
    let phys = fauna.config.founder(founder);
    !phys.active.awake(clock_on, daylight, &fauna.config.life) && !a.emergency
}

/// A member under the chase floor (`chase_reserve_floor` of its cap) that neither wants
/// food nor is in an emergency: it stops a chase. Since the minimal gut a hungry member
/// is the reason for the hunt (repair 2 item 7: any adult in an emergency starts one), so
/// a member that wants food or is in an emergency does not count as spent (repair 3
/// item 5). **Deliberately**, a hungry member therefore has no exhaustion stop of its own:
/// only the chase timeout (`chase_give_up_s`) ends its chase; a stamina rule belongs to
/// predator-cadence tuning.
fn spent(fauna: &Fauna, b: &Animal, t: &Pack) -> bool {
    !b.eating && !b.emergency && reserve_fraction(fauna, b) < t.chase_reserve_floor
}

/// **The pack's hunt start** (repair 2 item 7): at least half its adults want food (the
/// eating latch), or any adult is in an emergency.
fn pack_wants(adults: &[Animal]) -> bool {
    let wanting = adults.iter().filter(|b| b.eating).count();
    !adults.is_empty() && (2 * wanting >= adults.len() || adults.iter().any(|b| b.emergency))
}

/// The circadian stage's den rule (Brief R3: "adults rest and sleep within
/// `den_radius_m`"): whether a pack adult under its pack heuristic that wants a rest must
/// first walk home — it is further than `den_radius_m` from its den and has been walking
/// home for less than `home_give_up_s`. Counts the walk.
pub(crate) fn must_walk_home(fauna: &mut Fauna, i: usize, voxel_m: f64, ring: f64) -> bool {
    let a = fauna.animals[i];
    let (Some(m), Some(t)) = (a.pack, table(fauna, &a)) else {
        return false;
    };
    if m.pup || a.state == crate::State::Asleep || !fauna.controllers.pack_tactic(a.id) {
        return false;
    }
    let Some(den) = fauna.pack(m.pack).map(|p| p.den) else {
        return false;
    };
    let (cx, cz) = centre(den, voxel_m);
    if distance(&a, cx, cz, ring) <= t.den_radius_m {
        return false;
    }
    let give_up = cubarium_voxel::ticks_at_least_one(t.home_give_up_s);
    if u64::from(a.homing_ticks) >= give_up {
        return false;
    }
    fauna.animals[i].homing_ticks = a.homing_ticks.saturating_add(1);
    true
}

/// The signed angle from `heading` to `want`, in `(-π, π]`.
fn angle_to(heading: f64, want: f64) -> f64 {
    let d = (want - heading).rem_euclid(TAU);
    if d > PI { d - TAU } else { d }
}

/// Steer toward a heading: turn in place while it is more than 60° off, else walk at
/// `forward` turning toward it (full turn beyond 45°).
fn head(a: &Animal, want: f64, forward: f64, feed: f64) -> Actions {
    let diff = angle_to(a.pose.heading_rad, want);
    Actions {
        forward: if diff.abs() > FRAC_PI_3 { 0.0 } else { forward },
        turn: (diff / FRAC_PI_4).clamp(-1.0, 1.0),
        feed,
    }
}

/// How far from a body's own face its node may be, and how far from a prey's face the
/// node a hunter takes it from, columns: the disc's radius, and the radius plus the
/// strike's reach (a prey is struck from beside it, not stood on).
fn node_radii(geometry: &crate::Body, h: &crate::Hunter, v: f64) -> (u32, u32) {
    let r = geometry.footprint_radius();
    let reach = h.strike_reach_length_fraction * geometry.length_m;
    let cols = |m: f64| if m.is_finite() && v > 0.0 { (m / v).ceil().max(0.0) as u32 } else { 0 };
    (cols(r), cols(r + reach))
}

/// **STRIKE, the height band**: whether a hunter of `geometry` standing on a surface at `s0`
/// meets, with its jaw band (`[s0, s0 + jaw_band_height_fraction × height]`, the strike
/// capsule's band), a body whose base is at `base` and which is `height` tall.
fn band_meets(geometry: &crate::Body, h: &crate::Hunter, s0: f64, base: f64, height: f64) -> bool {
    base < s0 + h.jaw_band_height_fraction * geometry.height_m && s0 < base + height
}

/// The height of body `id` as it is now; 0 when it is gone.
fn height_of(fauna: &Fauna, id: u64) -> f64 {
    fauna.view().animal(id).and_then(|o| crate::body_of(&fauna.config, o)).map_or(0.0, |g| g.height_m)
}

/// **The node a member takes body `o` from** (STRIKE): on its graph within the strike's
/// radius of where it was known ([`node_radii`]), on a face whose jaw band meets its body
/// ([`band_meets`], `height` tall). `None` when there is none: a perched or ledged prey above
/// (or below) every face a member could stand on beside it is out of reach.
fn strike_node(graph: &crate::encounter::RouteGraph, o: &Track, height: f64, geometry: &crate::Body, h: &crate::Hunter, v: f64) -> Option<usize> {
    let (_, prey_r) = node_radii(geometry, h, v);
    graph.node_near_where(o.face(), (o.x, o.z), prey_r, |f| band_meets(geometry, h, crate::surface_m(f.y, v), o.y_m, height))
}

/// **STRIKE, the last metres** for the member `a` at `i` on its target `o` (as it knows it):
/// within `close_in_m` of it, with the prey in its jaw band from where it stands and a
/// straight walk to it on the disc's graph, it leaves its route and its flank point and
/// steers to its **stand-off** — the jaw's reach plus `stand_off_reach_fraction` of the
/// strike's reach from the prey's centre — facing it, as a wolf or a lion ends its approach
/// in reach and facing its prey:
/// - inside its outline (the prey's centre nearer than where a capsule facing it begins,
///   the jaw's reach less both radii) it steps off, straight away from the prey, to the
///   stand-off;
/// - else, not facing it (its centre outside the capsule's half-width), it turns in place,
///   paying the rotation as any turn;
/// - facing it, it walks on to the stand-off, never past it: no step outruns the distance
///   left (`cruise × controller period` is one sampling's walk), and the rush's sprint is the
///   most it asks.
///
/// Feed is held throughout: a prey in the capsule is struck. `None` when the rule does not
/// apply (off, out of range, the target gone, out of band, or a wall or a ledge between).
#[allow(clippy::too_many_arguments)]
fn close_in(
    fauna: &mut Fauna,
    view: &VoxelView<'_>,
    founder: crate::Founder,
    a: &Animal,
    o: &Track,
    geometry: &crate::Body,
    h: &crate::Hunter,
    t: &Pack,
    ring: f64,
) -> Option<Actions> {
    if !(t.close_in_m > 0.0) {
        return None;
    }
    let (dx, dz, d) = delta(a, o.x, o.z, ring);
    if d > t.close_in_m {
        return None;
    }
    let prey = fauna.view().animal(o.id).copied()?;
    let pg = crate::body_of(&fauna.config, &prey)?;
    let v = view.config.voxel_m;
    if a.mobility.climb.is_some() || !band_meets(geometry, h, crate::perception::base_m(a, v), o.y_m, pg.height_m) {
        return None;
    }
    let (own_r, _) = node_radii(geometry, h, v);
    let graph = fauna.pack_routes(view, founder);
    if let Some(from) = graph.node_near(a.site, (a.pose.x, a.pose.z), own_r) {
        if !graph.walk_line(from, (a.pose.x, a.pose.z), (o.x, o.z)) {
            return None;
        }
    }
    let manifest = fauna.config.manifest(founder);
    let length = geometry.length_m;
    let jaw = h.jaw_forward_length_fraction * length;
    let reach = h.strike_reach_length_fraction * length;
    let radii = geometry.footprint_radius() + pg.footprint_radius();
    let stand = jaw + t.stand_off_reach_fraction * reach;
    let near = (jaw - radii).max(0.0);
    let step = (manifest.cruise_m_per_s * manifest.controller_period_s).max(1e-9);
    fauna.ledger.pack_close_in_samples += 1;
    if d < near {
        fauna.ledger.pack_step_offs += 1;
        let away = if d > 1e-9 { (-dx).atan2(-dz) } else { a.pose.heading_rad };
        return Some(head(a, away, ((stand - d) / step).min(1.0), 1.0));
    }
    let diff = angle_to(a.pose.heading_rad, dx.atan2(dz));
    let aligned = diff.abs() <= (radii / d.max(radii)).asin();
    let forward = if aligned {
        let most = crate::vigil::stalk(fauna, a, o, d, 1.0).unwrap_or(1.0);
        ((d - stand) / step).clamp(0.0, most.max(0.0))
    } else {
        fauna.ledger.pack_pivots += 1;
        0.0
    };
    Some(Actions {
        forward,
        turn: (diff / FRAC_PI_4).clamp(-1.0, 1.0),
        feed: 1.0,
    })
}

/// **The next step on the route** (bellwing3 item 1, bellwing4): `None` when the member
/// can walk straight to `goal` on the disc's graph (or is off the graph); else the centre
/// of the next node on its route to the node it would strike the target from — the
/// target where the member last knew it to be (SENSES-CORE).
#[allow(clippy::too_many_arguments)]
fn route_step(
    fauna: &mut Fauna,
    view: &VoxelView<'_>,
    founder: crate::Founder,
    own: u64,
    t: &Pack,
    cruise_m_per_s: f64,
    a: &Animal,
    o: &Track,
    geometry: &crate::Body,
    h: &crate::Hunter,
    goal: (f64, f64),
) -> Option<(f64, f64)> {
    let v = view.config.voxel_m;
    let (own_r, _) = node_radii(geometry, h, v);
    let graph = fauna.pack_routes(view, founder);
    let from = graph.node_near(a.site, (a.pose.x, a.pose.z), own_r)?;
    if graph.walk_line(from, (a.pose.x, a.pose.z), goal) {
        return None;
    }
    // STRIKE: the node it would strike from, on a face whose jaw band meets the prey.
    let target = strike_node(&graph, o, height_of(fauna, o.id), geometry, h, v)?;
    let (graph, field) = fauna.pack_path(view, founder, own, target, reach_steps(t, cruise_m_per_s, v));
    graph.next_toward(&field, from).map(|j| graph.centre(j))
}

/// A pack's reach on foot in route steps (one voxel each): what it covers in a chase,
/// `cruise × chase_give_up_s`, times `route_reach_fraction`.
fn reach_steps(t: &Pack, cruise_m_per_s: f64, voxel_m: f64) -> u32 {
    let m = cruise_m_per_s * t.chase_give_up_s * t.route_reach_fraction;
    if m.is_finite() && m > 0.0 && voxel_m > 0.0 {
        (m / voxel_m).floor().min(f64::from(u32::MAX)) as u32
    } else {
        0
    }
}

// ------------------------------------------------------------------ the pack's senses

/// The pursuit's memory, ticks ([`crate::PerceptionConfig::give_up_s`]).
fn give_up_ticks(fauna: &Fauna) -> u64 {
    cubarium_voxel::ticks_at_least_one(fauna.config.perception.give_up_s)
}

/// **What the member at `i` knows of its prey** (SENSES-CORE): the prey tracks of its own
/// record, and those of every awake adult packmate standing within its hearing range — the
/// sighting shared by the pack's calls, not pooled across the whole pack (the branch's
/// telepathy). One track per body, the freshest; positions are where it was last sensed.
pub(crate) fn known(fauna: &Fauna, i: usize, ring: f64) -> Vec<Track> {
    let a = &fauna.animals[i];
    let prey = |p: &crate::Percept| -> Vec<Track> { p.tracks().iter().filter(|t| t.class == class::PREY).copied().collect() };
    let mut out: Vec<Track> = fauna.percept(a.id).map(prey).unwrap_or_default();
    let Some(m) = a.pack else {
        return out;
    };
    let hearing = a.founder.map_or(0.0, |f| fauna.config.founder(f).senses.hearing_m);
    if !(hearing > 0.0) {
        return out;
    }
    for o in &fauna.animals {
        if o.id == a.id
            || !o.pack.is_some_and(|om| om.pack == m.pack && !om.pup)
            || o.state == crate::State::Asleep
            || distance(a, o.pose.x, o.pose.z, ring) > hearing
        {
            continue;
        }
        let Some(p) = fauna.percept(o.id) else { continue };
        for t in prey(p) {
            match out.iter_mut().find(|k| k.id == t.id) {
                Some(k) if t.tick > k.tick => *k = t,
                Some(_) => {}
                None => out.push(t),
            }
        }
    }
    out
}

/// Whether the member at `i` could take a body it knows of where it was
/// ([`crate::perception::in_reach`]): not climbing, and on the ground or within the jaw
/// band over the support under it — a perched or low-hovering bellwing in reach counts
/// (Wrysk: hunters eat bellwings "grounded or in reach").
pub(crate) fn takeable(fauna: &Fauna, i: usize, t: &Track) -> bool {
    crate::perception::in_reach(&fauna.config, &fauna.animals[i], t)
}

/// What the member at `i` knows of body `tid` within the pursuit's memory: the freshest of
/// what it and its packmates in hearing sensed, and of its own focus (the last-known point).
fn knows_of(fauna: &Fauna, i: usize, tid: u64, ring: f64) -> Option<Track> {
    let a = &fauna.animals[i];
    let mut best = known(fauna, i, ring).into_iter().find(|t| t.id == tid);
    if let Some(f) = fauna.percept(a.id).map(|p| p.focus).filter(|f| f.known && f.id == tid) {
        if best.is_none_or(|b| f.tick > b.tick) {
            best = Some(f);
        }
    }
    best.filter(|t| t.age(fauna.tick) <= give_up_ticks(fauna))
}

// ------------------------------------------------------------------ PACK-TUNE: the prey choice

/// **The pack's prospect on body `o`** (PACK-TUNE, [`crate::Pack`]'s `select_*`), sensed as
/// `t`, with `n` of its members at hand, in [0, 1]: 0 when the member at `i` could not take
/// it there ([`takeable`]) or, for a hunkering lineage, when `n` cannot open it with the kin
/// beside it ([`crate::Hunker::members_to_open`]; it is assumed to clamp down when the pack
/// comes, as a musk ox stands); else the share of their pry left over its resistance, × the
/// kin, awareness, wound and flight factors. Every factor reads the body as it is now.
pub(crate) fn prospect(fauna: &Fauna, i: usize, o: &Animal, t: &Track, n: u32, ring: f64) -> f64 {
    prospect_of(fauna, i, o, t, n, ring, true)
}

/// [`prospect`], with the awareness factor only when `awareness` (CHURN's switch leaves it
/// out on both sides: a hunt alerts its target, and would alert any prey it turned to).
fn prospect_of(fauna: &Fauna, i: usize, o: &Animal, t: &Track, n: u32, ring: f64, awareness: bool) -> f64 {
    let a = &fauna.animals[i];
    let (Some(of), Some(m), Some(pk)) = (o.founder, a.pack, a.founder.and_then(|f| fauna.config.founder(f).pack)) else {
        return 0.0;
    };
    if !takeable(fauna, i, t) {
        return 0.0;
    }
    // A flyer counts only where the strike could take it now (S3's `is_prey_of` rule: in
    // flight, its body inside the member's jaw band over the member's own base), so the
    // `select_airborne_factor` below weighs only a bellwing in reach.
    if let Some(h) = a.founder.and_then(|f| fauna.config.founder(f).hunter) {
        if !crate::predation::flyer_takeable(&fauna.config, a, &h, of, o) {
            return 0.0;
        }
    }
    let near = |r: f64| -> u32 {
        fauna
            .animals
            .iter()
            .filter(|k| k.id != o.id && k.founder == Some(of) && k.state != crate::State::Asleep && distance(o, k.pose.x, k.pose.z, ring) <= r)
            .count() as u32
    };
    let mut p = 1.0;
    let hk = fauna.config.founder(of).hunker;
    let opened = fauna.percept(o.id).is_some_and(|r| fauna.tick < r.vigil.opened_until);
    if hk.enabled && !opened {
        let adult = fauna.config.founder(of).core.birth_body;
        let ringed = near(hk.ring_m);
        match hk.members_to_open(o.body, adult, ringed) {
            Some(need) if need <= n => p *= 1.0 - hk.resist_of(o.body, adult, ringed) / (f64::from(n) * hk.pry_per_member_s),
            _ => return 0.0,
        }
    }
    p *= pk.select_kin_factor.powi(near(pk.select_kin_m).min(pk.select_kin_max) as i32);
    let now = fauna.tick;
    let record = fauna.percept(o.id);
    let aware = fauna
        .animals
        .iter()
        .filter(|b| b.pack.is_some_and(|bm| bm.pack == m.pack && !bm.pup))
        .any(|b| crate::vigil::aware_of(&fauna.config, record, b.id, now));
    if aware && awareness {
        p *= pk.select_aware_factor;
    }
    p *= 1.0 + pk.select_wound_gain * o.wound.max(0.0);
    if o.mobility.mode == crate::MobilityMode::Fly {
        p *= pk.select_airborne_factor;
    }
    p.clamp(0.0, 1.0)
}

/// The members of pack `own` at hand at `(x, z)`: awake adults that take part (or are the
/// member `me`) within `radius`.
fn at_hand(fauna: &Fauna, own: u64, me: u64, x: f64, z: f64, radius: f64, ring: f64) -> u32 {
    fauna
        .animals
        .iter()
        .filter(|b| b.pack.is_some_and(|m| m.pack == own && !m.pup) && (b.id == me || participates(b, own)) && distance(b, x, z, ring) <= radius)
        .count() as u32
}

/// Whether pack `own` shuns body `id` now (PACK-TUNE: dropped alive within `shun_s`).
fn shuns(fauna: &Fauna, own: u64, id: u64) -> bool {
    fauna.pack(own).is_some_and(|p| p.shunned.iter().any(|&(s, until)| s == id && fauna.tick < until))
}

/// Pack `own` shuns body `id` for `shun_s` (the slot ending soonest is reused).
fn shun(fauna: &mut Fauna, own: u64, id: u64, shun_s: f64) {
    let until = fauna.tick + cubarium_voxel::ticks_at_least_one(shun_s);
    if let Some(p) = fauna.pack_mut(own) {
        let k = (0..p.shunned.len()).min_by_key(|&k| if p.shunned[k].0 == id { 0 } else { p.shunned[k].1.max(1) }).unwrap_or(0);
        p.shunned[k] = (id, until);
    }
}

/// Whether any awake adult of pack `own` knows of `tid` within the pursuit's memory.
fn pack_knows(fauna: &Fauna, own: u64, tid: u64, ring: f64) -> bool {
    fauna.animals.iter().enumerate().any(|(k, b)| {
        b.pack.is_some_and(|m| m.pack == own && !m.pup) && b.state != crate::State::Asleep && knows_of(fauna, k, tid, ring).is_some()
    })
}

/// A point a metre up the strongest scent of its prey the member at `i` smells (its
/// lineage's [`crate::SenseTable::smell_min`] or above, on a channel its prey lay:
/// [`crate::perception::prey_channels`]); `None` with none.
fn scent_step(fauna: &Fauna, i: usize, founder: crate::Founder) -> Option<(f64, f64)> {
    let a = &fauna.animals[i];
    let floor = fauna.config.founder(founder).senses.smell_min;
    if !(floor > 0.0) {
        return None;
    }
    let p = fauna.percept(a.id)?;
    let chans = crate::perception::prey_channels(&fauna.config, founder);
    let best = crate::ScentChannel::ALL
        .into_iter()
        .filter(|c| chans[c.index()])
        .map(|c| p.scent[c.index()])
        .filter(|r| r.level >= floor && (r.gx != 0.0 || r.gz != 0.0) && r.gx.is_finite() && r.gz.is_finite())
        .max_by(|x, y| x.level.total_cmp(&y.level))?;
    let g = (best.gx * best.gx + best.gz * best.gz).sqrt();
    Some((a.pose.x + best.gx / g, a.pose.z + best.gz / g))
}

/// **The pack's leader** (VEILHOUND-DIAG's leading breeder, VEILHOUND-SEARCH's draw): the
/// breeder when it is awake and wants food or is in an emergency, else the lowest-id adult
/// that is.
fn pack_leader(rec: &crate::PackRecord, adults: &[Animal]) -> Option<u64> {
    let hungry = |b: &&Animal| b.state != crate::State::Asleep && (b.eating || b.emergency);
    rec.breeder
        .and_then(|id| adults.iter().find(|b| b.id == id))
        .filter(hungry)
        .or_else(|| adults.iter().filter(hungry).min_by_key(|b| b.id))
        .map(|b| b.id)
}

/// The leader a hungry member of pack `own` follows while its pack is drawn to a prey (a
/// draw recorded, no target, the pack wanting food) — another body. `None` otherwise: the
/// pack keeps main's rule.
fn drawn_leader(fauna: &Fauna, own: u64, rec: &crate::PackRecord, adults: &[Animal], me: u64) -> Option<Animal> {
    let p = fauna.pack(own)?;
    if (p.drawn.is_none() && !p.drawn_scent) || p.target.is_some() || !pack_wants(adults) {
        return None;
    }
    let l = pack_leader(rec, adults).filter(|&l| l != me)?;
    adults.iter().find(|b| b.id == l).copied()
}

/// **The draw** (VEILHOUND-SEARCH, SENSES-CORE) for the pack's leader at `i`: the nearest
/// prey by route it knows of ([`known`]: its own sensing and its packmates' in hearing) that
/// it takes (in reach, its window, outside every other territory, not the abandoned target)
/// and stands on the leader's route component; the point to head for on the way — straight
/// at where it was last sensed when the line is walkable, else the next node of the route.
/// Recorded as the pack's [`crate::PackRecord::drawn`] (`before` is the draw at its last
/// sample: a new prey counts a draw under each sense that has it). Failing that, a point up
/// its prey's scent ([`scent_step`], [`crate::PackRecord::drawn_scent`], counted under smell
/// when it begins). `None` (and the record cleared) with neither.
#[allow(clippy::too_many_arguments)]
fn draw_step(
    fauna: &mut Fauna,
    i: usize,
    view: &VoxelView<'_>,
    own: u64,
    founder: crate::Founder,
    t: &Pack,
    geometry: &crate::Body,
    h: &crate::Hunter,
    before: Option<u64>,
    before_scent: bool,
) -> Option<(f64, f64)> {
    let a = fauna.animals[i];
    let v = view.config.voxel_m;
    let ring = f64::from(view.config.width) * v;
    let sensed: Vec<Track> = known(fauna, i, ring).into_iter().filter(|k| takeable(fauna, i, k)).collect();
    let set = |fauna: &mut Fauna, drawn: Option<(u64, u8)>, scent: bool| {
        if let Some((id, modes)) = drawn.filter(|d| Some(d.0) != before) {
            for (k, bit) in crate::perception::MODALITIES.iter().enumerate() {
                if modes & bit != 0 {
                    fauna.ledger.pack_draws_by_sense[k] += 1;
                }
            }
            crate::perception::approach(fauna, a.id, id);
        }
        if scent && !before_scent {
            fauna.ledger.pack_draws_by_sense[3] += 1;
        }
        if let Some(p) = fauna.pack_mut(own) {
            p.drawn = drawn.map(|d| d.0);
            p.drawn_scent = scent;
        }
    };
    let by_scent = |fauna: &mut Fauna| -> Option<(f64, f64)> {
        let go = scent_step(fauna, i, founder);
        set(fauna, None, go.is_some());
        go
    };
    if sensed.is_empty() {
        return by_scent(fauna);
    }
    let graph = fauna.pack_routes(view, founder);
    let (own_r, prey_r) = node_radii(geometry, h, v);
    let Some(from) = graph.node_near(a.site, (a.pose.x, a.pose.z), own_r) else {
        return by_scent(fauna);
    };
    let abandoned = fauna.pack(own).and_then(|p| p.abandoned);
    let window = window_body(fauna, i, ring);
    let component = graph.components[from];
    let mut goals: Vec<(usize, u64, u8, (f64, f64))> = Vec::new();
    for k in &sensed {
        let Some(o) = fauna.view().animal(k.id).copied() else { continue };
        if abandoned == Some(k.id)
            || !crate::predation::is_prey_in_window(&fauna.config, &a, &o, window)
            || trespass(fauna, own, k.x, k.z, t, v, ring)
        {
            continue;
        }
        let Some(j) = graph.node_near(k.face(), (k.x, k.z), prey_r) else { continue };
        if graph.components[j] == component {
            goals.push((j, k.id, k.modes, (k.x, k.z)));
        }
    }
    if goals.is_empty() {
        return by_scent(fauna);
    }
    // Route distances from the leader over its whole component — the pack's route field to
    // the leader, which its followers walk on too ([`follow_step`]).
    let (graph, field) = fauna.pack_path(view, founder, own, from, u32::MAX);
    let Some((d, j, id, s, xy)) = goals
        .into_iter()
        .filter_map(|(j, id, s, xy)| field.get(&j).map(|&d| (d, j, id, s, xy)))
        .min_by(|p, q| (p.0, p.2).cmp(&(q.0, q.2)))
    else {
        return by_scent(fauna);
    };
    set(fauna, Some((id, s)), false);
    if d == 0 || graph.walk_line(from, (a.pose.x, a.pose.z), xy) {
        return Some(xy);
    }
    // Back along the field from the prey's node to the leader's next node.
    let mut k = j;
    while field.get(&k).is_some_and(|&dk| dk > 1) {
        k = graph.next_toward(&field, k)?;
    }
    Some(graph.centre(k))
}

/// **A follower's step** (VEILHOUND-SEARCH) toward the draw's `leader`: straight at it when
/// the line is walkable on the route graph, else the next node of the pack's route field to
/// the leader's node (the field [`draw_step`] builds); straight at it off the graph.
fn follow_step(fauna: &mut Fauna, i: usize, view: &VoxelView<'_>, own: u64, founder: crate::Founder, leader: &Animal, geometry: &crate::Body, h: &crate::Hunter) -> (f64, f64) {
    let a = fauna.animals[i];
    let v = view.config.voxel_m;
    let goal_xy = (leader.pose.x, leader.pose.z);
    let graph = fauna.pack_routes(view, founder);
    let (own_r, _) = node_radii(geometry, h, v);
    let (Some(from), Some(to)) = (
        graph.node_near(a.site, (a.pose.x, a.pose.z), own_r),
        graph.node_near(leader.site, goal_xy, own_r),
    ) else {
        return goal_xy;
    };
    if from == to || graph.walk_line(from, (a.pose.x, a.pose.z), goal_xy) {
        return goal_xy;
    }
    let (graph, field) = fauna.pack_path(view, founder, own, to, u32::MAX);
    graph.next_toward(&field, from).map_or(goal_xy, |j| graph.centre(j))
}

/// Walk to a point; stop (turning to face it) within `stop_m`.
fn go_to(a: &Animal, x: f64, z: f64, ring: f64, stop_m: f64, feed: f64) -> Actions {
    let (dx, dz, d) = delta(a, x, z, ring);
    let want = dx.atan2(dz);
    let forward = if d <= stop_m { 0.0 } else { 1.0 };
    head(a, want, forward, feed)
}

/// Whether a point lies inside another live pack's territory.
fn trespass(fauna: &Fauna, own: u64, x: f64, z: f64, t: &Pack, voxel_m: f64, ring: f64) -> bool {
    fauna.packs().iter().filter(|p| p.id != own).any(|p| {
        let (cx, cz) = centre(p.den, voxel_m);
        let (dx, dz) = crate::pose::wrapped_delta(cx, cz, x, z, ring);
        (dx * dx + dz * dz).sqrt() < t.territory_radius_m
    })
}

/// **Territory** (Brief R3): a pack member's held actions never carry it into another
/// pack's `territory_radius_m`. Within that radius plus what it could cover before its
/// next sampling and a strike's lunge, a body facing into the territory stands, turns
/// away and holds no feed (so no lunge carries it in); facing out, it goes as it likes.
fn keep_out(fauna: &Fauna, i: usize, own: u64, act: Actions, t: &Pack, guard_m: f64, voxel_m: f64, ring: f64) -> Actions {
    let a = &fauna.animals[i];
    let (fx, fz) = a.pose.forward();
    for p in fauna.packs().iter().filter(|p| p.id != own) {
        let (cx, cz) = centre(p.den, voxel_m);
        let (dx, dz, d) = delta(a, cx, cz, ring);
        if d >= t.territory_radius_m + guard_m || !(d > 0.0) {
            continue;
        }
        if dx * fx + dz * fz > 0.0 {
            // The den's side: positive cross is to the right (a positive turn faces it).
            let right = fz * dx - fx * dz > 0.0;
            return Actions {
                forward: 0.0,
                turn: if right { -1.0 } else { 1.0 },
                feed: 0.0,
            };
        }
    }
    act
}

/// Work under [`WORK_EPS`] is no progress (a float tolerance).
const WORK_EPS: f64 = 1e-9;

/// **The work done on body `o`** (CHURN, the chase's progress): its wound, plus the pack's pry
/// on it, plus 1 while it is turned over (a pry completed resets to 0 as it opens).
fn work_on(fauna: &Fauna, o: &Animal) -> f64 {
    let (pry, open) = fauna.percept(o.id).map_or((0.0, false), |p| (p.vigil.pry, fauna.tick < p.vigil.opened_until));
    o.wound.max(0.0) + pry + if open { 1.0 } else { 0.0 }
}

/// The distance from `(x, z)` to the nearest member of pack `own` taking part (or `me`).
fn nearest_member(fauna: &Fauna, own: u64, me: u64, x: f64, z: f64, ring: f64) -> f64 {
    fauna
        .animals
        .iter()
        .filter(|b| b.pack.is_some_and(|m| m.pack == own && !m.pup) && (b.id == me || participates(b, own)))
        .map(|b| distance(b, x, z, ring))
        .fold(f64::INFINITY, f64::min)
}

/// **The choice score** (PACK-TUNE) of body `o`, known as `k`, to the member at `i` with `n`
/// members at hand: prospect × body^`select_body_exp` / (1 + distance / `select_judge_m`),
/// the member's own distance.
#[allow(clippy::too_many_arguments)]
fn score(fauna: &Fauna, i: usize, o: &Animal, k: &Track, n: u32, t: &Pack, ring: f64) -> f64 {
    let a = &fauna.animals[i];
    let d = distance(a, k.x, k.z, ring);
    prospect(fauna, i, o, k, n, ring) * o.body.max(0.0).powf(t.select_body_exp) / (1.0 + d / t.select_judge_m.max(1e-9))
}

/// **The switch's worth** (CHURN2) of body `o`, known as `k`, to the pack: prospect ×
/// body^`select_body_exp`, the prospect with the pack's `n` participants (the same count for
/// the target and every other prey, so no member weighs a pair differently from another)
/// and no awareness factor (a hunt alerts its target, and would alert any it turned to). No
/// distance: which prey is nearest a member does not steal a hunt; distance only gates it.
#[allow(clippy::too_many_arguments)]
fn worth(fauna: &Fauna, i: usize, o: &Animal, k: &Track, n: u32, t: &Pack, ring: f64) -> f64 {
    prospect_of(fauna, i, o, k, n, ring, false) * o.body.max(0.0).powf(t.select_body_exp)
}

/// CHURN2: how a [`choose`] weighs its candidates. `Choice` is the hunt start's score; a
/// `Switch` weighs [`worth`] and takes only a prey whose pack distance (its nearest member
/// taking part) is at most `within_m`.
#[derive(Clone, Copy)]
enum Weigh {
    Choice,
    Switch { within_m: f64 },
}

/// A target chosen: its score, id, face and the senses that had it.
#[derive(Clone, Copy)]
struct Choice {
    score: f64,
    id: u64,
    site: Site,
    modes: u8,
}

/// **The prey choice** (PACK-TUNE) of the member at `i` with `n` members at hand: of the
/// prey it knows of (its own record and its packmates' in hearing, [`known`]), where last
/// sensed, takeable there, in its window, outside every other territory, not `exclude`, the
/// pack's abandoned target or a shunned one, with a prospect of at least
/// `select_min_prospect` and a score of at least `floor` (`weigh`: the choice's [`score`], or
/// a switch's [`worth`] gated by the pack's distance, a low prospect then not counted
/// declined): the best it can reach on foot within the chase — on the taker's walking
/// route graph, at most `cruise × chase_give_up_s × route_reach_fraction` away by route
/// (bellwing-finding item 3, bellwing3 item 1: grazers on a cliff top 3 m over the pack drew
/// 74 timed-out chases in seed 1). Also the pack's abandoned target's score as it sees it now
/// (`None` unsensed), CHURN's readout.
#[allow(clippy::too_many_arguments)]
fn choose(
    fauna: &mut Fauna,
    i: usize,
    view: &VoxelView<'_>,
    own: u64,
    founder: crate::Founder,
    t: &Pack,
    geometry: &crate::Body,
    h: &crate::Hunter,
    cruise_m_per_s: f64,
    n: u32,
    ring: f64,
    exclude: Option<u64>,
    floor: f64,
    weigh: Weigh,
) -> (Option<Choice>, Option<f64>) {
    let switching = matches!(weigh, Weigh::Switch { .. });
    let weigh_of = |fauna: &Fauna, o: &Animal, k: &Track| match weigh {
        Weigh::Choice => score(fauna, i, o, k, n, t, ring),
        Weigh::Switch { .. } => worth(fauna, i, o, k, n, t, ring),
    };
    let a = fauna.animals[i];
    let v = view.config.voxel_m;
    let window = window_body(fauna, i, ring);
    let sensed: Vec<Track> = known(fauna, i, ring).into_iter().filter(|k| takeable(fauna, i, k)).collect();
    let abandoned = fauna.pack(own).and_then(|p| p.abandoned);
    let mut prev_score = None;
    let mut candidates: Vec<(f64, u64, Site, (f64, f64), u8, Track)> = Vec::new();
    for k in &sensed {
        let Some(o) = fauna.view().animal(k.id).copied() else { continue };
        if abandoned == Some(o.id) {
            prev_score = Some(weigh_of(fauna, &o, k));
        }
        if o.id == a.id || Some(o.id) == exclude || abandoned == Some(o.id) || shuns(fauna, own, o.id) {
            continue;
        }
        if !crate::predation::is_prey_in_window(&fauna.config, &a, &o, window) || trespass(fauna, own, k.x, k.z, t, v, ring) {
            continue;
        }
        let pr = if switching { prospect_of(fauna, i, &o, k, n, ring, false) } else { prospect(fauna, i, &o, k, n, ring) };
        if pr < t.select_min_prospect || !(pr > 0.0) {
            if !switching {
                fauna.ledger.pack_declined += 1;
            }
            continue;
        }
        if let (Weigh::Switch { within_m }, Some(m)) = (weigh, a.pack) {
            if nearest_member(fauna, m.pack, a.id, k.x, k.z, ring) > within_m {
                continue;
            }
        }
        let s = weigh_of(fauna, &o, k);
        if s >= floor {
            candidates.push((-s, o.id, k.face(), (k.x, k.z), k.modes, *k));
        }
    }
    if candidates.is_empty() {
        return (None, prev_score);
    }
    candidates.sort_by(|x, y| x.0.total_cmp(&y.0).then(x.1.cmp(&y.1)));
    let routes = fauna.pack_routes(view, founder);
    let (own_r, _) = node_radii(geometry, h, v);
    let Some(from) = routes.node_near(a.site, (a.pose.x, a.pose.z), own_r) else {
        return (None, prev_score);
    };
    let field = routes.distances(from, reach_steps(t, cruise_m_per_s, v));
    // STRIKE: reached at a node it could strike it from (its jaw band meeting the body): a
    // prey perched or ledged out of every such face is out of reach.
    let best = candidates
        .into_iter()
        .find(|c| strike_node(&routes, &c.5, height_of(fauna, c.1), geometry, h, v).is_some_and(|j| field.contains_key(&j)))
        .map(|c| Choice { score: -c.0, id: c.1, site: c.2, modes: c.4 });
    (best, prev_score)
}

/// **The commitment** of pack `own` to `c`, taken by the member at `i` with `n` members at
/// hand: the target, its chase and progress clocks, the counters by sense (and the draw's when
/// `was_drawn` or the pack's draw stands), the taker's focus and call, and the event (`prev`
/// the target it replaces, scored `prev_score`).
#[allow(clippy::too_many_arguments)]
fn commit(fauna: &mut Fauna, i: usize, own: u64, c: Choice, was_drawn: bool, n: u32, prev: Option<u64>, prev_score: Option<f64>, ring: f64) {
    let a = fauna.animals[i];
    let now = fauna.tick;
    let (best_m, work) = match fauna.view().animal(c.id).copied() {
        Some(o) => (nearest_member(fauna, own, a.id, o.pose.x, o.pose.z, ring), work_on(fauna, &o)),
        None => (f64::INFINITY, 0.0),
    };
    let mut drawn = false;
    if let Some(p) = fauna.pack_mut(own) {
        p.target = Some(c.id);
        p.chase_since_tick = now;
        p.chase_progress_tick = now;
        p.chase_best_m = best_m;
        p.chase_work = work;
        p.target_site = Some(c.site);
        p.abandoned = None;
        drawn = p.drawn.take().is_some() || was_drawn;
    }
    fauna.ledger.pack_chases += 1;
    for (k, bit) in crate::perception::MODALITIES.iter().enumerate() {
        if c.modes & bit != 0 {
            fauna.ledger.pack_chases_by_sense[k] += 1;
        }
    }
    if drawn {
        fauna.ledger.pack_draw_chases += 1;
    }
    crate::perception::approach(fauna, a.id, c.id);
    fauna.percepts.entry(a.id).or_default().set_focus(c.id, now);
    fauna.log_pack_event(crate::PackEvent::ChaseStart {
        tick: now,
        pack: own,
        participants: n,
        taker: a.id,
        target: c.id,
        score: c.score,
        prev,
        prev_score,
    });
    call(fauna, i);
}

/// **The scripted pack tactic** for body `i` (a pack member whose controller opts in),
/// over the actions its own heuristic just answered (`held`). `None` leaves them as they
/// are (a body with no pack, or no pack table).
pub(crate) fn steer(
    fauna: &mut Fauna,
    i: usize,
    view: &VoxelView<'_>,
    fv: &FloraView<'_>,
    clock_on: bool,
    daylight: f64,
    held: Actions,
    occupancy: Option<crate::senses::Occ<'_>>,
) -> Option<Actions> {
    let a = fauna.animals[i];
    let m = a.pack?;
    let founder = a.founder?;
    let phys = *fauna.config.founder(founder);
    let t = phys.pack?;
    let h = phys.hunter?;
    let rec = *fauna.pack(m.pack)?;
    let v = view.config.voxel_m;
    let ring = f64::from(view.config.width) * v;
    let (dcx, dcz) = centre(rec.den, v);
    let d_den = distance(&a, dcx, dcz, ring);

    let carrion_at = |s: Site| fv.ground_at(s).map_or(0.0, |g| g.carrion);
    // A feast lasts while its carcass does: carrion there and the plant layer's carcass
    // record standing (cleared at `carcass_gone_fraction` of the body). "While it has
    // carrion" never ended — a bite diminishes with the stock (`bite_half_stock`), so
    // eating leaves dust, and a hungry member nibbled it while the pack never hunted again
    // (bellwing-finding item 2).
    let feast_left = |s: Site| fv.ground_at(s).is_some_and(|g| g.carrion > 0.0 && g.carcass.is_some());
    // At the carcass: its mouth is over the feast's carrion, read from the body (as the
    // bite is), not from its heuristic's answer — that also stands with `feed` held while
    // it stalks a body close ahead, a packmate included, and read as eating it held a
    // member short of its feast for good (bellwing-finding item 2).
    let geometry = phys.body_at(a.body);
    let at_feast = |fs: Site| {
        let cols = crate::body::mouth_columns(view, &a.pose, &geometry);
        crate::body::mouth_pool_site(fv, &cols, a.site.y, crate::Food::Carrion).is_some_and(|(s, _)| s == fs)
    };
    let eat = Actions { forward: 0.0, turn: 0.0, feed: 1.0 };
    // Short of it, it walks on with no stop distance: a mouth takes a pool at its own
    // standing face, so a carcass on a face a step up is eaten only from that face.

    // A pup: at the pack's carcass when it lies by the den, else at the den.
    if m.pup {
        if let Some(fs) = rec.feast {
            let (fx, fz) = centre(fs, v);
            let (_, _, from_den) = {
                let (dx, dz) = crate::pose::wrapped_delta(dcx, dcz, fx, fz, ring);
                (dx, dz, (dx * dx + dz * dz).sqrt())
            };
            if from_den <= t.den_radius_m && feast_left(fs) && a.eating {
                if at_feast(fs) {
                    return Some(eat);
                }
                return Some(go_to(&a, fx, fz, ring, 0.0, 1.0));
            }
        }
        if d_den > 0.5 * t.den_radius_m {
            return Some(go_to(&a, dcx, dcz, ring, 0.0, 0.0));
        }
        return Some(Actions {
            forward: 0.0,
            turn: held.turn,
            feed: held.feed,
        });
    }

    // SENSES-CORE: the record's tick-rate parts (scent) before the tactic reads it.
    crate::perception::refresh(fauna, i, view, fv, occupancy, crate::perception::Want { scent: true, ..Default::default() });
    let manifest = fauna.config.manifest(founder);
    let guard = manifest.cruise_m_per_s * manifest.controller_period_s
        + (h.strike_reach_length_fraction + h.jaw_forward_length_fraction) * geometry.length_m;
    let own = m.pack;
    let clamp = |fauna: &Fauna, act: Actions| keep_out(fauna, i, own, act, &t, guard, v, ring);

    // Home to sleep: out of its phase.
    if wants_rest(fauna, i, clock_on, daylight)
        && d_den > t.den_radius_m
        && u64::from(a.homing_ticks) < cubarium_voxel::ticks_at_least_one(t.home_give_up_s)
    {
        return Some(clamp(fauna, go_to(&a, dcx, dcz, ring, 0.0, 0.0)));
    }

    let now = fauna.tick;
    if a.eating || a.emergency {
        fauna.ledger.pack_hungry_samples += 1;
    }
    let adults: Vec<Animal> = fauna
        .animals
        .iter()
        .filter(|o| o.pack.is_some_and(|om| om.pack == own && !om.pup))
        .copied()
        .collect();
    // The leader's draw stands only while it keeps drawing: cleared at each of its samples
    // and set again by [`draw_step`] (so a leader that stops at a carcass leads no one).
    if pack_leader(&rec, &adults) == Some(a.id) {
        if let Some(p) = fauna.pack_mut(own) {
            p.drawn = None;
            p.drawn_scent = false;
        }
    }

    // The target: dropped when it is gone (its carcass becomes the pack's), after
    // `chase_give_up_s` without progress, when an adult's reserve is under the floor, when
    // it stands in another pack's territory, when no member has sensed it within the
    // pursuit's memory, or when the test goes badly (the break-off).
    if let Some(tid) = rec.target {
        let target = fauna.view().animal(tid).copied();
        let mut keep = false;
        match target {
            None => {
                let mut killed = false;
                if let Some(ts) = rec.target_site {
                    if carrion_at(ts) > 0.0 {
                        killed = true;
                        fauna.ledger.pack_chase_kills += 1;
                        if let Some(old) = rec.feast.filter(|&f| f != ts) {
                            let eaten = rec.feast_eaten;
                            fauna.log_pack_event(crate::PackEvent::FeastEnd {
                                tick: now,
                                pack: own,
                                site: old,
                                eaten,
                                why: crate::FeastEnd::Replaced,
                            });
                        }
                        if let Some(p) = fauna.pack_mut(own) {
                            if p.feast != Some(ts) {
                                p.feast_eaten = 0.0;
                            }
                            p.feast = Some(ts);
                        }
                    }
                }
                fauna.log_pack_event(crate::PackEvent::ChaseEnd {
                    tick: now,
                    pack: own,
                    why: if killed { crate::ChaseEnd::Killed } else { crate::ChaseEnd::Gone },
                    target: tid,
                    by: a.id,
                });
            }
            Some(o) => {
                let o_height = crate::body_of(&fauna.config, &o).map_or(0.0, |g| g.height_m);
                // CHURN, the give-up: `chase_give_up_s` without progress — the pack's nearest
                // member closing on the target by `chase_progress_m` past its nearest so far,
                // or work done on it (a wound or a pry gained, or turned over). A pack at a
                // hunkered body it is prying open, or closing on a fleeing one, keeps it.
                let (mut best_m, mut since) = (rec.chase_best_m, rec.chase_progress_tick);
                if let Some(k) = knows_of(fauna, i, tid, ring) {
                    let d = nearest_member(fauna, own, a.id, k.x, k.z, ring);
                    if d <= best_m - t.chase_progress_m {
                        best_m = d;
                        since = now;
                    }
                }
                let w = work_on(fauna, &o);
                if w > rec.chase_work + WORK_EPS {
                    since = now;
                }
                if let Some(p) = fauna.pack_mut(own) {
                    p.chase_best_m = best_m;
                    p.chase_progress_tick = since;
                    p.chase_work = w;
                }
                let late = now.saturating_sub(since) >= cubarium_voxel::ticks_at_least_one(t.chase_give_up_s);
                let spent = adults.iter().any(|b| spent(fauna, b, &t));
                // SENSES-CORE: the member's own focus follows what it and its packmates in
                // hearing know of the target; the pack gives it up when no awake adult has
                // sensed it within the pursuit's memory.
                if let Some(k) = knows_of(fauna, i, tid, ring) {
                    let p = fauna.percepts.entry(a.id).or_default();
                    if !(p.focus.known && p.focus.id == tid && p.focus.tick >= k.tick) {
                        p.focus = k;
                    }
                }
                let lost = !pack_knows(fauna, own, tid, ring);
                let out = knows_of(fauna, i, tid, ring).is_some_and(|k| trespass(fauna, own, k.x, k.z, &t, v, ring));
                // PACK-TUNE, the test: the target as this member knows it, weighed with the
                // members at hand around it; a prospect under `break_off_below` ends it.
                // FINDINGS-FIX: a test goes badly only once it has begun (approach, watch,
                // attack: MacNulty et al. 2007). Until a member is within the strike-test
                // distance (`2 × pack_radius_m`) of the target, the pack is still closing in
                // and is weighed as the commit weighed it: the members at hand around this
                // member (itself and the participants within `pack_radius_m`).
                // STRIKE, the height band: a target no member could strike from any face
                // beside it (perched or ledged out of its jaw band) is out of reach: its
                // prospect is 0, and the test breaks it off.
                // So is one, in a chase stalled `switch_stall_s`, that none of the pack's members
                // taking part can walk to within the chase's reach (`reach_steps`, the choice's
                // own bound): the probe's stuck-short chases had the prey on a ledge 1.5 m up or
                // 4-5 m down a cliff, the members walking straight into the face for the whole
                // give-up.
                let stalled_now = now.saturating_sub(since) >= cubarium_voxel::ticks_at_least_one(t.switch_stall_s);
                let unreachable = !(late || spent || out || lost)
                    && knows_of(fauna, i, tid, ring).is_some_and(|k| {
                        let graph = fauna.pack_routes(view, founder);
                        let Some(goal) = strike_node(&graph, &k, o_height, &geometry, &h, v) else {
                            return true;
                        };
                        if !stalled_now {
                            return false;
                        }
                        let (_, field) = fauna.pack_path(view, founder, own, goal, reach_steps(&t, manifest.cruise_m_per_s, v));
                        !adults.iter().filter(|b| b.id == a.id || participates(b, own)).any(|b| {
                            let (br, _) = node_radii(&phys.body_at(b.body), &h, v);
                            graph.node_near(b.site, (b.pose.x, b.pose.z), br).is_some_and(|j| field.contains_key(&j))
                        })
                    });
                let broke = !(late || spent || out || lost)
                    && (unreachable || knows_of(fauna, i, tid, ring).is_some_and(|k| {
                        let arrived = at_hand(fauna, own, a.id, k.x, k.z, 2.0 * t.pack_radius_m, ring);
                        let n = if arrived > 0 {
                            arrived
                        } else {
                            at_hand(fauna, own, a.id, a.pose.x, a.pose.z, t.pack_radius_m, ring)
                        };
                        prospect(fauna, i, &o, &k, n, ring) < t.break_off_below
                    }));
                if unreachable {
                    fauna.ledger.pack_out_of_reach += 1;
                }
                keep = !(late || spent || out || lost || broke);
                if keep {
                    // The kill's bookkeeping: where the carcass will lie if a member kills it.
                    if let Some(p) = fauna.pack_mut(own) {
                        p.target_site = Some(o.site);
                    }
                } else {
                    let why = if broke {
                        fauna.ledger.pack_break_offs += 1;
                        crate::ChaseEnd::BrokeOff
                    } else if late {
                        crate::ChaseEnd::Late
                    } else if spent {
                        crate::ChaseEnd::Spent
                    } else if lost {
                        fauna.ledger.pack_targets_lost += 1;
                        crate::ChaseEnd::Lost
                    } else {
                        crate::ChaseEnd::Trespass
                    };
                    fauna.log_pack_event(crate::PackEvent::ChaseEnd { tick: now, pack: own, why, target: tid, by: a.id });
                }
            }
        }
        if !keep {
            let alive = target.is_some();
            if let Some(p) = fauna.pack_mut(own) {
                p.target = None;
                p.target_site = None;
                if alive {
                    p.abandoned = Some(tid);
                }
            }
            if alive {
                shun(fauna, own, tid, t.shun_s);
            }
        }
    }

    // CHURN, the commitment; CHURN2, only once stalled: a hungry member in a chase that has
    // gone `switch_stall_s` without progress (the give-up's progress above: no member closing
    // in, no work done on the target) weighs the other prey it knows of against the target
    // by what each is worth and how open it is ([`worth`]: prospect × body, with the pack's
    // participants for every one and no awareness factor), never by which is nearest a
    // member; distance only gates a candidate: no farther from the pack (its nearest member
    // taking part) than the target is. One worth at least `switch_margin` times the target
    // takes the hunt (the old one is not shunned). A pack closing on or working its target
    // never switches (MacNulty et al. 2007: wolves test,
    // then commit and stay on that animal; opportunism belongs to the choice, not to
    // abandoning one). The commit restarts the progress clock, so a pack switches at most
    // once per `switch_stall_s`.
    let stalled = fauna
        .pack(own)
        .is_some_and(|p| p.target.is_some() && now.saturating_sub(p.chase_progress_tick) >= cubarium_voxel::ticks_at_least_one(t.switch_stall_s));
    if stalled && (a.eating || a.emergency) && !adults.iter().any(|b| spent(fauna, b, &t)) {
        if let Some(tid) = fauna.pack(own).and_then(|p| p.target) {
            if let (Some(k), Some(o)) = (knows_of(fauna, i, tid, ring), fauna.view().animal(tid).copied()) {
                let n = adults.iter().filter(|b| b.id == a.id || participates(b, own)).count().max(1) as u32;
                let current = worth(fauna, i, &o, &k, n, &t, ring);
                let floor = t.switch_margin * current;
                let within_m = nearest_member(fauna, own, a.id, k.x, k.z, ring);
                if floor.is_finite() {
                    let (better, _) = choose(fauna, i, view, own, founder, &t, &geometry, &h, manifest.cruise_m_per_s, n, ring, Some(tid), floor, Weigh::Switch { within_m });
                    if let Some(c) = better {
                        fauna.ledger.pack_switches += 1;
                        fauna.log_pack_event(crate::PackEvent::ChaseEnd { tick: now, pack: own, why: crate::ChaseEnd::Switched, target: tid, by: a.id });
                        commit(fauna, i, own, c, false, n, Some(tid), Some(current), ring);
                    }
                }
            }
        }
    }

    // The pack's carcass first (repair 3 item 5): while its carcass lasts, a member that wants
    // food eats there, and no new target is taken.
    //
    // With the meal rule ([`crate::HuntConfig::meal_first`]) a member walks to it as the meal
    // step walks to a carcass — along its route graph, given up and remembered fruitless after
    // `patrol_give_up_s` — and a member that cannot reach it, or has given it up, is not held
    // by it: it may hunt (VEILHOUND-DIAG: adults walked straight at a feast on the face below
    // their ledge, which their disc could not step down to, for the whole of a 6-minute probe
    // while the breeder starved, and took no target meanwhile).
    let joins = a.eating || a.emergency;
    let mut feast_usable = false;
    if let Some(fs) = fauna.pack(own).and_then(|p| p.feast) {
        if feast_left(fs) {
            feast_usable = true;
            if joins {
                if at_feast(fs) {
                    return Some(clamp(fauna, eat));
                }
                if fauna.config.hunt.meal_first {
                    match crate::hunt::meal_step_to(fauna, i, view, fv, fs) {
                        Some(go) => return Some(clamp(fauna, go)),
                        None => feast_usable = false,
                    }
                } else {
                    let (fx, fz) = centre(fs, v);
                    return Some(clamp(fauna, go_to(&a, fx, fz, ring, 0.0, 1.0)));
                }
            }
        } else {
            let eaten = fauna.pack(own).map_or(0.0, |p| p.feast_eaten);
            fauna.log_pack_event(crate::PackEvent::FeastEnd {
                tick: now,
                pack: own,
                site: fs,
                eaten,
                why: crate::FeastEnd::Emptied,
            });
            if let Some(p) = fauna.pack_mut(own) {
                p.feast = None;
                p.feast_eaten = 0.0;
            }
        }
    }

    // HUNTER-MEAL: between hunts (no target), a member that wants food goes to a usable
    // carcass it can walk to and eats there before the pack takes a new target.
    if joins && fauna.pack(own).and_then(|p| p.target).is_none() {
        if let Some(go) = crate::hunt::meal_step(fauna, i, view, fv) {
            return Some(clamp(fauna, go));
        }
    }

    // A new target: a member that is hungry, with every adult above the floor and a packmate within
    // `pack_radius_m` takes the nearest prey it sees ahead (its cone's range) inside the
    // pack's window and outside every other territory. Alone, it hunts as the lanternjaw.
    let current = fauna.pack(own).and_then(|p| p.target);
    // The floor reads as the drop does: no chase begins while any adult is under it (else
    // a spent packmate drops each new target at the next sampling and the pack flips
    // between targets).
    let spent = adults.iter().any(|b| spent(fauna, b, &t));
    // Repair 2 item 7: the pack starts a hunt when at least half its adults want food or
    // any is in an emergency, and a member joins (here: takes the target) only if it
    // wants food or is in an emergency itself.
    if current.is_none() && !feast_usable && joins && pack_wants(&adults) && !spent {
        // A packmate nearby, as before (repair 2 item 7: any adult in an emergency starts
        // a hunt); the participants among them are counted for the measure.
        let near: Vec<&Animal> = adults
            .iter()
            .filter(|b| b.id != a.id && distance(&a, b.pose.x, b.pose.z, ring) <= t.pack_radius_m)
            .collect();
        let mates = near.len();
        let participating = near.iter().filter(|b| participates(b, own)).count();
        if mates > 0 {
            // Only prey it can reach on foot within the chase: on the taker's walking
            // route graph, at most `cruise × chase_give_up_s × route_reach_fraction` away by
            // route (bellwing-finding item 3, bellwing3 item 1: grazers on a cliff top 3 m
            // over the pack drew 74 timed-out chases in seed 1).
            // VEILHOUND-SEARCH, SENSES-CORE: what the member knows of (its own record and its
            // packmates' in hearing, [`known`]), where it was last sensed, takeable there
            // (main's rule was its own 2 m cone ahead).
            // PACK-TUNE, the prey choice: each weighed by its prospect with the members at
            // hand, its meat and its distance; the best first, none under the floor.
            let n = 1 + participating as u32;
            let (best, prev_score) = choose(fauna, i, view, own, founder, &t, &geometry, &h, manifest.cruise_m_per_s, n, ring, None, 0.0, Weigh::Choice);
            if let Some(c) = best {
                let prev = fauna.pack(own).and_then(|p| p.abandoned);
                commit(fauna, i, own, c, rec.drawn.is_some(), n, prev, prev_score, ring);
            }
        }
    }

    // The draw (VEILHOUND-SEARCH): a hungry pack with no target and no usable feast that
    // senses prey it cannot take yet (beyond its chase reach, or with no packmate by the
    // taker) on its route component goes there by route, its leader first ([`draw_step`]);
    // in reach the target choice above takes it.
    let draw_go = if joins
        && !feast_usable
        && !spent
        && pack_wants(&adults)
        && fauna.pack(own).is_some_and(|p| p.target.is_none())
        && pack_leader(&rec, &adults) == Some(a.id)
    {
        draw_step(fauna, i, view, own, founder, &t, &geometry, &h, rec.drawn, rec.drawn_scent)
    } else {
        None
    };
    if draw_go.is_some() {
        fauna.ledger.pack_draw_lead_samples += 1;
    }

    // STRIKE, the last metres ([`close_in`]): close in on its target, on its level, a member
    // steers to its stand-off facing the prey — before the escape below, whose route step
    // walked members beside a prey into a wall toward the next node (CHURN-2: two members 20 s
    // at 0.25 m from a juvenile seedporter).
    if joins {
        if let Some(o) = fauna.pack(own).and_then(|p| p.target).and_then(|tid| knows_of(fauna, i, tid, ring)) {
            if let Some(act) = close_in(fauna, view, founder, &a, &o, &geometry, &h, &t, ring) {
                return Some(clamp(fauna, act));
            }
        }
    }

    // Its motor was refused over the last interval (a wall, a ledge, a pocket): its own
    // heuristic's escape backs it off and turns it this sampling — unless it is chasing on
    // a route, whose next step is a move its body fits (bellwing4: the escape held members
    // in `Resting` beside a route; the route's step is the way out).
    let fb = a.founder_state.feedback;
    if fb.attempted_equivalent > 1e-12
        && fb.delivered_equivalent / fb.attempted_equivalent < fauna.config.teachers.motor_refused
    {
        let on_route = fauna
            .pack(own)
            .and_then(|p| p.target)
            .and_then(|tid| knows_of(fauna, i, tid, ring))
            .filter(|o| joins && distance(&a, o.x, o.z, ring) <= 2.0 * t.pack_radius_m)
            .and_then(|o| route_step(fauna, view, founder, own, &t, manifest.cruise_m_per_s, &a, &o, &geometry, &h, (o.x, o.z)));
        match on_route.or(draw_go) {
            Some((nx, nz)) => return Some(clamp(fauna, go_to(&a, nx, nz, ring, 0.0, 1.0))),
            None => return Some(clamp(fauna, held)),
        }
    }

    // The chase: the adult nearest the target drives straight at it; the others go to
    // points `flank_offset_m` either side of it (across the driver's line), alternately
    // by id, and close in once there. Feed is held throughout: a prey in the capsule is
    // struck.
    // SENSES-CORE: at the target's last-known point, as this member knows it.
    if let Some(o) = fauna
        .pack(own)
        .and_then(|p| p.target)
        .and_then(|tid| knows_of(fauna, i, tid, ring))
        .filter(|_| joins)
    {
        let d_target = distance(&a, o.x, o.z, ring);
        if d_target <= 2.0 * t.pack_radius_m {
            // Repair 3 item 5: the driver and the flankers are the participants near the
            // target — awake and wanting food or in an emergency — never a fed adult.
            let hunters: Vec<Animal> = adults
                .iter()
                .filter(|b| {
                    (b.id == a.id || participates(b, own))
                        && distance(b, o.x, o.z, ring) <= 2.0 * t.pack_radius_m
                })
                .copied()
                .collect();
            let mut driver: Option<(f64, u64)> = None;
            for b in &hunters {
                let d = distance(b, o.x, o.z, ring);
                if driver.is_none_or(|(bd, bid)| d < bd || (d == bd && b.id < bid)) {
                    driver = Some((d, b.id));
                }
            }
            let driver_id = driver.map(|(_, id)| id);
            let stop = 0.5 * geometry.length_m;
            // On the route (bellwing3 item 1): a member that cannot walk straight to
            // where it is going goes to the next face on its route to the target's face.
            let routed = |fauna: &mut Fauna, gx: f64, gz: f64| -> Option<(f64, f64)> {
                route_step(fauna, view, founder, own, &t, manifest.cruise_m_per_s, &a, &o, &geometry, &h, (gx, gz))
            };
            let act = if driver_id == Some(a.id) {
                match routed(fauna, o.x, o.z) {
                    Some((nx, nz)) => go_to(&a, nx, nz, ring, 0.0, 1.0),
                    None => go_to(&a, o.x, o.z, ring, stop, 1.0),
                }
            } else if let Some((nx, nz)) = routed(fauna, o.x, o.z) {
                go_to(&a, nx, nz, ring, 0.0, 1.0)
            } else {
                let rank = hunters
                    .iter()
                    .filter(|b| Some(b.id) != driver_id && b.id < a.id)
                    .count();
                let side = if rank % 2 == 0 { 1.0 } else { -1.0 };
                let d_ref = driver_id
                    .and_then(|id| hunters.iter().find(|b| b.id == id))
                    .copied()
                    .unwrap_or(a);
                let (lx, lz, ld) = delta(&d_ref, o.x, o.z, ring);
                let (ux, uz) = if ld > 0.0 { (lx / ld, lz / ld) } else { a.pose.forward() };
                let (px, pz) = (uz * side, -ux * side);
                let (gx, gz) = (
                    o.x + px * t.flank_offset_m,
                    o.z + pz * t.flank_offset_m,
                );
                if distance(&a, gx, gz, ring) <= 0.5 {
                    go_to(&a, o.x, o.z, ring, stop, 1.0)
                } else {
                    go_to(&a, gx, gz, ring, 0.0, 1.0)
                }
            };
            // S2, the stalk: under the prey's hearing beyond the commit distance, still while
            // it looks up, then the rush.
            let act = match crate::vigil::stalk(fauna, &a, &o, d_target, act.forward) {
                Some(forward) => Actions { forward, ..act },
                None => act,
            };
            return Some(clamp(fauna, act));
        }
    }

    // The draw's leader walks to the prey, feed held: a prey met in its capsule is struck.
    if let Some((nx, nz)) = draw_go {
        return Some(clamp(fauna, go_to(&a, nx, nz, ring, 0.0, 1.0)));
    }
    // While its pack is drawn, a hungry member follows the draw's leader.
    let drawn_by = if joins { drawn_leader(fauna, own, &rec, &adults, a.id) } else { None };
    if drawn_by.is_some() {
        fauna.ledger.pack_draw_follow_samples += 1;
    }

    // Together between hunts: an idle adult walks back to within `follow_radius_m` of the
    // breeder (of the draw's leader while the pack is drawn).
    if let Some(b) = drawn_by {
        if distance(&a, b.pose.x, b.pose.z, ring) > t.follow_radius_m {
            let (gx, gz) = follow_step(fauna, i, view, own, founder, &b, &geometry, &h);
            return Some(clamp(fauna, go_to(&a, gx, gz, ring, 0.0, 0.0)));
        }
    } else if let Some(b) = rec
        .breeder
        .filter(|&b| b != a.id)
        .and_then(|b| adults.iter().find(|o| o.id == b))
        .copied()
    {
        if distance(&a, b.pose.x, b.pose.z, ring) > t.follow_radius_m {
            return Some(clamp(fauna, go_to(&a, b.pose.x, b.pose.z, ring, 0.0, 0.0)));
        }
    }

    // Its own heuristic: the lanternjaw's ambush, with no lure — but not its hold on a body
    // it does not take (VEILHOUND-DIAG). The ambush stalks and waits on any body in its cone,
    // and a pack keeps its members in one another's cones: a hungry member crept up on a
    // packmate until they stood on one point and waited there, the wait renewed every sample
    // the packmate stayed in sight, so the pack stood still for four hours (the main arm,
    // every seed). Likewise its stop for carrion at its mouth where no carcass stands (a smear
    // the lanternjaw's patrol already passes over). With the world's patrol on
    // ([`crate::HuntConfig::patrol`]), a hungry member with no target whose ambush holds it
    // with feed held (still, or creeping at the stalk) when no prey its pack takes is within
    // its sight ahead, and no carcass is at its mouth, patrols as the lanternjaw does
    // ([`crate::hunt::steer`]): patch to patch along its prey's food.
    if joins && fauna.config.hunt.patrol && fauna.pack(own).and_then(|p| p.target).is_none() && held.feed > 0.0 && held.forward <= fauna.config.teachers.stalk {
        let window = window_body(fauna, i, ring);
        let sight = manifest.cone_range_m;
        let (fx, fz) = a.pose.forward();
        // SENSES-CORE: a prey in its own record, where it was sensed.
        let prey_ahead = fauna.percept(a.id).is_some_and(|p| {
            p.tracks().iter().filter(|k| k.class == class::PREY).any(|k| {
                let (dx, dz, d) = delta(&a, k.x, k.z, ring);
                d <= sight
                    && dx * fx + dz * fz >= 0.0
                    && fauna.view().animal(k.id).is_some_and(|o| crate::predation::is_prey_in_window(&fauna.config, &a, o, window))
            })
        });
        if !prey_ahead && !crate::hunt::mouth_on_carcass(fv, view, &a, &geometry) {
            // The breeder leads the search: a member keeps within the follow radius of
            // a breeder that wants food too, and the breeder (or a member whose breeder is
            // fed) patrols — so the pack moves together and a member that meets prey has a
            // packmate by it.
            let leader = drawn_by.or_else(|| {
                rec.breeder
                    .filter(|&b| b != a.id)
                    .and_then(|b| adults.iter().find(|o| o.id == b))
                    .filter(|b| b.eating || b.emergency)
                    .copied()
            });
            if let Some(b) = leader {
                return Some(clamp(fauna, go_to(&a, b.pose.x, b.pose.z, ring, t.follow_radius_m, 0.0)));
            }
            if let Some((go, _)) = crate::hunt::steer(fauna, i, view, fv, occupancy) {
                return Some(clamp(fauna, go));
            }
        }
    }
    Some(clamp(fauna, held))
}

/// **The lanternjaw's carcass yield** (roster R3, Fable): a body under the lanternjaw's
/// heuristic that is eating at a carcass when a body that hunts it — a threat by the
/// existing reading ([`crate::kind_of`]) — comes within `carcass_yield_radius_m` leaves
/// it, walking directly away until none is within `threat_clear_m`, then resumes. `None`
/// when it is not yielding.
pub(crate) fn carcass_yield(fauna: &mut Fauna, i: usize, ring: f64, at_carcass: bool) -> Option<Actions> {
    let a = fauna.animals[i];
    if !a.yielding && !at_carcass {
        return None;
    }
    let cfg = fauna.config.predation;
    let mut nearest: Option<(f64, f64, f64)> = None;
    for o in &fauna.animals {
        if o.id == a.id {
            continue;
        }
        let (dx, dz, d) = delta(&a, o.pose.x, o.pose.z, ring);
        if d > cfg.threat_clear_m || nearest.is_some_and(|(n, _, _)| d >= n) {
            continue;
        }
        if crate::kind_of(&fauna.config, &a, o, d) == crate::BodyKind::Threat {
            nearest = Some((d, dx, dz));
        }
    }
    if !a.yielding {
        match nearest {
            Some((d, _, _)) if d <= cfg.carcass_yield_radius_m => {
                fauna.animals[i].yielding = true;
                fauna.ledger.carcass_yields += 1;
            }
            _ => return None,
        }
    }
    let Some((_, dx, dz)) = nearest else {
        fauna.animals[i].yielding = false;
        return None;
    };
    Some(head(&a, (-dx).atan2(-dz), 1.0, 0.0))
}
