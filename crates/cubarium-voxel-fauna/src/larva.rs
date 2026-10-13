//! **The bellwing larva field** (LARVA, `design/larva-nuts-plan-2026-10-12.md` §3-§5;
//! Wrysk 2026-10-12: "go for both").
//!
//! A lineage with a [`Larva`] table (the bellwing's) lays its clutches into this field, not
//! onto a face: a **density on a host's leaves**, keyed by host — a latticevine
//! ([`LarvaHost::Vine`]) or a crowned vaulttree ([`LarvaHost::Stand`]). Each host holds at
//! most [`Larva::cohorts_max`] cohorts, each a count of larvae of one body with their
//! tissue, mineral and energy, merging its two oldest when a new clutch would make one
//! more (the seed-cohort precedent). Larvae are undrawn ("a for now, art later").
//!
//! Every [`Larva::step_s`] seconds each host's cohorts, in order:
//! 1. **die** at the background rate `death_per_s` (parasites, disease), expected count
//!    `n · exp(−μ dt)`, their tissue to the host's litter;
//! 2. **age**, and past `incubation_s` **grow** exponentially toward the emergence size
//!    over `period_s`, paying the lineage's upkeep, as far as the leaf they take funds it:
//!    tissue built ≤ `foliage_yield` × leaf and ≤ the leaf's mineral / `n_tissue`; the
//!    absorbed rest is respired, the unabsorbed rest and its unused mineral go to litter as
//!    frass. An upkeep the leaf cannot pay comes off the tissue (its mineral to litter), and
//!    larvae unfed for [`Larva::starve_s`] die (`died_starved`);
//! 3. **emerge**: a cohort whose larvae reach the emergence size joins its lineage's
//!    **pool of ready larvae** (count, tissue, mineral, energy; no death, no upkeep), and
//!    each whole larva the pool then holds emerges at that host as one adult body with a
//!    pool larva's share. No fractional adult is lost (LARVA-FIX).
//!
//! A host that no longer exists (its stand gone, its vine gone from the cover) drops its
//! larvae to litter. A host that stands but offers no leaf this step (a dormant or leafless
//! vine, a vaulttree with no edible leaf) keeps them: they pay upkeep from tissue and starve
//! (LARVA-FIX: real larvae on a defoliated plant starve; they do not vanish). Seedporters eat
//! larvae ([`take_larvae`]) and browsers take them by accident with a vaulttree's leaves
//! ([`browse_larvae`]). The field grazes through the plant layer's own take APIs, so
//! organic matter, mineral and energy only move; [`LarvaLedger`] books every flow.

use std::collections::BTreeMap;

use cubarium_voxel::{DT, VoxelView};
use cubarium_voxel_flora::{
    DepositKind, Face as VineFace, FaceLayer, Flora, FloraView, Site, Species as Plant, Stand, Taken, VineId,
};
use serde::{Deserialize, Serialize};

use crate::{Animal, Fauna, Food, Founder};

/// **A larva lineage's table** ([`crate::FounderPhysiology::larva`]). Every number is a
/// knob with a registry row (`fauna.founders.<lineage>.larva.*`).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Larva {
    /// The chance a clutch goes to a crowned vaulttree rather than a latticevine (a keyed
    /// draw per clutch). Wrysk 2026-10-12: "i would split the larvae between vaault tree and
    /// latticevine": 0.5.
    pub vaulttree_share: f64,
    /// The larval period, seconds: hatch to emergence size at full food. 4 h, today's
    /// hatch-to-adult median (245 min), visible in an 8-h check.
    pub period_s: f64,
    /// Background death rate, per second (parasitoids, disease): with predation, survival
    /// egg to adult of a few percent (Zalucki, Clarke & Malcolm 2002). 1.5e-4 (0.55/h).
    pub death_per_s: f64,
    /// Emergence body, × `body_max`: adult insects do not grow. 1.0.
    pub emerge_body_fraction: f64,
    /// An emerging adult's reserve, × its body (a moth's fat body). 1/6, the share today's
    /// hatchling egg carried.
    pub emerge_reserve_fraction: f64,
    /// Tissue built per unit of leaf eaten, at most (the leaf's mineral may bind lower).
    /// Real caterpillar ECI on tree leaves is ~0.1-0.2 (Scriber & Slansky 1981): 0.4,
    /// which the leaf's nitrogen holds to 0.2 on vine (0.01) and 0.4 on vaulttree (0.02).
    pub foliage_yield: f64,
    /// Cohorts a host holds at most; a new clutch merges the two oldest. 3.
    pub cohorts_max: u32,
    /// A host is food in sight when its hatched larvae number at least this. 1.
    pub visible_count: f64,
    /// The field's clock, seconds: deaths, growth and emergence run this often. 10 s, short
    /// against the larval period, so a whole field costs little.
    pub step_s: f64,
    /// A layer lays at a vaulttree when its body is within this of the crown's disc,
    /// metres. 0.25.
    pub lay_reach_m: f64,
    /// Founding: adults the founding cohorts are sized to emerge per hour. ~15/h, today's
    /// hourly maturing in h5-8.
    pub founding_emerge_per_h: f64,
    /// Founding: the this many hosts offering the most leaf now (a vine's leafy faces, a
    /// vaulttree's edible foliage; nearer the founders first among equals) take the founding
    /// cohorts, each in proportion to its leaf. 12.
    pub founding_hosts: u32,
    /// A hatched larva whose leaf does not pay its upkeep for this long dies
    /// (`died_starved`); a step that pays its upkeep in full resets the clock, a step that
    /// pays part runs it by the unpaid share. Early-instar caterpillars die within ~1-3 days
    /// unfed against a 2-4-week larval period (recalled from the rearing literature, not a
    /// measured source), about an eighth of it: 1800 s of the 4-h period.
    pub starve_s: f64,
}

