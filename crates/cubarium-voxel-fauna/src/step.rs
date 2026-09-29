//! One tick of the animal layer.
//!
//! # Order within a tick, in one place
//!
//! 1. **Terrain.** An animal whose support face is no longer a support face — buried, dug
//!    out, turned to air — leaves the world, booked `removed_*_out`. It does not fall and
//!    it does not die: there is no corpse, because nothing in this round says what a
//!    collapsing floor does to a body.
//! 2. **Maintenance.** `maintenance_per_s · body · dt` of organic matter is respired from
//!    the reserve, and from the body once the reserve is empty (dieback). The energy in it
//!    leaves as heat at the animal's own current density; the mineral it held is **shed
//!    to the gut** by the same fraction, and every `GUT_VOID_S` the gut is voided onto
//!    the ground as litter (dung). Ages
//!    advance here, so `age_ticks` is the number of maintenance payments it has made.
//!    Then **growth**: the reserve pays for new structure at no more than
//!    `growth_max_per_s`, scaled by how full it is, until `body_max` (package G).
//! 3. **Cue field.** On due ticks, update emission, decay and diffusion after maintenance
//!    and before any controller samples.
//! 4. **Controllers** — the sampling point, explicit and shared. Every founder whose
//!    controller period is due is sampled from the tick's pre-action state: its
//!    prior-interval feedback is consumed into the observation, its controller (heuristic
//!    diagnostic or GRU — interchangeable behind [`crate::Controller`]) is driven, and the
//!    resolved, deadbanded actions are **held** until the next sampling. A post-tick
//!    sampler is not this stage; the heuristic bodies have no controller and skip it.
//! 5. **Sense**, from one snapshot of the plant layer taken before any animal has eaten:
//!    what is in reach of the face each animal stands on, and the best face it can smell.
//!    Nothing an animal eats this tick moves another animal's plan this tick — the plant
//!    layer's own light phase works the same way — but a bite is still bounded by the
//!    stand it reads, so two animals on one stand cannot eat it twice.
//! 6. **Act**, in id order: for a heuristic body, crop if a whole bite is in reach, else
//!    step one support face toward the sensed face when a step is due, else rest. For a
//!    founder, resolve the held actions — the motor budget is paid on the requested
//!    equivalent displacement, the pose sweeps in bounded sub-steps, and on the due tick
//!    one local bite is attempted through the plant layer's real transfers — then record
//!    the interval's motion and intake feedback. Every bite is **sated** (package G): it
//!    asks in proportion to hunger, takes `E / (E + K)` of that from food holding `E`,
//!    and never more than the reserve's room after yield, so a full animal takes nothing
//!    and nothing eaten is respired as surplus.
//! 7. **Reproduction.** A clutch whose face is gone or drowned goes to carrion; every
//!    body's surplus counter and refractory move; a gestating frondgrazer pays its
//!    instalment into its escrow and gives birth at term; a littershredder in surplus
//!    lays a clutch on the litter it stands on; and clutches at term hatch. Only bodies
//!    that were alive at the start of the tick reproduce, so a newborn cannot itself
//!    breed on the tick it appears.
//! 8. **Death.** Starvation (`body < body_min`) or drowning (standing water deeper than
//!    `drown_depth_m` on its own face) hands the whole animal back as **carrion** and
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
//! every pass walks them in it; the plant layer's `reachable_foliage` returns its stands
//! in site order; and a candidate face is scored before any tie is drawn.

use cubarium_voxel::{DT, VoxelView, World};
use cubarium_voxel_flora::{DepositKind, Flora, Site, Taken};

use crate::body;
use crate::controller::Actions;
use crate::manifest::Founder;
use crate::predation::{Hit, Rest, Strike, StrikePhase};
use crate::{Animal, Departure, Fauna, Food, IntervalFeedback, SpeciesConfig, State, steppable};

/// Stream keys, so two draws in one tick cannot be the same draw. One per rule that draws.
const DOMAIN_TARGET: u64 = 1;
const DOMAIN_STEP: u64 = 2;

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
    /// could not hold would be booked here too, but the sated bite (package G) never takes
    /// more than the room after yield, so that share is zero by construction.
    Digestion,
    /// An interrupted gestation: the loss fraction of an escrow whose parent could not
    /// pay the next instalment, or died carrying it.
    Gestation,
    /// A hunt: a strike's cost and a lit lure (predation §2.3, §2.2).
    Hunt,
    /// Healing a wound (predation §2.4).
    Heal,
}

/// A deterministic scalar stream (splitmix64), keyed by the values that **identify** a
/// draw rather than seeded from stored state. The flora layer's `Rng`, field for field, so
/// the two layers draw the same way; it is small enough that sharing it through a public
/// API would be a worse dependency than writing it twice.
pub(crate) struct Rng(u64);

impl Rng {
    pub(crate) fn keyed(domain: u64, a: u64, b: u64, c: u64) -> Rng {
        let mut state = 0u64;
        for part in [domain, a, b, c] {
            state = mix(state ^ part);
        }
        Rng(state)
    }

    pub(crate) fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        mix(self.0)
    }

    fn below(&mut self, n: usize) -> usize {
        (self.next_u64() % n as u64) as usize
    }

    /// A uniform draw on `[0, 1)`.
    pub(crate) fn unit(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 / (1u64 << 53) as f64
    }
}

/// splitmix64's finalizer: the avalanche that makes neighbouring keys independent.
fn mix(mut z: u64) -> u64 {
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

/// What one animal sensed this tick, from the pre-bite snapshot of the plant layer.
struct Plan {
    /// The stands in reach of the face it stands on, in site order, with their foliage.
    reach: Vec<(Site, f64)>,
    /// The foliage in reach, summed.
    total: f64,
    /// The face it would rather be standing on: the one within `sense_radius` from which
    /// the most foliage is reachable, and at least a whole bite of it. `None` when nothing
    /// it can smell is worth walking to.
    target: Option<Site>,
}

pub(crate) fn step(fauna: &mut Fauna, world: &World, flora: &mut Flora, threads: usize) {
    tick_inner(fauna, world, flora, threads, None);
}

/// The static arena's tick: [`step`] with the per-arena [`Senses`] handle. The camera-free
/// side effect is the field update, which the tick fires at [`UPDATE_TICKS`] cadence
/// before any controller samples, per the frozen arena's tick order.
pub(crate) fn step_with_senses(
    fauna: &mut Fauna,
    world: &World,
    flora: &mut Flora,
    threads: usize,
    senses: &mut crate::Senses,
) {
    tick_inner(fauna, world, flora, threads, Some(senses));
}

fn tick_inner(
    fauna: &mut Fauna,
    world: &World,
    flora: &mut Flora,
    threads: usize,
    mut senses: Option<&mut crate::Senses>,
) {
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
            .is_some_and(|before| before >= crate::Activity::DAY_LINE)
        && daylight < crate::Activity::DAY_LINE;
    fauna.last_daylight = Some(daylight);

    cubarium_voxel::voxel_phase!(FaunaStep, {
        cubarium_voxel::voxel_phase!(FaunaTerrain, { terrain(fauna, &view) });
        cubarium_voxel::voxel_phase!(FaunaMaintenance, {
            // WX4: who is asleep this tick is settled before anything is paid, so the
            // upkeep, the controller stage and the act all read the same answer.
            circadian(fauna, clock_on, daylight);
            maintenance(fauna);
            heal(fauna);
            egg_upkeep(fauna);
            grow(fauna);
            void_guts(fauna, flora);
        });
        // Maintenance and age settle first. A due field update then reads the litter as
        // it stands after the previous tick's feeding, before this tick's controllers.
        if let Some(s) = senses.as_deref_mut() {
            if tick % crate::UPDATE_TICKS == 0 {
                let fv = flora.view();
                s.update(&view, &fv);
            }
        }
        let plans = cubarium_voxel::voxel_phase!(FaunaSense, {
            // The controller stage: sample due founders and hold their actions, before
            // anything moves or eats this tick, from the same pre-action state every
            // controller of the tick reads.
            controllers(fauna, &view, flora, senses);
            sense(fauna, &view, flora, seed, tick, threads)
        });
        cubarium_voxel::voxel_phase!(FaunaAct, {
            // Strike outcomes are events: the act only reads other bodies, and the hits
            // are applied here, serially, in the order the strikers acted.
            let mut hits: Vec<Hit> = Vec::new();
            act(fauna, &view, flora, &plans, seed, tick, &mut hits);
            for hit in hits {
                apply_hit(fauna, flora, hit.prey, hit.damage, hit.by);
            }
        });
        cubarium_voxel::voxel_phase!(FaunaBirths, { reproduce(fauna, &view, flora, dusk) });
        cubarium_voxel::voxel_phase!(FaunaDeaths, { deaths(fauna, &view, flora) });
        #[cfg(feature = "profile")]
        cubarium_voxel::profile::add(
            cubarium_voxel::profile::Count::Animals,
            fauna.animals.len() as u64,
        );
    });
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
        fauna.book_removed(a);
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
    let a = &mut fauna.animals[i];
    let before = a.organic();
    let from_reserve = want.min(a.reserve);
    let from_body = (want - from_reserve).min(a.body);
    let paid = from_reserve + from_body;
    if !(paid > 0.0) {
        return (0.0, 0.0);
    }
    a.reserve -= from_reserve;
    a.body -= from_body;
    // Energy leaves with the organic matter at the animal's own current density, which
    // is the rule the plant layer's dead pools use: a body half respired is the same
    // stuff it was. An animal respired down to nothing hands over every unit of its
    // energy rather than keeping float dust.
    let e = if paid >= before {
        a.energy
    } else {
        (a.energy * (paid / before)).min(a.energy)
    };
    a.energy -= e;
    // The mineral that organic matter held is shed to the gut by the same fraction rule,
    // so a body's mineral stays in proportion to its tissue instead of banking for life.
    let m = if paid >= before {
        a.mineral
    } else {
        (a.mineral * (paid / before)).min(a.mineral)
    };
    a.mineral -= m;
    a.gut.mineral += m;
    book_respired(fauna, paid, kind);
    fauna.ledger.heat_out += e;
    (paid, from_body)
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
    }
}

