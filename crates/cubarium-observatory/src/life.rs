//! **Life histories**, recorded on the observer's side.
//!
//! The animal layer keeps no per-animal event log, and this module adds none to it: a
//! [`LifeRecorder`] compares the layer with what it saw on the previous tick and writes
//! the difference as [`LifeEvent`]s — a new id is a birth, a vanished one a departure
//! (its cause read off the ledger's per-lineage departure counters), a bout of the eating
//! latch closing is a meal (its foods the stomach's growth during the bout), a strike
//! beginning is a hunt, a wound rising an injury, a gestation escrow emptying a birth, a
//! new clutch a laying, a pack's den face changing a den move.
//!
//! What the layer does not record has to be **inferred**, and is said where it is:
//! - a newborn's parent is the same-lineage body whose gestation ended this tick
//!   (nearest, if several did), or the body that laid the clutch it hatched from;
//! - a clutch's layer is the same-lineage body whose passed-on matter rose this tick
//!   without a birth (nearest to the clutch);
//! - a departure's cause is matched against the ledger's counter rises for its lineage —
//!   exact when one body of a lineage leaves on a tick (the common case), a best fit by
//!   the body's last state (drowning, drying, wounds, age) when several do;
//! - a killer is the body of the killing lineage that was striking the victim.
//!
//! A [`LifeBook`] keeps the events by animal, bounded, with parents and offspring, and
//! keeps the dead for a while so a history can still be opened after its end.

use std::collections::{BTreeMap, VecDeque};

use cubarium_voxel::DT;
use cubarium_voxel_fauna::{BoutKind, Departure, Fauna, Food, Founder};
use cubarium_voxel_flora::Site;
use serde::{Deserialize, Serialize};

use crate::query::{FoodAmount, LifeStage, lineage_label, stage_of};

/// One thing that happened to one animal.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LifeEvent {
    pub tick: u64,
    pub animal: u64,
    #[serde(flatten)]
    pub what: What,
}

/// What happened ([`LifeEvent`]).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum What {
    /// Alive when the record began: its history before then is not known.
    Observed { species: String, stage: LifeStage, age_s: f64 },
    /// Born in the world (`live_birth`, `hatched`) or put there (`introduced`).
    Born { species: String, how: String, parent: Option<u64> },
    /// Reached its lineage's adult (breeding) size.
    Matured,
    /// A pup came of age and left the den's hold.
    Independent,
    /// A rider let go of its host.
    LetGo { host: u64 },
    /// A feeding bout closed: its kind (`normal`, `growth`, `provisioning`, `emergency`),
    /// what went into the stomach during it, and how long it ran.
    Meal { bout: String, foods: Vec<FoodAmount>, duration_s: f64 },
    /// A strike began on `prey`.
    Hunt { prey: u64 },
    /// `prey` died of this body's strike.
    Kill { prey: u64, prey_species: String },
    /// The wound stock rose to `wound`; `by` is the body that was striking it, if any.
    Injured { wound: f64, by: Option<u64> },
    /// Live young born.
    GaveBirth { young: Vec<u64> },
    /// A clutch laid on a face.
    LaidEggs { eggs: u32, site: [u32; 3] },
    /// Its pack's den moved.
    DenMoved { from: [u32; 3], to: [u32; 3] },
    /// Left the world: a death cause (`starved`, `drowned`, `aged`, `killed_by_*`,
    /// `desiccated`, `fell`), `removed`, or `unknown` when no counter explained it.
    Died { cause: String, by: Option<u64> },
}

impl What {
    pub fn name(&self) -> &'static str {
        match self {
            What::Observed { .. } => "observed",
            What::Born { .. } => "born",
            What::Matured => "matured",
            What::Independent => "independent",
            What::LetGo { .. } => "let_go",
            What::Meal { .. } => "meal",
            What::Hunt { .. } => "hunt",
            What::Kill { .. } => "kill",
            What::Injured { .. } => "injured",
            What::GaveBirth { .. } => "gave_birth",
            What::LaidEggs { .. } => "laid_eggs",
            What::DenMoved { .. } => "den_moved",
            What::Died { .. } => "died",
        }
    }
}

/// The smallest wound rise that counts as an injury: below this is float noise. Healing
/// only lowers the stock, so any real hit clears it.
const WOUND_RISE: f64 = 1e-9;

