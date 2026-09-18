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
//! 3. **Sense**, from one snapshot of the plant layer taken before any animal has eaten:
//!    what is in reach of the face each animal stands on, and the best face it can smell.
//!    Nothing an animal eats this tick moves another animal's plan this tick — the plant
//!    layer's own light phase works the same way — but a bite is still bounded by the
//!    stand it reads, so two animals on one stand cannot eat it twice.
//! 4. **Act**, in id order: crop if a whole bite is in reach, else step one support face
//!    toward the sensed face when a step is due, else rest.
//! 5. **Births.** An adult with the reserve for one pays it and a newborn appears on its
//!    face. Only animals that were alive at the start of the tick can give birth, so a
//!    newborn cannot itself breed on the tick it is born.
//! 6. **Death.** Starvation (`body < body_min`) or drowning (standing water deeper than
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

use crate::{Animal, Fauna, SpeciesConfig, State, steppable};

/// Stream keys, so two draws in one tick cannot be the same draw. One per rule that draws.
const DOMAIN_TARGET: u64 = 1;
const DOMAIN_STEP: u64 = 2;

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

pub(crate) fn step(fauna: &mut Fauna, world: &World, flora: &mut Flora) {
    // The tick counter moves first, so `fauna.tick` is the tick this step produces — the
    // flora layer's own rule, and the tick the keyed streams are keyed by.
    fauna.tick += 1;
    let tick = fauna.tick;
    let view = world.view();
    let seed = view.config.seed;

    terrain(fauna, &view);
    maintenance(fauna);
    let plans = sense(fauna, &view, flora, seed, tick);
    act(fauna, &view, flora, &plans, seed, tick);
    births(fauna);
    deaths(fauna, &view, flora);
}

/// Step 1: an animal standing on what is no longer a support face leaves the world.
fn terrain(fauna: &mut Fauna, view: &VoxelView<'_>) {
    if fauna.animals.iter().all(|a| view.is_support(i64::from(a.site.x), a.site.y, a.site.z)) {
        return;
    }
    let gone: Vec<Animal> = fauna
        .animals
        .iter()
        .filter(|a| !view.is_support(i64::from(a.site.x), a.site.y, a.site.z))
        .copied()
        .collect();
    fauna.animals.retain(|a| view.is_support(i64::from(a.site.x), a.site.y, a.site.z));
    for a in &gone {
        fauna.book_removed(a);
    }
}

/// Step 2: the upkeep, out of the reserve and then out of the body.
fn maintenance(fauna: &mut Fauna) {
    for i in 0..fauna.animals.len() {
        let sc = *fauna.config.species(fauna.animals[i].species);
        let a = &mut fauna.animals[i];
        a.age_ticks = a.age_ticks.saturating_add(1);
        let want = sc.maintenance_per_s * a.body * DT;
        if !(want > 0.0) {
            continue;
        }
        let before = a.organic();
        let from_reserve = want.min(a.reserve);
        let from_body = (want - from_reserve).min(a.body);
        let paid = from_reserve + from_body;
        if !(paid > 0.0) {
            continue;
        }
        a.reserve -= from_reserve;
        a.body -= from_body;
        // Energy leaves with the organic matter at the animal's own current density, which
        // is the rule the plant layer's dead pools use: a body half respired is the same
        // stuff it was. An animal respired down to nothing hands over every unit of its
        // energy rather than keeping float dust.
        let e = if paid >= before { a.energy } else { (a.energy * (paid / before)).min(a.energy) };
        a.energy -= e;
        fauna.ledger.respired_out += paid;
        fauna.ledger.heat_out += e;
    }
}

