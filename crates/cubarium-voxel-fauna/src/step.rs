//! One tick of the animal layer.
//!
//! # Order within a tick, in one place
//!
//! 1. **Terrain.** An animal whose support face is no longer a support face — buried, dug
//!    out, turned to air — leaves the world, booked `removed_*_out`. It does not fall and
//!    it does not die: there is no corpse.
//! 2. **Maintenance.** `maintenance_per_s · body · dt` of organic matter is respired from
//!    the reserve, and from the body once the reserve is empty (dieback). The energy in it
//!    leaves as heat at the animal's own current density; the mineral it held is **shed
//!    to the gut** by the same fraction, and every `GUT_VOID_S` the gut is voided onto
//!    the ground as litter (dung). Ages
//!    advance here, so `age_ticks` is the number of maintenance payments it has made.
//!    A parasite rider then **takes** from its host (`parasite_take_body_per_day` × its
//!    own body per ecological day, at most what the host bears, by the same pro-rata rule).
//!    Then **growth**: the reserve pays
//!    for new structure at no more than `growth_max_per_s`, scaled by how full it is,
//!    until `body_max`; a rider grown to its detach size, or past what its host bears,
//!    **drops off**.
//! 3. **Cue field.** On due ticks, update emission, decay and diffusion after maintenance
//!    and before any controller samples.
//! 4. **Controllers** — the sampling point, explicit and shared. Every founder whose
//!    controller period is due is sampled from the tick's pre-action state: its
//!    prior-interval feedback is consumed into the observation, its controller (a
//!    diagnostic heuristic or a GRU — interchangeable behind [`crate::Controller`]) is
//!    driven, and the resolved, deadbanded actions are **held** until the next sampling.
//! 5. **Act**, in id order: resolve each founder's held actions — the motor budget is
//!    paid on the requested equivalent displacement, the pose sweeps in bounded
//!    sub-steps, and on the due tick one local bite is attempted through the plant
//!    layer's real transfers — then record the interval's motion and intake feedback. A
//!    body with no lineage rests. Every bite is **sated**: it
//!    asks in proportion to hunger, takes `E / (E + K)` of that from food holding `E`,
//!    and never more than the reserve's room after yield, so a full animal takes nothing
//!    and nothing eaten is respired as surplus. A rider does none of this: after the act
//!    it is **carried** to its host's face and pose.
//! 6. **Reproduction.** A clutch whose face is gone or drowned goes to carrion; every
//!    body's surplus counter and refractory move; a gestating frondgrazer pays its
//!    instalment into its escrow and gives birth at term; a littershredder in surplus
//!    lays a clutch on the litter it stands on; and clutches at term hatch, a dormant egg
//!    at dusk only onto a host in reach (parasite hatchlings). Only bodies
//!    that were alive at the start of the tick reproduce, so a newborn cannot itself
//!    breed on the tick it appears.
//! 7. **Death.** Starvation (`body < body_min`) or drowning (standing water over its feet
//!    deeper than its drown depth for its lineage's `drown_after_s`) hands the whole animal back as **carrion** and
//!    books a death.
//!
//! Withdrawals and deposits are the plant layer's between-tick transfers
//! (`cubarium_voxel_flora::step`'s module doc): a bite lowers the foliage the *next* plant
//! tick reads, and a deposit is in the snapshot of the next plant tick's decomposition.
//! This layer therefore steps **after** `Flora::step`, and the whole coupled tick is
//! `World::step`, `Flora::step`, `Fauna::step`.
//!
//! # What is deterministic, and from what
//!
//! Every choice that could depend on iteration order is made from a **keyed splitmix64
//! stream** — the flora layer's own, key `(domain, world seed, animal id, tick)` — so the
//! same world, the same animal and the same tick always break a tie the same way, and
//! nothing about storage order can reach the result. Animals are held in id order and
//! every pass walks them in it.

use cubarium_voxel::{DT, VoxelView, World};
use cubarium_voxel_flora::{DepositKind, Flora, Site, Taken};

use crate::body;
#[cfg(test)]
#[path = "testpass3_snail_taste_tests.rs"]
mod testpass3_snail_taste_tests;
use crate::controller::Actions;
use crate::manifest::Founder;
use crate::predation::{Hit, Rest, Strike, StrikePhase};
use crate::{Animal, Departure, Fauna, Food, IntervalFeedback, SpeciesConfig, State};

#[cfg(test)]
#[path = "groom_tests.rs"]
mod groom_tests;

#[cfg(test)]
#[path = "snail_stuck_tests.rs"]
mod snail_stuck_tests;

/// What a respiration is for, as [`FaunaLedger`](crate::FaunaLedger) books it. The four
/// split counters sum to `respired_out` to the bit — every booking adds the same addend
/// to the total and to one split at the same point — so a driver can read the episode's
/// upkeep, motor, digestion and failed-gestation charges apart without a second ledger.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Respiration {
    /// Basal upkeep, every animal.
    Maintenance,
    /// The founder's paid motion, `motor_respiration_per_s` scaled by the requested
    /// equivalent displacement.
    Motor,
    /// Digestion: the absorbed share of a bite its mineral cannot fund (the undigested
    /// share is egested to the gut, not respired). Anything assimilated that the reserve
    /// could not hold would be booked here too, but the sated bite never takes
    /// more than the room after yield, so that share is zero by construction.
    Digestion,
    /// An interrupted gestation: the loss fraction of an escrow whose parent could not
    /// pay the next instalment, or died carrying it.
    Gestation,
    /// A hunt: a strike's cost and a lit lure (predation §2.3, §2.2).
    Hunt,
    /// Healing a wound (predation §2.4).
    Heal,
    /// The provisioning loss (roster R3): the share of what an adult passes a pup that the
    /// pup does not receive. Only in `respired_out`; the ledger's provisioning lines carry
    /// the split.
    Provision,
}

pub(crate) use cubarium_voxel::KeyedRng as Rng;

#[path = "glider.rs"]
mod glider;
#[path = "places.rs"]
pub(crate) mod places;
#[cfg(test)]
pub(crate) use glider::{better_roost, safe_crown, Roost};
#[cfg(test)]
pub(crate) use glider::fruit_candidates;
pub(crate) use glider::{on_ledge_top, refuge_height_m, OffGraph, glider_fruit_in_reach, glider_larva_in_reach, leafy_vine_in_reach, off_graph_exit};
#[cfg(test)]
pub(crate) use glider::fruit_candidates as glider_fruit_candidates;
#[path = "jump.rs"]
pub(crate) mod jump;
#[path = "forage.rs"]
pub(crate) mod forage;
#[path = "rove.rs"]
pub(crate) mod rove;

/// The tick's sky, read once (F1): what a flyer's act needs of the weather and the clock.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Sky {
    /// A downpour is falling: no takeoff, and a flyer aloft comes down (§3.8).
    pub downpour: bool,
    /// The rain rate now, m/s (the shower cost's numerator).
    pub rain_m_per_s: f64,
    pub clock_on: bool,
    pub daylight: f64,
}
use cubarium_voxel::organism::{Budget, build_from, settle_mineral, tissue_funded};
use cubarium_voxel::pro_rata;

pub(crate) fn step(fauna: &mut Fauna, world: &World, flora: &mut Flora, threads: usize) {
    tick_inner(fauna, world, flora, None, threads);
}

/// The static arena's tick: [`step`] with the per-arena [`Senses`] handle. The camera-free
/// side effect is the field update, which the tick fires at [`UPDATE_TICKS`] cadence
/// before any controller samples, per the frozen arena's tick order.
pub(crate) fn step_with_senses(
    fauna: &mut Fauna,
    world: &World,
    flora: &mut Flora,
    senses: &mut crate::Senses,
    threads: usize,
) {
    tick_inner(fauna, world, flora, Some(senses), threads);
}

fn tick_inner(
    fauna: &mut Fauna,
    world: &World,
    flora: &mut Flora,
    mut senses: Option<&mut crate::Senses>,
    threads: usize,
) {
    cubarium_voxel::voxel_phase!(FaunaStep, {
        let start = begin(fauna, world, flora);
        read(fauna, world, flora, senses.as_deref_mut(), &start, threads);
        finish(fauna, world, flora, start);
    });
}

/// What one animal tick carries from its start ([`begin`]) through its read side
/// ([`read`]) to its writes ([`finish`]): the tick it produces and what it read of the
/// world's clock and weather when it began.
#[derive(Clone, Copy, Debug)]
pub(crate) struct TickStart {
    tick: u64,
    seed: u64,
    dusk: bool,
    sky: Sky,
}

/// **The animal tick's first part**: the clock, the terrain, and maintenance — circadian
/// state, upkeep, the gut and its voiding (dung and gut seeds onto the plant layer's
/// ground), eggs, growth and the packs. Writes the animals and, through the voiding, the
/// plant layer's `ground`.
pub(crate) fn begin(fauna: &mut Fauna, world: &World, flora: &mut Flora) -> TickStart {
    // The tick counter moves first, so `fauna.tick` is the tick this step produces — the
    // flora layer's own rule, and the tick the keyed streams are keyed by.
    fauna.tick += 1;
    let tick = fauna.tick;
    let view = world.view();
    fauna.world_geometry = Some((f64::from(view.config.width) * view.config.voxel_m, view.config.voxel_m));
    let seed = view.config.seed;
    // Dusk (the egg bank's hour, predation §2.13): the tick the world's daylight first
    // falls below the day line, with the clock on.
    let clock_on = view.config.day.mode != cubarium_voxel::weather::DayMode::Off;
    let daylight = world.life_daylight();
    let dusk = clock_on
        && fauna
            .last_daylight
            .is_some_and(|before| before >= fauna.config.life.day_line)
        && daylight < fauna.config.life.day_line;
    fauna.last_daylight = Some(daylight);
    // LURE: where in the night it is, for the lineages' activity windows.
    fauna.night = if clock_on { crate::NightClock::of(&world.weather().sun()) } else { crate::NightClock::DAY };
    // RHYTHMS-CADENCE: where in the day it is, for the leanings.
    fauna.horizon = if clock_on { crate::Horizon::of(&world.weather().sun()) } else { crate::Horizon::NOON };
    // Contract v3: the day's phase, for the v3 observation's clock.
    fauna.day_phase = if clock_on { world.weather().sun().phase } else { 0.5 };
    let sky = Sky {
        downpour: view.weather.mode == cubarium_voxel::weather::RainMode::Downpour,
        rain_m_per_s: f64::from(view.weather.rain_m_per_s).max(0.0),
        clock_on,
        daylight,
    };

    cubarium_voxel::voxel_phase!(FaunaTerrain, { terrain(fauna, &view) });
    cubarium_voxel::voxel_phase!(FaunaMaintenance, {
        // WX4: who is asleep this tick is settled before anything is paid, so the
        // upkeep, the controller stage and the act all read the same answer.
        crate::pack::renew_dens(fauna, &view, flora.config().species(cubarium_voxel_flora::Species::Glowcap));
        circadian(fauna, &view, &flora.view(), clock_on, daylight);
        rain_latch(fauna, sky.rain_m_per_s);
        maintenance(fauna);
        drain_riders(fauna);
        // The minimal gut: the stomach digests into the room upkeep and the rider's
        // take just made.
        digest(fauna);
        heal(fauna);
        egg_upkeep(fauna);
        let matured = grow(fauna);
        disperse(fauna, &view, flora, &matured, crate::DispersalAt::Adult);
        drop_off_grown(fauna, &view);
        void_guts(fauna, flora);
        glider::void_gut_seeds(fauna, flora);
        // R3: calls, pups' independence, succession and provisioning.
        let pups: Vec<u64> = fauna
            .animals
            .iter()
            .filter(|a| {
                a.pack.is_some_and(|m| m.pup)
                    && a.founder.is_some_and(|f| {
                        let d = fauna.config.founder(f).dispersal;
                        d.enabled && d.at == crate::DispersalAt::Independent
                    })
            })
            .map(|a| a.id)
            .collect();
        crate::pack::upkeep(fauna, &view);
        // SP-NURSE: mothers nurse their dependent young.
        crate::nurse::nurse(fauna, &view);
        let independent: Vec<u64> = pups
            .into_iter()
            .filter(|id| {
                fauna
                    .animals
                    .iter()
                    .any(|a| a.id == *id && a.pack.is_some_and(|m| !m.pup))
            })
            .collect();
        disperse(
            fauna,
            &view,
            flora,
            &independent,
            crate::DispersalAt::Independent,
        );
    });
    TickStart { tick, seed, dusk, sky }
}

/// **The animal tick's read side**: the litter field's due update and the controller
/// stage. Reads the plant layer only (`&Flora`), so a schedule may run it beside a
/// plant step's own reads (light and the planned drink), which write nothing it reads.
/// Writes the animals' held actions and the senses.
pub(crate) fn read(
    fauna: &mut Fauna,
    world: &World,
    flora: &Flora,
    mut senses: Option<&mut crate::Senses>,
    start: &TickStart,
    threads: usize,
) {
    let view = world.view();
    // Maintenance and age settle first. A due field update then reads the litter as
    // it stands after the previous tick's feeding, before this tick's controllers.
    cubarium_voxel::voxel_phase!(FaunaField, {
        if let Some(s) = senses.as_deref_mut() {
            if start.tick % crate::UPDATE_TICKS == 0 {
                let fv = flora.view();
                s.update(&view, &fv);
            }
        }
        // SENSES-CORE: the scent field, while a live lineage smells it.
        if start.tick % crate::UPDATE_TICKS == 0 && fauna.scent_wanted() {
            let fv = flora.view();
            let mut field = std::mem::take(&mut fauna.scent);
            field.update(&view, &fv, &fauna.animals, &fauna.config);
            fauna.scent = field;
        }
    });
    cubarium_voxel::voxel_phase!(FaunaSense, {
        // The controller stage: sample due founders and hold their actions, before
        // anything moves or eats this tick, from the same pre-action state every
        // controller of the tick reads.
        // BRAIN-NEXT: the dry-run teachers read the layer as it stands before the stage,
        // after it, on a copy (`crate::dry`); the stage itself runs as without them.
        let before = crate::dry::before(fauna);
        controllers(fauna, &view, flora, senses, &start.sky, threads, None);
        if let Some(copy) = before {
            crate::dry::after(fauna, copy, &view, flora, &start.sky, threads);
        }
    });
}

/// **The animal tick's writes**: the act (motion, feeding through the plant layer's
/// `take_*`, strikes), births, deaths and the burr pass.
pub(crate) fn finish(fauna: &mut Fauna, world: &World, flora: &mut Flora, start: TickStart) {
    let TickStart { tick, seed, dusk, sky } = start;
    let view = world.view();
    cubarium_voxel::voxel_phase!(FaunaAct, {
        // Strike outcomes are events: the act only reads other bodies, and the hits
        // are applied here, serially, in the order the strikers acted.
        let mut hits: Vec<Hit> = Vec::new();
        // The balance measures: a free founder awake with its eating latch on wants
        // food this tick (a counter only).
        for a in fauna.animals.iter() {
            if let Some(f) = a.founder
                && a.attached.is_none()
            {
                fauna.ledger.alive_ticks_by_founder[f.index()] += 1;
                if fauna.horizon.up {
                    fauna.ledger.alive_day_ticks_by_founder[f.index()] += 1;
                }
            }
            if let Some(f) = a.founder
                && a.state != State::Asleep
                && a.attached.is_none()
            {
                fauna.ledger.awake_ticks_by_founder[f.index()] += 1;
                if fauna.horizon.up {
                    fauna.ledger.awake_day_ticks_by_founder[f.index()] += 1;
                }
                if a.eating {
                    fauna.ledger.wanting_ticks_by_founder[f.index()] += 1;
                }
            }
        }
        act(fauna, &view, flora, seed, tick, &sky, &mut hits);
        // CONTRACTS: the kicks' windups, contacts and recoveries (none without a kick table).
        let kicks = crate::kick::advance(fauna, &view, seed);
        let ring = f64::from(view.config.width) * view.config.voxel_m;
        // HUNKER: the pack's work on its hunkered prey, before this tick's strikes land.
        crate::vigil::pry(fauna, ring);
        for hit in hits {
            if hit.counter {
                apply_counter(fauna, flora, &hit, Some(&view));
                continue;
            }
            let stood = fauna
                .animals
                .binary_search_by_key(&hit.prey, |a| a.id)
                .ok()
                .map(|j| (fauna.animals[j].pose.x, fauna.animals[j].pose.z));
            // FINDINGS-FIX-2: a kill by a strike that began on a prey drawn to a lure is a
            // lure kill, though the lunge's loom closed the visit (booked once: the kill
            // books it itself while the prey is still drawn).
            let lure_kill = hit.lure && !crate::light::drawn_to_lure(fauna, hit.prey);
            let prey_founder = fauna.animals.binary_search_by_key(&hit.prey, |a| a.id).ok().and_then(|j| fauna.animals[j].founder);
            let tapped = hit.by == crate::Founder::Seedporter && prey_founder == Some(crate::Founder::Bellwing) && sp_miss_enabled();
            apply_hit(fauna, flora, hit.prey, hit.damage, hit.by, Some(&view));
            // CONTRACTS: a bite that landed may draw the bitten body's kick-back (its kick
            // table's switch; no lineage has one by default).
            crate::kick::kick_back(fauna, hit.prey, hit.striker);
            if tapped { eprintln!("SPMISS hit tick={} hunter={} prey={} damage={} killed={}", tick, hit.striker, hit.prey, hit.damage, fauna.animals.binary_search_by_key(&hit.prey, |a| a.id).is_err()); }

            if let (true, Some(f)) = (lure_kill, prey_founder) {
                if fauna.animals.binary_search_by_key(&hit.prey, |a| a.id).is_err() {
                    fauna.ledger.lure_kills[f.index()] += 1;
                }
            }
            // HUNTER-MEAL: a kill marks its place productive in the striker's memory.
            if let Some((x, z)) = stood {
                if fauna.animals.binary_search_by_key(&hit.prey, |a| a.id).is_err() {
                    crate::hunt::remember_kill(fauna, hit.striker, x, z, ring);
                }
            }
        }
        for k in kicks {
            fauna.ledger.kicks_landed_by_founder[k.by.index()] += 1;
            wound(fauna, flora, k.target, k.damage, k.by, Some(&view));
            // VH-FEED-KICK: a hound feels the kick that landed on it, and whose it was.
            crate::pack::tactics::felt_kick(fauna, k.target, k.kicker);
        }
    });
    crate::groom::step(fauna, &view, flora);
    cubarium_voxel::voxel_phase!(FaunaBirths, { reproduce(fauna, &view, flora, dusk) });
    cubarium_voxel::voxel_phase!(FaunaDeaths, {
        desiccation(fauna, &view, &sky);
        deaths(fauna, &view, flora)
    });
    // Burrs ride the bodies as they now stand: a seed latches on a step onto a donor's
    // face, falls off as its carrier walks, and drops where a body died.
    cubarium_voxel::voxel_phase!(FaunaBurrs, { burrs(fauna, world, flora) });
    #[cfg(feature = "profile")]
    cubarium_voxel::profile::add(
        cubarium_voxel::profile::Count::Animals,
        fauna.animals.len() as u64,
    );
}

/// **Whether a body is on the ground** for the burr pass: walking on its support face, or
/// perched on a ground face; not flying, gliding, climbing a stem or a wall, perched in a
/// crown, or riding a host.
pub(crate) fn grounded(a: &Animal) -> bool {
    if a.attached.is_some() {
        return false;
    }
    match a.mobility.mode {
        crate::MobilityMode::Ground => true,
        crate::MobilityMode::Perched => {
            matches!(a.mobility.perch, Some(p) if matches!(p.anchor, crate::PerchAnchor::Face(_)))
        }
        _ => false,
    }
}

/// One body as the plant layer's burr pass sees it ([`cubarium_voxel_flora::Carrier`]):
/// its face, its place, whether it is on the ground, and its lineage's coat factor (0 for a
/// body with no lineage).
pub(crate) fn burr_carrier(fauna: &Fauna, a: &Animal) -> cubarium_voxel_flora::Carrier {
    let coat = a.founder.map_or(0.0, |f| fauna.config.founder(f).burr_coat);
    cubarium_voxel_flora::Carrier {
        id: a.id,
        site: a.site,
        x_m: a.pose.x,
        z_m: a.pose.z,
        grounded: grounded(a),
        coat,
    }
}

/// The burr pass: every body handed to [`Flora::carry_burrs`] once a tick.
fn burrs(fauna: &Fauna, world: &World, flora: &mut Flora) {
    let carriers: Vec<cubarium_voxel_flora::Carrier> =
        fauna.animals.iter().map(|a| burr_carrier(fauna, a)).collect();
    flora.carry_burrs(world, &carriers);
}

/// Step 1: an animal standing on what is no longer a support face leaves the world.
fn terrain(fauna: &mut Fauna, view: &VoxelView<'_>) {
    if fauna
        .animals
        .iter()
        .all(|a| view.is_support(i64::from(a.site.x), a.site.y, a.site.z))
    {
        return;
    }
    let gone: Vec<Animal> = fauna
        .animals
        .iter()
        .filter(|a| !view.is_support(i64::from(a.site.x), a.site.y, a.site.z))
        .copied()
        .collect();
    fauna
        .animals
        .retain(|a| view.is_support(i64::from(a.site.x), a.site.y, a.site.z));
    for a in &gone {
        // As with a death: the body left the world, so its controller does too.
        fauna.controllers.take(a.id);
        fauna.trapped.remove(&a.id);
        fauna.drop_plans.remove(&a.id);
        fauna.wading.remove(&a.id);
        fauna.flushes.remove(&a.id);
        fauna.book_removed(a);
        fauna.free_riders(a, Some(view));
    }
}

/// Step 2: the upkeep, out of fuel, reserve and then body.
///
/// Respiration is one helper shared with the motor budget: organic matter from the
/// fuel, then reserve and body, each stock's energy leaving at its own density.
/// Only tissue sheds mineral to the gut. It returns what was paid and how much
/// of it came out of the body — the body's share is the structural loss a founder's
/// `Self` channel reports, and it is internal tissue loss, not a fictitious injury
/// model.
fn respire(fauna: &mut Fauna, i: usize, want: f64, kind: Respiration) -> (f64, f64) {
    if !(want > 0.0) {
        return (0.0, 0.0);
    }
    let brood_share = if fauna.animals[i].reproduction.brood > 0.0 {
        body::effective_config(&fauna.config, &fauna.animals[i]).reproduction.brood_share
    } else {
        0.0
    };
    let a = &mut fauna.animals[i];
    let reserve_before = a.reserve;
    let (out, from_body) = withdraw(a, want);
    let paid = out.organic;
    if !(paid > 0.0) {
        return (0.0, 0.0);
    }
    // The brood pays its share of what the reserve respired (D1b): the κ-rule splits net
    // income, so the soma's share of upkeep is not charged to the eggs.
    if brood_share > 0.0 {
        a.reproduction.brood =
            (a.reproduction.brood - brood_share * (reserve_before - a.reserve)).max(0.0);
    }
    // The mineral that organic matter held is shed to the gut, so a body's mineral stays
    // in proportion to its tissue instead of banking for life; the energy leaves as heat.
    a.gut.mineral += out.mineral;
    let founder = a.founder;
    book_respired(fauna, paid, kind);
    fauna.ledger.heat_out += out.energy;
    // The balance measures' spend by lineage (counters only).
    if let Some(f) = founder {
        match kind {
            Respiration::Maintenance => fauna.ledger.upkeep_by_founder[f.index()] += paid,
            Respiration::Motor | Respiration::Hunt => fauna.ledger.motor_by_founder[f.index()] += paid,
            _ => {}
        }
    }
    (paid, from_body)
}

/// **Organic matter out of a body**, the one pro-rata rule respiration and a parasite's
/// drain share: nitrogen-free fuel first at its own energy density, then reserve and
/// structure at their tissue density — the rule the plant layer's dead pools use,
/// so a body half respired is the same stuff it was. A body taken down to nothing hands
/// over every unit of its mineral and energy rather than keeping float dust. Returns the
/// three currencies that left and how much organic matter came out of the structure.
fn withdraw(a: &mut Animal, want: f64) -> (Taken, f64) {
    if !(want > 0.0) {
        return (Taken::default(), 0.0);
    }
    let fuel = take_fuel(a, want);
    let before = a.body + a.reserve;
    // The organism budget's upkeep with no income at this point of the tick: the reserve
    // first, and what it cannot pay is the body's dieback, respired out of the structure.
    let up = Budget::default().upkeep(&mut a.reserve, (want - fuel.organic).max(0.0));
    let from_body = up.unpaid.min(a.body);
    let paid = up.from_reserve + from_body;
    if !(paid > 0.0) {
        return (fuel, 0.0);
    }
    a.body -= from_body;
    let share = |stock: f64| {
        if paid >= before {
            stock
        } else {
            pro_rata(stock, paid, before)
        }
    };
    let e = share(a.energy);
    let m = share(a.mineral);
    a.energy -= e;
    a.mineral -= m;
    (
        Taken {
            organic: fuel.organic + paid,
            mineral: m,
            energy: fuel.energy + e,
        },
        from_body,
    )
}

/// Nitrogen-free fuel out at its own stored energy density.
fn take_fuel(a: &mut Animal, want: f64) -> Taken {
    let organic = want.max(0.0).min(a.fuel);
    let energy = if organic >= a.fuel {
        a.fuel_energy
    } else {
        pro_rata(a.fuel_energy, organic, a.fuel)
    };
    a.fuel -= organic;
    a.fuel_energy -= energy;
    Taken {
        organic,
        mineral: 0.0,
        energy,
    }
}

/// A reduced config cap returns excess fuel as digestion heat, conserving all stocks.
pub(crate) fn cap_fuel(fauna: &mut Fauna, i: usize) {
    let cap =
        body::effective_config(&fauna.config, &fauna.animals[i]).fuel_cap * fauna.animals[i].body;
    let excess = (fauna.animals[i].fuel - cap).max(0.0);
    if excess > 0.0 {
        let out = take_fuel(&mut fauna.animals[i], excess);
        book_respired(fauna, out.organic, Respiration::Digestion);
        fauna.ledger.heat_out += out.energy;
    }
}

/// `organic` out of the reserve, which must hold it, with the mineral and energy at the
/// body's own density: a gestation instalment's and a clutch's one pro-rata rule.
pub(crate) fn take_from_reserve(a: &mut Animal, organic: f64) -> Taken {
    let before = a.body + a.reserve;
    let mineral = pro_rata(a.mineral, organic, before);
    let energy = pro_rata(a.energy, organic, before);
    a.reserve -= organic;
    a.mineral -= mineral;
    a.energy -= energy;
    Taken {
        organic,
        mineral,
        energy,
    }
}

/// Step 2: **the parasites' take** (`design/handoffs/voxel-predation-parasite-2026-10-01.md`
/// §3, re-tied by LANTERNJAW-LIFE). Each rider takes [`crate::rider_take_per_s`] — its own
/// body × `parasite_take_body_per_day` per ecological day, so more the bigger it is — out of
/// its host by [`withdraw`]'s pro-rata rule, into its own reserve, mineral and energy, never
/// more than the host bears (`parasite_host_bear_per_day` × the host's body; one that wants
/// more drops off this tick, [`drop_off_grown`]). A transfer inside this layer, so no
/// boundary flow moves; the host's structural share is its `Self` channel's loss, as an
/// upkeep's is. Nothing protects the host: one drained below `body_min` starves as any other.
fn drain_riders(fauna: &mut Fauna) {
    let day = fauna.config.life.ecological_day_s;
    if !(day > 0.0) || !(fauna.config.parasite_take_body_per_day > 0.0) {
        return;
    }
    let bear_per_s = fauna.config.parasite_host_bear_per_day / day;
    for r in 0..fauna.animals.len() {
        let Some(hold) = fauna.animals[r].attached else {
            continue;
        };
        let Ok(h) = fauna.animals.binary_search_by_key(&hold.host, |a| a.id) else {
            continue;
        };
        let rate = crate::rider_take_per_s(&fauna.config, fauna.animals[r].body)
            .min(bear_per_s * fauna.animals[h].body);
        let want = rate * DT;
        let host = &mut fauna.animals[h];
        let (fuel_before, energy_before) = (host.fuel, host.fuel_energy);
        let (taken, from_body) = withdraw(host, want);
        let fuel_taken = fuel_before - host.fuel;
        let fuel_energy = energy_before - host.fuel_energy;
        if host.founder.is_some() && from_body > 0.0 {
            host.founder_state.feedback.structural_loss += from_body;
        }
        fauna.ledger.parasite_taken_organic += taken.organic;
        if let Some(f) = fauna.animals[h].founder {
            fauna.ledger.rider_take_lost_by_founder[f.index()] += taken.organic;
        }
        let fuel_cap = body::effective_config(&fauna.config, &fauna.animals[r]).fuel_cap;
        let rider = &mut fauna.animals[r];
        let fuel_kept = fuel_taken.min((fuel_cap * rider.body - rider.fuel).max(0.0));
        let kept_energy = pro_rata(fuel_energy, fuel_kept, fuel_taken);
        rider.fuel += fuel_kept;
        rider.fuel_energy += kept_energy;
        rider.reserve += taken.organic - fuel_taken;
        rider.mineral += taken.mineral;
        rider.energy += taken.energy - fuel_energy;
        book_respired(fauna, fuel_taken - fuel_kept, Respiration::Digestion);
        fauna.ledger.heat_out += fuel_energy - kept_energy;
    }
}

/// Step 2, after growth: **a rider drops off** (§4) where its host stands, a free juvenile
/// from here on, when it reaches its detach size — `parasite_detach_body_fraction` × its
/// lineage's `body_max` — or (LANTERNJAW-LIFE, Wrysk 2026-10-10: "they should drop off when
/// theyre too big") when it has **outgrown its host**: its host no longer bears its take
/// ([`crate::host_bears`]), whatever its size. One that outgrew its host below its detach
/// size then looks for a host that bears it (LJ-HOSTSWITCH, [`seek_hosts`]) while
/// `parasite_switch` is on.
fn drop_off_grown(fauna: &mut Fauna, view: &VoxelView<'_>) {
    let fraction = fauna.config.parasite_detach_body_fraction;
    let give_up = cubarium_voxel::ticks_at_least_one(fauna.config.parasite_switch_give_up_s);
    for r in 0..fauna.animals.len() {
        let Some(hold) = fauna.animals[r].attached else {
            continue;
        };
        let Ok(h) = fauna.animals.binary_search_by_key(&hold.host, |a| a.id) else {
            continue;
        };
        let host = fauna.animals[h];
        let line = fraction * body::effective_config(&fauna.config, &fauna.animals[r]).body_max;
        let rider = fauna.animals[r].body;
        if rider < line && crate::host_bears(&fauna.config, rider, host.body) {
            continue;
        }
        crate::release(&mut fauna.animals[r], &host);
        fauna.step_off_beside(view, r, &host);
        fauna.ledger.rider_dropoffs += 1;
        if rider < line && fauna.config.parasite_switch {
            fauna.host_quests.insert(fauna.animals[r].id, fauna.tick + give_up);
            fauna.host_switch.tried += 1;
        }
    }
    seek_hosts(fauna, view);
}

/// Step 2, after the drop-offs: **an outgrown rider takes a new host** (LJ-HOSTSWITCH; Wrysk
/// 2026-10-10: an outgrown larva looks for a bigger host). Each one looking, in id order,
/// takes the nearest host that bears it within `parasite_attach_radius_m` — the hatchlings'
/// rule, [`bearing_host`] — onto its back as a hatchling does. One at or past its detach
/// size needs none and stops looking; one past its give-up tick goes on free. The walk to a
/// host farther off is its heuristic's ([`host_seek`]).
fn seek_hosts(fauna: &mut Fauna, view: &VoxelView<'_>) {
    if fauna.host_quests.is_empty() {
        return;
    }
    let tick = fauna.tick;
    let ring = f64::from(view.config.width) * view.config.voxel_m;
    let fraction = fauna.config.parasite_detach_body_fraction;
    let quests: Vec<(u64, u64)> = fauna.host_quests.iter().map(|(&id, &until)| (id, until)).collect();
    for (id, until) in quests {
        let Ok(r) = fauna.animals.binary_search_by_key(&id, |a| a.id) else {
            // Gone (eaten, starved): nothing to tally.
            fauna.host_quests.remove(&id);
            continue;
        };
        let a = fauna.animals[r];
        let Some(lineage) = a.founder else {
            fauna.host_quests.remove(&id);
            continue;
        };
        if a.attached.is_some() {
            fauna.host_quests.remove(&id);
            continue;
        }
        if a.body >= fraction * body::effective_config(&fauna.config, &a).body_max {
            fauna.host_quests.remove(&id);
            fauna.host_switch.grew += 1;
            continue;
        }
        let at = (a.pose.x, a.pose.z);
        // Mid-strike or on a wall it finishes first; it takes a host from the ground.
        let free = a.strike.is_none() && a.mobility.climb.is_none();
        let radius = fauna.config.parasite_attach_radius_m;
        if let Some(host) = free.then(|| bearing_host(fauna, &[], lineage, a.body, at, radius, ring)).flatten() {
            let rider = &mut fauna.animals[r];
            rider.attached = Some(crate::Attachment { host: host.id });
            rider.site = host.site;
            rider.pose = host.pose;
            fauna.drop_plans.remove(&id);
            fauna.wading.remove(&id);
            fauna.trapped.remove(&id);
            fauna.host_quests.remove(&id);
            fauna.host_switch.made += 1;
            if let Some(f) = host.founder {
                fauna.host_switch.by_host[f.index()] += 1;
            }
            continue;
        }
        if tick >= until {
            fauna.host_quests.remove(&id);
            fauna.host_switch.gave_up += 1;
        }
    }
}

/// The write side's **host seek** (LJ-HOSTSWITCH): an outgrown rider looking for a new host,
/// over its own heuristic, walks straight at the nearest host that bears it within
/// `parasite_switch_radius_m` ([`bearing_host`]); `None` when it is not looking or none is
/// in reach, and its heuristic goes on (a lanternjaw is fast: it runs a host down rather
/// than waiting for one to pass, and hunts meanwhile when none is near).
fn host_seek(fauna: &Fauna, i: usize, view: &VoxelView<'_>) -> Option<Actions> {
    let a = &fauna.animals[i];
    if a.attached.is_some() || !fauna.host_quests.contains_key(&a.id) {
        return None;
    }
    let lineage = a.founder?;
    let ring = f64::from(view.config.width) * view.config.voxel_m;
    let host = bearing_host(fauna, &[], lineage, a.body, (a.pose.x, a.pose.z), fauna.config.parasite_switch_radius_m, ring)?;
    let (dx, dz) = crate::pose::wrapped_delta(a.pose.x, a.pose.z, host.pose.x, host.pose.z, ring);
    Some(glider::walk_toward(a, dx, dz, 0.0))
}

/// After the act: **every rider is carried** to its host's place (§2) — the host's face,
/// and the host's pose as its anchor on the host's back — so a rider crosses a seam, a
/// ledge or a wall exactly as its host did.
fn carry_riders(fauna: &mut Fauna) {
    for r in 0..fauna.animals.len() {
        let Some(hold) = fauna.animals[r].attached else {
            continue;
        };
        let Ok(h) = fauna.animals.binary_search_by_key(&hold.host, |a| a.id) else {
            continue;
        };
        let (site, pose) = (fauna.animals[h].site, fauna.animals[h].pose);
        let rider = &mut fauna.animals[r];
        rider.site = site;
        rider.pose = pose;
    }
}

/// One respiration on the boundary: the total and its one split, the same addend at the
/// same point, so the three splits sum to the total to the bit.
fn book_respired(fauna: &mut Fauna, paid: f64, kind: Respiration) {
    fauna.ledger.respired_out += paid;
    match kind {
        Respiration::Maintenance => fauna.ledger.respired_maintenance_out += paid,
        Respiration::Motor => fauna.ledger.respired_motor_out += paid,
        Respiration::Digestion => fauna.ledger.respired_digestion_out += paid,
        Respiration::Gestation => fauna.ledger.respired_gestation_out += paid,
        Respiration::Hunt => fauna.ledger.respired_hunt_out += paid,
        Respiration::Heal => fauna.ledger.respired_heal_out += paid,
        Respiration::Provision => {}
    }
}

/// The provisioning loss (roster R3, [`crate::pack`]): `organic` respired and `energy`
/// out as heat.
pub(crate) fn book_provision_loss(fauna: &mut Fauna, organic: f64, energy: f64) {
    if organic > 0.0 {
        book_respired(fauna, organic, Respiration::Provision);
    }
    fauna.ledger.heat_out += energy;
}

/// Step 2, last: every animal whose age is a multiple of [`crate::LifeConfig::gut_void_s`]
/// (30 s) empties its gut
/// onto its own face as litter. Birth ticks stagger the voids; a gut holding only mineral
/// goes straight to the soluble pool (the flora layer's zero-organic deposit rule).
fn void_guts(fauna: &mut Fauna, flora: &mut Flora) {
    let period = cubarium_voxel::ticks_at_least_one(fauna.config.life.gut_void_s);
    for i in 0..fauna.animals.len() {
        let a = &mut fauna.animals[i];
        if a.age_ticks % period != 0 {
            continue;
        }
        let dung = std::mem::take(&mut a.gut);
        let site = a.site;
        fauna.book_deposit(flora, site, DepositKind::Litter, dung);
    }
}

/// **Rain grounds flyers** (backlog §12, Wrysk 2026-10-08: "rain should ground
/// bellwings"): each flying lineage's latch ([`Mobility::rain_grounded`]) goes on at the
/// world's rain rate (`rain`, m/s; the weather's one rate, what the plants read too) at or
/// over its [`crate::Flight::rain_ground_m_per_s`], and off once the rain is under that
/// less [`crate::Flight::rain_ground_hysteresis_m_per_s`] or has stopped.
fn rain_latch(fauna: &mut Fauna, rain: f64) {
    for k in 0..fauna.animals.len() {
        let Some(founder) = fauna.animals[k].founder else {
            continue;
        };
        let Some(flight) = fauna.config.founder(founder).flight else {
            continue;
        };
        let m = &mut fauna.animals[k].mobility;
        m.rain_grounded = rain > 0.0
            && if m.rain_grounded {
                rain >= flight.rain_ground_m_per_s - flight.rain_ground_hysteresis_m_per_s
            } else {
                rain >= flight.rain_ground_m_per_s
            };
    }
}

/// Step 1b: who sleeps this tick, and the two latches — **one precedence**
/// (`design/metabolism-meals-2026-10-02.md` repair 2 item 7):
///
/// - **a. Safety and alert**: a body on a wall, flying, not perched (a glider), under
///   water, mid-strike or alerted by a hit ([`crate::PredationConfig::alert_s`]) stays
///   awake, as before.
/// - **b. The emergency latch** ([`Animal::emergency`]): on at reserve hunger
///   `wake_hunger`, off below `emergency_clear_hunger`. While on, the body is awake in any
///   phase; it may seek and eat when its eating latch is on, and with a full stomach it
///   stays up waiting for digestion.
/// - **c. Off-phase**: otherwise a body sleeps when its lineage's
///   [`Activity`](crate::Activity) says it is not its hour, where it stands.
/// - **d. On-phase**: awake. The eating latch decides whether it eats and hunts.
///
/// **The eating latch** ([`Animal::eating`]) goes on here, in an awake body that is
/// on-phase or in an emergency, once its stomach is at or under `s_start · K`; it goes off
/// only at a full event, where the intake is. A body that falls asleep drops its held
/// actions, so waking does not replay a stale stride; heuristic bodies carry no lineage
/// and never sleep. A recovery is dropped by a sleep that outlasts it.
fn circadian(fauna: &mut Fauna, view: &VoxelView<'_>, fv: &cubarium_voxel_flora::FloraView<'_>, clock_on: bool, daylight: f64) {
    let downpour = view.weather.mode == cubarium_voxel::weather::RainMode::Downpour;
    let life = fauna.config.life;
    let v = view.config.voxel_m;
    let tick = fauna.tick;
    // RHYTHMS-CADENCE: the tick on the day's 24-hour clock, and the running bodies a sleeper
    // may hear (none to read while nobody sleeps).
    let hours = crate::rhythm::hours_per_tick(&view.config.day);
    let seed = view.config.seed;
    let loud = if fauna.animals.iter().any(|a| a.state == State::Asleep) {
        crate::rhythm::loud_bodies(fauna)
    } else {
        Vec::new()
    };
    let alert_ticks = u32::try_from(cubarium_voxel::ticks_at_least_one(fauna.config.predation.alert_s)).unwrap_or(u32::MAX);
    // The packs raising pups: an adult's bout there is a provisioning one.
    let mut nurseries: Vec<u64> = fauna
        .animals
        .iter()
        .filter_map(|a| a.pack.filter(|m| m.pup).map(|m| m.pack))
        .collect();
    nurseries.sort_unstable();
    nurseries.dedup();
    // BRAIN-REST: the saved asks and bouts of bodies gone from the world.
    if tick % 64 == 0 && !(fauna.rest_asks.is_empty() && fauna.brain_bouts.is_empty()) {
        let alive: rustc_hash::FxHashSet<u64> = fauna.animals.iter().map(|a| a.id).collect();
        fauna.rest_asks.retain(|id, _| alive.contains(id));
        fauna.brain_bouts.retain(|id, _| alive.contains(id));
    }
    for i in 0..fauna.animals.len() {
        let Some(founder) = fauna.animals[i].founder else {
            continue;
        };
        let phys = *fauna.config.founder(founder);
        let leaning = clock_on && phys.active == crate::Activity::Leaning;
        let shade = leaning && phys.rhythm.shade_is_night;
        // A dark-active body reads the light at its own face (§3.1B): the face's
        // terrain sky, cached per face. So does a leaning that takes shade for night.
        let sky = if clock_on && (phys.active.reads_face_light() || shade) {
            let a = &fauna.animals[i];
            match a.pose.column(v, view.config.depth) {
                Some((cx, cz)) => {
                    let wx = cx.rem_euclid(i64::from(view.config.width));
                    let y = a.sense_layer(v);
                    fauna.face_sky(view, wx, y, cz)
                }
                None => 1.0,
            }
        } else {
            1.0
        };
        // RHYTHMS-CADENCE: a leaning's rest bouts, drawn from its chance now.
        let dark_face = sky.clamp(0.0, 1.0) * daylight < life.day_line;
        {
            let a = &mut fauna.animals[i];
            if leaning {
                let p = phys.rhythm.chance(fauna.horizon, dark_face);
                let left = crate::rhythm::advance(&phys.rhythm, a.rest_ticks, p, hours, seed, a.id, tick);
                if a.rest_ticks == 0 && left > 0 {
                    fauna.ledger.rest_bouts_by_founder[founder.index()] += 1;
                }
                a.rest_ticks = left;
            } else {
                a.rest_ticks = 0;
            }
        }
        // CALLS-REST: the one wake rule (`rhythm::wakes`): a footfall it hears (a run, or a
        // rush where its lineage hears one), a heeded call, or in light rest a pressing threat.
        // It stays up for `alert_s`.
        if fauna.animals[i].state == State::Asleep {
            if let Some(why) = crate::rhythm::wakes(fauna, view, i, &loud) {
                fauna.animals[i].alert_ticks = fauna.animals[i].alert_ticks.max(alert_ticks);
                let k = founder.index();
                match why {
                    crate::rhythm::Wake::Footfall => fauna.ledger.alarm_wakes_by_founder[k] += 1,
                    crate::rhythm::Wake::Call => fauna.ledger.call_wakes_by_founder[k] += 1,
                    crate::rhythm::Wake::Sight => fauna.ledger.sight_wakes_by_founder[k] += 1,
                }
            }
        }
        let a = &fauna.animals[i];
        let hungry = hunger(a, &phys.core);
        let emergency = if a.emergency {
            hungry >= phys.emergency_clear_hunger
        } else {
            hungry >= phys.wake_hunger
        };
        let on_phase = phys.lure_daylight
            || if leaning {
                a.rest_ticks == 0
            } else {
                phys.active.awake_in(clock_on, daylight, sky, fauna.night, phys.active_window, &life)
            };
        let dark_awake = on_phase
            && clock_on
            && daylight >= life.day_line
            && (phys.active.reads_face_light() || (shade && dark_face));
        let free = a.alert_ticks == 0
            && a.mobility.climb.is_none()
            && a.mobility.mode != crate::MobilityMode::Fly
            // A glider (F3) sleeps only perched.
            && (phys.glider.is_none() || a.mobility.mode == crate::MobilityMode::Perched)
            && a.mobility.submerged_ticks == 0
            // AMPHIBIOUS-SNAIL: an amphibious body out of water stays up to get back.
            && !a.mobility.ashore
            // Repair 3 item 4: attached (a rider) and falling bodies are awake.
            && a.attached.is_none()
            && !a.mobility.falling
            && !a
                .strike
                .is_some_and(|s| s.phase != StrikePhase::Recovery);
        let rest = if free && !on_phase && !emergency {
            Some(Rest::Sleep)
        } else {
            None
        };
        // R3: a pack adult under its pack heuristic rests and sleeps at its den; away from
        // it, it stays awake while a reachable trip fits its reserve budget.
        // ROOSTS (backlog §12): a glider under its own heuristic, about to fall asleep on a
        // perch, stays up while a better roost (a vaulttree crown, a vine-climbed ledge top)
        // is in reach of its budget and reserve — den rest's rule. One already asleep stays.
        // CONTRACTS: rest through the action interface. A heuristic's rest decisions are its
        // requests to stay up (the walk home, the better roost), unchanged; a brain's request
        // to rest is granted when the body is free and not in an emergency. The rhythm stays
        // the backstop.
        if rest.is_none() {
            // CALLS-REST step 5: a bed search lasts one rest onset.
            fauna.animals[i].doze.bed_until = 0;
        }
        let ask = match rest {
            Some(_) => heuristic_rest_ask(fauna, i, view, fv, downpour),
            None => fauna.rest_asks.get(&fauna.animals[i].id).copied().unwrap_or_default(),
        };
        fauna.animals[i].intent.executed.rest = ask;
        // BRAIN-REST: a granted brain rest lasts one bout (`rest.brain_bout_s` from its first
        // granted tick), then the ask clears and the body is awake for its brain's next
        // sample; not inside a deep bout. Any other outcome (the rhythm's sleep, a wake, an
        // emergency, no ask) ends the bout record.
        let id = fauna.animals[i].id;
        let rest = match (rest, ask) {
            (Some(_), crate::RestAsk::Stay) => {
                fauna.brain_bouts.remove(&id);
                None
            }
            (Some(r), _) => {
                fauna.brain_bouts.remove(&id);
                Some(r)
            }
            (None, crate::RestAsk::Rest) if free && !emergency => {
                let bout = phys.rest.brain_bout_s;
                if bout.is_finite() && bout > 0.0 {
                    let until = *fauna
                        .brain_bouts
                        .entry(id)
                        .or_insert_with(|| tick + cubarium_voxel::ticks_at_least_one(bout));
                    if tick >= until && !crate::sleep::deep_asleep(&fauna.animals[i]) {
                        fauna.brain_bouts.remove(&id);
                        fauna.rest_asks.remove(&id);
                        fauna.animals[i].intent.executed.rest = crate::RestAsk::Default;
                        fauna.animals[i].homing_ticks = 0;
                        None
                    } else {
                        Some(Rest::Sleep)
                    }
                } else {
                    Some(Rest::Sleep)
                }
            }
            (None, _) => {
                fauna.brain_bouts.remove(&id);
                fauna.animals[i].homing_ticks = 0;
                None
            }
        };
        let a = &mut fauna.animals[i];
        a.alert_ticks = a.alert_ticks.saturating_sub(1);
        let was_asleep = a.state == State::Asleep;
        if emergency && !a.emergency && was_asleep {
            fauna.ledger.emergency_wakes_by_founder[founder.index()] += 1;
        }
        a.emergency = emergency;
        match rest {
            Some(reason) => {
                if a.state != State::Asleep {
                    a.state = State::Asleep;
                    a.founder_state.held = Actions::REST;
                    a.founder_state.lure = 0.0;
                    a.founder_state.lift = 0.0;
                }
                a.rest = Some(reason);
                if a.strike.is_some_and(|s| s.phase == StrikePhase::Recovery) {
                    a.strike = None;
                }
            }
            None => {
                if a.state == State::Asleep {
                    a.state = State::Resting;
                }
                a.rest = None;
            }
        }
        if was_asleep != (a.state == State::Asleep) {
            fauna.ledger.sleep_flips_by_founder[founder.index()] += 1;
        }
        if dark_awake && a.state != State::Asleep {
            fauna.ledger.dark_awake_ticks_by_founder[founder.index()] += 1;
        }
        // The eating latch goes on (rule 7: on-phase, or in an emergency; awake; not
        // riding a host).
        let nursing = a
            .pack
            .is_some_and(|m| !m.pup && nurseries.binary_search(&m.pack).is_ok());
        let kind = bout_kind_now(a, &phys, nursing);
        if !a.eating
            && a.state != State::Asleep
            && (on_phase || emergency)
            && a.attached.is_none()
            && a.stomach.organic() <= phys.gut.s_start(hungry) * phys.gut.capacity(a.body)
        {
            a.eating = true;
            a.bout = Some(crate::Bout {
                start_tick: tick,
                kind,
            });
        }
        if let Some(b) = a.bout.as_mut() {
            b.kind = b.kind.max(kind);
        }
        // CALLS-REST: the body's light-or-deep choice inside its rest ([`crate::sleep`]).
        let began = crate::sleep::step(fauna, i, founder, was_asleep, seed);
        let a = &fauna.animals[i];
        let k = founder.index();
        if a.state == State::Asleep {
            if a.doze.light {
                fauna.ledger.light_rest_ticks_by_founder[k] += 1;
            } else {
                fauna.ledger.deep_sleep_ticks_by_founder[k] += 1;
            }
        }
        if began {
            fauna.ledger.deep_bouts_by_founder[k] += 1;
        }
    }
}

/// **A heuristic's rest request** when its rhythm would put it to rest (CONTRACTS): stay up
/// while a pack adult must walk home to its den, or a glider under its own heuristic has a
/// better roost in reach (unless in a downpour or already asleep); else no request.
fn heuristic_rest_ask(fauna: &mut Fauna, i: usize, view: &VoxelView<'_>, fv: &cubarium_voxel_flora::FloraView<'_>, downpour: bool) -> crate::RestAsk {
    let founder = fauna.animals[i].founder.expect("a founder");
    let phys = *fauna.config.founder(founder);
    // CALLS-REST step 5: a heuristic body looks for a bed with no hunter scent first.
    let heuristic = fauna.controllers.layer(fauna.animals[i].id) == crate::Layer::Heuristic;
    if heuristic && crate::sleep::bed_ask(fauna, i, view) {
        return crate::RestAsk::Stay;
    }
    if crate::pack::must_walk_home(fauna, i, view)
        || (phys.glider.is_some()
            && !downpour
            && fauna.animals[i].state != State::Asleep
            && (fauna.controllers.layer(fauna.animals[i].id) == crate::Layer::Heuristic)
            && glider::better_roost(fauna, i, view, fv).is_some())
    {
        crate::RestAsk::Stay
    } else {
        crate::RestAsk::Default
    }
}

/// **Out of its waking now** (before the emergency latch): its lineage's rhythm says it is
/// not its hour, or, a leaning ([`crate::Activity::Leaning`]), it is in a rest bout. The
/// circadian stage settles the bout first each tick; the reflexes that send a body to its
/// rest (the roost, the den, a flyer's descent) read this.
pub(crate) fn off_phase(fauna: &Fauna, i: usize, clock_on: bool, daylight: f64) -> bool {
    let a = &fauna.animals[i];
    let Some(f) = a.founder else {
        return false;
    };
    let phys = fauna.config.founder(f);
    if phys.active == crate::Activity::Leaning {
        return clock_on && a.rest_ticks > 0;
    }
    !phys.active.awake_in(clock_on, daylight, 1.0, fauna.night, phys.active_window, &fauna.config.life)
}

/// The reason a bout has this tick (repair 2 item 8): an emergency, then a body feeding
/// young (carrying an escrow, or a pack adult whose pack has pups; a giver's bout is also
/// marked where it regurgitates, [`crate::pack`]), then a juvenile under its lineage's
/// `birth_body`, else normal.
fn bout_kind_now(a: &Animal, phys: &crate::FounderPhysiology, nursing: bool) -> crate::BoutKind {
    if a.emergency {
        crate::BoutKind::Emergency
    } else if a.reproduction.escrow.is_some() || nursing {
        crate::BoutKind::Provisioning
    } else if a.body < phys.core.birth_body {
        crate::BoutKind::Growth
    } else {
        crate::BoutKind::Normal
    }
}

/// **The full event** (repair 2 item 3): an eating body whose stomach has reached
/// `full_tolerance · K` stops wanting food, and its bout is booked as a meal of its kind
/// with its duration. Called wherever food enters a stomach.
pub(crate) fn settle_full(fauna: &mut Fauna, i: usize) {
    let a = &fauna.animals[i];
    let Some(f) = a.founder else {
        return;
    };
    if !a.eating {
        return;
    }
    let gut = fauna.config.founder(f).gut;
    if a.stomach.organic() < gut.full_tolerance * gut.capacity(a.body) {
        return;
    }
    let tick = fauna.tick;
    let a = &mut fauna.animals[i];
    a.eating = false;
    if let Some(b) = a.bout.take() {
        let k = b.kind.index();
        fauna.ledger.meals_by_founder_kind[f.index()][k] += 1;
        fauna.ledger.meal_ticks_by_founder_kind[f.index()][k] += tick.saturating_sub(b.start_tick);
    }
}

/// Raw food into a body's stomach, booked swallowed, then the full event.
pub(crate) fn swallow(fauna: &mut Fauna, i: usize, food: Food, t: Taken) {
    let p = &mut fauna.animals[i].stomach.pools[food.index()];
    p.organic += t.organic;
    p.mineral += t.mineral;
    p.energy += t.energy;
    let l = &mut fauna.ledger;
    l.swallowed.organic += t.organic;
    l.swallowed.mineral += t.mineral;
    l.swallowed.energy += t.energy;
    if fauna.animals[i].pack.is_some_and(|m| m.pup) {
        l.pup_swallowed_organic += t.organic;
    }
    if let Some(f) = fauna.animals[i].founder {
        l.swallowed_by_founder[f.index()] += t.organic;
        // A bite covers its controller interval: the balance measures' eating ticks.
        l.eating_ticks_by_founder[f.index()] += fauna.config.manifest(f).cadence_ticks();
    }
    settle_full(fauna, i);
}

/// `organic` of raw food out of a pool, with its mineral and energy pro rata; the whole
/// pool when that is all of it, so no float dust is left.
pub(crate) fn take_from_pool(pool: &mut Taken, organic: f64) -> Taken {
    if !(organic > 0.0) || !(pool.organic > 0.0) {
        return Taken::default();
    }
    if organic >= pool.organic {
        return std::mem::take(pool);
    }
    let t = Taken {
        organic,
        mineral: pro_rata(pool.mineral, organic, pool.organic),
        energy: pro_rata(pool.energy, organic, pool.organic),
    };
    pool.organic -= t.organic;
    pool.mineral -= t.mineral;
    pool.energy -= t.energy;
    t
}

/// **The gut's bite** (repair 2 item 4): `b · body · period · effort` of raw food at the
/// mouth, diminished `E / (E + bite_half_stock)` by the stock `E` there, and never more
/// than reaches the stomach's room `K − S` after the share a sloppy feeder spills. Zero
/// unless the eating latch is on: a body that does not want food takes no bite, and a
/// full stomach stops the mouth.
fn gut_bite(a: &Animal, phys: &crate::FounderPhysiology, food: Food, period_s: f64, effort: f64, stock: f64) -> f64 {
    if !a.eating || !(stock > 0.0) {
        return 0.0;
    }
    // A small body bites faster per unit of itself (the gut's mass exponent, D1b).
    let want = phys.gut.bite_per_body_s * phys.gut.throughput_factor(a.body, phys.core.body_max) * a.body * period_s * effort;
    if !(want > 0.0) {
        return 0.0;
    }
    let k = phys.bite_half_stock_for(food);
    let diminished = if k > 0.0 { want * (stock / (stock + k)) } else { want };
    let room = (phys.gut.capacity(a.body) - a.stomach.organic()).max(0.0);
    let reaches = (1.0 - phys.bite_spill_fraction.clamp(0.0, 1.0)).max(1e-12);
    diminished.min(room / reaches).max(0.0)
}

/// Step 2, after upkeep: **digestion** (repair 2 item 2, repair 3 item 1). Each food
/// class's pool empties at `S_c · dt / τ_c`, as far as the reserve has room for what it
/// yields: the assimilable classes' raw digestion is scaled down together so that their
/// effective, mineral-limited yields `y_eff` sum to at most `room = reserve cap −
/// reserve`. A class whose `y_eff` is zero (no mineral) **passes** at `S_c · dt / τ_c`
/// whatever the room: it places no reserve, so reserve room does not throttle it. Everything
/// digested or passed goes through the one assimilation rule ([`assimilate`]): the yield
/// into the reserve as far as its mineral funds it, the undigested share to egesta, the
/// unfunded share to fuel and its overflow respired; what is not digested stays in the stomach. No mineral is
/// invented.
fn digest(fauna: &mut Fauna) {
    for i in 0..fauna.animals.len() {
        let a = &fauna.animals[i];
        let Some(f) = a.founder else {
            continue;
        };
        if a.stomach.is_empty() {
            continue;
        }
        let phys = *fauna.config.founder(f);
        let sc = phys.core;
        let room = (sc.reserve_of(a.body) - a.reserve).max(0.0);
        // A small body digests faster per unit of itself (the gut's mass exponent, D1b).
        let speed = phys.gut.throughput_factor(a.body, sc.body_max);
        let mut raw = [0.0f64; Food::COUNT];
        let mut passing = [0.0f64; Food::COUNT];
        let mut yields = 0.0;
        for food in Food::ALL {
            let p = a.stomach.pools[food.index()];
            if !(p.organic > 0.0) {
                continue;
            }
            let tau = phys.gut.digestion_s[food.index()] / speed;
            let d = if tau > 0.0 { (p.organic * DT / tau).min(p.organic) } else { p.organic };
            let funded = if sc.n_tissue > 0.0 {
                p.mineral / (sc.n_tissue * p.organic)
            } else {
                f64::INFINITY
            };
            let y_eff = phys.yield_for(food).min(funded).max(0.0);
            if !(y_eff > 0.0) {
                passing[food.index()] = d;
                continue;
            }
            raw[food.index()] = d;
            yields += d * y_eff;
        }
        let scale = if yields > room { room / yields } else { 1.0 };
        for food in Food::ALL {
            let d = raw[food.index()] * scale + passing[food.index()];
            if !(d > 0.0) {
                continue;
            }
            let t = take_from_pool(&mut fauna.animals[i].stomach.pools[food.index()], d);
            let l = &mut fauna.ledger;
            l.digested.organic += t.organic;
            l.digested.mineral += t.mineral;
            l.digested.energy += t.energy;
            l.digested_by_founder[f.index()] += t.organic;
            let fuel_before = fauna.animals[i].fuel;
            let placed = assimilate(fauna, i, &sc, t, phys.yield_for(food)) + fauna.animals[i].fuel
                - fuel_before;
            fauna.ledger.assimilated_by_founder[f.index()] += placed;
            if fauna.animals[i].mobility.ashore {
                fauna.ledger.ashore_assimilated += placed;
            }
            fauna.animals[i].founder_state.feedback.intake += placed;
        }
    }
}

fn maintenance(fauna: &mut Fauna) {
    for i in 0..fauna.animals.len() {
        // A founder body pays upkeep under its own physiology's rate.
        let sc = body::effective_config(&fauna.config, &fauna.animals[i]);
        let is_founder = fauna.animals[i].founder.is_some();
        // Asleep it pays its lineage's sleep factor of it; so does a flyer sitting out the
        // rain on a perch (backlog §12, Wrysk 2026-10-08: "should basically be like
        // sleeping while it rains").
        let m = fauna.animals[i].mobility;
        let rain_rest = m.rain_grounded && m.mode == crate::MobilityMode::Perched;
        let rest = match fauna.animals[i].founder {
            Some(founder) if fauna.animals[i].state == State::Asleep || rain_rest => {
                fauna.config.founder(founder).sleep_upkeep
            }
            _ => 1.0,
        };
        let a = &mut fauna.animals[i];
        a.age_ticks = a.age_ticks.saturating_add(1);
        // LIFE-HISTORY: a gestating body pays its lineage's pregnancy upkeep factor.
        let pregnant = if a.reproduction.escrow.is_some() {
            sc.reproduction.gestation_upkeep_factor.max(0.0)
        } else {
            1.0
        };
        let pup = a.pack.is_some_and(|m| m.pup);
        let want = sc.maintenance_per_s * rest * pregnant * a.body * DT;
        let (paid, from_body) = respire(fauna, i, want, Respiration::Maintenance);
        if pup {
            fauna.ledger.pup_upkeep_organic += paid;
        }
        if is_founder && from_body > 0.0 {
            fauna.animals[i].founder_state.feedback.structural_loss += from_body;
        }
    }
}

/// Step 2: **healing** (predation §2.4). A wound heals at [`crate::PredationConfig::wound_heal_per_s`],
/// each unit costing [`crate::PredationConfig::heal_cost`] × its body out of the **reserve** alone: a body
/// whose reserve cannot pay heals only as far as it can, and a starving one not at all.
fn heal(fauna: &mut Fauna) {
    let cfg = fauna.config.predation;
    for i in 0..fauna.animals.len() {
        let a = &fauna.animals[i];
        if !(a.wound > 0.0) || !(a.reserve > 0.0) || !(a.body > 0.0) {
            continue;
        }
        let unit = cfg.heal_cost * a.body;
        let mut healed = a.wound.min(cfg.wound_heal_per_s * DT);
        if unit > 0.0 && healed * unit > a.reserve {
            healed = a.reserve / unit;
        }
        if !(healed > 0.0) {
            continue;
        }
        let (paid, _) = respire(fauna, i, healed * unit, Respiration::Heal);
        if !(paid > 0.0) && unit > 0.0 {
            continue;
        }
        let a = &mut fauna.animals[i];
        a.wound = (a.wound - healed).max(0.0);
        if let Some(f) = a.founder {
            fauna.ledger.healed_by_founder[f.index()] += healed;
        }
    }
}

/// Step 2: a **dormant egg's upkeep** (predation §2.13): [`crate::PredationConfig::egg_upkeep_per_s`] of
/// its own organic matter, respired with the energy at its own density; its mineral stays
/// in the egg.
fn egg_upkeep(fauna: &mut Fauna) {
    let upkeep = fauna.config.predation.egg_upkeep_per_s;
    for k in 0..fauna.clutches.len() {
        let c = &mut fauna.clutches[k];
        if c.dormancy.is_none() || !(c.organic > 0.0) {
            continue;
        }
        let paid = (upkeep * c.organic * DT).min(c.organic);
        let heat = pro_rata(c.energy, paid, c.organic);
        c.organic -= paid;
        c.energy -= heat;
        book_respired(fauna, paid, Respiration::Maintenance);
        fauna.ledger.heat_out += heat;
    }
}

/// Step 2, second half: **growth on plant time**. New structure is
/// paid out of the reserve — a relabelling inside the body, so mineral and energy stay
/// where they are and no ledger moves — at `growth_max_per_s · DT · fullness`, where
/// `fullness` is the reserve over `reserve_cap · body`, at most one, and never past
/// `body_max` or what the reserve holds.
///
/// So however much a juvenile eats it cannot outgrow the cap, and one short of food slows
/// its growth instead of spending the reserve it lives on. Intake never builds structure
/// directly any more: it goes to the reserve ([`assimilate`]), and this is the only way
/// in to the body.
fn grow(fauna: &mut Fauna) -> Vec<u64> {
    let mut matured = Vec::new();
    for i in 0..fauna.animals.len() {
        let sc = body::effective_config(&fauna.config, &fauna.animals[i]);
        let a = &mut fauna.animals[i];
        // The brood is at most the reserve; structure is built only above it (D1b).
        a.reproduction.brood = a.reproduction.brood_held(a.reserve);
        let free = a.reserve - a.reproduction.brood;
        let room = (sc.body_max - a.body).max(0.0);
        if !(room > 0.0) || !(free > 0.0) {
            continue;
        }
        let cap = sc.reserve_of(a.body);
        let fullness = if cap > 0.0 {
            (a.reserve / cap).min(1.0)
        } else {
            1.0
        };
        // The organism budget's build out of a held reserve, at par: a body's structure
        // costs no construction respiration and its mineral is already in it.
        let before = a.body;
        a.body += build_from(
            &mut a.reserve,
            (sc.growth_max_per_s * DT * fullness).min(room).min(free),
            1.0,
        );
        // The measured maturation: the tick it grows through its breeding size.
        if let Some(f) = a.founder {
            fauna.ledger.growth_by_founder[f.index()] += a.body - before;
            if before < sc.birth_body && a.body >= sc.birth_body {
                matured.push(a.id);
                fauna.ledger.matured_by_founder[f.index()] += 1;
                fauna.ledger.maturation_ticks_by_founder[f.index()] += a.age_ticks;
            }
        }
    }
    matured
}

/// Only this tick's transition candidates can leave. A candidate leaves when its group's
/// **established** members plus itself exceed `group_max`:
/// - a pack's group is its record; established is every member past independence (not a
///   pup), so pups at the den never count; the breeder never leaves;
/// - a herding lineage's group is its herd ([`crate::herd_labels`]: chains within
///   `herding.comfort_m`, x wrapping), whatever `herding.young_only`; established is every
///   adult.
///
/// Candidates still to be judged are not established yet; one that stays counts for the
/// candidates after it, so the order of a tick's arrivals cannot overfill a group.
fn disperse(
    fauna: &mut Fauna,
    view: &VoxelView<'_>,
    flora: &mut Flora,
    candidates: &[u64],
    at: crate::DispersalAt,
) {
    let ring = f64::from(view.config.width) * view.config.voxel_m;
    let mut pack_changed = false;
    for (k, &id) in candidates.iter().enumerate() {
        let pending = &candidates[k + 1..];
        let Some(i) = fauna.animals.iter().position(|a| a.id == id) else {
            continue;
        };
        let a = &fauna.animals[i];
        let Some(f) = a.founder else { continue };
        let phys = fauna.config.founder(f);
        let d = phys.dispersal;
        if !d.enabled || d.at != at {
            continue;
        }
        let established = if let Some(m) = a.pack {
            let Some(pack) = fauna.packs().iter().find(|p| p.id == m.pack) else {
                continue;
            };
            if pack.breeder == Some(id) {
                continue;
            }
            fauna
                .animals
                .iter()
                .filter(|o| {
                    o.id != id
                        && !pending.contains(&o.id)
                        && o.pack.is_some_and(|p| p.pack == m.pack && !p.pup)
                })
                .count()
        } else {
            // LS-DEFEND (§11.6): a herding lineage's group is its herd whether the pull is for
            // the young only or for all ages (`herding.young_only` no longer gates it).
            if at != crate::DispersalAt::Adult || phys.herding.comfort_m <= 0.0 {
                continue;
            }
            let lineage: Vec<&Animal> =
                fauna.animals.iter().filter(|o| o.founder == Some(f)).collect();
            let herd = crate::herd_labels(&lineage, phys.herding.comfort_m, ring);
            let Some(me) = lineage.iter().position(|o| o.id == id) else { continue };
            lineage
                .iter()
                .zip(&herd)
                .filter(|(o, h)| {
                    **h == herd[me]
                        && o.id != id
                        && !pending.contains(&o.id)
                        && o.body >= phys.core.birth_body
                })
                .count()
        };
        let count = established + 1;
        if count <= d.group_max as usize {
            continue;
        }
        // Deposit first: if the parcel is refused, keep the animal and all its matter.
        // Unlike book_deposit's other callers, this path must never fall back to export.
        let parcel = Taken {
            organic: a.stored_organic(),
            mineral: a.stored_mineral(),
            energy: a.stored_energy(),
        };
        if !flora.deposit_recycled(
            a.site,
            cubarium_voxel_flora::Deposit {
                kind: DepositKind::Litter,
                organic: parcel.organic,
                mineral: parcel.mineral,
                energy: parcel.energy,
            },
        ) {
            continue;
        }
        fauna.ledger.deposited_organic_out += parcel.organic;
        fauna.ledger.deposited_mineral_out += parcel.mineral;
        fauna.ledger.deposited_energy_out += parcel.energy;
        let fed = a.stomach.total();
        fauna.ledger.stomach_out.organic += fed.organic;
        fauna.ledger.stomach_out.mineral += fed.mineral;
        fauna.ledger.stomach_out.energy += fed.energy;
        fauna.ledger.dispersed_by_founder[f.index()] += 1;
        let a = fauna.animals.remove(i);
        pack_changed |= a.pack.is_some();
        fauna.controllers.take(a.id);
        fauna.trapped.remove(&a.id);
        fauna.drop_plans.remove(&a.id);
        fauna.wading.remove(&a.id);
        fauna.flushes.remove(&a.id);
        fauna.free_riders(&a, Some(view));
    }
    if pack_changed {
        crate::pack::succession(fauna);
    }
}

/// **Hunger**: how far the reserve is short of its cap `reserve_cap · body`, from 0 (full)
/// to 1 (empty). A newborn's paid endowment above the cap reads as sated.
pub(crate) fn hunger(a: &Animal, sc: &SpeciesConfig) -> f64 {
    let cap = sc.reserve_of(a.body);
    if !(cap > 0.0) {
        return 0.0;
    }
    // The brood does not sate (D1b): a body saving for eggs is as hungry as the reserve
    // above its brood says.
    let fed = a.reserve - a.reproduction.brood_held(a.reserve);
    (1.0 - fed / cap).clamp(0.0, 1.0)
}

/// LEADS-PAY also binds a brain following sensed food or attempting to feed while
/// moving. It never classifies a brain's no-signal movement as heuristic search.
fn brain_seek(manifest: &crate::Manifest, obs: &[f64], answer: crate::Resolved, teachers: &crate::TeacherConfig) -> crate::Seek {
    let taste = manifest.modules.iter().find(|m| m.name == "Taste(1)");
    if taste.is_some_and(|m| obs[m.offset + 2] > 0.5 && obs[m.offset] > teachers.feed_gate) {
        return crate::Seek::Taste;
    }
    let moving = answer.actions.forward > 0.0 || answer.body.lift != 0.0;
    let food = manifest.modules.iter().any(|m| match m.name {
        name if name.starts_with("Chem(") || name == "Scent(nectar)" || name == "Scent(carrion)" => obs[m.offset] > 0.0,
        "Cone(3, foliage/body)" => (0..3).any(|ray| obs[m.offset + ray * 6 + 2] > 0.0),
        "Fan(3, bloom/foliage/body)" => (0..3).any(|ray| obs[m.offset + ray * 8 + 2] > 0.0),
        _ => false,
    });
    if moving && (answer.actions.feed > 0.0 || food) { crate::Seek::Lead } else { crate::Seek::Unknown }
}

/// One due body's read side: its manifest, pending sense writes and resolved response.
struct Sensed {
    manifest: crate::manifest::Manifest,
    writes: crate::senses::SenseWrites,
    actions: Actions,
    lure: f64,
    lift: f64,
    seek: crate::Seek,
    body_controlled: bool,
    driven: bool,
    /// The tap's observation sampled; `None` while the tap is off.
    tapped: Option<Vec<f64>>,
    /// Body actions are held even when no tap records.
    body: crate::controller::BodyActions,
    /// CONTRACTS: a menu brain's decision, and the decision sample's inputs (built for a
    /// menu lineage's brain, or while the tap records a menu body).
    decision: Option<crate::Decision>,
    prep: Option<crate::subjects::DecisionPrep>,
    /// BRAIN-NEXT: the observation a dry-run teacher reads beside this body ([`crate::dry`]).
    dry_obs: Option<Vec<f64>>,
}

/// The fewest due bodies one observation job takes.
const BODIES_PER_JOB: usize = 2;
/// Observation jobs per worker thread.
const JOBS_PER_WORKER: usize = 4;

/// Step 3: the **controller stage** — sample every founder whose controller period is
/// due, consume its prior-interval feedback into the observation, and hold the resolved
/// actions until the next sampling.
///
/// This is the tick's one sampling point, and it is shared: it runs inside the fauna
/// tick, after maintenance and age, before any motion or feeding, and every driver —
/// the live schedule, the static arena, an ES episode harness — reaches it through
/// [`Fauna::step`]. A post-tick sampler is not this stage. Every controller of the tick
/// reads the same pre-action state.
///
/// **Read side, then write side.** The read side — the occupancy windows, every due body's
/// observation and its controller's answer — reads only and runs on the compute pool
/// with `threads`; the write side — the senses' writes, the reflexes, the held actions —
/// runs after it, serially, in body order.
pub(crate) fn controllers(
    fauna: &mut Fauna,
    view: &VoxelView<'_>,
    flora: &Flora,
    mut senses: Option<&mut crate::Senses>,
    sky: &Sky,
    threads: usize,
    only: Option<&[u64]>,
) {
    #[cfg(feature = "profile")]
    let mut timer = cubarium_voxel::profile::start(cubarium_voxel::profile::Phase::FaunaWindows);
    // SENSES-CORE: the live lineages' remote senses, staggered by id, before any controller
    // of the tick reads a record.
    crate::perception::sense_pass(fauna, view, sky.clock_on, sky.daylight, threads);
    // CONTRACTS: the calls emitted since the last tick, heard by their own range, asleep too.
    crate::calls::hear(fauna, view);
    crate::places::refresh(fauna, view, &flora.view(), sky);
    // VH-TEACH: the den route fields the observations' `Reach.den` reads, current with the
    // route graphs as last built (no graph is built here).
    crate::pack::warm_den_paths(fauna);
    let due: Vec<usize> = fauna
        .animals
        .iter()
        .enumerate()
        .filter(|(_, a)| {
            let Some(founder) = a.founder else {
                return false;
            };
            // An asleep body's controller is not consulted.
            // WX4: a phase input for trained brains is the next training round's job; the
            // sensor manifest gains it there, not here.
            // A rider has no controller of its own while it rides (parasite §2).
            a.state != State::Asleep
                && a.attached.is_none()
                && a.age_ticks % founder.manifest().cadence_ticks() == 0
                // BRAIN-NEXT: a dry-run copy samples only its dry bodies.
                && only.is_none_or(|o| o.contains(&a.id))
        })
        .map(|(i, _)| i)
        .collect();
    if due.is_empty() {
        return;
    }
    // "Initial intake/loss/motion feedback is zero" (the plan's manifest rules).
    // The ticks before a body's *first* sampling are not an interval it acted in:
    // it held nothing, so there is nothing to report back about an action, and the
    // upkeep it paid while standing there is not feedback. Clear the accumulator
    // before the first observation is built.
    //
    // This was invisible until P2-C. A founder used to arrive with a full reserve,
    // so those ticks' maintenance came out of the reserve, `structural_loss` was
    // never incremented, and channel 3 read zero by accident. A hungry founder pays
    // upkeep out of its structure from tick one (P2-T finding 3).
    //
    // A body's feedback is read only by its own observation, so every due body's is
    // cleared here, before any observation.
    for &i in &due {
        let founder = fauna.animals[i].founder.expect("a due founder");
        if fauna.animals[i].age_ticks == fauna.config.manifest(founder).cadence_ticks() {
            fauna.animals[i].founder_state.feedback = IntervalFeedback::default();
        }
    }
    let fv = flora.view();
    let full_observation: Vec<bool> = fauna.animals.iter().map(|a| !fauna.controllers.taste_only(a.id)).collect();
    // Contract v3: which due bodies' v3 blocks are encoded — a controller that reads them
    // (a brain), or every body while the executed-action tap records. Encoding reads only;
    // the two caches its read side borrows (the beacons, the carcass faces) are keyed by
    // the plants' tick and the world's, so filling them here first changes nothing.
    let wants_tail: Vec<bool> = {
        let tap = fauna.tap.is_some();
        let mut w = vec![false; fauna.animals.len()];
        for &i in &due {
            w[i] = tap || fauna.controllers.reads_tail(fauna.animals[i].id);
        }
        w
    };
    let clock = crate::observe_v3::Clock { clock_on: sky.clock_on, daylight: sky.daylight, day_phase: fauna.day_phase };
    {
        let tailed = || due.iter().filter(|&&i| wants_tail[i]).filter_map(|&i| fauna.animals[i].founder);
        let light = tailed().any(|f| {
            let m = fauna.config.manifest(f);
            fauna.config.founder(f).senses.light_m > 0.0 || m.modules.iter().any(|m| m.name == "Scent(nectar)")
        });
        let carcass = tailed().any(|f| fauna.config.manifest(f).modules.iter().any(|m| m.name == "Carcass"));
        if light {
            fauna.beacons.refresh(view, &fv, fauna.config.scent.nectar_full);
        }
        if carcass {
            let _ = fauna.carcass_sites(&fv);
        }
    }
    let uses_cone = |i: usize| {
        fauna.animals[i].founder.is_some_and(|f| {
            fauna.config.founder(f).flight.is_none() || full_observation[i]
        })
    };
    // The occupancies every due body's rays read: one grid of stand classes and ground
    // pools held across stages and patched where something moved (SPEED-MATURE,
    // `senses::LiveGrid`), with the bodies laid in two flavours — the v2 cone's one cell
    // per body (every sighted forager's: a cone and no hunter's table) and the predation
    // occupancy's bodies with volume (predation §2.2: the lanternjaw's own sight and a
    // heuristic prey's look for a threat). A static episode's senses hold the v2 cone
    // instead (cone-speed item 4): built once, patched from bites and deaths, bodies
    // re-indexed — the same grid either way.
    let v2_cone: [bool; Founder::COUNT] = Founder::ALL
        .map(|f| f.manifest().cone_range_m > 0.0 && fauna.config.founder(f).active_hunter().is_none());
    let observers = due
        .iter()
        .any(|&i| uses_cone(i) && fauna.animals[i].founder.is_some_and(|f| v2_cone[f.index()]));
    // Every flyer reads the same pre-action flowers. Build their heads once per stage,
    // only while a controller consumes its fan, and lend them to the parallel readers.
    let blooms = due.iter().any(|&i| full_observation[i] && fauna.animals[i].founder
        .is_some_and(|f| fauna.config.founder(f).flight.is_some()))
        .then(|| crate::flight::Blooms::of(&fv));
    let mut held = senses.as_deref_mut().and_then(|s| s.take_cone());
    // The predation occupancy is read only while a hunter is in the world.
    let hunters_here = fauna
        .animals
        .iter()
        .any(|a| a.founder.is_some_and(|f| fauna.config.founder(f).active_hunter().is_some()));
    let predators = hunters_here
        && due
            .iter()
            .any(|&i| uses_cone(i) && fauna.animals[i].founder.is_some_and(|f| f.manifest().cone_range_m > 0.0));
    let mut live = std::mem::take(&mut fauna.grid_scratch);
    let flat = observers && held.is_none();
    if predators || flat {
        live.refresh(view, &fv, &fauna.view(), threads, (flat, predators));
    }
    let predation_occupancy =
        predators.then(|| crate::senses::Occ::Live(live.view(view, crate::senses::Flavour::Volume)));
    let cone_occupancy = if !observers {
        None
    } else if let Some(held) = held.as_mut() {
        Some(crate::senses::Occ::Grid(held.refresh(view, &fv, &fauna.view())))
    } else {
        Some(crate::senses::Occ::Live(live.view(view, crate::senses::Flavour::Flat)))
    };

    // **Read side**: every due body's observation and its controller's answer, side by
    // side on the compute pool. An observation reads the world, the plants, the bodies
    // and the senses as they stand before this stage writes anything; a controller reads
    // its observation and its own memory. Nothing here writes shared state: the senses'
    // writes come back as [`crate::senses::SenseWrites`], and the held actions are written
    // below, serially, in body order.
    #[cfg(feature = "profile")]
    timer.next(cubarium_voxel::profile::Phase::FaunaObserve);
    if let Some(s) = senses.as_deref_mut() {
        s.index_light(view);
    }
    let ids: Vec<u64> = due.iter().map(|&i| fauna.animals[i].id).collect();
    // CONTRACTS: a loaded brain's memory goes back to its controller before it samples.
    if !fauna.brain_memory.is_empty() {
        for &id in &ids {
            if fauna.controllers.has(id) {
                if let Some(m) = fauna.brain_memory.remove(&id) {
                    fauna.controllers.restore(id, &m);
                }
            }
        }
    }
    let tap = fauna.tap.is_some();
    let mut table = std::mem::take(&mut fauna.controllers);
    let sensed: Vec<Sensed> = {
        let mut slots = table.slots_mut(&ids);
        let fauna: &Fauna = fauna;
        let senses = senses.as_deref();
        let fv = &fv;
        let read = |i: usize, slot: &mut Option<&mut Box<dyn crate::Controller>>| -> Sensed {
            let founder = fauna.animals[i].founder.expect("a due founder");
            // YOUNG-ROAM: its resolved forward against its own cruise's reference.
            let manifest = fauna.config.body_manifest(founder, fauna.animals[i].body);
            let occupancy = if v2_cone[founder.index()] {
                cone_occupancy
            } else {
                predation_occupancy
            };
            // BRAIN-NEXT: a dry teacher reads the observation its body's brain read.
            let (obs, writes) = match fauna.dry.given(fauna.animals[i].id) {
                Some(o) => (o.to_vec(), crate::senses::SenseWrites::default()),
                None => body::observation_read(
                    fauna,
                    i,
                    view,
                    fv,
                    &manifest,
                    senses,
                    occupancy,
                    full_observation[i] || wants_tail[i],
                    blooms.as_ref(),
                    wants_tail[i].then_some(clock),
                ),
            };
            // Reject rather than train through: an observation with a non-finite channel
            // holds rest instead of reaching the controller at all.
            // A taste-only controller encoded in full for the tap sees what it sees untapped:
            // its taste, everything else zero.
            let masked;
            let seen: &[f64] = if full_observation[i] || !wants_tail[i] {
                &obs
            } else {
                let tm = manifest.modules.iter().find(|m| m.name == "Taste(1)");
                masked = obs
                    .iter()
                    .enumerate()
                    .map(|(k, &v)| if tm.is_some_and(|m| k >= m.offset && k <= m.end()) { v } else { 0.0 })
                    .collect::<Vec<f64>>();
                &masked
            };
            // CONTRACTS: a menu lineage's brain decides with its intent mask and candidates; a
            // plain lineage (and every heuristic) is driven exactly as before.
            let menu_body = !manifest.menu.plain();
            let mut decision = None;
            let mut prep = None;
            let mut dry_obs = None;
            let (resolved, seek, body_controlled, driven) = match slot.as_deref_mut() {
                Some(c) if obs.iter().all(|v| v.is_finite()) => {
                    let brain = c.layer() == crate::Layer::Brain;
                    if fauna.dry.wants(founder, c.layer()) {
                        dry_obs = Some(obs.clone());
                    }
                    if menu_body && (brain || tap) {
                        #[cfg(feature = "profile")]
                        let _intent = cubarium_voxel::profile::start(cubarium_voxel::profile::Phase::FaunaIntent);
                        prep = Some(crate::subjects::decision_prep(fauna, i, founder));
                    }
                    let response = match prep.as_ref() {
                        Some(p) if brain => {
                            let (r, d) = c.decide(seen, &p.input());
                            decision = d;
                            r
                        }
                        _ => c.drive(seen),
                    };
                    {
                        let body_controlled = match response {
                            crate::Response::Body(_, _) => true,
                            crate::Response::Logits(l) => l[3..manifest.actions.len()].iter().any(|&v| v != f64::MIN),
                            crate::Response::Bounded(_) => false,
                        };
                        let resolved = crate::controller::resolve(response, &manifest);
                        let seek = if c.layer() == crate::Layer::Heuristic {
                            c.seek()
                        } else {
                            brain_seek(&manifest, &obs, resolved, &fauna.config.teachers)
                        };
                        (resolved, seek, body_controlled, true)
                    }
                }
                _ => (crate::controller::Resolved::default(), crate::Seek::Unknown, false, false),
            };
            let tapped = tap.then_some(obs);
            Sensed { manifest, writes, actions: resolved.actions, lure: resolved.body.lure, lift: resolved.body.lift, seek, body_controlled, driven, tapped, body: resolved.body, decision, prep, dry_obs }
        };
        // Several jobs per worker: one body's observation can cost many another's (a
        // flyer's fan), and the pool evens that out as workers come free.
        let ranges = crate::par::ranges(due.len(), threads * JOBS_PER_WORKER, BODIES_PER_JOB);
        if ranges.len() <= 1 {
            due.iter().zip(slots.iter_mut()).map(|(&i, slot)| read(i, slot)).collect()
        } else {
            let read = &read;
            let mut jobs: Vec<crate::par::Job<'_, Vec<Sensed>>> = Vec::new();
            let mut rest: &mut [Option<&mut Box<dyn crate::Controller>>] = &mut slots;
            for r in ranges {
                let (mine, tail) = std::mem::take(&mut rest).split_at_mut(r.len());
                rest = tail;
                let bodies = &due[r];
                jobs.push(Box::new(move || {
                    bodies.iter().zip(mine.iter_mut()).map(|(&i, slot)| read(i, slot)).collect()
                }));
            }
            crate::par::run(threads, jobs).into_iter().flatten().collect()
        }
    };
    fauna.controllers = table;
    if let Some(s) = senses.as_deref_mut() {
        for got in &sensed {
            got.writes.apply(s, view);
        }
    }
    {
        let dry: Vec<(u64, Vec<f64>)> = due
            .iter()
            .zip(&sensed)
            .filter_map(|(&i, s)| s.dry_obs.clone().map(|o| (fauna.animals[i].id, o)))
            .collect();
        if !dry.is_empty() {
            fauna.dry.capture(dry);
        }
    }

    // **Write side**, serially in body order: the reflexes and the held actions.
    #[cfg(feature = "profile")]
    timer.next(cubarium_voxel::profile::Phase::FaunaReflex);
    for (k, &i) in due.iter().enumerate() {
        let id = fauna.animals[i].id;
        let founder = fauna.animals[i].founder.expect("a due founder");
        let Sensed { ref manifest, actions, lure, lift, driven, .. } = sensed[k];
        let (mut actions, mut lure, mut lift) = (actions, lure, lift);
        let own = (actions, lure, lift);
        let heuristic = fauna.controllers.layer(id) == crate::Layer::Heuristic;
        let flyer = heuristic && fauna.config.founder(founder).flight.is_some();
        let prey = heuristic && !flyer && !matches!(founder, crate::Founder::Lanternjaw | crate::Founder::Chorister);
        let forager = prey || flyer;
        let patrol = heuristic && founder == crate::Founder::Lanternjaw;
        let pack = heuristic && founder == crate::Founder::Chorister;
        // EDGE's readout: which layer sets the held actions, the last override winning.
        let mut src = if driven { crate::Steer::Heuristic } else { crate::Steer::Rest };
        // EDGE: the interval just ended pressed (held forward, its motor refused), on the
        // ground: the run of pressed ticks the reflexes below re-plan by.
        {
            let a = &fauna.animals[i];
            let fb = a.founder_state.feedback;
            let pressed = a.mobility.mode == crate::MobilityMode::Ground
                && a.founder_state.held.forward > 0.0
                && fb.attempted_equivalent > 1e-12
                && fb.delivered_equivalent / fb.attempted_equivalent < fauna.config.teachers.motor_refused;
            // A fruit approach can stall without asking forward (its aim coincides
            // with an unreachable wall centre). Count no displacement too, so that
            // the same target/shun clock covers this path as well as a refused walk.
            let fruit_stalled = heuristic && fauna.config.founder(founder).glider.is_some()
                && a.mobility.mode == crate::MobilityMode::Ground
                && matches!(fauna.steer.get(&id), Some(crate::Steer::Fruit | crate::Steer::Check))
                && fb.delivered_forward <= 1e-12 && fb.delivered_turn.abs() <= 1e-12
                // ECO-INT: the stillness of a carcass meal is not a stalled walk.
                && !(crate::perception::is_day(&fauna.config, sky.clock_on, sky.daylight)
                    && crate::hunt::eating_carrion_here(fauna, i, view, &fv));
            let vine_stalled = heuristic && fauna.config.founder(founder).glider.is_some()
                && matches!(a.mobility.mode, crate::MobilityMode::Climb | crate::MobilityMode::Perched)
                && a.mobility.perch.is_some_and(|p| matches!(p.anchor, crate::PerchAnchor::Vine(_)))
                && a.eating && a.strike.is_none()
                && fb.delivered_forward <= 1e-12
                && glider::glider_fruit_in_reach(view, &fv, a, fauna.config.founder(founder)).is_none();
            if pressed || fruit_stalled || vine_stalled {
                *fauna.pressed.entry(id).or_default() += manifest.cadence_ticks().max(1);
            } else {
                fauna.pressed.remove(&id);
            }
        }
        // HUNGRY-SEARCH (`rove`): a heuristic forager wanting food, `search_patience_s`
        // after its last taste of food, ranges on straight legs away from ground it has
        // covered wherever its own answer is its no-signal search. First, so every layer
        // below still wins; a layer that overrides a leg's steering ends it (below).
        // Brains share only LEADS-PAY here: brain_seek never requests no-signal search.
        let mut searched = None;
        // CONTRACTS: the herd pull bent its walk (the intent record's label).
        let mut herded = false;
        if driven && (forager || !heuristic) {
            let seek = sensed[k].seek;
            let ring = f64::from(view.config.width) * view.config.voxel_m;
            let period_s = manifest.cadence_ticks().max(1) as f64 * DT;
            if let Some(go) = rove::search(fauna, i, actions, seek, period_s, ring) {
                actions = go;
                src = crate::Steer::Search;
                searched = Some(go);
            }
        }
        if driven && !heuristic {
            let body = sensed[k].body;
            let top = crate::gait::sprint_forward(&fauna.config, founder);
            if fauna.animals[i].mobility.mode == crate::MobilityMode::Ground && fauna.animals[i].mobility.climb.is_none() {
                actions.forward *= 1.0 + body.sprint * (top - 1.0);
            }
            // A three-action diagnostic leaves a manually placed body pose alone.
            // Explicit body responses (including zero) and full brain logits own it.
            if sensed[k].body_controlled && crate::vigil::hunker_action(fauna, i, body.hunker) {
                actions = Actions::REST;
            }
            if body.retract > 0.0 {
                retract_action(fauna, i);
            }
        }
        // The prey heuristics' flee rule (predation §2.7): a body driven by its own
        // heuristic — not a saved policy, whose behaviour changes only through the world
        // — turns away from a seen threat within 3 m, or from tremor, and resumes after.
        // Only while a hunter is in the world: a teacher in a world with none behaves as
        // it always did.
        // A glider's fruit search (roster mechanics fix 4): hungry on the ground with no
        // fruit at its mouth, its heuristic gives way to a walk to the nearest fruiting
        // stand in reach. Flight from a threat and the roost come after it and win.
        if driven && prey && fauna.config.founder(founder).glider.is_some() {
            let day = crate::perception::is_day(&fauna.config, sky.clock_on, sky.daylight);
            if let Some((go, kind)) = glider::fruit_reflex(fauna, i, view, &fv, day) {
                actions = go;
                src = kind;
            }
        }
        // FORAGE-LEAVE: a heuristic body holding still to feed on a patch that cannot end a
        // meal gives it up and walks on (`forage`). Flight and the reflexes below still win.
        if driven && forager {
            let ring = f64::from(view.config.width) * view.config.voxel_m;
            if let Some(go) = forage::leave(fauna, i, actions, ring) {
                actions = go;
                src = crate::Steer::Leave;
            }
        }
        // HERD-PULL (`crate::herding`): a heuristic body of a herding lineage, walking on its
        // own steering, a search leg or off a spent patch, with its nearest known kin beyond
        // its comfort, leans its turn back toward them. Its pace and feeding stay its own;
        // flight and every reflex below still win. A bent search leg is still the leg.
        if driven
            && prey
            && matches!(src, crate::Steer::Heuristic | crate::Steer::Search | crate::Steer::Leave)
        {
            let ring = f64::from(view.config.width) * view.config.voxel_m;
            let seek = fauna.controllers.seek(id);
            if let Some(go) = crate::herding::steer(fauna, i, actions, seek, ring) {
                actions = go;
                herded = true;
                if src == crate::Steer::Search {
                    searched = Some(go);
                }
            }
        }
        // YOUNG-ROAM (`crate::roam`): a leashed young's excursion, over its own steering and the
        // herd pull; else a mother's walk to her own young's distress call. Flight, the defence
        // layer and every reflex below still win.
        if driven && prey {
            let ring = f64::from(view.config.width) * view.config.voxel_m;
            let m = fauna.animals[i].mobility;
            let calm = matches!(src, crate::Steer::Heuristic | crate::Steer::Search | crate::Steer::Leave)
                && m.mode == crate::MobilityMode::Ground
                && m.climb.is_none();
            let period_s = manifest.cadence_ticks().max(1) as f64 * DT;
            if let Some(go) = crate::roam::excursion(fauna, i, calm, period_s, view.config.seed, ring) {
                actions = go;
                src = crate::Steer::Explore;
                herded = false;
            } else if calm {
                if let Some((go, _)) = crate::roam::answer(fauna, i, ring) {
                    actions = go;
                    src = crate::Steer::Answer;
                    herded = false;
                }
            }
        }
        // AMPHIBIOUS-SNAIL: a body that retracts (its lineage's `retract_hold_s`) does so
        // instead of fleeing: a threat it sees or feels stops and closes it.
        let retracts = fauna.config.founder(founder).amphibious.is_some_and(|am| am.retract_hold_s > 0.0);
        if driven && hunters_here && retracts && prey {
            retract(fauna, i);
        }
        // SHREDDER-NEST: a layer the crowd refused walks to the free egg face it chose, over
        // its forager, its search and the herd; flight, deep water and thirst (below) win.
        if driven && prey {
            if let Some(go) = nest_seek_reflex(fauna, i, view, &fv) {
                actions = go;
                lure = 0.0;
                lift = 0.0;
                src = crate::Steer::Nest;
            }
        }
        let mut fled = false;
        // LS-DEFEND: the defence layer's aimed kick, started below once its steering stands.
        let mut defend_kick = None;
        fauna.ls_teach.remove(&id);
        fauna.call_answer.remove(&id);
        if driven && hunters_here && !retracts && prey {
            if let Some(flee) = flee(fauna, i, view, &fv, predation_occupancy, actions) {
                actions = flee;
                lure = 0.0;
                lift = 0.0;
                fled = true;
                src = crate::Steer::Flee;
            }
            // LS-DEFEND: the loftstrider's defence layer over its ground response, while it
            // holds a defence intent (watch, move off, go to adults or calf, guard).
            if crate::defend::on(&fauna.config, founder) {
                if let Some((go, kick)) = crate::defend::steer(fauna, i, view) {
                    defend_kick = kick;
                    actions = go;
                    lure = 0.0;
                    lift = 0.0;
                    fled = true;
                    src = crate::Steer::Defend;
                }
            }
        }
        // CALLS-REST step 5: about to rest, a body smelling hunters walks down their scent to
        // a bed (`sleep::bed_reflex`). Flight wins.
        if driven && prey && !fled {
            if let Some(go) = crate::sleep::bed_reflex(fauna, i, view) {
                actions = go;
                lure = 0.0;
                lift = 0.0;
                src = crate::Steer::Bed;
            }
        }
        // A glider's roost reflex (F3): at its rest hour or in a downpour, its own
        // heuristic gives way to a walk to the nearest crown it can climb.
        if driven && prey && fauna.config.founder(founder).glider.is_some() {
            if let Some(roost) = glider::roost_reflex(fauna, i, view, &fv, sky) {
                actions = roost;
                src = crate::Steer::Roost;
            }
        }
        // SP-FOLLOW (`crate::nurse::mother_goal`): a dependent young goes to its mother as to
        // food, over its own heuristic, the fruit walk, the herd and the roost (it roosts where
        // she does), and holds still beside her to nurse; flight wins, as do the reflexes
        // below (deep water, thirst, a trapped body's way out), but not its catch.
        if driven && prey && !fled && fauna.config.founder(founder).glider.is_some() {
            let go = match crate::nurse::mother_goal(fauna, i, view) {
                Some(crate::nurse::MotherGoal::Hold) => Some(Actions { forward: 0.0, turn: 0.0, feed: 1.0 }),
                Some(crate::nurse::MotherGoal::Go { x, z, y_m, lift_m, site }) => glider::to_mother(fauna, i, view, &fv, x, z, y_m, lift_m, site),
                None => None,
            };
            if let Some(go) = go {
                actions = go;
                lure = 0.0;
                lift = 0.0;
                src = crate::Steer::Nurse;
            }
        }
        // Fed perching (the minimal gut's repair 3 item 8): a body whose controller rests
        // when fed (the bellwing's hover-sip teacher) and whose eating latch is off holds
        // still on its perch, or comes down onto what is below it, read from the latch.
        if driven && flyer && !fauna.animals[i].eating {
            actions = Actions { forward: 0.0, turn: 0.0, feed: 0.0 };
            lure = 0.0;
            src = crate::Steer::FedRest;
            lift = if fauna.animals[i].mobility.mode == crate::MobilityMode::Perched {
                0.0
            } else {
                -1.0
            };
        }
        // The shallows reflex (roster mechanics fix 2): a body in water deeper than it wades
        // heads for the nearest face it wades, over everything its heuristic asked.
        if driven && prey {
            if let Some(out) = shallows_reflex(fauna, i, view) {
                actions = out;
                lure = 0.0;
                lift = 0.0;
                src = crate::Steer::Shallows;
            }
        }
        // SNAIL-FIX: the roam drive. At night and in or after rain, an amphibious body
        // that is not thirsty and not feeding where it stands walks out onto wet ground,
        // to land film where there is any.
        if driven && prey {
            if let Some((go, key)) = roam_reflex(fauna, i, view, &fv, sky, actions) {
                actions = go;
                lure = 0.0;
                lift = 0.0;
                src = crate::Steer::Roam;
                amphibious_walk(fauna, id, key, src);
            }
        }
        // SNAIL-LAND-2: the giving-up density. A grazer wanting food on a grazed face (its
        // edible film under `graze_give_up` of a site's capacity) walks to a richer face in
        // reach.
        if driven && prey {
            if let Some((go, key)) = graze_reflex(fauna, i, view, &fv, sky) {
                actions = go;
                lure = 0.0;
                lift = 0.0;
                amphibious_walk(fauna, id, key, src);
            }
        }
        // SNAIL-LAND: a ready layer whose eggs go on the shore walks to a shore egg face
        // its drying clock can afford, and outside its roam hours one ashore that is not
        // ready goes back to the water. Thirst (below) still wins.
        if driven && prey {
            if let Some((go, key)) = shore_reflex(fauna, i, view, &fv, sky) {
                actions = go;
                lure = 0.0;
                lift = 0.0;
                amphibious_walk(fauna, id, key, src);
            }
        }
        // AMPHIBIOUS-SNAIL: a thirsty body out of water heads for the nearest water.
        if driven && prey {
            if let Some((go, key)) = water_reflex(fauna, i, view, &fv) {
                actions = go;
                src = crate::Steer::Roam;
                amphibious_walk(fauna, id, key, src);
            }
        }
        // A retracted body is stopped and closed; a desiccated one cannot feed.
        {
            let m = fauna.animals[i].mobility;
            if m.retract_ticks > 0 {
                actions = Actions { forward: 0.0, turn: 0.0, feed: 0.0 };
            }
            if fauna.config.founder(founder).amphibious.is_some_and(|am| am.on_land) && m.dryness >= 1.0 {
                actions.feed = 0.0;
            }
        }
        // The seedporter's catch: its carrion meal, a prey in reach, a foray. SP-FOLLOW-2: not
        // over a dependent young's walk to its mother or its nursing hold; it ran after them,
        // left the readout at Nurse, and held young eating at a carcass 20 m from their mothers.
        if driven
            && !fled
            && prey
            && src != crate::Steer::Nurse
            && founder == crate::Founder::Seedporter
            && (!sky.clock_on || sky.daylight >= fauna.config.life.day_line)
        {
            if let Some(go) = crate::hunt::catch_step(fauna, i, view, &fv) {
                actions = go;
                lure = 0.0;
            }
        }
        // The detritus web D2: the lanternjaw's patrol (its heuristic only, while the
        // world's `hunt.patrol` is on). The carcass yield below still wins.
        if driven && patrol && !fauna.controllers.at_carcass(id) {
            if let Some((go, lit)) =
                crate::hunt::steer(fauna, i, view, &fv, predation_occupancy)
            {
                actions = go;
                src = crate::Steer::Patrol;
                // S3 (D1): the lure is lit only where it is dark — the light at its face,
                // `sky × daylight`, under the day line (at night, in caves, in deep shade).
                lure = if lit > 0.0 && sky.clock_on {
                    let a = &fauna.animals[i];
                    let v = view.config.voxel_m;
                    let face_sky = match a.pose.column(v, view.config.depth) {
                        Some((cx, cz)) => {
                            let wx = cx.rem_euclid(i64::from(view.config.width));
                            let y = a.sense_layer(v);
                            fauna.face_sky(view, wx, y, cz)
                        }
                        None => 1.0,
                    };
                    if fauna.config.founder(founder).lure_daylight
                        || face_sky * sky.daylight < fauna.config.life.day_line { lit } else { 0.0 }
                } else {
                    lit
                };
            }
        }
        // R3: the lanternjaw's carcass yield (its heuristic only), and the chorister's
        // scripted pack tactic.
        if driven && heuristic && hunters_here {
            let at_carcass = fauna.controllers.at_carcass(id);
            if at_carcass || fauna.animals[i].yielding {
                let ring = f64::from(view.config.width) * view.config.voxel_m;
                if let Some(away) = crate::pack::carcass_yield(fauna, i, ring, at_carcass) {
                    actions = away;
                    lure = 0.0;
                    src = crate::Steer::Yield;
                }
            }
        }
        // LJ-HOSTSWITCH: an outgrown rider looking for a new host walks to the nearest one
        // in reach that bears it, over its patrol (its own heuristic only).
        if driven && patrol {
            if let Some(go) = host_seek(fauna, i, view) {
                actions = go;
                lure = 0.0;
                src = crate::Steer::Host;
            }
        }
        if driven && pack {
            if let Some(tactic) =
                crate::pack::steer(fauna, i, view, &fv, sky.clock_on, sky.daylight, actions, predation_occupancy)
            {
                actions = tactic;
                lure = 0.0;
                src = crate::Steer::Pack;
            }
        }
        // VINE-C: a flyer nesting in the vines, ready to lay, holds at a vine nest site or flies
        // to one it can find (over its own heuristic only, the hover-sip teacher); the light
        // below still turns it from a looming shadow and a hungry one to food.
        if driven && !fled && flyer {
            if let Some((go, up)) = nest_reflex(fauna, i, view, flora) {
                actions = go;
                lift = up;
                lure = 0.0;
                src = crate::Steer::Nest;
            }
        }
        // S3 LIGHT AND FLIGHT (`crate::light`): over a live lineage's heuristic with a light
        // sense — the bellwing's looming, light and nectar smell over its hover-sip (the fed
        // perching above stands but for a looming shadow), the capgnawer's cyan over its
        // forager when hungry with no fungal scent near. Flight from a threat wins. Over its
        // own heuristic only (the hover-sip teacher, which rests when fed; a forager, which
        // flees): a saved policy or a scripted body behaves only through the world.
        if driven && !fled {
            let phys = *fauna.config.founder(founder);
            let table = phys.senses;
            let light_heuristic = if phys.flight.is_some() { flyer } else { prey };
            if light_heuristic && table.live && (table.light_m > 0.0 || table.loom_m > 0.0) {
                let daylight = if sky.clock_on { sky.daylight } else { 1.0 };
                if phys.flight.is_some() {
                    if let Some((go, up)) = crate::light::steer_flyer(fauna, i, view, &fv, daylight, actions, lift) {
                        actions = go;
                        lift = up;
                        lure = 0.0;
                        src = crate::Steer::Light;
                    }
                } else {
                    let fungal = match (senses.as_deref(), manifest.modules.iter().any(|m| m.name == "Chem(fungal)")) {
                        (Some(s), true) => {
                            let a = &fauna.animals[i];
                            s.sample_fungal(view, &a.pose, a.sense_layer(view.config.voxel_m)).map(|c| {
                                let sat = manifest.tunings.chem_saturation;
                                if c + sat > 0.0 { c / (c + sat) } else { 0.0 }
                            })
                        }
                        _ => None,
                    };
                    if let Some(go) = crate::light::steer_glow(fauna, i, view, &fv, daylight, actions, fungal) {
                        actions = go;
                        src = crate::Steer::Light;
                    }
                }
            }
        }
        // The stay-on-food readout: a heuristic body holding still to feed.
        if driven && prey && actions.feed > 0.5 && actions.forward <= fauna.config.teachers.forage_stay_forward {
            fauna.ledger.feed_still_ticks_by_founder[founder.index()] += 1;
        }
        // TWO-LAYER staged scenes (`Fauna::force_own`): a forced body holds its controller's
        // own answer over every heuristic-layer override above; the physical ways out below
        // (DROP, WADE-OUT) and the shallows still win.
        let forced = driven && !fauna.forced_own.is_empty() && fauna.forced_own.contains(&id) && src != crate::Steer::Shallows;
        if forced {
            (actions, lure, lift) = own;
            src = crate::Steer::Heuristic;
            fled = false;
            herded = false;
            fauna.ls_teach.remove(&id);
        }
        // DROP: a body trapped off its route graph (no walking way back on, no glide) for its
        // lineage's `drop.after_s` jumps down; last, as it is the only way out.
        if driven {
            if let Some(go) = jump::reflex(fauna, i, view, &fv) {
                actions = go;
                lure = 0.0;
                lift = 0.0;
                src = crate::Steer::Drop;
            } else if let Some(go) = jump::wade_reflex(fauna, i, view) {
                // WADE-OUT: held in water it may not wade, or on a pocket of its graph, it
                // walks out through water short of its drown depth; as DROP, a last way out.
                actions = go;
                lure = 0.0;
                lift = 0.0;
                src = crate::Steer::Wade;
            }
        }
        if lure > 0.0 && sky.clock_on && !fauna.config.founder(founder).lure_daylight {
            let a = &fauna.animals[i];
            let v = view.config.voxel_m;
            let face_sky = a.pose.column(v, view.config.depth).map_or(1.0, |(cx, cz)| {
                view.sky_visibility(
                    cx.rem_euclid(i64::from(view.config.width)),
                    a.sense_layer(v),
                    cz,
                )
            });
            if face_sky * sky.daylight >= fauna.config.life.day_line {
                lure = 0.0;
            }
        }
        if searched.is_some_and(|go| (go.forward, go.turn) != (actions.forward, actions.turn)) {
            rove::overridden(fauna, id);
        }
        // SP-GROOM: an available body approaches infested kin through its normal routes.
        // Flight, a strike, and emergency feeding retain priority; escape routes also win, as
        // does a dependent young's walk to its mother (SP-FOLLOW).
        if driven && !fled && !forced && !matches!(src, crate::Steer::Drop | crate::Steer::Wade | crate::Steer::Nurse) {
            let day = crate::perception::is_day(&fauna.config, sky.clock_on, sky.daylight);
            if let Some(go) = crate::groom::steer(fauna, i, view, &fv, day) {
                actions = go;
                lure = 0.0;
                lift = 0.0;
                src = crate::Steer::Groom;
            }
        }
        // RHYTHMS-CADENCE: stop and go. A walker or climber searching or foraging on its own
        // steering moves in bouts with pauses to look and listen; flight and every directed
        // reflex above keep their pace. After the search leg's check, so a pause does not end
        // the leg.
        if driven && !fled && (prey || pack) {
            let period = manifest.cadence_ticks().max(1);
            actions = pace(fauna, i, view.config.seed, src, actions, period);
        }
        if let (Some(t), crate::Steer::Defend) = (defend_kick, src) {
            crate::defend::kick_now(fauna, i, t);
        }
        fauna.steer.insert(id, src);
        // EDGE: a reflex walk overridden by a later layer is not the one held.
        fauna.walk_commit(id, founder, src);
        // CONTRACTS: the intent record (records only; no rule reads it).
        if driven {
            let held_before = fauna.animals[i].intent;
            match (sensed[k].decision, sensed[k].prep.as_ref()) {
                (Some(d), Some(prep)) => {
                    // The discrete actions: a rest request for the circadian stage, and a
                    // kick at the subject (or the nearest threat it senses) when ready.
                    if d.rest {
                        fauna.rest_asks.insert(id, crate::RestAsk::Rest);
                    } else {
                        fauna.rest_asks.remove(&id);
                    }
                    let kicked = d.kick && {
                        let target = d
                            .subject
                            .and_then(|r| prep.candidates.rows.get(r))
                            .and_then(|c| c.subject.body())
                            .or_else(|| fauna.percepts.get(&id).map(|p| p.best[usize::from(crate::perception::class::THREAT)]).filter(|t| t.known).map(|t| t.id));
                        target.is_some_and(|t| crate::kick::request(fauna, i, t, crate::kick::Source::Aimed).is_ok())
                    };
                    // A call kind (none, or the lineage's kind `call − 1`) into the substrate.
                    let called = d.call > 0 && crate::calls::emit(fauna, i, usize::from(d.call) - 1);
                    let executed = crate::Executed {
                        actions,
                        kick: kicked,
                        call: if called { d.call } else { 0 },
                        rest: if d.rest { crate::RestAsk::Rest } else { crate::RestAsk::Default },
                    };
                    crate::intent::record_brain(fauna, i, src, &d, prep, executed);
                }
                _ => {
                    #[cfg(feature = "profile")]
                    let _intent = cubarium_voxel::profile::start(cubarium_voxel::profile::Phase::FaunaIntent);
                    let teach = if heuristic {
                        crate::intent::teacher_label(fauna, i, founder, src, actions, herded, fauna.controllers.at_carcass(id))
                    } else {
                        crate::intent::Teach::default()
                    };
                    let valid = match sensed[k].prep.as_ref() {
                        Some(p) => p.intent_mask.clone(),
                        None => crate::intent::body_mask(fauna, i, founder),
                    };
                    let executed = crate::Executed { actions, ..crate::Executed::default() };
                    crate::intent::record(fauna, i, founder, src, teach, &valid, executed);
                }
            }
            // LS-DEFEND: the stance sight shows (§2.1): guard while the guard intent is held —
            // a brain's held intent, or its defence layer's (never read off the record).
            let guard = crate::intent::menu_of(founder).index("guard");
            let guarding = match sensed[k].decision {
                Some(_) => guard.is_some_and(|g| usize::from(fauna.animals[i].intent.held) == g),
                None => fauna.ls_teach.get(&id).and_then(|t| t.held) == Some("guard"),
            };
            fauna.animals[i].stance = if guarding { crate::perception::stance::GUARD } else { crate::perception::stance::NONE };
            // The decision tap: the sample's inputs, the label before the hold and what was
            // held, for cloning (§10.2, §11.3).
            if let (Some(prep), true) = (sensed[k].prep.as_ref(), fauna.tap.is_some()) {
                let rec = fauna.animals[i].intent;
                let sample = crate::TapDecision {
                    tick: fauna.tick,
                    prep: prep.clone(),
                    held_before: held_before.held,
                    proposal: rec.proposal,
                    proposal_row: prep.candidates.row_of(rec.proposal_subject),
                    held: rec.held,
                    held_row: prep.candidates.row_of(rec.subject),
                    executed: rec.executed,
                    overridden: rec.overridden,
                };
                if let Some(t) = fauna.tap.as_mut() {
                    t.push_decision(id, founder, sample);
                }
            }
        }
        let a = &mut fauna.animals[i];
        a.founder_state.held = actions;
        a.founder_state.lure = if lure.is_finite() { lure.clamp(0.0, 1.0) } else { 0.0 };
        // The lift (F1): clamped and deadbanded as the turn is.
        a.founder_state.lift = if lift.is_finite() && lift.abs() >= manifest.deadband {
            lift.clamp(-1.0, 1.0)
        } else {
            0.0
        };
        a.founder_state.feedback = IntervalFeedback::default();
        // P0, the executed-action tap: the observation, its controller's own answer and
        // what was held, after every layer above.
        if let Some(obs) = sensed[k].tapped.clone() {
            let a = &fauna.animals[i];
            let clean_lure = |l: f64| if l.is_finite() { l.clamp(0.0, 1.0) } else { 0.0 };
            let clean_lift = |l: f64| if l.is_finite() && l.abs() >= manifest.deadband { l.clamp(-1.0, 1.0) } else { 0.0 };
            let own = crate::Held {
                actions: own.0,
                body: crate::controller::BodyActions { lure: clean_lure(own.1), lift: clean_lift(own.2), ..sensed[k].body },
            };
            let executed = crate::Held {
                actions: a.founder_state.held,
                body: crate::controller::BodyActions {
                    sprint: 0.0,
                    hunker: f64::from(a.hunkered),
                    retract: f64::from(a.mobility.retract_ticks > 0),
                    lure: a.founder_state.lure,
                    lift: a.founder_state.lift,
                },
            };
            let sample = crate::TapSample { tick: fauna.tick, observation: obs, own, executed, steer: src };
            if let Some(t) = fauna.tap.as_mut() {
                t.push(id, founder, sample);
            }
        }
    }
    if let Some(s) = senses {
        s.return_cone(held);
    }
    fauna.grid_scratch = live;
}

/// **The stop-and-go pace** ([`crate::Cadence`]) over a heuristic body's held `actions`,
/// counted for `period` ticks (its controller interval): only on its own steering, a search
/// leg or off a spent patch (`src`), on the ground or a wall (not in the air), and moving —
/// faster than a stay on food (the teachers' `forage_stay_forward`), so a grazer browsing on
/// the go is paced and one holding still to feed is not; paused, it stands (no step, no turn,
/// its feeding as asked), else its forward is the bout's share.
pub(crate) fn pace(fauna: &mut Fauna, i: usize, seed: u64, src: crate::Steer, actions: Actions, period: u64) -> Actions {
    let a = &fauna.animals[i];
    let Some(founder) = a.founder else {
        return actions;
    };
    let cadence = fauna.config.founder(founder).cadence;
    let own = matches!(src, crate::Steer::Heuristic | crate::Steer::Search | crate::Steer::Leave);
    let footed = matches!(a.mobility.mode, crate::MobilityMode::Ground | crate::MobilityMode::Wall | crate::MobilityMode::Climb);
    let moving = actions.forward > fauna.config.teachers.forage_stay_forward.max(0.0);
    if !cadence.on() || !own || !footed || !moving {
        return actions;
    }
    let p = cadence.at(seed, a.id, fauna.tick);
    let l = &mut fauna.ledger;
    l.paced_ticks_by_founder[founder.index()] += period;
    if p.paused {
        l.paused_ticks_by_founder[founder.index()] += period;
        return Actions { forward: 0.0, turn: 0.0, feed: actions.feed };
    }
    Actions { forward: actions.forward * p.speed, ..actions }
}

/// **The flee rule** of the prey heuristics (predation §2.7), `None` when there is
/// nothing to flee, read from the body's record ([`crate::perception`]): the grazer turns
/// away from the nearest threat it sees within `flee_sight_m` ahead of it (by the interim
/// rule: [`crate::kind_of`], so a still hunter beyond 0.5 m is not one, and not hidden by
/// terrain or foliage); the blind shredder turns away from tremor above
/// [`crate::PredationConfig::flee_tremor`] (with a live record, its hunters' footfall
/// only: [`crate::vigil::threat_tremor`]). Full forward, no feeding.
///
/// **S2**: a lineage whose table is live and that has a ground response
/// ([`crate::Vigilance::mode`]) runs that instead ([`crate::vigil::respond`]), on its record
/// alone; `held` is what its heuristic asked this sample.
fn flee(
    fauna: &mut Fauna,
    i: usize,
    view: &VoxelView<'_>,
    fv: &cubarium_voxel_flora::FloraView<'_>,
    occupancy: Option<crate::senses::Occ<'_>>,
    held: Actions,
) -> Option<Actions> {
    let a = fauna.animals[i];
    let founder = a.founder?;
    let config = &fauna.config;
    let hunted = Founder::ALL
        .iter()
        .any(|&f| config.founder(f).active_hunter().is_some_and(|h| h.hunts(founder)));
    if !hunted {
        return None;
    }
    let phys = config.founder(founder);
    if phys.senses.live && phys.vigilance.mode != crate::VigilMode::None {
        let want = crate::perception::Want { tremor: phys.vigilance.mode == crate::VigilMode::Freeze, ..Default::default() };
        crate::perception::refresh(fauna, i, view, fv, occupancy, want);
        return crate::vigil::respond(fauna, i, view, fv, held);
    }
    let away = |turn: f64| Actions {
        forward: 1.0,
        turn,
        feed: 0.0,
    };
    let blind = founder.manifest().cone_range_m <= 0.0;
    if !blind && occupancy.is_none() {
        return None;
    }
    let want = crate::perception::Want { threats: !blind, tremor: blind, ..Default::default() };
    crate::perception::refresh(fauna, i, view, fv, occupancy, want);
    let p = crate::perception::percept(fauna, a.id)?;
    let ring = f64::from(view.config.width) * view.config.voxel_m;
    if blind {
        // FROZEN-PREY: a live record counts its hunters' footfall only (the capgnawer fled a
        // grazer's step, 0.6 of its awake time); without one, the S1 reading of every step.
        let [left, right] = if fauna.config.founder(founder).senses.live {
            crate::vigil::threat_tremor(&fauna.config, &a, p, fauna.tick, ring)
        } else {
            p.tremor
        };
        if left + right < fauna.config.predation.flee_tremor {
            return None;
        }
        return Some(away(if left > right { 1.0 } else { -1.0 }));
    }
    let (fx, fz) = a.pose.forward();
    let (rx, rz) = (fz, -fx);
    let mut nearest: Option<(f64, f64)> = None;
    for t in p.tracks().iter().filter(|t| t.class == crate::perception::class::THREAT && t.modes & crate::perception::SIGHT != 0) {
        let (dx, dz) = crate::pose::wrapped_delta(a.pose.x, a.pose.z, t.x, t.z, ring);
        let d = (dx * dx + dz * dz).sqrt();
        if d > fauna.config.predation.flee_sight_m || dx * fx + dz * fz < 0.0 || nearest.is_some_and(|(n, _)| d >= n) {
            continue;
        }
        nearest = Some((d, dx * rx + dz * rz));
    }
    let (_, side) = nearest?;
    Some(away(if side > 0.0 { -1.0 } else { 1.0 }))
}

/// **The arboreal refuge** (S2, [`crate::vigil`]): a grounded glider's walk to the nearest
/// stand it can climb within `search_m` that it can walk to (EDGE), walking into the stem
/// climbs it.
pub(crate) fn refuge_step(
    fauna: &mut Fauna,
    i: usize,
    view: &VoxelView<'_>,
    fv: &cubarium_voxel_flora::FloraView<'_>,
    search_m: f64,
) -> Option<Actions> {
    glider::refuge_step(fauna, i, view, fv, search_m)
}

/// **The retraction** (AMPHIBIOUS-SNAIL, senses review §1): a body whose record holds a
/// threat it sensed by sight (a looming shadow within its short sight) or tremor at its
/// latest sensing stops and closes for its lineage's `retract_hold_s`
/// ([`crate::Amphibious`]), renewed while the threat stays. Reads the live record
/// ([`crate::perception`]); a lineage whose table is not live has no tracks and never
/// retracts.
fn retract(fauna: &mut Fauna, i: usize) {
    let a = fauna.animals[i];
    let window = fauna.config.perception.stagger_ticks();
    let now = fauna.tick;
    let Some(p) = crate::perception::percept(fauna, a.id) else { return };
    let threat = p.tracks().iter().any(|t| {
        t.class == crate::perception::class::THREAT
            && t.modes & (crate::perception::SIGHT | crate::perception::TREMOR) != 0
            && now.saturating_sub(t.tick) <= window
    });
    if !threat {
        return;
    }
    retract_action(fauna, i);
}

/// Enter/renew retraction for its minimum hold, independently of the policy layer.
fn retract_action(fauna: &mut Fauna, i: usize) {
    let Some(founder) = fauna.animals[i].founder else { return };
    let Some(am) = fauna.config.founder(founder).amphibious else { return };
    if am.retract_hold_s <= 0.0 {
        return;
    }
    let m = &mut fauna.animals[i].mobility;
    if m.retract_ticks == 0 {
        fauna.ledger.retracts += 1;
    }
    m.retract_ticks = m.retract_ticks.max(crate::Hunter::ticks(am.retract_hold_s));
}

/// **The water reflex** (AMPHIBIOUS-SNAIL): a thirsty amphibious body out of water
/// ([`crate::Mobility::thirsty`], [`crate::Mobility::ashore`]) walks to the nearest support
/// face with standing water within its lineage's `water_search_m`, at most its step up
/// above its own face. A heuristic world query, not a sense (as the shallows reflex is).
/// `None` when it is not thirsty ashore or no water is in reach.
fn water_reflex(fauna: &mut Fauna, i: usize, view: &VoxelView<'_>, fv: &cubarium_voxel_flora::FloraView<'_>) -> Option<(Actions, Option<u64>)> {
    if !fauna.animals[i].mobility.thirsty {
        return None;
    }
    to_water(fauna, i, view, fv)
}

/// The water reflex's walk: an amphibious body allowed on land, ashore on the ground and
/// free, to the nearest support face with standing water within `water_search_m` that it
/// can walk to ([`walk_to_face`]; SNAIL-STUCK).
fn to_water(fauna: &mut Fauna, i: usize, view: &VoxelView<'_>, fv: &cubarium_voxel_flora::FloraView<'_>) -> Option<(Actions, Option<u64>)> {
    let a = &fauna.animals[i];
    let founder = a.founder?;
    let phys = fauna.config.founder(founder);
    let am = phys.amphibious.filter(|am| am.on_land)?;
    if !a.mobility.ashore || a.mobility.mode != crate::MobilityMode::Ground || a.attached.is_some() {
        return None;
    }
    let faces = water_faces(fauna, i, view, am.water_search_m, |_| true, FACE_TRIES);
    walk_to_face(fauna, i, view, fv, &faces, am.water_search_m, 0.0)
}

/// **SNAIL-STUCK: an amphibious body's walk to a face**, for its own reflexes (the water
/// reflex, the giving-up density, the trip ashore, the roam): of `faces`, best first, the
/// first it can walk to on its lineage's route graph, and the actions that walk it there —
/// straight at the face when the line is walkable, else along the route — as every other
/// reflex walk goes ([`glider::walk_target`], EDGE). A walk it presses against for
/// `teachers.press_replan_s` is given up for `teachers.press_shun_s` and the next taken.
/// With the walk, the key its target goes by, which the caller hands to
/// [`Fauna::walking_to`] under the layer that holds it. `None` when it can walk to none of
/// them: its heuristic steers.
///
/// It steered straight at the face's centre before, wherever the face was: a shelf's top
/// over its own column, a few millimetres away across the plane, turned it round its own
/// centre for hours; a face across a trench or up a wall it could not take held it
/// pressing and turning back.
fn walk_to_face(
    fauna: &mut Fauna,
    i: usize,
    view: &VoxelView<'_>,
    fv: &cubarium_voxel_flora::FloraView<'_>,
    faces: &[Site],
    search_m: f64,
    feed: f64,
) -> Option<(Actions, Option<u64>)> {
    let cands: Vec<(f64, u64, Site)> = faces.iter().take(FACE_TRIES).map(|&s| (0.0, Fauna::walk_key_face(s), s)).collect();
    let (go, k) = glider::walk_target(fauna, i, view, fv, &cands, search_m, feed)?;
    Some((go, Some(cands[k].1)))
}

/// The faces an amphibious reflex's walk tries, best first ([`walk_to_face`]): a cost bound
/// on the route searches, not a knob.
const FACE_TRIES: usize = 16;

/// An amphibious reflex took body `id`'s held actions under layer `held`: its walk to the
/// face `key` starts or continues the pressed-walk clock (EDGE, [`Fauna::walking_to`]); a
/// hold that walks nowhere ends any walk an earlier layer of the sample put down.
fn amphibious_walk(fauna: &mut Fauna, id: u64, key: Option<u64>, held: crate::Steer) {
    match key {
        Some(key) => fauna.walking_to(id, key, held),
        None => {
            fauna.walk_pending.remove(&id);
        }
    }
}

pub(crate) fn walk_to_place(a: &Animal, view: &VoxelView<'_>, site: Site, feed: f64) -> Actions {
    let v = view.config.voxel_m;
    let (x, z) = crate::pack::centre(site, v);
    let (dx, dz) = crate::pose::wrapped_delta(a.pose.x, a.pose.z, x, z, f64::from(view.config.width) * v);
    glider::walk_toward(a, dx, dz, feed)
}

/// The water reflex's search, without its thirst/ashore trigger or any writes.
pub(crate) fn water_target(fauna: &Fauna, i: usize, view: &VoxelView<'_>, reach: f64, keep: impl Fn(Site) -> bool) -> Option<Site> {
    water_faces(fauna, i, view, reach, keep, 1).into_iter().next()
}

/// The water reflex's faces, nearest first, up to `limit` of them that pass `keep`: every
/// support face with standing water within `reach` of body `i`, at most its step up above
/// its own face, not in its own column (a face over or under where it stands is no walk).
pub(crate) fn water_faces(fauna: &Fauna, i: usize, view: &VoxelView<'_>, reach: f64, keep: impl Fn(Site) -> bool, limit: usize) -> Vec<Site> {
    let Some(founder) = fauna.animals[i].founder else { return Vec::new() };
    let a = &fauna.animals[i];
    let phys = fauna.config.founder(founder);
    let c = view.config;
    let v = c.voxel_m;
    let reach = reach.max(0.0);
    let k = (reach / v).ceil() as i64;
    let up = (phys.step_up_m / v).floor().max(0.0) as u32;
    let (hx, hz) = (i64::from(a.site.x), i64::from(a.site.z));
    let ring = f64::from(c.width) * v;
    let top = (a.site.y + up).min(c.height.saturating_sub(1));
    // In range, nearest first (ties in scan order); the terrain and `keep` only after.
    let mut near: Vec<(f64, i64, u32)> = Vec::new();
    for dz in -k..=k {
        let z = hz + dz;
        if z < 0 || z >= i64::from(c.depth) {
            continue;
        }
        for dx in -k..=k {
            if dx == 0 && dz == 0 {
                continue;
            }
            let x = (hx + dx).rem_euclid(i64::from(c.width));
            let (tx, tz) = ((x as f64 + 0.5) * v, (z as f64 + 0.5) * v);
            let (ddx, ddz) = crate::pose::wrapped_delta(a.pose.x, a.pose.z, tx, tz, ring);
            let d = (ddx * ddx + ddz * ddz).sqrt();
            if d <= reach {
                near.push((d, x, z as u32));
            }
        }
    }
    near.sort_by(|p, q| p.0.total_cmp(&q.0));
    let mut out = Vec::new();
    for (_, x, z) in near {
        if out.len() >= limit {
            break;
        }
        let Some(y) = view.highest_support_in(x, z, 0, top) else {
            continue;
        };
        if !(view.standing_depth_m(x, y, z) > 0.0) {
            continue;
        }
        let site = Site { x: x as u32, y, z };
        if keep(site) {
            out.push(site);
        }
    }
    out
}

/// **Ready for the trip ashore** (SNAIL-LAND-2): the laying's own test ([`lay`]) — an
/// egg-layer, not dormant, its hold met, no retry pending, a clutch its reserve pays for —
/// and not saving: with a clutch from surplus and a wait (`clutch_hold_max_s`), a reserve
/// short of the full clutch the reserve cap `cap` pays for waits out the wait first,
/// feeding where it is.
pub(crate) fn trip_ready(rule: &crate::Reproduction, st: &crate::ReproductionState, reserve: f64, cap: f64) -> bool {
    if rule.mode != crate::BirthMode::Eggs
        || rule.dormant
        || !rule.hold_met(st)
        || st.retry_ticks > 0
        || rule.clutch_for(reserve) == 0
    {
        return false;
    }
    let wait = rule.clutch_hold_max_ticks();
    let saving = rule.clutch_max > 0 && wait > 0 && rule.clutch_for(reserve) < rule.full_clutch(cap) && st.clutch_wait_ticks < wait;
    !saving
}

/// **The giving-up density** (SNAIL-LAND-2, [`crate::Amphibious::graze_give_up`]): an
/// amphibious grazer driven by its own heuristic, on the ground, awake, free, not
/// retracted and not thirsty, wanting food (its eating latch on), on a face whose edible
/// film (above the grazing refuge, TASTE) is under `graze_give_up` of one site's
/// underwater capacity, walks to the face
/// [`graze_target`] picks, feeding as it goes. `None` otherwise (its heuristic stays on
/// the film it tastes). A heuristic world query over the plant layer, as the roam is.
fn graze_reflex(
    fauna: &mut Fauna,
    i: usize,
    view: &VoxelView<'_>,
    fv: &cubarium_voxel_flora::FloraView<'_>,
    sky: &Sky,
) -> Option<(Actions, Option<u64>)> {
    let a = &fauna.animals[i];
    let founder = a.founder?;
    let phys = fauna.config.founder(founder);
    let am = phys.amphibious.filter(|am| am.graze_give_up > 0.0 && am.graze_search_m > 0.0)?;
    let m = &a.mobility;
    if !a.eating
        || m.thirsty
        || m.retract_ticks > 0
        || m.mode != crate::MobilityMode::Ground
        || a.attached.is_some()
        || a.state == State::Asleep
    {
        return None;
    }
    let v = view.config.voxel_m;
    let k_site = fv.config.film_capacity_per_m2 * v * v;
    // TASTE: the edible film, above the grazing refuge, as its mouth tastes it.
    if !(k_site > 0.0) || fv.edible_film_at(a.site) >= am.graze_give_up * k_site {
        return None;
    }
    let roam = am.on_land && roam_hours(fauna, &am, view, sky);
    let others: Vec<Site> = fauna
        .animals
        .iter()
        .filter(|b| b.id != a.id && b.founder == Some(founder))
        .map(|b| b.site)
        .collect();
    // SNAIL-STUCK: the richest faces first, to the first it can walk to.
    let faces = film_faces(view, &am, a, roam, k_site, &|s| fv.edible_film_at(s), &others, |_| true, FACE_TRIES);
    walk_to_face(fauna, i, view, fv, &faces, am.graze_search_m, 1.0)
}

/// **Where a grazer goes** ([`graze_reflex`]): over the support faces within
/// `graze_search_m` of body `a` (each column's highest at most `roam_search_m` above its
/// own face, as the roam's search), not in its own column, under standing water — or, with `roam`
/// (its roam hours, on land allowed), out of it on ground of `step_moisture_min` or more —
/// the face whose `film` per snail on it (itself counted; `others` are the lineage's other
/// bodies' faces) is at least `graze_worth · k_site` and beats its own face's film by the
/// most per metre. The offset `(dx, dz)` to its centre, metres (`x` wraps); `None` when no
/// face is worth the walk. The reflex walks to the first of [`film_faces`] it can reach
/// (SNAIL-STUCK); this is the first of them, for the tests of the choice.
#[cfg(test)]
pub(crate) fn graze_target<F: Fn(Site) -> f64>(
    view: &VoxelView<'_>,
    am: &crate::Amphibious,
    a: &Animal,
    roam: bool,
    k_site: f64,
    film: &F,
    others: &[Site],
) -> Option<(f64, f64)> {
    let site = film_target(view, am, a, roam, k_site, film, others, |_| true)?;
    let v = view.config.voxel_m;
    let (x, z) = crate::pack::centre(site, v);
    Some(crate::pose::wrapped_delta(a.pose.x, a.pose.z, x, z, f64::from(view.config.width) * v))
}

/// The film reflex's scored search, with an additional sensory eligibility test.
pub(crate) fn film_target<F: Fn(Site) -> f64>(view: &VoxelView<'_>, am: &crate::Amphibious, a: &Animal, roam: bool, k_site: f64, film: &F, others: &[Site], keep: impl Fn(Site) -> bool) -> Option<Site> {
    film_faces(view, am, a, roam, k_site, film, others, keep, 1).into_iter().next()
}

/// [`graze_target`]'s faces, best first (ties in scan order), up to `limit` of them that
/// pass `keep`. Not in its own column (SNAIL-STUCK): a shelf's top over where it stands is
/// a different face a few millimetres off across the plane, and its film per metre beat
/// every other face's.
#[allow(clippy::too_many_arguments)]
pub(crate) fn film_faces<F: Fn(Site) -> f64>(
    view: &VoxelView<'_>,
    am: &crate::Amphibious,
    a: &Animal,
    roam: bool,
    k_site: f64,
    film: &F,
    others: &[Site],
    keep: impl Fn(Site) -> bool,
    limit: usize,
) -> Vec<Site> {
    let c = view.config;
    let v = c.voxel_m;
    let reach = am.graze_search_m;
    if !(reach > 0.0) || !(v > 0.0) {
        return Vec::new();
    }
    let here = film(a.site);
    let worth = am.graze_worth * k_site;
    let k = (reach / v).ceil() as i64;
    let rise = (am.roam_search_m.max(0.0) / v).ceil() as u32;
    let top = (a.site.y + rise).min(c.height.saturating_sub(1));
    let (hx, hz) = (i64::from(a.site.x), i64::from(a.site.z));
    let ring = f64::from(c.width) * v;
    let mut scored: Vec<(f64, Site)> = Vec::new();
    for dz in -k..=k {
        let z = hz + dz;
        if z < 0 || z >= i64::from(c.depth) {
            continue;
        }
        for dx in -k..=k {
            if dx == 0 && dz == 0 {
                continue;
            }
            let x = (hx + dx).rem_euclid(i64::from(c.width));
            let (tx, tz) = ((x as f64 + 0.5) * v, (z as f64 + 0.5) * v);
            let (ddx, ddz) = crate::pose::wrapped_delta(a.pose.x, a.pose.z, tx, tz, ring);
            let d = (ddx * ddx + ddz * ddz).sqrt();
            if d > reach || !(d > 0.0) {
                continue;
            }
            let Some(y) = view.highest_support_in(x, z as u32, 0, top) else {
                continue;
            };
            let s = Site { x: x as u32, y, z: z as u32 };
            let wet = view.standing_depth_m(x, y, s.z) > 0.0;
            if !wet && !(roam && body::ground_moisture(view, x, y, s.z) >= am.step_moisture_min) {
                continue;
            }
            let crowd = 1 + others.iter().filter(|&&o| o == s).count();
            let per = film(s) / crowd as f64;
            if per < worth || per <= here {
                continue;
            }
            scored.push(((per - here) / d, s));
        }
    }
    // Best first; a ray (`keep`) only for faces taken in that order.
    scored.sort_by(|p, q| q.0.total_cmp(&p.0));
    scored.into_iter().map(|(_, s)| s).filter(|&s| keep(s)).take(limit).collect()
}

/// **The trip ashore to lay** (SNAIL-LAND, [`crate::EggSite::Shore`]). For an amphibious
/// body allowed on land whose lineage lays on the shore, on the ground, free, awake, not
/// retracted and not thirsty:
/// - **ready to lay** ([`trip_ready`]: its hold met, no retry pending, a clutch its reserve pays for,
///   not saving through its clutch wait): on a
///   shore egg face ([`shore_egg_site`]) whose nest is not crowded it holds still to lay;
///   otherwise it walks to the nearest such face within `egg_search_m` (SNAIL-LAND-2; it
///   was `roam_search_m`) whose round trip its drying clock can afford — the clock now plus twice the walk at cruise, at the drying rate of the
///   target's ground (times the day factor by day), stays under the line it turns back
///   at — and with none, its heuristic steers;
/// - **not ready**, ashore and outside its roam hours: back to the water ([`to_water`]):
///   after a laying, and a hatchling on the bank.
fn shore_reflex(fauna: &mut Fauna, i: usize, view: &VoxelView<'_>, fv: &cubarium_voxel_flora::FloraView<'_>, sky: &Sky) -> Option<(Actions, Option<u64>)> {
    let a = &fauna.animals[i];
    let founder = a.founder?;
    let phys = fauna.config.founder(founder);
    if phys.egg_site != crate::EggSite::Shore {
        return None;
    }
    let am = phys.amphibious.filter(|am| am.on_land)?;
    let m = &a.mobility;
    if m.thirsty
        || m.retract_ticks > 0
        || m.mode != crate::MobilityMode::Ground
        || a.attached.is_some()
        || a.state == State::Asleep
    {
        return None;
    }
    let sc = body::effective_config(&fauna.config, a);
    let rule = sc.reproduction;
    // SNAIL-LAND-2: ready by the laying's own rule — a layer saving for its full clutch
    // keeps feeding through its wait instead of holding still ashore.
    let ready = trip_ready(&rule, &a.reproduction, a.reserve, sc.reserve_of(a.body));
    if !ready {
        return if m.ashore && !roam_hours(fauna, &am, view, sky) { to_water(fauna, i, view, fv) } else { None };
    }
    // It holds to lay on a shore face whose nest is not crowded (the laying's own test).
    if shore_egg_site(view, &am, a.site) && !nest_crowded(fauna, i, view, &rule) {
        return Some((Actions { forward: 0.0, turn: 0.0, feed: 0.0 }, None));
    }
    // SNAIL-STUCK: the nearest it can walk to.
    let faces = shore_faces(fauna, i, view, sky, am.egg_search_m, |_| true, FACE_TRIES);
    walk_to_face(fauna, i, view, fv, &faces, am.egg_search_m, 0.0)
}

/// The shore-egg reflex's nearest affordable, uncrowded egg face.
pub(crate) fn shore_target(fauna: &Fauna, i: usize, view: &VoxelView<'_>, sky: &Sky, reach: f64, keep: impl Fn(Site) -> bool) -> Option<Site> {
    shore_faces(fauna, i, view, sky, reach, keep, 1).into_iter().next()
}

/// [`shore_target`]'s faces, nearest first, up to `limit` of them that pass `keep`; not in
/// its own column (SNAIL-STUCK: a face over or under where it stands is no walk).
pub(crate) fn shore_faces(fauna: &Fauna, i: usize, view: &VoxelView<'_>, sky: &Sky, reach: f64, keep: impl Fn(Site) -> bool, limit: usize) -> Vec<Site> {
    let a = &fauna.animals[i];
    let Some(founder) = a.founder else { return Vec::new() };
    let phys = fauna.config.founder(founder);
    let Some(am) = phys.amphibious else { return Vec::new() };
    let m = &a.mobility;
    let rule = body::effective_config(&fauna.config, a).reproduction;
    let c = view.config;
    let v = c.voxel_m;
    let day = sky.clock_on && sky.daylight >= fauna.config.life.day_line;
    let raining = view.is_raining();
    let back_at = if day && !raining { am.day_return_fraction } else { am.return_fraction };
    let speed = crate::body_of(&fauna.config, a).map_or(0.0, |b| b.length_m) * phys.cruise_body_lengths_per_s;
    if !(speed > 0.0) {
        return Vec::new();
    }
    // SNAIL-LAND-2: its own reach for the trip (it was `roam_search_m`).
    let reach = reach.max(0.0);
    let k = (reach / v).ceil() as i64;
    let (hx, hz) = (i64::from(a.site.x), i64::from(a.site.z));
    let ring = f64::from(c.width) * v;
    // A pond's bank stands above the bed: faces up to `roam_search_m` above its own.
    let rise = (am.roam_search_m.max(0.0) / v).ceil() as u32;
    let top = (a.site.y + rise).min(c.height.saturating_sub(1));
    // The faces in reach that pass the cheap tests, nearest first; the first that is a
    // shore egg face whose round trip the clock affords and whose nest is not crowded
    // (SNAIL-LAND-2) is the target.
    let mut near: Vec<(f64, f64, f64, Site)> = Vec::new();
    for dz in -k..=k {
        let z = hz + dz;
        if z < 0 || z >= i64::from(c.depth) {
            continue;
        }
        for dx in -k..=k {
            if dx == 0 && dz == 0 {
                continue;
            }
            let x = (hx + dx).rem_euclid(i64::from(c.width));
            let Some(y) = view.highest_support_in(x, z as u32, 0, top) else {
                continue;
            };
            let z = z as u32;
            if view.standing_depth_m(x, y, z) > 0.0 || body::ground_moisture(view, x, y, z) < am.egg_moisture_min {
                continue;
            }
            let (tx, tz) = ((x as f64 + 0.5) * v, (f64::from(z) + 0.5) * v);
            let (ddx, ddz) = crate::pose::wrapped_delta(a.pose.x, a.pose.z, tx, tz, ring);
            let d = (ddx * ddx + ddz * ddz).sqrt();
            if d <= reach {
                near.push((d, ddx, ddz, Site { x: x as u32, y, z }));
            }
        }
    }
    near.sort_by(|p, q| p.0.total_cmp(&q.0));
    let mut out = Vec::new();
    for (d, _, _, s) in near {
        if out.len() >= limit {
            break;
        }
        // The clock's check: the round trip at the target's drying rate.
        let mut rate = am.drying_per_s(body::ground_moisture(view, i64::from(s.x), s.y, s.z));
        if day {
            rate *= am.day_drying_factor;
        }
        if raining {
            rate *= am.rain_drying_factor;
        }
        if !(m.dryness + rate * 2.0 * d / speed < back_at) || !shore_egg_site(view, &am, s) {
            continue;
        }
        let (fx, fz) = ((f64::from(s.x) + 0.5) * v, (f64::from(s.z) + 0.5) * v);
        // Sorted in the original distance/tie order. Reject unsuitable nests
        // before tracing a ray, then take the first perceptible one.
        if nest_crowded_at(fauna, a.id, founder, fx, fz, view, &rule) || !keep(s) {
            continue;
        }
        out.push(s);
    }
    out
}

/// Whether `sky` and the rain record make this an amphibious body's **roam hours**
/// (SNAIL-FIX): night (the clock on and daylight under the day line, or the clock off, as
/// the drying clock reads it), rain falling, or within its `roam_after_rain_s` of the last
/// rain.
pub(crate) fn roam_hours(fauna: &Fauna, am: &crate::Amphibious, view: &VoxelView<'_>, sky: &Sky) -> bool {
    let day = sky.clock_on && sky.daylight >= fauna.config.life.day_line;
    if !day || view.is_raining() {
        return true;
    }
    let after = (am.roam_after_rain_s.max(0.0) / DT).round() as u64;
    fauna
        .last_rain_tick
        .is_some_and(|t| fauna.tick.saturating_sub(t) <= after)
}

/// **The roam drive** (SNAIL-FIX, [`crate::Amphibious::roam_search_m`]): in its roam hours
/// ([`roam_hours`]) an amphibious body allowed on land, on the ground, awake, not thirsty,
/// not retracted, and not feeding where it stands (its eating latch on and its heuristic
/// tasting food) walks to the support face within `roam_search_m` that is out of water
/// and wet enough to step on (`step_moisture_min`) and holds the most film per metre of
/// distance (the plant layer's land film); in water with no such film in reach, to the
/// nearest such face. Its own face is never the target, so a sated body moves on from a
/// grazed patch. `None` otherwise: ashore with no land film in reach, its heuristic
/// (film chemistry, or its wander) steers. A heuristic world query, not a sense, as the
/// water reflex is.
fn roam_reflex(
    fauna: &mut Fauna,
    i: usize,
    view: &VoxelView<'_>,
    fv: &cubarium_voxel_flora::FloraView<'_>,
    sky: &Sky,
    asked: Actions,
) -> Option<(Actions, Option<u64>)> {
    let a = &fauna.animals[i];
    let founder = a.founder?;
    let phys = fauna.config.founder(founder);
    let am = phys.amphibious.filter(|am| am.on_land && am.roam_search_m > 0.0)?;
    let m = &a.mobility;
    if m.thirsty
        || m.retract_ticks > 0
        || m.mode != crate::MobilityMode::Ground
        || a.attached.is_some()
        || a.state == State::Asleep
        || (a.eating && asked.feed > 0.5)
        || !roam_hours(fauna, &am, view, sky)
    {
        return None;
    }
    let c = view.config;
    let v = c.voxel_m;
    let reach = am.roam_search_m;
    let k = (reach / v).ceil() as i64;
    let (hx, hz) = (i64::from(a.site.x), i64::from(a.site.z));
    let ring = f64::from(c.width) * v;
    // A pond's bank stands above the bed it grazes: faces up to the search reach above
    // its own (a slope it walks up in strides; the step rule refuses what it cannot).
    let top = (a.site.y + k.max(0) as u32).min(c.height.saturating_sub(1));
    // (score, face): the most film per metre; (distance, face): the nearest wet faces.
    let mut film_faces: Vec<(f64, Site)> = Vec::new();
    let mut near: Vec<(f64, Site)> = Vec::new();
    for dz in -k..=k {
        let z = hz + dz;
        if z < 0 || z >= i64::from(c.depth) {
            continue;
        }
        for dx in -k..=k {
            if dx == 0 && dz == 0 {
                continue;
            }
            let x = (hx + dx).rem_euclid(i64::from(c.width));
            let Some(y) = view.highest_support_in(x, z as u32, 0, top) else {
                continue;
            };
            let z = z as u32;
            if body::water_within(view, x, y, z, phys.water_margin_m)
                || body::ground_moisture(view, x, y, z) < am.step_moisture_min
            {
                continue;
            }
            let (tx, tz) = ((x as f64 + 0.5) * v, (z as f64 + 0.5) * v);
            let (ddx, ddz) = crate::pose::wrapped_delta(a.pose.x, a.pose.z, tx, tz, ring);
            let d = (ddx * ddx + ddz * ddz).sqrt();
            if d > reach || d <= 0.0 {
                continue;
            }
            // TASTE: the edible film, above the grazing refuge.
            let s = Site { x: x as u32, y, z };
            let film = fv.edible_film_at(s);
            if film > 0.0 {
                film_faces.push((film / d, s));
            }
            near.push((d, s));
        }
    }
    // SNAIL-STUCK: to the first of them it can walk to (best first, ties in scan order); in
    // water with no land film it can walk to, to the nearest wet land it can.
    let ashore = m.ashore;
    film_faces.sort_by(|p, q| q.0.total_cmp(&p.0));
    let film: Vec<Site> = film_faces.into_iter().map(|(_, s)| s).collect();
    if let Some(go) = walk_to_face(fauna, i, view, fv, &film, reach, 0.0) {
        return Some(go);
    }
    if ashore {
        return None;
    }
    near.sort_by(|p, q| p.0.total_cmp(&q.0));
    let near: Vec<Site> = near.into_iter().map(|(_, s)| s).collect();
    walk_to_face(fauna, i, view, fv, &near, reach, 0.0)
}

/// Step 5: resolve each founder's held actions, in id order. A body with no lineage
/// rests.
fn act(
    fauna: &mut Fauna,
    view: &VoxelView<'_>,
    flora: &mut Flora,
    seed: u64,
    tick: u64,
    sky: &Sky,
    hits: &mut Vec<Hit>,
) {
    let v = view.config.voxel_m;
    let ring = f64::from(view.config.width) * v;
    // FLUSH: the bodies a flyer may sense coming at it, the hunters (indices: no body leaves
    // during the act).
    let hunters: Vec<usize> = (0..fauna.animals.len())
        .filter(|&k| {
            let o = &fauna.animals[k];
            o.attached.is_none() && o.founder.is_some_and(|f| fauna.config.founder(f).active_hunter().is_some())
        })
        .collect();
    for i in 0..fauna.animals.len() {
        // A rider has no motion, feeding or strike of its own (parasite §2): it is alive
        // this tick and still, and is carried below.
        if fauna.animals[i].attached.is_some() {
            if let Some(founder) = fauna.animals[i].founder {
                fauna.ledger.founder_ticks_by_founder[founder.index()] += 1;
            }
            let a = &mut fauna.animals[i];
            a.speed_m_s = 0.0;
            a.still_ticks = a.still_ticks.saturating_add(1);
            continue;
        }
        // Contact owns this body's act even between controller samples. The post-act
        // grooming pass checks again after every host has moved and all strikes landed.
        if crate::groom::holding(fauna, i, view) {
            let a = &mut fauna.animals[i];
            a.state = State::Resting;
            a.speed_m_s = 0.0;
            a.still_ticks = a.still_ticks.saturating_add(1);
            if let Some(f) = a.founder {
                fauna.ledger.founder_ticks_by_founder[f.index()] += 1;
                crate::gait::rest(fauna, i, f);
            }
            continue;
        }
        let before = fauna.animals[i].pose;
        let (flyer, glider) = fauna.animals[i]
            .founder
            .map_or((None, None), |f| (fauna.config.founder(f).flight, fauna.config.founder(f).glider));
        match (flyer, glider) {
            (Some(flight), _) => flyer_act(fauna, view, flora, sky, flight, i, &hunters),
            (None, Some(g)) => glider::glider_act(fauna, view, flora, seed, tick, sky, hits, g, i),
            (None, None) => {
                act_one(fauna, view, flora, seed, tick, hits, i);
                // A walker's base is the top of its support face (F1).
                let a = &mut fauna.animals[i];
                a.pose.y_m = crate::surface_m(a.site.y, v);
            }
        }
        // SEEDPORTER-POLLEN: pollen brushes off a body that carries it onto the vine flowers
        // it moved into touch with.
        if fauna.animals[i].founder.is_some_and(|f| fauna.config.founder(f).pollen_by_contact) {
            brush_pollen(fauna, view, flora, &before, i);
        }
        // What the body did, for the senses of others (predation §2.2): its ground speed
        // this tick, and how long it has been still.
        let length = crate::body_of(&fauna.config, &fauna.animals[i]).map_or(v, |b| b.length_m);
        let a = &mut fauna.animals[i];
        let (dx, dz) = crate::pose::wrapped_delta(before.x, before.z, a.pose.x, a.pose.z, ring);
        let speed = (dx * dx + dz * dz).sqrt() / DT;
        a.speed_m_s = if speed.is_finite() { speed } else { 0.0 };
        if a.speed_m_s > fauna.config.predation.still_speed_bl_per_s * length {
            a.still_ticks = 0;
        } else {
            a.still_ticks = a.still_ticks.saturating_add(1);
        }
    }
    carry_riders(fauna);
}

/// **Pollen by contact** (SEEDPORTER-POLLEN, Wrysk 2026-10-09: "seedporters should also
/// polinate by moving accross them or around them"), for a body whose lineage has
/// [`crate::FounderPhysiology::pollen_by_contact`]: the open vine flowers its body moved
/// into touch with this tick — within [`crate::flight::PRESSED_M`] of its collision
/// cylinder (its footprint disc, or a flyer's span disc, its height tall) — are visited when it brings pollen from
/// another flower ([`crate::flight::brushed_flowers`] has the rule), booked to the flowers
/// ([`Flora::visit_face`]) and to the lineage's contact visits. A body that did not move
/// touched nothing new, so it is not scanned; nor is a world without vines.
fn brush_pollen(fauna: &mut Fauna, view: &VoxelView<'_>, flora: &mut Flora, before: &crate::Pose, i: usize) {
    let a = &fauna.animals[i];
    if a.pose == *before {
        return;
    }
    let Some(founder) = a.founder else {
        return;
    };
    let (visited, carries) = {
        let fv = flora.view();
        if fv.cover.face_count() == 0 {
            return;
        }
        // The body's own collision shape: a flyer's span disc, a walker's footprint disc.
        let phys = fauna.config.founder(founder);
        let shape = match phys.flight {
            Some(flight) => crate::flight::Shape::of(phys, &flight, a.body),
            None => {
                let g = phys.body_at(a.body);
                crate::flight::Shape {
                    r: g.footprint_radius(),
                    h: g.height_m.max(1e-6),
                }
            }
        };
        crate::flight::brushed_flowers(view, &fv, before, &a.pose, shape, crate::flight::PRESSED_M, a.last_bloom)
    };
    fauna.animals[i].last_bloom = carries;
    for f in visited {
        if flora.visit_face(f) {
            fauna.ledger.contact_visits_by_founder[founder.index()] += 1;
        }
    }
}

/// One body's act (step 5).
fn act_one(
    fauna: &mut Fauna,
    view: &VoxelView<'_>,
    flora: &mut Flora,
    seed: u64,
    tick: u64,
    hits: &mut Vec<Hit>,
    i: usize,
) {
    if fauna.animals[i].state == State::Asleep {
        // No movement, no feeding; the founder's tick is still counted alive.
        if let Some(founder) = fauna.animals[i].founder {
            fauna.ledger.founder_ticks_by_founder[founder.index()] += 1;
        }
        return;
    }
    if fauna.animals[i].founder.is_some() {
        founder_act(fauna, i, view, flora, seed, tick, hits);
    } else {
        fauna.animals[i].state = State::Resting;
    }
}

/// One founder tick: pay the motor budget on the **requested** equivalent
/// displacement, resolve the held motion on the continuous pose, re-derive the support
/// site from the pose, record the interval's motion feedback, and — on the due tick —
/// attempt the one local bite. A founder with a zero held action set (no controller
/// yet, or a controller at rest) pays no motor budget and eats nothing.
fn founder_act(
    fauna: &mut Fauna,
    i: usize,
    view: &VoxelView<'_>,
    flora: &mut Flora,
    seed: u64,
    tick: u64,
    hits: &mut Vec<Hit>,
) {
    let (founder, mut held, due) = {
        let a = &fauna.animals[i];
        let founder = a.founder.expect("a founder body");
        (
            founder,
            a.founder_state.held,
            a.age_ticks % founder.manifest().cadence_ticks() == 0,
        )
    };
    // YOUNG-ROAM: a young body walks and runs at its own length's pace.
    let manifest = fauna.config.body_manifest(founder, fauna.animals[i].body);
    let phys = *fauna.config.founder(founder);
    let sc = phys.core;
    if (fauna.controllers.layer(fauna.animals[i].id) == crate::Layer::Brain && fauna.animals[i].hunkered) || fauna.animals[i].mobility.retract_ticks > 0 {
        held = Actions::REST;
    }
    fauna.ledger.founder_ticks_by_founder[founder.index()] += 1;

    // A carnivore's lit lure costs `lure_power` × its awake upkeep (predation §2.2). It
    // lures only while it wants food (repair 2 item 7): a fed hunter's lure is dark.
    if let Some(h) = phys.active_hunter() {
        if !fauna.animals[i].eating {
            fauna.animals[i].founder_state.lure = 0.0;
        }
        if fauna.animals[i].founder_state.lure > 0.0 {
            let cost = h.lure_power * sc.maintenance_per_s * fauna.animals[i].body * DT;
            respire(fauna, i, cost, Respiration::Hunt);
        }
        // A strike's windup and lunge own the tick; a recovery counts down.
        match fauna.animals[i].strike {
            Some(s) if s.phase != StrikePhase::Recovery => {
                strike_tick(fauna, i, view, &phys, h, seed, tick, hits);
                if let Some(tap) = fauna.tap.as_mut() {
                    tap.gait(fauna.animals[i].id, fauna.tick, 0.0, 1.0);
                }
                return;
            }
            Some(mut s) => {
                s.ticks_left = s.ticks_left.saturating_sub(1);
                fauna.animals[i].strike = (s.ticks_left > 0).then_some(s);
            }
            None => {}
        }
    }
    // The speed cap (predation §2.4): × (1 − wound_slow·wound), and `recovery_speed`
    // (half) in a recovery.
    {
        let pc = &fauna.config.predation;
        let a = &fauna.animals[i];
        let mut slow = (1.0 - pc.wound_slow * a.wound).clamp(0.0, 1.0);
        if a.strike.is_some_and(|s| s.phase == StrikePhase::Recovery) {
            slow *= pc.recovery_speed;
        }
        held.forward *= slow;
    }
    // GAIT: a forward above cruise is a sprint, allowed on its feet while its stamina lasts.
    let on_feet = fauna.animals[i].mobility.climb.is_none();
    crate::gait::gate(fauna, i, founder, on_feet, &mut held);

    // On a wall the tick is the climb's, and nothing else: no feeding, no sweep.
    if fauna.animals[i].mobility.climb.is_some() {
        fauna.ledger.wall_ticks_by_founder[founder.index()] += 1;
        climb_tick(fauna, i, view, &manifest, &phys, held);
        return;
    }

    // The motor budget: `motor_respiration_per_s` of organic matter per unit of body
    // per second at full cruise, scaled by the requested equivalent displacement
    // `|v| + r·|yaw rate|` against the cruise reference. Turning while stopped is
    // permitted and paid; an attempt a wall blocks is attempted in full and paid in
    // full; zero movement is rest and pays nothing here.
    // The body this animal has right now: its footprint, its clearance, its mouth band
    // and its receptors are all fractions of it (decisions §1).
    let geometry = phys.body_at(fauna.animals[i].body);
    let r = geometry.footprint_radius();
    let v_req = held.forward * manifest.cruise_m_per_s;
    let yaw_req = held.turn * manifest.yaw_cap_rad_per_s;
    let eq_rate = v_req.abs() + r * yaw_req.abs();
    let cost = phys.motor_respiration_per_s
        * fauna.animals[i].body
        * (eq_rate / manifest.cruise_m_per_s)
        * DT;
    let (motor_paid, motor_body) = respire(fauna, i, cost, Respiration::Motor);

    // Paid heading motion on the continuous pose: turn, then a bounded sub-step sweep.
    // The sweep may put the body's feet on a support face within its lineage's step up
    // or down of the one it started on, so the standing layer goes in and comes back out
    // (`design/handoffs/voxel-founder-step-2026-09-22.md`; split up/down and the water
    // read against the body's own height by package mobility).
    let step_limits = phys.step_limits(view.config.voxel_m);
    let wall_friction = fauna.config.life.wall_friction;
    // WADE-OUT: a body wading its way out admits the way's water (`jump::Wade`).
    let wade_m = fauna.wade_limit(fauna.animals[i].id).map_or(phys.wade_depth_m(&geometry), |w| w.max(phys.wade_depth_m(&geometry)));
    let (motion, standing_y) = {
        let a = &mut fauna.animals[i];
        let mut standing_y = a.site.y;
        let motion = body::resolve_motion_in(
            view,
            &mut a.pose,
            &mut standing_y,
            &manifest,
            &geometry,
            wade_m,
            step_limits,
            wall_friction,
            held,
            // A thirsty amphibious body is held to the water-bound rule until it has
            // drunk (AMPHIBIOUS-SNAIL).
            body::WaterRule::of(&phys).map(|mut r| {
                if a.mobility.thirsty {
                    r.land_moisture_min = None;
                }
                r
            }),
        );
        (motion, standing_y)
    };
    {
        let from = fauna.animals[i].site.y;
        let l = &mut fauna.ledger;
        if standing_y > from {
            l.ledges_up_by_founder[founder.index()] += 1;
        } else if standing_y < from {
            l.ledges_down_by_founder[founder.index()] += 1;
        }
    }
    // The pose is authoritative for a founder: the support site follows the centre
    // column, and its layer follows whatever ledge the sweep stepped onto. Movement is
    // constrained so this always stays a support face; a pose that cannot be resolved at
    // all leaves the site as it was.
    {
        let a = &mut fauna.animals[i];
        if let Some((cx, cz)) = a.pose.column(view.config.voxel_m, view.config.depth) {
            a.site = Site {
                x: cx.rem_euclid(i64::from(view.config.width)) as u32,
                y: standing_y,
                z: cz,
            };
        }
    }
    // DROP: walked to the edge its jump leaves from, facing the landing, it jumps down.
    jump::take_off(fauna, i, founder, view, held);
    // The interval's motion feedback, attempted and delivered separately, plus the
    // motor respiration the interval paid — the per-interval counter the search driver's
    // score reads alongside the ledger's own split.
    {
        let fb = &mut fauna.animals[i].founder_state.feedback;
        fb.attempted_equivalent += motion.attempted_equivalent;
        fb.delivered_equivalent += motion.delivered_equivalent;
        fb.delivered_forward += motion.delivered_forward;
        fb.delivered_turn += motion.delivered_turn;
        fb.structural_loss += motor_body;
        fb.motor_respiration += motor_paid;
    }

    // A climber stopped by terrain it could go up or down attaches to the face instead;
    // one stopped under an overhang turns back (package mobility, decision 1).
    if motion.blocked && held.forward > 0.0 && phys.climbs_walls {
        match attach(fauna, i, view, &phys, &geometry, step_limits) {
            Attach::Climbing => {
                fauna.animals[i].state = State::Climbing;
                return;
            }
            // SNAIL-STUCK: a face that turns it round is in its search leg's way.
            Attach::TurnedBack => {
                let period_s = manifest.cadence_ticks().max(1) as f64 * DT;
                rove::turned_back(fauna, i, period_s, f64::from(view.config.width) * view.config.voxel_m);
            }
            Attach::Nothing => {}
        }
    }

    // Feeding: one attempt per controller interval, on the due tick, from the
    // post-motion mouth, through the plant layer's real withdrawals. A failed or
    // no-contact attempt transfers nothing.
    let mut fed = false;
    if due && held.feed > 0.0 {
        // A carnivore's `feed` is a strike when a prey is in its capsule (predation
        // §2.3), and a mouthful of carrion otherwise.
        // R3: a pup's strike is refused, whatever its controller asks; and a hunter strikes
        // only while it wants food (repair 2 item 7).
        let pup = fauna.animals[i].pack.is_some_and(|m| m.pup);
        let wants = fauna.animals[i].eating;
        let target = match phys.active_hunter() {
            Some(h) if fauna.animals[i].strike.is_none() && !pup && wants => {
                prey_in_capsule(fauna, i, view, &geometry, &h)
            }
            _ => None,
        };
        // HUNTER-MEAL: a hunter whose mouth is over a usable carcass eats it; it does not
        // strike while that meal lies there.
        let target = match target {
            Some(_)
                if fauna.config.hunt.meal_first
                    && crate::hunt::mouth_on_carcass(&flora.view(), view, &fauna.animals[i], &geometry) =>
            {
                fauna.ledger.hunt_strikes_withheld += 1;
                None
            }
            t => t,
        };
        if let Some(prey) = target {
            if crate::pack::tactics::strike_rule(fauna, i).is_some_and(|r| r.target != Some(prey)) {
                fauna.ledger.hound_substitutions += 1;
            }
            start_strike(fauna, i, prey);
            return;
        }
        fed = founder_feed(fauna, i, view, flora, &manifest, &geometry, &phys, held.feed);
    }

    fauna.animals[i].state = if fed {
        State::Cropping
    } else if motion.delivered_forward > 0.0 {
        State::Walking
    } else {
        State::Resting
    };
}

/// **One flyer's tick** (F1, predation-and-flight §3.2–3.8), in place of a walker's act:
///
/// 1. **The perch**: a perched flyer whose perch is gone (a crown eaten, a face uncovered)
///    takes off when it is awake, the sky allows it and it can pay a takeoff (its reserve
///    and its body above `body_min`, as every motor cost is paid); else
///    it **drops** — straight down at the lift speed, paying nothing aloft — onto the
///    first perch below, or onto water too deep to stand in, where the drop ends and it
///    hovers on the surface (BW-STUCK).
/// 2. **Forced down**: in a downpour, grounded by rain ([`Mobility::rain_grounded`],
///    backlog §12), or sleepy (its lineage's rest hour and not hungry), a flyer aloft
///    descends (`lift` −1, steering as it likes) and a perched one cannot take off.
///    Grounded by the weather and awake, it instead rises under cover it can reach
///    overhead ([`crate::flight::cover_overhead`]: a crown or an overhang, the underside
///    roost), and it does not sip. Asleep it does nothing else (it sleeps only perched).
/// 3. **Perched**: `lift` up (down from an underside) or `forward` takes off, paying
///    `takeoff_s` of flight out of the reserve; otherwise it may turn where it is, and sip.
/// 4. **Aloft**: it pays `(factor − 1) ×` basal for the second beyond the upkeep it has
///    paid (`factor` the hover's at forward 0, the cruise's at full forward, linear
///    between; × `1 + rain / R₀` in rain), moves ([`crate::flight::move_through_air`]),
///    pays `climb_work_per_m × body ×` the height it gained, and lands on what it met.
/// 5. **A sip** ([`flyer_sip`]): on the due tick, `feed` with a bloom in reach while
///    hovering (forward 0) or perched.
fn flyer_act(
    fauna: &mut Fauna,
    view: &VoxelView<'_>,
    flora: &mut Flora,
    sky: &Sky,
    flight: crate::Flight,
    i: usize,
    hunters: &[usize],
) {
    use crate::flight;
    use crate::{MobilityMode, PerchKind};
    let founder = fauna.animals[i].founder.expect("a founder body");
    let phys = *fauna.config.founder(founder);
    let manifest = fauna.config.body_manifest(founder, fauna.animals[i].body);
    let sc = phys.core;
    fauna.ledger.founder_ticks_by_founder[founder.index()] += 1;
    let (asleep, mut held, lift_held, due) = {
        let a = &fauna.animals[i];
        (
            a.state == State::Asleep,
            a.founder_state.held,
            a.founder_state.lift,
            a.age_ticks % founder.manifest().cadence_ticks() == 0,
        )
    };
    {
        let pc = &fauna.config.predation;
        let a = &fauna.animals[i];
        held.forward *= (1.0 - pc.wound_slow * a.wound).clamp(0.0, 1.0);
    }
    let shape = flight::Shape::of(&phys, &flight, fauna.animals[i].body);
    let basal = sc.maintenance_per_s * fauna.animals[i].body;
    let takeoff_cost = flight.takeoff_s * flight.flight_factor * basal;
    let sleepy = off_phase(fauna, i, sky.clock_on, sky.daylight) && !fauna.animals[i].emergency;
    // Backlog §12: a downpour, or rain over its threshold (the latch), grounds it.
    let weather = sky.downpour || fauna.animals[i].mobility.rain_grounded;
    let grounded = weather || sleepy;
    // JUVENILE-FOOD: a takeoff is paid like every other motor cost — out of the reserve,
    // then out of the body above its death line (`body_min`). Gated on the reserve alone, a
    // growing bellwing, whose growth spends its reserve down to nothing on the perch
    // overnight, woke hungry and could never take off again: it sat perched beside full
    // blooms until it starved (s1 minute 120: 3045 of ~3200 juvenile samples perched,
    // eating latch and emergency on, stomach empty, lift up, never airborne).
    let can_pay = |a: &Animal| a.fuel + a.reserve + (a.body - sc.body_min).max(0.0) >= takeoff_cost;
    let puddle = phys.drown_depth_m(&phys.body_at(fauna.animals[i].body));
    // FLUSH (HUNT-ESCAPE, `crate::flush`): a hunter sensed coming sends it off in a burst.
    let escape = {
        let v = view.config.voxel_m;
        let ring = f64::from(view.config.width) * v;
        let calm = asleep || grounded || fauna.animals[i].mobility.falling;
        let mut escape = crate::flush::step(fauna, i, &flight, hunters, calm, v, ring);
        if escape.is_some_and(|e| e.takeoff) && fauna.animals[i].mobility.mode == crate::MobilityMode::Perched {
            if can_pay(&fauna.animals[i]) {
                flyer_takeoff(fauna, i, takeoff_cost);
            } else {
                // Too spent to take off: it stays where the strike finds it.
                fauna.flushes.remove(&fauna.animals[i].id);
                escape = None;
            }
        }
        escape
    };

    // 1. The perch.
    if fauna.animals[i].mobility.mode == MobilityMode::Perched {
        let holds = flight::perch_holds(view, &flora.view(), &fauna.animals[i], shape, puddle);
        if !holds {
            if !asleep && !grounded && can_pay(&fauna.animals[i]) {
                flyer_takeoff(fauna, i, takeoff_cost);
            } else {
                let m = &mut fauna.animals[i].mobility;
                m.mode = MobilityMode::Fly;
                m.perch = None;
                m.falling = true;
                fauna.ledger.drops += 1;
            }
        }
    }
    let (mode, falling) = {
        let m = &fauna.animals[i].mobility;
        (m.mode, m.falling)
    };
    if asleep && !falling {
        return;
    }

    // 2. The controls, and what the sky and the hour force.
    let (mut forward, mut turn, mut lift) = (held.forward.max(0.0), held.turn, lift_held);
    // A flush's burst moves at its own speed and climb (× cruise); otherwise 1 and the lift's.
    let (mut speed, mut climb) = (1.0, flight.lift_speed_fraction);
    if let Some(e) = escape {
        (forward, turn, lift) = (1.0, 0.0, 1.0);
        (speed, climb) = (e.speed, e.climb);
        held.feed = 0.0;
        fauna.animals[i].pose.heading_rad = e.heading;
    }
    if falling {
        (forward, turn, lift) = (0.0, 0.0, -1.0);
    } else if grounded {
        // Grounded by the weather (awake), it rises to hang under cover it can reach
        // overhead; anything else brings it down onto what is below.
        let a = &fauna.animals[i];
        lift = if weather
            && !sleepy
            && mode == MobilityMode::Fly
            && flight::cover_overhead(view, &flora.view(), shape, flight.band_max_m, &a.pose)
        {
            1.0
        } else {
            -1.0
        };
    }

    // 3. Perched.
    if mode == MobilityMode::Perched {
        let kind = fauna.animals[i].mobility.perch.map_or(PerchKind::Face, |p| p.kind);
        let wants = match kind {
            PerchKind::Underside => lift < 0.0 || forward > 0.0,
            _ => lift > 0.0 || forward > 0.0,
        };
        if wants && !grounded && can_pay(&fauna.animals[i]) {
            flyer_takeoff(fauna, i, takeoff_cost);
        } else {
            let a = &mut fauna.animals[i];
            a.pose.heading_rad = (a.pose.heading_rad + turn * manifest.yaw_cap_rad_per_s * DT)
                .rem_euclid(std::f64::consts::TAU);
            // On a face it stands on the top of whatever puddle is there.
            if let Some(crate::Perch {
                anchor: crate::PerchAnchor::Face(site),
                ..
            }) = a.mobility.perch
            {
                a.pose.y_m = crate::surface_m(site.y, view.config.voxel_m)
                    + view.standing_depth_m(i64::from(site.x), site.y, site.z);
            }
            // Grounded by the weather it does not sip (backlog §12).
            let fed = due
                && held.feed > 0.0
                && !weather
                && flyer_sip(fauna, i, view, flora, &manifest, &phys, shape, held.feed);
            fauna.animals[i].state = if fed { State::Cropping } else { State::Resting };
            return;
        }
    }

    // 4. Aloft: the second's cost beyond basal, then the motion, then the lift work.
    let (mut motor_paid, mut motor_body) = (0.0, 0.0);
    if !falling {
        let factor = flight.hover_factor + (flight.flight_factor - flight.hover_factor) * forward;
        let shower = if flight.shower_rain_reference_m_per_s > 0.0 {
            1.0 + sky.rain_m_per_s / flight.shower_rain_reference_m_per_s
        } else {
            1.0
        };
        let cost = (factor - 1.0).max(0.0) * basal * DT * shower * speed;
        let (paid, body) = respire(fauna, i, cost, Respiration::Motor);
        fauna.ledger.respired_flight_out += paid;
        motor_paid += paid;
        motor_body += body;
    }
    let cruise = manifest.cruise_m_per_s;
    let moved = {
        let fv = flora.view();
        let a = &mut fauna.animals[i];
        a.pose.heading_rad = (a.pose.heading_rad + turn * manifest.yaw_cap_rad_per_s * DT)
            .rem_euclid(std::f64::consts::TAU);
        let slow = if flight::inside_foliage(view, &fv, shape, a.pose.x, a.pose.z, a.pose.y_m) {
            flight.foliage_speed_factor
        } else {
            1.0
        };
        let distance = forward * speed * cruise * slow * DT;
        let dy = lift * climb * cruise * DT;
        flight::float_on_water(view, shape, &mut a.pose);
        let moved = flight::move_through_air(view, &fv, shape, flight.band_max_m, puddle, &mut a.pose, distance, dy);
        if let Some(site) = flight::support_below(view, a.pose.x, a.pose.z, a.pose.y_m) {
            a.site = site;
        }
        if let Some(p) = moved.landed {
            a.mobility.mode = MobilityMode::Perched;
            a.mobility.perch = Some(p);
            a.mobility.falling = false;
        } else if moved.on_water {
            // BW-STUCK: a drop onto water too deep to stand in is over: it hovers on the
            // surface, a flyer aloft with its own controls back (it never perches there).
            a.mobility.falling = false;
        }
        let fb = &mut a.founder_state.feedback;
        fb.attempted_equivalent += forward * cruise * DT;
        fb.delivered_equivalent += moved.distance_m;
        fb.delivered_forward += moved.distance_m;
        moved
    };
    if let Some(p) = moved.landed {
        fauna.ledger.landings_by_perch[p.kind.index()] += 1;
        // A flush that runs into a face ends there.
        if escape.is_some() {
            fauna.flushes.remove(&fauna.animals[i].id);
        }
    }
    if !falling && moved.rise_m > 0.0 {
        let work = flight.climb_work_per_m * fauna.animals[i].body * moved.rise_m;
        let (paid, body) = respire(fauna, i, work, Respiration::Motor);
        fauna.ledger.respired_flight_out += paid;
        motor_paid += paid;
        motor_body += body;
    }
    {
        let fb = &mut fauna.animals[i].founder_state.feedback;
        fb.structural_loss += motor_body;
        fb.motor_respiration += motor_paid;
    }

    // 5. A sip, hovering or perched.
    let hovering = forward <= 0.0 || fauna.animals[i].mobility.mode == MobilityMode::Perched;
    let fed = due
        && !falling
        && held.feed > 0.0
        && hovering
        && !weather
        && flyer_sip(fauna, i, view, flora, &manifest, &phys, shape, held.feed);
    fauna.animals[i].state = if fed {
        State::Cropping
    } else if moved.distance_m > 0.0 || moved.rise_m != 0.0 {
        State::Walking
    } else {
        State::Resting
    };
}

/// A paid takeoff: `takeoff_s` of flight out of the reserve (the caller checked it holds
/// it), and the body is aloft where it was.
fn flyer_takeoff(fauna: &mut Fauna, i: usize, cost: f64) {
    let (paid, body) = respire(fauna, i, cost, Respiration::Motor);
    fauna.ledger.respired_flight_out += paid;
    fauna.ledger.takeoffs += 1;
    let a = &mut fauna.animals[i];
    a.founder_state.feedback.motor_respiration += paid;
    a.founder_state.feedback.structural_loss += body;
    a.mobility.mode = crate::MobilityMode::Fly;
    a.mobility.perch = None;
    a.mobility.falling = false;
}

/// **A sip** (F1, §3.7): the gut's bite ([`gut_bite`]) out of the nearest bloom within the
/// mouth's reach (the body's radius plus its mouth reach, from its middle) — a
/// bloomcrown's nectar through [`Flora::take_stand_nectar`], a latticevine face's through
/// [`Flora::take_nectar`] — into the stomach's nectar pool, and the bloom recorded on the
/// body (the pollen seam). A sip at another bloom than the last is a pollinator's visit,
/// booked to it ([`Flora::visit_stand`], [`Flora::visit_face`]). Only while the eating
/// latch is on.
#[allow(clippy::too_many_arguments)]
fn flyer_sip(
    fauna: &mut Fauna,
    i: usize,
    view: &VoxelView<'_>,
    flora: &mut Flora,
    manifest: &crate::Manifest,
    phys: &crate::FounderPhysiology,
    shape: crate::flight::Shape,
    effort: f64,
) -> bool {
    if !fauna.animals[i].eating {
        return false;
    }
    let founder = fauna.animals[i].founder.expect("a founder body");
    let reach = shape.r + phys.body_at(fauna.animals[i].body).mouth_reach_m;
    let p = crate::flight::mouth_point(&fauna.animals[i], shape);
    let Some((bloom, stock)) = crate::flight::bloom_in_reach(view, &flora.view(), p, reach) else {
        fauna.ledger.sip_misses_by_founder[founder.index()] += 1;
        return false;
    };
    forage::note_bite(fauna, i, phys, Food::Nectar, manifest.controller_period_s, effort, stock, None);
    let bite = gut_bite(&fauna.animals[i], phys, Food::Nectar, manifest.controller_period_s, effort, stock);
    if !(bite > 0.0) {
        return false;
    }
    // VINE-C (Wrysk 2026-10-09, option C): a sip at a vine draws on every open flower in the
    // mouth's reach, each in proportion to what it holds (the stock read above is their sum).
    let sipped: Vec<cubarium_voxel_flora::Face> = match bloom {
        crate::Bloom::Stand(_) => Vec::new(),
        crate::Bloom::Vine(_) => crate::flight::vine_patch(view, &flora.view(), p, reach, cubarium_voxel_flora::SpurPhase::Flower)
            .into_iter()
            .map(|r| r.face)
            .collect(),
    };
    let taken = match bloom {
        crate::Bloom::Stand(site) => flora.take_stand_nectar(site, bite),
        crate::Bloom::Vine(_) => flora.take_nectar_patch(&sipped, bite),
    };
    let Some(taken) = taken else {
        return false;
    };
    let taken = fauna.book_eaten(taken, Food::Nectar);
    let l = &mut fauna.ledger;
    l.bites_by_founder[founder.index()] += 1;
    l.bites_by_founder_food[founder.index()][Food::Nectar.index()] += 1;
    l.sips_by_bloom[match bloom {
        crate::Bloom::Stand(_) => 0,
        crate::Bloom::Vine(_) => 1,
    }] += 1;
    // FLOWER-FRUIT (Wrysk 2026-10-09): a sip at a bloom other than the last it sipped at is
    // a pollinator's visit — it brings pollen from elsewhere — booked to the flowers it
    // came to, where the visits decide the fruit they set.
    //
    // VINE-C: at a vine the visit pollinates every open flower the body brushes — within
    // half its span (its wing disc) of where its mouth meets the wall — and every flower it
    // drank from.
    if fauna.animals[i].last_bloom != Some(bloom) {
        match bloom {
            crate::Bloom::Stand(site) => {
                flora.visit_stand(site);
            }
            crate::Bloom::Vine(face) => {
                for f in crate::flight::pollinated_faces(view, &flora.view(), p, face, shape.r, &sipped) {
                    flora.visit_face(f);
                }
            }
        }
    }
    fauna.animals[i].last_bloom = Some(bloom);
    swallow(fauna, i, Food::Nectar, taken);
    true
}

/// The jaw's point: the body's nose, `jaw_forward_length_fraction` of its length ahead of
/// its centre (half, for the lanternjaw).
pub(crate) fn jaw_of(x: f64, z: f64, heading: f64, length_m: f64, forward_fraction: f64) -> (f64, f64) {
    (
        x + forward_fraction * length_m * heading.sin(),
        z + forward_fraction * length_m * heading.cos(),
    )
}

/// The horizontal distance from `(px, pz)` to the segment `a → b`, `x` on the ring.
fn segment_distance(a: (f64, f64), b: (f64, f64), p: (f64, f64), ring: f64) -> f64 {
    let (bx, bz) = crate::pose::wrapped_delta(a.0, a.1, b.0, b.1, ring);
    let (px, pz) = crate::pose::wrapped_delta(a.0, a.1, p.0, p.1, ring);
    let len2 = bx * bx + bz * bz;
    let t = if len2 > 0.0 {
        ((px * bx + pz * bz) / len2).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let (dx, dz) = (px - t * bx, pz - t * bz);
    (dx * dx + dz * dz).sqrt()
}

/// Whether `prey`'s cylinder meets a capsule of radius `radius` along `jaw_from → jaw_to`
/// over the striker's jaw band (predation §2.3).
#[allow(clippy::too_many_arguments)]
fn meets_capsule(
    config: &crate::FaunaConfig,
    view: &VoxelView<'_>,
    striker: &Animal,
    geometry: &crate::Body,
    h: &crate::Hunter,
    jaw_from: (f64, f64),
    jaw_to: (f64, f64),
    prey: &Animal,
) -> bool {
    let Some(pg) = crate::body_of(config, prey) else {
        return false;
    };
    let v = view.config.voxel_m;
    let ring = f64::from(view.config.width) * v;
    let reach = geometry.footprint_radius() + pg.footprint_radius();
    if segment_distance(jaw_from, jaw_to, (prey.pose.x, prey.pose.z), ring) > reach {
        return false;
    }
    in_strike_band(view, striker, geometry, h, prey, &pg) || lunge_start_band(config, view, striker, geometry, h, prey, &pg)
}

/// **The lunge sweeps its band** (LJ-HUNT, [`crate::HuntConfig::lunge_sweeps_band`]): a
/// lanternjaw's lunge that stepped up or down a ledge on the way also meets a prey in the band
/// over the face it began on — the support face of its start column nearest the one it stands
/// on now, within its step limits. False off the switch, for another lineage, a perched
/// striker, or no lunge.
fn lunge_start_band(
    config: &crate::FaunaConfig,
    view: &VoxelView<'_>,
    striker: &Animal,
    geometry: &crate::Body,
    h: &crate::Hunter,
    prey: &Animal,
    pg: &crate::Body,
) -> bool {
    let Some(s) = striker.strike else { return false };
    if !config.hunt.lunge_sweeps_band
        || striker.founder != Some(crate::Founder::Lanternjaw)
        || s.phase != StrikePhase::Lunge
        || striker.mobility.mode == crate::MobilityMode::Perched
    {
        return false;
    }
    let v = view.config.voxel_m;
    let Some((cx, cz)) = (crate::Pose { x: s.from_x, z: s.from_z, ..striker.pose }).column(v, view.config.depth) else {
        return false;
    };
    let limits = config.founder(crate::Founder::Lanternjaw).step_limits(v);
    // It stepped down at most `limits.down` from where it began, or up at most `limits.up`.
    match view.nearest_support(cx.rem_euclid(i64::from(view.config.width)), cz, striker.site.y, limits.up, limits.down) {
        Some(y) if y != striker.site.y => band_meets(view, crate::surface_m(y, v) + striker.wall_offset_m(), geometry, h, prey, pg),
        _ => false,
    }
}

/// The base of `striker`'s jaw band, metres: a perched body's pose height, a walker's
/// support face's top (plus its wall offset).
fn band_base(view: &VoxelView<'_>, striker: &Animal) -> f64 {
    if striker.mobility.mode == crate::MobilityMode::Perched {
        striker.pose.y_m
    } else {
        crate::surface_m(striker.site.y, view.config.voxel_m) + striker.wall_offset_m()
    }
}

/// The strike capsule's height test ([`meets_capsule`]): the band from the striker's base up
/// its `strike_height_m` meets the prey's cylinder (`pg`, its body), its base on its support
/// face, or at its pose off the ground.
pub(crate) fn in_strike_band(
    view: &VoxelView<'_>,
    striker: &Animal,
    geometry: &crate::Body,
    h: &crate::Hunter,
    prey: &Animal,
    pg: &crate::Body,
) -> bool {
    band_meets(view, band_base(view, striker), geometry, h, prey, pg)
}

/// Whether the jaw band over the base `s0` (metres) meets `prey`'s cylinder (`pg`, its body).
fn band_meets(view: &VoxelView<'_>, s0: f64, geometry: &crate::Body, h: &crate::Hunter, prey: &Animal, pg: &crate::Body) -> bool {
    let v = view.config.voxel_m;
    let band = (s0, s0 + h.strike_height_m(geometry));
    // A body off the ground (perched, flying, climbing, gliding) has its base at its
    // pose's height; a walker's is its support face's top.
    let p0 = if matches!(
        prey.mobility.mode,
        crate::MobilityMode::Perched | crate::MobilityMode::Fly | crate::MobilityMode::Climb | crate::MobilityMode::Glide
    ) && prey.pose.y_m.is_finite()
    {
        prey.pose.y_m
    } else {
        crate::surface_m(prey.site.y, v) + prey.wall_offset_m()
    };
    let body = (p0, p0 + pg.height_m);
    body.0 < band.1 && band.0 < body.1
}

/// The prey whose cylinder meets the **strike capsule** now — forward from the jaw by
/// `strike_reach` over the jaw band — nearest the jaw first; `None` when there is none
/// (predation §2.3). The prey list and the size window are [`crate::is_prey_of`]'s.
pub(crate) fn prey_in_capsule(
    fauna: &Fauna,
    i: usize,
    view: &VoxelView<'_>,
    geometry: &crate::Body,
    h: &crate::Hunter,
) -> Option<u64> {
    let a = &fauna.animals[i];
    if a.mobility.climb.is_some()
        || (a.founder == Some(crate::Founder::Seedporter) && fauna.night.since_set_h.is_finite()) {
        return None;
    }
    let ring = f64::from(view.config.width) * view.config.voxel_m;
    let jaw = jaw_of(
        a.pose.x,
        a.pose.z,
        a.pose.heading_rad,
        geometry.length_m,
        h.jaw_forward_length_fraction,
    );
    let reach = h.strike_reach_length_fraction * geometry.length_m;
    let tip = (
        jaw.0 + reach * a.pose.heading_rad.sin(),
        jaw.1 + reach * a.pose.heading_rad.cos(),
    );
    // The pack seam (R3): a pack member's window is its nearby adult packmates' body.
    let window = crate::pack::window_body(fauna, i, ring);
    // D2b: a patroller strikes only a prey its attack choice accepts this window.
    let choosy = fauna.config.hunt.patrol && (a.founder == Some(crate::Founder::Lanternjaw) && fauna.controllers.layer(a.id) == crate::Layer::Heuristic);
    // VH-TACTICS (§11.4): a tactics hound strikes its target, or a young body it substitutes.
    let rule = crate::pack::tactics::strike_rule(fauna, i);
    let mut best: Option<(f64, u64)> = None;
    let mut chosen: Option<(f64, u64)> = None;
    let mut projected = None;
    for o in &fauna.animals {
        if o.id == a.id
            || o.mobility.climb.is_some()
            || !crate::predation::is_prey_in_window(&fauna.config, a, o, window)
            || !meets_capsule(&fauna.config, view, a, geometry, h, jaw, tip, o)
            || (choosy && fauna.config.hunt.strike_chosen_only && !crate::hunt::accepts(&fauna.config, a, o, fauna.tick))
            || rule.as_ref().is_some_and(|r| !r.allows(o.id))
        {
            continue;
        }
        // Check the same delivered sweep as resolve. Predict once, without changing
        // the world or paying a strike cost; prey can still move during the windup.
        let (end, from, to) = projected.get_or_insert_with(|| {
            let mut end = *a;
            let phys = fauna.config.founder(a.founder.expect("a founder body"));
            end.strike = Some(Strike { phase: StrikePhase::Lunge, ticks_left: crate::Hunter::ticks(h.lunge_s), prey: o.id, from_x: a.pose.x, from_z: a.pose.z, lure: false });
            for _ in 0..crate::Hunter::ticks(h.lunge_s) {
                advance_lunge(&fauna.config, view, &mut end, phys, h, geometry);
            }
            let (from, to) = lunge_jaws(&end, geometry, h, (a.pose.x, a.pose.z));
            (end, from, to)
        });
        if !meets_capsule(&fauna.config, view, end, geometry, h, *from, *to, o) {
            continue;
        }
        let (dx, dz) = crate::pose::wrapped_delta(jaw.0, jaw.1, o.pose.x, o.pose.z, ring);
        let d = dx * dx + dz * dz;
        if rule.as_ref().is_some_and(|r| r.target == Some(o.id)) {
            chosen = Some((d, o.id));
        }
        if best.is_none_or(|(b, _)| d < b) {
            best = Some((d, o.id));
        }
    }
    // Its target first; else the nearest substitute.
    chosen.or(best).map(|(_, id)| id)
}

/// **Bodies block** (VH-TACTICS, §11.4): the first body a lunge's jaw sweep from `from` to
/// `to` meets, if it is not `prey` and lies before it along the sweep — an adult standing
/// between a hound and a calf. `None`: nothing comes before the prey.
fn first_in_sweep(config: &crate::FaunaConfig, view: &VoxelView<'_>, fauna: &Fauna, a: &Animal, geometry: &crate::Body, h: &crate::Hunter, from: (f64, f64), to: (f64, f64), prey: &Animal) -> Option<Animal> {
    let ring = f64::from(view.config.width) * view.config.voxel_m;
    let (sx, sz) = crate::pose::wrapped_delta(from.0, from.1, to.0, to.1, ring);
    let len2 = sx * sx + sz * sz;
    let along = |o: &Animal| {
        let (ox, oz) = crate::pose::wrapped_delta(from.0, from.1, o.pose.x, o.pose.z, ring);
        if len2 > 0.0 { (ox * sx + oz * sz) / len2 } else { 0.0 }
    };
    let at = along(prey);
    fauna
        .animals
        .iter()
        .filter(|o| o.id != a.id && o.id != prey.id && o.attached.is_none() && o.mobility.climb.is_none())
        .filter(|o| meets_capsule(config, view, a, geometry, h, from, to, o))
        .map(|o| (along(o), *o))
        // Ahead of where the jaw's sweep began, short of the prey.
        .filter(|(s, _)| *s > 0.0 && *s < at)
        .min_by(|x, y| x.0.total_cmp(&y.0).then(x.1.id.cmp(&y.1.id)))
        .map(|(_, o)| o)
}

/// Advance one lunge tick, on a copy when checking a prospective strike. The
/// terrain-constrained motion is shared with resolution so a nominal reach through
/// a wall or across an unsteppable face cannot start an endless series of misses.
fn advance_lunge(
    config: &crate::FaunaConfig,
    view: &VoxelView<'_>,
    a: &mut Animal,
    phys: &crate::FounderPhysiology,
    h: &crate::Hunter,
    geometry: &crate::Body,
) {
    let founder = a.founder.expect("a founder body");
    let manifest = config.manifest(founder);
    let reach = h.strike_reach_length_fraction * geometry.length_m;
    let per_tick = reach / f64::from(crate::Hunter::ticks(h.lunge_s));
    let lunge = Actions {
        forward: per_tick / (manifest.cruise_m_per_s * DT),
        turn: 0.0,
        feed: 0.0,
    };
    let mut standing_y = a.site.y;
    if a.mobility.mode != crate::MobilityMode::Perched {
        body::resolve_motion(
            view,
            &mut a.pose,
            &mut standing_y,
            &manifest,
            geometry,
            phys.wade_depth_m(geometry),
            phys.step_limits(view.config.voxel_m),
            config.life.wall_friction,
            lunge,
        );
    }
    if let Some((cx, cz)) = a.pose.column(view.config.voxel_m, view.config.depth) {
        a.site = Site {
            x: cx.rem_euclid(i64::from(view.config.width)) as u32,
            y: standing_y,
            z: cz,
        };
    }
}

/// The jaw sweep actually delivered by the lunge; a perched catch extends its
/// arms, while a ground catch uses the body's terrain-constrained endpoint.
fn lunge_jaws(
    a: &Animal,
    geometry: &crate::Body,
    h: &crate::Hunter,
    from: (f64, f64),
) -> ((f64, f64), (f64, f64)) {
    let heading = a.pose.heading_rad;
    let jaw_from = jaw_of(
        from.0,
        from.1,
        heading,
        geometry.length_m,
        h.jaw_forward_length_fraction,
    );
    let jaw_to = if a.mobility.mode == crate::MobilityMode::Perched {
        let reach = h.strike_reach_length_fraction * geometry.length_m;
        (
            jaw_from.0 + reach * heading.sin(),
            jaw_from.1 + reach * heading.cos(),
        )
    } else {
        jaw_of(
            a.pose.x,
            a.pose.z,
            heading,
            geometry.length_m,
            h.jaw_forward_length_fraction,
        )
    };
    (jaw_from, jaw_to)
}

/// One tick of a strike's **windup** or **lunge** (predation §2.3). The windup is a
/// crouch; at its end the strike's cost is paid, hit or miss, and the lunge begins. The
/// lunge moves the body forward `strike_reach` over its ticks under the ordinary step rule
/// — its own motion paid inside the strike cost — and on its last tick the jaw's swept
/// capsule is tested against the prey's cylinder **where the prey is now**: a miss by
/// geometry, or a contact, which the prey then dodges or takes as a [`Hit`] event.
#[allow(clippy::too_many_arguments)]
fn strike_tick(
    fauna: &mut Fauna,
    i: usize,
    view: &VoxelView<'_>,
    phys: &crate::FounderPhysiology,
    h: crate::Hunter,
    seed: u64,
    tick: u64,
    hits: &mut Vec<Hit>,
) {
    let founder = fauna.animals[i].founder.expect("a founder body");
    let mut s = fauna.animals[i].strike.expect("a strike in progress");
    fauna.animals[i].state = State::Resting;
    if s.phase == StrikePhase::Windup {
        s.ticks_left = s.ticks_left.saturating_sub(1);
        if s.ticks_left > 0 {
            fauna.animals[i].strike = Some(s);
            return;
        }
        // The lunge: paid now, whatever it meets.
        let body = fauna.animals[i].body;
        let cost = h.strike_cost_s * phys.core.maintenance_per_s * body;
        respire(fauna, i, cost, Respiration::Hunt);
        // FINDINGS-FIX-2: the lure draw at the lunge, before its loom closes the visit.
        let lure = s.lure || crate::light::drawn_to_lure(fauna, s.prey);
        let a = &fauna.animals[i];
        s = Strike {
            phase: StrikePhase::Lunge,
            ticks_left: crate::Hunter::ticks(h.lunge_s),
            from_x: a.pose.x,
            from_z: a.pose.z,
            lure,
            ..s
        };
    }
    // One lunge tick: `strike_reach / lunge ticks` forward, turn held still.
    let geometry = phys.body_at(fauna.animals[i].body);
    advance_lunge(&fauna.config, view, &mut fauna.animals[i], phys, &h, &geometry);
    s.ticks_left = s.ticks_left.saturating_sub(1);
    if s.ticks_left > 0 {
        fauna.animals[i].strike = Some(s);
        return;
    }
    // Contact: the jaw's sweep from where the lunge began to where it ended.
    let a = fauna.animals[i];
    let (jaw_from, jaw_to) = lunge_jaws(&a, &geometry, &h, (s.from_x, s.from_z));
    let prey = fauna
        .animals
        .binary_search_by_key(&s.prey, |p| p.id)
        .ok()
        .map(|j| fauna.animals[j]);
    // VH-TACTICS (§11.4): a lunge contacts the first body along its path. A body before the
    // prey that the strike takes (its list and window) takes it; any other blocks it.
    let mut blocked = false;
    let prey = match prey {
        Some(p) if h.lunge_blocks && meets_capsule(&fauna.config, view, &a, &geometry, &h, jaw_from, jaw_to, &p) => {
            match first_in_sweep(&fauna.config, view, fauna, &a, &geometry, &h, jaw_from, jaw_to, &p) {
                Some(b) => {
                    let ring = f64::from(view.config.width) * view.config.voxel_m;
                    let window = crate::pack::window_body(fauna, i, ring);
                    fauna.ledger.lunges_blocked += 1;
                    if crate::predation::is_prey_in_window(&fauna.config, &a, &b, window) {
                        Some(b)
                    } else {
                        blocked = true;
                        Some(p)
                    }
                }
                None => Some(p),
            }
        }
        p => p,
    };
    if let Some(p) = prey {
        let contact = !blocked
            && meets_capsule(&fauna.config, view, &a, &geometry, &h, jaw_from, jaw_to, &p)
            && (founder != crate::Founder::Seedporter || crate::is_prey_of(&fauna.config, &a, &p));
        if sp_miss_tap(&a, &p) {
            let pg = crate::body_of(&fauna.config, &p).unwrap();
            let horizontal = segment_distance(jaw_from, jaw_to, (p.pose.x, p.pose.z), f64::from(view.config.width) * view.config.voxel_m);
            eprintln!("SPMISS contact tick={} hunter={} prey={} contact={} fled={} lure={} distance={} radius={} band={} takeable={} ax={} az={} ay={} px={} pz={} py={} mode={:?} reach={} length={}", tick, a.id, p.id, contact, crate::flush::fled(fauna, p.id), s.lure, horizontal, geometry.footprint_radius() + pg.footprint_radius(), in_strike_band(view, &a, &geometry, &h, &p, &pg), crate::is_prey_of(&fauna.config, &a, &p), a.pose.x, a.pose.z, a.pose.y_m, p.pose.x, p.pose.z, p.pose.y_m, p.mobility.mode, h.strike_reach_length_fraction * geometry.length_m, geometry.length_m);
        }
        // FLUSH: a jaw that closed on air because its prey had burst away from it.
        if !contact && crate::flush::fled(fauna, p.id) {
            if let Some(pf) = p.founder {
                fauna.ledger.flush_escapes_by_founder[pf.index()] += 1;
            }
        }
        // HUNT-ESCAPE: a grown body with a counter-strike answers the strike that reached it.
        if contact {
            if let Some(answer) = counter_answer(&fauna.config, view, &a, jaw_to, &p) {
                hits.push(answer);
            }
        }
        if contact {
            let pf = p.founder.expect("a prey has a lineage");
            let pp = fauna.config.founder(pf);
            let ring = f64::from(view.config.width) * view.config.voxel_m;
            let (dx, dz) = crate::pose::wrapped_delta(p.pose.x, p.pose.z, a.pose.x, a.pose.z, ring);
            let (fx, fz) = p.pose.forward();
            let from_rear = dx * fx + dz * fz < 0.0;
            let cap = pp.core.reserve_of(p.body);
            let condition = if cap > 0.0 { p.reserve / cap } else { 0.0 };
            // D5 (S2): a live prey's awareness is whether its record holds the striker.
            let (awareness, aware) = {
                let record = fauna.percept(p.id);
                (
                    crate::vigil::awareness(&fauna.config, &p, record, a.id, tick),
                    crate::vigil::aware_of(&fauna.config, record, a.id, tick),
                )
            };
            if pp.senses.live && p.state != State::Asleep {
                if aware {
                    fauna.ledger.vigil.aware_strikes[pf.index()] += 1;
                } else {
                    fauna.ledger.vigil.unaware_strikes[pf.index()] += 1;
                }
            }
            // HUNKER: a hunkered prey neither dodges nor runs; its shield may turn the strike,
            // and turned over by the pack it has no armour at all.
            let guard = crate::vigil::guard(&fauna.config, &p, fauna.percept(p.id), tick);
            let (chance, armour) = match guard {
                crate::vigil::Guard::Open => {
                    (crate::dodge_chance_with(&fauna.config.predation, pp.agility, awareness, from_rear, condition), pp.armour)
                }
                crate::vigil::Guard::Shield(_) => (0.0, pp.armour),
                crate::vigil::Guard::Bared => (0.0, 0.0),
            };
            let glanced = match guard {
                // A keyed draw of its own, apart from the dodge's.
                crate::vigil::Guard::Shield(shield) => crate::dodges(shield, seed ^ GLANCE_KEY, a.id, p.id, tick),
                _ => false,
            };
            // HERD: a closed adult between the jaw and its calf obstructs the lunge.
            // Its own keyed draw avoids changing the existing shield/dodge draws.
            let exposure = crate::vigil::shelter_exposure(fauna, &p, &a, f64::from(view.config.width) * view.config.voxel_m, view.config.voxel_m);
            let covered = crate::dodges(1.0 - exposure, seed ^ 0x5348_454C_5445_5200, a.id, p.id, tick);
            if sp_miss_tap(&a, &p) {
                eprintln!("SPMISS defence tick={} hunter={} prey={} chance={} awareness={} agility={} aspect={} condition={} covered={} glanced={} dodged={}", tick, a.id, p.id, chance, awareness, pp.agility, from_rear, condition, covered, glanced, crate::dodges(chance, seed, a.id, p.id, tick));
            }
            if covered {
                // A missed reach: no damage, no body transfers, and normal recovery below.
            } else if glanced {
                fauna.ledger.vigil.glances[pf.index()] += 1;
            } else if crate::dodges(chance, seed, a.id, p.id, tick) {
                fauna.ledger.dodges_by_founder[founder.index()] += 1;
            } else {
                // Counted when `apply_hit` lands it on a live prey, not here.
                let damage = if p.body > 0.0 {
                    // A retracted amphibious body is closed (AMPHIBIOUS-SNAIL): slower
                    // to eat, not immune.
                    let closed = match pp.amphibious {
                        Some(am) if p.mobility.retract_ticks > 0 => 1.0 - am.retract_armour,
                        _ => 1.0,
                    };
                    h.strike_power * (phys.size(a.body) / pp.size(p.body)) * (1.0 - armour) * closed
                } else {
                    1.0
                };
                hits.push(Hit {
                    prey: p.id,
                    damage,
                    by: founder,
                    striker: a.id,
                    lure: s.lure,
                    counter: false,
                });
            }
        }
    }
    if prey.is_none() && founder == crate::Founder::Seedporter && sp_miss_enabled() {
        eprintln!("SPMISS missing tick={} hunter={} prey={} lure={}", tick, a.id, s.prey, s.lure);
    }
    fauna.animals[i].strike = Some(Strike {
        phase: StrikePhase::Recovery,
        ticks_left: crate::Hunter::ticks(h.recovery_s),
        ..s
    });
}

/// **The counter-strike** ([`crate::Counter`], HUNT-ESCAPE): the answer of a grown `prey`
/// with a counter to the strike of `striker` that reached it, its jaw ending at `jaw_to` —
/// a [`Hit`] on the striker, `damage × (S_prey / S_striker) × (1 − the striker's armour)`,
/// when that jaw is within the counter's reach of the prey's footprint. `None` for a
/// juvenile, a rider, a lineage with no counter, or a jaw out of its reach.
fn counter_answer(config: &crate::FaunaConfig, view: &VoxelView<'_>, striker: &Animal, jaw_to: (f64, f64), prey: &Animal) -> Option<Hit> {
    let pf = prey.founder?;
    // LS-DEFEND (§3.3, one kick, one budget): a lineage with a kick answers a landed bite
    // only through its kick-back ([`crate::kick::kick_back`]), never a second, free counter.
    if config.founder(pf).kick.is_some() {
        return None;
    }
    let c = config.founder(pf).counter?;
    let sf = striker.founder?;
    if prey.attached.is_some() || prey.body < crate::effective_config(config, prey).birth_body || !(striker.body > 0.0) {
        return None;
    }
    let pg = crate::body_of(config, prey)?;
    let ring = f64::from(view.config.width) * view.config.voxel_m;
    let (dx, dz) = crate::pose::wrapped_delta(prey.pose.x, prey.pose.z, jaw_to.0, jaw_to.1, ring);
    if (dx * dx + dz * dz).sqrt() > pg.footprint_radius() + c.reach {
        return None;
    }
    let armour = config.founder(sf).armour;
    let damage = c.damage * (config.founder(pf).size(prey.body) / config.founder(sf).size(striker.body)) * (1.0 - armour).clamp(0.0, 1.0);
    (damage.is_finite() && damage > 0.0).then_some(Hit { prey: striker.id, damage, by: pf, striker: prey.id, lure: false, counter: true })
}

/// The key the shield's glance draw is salted with, apart from the dodge's (a pure numeric).
const GLANCE_KEY: u64 = 0x6c61_6e63_6500_0001;

/// **One [`Hit`] applied** (predation §2.4, §2.5): the wound adds, the hit wakes the prey
/// and keeps it up for [`crate::PredationConfig::alert_s`], and a wound of 1 kills — the whole body to
/// carrion on its face, a carcass record on the pool, departed `Killed { by }`. A prey
/// that is gone already is left alone: it dies once.
pub(crate) fn apply_hit(fauna: &mut Fauna, flora: &mut Flora, prey: u64, damage: f64, by: Founder, view: Option<&VoxelView<'_>>) {
    if fauna.animals.binary_search_by_key(&prey, |a| a.id).is_err() {
        return;
    }
    // A hit is counted where it lands: on a prey still alive.
    fauna.ledger.hits_by_founder[by.index()] += 1;
    wound(fauna, flora, prey, damage, by, view);
}

/// **A counter-strike applied** ([`counter_answer`], HUNT-ESCAPE): the attacker `hit.prey`
/// takes the wound by the hit path ([`wound`]); the answering body `hit.striker`, if it
/// still lives, pays its counter's cost. Booked as a counter, not a hit.
fn apply_counter(fauna: &mut Fauna, flora: &mut Flora, hit: &Hit, view: Option<&VoxelView<'_>>) {
    if fauna.animals.binary_search_by_key(&hit.prey, |a| a.id).is_err() {
        return;
    }
    fauna.ledger.counters_by_founder[hit.by.index()] += 1;
    fauna.ledger.counter_wound_by_founder[hit.by.index()] += hit.damage;
    if let Ok(k) = fauna.animals.binary_search_by_key(&hit.striker, |a| a.id) {
        let phys = *fauna.config.founder(hit.by);
        if let Some(c) = phys.counter {
            let cost = c.cost * phys.core.maintenance_per_s * fauna.animals[k].body;
            respire(fauna, k, cost, Respiration::Hunt);
        }
    }
    wound(fauna, flora, hit.prey, hit.damage, hit.by, view);
}

/// The wound of a hit on the live body `prey` ([`apply_hit`]'s, less its booking).
fn wound(fauna: &mut Fauna, flora: &mut Flora, prey: u64, damage: f64, by: Founder, view: Option<&VoxelView<'_>>) {
    let Ok(j) = fauna.animals.binary_search_by_key(&prey, |a| a.id) else {
        return;
    };
    let alert_s = fauna.config.predation.alert_s;
    let harmed = damage.is_finite() && damage > 0.0;
    let young = crate::roam::young_of_mother(&fauna.config, &fauna.animals[j]);
    if let (true, Some(f)) = (harmed, fauna.animals[j].founder) {
        let t = &mut fauna.harm_tally;
        t.wounds[by.index()][f.index()] += 1;
        if young {
            t.young_wounds[by.index()][f.index()] += 1;
        }
    }
    let a = &mut fauna.animals[j];
    if harmed {
        a.wound += damage;
    }
    if a.state == State::Asleep {
        a.state = State::Resting;
        if let Some(f) = a.founder {
            fauna.ledger.sleep_flips_by_founder[f.index()] += 1;
        }
    }
    a.rest = None;
    a.alert_ticks = u32::try_from(cubarium_voxel::ticks_at_least_one(alert_s)).unwrap_or(u32::MAX);
    if a.wound >= 1.0 {
        if let (true, Some(f)) = (young, a.founder) {
            fauna.harm_tally.young_kills[by.index()][f.index()] += 1;
        }
        kill(fauna, flora, j, Departure::Killed { by }, view);
    }
}

/// A death by `cause` of the body at `j`, now: an open gestation is resolved first, then
/// the whole body goes to carrion on its face and the death is booked; a kill also lays
/// the carcass record.
fn kill(fauna: &mut Fauna, flora: &mut Flora, j: usize, cause: Departure, view: Option<&VoxelView<'_>>) {
    kill_into(fauna, flora, j, cause, view, None);
}

/// The same kill path; an immediate eater takes fresh corpse matter before it pools.
fn kill_into(fauna: &mut Fauna, flora: &mut Flora, j: usize, cause: Departure, view: Option<&VoxelView<'_>>, eater: Option<u64>) {
    let rule = body::effective_config(&fauna.config, &fauna.animals[j]).reproduction;
    fail_gestation(fauna, j, &rule);
    if fauna.animals[j].pack.is_some_and(|m| m.pup) {
        fauna.ledger.pups_lost_other += 1;
    }
    // S3: a kill of a prey drawn to a lure.
    if let (Departure::Killed { by }, Some(f)) = (cause, fauna.animals[j].founder) {
        if crate::light::drawn_to_lure(fauna, fauna.animals[j].id) {
            fauna.ledger.lure_kills[f.index()] += 1;
        }
        // S3b: a prey with a light sense still naive about the lure's class to its eye (cyan, or near-IR when the lure mimics a flower).
        if fauna.config.founder(f).senses.light_m > 0.0 && crate::light::naive(fauna, fauna.animals[j].id, f) {
            fauna.ledger.naive_kills[by.index()][f.index()] += 1;
        }
    }
    if let (Departure::Killed { by }, Some(Founder::Browser)) = (cause, fauna.animals[j].founder) {
        let prey = &fauna.animals[j];
        let posture = if prey.body < fauna.config.founder(Founder::Browser).core.birth_body {
            if fauna.world_geometry.is_some_and(|(ring, vm)| crate::vigil::sheltered(fauna, prey, ring, vm)) { 0 } else { 1 }
        } else if prey.hunkered { 2 } else if fauna.percept(prey.id).is_some_and(|p| p.vigil.running(fauna.tick)) { 3 } else { 4 };
        fauna.ledger.vigil.grazer_kills_by_posture[by.index()][posture] += 1;
    }
    sp_miss_depart(fauna, &fauna.animals[j], cause);
    let a = fauna.animals.remove(j);
    fauna.controllers.take(a.id);
    fauna.trapped.remove(&a.id);
    fauna.drop_plans.remove(&a.id);
    fauna.wading.remove(&a.id);
    fauna.flushes.remove(&a.id);
    fauna.free_riders(&a, view);
    if let (Departure::Killed { by }, Some(f)) = (cause, a.founder) {
        fauna.ledger.funnel[by.index()][f.index()][crate::perception::KILLED] += 1;
        if let Some(r) = &mut fauna.census_readout {
            r.predation.push(crate::CensusPredation { prey_id: a.id, predator: by, prey: f, prey_body: a.body, kill: true });
        }
        // HUNKER's readout: the kill by the prey's age class.
        if a.body < fauna.config.founder(f).core.birth_body {
            fauna.ledger.vigil.kills_juvenile[f.index()] += 1;
        } else {
            fauna.ledger.vigil.kills_adult[f.index()] += 1;
        }
    }
    let (laid, organic) = bury_corpse_into(fauna, flora, &a, cause, eater);
    if laid && matches!(cause, Departure::Killed { .. } | Departure::Groomed) {
        if let Some(f) = a.founder {
            flora.lay_carcass(
                a.site,
                cubarium_voxel_flora::Carcass {
                    lineage: f.index() as u8,
                    heading_rad: a.pose.heading_rad,
                    organic,
                },
            );
        }
    }
}

/// The whole animal to carrion on its face, and the death booked. Whether the plant
/// layer took the corpse.
fn bury_corpse(fauna: &mut Fauna, flora: &mut Flora, a: &Animal, cause: Departure) -> bool {
    bury_corpse_into(fauna, flora, a, cause, None).0
}

fn bury_corpse_into(fauna: &mut Fauna, flora: &mut Flora, a: &Animal, cause: Departure, eater: Option<u64>) -> (bool, f64) {
    // The whole animal, whatever is left of it: a corpse with no organic matter left is
    // still a corpse with mineral in it, and keeping an inert remainder on a dead animal
    // is how a ledger stops closing.
    // The stomach's raw food goes with it, as raw matter: no carcass yield of its own.
    let fed = a.stomach.total();
    let mut corpse = corpse_contents(a);
    if let Some(i) = eater.and_then(|id| fauna.animals.binary_search_by_key(&id, |a| a.id).ok()) {
        let f = fauna.animals[i].founder.expect("a corpse eater");
        let room = (fauna.config.founder(f).gut.capacity(fauna.animals[i].body) - fauna.animals[i].stomach.organic()).max(0.0);
        let taken = take_from_pool(&mut corpse, room);
        if taken.organic > 0.0 {
            // Internal body-to-stomach transfer, not an intake across the flora boundary.
            fauna.ledger.bites += 1;
            fauna.ledger.bites_by_food[Food::Carrion.index()] += 1;
            fauna.animals[i].bite = crate::LastBite(Some(Site { y: a.site.y + 1, ..a.site }));
            if let Some(prey) = a.founder {
                fauna.ledger.eaten_carrion_by_founder_prey[f.index()][prey.index()] += taken.organic;
            }
            let taken = ingest_bite(fauna, i, flora, Food::Carrion, taken);
            fauna.ledger.eaten_carrion_by_founder[f.index()] += taken.organic;
        }
    }
    let laid = fauna.book_deposit(
        flora,
        a.site,
        DepositKind::Carrion,
        corpse,
    );
    let l = &mut fauna.ledger;
    l.stomach_out.organic += fed.organic;
    l.stomach_out.mineral += fed.mineral;
    l.stomach_out.energy += fed.energy;
    fauna.ledger.deaths += 1;
    fauna.book_departure(a, cause);
    (laid, corpse.organic)
}

/// What a blocked climber did with the face in front of it.
enum Attach {
    /// It is on the face now.
    Climbing,
    /// The way along the face was shut (an overhang, a roof, no room at the far end): it
    /// turned round.
    TurnedBack,
    /// There was no face: it stays blocked, as any body does.
    Nothing,
}

/// The axis step `(dx, dz)` nearest a heading, and the heading that points exactly along
/// it (`Pose::forward` is `(sin h, cos h)`).
fn axis_of(heading: f64) -> ((i64, i64), f64) {
    let (fx, fz) = (heading.sin(), heading.cos());
    let dir = if fx.abs() >= fz.abs() {
        (if fx >= 0.0 { 1 } else { -1 }, 0)
    } else {
        (0, if fz >= 0.0 { 1 } else { -1 })
    };
    (dir, heading_of(dir))
}

/// The heading that points exactly along an axis step.
fn heading_of(dir: (i64, i64)) -> f64 {
    match dir {
        (1, 0) => std::f64::consts::FRAC_PI_2,
        (-1, 0) => 3.0 * std::f64::consts::FRAC_PI_2,
        (0, 1) => 0.0,
        _ => std::f64::consts::PI,
    }
}

/// The coordinate, along the axis of `dir`, of the plane between column `col` and the
/// next one along `dir`, and the pose coordinate `r` beyond it on side `side` (`1` past
/// the plane in `dir`, `-1` short of it).
fn beside_plane(col: i64, dir: i64, voxel_m: f64, r: f64, side: f64) -> f64 {
    let plane = if dir > 0 {
        (col + 1) as f64
    } else {
        col as f64
    } * voxel_m;
    // A hair more than the radius, so the disc is tangent to the face and never in it.
    plane + side * dir as f64 * r * (1.0 + 1e-9)
}

/// **Attach to a face**. The body's sweep was refused
/// while it held forward effort; if the column ahead along its heading's axis is a wall
/// taller than its step ([`body::wall_ascent`]) or a cliff deeper than its step down
/// ([`body::wall_descent`]) and the body is pressed against it, it goes onto the face:
/// heading snapped into the face, pose tangent to it (hanging beside the cliff for a
/// descent), and a [`crate::WallClimb`] on the body. Its `site` stays where it stood.
///
/// The way along the face is checked **whole, here**, for the column (overhang, roof,
/// room at the end) and for the body's disc over every layer it will pass; a shut way
/// turns the body round (the brief's overhang rule). Water at the far end deeper than it
/// wades refuses the face the way it refuses a step, without a turn. An open way is
/// climbed on the following ticks by [`climb_tick`].
fn attach(
    fauna: &mut Fauna,
    i: usize,
    view: &VoxelView<'_>,
    phys: &crate::FounderPhysiology,
    geometry: &crate::Body,
    step_limits: crate::StepLimits,
) -> Attach {
    let c = view.config;
    let v = c.voxel_m;
    let a = fauna.animals[i];
    let r = geometry.footprint_radius();
    let headroom = geometry.headroom_voxels(v);
    let Some((cx, cz)) = a.pose.column(v, c.depth) else {
        return Attach::Nothing;
    };
    let cx = cx.rem_euclid(i64::from(c.width));
    let (dir, into) = axis_of(a.pose.heading_rad);
    // Pressed against it: the centre within a radius of the plane it faces.
    let (along, col) = if dir.0 != 0 {
        (a.pose.x, cx)
    } else {
        (a.pose.z, i64::from(cz))
    };
    let plane = beside_plane(col, if dir.0 != 0 { dir.0 } else { dir.1 }, v, 0.0, 0.0);
    let gap = if dir.0 != 0 {
        // x wraps: measure the short way round.
        let w = f64::from(c.width) * v;
        let d = (plane - along).rem_euclid(w);
        d.min(w - d)
    } else {
        (plane - along).abs()
    };
    if gap > r * (1.0 + 1e-3) {
        return Attach::Nothing;
    }
    let y = a.site.y;
    let up = body::wall_ascent(view, cx, y, cz, dir, headroom, step_limits.up);
    let (face, ascending) = match up {
        body::Face::Not => (
            body::wall_descent(view, cx, y, cz, dir, headroom, step_limits.down),
            false,
        ),
        other => (other, true),
    };
    let turn_back = |fauna: &mut Fauna| {
        let a = &mut fauna.animals[i];
        a.pose.heading_rad =
            (a.pose.heading_rad + std::f64::consts::PI).rem_euclid(std::f64::consts::TAU);
        Attach::TurnedBack
    };
    let to_y = match face {
        body::Face::Not => return Attach::Nothing,
        body::Face::Blocked => return turn_back(fauna),
        body::Face::Climb(to_y) => to_y,
    };
    let ax = (cx + dir.0).rem_euclid(i64::from(c.width));
    let az = (i64::from(cz) + dir.1) as u32;
    let to = Site {
        x: ax as u32,
        y: to_y,
        z: az,
    };
    // The body's own passage: its disc at the pose it climbs at, over every layer it
    // will pass, and at the pose it will stand in at the end.
    let s = if dir.0 != 0 { dir.0 } else { dir.1 };
    let hang_side = if ascending { -1.0 } else { 1.0 };
    let climb_at = beside_plane(col, s, v, r, hang_side);
    let end_at = beside_plane(col, s, v, r, 1.0);
    let at = |coord: f64| -> (f64, f64) {
        if dir.0 != 0 {
            (coord.rem_euclid(f64::from(c.width) * v), a.pose.z)
        } else {
            (a.pose.x, coord)
        }
    };
    let (px, pz) = at(climb_at);
    let (ex, ez) = at(end_at);
    let (lo, hi) = (y.min(to_y) + 1, (y.max(to_y) + headroom).min(c.height - 1));
    let end_layers = to_y + 1..=(to_y + headroom).min(c.height - 1);
    if !body::disc_clear(view, px, pz, r, lo..=hi) || !body::disc_clear(view, ex, ez, r, end_layers)
    {
        return turn_back(fauna);
    }
    // Water at the far end deeper than it wades is refused the way a step into it is:
    // the body stays where it is and does not turn on its own.
    if view.water_depth_m(ax, to_y, az) > phys.wade_depth_m(geometry) {
        return Attach::Nothing;
    }
    // A water-bound body (roster R2b) climbs only to a face with standing water within
    // its margin, refused as a step there would be (a stranded body is not held).
    if let Some(rule) = body::WaterRule::of(phys).map(|mut r| {
        if a.mobility.thirsty {
            r.land_moisture_min = None;
        }
        r
    }) && rule.admits(view, cx, y, cz)
        && !rule.admits(view, ax, to_y, az)
    {
        return Attach::Nothing;
    }
    let a = &mut fauna.animals[i];
    a.pose.x = px;
    a.pose.z = pz;
    a.pose.heading_rad = if ascending {
        into
    } else {
        (into + std::f64::consts::PI).rem_euclid(std::f64::consts::TAU)
    };
    a.mobility.climb = Some(crate::WallClimb {
        ascending,
        from: a.site,
        to,
        dir: (dir.0 as i8, dir.1 as i8),
        progress_m: 0.0,
        rise_m: f64::from(y.abs_diff(to_y)) * v,
    });
    Attach::Climbing
}

/// **One tick on a face**. The feet move along it at half
/// the walking pace the held forward effort asks for, `0.5 · forward · cruise`, and the
/// motor cost is the walking cost at that effort × [`crate::FounderPhysiology::
/// wall_climb_cost_factor`]. Turn effort does not turn a body on a face (the heading stays
/// into it); a turn with no forward backs it off the face ([`back_off_face`]). The
/// interval's motion feedback books the climbing gait's own request (half pace at
/// the held effort) as attempted and what the feet covered as delivered, so an open face
/// reads as an open road on the motor-delivery channel. At the end of the face it stands
/// on the far face, beside the plane, heading on in the direction it came: a wall's top
/// or a cliff's foot. If the far face is gone by then (terrain moved) it slides back to
/// where it started and turns round.
fn climb_tick(
    fauna: &mut Fauna,
    i: usize,
    view: &VoxelView<'_>,
    manifest: &crate::Manifest,
    phys: &crate::FounderPhysiology,
    held: Actions,
) {
    let founder = fauna.animals[i].founder.expect("a founder body");
    let climb_speed_fraction = phys.climb_speed_fraction;
    // TRAPS: a turn asked with no forward is a wish to turn round, which a face does not
    // allow: the body goes back the way it came, at the climbing pace of the turn's effort,
    // and stands on the face it started from, facing away. (Every reflex walk turns in place
    // toward a target behind it; on a face that held it there for good.)
    if !(held.forward > 0.0) && held.turn != 0.0 && held.turn.is_finite() {
        back_off_face(fauna, i, view, manifest, phys, held.turn.abs().min(1.0));
        return;
    }
    let effort = held.forward.clamp(0.0, 1.0);
    let cost = phys.motor_respiration_per_s
        * fauna.animals[i].body
        * effort
        * phys.wall_climb_cost_factor
        * DT;
    let (motor_paid, motor_body) = respire(fauna, i, cost, Respiration::Motor);
    let rate = climb_speed_fraction * effort * manifest.cruise_m_per_s;
    let a = &mut fauna.animals[i];
    let mut climb = a.mobility.climb.expect("a body on a face");
    let before = climb.progress_m;
    climb.progress_m = (climb.progress_m + rate * DT).min(climb.rise_m);
    let moved = climb.progress_m - before;
    {
        let fb = &mut a.founder_state.feedback;
        // What the climbing gait was asked for — half pace at this effort — so a face
        // with nothing in the way delivers all of it, as open ground does.
        fb.attempted_equivalent += rate * DT;
        fb.delivered_equivalent += moved;
        fb.delivered_forward += moved;
        fb.structural_loss += motor_body;
        fb.motor_respiration += motor_paid;
    }
    // Holding on without going anywhere (no forward effort) is not climbing (LOAD-CLIMB):
    // a walker's still tick is Resting.
    a.state = if moved > 0.0 { State::Climbing } else { State::Resting };
    if climb.progress_m < climb.rise_m - 1e-12 {
        a.mobility.climb = Some(climb);
        return;
    }
    // The end of the face.
    let c = view.config;
    let v = c.voxel_m;
    let geometry = phys.body_at(a.body);
    let r = geometry.footprint_radius();
    let headroom = geometry.headroom_voxels(v);
    let dir = (i64::from(climb.dir.0), i64::from(climb.dir.1));
    let s = if dir.0 != 0 { dir.0 } else { dir.1 };
    let col = if dir.0 != 0 {
        i64::from(climb.from.x)
    } else {
        i64::from(climb.from.z)
    };
    let end_at = beside_plane(col, s, v, r, 1.0);
    let (ex, ez) = if dir.0 != 0 {
        (end_at.rem_euclid(f64::from(c.width) * v), a.pose.z)
    } else {
        (a.pose.x, end_at)
    };
    let to = climb.to;
    let top = (to.y + headroom).min(c.height - 1);
    let travel = heading_of(dir);
    a.mobility.climb = None;
    if view.is_support(i64::from(to.x), to.y, to.z)
        && body::disc_clear(view, ex, ez, r, to.y + 1..=top)
    {
        a.pose.x = ex;
        a.pose.z = ez;
        a.pose.heading_rad = travel;
        a.site = to;
        if climb.ascending {
            fauna.ledger.wall_ascents_by_founder[founder.index()] += 1;
        } else {
            fauna.ledger.wall_descents_by_founder[founder.index()] += 1;
        }
    } else {
        // Back where it started, facing away from the face.
        let back = beside_plane(col, s, v, r, -1.0);
        if dir.0 != 0 {
            a.pose.x = back.rem_euclid(f64::from(c.width) * v);
        } else {
            a.pose.z = back;
        }
        a.pose.heading_rad = (travel + std::f64::consts::PI).rem_euclid(std::f64::consts::TAU);
    }
}

/// **Back off a face** (TRAPS, [`climb_tick`]): the climber at `i` goes back along its face
/// toward where it started, at the climbing pace of `effort` and the climbing cost; at the
/// start it stands on the face it came from (its `site` all along), beside the plane on that
/// side, facing away from the face. The interval's feedback books the move as delivered
/// (it is not forward progress).
fn back_off_face(
    fauna: &mut Fauna,
    i: usize,
    view: &VoxelView<'_>,
    manifest: &crate::Manifest,
    phys: &crate::FounderPhysiology,
    effort: f64,
) {
    let cost = phys.motor_respiration_per_s * fauna.animals[i].body * effort * phys.wall_climb_cost_factor * DT;
    let (motor_paid, motor_body) = respire(fauna, i, cost, Respiration::Motor);
    let rate = phys.climb_speed_fraction * effort * manifest.cruise_m_per_s;
    let a = &mut fauna.animals[i];
    let mut climb = a.mobility.climb.expect("a body on a face");
    let before = climb.progress_m;
    climb.progress_m = (climb.progress_m - rate * DT).max(0.0);
    let moved = before - climb.progress_m;
    {
        let fb = &mut a.founder_state.feedback;
        fb.attempted_equivalent += rate * DT;
        fb.delivered_equivalent += moved;
        fb.structural_loss += motor_body;
        fb.motor_respiration += motor_paid;
    }
    a.state = if moved > 0.0 { State::Climbing } else { State::Resting };
    if climb.progress_m > 1e-12 {
        a.mobility.climb = Some(climb);
        return;
    }
    // Back at the start: off the face, on its own side of the plane, facing away.
    let c = view.config;
    let v = c.voxel_m;
    let r = phys.body_at(a.body).footprint_radius();
    let dir = (i64::from(climb.dir.0), i64::from(climb.dir.1));
    let s = if dir.0 != 0 { dir.0 } else { dir.1 };
    let col = if dir.0 != 0 { i64::from(climb.from.x) } else { i64::from(climb.from.z) };
    let back = beside_plane(col, s, v, r, -1.0);
    if dir.0 != 0 {
        a.pose.x = back.rem_euclid(f64::from(c.width) * v);
    } else {
        a.pose.z = back;
    }
    a.pose.heading_rad = (heading_of(dir) + std::f64::consts::PI).rem_euclid(std::f64::consts::TAU);
    a.site = climb.from;
    a.mobility.climb = None;
    a.state = State::Walking;
}

/// The shared raw-food ingestion path for normal bites and groomed prey.
fn ingest_bite(fauna: &mut Fauna, i: usize, flora: &mut Flora, food: Food, taken: Taken) -> Taken {
    let founder = fauna.animals[i].founder.expect("a feeding founder");
    fauna.ledger.eaten_by_founder_food[founder.index()][food.index()] += taken.organic;
    // The sloppy feeder (roster R1): `bite_spill_fraction` of what the plant layer gave
    // falls at once to the litter at its standing face, pro rata in all three currencies,
    // booked `deposited_*`; only the rest reaches the gut.
    let spill_fraction = fauna.config.founder(founder).bite_spill_fraction.clamp(0.0, 1.0);
    let taken = if spill_fraction > 0.0 {
        let spilt = Taken {
            organic: taken.organic * spill_fraction,
            mineral: taken.mineral * spill_fraction,
            energy: taken.energy * spill_fraction,
        };
        let face = fauna.animals[i].site;
        fauna.book_deposit(flora, face, DepositKind::Litter, spilt);
        Taken {
            organic: taken.organic - spilt.organic,
            mineral: taken.mineral - spilt.mineral,
            energy: taken.energy - spilt.energy,
        }
    } else {
        taken
    };
    fauna.ledger.bites_by_founder[founder.index()] += 1;
    fauna.ledger.bites_by_founder_food[founder.index()][food.index()] += 1;
    swallow(fauna, i, food, taken);
    taken
}

/// The whole corpse, before any food capacity or yield is applied.
fn corpse_contents(a: &Animal) -> Taken {
    let seeds = a.gut_seeds.total();
    let fed = a.stomach.total();
    Taken {
        organic: a.organic() + a.gut.organic + seeds.organic + fed.organic,
        mineral: a.mineral + a.gut.mineral + seeds.mineral + fed.mineral,
        energy: a.energy + a.fuel_energy + a.gut.energy + seeds.energy + fed.energy,
    }
}

/// SP-GROOM: reuse the catch's kill-to-carcass and raw-food ingestion paths.
pub(crate) fn groom_prey(fauna: &mut Fauna, flora: &mut Flora, groomer: u64, rider: u64, view: &VoxelView<'_>) {
    let Ok(j) = fauna.animals.binary_search_by_key(&rider, |a| a.id) else { return; };
    let prey = fauna.animals[j];
    let Some(pf) = prey.founder else { return; };
    let Some(gf) = fauna.view().animal(groomer).and_then(|a| a.founder) else { return; };
    // A grooming removal is neither a grown drop-off nor a freed rider.
    fauna.animals[j].attached = None;
    kill_into(fauna, flora, j, Departure::Groomed, Some(view), Some(groomer));
    fauna.ledger.groomed_by_groomer[gf.index()] += 1;
    fauna.ledger.groomed_by_rider[pf.index()] += 1;
}

/// Use the same terrain route and directed walking machinery as a fruit approach.
pub(crate) fn groom_approach(fauna: &mut Fauna, i: usize, view: &VoxelView<'_>, fv: &cubarium_voxel_flora::FloraView<'_>, host: (f64,u64,Site), seek: f64) -> Option<Actions> {
    glider::walk_target(fauna, i, view, fv, &[host], seek, 0.0).map(|(go,_)| go)
}

/// The founder's supported local bite: the blind founder takes **detritus** — litter or
/// carrion off the ground under its mouth region, or cap tissue off a fungal stand in
/// its band — and the browser crops a vascular stand whose crown cells it touches. One
/// withdrawal per attempt, through the plant layer's own `take_*`, debiting the real
/// stocks. Nothing in reach, a mouth in the air, or an unsupported body transfers
/// nothing at all.
///
/// Decisions §3 gave the shredder its three foods; since R0 every lineage's foods are its
/// physiology's diet list. There is no new action: `bite` still means "take what is at
/// the mouth", and [`body::mouth_meal_worth`] resolves which of the diet's foods that is (the
/// richest available, ties in [`Food::ALL`] order). Since the minimal gut the bite goes
/// raw into that food class's stomach pool ([`swallow`]); digestion ([`digest`]) applies
/// the class's `yield` ([`crate::FounderPhysiology::yield_for`]) later.
#[allow(clippy::too_many_arguments)]
fn founder_feed(
    fauna: &mut Fauna,
    i: usize,
    view: &VoxelView<'_>,
    flora: &mut Flora,
    manifest: &crate::Manifest,
    geometry: &crate::Body,
    phys: &crate::FounderPhysiology,
    effort: f64,
) -> bool {
    // Only a body that wants food bites (the eating latch, repair 2 item 3).
    if !fauna.animals[i].eating || !(effort > 0.0) {
        return false;
    }
    let (founder, pose, site_y, supported) = {
        let a = &fauna.animals[i];
        let Some((cx, cz)) = a.pose.column(view.config.voxel_m, view.config.depth) else {
            return false;
        };
        let cx = cx.rem_euclid(i64::from(view.config.width));
        (
            a.founder.expect("a founder body"),
            a.pose,
            a.site.y,
            view.is_support(cx, a.site.y, cz),
        )
    };
    if !supported {
        return false;
    }
    let cols = body::mouth_columns(view, &pose, geometry);
    // Which food is at this mouth, and how much of it — the `E` the bite diminishes
    // with — before anything is withdrawn: the richest of the lineage's diet
    // ([`body::mouth_meal_worth`]). Its class is the source's own: a pool names it, and a
    // stand's is its species' income rule.
    let diet = phys.eating_diet();
    let Some((meal, stock, _)) =
        body::mouth_meal_worth(&flora.view(), view, &cols, site_y, geometry, diet, &phys.mouth_weigh())
    else {
        return false;
    };
    let food = meal.food();
    // FORAGE-LEAVE: what this patch offers the mouth, for the giving-up rule.
    forage::note_bite(fauna, i, phys, food, manifest.controller_period_s, effort, stock, None);
    // The bite this body, this stock and this stomach's room allow.
    let bite = gut_bite(&fauna.animals[i], phys, food, manifest.controller_period_s, effort, stock);
    if !(bite > 0.0) {
        return false;
    }
    // The mouth's own cell range, handed to the plant layer: a foliage bite comes out
    // of the layers the band reaches, lowest first, and the crown above them is not
    // offered (`design/handoffs/voxel-plant-layers-2026-09-22.md`).
    let layers = body::mouth_crown_layers(site_y, geometry, view.config.voxel_m);
    // Whose tissue this is, read before the withdrawal, so the report says which
    // species was actually eaten.
    // The place on the face is the food's: cap tissue off the fungus, foliage off the plant.
    let plant = match meal {
        body::Meal::Stand { root, place, .. } => flora.view().stand_in(root, place).map(|s| s.species),
        body::Meal::Mycelium { root } => flora.view().fungus_at(root).map(|s| s.species),
        body::Meal::Pool { .. } => None,
    };
    // The funnel's eaten stage (SENSES-CORE): a bite of a carcass, by the dead body's lineage.
    let carcass_of = match meal {
        body::Meal::Pool { site, food: Food::Carrion } => flora.view().ground_at(site).and_then(|g| g.carcass).map(|c| (usize::from(c.lineage), site)),
        _ => None,
    };
    // The aim's cell (FEED-ROLLOUT round 2): where this bite comes from, read before the
    // withdrawal and kept on the body only if the bite is taken. Presentation memory: no
    // rule reads it ([`crate::LastBite`]).
    let bitten = match meal {
        body::Meal::Stand { root, place, .. } => body::bitten_cell(&flora.view(), view, root, place, &cols, &layers),
        body::Meal::Mycelium { root } => Some(Site { y: root.y + 1, ..root }),
        body::Meal::Pool { site, .. } => Some(Site { y: site.y + 1, ..site }),
    };
    // LARVA (decision 4, "b+c"): a bite off a vaulttree host takes its larvae by accident,
    // the eaten fraction of its edible foliage; read before the withdrawal.
    let host_before = match meal {
        body::Meal::Stand { root, place, food: Food::Foliage } => {
            let fv = flora.view();
            fv.stand_in(root, place).filter(|s| crate::larva::stand_hosts(&fv, s)).map(|s| (s.id, fv.edible_foliage(s)))
        }
        _ => None,
    };
    let mut mycelium = 0.0;
    let taken = match meal {
        body::Meal::Stand { root, place, .. } => {
            let Some(mut caps) = flora.take_foliage_of(root, place, bite, &layers).map(|t| t.taken) else { return false; };
            if food == Food::CapTissue && root.y == site_y && layers.contains(&(i64::from(site_y)+1)) {
                if let Some(t) = flora.take_mycelium(root, (bite-caps.organic).max(0.0)) {
                    mycelium = t.organic;
                    caps.organic += t.organic;
                    caps.mineral += t.mineral;
                    caps.energy += t.energy;
                }
            }
            Some(caps)
        },
        body::Meal::Mycelium { root } => flora.take_mycelium(root, bite).inspect(|t| { mycelium = t.organic; }),
        body::Meal::Pool { site, food } => match food {
            Food::Litter => {
                let taken = flora.take_litter(site, bite);
                if taken.is_some() && founder == Founder::Blind { flora.fragment_litter_world(view, site); }
                taken
            },
            Food::Carrion => flora.take_carrion(site, bite),
            Food::Film => flora.take_film(site, bite),
            Food::CapTissue | Food::Foliage | Food::Nectar | Food::Fruit | Food::Larva => {
                unreachable!("a stand's tissue is not a ground pool")
            }
        },
    };
    let Some(taken) = taken else {
        return false;
    };
    fauna.animals[i].bite = crate::LastBite(bitten);
    fauna.ledger.eaten_mycelium_by_founder[founder.index()] += mycelium;
    let mut taken = fauna.book_eaten(taken, food);
    // The larvae ride in the bite as foliage: a transfer inside this layer, not eaten in.
    if let Some((id, edible)) = host_before
        && edible > 0.0
        && let Some(larvae) = crate::larva::browse_larvae(fauna, id, taken.organic / edible)
    {
        taken.organic += larvae.organic;
        taken.mineral += larvae.mineral;
        taken.energy += larvae.energy;
    }
    // CHECK-DIET: whose carcass this raw carrion was.
    if let Some((prey, _)) = carcass_of
        && prey < Founder::COUNT
    {
        fauna.ledger.eaten_carrion_by_founder_prey[founder.index()][prey] += taken.organic;
    }
    let taken = ingest_bite(fauna, i, flora, food, taken);
    if let Some(plant) = plant {
        fauna.ledger.bites_by_plant[plant.index()] += 1;
        fauna.ledger.eaten_by_plant[plant.index()] += taken.organic;
    }
    if food == Food::Carrion {
        fauna.ledger.eaten_carrion_by_founder[founder.index()] += taken.organic;
        if let Some((prey, site)) = carcass_of {
            let id = fauna.animals[i].id;
            crate::perception::eaten(fauna, id, founder, prey, site);
        }
        // A pack member eating at its pack's feast (repair 3's measure).
        if let (body::Meal::Pool { site, .. }, Some(m)) = (meal, fauna.animals[i].pack) {
            if let Some(p) = fauna.pack_mut(m.pack).filter(|p| p.feast == Some(site)) {
                p.feast_eaten += taken.organic;
            }
        }
    }
    true
}

/// A bite becoming tissue and fuel: the food class's own yield is stored **as far as the
/// bite's own mineral pays for**, the undigested `(1 − yield)` share is egested, the
/// unfunded rest fills fuel, its overflow is respired, and energy follows at the bite's own
/// density, and the undigested share and the mineral that came with more tissue than was
/// built go to the gut, voided later as dung ([`void_guts`]).
///
/// **The mineral budget comes before the tissue.** `n_tissue` is mineral per
/// unit of tissue *built*, so a bite can only build `t.mineral / n_tissue` of it. At the
/// placeholders that binds on every bite: a plant's foliage carries `n_tissue` 0.02 and
/// this animal's tissue wants 0.05, so a `1e-4` bite brings `2e-6` of mineral and funds
/// `4e-5` of the `5e-5` a yield of 0.5 would otherwise have built. The unfunded
/// organic matter fills nitrogen-free fuel up to `fuel_cap · body`; overflow leaves
/// with its energy as heat.
///
/// **The yield is the food's, not the eater's**: the caller hands in
/// [`crate::FounderPhysiology::yield_for`] for what was actually eaten, because a
/// detritivore with three foods has no single yield.
///
/// **No internal mineral reserve.** [`Animal::mineral`] is an inventory of what is already
/// in the tissue and not a stock growth may draw on — the plant layer's `Stand::mineral`
/// is the same — so an animal holding mineral still builds nothing out of a mineral-free
/// bite.
/// Returns the organic matter placed into the reserve. The caller adds the fuel gained
/// when recording total assimilation and prior-interval intake feedback.
fn assimilate(
    fauna: &mut Fauna,
    i: usize,
    sc: &SpeciesConfig,
    t: Taken,
    yield_fraction: f64,
) -> f64 {
    let a = &mut fauna.animals[i];
    // The feeding income function: the food's yield of the bite, as far as this bite's
    // mineral can pay for the tissue.
    let funded = tissue_funded(t.mineral, sc.n_tissue);
    let assimilated = (yield_fraction * t.organic).min(funded);
    // Egested: the share the yield never absorbs goes to the gut, not to heat. Only the
    // absorbed matter the bite's mineral cannot fund fills fuel; cap overflow and
    // the reserve backstop are respired.
    // The absorbed share its mineral cannot fund, exactly zero when the mineral pays for
    // the whole yield; the egested share is the remainder, so the three add up.
    let unfunded = (yield_fraction * t.organic - assimilated).clamp(0.0, t.organic - assimilated);
    let egested = (t.organic - assimilated - unfunded).max(0.0);

    // The organism budget: the income stored into the reserve, up to `reserve_cap · body`.
    // Structure is built out of the reserve at the growth rate ([`grow`]), never straight
    // from a bite. What the reserve cannot hold is respired at the close, but the sated
    // bite ([`sated_bite`]) never takes more than the room after yield, so that is the
    // conservation backstop and not a rule that fires.
    let mut budget = Budget::new(assimilated, funded);
    let placed = budget.store((sc.reserve_of(a.body) - a.reserve).max(0.0));
    a.reserve += placed;
    // The brood (D1b): a mature body earmarks its share of the income for its next clutch,
    // up to what that clutch costs.
    let rule = sc.reproduction;
    if rule.brood_share > 0.0 && a.body >= sc.birth_body {
        let target = rule.brood_target(sc, a.body);
        if a.reproduction.brood < target {
            a.reproduction.brood = (a.reproduction.brood + rule.brood_share * placed).min(target);
        }
    }
    let fuel_kept = unfunded.min((sc.fuel_cap * a.body - a.fuel).max(0.0));
    a.fuel += fuel_kept;
    let respired = unfunded - fuel_kept + budget.close();

    // Energy: the density of what was eaten, so nothing is created for any food.
    let kept_energy = pro_rata(t.energy, placed, t.organic);
    a.energy += kept_energy;
    let fuel_energy =
        pro_rata(t.energy, fuel_kept, t.organic).min((t.energy - kept_energy).max(0.0));
    a.fuel_energy += fuel_energy;
    let egested_energy = if t.organic > 0.0 {
        (t.energy * (egested / t.organic)).min((t.energy - kept_energy - fuel_energy).max(0.0))
    } else {
        0.0
    };
    let heat = (t.energy - kept_energy - fuel_energy - egested_energy).max(0.0);

    // Mineral: what the new tissue needs is kept and the excess joins the gut, voided
    // with the undigested matter as dung. An animal draws mineral from nothing but its
    // food, so there is no pool to draw a shortfall from.
    let settled = settle_mineral(t.mineral, sc.n_tissue * placed, 0.0);
    a.mineral += t.mineral - settled.released;
    a.gut.organic += egested;
    a.gut.mineral += settled.released;
    a.gut.energy += egested_energy;

    book_respired(fauna, respired, Respiration::Digestion);
    fauna.ledger.heat_out += heat;
    placed
}

/// Step 6: **reproduction** — the gestation escrow, live birth, egg laying, incubation
/// and hatching, in that one place.
///
/// The rule the two founder lineages were given
/// (`design/handoffs/voxel-reproduction-2026-09-21.md`, and the escrow of
/// `design/evolution.md` §"Birth, mutation, and lineage"), replacing the placeholder
/// that let any adult at `birth_body` with `birth_cost` of reserve give birth every
/// tick it could:
///
/// 1. **Eligibility.** Structure at `birth_body` and a reserve at or above the
///    offspring package plus `surplus_floor`, held unbroken for `surplus_hold_s`. Any
///    tick below the floor resets the counter, and a body inside its post-offspring
///    refractory is not accumulating at all.
/// 2. **Gestation** ([`crate::BirthMode::Gestation`]). The package leaves the reserve in
///    equal instalments over `gestation_s` into an [`crate::Escrow`] on the animal,
///    mineral and energy pro rata per instalment by the same fraction rule every other
///    transfer in this world uses. The escrow pays no upkeep and the parent cannot
///    spend it. A parent that cannot pay an instalment — or that dies carrying one —
///    loses `gestation_loss_fraction` of the escrowed organic matter to respiration and
///    gets the rest back, mineral included.
/// 3. **Birth.** At term one newborn is built out of the escrow exactly as the old rule
///    built one out of the reserve, and `birth_interval_s` of refractory follows.
/// 4. **Laying** ([`crate::BirthMode::Eggs`]). A whole clutch is paid out of the reserve
///    in one tick and left on the face the parent stands on, which must hold litter;
///    otherwise nothing happens and nothing is paid. Then the same refractory.
/// 5. **Incubation.** Eggs pay no upkeep. A clutch whose face is gone or under water
///    deeper than the lineage's `drown_depth_m` goes to carrion where it lies; at
///    `incubation_s` the rest hatch together into `body_min` juveniles holding what
///    their egg had left.
///
/// **Paid, never free, at every step**: an offspring's organic matter, mineral and
/// energy are the parent's, an interrupted gestation's loss is booked as respiration
/// with its energy as heat, and a lost clutch is deposited. The layer's stored totals
/// count an escrow and a clutch ([`crate::FaunaView::organic`]), so none of this can
/// create or destroy matter without the residual saying so.
///
/// **A newborn or hatchling inherits no mind.** Each is given a *fresh* controller of
/// its lineage from that lineage's registered [`crate::ControllerFactory`] — never the
/// parent's instance, which would mean two bodies sharing one memory. A lineage with no
/// factory registered breeds resting bodies, which is the pre-existing behaviour.
fn reproduce(fauna: &mut Fauna, view: &VoxelView<'_>, flora: &mut Flora, dusk: bool) {
    if !fauna.births_enabled {
        // A dormant egg still hears, runs out of viability and dies to the litter with
        // births off; only its hatching waits for births to be on.
        let mut none = Vec::new();
        dormant_eggs(fauna, view, flora, false, &mut none);
        return;
    }
    clutch_fate(fauna, view, flora);
    shore_eggs(fauna, view, flora);
    let parents = fauna.animals.len();
    let mut newborns: Vec<Animal> = Vec::new();
    // LIFE-HISTORY: the Cubarium year, for the lineages with a breeding season.
    let year = view.year();
    for i in 0..parents {
        let sc = body::effective_config(&fauna.config, &fauna.animals[i]);
        let rule = sc.reproduction;
        advance_eligibility(fauna, i, &sc, &rule);
        match rule.mode {
            crate::BirthMode::Gestation => {
                gestate(fauna, i, &sc, &rule, (view.config.seed, year), &mut newborns);
            }
            crate::BirthMode::Eggs => lay(fauna, i, view, flora, &sc, &rule),
        }
    }
    hatch(fauna, view, &mut newborns);
    // LARVA: the larva field's deaths, growth and emergence; emerged adults are born here.
    crate::larva::step_field(fauna, view, flora, &mut newborns);
    dormant_eggs(fauna, view, flora, dusk, &mut newborns);
    let fresh: Vec<(u64, Founder)> = newborns
        .iter()
        .filter_map(|n| n.founder.map(|f| (n.id, f)))
        .collect();
    for mut n in newborns {
        // A newborn flyer is perched on the face it was born on (F1).
        // One hatched in the vines (VINE-C) already clings to its face.
        // An adult emerging in a vaulttree's crown (LARVA) is already flying there.
        if n.mobility.perch.is_none()
            && n.mobility.mode != crate::MobilityMode::Fly
            && n.founder.is_some_and(|f| fauna.config.founder(f).flight.is_some())
        {
            n.pose.y_m = crate::surface_m(n.site.y, view.config.voxel_m);
            let site = n.site;
            crate::flight::perch_on_face(&mut n, site);
        }
        fauna.insert(n);
    }
    for (id, founder) in fresh {
        if let Some(controller) = fauna.factories.make(founder) {
            fauna.controllers.set(id, controller);
        }
    }
}

/// The refractory counts down, and the surplus counter moves: up on an eligible tick,
/// back to zero on any tick that is not. A body already gestating is not accumulating a
/// *second* offspring's hold, so its counter stands still.
fn advance_eligibility(
    fauna: &mut Fauna,
    i: usize,
    sc: &SpeciesConfig,
    rule: &crate::Reproduction,
) {
    let a = &mut fauna.animals[i];
    a.reproduction.retry_ticks = a.reproduction.retry_ticks.saturating_sub(1);
    if a.reproduction.refractory_ticks > 0 {
        a.reproduction.refractory_ticks -= 1;
        a.reproduction.surplus_ticks = 0;
        a.reproduction.window.reset();
        return;
    }
    if a.reproduction.escrow.is_some() {
        return;
    }
    let eligible = a.body >= sc.birth_body
        && a.reserve >= rule.surplus_reserve(sc)
        && rule.package_cost(sc) > 0.0;
    let hold = rule.hold_ticks();
    a.reproduction.window.tick(eligible, hold);
    a.reproduction.surplus_ticks = a.reproduction.window.count_for(hold);
    a.reproduction.eligible_now = eligible;
}

/// One gestating body's tick: open an escrow when the hold has elapsed, pay this tick's
/// instalment, and hand back the newborn at term. `None` on every other tick.
fn gestate(
    fauna: &mut Fauna,
    i: usize,
    sc: &SpeciesConfig,
    rule: &crate::Reproduction,
    (seed, year): (u64, Option<cubarium_voxel::weather::Year>),
    newborns: &mut Vec<Animal>,
) {
    let a = &fauna.animals[i];
    if a.reproduction.escrow.is_none() {
        if !rule.hold_met(&a.reproduction) || a.reproduction.retry_ticks > 0 {
            return;
        }
        if at_body_ceiling(fauna, i, rule) {
            return;
        }
        // R3: a pack lineage's litter — only its pack's breeder conceives, no sooner than
        // `litter_interval_s` after its last litter, 2 to `litter_size_max` young.
        let young = match litter_size(fauna, i, sc, rule, seed) {
            Litter::One => 1,
            Litter::Of(n) => n,
            Litter::Not => return,
        };
        // LIFE-HISTORY: a breeding season conceives only inside its window, and no more
        // than `litters_per_year` times a year (a pack's count is the pack's).
        if !rule.season_allows(year, season_of(fauna, i)) {
            fauna.ledger.season_waits += 1;
            return;
        }
        if let Some(y) = year.filter(|_| rule.litters_per_year > 0) {
            count_season(fauna, i, y.number);
        }
        fauna.animals[i].reproduction.escrow = Some(crate::Escrow {
            young,
            ..crate::Escrow::default()
        });
        fauna.ledger.gestations_opened += 1;
    }
    let n = rule.gestation_ticks();
    let (paid, young) = {
        let e = fauna.animals[i].reproduction.escrow.expect("an open escrow");
        (e.ticks, e.young.max(1))
    };
    let k = paid + 1;
    // The instalment is the distance to this tick's **cumulative target**, so the
    // instalments sum to `birth_cost` exactly however the division rounds, and the last
    // one closes whatever the earlier ones left.
    let target = rule.package_cost(sc) * f64::from(young) * (k as f64 / n as f64);
    let want = target
        - fauna.animals[i]
            .reproduction
            .escrow
            .expect("an open escrow")
            .organic;
    if want > 0.0 {
        // "If the parent's reserve hits zero mid-gestation ... the gestation fails": a
        // reserve that cannot pay this instalment in full is that condition, and a
        // reserve at zero cannot pay anything. Nothing is part-paid.
        if fauna.animals[i].reserve < want {
            fail_gestation(fauna, i, rule);
            return;
        }
        let a = &mut fauna.animals[i];
        let paid = take_from_reserve(a, want);
        if let Some(f) = a.founder {
            fauna.ledger.reproduction_by_founder[f.index()] += paid.organic;
        }
        let a = &mut fauna.animals[i];
        let e = a.reproduction.escrow.as_mut().expect("an open escrow");
        e.organic += paid.organic;
        e.mineral += paid.mineral;
        e.energy += paid.energy;
    }
    let e = fauna.animals[i]
        .reproduction
        .escrow
        .as_mut()
        .expect("an open escrow");
    e.ticks = k;
    if k < n {
        return;
    }
    // Term. The escrow becomes the newborn, whole: this is the same parcel the parent
    // paid for, so nothing is clamped and nothing is rounded away.
    let e = fauna.animals[i]
        .reproduction
        .escrow
        .take()
        .expect("an open escrow");
    book_first_young(fauna, i);
    let a = &mut fauna.animals[i];
    a.reproduction.refractory_ticks = rule.interval_ticks();
    a.reproduction.surplus_ticks = 0;
    a.reproduction.window.reset();
    a.reproduction.given += e.organic;
    if let (Some(r), Some(f)) = (&mut fauna.census_readout, a.founder) {
        r.organic_given[f.index()] += e.organic;
    }
    let (site, species, pose, founder, pack, mother) = (a.site, a.species, a.pose, a.founder, a.pack, a.id);
    // A litter's parcel is split evenly; the last young takes what division left, so the
    // parcels sum to the escrow exactly.
    let young = e.young.max(1);
    let mut left = Taken {
        organic: e.organic,
        mineral: e.mineral,
        energy: e.energy,
    };
    for k in 0..young {
        let share = if k + 1 == young {
            left
        } else {
            let f = 1.0 / f64::from(young);
            Taken {
                organic: e.organic * f,
                mineral: e.mineral * f,
                energy: e.energy * f,
            }
        };
        left.organic -= share.organic;
        left.mineral -= share.mineral;
        left.energy -= share.energy;
        let mut n = newborn(
            fauna,
            (species, founder),
            (site, pose),
            sc.body_min,
            share,
            sc.lifespan_median_s,
        );
        // SP-NURSE: the young knows its mother ([`crate::nurse`]).
        n.mother = Some(mother);
        // Born into the parent's pack, a pup.
        if let Some(m) = pack {
            n.pack = Some(crate::PackMember {
                pack: m.pack,
                pup: true,
                calling_ticks: 0,
            });
        }
        newborns.push(n);
    }
    if let Some(m) = pack {
        let tick = fauna.tick;
        if let Some(p) = fauna.pack_mut(m.pack) {
            p.last_litter_tick = Some(tick);
        }
        fauna.ledger.litters += 1;
        fauna.ledger.pups_born += u64::from(young);
    }
}

/// [`Fauna::whelp`] (PUP's scenario control): a litter of `young` born now at pack `pack`'s den,
/// paid out of its breeder's reserve at the lineage's package per pup.
pub(crate) fn whelp(fauna: &mut Fauna, view: &VoxelView<'_>, pack: u64, young: u32) -> Result<Vec<u64>, String> {
    let rec = fauna.pack(pack).ok_or_else(|| format!("no pack {pack}"))?;
    let (den, breeder) = (rec.den, rec.breeder.ok_or("the pack has no breeder")?);
    let i = fauna.animals.iter().position(|a| a.id == breeder).ok_or("the breeder is gone")?;
    let young = young.max(1);
    let sc = body::effective_config(&fauna.config, &fauna.animals[i]);
    let rule = sc.reproduction;
    let cost = rule.package_cost(&sc) * f64::from(young);
    if !(cost > sc.body_min) || fauna.animals[i].reserve < cost {
        return Err(format!("the breeder's reserve {:.3} cannot pay {cost:.3}", fauna.animals[i].reserve));
    }
    let paid = take_from_reserve(&mut fauna.animals[i], cost);
    if let Some(f) = fauna.animals[i].founder {
        fauna.ledger.reproduction_by_founder[f.index()] += paid.organic;
    }
    if let Some(y) = view.year().filter(|_| rule.litters_per_year > 0) {
        count_season(fauna, i, y.number);
    }
    let a = &mut fauna.animals[i];
    a.reproduction.refractory_ticks = rule.interval_ticks();
    a.reproduction.surplus_ticks = 0;
    a.reproduction.window.reset();
    a.reproduction.given += paid.organic;
    if let (Some(r), Some(f)) = (&mut fauna.census_readout, a.founder) {
        r.organic_given[f.index()] += paid.organic;
    }
    let (species, founder, mut pose) = (a.species, a.founder, a.pose);
    let v = view.config.voxel_m;
    let (x, z) = crate::pack::centre(den, v);
    pose.x = x;
    pose.z = z;
    pose.y_m = crate::surface_m(den.y, v);
    let mut ids = Vec::new();
    let f = 1.0 / f64::from(young);
    for _ in 0..young {
        let share = Taken { organic: paid.organic * f, mineral: paid.mineral * f, energy: paid.energy * f };
        let mut n = newborn(fauna, (species, founder), (den, pose), sc.body_min, share, sc.lifespan_median_s);
        n.pack = Some(crate::PackMember { pack, pup: true, calling_ticks: 0 });
        n.mother = Some(breeder);
        ids.push(n.id);
        let fd = n.founder;
        fauna.insert(n);
        if let Some(fd) = fd {
            if let Some(c) = fauna.factories.make(fd) {
                fauna.controllers.set(ids[ids.len() - 1], c);
            }
        }
    }
    let tick = fauna.tick;
    if let Some(p) = fauna.pack_mut(pack) {
        p.last_litter_tick = Some(tick);
    }
    fauna.ledger.litters += 1;
    fauna.ledger.pups_born += u64::from(young);
    Ok(ids)
}

/// The breeding season a body's conceptions count against (LIFE-HISTORY): its pack's for a
/// pack member, its own otherwise.
fn season_of(fauna: &Fauna, i: usize) -> crate::Season {
    let a = &fauna.animals[i];
    match a.pack.and_then(|m| fauna.pack(m.pack)) {
        Some(p) => p.season,
        None => a.reproduction.season,
    }
}

/// Count one conception in `year` against [`season_of`]'s season.
fn count_season(fauna: &mut Fauna, i: usize, year: i64) {
    match fauna.animals[i].pack.map(|m| m.pack) {
        Some(pack) if fauna.pack(pack).is_some() => {
            if let Some(p) = fauna.pack_mut(pack) {
                p.season.count(year);
            }
        }
        _ => fauna.animals[i].reproduction.season.count(year),
    }
}

/// What a gestating lineage's next conception carries (roster R3).
enum Litter {
    /// Not a pack lineage: one young, the ordinary rule.
    One,
    /// A pack breeder's litter of this many.
    Of(u32),
    /// A pack lineage that does not conceive now: not its pack's breeder, inside the
    /// litter interval, or a reserve that cannot fund the smallest litter.
    Not,
}

/// Stream key for the litter-size draw.
const DOMAIN_LITTER: u64 = 0x11_77E2;

/// R3: a pack lineage's conception rule. Only the pack's breeder conceives, and no
/// sooner than `litter_interval_s` after its last litter. The litter is drawn uniform on
/// 2..=`litter_size_max` (keyed by the world seed, the breeder and the tick), and is
/// never more young than its reserve above the surplus floor funds; with too little for
/// two, it waits.
///
/// LIFE-HISTORY: the breeder conceives only with its reserve at `breed_reserve_fraction` of
/// its cap or more, and with `litter_by_condition` the litter is not drawn but follows that
/// condition, 1 to `litter_size_max` ([`Pack::litter_by_condition`]); with funds for none,
/// it waits.
fn litter_size(fauna: &Fauna, i: usize, sc: &SpeciesConfig, rule: &crate::Reproduction, seed: u64) -> Litter {
    let a = &fauna.animals[i];
    let Some(t) = a.founder.and_then(|f| fauna.config.founder(f).pack) else {
        return if rule.young_per_birth > 1 {
            Litter::Of(rule.young_per_birth)
        } else {
            Litter::One
        };
    };
    let Some(m) = a.pack.filter(|m| !m.pup) else {
        return Litter::Not;
    };
    let Some(record) = fauna.pack(m.pack) else {
        return Litter::Not;
    };
    if record.breeder != Some(a.id) {
        return Litter::Not;
    }
    if let Some(last) = record.last_litter_tick {
        if fauna.tick.saturating_sub(last) < cubarium_voxel::ticks_at_least_one(t.litter_interval_s) {
            return Litter::Not;
        }
    }
    let hi = t.litter_size_max.max(1);
    let cap = sc.reserve_of(a.body);
    let r = if cap > 0.0 { a.reserve / cap } else { 0.0 };
    let f = t.breed_reserve_fraction.clamp(0.0, 1.0);
    if f > 0.0 && r < f {
        return Litter::Not;
    }
    let (lo, drawn) = if t.litter_by_condition {
        (1, t.condition_litter(r))
    } else {
        let lo = hi.min(2);
        let mut rng = Rng::keyed(DOMAIN_LITTER, seed, a.id, fauna.tick);
        (lo, lo + (rng.unit() * f64::from(hi - lo + 1)).floor().min(f64::from(hi - lo)) as u32)
    };
    let package = rule.package_cost(sc);
    let funds = if package > 0.0 {
        ((a.reserve - rule.surplus_floor) / package).floor().max(0.0)
    } else {
        f64::from(hi)
    };
    let n = drawn.min(funds.min(f64::from(u32::MAX)) as u32);
    if n < lo {
        Litter::Not
    } else {
        Litter::Of(n)
    }
}

/// A newborn out of a paid `parcel`: a `body_min` juvenile with the rest of the parcel's
/// organic matter as its reserve and all of its mineral and energy, booked born. The one
/// construction a gestation's term and an egg's hatching share.
pub(crate) fn newborn(
    fauna: &mut Fauna,
    (species, founder): (crate::Species, Option<crate::Founder>),
    (site, pose): (Site, crate::Pose),
    body_min: f64,
    parcel: Taken,
    lifespan_median_s: f64,
) -> Animal {
    let id = fauna.ledger.births;
    fauna.ledger.births += 1;
    fauna.ledger.born += 1;
    if let Some(f) = founder {
        fauna.born_by_founder[f.index()] += 1;
    }
    Animal {
        id,
        species,
        site,
        pose,
        founder,
        founder_state: crate::FounderState::default(),
        reproduction: crate::ReproductionState {
            native: true,
            ..crate::ReproductionState::default()
        },
        body: body_min,
        reserve: parcel.organic - body_min,
        fuel: 0.0,
        fuel_energy: 0.0,
        mineral: parcel.mineral,
        energy: parcel.energy,
        age_ticks: 0,
        lifespan_ticks: crate::draw_lifespan_ticks(
            &fauna.config.life,
            lifespan_median_s,
            id,
            crate::lifespan_salt(site, fauna.tick),
            false,
        ),
        gut: Taken::default(),
        state: State::Resting,
        mobility: crate::Mobility::default(),
        wound: 0.0,
        strike: None,
        rest: None,
        still_ticks: fauna.config.predation.still_after_ticks(),
        speed_m_s: 0.0,
        alert_ticks: 0,
        attached: None,
        last_bloom: None,
        gut_seeds: crate::GutSeedLots::default(),
        pack: None,
        homing_ticks: 0,
        rest_ticks: 0,
        doze: crate::sleep::Doze::default(),
        yielding: false,
        hunkered: false,
        // A newborn starts with an empty stomach and its eating latch off (repair 2
        // item 3).
        stomach: crate::Stomach::default(),
        eating: false,
        emergency: false,
        bout: None,
        hunt: crate::HuntMemory::default(),
        mother: None,
        intent: crate::IntentRecord::default(),
        kick: crate::kick::KickState::default(),
        stance: 0,
        bite: crate::LastBite::default(),
    }
}

#[cfg(test)]
mod birth_accounting_tests {
    use super::*;

    #[test]
    fn paid_creation_is_counted_even_without_a_surviving_body() {
        let mut fauna = Fauna::new(crate::FaunaConfig::default());
        let founder = crate::Founder::Blind;
        let site = Site { x: 1, y: 1, z: 1 };
        let a = newborn(&mut fauna, (crate::Species::Frondgrazer, Some(founder)),
            (site, crate::Pose::at_site(site, 0.25)), 0.01,
            Taken { organic: 0.02, mineral: 0.001, energy: 0.1 }, 100.0);
        assert_eq!(a.founder, Some(founder));
        assert!(fauna.view().animals.is_empty(), "creation counter is independent of survivors");
        assert_eq!(fauna.born_by_founder()[founder.index()], 1);
        assert_eq!(fauna.view().ledger.born, 1);
    }
}

/// An interrupted gestation: the loss fraction of the escrowed organic matter is
/// respired with the energy that was in it, the mineral it held is shed to the parent's
/// gut by the same fraction, and everything else — organic matter, mineral, the rest of
/// the energy — goes back to the parent.
fn fail_gestation(fauna: &mut Fauna, i: usize, rule: &crate::Reproduction) {
    let Some(e) = fauna.animals[i].reproduction.escrow.take() else {
        return;
    };
    fauna.ledger.gestations_failed += 1;
    let lost = (e.organic * rule.gestation_loss_fraction).clamp(0.0, e.organic);
    let heat = pro_rata(e.energy, lost, e.organic);
    // The lost share's mineral is shed to the gut, as every respiration sheds it.
    let shed = pro_rata(e.mineral, lost, e.organic);
    let a = &mut fauna.animals[i];
    a.reserve += e.organic - lost;
    a.mineral += e.mineral - shed;
    a.gut.mineral += shed;
    a.energy += e.energy - heat;
    if lost > 0.0 || heat > 0.0 {
        book_respired(fauna, lost, Respiration::Gestation);
        fauna.ledger.heat_out += heat;
    }
}

/// One egg-laying body's tick: a whole clutch, paid out of the reserve in one tick and
/// left on the face the body stands on.
///
/// The face has to hold litter — the littershredder lays in the litter it lives in — or,
/// for a lineage whose [`crate::EggSite`] is film, film; a vine nester's clutch goes on a
/// latticevine face instead ([`vine_egg_site`], VINE-C); and a laying that has nowhere to go **costs nothing**: no escrow, no partial payment,
/// no refractory, and the surplus keeps standing for the next tick.
/// SHREDDER-NEST: a walk off a crowded nest ends without a clutch — given up — when its
/// [`crate::Reproduction::nest_seek_give_up_s`] has run out, or when the body stands on its
/// face and the face no longer holds its egg-site rule (the litter eaten or washed off). The
/// body then waits out its retry, as a refused layer does; at its next laying the crowd
/// refuses it again and it looks again.
fn end_nest_trip(fauna: &mut Fauna, i: usize, view: &VoxelView<'_>, flora: &Flora, rule: &crate::Reproduction) {
    let a = &fauna.animals[i];
    let Some(trip) = fauna.nest_trips.get(&a.id).copied() else { return };
    let Some(lineage) = a.founder else { return };
    let late = fauna.tick.saturating_sub(trip.since) >= rule.nest_seek_give_up_ticks();
    let arrived = (a.site.x, a.site.z) == (trip.target.x, trip.target.z);
    if late || (arrived && !egg_site_fits(fauna, lineage, view, flora, a.site, rule)) {
        give_up_nest_trip(fauna, i, lineage, rule);
    }
}

/// A walk off a crowded nest given up: the trip ends without a clutch, booked as given up,
/// and body `i` of `lineage` waits out its retry, as a refused layer does.
fn give_up_nest_trip(fauna: &mut Fauna, i: usize, lineage: Founder, rule: &crate::Reproduction) {
    let id = fauna.animals[i].id;
    fauna.nest_trips.remove(&id);
    fauna.nest_seek_tally[lineage.index()].give_ups += 1;
    fauna.animals[i].reproduction.retry_ticks = rule.retry_ticks();
}

/// Whether `site` holds lineage `lineage`'s egg-site rule for a clutch on the ground
/// ([`crate::EggSite`]): litter over `litter_min`, film under standing water, a fungus within
/// `egg_fungus_radius_m`, a shore egg face. A vine nester's clutch never goes on the ground.
fn egg_site_fits(fauna: &Fauna, lineage: Founder, view: &VoxelView<'_>, flora: &Flora, site: Site, rule: &crate::Reproduction) -> bool {
    let ground = flora.view().ground_at(site);
    match fauna.config.founder(lineage).egg_site {
        crate::EggSite::Litter => ground.is_some_and(|g| g.litter > rule.litter_min),
        // Under water (SNAIL-FIX): land film is grazed, not laid on; laying above
        // the waterline is not decided.
        crate::EggSite::Film => {
            ground.is_some_and(|g| g.film > 0.0) && view.standing_depth_m(i64::from(site.x), site.y, site.z) > 0.0
        }
        crate::EggSite::Fungus => fungus_near(view, flora, site, rule.egg_fungus_radius_m),
        crate::EggSite::Shore => fauna
            .config
            .founder(lineage)
            .amphibious
            .is_some_and(|am| shore_egg_site(view, &am, site)),
        // Found above, or not at all: never the ground, never a bloomcrown.
        crate::EggSite::Vine => false,
    }
}

/// **Where a refused layer walks to** (SHREDDER-NEST, [`crate::Reproduction::nest_seek`]): the
/// nearest support face within `nest_seek_m` (level distance around the ring and across,
/// faces within one `step_up_m` of its own height, so the walk is not up or down a wall) that
/// holds its egg-site rule ([`egg_site_fits`]) and whose nest is not crowded
/// ([`nest_crowded_at`], the laying's own test). Its own face is never the target. Empty
/// for a vine nester or a shore layer (each has its own trip), or with no such face. A
/// heuristic world query, as the shore trip's is.
///
/// SHRED-NEST: up to `limit` such faces, nearest first; the layer sets out for the first it
/// can walk to ([`nest_seek_target`]).
fn nest_seek_faces(fauna: &Fauna, i: usize, view: &VoxelView<'_>, flora: &Flora, rule: &crate::Reproduction, limit: usize) -> Vec<Site> {
    let a = &fauna.animals[i];
    let Some(founder) = a.founder else { return Vec::new() };
    let phys = fauna.config.founder(founder);
    if matches!(phys.egg_site, crate::EggSite::Vine | crate::EggSite::Shore) {
        return Vec::new();
    }
    let c = view.config;
    let v = c.voxel_m;
    let reach = rule.nest_seek_m.max(0.0);
    if !(reach > 0.0) {
        return Vec::new();
    }
    let k = (reach / v).ceil() as i64;
    let step = (phys.step_up_m.max(0.0) / v).floor() as u32;
    let top = (a.site.y + step).min(c.height.saturating_sub(1));
    let low = a.site.y.saturating_sub(step);
    let (hx, hz) = (i64::from(a.site.x), i64::from(a.site.z));
    let ring = f64::from(c.width) * v;
    // The faces in reach that pass the cheap tests, nearest first; the first whose nest is
    // not crowded is the target.
    let mut near: Vec<(f64, Site)> = Vec::new();
    for dz in -k..=k {
        let z = hz + dz;
        if z < 0 || z >= i64::from(c.depth) {
            continue;
        }
        for dx in -k..=k {
            let x = (hx + dx).rem_euclid(i64::from(c.width));
            let (tx, tz) = ((x as f64 + 0.5) * v, (z as f64 + 0.5) * v);
            let (ddx, ddz) = crate::pose::wrapped_delta(a.pose.x, a.pose.z, tx, tz, ring);
            let d = (ddx * ddx + ddz * ddz).sqrt();
            if !(d <= reach) {
                continue;
            }
            let Some(y) = view.highest_support_in(x, z as u32, 0, top) else {
                continue;
            };
            let s = Site { x: x as u32, y, z: z as u32 };
            if y < low || (s.x, s.z) == (a.site.x, a.site.z) || !egg_site_fits(fauna, founder, view, flora, s, rule) {
                continue;
            }
            near.push((d, s));
        }
    }
    near.sort_by(|p, q| p.0.total_cmp(&q.0));
    near.into_iter()
        .filter(|&(_, s)| !nest_crowded_at(fauna, a.id, founder, (f64::from(s.x) + 0.5) * v, (f64::from(s.z) + 0.5) * v, view, rule))
        .map(|(_, s)| s)
        .take(limit)
        .collect()
}

/// **SHRED-NEST: where a refused layer sets out for**: of [`nest_seek_faces`], nearest first,
/// the first it can walk to on its lineage's route graph ([`glider::walk_target`], as every
/// reflex walk goes). `None` when it can walk to none of them: it is refused, as with no free
/// face. It took the nearest free face before, wherever it was, and steered straight at it:
/// a face over a wall it could not climb or across a trench held it pressing at the barrier,
/// turning on the spot, until its give-up time.
fn nest_seek_target(fauna: &mut Fauna, i: usize, view: &VoxelView<'_>, flora: &Flora, rule: &crate::Reproduction) -> Option<Site> {
    let faces = nest_seek_faces(fauna, i, view, flora, rule, FACE_TRIES);
    let cands: Vec<(f64, u64, Site)> = faces.iter().map(|&s| (0.0, Fauna::walk_key_face(s), s)).collect();
    let fv = flora.view();
    let (_, k) = glider::walk_target(fauna, i, view, &fv, &cands, rule.nest_seek_m, 0.0)?;
    Some(cands[k].2)
}

/// **The walk off a crowded nest** (SHREDDER-NEST): a body on a [`crate::NestTrip`], on the
/// ground, free and awake, still ready to lay ([`trip_ready`]: a body whose reserve no longer
/// pays for its clutch goes back to feeding) walks to the trip's face, and holds still on
/// it. Committed: nothing but the trip's own end ([`end_nest_trip`], or a clutch) takes the
/// goal away, so it never turns back to the huddle between samples. Flight from a threat,
/// deep water and sleep still win (the step's chain); `None` otherwise.
///
/// SHRED-NEST: it walks by its lineage's route graph ([`glider::walk_target`]): straight at
/// the face when the line is walkable, else along the route. A walk it presses against for
/// `teachers.press_replan_s` ([`Fauna::walk_shunned`]), or a face it can no longer reach, is
/// given up ([`give_up_nest_trip`]) and its heuristic steers: it forages through its retry.
fn nest_seek_reflex(fauna: &mut Fauna, i: usize, view: &VoxelView<'_>, fv: &cubarium_voxel_flora::FloraView<'_>) -> Option<Actions> {
    let a = fauna.animals[i];
    let trip = *fauna.nest_trips.get(&a.id)?;
    if a.state == State::Asleep || a.attached.is_some() || a.mobility.mode != crate::MobilityMode::Ground {
        return None;
    }
    let sc = body::effective_config(&fauna.config, &a);
    if !trip_ready(&sc.reproduction, &a.reproduction, a.reserve, sc.reserve_of(a.body)) {
        return None;
    }
    if (a.site.x, a.site.z) == (trip.target.x, trip.target.z) {
        return Some(Actions { forward: 0.0, turn: 0.0, feed: 0.0 });
    }
    let key = Fauna::walk_key_face(trip.target);
    let cands = [(0.0, key, trip.target)];
    match glider::walk_target(fauna, i, view, fv, &cands, sc.reproduction.nest_seek_m, 0.0) {
        Some((go, _)) => {
            fauna.walking_to(a.id, key, crate::Steer::Nest);
            Some(go)
        }
        None => {
            if let Some(lineage) = a.founder {
                give_up_nest_trip(fauna, i, lineage, &sc.reproduction);
            }
            None
        }
    }
}

/// [`crate::LAY_GATES`]' indices.
const LAY_SURPLUS: usize = 0;
const LAY_INTERVAL: usize = 1;
const LAY_CLUTCH_WAIT: usize = 2;
const LAY_WALL: usize = 3;
const LAY_SITE: usize = 4;
const LAY_CROWDED: usize = 5;
const LAY_SEEKING: usize = 6;

/// ROLES5: one body-tick held at `gate`, for a founder body of breeding size (a juvenile
/// is not yet held by any gate). Instrumentation only.
fn lay_held(fauna: &mut Fauna, i: usize, sc: &SpeciesConfig, gate: usize) {
    let a = &fauna.animals[i];
    if let Some(f) = a.founder
        && a.body >= sc.birth_body
    {
        fauna.lay_refusals_by_founder[f.index()][gate] += 1;
    }
}

fn lay(
    fauna: &mut Fauna,
    i: usize,
    view: &VoxelView<'_>,
    flora: &mut Flora,
    sc: &SpeciesConfig,
    rule: &crate::Reproduction,
) {
    let seed = view.config.seed;
    end_nest_trip(fauna, i, view, flora, rule);
    // ROLES5: which gate held a breeding-size body back, counted per tick (`LAY_GATES`).
    if fauna.animals[i].reproduction.retry_ticks > 0 {
        lay_held(fauna, i, sc, LAY_CROWDED);
        return;
    }
    if !rule.hold_met(&fauna.animals[i].reproduction) {
        let gate = if fauna.animals[i].reproduction.refractory_ticks > 0 { LAY_INTERVAL } else { LAY_SURPLUS };
        lay_held(fauna, i, sc, gate);
        return;
    }
    // LIFE-HISTORY: a layer with a breeding season lays only inside its window. Gated before
    // the clutch wait (FINDINGS-FIX-2), so the wait is not spent while the window is shut.
    let year = view.year();
    if !rule.season_allows(year, season_of(fauna, i)) {
        fauna.ledger.season_waits += 1;
        return;
    }
    // **A full clutch first** (the capgnawer package): with a clutch from surplus and a
    // wait, a body short of `clutch_max` eggs keeps feeding and lays when its reserve pays
    // for the full clutch, or what it can at the end of the wait.
    if rule.clutch_max > 0 {
        let wait = rule.clutch_hold_max_ticks();
        let a = &mut fauna.animals[i];
        // Full: what this body's own reserve cap can pay for, at most `clutch_max` (D1b).
        let full = rule.full_clutch(sc.reserve_of(a.body));
        if wait > 0 && rule.clutch_for(a.reserve) < full && a.reproduction.clutch_wait_ticks < wait {
            a.reproduction.clutch_wait_ticks += 1;
            lay_held(fauna, i, sc, LAY_CLUTCH_WAIT);
            return;
        }
    }
    // Not on a wall: a clutch goes on the litter the body is standing in.
    if fauna.animals[i].mobility.climb.is_some() {
        lay_held(fauna, i, sc, LAY_WALL);
        return;
    }
    let Some(lineage) = fauna.animals[i].founder else {
        return;
    };
    let mut site = fauna.animals[i].site;
    // LARVA: a larva lineage's clutch goes into the larva field, on the host it is at (the
    // vine face it touches or the vaulttree crown it is in); with none here it keeps it.
    let larva_at = if fauna.config.founder(lineage).larva.is_some() && !rule.dormant {
        match larva_goal(fauna, i, view, flora) {
            Some(LarvaGoal::Here(at)) => Some(at),
            _ => {
                lay_held(fauna, i, sc, LAY_SITE);
                return;
            }
        }
    } else {
        None
    };
    // VINE-C: a vine nester's clutch goes on a vine face it clings to or touches, with room
    // for its hatchling to cling there; `site` becomes the support face below it.
    let egg_site = fauna.config.founder(lineage).egg_site;
    let vine = if egg_site == crate::EggSite::Vine && !rule.dormant && larva_at.is_none() {
        let found = vine_egg_site(fauna, i, view, flora, sc.body_min);
        if found.is_none() {
            lay_held(fauna, i, sc, LAY_SITE);
            return;
        }
        found
    } else {
        None
    };
    if let Some((_, under)) = vine {
        site = under;
    }
    // A dormant egg is buried in soil or litter (predation §2.13); the shredder lays in
    // the litter it lives in, the ripple-snail on the film it grazes (roster R2b,
    // [`crate::EggSite`]).
    if !rule.dormant && vine.is_none() && larva_at.is_none() {
        let fits = egg_site_fits(fauna, lineage, view, flora, site, rule);
        if !fits {
            lay_held(fauna, i, sc, LAY_SITE);
            return;
        }
    }
    // A flyer's egg needs room to fly from (bellwing3 item 2): its hatchling appears in
    // the air on this face, so the face needs `egg_headroom_m` clear over the hatchling's
    // body, and its air must reach the open air.
    if let Some(flight) = fauna.config.founder(lineage).flight.filter(|_| vine.is_none() && larva_at.is_none()) {
        let phys = *fauna.config.founder(lineage);
        let shape = crate::flight::Shape::of(&phys, &flight, sc.body_min);
        let v = view.config.voxel_m;
        let (x, z) = ((f64::from(site.x) + 0.5) * v, (f64::from(site.z) + 0.5) * v);
        let room = crate::flight::overhead_clear(view, shape, x, z, crate::surface_m(site.y, v), flight.egg_headroom_m);
        if room + crate::flight::EPS < flight.egg_headroom_m {
            lay_held(fauna, i, sc, LAY_SITE);
            return;
        }
        if !fauna.open_air(view, &flight).over_face_is_open(site) {
            lay_held(fauna, i, sc, LAY_SITE);
            return;
        }
    }
    // The clutch the reserve pays for: the fixed clutch, or with a clutch from surplus
    // the eggs above the floor (§3.1C). None affordable is not a laying, and costs nothing.
    let eggs = rule.clutch_for(fauna.animals[i].reserve);
    let cost = if rule.clutch_max > 0 {
        f64::from(eggs) * rule.egg_organic
    } else {
        rule.package_cost(sc)
    };
    if eggs == 0 || !(cost > 0.0) || fauna.animals[i].reserve < cost {
        return;
    }
    // The compute guard, then the nest site: either refuses a clutch that would have
    // been paid for, at no cost, and the body tries again later.
    if at_body_ceiling(fauna, i, rule) {
        return;
    }
    if !rule.dormant && nest_crowded(fauna, i, view, rule) {
        // SHREDDER-NEST: on its way off a crowded nest, the crowd where it stands holds it
        // (no refusal booked, no retry) until it reaches the free face; a face crowded on
        // arrival ends the walk as a refusal.
        let id = fauna.animals[i].id;
        if let Some(trip) = fauna.nest_trips.get(&id).copied() {
            let here = fauna.animals[i].site;
            if (here.x, here.z) != (trip.target.x, trip.target.z) {
                lay_held(fauna, i, sc, LAY_SEEKING);
                return;
            }
            fauna.nest_trips.remove(&id);
            fauna.nest_seek_tally[lineage.index()].give_ups += 1;
        } else if rule.nest_seek
            && let Some(target) = nest_seek_target(fauna, i, view, flora, rule)
        {
            // Refused here, it sets out for the nearest free egg face instead of waiting.
            fauna.nest_trips.insert(id, crate::NestTrip { target, since: fauna.tick });
            fauna.nest_seek_tally[lineage.index()].walk_offs += 1;
            fauna.ledger.nest_refusals_by_founder[lineage.index()] += 1;
            lay_held(fauna, i, sc, LAY_CROWDED);
            return;
        }
        fauna.animals[i].reproduction.retry_ticks = rule.retry_ticks();
        fauna.ledger.nest_refusals_by_founder[lineage.index()] += 1;
        lay_held(fauna, i, sc, LAY_CROWDED);
        return;
    }
    if fauna.nest_trips.remove(&fauna.animals[i].id).is_some() {
        fauna.nest_seek_tally[lineage.index()].walk_lays += 1;
    }
    // LIFE-HISTORY: no more than `litters_per_year` clutches a year (the window is gated
    // above).
    if let Some(y) = year.filter(|_| rule.litters_per_year > 0) {
        count_season(fauna, i, y.number);
    }
    book_first_young(fauna, i);
    let a = &mut fauna.animals[i];
    let Taken {
        organic: paid,
        mineral,
        energy,
    } = take_from_reserve(a, cost);
    // The clutch is paid out of the brood first (D1b).
    a.reproduction.brood = (a.reproduction.brood - paid).max(0.0).min(a.reserve);
    a.reproduction.given += paid;
    if let (Some(r), Some(f)) = (&mut fauna.census_readout, a.founder) {
        r.organic_given[f.index()] += paid;
    }
    a.reproduction.refractory_ticks = rule.interval_ticks();
    a.reproduction.surplus_ticks = 0;
    a.reproduction.window.reset();
    a.reproduction.clutch_wait_ticks = 0;
    let laid_tick = fauna.tick;
    if let Some(at) = larva_at {
        crate::larva::lay_eggs(fauna, flora, lineage, at.key, at.site, at.anchor, eggs, Taken { organic: paid, mineral, energy });
        fauna.ledger.eggs_laid += u64::from(eggs);
    } else if rule.dormant {
        // One clutch per egg, each with its own dormancy; the last egg takes the
        // remainder so the three currencies add back up to what was paid.
        let n = eggs.max(1);
        let (mut left_o, mut left_m, mut left_e) = (cost, mineral, energy);
        for k in 0..n {
            let last = k + 1 == n;
            let share = 1.0 / f64::from(n);
            let (o, m, e) = if last {
                (left_o, left_m, left_e)
            } else {
                (cost * share, mineral * share, energy * share)
            };
            left_o -= o;
            left_m -= m;
            left_e -= e;
            let serial = fauna.ledger.eggs_laid + fauna.ledger.eggs_buried;
            fauna.clutches.push(crate::Clutch {
                site,
                count: 1,
                organic: o,
                mineral: m,
                energy: e,
                laid_tick,
                lineage,
                dormancy: Some(crate::Dormancy::draw(&fauna.config.predation, seed, serial, laid_tick)),
                wet_ticks: 0,
                dry_ticks: 0,
                face: None,
            });
            fauna.ledger.eggs_laid += 1;
        }
    } else {
        fauna.clutches.push(crate::Clutch {
            site,
            count: eggs,
            organic: cost,
            mineral,
            energy,
            laid_tick,
            lineage,
            dormancy: None,
            wet_ticks: 0,
            dry_ticks: 0,
            face: vine.map(|(f, _)| f),
        });
        fauna.ledger.eggs_laid += u64::from(eggs);
    }
    fauna.ledger.clutches_laid += 1;
    fauna.ledger.clutches_by_founder[lineage.index()] += 1;
    if vine.is_some() {
        fauna.ledger.vine_clutches_by_founder[lineage.index()] += 1;
    }
    fauna.ledger.eggs_laid_by_founder[lineage.index()] += u64::from(eggs);
    fauna.ledger.reproduction_by_founder[lineage.index()] += cost;
}

/// **Looking for a nest in the vines** (VINE-C, [`crate::EggSite::Vine`]). For a flyer whose
/// lineage nests in the vines, awake, free and ready to lay ([`trip_ready`]): at a vine egg
/// site ([`vine_egg_site`]) whose nest is not crowded it holds still to lay (a hover, or its
/// cling); otherwise it flies to the nearest covered wall face within
/// [`crate::Flight::nest_search_m`] (level distance, rows within its band of where it is)
/// that would take its clutch and whose nest is not crowded, toward the pose it would cling
/// to it at. With none, `None`: its own steering goes on and it keeps its clutch. Never a
/// bloomcrown. `(actions, lift)`.
fn nest_reflex(fauna: &mut Fauna, i: usize, view: &VoxelView<'_>, flora: &Flora) -> Option<(Actions, f64)> {
    let a = fauna.animals[i];
    let founder = a.founder?;
    let phys = *fauna.config.founder(founder);
    if phys.egg_site != crate::EggSite::Vine || a.attached.is_some() || a.state == State::Asleep {
        return None;
    }
    let flight = phys.flight?;
    let sc = body::effective_config(&fauna.config, &a);
    let rule = sc.reproduction;
    if !trip_ready(&rule, &a.reproduction, a.reserve, sc.reserve_of(a.body)) {
        return None;
    }
    // LARVA: to the host its clutch's draw names (else the other kind), and hold there.
    let t = if phys.larva.is_some() {
        match larva_goal(fauna, i, view, flora)? {
            LarvaGoal::Here(_) => {
                fauna.ledger.nest_holds_by_founder[founder.index()] += 1;
                return Some((Actions { forward: 0.0, turn: 0.0, feed: 0.0 }, 0.0));
            }
            LarvaGoal::Fly(t) => t,
        }
    } else {
        if vine_egg_site(fauna, i, view, flora, sc.body_min).is_some() && !nest_crowded(fauna, i, view, &rule) {
            fauna.ledger.nest_holds_by_founder[founder.index()] += 1;
            return Some((Actions { forward: 0.0, turn: 0.0, feed: 0.0 }, 0.0));
        }
        nest_target(fauna, i, view, &flora.view(), flight.nest_search_m.max(0.0), |_| true)?
    };
    let v = view.config.voxel_m;
    let ring = f64::from(view.config.width) * v;
    fauna.ledger.nest_searches_by_founder[founder.index()] += 1;
    let (dx, dz) = crate::pose::wrapped_delta(a.pose.x, a.pose.z, t.x, t.z, ring);
    let cfg = fauna.config.light;
    let lift = (cfg.lift_per_m * (t.y_m - a.pose.y_m)).clamp(-1.0, 1.0);
    let dh = (dx * dx + dz * dz).sqrt();
    let forward = if dh <= cfg.near_m { cfg.near_forward } else { 1.0 };
    let want = if dh > 0.0 { dx.atan2(dz) } else { t.heading_rad };
    Some((crate::hunt::head(a.pose.heading_rad, want, forward, 1.0), lift))
}

/// The nest reflex's nearest fitting vine face (bounded by NEST_CANDIDATES).
pub(crate) fn nest_target(fauna: &mut Fauna, i: usize, view: &VoxelView<'_>, fv: &cubarium_voxel_flora::FloraView<'_>, reach: f64, keep: impl Fn(crate::Pose) -> bool) -> Option<crate::Pose> {
    nest_target_where(fauna, i, view, fv, reach, |_| true, keep)
}

/// [`nest_target`] over the vine faces `face_ok` accepts (LARVA: leafy ones).
pub(crate) fn nest_target_where(
    fauna: &mut Fauna,
    i: usize,
    view: &VoxelView<'_>,
    fv: &cubarium_voxel_flora::FloraView<'_>,
    reach: f64,
    face_ok: impl Fn(cubarium_voxel_flora::Face) -> bool,
    keep: impl Fn(crate::Pose) -> bool,
) -> Option<crate::Pose> {
    let a = fauna.animals[i];
    let founder = a.founder?;
    let phys = *fauna.config.founder(founder);
    let flight = phys.flight?;
    let sc = body::effective_config(&fauna.config, &a);
    let rule = sc.reproduction;
    let c = view.config;
    let v = c.voxel_m;
    let ring = f64::from(c.width) * v;
    let (lo, hi) = (a.pose.y_m - flight.band_max_m, a.pose.y_m + flight.band_max_m);
    let mut near: Vec<(f64, cubarium_voxel_flora::Face)> = fv
        .cover
        .wall_faces_in_columns(move |x| {
            let (dx, _) = crate::pose::wrapped_delta(
                a.pose.x,
                a.pose.z,
                (f64::from(x) + 0.5) * v,
                a.pose.z,
                ring,
            );
            // A conservative broad phase: keep a voxel of padding at the boundary,
            // then use the original distance calculation and cutoff below.
            dx.abs() <= reach + v
        })
        .filter_map(|f| {
            let y = (f64::from(f.y) + 0.5) * v;
            if y < lo || y > hi {
                return None;
            }
            let (dx, dz) = crate::pose::wrapped_delta(
                a.pose.x,
                a.pose.z,
                (f64::from(f.x) + 0.5) * v,
                (f64::from(f.z) + 0.5) * v,
                ring,
            );
            let d = (dx * dx + dz * dz).sqrt();
            (d <= reach).then_some((d, f))
        })
        .collect();
    let order = |p: &(f64, cubarium_voxel_flora::Face), q: &(f64, cubarium_voxel_flora::Face)| {
        p.0.total_cmp(&q.0).then(p.1.cmp(&q.1))
    };
    if near.len() > NEST_CANDIDATES {
        near.select_nth_unstable_by(NEST_CANDIDATES, order);
        near.truncate(NEST_CANDIDATES);
    }
    near.sort_unstable_by(order);
    let air = fauna.open_air(view, &flight);
    let mut target = None;
    // The nearest few that would take the clutch; a scan past them would cost a tick.
    for (_, face) in near.into_iter().filter(|(_, f)| face_ok(*f)).take(NEST_CANDIDATES) {
        if vine_nest_fits(view, &air, &phys, &flight, sc.body_min, face).is_none() {
            continue;
        }
        let Some((pose, _)) = crate::flight::vine_cling(view, &phys, &flight, a.body, face) else {
            continue;
        };
        if !keep(pose) { continue; }
        if nest_crowded_at(fauna, a.id, founder, pose.x, pose.z, view, &rule) {
            continue;
        }
        target = Some(pose);
        break;
    }
    target
}

/// **Where a larva lineage's clutch goes** (LARVA, `design/larva-nuts-plan-2026-10-12.md`
/// §3): the host it is at and lays on now, with its litter site and (a vine) the face.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct LarvaAt {
    pub key: crate::larva::LarvaHost,
    pub site: Site,
    pub anchor: Option<cubarium_voxel_flora::Face>,
}

/// A larva layer's goal: lay here, or fly toward this pose.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum LarvaGoal {
    Here(LarvaAt),
    Fly(crate::Pose),
}

/// **A larva layer's host** (LARVA §3): a keyed draw per clutch picks the kind, a crowned
/// vaulttree with chance `vaulttree_share`, else a latticevine; the nearest host of that kind
/// with leaves within its nest search ([`crate::Flight::nest_search_m`]) is the goal, and
/// with none in range the nearest of the other kind. At it (a leafy vine face within its
/// mouth's reach, or within `lay_reach_m` of a crown's disc and band) it lays there.
/// `None`: no host in range.
pub(crate) fn larva_goal(fauna: &mut Fauna, i: usize, view: &VoxelView<'_>, flora: &Flora) -> Option<LarvaGoal> {
    let a = fauna.animals[i];
    let lineage = a.founder?;
    let phys = *fauna.config.founder(lineage);
    let t = phys.larva?;
    let flight = phys.flight?;
    let stand_first = crate::larva::draws_vaulttree(fauna, i, view.config.seed, t.vaulttree_share);
    for stand in [stand_first, !stand_first] {
        let goal = if stand {
            larva_stand_goal(fauna, i, view, flora, &phys, &flight, &t)
        } else {
            larva_vine_goal(fauna, i, view, flora, &flight)
        };
        if goal.is_some() {
            return goal;
        }
    }
    None
}

fn larva_vine_goal(fauna: &mut Fauna, i: usize, view: &VoxelView<'_>, flora: &Flora, flight: &crate::Flight) -> Option<LarvaGoal> {
    let lineage = fauna.animals[i].founder?;
    let body = crate::larva::emerge_size(fauna, lineage).map_or(fauna.animals[i].body, |(b, _)| b);
    if let Some((face, _)) = vine_egg_site_where(fauna, i, view, flora, body, |c| c.leafiness > 0.0) {
        let fv = flora.view();
        let owner = fv.cover.face(face)?.owner;
        let root = fv.cover.vine(owner)?.root;
        return Some(LarvaGoal::Here(LarvaAt { key: crate::larva::LarvaHost::Vine(owner), site: root, anchor: Some(face) }));
    }
    let fv = flora.view();
    let leafy = |f: cubarium_voxel_flora::Face| fv.cover.face(f).is_some_and(|c| c.leafiness > 0.0);
    nest_target_where(fauna, i, view, &fv, flight.nest_search_m.max(0.0), leafy, |_| true).map(LarvaGoal::Fly)
}

#[allow(clippy::too_many_arguments)]
fn larva_stand_goal(
    fauna: &Fauna,
    i: usize,
    view: &VoxelView<'_>,
    flora: &Flora,
    phys: &crate::FounderPhysiology,
    flight: &crate::Flight,
    t: &crate::larva::Larva,
) -> Option<LarvaGoal> {
    let a = fauna.animals[i];
    let fv = flora.view();
    let v = view.config.voxel_m;
    let ring = f64::from(view.config.width) * v;
    let reach = flight.nest_search_m.max(0.0);
    let h = phys.body_at(a.body).height_m;
    // (distance, stand, crown low, crown high, crown radius)
    let mut best: Option<(f64, &cubarium_voxel_flora::Stand, f64, f64, f64)> = None;
    for s in fv.stands.iter().filter(|s| s.species == crate::larva::STAND_HOST && s.profile_stage >= 1) {
        let (sx, sz) = ((f64::from(s.site.x) + 0.5) * v, (f64::from(s.site.z) + 0.5) * v);
        let (dx, dz) = crate::pose::wrapped_delta(a.pose.x, a.pose.z, sx, sz, ring);
        let d = dx.hypot(dz);
        if d > reach || best.is_some_and(|b| d >= b.0) || !crate::larva::stand_hosts(&fv, s) {
            continue;
        }
        let Some((low, high)) = crate::larva::crown_band(&fv, s) else { continue };
        // A crown it can reach under its ceiling over the stand's own face.
        if low - crate::surface_m(s.site.y, v) > flight.band_max_m {
            continue;
        }
        let radius = fv.config.species(s.species).crown_radius_m_at(s.wood);
        best = Some((d, s, low, high, radius));
    }
    let (d, s, low, high, radius) = best?;
    let r = t.lay_reach_m.max(0.0);
    let y = a.pose.y_m;
    if d <= radius + r && y + h >= low - r && y <= high + r {
        return Some(LarvaGoal::Here(LarvaAt { key: crate::larva::LarvaHost::Stand(s.id), site: s.site, anchor: None }));
    }
    let (sx, sz) = ((f64::from(s.site.x) + 0.5) * v, (f64::from(s.site.z) + 0.5) * v);
    let ground = crate::surface_m(s.site.y, v);
    let ty = low.min(ground + flight.band_max_m - h).max(ground);
    let (dx, dz) = crate::pose::wrapped_delta(a.pose.x, a.pose.z, sx, sz, ring);
    Some(LarvaGoal::Fly(crate::Pose { x: sx, z: sz, y_m: ty, heading_rad: dx.atan2(dz) }))
}

/// How many of the nearest covered wall faces [`nest_reflex`] tries as a nest.
const NEST_CANDIDATES: usize = 24;

/// **A vine egg site** (VINE-C, [`crate::EggSite::Vine`]) for body `i`: the
/// latticevine-covered wall face it clings to, else the nearest one within its mouth's reach
/// (its radius plus its mouth reach, from its middle) that it is in front of, on which a
/// hatchling of `body_min` clings ([`crate::flight::vine_cling`]) with
/// [`crate::Flight::egg_headroom_m`] clear over it in the open air. `(the face, the support face
/// below the hatchling)`; `None` for a body that does not fly or no such face. A bloomcrown is
/// never one.
pub(crate) fn vine_egg_site(fauna: &mut Fauna, i: usize, view: &VoxelView<'_>, flora: &Flora, body_min: f64) -> Option<(cubarium_voxel_flora::Face, Site)> {
    vine_egg_site_where(fauna, i, view, flora, body_min, |_| true)
}

/// [`vine_egg_site`] over the covered faces `face_ok` accepts (LARVA: leafy ones).
pub(crate) fn vine_egg_site_where(
    fauna: &mut Fauna,
    i: usize,
    view: &VoxelView<'_>,
    flora: &Flora,
    body_min: f64,
    face_ok: impl Fn(&cubarium_voxel_flora::CoveredFace) -> bool,
) -> Option<(cubarium_voxel_flora::Face, Site)> {
    let a = fauna.animals[i];
    let phys = *fauna.config.founder(a.founder?);
    let flight = phys.flight?;
    let fv = flora.view();
    let mut faces: Vec<cubarium_voxel_flora::Face> = Vec::new();
    if let Some(crate::Perch { anchor: crate::PerchAnchor::Vine(face), .. }) = a.mobility.perch {
        faces.push(face);
    }
    let shape = crate::flight::Shape::of(&phys, &flight, a.body);
    let p = crate::flight::mouth_point(&a, shape);
    let reach = shape.r + phys.body_at(a.body).mouth_reach_m;
    let mut near = crate::flight::vine_faces_within(view, &fv, p, p, reach, |_| Some(0.0));
    near.sort_by(|x, y| x.d.total_cmp(&y.d));
    faces.extend(near.into_iter().map(|r| r.face));
    let air = fauna.open_air(view, &flight);
    faces.into_iter().find_map(|face| {
        let cf = fv.cover.face(face)?;
        if !face_ok(cf) {
            return None;
        }
        vine_nest_fits(view, &air, &phys, &flight, body_min, face).map(|under| (face, under))
    })
}

/// Whether a hatchling of `body_min` clings to vine face `face` with
/// [`crate::Flight::egg_headroom_m`] clear over it and its cell in the open air: the support
/// face below it when it does.
pub(crate) fn vine_nest_fits(
    view: &VoxelView<'_>,
    air: &crate::flight::OpenAir,
    phys: &crate::FounderPhysiology,
    flight: &crate::Flight,
    body_min: f64,
    face: cubarium_voxel_flora::Face,
) -> Option<Site> {
    let (pose, under) = crate::flight::vine_cling(view, phys, flight, body_min, face)?;
    let shape = crate::flight::Shape::of(phys, flight, body_min);
    let room = crate::flight::overhead_clear(view, shape, pose.x, pose.z, pose.y_m, flight.egg_headroom_m);
    if room + crate::flight::EPS < flight.egg_headroom_m {
        return None;
    }
    let v = view.config.voxel_m;
    let (cx, cz) = pose.column(v, view.config.depth)?;
    let cy = ((pose.y_m + 0.5 * shape.h) / v).floor();
    if cy < 0.0 {
        return None;
    }
    air.cell_is_open(cx.rem_euclid(i64::from(view.config.width)) as u32, cy as u32, cz as u32)
        .then_some(under)
}

/// **A shore egg site** (SNAIL-LAND, [`crate::EggSite::Shore`]): `site` has no standing
/// water over it, its ground moisture ([`body::ground_moisture`]) is at least
/// `egg_moisture_min`, and a column within `egg_water_max_m` (horizontal, centre to centre;
/// `x` wraps) has its highest support face at or below `site` under standing water whose
/// surface is at most `egg_climb_max_m` below `site`'s top.
pub fn shore_egg_site(view: &VoxelView<'_>, am: &crate::Amphibious, site: Site) -> bool {
    let (x, y, z) = (i64::from(site.x), site.y, site.z);
    if view.standing_depth_m(x, y, z) > 0.0 || body::ground_moisture(view, x, y, z) < am.egg_moisture_min {
        return false;
    }
    let c = view.config;
    let v = c.voxel_m;
    let reach = am.egg_water_max_m;
    if !(reach >= 0.0) || !(v > 0.0) {
        return false;
    }
    let top = crate::surface_m(y, v);
    let r = reach / v;
    let k = (r + 1e-9).floor() as i64;
    for dz in -k..=k {
        let zz = i64::from(z) + dz;
        if zz < 0 || zz >= i64::from(c.depth) {
            continue;
        }
        for dx in -k..=k {
            if ((dx * dx + dz * dz) as f64) > r * r + 1e-9 {
                continue;
            }
            let xx = (x + dx).rem_euclid(i64::from(c.width));
            let Some(wy) = view.highest_support_in(xx, zz as u32, 0, y) else {
                continue;
            };
            let depth = view.standing_depth_m(xx, wy, zz as u32);
            if depth > 0.0 && top - (crate::surface_m(wy, v) + depth) <= am.egg_climb_max_m + 1e-9 {
                return true;
            }
        }
    }
    false
}

/// **A shore egg's risks** (SNAIL-LAND): every clutch of a lineage laying on the shore
/// ([`crate::EggSite::Shore`]) with an amphibious table counts its ticks in a row under
/// standing water and on ground drier than `egg_dry_moisture`; past `egg_drown_s` or
/// `egg_dry_s` the clutch dies, its material to the litter of its face, booked drowned or
/// dried.
fn shore_eggs(fauna: &mut Fauna, view: &VoxelView<'_>, flora: &mut Flora) {
    let mut k = 0;
    while k < fauna.clutches.len() {
        let c = fauna.clutches[k];
        let phys = fauna.config.founder(c.lineage);
        let am = match (phys.egg_site, phys.amphibious) {
            (crate::EggSite::Shore, Some(am)) if c.dormancy.is_none() => am,
            _ => {
                k += 1;
                continue;
            }
        };
        let (x, y, z) = (i64::from(c.site.x), c.site.y, c.site.z);
        let under = view.standing_depth_m(x, y, z) > 0.0;
        let dry = !under && body::ground_moisture(view, x, y, z) < am.egg_dry_moisture;
        let e = &mut fauna.clutches[k];
        e.wet_ticks = if under { e.wet_ticks.saturating_add(1) } else { 0 };
        e.dry_ticks = if dry { e.dry_ticks.saturating_add(1) } else { 0 };
        let past = |ticks: u32, s: f64| f64::from(ticks) * DT > s;
        let drowned = past(e.wet_ticks, am.egg_drown_s);
        let dried = past(e.dry_ticks, am.egg_dry_s);
        if !(drowned || dried) {
            k += 1;
            continue;
        }
        let c = fauna.clutches.remove(k);
        let n = u64::from(c.count);
        fauna.ledger.eggs_died += n;
        if drowned {
            fauna.ledger.eggs_drowned_by_founder[c.lineage.index()] += n;
        } else {
            fauna.ledger.eggs_dried_by_founder[c.lineage.index()] += n;
        }
        fauna.book_deposit(
            flora,
            c.site,
            DepositKind::Litter,
            Taken {
                organic: c.organic,
                mineral: c.mineral,
                energy: c.energy,
            },
        );
    }
}

/// **A fungal egg site** ([`crate::EggSite::Fungus`]): whether a fungus holding cap tissue
/// stands on a support face within `radius_m` of `site` — around the ring, across the
/// strip, and on `site`'s own row or one up or down.
pub(crate) fn fungus_near(view: &VoxelView<'_>, flora: &Flora, site: Site, radius_m: f64) -> bool {
    let fv = flora.view();
    let v = view.config.voxel_m;
    let r = if radius_m.is_finite() && radius_m > 0.0 { radius_m } else { 0.0 };
    let reach = (r / v).floor() as i64;
    let (width, depth, height) = (i64::from(view.config.width), i64::from(view.config.depth), i64::from(view.config.height));
    for dz in -reach..=reach {
        let z = i64::from(site.z) + dz;
        if z < 0 || z >= depth {
            continue;
        }
        for dx in -reach..=reach {
            let (fx, fz) = (dx as f64 * v, dz as f64 * v);
            if fx * fx + fz * fz > r * r + 1e-12 {
                continue;
            }
            let x = (i64::from(site.x) + dx).rem_euclid(width);
            for dy in -1..=1i64 {
                let y = i64::from(site.y) + dy;
                if y < 0 || y >= height {
                    continue;
                }
                let s = Site { x: x as u32, y: y as u32, z: z as u32 };
                if fv.fungus_at(s).is_some_and(|f| f.foliage > 0.0) {
                    return true;
                }
            }
        }
    }
    false
}

/// **Replacement** (the detritus web's readout): a body born in the world laying its first
/// clutch or bearing its first young is booked with its age then. Call before the
/// package is booked as given.
fn book_first_young(fauna: &mut Fauna, i: usize) {
    let a = &fauna.animals[i];
    let Some(f) = a.founder else { return };
    if a.reproduction.native && !(a.reproduction.given > 0.0) {
        fauna.ledger.first_clutches_by_founder[f.index()] += 1;
        fauna.ledger.first_clutch_age_ticks_by_founder[f.index()] += a.age_ticks;
    }
}

/// **The body ceiling** ([`crate::FaunaConfig::lineage_body_ceiling`]), a compute guard and
/// not ecology: true when body `i`'s lineage already holds the ceiling's living bodies, in
/// which case the refusal is booked and logged, and the body waits
/// [`crate::Reproduction::nest_retry_s`] before it tries again.
fn at_body_ceiling(fauna: &mut Fauna, i: usize, rule: &crate::Reproduction) -> bool {
    let ceiling = fauna.config.lineage_body_ceiling;
    let Some(f) = fauna.animals[i].founder else {
        return false;
    };
    if ceiling == 0 {
        return false;
    }
    let living = fauna.animals.iter().filter(|a| a.founder == Some(f)).count();
    if living < ceiling as usize {
        return false;
    }
    fauna.animals[i].reproduction.retry_ticks = rule.retry_ticks();
    fauna.ledger.ceiling_refusals_by_founder[f.index()] += 1;
    eprintln!(
        "fauna compute guard: {} holds {living} bodies, at its ceiling of {ceiling}; a breeding was refused at tick {} (the ecological brakes failed)",
        f.name(),
        fauna.tick,
    );
    true
}

/// **The nest-site brake** (§3.1D): whether more of body `i`'s own lineage than
/// [`crate::Reproduction::nest_crowd_max`] — living bodies of any age, itself not counted,
/// and eggs waiting to hatch — stand within [`crate::Reproduction::nest_radius_m`] of it.
/// Off (false) at a radius of 0.
fn nest_crowded(fauna: &Fauna, i: usize, view: &VoxelView<'_>, rule: &crate::Reproduction) -> bool {
    let me = &fauna.animals[i];
    let Some(f) = me.founder else { return false };
    nest_crowded_at(fauna, me.id, f, me.pose.x, me.pose.z, view, rule)
}

/// [`nest_crowded`] for a body `me` of lineage `f` at the point `(px, pz)`, metres: the
/// lineage's other living bodies and waiting eggs within `nest_radius_m` of it, more than
/// `nest_crowd_max` (SNAIL-LAND-2: the trip asks it of a target face before walking there).
fn nest_crowded_at(fauna: &Fauna, me: u64, f: Founder, px: f64, pz: f64, view: &VoxelView<'_>, rule: &crate::Reproduction) -> bool {
    let r = rule.nest_radius_m;
    if !(r > 0.0) {
        return false;
    }
    let v = view.config.voxel_m;
    let ring = f64::from(view.config.width) * v;
    let near = |x: f64, z: f64| {
        let (dx, dz) = crate::pose::wrapped_delta(px, pz, x, z, ring);
        dx * dx + dz * dz <= r * r
    };
    let mut n: u64 = fauna
        .animals
        .iter()
        .filter(|a| a.id != me && a.founder == Some(f) && near(a.pose.x, a.pose.z))
        .count() as u64;
    for c in fauna.clutches.iter().filter(|c| c.lineage == f && c.dormancy.is_none()) {
        let (x, z) = ((f64::from(c.site.x) + 0.5) * v, (f64::from(c.site.z) + 0.5) * v);
        if near(x, z) {
            n += u64::from(c.count);
        }
    }
    n > u64::from(rule.nest_crowd_max)
}

/// A clutch whose face the terrain took away, or that standing water has covered deeper
/// than its lineage drowns in, is **lost to carrion where it lies** — the same deposit
/// path a death uses, so the material goes back to the plant layer's pools and the
/// ledger books it.
fn clutch_fate(fauna: &mut Fauna, view: &VoxelView<'_>, flora: &mut Flora) {
    if fauna.clutches.is_empty() {
        return;
    }
    let lost: Vec<crate::Clutch> = fauna
        .clutches
        .iter()
        .filter(|c| !clutch_holds(fauna, view, c))
        .copied()
        .collect();
    if lost.is_empty() {
        return;
    }
    let kept: Vec<bool> = fauna.clutches.iter().map(|c| clutch_holds(fauna, view, c)).collect();
    let mut keep = kept.into_iter();
    fauna.clutches.retain(|_| keep.next().unwrap_or(true));
    for c in lost {
        fauna.ledger.eggs_lost += u64::from(c.count);
        fauna.ledger.eggs_lost_by_founder[c.lineage.index()] += u64::from(c.count);
        fauna.book_deposit(
            flora,
            c.site,
            DepositKind::Carrion,
            Taken {
                organic: c.organic,
                mineral: c.mineral,
                energy: c.energy,
            },
        );
    }
}

/// Whether a clutch keeps its place: its face is still a support face and, unless its
/// lineage is aquatic (roster R2b: the ripple-snail lays under water by design), no
/// standing water over it is deeper than its lineage drowns in.
///
/// A clutch in the vines (VINE-C) is on its wall face, not the ground: it holds while the
/// face's cell stays solid and the air in front of it under no deeper water than its lineage
/// drowns in, the support face below playing no part.
fn clutch_holds(fauna: &Fauna, view: &VoxelView<'_>, c: &crate::Clutch) -> bool {
    let phys = fauna.config.founder(c.lineage);
    if let Some(face) = c.face {
        if !view.is_solid(i64::from(face.x), face.y, face.z) {
            return false;
        }
        let Some((fx, fy, fz)) = face.front(view) else {
            return false;
        };
        // The water standing in the eggs' row and up, in the cell in front of them.
        let depth = if fy == 0 { 0.0 } else { view.water_depth_m(i64::from(fx), fy - 1, fz) };
        return phys.aquatic || depth <= phys.core.drown_depth_m;
    }
    let (x, y, z) = (i64::from(c.site.x), c.site.y, c.site.z);
    if !view.is_support(x, y, z) {
        return false;
    }
    phys.aquatic || view.water_depth_m(x, y, z) <= phys.core.drown_depth_m
}

/// Clutches at term hatch together: `count` `body_min` juveniles at the clutch's site,
/// each holding one egg's share of what the clutch was paid, and each carrying the
/// lineage that laid it.
fn hatch(fauna: &mut Fauna, view: &VoxelView<'_>, newborns: &mut Vec<Animal>) {
    if fauna.clutches.is_empty() {
        return;
    }
    let tick = fauna.tick;
    let at_term = |fauna: &Fauna, c: &crate::Clutch| {
        let rule = fauna.config.founder(c.lineage).core.reproduction;
        c.dormancy.is_none() && tick.saturating_sub(c.laid_tick) >= rule.incubation_ticks()
    };
    let due: Vec<crate::Clutch> = fauna
        .clutches
        .iter()
        .filter(|c| at_term(fauna, c))
        .copied()
        .collect();
    if due.is_empty() {
        return;
    }
    let keep: Vec<bool> = fauna.clutches.iter().map(|c| !at_term(fauna, c)).collect();
    let mut k = 0;
    fauna.clutches.retain(|_| {
        k += 1;
        keep[k - 1]
    });
    for c in due {
        let sc = fauna.config.founder(c.lineage).core;
        // The last hatchling takes the remainder, so the clutch is divided exactly: the
        // three currencies of `count` eggs add back up to what the parent paid.
        let (mut left_o, mut left_m, mut left_e) = (c.organic, c.mineral, c.energy);
        for k in 0..c.count {
            let egg = c.egg();
            let last = k + 1 == c.count;
            let (o, m, e) = if last {
                (left_o, left_m, left_e)
            } else {
                (egg.organic, egg.mineral, egg.energy)
            };
            left_o -= o;
            left_m -= m;
            left_e -= e;
            let young = hatchling(fauna, &c, sc.body_min, o, m, e, view);
            newborns.push(young);
        }
    }
}

/// One hatchling out of an egg holding `(o, m, e)`: a `body_min` juvenile of the clutch's
/// lineage with the rest as its reserve, booked born and hatched.
fn hatchling(
    fauna: &mut Fauna,
    c: &crate::Clutch,
    body_min: f64,
    o: f64,
    m: f64,
    e: f64,
    view: &VoxelView<'_>,
) -> Animal {
    let voxel_m = view.config.voxel_m;
    let lifespan_median_s = fauna.config.founder(c.lineage).core.lifespan_median_s;
    fauna.ledger.hatched += 1;
    fauna.ledger.eggs_hatched_by_founder[c.lineage.index()] += 1;
    let parcel = Taken {
        organic: o,
        mineral: m,
        energy: e,
    };
    // VINE-C: a clutch in the vines hatches clinging to its face, where it can.
    let phys = *fauna.config.founder(c.lineage);
    let cling = match (c.face, phys.flight) {
        (Some(face), Some(flight)) => crate::flight::vine_cling(view, &phys, &flight, body_min, face).map(|p| (face, p)),
        _ => None,
    };
    let (site, pose) = match cling {
        Some((_, (pose, under))) => (under, pose),
        None => (c.site, crate::Pose::at_site(c.site, voxel_m)),
    };
    let mut young = newborn(
        fauna,
        (crate::Species::Frondgrazer, Some(c.lineage)),
        (site, pose),
        body_min,
        parcel,
        lifespan_median_s,
    );
    if let Some((face, _)) = cling {
        young.mobility.mode = crate::MobilityMode::Perched;
        young.mobility.perch = Some(crate::Perch {
            kind: crate::PerchKind::VineFace,
            anchor: crate::PerchAnchor::Vine(face),
        });
    }
    young
}

/// **The dormant egg bank's tick** (predation §2.13): each dormant egg hears the prey of
/// its lineage that pass within 2 m loudly enough ([`crate::PredationConfig::footfall_tremor_min`], the
/// tremor rule); an egg whose viability has run out, or whose upkeep has taken it below a
/// hatchling, dies to the litter of its face; and at **dusk** an egg whose footfall over
/// the last clock-day has reached its threshold hatches, unless an adult of its lineage
/// is within `hatch_suppress_m` ([`crate::egg_hatches`]).
fn dormant_eggs(
    fauna: &mut Fauna,
    view: &VoxelView<'_>,
    flora: &mut Flora,
    dusk: bool,
    newborns: &mut Vec<Animal>,
) {
    if !fauna.clutches.iter().any(|c| c.dormancy.is_some()) {
        return;
    }
    let tick = fauna.tick;
    let v = view.config.voxel_m;
    let ring = f64::from(view.config.width) * v;
    let day_ticks = cubarium_voxel::ticks_at_least_one(view.config.day.length_min * 60.0);
    let pc = fauna.config.predation;
    let mut k = 0;
    while k < fauna.clutches.len() {
        let c = fauna.clutches[k];
        let Some(mut d) = c.dormancy else {
            k += 1;
            continue;
        };
        let phys = *fauna.config.founder(c.lineage);
        if tick >= d.viable_until_tick || c.organic < phys.core.body_min {
            fauna.clutches.remove(k);
            fauna.ledger.eggs_died += 1;
            fauna.book_deposit(
                flora,
                c.site,
                DepositKind::Litter,
                Taken {
                    organic: c.organic,
                    mineral: c.mineral,
                    energy: c.energy,
                },
            );
            continue;
        }
        let (ex, ez) = ((f64::from(c.site.x) + 0.5) * v, (f64::from(c.site.z) + 0.5) * v);
        let mut nearest_adult: Option<f64> = None;
        for a in &fauna.animals {
            let Some(f) = a.founder else { continue };
            let (dx, dz) = crate::pose::wrapped_delta(ex, ez, a.pose.x, a.pose.z, ring);
            let dist = (dx * dx + dz * dz).sqrt();
            if f == c.lineage && a.body >= phys.core.birth_body {
                nearest_adult = Some(nearest_adult.map_or(dist, |n: f64| n.min(dist)));
            }
            let prey = phys.active_hunter().is_some_and(|h| h.hunts(f));
            // Footfall is feet on the ground: a flyer aloft or a glider in the air makes none.
            let on_foot = !matches!(a.mobility.mode, crate::MobilityMode::Fly | crate::MobilityMode::Glide);
            if prey
                && on_foot
                && dist <= pc.footfall_radius_m
                && crate::tremor_of(a, dist, pc.tremor_near_m) >= pc.footfall_tremor_min
            {
                d.hear(a.id, tick);
            }
        }
        fauna.clutches[k].dormancy = Some(d);
        // The threshold gate, then the per-dusk draw (bet-hedging across dusks).
        if dusk
            && crate::egg_hatches(&pc, d.threshold, d.footfall(tick, day_ticks), dusk, nearest_adult)
            && crate::hatch_draw(
                fauna.config.hatch_chance_per_dusk,
                view.config.seed,
                d.serial,
                tick,
            )
        {
            // A hatchling needs a host (parasite §1): with none in reach the egg stays in
            // the bank, its viability and footfall running on, and tries a later dusk.
            if let Some(host) = host_for(fauna, newborns, &c, (ex, ez), ring) {
                fauna.clutches.remove(k);
                let (o, m, e) = (c.organic, c.mineral, c.energy);
                let mut young = hatchling(fauna, &c, phys.core.body_min, o, m, e, view);
                young.attached = Some(crate::Attachment { host: host.id });
                young.site = host.site;
                young.pose = host.pose;
                fauna.ledger.riders_attached += 1;
                newborns.push(young);
                continue;
            }
        }
        k += 1;
    }
}

/// **A hatching egg's host** (parasite §1): [`bearing_host`] for a `body_min` hatchling of
/// the egg's lineage within `parasite_attach_radius_m` of the egg's column centre.
fn host_for(
    fauna: &Fauna,
    newborns: &[Animal],
    c: &crate::Clutch,
    at: (f64, f64),
    ring: f64,
) -> Option<Animal> {
    let body = fauna.config.founder(c.lineage).core.body_min;
    bearing_host(fauna, newborns, c.lineage, body, at, fauna.config.parasite_attach_radius_m, ring)
}

/// **The nearest host that bears a rider** (parasite §1; the one rule both a hatching egg
/// and an outgrown rider looking for a new host, LJ-HOSTSWITCH, take a host by): the
/// nearest living animal not of the rider's `lineage`, and not itself riding, within
/// `radius` of `(x, z)` (wrapped on the ring, horizontal), carrying fewer than
/// `parasite_max_per_host` riders — `newborns` counted — and able to bear a rider of
/// `rider_body` on its current body ([`crate::host_bears`], LANTERNJAW-LIFE). A tie goes
/// to the lower id.
pub(crate) fn bearing_host(
    fauna: &Fauna,
    newborns: &[Animal],
    lineage: crate::Founder,
    rider_body: f64,
    (x, z): (f64, f64),
    radius: f64,
    ring: f64,
) -> Option<Animal> {
    let most = fauna.config.parasite_max_per_host;
    let riders = |id: u64| {
        fauna
            .animals
            .iter()
            .chain(newborns)
            .filter(|a| a.attached == Some(crate::Attachment { host: id }))
            .count()
    };
    let mut best: Option<(f64, Animal)> = None;
    for a in &fauna.animals {
        if a.founder == Some(lineage) || a.attached.is_some() {
            continue;
        }
        let (dx, dz) = crate::pose::wrapped_delta(x, z, a.pose.x, a.pose.z, ring);
        let d = (dx * dx + dz * dz).sqrt();
        if d > radius || best.is_some_and(|(b, _)| d >= b) {
            continue;
        }
        // LANTERNJAW-LIFE: a host that cannot bear the rider is no host.
        if !crate::host_bears(&fauna.config, rider_body, a.body) {
            continue;
        }
        if riders(a.id) >= most as usize {
            continue;
        }
        best = Some((d, *a));
    }
    best.map(|(_, a)| a)
}

/// The standing water over a body's feet, metres: the water on its face on the ground;
/// on a wall, the water on the lower face less the feet's height over it (up a wall from
/// a flooded foot it climbs out; down a cliff into a pool it goes under as it nears the
/// foot).
fn water_over_feet(view: &VoxelView<'_>, a: &Animal) -> f64 {
    let depth = |s: Site| view.water_depth_m(i64::from(s.x), s.y, s.z);
    // A flyer's feet are at its base, over the support face below it (F1).
    if matches!(a.mobility.mode, crate::MobilityMode::Fly | crate::MobilityMode::Perched) {
        return depth(a.site) - (a.pose.y_m - crate::surface_m(a.site.y, view.config.voxel_m));
    }
    match a.mobility.climb {
        None => depth(a.site),
        Some(c) if c.ascending => depth(c.from) - c.progress_m,
        Some(c) => depth(c.to) - (c.rise_m - c.progress_m),
    }
}

/// **The shallows reflex** (roster mechanics fix 2): a walker on the ground (not aquatic)
/// standing in water deeper than it wades — put down in it, the water rose round it, or a
/// starving body shrank under the water it stood in — walks to the nearest support face
/// within [`crate::LifeConfig::shallows_search_m`] (horizontally, `x` wrapped) whose water
/// it wades, at most its step up above its own face, where its whole disc stands clear
/// and its centre is inside the strip's `z` ends. A heuristic world query, not a sense:
/// read by this reflex only, never an observation. `None` when it is not in too-deep water
/// or no such face is in reach.
fn shallows_reflex(fauna: &mut Fauna, i: usize, view: &VoxelView<'_>) -> Option<Actions> {
    let a = fauna.animals[i];
    let founder = a.founder?;
    let phys = *fauna.config.founder(founder);
    if phys.aquatic || a.mobility.mode != crate::MobilityMode::Ground || a.attached.is_some() {
        return None;
    }
    let geometry = phys.body_at(a.body);
    let wade = phys.wade_depth_m(&geometry);
    let here = view.water_depth_m(i64::from(a.site.x), a.site.y, a.site.z);
    if here <= wade {
        return None;
    }
    let face = shallows_target(fauna, i, view, fauna.config.life.shallows_search_m, |_| true)?;
    fauna.walking_to(a.id, Fauna::walk_key_face(face), crate::Steer::Shallows);
    Some(walk_to_place(&a, view, face, 0.0))
}

/// The shallows reflex's own way-on/standable-face query, without steering writes.
pub(crate) fn shallows_target(fauna: &mut Fauna, i: usize, view: &VoxelView<'_>, reach: f64, keep: impl Fn(Site) -> bool) -> Option<Site> {
    let a = fauna.animals[i];
    let founder = a.founder?;
    let phys = *fauna.config.founder(founder);
    let geometry = phys.body_at(a.body);
    let wade = phys.wade_depth_m(&geometry);
    let c = view.config;
    let v = c.voxel_m;
    let r = geometry.footprint_radius();
    let depth_m = f64::from(c.depth) * v;
    let limits = phys.step_limits(v);
    let k = (reach / v).ceil() as i64;
    let up = (phys.step_up_m / v).floor().max(0.0) as u32;
    let (hx, hz) = (i64::from(a.site.x), i64::from(a.site.z));
    let ring = f64::from(c.width) * v;
    let top = (a.site.y + up).min(c.height.saturating_sub(1));
    // EDGE: a face it pressed toward without a break is given up (`teachers.press_replan_s`).
    let shunned = fauna.walk_shunned(a.id, founder);
    // EDGE: off its lineage's route graph (whose faces are all within the adult's wade), its
    // own body's way back onto it (`RouteGraph::way_on`: its disc, its step, water no deeper
    // than it stands in) is the way out of the water, where one is within reach.
    {
        let graph = fauna.routes(view, founder, fauna.config.teachers.route_refresh_s);
        let own = (r / v).ceil().max(0.0) as u32;
        if graph.node_near(a.site, (a.pose.x, a.pose.z), own).is_none() {
            let cols = if reach.is_finite() && reach > 0.0 { (reach / v).ceil() as u32 } else { 0 };
            let way = graph.way_on(view, a.site, (a.pose.x, a.pose.z), r, geometry.headroom_voxels(v), limits, wade, own, cols);
            if let Some(f) = way.filter(|f| !shunned.contains(&Fauna::walk_key_face(*f)) && keep(*f)) {
                return Some(f);
            }
        }
    }
    // Seed the nearest search locally, then bound the rescan by that candidate.
    // Rank ties by the original z/dx scan, including its first occurrence when
    // the search square wraps around x more than once. `keep` is a pure predicate.
    let width = i64::from(c.width);
    let order = |s: Site| (s.z, (i64::from(s.x) - hx + k).rem_euclid(width) - k);
    let choose = |x: i64, z: i64, best: &mut Option<(f64, f64, f64, Site)>| {
        if z < 0 || z >= i64::from(c.depth) || z < hz - k || z > hz + k {
            return;
        }
        let x = x.rem_euclid(width);
        let first_dx = (x - hx + k).rem_euclid(width) - k;
        if first_dx > k {
            return; // This column was outside the original search square.
        }
        let tx = (x as f64 + 0.5) * v;
        let tz = (z as f64 + 0.5) * v;
        let (ddx, ddz) = crate::pose::wrapped_delta(a.pose.x, a.pose.z, tx, tz, ring);
        let d = (ddx * ddx + ddz * ddz).sqrt();
        if !(d <= reach && best.is_none_or(|(b, _, _, s)| d < b || (d == b && (z as u32, first_dx) < order(s)))) {
            return;
        }
        let Some(y) = view.highest_support_in(x, z as u32, 0, top) else {
            return;
        };
        if view.water_depth_m(x, y, z as u32) > wade {
            return;
        }
        // Only where the body itself can stand: its centre inside the strip's z
        // ends, its disc clear over the face. Sight rejects range/FOV cheaply.
        if tz < r || tz > depth_m - r {
            return;
        }
        let face = Site { x: x as u32, y, z: z as u32 };
        if shunned.contains(&Fauna::walk_key_face(face)) || !keep(face) || !body::disc_clear_on_face(view, &geometry, face, limits) {
            return;
        }
        *best = Some((d, ddx, ddz, face));
    };
    let mut best = None;
    // A nearby face commonly wins on dry ground. This only seeds a bound; it
    // neither returns early nor replaces the original search if nothing is seen.
    let local = k.clamp(0, 1);
    for dz in -local..=local {
        for dx in -local..=local {
            choose(hx + dx, hz + dz, &mut best);
        }
    }
    if let Some((d, _, _, _)) = best {
        // A winning face lies inside this square about the actual pose. Pad by
        // one column so rounding at its boundary cannot exclude an equal tie.
        let span = (d / v).ceil() as i64 + 1;
        let cx = (a.pose.x / v).floor() as i64;
        let cz = (a.pose.z / v).floor() as i64;
        for z in (cz - span).max(hz - k).max(0)..=(cz + span).min(hz + k).min(i64::from(c.depth) - 1) {
            for x in cx - span..=cx + span {
                choose(x, z, &mut best);
            }
        }
    } else {
        for dz in -k..=k {
            for dx in -k..=k {
                choose(hx + dx, hz + dz, &mut best);
            }
        }
    }
    best.map(|(_, _, _, face)| face)
}

/// **The drying clock** (AMPHIBIOUS-SNAIL, [`crate::Amphibious`]): every amphibious body's
/// retraction counts down; one allowed on land rehydrates in water (standing water
/// within its `water_margin_m`, or riding) and dries out of it at the rate its face's
/// [`body::ground_moisture`] sets, times the day factor while the sun is up (clock on,
/// daylight at or above the day line) and the rain factor while rain falls. The
/// readouts count its ticks in water and ashore, by day and by night.
fn desiccation(fauna: &mut Fauna, view: &VoxelView<'_>, sky: &Sky) {
    let day = sky.clock_on && sky.daylight >= fauna.config.life.day_line;
    let raining = view.is_raining();
    if raining {
        fauna.last_rain_tick = Some(fauna.tick);
    }
    for i in 0..fauna.animals.len() {
        let a = fauna.animals[i];
        let Some(f) = a.founder else { continue };
        let phys = fauna.config.founder(f);
        let Some(amph) = phys.amphibious else { continue };
        let m = &mut fauna.animals[i].mobility;
        m.retract_ticks = m.retract_ticks.saturating_sub(1);
        if !amph.on_land {
            m.ashore = false;
            m.thirsty = false;
            continue;
        }
        let (x, y, z) = (i64::from(a.site.x), a.site.y, a.site.z);
        let in_water = a.attached.is_some() || body::water_within(view, x, y, z, phys.water_margin_m);
        if in_water {
            m.dryness = (m.dryness - amph.rehydrate_per_s * DT).max(0.0);
        } else {
            let mut rate = amph.drying_per_s(body::ground_moisture(view, x, y, z));
            if day {
                rate *= amph.day_drying_factor;
            }
            if raining {
                rate *= amph.rain_drying_factor;
            }
            if rate.is_finite() {
                m.dryness += rate * DT;
            } else {
                m.dryness = amph.lethal_fraction;
            }
        }
        m.ashore = !in_water;
        let back_at = if day && !raining { amph.day_return_fraction } else { amph.return_fraction };
        m.thirsty = m.dryness > back_at;
        fauna.ledger.amphibious_ticks[usize::from(!day)][usize::from(!in_water)] += 1;
    }
}

/// Whether `a`'s drying clock has reached its lineage's `lethal_fraction`.
fn desiccated(config: &crate::FaunaConfig, a: &Animal) -> bool {
    a.founder
        .and_then(|f| config.founder(f).amphibious)
        .is_some_and(|am| am.on_land && a.mobility.dryness >= am.lethal_fraction)
}

/// Step 7: starvation, drowning and old age, and the carrion they leave.
fn deaths(fauna: &mut Fauna, view: &VoxelView<'_>, flora: &mut Flora) {
    // A founder drowns **by its own height and over time** (package mobility, decision
    // 3): each tick the water over its feet is deeper than its drown depth advances its
    // count, any other tick resets it, and it drowns when the count reaches the lineage's
    // `drown_ticks`. A body with no lineage keeps the instant rule on `drown_depth_m`.
    let mut drowning: Vec<bool> = Vec::with_capacity(fauna.animals.len());
    for i in 0..fauna.animals.len() {
        let a = fauna.animals[i];
        let sc = body::effective_config(&fauna.config, &a);
        // A rider is on its host's back, above the water its host wades: its fate there
        // is its host's (parasite §2).
        if a.attached.is_some() {
            fauna.animals[i].mobility.submerged_ticks = 0;
            drowning.push(false);
            continue;
        }
        let Some(founder) = a.founder else {
            drowning.push(
                view.water_depth_m(i64::from(a.site.x), a.site.y, a.site.z) > sc.drown_depth_m,
            );
            continue;
        };
        let phys = fauna.config.founder(founder);
        let geometry = phys.body_at(a.body);
        // An aquatic body (roster R2b) never drowns.
        let under = !phys.aquatic && water_over_feet(view, &a) > phys.drown_depth_m(&geometry);
        let m = &mut fauna.animals[i].mobility;
        m.submerged_ticks = if under { m.submerged_ticks + 1 } else { 0 };
        drowning.push(m.submerged_ticks >= phys.drown_ticks());
    }
    // The cause is read off the same two clauses the rule is made of, in the order the
    // rule reads them: this is a label on an existing decision, not a second decision.
    // A body that satisfies both is `Starved`, which the `else if` fixes explicitly.
    let dying: Vec<(usize, Departure)> = fauna
        .animals
        .iter()
        .enumerate()
        .filter_map(|(i, a)| {
            let sc = body::effective_config(&fauna.config, a);
            // PUP: a pup of a lineage whose pups live on their reserve starves when it is
            // spent and its stomach is empty.
            let pup_spent = a.pack.is_some_and(|m| m.pup)
                && a.founder.and_then(|f| fauna.config.founder(f).pack).is_some_and(|t| t.pup_starves_on_empty)
                && !(a.reserve > 0.0)
                && !(a.fuel > 0.0)
                && !(a.stomach.organic() > 0.0);
            if a.body < sc.body_min || pup_spent {
                Some((i, Departure::Starved))
            } else if drowning[i] {
                Some((i, Departure::Drowned))
            } else if desiccated(&fauna.config, a) {
                Some((i, Departure::Desiccation))
            } else if a.lifespan_ticks > 0 && a.age_ticks >= a.lifespan_ticks {
                Some((i, Departure::Aged))
            } else if a.wound >= 1.0 {
                // DROP: a hit that wounds to 1 kills at once (`apply_hit`); a wound of 1 here
                // is a jump down's.
                Some((i, Departure::Fell))
            } else {
                None
            }
        })
        .collect();
    if dying.is_empty() {
        return;
    }
    // A body that dies carrying an escrow loses the gestation first: the loss fraction
    // is respired and the rest is back in the reserve before the corpse is weighed, so
    // "the rest returns to the reserve (or the corpse)" is one path and not two.
    for &(i, _) in &dying {
        let rule = body::effective_config(&fauna.config, &fauna.animals[i]).reproduction;
        fail_gestation(fauna, i, &rule);
    }
    let dead: Vec<(Animal, Departure)> = dying
        .iter()
        .map(|&(i, cause)| (fauna.animals[i], cause))
        .collect();
    let ids: Vec<u64> = dead.iter().map(|(a, _)| a.id).collect();
    fauna.animals.retain(|a| !ids.contains(&a.id));
    for id in &ids {
        // A dead body's controller goes with it. Ids are never reused, so a table that
        // only ever grows is a leak once the world breeds and buries.
        fauna.controllers.take(*id);
        fauna.trapped.remove(id);
        fauna.drop_plans.remove(id);
        fauna.wading.remove(id);
        fauna.flushes.remove(id);
        fauna.forage.remove(id);
        fauna.rove.remove(id);
        fauna.nest_trips.remove(id);
    }
    for (a, cause) in &dead {
        sp_miss_depart(fauna, a, *cause);
        if a.pack.is_some_and(|m| m.pup) {
            if *cause == Departure::Starved {
                fauna.ledger.pups_starved += 1;
            } else {
                fauna.ledger.pups_lost_other += 1;
            }
        }
        bury_corpse(fauna, flora, a, *cause);
    }
    // A host's riders are freed where it fell, whatever their size (parasite §5).
    for (a, _) in &dead {
        fauna.free_riders(a, Some(view));
    }
}

#[cfg(test)]
#[path = "capgnawer_tests.rs"]
mod capgnawer_tests;
#[cfg(test)]
#[path = "taste_tests.rs"]
mod taste_tests;
#[cfg(test)]
#[path = "circadian_tests.rs"]
mod circadian_tests;

#[cfg(test)]
#[path = "den_rest_birth_tests.rs"]
mod den_rest_birth_tests;
#[cfg(test)]
#[path = "detritivore_tests.rs"]
mod detritivore_tests;
#[cfg(test)]
#[path = "shredder_nest_tests.rs"]
mod shredder_nest_tests;

#[cfg(test)]
#[path = "shred_nest_tests.rs"]
mod shred_nest_tests;
#[cfg(test)]
#[path = "generation_tests.rs"]
mod generation_tests;
#[cfg(test)]
#[path = "dung_tests.rs"]
mod dung_tests;
#[cfg(test)]
#[path = "predation_fixture.rs"]
mod predation_fixture;
#[cfg(test)]
#[path = "predation_tests.rs"]
mod predation_tests;
#[cfg(test)]
#[path = "predation_senses_tests.rs"]
mod predation_senses_tests;
#[cfg(test)]
#[path = "predation_life_tests.rs"]
mod predation_life_tests;
#[cfg(test)]
#[path = "parasite_tests.rs"]
mod parasite_tests;
#[cfg(test)]
#[path = "diet_tests.rs"]
mod diet_tests;
#[cfg(test)]
#[path = "chorister_tests.rs"]
mod chorister_tests;
#[cfg(test)]
#[path = "life_history_tests.rs"]
mod life_history_tests;
#[cfg(test)]
#[path = "pup_tests.rs"]
mod pup_tests;
#[cfg(test)]
#[path = "night_survival_tests.rs"]
mod night_survival_tests;
#[cfg(test)]
#[path = "gut_tests.rs"]
mod gut_tests;

#[cfg(test)]
#[path = "fuel_tests.rs"]
mod fuel_tests;
#[cfg(test)]
#[path = "hunt_tests.rs"]
mod hunt_tests;
#[cfg(test)]
#[path = "hunt_meal_tests.rs"]
mod hunt_meal_tests;
#[cfg(test)]
#[path = "lj_hunt_tests.rs"]
mod lj_hunt_tests;
#[cfg(test)]
#[path = "pack_search_tests.rs"]
mod pack_search_tests;
#[cfg(test)]
#[path = "senses_core_tests.rs"]
mod senses_core_tests;
#[cfg(test)]
#[path = "rhythm_tests.rs"]
mod rhythm_tests;
#[cfg(test)]
#[path = "light_tests.rs"]
mod light_tests;
#[cfg(test)]
#[path = "nectar_scent_tests.rs"]
mod nectar_scent_tests;
#[cfg(test)]
#[path = "light_learning_tests.rs"]
mod light_learning_tests;
#[cfg(test)]
#[path = "flower_fruit_light_tests.rs"]
mod flower_fruit_light_tests;
#[cfg(test)]
#[path = "bellwing_lure_tests.rs"]
mod bellwing_lure_tests;
#[cfg(test)]
#[path = "lure_dusk_tests.rs"]
mod lure_dusk_tests;

#[cfg(test)]
#[path = "ground_response_tests.rs"]
mod ground_response_tests;
#[cfg(test)]
#[path = "calls_rest_tests.rs"]
mod calls_rest_tests;
#[cfg(test)]
#[path = "brain_rest_tests.rs"]
mod brain_rest_tests;
#[cfg(test)]
#[path = "edge_tests.rs"]
mod edge_tests;
#[cfg(test)]
#[path = "fear_tests.rs"]
mod fear_tests;
#[cfg(test)]
#[path = "fear2_tests.rs"]
mod fear2_tests;
#[cfg(test)]
#[path = "drop_tests.rs"]
mod drop_tests;
#[cfg(test)]
#[path = "route_walk_tests.rs"]
mod route_walk_tests;
#[cfg(test)]
#[path = "hunker_tests.rs"]
mod hunker_tests;
#[cfg(test)]
#[path = "gait_tests.rs"]
mod gait_tests;

#[cfg(test)]
#[path = "body_actions_tests.rs"]
mod body_actions_tests;
#[cfg(test)]
#[path = "testpass_vigil_tests.rs"]
mod testpass_vigil_tests;
#[cfg(test)]
#[path = "testpass_pack_tests.rs"]
mod testpass_pack_tests;
#[cfg(test)]
#[path = "churn_tests.rs"]
mod churn_tests;
#[cfg(test)]
#[path = "strike_tests.rs"]
mod strike_tests;
#[cfg(test)]
#[path = "testpass_light_tests.rs"]
mod testpass_light_tests;
#[cfg(test)]
#[path = "testpass_snail_tests.rs"]
mod testpass_snail_tests;
#[cfg(test)]
#[path = "testpass2_pack_tests.rs"]
mod testpass2_pack_tests;
#[cfg(test)]
#[path = "testpass2_life_tests.rs"]
mod testpass2_life_tests;
#[cfg(test)]
#[path = "testpass2_light_tests.rs"]
mod testpass2_light_tests;
#[cfg(test)]
#[path = "testpass2_edge_tests.rs"]
mod testpass2_edge_tests;
#[cfg(test)]
#[path = "testpass4_pup_tests.rs"]
mod testpass4_pup_tests;
#[cfg(test)]
#[path = "testpass4_mimic_tests.rs"]
mod testpass4_mimic_tests;
#[cfg(test)]
#[path = "testpass4_glowcap_tests.rs"]
mod testpass4_glowcap_tests;

#[cfg(test)]
#[path = "mimic_tests.rs"]
mod mimic_tests;

#[cfg(test)]
#[path = "hunt_escape_tests.rs"]
mod hunt_escape_tests;

#[cfg(test)]
#[path = "traps_tests.rs"]
mod traps_tests;
#[cfg(test)]
#[path = "sp_protein_tests.rs"]
mod sp_protein_tests;

/// Begin the common windup, used by walking and perched catchers.
pub(crate) fn start_strike(fauna: &mut Fauna, i: usize, prey: u64) {
    let founder = fauna.animals[i].founder.expect("founder");
    let phys = *fauna.config.founder(founder);
    let h = phys.active_hunter().expect("a hunter");
    let lure = crate::light::drawn_to_lure(fauna, prey);
    let a = &mut fauna.animals[i];
    a.strike = Some(Strike {
        phase: StrikePhase::Windup,
        ticks_left: crate::Hunter::ticks(h.windup_s),
        prey,
        from_x: a.pose.x,
        from_z: a.pose.z,
        lure,
    });
    a.state = State::Resting;
    fauna.ledger.strikes_by_founder[founder.index()] += 1;
    if let Ok(j) = fauna.animals.binary_search_by_key(&prey, |p| p.id) {
        if let Some(pf) = fauna.animals[j].founder {
            fauna.ledger.strikes_on_by_founder[pf.index()] += 1;
            fauna.ledger.funnel[founder.index()][pf.index()][crate::perception::STRUCK] += 1;
            if let Some(r) = &mut fauna.census_readout {
                r.predation.push(crate::CensusPredation { prey_id: prey, predator: founder, prey: pf, prey_body: fauna.animals[j].body, kill: false });
            }
            // D2b readout: a patroller's strike its choice did not accept.
            let choosy =
                fauna.config.hunt.patrol && (founder == crate::Founder::Lanternjaw && fauna.controllers.layer(fauna.animals[i].id) == crate::Layer::Heuristic);
            if choosy
                && !crate::hunt::accepts(
                    &fauna.config,
                    &fauna.animals[i],
                    &fauna.animals[j],
                    fauna.tick,
                )
            {
                fauna.ledger.strikes_unchosen_by_prey[pf.index()] += 1;
            }
        }
    }
    if let Ok(j) = fauna.animals.binary_search_by_key(&prey, |p| p.id) {
        let a = &fauna.animals[i];
        let p = &fauna.animals[j];
        if sp_miss_tap(a, p) {
            eprintln!("SPMISS start tick={} hunter={} prey={} lure={} ax={} az={} ay={} px={} pz={} py={} mode={:?} hbody={} pbody={}", fauna.tick, a.id, p.id, lure, a.pose.x, a.pose.z, a.pose.y_m, p.pose.x, p.pose.z, p.pose.y_m, p.mobility.mode, a.body, p.body);
        }
    }
    // R3: a pack member calls as it strikes.
    crate::pack::call(fauna, i);
}

#[cfg(test)]
#[path = "dispersal_tests.rs"]
mod dispersal_tests;
#[cfg(test)]
#[path = "roles5_tests.rs"]
mod roles5_tests;
#[cfg(test)]
#[path = "size_not_mass_step_tests.rs"]
mod size_not_mass_tests;

#[cfg(test)]
#[path = "sp_loop_tests.rs"]
mod sp_loop_tests;

fn sp_miss_tap(a: &Animal, p: &Animal) -> bool {
    sp_miss_enabled()
        && a.founder == Some(crate::Founder::Seedporter)
        && p.founder == Some(crate::Founder::Bellwing)
}

fn sp_miss_depart(fauna: &Fauna, a: &Animal, cause: Departure) {
    if !sp_miss_enabled() { return; }
    if a.founder == Some(crate::Founder::Bellwing) {
        eprintln!("SPMISS departure tick={} prey={} cause={:?}", fauna.tick, a.id, cause);
    }
    if a.founder == Some(crate::Founder::Seedporter) {
        if let Some(s) = a.strike.filter(|s| s.phase != StrikePhase::Recovery) {
            eprintln!("SPMISS abort tick={} hunter={} prey={} cause={:?}", fauna.tick, a.id, s.prey, cause);
        }
    }
}

/// Opt-in strike outcome diagnostic (`SP_MISS_TAP=1`, unset by default). Records
/// seedporter/bellwing starts, contact geometry/mobility, defence, damage and
/// departures to stderr. The environment is read once; the disabled path is a
/// cached boolean check. Used by `runs/sp-miss/summarize.py` for local probes.
fn sp_miss_enabled() -> bool {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ON.get_or_init(|| std::env::var_os("SP_MISS_TAP").is_some())
}