impl Larva {
    /// The bellwing's table (LARVA §3, §8).
    pub const BELLWING: Larva = Larva {
        vaulttree_share: 0.5,
        period_s: 4.0 * 3600.0,
        death_per_s: 1.5e-4,
        emerge_body_fraction: 1.0,
        emerge_reserve_fraction: 1.0 / 6.0,
        foliage_yield: 0.4,
        cohorts_max: 3,
        visible_count: 1.0,
        step_s: 10.0,
        lay_reach_m: 0.25,
        founding_emerge_per_h: 15.0,
        founding_hosts: 12,
        starve_s: 1800.0,
    };
}

/// The plant a host is: the larvae live on its leaves.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum LarvaHost {
    /// A latticevine, by its id.
    Vine(VineId),
    /// A crowned vaulttree, by its stand id.
    Stand(u64),
}

/// The stand species larvae live on (his decision 1: latticevine and vaulttree).
pub const STAND_HOST: Plant = Plant::Vaulttree;

/// One cohort: `count` larvae (a density, not whole bodies) holding `organic` tissue in
/// all, with its `mineral` and `energy`, `age_s` since laid.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Cohort {
    pub count: f64,
    pub organic: f64,
    pub mineral: f64,
    pub energy: f64,
    pub age_s: f64,
    /// Seconds unfed (upkeep unpaid, weighted by the unpaid share); at
    /// [`Larva::starve_s`] the cohort dies.
    #[serde(default)]
    pub unfed_s: f64,
}

impl Cohort {
    /// One larva's tissue.
    pub fn body(&self) -> f64 {
        if self.count > 0.0 { self.organic / self.count } else { 0.0 }
    }

    /// The `share` of the cohort, count and currencies alike, taken out of it.
    fn take_share(&mut self, share: f64) -> (f64, Taken) {
        let s = share.clamp(0.0, 1.0);
        let out = if s >= 1.0 {
            (self.count, Taken { organic: self.organic, mineral: self.mineral, energy: self.energy })
        } else {
            (
                self.count * s,
                Taken { organic: self.organic * s, mineral: self.mineral * s, energy: self.energy * s },
            )
        };
        self.count -= out.0;
        self.organic -= out.1.organic;
        self.mineral -= out.1.mineral;
        self.energy -= out.1.energy;
        if s >= 1.0 {
            *self = Cohort { age_s: self.age_s, unfed_s: self.unfed_s, ..Cohort::default() };
        }
        out
    }
}

/// **A lineage's pool of ready larvae** (LARVA-FIX): emergence remainders, larvae that
/// reached the emergence size but not a whole adult, with what they carry. It takes no
/// death and no upkeep; each whole larva it holds emerges.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ReadyPool {
    pub count: f64,
    pub organic: f64,
    pub mineral: f64,
    pub energy: f64,
}

impl ReadyPool {
    /// The `share` of the pool taken out of it (all of it at 1).
    fn take_share(&mut self, share: f64) -> Taken {
        let s = share.clamp(0.0, 1.0);
        if s >= 1.0 {
            let all = Taken { organic: self.organic, mineral: self.mineral, energy: self.energy };
            *self = ReadyPool::default();
            return all;
        }
        let t = Taken { organic: self.organic * s, mineral: self.mineral * s, energy: self.energy * s };
        self.count -= self.count * s;
        self.organic -= t.organic;
        self.mineral -= t.mineral;
        self.energy -= t.energy;
        t
    }
}

/// One host's larvae.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct HostLarvae {
    pub lineage: Founder,
    /// The litter site: a vaulttree's root face, a vine's root face.
    pub site: Site,
    /// A vine host's face eggs were last laid on (where it is seen from), kept on one of
    /// its leafy faces.
    pub anchor: Option<VineFace>,
    pub cohorts: Vec<Cohort>,
}

impl HostLarvae {
    /// Larvae past incubation (eggs are not seen and not eaten by a seedporter).
    pub fn hatched(&self, incubation_s: f64) -> (f64, f64) {
        self.cohorts
            .iter()
            .filter(|c| c.age_s >= incubation_s)
            .fold((0.0, 0.0), |(n, o), c| (n + c.count, o + c.organic))
    }

    pub fn count(&self) -> f64 {
        self.cohorts.iter().map(|c| c.count).sum()
    }

    pub fn organic(&self) -> f64 {
        self.cohorts.iter().map(|c| c.organic).sum()
    }
}

/// **The larva field**: every host's larvae, in host order.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct LarvaField {
    hosts: BTreeMap<LarvaHost, HostLarvae>,
    /// Each lineage's pool of ready larvae (LARVA-FIX).
    pool: Vec<(Founder, ReadyPool)>,
}

/// An empty field, for a view built without one (a renderer snapshot: larvae are undrawn).
pub static NO_LARVAE: LarvaField = LarvaField::new();

impl LarvaField {
    pub const fn new() -> LarvaField {
        LarvaField { hosts: BTreeMap::new(), pool: Vec::new() }
    }

    /// A lineage's pool of ready larvae (empty when it has none).
    pub fn pool_of(&self, lineage: Founder) -> ReadyPool {
        self.pool.iter().find(|(f, _)| *f == lineage).map(|(_, p)| *p).unwrap_or_default()
    }

    /// Ready larvae pooled, every lineage.
    pub fn pool_count(&self) -> f64 {
        self.pool.iter().map(|(_, p)| p.count).sum()
    }

    fn pool_mut(&mut self, lineage: Founder) -> &mut ReadyPool {
        let k = match self.pool.iter().position(|(f, _)| *f == lineage) {
            Some(k) => k,
            None => {
                self.pool.push((lineage, ReadyPool::default()));
                self.pool.len() - 1
            }
        };
        &mut self.pool[k].1
    }

    pub fn hosts(&self) -> impl Iterator<Item = (&LarvaHost, &HostLarvae)> {
        self.hosts.iter()
    }

    pub fn host(&self, host: LarvaHost) -> Option<&HostLarvae> {
        self.hosts.get(&host)
    }

    /// No host holds larvae and no pool holds ready ones.
    pub fn is_empty(&self) -> bool {
        self.hosts.is_empty() && self.pool.iter().all(|(_, p)| !(p.count > 0.0) && !(p.organic > 0.0))
    }

