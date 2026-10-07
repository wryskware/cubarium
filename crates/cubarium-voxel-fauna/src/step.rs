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
//!    A parasite rider then **takes** from its host (`parasite_take_per_day` × the
//!    host's body per ecological day, by the same pro-rata rule). Then **growth**: the reserve pays
//!    for new structure at no more than `growth_max_per_s`, scaled by how full it is,
//!    until `body_max`; a rider grown to its detach size **drops off**.
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
use crate::controller::Actions;
use crate::manifest::Founder;
use crate::predation::{Hit, Rest, Strike, StrikePhase};
use crate::{Animal, Departure, Fauna, Food, IntervalFeedback, SpeciesConfig, State};

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
pub(crate) use glider::{OffGraph, glider_fruit_in_reach, off_graph_exit};
#[path = "jump.rs"]
pub(crate) mod jump;

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
        circadian(fauna, &view, clock_on, daylight);
        maintenance(fauna);
        drain_riders(fauna);
        // The minimal gut: the stomach digests into the room upkeep and the rider's
        // take just made.
        digest(fauna);
        heal(fauna);
        egg_upkeep(fauna);
        grow(fauna);
        drop_off_grown(fauna);
        void_guts(fauna, flora);
        glider::void_gut_seeds(fauna, flora);
        // R3: calls, pups' independence, succession and provisioning.
        crate::pack::upkeep(fauna, &view);
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
        controllers(fauna, &view, flora, senses, &start.sky, threads);
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
            }
            if let Some(f) = a.founder
                && a.state != State::Asleep
                && a.attached.is_none()
            {
                fauna.ledger.awake_ticks_by_founder[f.index()] += 1;
                if a.eating {
                    fauna.ledger.wanting_ticks_by_founder[f.index()] += 1;
                }
            }
        }
        act(fauna, &view, flora, seed, tick, &sky, &mut hits);
        let ring = f64::from(view.config.width) * view.config.voxel_m;
        // HUNKER: the pack's work on its hunkered prey, before this tick's strikes land.
        crate::vigil::pry(fauna, ring);
        for hit in hits {
            let stood = fauna
                .animals
                .binary_search_by_key(&hit.prey, |a| a.id)
                .ok()
                .map(|j| (fauna.animals[j].pose.x, fauna.animals[j].pose.z));
            apply_hit(fauna, flora, hit.prey, hit.damage, hit.by);
            // HUNTER-MEAL: a kill marks its place productive in the striker's memory.
            if let Some((x, z)) = stood {
                if fauna.animals.binary_search_by_key(&hit.prey, |a| a.id).is_err() {
                    crate::hunt::remember_kill(fauna, hit.striker, x, z, ring);
                }
            }
        }
    });
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
        fauna.book_removed(a);
        fauna.free_riders(a);
    }
}

/// Step 2: the upkeep, out of the reserve and then out of the body.
///
/// Respiration is one helper shared with the motor budget: organic matter from the
/// reserve and then from the body, the energy leaving with it at the animal's own
/// current density, the mineral it held shed to the gut. It returns what was paid and how much
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
    let (out, from_body) = withdraw(a, want);
    let paid = out.organic;
    if !(paid > 0.0) {
        return (0.0, 0.0);
    }
    // The brood pays its share of what the reserve respired (D1b): the κ-rule splits net
    // income, so the soma's share of upkeep is not charged to the eggs.
    if brood_share > 0.0 {
        a.reproduction.brood = (a.reproduction.brood - brood_share * (paid - from_body)).max(0.0);
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
/// drain share: from the reserve and then from the structure, with the mineral and energy
/// it held at the body's own current density — the rule the plant layer's dead pools use,
/// so a body half respired is the same stuff it was. A body taken down to nothing hands
/// over every unit of its mineral and energy rather than keeping float dust. Returns the
/// three currencies that left and how much organic matter came out of the structure.
fn withdraw(a: &mut Animal, want: f64) -> (Taken, f64) {
    if !(want > 0.0) {
        return (Taken::default(), 0.0);
    }
    let before = a.organic();
    // The organism budget's upkeep with no income at this point of the tick: the reserve
    // first, and what it cannot pay is the body's dieback, respired out of the structure.
    let up = Budget::default().upkeep(&mut a.reserve, want);
    let from_body = up.unpaid.min(a.body);
    let paid = up.from_reserve + from_body;
    if !(paid > 0.0) {
        return (Taken::default(), 0.0);
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
            organic: paid,
            mineral: m,
            energy: e,
        },
        from_body,
    )
}

