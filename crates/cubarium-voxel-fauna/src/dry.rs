//! **The dry-run teacher** (BRAIN-NEXT step 2; `design/behaviour-ai-2026-10-11.md` §11.3,
//! "a short closed-loop pass: the learner picks intents, and the teacher labels the states it
//! actually reaches").
//!
//! Beside a brain-driven body of a lineage switched on ([`crate::Fauna::dry_teach`]), the
//! lineage's heuristic runs **dry**: at each of the body's decision samples it reads the
//! observation the brain read, its heuristic layer (flight, roost, grooming, the walk to a
//! mother, the herd pull, the patrol, the carcass yield, …) runs as it would for a heuristic
//! body, and the rules' intent record proposes and holds. Nothing it does reaches the world:
//!
//! - it runs on a **copy** of the animal layer taken before the controller stage (the copy
//!   is cheap: no controllers, no tap), with only the dry bodies due, and the copy is
//!   dropped; the real stage runs first and exactly as it would without it;
//! - the teacher's own memory — its controller, its intent record and hold, its vigil, its
//!   patrol and yield latch, its search, patch and walk records — is kept here, per body,
//!   and put into the copy before each sample and read back after, so the dry teacher
//!   remembers as a heuristic body would while the real body's records stay the brain's;
//! - what it decided (proposal and held intent with their subjects, steering source, own and
//!   executed actions) goes to the tap ([`crate::ActionTap::teacher_labels`]) for cloning.
//!
//! The teacher sees the brain's body as it stands (pose, stores, mobility, wounds): it labels
//! the states the learner reaches, not states of its own making.
//!
//! [`DryScope::Every`] also runs it beside heuristic bodies, where its labels can be checked
//! against the real teacher's.

use std::collections::BTreeMap;

use rustc_hash::FxHashMap;
use serde::{Deserialize, Serialize};

use crate::intent::{IntentRecord, Subject};
use crate::{Controller, Fauna, Founder, Held, Layer, Steer};

/// Which bodies of a lineage the dry teacher runs beside.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum DryScope {
    /// Brain-driven bodies (the closed-loop pass).
    Brains,
    /// Every driven body, heuristic ones too (a check of the dry run against the real one).
    Every,
}

/// **The dry teacher's label** for one decision sample of one body.
#[derive(Clone, Debug, PartialEq)]
pub struct TeacherLabel {
    pub tick: u64,
    /// The rules' proposal before their hold, and its subject.
    pub proposal: u8,
    pub proposal_subject: Subject,
    /// The subject's row in the body's own decision sample (the brain's candidates), if it
    /// is one of them.
    pub proposal_row: Option<usize>,
    /// What the rules held after their hold, and its subject and row.
    pub held: u8,
    pub subject: Subject,
    pub held_row: Option<usize>,
    /// The heuristic layer's steering source.
    pub steer: Steer,
    /// The teacher controller's own answer, and what its layer would have held.
    pub own: Held,
    pub executed: Held,
}

/// One body's dry teacher and its memory.
struct DryBody {
    founder: Founder,
    teacher: Option<Box<dyn Controller>>,
    record: IntentRecord,
    vigil: crate::Vigil,
    hunt: crate::hunt::HuntMemory,
    yielding: bool,
    bed_until: u64,
    steer: Option<Steer>,
    rove: Option<crate::step::rove::Rove>,
    forage: Option<crate::step::forage::Patch>,
    mother_seen: Option<crate::Track>,
    route_flee: Option<u64>,
    walk_targets: Option<(u64, u64, u64)>,
    walk_pending: Option<(u64, Steer)>,
    walk_shun: Option<Vec<(u64, u64)>>,
}

fn put<V>(map: &mut FxHashMap<u64, V>, id: u64, v: Option<V>) {
    match v {
        Some(v) => {
            map.insert(id, v);
        }
        None => {
            map.remove(&id);
        }
    }
}