    /// The field's organic matter: every host's larvae and the ready pools.
    pub fn organic(&self) -> f64 {
        self.hosts.values().map(HostLarvae::organic).sum::<f64>() + self.pool.iter().map(|(_, p)| p.organic).sum::<f64>()
    }

    pub fn mineral(&self) -> f64 {
        self.hosts.values().flat_map(|h| h.cohorts.iter()).map(|c| c.mineral).sum::<f64>()
            + self.pool.iter().map(|(_, p)| p.mineral).sum::<f64>()
    }

    pub fn energy(&self) -> f64 {
        self.hosts.values().flat_map(|h| h.cohorts.iter()).map(|c| c.energy).sum::<f64>()
            + self.pool.iter().map(|(_, p)| p.energy).sum::<f64>()
    }

    /// Larvae standing (eggs included), by host kind: `(vine, vaulttree)`.
    pub fn count_by_kind(&self) -> (f64, f64) {
        self.hosts.iter().fold((0.0, 0.0), |(v, s), (k, h)| match k {
            LarvaHost::Vine(_) => (v + h.count(), s),
            LarvaHost::Stand(_) => (v, s + h.count()),
        })
    }

    /// Larvae standing on hosts of one lineage, eggs included (the ready pool is not).
    pub fn count_of(&self, lineage: Founder) -> f64 {
        self.hosts.values().filter(|h| h.lineage == lineage).map(HostLarvae::count).sum()
    }

    /// Larval tissue standing on hosts of one lineage (the ready pool is not).
    pub fn organic_of(&self, lineage: Founder) -> f64 {
        self.hosts.values().filter(|h| h.lineage == lineage).map(HostLarvae::organic).sum()
    }
}

/// Three currencies, cumulative.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Books {
    pub organic: f64,
    pub mineral: f64,
    pub energy: f64,
}

impl Books {
    pub fn add(&mut self, t: Taken) {
        self.organic += t.organic;
        self.mineral += t.mineral;
        self.energy += t.energy;
    }
}

/// **The larva field's books** ([`crate::FaunaLedger::larva`]), cumulative. Over any
/// interval, for each currency: field change = `eggs_in + founded_in + leaf_in −
/// emerged_out − eaten_seedporter_out − eaten_browser_out − litter_out − respired`
/// (organic: `respired_out`; energy: `heat_out`; mineral: none respired).
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct LarvaLedger {
    pub eggs_laid: u64,
    pub eggs_in: Books,
    /// Founding cohorts' larvae, and what they were introduced with.
    pub founded: f64,
    pub founded_in: Books,
    /// Leaf eaten by larvae, in all and by host kind (organic).
    pub leaf_in: Books,
    pub leaf_vine: f64,
    pub leaf_vaulttree: f64,
    pub emerged: u64,
    pub emerged_out: Books,
    /// Larvae eaten (count) and what they carried.
    pub eaten_seedporter: f64,
    pub eaten_seedporter_out: Books,
    pub eaten_browser: f64,
    pub eaten_browser_out: Books,
    /// Larvae dead (count): background, their host gone, starved (an emergence remainder
    /// is pooled, not killed: LARVA-FIX).
    pub died_background: f64,
    pub died_host: f64,
    pub died_starved: f64,
    /// Everything that went to litter: deaths, frass, an egg's unfunded share.
    pub litter_out: Books,
    pub respired_out: f64,
    pub heat_out: f64,
}

/// The lineage's larva table, if it has one.
pub(crate) fn table(fauna: &Fauna, lineage: Founder) -> Option<Larva> {
    fauna.config.founder(lineage).larva
}

/// The size at which a lineage's larvae emerge: its emergence body and reserve, per larva.
pub(crate) fn emerge_size(fauna: &Fauna, lineage: Founder) -> Option<(f64, f64)> {
    let t = table(fauna, lineage)?;
    let sc = fauna.config.founder(lineage).core;
    let body = t.emerge_body_fraction.max(0.0) * sc.body_max;
    Some((body, body * (1.0 + t.emerge_reserve_fraction.max(0.0))))
}

/// The exponential growth rate toward emergence, per second: an egg's tissue to the
/// emergence size over `period_s`.
fn growth_rate(t: &Larva, egg: f64, target: f64) -> f64 {
    if egg > 0.0 && target > egg && t.period_s > 0.0 { (target / egg).ln() / t.period_s } else { 0.0 }
}

/// **A stand is a larva host** when it is a crowned vaulttree ([`STAND_HOST`],
/// `profile_stage >= 1`) holding edible foliage.
pub fn stand_hosts(fv: &FloraView<'_>, s: &Stand) -> bool {
    s.species == STAND_HOST && s.profile_stage >= 1 && fv.edible_foliage(s) > 0.0
}

/// **May this plant hold larvae an eater cannot see from where it is?** LARVA's part of the
/// seedporter's tree check ([`crate::treecheck::may_hold_hidden_food`] takes it in): a
/// crowned vaulttree with leaves ([`stand_hosts`]), or a latticevine that is not dormant, as
/// found at one of its leafy faces.
pub(crate) fn may_hold_hidden_larvae(fv: &FloraView<'_>, host: crate::treecheck::Host<'_>) -> bool {
    match host {
        crate::treecheck::Host::Vine(v, face) => !v.dormant && face.owner == v.id && face.leafiness > 0.0,
        crate::treecheck::Host::Stand(s) => stand_hosts(fv, s),
    }
}

/// The stand of host id `id` on `site`, if it still stands there.
fn host_stand<'a>(fv: &FloraView<'a>, site: Site, id: u64) -> Option<&'a Stand> {
    fv.stand_in(site, FaceLayer::Plant).filter(|s| s.id == id)
}