/// How often an animal voids its gut, seconds. Review-tunable placeholder.
pub(crate) const GUT_VOID_S: f64 = 30.0;

/// Step 2, last: every animal whose age is a multiple of [`GUT_VOID_S`] empties its gut
/// onto its own face as litter. Birth ticks stagger the voids; a gut holding only mineral
/// goes straight to the soluble pool (the flora layer's zero-organic deposit rule).
fn void_guts(fauna: &mut Fauna, flora: &mut Flora) {
    let period = ((GUT_VOID_S / DT).round() as u64).max(1);
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

/// Step 1b (WX4): who sleeps this tick. A founder sleeps when its lineage's
/// [`Activity`](crate::Activity) says it is not its hour, unless a wake rule fires: it is
/// hungry (its reserve short of its cap by at least `wake_hunger`), it is on a wall face,
/// or it is under water. It sleeps where it stands. Waking is the same test failing, so
/// there is nothing else to time: the phase's own change, or a rule, wakes it at once.
/// A body that falls asleep drops its held actions, so waking does not replay a stale
/// stride; heuristic bodies carry no lineage and never sleep.
///
/// **A rest with a reason** (predation §2.8): the same state carries WX4's [`Rest::Sleep`]
/// and a carnivore's [`Rest::Satiated`] — down in any phase once its reserve is at
/// `rest_at` of its cap, up again below `wake_at` (or by the hunger rule), at its
/// `torpor_upkeep`. A body a hit woke stays up for [`crate::ALERT_S`]; a body mid-strike
/// does not lie down, and a recovery is dropped by a rest that outlasts it.
fn circadian(fauna: &mut Fauna, clock_on: bool, daylight: f64) {
    for i in 0..fauna.animals.len() {
        let Some(founder) = fauna.animals[i].founder else {
            continue;
        };
        let phys = fauna.config.founder(founder);
        let a = &fauna.animals[i];
        let hungry = hunger(a, &phys.core);
        let free = a.alert_ticks == 0
            && a.mobility.climb.is_none()
            && a.mobility.submerged_ticks == 0
            && !a
                .strike
                .is_some_and(|s| s.phase != StrikePhase::Recovery);
        let satiated = phys.hunter.is_some_and(|h| {
            let cap = phys.core.reserve_of(a.body);
            let fraction = if cap > 0.0 { a.reserve / cap } else { 0.0 };
            if a.rest == Some(Rest::Satiated) {
                fraction >= h.wake_at && hungry < phys.wake_hunger
            } else {
                fraction >= h.rest_at
            }
        });
        let rest = if !free {
            None
        } else if satiated {
            Some(Rest::Satiated)
        } else if !phys.active.awake(clock_on, daylight) && hungry < phys.wake_hunger {
            Some(Rest::Sleep)
        } else {
            None
        };
        let a = &mut fauna.animals[i];
        a.alert_ticks = a.alert_ticks.saturating_sub(1);
        match rest {
            Some(reason) => {
                if a.state != State::Asleep {
                    a.state = State::Asleep;
                    a.founder_state.held = Actions::REST;
                    a.founder_state.lure = 0.0;
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
    }
}

fn maintenance(fauna: &mut Fauna) {
    for i in 0..fauna.animals.len() {
        // A founder body pays upkeep under its own physiology's rate.
        let sc = body::effective_config(&fauna.config, &fauna.animals[i]);
        let is_founder = fauna.animals[i].founder.is_some();
        // Asleep it pays the lineage's fraction of it (WX4): its sleep factor, or a
        // carnivore's torpor in a satiated rest (predation §2.8).
        let rest = match fauna.animals[i].founder {
            Some(founder) if fauna.animals[i].state == State::Asleep => {
                let phys = fauna.config.founder(founder);
                match (fauna.animals[i].rest, phys.hunter) {
                    (Some(Rest::Satiated), Some(h)) => h.torpor_upkeep,
                    _ => phys.sleep_upkeep,
                }
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

/// Step 2: **healing** (predation §2.4). A wound heals at [`crate::WOUND_HEAL_PER_S`],
/// each unit costing [`crate::HEAL_COST`] × its body out of the **reserve** alone: a body
/// whose reserve cannot pay heals only as far as it can, and a starving one not at all.
fn heal(fauna: &mut Fauna) {
    for i in 0..fauna.animals.len() {
        let a = &fauna.animals[i];
        if !(a.wound > 0.0) || !(a.reserve > 0.0) || !(a.body > 0.0) {
            continue;
        }
        let unit = crate::HEAL_COST * a.body;
        let mut healed = a.wound.min(crate::WOUND_HEAL_PER_S * DT);
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

/// Step 2: a **dormant egg's upkeep** (predation §2.13): [`crate::EGG_UPKEEP_PER_S`] of
/// its own organic matter, respired with the energy at its own density; its mineral stays
/// in the egg.
fn egg_upkeep(fauna: &mut Fauna) {
    for k in 0..fauna.clutches.len() {
        let c = &mut fauna.clutches[k];
        if c.dormancy.is_none() || !(c.organic > 0.0) {
            continue;
        }
        let paid = (crate::EGG_UPKEEP_PER_S * c.organic * DT).min(c.organic);
        let heat = (c.energy * (paid / c.organic)).min(c.energy);
        c.organic -= paid;
        c.energy -= heat;
        book_respired(fauna, paid, Respiration::Maintenance);
        fauna.ledger.heat_out += heat;
    }
}

/// Step 2, second half: **growth on plant time** (package G, decision 4). New structure is
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
        let room = (sc.body_max - a.body).max(0.0);
        if !(room > 0.0) || !(a.reserve > 0.0) {
            continue;
        }
        let cap = sc.reserve_of(a.body);
        let fullness = if cap > 0.0 {
            (a.reserve / cap).min(1.0)
        } else {
            1.0
        };
        let built = (sc.growth_max_per_s * DT * fullness)
            .min(room)
            .min(a.reserve);
        if built > 0.0 {
            a.reserve -= built;
            a.body += built;
        }
    }
}

/// **Hunger**: how far the reserve is short of its cap `reserve_cap · body`, from 0 (full)
/// to 1 (empty). A newborn's paid endowment above the cap reads as sated.
fn hunger(a: &Animal, sc: &SpeciesConfig) -> f64 {
    let cap = sc.reserve_of(a.body);
    if !(cap > 0.0) {
        return 0.0;
    }
    (1.0 - a.reserve / cap).clamp(0.0, 1.0)
}

/// **The sated bite** (package G, decisions 1 and 2): `want · E / (E + K)` from food
/// holding `E` at the mouth, and never more than `room / yield` — the organic matter
/// whose assimilated share exactly fills the reserve's room. Assimilation is at most
/// `yield ·` the bite (less when the bite's mineral binds), so what is built always fits
/// and nothing is respired as surplus.
fn sated_bite(want: f64, stock: f64, sc: &SpeciesConfig, a: &Animal, yield_fraction: f64) -> f64 {
    if !(want > 0.0) || !(stock > 0.0) {
        return 0.0;
    }
    let k = sc.bite_half_stock;
    let diminished = if k > 0.0 {
        want * (stock / (stock + k))
    } else {
        want
    };
    diminished
        .min(room_after_yield(a, sc, yield_fraction))
        .max(0.0)
}

/// The most organic matter a bite may take: the reserve's room over the yield, so that
/// what the bite builds exactly fills it. Unbounded for a food that builds nothing.
fn room_after_yield(a: &Animal, sc: &SpeciesConfig, yield_fraction: f64) -> f64 {
    let room = (sc.reserve_of(a.body) - a.reserve).max(0.0);
    if yield_fraction > 0.0 {
        room / yield_fraction
    } else {
        f64::INFINITY
    }
}

/// Step 3: the **controller stage** — sample every founder whose controller period is
/// due, consume its prior-interval feedback into the observation, and hold the resolved
/// actions until the next sampling.
///
/// This is the tick's one sampling point, and it is shared: it runs inside the fauna
/// tick, after maintenance and age, before any motion or feeding, and every driver —
/// the live schedule, the static arena, an ES episode harness — reaches it through
/// [`Fauna::step`]. A post-tick sampler is not this stage. Every controller of the tick
/// reads the same pre-action state.
fn controllers(
    fauna: &mut Fauna,
    view: &VoxelView<'_>,
    flora: &Flora,
    mut senses: Option<&mut crate::Senses>,
) {
    let due: Vec<usize> = fauna
        .animals
        .iter()
        .enumerate()
        .filter(|(_, a)| {
            let Some(founder) = a.founder else {
                return false;
            };
            // An asleep body's controller is not consulted (WX4).
            // WX4: a phase input for trained brains is the next training round's job; the
            // sensor manifest gains it there, not here.
            a.state != State::Asleep && a.age_ticks % founder.manifest().cadence_ticks() == 0
        })
        .map(|(i, _)| i)
        .collect();
    if due.is_empty() {
        return;
    }
    let fv = flora.view();
    // The cone occupancy is built once per stage, and only over the columns the due
    // browsers' rays can reach (P5-A item 7): a ray reads exactly what it would against
    // the whole world, and a stage with one browser due indexes one browser's share of
    // the ring. A static episode's senses hold it instead (cone-speed item 4): built
    // once, patched from bites and deaths, bodies re-indexed — the same grid either way.
    let observers: Vec<f64> = due
        .iter()
        .filter(|&&i| {
            fauna.animals[i]
                .founder
                .is_some_and(|f| f.manifest().cone_range_m > 0.0)
        })
        .map(|&i| fauna.animals[i].pose.x)
        .collect();
    // A heuristic prey looks for threats out to `FLEE_SIGHT_M`, farther than its cone:
    // the window covers it whenever there is a hunter in the world to look for.
    let hunters_here = fauna
        .animals
        .iter()
        .any(|a| a.founder.is_some_and(|f| fauna.config.founder(f).hunter.is_some()));
    let window_m = if hunters_here {
        Founder::Browser.manifest().cone_range_m.max(FLEE_SIGHT_M)
    } else {
        Founder::Browser.manifest().cone_range_m
    };
    let mut held = senses.as_deref_mut().and_then(|s| s.take_cone());
    let windowed;
    let cone_occupancy = if observers.is_empty() {
        None
    } else if let Some(held) = held.as_mut() {
        Some(held.refresh(view, &fv, &fauna.view()))
    } else {
        windowed = crate::senses::cone_occupancy_window(
            view,
            &fv,
            &fauna.view(),
            observers.iter().copied(),
            window_m,
        );
        Some(&windowed)
    };
    for i in due {
        let id = fauna.animals[i].id;
        let founder = fauna.animals[i].founder.expect("a due founder");
        let manifest = founder.manifest();
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
        if fauna.animals[i].age_ticks == manifest.cadence_ticks() {
            fauna.animals[i].founder_state.feedback = IntervalFeedback::default();
        }
        let obs = body::observation(
            fauna,
            i,
            view,
            &fv,
            &manifest,
            senses.as_deref_mut(),
            cone_occupancy,
        );
        // Reject rather than train through: an observation with a non-finite channel
        // holds rest instead of reaching the controller at all.
        let (mut actions, mut lure, driven) = if obs.iter().all(|v| v.is_finite()) {
            let response = fauna.controllers.drive(id, &obs);
            let driven = response.is_some();
            let lure = if driven { fauna.controllers.lure(id) } else { 0.0 };
            (
                response.map_or(Actions::REST, |r| {
                    crate::controller::resolve_actions(r, &manifest)
                }),
                lure,
                driven,
            )
        } else {
            (Actions::REST, 0.0, false)
        };
        // The prey heuristics' flee rule (predation §2.7): a body driven by its own
        // heuristic — not a saved policy, whose behaviour changes only through the world
        // — turns away from a seen threat within 3 m, or from tremor, and resumes after.
        // Only while a hunter is in the world: a teacher in a world with none behaves as
        // it always did.
        if driven && hunters_here && fauna.controllers.flees(id) {
            if let Some(flee) = flee(fauna, i, view, cone_occupancy) {
                actions = flee;
                lure = 0.0;
            }
        }
        let a = &mut fauna.animals[i];
        a.founder_state.held = actions;
        a.founder_state.lure = if lure.is_finite() { lure.clamp(0.0, 1.0) } else { 0.0 };
        a.founder_state.feedback = IntervalFeedback::default();
    }
    if let Some(s) = senses {
        s.return_cone(held);
    }
}

/// How far a sighted prey heuristic looks for a threat (predation §2.7, the grazer's 3 m).
pub(crate) const FLEE_SIGHT_M: f64 = 3.0;
/// The tremor, `[left + right]`, a blind prey heuristic flees (predation §2.7): a walking
/// lanternjaw anywhere inside the 1.5 m radius (≥ 0.019), a walking grazer inside it, a
/// walking shredder within about 0.7 m. A heuristic's number, not the model's.
pub(crate) const FLEE_TREMOR: f64 = 0.015;

/// **The flee rule** of the prey heuristics (predation §2.7), `None` when there is
/// nothing to flee: the grazer turns away from the nearest threat it can see within 3 m
/// ([`crate::kind_of`], so a still hunter beyond 0.5 m is not one), inside its field of
/// view and not hidden by terrain or foliage; the blind shredder turns away from tremor
/// above [`FLEE_TREMOR`]. Full forward, no feeding.
fn flee(
    fauna: &Fauna,
    i: usize,
    view: &VoxelView<'_>,
    occupancy: Option<&crate::senses::ConeOccupancy>,
) -> Option<Actions> {
    let a = &fauna.animals[i];
    let founder = a.founder?;
    let config = &fauna.config;
    let hunted = Founder::ALL
        .iter()
        .any(|&f| config.founder(f).hunter.is_some_and(|h| h.hunts(founder)));
    if !hunted {
        return None;
    }
    let away = |turn: f64| Actions {
        forward: 1.0,
        turn,
        feed: 0.0,
    };
    if founder.manifest().cone_range_m <= 0.0 {
        let [left, right] = fauna.view().tremor(view, a.id);
        if left + right < FLEE_TREMOR {
            return None;
        }
        return Some(away(if left > right { 1.0 } else { -1.0 }));
    }
    let occupancy = occupancy?;
    let geometry = crate::body_of(config, a)?;
    let v = view.config.voxel_m;
    let ring = f64::from(view.config.width) * v;
    let origin = crate::senses::cone_origin(view, &a.pose, a.sense_layer(v), &geometry);
    let (fx, fz) = a.pose.forward();
    let (rx, rz) = (fz, -fx);
    let mut nearest: Option<(f64, f64)> = None;
    for o in &fauna.animals {
        if o.id == a.id {
            continue;
        }
        let (dx, dz) = crate::pose::wrapped_delta(a.pose.x, a.pose.z, o.pose.x, o.pose.z, ring);
        let d = (dx * dx + dz * dz).sqrt();
        if d > FLEE_SIGHT_M || nearest.is_some_and(|(n, _)| d >= n) {
            continue;
        }
        if crate::kind_of(config, a, o, d) != crate::BodyKind::Threat {
            continue;
        }
        // The cone looks forward: a threat behind it is not seen.
        if dx * fx + dz * fz < 0.0 {
            continue;
        }
        let Some(og) = crate::body_of(config, o) else {
            continue;
        };
        let ty = crate::surface_m(o.sense_layer(v), v) + 0.5 * og.height_m;
        let (tx, tz) = (origin.0 + dx, origin.2 + dz);
        let (ex, ey, ez) = (tx - origin.0, ty - origin.1, tz - origin.2);
        let dist = (ex * ex + ey * ey + ez * ez).sqrt();
        if !(dist > 0.0) {
            continue;
        }
        let dir = (ex / dist, ey / dist, ez / dist);
        let seen = matches!(
            crate::senses::ray_first_hit(view, occupancy, a.id, origin, dir, dist + og.width_m),
            Some((hit, crate::senses::Fine::Body)) if hit >= dist - og.width_m - v
        );
        if seen {
            nearest = Some((d, dx * rx + dz * rz));
        }
    }
    let (_, side) = nearest?;
    Some(away(if side > 0.0 { -1.0 } else { 1.0 }))
}

/// Step 4: one snapshot of what every animal can reach and smell.
///
/// **The sensing domain is a set of standing faces (Astra R9.5).** A candidate is any
/// support face within `sense_radius` of the animal's own face — wrapped `x` and plain `z`,
/// each measured on its own — that this species could stand on, and its score is
/// `reachable_foliage` from that face, the model's own reach query. So what an animal can
/// find is bounded by where it could stand, and what it can eat there is decided by the
/// same rule a bite uses. Nothing here classifies a stand as edible.
///
/// Two approximations stay, and stay stated: there is **no line of sight** — the reach box
/// and this radius are both geometry, so neither knows about a wall between the animal and
/// the food — and the walk is **greedy** (`toward`), one strictly-closer orthogonal step
/// per period. Neither establishes a body-sized passage, an obstacle refuge or a path.
fn sense(
    fauna: &Fauna,
    view: &VoxelView<'_>,
    flora: &Flora,
    seed: u64,
    tick: u64,
    threads: usize,
) -> Vec<Plan> {
    let fv = flora.view();
    let width = i64::from(view.config.width);
    let animals = &fauna.config;

    #[cfg(feature = "parallel")]
    if threads > 1 && fauna.animals.len() >= 2 {
        // One chunk per worker, at least one animal each. Each chunk returns its plans in
        // its own animal order together with **which chunk it was**, and the chunks are
        // folded back in that order, so the finishing order of the workers cannot reach
        // the result: `out` is the same `Vec<Plan>` in the same animal order the serial
        // pass builds. `act` still applies them in id order.
        let per = fauna.animals.len().div_ceil(threads);
        let pool = bevy_tasks::ComputeTaskPool::get_or_init(|| {
            bevy_tasks::TaskPoolBuilder::new()
                .num_threads(threads)
                .build()
        });
        let mut parts: Vec<(usize, Vec<Plan>)> = pool.scope(|scope| {
            for (k, chunk) in fauna.animals.chunks(per).enumerate() {
                scope.spawn(async move {
                    let plans = chunk
                        .iter()
                        .map(|a| plan_for(a, animals, view, &fv, width, seed, tick))
                        .collect::<Vec<Plan>>();
                    (k, plans)
                });
            }
        });
        parts.sort_by_key(|&(k, _)| k);
        let mut out = Vec::with_capacity(fauna.animals.len());
        for (_, plans) in parts {
            out.extend(plans);
        }
        return out;
    }
    let _ = threads;

    let mut out = Vec::with_capacity(fauna.animals.len());
    for a in &fauna.animals {
        out.push(plan_for(a, animals, view, &fv, width, seed, tick));
    }
    out
}

/// What one animal senses: a **pure function** of the world, the plant layer as it stood
/// before anything ate, and this animal. Nothing it returns depends on any other animal,
/// which is what makes [`sense`] splittable.
fn plan_for(
    a: &Animal,
    config: &crate::FaunaConfig,
    view: &VoxelView<'_>,
    fv: &cubarium_voxel_flora::FloraView<'_>,
    width: i64,
    seed: u64,
    tick: u64,
) -> Plan {
    // A phase-one founder runs **none of the live heuristic** in P1-A: no sensing, no
    // target, no crop. It rests where it was placed while maintenance, ageing and death
    // still run. P1-B replaces this empty plan with the founder's local action resolution;
    // P1-C fills the senses. Keeping the branch here (and not in `sense`) means the plans
    // stay one per animal in id order.
    if a.founder.is_some() {
        return Plan {
            reach: Vec::new(),
            total: 0.0,
            target: None,
        };
    }
    {
        let sc = *config.species(a.species);
        let headroom = body_headroom(a, config, view);
        let reach = fv.reachable_foliage(view, a.site, sc.reach);
        let total: f64 = reach.iter().map(|&(_, f)| f).sum();
        let bite = sc.bite_per_s * DT;

        // Candidate faces, in **candidate-face coordinates** (Astra R9.5). Every support
        // face this animal could stand on whose distance from the face it *is* standing on
        // is within `sense_radius` — wrapped `x`, plain `z`, each measured on its own, the
        // same box `sense_radius` always meant — is a candidate, and it is scored below
        // with `reachable_foliage` **from that face**. The sensing domain belongs to the
        // candidate and not to the food: the old stand-centre prefilter bounded the food's
        // column and then added neighbouring faces without bounding them, so it admitted a
        // face nine columns away whose stand was eight, and missed a face eight columns
        // away whose stand was nine. `climb` is deliberately not applied here —
        // `steppable` reads it against the animal's *current* face, and a target several
        // voxels up is still worth walking toward one climbable step at a time.
        //
        // Walked from the stands rather than over the whole radius, which is the **same
        // set**: a face with nothing in reach scores zero and is dropped by the `t < bite`
        // test below, so the only faces that can survive are those within
        // `reach.horizontal` of some crown cell of a foliage-bearing stand. The crown
        // radius is in the span because reach is measured to a crown *cell* and a crown is
        // wider than its own column.
        let sense = i64::from(sc.sense_radius);
        let mut candidates: Vec<Site> = Vec::new();
        for stand in fv.stands.iter().filter(|s| s.foliage > 0.0) {
            let crown = fv
                .config
                .species(stand.species)
                .crown_radius(stand.wood, fv.config.voxel_m)
                .max(0.0);
            let span = i64::from(sc.reach.horizontal) + crown.floor() as i64;
            for dz in -span..=span {
                let z = i64::from(stand.site.z) + dz;
                if z < 0 || z >= i64::from(view.config.depth) {
                    continue;
                }
                for dx in -span..=span {
                    let x = i64::from(stand.site.x) + dx;
                    for face in faces_in_column(view, x, z as u32, &sc, headroom) {
                        if wrapped_dx(width, i64::from(face.x), i64::from(a.site.x)) > sense
                            || (i64::from(face.z) - i64::from(a.site.z)).abs() > sense
                        {
                            continue;
                        }
                        if let Err(at) = candidates.binary_search(&face) {
                            candidates.insert(at, face);
                        }
                    }
                }
            }
        }

        // Score each candidate with the **model's own** reach query, from that face: which
        // stands a browser can eat depends on the face it stands on and not on the species
        // it is looking at (Astra round 8), so nothing here classifies a stand as edible.
        let mut best: Vec<Site> = Vec::new();
        let mut best_total = 0.0f64;
        for &face in &candidates {
            if face == a.site {
                continue;
            }
            let t: f64 = fv
                .reachable_foliage(view, face, sc.reach)
                .iter()
                .map(|&(_, f)| f)
                .sum();
            if t < bite {
                continue;
            }
            if t > best_total {
                best_total = t;
                best.clear();
                best.push(face);
            } else if t == best_total {
                best.push(face);
            }
        }
        #[cfg(feature = "profile")]
        {
            use cubarium_voxel::profile::{Count, add};
            add(Count::ReachQueries, 1 + candidates.len() as u64);
            add(Count::CandidateFaces, candidates.len() as u64);
        }
        let target = match best.len() {
            0 => None,
            1 => Some(best[0]),
            n => {
                let mut rng = Rng::keyed(DOMAIN_TARGET, seed, a.id, tick);
                Some(best[rng.below(n)])
            }
        };
        Plan {
            reach,
            total,
            target,
        }
    }
}

/// The void this body needs over the face it stands on: a phase-one founder's own
/// **height**, in whole voxels ([`crate::Body::headroom_voxels`]). A heuristic species
/// carries no geometry at all, so it asks for the one voxel `is_support` already
/// guarantees.
fn body_headroom(a: &Animal, config: &crate::FaunaConfig, view: &VoxelView<'_>) -> u32 {
    crate::body_of(config, a).map_or(1, |b| b.headroom_voxels(view.config.voxel_m))
}

/// Every support face of a column an animal of this species could stand on: shallow
/// enough water to wade, and `headroom` voxels of void over it, so a body is not offered
/// a slot it does not fit in ([`crate::body::headroom_voxels`]). The `climb` bound
/// belongs to a step and is applied there.
fn faces_in_column(
    view: &VoxelView<'_>,
    x: i64,
    z: u32,
    sc: &SpeciesConfig,
    headroom: u32,
) -> Vec<Site> {
    let c = view.config;
    if z >= c.depth {
        return Vec::new();
    }
    let mut out = Vec::new();
    for y in 0..c.height {
        if view.is_support(x, y, z)
            && view.water_depth_m(x, y, z) <= sc.wade_depth_m
            && crate::body::has_headroom(view, x, y, z, headroom)
        {
            out.push(Site {
                x: x.rem_euclid(i64::from(c.width)) as u32,
                y,
                z,
            });
        }
    }
    out
}

/// Step 5: crop, walk or rest for a heuristic body; resolve the held actions for a
/// founder — all in id order.
fn act(
    fauna: &mut Fauna,
    view: &VoxelView<'_>,
    flora: &mut Flora,
    plans: &[Plan],
    seed: u64,
    tick: u64,
    hits: &mut Vec<Hit>,
) {
    let v = view.config.voxel_m;
    let ring = f64::from(view.config.width) * v;
    for i in 0..fauna.animals.len() {
        let before = fauna.animals[i].pose;
        act_one(fauna, view, flora, plans, seed, tick, hits, i);
        // What the body did, for the senses of others (predation §2.2): its ground speed
        // this tick, and how long it has been still.
        let length = crate::body_of(&fauna.config, &fauna.animals[i]).map_or(v, |b| b.length_m);
        let a = &mut fauna.animals[i];
        let (dx, dz) = crate::pose::wrapped_delta(before.x, before.z, a.pose.x, a.pose.z, ring);
        let speed = (dx * dx + dz * dz).sqrt() / DT;
        a.speed_m_s = if speed.is_finite() { speed } else { 0.0 };
        if a.speed_m_s > crate::STILL_SPEED_BL_PER_S * length {
            a.still_ticks = 0;
        } else {
            a.still_ticks = a.still_ticks.saturating_add(1);
        }
    }
}

/// One body's act (step 5).
#[allow(clippy::too_many_arguments)]
fn act_one(
    fauna: &mut Fauna,
    view: &VoxelView<'_>,
    flora: &mut Flora,
    plans: &[Plan],
    seed: u64,
    tick: u64,
    hits: &mut Vec<Hit>,
    i: usize,
) {
    {
        if fauna.animals[i].state == State::Asleep {
            // WX4: no movement, no feeding; the founder's tick is still counted alive.
            if let Some(founder) = fauna.animals[i].founder {
                fauna.ledger.founder_ticks_by_founder[founder.index()] += 1;
            }
            return;
        }
        if fauna.animals[i].founder.is_some() {
            founder_act(fauna, i, view, flora, seed, tick, hits);
            return;
        }
        let sc = *fauna.config.species(fauna.animals[i].species);
        let bite = sc.bite_per_s * DT;
        let plan = &plans[i];
        if plan.total >= bite && bite > 0.0 {
            // At food: a sated body rests where it is, a hungry one crops.
            let want = bite * hunger(&fauna.animals[i], &sc);
            fauna.animals[i].state = if want > 0.0 {
                crop(fauna, flora, i, &sc, want, &plan.reach);
                State::Cropping
            } else {
                State::Resting
            };
            return;
        }
        let Some(target) = plan.target else {
            fauna.animals[i].state = State::Resting;
            return;
        };
        fauna.animals[i].state = State::Walking;
        if fauna.animals[i].age_ticks % sc.step_period_ticks() != 0 {
            return;
        }
        let a = fauna.animals[i];
        let config = fauna.config;
        if let Some(to) = toward(view, &a, &config, target, &sc, seed, tick) {
            fauna.animals[i].site = to;
            // Keep the continuous pose tied to the support face the heuristic just moved
            // to; P1-A's founders do not take this path at all.
            fauna.animals[i].pose = crate::Pose::at_site(to, view.config.voxel_m);
            fauna.ledger.steps += 1;
        }
    }
}

/// One founder tick: pay the motor budget on the **requested** equivalent
/// displacement, resolve the held motion on the continuous pose, re-derive the support
/// site from the pose, record the interval's motion feedback, and — on the due tick —
/// attempt the one local bite. A founder with a zero held action set (no controller
/// yet, or a controller at rest) pays no motor budget and eats nothing, exactly as in
/// P1-A.
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
    let manifest = founder.manifest();
    let phys = *fauna.config.founder(founder);
    let sc = phys.core;
    fauna.ledger.founder_ticks_by_founder[founder.index()] += 1;

    // A carnivore's lit lure costs `lure_power` × its awake upkeep (predation §2.2).
    if let Some(h) = phys.hunter {
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
    // The speed cap (predation §2.4): × (1 − 0.5·wound), and half speed in a recovery.
    {
        let a = &fauna.animals[i];
        let mut slow = (1.0 - 0.5 * a.wound).clamp(0.0, 1.0);
        if a.strike.is_some_and(|s| s.phase == StrikePhase::Recovery) {
            slow *= 0.5;
        }
        held.forward *= slow;
    }

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
    let (motion, standing_y) = {
        let a = &mut fauna.animals[i];
        let mut standing_y = a.site.y;
        let motion = body::resolve_motion(
            view,
            &mut a.pose,
            &mut standing_y,
            &manifest,
            &geometry,
            phys.wade_depth_m(&geometry),
            step_limits,
            held,
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
        let target = match phys.hunter {
            Some(h) if fauna.animals[i].strike.is_none() => {
                prey_in_capsule(fauna, i, view, &geometry, &h)
            }
            _ => None,
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
            return;
        }
        fed = founder_feed(fauna, i, view, flora, &manifest, &geometry, &sc, held.feed);
    }

    fauna.animals[i].state = if fed {
        State::Cropping
    } else if motion.delivered_forward > 0.0 {
        State::Walking
    } else {
        State::Resting
    };
}

/// The jaw's point: the body's nose, half its length ahead of its centre.
fn jaw_of(x: f64, z: f64, heading: f64, length_m: f64) -> (f64, f64) {
    (
        x + 0.5 * length_m * heading.sin(),
        z + 0.5 * length_m * heading.cos(),
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
    let p0 = crate::surface_m(prey.site.y, v) + prey.wall_offset_m();
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
    let jaw = jaw_of(a.pose.x, a.pose.z, a.pose.heading_rad, geometry.length_m);
    let reach = h.strike_reach_length_fraction * geometry.length_m;
    let tip = (
        jaw.0 + reach * a.pose.heading_rad.sin(),
        jaw.1 + reach * a.pose.heading_rad.cos(),
    );
    let mut best: Option<(f64, u64)> = None;
    for o in &fauna.animals {
        if o.id == a.id
            || o.mobility.climb.is_some()
            || !crate::is_prey_of(&fauna.config, a, o)
            || !meets_capsule(&fauna.config, view, a, geometry, h, jaw, tip, o)
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
    let manifest = founder.manifest();
    let reach = h.strike_reach_length_fraction * geometry.length_m;
    let per_tick = reach / f64::from(crate::Hunter::ticks(h.lunge_s));
    let lunge = Actions {
        forward: per_tick / (manifest.cruise_m_per_s * DT),
        turn: 0.0,
        feed: 0.0,
    };
    let step_limits = phys.step_limits(view.config.voxel_m);
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
    let jaw_from = jaw_of(s.from_x, s.from_z, heading, geometry.length_m);
    let jaw_to = jaw_of(a.pose.x, a.pose.z, heading, geometry.length_m);
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
            let chance = crate::dodge_chance(pp.agility, p.state, from_rear, condition);
            if crate::dodges(chance, seed, a.id, p.id, tick) {
                fauna.ledger.dodges_by_founder[founder.index()] += 1;
            } else {
                fauna.ledger.hits_by_founder[founder.index()] += 1;
                let damage = if p.body > 0.0 {
                    h.strike_power * (a.body / p.body) * (1.0 - pp.armour)
                } else {
                    1.0
                };
                hits.push(Hit {
                    prey: p.id,
                    damage,
                    by: founder,
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

/// **One [`Hit`] applied** (predation §2.4, §2.5): the wound adds, the hit wakes the prey
/// and keeps it up for [`crate::ALERT_S`], and a wound of 1 kills — the whole body to
/// carrion on its face, a carcass record on the pool, departed `Killed { by }`. A prey
/// that is gone already is left alone: it dies once.
pub(crate) fn apply_hit(fauna: &mut Fauna, flora: &mut Flora, prey: u64, damage: f64, by: Founder) {
    let Ok(j) = fauna.animals.binary_search_by_key(&prey, |a| a.id) else {
        return;
    };
    let a = &mut fauna.animals[j];
    if damage.is_finite() && damage > 0.0 {
        a.wound += damage;
    }
    if a.state == State::Asleep {
        a.state = State::Resting;
    }
    a.rest = None;
    a.alert_ticks = ((crate::ALERT_S / DT).round() as u32).max(1);
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
    let a = fauna.animals.remove(j);
    fauna.controllers.take(a.id);
    let organic = a.organic() + a.gut.organic;
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
    let laid = fauna.book_deposit(
        flora,
        a.site,
        DepositKind::Carrion,
        Taken {
            organic: a.organic() + a.gut.organic,
            mineral: a.mineral + a.gut.mineral,
            energy: a.energy + a.gut.energy,
        },
    );
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

/// **Attach to a face** (package mobility, decision 1). The body's sweep was refused
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

/// **One tick on a face** (package mobility, decision 1). The feet move along it at half
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
    let effort = held.forward.clamp(0.0, 1.0);
    let cost = phys.motor_respiration_per_s
        * fauna.animals[i].body
        * effort
        * phys.wall_climb_cost_factor
        * DT;
    let (motor_paid, motor_body) = respire(fauna, i, cost, Respiration::Motor);
    let rate = 0.5 * effort * manifest.cruise_m_per_s;
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
/// Decisions §3 gave the shredder its three foods. There is no new action: `bite` still
/// means "take what is at the mouth", and [`body::mouth_detritus`] resolves which of
/// the three that is (the richest available, ties in [`Food::ALL`] order). The
/// assimilation rule is unchanged; what the food class chooses is the `yield` it reads
/// ([`crate::FounderPhysiology::yield_for`]).
#[allow(clippy::too_many_arguments)]
fn founder_feed(
    fauna: &mut Fauna,
    i: usize,
    view: &VoxelView<'_>,
    flora: &mut Flora,
    manifest: &crate::Manifest,
    geometry: &crate::Body,
    sc: &SpeciesConfig,
    effort: f64,
) -> bool {
    // The hungry rate, scaled by hunger: a full reserve asks for nothing at all.
    let want =
        sc.bite_per_s * manifest.controller_period_s * effort * hunger(&fauna.animals[i], sc);
    if !(want > 0.0) {
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
    // with — before anything is withdrawn.
    let meal = match founder {
        Founder::Blind => body::mouth_detritus(&flora.view(), view, &cols, site_y, geometry),
        // The carnivore's mouth takes carrion and nothing else (predation §2.6).
        Founder::Lanternjaw => {
            body::mouth_pool_site(&flora.view(), &cols, site_y, Food::Carrion).map(|(site, stock)| {
                (
                    body::Meal::Pool {
                        site,
                        food: Food::Carrion,
                    },
                    stock,
                )
            })
        }
        Founder::Browser => body::mouth_foliage_stand(
            &flora.view(),
            view,
            &cols,
            site_y,
            geometry,
            crate::Diet::of(founder),
        )
        .map(|(root, stock)| (body::Meal::Stand { root }, stock)),
    };
    let Some((meal, stock)) = meal else {
        return false;
    };
    // Which class this is: a pool names its own, and a stand's is whose mouth chose it
    // — a shredder's stand is fungal because its diet is, a browser's vascular.
    let food = match meal {
        body::Meal::Pool { food, .. } => food,
        body::Meal::Stand { .. } => match founder {
            Founder::Blind => Food::CapTissue,
            Founder::Browser => Food::Foliage,
            Founder::Lanternjaw => unreachable!("a carnivore's mouth takes no stand"),
        },
    };
    // The bite this hunger, this stock and this room allow.
    let yield_fraction = fauna.config.founder(founder).yield_for(food);
    let bite = sated_bite(want, stock, sc, &fauna.animals[i], yield_fraction);
    if !(bite > 0.0) {
        return false;
    }
    // The mouth's own cell range, handed to the plant layer: a foliage bite comes out
    // of the layers the band reaches, lowest first, and the crown above them is not
    // offered (`design/handoffs/voxel-plant-layers-2026-09-22.md`).
    let layers = body::mouth_crown_layers(site_y, geometry, view.config.voxel_m);
    // Whose tissue this is, read before the withdrawal, so the report says which
    // species was actually eaten.
    let plant = match meal {
        body::Meal::Stand { root } => flora.view().stand_at(root).map(|s| s.species),
        body::Meal::Pool { .. } => None,
    };
    let taken = match meal {
        body::Meal::Stand { root } => flora
            .take_foliage_in_layers(root, bite, &layers)
            .map(|t| t.taken),
        body::Meal::Pool { site, food } => match food {
            Food::Litter => flora.take_litter(site, bite),
            Food::Carrion => flora.take_carrion(site, bite),
            Food::CapTissue | Food::Foliage => {
                unreachable!("a stand's tissue is not a ground pool")
            }
        },
    };
    let Some(taken) = taken else {
        return false;
    };
    let taken = fauna.book_eaten(taken, food);
    if let Some(plant) = plant {
        fauna.ledger.bites_by_plant[plant.index()] += 1;
        fauna.ledger.eaten_by_plant[plant.index()] += taken.organic;
    }
    if food == Food::Carrion {
        fauna.ledger.eaten_carrion_by_founder[founder.index()] += taken.organic;
    }
    let placed = assimilate(fauna, i, sc, taken, yield_fraction);
    fauna.ledger.bites_by_founder[founder.index()] += 1;
    fauna.ledger.assimilated_by_founder[founder.index()] += placed;
    fauna.animals[i].founder_state.feedback.intake += placed;
    true
}

/// One mouthful, spent over the stands in reach in site order: the plant layer bounds each
/// withdrawal by the foliage it finds, so what comes back is what the animal got.
///
/// `want` is already scaled by hunger. The sated bite applies here as in the founder
/// mouth: each stand holding `E` edible gives `E / (E + K)` of what is still wanted, and
/// the whole mouthful is capped at the reserve's room after yield.
///
/// **This is the legacy heuristic-species path and it has no diet.** Decisions §3's
/// acceptance rule is written against the two founder *lineages* and is applied at
/// [`crate::Diet`] in the founder mouth; a non-founder body still reaches through
/// `reachable_foliage`, which is species-agnostic, so a heuristic grazer standing at a
/// glowcap would still crop it. Giving the live species a diet means a field on
/// [`SpeciesConfig`] and is not this package's (the live world runs founders).
fn crop(
    fauna: &mut Fauna,
    flora: &mut Flora,
    i: usize,
    sc: &SpeciesConfig,
    want: f64,
    reach: &[(Site, f64)],
) {
    // The room cap, once for the whole mouthful.
    let mut left = want.min(room_after_yield(&fauna.animals[i], sc, sc.yield_fraction));
    for &(site, stock) in reach {
        if !(left > 0.0) {
            break;
        }
        let ask = if sc.bite_half_stock > 0.0 {
            left * (stock / (stock + sc.bite_half_stock))
        } else {
            left
        };
        if !(ask > 0.0) {
            continue;
        }
        // Whose foliage this is, read before the withdrawal, so the report can say which
        // species was eaten rather than which species is in principle edible.
        let plant = flora.view().stand_at(site).map(|s| s.species);
        let Some(taken) = flora.take_foliage(site, ask) else {
            continue;
        };
        let taken = fauna.book_eaten(taken, Food::Foliage);
        if let Some(plant) = plant {
            fauna.ledger.bites_by_plant[plant.index()] += 1;
            fauna.ledger.eaten_by_plant[plant.index()] += taken.organic;
        }
        left -= taken.organic;
        assimilate(fauna, i, sc, taken, sc.yield_fraction);
    }
}

/// A bite becoming tissue: the food class's own `yield_fraction` of it is built **as far as the bite's own
/// mineral pays for**, the undigested `(1 − yield)` share is egested, the unfunded rest is
/// respired, the energy follows the organic matter at the
/// bite's own density, and the undigested share and the mineral that came with more
/// tissue than was built go to the gut, voided later as dung ([`void_guts`]).
///
/// **The mineral budget comes before the tissue (Astra R9.1).** `n_tissue` is mineral per
/// unit of tissue *built*, so a bite can only build `t.mineral / n_tissue` of it. At the
/// placeholders that binds on every bite: a plant's foliage carries `n_tissue` 0.02 and
/// this animal's tissue wants 0.05, so a `1e-4` bite brings `2e-6` of mineral and funds
/// `4e-5` of the `5e-5` its `yield_fraction` would otherwise have built. The unfunded
/// organic matter is **respired with its energy as heat**, exactly like the fraction a
/// full body and a full reserve cannot hold.
///
/// **The yield is the food's, not the eater's** (decisions §3). The rule is unchanged;
/// the caller hands in [`crate::FounderPhysiology::yield_for`] for what was actually
/// eaten, because a detritivore with three foods has no single yield. Every placeholder
/// is 0.5 today, so this moves no number.
///
/// **No internal mineral reserve is adopted.** [`Animal::mineral`] is an inventory of what
/// is already in the tissue and not a stock growth may draw on — the plant layer's
/// `Stand::mineral` is the same and says so (Astra R4.3) — so an animal holding mineral
/// still builds nothing out of a mineral-free bite. Storing mineral for later, and
/// spending it, is a rule decision and not a repair.
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
    // What the yield would build, and what this bite's mineral can actually pay for.
    let funded = if sc.n_tissue > 0.0 {
        (t.mineral / sc.n_tissue).max(0.0)
    } else {
        f64::INFINITY
    };
    let assimilated = (yield_fraction * t.organic).min(funded);
    // Egested: the share the yield never absorbs goes to the gut, not to heat. Only the
    // absorbed matter the bite's mineral cannot fund (and the reserve backstop) is
    // respired.
    let egested = ((1.0 - yield_fraction) * t.organic).clamp(0.0, t.organic - assimilated);
    let mut respired = (t.organic - assimilated - egested).max(0.0);

    // Store: into the reserve, up to `reserve_cap · body`. Structure is built out of the
    // reserve at the growth rate ([`grow`]), never straight from a bite. What the reserve
    // cannot hold is respired, but the sated bite ([`sated_bite`]) never takes more than
    // the room after yield, so this is the conservation backstop and not a rule that
    // fires (package G: an animal never takes food it cannot store).
    let room = (sc.reserve_of(a.body) - a.reserve).max(0.0);
    let to_reserve = assimilated.min(room);
    a.reserve += to_reserve;
    respired += assimilated - to_reserve;
    let placed = to_reserve;

    // Energy: the density of what was eaten, so nothing is created for any food.
    let kept_energy = if t.organic > 0.0 {
        t.energy * (placed / t.organic)
    } else {
        0.0
    };
    let kept_energy = kept_energy.min(t.energy);
    a.energy += kept_energy;
    let egested_energy = if t.organic > 0.0 {
        (t.energy * (egested / t.organic)).min(t.energy - kept_energy)
    } else {
        0.0
    };
    let heat = t.energy - kept_energy - egested_energy;

    // Mineral: what the new tissue needs is kept and the excess joins the gut, voided
    // with the undigested matter as dung. Mineral is never respired and never created.
    let keep_mineral = t.mineral.min(sc.n_tissue * placed);
    a.mineral += keep_mineral;
    a.gut.organic += egested;
    a.gut.mineral += t.mineral - keep_mineral;
    a.gut.energy += egested_energy;

    book_respired(fauna, respired, Respiration::Digestion);
    fauna.ledger.heat_out += heat;
    placed
}

/// One step toward `target`: the strictly closer support face of an orthogonal neighbour
/// column that this animal can climb and wade to, ties broken by the keyed stream.
///
/// `None` when nothing it can stand on is closer — a wall it cannot climb, a pool it
/// cannot wade, or the target being where it already is.
fn toward(
    view: &VoxelView<'_>,
    a: &Animal,
    config: &crate::FaunaConfig,
    target: Site,
    sc: &SpeciesConfig,
    seed: u64,
    tick: u64,
) -> Option<Site> {
    let width = i64::from(view.config.width);
    let headroom = body_headroom(a, config, view);
    let here = distance(width, a.site, target);
    if here == 0 {
        return None;
    }
    let mut best: Vec<Site> = Vec::new();
    let mut best_d = here;
    for (dx, dz) in [(-1i64, 0i64), (1, 0), (0, -1), (0, 1)] {
        let z = i64::from(a.site.z) + dz;
        if z < 0 || z >= i64::from(view.config.depth) {
            continue;
        }
        let x = i64::from(a.site.x) + dx;
        // At most one face per column is a candidate: the one closest in height to the
        // face the animal is standing on, and the lower of two equally close ones. A
        // column is a place to stand, not a choice of storeys.
        let mut faces = steppable(view, a.site, x, z as u32, sc, headroom);
        faces.sort_by_key(|f| (f.y.abs_diff(a.site.y), f.y));
        let Some(&face) = faces.first() else { continue };
        let d = distance(width, face, target);
        if d < best_d {
            best_d = d;
            best.clear();
            best.push(face);
        } else if d == best_d && d < here {
            best.push(face);
        }
    }
    match best.len() {
        0 => None,
        1 => Some(best[0]),
        n => {
            best.sort_unstable();
            let mut rng = Rng::keyed(DOMAIN_STEP, seed, a.id, tick);
            Some(best[rng.below(n)])
        }
    }
}

/// Manhattan distance on the ring: `x` takes the short way round, `z` has real ends. The
/// height is not in it — a browser walks the surface it can stand on, and `climb` is what
/// says whether it can.
fn distance(width: i64, from: Site, to: Site) -> i64 {
    wrapped_dx(width, i64::from(from.x), i64::from(to.x))
        + (i64::from(from.z) - i64::from(to.z)).abs()
}

/// The shorter of the two ways round the strip, in voxels.
fn wrapped_dx(width: i64, a: i64, b: i64) -> i64 {
    let w = width.max(1);
    let d = (a - b).rem_euclid(w);
    d.min(w - d)
}

/// Step 7: **reproduction** — the gestation escrow, live birth, egg laying, incubation
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
                if let Some(newborn) = gestate(fauna, i, &sc, &rule) {
                    newborns.push(newborn);
                }
            }
            crate::BirthMode::Eggs => lay(fauna, i, flora, &sc, &rule, view.config.seed),
        }
    }
    hatch(fauna, view, &mut newborns);
    dormant_eggs(fauna, view, flora, dusk, &mut newborns);
    let fresh: Vec<(u64, Founder)> = newborns
        .iter()
        .filter_map(|n| n.founder.map(|f| (n.id, f)))
        .collect();
    for n in newborns {
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
    if a.reproduction.refractory_ticks > 0 {
        a.reproduction.refractory_ticks -= 1;
        a.reproduction.surplus_ticks = 0;
        return;
    }
    if a.reproduction.escrow.is_some() {
        return;
    }
    let eligible = a.body >= sc.birth_body
        && a.reserve >= rule.surplus_reserve(sc)
        && rule.package_cost(sc) > 0.0;
    if eligible {
        a.reproduction.surplus_ticks += 1;
    } else {
        a.reproduction.surplus_ticks = 0;
    }
}

/// One gestating body's tick: open an escrow when the hold has elapsed, pay this tick's
/// instalment, and hand back the newborn at term. `None` on every other tick.
fn gestate(
    fauna: &mut Fauna,
    i: usize,
    sc: &SpeciesConfig,
    rule: &crate::Reproduction,
) -> Option<Animal> {
    let a = &fauna.animals[i];
    if a.reproduction.escrow.is_none() {
        if a.reproduction.surplus_ticks < rule.hold_ticks() {
            return None;
        }
        fauna.animals[i].reproduction.escrow = Some(crate::Escrow::default());
        fauna.ledger.gestations_opened += 1;
    }
    let n = rule.gestation_ticks();
    let paid = fauna.animals[i]
        .reproduction
        .escrow
        .expect("an open escrow")
        .ticks;
    let k = paid + 1;
    // The instalment is the distance to this tick's **cumulative target**, so the
    // instalments sum to `birth_cost` exactly however the division rounds, and the last
    // one closes whatever the earlier ones left.
    let target = rule.package_cost(sc) * (k as f64 / n as f64);
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
            return None;
        }
        let a = &mut fauna.animals[i];
        let before = a.organic();
        let f = if before > 0.0 {
            (want / before).clamp(0.0, 1.0)
        } else {
            0.0
        };
        let mineral = (a.mineral * f).min(a.mineral);
        let energy = (a.energy * f).min(a.energy);
        a.reserve -= want;
        a.mineral -= mineral;
        a.energy -= energy;
        let e = a.reproduction.escrow.as_mut().expect("an open escrow");
        e.organic += want;
        e.mineral += mineral;
        e.energy += energy;
    }
    let e = fauna.animals[i]
        .reproduction
        .escrow
        .as_mut()
        .expect("an open escrow");
    e.ticks = k;
    if k < n {
        return None;
    }
    // Term. The escrow becomes the newborn, whole: this is the same parcel the parent
    // paid for, so nothing is clamped and nothing is rounded away.
    let e = fauna.animals[i]
        .reproduction
        .escrow
        .take()
        .expect("an open escrow");
    let a = &mut fauna.animals[i];
    a.reproduction.refractory_ticks = rule.interval_ticks();
    a.reproduction.surplus_ticks = 0;
    let (site, species, pose, founder) = (a.site, a.species, a.pose, a.founder);
    let id = fauna.ledger.births;
    fauna.ledger.births += 1;
    fauna.ledger.born += 1;
    Some(Animal {
        id,
        species,
        site,
        pose,
        founder,
        founder_state: crate::FounderState::default(),
        reproduction: crate::ReproductionState::default(),
        body: sc.body_min,
        reserve: e.organic - sc.body_min,
        mineral: e.mineral,
        energy: e.energy,
        age_ticks: 0,
        lifespan_ticks: crate::draw_lifespan_ticks(
            sc.lifespan_median_s,
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
        still_ticks: crate::still_after_ticks(),
        speed_m_s: 0.0,
        alert_ticks: 0,
    })
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
    let heat = if e.organic > 0.0 {
        (e.energy * (lost / e.organic)).min(e.energy)
    } else {
        0.0
    };
    // The lost share's mineral is shed to the gut, as every respiration sheds it.
    let shed = if e.organic > 0.0 {
        (e.mineral * (lost / e.organic)).min(e.mineral)
    } else {
        0.0
    };
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
/// The face has to hold litter — the littershredder lays in the litter it lives in —
/// and a laying that has nowhere to go **costs nothing**: no escrow, no partial payment,
/// no refractory, and the surplus keeps standing for the next tick.
fn lay(
    fauna: &mut Fauna,
    i: usize,
    flora: &Flora,
    sc: &SpeciesConfig,
    rule: &crate::Reproduction,
    seed: u64,
) {
    if fauna.animals[i].reproduction.surplus_ticks < rule.hold_ticks() {
        return;
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
    // the litter it lives in.
    if !rule.dormant && !flora.view().ground_at(site).is_some_and(|g| g.litter > 0.0) {
        return;
    }
    let cost = rule.package_cost(sc);
    let a = &mut fauna.animals[i];
    if !(cost > 0.0) || a.reserve < cost {
        return;
    }
    let before = a.organic();
    let f = if before > 0.0 {
        (cost / before).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let mineral = (a.mineral * f).min(a.mineral);
    let energy = (a.energy * f).min(a.energy);
    a.reserve -= cost;
    a.mineral -= mineral;
    a.energy -= energy;
    a.reproduction.refractory_ticks = rule.interval_ticks();
    a.reproduction.surplus_ticks = 0;
    let laid_tick = fauna.tick;
    if rule.dormant {
        // One clutch per egg, each with its own dormancy; the last egg takes the
        // remainder so the three currencies add back up to what was paid.
        let n = rule.clutch_size.max(1);
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
                dormancy: Some(crate::Dormancy::draw(seed, serial, laid_tick)),
            });
            fauna.ledger.eggs_laid += 1;
        }
    } else {
        fauna.clutches.push(crate::Clutch {
            site,
            count: rule.clutch_size,
            organic: cost,
            mineral,
            energy,
            laid_tick,
            lineage,
            dormancy: None,
        });
        fauna.ledger.eggs_laid += u64::from(rule.clutch_size);
    }
    fauna.ledger.clutches_laid += 1;
    fauna.ledger.eggs_laid_by_founder[lineage.index()] += u64::from(rule.clutch_size);
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
        .filter(|c| {
            let drown = fauna.config.founder(c.lineage).core.drown_depth_m;
            !view.is_support(i64::from(c.site.x), c.site.y, c.site.z)
                || view.water_depth_m(i64::from(c.site.x), c.site.y, c.site.z) > drown
        })
        .copied()
        .collect();
    if lost.is_empty() {
        return;
    }
    fauna.clutches.retain(|c| {
        let drown = fauna.config.founder(c.lineage).core.drown_depth_m;
        view.is_support(i64::from(c.site.x), c.site.y, c.site.z)
            && view.water_depth_m(i64::from(c.site.x), c.site.y, c.site.z) <= drown
    });
    for c in lost {
        fauna.ledger.eggs_lost += u64::from(c.count);
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
    let sc = fauna.config.founder(c.lineage).core;
    let id = fauna.ledger.births;
    fauna.ledger.births += 1;
    fauna.ledger.born += 1;
    fauna.ledger.hatched += 1;
    fauna.ledger.eggs_hatched_by_founder[c.lineage.index()] += 1;
    Animal {
        id,
        species: crate::Species::Frondgrazer,
        site: c.site,
        pose: crate::Pose::at_site(c.site, voxel_m),
        founder: Some(c.lineage),
        founder_state: crate::FounderState::default(),
        reproduction: crate::ReproductionState::default(),
        body: body_min,
        reserve: o - body_min,
        mineral: m,
        energy: e,
        age_ticks: 0,
        lifespan_ticks: crate::draw_lifespan_ticks(
            sc.lifespan_median_s,
            id,
            crate::lifespan_salt(c.site, fauna.tick),
            false,
        ),
        gut: Taken::default(),
        state: State::Resting,
        mobility: crate::Mobility::default(),
        wound: 0.0,
        strike: None,
        rest: None,
        still_ticks: crate::still_after_ticks(),
        speed_m_s: 0.0,
        alert_ticks: 0,
    }
}

/// **The dormant egg bank's tick** (predation §2.13): each dormant egg hears the prey of
/// its lineage that pass within 2 m loudly enough ([`crate::FOOTFALL_TREMOR_MIN`], the
/// tremor rule); an egg whose viability has run out, or whose upkeep has taken it below a
/// hatchling, dies to the litter of its face; and at **dusk** an egg whose footfall over
/// the last clock-day has reached its threshold hatches, unless an adult of its lineage
/// is within 4 m ([`crate::egg_hatches`]).
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
    let day_ticks = ((view.config.day.length_min * 60.0 / DT).round() as u64).max(1);
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
            if prey
                && dist <= crate::FOOTFALL_RADIUS_M
                && crate::tremor_of(a, dist) >= crate::FOOTFALL_TREMOR_MIN
            {
                d.hear(a.id, tick);
            }
        }
        fauna.clutches[k].dormancy = Some(d);
        // The threshold gate, then the per-dusk draw (bet-hedging across dusks).
        if dusk
            && crate::egg_hatches(d.threshold, d.footfall(tick, day_ticks), dusk, nearest_adult)
            && crate::hatch_draw(
                fauna.config.hatch_chance_per_dusk,
                view.config.seed,
                d.serial,
                tick,
            )
        {
            fauna.clutches.remove(k);
            let young = hatchling(fauna, &c, phys.core.body_min, c.organic, c.mineral, c.energy, v);
            newborns.push(young);
            continue;
        }
        k += 1;
    }
}

/// The standing water over a body's feet, metres: the water on its face on the ground;
/// on a wall, the water on the lower face less the feet's height over it (up a wall from
/// a flooded foot it climbs out; down a cliff into a pool it goes under as it nears the
/// foot).
fn water_over_feet(view: &VoxelView<'_>, a: &Animal) -> f64 {
    let depth = |s: Site| view.water_depth_m(i64::from(s.x), s.y, s.z);
    match a.mobility.climb {
        None => depth(a.site),
        Some(c) if c.ascending => depth(c.from) - c.progress_m,
        Some(c) => depth(c.to) - (c.rise_m - c.progress_m),
    }
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
        let Some(founder) = a.founder else {
            drowning.push(
                view.water_depth_m(i64::from(a.site.x), a.site.y, a.site.z) > sc.drown_depth_m,
            );
            continue;
        };
        let phys = fauna.config.founder(founder);
        let geometry = phys.body_at(a.body);
        let under = water_over_feet(view, &a) > phys.drown_depth_m(&geometry);
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
            } else if a.lifespan_ticks > 0 && a.age_ticks >= a.lifespan_ticks {
                Some((i, Departure::Aged))
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
    }
    for (a, cause) in &dead {
        bury_corpse(fauna, flora, a, *cause);
    }
}

#[cfg(test)]
#[path = "circadian_tests.rs"]
mod circadian_tests;
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
