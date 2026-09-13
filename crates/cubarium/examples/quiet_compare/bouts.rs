//! Truthful rest classification, bout histories and tickwise accounting for the ordinary-quiet
//! comparison.
//!
//! ## Why this module exists
//!
//! The frozen ten-minute quiet diagnostic counted rest by reading `Mode::Resting` and assuming
//! every decision came from the ordinary controller. Under `post_birth_pause_v1` that assumption
//! is false: a held parent is *publicly* Resting because a pause imposed it, and a diagnostic
//! that cannot tell that apart from hunger-driven rest cannot validate the candidate at all. So
//! nothing here infers a class from the mode alone. Each completed interval is classified from
//! **actual core records** plus the pause set captured *before* the decision that produced it:
//!
//! | class | decided by |
//! | --- | --- |
//! | `newborn_initial` | the organism's own `born_tick` equals this completed tick |
//! | `post_birth_recovery` | a pause existed before the decision, `holds(T)` was true, and the world published no `Abort` for it at `T` |
//! | `satiated` | it is Resting and neither of the above — the ordinary controller chose it |
//!
//! The two boundaries the naive reading gets wrong are both handled explicitly. At
//! `T == end_tick` the entry still exists but `holds(T)` is false: that decision is the ordinary
//! **release**, so the interval it produces is not recovery. At an early **abort** the entry
//! existed and `holds(T)` was true, but the world abandoned the pause before the decision, so
//! that interval is not recovery either — the `Abort` record at `T` is what says so.
//!
//! ## What is exact and what is not
//!
//! Exact: every bout with full generational IDs, every admission, refusal, abort reason and
//! completed duration, organism-ticks by class and by mode, intake ticks, births, deaths by
//! cause, and same-face transported displacement.
//!
//! **Not exact, and labelled as such in the output:** total transported path length. The world's
//! own per-tick segments are only reachable through `World::render_view`, which clones four
//! whole fields per call — affordable at a census cadence, not 5.18 million times per arm. So
//! this module accumulates same-face chart displacement every tick (exact, and the only kind a
//! held parent's 0.0126 px drift can be), counts the ticks where an organism changed face
//! instead of accumulating a meaningless chart delta, and separately records the world's exact
//! transported length on the census cadence alone. A reader gets an exact partial total, an
//! exact count of what it omits, and an exact sampled rate — never a fabricated whole.
//!
//! Nothing here infers funding or oxidation from post-step deltas.

use std::collections::BTreeMap;

use cubarium_core::organism::Mode;
use cubarium_core::quiet::{POST_BIRTH_PAUSE_TICKS, QuietEvent, QuietPause, QuietReason};
use cubarium_core::rng::Counter;
use cubarium_core::{LifeEvent, OrganismId, World, WorldState};
use serde_json::{Value, json};

/// How many violation details are kept verbatim. The *count* is always exact; this bounds the
/// text, not the evidence that something went wrong.
pub const RETAINED_VIOLATIONS: usize = 16;

/// A compensated running total, so a 72-hour accumulation does not round its own tail away.
#[derive(Clone, Copy, Default)]
pub struct Sum {
    raw: f64,
    correction: f64,
}

impl Sum {
    pub fn add(&mut self, x: f64) {
        let next = self.raw + x;
        self.correction += if self.raw.abs() >= x.abs() {
            (self.raw - next) + x
        } else {
            (x - next) + self.raw
        };
        self.raw = next;
    }

    pub fn value(self) -> f64 {
        self.raw + self.correction
    }
}

/// Which kind of rest a completed interval was.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum RestClass {
    NewbornInitial,
    PostBirthRecovery,
    Satiated,
}

impl RestClass {
    pub fn as_str(self) -> &'static str {
        match self {
            RestClass::NewbornInitial => "newborn_initial",
            RestClass::PostBirthRecovery => "post_birth_recovery",
            RestClass::Satiated => "satiated",
        }
    }
}