/// One body as the recorder reads it: what [`BodyState::of`] takes from an animal.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BodyState {
    pub id: u64,
    pub founder: Option<Founder>,
    pub site: Site,
    /// Its place on the support layer, metres.
    pub x: f64,
    pub z: f64,
    pub stage: LifeStage,
    /// The feeding bout in progress: the tick it opened and its kind.
    pub bout: Option<(u64, BoutKind)>,
    /// Raw organic matter in the stomach, by food ([`Food::index`]).
    pub stomach: [f64; Food::COUNT],
    pub wound: f64,
    /// The prey of the strike in progress.
    pub striking: Option<u64>,
    /// Carrying a gestation escrow.
    pub escrow: bool,
    /// Organic matter passed on to offspring, cumulative.
    pub given: f64,
    pub pack: Option<u64>,
    pub riding: Option<u64>,
    /// Born in the world rather than founded or introduced.
    pub native: bool,
    pub submerged: bool,
    pub dryness: f64,
    /// At the last tick of its drawn lifespan.
    pub aged: bool,
    pub age_ticks: u64,
    /// What went into the stomach since the bout opened (the recorder's own).
    pub(crate) meal: [f64; Food::COUNT],
}

impl BodyState {
    pub fn of(config: &cubarium_voxel_fauna::FaunaConfig, a: &cubarium_voxel_fauna::Animal) -> BodyState {
        let mut stomach = [0.0; Food::COUNT];
        for f in Food::ALL {
            stomach[f.index()] = a.stomach.pools[f.index()].organic;
        }
        BodyState {
            id: a.id,
            founder: a.founder,
            site: a.site,
            x: a.pose.x,
            z: a.pose.z,
            stage: stage_of(config, a),
            bout: a.bout.map(|b| (b.start_tick, b.kind)),
            stomach,
            wound: a.wound,
            striking: a.strike.map(|s| s.prey),
            escrow: a.reproduction.escrow.is_some(),
            given: a.reproduction.given,
            pack: a.pack.map(|p| p.pack),
            riding: a.attached.map(|h| h.host),
            native: a.reproduction.native,
            submerged: a.mobility.submerged_ticks > 0,
            dryness: a.mobility.dryness,
            aged: a.lifespan_ticks > 0 && a.age_ticks + 1 >= a.lifespan_ticks,
            age_ticks: a.age_ticks,
            meal: [0.0; Food::COUNT],
        }
    }
}

type Seen = BodyState;

/// A clutch as the recorder reads it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ClutchState {
    pub lineage: Founder,
    pub site: Site,
    pub laid_tick: u64,
    pub eggs: u32,
}

/// Everything the recorder reads on one tick: [`Census::of`] a layer, or built by hand.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Census {
    /// The animal layer's own tick: what bouts and clutches are timed on.
    pub tick: u64,
    /// The world's tick, which events are stamped with (the layer's tick counts from the
    /// layer's making, after the world's pre-roll); `None` stamps them with `tick`.
    pub world_tick: Option<u64>,
    /// Sorted by id.
    pub bodies: Vec<BodyState>,
    pub clutches: Vec<ClutchState>,
    /// Each pack's id and den face.
    pub dens: Vec<(u64, Site)>,
    /// Per lineage ([`Founder::index`]), the ledger's departures by cause
    /// ([`Departure::index`]), cumulative.
    pub departed: Vec<[u64; Departure::COUNT]>,
}

impl Census {
    /// Read `fauna` into this census, reusing its buffers.
    pub fn read(&mut self, fauna: &Fauna, world_tick: u64) {
        let view = fauna.view();
        self.tick = view.tick;
        self.world_tick = Some(world_tick);
        self.bodies.clear();
        self.bodies.extend(view.animals.iter().map(|a| BodyState::of(view.config, a)));
        self.clutches.clear();
        self.clutches.extend(view.clutches.iter().map(|c| ClutchState {
            lineage: c.lineage,
            site: c.site,
            laid_tick: c.laid_tick,
            eggs: c.count,
        }));
        self.dens.clear();
        self.dens.extend(fauna.packs().iter().map(|p| (p.id, p.den)));
        self.departed.clear();
        self.departed.extend(
            Founder::ALL
                .into_iter()
                .map(|f| Departure::ALL.map(|c| view.ledger.departed_founder(f, c))),
        );
    }