impl DryBody {
    /// A fresh teacher for body `i` of `fauna`, its memory starting from the body's own.
    fn new(fauna: &Fauna, i: usize, founder: Founder) -> DryBody {
        let a = &fauna.animals[i];
        let id = a.id;
        DryBody {
            founder,
            teacher: Some(crate::heuristic(founder, fauna.config.teachers)),
            record: IntentRecord { last_wound: a.wound, ..IntentRecord::default() },
            vigil: fauna.percepts.get(&id).map(|p| p.vigil).unwrap_or_default(),
            hunt: a.hunt,
            yielding: a.yielding,
            bed_until: a.doze.bed_until,
            steer: fauna.steer.get(&id).copied(),
            rove: fauna.rove.get(&id).cloned(),
            forage: fauna.forage.get(&id).copied(),
            mother_seen: fauna.mother_seen.get(&id).copied(),
            route_flee: fauna.route_flee.get(&id).copied(),
            walk_targets: fauna.walk_targets.get(&id).copied(),
            walk_pending: fauna.walk_pending.get(&id).copied(),
            walk_shun: fauna.walk_shun.get(&id).cloned(),
        }
    }

    /// Its memory into the copy's body `i`.
    fn inject(&self, copy: &mut Fauna, i: usize) {
        let id = copy.animals[i].id;
        let a = &mut copy.animals[i];
        a.intent = self.record;
        a.hunt = self.hunt;
        a.yielding = self.yielding;
        a.doze.bed_until = self.bed_until;
        if let Some(p) = copy.percepts.get_mut(&id) {
            p.vigil = self.vigil;
        }
        put(&mut copy.steer, id, self.steer);
        put(&mut copy.rove, id, self.rove.clone());
        put(&mut copy.forage, id, self.forage);
        put(&mut copy.mother_seen, id, self.mother_seen);
        put(&mut copy.route_flee, id, self.route_flee);
        put(&mut copy.walk_targets, id, self.walk_targets);
        put(&mut copy.walk_pending, id, self.walk_pending);
        put(&mut copy.walk_shun, id, self.walk_shun.clone());
    }

    /// Its memory back from the copy's body `i` after the sample.
    fn extract(&mut self, copy: &Fauna, i: usize) {
        let a = &copy.animals[i];
        let id = a.id;
        self.record = a.intent;
        self.hunt = a.hunt;
        self.yielding = a.yielding;
        self.bed_until = a.doze.bed_until;
        if let Some(p) = copy.percepts.get(&id) {
            self.vigil = p.vigil;
        }
        self.steer = copy.steer.get(&id).copied();
        self.rove = copy.rove.get(&id).cloned();
        self.forage = copy.forage.get(&id).copied();
        self.mother_seen = copy.mother_seen.get(&id).copied();
        self.route_flee = copy.route_flee.get(&id).copied();
        self.walk_targets = copy.walk_targets.get(&id).copied();
        self.walk_pending = copy.walk_pending.get(&id).copied();
        self.walk_shun = copy.walk_shun.get(&id).cloned();
    }
}

/// **The dry teachers of a layer**: which lineages run one, each body's teacher and memory,
/// and this stage's observations. Session state: not saved, and a copy of the layer starts
/// without any (as its controllers do).
#[derive(Default)]
pub(crate) struct DryTeachers {
    scope: [Option<DryScope>; Founder::COUNT],
    bodies: BTreeMap<u64, DryBody>,
    /// The observations of this stage's dry bodies, read beside their controllers.
    captured: Vec<(u64, Vec<f64>)>,
    /// In a copy: the observation each dry body reads instead of sensing again.
    given: FxHashMap<u64, Vec<f64>>,
}

impl Clone for DryTeachers {
    fn clone(&self) -> Self {
        DryTeachers::default()
    }
}

impl std::fmt::Debug for DryTeachers {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DryTeachers").field("scope", &self.scope).field("bodies", &self.bodies.len()).finish()
    }
}

impl DryTeachers {
    pub(crate) fn set(&mut self, founder: Founder, scope: Option<DryScope>) {
        self.scope[founder.index()] = scope;
        if scope.is_none() {
            self.bodies.retain(|_, b| b.founder != founder);
        }
    }

    fn on(&self) -> bool {
        self.scope.iter().any(Option::is_some)
    }

    /// Whether a body of `founder` driven by a controller of `layer` gets a dry teacher.
    pub(crate) fn wants(&self, founder: Founder, layer: Layer) -> bool {
        match self.scope[founder.index()] {
            Some(DryScope::Brains) => layer == Layer::Brain,
            Some(DryScope::Every) => true,
            None => false,
        }
    }