/// Why a bout stopped. `Censored` means the horizon arrived first — the bout was still running.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BoutEnd {
    /// The pause reached its promised end and released at the exact boundary.
    Released,
    /// The pause stopped early, for this reason.
    Aborted(QuietReason),
    /// The organism became active again.
    Woke,
    /// It went on resting, but for a different reason than before.
    Reclassified(RestClass),
    /// It died.
    Died,
    /// The run ended while the bout was open.
    Censored,
}

impl BoutEnd {
    fn as_str(self) -> &'static str {
        match self {
            BoutEnd::Released => "released",
            BoutEnd::Aborted(_) => "aborted",
            BoutEnd::Woke => "woke",
            BoutEnd::Reclassified(_) => "reclassified",
            BoutEnd::Died => "died",
            BoutEnd::Censored => "censored",
        }
    }

    fn detail(self) -> Value {
        match self {
            BoutEnd::Aborted(reason) => json!(reason.as_str()),
            BoutEnd::Reclassified(class) => json!(class.as_str()),
            _ => Value::Null,
        }
    }
}

#[derive(Clone, Copy)]
struct Open {
    class: RestClass,
    start_tick: u64,
    last_tick: u64,
    /// The birth that offered the pause, for a recovery bout.
    origin: Option<(OrganismId, u64)>,
}

/// One completed rest bout, with the full generational identity of whoever had it.
pub struct Bout {
    pub id: OrganismId,
    pub class: RestClass,
    pub start_tick: u64,
    pub end_tick: u64,
    pub ticks: u64,
    pub origin: Option<(OrganismId, u64)>,
    pub end: BoutEnd,
}

impl Bout {
    pub fn json(&self) -> Value {
        json!({
            "id": self.id, "class": self.class.as_str(),
            "start_tick": self.start_tick, "end_tick": self.end_tick, "ticks": self.ticks,
            "origin_child": self.origin.map(|(c, _)| c),
            "origin_boundary": self.origin.map(|(_, b)| b),
            "end": self.end.as_str(), "end_detail": self.end.detail(),
        })
    }
}

/// Exact per-class organism-tick accounting.
#[derive(Clone, Copy, Default)]
pub struct ClassTotals {
    pub organism_ticks: u64,
    pub bouts: u64,
    pub same_face_px: Sum,
    pub seam_ticks: u64,
}

/// What the observer captured before a step, so the classification can read the world as it was
/// when the decision was made rather than as it is afterwards.
pub struct Pre {
    tick: u64,
    pauses: BTreeMap<OrganismId, QuietPause>,
    /// Only for parents whose decision this step may hold: escrow presence and turn counter.
    held_before: BTreeMap<OrganismId, (bool, Counter)>,
    positions: BTreeMap<OrganismId, (u8, f64, f64)>,
    alive: u64,
}

/// The whole per-arm observer: classification, bout histories, tickwise totals and the
/// reconciliation checks that make the records trustworthy.
pub struct Observer {
    opening_tick: u64,
    last_tick: u64,
    open: BTreeMap<OrganismId, Open>,
    completed: Vec<Bout>,
    pub totals: [ClassTotals; 3],
    pub active_ticks: u64,
    pub organism_ticks: u64,
    pub mode_ticks: [u64; 3],
    pub intake_ticks: u64,
    pub held_intake_ticks: u64,
    pub admissions: u64,
    pub refusals: BTreeMap<&'static str, u64>,
    pub aborts: BTreeMap<&'static str, u64>,
    pub releases: u64,
    pub completed_held_ticks: Sum,
    pub sampled_transported_px: Sum,
    pub sampled_path_ticks: u64,
    violations: Vec<Value>,
    violation_count: u64,
}

impl Observer {
    pub fn new(state: &WorldState) -> Self {
        Observer {
            opening_tick: state.tick,
            last_tick: state.tick,
            open: BTreeMap::new(),
            completed: Vec::new(),
            totals: [ClassTotals::default(); 3],
            active_ticks: 0,
            organism_ticks: 0,
            mode_ticks: [0; 3],
            intake_ticks: 0,
            held_intake_ticks: 0,
            admissions: 0,
            refusals: BTreeMap::new(),
            aborts: BTreeMap::new(),
            releases: 0,
            completed_held_ticks: Sum::default(),
            sampled_transported_px: Sum::default(),
            sampled_path_ticks: 0,
            violations: Vec::new(),
            violation_count: 0,
        }
    }