    pub fn of(fauna: &Fauna, world_tick: u64) -> Census {
        let mut c = Census::default();
        c.read(fauna, world_tick);
        c
    }
}

/// A clutch's identity: its lineage, its face and the tick it was laid.
type ClutchKey = (usize, Site, u64);

/// Compares the animal layer tick by tick and writes [`LifeEvent`]s. Call
/// [`LifeRecorder::observe`] after every step (missing ticks merges what happened in
/// them, and loses the inference that needs one tick's resolution); collect with
/// [`LifeRecorder::take_events`].
#[derive(Debug, Default)]
pub struct LifeRecorder {
    started: bool,
    seen: Vec<Seen>,
    next: Vec<Seen>,
    clutches: Vec<(ClutchKey, Option<u64>)>,
    dens: Vec<(u64, Site)>,
    departed: Vec<[u64; Departure::COUNT]>,
    pending: Vec<LifeEvent>,
    census: Census,
    /// The world's tick less the layer's, on the last census.
    stamp_offset: i64,
}

fn site3(s: Site) -> [u32; 3] {
    [s.x, s.y, s.z]
}

fn dist2(ax: f64, az: f64, bx: f64, bz: f64) -> f64 {
    (ax - bx).powi(2) + (az - bz).powi(2)
}

impl LifeRecorder {
    pub fn new() -> LifeRecorder {
        LifeRecorder::default()
    }

    /// The events since the last call, oldest first.
    pub fn take_events(&mut self) -> Vec<LifeEvent> {
        std::mem::take(&mut self.pending)
    }

    /// Whether a first census has been read (and its bodies recorded as observed).
    pub fn started(&self) -> bool {
        self.started
    }

    /// Events waiting to be taken.
    pub fn pending(&self) -> usize {
        self.pending.len()
    }