    pub(crate) fn capture(&mut self, observations: Vec<(u64, Vec<f64>)>) {
        self.captured.extend(observations);
    }

    /// The observation a copy's dry body reads instead of sensing.
    pub(crate) fn given(&self, id: u64) -> Option<&[f64]> {
        if self.given.is_empty() {
            return None;
        }
        self.given.get(&id).map(Vec::as_slice)
    }
}

/// Before the controller stage: a copy of the layer as it stands, when a dry teacher runs.
pub(crate) fn before(fauna: &mut Fauna) -> Option<Fauna> {
    if !fauna.dry.on() {
        return None;
    }
    // The tap's recording is not copied (it only grows); the copy gets its own.
    let tap = fauna.tap.take();
    let copy = fauna.clone();
    fauna.tap = tap;
    Some(copy)
}

/// After the controller stage: run this stage's dry bodies' teachers on `copy` (the layer
/// as it stood before the stage) and record what they decided.
pub(crate) fn after(
    fauna: &mut Fauna,
    mut copy: Fauna,
    view: &cubarium_voxel::VoxelView<'_>,
    flora: &cubarium_voxel_flora::Flora,
    sky: &crate::step::Sky,
    threads: usize,
) {
    let captured = std::mem::take(&mut fauna.dry.captured);
    let live: std::collections::BTreeSet<u64> = fauna.animals.iter().map(|a| a.id).collect();
    fauna.dry.bodies.retain(|id, _| live.contains(id));
    if captured.is_empty() {
        return;
    }
    let mut ids = Vec::with_capacity(captured.len());
    for (id, obs) in captured {
        let Ok(i) = copy.animals.binary_search_by_key(&id, |a| a.id) else { continue };
        let Some(founder) = copy.animals[i].founder else { continue };
        let body = fauna.dry.bodies.entry(id).or_insert_with(|| DryBody::new(&copy, i, founder));
        body.inject(&mut copy, i);
        let Some(teacher) = body.teacher.take() else { continue };
        copy.controllers.set(id, teacher);
        copy.dry.given.insert(id, obs);
        ids.push(id);
    }
    copy.tap = Some(crate::ActionTap::default());
    crate::step::controllers(&mut copy, view, flora, None, sky, threads, Some(&ids));
    let tap = copy.tap.take().unwrap_or_default();
    for id in ids {
        let Ok(i) = copy.animals.binary_search_by_key(&id, |a| a.id) else { continue };
        let Some(body) = fauna.dry.bodies.get_mut(&id) else { continue };
        body.teacher = copy.controllers.take(id);
        body.extract(&copy, i);
        let (Some(sample), Some(real)) = (tap.stream(id).and_then(<[_]>::last), fauna.tap.as_mut()) else {
            continue;
        };
        if sample.tick != fauna.tick {
            continue;
        }
        let rec = copy.animals[i].intent;
        let rows = real
            .decisions(id)
            .and_then(<[_]>::last)
            .filter(|d| d.tick == sample.tick)
            .map(|d| (d.prep.candidates.row_of(rec.proposal_subject), d.prep.candidates.row_of(rec.subject)));
        let (proposal_row, held_row) = rows.unwrap_or((None, None));
        // As the tap finalizes a sample after the act: a held forward past 1 is a sprint
        // (`ActionTap::gait`; the dry teacher's body never runs its stamina gate).
        let mut executed = sample.executed;
        let top = crate::gait::sprint_forward(&copy.config, body.founder);
        let forward = executed.actions.forward;
        executed.body.sprint = if top > 1.0 { ((forward - 1.0) / (top - 1.0)).clamp(0.0, 1.0) } else { 0.0 };
        executed.actions.forward = forward.min(1.0);
        real.push_teacher(
            id,
            body.founder,
            TeacherLabel {
                tick: sample.tick,
                proposal: rec.proposal,
                proposal_subject: rec.proposal_subject,
                proposal_row,
                held: rec.held,
                subject: rec.subject,
                held_row,
                steer: sample.steer,
                own: sample.own,
                executed,
            },
        );
    }
}