/// A crown's lowest foliage bottom and its top, metres above the world floor.
pub(crate) fn crown_band(fv: &FloraView<'_>, s: &Stand) -> Option<(f64, f64)> {
    let mut low = f64::INFINITY;
    let mut high = f64::NEG_INFINITY;
    for l in fv.layers(s) {
        if l.stock > 0.0 {
            low = low.min(l.band_m[0]);
            high = high.max(l.band_m[1]);
        }
    }
    (low.is_finite() && high.is_finite()).then_some((low, high))
}

/// **Into the field**: a clutch of `eggs` paid `paid`, laid on `host`. The egg's tissue is
/// what its mineral funds at the lineage's `n_tissue` (normally all of it); an unfunded
/// share and surplus mineral go to the host's litter.
#[allow(clippy::too_many_arguments)]
pub(crate) fn lay_eggs(
    fauna: &mut Fauna,
    flora: &mut Flora,
    lineage: Founder,
    host: LarvaHost,
    site: Site,
    anchor: Option<VineFace>,
    eggs: u32,
    paid: Taken,
) {
    let Some(t) = table(fauna, lineage) else { return };
    let n = fauna.config.founder(lineage).core.n_tissue;
    fauna.ledger.larva.eggs_laid += u64::from(eggs);
    fauna.ledger.larva.eggs_in.add(paid);
    let tissue = if n > 0.0 { paid.organic.min(paid.mineral / n) } else { paid.organic }.max(0.0);
    let energy = if paid.organic > 0.0 { paid.energy * tissue / paid.organic } else { 0.0 };
    let cohort = Cohort { count: f64::from(eggs), organic: tissue, mineral: n * tissue, energy, age_s: 0.0, unfed_s: 0.0 };
    let excess = Taken {
        organic: (paid.organic - cohort.organic).max(0.0),
        mineral: (paid.mineral - cohort.mineral).max(0.0),
        energy: (paid.energy - cohort.energy).max(0.0),
    };
    insert_cohort(fauna, &t, lineage, host, site, anchor, cohort);
    to_litter(fauna, flora, site, excess);
}

fn insert_cohort(
    fauna: &mut Fauna,
    t: &Larva,
    lineage: Founder,
    host: LarvaHost,
    site: Site,
    anchor: Option<VineFace>,
    cohort: Cohort,
) {
    let h = fauna
        .larvae
        .hosts
        .entry(host)
        .or_insert_with(|| HostLarvae { lineage, site, anchor, cohorts: Vec::new() });
    if anchor.is_some() {
        h.anchor = anchor;
    }
    h.cohorts.push(cohort);
    // Oldest first; merge the two oldest while there are too many.
    h.cohorts.sort_by(|a, b| b.age_s.total_cmp(&a.age_s));
    let most = t.cohorts_max.max(1) as usize;
    while h.cohorts.len() > most {
        let b = h.cohorts.remove(1);
        let a = &mut h.cohorts[0];
        let n = a.count + b.count;
        a.age_s = if n > 0.0 { (a.age_s * a.count + b.age_s * b.count) / n } else { a.age_s.max(b.age_s) };
        a.unfed_s = if n > 0.0 { (a.unfed_s * a.count + b.unfed_s * b.count) / n } else { a.unfed_s.max(b.unfed_s) };
        a.count = n;
        a.organic += b.organic;
        a.mineral += b.mineral;
        a.energy += b.energy;
    }
}

/// Larval matter to litter at `site`, booked in the field's books and on the boundary.
fn to_litter(fauna: &mut Fauna, flora: &mut Flora, site: Site, t: Taken) {
    if !(t.organic > 0.0 || t.mineral > 0.0 || t.energy > 0.0) {
        return;
    }
    fauna.ledger.larva.litter_out.add(t);
    fauna.book_deposit(flora, site, DepositKind::Litter, t);
}

/// Organic `o` and its energy `e` respired, booked as upkeep (`maintenance`) or digestion.
fn respire(fauna: &mut Fauna, o: f64, e: f64, maintenance: bool) {
    if !(o > 0.0) && !(e > 0.0) {
        return;
    }
    let (o, e) = (o.max(0.0), e.max(0.0));
    let l = &mut fauna.ledger;
    l.respired_out += o;
    if maintenance {
        l.respired_maintenance_out += o;
    } else {
        l.respired_digestion_out += o;
    }
    l.heat_out += e;
    l.larva.respired_out += o;
    l.larva.heat_out += e;
}

/// What the host offers this step: its leafy vine faces with their leaf, or the stand.
enum Offer {
    Vine(Vec<(VineFace, f64)>),
    Stand(Site),
}

/// **The field's tick** (inside [`crate::step`]'s reproduction, births on): on each larva
/// lineage's clock, every host's deaths, growth and emergence. Emerged adults go into
/// `newborns`.
pub(crate) fn step_field(fauna: &mut Fauna, view: &VoxelView<'_>, flora: &mut Flora, newborns: &mut Vec<Animal>) {
    if fauna.larvae.hosts.is_empty() {
        return;
    }
    let keys: Vec<LarvaHost> = fauna.larvae.hosts.keys().copied().collect();
    // Each vine host's leafy faces, in one pass over the cover (only when one is due).
    let mut vine_faces: BTreeMap<VineId, Vec<(VineFace, f64)>> = BTreeMap::new();
    let due = |fauna: &Fauna, lineage: Founder| {
        table(fauna, lineage).is_some_and(|t| fauna.tick % cubarium_voxel::ticks_at_least_one(t.step_s.max(DT)) == 0)
    };
    let any_vine_due = keys.iter().any(|k| {
        matches!(k, LarvaHost::Vine(_)) && fauna.larvae.hosts.get(k).is_some_and(|h| due(fauna, h.lineage))
    });
    if any_vine_due {
        let leaf_mass = flora.config().latticevine.leaf_mass;
        let fv = flora.view();
        for k in &keys {
            if let LarvaHost::Vine(id) = k {
                vine_faces.entry(*id).or_default();
            }
        }
        for f in fv.cover.iter() {
            if f.leafiness > 0.0
                && let Some(list) = vine_faces.get_mut(&f.owner)
            {
                list.push((f.face, f.leafiness * leaf_mass));
            }
        }
        for list in vine_faces.values_mut() {
            list.sort_by(|a, b| a.0.cmp(&b.0));
        }
    }
    for key in keys {
        let Some(lineage) = fauna.larvae.hosts.get(&key).map(|h| h.lineage) else { continue };
        if !due(fauna, lineage) {
            continue;
        }
        step_host(fauna, view, flora, key, &vine_faces, newborns);
    }
}