    /// Intake already inferred in open bouts, for censored study life tables. Does
    /// not close the bout or emit an event. No stomach reading is duplicated.
    pub fn unfinished_meals(&self) -> impl Iterator<Item = (u64, [f64; Food::COUNT])> + '_ {
        self.seen.iter().map(|s| (s.id, s.meal))
    }

    /// Open-bout intake of one body; zero after a bout closes or a body departs.
    pub fn unfinished_meal(&self, id: u64) -> [f64; Food::COUNT] {
        self.seen.binary_search_by_key(&id, |s| s.id)
            .map_or([0.0; Food::COUNT], |i| self.seen[i].meal)
    }

    /// Record `what` at the layer's `tick`, stamped on the world's clock.
    fn emit(&mut self, tick: u64, animal: u64, what: What) {
        let tick = (tick as i64 + self.stamp_offset).max(0) as u64;
        self.pending.push(LifeEvent { tick, animal, what });
    }

    /// Compare the layer with the last call and record what changed, stamping events with
    /// the world's tick `world_tick`.
    pub fn observe(&mut self, fauna: &Fauna, world_tick: u64) {
        let mut census = std::mem::take(&mut self.census);
        census.bodies = std::mem::take(&mut self.next);
        census.read(fauna, world_tick);
        self.observe_census(&mut census);
        self.census = census;
    }

    /// [`LifeRecorder::observe`] on a census already read (or built by hand). Its body
    /// list is taken; the rest is left as it was.
    pub fn observe_census(&mut self, census: &mut Census) {
        let tick = census.tick;
        self.stamp_offset = census.world_tick.map_or(0, |w| w as i64 - tick as i64);
        let mut next = std::mem::take(&mut census.bodies);
        let clutches_now: Vec<ClutchKey> = census
            .clutches
            .iter()
            .map(|c| (c.lineage.index(), c.site, c.laid_tick))
            .collect();
        let clutch_states = census.clutches.clone();
        let dens_now = census.dens.clone();
        let departed_now = census.departed.clone();

        if !self.started {
            self.started = true;
            for s in &next {
                let species = s.founder.map_or("unknown", |f| f.name()).to_string();
                let what = What::Observed { species, stage: s.stage, age_s: s.age_ticks as f64 * DT };
                self.emit(tick, s.id, what);
            }
            self.clutches = clutches_now.into_iter().map(|k| (k, None)).collect();
            self.dens = dens_now;
            self.departed = departed_now;
            self.next = std::mem::replace(&mut self.seen, next);
            return;
        }

        // The merge: both lists are sorted by id, which never repeats.
        let prev = std::mem::take(&mut self.seen);
        let mut vanished: Vec<Seen> = Vec::new();
        let mut arrived: Vec<usize> = Vec::new();
        // Bodies whose gestation ended with a birth, and bodies that passed matter on
        // without one (a laying), as indices into `next`.
        let mut gave: Vec<usize> = Vec::new();
        let mut laid: Vec<usize> = Vec::new();
        let (mut i, mut j) = (0, 0);
        while i < prev.len() || j < next.len() {
            let (p, n) = (prev.get(i), next.get(j));
            match (p, n) {
                (Some(p), Some(n)) if p.id == n.id => {
                    let (p, k) = (*p, j);
                    self.compare(tick, &prev, &p, &mut next[k]);
                    let n = next[k];
                    if n.given > p.given {
                        if p.escrow && !n.escrow {
                            gave.push(k);
                        } else {
                            laid.push(k);
                        }
                    }
                    i += 1;
                    j += 1;
                }
                (Some(p), Some(n)) if p.id < n.id => {
                    vanished.push(*p);
                    i += 1;
                }
                (Some(p), None) => {
                    vanished.push(*p);
                    i += 1;
                }
                (_, Some(_)) => {
                    arrived.push(j);
                    j += 1;
                }
                (None, None) => unreachable!(),
            }
        }

        // Clutches: the ones laid this tick get their layer, the ones gone hatched (or
        // were lost; a hatchling's arrival is what tells them apart).
        let mut clutches: Vec<(ClutchKey, Option<u64>)> = Vec::with_capacity(clutches_now.len());
        let mut layings: Vec<(u64, u32, Site)> = Vec::new();
        for (c, key) in clutch_states.iter().zip(&clutches_now) {
            if let Some(&(_, parent)) = self.clutches.iter().find(|(k, _)| k == key) {
                clutches.push((*key, parent));
                continue;
            }
            let parent = laid
                .iter()
                .map(|&k| &next[k])
                .filter(|s| s.founder.map(|f| f.index()) == Some(key.0))
                .min_by(|a, b| {
                    let da = (i64::from(a.site.x) - i64::from(c.site.x)).abs() + (i64::from(a.site.z) - i64::from(c.site.z)).abs();
                    let db = (i64::from(b.site.x) - i64::from(c.site.x)).abs() + (i64::from(b.site.z) - i64::from(c.site.z)).abs();
                    da.cmp(&db)
                })
                .map(|s| s.id);
            if let Some(p) = parent {
                layings.push((p, c.eggs, c.site));
            }
            clutches.push((*key, parent));
        }
        let hatched: Vec<(ClutchKey, Option<u64>)> = self
            .clutches
            .iter()
            .filter(|(k, _)| !clutches_now.contains(k))
            .copied()
            .collect();
        self.clutches = clutches;
        for (parent, eggs, site) in layings {
            self.emit(tick, parent, What::LaidEggs { eggs, site: site3(site) });
        }

        // Births.
        let mut young: Vec<(u64, Vec<u64>)> = Vec::new();
        for &k in &arrived {
            let n = next[k];
            let species = n.founder.map_or("unknown", |f| f.name()).to_string();
            let lineage = n.founder.map(|f| f.index());
            let (how, parent) = if !n.native {
                ("introduced", None)
            } else if let Some((_, parent)) = hatched
                .iter()
                .filter(|((l, _, _), _)| Some(*l) == lineage)
                .min_by_key(|((_, s, _), _)| (i64::from(s.x) - i64::from(n.site.x)).abs() + (i64::from(s.z) - i64::from(n.site.z)).abs())
            {
                ("hatched", *parent)
            } else {
                let mother = gave
                    .iter()
                    .map(|&g| &next[g])
                    .filter(|s| s.founder == n.founder)
                    .min_by(|a, b| dist2(a.x, a.z, n.x, n.z).total_cmp(&dist2(b.x, b.z, n.x, n.z)))
                    .map(|s| s.id);
                ("live_birth", mother)
            };
            if how == "live_birth"
                && let Some(m) = parent
            {
                match young.iter_mut().find(|(id, _)| *id == m) {
                    Some((_, v)) => v.push(n.id),
                    None => young.push((m, vec![n.id])),
                }
            }
            self.emit(tick, n.id, What::Born { species, how: how.to_string(), parent });
        }
        for (mother, v) in young {
            self.emit(tick, mother, What::GaveBirth { young: v });
        }

        // Departures, matched against the ledger's rises for each lineage.
        if !vanished.is_empty() {
            let mut rises: Vec<[u64; Departure::COUNT]> = departed_now
                .iter()
                .zip(&self.departed)
                .map(|(now, was)| {
                    let mut d = [0u64; Departure::COUNT];
                    for c in 0..Departure::COUNT {
                        d[c] = now[c].saturating_sub(was[c]);
                    }
                    d
                })
                .collect();
            for v in &vanished {
                let striker = |by: Founder| {
                    prev.iter()
                        .find(|s| s.founder == Some(by) && s.striking == Some(v.id))
                        .map(|s| s.id)
                };
                let (cause, by) = match v.founder {
                    None => ("unknown".to_string(), None),
                    Some(f) => {
                        let d = &mut rises[f.index()];
                        let mut likely: Vec<Departure> = Vec::new();
                        if v.riding.is_some() {
                            likely.push(Departure::Groomed);
                        }
                        if v.aged {
                            likely.push(Departure::Aged);
                        }
                        let struck = prev.iter().any(|s| s.striking == Some(v.id));
                        if struck || v.wound > 0.0 {
                            likely.extend(Founder::ALL.map(|by| Departure::Killed { by }));
                            likely.push(Departure::Fell);
                        }
                        if v.submerged {
                            likely.push(Departure::Drowned);
                        }
                        if v.dryness > 0.0 {
                            likely.push(Departure::Desiccation);
                        }
                        likely.push(Departure::Starved);
                        let pick = likely
                            .into_iter()
                            .find(|c| d[c.index()] > 0)
                            .or_else(|| Departure::ALL.into_iter().find(|c| d[c.index()] > 0));
                        match pick {
                            Some(c) => {
                                d[c.index()] -= 1;
                                let by = match c {
                                    Departure::Killed { by } => striker(by),
                                    _ => None,
                                };
                                (c.name().to_string(), by)
                            }
                            None => ("unknown".to_string(), None),
                        }
                    }
                };
                if let Some(k) = by {
                    let prey_species = v.founder.map_or("unknown", lineage_label).to_string();
                    self.emit(tick, k, What::Kill { prey: v.id, prey_species });
                }
                self.emit(tick, v.id, What::Died { cause, by });
            }
        }
        self.departed = departed_now;

        // Dens.
        for &(pack, den) in &dens_now {
            if let Some(&(_, was)) = self.dens.iter().find(|(p, _)| *p == pack)
                && was != den
            {
                let members: Vec<u64> = next.iter().filter(|s| s.pack == Some(pack)).map(|s| s.id).collect();
                for m in members {
                    self.emit(tick, m, What::DenMoved { from: site3(was), to: site3(den) });
                }
            }
        }
        self.dens = dens_now;

        self.next = prev;
        self.seen = next;
    }

    /// One surviving body's changes between `p` (last tick) and `n` (this tick); carries
    /// the meal forward into `n`.
    fn compare(&mut self, tick: u64, prev: &[Seen], p: &Seen, n: &mut Seen) {
        let id = n.id;
        // The stomach's growth this tick: what was eaten (digestion only lowers it).
        let mut ate = [0.0; Food::COUNT];
        for (k, a) in ate.iter_mut().enumerate() {
            *a = (n.stomach[k] - p.stomach[k]).max(0.0);
        }
        match (p.bout, n.bout) {
            (Some((start, kind)), now) if now.map(|b| b.0) != Some(start) => {
                // The bout closed (or closed and a new one opened on the same tick).
                let foods = Food::ALL
                    .into_iter()
                    .filter(|f| p.meal[f.index()] + ate[f.index()] > 0.0)
                    .map(|f| FoodAmount { food: f.name().to_string(), organic: p.meal[f.index()] + ate[f.index()] })
                    .collect();
                self.emit(
                    tick,
                    id,
                    What::Meal {
                        bout: kind.name().to_string(),
                        foods,
                        duration_s: tick.saturating_sub(start) as f64 * DT,
                    },
                );
                n.meal = [0.0; Food::COUNT];
            }
            (None, Some(_)) => n.meal = ate,
            (Some(_), Some(_)) => {
                for k in 0..Food::COUNT {
                    n.meal[k] = p.meal[k] + ate[k];
                }
            }
            _ => {}
        }
        if n.stage == LifeStage::Adult && matches!(p.stage, LifeStage::Juvenile | LifeStage::Pup) {
            self.emit(tick, id, What::Matured);
        }
        if p.stage == LifeStage::Pup && n.stage != LifeStage::Pup {
            self.emit(tick, id, What::Independent);
        }
        if let (Some(host), None) = (p.riding, n.riding) {
            self.emit(tick, id, What::LetGo { host });
        }
        if let Some(prey) = n.striking
            && p.striking != Some(prey)
        {
            self.emit(tick, id, What::Hunt { prey });
        }
        if n.wound > p.wound + WOUND_RISE {
            let by = prev.iter().find(|s| s.striking == Some(id)).map(|s| s.id);
            self.emit(tick, id, What::Injured { wound: n.wound, by });
        }
    }
}