/// `organic` out of the reserve, which must hold it, with the mineral and energy at the
/// body's own density: a gestation instalment's and a clutch's one pro-rata rule.
pub(crate) fn take_from_reserve(a: &mut Animal, organic: f64) -> Taken {
    let before = a.organic();
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
/// §3, re-tied by the minimal gut's repair 2 item 9). Each rider takes
/// `parasite_take_per_day` × its host's body per ecological day out of the host by
/// [`withdraw`]'s pro-rata rule, into its own reserve, mineral and energy. A transfer
/// inside this layer, so no boundary flow moves; the host's structural share is its
/// `Self` channel's loss, as an upkeep's is. Nothing protects the host: one drained below
/// `body_min` starves as any other.
fn drain_riders(fauna: &mut Fauna) {
    let day = fauna.config.life.ecological_day_s;
    let per_s = if day > 0.0 { fauna.config.parasite_take_per_day / day } else { 0.0 };
    if !(per_s > 0.0) {
        return;
    }
    for r in 0..fauna.animals.len() {
        let Some(hold) = fauna.animals[r].attached else {
            continue;
        };
        let Ok(h) = fauna.animals.binary_search_by_key(&hold.host, |a| a.id) else {
            continue;
        };
        let want = per_s * fauna.animals[h].body * DT;
        let host = &mut fauna.animals[h];
        let (taken, from_body) = withdraw(host, want);
        if host.founder.is_some() && from_body > 0.0 {
            host.founder_state.feedback.structural_loss += from_body;
        }
        fauna.ledger.parasite_taken_organic += taken.organic;
        if let Some(f) = fauna.animals[h].founder {
            fauna.ledger.rider_take_lost_by_founder[f.index()] += taken.organic;
        }
        let rider = &mut fauna.animals[r];
        rider.reserve += taken.organic;
        rider.mineral += taken.mineral;
        rider.energy += taken.energy;
    }
}

/// Step 2, after growth: **a rider at its detach size drops off** (§4) where its host
/// stands — `parasite_detach_body_fraction` × its lineage's `body_max` — and is a free
/// juvenile from here on.
fn drop_off_grown(fauna: &mut Fauna) {
    let fraction = fauna.config.parasite_detach_body_fraction;
    for r in 0..fauna.animals.len() {
        let Some(hold) = fauna.animals[r].attached else {
            continue;
        };
        let line = fraction * body::effective_config(&fauna.config, &fauna.animals[r]).body_max;
        if fauna.animals[r].body < line {
            continue;
        }
        let Ok(h) = fauna.animals.binary_search_by_key(&hold.host, |a| a.id) else {
            continue;
        };
        let host = fauna.animals[h];
        crate::release(&mut fauna.animals[r], &host);
        fauna.ledger.rider_dropoffs += 1;
    }
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
fn circadian(fauna: &mut Fauna, view: &VoxelView<'_>, clock_on: bool, daylight: f64) {
    let life = fauna.config.life;
    let v = view.config.voxel_m;
    let ring = f64::from(view.config.width) * v;
    let tick = fauna.tick;
    // The packs raising pups: an adult's bout there is a provisioning one.
    let mut nurseries: Vec<u64> = fauna
        .animals
        .iter()
        .filter_map(|a| a.pack.filter(|m| m.pup).map(|m| m.pack))
        .collect();
    nurseries.sort_unstable();
    nurseries.dedup();
    for i in 0..fauna.animals.len() {
        let Some(founder) = fauna.animals[i].founder else {
            continue;
        };
        let phys = *fauna.config.founder(founder);
        // A dark-active body reads the light at its own face (§3.1B): the face's
        // terrain sky, cached per face.
        let sky = if clock_on && phys.active.reads_face_light() {
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
        let a = &fauna.animals[i];
        let hungry = hunger(a, &phys.core);
        let emergency = if a.emergency {
            hungry >= phys.emergency_clear_hunger
        } else {
            hungry >= phys.wake_hunger
        };
        let on_phase = phys.active.awake_under(clock_on, daylight, sky, &life);
        let dark_awake = on_phase && clock_on && phys.active.reads_face_light() && daylight >= life.day_line;
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
        // it, it stays awake and its tactic walks it home (for at most `home_give_up_s`).
        let rest = match rest {
            Some(_) if crate::pack::must_walk_home(fauna, i, v, ring) => None,
            Some(r) => Some(r),
            None => {
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
    }
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
fn gut_bite(a: &Animal, phys: &crate::FounderPhysiology, period_s: f64, effort: f64, stock: f64) -> f64 {
    if !a.eating || !(stock > 0.0) {
        return 0.0;
    }
    // A small body bites faster per unit of itself (the gut's mass exponent, D1b).
    let want = phys.gut.bite_per_body_s * phys.gut.throughput_factor(a.body, phys.core.body_max) * a.body * period_s * effort;
    if !(want > 0.0) {
        return 0.0;
    }
    let k = phys.core.bite_half_stock;
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
/// whatever the room: it places nothing, so the cap does not apply to it. Everything
/// digested or passed goes through the one assimilation rule ([`assimilate`]): the yield
/// into the reserve as far as its mineral funds it, the undigested share to egesta, the
/// unfunded share respired; what is not digested stays in the stomach. No mineral is
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
            let placed = assimilate(fauna, i, &sc, t, phys.yield_for(food));
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
        // Asleep it pays its lineage's sleep factor of it.
        let rest = match fauna.animals[i].founder {
            Some(founder) if fauna.animals[i].state == State::Asleep => {
                fauna.config.founder(founder).sleep_upkeep
            }
            _ => 1.0,
        };
        let a = &mut fauna.animals[i];
        a.age_ticks = a.age_ticks.saturating_add(1);
        let want = sc.maintenance_per_s * rest * a.body * DT;
        let (_, from_body) = respire(fauna, i, want, Respiration::Maintenance);
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
fn grow(fauna: &mut Fauna) {
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
                fauna.ledger.matured_by_founder[f.index()] += 1;
                fauna.ledger.maturation_ticks_by_founder[f.index()] += a.age_ticks;
            }
        }
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

/// One due body's read side of the controller stage: its manifest, what its observation
/// would have written into the senses, and its controller's resolved answer (`driven`
/// false — rest — without a controller or with a non-finite observation).
struct Sensed {
    manifest: crate::manifest::Manifest,
    writes: crate::senses::SenseWrites,
    actions: Actions,
    lure: f64,
    lift: f64,
    driven: bool,
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
fn controllers(
    fauna: &mut Fauna,
    view: &VoxelView<'_>,
    flora: &Flora,
    mut senses: Option<&mut crate::Senses>,
    sky: &Sky,
    threads: usize,
) {
    #[cfg(feature = "profile")]
    let mut timer = cubarium_voxel::profile::start(cubarium_voxel::profile::Phase::FaunaWindows);
    // SENSES-CORE: the live lineages' remote senses, staggered by id, before any controller
    // of the tick reads a record.
    crate::perception::sense_pass(fauna, view, sky.clock_on, sky.daylight, threads);
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
    // The occupancies every due body's rays read: one grid of stand classes and ground
    // pools held across stages and patched where something moved (SPEED-MATURE,
    // `senses::LiveGrid`), with the bodies laid in two flavours — the v2 cone's one cell
    // per body (every sighted forager's: a cone and no hunter's table) and the predation
    // occupancy's bodies with volume (predation §2.2: the lanternjaw's own sight and a
    // heuristic prey's look for a threat). A static episode's senses hold the v2 cone
    // instead (cone-speed item 4): built once, patched from bites and deaths, bodies
    // re-indexed — the same grid either way.
    let v2_cone: [bool; Founder::COUNT] = Founder::ALL
        .map(|f| f.manifest().cone_range_m > 0.0 && fauna.config.founder(f).hunter.is_none());
    let observers = due
        .iter()
        .any(|&i| fauna.animals[i].founder.is_some_and(|f| v2_cone[f.index()]));
    let mut held = senses.as_deref_mut().and_then(|s| s.take_cone());
    // The predation occupancy is read only while a hunter is in the world.
    let hunters_here = fauna
        .animals
        .iter()
        .any(|a| a.founder.is_some_and(|f| fauna.config.founder(f).hunter.is_some()));
    let predators = hunters_here
        && due
            .iter()
            .any(|&i| fauna.animals[i].founder.is_some_and(|f| f.manifest().cone_range_m > 0.0));
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
    let mut table = std::mem::take(&mut fauna.controllers);
    let sensed: Vec<Sensed> = {
        let mut slots = table.slots_mut(&ids);
        let fauna: &Fauna = fauna;
        let senses = senses.as_deref();
        let fv = &fv;
        let read = |i: usize, slot: &mut Option<&mut Box<dyn crate::Controller>>| -> Sensed {
            let founder = fauna.animals[i].founder.expect("a due founder");
            let manifest = fauna.config.manifest(founder);
            let occupancy = if v2_cone[founder.index()] {
                cone_occupancy
            } else {
                predation_occupancy
            };
            let (obs, writes) =
                body::observation_read(fauna, i, view, fv, &manifest, senses, occupancy);
            // Reject rather than train through: an observation with a non-finite channel
            // holds rest instead of reaching the controller at all.
            let (actions, lure, lift, driven) = match slot.as_deref_mut() {
                Some(c) if obs.iter().all(|v| v.is_finite()) => {
                    let response = c.drive(&obs);
                    (crate::controller::resolve_actions(response, &manifest), c.lure(), c.lift(), true)
                }
                _ => (Actions::REST, 0.0, 0.0, false),
            };
            Sensed { manifest, writes, actions, lure, lift, driven }
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

    // **Write side**, serially in body order: the reflexes and the held actions.
    #[cfg(feature = "profile")]
    timer.next(cubarium_voxel::profile::Phase::FaunaReflex);
    for (k, &i) in due.iter().enumerate() {
        let id = fauna.animals[i].id;
        let founder = fauna.animals[i].founder.expect("a due founder");
        let Sensed { ref manifest, actions, lure, lift, driven, .. } = sensed[k];
        let (mut actions, mut lure, mut lift) = (actions, lure, lift);
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
            if pressed {
                *fauna.pressed.entry(id).or_default() += manifest.cadence_ticks().max(1);
            } else {
                fauna.pressed.remove(&id);
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
        if driven && fauna.controllers.flees(id) && fauna.config.founder(founder).glider.is_some() {
            let day = crate::perception::is_day(&fauna.config, sky.clock_on, sky.daylight);
            if let Some(go) = glider::fruit_reflex(fauna, i, view, &fv, day) {
                actions = go;
                src = crate::Steer::Fruit;
            }
        }
        // AMPHIBIOUS-SNAIL: a body that retracts (its lineage's `retract_hold_s`) does so
        // instead of fleeing: a threat it sees or feels stops and closes it.
        let retracts = fauna.config.founder(founder).amphibious.is_some_and(|am| am.retract_hold_s > 0.0);
        if driven && hunters_here && retracts && fauna.controllers.flees(id) {
            retract(fauna, i);
        }
        let mut fled = false;
        if driven && hunters_here && !retracts && fauna.controllers.flees(id) {
            if let Some(flee) = flee(fauna, i, view, &fv, predation_occupancy, actions) {
                actions = flee;
                lure = 0.0;
                lift = 0.0;
                fled = true;
                src = crate::Steer::Flee;
            }
        }
        // A glider's roost reflex (F3): at its rest hour or in a downpour, its own
        // heuristic gives way to a walk to the nearest crown it can climb.
        if driven && fauna.controllers.flees(id) && fauna.config.founder(founder).glider.is_some() {
            if let Some(roost) = glider::roost_reflex(fauna, i, view, &fv, sky) {
                actions = roost;
                src = crate::Steer::Roost;
            }
        }
        // Fed perching (the minimal gut's repair 3 item 8): a body whose controller rests
        // when fed (the bellwing's hover-sip teacher) and whose eating latch is off holds
        // still on its perch, or comes down onto what is below it, read from the latch.
        if driven && fauna.controllers.rests_when_fed(id) && !fauna.animals[i].eating {
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
        if driven && fauna.controllers.flees(id) {
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
        if driven && fauna.controllers.flees(id) {
            if let Some(go) = roam_reflex(fauna, i, view, &fv, sky, actions) {
                actions = go;
                lure = 0.0;
                lift = 0.0;
                src = crate::Steer::Roam;
            }
        }
        // AMPHIBIOUS-SNAIL: a thirsty body out of water heads for the nearest water.
        if driven && fauna.controllers.flees(id) {
            if let Some(go) = water_reflex(fauna, i, view) {
                actions = go;
                src = crate::Steer::Roam;
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
        // The detritus web D2: the lanternjaw's patrol (its heuristic only, while the
        // world's `hunt.patrol` is on). The carcass yield below still wins.
        if driven && fauna.controllers.hunt_patrol(id) && !fauna.controllers.at_carcass(id) {
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
                    if face_sky * sky.daylight < fauna.config.life.day_line { lit } else { 0.0 }
                } else {
                    lit
                };
            }
        }
        // R3: the lanternjaw's carcass yield (its heuristic only), and the chorister's
        // scripted pack tactic.
        if driven && hunters_here {
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
        if driven && fauna.controllers.pack_tactic(id) {
            if let Some(tactic) =
                crate::pack::steer(fauna, i, view, &fv, sky.clock_on, sky.daylight, actions, predation_occupancy)
            {
                actions = tactic;
                lure = 0.0;
                src = crate::Steer::Pack;
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
            let heuristic = if phys.flight.is_some() { fauna.controllers.rests_when_fed(id) } else { fauna.controllers.flees(id) };
            if heuristic && table.live && (table.light_m > 0.0 || table.loom_m > 0.0) {
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
        if driven && fauna.controllers.flees(id) && actions.feed > 0.5 && actions.forward <= fauna.config.teachers.forage_stay_forward {
            fauna.ledger.feed_still_ticks_by_founder[founder.index()] += 1;
        }
        // DROP: a body trapped off its route graph (no walking way back on, no glide) for its
        // lineage's `drop.after_s` jumps down; last, as it is the only way out.
        if driven {
            if let Some(go) = jump::reflex(fauna, i, view, &fv) {
                actions = go;
                lure = 0.0;
                lift = 0.0;
                src = crate::Steer::Drop;
            }
        }
        fauna.steer.insert(id, src);
        // EDGE: a reflex walk overridden by a later layer is not the one held.
        fauna.walk_commit(id, founder, src);
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
    }
    if let Some(s) = senses {
        s.return_cone(held);
    }
    fauna.grid_scratch = live;
}

/// **The flee rule** of the prey heuristics (predation §2.7), `None` when there is
/// nothing to flee, read from the body's record ([`crate::perception`]): the grazer turns
/// away from the nearest threat it sees within `flee_sight_m` ahead of it (by the interim
/// rule: [`crate::kind_of`], so a still hunter beyond 0.5 m is not one, and not hidden by
/// terrain or foliage); the blind shredder turns away from tremor above
/// [`crate::PredationConfig::flee_tremor`]. Full forward, no feeding.
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
        .any(|&f| config.founder(f).hunter.is_some_and(|h| h.hunts(founder)));
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
    if blind {
        let [left, right] = p.tremor;
        if left + right < fauna.config.predation.flee_tremor {
            return None;
        }
        return Some(away(if left > right { 1.0 } else { -1.0 }));
    }
    let ring = f64::from(view.config.width) * view.config.voxel_m;
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
    let Some(founder) = a.founder else { return };
    let Some(am) = fauna.config.founder(founder).amphibious else { return };
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
    let m = &mut fauna.animals[i].mobility;
    if m.retract_ticks == 0 {
        fauna.ledger.retracts += 1;
    }
    fauna.animals[i].mobility.retract_ticks = crate::Hunter::ticks(am.retract_hold_s);
}

/// **The water reflex** (AMPHIBIOUS-SNAIL): a thirsty amphibious body out of water
/// ([`crate::Mobility::thirsty`], [`crate::Mobility::ashore`]) walks to the nearest support
/// face with standing water within its lineage's `water_search_m`, at most its step up
/// above its own face. A heuristic world query, not a sense (as the shallows reflex is).
/// `None` when it is not thirsty ashore or no water is in reach.
fn water_reflex(fauna: &Fauna, i: usize, view: &VoxelView<'_>) -> Option<Actions> {
    let a = &fauna.animals[i];
    let founder = a.founder?;
    let phys = fauna.config.founder(founder);
    let am = phys.amphibious.filter(|am| am.on_land)?;
    if !(a.mobility.thirsty && a.mobility.ashore) || a.mobility.mode != crate::MobilityMode::Ground || a.attached.is_some() {
        return None;
    }
    let c = view.config;
    let v = c.voxel_m;
    let reach = am.water_search_m.max(0.0);
    let k = (reach / v).ceil() as i64;
    let up = (phys.step_up_m / v).floor().max(0.0) as u32;
    let (hx, hz) = (i64::from(a.site.x), i64::from(a.site.z));
    let ring = f64::from(c.width) * v;
    let top = (a.site.y + up).min(c.height.saturating_sub(1));
    let mut best: Option<(f64, f64, f64)> = None;
    for dz in -k..=k {
        let z = hz + dz;
        if z < 0 || z >= i64::from(c.depth) {
            continue;
        }
        for dx in -k..=k {
            let x = (hx + dx).rem_euclid(i64::from(c.width));
            let Some(y) = view.highest_support_in(x, z as u32, 0, top) else {
                continue;
            };
            if !(view.standing_depth_m(x, y, z as u32) > 0.0) {
                continue;
            }
            let (tx, tz) = ((x as f64 + 0.5) * v, (z as f64 + 0.5) * v);
            let (ddx, ddz) = crate::pose::wrapped_delta(a.pose.x, a.pose.z, tx, tz, ring);
            let d = (ddx * ddx + ddz * ddz).sqrt();
            if d <= reach && best.is_none_or(|(b, _, _)| d < b) {
                best = Some((d, ddx, ddz));
            }
        }
    }
    let (_, dx, dz) = best?;
    Some(glider::walk_toward(a, dx, dz, 0.0))
}

/// Whether `sky` and the rain record make this an amphibious body's **roam hours**
/// (SNAIL-FIX): night (the clock on and daylight under the day line, or the clock off, as
/// the drying clock reads it), rain falling, or within its `roam_after_rain_s` of the last
/// rain.
fn roam_hours(fauna: &Fauna, am: &crate::Amphibious, view: &VoxelView<'_>, sky: &Sky) -> bool {
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
    fauna: &Fauna,
    i: usize,
    view: &VoxelView<'_>,
    fv: &cubarium_voxel_flora::FloraView<'_>,
    sky: &Sky,
    asked: Actions,
) -> Option<Actions> {
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
    // (score, distance, dx, dz): the most film per metre, and the nearest bare wet face.
    let mut film_best: Option<(f64, f64, f64)> = None;
    let mut near: Option<(f64, f64, f64)> = None;
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
            let film = fv.film_at(cubarium_voxel_flora::Site { x: x as u32, y, z });
            if film > 0.0 {
                let score = film / d;
                if film_best.is_none_or(|(b, _, _)| score > b) {
                    film_best = Some((score, ddx, ddz));
                }
            }
            if near.is_none_or(|(b, _, _)| d < b) {
                near = Some((d, ddx, ddz));
            }
        }
    }
    let (_, dx, dz) = match film_best {
        Some(t) => t,
        None if !m.ashore => near?,
        None => return None,
    };
    Some(glider::walk_toward(a, dx, dz, 0.0))
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
        let before = fauna.animals[i].pose;
        let (flyer, glider) = fauna.animals[i]
            .founder
            .map_or((None, None), |f| (fauna.config.founder(f).flight, fauna.config.founder(f).glider));
        match (flyer, glider) {
            (Some(flight), _) => flyer_act(fauna, view, flora, sky, flight, i),
            (None, Some(g)) => glider::glider_act(fauna, view, flora, seed, tick, sky, hits, g, i),
            (None, None) => {
                act_one(fauna, view, flora, seed, tick, hits, i);
                // A walker's base is the top of its support face (F1).
                let a = &mut fauna.animals[i];
                a.pose.y_m = crate::surface_m(a.site.y, v);
            }
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
    let manifest = fauna.config.manifest(founder);
    let phys = *fauna.config.founder(founder);
    let sc = phys.core;
    fauna.ledger.founder_ticks_by_founder[founder.index()] += 1;

    // A carnivore's lit lure costs `lure_power` × its awake upkeep (predation §2.2). It
    // lures only while it wants food (repair 2 item 7): a fed hunter's lure is dark.
    if let Some(h) = phys.hunter {
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
    let (motion, standing_y) = {
        let a = &mut fauna.animals[i];
        let mut standing_y = a.site.y;
        let motion = body::resolve_motion_in(
            view,
            &mut a.pose,
            &mut standing_y,
            &manifest,
            &geometry,
            phys.wade_depth_m(&geometry),
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
            Attach::TurnedBack | Attach::Nothing => {}
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
        let target = match phys.hunter {
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
            let h = phys.hunter.expect("a hunter");
            let a = &mut fauna.animals[i];
            a.strike = Some(Strike {
                phase: StrikePhase::Windup,
                ticks_left: crate::Hunter::ticks(h.windup_s),
                prey,
                from_x: a.pose.x,
                from_z: a.pose.z,
            });
            a.state = State::Resting;
            fauna.ledger.strikes_by_founder[founder.index()] += 1;
            if let Ok(j) = fauna.animals.binary_search_by_key(&prey, |p| p.id) {
                if let Some(pf) = fauna.animals[j].founder {
                    fauna.ledger.strikes_on_by_founder[pf.index()] += 1;
                    fauna.ledger.funnel[founder.index()][pf.index()][crate::perception::STRUCK] += 1;
                    // D2b readout: a patroller's strike its choice did not accept.
                    let choosy = fauna.config.hunt.patrol && fauna.controllers.hunt_patrol(fauna.animals[i].id);
                    if choosy && !crate::hunt::accepts(&fauna.config, &fauna.animals[i], &fauna.animals[j], fauna.tick) {
                        fauna.ledger.strikes_unchosen_by_prey[pf.index()] += 1;
                    }
                }
            }
            // R3: a pack member calls as it strikes.
            crate::pack::call(fauna, i);
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
///    takes off when it is awake, the sky allows it and its reserve pays a takeoff; else
///    it **drops** — straight down at the lift speed, paying nothing aloft — onto the
///    first perch below.
/// 2. **Forced down**: in a downpour, or sleepy (its lineage's rest hour and not hungry),
///    a flyer aloft descends (`lift` −1, steering as it likes) and a perched one cannot
///    take off. Asleep it does nothing else (it sleeps only perched).
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
) {
    use crate::flight;
    use crate::{MobilityMode, PerchKind};
    let founder = fauna.animals[i].founder.expect("a founder body");
    let phys = *fauna.config.founder(founder);
    let manifest = fauna.config.manifest(founder);
    let sc = phys.core;
    let life = fauna.config.life;
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
    let sleepy = !phys.active.awake(sky.clock_on, sky.daylight, &life) && !fauna.animals[i].emergency;
    let grounded = sky.downpour || sleepy;
    let can_pay = |a: &Animal| a.reserve >= takeoff_cost;
    let puddle = phys.drown_depth_m(&phys.body_at(fauna.animals[i].body));

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
    if falling {
        (forward, turn, lift) = (0.0, 0.0, -1.0);
    } else if grounded {
        lift = -1.0;
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
            let fed = due
                && held.feed > 0.0
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
        let cost = (factor - 1.0).max(0.0) * basal * DT * shower;
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
        let distance = forward * cruise * slow * DT;
        let dy = lift * flight.lift_speed_fraction * cruise * DT;
        let moved = flight::move_through_air(view, &fv, shape, flight.band_max_m, puddle, &mut a.pose, distance, dy);
        if let Some(site) = flight::support_below(view, a.pose.x, a.pose.z, a.pose.y_m) {
            a.site = site;
        }
        if let Some(p) = moved.landed {
            a.mobility.mode = MobilityMode::Perched;
            a.mobility.perch = Some(p);
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
/// body (the pollen seam). Only while the eating latch is on.
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
    let bite = gut_bite(&fauna.animals[i], phys, manifest.controller_period_s, effort, stock);
    if !(bite > 0.0) {
        return false;
    }
    let taken = match bloom {
        crate::Bloom::Stand(site) => flora.take_stand_nectar(site, bite),
        crate::Bloom::Vine(face) => flora.take_nectar(face, bite),
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
    fauna.animals[i].last_bloom = Some(bloom);
    swallow(fauna, i, Food::Nectar, taken);
    true
}

/// The jaw's point: the body's nose, `jaw_forward_length_fraction` of its length ahead of
/// its centre (half, for the lanternjaw).
fn jaw_of(x: f64, z: f64, heading: f64, length_m: f64, forward_fraction: f64) -> (f64, f64) {
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
    let s0 = crate::surface_m(striker.site.y, v) + striker.wall_offset_m();
    let band = (s0, s0 + h.jaw_band_height_fraction * geometry.height_m);
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
fn prey_in_capsule(
    fauna: &Fauna,
    i: usize,
    view: &VoxelView<'_>,
    geometry: &crate::Body,
    h: &crate::Hunter,
) -> Option<u64> {
    let a = &fauna.animals[i];
    if a.mobility.climb.is_some() {
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
    let choosy = fauna.config.hunt.patrol && fauna.controllers.hunt_patrol(a.id);
    let mut best: Option<(f64, u64)> = None;
    for o in &fauna.animals {
        if o.id == a.id
            || o.mobility.climb.is_some()
            || !crate::predation::is_prey_in_window(&fauna.config, a, o, window)
            || !meets_capsule(&fauna.config, view, a, geometry, h, jaw, tip, o)
            || (choosy && fauna.config.hunt.strike_chosen_only && !crate::hunt::accepts(&fauna.config, a, o, fauna.tick))
        {
            continue;
        }
        let (dx, dz) = crate::pose::wrapped_delta(jaw.0, jaw.1, o.pose.x, o.pose.z, ring);
        let d = dx * dx + dz * dz;
        if best.is_none_or(|(b, _)| d < b) {
            best = Some((d, o.id));
        }
    }
    best.map(|(_, id)| id)
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
        let a = &fauna.animals[i];
        s = Strike {
            phase: StrikePhase::Lunge,
            ticks_left: crate::Hunter::ticks(h.lunge_s),
            from_x: a.pose.x,
            from_z: a.pose.z,
            ..s
        };
    }
    // One lunge tick: `strike_reach / lunge ticks` forward, turn held still.
    let geometry = phys.body_at(fauna.animals[i].body);
    let manifest = fauna.config.manifest(founder);
    let reach = h.strike_reach_length_fraction * geometry.length_m;
    let per_tick = reach / f64::from(crate::Hunter::ticks(h.lunge_s));
    let lunge = Actions {
        forward: per_tick / (manifest.cruise_m_per_s * DT),
        turn: 0.0,
        feed: 0.0,
    };
    let step_limits = phys.step_limits(view.config.voxel_m);
    let wall_friction = fauna.config.life.wall_friction;
    let standing_y = {
        let a = &mut fauna.animals[i];
        let mut standing_y = a.site.y;
        body::resolve_motion(
            view,
            &mut a.pose,
            &mut standing_y,
            &manifest,
            &geometry,
            phys.wade_depth_m(&geometry),
            step_limits,
            wall_friction,
            lunge,
        );
        standing_y
    };
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
    s.ticks_left = s.ticks_left.saturating_sub(1);
    if s.ticks_left > 0 {
        fauna.animals[i].strike = Some(s);
        return;
    }
    // Contact: the jaw's sweep from where the lunge began to where it ended.
    let a = fauna.animals[i];
    let heading = a.pose.heading_rad;
    let jaw_from = jaw_of(s.from_x, s.from_z, heading, geometry.length_m, h.jaw_forward_length_fraction);
    let jaw_to = jaw_of(a.pose.x, a.pose.z, heading, geometry.length_m, h.jaw_forward_length_fraction);
    let prey = fauna
        .animals
        .binary_search_by_key(&s.prey, |p| p.id)
        .ok()
        .map(|j| fauna.animals[j]);
    if let Some(p) = prey {
        if meets_capsule(&fauna.config, view, &a, &geometry, &h, jaw_from, jaw_to, &p) {
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
            if glanced {
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
                    h.strike_power * (a.body / p.body) * (1.0 - armour) * closed
                } else {
                    1.0
                };
                hits.push(Hit {
                    prey: p.id,
                    damage,
                    by: founder,
                    striker: a.id,
                });
            }
        }
    }
    fauna.animals[i].strike = Some(Strike {
        phase: StrikePhase::Recovery,
        ticks_left: crate::Hunter::ticks(h.recovery_s),
        ..s
    });
}

/// The key the shield's glance draw is salted with, apart from the dodge's (a pure numeric).
const GLANCE_KEY: u64 = 0x6c61_6e63_6500_0001;

/// **One [`Hit`] applied** (predation §2.4, §2.5): the wound adds, the hit wakes the prey
/// and keeps it up for [`crate::PredationConfig::alert_s`], and a wound of 1 kills — the whole body to
/// carrion on its face, a carcass record on the pool, departed `Killed { by }`. A prey
/// that is gone already is left alone: it dies once.
pub(crate) fn apply_hit(fauna: &mut Fauna, flora: &mut Flora, prey: u64, damage: f64, by: Founder) {
    let Ok(j) = fauna.animals.binary_search_by_key(&prey, |a| a.id) else {
        return;
    };
    // A hit is counted where it lands: on a prey still alive.
    fauna.ledger.hits_by_founder[by.index()] += 1;
    let alert_s = fauna.config.predation.alert_s;
    let a = &mut fauna.animals[j];
    if damage.is_finite() && damage > 0.0 {
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
        kill(fauna, flora, j, Departure::Killed { by });
    }
}

/// A death by `cause` of the body at `j`, now: an open gestation is resolved first, then
/// the whole body goes to carrion on its face and the death is booked; a kill also lays
/// the carcass record.
fn kill(fauna: &mut Fauna, flora: &mut Flora, j: usize, cause: Departure) {
    let rule = body::effective_config(&fauna.config, &fauna.animals[j]).reproduction;
    fail_gestation(fauna, j, &rule);
    // S3: a kill of a prey drawn to a lure.
    if let (Departure::Killed { by }, Some(f)) = (cause, fauna.animals[j].founder) {
        if crate::light::drawn_to_lure(fauna, fauna.animals[j].id) {
            fauna.ledger.lure_kills[f.index()] += 1;
        }
        // S3b: a prey with a light sense still naive about cyan.
        if fauna.config.founder(f).senses.light_m > 0.0 && crate::light::naive(fauna, fauna.animals[j].id) {
            fauna.ledger.naive_kills[by.index()][f.index()] += 1;
        }
    }
    let a = fauna.animals.remove(j);
    fauna.controllers.take(a.id);
    fauna.trapped.remove(&a.id);
    fauna.drop_plans.remove(&a.id);
    fauna.free_riders(&a);
    let organic = a.organic() + a.gut.organic + a.gut_seeds.total().organic + a.stomach.organic();
    if let (Departure::Killed { by }, Some(f)) = (cause, a.founder) {
        fauna.ledger.funnel[by.index()][f.index()][crate::perception::KILLED] += 1;
        // HUNKER's readout: the kill by the prey's age class.
        if a.body < fauna.config.founder(f).core.birth_body {
            fauna.ledger.vigil.kills_juvenile[f.index()] += 1;
        } else {
            fauna.ledger.vigil.kills_adult[f.index()] += 1;
        }
    }
    let laid = bury_corpse(fauna, flora, &a, cause);
    if laid && matches!(cause, Departure::Killed { .. }) {
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
    // The whole animal, whatever is left of it: a corpse with no organic matter left is
    // still a corpse with mineral in it, and keeping an inert remainder on a dead animal
    // is how a ledger stops closing.
    // The stomach's raw food goes with it, as raw matter: no carcass yield of its own.
    let fed = a.stomach.total();
    let laid = fauna.book_deposit(
        flora,
        a.site,
        DepositKind::Carrion,
        Taken {
            organic: a.organic() + a.gut.organic + a.gut_seeds.total().organic + fed.organic,
            mineral: a.mineral + a.gut.mineral + a.gut_seeds.total().mineral + fed.mineral,
            energy: a.energy + a.gut.energy + a.gut_seeds.total().energy + fed.energy,
        },
    );
    let l = &mut fauna.ledger;
    l.stomach_out.organic += fed.organic;
    l.stomach_out.mineral += fed.mineral;
    l.stomach_out.energy += fed.energy;
    fauna.ledger.deaths += 1;
    fauna.book_departure(a, cause);
    laid
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
/// wall_climb_cost_factor`]. Turn effort is not read on a face: the heading stays into
/// it. The interval's motion feedback books the climbing gait's own request (half pace at
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
    a.state = State::Climbing;
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

/// The founder's supported local bite: the blind founder takes **detritus** — litter or
/// carrion off the ground under its mouth region, or cap tissue off a fungal stand in
/// its band — and the browser crops a vascular stand whose crown cells it touches. One
/// withdrawal per attempt, through the plant layer's own `take_*`, debiting the real
/// stocks. Nothing in reach, a mouth in the air, or an unsupported body transfers
/// nothing at all.
///
/// Decisions §3 gave the shredder its three foods; since R0 every lineage's foods are its
/// physiology's diet list. There is no new action: `bite` still means "take what is at
/// the mouth", and [`body::mouth_meal`] resolves which of the diet's foods that is (the
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
    // ([`body::mouth_meal`]). Its class is the source's own: a pool names it, and a
    // stand's is its species' income rule.
    let diet = fauna.config.founder(founder).diet;
    let Some((meal, stock, _)) =
        body::mouth_meal(&flora.view(), view, &cols, site_y, geometry, diet)
    else {
        return false;
    };
    let food = meal.food();
    // The bite this body, this stock and this stomach's room allow.
    let bite = gut_bite(&fauna.animals[i], phys, manifest.controller_period_s, effort, stock);
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
        body::Meal::Pool { .. } => None,
    };
    // The funnel's eaten stage (SENSES-CORE): a bite of a carcass, by the dead body's lineage.
    let carcass_of = match meal {
        body::Meal::Pool { site, food: Food::Carrion } => flora.view().ground_at(site).and_then(|g| g.carcass).map(|c| (usize::from(c.lineage), site)),
        _ => None,
    };
    let taken = match meal {
        body::Meal::Stand { root, place, .. } => flora
            .take_foliage_of(root, place, bite, &layers)
            .map(|t| t.taken),
        body::Meal::Pool { site, food } => match food {
            Food::Litter => flora.take_litter(site, bite),
            Food::Carrion => flora.take_carrion(site, bite),
            Food::Film => flora.take_film(site, bite),
            Food::CapTissue | Food::Foliage | Food::Nectar | Food::Fruit => {
                unreachable!("a stand's tissue is not a ground pool")
            }
        },
    };
    let Some(taken) = taken else {
        return false;
    };
    let taken = fauna.book_eaten(taken, food);
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
    fauna.ledger.bites_by_founder[founder.index()] += 1;
    fauna.ledger.bites_by_founder_food[founder.index()][food.index()] += 1;
    swallow(fauna, i, food, taken);
    true
}

/// A bite becoming tissue: the food class's own yield of it is stored **as far as the
/// bite's own mineral pays for**, the undigested `(1 − yield)` share is egested, the
/// unfunded rest is respired, the energy follows the organic matter at the bite's own
/// density, and the undigested share and the mineral that came with more tissue than was
/// built go to the gut, voided later as dung ([`void_guts`]).
///
/// **The mineral budget comes before the tissue.** `n_tissue` is mineral per
/// unit of tissue *built*, so a bite can only build `t.mineral / n_tissue` of it. At the
/// placeholders that binds on every bite: a plant's foliage carries `n_tissue` 0.02 and
/// this animal's tissue wants 0.05, so a `1e-4` bite brings `2e-6` of mineral and funds
/// `4e-5` of the `5e-5` a yield of 0.5 would otherwise have built. The unfunded
/// organic matter is **respired with its energy as heat**, exactly like the fraction a
/// full body and a full reserve cannot hold.
///
/// **The yield is the food's, not the eater's**: the caller hands in
/// [`crate::FounderPhysiology::yield_for`] for what was actually eaten, because a
/// detritivore with three foods has no single yield.
///
/// **No internal mineral reserve.** [`Animal::mineral`] is an inventory of what is already
/// in the tissue and not a stock growth may draw on — the plant layer's `Stand::mineral`
/// is the same — so an animal holding mineral still builds nothing out of a mineral-free
/// bite.
/// Returns the organic matter actually **placed** — into the reserve — which is
/// what a founder's prior-interval intake feedback records, and not the bite's whole
/// organic matter.
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
    // absorbed matter the bite's mineral cannot fund (and the reserve backstop) is
    // respired.
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
    let respired = unfunded + budget.close();

    // Energy: the density of what was eaten, so nothing is created for any food.
    let kept_energy = pro_rata(t.energy, placed, t.organic);
    a.energy += kept_energy;
    let egested_energy = if t.organic > 0.0 {
        (t.energy * (egested / t.organic)).min(t.energy - kept_energy)
    } else {
        0.0
    };
    let heat = t.energy - kept_energy - egested_energy;

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
    let parents = fauna.animals.len();
    let mut newborns: Vec<Animal> = Vec::new();
    for i in 0..parents {
        let sc = body::effective_config(&fauna.config, &fauna.animals[i]);
        let rule = sc.reproduction;
        advance_eligibility(fauna, i, &sc, &rule);
        match rule.mode {
            crate::BirthMode::Gestation => {
                gestate(fauna, i, &sc, &rule, view.config.seed, &mut newborns);
            }
            crate::BirthMode::Eggs => lay(fauna, i, view, flora, &sc, &rule),
        }
    }
    hatch(fauna, view, &mut newborns);
    dormant_eggs(fauna, view, flora, dusk, &mut newborns);
    let fresh: Vec<(u64, Founder)> = newborns
        .iter()
        .filter_map(|n| n.founder.map(|f| (n.id, f)))
        .collect();
    for mut n in newborns {
        // A newborn flyer is perched on the face it was born on (F1).
        if n.founder.is_some_and(|f| fauna.config.founder(f).flight.is_some()) {
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
    seed: u64,
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
    let (site, species, pose, founder, pack) = (a.site, a.species, a.pose, a.founder, a.pack);
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
fn litter_size(fauna: &Fauna, i: usize, sc: &SpeciesConfig, rule: &crate::Reproduction, seed: u64) -> Litter {
    let a = &fauna.animals[i];
    let Some(t) = a.founder.and_then(|f| fauna.config.founder(f).pack) else {
        return Litter::One;
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
    let lo = hi.min(2);
    let mut rng = Rng::keyed(DOMAIN_LITTER, seed, a.id, fauna.tick);
    let drawn = lo + (rng.unit() * f64::from(hi - lo + 1)).floor().min(f64::from(hi - lo)) as u32;
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
fn newborn(
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
        yielding: false,
        hunkered: false,
        // A newborn starts with an empty stomach and its eating latch off (repair 2
        // item 3).
        stomach: crate::Stomach::default(),
        eating: false,
        emergency: false,
        bout: None,
        hunt: crate::HuntMemory::default(),
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
/// for a lineage whose [`crate::EggSite`] is film, film; and a laying that has nowhere to go **costs nothing**: no escrow, no partial payment,
/// no refractory, and the surplus keeps standing for the next tick.
fn lay(
    fauna: &mut Fauna,
    i: usize,
    view: &VoxelView<'_>,
    flora: &Flora,
    sc: &SpeciesConfig,
    rule: &crate::Reproduction,
) {
    let seed = view.config.seed;
    if !rule.hold_met(&fauna.animals[i].reproduction) || fauna.animals[i].reproduction.retry_ticks > 0 {
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
            return;
        }
    }
    // Not on a wall: a clutch goes on the litter the body is standing in.
    if fauna.animals[i].mobility.climb.is_some() {
        return;
    }
    let Some(lineage) = fauna.animals[i].founder else {
        return;
    };
    let site = fauna.animals[i].site;
    // A dormant egg is buried in soil or litter (predation §2.13); the shredder lays in
    // the litter it lives in, the ripple-snail on the film it grazes (roster R2b,
    // [`crate::EggSite`]).
    if !rule.dormant {
        let ground = flora.view().ground_at(site);
        let fits = match fauna.config.founder(lineage).egg_site {
            crate::EggSite::Litter => ground.is_some_and(|g| g.litter > rule.litter_min),
            // Under water (SNAIL-FIX): land film is grazed, not laid on; laying above
            // the waterline is not decided.
            crate::EggSite::Film => {
                ground.is_some_and(|g| g.film > 0.0)
                    && view.standing_depth_m(i64::from(site.x), site.y, site.z) > 0.0
            }
            crate::EggSite::Fungus => fungus_near(view, flora, site, rule.egg_fungus_radius_m),
        };
        if !fits {
            return;
        }
    }
    // A flyer's egg needs room to fly from (bellwing3 item 2): its hatchling appears in
    // the air on this face, so the face needs `egg_headroom_m` clear over the hatchling's
    // body, and its air must reach the open air.
    if let Some(flight) = fauna.config.founder(lineage).flight {
        let phys = *fauna.config.founder(lineage);
        let shape = crate::flight::Shape::of(&phys, &flight, sc.body_min);
        let v = view.config.voxel_m;
        let (x, z) = ((f64::from(site.x) + 0.5) * v, (f64::from(site.z) + 0.5) * v);
        let room = crate::flight::overhead_clear(view, shape, x, z, crate::surface_m(site.y, v), flight.egg_headroom_m);
        if room + crate::flight::EPS < flight.egg_headroom_m {
            return;
        }
        if !fauna.open_air(view, &flight).over_face_is_open(site) {
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
        fauna.animals[i].reproduction.retry_ticks = rule.retry_ticks();
        fauna.ledger.nest_refusals_by_founder[lineage.index()] += 1;
        return;
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
    a.reproduction.refractory_ticks = rule.interval_ticks();
    a.reproduction.surplus_ticks = 0;
    a.reproduction.window.reset();
    a.reproduction.clutch_wait_ticks = 0;
    let laid_tick = fauna.tick;
    if rule.dormant {
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
        });
        fauna.ledger.eggs_laid += u64::from(eggs);
    }
    fauna.ledger.clutches_laid += 1;
    fauna.ledger.clutches_by_founder[lineage.index()] += 1;
    fauna.ledger.eggs_laid_by_founder[lineage.index()] += u64::from(eggs);
    fauna.ledger.reproduction_by_founder[lineage.index()] += cost;
}

/// **A fungal egg site** ([`crate::EggSite::Fungus`]): whether a fungus holding cap tissue
/// stands on a support face within `radius_m` of `site` — around the ring, across the
/// strip, and on `site`'s own row or one up or down.
fn fungus_near(view: &VoxelView<'_>, flora: &Flora, site: Site, radius_m: f64) -> bool {
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
    let r = rule.nest_radius_m;
    if !(r > 0.0) {
        return false;
    }
    let me = &fauna.animals[i];
    let Some(f) = me.founder else { return false };
    let v = view.config.voxel_m;
    let ring = f64::from(view.config.width) * v;
    let near = |x: f64, z: f64| {
        let (dx, dz) = crate::pose::wrapped_delta(me.pose.x, me.pose.z, x, z, ring);
        dx * dx + dz * dz <= r * r
    };
    let mut n: u64 = fauna
        .animals
        .iter()
        .filter(|a| a.id != me.id && a.founder == Some(f) && near(a.pose.x, a.pose.z))
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
fn clutch_holds(fauna: &Fauna, view: &VoxelView<'_>, c: &crate::Clutch) -> bool {
    let (x, y, z) = (i64::from(c.site.x), c.site.y, c.site.z);
    if !view.is_support(x, y, z) {
        return false;
    }
    let phys = fauna.config.founder(c.lineage);
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
    let voxel_m = view.config.voxel_m;
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
            let young = hatchling(fauna, &c, sc.body_min, o, m, e, voxel_m);
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
    voxel_m: f64,
) -> Animal {
    let lifespan_median_s = fauna.config.founder(c.lineage).core.lifespan_median_s;
    fauna.ledger.hatched += 1;
    fauna.ledger.eggs_hatched_by_founder[c.lineage.index()] += 1;
    let parcel = Taken {
        organic: o,
        mineral: m,
        energy: e,
    };
    newborn(
        fauna,
        (crate::Species::Frondgrazer, Some(c.lineage)),
        (c.site, crate::Pose::at_site(c.site, voxel_m)),
        body_min,
        parcel,
        lifespan_median_s,
    )
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
            let prey = phys.hunter.is_some_and(|h| h.hunts(f));
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
                let mut young = hatchling(fauna, &c, phys.core.body_min, o, m, e, v);
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

/// **A hatching egg's host** (parasite §1): the nearest living animal not of the egg's own
/// lineage, and not itself riding, within `parasite_attach_radius_m` of the egg's column
/// centre (wrapped on the ring), carrying fewer than `parasite_max_per_host` riders —
/// this tick's hatchlings counted. A tie goes to the lower id.
fn host_for(
    fauna: &Fauna,
    newborns: &[Animal],
    c: &crate::Clutch,
    (ex, ez): (f64, f64),
    ring: f64,
) -> Option<Animal> {
    let radius = fauna.config.parasite_attach_radius_m;
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
        if a.founder == Some(c.lineage) || a.attached.is_some() {
            continue;
        }
        let (dx, dz) = crate::pose::wrapped_delta(ex, ez, a.pose.x, a.pose.z, ring);
        let d = (dx * dx + dz * dz).sqrt();
        if d > radius || best.is_some_and(|(b, _)| d >= b) {
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
    let c = view.config;
    let v = c.voxel_m;
    let r = geometry.footprint_radius();
    let depth_m = f64::from(c.depth) * v;
    let limits = phys.step_limits(v);
    let reach = fauna.config.life.shallows_search_m;
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
            let way = graph.way_on(view, a.site, r, geometry.headroom_voxels(v), limits, wade, own, cols);
            if let Some(f) = way.filter(|f| !shunned.contains(&Fauna::walk_key_face(*f))) {
                let (dx, dz) = crate::pose::wrapped_delta(a.pose.x, a.pose.z, (f64::from(f.x) + 0.5) * v, (f64::from(f.z) + 0.5) * v, ring);
                fauna.walking_to(a.id, Fauna::walk_key_face(f), crate::Steer::Shallows);
                return Some(glider::walk_toward(&a, dx, dz, 0.0));
            }
        }
    }
    let mut best: Option<(f64, f64, f64, Site)> = None;
    for dz in -k..=k {
        let z = hz + dz;
        if z < 0 || z >= i64::from(c.depth) {
            continue;
        }
        for dx in -k..=k {
            let x = (hx + dx).rem_euclid(i64::from(c.width));
            let Some(y) = view.highest_support_in(x, z as u32, 0, top) else {
                continue;
            };
            if view.water_depth_m(x, y, z as u32) > wade {
                continue;
            }
            let tx = (x as f64 + 0.5) * v;
            let tz = (z as f64 + 0.5) * v;
            // Only where the body itself can stand: its centre inside the strip's `z`
            // ends, its disc clear over the face.
            if tz < r || tz > depth_m - r {
                continue;
            }
            let face = Site { x: x as u32, y, z: z as u32 };
            if shunned.contains(&Fauna::walk_key_face(face)) || !body::disc_clear_on_face(view, &geometry, face, limits) {
                continue;
            }
            let (ddx, ddz) = crate::pose::wrapped_delta(a.pose.x, a.pose.z, tx, tz, ring);
            let d = (ddx * ddx + ddz * ddz).sqrt();
            if d <= reach && best.is_none_or(|(b, _, _, _)| d < b) {
                best = Some((d, ddx, ddz, face));
            }
        }
    }
    let (_, dx, dz, face) = best?;
    fauna.walking_to(a.id, Fauna::walk_key_face(face), crate::Steer::Shallows);
    Some(glider::walk_toward(&a, dx, dz, 0.0))
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
            if a.body < sc.body_min {
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
    }
    for (a, cause) in &dead {
        bury_corpse(fauna, flora, a, *cause);
    }
    // A host's riders are freed where it fell, whatever their size (parasite §5).
    for (a, _) in &dead {
        fauna.free_riders(a);
    }
}

#[cfg(test)]
#[path = "capgnawer_tests.rs"]
mod capgnawer_tests;
#[cfg(test)]
#[path = "circadian_tests.rs"]
mod circadian_tests;
#[cfg(test)]
#[path = "detritivore_tests.rs"]
mod detritivore_tests;
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
#[path = "chorister_tests.rs"]
mod chorister_tests;
#[cfg(test)]
#[path = "night_survival_tests.rs"]
mod night_survival_tests;
#[cfg(test)]
#[path = "gut_tests.rs"]
mod gut_tests;
#[cfg(test)]
#[path = "hunt_tests.rs"]
mod hunt_tests;
#[cfg(test)]
#[path = "hunt_meal_tests.rs"]
mod hunt_meal_tests;
#[cfg(test)]
#[path = "pack_search_tests.rs"]
mod pack_search_tests;
#[cfg(test)]
#[path = "senses_core_tests.rs"]
mod senses_core_tests;
#[cfg(test)]
#[path = "light_tests.rs"]
mod light_tests;
#[cfg(test)]
#[path = "light_learning_tests.rs"]
mod light_learning_tests;

#[cfg(test)]
#[path = "ground_response_tests.rs"]
mod ground_response_tests;
#[cfg(test)]
#[path = "edge_tests.rs"]
mod edge_tests;
#[cfg(test)]
#[path = "drop_tests.rs"]
mod drop_tests;
#[cfg(test)]
#[path = "hunker_tests.rs"]
mod hunker_tests;
#[cfg(test)]
#[path = "gait_tests.rs"]
mod gait_tests;
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