fn step_host(
    fauna: &mut Fauna,
    view: &VoxelView<'_>,
    flora: &mut Flora,
    key: LarvaHost,
    vine_faces: &BTreeMap<VineId, Vec<(VineFace, f64)>>,
    newborns: &mut Vec<Animal>,
) {
    let Some(mut host) = fauna.larvae.hosts.remove(&key) else { return };
    let lineage = host.lineage;
    let Some(t) = table(fauna, lineage) else {
        // A lineage that lost its table drops its larvae.
        drop_host(fauna, flora, host);
        return;
    };
    let phys = *fauna.config.founder(lineage);
    let sc = phys.core;
    let dt = cubarium_voxel::ticks_at_least_one(t.step_s.max(DT)) as f64 * DT;
    // The host, and what it offers. A host that no longer exists drops its larvae; one that
    // stands but offers no leaf this step (a dormant or leafless vine, a vaulttree with no
    // edible leaf) keeps them, unfed (LARVA-FIX).
    let (exists, offer, mf) = {
        let fv = flora.view();
        match key {
            LarvaHost::Stand(id) => match host_stand(&fv, host.site, id) {
                Some(s) if stand_hosts(&fv, s) => {
                    let mat = s.material();
                    (true, Some(Offer::Stand(s.site)), if mat > 0.0 { s.mineral / mat } else { 0.0 })
                }
                Some(_) => (true, None, 0.0),
                None => (false, None, 0.0),
            },
            LarvaHost::Vine(id) => match fv.cover.vine(id) {
                None => (false, None, 0.0),
                Some(v) => {
                    host.site = v.root;
                    let faces = vine_faces.get(&id).cloned().unwrap_or_default();
                    if !v.dormant && !faces.is_empty() {
                        if host.anchor.is_none_or(|a| !faces.iter().any(|(f, _)| *f == a)) {
                            host.anchor =
                                faces.iter().max_by(|a, b| a.1.total_cmp(&b.1).then(b.0.cmp(&a.0))).map(|(f, _)| *f);
                        }
                        (true, Some(Offer::Vine(faces)), flora.config().latticevine.mineral_fraction)
                    } else {
                        (true, None, 0.0)
                    }
                }
            },
        }
    };
    if !exists {
        drop_host(fauna, flora, host);
        return;
    }
    let site = host.site;
    let mut litter = Taken::default();
    let add = |l: &mut Taken, t: Taken| {
        l.organic += t.organic;
        l.mineral += t.mineral;
        l.energy += t.energy;
    };
    // 1. Background deaths.
    let p = 1.0 - (-t.death_per_s.max(0.0) * dt).exp();
    for c in &mut host.cohorts {
        if p > 0.0 {
            let (n, dead) = c.take_share(p);
            fauna.ledger.larva.died_background += n;
            add(&mut litter, dead);
        }
        c.age_s += dt;
    }
    // 2. Growth: each hatched cohort's want, then one take of leaf for the host.
    let inc = sc.reproduction.incubation_s.max(0.0);
    let egg = sc.reproduction.egg_organic;
    let (emerge_body, target) = emerge_size(fauna, lineage).unwrap_or((sc.body_max, sc.body_max));
    let r = growth_rate(&t, egg, target);
    let n = sc.n_tissue;
    let y = t.foliage_yield.clamp(0.0, 1.0);
    // (grow, upkeep) per cohort.
    let wants: Vec<(f64, f64)> = host
        .cohorts
        .iter()
        .map(|c| {
            let grown = (c.age_s - inc).clamp(0.0, dt);
            if !(c.count > 0.0) || !(grown > 0.0) {
                return (0.0, 0.0);
            }
            let room = (c.count * target - c.organic).max(0.0);
            let g = (c.organic * ((r * grown).exp() - 1.0)).min(room).max(0.0);
            let u = c.organic * sc.maintenance_per_s.max(0.0) * grown;
            (g, u)
        })
        .collect();
    let leaf_for = |g: f64, u: f64| {
        let by_yield = if y > 0.0 { (g + u) / y } else { 0.0 };
        let by_mineral = if mf > 0.0 && n > 0.0 { g * n / mf } else if g > 0.0 { f64::INFINITY } else { 0.0 };
        by_yield.max(by_mineral)
    };
    let demand: Vec<f64> = wants.iter().map(|&(g, u)| leaf_for(g, u)).collect();
    let total: f64 = demand.iter().filter(|d| d.is_finite()).sum();
    let leaf = match &offer {
        Some(o) if total > 0.0 => take_leaf(flora, o, total),
        _ => Taken::default(),
    };
    if leaf.organic > 0.0 || leaf.mineral > 0.0 {
        let l = &mut fauna.ledger;
        l.eaten_organic_in += leaf.organic;
        l.eaten_mineral_in += leaf.mineral;
        l.eaten_energy_in += leaf.energy;
        l.eaten_by_food[Food::Foliage.index()] += leaf.organic;
        l.larva.leaf_in.add(leaf);
        match key {
            LarvaHost::Vine(_) => l.larva.leaf_vine += leaf.organic,
            LarvaHost::Stand(_) => l.larva.leaf_vaulttree += leaf.organic,
        }
    }
    for (k, c) in host.cohorts.iter_mut().enumerate() {
        let (g, u) = wants[k];
        if !(g > 0.0) && !(u > 0.0) {
            continue;
        }
        let share = if total > 0.0 && demand[k].is_finite() { demand[k] / total } else { 0.0 };
        let (o, m, e) = (leaf.organic * share, leaf.mineral * share, leaf.energy * share);
        let absorbed = y * o;
        let paid = u.min(absorbed);
        let built = g.min(absorbed - paid).min(if n > 0.0 { m / n } else { f64::INFINITY }).max(0.0);
        let digest = (absorbed - paid - built).max(0.0);
        let per = |x: f64| if o > 0.0 { e * x / o } else { 0.0 };
        let (e_built, e_paid, e_digest) = (per(built), per(paid), per(digest));
        c.organic += built;
        c.mineral += n * built;
        c.energy += e_built;
        respire(fauna, paid, e_paid, true);
        respire(fauna, digest, e_digest, false);
        add(
            &mut litter,
            Taken {
                organic: (o - absorbed).max(0.0),
                mineral: (m - n * built).max(0.0),
                energy: (e - e_built - e_paid - e_digest).max(0.0),
            },
        );
        // Upkeep the leaf did not pay comes off the tissue; its mineral is excreted.
        let short = (u - paid).max(0.0);
        if short > 0.0 {
            if short >= c.organic {
                let (dead, all) = c.take_share(1.0);
                fauna.ledger.larva.died_starved += dead;
                respire(fauna, all.organic, all.energy, true);
                add(&mut litter, Taken { organic: 0.0, mineral: all.mineral, energy: 0.0 });
            } else {
                let f = short / c.organic;
                let e_s = c.energy * f;
                let m_s = c.mineral * f;
                c.organic -= short;
                c.energy -= e_s;
                c.mineral -= m_s;
                respire(fauna, short, e_s, true);
                add(&mut litter, Taken { organic: 0.0, mineral: m_s, energy: 0.0 });
            }
        }
        // The starvation clock: run by the unpaid share of the upkeep, reset by a step
        // that pays it in full; at `starve_s` the cohort dies, its tissue to litter.
        if c.count > 0.0 && u > 0.0 {
            if short > 0.0 {
                c.unfed_s += dt * (short / u).min(1.0);
            } else {
                c.unfed_s = 0.0;
            }
            if c.unfed_s >= t.starve_s.max(0.0) - 1e-9 && short > 0.0 {
                let (dead, all) = c.take_share(1.0);
                fauna.ledger.larva.died_starved += dead;
                add(&mut litter, all);
            }
        }
    }
    // 3. Emergence: a ready cohort joins its lineage's pool of ready larvae (LARVA-FIX), and
    // each whole larva the pool then holds emerges here, at the host that completed it, with
    // a pool larva's share of tissue, mineral and energy.
    let mut k = 0;
    while k < host.cohorts.len() {
        let c = host.cohorts[k];
        let ready = c.count > 0.0 && c.age_s >= inc && c.body() >= target * (1.0 - 1e-9);
        if !ready {
            k += 1;
            continue;
        }
        host.cohorts.remove(k);
        {
            let pool = fauna.larvae.pool_mut(lineage);
            pool.count += c.count;
            pool.organic += c.organic;
            pool.mineral += c.mineral;
            pool.energy += c.energy;
        }
        loop {
            let pool = fauna.larvae.pool_mut(lineage);
            if pool.count < 1.0 - 1e-9 {
                break;
            }
            let share = if pool.count > 1.0 { 1.0 / pool.count } else { 1.0 };
            let parcel = pool.take_share(share);
            let adult = emerge(fauna, view, &flora.view(), lineage, key, &host, emerge_body.min(parcel.organic), parcel);
            fauna.ledger.larva.emerged += 1;
            fauna.ledger.larva.emerged_out.add(parcel);
            newborns.push(adult);
        }
    }
    // Spent cohorts leave their dust to litter.
    host.cohorts.retain(|c| {
        let keep = c.count > 1e-9 && c.organic > 0.0;
        if !keep {
            add(&mut litter, Taken { organic: c.organic.max(0.0), mineral: c.mineral.max(0.0), energy: c.energy.max(0.0) });
        }
        keep
    });
    to_litter(fauna, flora, site, litter);
    if !host.cohorts.is_empty() {
        fauna.larvae.hosts.insert(key, host);
    }
}