    fn slot(class: RestClass) -> usize {
        match class {
            RestClass::NewbornInitial => 0,
            RestClass::PostBirthRecovery => 1,
            RestClass::Satiated => 2,
        }
    }

    fn violation(&mut self, what: &str, detail: Value) {
        self.violation_count += 1;
        if self.violations.len() < RETAINED_VIOLATIONS {
            self.violations.push(json!({"what": what, "detail": detail}));
        }
    }

    /// True when every reconciliation check has held so far. An arm that fails one is retained
    /// and reported, never quietly presented as a completed measurement.
    pub fn reconciled(&self) -> bool {
        self.violation_count == 0
    }

    /// Capture the world **before** `step`, which is the only moment the pause set and the
    /// pre-decision stocks are the ones the decision will actually see.
    pub fn before(&mut self, world: &World) -> Pre {
        let state = &world.state;
        let mut pauses = BTreeMap::new();
        let mut held_before = BTreeMap::new();
        for p in &state.quiet.pauses {
            pauses.insert(p.parent, *p);
            if p.holds(state.tick)
                && let Some(o) = state.organisms.get(p.parent)
            {
                held_before.insert(p.parent, (o.escrow.is_some(), o.turn_counter));
            }
        }
        let positions = state
            .organisms
            .iter()
            .map(|(id, o)| (id, (o.pos.face.index() as u8, o.pos.u, o.pos.v)))
            .collect();
        Pre {
            tick: state.tick,
            pauses,
            held_before,
            positions,
            alive: state.organisms.len() as u64,
        }
    }

