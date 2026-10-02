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
                || o.pack.is_some_and(|om| om.pack == m.pack && !om.pup)
                    && distance(a, o.pose.x, o.pose.z, ring) <= t.pack_radius_m
        })
        .map(|o| o.body)
        .sum()
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

/// **Provisioning** (Brief R3): every adult within `den_radius_m` of its den whose reserve
/// is above `provision_reserve_fraction` of its cap passes `provision_rate_per_s` × its
/// body per second out of its reserve to the hungriest pup of its pack there (the pup
/// with the lowest reserve fraction, lowest id on a tie, with room), never below the
/// fraction and never more than the pup can take. The pup receives
/// `provision_efficiency` of it — organic matter, mineral and energy alike — and the rest
/// is respired: its organic matter booked respired, its energy as heat, its mineral shed
/// to the giver's gut, as every respiration sheds it.
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
        let sc = crate::body::effective_config(&fauna.config, &giver);
        let floor = t.provision_reserve_fraction * sc.reserve_of(giver.body);
        if !(giver.reserve > floor) {
            continue;
        }
        // The hungriest pup of its pack at the den, with room.
        let mut best: Option<(f64, usize)> = None;
        for (k, p) in fauna.animals.iter().enumerate() {
            if !p.pack.is_some_and(|pm| pm.pack == m.pack && pm.pup)
                || distance(p, cx, cz, ring) > t.den_radius_m
            {
                continue;
            }
            let psc = crate::body::effective_config(&fauna.config, p);
            let cap = psc.reserve_of(p.body);
            if !(cap > p.reserve) {
                continue;
            }
            let f = if cap > 0.0 { p.reserve / cap } else { 1.0 };
            if best.is_none_or(|(bf, _)| f < bf) {
                best = Some((f, k));
            }
        }
        let Some((_, k)) = best else { continue };
        let eff = t.provision_efficiency.clamp(0.0, 1.0);
        if !(eff > 0.0) {
            continue;
        }
        let pup = fauna.animals[k];
        let room = crate::body::effective_config(&fauna.config, &pup).reserve_of(pup.body) - pup.reserve;
        let give = (t.provision_rate_per_s * giver.body * DT)
            .min(giver.reserve - floor)
            .min(room / eff);
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
        fauna.ledger.provision_received_organic += taken.organic - lost_organic;
    }
}