/// A host's whole field to litter (its plant gone).
fn drop_host(fauna: &mut Fauna, flora: &mut Flora, host: HostLarvae) {
    let mut all = Taken::default();
    for c in &host.cohorts {
        fauna.ledger.larva.died_host += c.count;
        all.organic += c.organic;
        all.mineral += c.mineral;
        all.energy += c.energy;
    }
    to_litter(fauna, flora, host.site, all);
}

/// Up to `want` of leaf off the host: a vaulttree's crown (lowest layer first, no browse
/// defence raised: [`Flora::take_foliage_by_insects`]), or a vine's leafy faces in
/// proportion to their leaf ([`Flora::crop`]).
fn take_leaf(flora: &mut Flora, offer: &Offer, want: f64) -> Taken {
    match offer {
        Offer::Stand(site) => flora.take_foliage_by_insects(*site, want).map(|t| t.taken).unwrap_or_default(),
        Offer::Vine(faces) => {
            let sum: f64 = faces.iter().map(|(_, l)| *l).sum();
            let mut got = Taken::default();
            if !(sum > 0.0) {
                return got;
            }
            for (face, l) in faces {
                if let Some(t) = flora.crop(*face, want * l / sum) {
                    got.organic += t.organic;
                    got.mineral += t.mineral;
                    got.energy += t.energy;
                }
            }
            got
        }
    }
}