// --- The book -------------------------------------------------------------------------

/// One animal's life as recorded: where it came from, its family, and its events.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LifeRecord {
    pub id: u64,
    pub species: String,
    pub label: String,
    /// Its first event: `born` or `observed`.
    pub origin: LifeEvent,
    pub parent: Option<u64>,
    pub offspring: Vec<u64>,
    /// Its `died` event once it has left.
    pub death: Option<LifeEvent>,
    /// Every later event, oldest first, at most [`LifeBook::events_per_animal`] of the
    /// newest; `dropped` counts the older ones let go.
    pub events: VecDeque<LifeEvent>,
    pub dropped: u64,
}

/// The records by animal, bounded: each keeps its newest events, and the dead are kept
/// until [`LifeBook::dead_kept`] newer deaths have pushed them out.
#[derive(Clone, Debug)]
pub struct LifeBook {
    records: BTreeMap<u64, LifeRecord>,
    dead: VecDeque<u64>,
    pub events_per_animal: usize,
    pub dead_kept: usize,
}

impl Default for LifeBook {
    fn default() -> LifeBook {
        LifeBook::new(200, 2000)
    }
}

impl LifeBook {
    pub fn new(events_per_animal: usize, dead_kept: usize) -> LifeBook {
        LifeBook { records: BTreeMap::new(), dead: VecDeque::new(), events_per_animal, dead_kept }
    }

