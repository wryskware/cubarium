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
//!    leaves as heat at the animal's own current density; the **mineral stays**, as it
//!    does in a plant: respiration takes organic matter and leaves mineral behind. Ages
//!    advance here, so `age_ticks` is the number of maintenance payments it has made.
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
//!    the interval's motion and intake feedback.
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
use crate::{Animal, Departure, Fauna, IntervalFeedback, SpeciesConfig, State, steppable};

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
    /// Digestion: the undigested fraction of a bite, and anything assimilated that a full
    /// body and a full reserve could not hold.
    Digestion,
    /// An interrupted gestation: the loss fraction of an escrow whose parent could not
    /// pay the next instalment, or died carrying it.
    Gestation,
}

/// A deterministic scalar stream (splitmix64), keyed by the values that **identify** a
/// draw rather than seeded from stored state. The flora layer's `Rng`, field for field, so
/// the two layers draw the same way; it is small enough that sharing it through a public
/// API would be a worse dependency than writing it twice.
struct Rng(u64);

impl Rng {
    fn keyed(domain: u64, a: u64, b: u64, c: u64) -> Rng {
        let mut state = 0u64;
        for part in [domain, a, b, c] {
            state = mix(state ^ part);
        }
        Rng(state)
    }

    fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        mix(self.0)
    }

    fn below(&mut self, n: usize) -> usize {
        (self.next_u64() % n as u64) as usize
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

    cubarium_voxel::voxel_phase!(FaunaStep, {
        cubarium_voxel::voxel_phase!(FaunaTerrain, { terrain(fauna, &view) });
        cubarium_voxel::voxel_phase!(FaunaMaintenance, { maintenance(fauna) });
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
        cubarium_voxel::voxel_phase!(FaunaAct, { act(fauna, &view, flora, &plans, seed, tick) });
        cubarium_voxel::voxel_phase!(FaunaBirths, { reproduce(fauna, &view, flora) });
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
/// current density, the mineral staying behind. It returns what was paid and how much
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
    }
}

fn maintenance(fauna: &mut Fauna) {
    for i in 0..fauna.animals.len() {
        // A founder body pays upkeep under its own physiology's rate.
        let sc = body::effective_config(&fauna.config, &fauna.animals[i]);
        let is_founder = fauna.animals[i].founder.is_some();
        let a = &mut fauna.animals[i];
        a.age_ticks = a.age_ticks.saturating_add(1);
        let want = sc.maintenance_per_s * a.body * DT;
        let (_, from_body) = respire(fauna, i, want, Respiration::Maintenance);
        if is_founder && from_body > 0.0 {
            fauna.animals[i].founder_state.feedback.structural_loss += from_body;
        }
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
            a.age_ticks % founder.manifest().cadence_ticks() == 0
        })
        .map(|(i, _)| i)
        .collect();
    if due.is_empty() {
        return;
    }
    let fv = flora.view();
    let cone_occupancy = due
        .iter()
        .any(|&i| fauna.animals[i].founder == Some(Founder::Browser))
        .then(|| crate::senses::cone_occupancy(view, &fv, &fauna.view()));
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
            cone_occupancy.as_ref(),
        );
        // Reject rather than train through: an observation with a non-finite channel
        // holds rest instead of reaching the controller at all.
        let held = if obs.iter().all(|v| v.is_finite()) {
            let response = fauna.controllers.drive(id, &obs);
            response.map_or(Actions::REST, |r| {
                crate::controller::resolve_actions(r, &manifest)
            })
        } else {
            Actions::REST
        };
        let a = &mut fauna.animals[i];
        a.founder_state.held = held;
        a.founder_state.feedback = IntervalFeedback::default();
    }
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
                .crown_radius(stand.wood)
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
) {
    for i in 0..fauna.animals.len() {
        if fauna.animals[i].founder.is_some() {
            founder_act(fauna, i, view, flora);
            continue;
        }
        let sc = *fauna.config.species(fauna.animals[i].species);
        let bite = sc.bite_per_s * DT;
        let plan = &plans[i];
        if plan.total >= bite && bite > 0.0 {
            crop(fauna, flora, i, &sc, bite, &plan.reach);
            fauna.animals[i].state = State::Cropping;
            continue;
        }
        let Some(target) = plan.target else {
            fauna.animals[i].state = State::Resting;
            continue;
        };
        fauna.animals[i].state = State::Walking;
        if fauna.animals[i].age_ticks % sc.step_period_ticks() != 0 {
            continue;
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
fn founder_act(fauna: &mut Fauna, i: usize, view: &VoxelView<'_>, flora: &mut Flora) {
    let (founder, held, due) = {
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
    // The sweep may put the body's feet on a support face within its lineage's climb of
    // the one it started on, so the standing layer goes in and comes back out
    // (`design/handoffs/voxel-founder-step-2026-09-22.md`).
    let climb = body::climb_voxels(&phys, view.config.voxel_m);
    let (motion, standing_y) = {
        let a = &mut fauna.animals[i];
        let mut standing_y = a.site.y;
        let motion = body::resolve_motion(
            view,
            &mut a.pose,
            &mut standing_y,
            &manifest,
            &geometry,
            sc.wade_depth_m,
            climb,
            held,
        );
        (motion, standing_y)
    };
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

    // Feeding: one attempt per controller interval, on the due tick, from the
    // post-motion mouth, through the plant layer's real withdrawals. A failed or
    // no-contact attempt transfers nothing.
    let mut fed = false;
    if due && held.feed > 0.0 {
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

/// The founder's supported local bite: the blind founder takes litter off the ground
/// under its mouth region, the browser crops a stand whose crown cells it touches —
/// through [`Flora::take_litter`] / [`Flora::take_foliage`], debiting the real stocks,
/// exactly one withdrawal per attempt. Nothing in reach, a mouth in the air, or an
/// unsupported body transfers nothing at all.
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
    let want = sc.bite_per_s * manifest.controller_period_s * effort;
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
    match founder {
        Founder::Blind => {
            let Some(site) = body::mouth_litter_site(&flora.view(), &cols, site_y) else {
                return false;
            };
            let Some(taken) = flora.take_litter(site, want) else {
                return false;
            };
            let taken = fauna.book_eaten(taken);
            let placed = assimilate(fauna, flora, i, sc, taken);
            fauna.ledger.bites_by_founder[founder.index()] += 1;
            fauna.ledger.assimilated_by_founder[founder.index()] += placed;
            fauna.animals[i].founder_state.feedback.intake += placed;
            true
        }
        Founder::Browser => {
            let Some((root, _)) =
                body::mouth_foliage_stand(&flora.view(), view, &cols, site_y, geometry)
            else {
                return false;
            };
            // Whose foliage this is, read before the withdrawal, so the report says
            // which species was actually eaten.
            let plant = flora.view().stand_at(root).map(|s| s.species);
            // The mouth's own cell range, handed to the plant layer: the bite comes out
            // of the layers the band reaches, lowest first, and the crown above them is
            // not offered (`design/handoffs/voxel-plant-layers-2026-09-22.md`).
            let layers = body::mouth_crown_layers(site_y, geometry, view.config.voxel_m);
            let Some(taken) = flora
                .take_foliage_in_layers(root, want, &layers)
                .map(|t| t.taken)
            else {
                return false;
            };
            let taken = fauna.book_eaten(taken);
            if let Some(plant) = plant {
                fauna.ledger.bites_by_plant[plant.index()] += 1;
                fauna.ledger.eaten_by_plant[plant.index()] += taken.organic;
            }
            let placed = assimilate(fauna, flora, i, sc, taken);
            fauna.ledger.bites_by_founder[founder.index()] += 1;
            fauna.ledger.assimilated_by_founder[founder.index()] += placed;
            fauna.animals[i].founder_state.feedback.intake += placed;
            true
        }
    }
}

/// One mouthful, spent over the stands in reach in site order: the plant layer bounds each
/// withdrawal by the foliage it finds, so what comes back is what the animal got.
fn crop(
    fauna: &mut Fauna,
    flora: &mut Flora,
    i: usize,
    sc: &SpeciesConfig,
    bite: f64,
    reach: &[(Site, f64)],
) {
    let mut left = bite;
    for &(site, _) in reach {
        if !(left > 0.0) {
            break;
        }
        // Whose foliage this is, read before the withdrawal, so the report can say which
        // species was eaten rather than which species is in principle edible.
        let plant = flora.view().stand_at(site).map(|s| s.species);
        let Some(taken) = flora.take_foliage(site, left) else {
            continue;
        };
        let taken = fauna.book_eaten(taken);
        if let Some(plant) = plant {
            fauna.ledger.bites_by_plant[plant.index()] += 1;
            fauna.ledger.eaten_by_plant[plant.index()] += taken.organic;
        }
        left -= taken.organic;
        assimilate(fauna, flora, i, sc, taken);
    }
}

/// A bite becoming tissue: `yield_fraction` of it is built **as far as the bite's own
/// mineral pays for**, the rest is respired, the energy follows the organic matter at the
/// bite's own density, and the mineral that came with more tissue than was built is
/// excreted.
///
/// **The mineral budget comes before the tissue (Astra R9.1).** `n_tissue` is mineral per
/// unit of tissue *built*, so a bite can only build `t.mineral / n_tissue` of it. At the
/// placeholders that binds on every bite: a plant's foliage carries `n_tissue` 0.02 and
/// this animal's tissue wants 0.05, so a `1e-4` bite brings `2e-6` of mineral and funds
/// `4e-5` of the `5e-5` its `yield_fraction` would otherwise have built. The unfunded
/// organic matter is **respired with its energy as heat**, exactly like the fraction a
/// full body and a full reserve cannot hold.
///
/// **No internal mineral reserve is adopted.** [`Animal::mineral`] is an inventory of what
/// is already in the tissue and not a stock growth may draw on — the plant layer's
/// `Stand::mineral` is the same and says so (Astra R4.3) — so an animal holding mineral
/// still builds nothing out of a mineral-free bite. Storing mineral for later, and
/// spending it, is a rule decision and not a repair.
/// Returns the organic matter actually **placed** — into body and reserve — which is
/// what a founder's prior-interval intake feedback records, and not the bite's whole
/// organic matter.
fn assimilate(fauna: &mut Fauna, flora: &mut Flora, i: usize, sc: &SpeciesConfig, t: Taken) -> f64 {
    let a = &mut fauna.animals[i];
    // What the yield would build, and what this bite's mineral can actually pay for.
    let funded = if sc.n_tissue > 0.0 {
        (t.mineral / sc.n_tissue).max(0.0)
    } else {
        f64::INFINITY
    };
    let assimilated = (sc.yield_fraction * t.organic).min(funded);
    let mut respired = t.organic - assimilated;

    // Build: structure first, up to `body_max`, then the reserve, up to `reserve_cap ·
    // body`. What neither can hold is respired — an animal cannot store what it cannot
    // carry, and refusing the bite instead would need a rule about hunger this round does
    // not have.
    let to_body = (sc.body_max - a.body).max(0.0).min(assimilated);
    a.body += to_body;
    let rest = assimilated - to_body;
    let room = (sc.reserve_of(a.body) - a.reserve).max(0.0);
    let to_reserve = rest.min(room);
    a.reserve += to_reserve;
    respired += rest - to_reserve;
    let placed = to_body + to_reserve;

    // Energy: the density of what was eaten, so nothing is created for any food.
    let kept_energy = if t.organic > 0.0 {
        t.energy * (placed / t.organic)
    } else {
        0.0
    };
    let kept_energy = kept_energy.min(t.energy);
    a.energy += kept_energy;
    let heat = t.energy - kept_energy;

    // Mineral: what the new tissue needs is kept and the excess is dung, which is litter
    // this round. Mineral is never respired and never created.
    let keep_mineral = t.mineral.min(sc.n_tissue * placed);
    a.mineral += keep_mineral;
    let excess = t.mineral - keep_mineral;
    let site = a.site;

    book_respired(fauna, respired, Respiration::Digestion);
    fauna.ledger.heat_out += heat;
    if excess > 0.0 {
        fauna.book_deposit(
            flora,
            site,
            DepositKind::Litter,
            Taken {
                organic: 0.0,
                mineral: excess,
                energy: 0.0,
            },
        );
    }
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
fn reproduce(fauna: &mut Fauna, view: &VoxelView<'_>, flora: &mut Flora) {
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
            crate::BirthMode::Eggs => lay(fauna, i, flora, &sc, &rule),
        }
    }
    hatch(fauna, view, &mut newborns);
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
        state: State::Resting,
    })
}

/// An interrupted gestation: the loss fraction of the escrowed organic matter is
/// respired with the energy that was in it, and everything else — organic matter,
/// **all** the mineral, the rest of the energy — goes back to the parent. The mineral
/// comes back in full because respiration in this layer never takes mineral: it takes
/// organic matter and leaves the mineral behind, in an animal as in a plant.
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
    let a = &mut fauna.animals[i];
    a.reserve += e.organic - lost;
    a.mineral += e.mineral;
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
fn lay(fauna: &mut Fauna, i: usize, flora: &Flora, sc: &SpeciesConfig, rule: &crate::Reproduction) {
    if fauna.animals[i].reproduction.surplus_ticks < rule.hold_ticks() {
        return;
    }
    let Some(lineage) = fauna.animals[i].founder else {
        return;
    };
    let site = fauna.animals[i].site;
    if !flora.view().ground_at(site).is_some_and(|g| g.litter > 0.0) {
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
    fauna.clutches.push(crate::Clutch {
        site,
        count: rule.clutch_size,
        organic: cost,
        mineral,
        energy,
        laid_tick,
        lineage,
    });
    fauna.ledger.clutches_laid += 1;
    fauna.ledger.eggs_laid += u64::from(rule.clutch_size);
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
    let due: Vec<crate::Clutch> = fauna
        .clutches
        .iter()
        .filter(|c| {
            let rule = fauna.config.founder(c.lineage).core.reproduction;
            tick.saturating_sub(c.laid_tick) >= rule.incubation_ticks()
        })
        .copied()
        .collect();
    if due.is_empty() {
        return;
    }
    fauna.clutches.retain(|c| {
        let rule = fauna.config.founder(c.lineage).core.reproduction;
        tick.saturating_sub(c.laid_tick) < rule.incubation_ticks()
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
            let id = fauna.ledger.births;
            fauna.ledger.births += 1;
            fauna.ledger.born += 1;
            fauna.ledger.hatched += 1;
            newborns.push(Animal {
                id,
                species: crate::Species::Frondgrazer,
                site: c.site,
                pose: crate::Pose::at_site(c.site, voxel_m),
                founder: Some(c.lineage),
                founder_state: crate::FounderState::default(),
                reproduction: crate::ReproductionState::default(),
                body: sc.body_min,
                reserve: o - sc.body_min,
                mineral: m,
                energy: e,
                age_ticks: 0,
                state: State::Resting,
            });
        }
    }
}

/// Step 7: starvation and drowning, and the carrion they leave.
fn deaths(fauna: &mut Fauna, view: &VoxelView<'_>, flora: &mut Flora) {
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
            } else if view.water_depth_m(i64::from(a.site.x), a.site.y, a.site.z) > sc.drown_depth_m
            {
                Some((i, Departure::Drowned))
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
        // The whole animal, whatever is left of it: a corpse with no organic matter left
        // is still a corpse with mineral in it, and keeping an inert remainder on a dead
        // animal is how a ledger stops closing.
        fauna.book_deposit(
            flora,
            a.site,
            DepositKind::Carrion,
            Taken {
                organic: a.organic(),
                mineral: a.mineral,
                energy: a.energy,
            },
        );
        fauna.ledger.deaths += 1;
        fauna.book_departure(a, *cause);
    }
}