    /// Classify the interval the step just completed, extend or close bouts, accumulate the
    /// tickwise totals, and reconcile the published records against the world itself.
    pub fn after(
        &mut self,
        world: &World,
        pre: Pre,
        life: &[LifeEvent],
        quiet: &[QuietEvent],
    ) -> anyhow::Result<()> {
        let state = &world.state;
        anyhow::ensure!(
            state.tick == pre.tick + 1,
            "quiet observer skipped a tick: {} after {}",
            state.tick,
            pre.tick
        );
        let now = state.tick;
        self.last_tick = now;

        // --- the records, reconciled against the world that published them -----------------
        let mut aborted_at_pre: BTreeMap<OrganismId, QuietReason> = BTreeMap::new();
        for event in quiet {
            match *event {
                QuietEvent::Begin { tick, parent, child, end_tick, .. } => {
                    self.admissions += 1;
                    // An admission must correspond to a real paid insertion committed at this
                    // very boundary, by this very parent.
                    let matched = life.iter().any(|e| {
                        matches!(e, LifeEvent::Birth { tick: t, id, parent: p, .. }
                            if *t == tick && *id == child && *p == parent)
                    });
                    if !matched {
                        self.violation("admission without a matching paid birth", json!({"tick": tick, "parent": parent, "child": child}));
                    }
                    if end_tick != tick + POST_BIRTH_PAUSE_TICKS {
                        self.violation("admission window is not the candidate's", json!({"tick": tick, "end_tick": end_tick}));
                    }
                    if state.organisms.get(parent).is_none() {
                        self.violation("admission for a parent that is not alive", json!({"parent": parent}));
                    }
                    if state.organisms.get(parent).is_some_and(|o| o.born_tick == tick) {
                        self.violation("admission for a newborn", json!({"parent": parent}));
                    }
                    if parent == child {
                        self.violation("admission naming its own child", json!({"parent": parent}));
                    }
                }
                QuietEvent::Refuse { reason, .. } => {
                    *self.refusals.entry(reason.as_str()).or_default() += 1;
                }
                QuietEvent::End { tick, parent, completed_ticks, .. } => {
                    self.releases += 1;
                    self.completed_held_ticks.add(completed_ticks as f64);
                    if completed_ticks != POST_BIRTH_PAUSE_TICKS {
                        self.violation("a release did not complete the window", json!({"parent": parent, "completed_ticks": completed_ticks, "tick": tick}));
                    }
                }
                QuietEvent::Abort { tick, parent, completed_ticks, reason, .. } => {
                    *self.aborts.entry(reason.as_str()).or_default() += 1;
                    self.completed_held_ticks.add(completed_ticks as f64);
                    if completed_ticks >= POST_BIRTH_PAUSE_TICKS {
                        self.violation("an abort completed the whole window", json!({"parent": parent, "completed_ticks": completed_ticks}));
                    }
                    if tick == pre.tick {
                        aborted_at_pre.insert(parent, reason);
                    }
                }
            }
        }

        // --- classification -----------------------------------------------------------------
        let mut seen: Vec<OrganismId> = Vec::with_capacity(state.organisms.len());
        for (id, o) in state.organisms.iter() {
            seen.push(id);
            self.organism_ticks += 1;
            self.mode_ticks[match o.mode {
                Mode::Resting => 0,
                Mode::Seeking => 1,
                Mode::Feeding => 2,
            }] += 1;
            if o.fed_this_tick {
                self.intake_ticks += 1;
            }

            // Was this organism's decision held? Only the pre-decision pause set can say, and
            // only if the world did not abandon the pause before making that decision.
            let held = pre
                .pauses
                .get(&id)
                .is_some_and(|p| p.holds(pre.tick) && !aborted_at_pre.contains_key(&id));

            let class = if o.born_tick == now {
                if held {
                    self.violation("a newborn was held by a pause", json!({"id": id, "tick": now}));
                }
                Some(RestClass::NewbornInitial)
            } else if held {
                if o.mode != Mode::Resting {
                    self.violation("a held interval is not Resting", json!({"id": id, "tick": now, "mode": format!("{:?}", o.mode)}));
                }
                if o.fed_this_tick {
                    self.held_intake_ticks += 1;
                    self.violation("a held interval took intake", json!({"id": id, "tick": now}));
                }
                if let Some((escrow_before, counter_before)) = pre.held_before.get(&id) {
                    if !escrow_before && o.escrow.is_some() {
                        self.violation("a held interval opened a new gestation", json!({"id": id, "tick": now}));
                    }
                    let drew = o.turn_counter.0.wrapping_sub(counter_before.0);
                    if drew != 4 {
                        self.violation("a held interval drew a different number of turn counters", json!({"id": id, "tick": now, "drew": drew}));
                    }
                }
                Some(RestClass::PostBirthRecovery)
            } else if o.mode == Mode::Resting {
                Some(RestClass::Satiated)
            } else {
                self.active_ticks += 1;
                None
            };

            // Real motion, as far as the public API can state it exactly.
            let moved_px = match pre.positions.get(&id) {
                Some((face, u, v)) if *face == o.pos.face.index() as u8 => {
                    Some(((o.pos.u - u).powi(2) + (o.pos.v - v).powi(2)).sqrt())
                }
                Some(_) => None,
                None => Some(0.0),
            };

            match class {
                Some(class) => {
                    let slot = Self::slot(class);
                    self.totals[slot].organism_ticks += 1;
                    match moved_px {
                        Some(px) => self.totals[slot].same_face_px.add(px),
                        None => self.totals[slot].seam_ticks += 1,
                    }
                    let origin = if class == RestClass::PostBirthRecovery {
                        pre.pauses.get(&id).map(|p| (p.child, p.start_tick))
                    } else {
                        None
                    };
                    match self.open.get_mut(&id) {
                        Some(open) if open.class == class => open.last_tick = now,
                        Some(_) => {
                            self.close(id, BoutEnd::Reclassified(class));
                            self.begin(id, class, now, origin);
                        }
                        None => self.begin(id, class, now, origin),
                    }
                }
                None => {
                    if self.open.contains_key(&id) {
                        // A recovery bout that ended because the pause released is labelled by
                        // the world's own record, not by the mode change.
                        let end = self
                            .open
                            .get(&id)
                            .filter(|o| o.class == RestClass::PostBirthRecovery)
                            .and_then(|_| {
                                quiet.iter().find_map(|e| match *e {
                                    QuietEvent::End { parent, .. } if parent == id => {
                                        Some(BoutEnd::Released)
                                    }
                                    QuietEvent::Abort { parent, reason, .. } if parent == id => {
                                        Some(BoutEnd::Aborted(reason))
                                    }
                                    _ => None,
                                })
                            })
                            .unwrap_or(BoutEnd::Woke);
                        self.close(id, end);
                    }
                }
            }
        }

        // Anyone whose bout was open and who is no longer here died this tick.
        let gone: Vec<OrganismId> = self
            .open
            .keys()
            .copied()
            .filter(|id| state.organisms.get(*id).is_none())
            .collect();
        for id in gone {
            self.close(id, BoutEnd::Died);
        }
        let _ = (pre.alive, seen);
        Ok(())
    }