/// Step 3: one snapshot of what every animal can reach and smell.
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
) -> Vec<Plan> {
    let fv = flora.view();
    let width = i64::from(view.config.width);
    let mut out = Vec::with_capacity(fauna.animals.len());
    for a in &fauna.animals {
        let sc = *fauna.config.species(a.species);
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
            let crown = fv.config.species(stand.species).crown_radius(stand.wood).max(0.0);
            let span = i64::from(sc.reach.horizontal) + crown.floor() as i64;
            for dz in -span..=span {
                let z = i64::from(stand.site.z) + dz;
                if z < 0 || z >= i64::from(view.config.depth) {
                    continue;
                }
                for dx in -span..=span {
                    let x = i64::from(stand.site.x) + dx;
                    for face in faces_in_column(view, x, z as u32, &sc) {
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
            let t: f64 = fv.reachable_foliage(view, face, sc.reach).iter().map(|&(_, f)| f).sum();
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
        let target = match best.len() {
            0 => None,
            1 => Some(best[0]),
            n => {
                let mut rng = Rng::keyed(DOMAIN_TARGET, seed, a.id, tick);
                Some(best[rng.below(n)])
            }
        };
        out.push(Plan { reach, total, target });
    }
    out
}

/// Every support face of a column an animal of this species could stand on: shallow
/// enough water to wade. The `climb` bound belongs to a step and is applied there.
fn faces_in_column(view: &VoxelView<'_>, x: i64, z: u32, sc: &SpeciesConfig) -> Vec<Site> {
    let c = view.config;
    if z >= c.depth {
        return Vec::new();
    }
    let mut out = Vec::new();
    for y in 0..c.height {
        if view.is_support(x, y, z) && view.water_depth_m(x, y, z) <= sc.wade_depth_m {
            out.push(Site { x: x.rem_euclid(i64::from(c.width)) as u32, y, z });
        }
    }
    out
}

/// Step 4: crop, walk or rest, in id order.
fn act(
    fauna: &mut Fauna,
    view: &VoxelView<'_>,
    flora: &mut Flora,
    plans: &[Plan],
    seed: u64,
    tick: u64,
) {
    for i in 0..fauna.animals.len() {
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
        if let Some(to) = toward(view, &a, target, &sc, seed, tick) {
            fauna.animals[i].site = to;
            fauna.ledger.steps += 1;
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
        let Some(taken) = flora.take_foliage(site, left) else { continue };
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
fn assimilate(fauna: &mut Fauna, flora: &mut Flora, i: usize, sc: &SpeciesConfig, t: Taken) {
    let a = &mut fauna.animals[i];
    // What the yield would build, and what this bite's mineral can actually pay for.
    let funded =
        if sc.n_tissue > 0.0 { (t.mineral / sc.n_tissue).max(0.0) } else { f64::INFINITY };
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
    let kept_energy = if t.organic > 0.0 { t.energy * (placed / t.organic) } else { 0.0 };
    let kept_energy = kept_energy.min(t.energy);
    a.energy += kept_energy;
    let heat = t.energy - kept_energy;

    // Mineral: what the new tissue needs is kept and the excess is dung, which is litter
    // this round. Mineral is never respired and never created.
    let keep_mineral = t.mineral.min(sc.n_tissue * placed);
    a.mineral += keep_mineral;
    let excess = t.mineral - keep_mineral;
    let site = a.site;

    fauna.ledger.respired_out += respired;
    fauna.ledger.heat_out += heat;
    if excess > 0.0 {
        fauna.book_deposit(
            flora,
            site,
            DepositKind::Litter,
            Taken { organic: 0.0, mineral: excess, energy: 0.0 },
        );
    }
}

/// One step toward `target`: the strictly closer support face of an orthogonal neighbour
/// column that this animal can climb and wade to, ties broken by the keyed stream.
///
/// `None` when nothing it can stand on is closer — a wall it cannot climb, a pool it
/// cannot wade, or the target being where it already is.
fn toward(
    view: &VoxelView<'_>,
    a: &Animal,
    target: Site,
    sc: &SpeciesConfig,
    seed: u64,
    tick: u64,
) -> Option<Site> {
    let width = i64::from(view.config.width);
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
        let mut faces = steppable(view, a.site, x, z as u32, sc);
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

/// Step 5: one newborn per adult that can pay for it, out of the parent's reserve.
fn births(fauna: &mut Fauna) {
    let parents = fauna.animals.len();
    let mut newborns: Vec<Animal> = Vec::new();
    for i in 0..parents {
        let sc = *fauna.config.species(fauna.animals[i].species);
        let a = &mut fauna.animals[i];
        if !(a.body >= sc.birth_body && a.reserve >= sc.birth_cost && sc.birth_cost > 0.0) {
            continue;
        }
        // The parcel leaves the parent as a fraction of its whole material, so its mineral
        // and its energy leave with it by the same fraction rule every other transfer in
        // this world uses.
        let before = a.organic();
        let f = (sc.birth_cost / before).clamp(0.0, 1.0);
        a.reserve -= sc.birth_cost;
        let mineral = (a.mineral * f).min(a.mineral);
        let energy = (a.energy * f).min(a.energy);
        a.mineral -= mineral;
        a.energy -= energy;
        let (site, species) = (a.site, a.species);
        let id = fauna.ledger.births;
        fauna.ledger.births += 1;
        fauna.ledger.born += 1;
        newborns.push(Animal {
            id,
            species,
            site,
            body: sc.body_min,
            reserve: sc.birth_cost - sc.body_min,
            mineral,
            energy,
            age_ticks: 0,
            state: State::Resting,
        });
    }
    for n in newborns {
        fauna.insert(n);
    }
}

/// Step 6: starvation and drowning, and the carrion they leave.
fn deaths(fauna: &mut Fauna, view: &VoxelView<'_>, flora: &mut Flora) {
    let dead: Vec<Animal> = fauna
        .animals
        .iter()
        .filter(|a| {
            let sc = fauna.config.species(a.species);
            a.body < sc.body_min
                || view.water_depth_m(i64::from(a.site.x), a.site.y, a.site.z) > sc.drown_depth_m
        })
        .copied()
        .collect();
    if dead.is_empty() {
        return;
    }
    let ids: Vec<u64> = dead.iter().map(|a| a.id).collect();
    fauna.animals.retain(|a| !ids.contains(&a.id));
    for a in &dead {
        // The whole animal, whatever is left of it: a corpse with no organic matter left
        // is still a corpse with mineral in it, and keeping an inert remainder on a dead
        // animal is how a ledger stops closing.
        fauna.book_deposit(
            flora,
            a.site,
            DepositKind::Carrion,
            Taken { organic: a.organic(), mineral: a.mineral, energy: a.energy },
        );
        fauna.ledger.deaths += 1;
    }
}
