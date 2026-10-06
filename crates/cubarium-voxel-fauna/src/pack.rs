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

/// **The next step on the route** (bellwing3 item 1, bellwing4): `None` when the member
/// can walk straight to `goal` on the disc's graph (or is off the graph); else the centre
/// of the next node on its route to the node it would strike the target from.
#[allow(clippy::too_many_arguments)]
fn route_step(
    fauna: &mut Fauna,
    view: &VoxelView<'_>,
    founder: crate::Founder,
    own: u64,
    t: &Pack,
    cruise_m_per_s: f64,
    a: &Animal,
    o: &Animal,
    geometry: &crate::Body,
    h: &crate::Hunter,
    goal: (f64, f64),
) -> Option<(f64, f64)> {
    let v = view.config.voxel_m;
    let (own_r, prey_r) = node_radii(geometry, h, v);
    let graph = fauna.pack_routes(view, founder);
    let from = graph.node_near(a.site, (a.pose.x, a.pose.z), own_r)?;
    if graph.walk_line(from, (a.pose.x, a.pose.z), goal) {
        return None;
    }
    let target = graph.node_near(o.site, (o.pose.x, o.pose.z), prey_r)?;
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
    let adults: Vec<Animal> = fauna
        .animals
        .iter()
        .filter(|o| o.pack.is_some_and(|om| om.pack == own && !om.pup))
        .copied()
        .collect();

    // The target: dropped when it is gone (its carcass becomes the pack's), after
    // `chase_give_up_s`, when an adult's reserve is under the floor, or when it stands in
    // another pack's territory.
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
                });
            }
            Some(o) => {
                let late = now.saturating_sub(rec.chase_since_tick)
                    >= cubarium_voxel::ticks_at_least_one(t.chase_give_up_s);
                let spent = adults.iter().any(|b| spent(fauna, b, &t));
                let out = trespass(fauna, own, o.pose.x, o.pose.z, &t, v, ring);
                keep = !(late || spent || out);
                if keep {
                    if let Some(p) = fauna.pack_mut(own) {
                        p.target_site = Some(o.site);
                    }
                } else {
                    let why = if late {
                        crate::ChaseEnd::Late
                    } else if spent {
                        crate::ChaseEnd::Spent
                    } else {
                        crate::ChaseEnd::Trespass
                    };
                    fauna.log_pack_event(crate::PackEvent::ChaseEnd { tick: now, pack: own, why });
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
            let window = window_body(fauna, i, ring);
            let sight = manifest.cone_range_m;
            let (fx, fz) = a.pose.forward();
            let mut candidates: Vec<(f64, u64, Site, (f64, f64))> = Vec::new();
            for o in &fauna.animals {
                if o.id == a.id || o.mobility.climb.is_some() {
                    continue;
                }
                let (dx, dz, d) = delta(&a, o.pose.x, o.pose.z, ring);
                if d > sight
                    || dx * fx + dz * fz < 0.0
                    || fauna.pack(own).and_then(|p| p.abandoned) == Some(o.id)
                {
                    continue;
                }
                if !crate::predation::is_prey_in_window(&fauna.config, &a, o, window)
                    || trespass(fauna, own, o.pose.x, o.pose.z, &t, v, ring)
                {
                    continue;
                }
                candidates.push((d, o.id, o.site, (o.pose.x, o.pose.z)));
            }
            let mut best: Option<(f64, u64, Site)> = None;
            if !candidates.is_empty() {
                candidates.sort_by(|x, y| x.0.total_cmp(&y.0).then(x.1.cmp(&y.1)));
                let routes = fauna.pack_routes(view, founder);
                let (own_r, prey_r) = node_radii(&geometry, &h, v);
                if let Some(from) = routes.node_near(a.site, (a.pose.x, a.pose.z), own_r) {
                    let field = routes.distances(from, reach_steps(&t, manifest.cruise_m_per_s, v));
                    best = candidates
                        .into_iter()
                        .find(|c| routes.node_near(c.2, c.3, prey_r).is_some_and(|j| field.contains_key(&j)))
                        .map(|c| (c.0, c.1, c.2));
                }
            }
            if let Some((_, tid, site)) = best {
                if let Some(p) = fauna.pack_mut(own) {
                    p.target = Some(tid);
                    p.chase_since_tick = now;
                    p.target_site = Some(site);
                    p.abandoned = None;
                }
                fauna.ledger.pack_chases += 1;
                let participants = 1 + participating as u32;
                fauna.log_pack_event(crate::PackEvent::ChaseStart { tick: now, pack: own, participants });
                call(fauna, i);
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
            .and_then(|tid| fauna.view().animal(tid).copied())
            .filter(|o| joins && distance(&a, o.pose.x, o.pose.z, ring) <= 2.0 * t.pack_radius_m)
            .and_then(|o| route_step(fauna, view, founder, own, &t, manifest.cruise_m_per_s, &a, &o, &geometry, &h, (o.pose.x, o.pose.z)));
        match on_route {
            Some((nx, nz)) => return Some(clamp(fauna, go_to(&a, nx, nz, ring, 0.0, 1.0))),
            None => return Some(clamp(fauna, held)),
        }
    }

    // The chase: the adult nearest the target drives straight at it; the others go to
    // points `flank_offset_m` either side of it (across the driver's line), alternately
    // by id, and close in once there. Feed is held throughout: a prey in the capsule is
    // struck.
    if let Some(o) = fauna
        .pack(own)
        .and_then(|p| p.target)
        .and_then(|tid| fauna.view().animal(tid).copied())
        .filter(|_| joins)
    {
        let d_target = distance(&a, o.pose.x, o.pose.z, ring);
        if d_target <= 2.0 * t.pack_radius_m {
            // Repair 3 item 5: the driver and the flankers are the participants near the
            // target — awake and wanting food or in an emergency — never a fed adult.
            let hunters: Vec<Animal> = adults
                .iter()
                .filter(|b| {
                    (b.id == a.id || participates(b, own))
                        && distance(b, o.pose.x, o.pose.z, ring) <= 2.0 * t.pack_radius_m
                })
                .copied()
                .collect();
            let mut driver: Option<(f64, u64)> = None;
            for b in &hunters {
                let d = distance(b, o.pose.x, o.pose.z, ring);
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
                match routed(fauna, o.pose.x, o.pose.z) {
                    Some((nx, nz)) => go_to(&a, nx, nz, ring, 0.0, 1.0),
                    None => go_to(&a, o.pose.x, o.pose.z, ring, stop, 1.0),
                }
            } else if let Some((nx, nz)) = routed(fauna, o.pose.x, o.pose.z) {
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
                let (lx, lz, ld) = delta(&d_ref, o.pose.x, o.pose.z, ring);
                let (ux, uz) = if ld > 0.0 { (lx / ld, lz / ld) } else { a.pose.forward() };
                let (px, pz) = (uz * side, -ux * side);
                let (gx, gz) = (
                    o.pose.x + px * t.flank_offset_m,
                    o.pose.z + pz * t.flank_offset_m,
                );
                if distance(&a, gx, gz, ring) <= 0.5 {
                    go_to(&a, o.pose.x, o.pose.z, ring, stop, 1.0)
                } else {
                    go_to(&a, gx, gz, ring, 0.0, 1.0)
                }
            };
            return Some(clamp(fauna, act));
        }
    }

    // Together between hunts: an idle adult walks back to within `follow_radius_m` of the
    // breeder.
    if let Some(b) = rec
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
        let prey_ahead = fauna.animals.iter().any(|o| {
            if o.id == a.id || o.pack.is_some_and(|om| om.pack == own) {
                return false;
            }
            let (dx, dz, d) = delta(&a, o.pose.x, o.pose.z, ring);
            d <= sight && dx * fx + dz * fz >= 0.0 && crate::predation::is_prey_in_window(&fauna.config, &a, o, window)
        });
        if !prey_ahead && !crate::hunt::mouth_on_carcass(fv, view, &a, &geometry) {
            // The breeder leads the search: a member keeps within the follow radius of
            // a breeder that wants food too, and the breeder (or a member whose breeder is
            // fed) patrols — so the pack moves together and a member that meets prey has a
            // packmate by it.
            let leader = rec
                .breeder
                .filter(|&b| b != a.id)
                .and_then(|b| adults.iter().find(|o| o.id == b))
                .filter(|b| b.eating || b.emergency)
                .copied();
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