    fn begin(&mut self, id: OrganismId, class: RestClass, now: u64, origin: Option<(OrganismId, u64)>) {
        self.open.insert(id, Open { class, start_tick: now, last_tick: now, origin });
    }

    fn close(&mut self, id: OrganismId, end: BoutEnd) {
        let Some(open) = self.open.remove(&id) else { return };
        let slot = Self::slot(open.class);
        self.totals[slot].bouts += 1;
        self.completed.push(Bout {
            id,
            class: open.class,
            start_tick: open.start_tick,
            end_tick: open.last_tick,
            ticks: open.last_tick - open.start_tick + 1,
            origin: open.origin,
            end,
        });
    }

    /// Take the bouts finished since the last drain, so a long run streams instead of growing.
    pub fn drain_bouts(&mut self) -> Vec<Bout> {
        std::mem::take(&mut self.completed)
    }

    /// Close every still-running bout as censored, at the horizon.
    pub fn close_censored(&mut self) {
        let open: Vec<OrganismId> = self.open.keys().copied().collect();
        for id in open {
            self.close(id, BoutEnd::Censored);
        }
    }

    /// The world's own exact transported path length at this instant, on the census cadence.
    /// Called rarely on purpose: `render_view` clones four whole fields.
    pub fn sample_transported(&mut self, world: &World) {
        let view = world.render_view();
        let mut total = 0.0;
        for o in &view.organisms {
            for segment in &o.moved {
                total += segment.length();
            }
        }
        self.sampled_transported_px.add(total);
        self.sampled_path_ticks += 1;
    }