/// One emerging adult: on its vine where it clings ([`crate::flight::vine_cling`]),
/// flying in the crown of its vaulttree, else perched on the face below.
#[allow(clippy::too_many_arguments)]
fn emerge(
    fauna: &mut Fauna,
    view: &VoxelView<'_>,
    fv: &FloraView<'_>,
    lineage: Founder,
    key: LarvaHost,
    host: &HostLarvae,
    body: f64,
    parcel: Taken,
) -> Animal {
    let phys = *fauna.config.founder(lineage);
    let v = view.config.voxel_m;
    let lifespan = phys.core.lifespan_median_s;
    let mut site = host.site;
    let mut pose = crate::Pose::at_site(site, v);
    let mut cling = None;
    let mut aloft = false;
    match key {
        LarvaHost::Vine(_) => {
            if let (Some(face), Some(flight)) = (host.anchor, phys.flight)
                && let Some((p, under)) = crate::flight::vine_cling(view, &phys, &flight, body, face)
            {
                site = under;
                pose = p;
                cling = Some(face);
            }
        }
        LarvaHost::Stand(id) => {
            if let (Some(s), Some(flight)) = (host_stand(fv, host.site, id), phys.flight)
                && let Some((low, _)) = crown_band(fv, s)
            {
                let (x, z) = ((f64::from(s.site.x) + 0.5) * v, (f64::from(s.site.z) + 0.5) * v);
                let ground = crate::surface_m(s.site.y, v);
                let h = phys.body_at(body).height_m;
                let y = low.min(ground + flight.band_max_m - h).max(ground);
                pose = crate::Pose { x, z, y_m: y, heading_rad: 0.0 };
                site = s.site;
                aloft = y > ground + crate::flight::EPS;
            }
        }
    }
    let mut a = crate::step::newborn(fauna, (crate::Species::Frondgrazer, Some(lineage)), (site, pose), body, parcel, lifespan);
    if let Some(face) = cling {
        a.mobility.mode = crate::MobilityMode::Perched;
        a.mobility.perch = Some(crate::Perch { kind: crate::PerchKind::VineFace, anchor: crate::PerchAnchor::Vine(face) });
    } else if aloft {
        a.mobility.mode = crate::MobilityMode::Fly;
    }
    a
}

/// **A seedporter's bite of larvae** (LARVA §4): up to `want` organic of `host`'s hatched
/// larvae, oldest cohort first (the youngest last), each larva whole in count terms: the
/// count falls by what was taken over the cohort's body. Booked in the field's books; the
/// caller swallows it as [`Food::Larva`].
pub(crate) fn take_larvae(fauna: &mut Fauna, host: LarvaHost, want: f64) -> Option<(Taken, f64)> {
    if !(want > 0.0) {
        return None;
    }
    let h = fauna.larvae.hosts.get_mut(&host)?;
    let inc = fauna.config.founder(h.lineage).core.reproduction.incubation_s;
    h.cohorts.sort_by(|a, b| b.age_s.total_cmp(&a.age_s));
    let mut got = Taken::default();
    let mut count = 0.0;
    let mut left = want;
    for c in h.cohorts.iter_mut().filter(|c| c.age_s >= inc && c.organic > 0.0) {
        if !(left > 0.0) {
            break;
        }
        let share = (left / c.organic).min(1.0);
        let (n, t) = c.take_share(share);
        count += n;
        got.organic += t.organic;
        got.mineral += t.mineral;
        got.energy += t.energy;
        left -= t.organic;
    }
    h.cohorts.retain(|c| c.count > 0.0 && c.organic > 0.0);
    if h.cohorts.is_empty() {
        fauna.larvae.hosts.remove(&host);
    }
    if !(got.organic > 0.0) {
        return None;
    }
    let l = &mut fauna.ledger.larva;
    l.eaten_seedporter += count;
    l.eaten_seedporter_out.add(got);
    Some((got, count))
}

/// **A browser's accident** (LARVA §4, decision 4 "b+c"): a bite that took `fraction` of a
/// vaulttree host's edible foliage takes that fraction of every cohort on it, eggs too.
/// What they carried, for the caller to add to the bite; `None` for no larvae.
pub(crate) fn browse_larvae(fauna: &mut Fauna, stand_id: u64, fraction: f64) -> Option<Taken> {
    let f = fraction.clamp(0.0, 1.0);
    if !(f > 0.0) {
        return None;
    }
    let key = LarvaHost::Stand(stand_id);
    let h = fauna.larvae.hosts.get_mut(&key)?;
    let mut got = Taken::default();
    let mut count = 0.0;
    for c in &mut h.cohorts {
        let (n, t) = c.take_share(f);
        count += n;
        got.organic += t.organic;
        got.mineral += t.mineral;
        got.energy += t.energy;
    }
    h.cohorts.retain(|c| c.count > 1e-12 && c.organic > 0.0);
    if h.cohorts.is_empty() {
        fauna.larvae.hosts.remove(&key);
    }
    let l = &mut fauna.ledger.larva;
    l.eaten_browser += count;
    l.eaten_browser_out.add(got);
    (got.organic > 0.0).then_some(got)
}

/// Visible larvae on `host` for an eater: `(hatched count, their tissue)`, `None` under the
/// lineage's `visible_count`.
pub(crate) fn visible(fauna: &Fauna, host: LarvaHost) -> Option<(f64, f64)> {
    let h = fauna.larvae.hosts.get(&host)?;
    let t = table(fauna, h.lineage)?;
    let inc = fauna.config.founder(h.lineage).core.reproduction.incubation_s;
    let (n, o) = h.hatched(inc);
    (n >= t.visible_count && o > 0.0).then_some((n, o))
}

/// **A clutch's host kind** (LARVA §3): a keyed draw per clutch, stable while the body
/// carries it (keyed on what it has given so far). `true`: a vaulttree first.
pub(crate) fn draws_vaulttree(fauna: &Fauna, i: usize, seed: u64, share: f64) -> bool {
    const DOMAIN_LARVA_HOST: u64 = 0x1A2_7A05;
    let a = &fauna.animals[i];
    if share >= 1.0 {
        return true;
    }
    if share <= 0.0 {
        return false;
    }
    let mut rng = crate::step::Rng::keyed(DOMAIN_LARVA_HOST, seed, a.id, a.reproduction.given.to_bits());
    rng.unit() < share
}