/// Whether body `i` wants a rest now: a hunter at its satiated line, or out of its
/// activity phase and not hungry — the circadian stage's own reading for an awake body.
pub(crate) fn wants_rest(fauna: &Fauna, i: usize, clock_on: bool, daylight: f64) -> bool {
    let a = &fauna.animals[i];
    let Some(founder) = a.founder else {
        return false;
    };
    let phys = fauna.config.founder(founder);
    let frac = reserve_fraction(fauna, a);
    let hungry = (1.0 - frac).clamp(0.0, 1.0);
    phys.hunter.is_some_and(|h| frac >= h.rest_at)
        || (!phys.active.awake(clock_on, daylight, &fauna.config.life) && hungry < phys.wake_hunger)
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
    let frac = reserve_fraction(fauna, &a);
    let carrion_at = |s: Site| fv.ground_at(s).map_or(0.0, |g| g.carrion);
    let eating = held.forward == 0.0 && held.feed > 0.0;

    // A pup: at the pack's carcass when it lies by the den, else at the den.
    if m.pup {
        if let Some(fs) = rec.feast {
            let (fx, fz) = centre(fs, v);
            let (_, _, from_den) = {
                let (dx, dz) = crate::pose::wrapped_delta(dcx, dcz, fx, fz, ring);
                (dx, dz, (dx * dx + dz * dz).sqrt())
            };
            if from_den <= t.den_radius_m && carrion_at(fs) > 0.0 && frac < h.rest_at {
                if eating {
                    return Some(held);
                }
                return Some(go_to(&a, fx, fz, ring, 0.25, 1.0));
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
    let geometry = phys.body_at(a.body);
    let guard = manifest.cruise_m_per_s * manifest.controller_period_s
        + (h.strike_reach_length_fraction + h.jaw_forward_length_fraction) * geometry.length_m;
    let own = m.pack;
    let clamp = |fauna: &Fauna, act: Actions| keep_out(fauna, i, own, act, &t, guard, v, ring);

    // Home to rest: satiated, or out of its phase.
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
                if let Some(ts) = rec.target_site {
                    if carrion_at(ts) > 0.0 {
                        fauna.ledger.pack_chase_kills += 1;
                        if let Some(p) = fauna.pack_mut(own) {
                            p.feast = Some(ts);
                        }
                    }
                }
            }
            Some(o) => {
                let late = now.saturating_sub(rec.chase_since_tick)
                    >= cubarium_voxel::ticks_at_least_one(t.chase_give_up_s);
                let spent = adults
                    .iter()
                    .any(|b| reserve_fraction(fauna, b) < t.chase_reserve_floor);
                let out = trespass(fauna, own, o.pose.x, o.pose.z, &t, v, ring);
                keep = !(late || spent || out);
                if keep {
                    if let Some(p) = fauna.pack_mut(own) {
                        p.target_site = Some(o.site);
                    }
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

    // A new target: a member that is hungry, with every adult above the floor and a packmate within
    // `pack_radius_m` takes the nearest prey it sees ahead (its cone's range) inside the
    // pack's window and outside every other territory. Alone, it hunts as the lanternjaw.
    let current = fauna.pack(own).and_then(|p| p.target);
    // The floor reads as the drop does: no chase begins while any adult is under it (else
    // a spent packmate drops each new target at the next sampling and the pack flips
    // between targets).
    let spent = adults
        .iter()
        .any(|b| reserve_fraction(fauna, b) < t.chase_reserve_floor);
    if current.is_none() && frac < h.rest_at && !spent {
        let mates = adults
            .iter()
            .filter(|b| b.id != a.id && distance(&a, b.pose.x, b.pose.z, ring) <= t.pack_radius_m)
            .count();
        if mates > 0 {
            let window = window_body(fauna, i, ring);
            let sight = manifest.cone_range_m;
            let (fx, fz) = a.pose.forward();
            let mut best: Option<(f64, u64, Site)> = None;
            for o in &fauna.animals {
                if o.id == a.id || o.mobility.climb.is_some() {
                    continue;
                }
                let (dx, dz, d) = delta(&a, o.pose.x, o.pose.z, ring);
                if d > sight
                    || dx * fx + dz * fz < 0.0
                    || best.is_some_and(|(b, _, _)| d >= b)
                    || fauna.pack(own).and_then(|p| p.abandoned) == Some(o.id)
                {
                    continue;
                }
                if !crate::predation::is_prey_in_window(&fauna.config, &a, o, window)
                    || trespass(fauna, own, o.pose.x, o.pose.z, &t, v, ring)
                {
                    continue;
                }
                best = Some((d, o.id, o.site));
            }
            if let Some((_, tid, site)) = best {
                if let Some(p) = fauna.pack_mut(own) {
                    p.target = Some(tid);
                    p.chase_since_tick = now;
                    p.target_site = Some(site);
                    p.abandoned = None;
                }
                fauna.ledger.pack_chases += 1;
                call(fauna, i);
            }
        }
    }

    // Its motor was refused over the last interval (a wall, a ledge, a pocket): its own
    // heuristic's escape backs it off and turns it this sampling.
    let fb = a.founder_state.feedback;
    if fb.attempted_equivalent > 1e-12
        && fb.delivered_equivalent / fb.attempted_equivalent < fauna.config.teachers.motor_refused
    {
        return Some(clamp(fauna, held));
    }

    // The chase: the adult nearest the target drives straight at it; the others go to
    // points `flank_offset_m` either side of it (across the driver's line), alternately
    // by id, and close in once there. Feed is held throughout: a prey in the capsule is
    // struck.
    if let Some(o) = fauna
        .pack(own)
        .and_then(|p| p.target)
        .and_then(|tid| fauna.view().animal(tid).copied())
    {
        let d_target = distance(&a, o.pose.x, o.pose.z, ring);
        if d_target <= 2.0 * t.pack_radius_m {
            let mut driver: Option<(f64, u64)> = None;
            for b in &adults {
                let d = distance(b, o.pose.x, o.pose.z, ring);
                if driver.is_none_or(|(bd, bid)| d < bd || (d == bd && b.id < bid)) {
                    driver = Some((d, b.id));
                }
            }
            let driver_id = driver.map(|(_, id)| id);
            let stop = 0.5 * geometry.length_m;
            let act = if driver_id == Some(a.id) {
                go_to(&a, o.pose.x, o.pose.z, ring, stop, 1.0)
            } else {
                let rank = adults
                    .iter()
                    .filter(|b| Some(b.id) != driver_id && b.id < a.id)
                    .count();
                let side = if rank % 2 == 0 { 1.0 } else { -1.0 };
                let d_ref = driver_id
                    .and_then(|id| adults.iter().find(|b| b.id == id))
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

    // The kill: the pack eats at its carcass until the carrion is gone or it is full.
    if let Some(fs) = fauna.pack(own).and_then(|p| p.feast) {
        if carrion_at(fs) > 0.0 {
            if frac < h.rest_at {
                if eating {
                    return Some(clamp(fauna, held));
                }
                let (fx, fz) = centre(fs, v);
                return Some(clamp(fauna, go_to(&a, fx, fz, ring, 0.25, 1.0)));
            }
        } else if let Some(p) = fauna.pack_mut(own) {
            p.feast = None;
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

    // Its own heuristic: the lanternjaw's ambush, with no lure.
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