    pub fn summary(&self) -> Value {
        let class = |c: RestClass| {
            let t = &self.totals[Self::slot(c)];
            json!({
                "organism_ticks": t.organism_ticks,
                "bouts": t.bouts,
                "same_face_path_px": t.same_face_px.value(),
                "seam_ticks": t.seam_ticks,
            })
        };
        json!({
            "opening_tick": self.opening_tick, "last_tick": self.last_tick,
            "classification_basis": "each completed interval is classified from the organism's own born_tick, the pause set captured BEFORE the decision, and the world's own abort records; never from Mode::Resting alone",
            "reconciled": self.reconciled(),
            "violations": self.violation_count,
            "retained_violations": self.violations,
            "newborn_initial": class(RestClass::NewbornInitial),
            "post_birth_recovery": class(RestClass::PostBirthRecovery),
            "satiated": class(RestClass::Satiated),
            "active_organism_ticks": self.active_ticks,
            "organism_ticks": self.organism_ticks,
            "mode_ticks": {"resting": self.mode_ticks[0], "seeking": self.mode_ticks[1], "feeding": self.mode_ticks[2]},
            "intake_ticks": self.intake_ticks,
            "held_intake_ticks": self.held_intake_ticks,
            "quiet_records": {
                "admissions": self.admissions,
                "releases": self.releases,
                "refusals": self.refusals,
                "aborts": self.aborts,
                "completed_held_ticks": self.completed_held_ticks.value(),
            },
            "path_basis": "same_face_path_px is the exact per-tick chart displacement of organisms that stayed on one face; seam_ticks counts the ticks omitted from it because the organism changed face and a chart delta would be meaningless. sampled_transported_px is the world's own exact transported length, summed only on the census cadence — a rate, never a total.",
            "sampled_transported_px": self.sampled_transported_px.value(),
            "sampled_path_ticks": self.sampled_path_ticks,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cubarium_core::quiet::QuietState;
    use cubarium_core::{WorldConfig, decode_snapshot};

    fn mature() -> WorldState {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../cubarium-core/tests/fixtures/quiet-v12-plain-3000.cubw");
        let (_, state) = decode_snapshot(&std::fs::read(path).expect("the fixture")).expect("loads");
        state
    }

    fn run(state: WorldState, ticks: u64) -> (World, Observer, Vec<Bout>) {
        let mut world = World::from_state(state).expect("valid");
        let mut observer = Observer::new(&world.state);
        let mut bouts = Vec::new();
        for _ in 0..ticks {
            let pre = observer.before(&world);
            world.step();
            let life = world.drain_events();
            let quiet = world.drain_quiet_events();
            observer.after(&world, pre, &life, &quiet).expect("observed");
            bouts.extend(observer.drain_bouts());
        }
        observer.close_censored();
        bouts.extend(observer.drain_bouts());
        (world, observer, bouts)
    }

    /// An Off world has no recovery at all, and its newborn rest is still counted separately from
    /// hunger-driven rest: the three classes are distinguished even where one is always empty.
    #[test]
    fn an_off_world_records_no_recovery_and_still_separates_the_other_two() {
        let (world, observer, bouts) = run(mature(), 1200);
        assert!(observer.reconciled(), "{:?}", observer.summary());
        assert_eq!(observer.admissions, 0);
        assert_eq!(observer.totals[Observer::slot(RestClass::PostBirthRecovery)].organism_ticks, 0);
        assert!(bouts.iter().all(|b| b.class != RestClass::PostBirthRecovery));
        // Newborn rest really is observed, and it comes from real births.
        let newborns = bouts.iter().filter(|b| b.class == RestClass::NewbornInitial).count();
        assert!(newborns > 0, "the fixture must produce births in 1200 ticks");
        assert!(world.state.births_total > 0);
        // Every organism-tick is accounted for exactly once.
        let classified: u64 = observer.totals.iter().map(|t| t.organism_ticks).sum();
        assert_eq!(classified + observer.active_ticks, observer.organism_ticks);
        assert_eq!(observer.mode_ticks[0], classified, "resting ticks are the classified ones");
    }

    /// The candidate's recovery bouts are real, exactly forty ticks when released, and every one
    /// of them reconciles to a paid birth.
    #[test]
    fn candidate_recovery_bouts_are_forty_ticks_and_reconcile_to_births() {
        let mut state = mature();
        state.quiet = QuietState::post_birth_pause_v1();
        let (_, observer, bouts) = run(state, 3000);
        assert!(observer.reconciled(), "{:?}", observer.summary());
        assert!(observer.admissions > 0, "the fixture must admit a pause in 3000 ticks");

        let recovery: Vec<&Bout> =
            bouts.iter().filter(|b| b.class == RestClass::PostBirthRecovery).collect();
        assert!(!recovery.is_empty());
        for b in &recovery {
            assert!(b.origin.is_some(), "a recovery bout must name its originating birth");
            let (_, boundary) = b.origin.unwrap();
            assert_eq!(b.start_tick, boundary + 1, "the first completed interval is B+1");
            assert!(b.ticks <= POST_BIRTH_PAUSE_TICKS);
            match b.end {
                BoutEnd::Released => {
                    assert_eq!(b.ticks, POST_BIRTH_PAUSE_TICKS);
                    assert_eq!(b.end_tick, boundary + POST_BIRTH_PAUSE_TICKS);
                }
                BoutEnd::Aborted(_) => assert!(b.ticks < POST_BIRTH_PAUSE_TICKS),
                BoutEnd::Died | BoutEnd::Censored | BoutEnd::Reclassified(_) => {}
                BoutEnd::Woke => panic!("a recovery bout ended without a record: {:?}", b.json()),
            }
        }
        // Released bouts equal the world's own release records.
        let released = recovery.iter().filter(|b| b.end == BoutEnd::Released).count() as u64;
        assert_eq!(released, observer.releases);
        assert_eq!(observer.held_intake_ticks, 0, "no held interval may take intake");
    }

    /// The release boundary is not a forty-first held interval, and an abort's own tick is not a
    /// held one either. Both are the cases a mode-only reading gets wrong.
    #[test]
    fn the_release_and_abort_boundaries_are_ordinary_decisions() {
        let mut state = mature();
        state.quiet = QuietState::post_birth_pause_v1();
        let mut world = World::from_state(state).expect("valid");
        let mut observer = Observer::new(&world.state);

        // Run to the first admission, then watch the exact window.
        let (parent, b) = loop {
            let pre = observer.before(&world);
            world.step();
            let life = world.drain_events();
            let quiet = world.drain_quiet_events();
            observer.after(&world, pre, &life, &quiet).expect("observed");
            observer.drain_bouts();
            if let Some((parent, tick)) = quiet.iter().find_map(|e| match *e {
                QuietEvent::Begin { parent, tick, .. } => Some((parent, tick)),
                _ => None,
            }) {
                break (parent, tick);
            }
            assert!(world.tick() < 147_000 + 20_000, "no admission");
        };

        let before_recovery =
            observer.totals[Observer::slot(RestClass::PostBirthRecovery)].organism_ticks;
        // Forty held decisions produce forty recovery intervals and not one more.
        for _ in 0..POST_BIRTH_PAUSE_TICKS {
            let pre = observer.before(&world);
            world.step();
            let life = world.drain_events();
            let quiet = world.drain_quiet_events();
            observer.after(&world, pre, &life, &quiet).expect("observed");
        }
        let after_forty =
            observer.totals[Observer::slot(RestClass::PostBirthRecovery)].organism_ticks;
        assert_eq!(after_forty - before_recovery, POST_BIRTH_PAUSE_TICKS);
        assert_eq!(world.tick(), b + POST_BIRTH_PAUSE_TICKS);

        // The release decision. The entry still exists going in, and `holds` is false, so the
        // interval it produces is not recovery even if the parent happens to rest on.
        let pre = observer.before(&world);
        assert!(pre.pauses.contains_key(&parent), "the entry is still there at the boundary");
        assert!(!pre.pauses[&parent].holds(pre.tick), "but it holds no decision");
        world.step();
        let life = world.drain_events();
        let quiet = world.drain_quiet_events();
        observer.after(&world, pre, &life, &quiet).expect("observed");
        let after_release =
            observer.totals[Observer::slot(RestClass::PostBirthRecovery)].organism_ticks;
        assert_eq!(after_release, after_forty, "the release produced a forty-first recovery tick");
        assert_eq!(observer.releases, 1);
        assert!(observer.reconciled());
    }

    /// An aborted pause stops contributing recovery on the abort's own tick, and the bout is
    /// labelled with the world's reason rather than guessed from the mode.
    #[test]
    fn an_aborted_pause_is_labelled_by_the_worlds_reason_and_stops_at_its_tick() {
        let mut state = mature();
        state.quiet = QuietState::post_birth_pause_v1();
        let mut world = World::from_state(state).expect("valid");
        let mut observer = Observer::new(&world.state);
        let mut bouts = Vec::new();

        let parent = loop {
            let pre = observer.before(&world);
            world.step();
            let life = world.drain_events();
            let quiet = world.drain_quiet_events();
            observer.after(&world, pre, &life, &quiet).expect("observed");
            bouts.extend(observer.drain_bouts());
            if let Some(parent) = quiet.iter().find_map(|e| match *e {
                QuietEvent::Begin { parent, .. } => Some(parent),
                _ => None,
            }) {
                break parent;
            }
            assert!(world.tick() < 147_000 + 20_000, "no admission");
        };
        // Hold five, then strip the parent so it cannot cover the rest.
        for _ in 0..5 {
            let pre = observer.before(&world);
            world.step();
            let life = world.drain_events();
            let quiet = world.drain_quiet_events();
            observer.after(&world, pre, &life, &quiet).expect("observed");
            bouts.extend(observer.drain_bouts());
        }
        world.state.organisms.get_mut(parent).expect("alive").energy = 1e-9;
        let recovery_before =
            observer.totals[Observer::slot(RestClass::PostBirthRecovery)].organism_ticks;
        let pre = observer.before(&world);
        world.step();
        let life = world.drain_events();
        let quiet = world.drain_quiet_events();
        observer.after(&world, pre, &life, &quiet).expect("observed");
        bouts.extend(observer.drain_bouts());

        assert_eq!(
            observer.totals[Observer::slot(RestClass::PostBirthRecovery)].organism_ticks,
            recovery_before,
            "the abort's own tick was counted as recovery"
        );
        assert_eq!(observer.aborts.values().sum::<u64>(), 1);
        let aborted = bouts
            .iter()
            .find(|b| b.id == parent && b.class == RestClass::PostBirthRecovery)
            .expect("the aborted bout is closed");
        assert!(matches!(aborted.end, BoutEnd::Aborted(QuietReason::UnaffordableRemaining)));
        assert_eq!(aborted.ticks, 5, "five completed intervals, not six");
        assert!(observer.reconciled());
    }

    /// A bout still running at the horizon is censored, not silently dropped or rounded up.
    #[test]
    fn an_open_bout_at_the_horizon_is_censored() {
        let mut state = mature();
        state.quiet = QuietState::post_birth_pause_v1();
        let mut world = World::from_state(state).expect("valid");
        let mut observer = Observer::new(&world.state);
        let mut bouts = Vec::new();
        loop {
            let pre = observer.before(&world);
            world.step();
            let life = world.drain_events();
            let quiet = world.drain_quiet_events();
            observer.after(&world, pre, &life, &quiet).expect("observed");
            bouts.extend(observer.drain_bouts());
            if quiet.iter().any(|e| matches!(e, QuietEvent::Begin { .. })) {
                break;
            }
            assert!(world.tick() < 147_000 + 20_000, "no admission");
        }
        // Stop one tick into the window, with the bout open.
        let pre = observer.before(&world);
        world.step();
        let life = world.drain_events();
        let quiet = world.drain_quiet_events();
        observer.after(&world, pre, &life, &quiet).expect("observed");
        observer.close_censored();
        bouts.extend(observer.drain_bouts());
        let censored: Vec<&Bout> = bouts.iter().filter(|b| b.end == BoutEnd::Censored).collect();
        assert!(
            censored.iter().any(|b| b.class == RestClass::PostBirthRecovery && b.ticks == 1),
            "the open recovery bout must be censored at one tick"
        );
    }

    /// The observer refuses to describe a world it has not kept up with.
    #[test]
    fn a_skipped_tick_is_refused() {
        let mut world = World::from_state(mature()).expect("valid");
        let mut observer = Observer::new(&world.state);
        let pre = observer.before(&world);
        world.step();
        world.step();
        world.drain_events();
        assert!(observer.after(&world, pre, &[], &[]).is_err());
    }

    /// Observation is inert: running the observer changes nothing about the world.
    #[test]
    fn observing_does_not_move_the_world() {
        let mut cfg = WorldConfig::default();
        cfg.founders.count = 24;
        let state = World::new(cfg).expect("valid").state;
        let (observed, _, _) = run(state.clone(), 400);
        let mut plain = World::from_state(state).expect("valid");
        for _ in 0..400 {
            plain.step();
            plain.drain_events();
            plain.drain_quiet_events();
        }
        assert_eq!(
            cubarium_core::snapshot::state_hash(&observed.state),
            cubarium_core::snapshot::state_hash(&plain.state)
        );
    }
}