/// **Founding cohorts** (LARVA §3; LARVA-FIX): the [`Larva::founding_hosts`] hosts that
/// offer the most leaf now (a non-dormant vine's leafy faces, a crowned vaulttree's edible
/// foliage; nearer the lineage's living bodies first among equals) each take
/// [`Larva::cohorts_max`] cohorts, sized in proportion to the host's leaf (a seedling vine
/// cannot feed what a crown can), at ages staggered over the time to emergence and sized at
/// the background death rate so ~`founding_emerge_per_h` adults emerge each hour. Each host's
/// cohorts sit at the midpoint of its share of the leaf, so the emergence stream is even.
/// Introduced matter, booked `introduced_*`. How many larvae.
pub(crate) fn found(fauna: &mut Fauna, view: &VoxelView<'_>, flora: &Flora, lineage: Founder) -> f64 {
    let Some(t) = table(fauna, lineage) else { return 0.0 };
    let sc = fauna.config.founder(lineage).core;
    let Some((_, target)) = emerge_size(fauna, lineage) else { return 0.0 };
    let egg = sc.reproduction.egg_organic;
    let inc = sc.reproduction.incubation_s.max(0.0);
    let r = growth_rate(&t, egg, target);
    if !(r > 0.0) || !(t.founding_emerge_per_h > 0.0) || t.founding_hosts == 0 {
        return 0.0;
    }
    let v = view.config.voxel_m;
    let ring = f64::from(view.config.width) * v;
    let bodies: Vec<(f64, f64)> =
        fauna.animals.iter().filter(|a| a.founder == Some(lineage)).map(|a| (a.pose.x, a.pose.z)).collect();
    if bodies.is_empty() {
        return 0.0;
    }
    let near = |x: f64, z: f64| {
        bodies
            .iter()
            .map(|&(bx, bz)| {
                let (dx, dz) = crate::pose::wrapped_delta(bx, bz, x, z, ring);
                dx.hypot(dz)
            })
            .fold(f64::INFINITY, f64::min)
    };
    let fv = flora.view();
    let leaf_mass = flora.config().latticevine.leaf_mass;
    // (leaf offered, distance, host, site, anchor)
    let mut cands: Vec<(f64, f64, LarvaHost, Site, Option<VineFace>)> = Vec::new();
    for s in fv.stands.iter().filter(|s| stand_hosts(&fv, s)) {
        let d = near((f64::from(s.site.x) + 0.5) * v, (f64::from(s.site.z) + 0.5) * v);
        cands.push((fv.edible_foliage(s), d, LarvaHost::Stand(s.id), s.site, None));
    }
    // Per vine: its leaf, and its nearest leafy face.
    let mut vines: BTreeMap<VineId, (f64, f64, VineFace)> = BTreeMap::new();
    for f in fv.cover.iter().filter(|f| f.leafiness > 0.0) {
        if fv.cover.vine(f.owner).is_none_or(|vn| vn.dormant) {
            continue;
        }
        let d = near((f64::from(f.face.x) + 0.5) * v, (f64::from(f.face.z) + 0.5) * v);
        let e = vines.entry(f.owner).or_insert((0.0, d, f.face));
        e.0 += f.leafiness * leaf_mass;
        if d < e.1 || (d == e.1 && f.face < e.2) {
            e.1 = d;
            e.2 = f.face;
        }
    }
    for (id, (leaf, d, face)) in vines {
        if let Some(vn) = fv.cover.vine(id) {
            cands.push((leaf, d, LarvaHost::Vine(id), vn.root, Some(face)));
        }
    }
    cands.retain(|c| c.0 > 0.0 && c.0.is_finite());
    cands.sort_by(|a, b| b.0.total_cmp(&a.0).then(a.1.total_cmp(&b.1)).then(a.2.cmp(&b.2)));
    cands.truncate(t.founding_hosts as usize);
    let leaf_total: f64 = cands.iter().map(|c| c.0).sum();
    if cands.is_empty() || !(leaf_total > 0.0) {
        return 0.0;
    }
    let k = t.cohorts_max.max(1) as usize;
    let to_emerge = inc + t.period_s;
    // Survivors per round of cohorts (one per host), every host together.
    let per_round = t.founding_emerge_per_h * (to_emerge / 3600.0) / k as f64;
    let mut larvae = 0.0;
    let mut added = Taken::default();
    let mut before = 0.0;
    for (leaf, _, host, site, anchor) in cands {
        let w = leaf / leaf_total;
        // The host's phase: the midpoint of its share of the leaf.
        let phase = before + 0.5 * w;
        before += w;
        for c in 0..k {
            let age = to_emerge * (c as f64 + phase) / k as f64;
            let count = per_round * w * (t.death_per_s.max(0.0) * (to_emerge - age)).exp();
            let body = if age <= inc { egg } else { (egg * (r * (age - inc)).exp()).min(target) };
            let organic = count * body;
            let cohort = Cohort {
                count,
                organic,
                mineral: sc.n_tissue * organic,
                energy: sc.energy_density * organic,
                age_s: age,
                unfed_s: 0.0,
            };
            added.organic += cohort.organic;
            added.mineral += cohort.mineral;
            added.energy += cohort.energy;
            larvae += count;
            insert_cohort(fauna, &t, lineage, host, site, anchor, cohort);
        }
    }
    let l = &mut fauna.ledger;
    l.introduced_organic_in += added.organic;
    l.introduced_mineral_in += added.mineral;
    l.introduced_energy_in += added.energy;
    l.larva.founded += larvae;
    l.larva.founded_in.add(added);
    larvae
}

/// Test and edit access: put a cohort on a host as an introduction (booked
/// `introduced_*` and the field's `founded_in`).
pub(crate) fn introduce(fauna: &mut Fauna, lineage: Founder, host: LarvaHost, site: Site, anchor: Option<VineFace>, cohort: Cohort) {
    let Some(t) = table(fauna, lineage) else { return };
    let l = &mut fauna.ledger;
    l.introduced_organic_in += cohort.organic;
    l.introduced_mineral_in += cohort.mineral;
    l.introduced_energy_in += cohort.energy;
    l.larva.founded += cohort.count;
    l.larva.founded_in.add(Taken { organic: cohort.organic, mineral: cohort.mineral, energy: cohort.energy });
    insert_cohort(fauna, &t, lineage, host, site, anchor, cohort);
}