    pub fn record(&self, id: u64) -> Option<&LifeRecord> {
        self.records.get(&id)
    }

    pub fn records(&self) -> impl Iterator<Item = &LifeRecord> {
        self.records.values()
    }

    pub fn len(&self) -> usize {
        self.records.len()
    }

    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }

    /// File events, oldest first.
    pub fn apply(&mut self, events: &[LifeEvent]) {
        for e in events {
            match &e.what {
                What::Observed { species, .. } | What::Born { species, .. } => {
                    let parent = match &e.what {
                        What::Born { parent, .. } => *parent,
                        _ => None,
                    };
                    let label = Founder::ALL
                        .into_iter()
                        .find(|f| f.name() == species)
                        .map_or_else(|| species.clone(), |f| lineage_label(f).to_string());
                    self.records.insert(
                        e.animal,
                        LifeRecord {
                            id: e.animal,
                            species: species.clone(),
                            label,
                            origin: e.clone(),
                            parent,
                            offspring: Vec::new(),
                            death: None,
                            events: VecDeque::new(),
                            dropped: 0,
                        },
                    );
                    if let Some(p) = parent
                        && let Some(r) = self.records.get_mut(&p)
                    {
                        r.offspring.push(e.animal);
                    }
                }
                What::Died { .. } => {
                    if let Some(r) = self.records.get_mut(&e.animal) {
                        r.death = Some(e.clone());
                        self.dead.push_back(e.animal);
                    }
                    while self.dead.len() > self.dead_kept {
                        if let Some(old) = self.dead.pop_front() {
                            self.records.remove(&old);
                        }
                    }
                }
                _ => {
                    let cap = self.events_per_animal;
                    if let Some(r) = self.records.get_mut(&e.animal) {
                        r.events.push_back(e.clone());
                        while r.events.len() > cap {
                            r.events.pop_front();
                            r.dropped += 1;
                        }
                    }
                }
            }
        }
    }
}
