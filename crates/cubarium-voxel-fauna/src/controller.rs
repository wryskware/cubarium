//! **The phase-one controller boundary** — the one seam between a body and whatever
//! drives it (`design/voxel-senses-phase1-plan.md`, "Phase-one body and action contract").
//!
//! A [`Controller`] receives **only the observation vector and its own private memory**
//! and answers with the local and body actions. It sees no world, no flora stock, no site,
//! no heading and no clock: every world-derived quantity reaches it inside the vector, in
//! the order the founder's [`Manifest`](crate::Manifest) declares, and nothing in the
//! resolve path below can hand a policy a privileged target. That is the policy boundary
//! the tests plan's §1 "Policy boundary" row names, and it is enforced by shape: there is
//! no other input.
//!
//! Two kinds of controller are interchangeable behind the trait:
//!
//! - **Diagnostic controllers** (the observation-only heuristics; [`Scripted`] here for
//!   tests)
//!   return [`Response::Bounded`] — actions already inside the manifest's bounds.
//! - **A GRU policy** returns [`Response::Logits`] — the raw network outputs. The
//!   adapter ([`resolve_actions`]) applies the manifest's transfers (sigmoid for
//!   forward/feed, tanh for turn) and the fixed 0.05 deadband to logits, and the same
//!   deadband to bounded actions, **identically in training and viewing**: the arena
//!   tick, the ES episode driver and the viewer all resolve through this one function.
//!
//! The shape-aware GRU runtime itself lives in `crates/cubarium-search`: it plugs in as
//! a `Controller` whose `drive` runs one GRU forward step over the observation and wraps
//! the three outputs in [`Response::Logits`], keeping its hidden state as its private
//! memory and clearing it in `reset` when a fresh episode starts.

use serde::{Deserialize, Serialize};

use crate::TeacherConfig;
use crate::manifest::{Founder, Manifest, Transfer};

/// The three bounded local actions, in manifest order: forward effort, signed turn
/// effort, feed effort. Zero movement is rest; turning while stopped is permitted and
/// paid; a feed above the deadband attempts one local bite for the interval it is held.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Actions {
    pub forward: f64,
    pub turn: f64,
    pub feed: f64,
}

impl Actions {
    /// No action at all: what a founder holds before its controller is ever sampled.
    pub const REST: Actions = Actions {
        forward: 0.0,
        turn: 0.0,
        feed: 0.0,
    };
}

/// **The body's own actions** beyond forward, turn and feed (contract v3, proposal §2.3 B),
/// each 0 for a lineage whose manifest does not declare it. Resolved by [`resolve`] from a
/// network's logits or a bounded response, then delivered through the body constraints.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct BodyActions {
    /// Fraction of the speed margin from cruise to the lineage's sprint, before stamina.
    pub sprint: f64,
    /// Nonzero after deadband enters/holds the shield pose; zero releases it.
    pub hunker: f64,
    /// Nonzero renews the body's minimum retraction hold.
    pub retract: f64,
    /// Lure light, limited by daylight and the eating latch.
    pub lure: f64,
    /// Signed vertical flight effort.
    pub lift: f64,
}

/// Everything one response resolves to under a manifest ([`resolve`]): the three local
/// actions, the body's own, and the option the head chose (`None` without an option head).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Resolved {
    pub actions: Actions,
    pub body: BodyActions,
    pub option: Option<usize>,
}

/// What a controller answers with. The distinction is the whole adapter: bounded
/// responses are deadbanded as they are, logits are transferred first, and both paths
/// are the same code in training and viewing.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Response {
    /// Actions already inside the manifest's bounds, from a diagnostic controller.
    Bounded(Actions),
    /// Bounded local and body actions, from a teacher or diagnostic controller.
    Body(Actions, BodyActions),
    /// Unbounded logits from a network controller: the manifest's actions in its order,
    /// then its option head ([`Manifest::outputs`]); slots past those are not read. The
    /// adapter applies the manifest's transfer per action before the deadband.
    Logits([f64; crate::manifest::MAX_OUTPUTS]),
}

impl Response {
    /// Attach bounded body actions to a bounded local response.
    pub fn with_body(actions: Actions, body: BodyActions) -> Response {
        Response::Body(actions, body)
    }

    /// A network's outputs as a response: `outputs` in manifest order, at most
    /// [`crate::manifest::MAX_OUTPUTS`] (more is a shape error in the caller).
    pub fn logits(outputs: &[f64]) -> Response {
        assert!(
            outputs.len() <= crate::manifest::MAX_OUTPUTS,
            "{} network outputs exceed the response's {}",
            outputs.len(),
            crate::manifest::MAX_OUTPUTS
        );
        // An absent action is refused by the adapter, rather than becoming a half
        // effort through sigmoid(0). In particular, three-output diagnostic nets
        // must not hunker or sprint while the nine-shape runtime is being wired.
        let mut l = [f64::MIN; crate::manifest::MAX_OUTPUTS];
        l[..outputs.len()].copy_from_slice(outputs);
        Response::Logits(l)
    }
}

/// One controller: a pure function of the observation and its own private state. The
/// observation slice is guaranteed finite by the sampler that builds it; a controller
/// must not need more than `manifest.inputs()` of it, and must not get anything else.
///
/// Controllers are `Send + Sync` because the layer they sit in is a bevy resource and
/// an episode worker's payload: one body's controller is driven from one tick at a
/// time, but the table itself has to move between threads.
pub trait Controller: Send + Sync {
    /// Answer one observation sample, once per controller period. The only inputs are
    /// the observation vector and this controller's own private memory (`&mut self`).
    fn drive(&mut self, observation: &[f64]) -> Response;

    /// Drop all private memory. A fresh episode starts from here: hidden state is
    /// episode-private and never carried across one.
    fn reset(&mut self);

    /// Whether only the next flyer observation's Taste channels can affect the answer.
    /// Returning true promises to ignore every other channel. Policies and diagnostic
    /// controllers receive the complete observation by default.
    fn taste_only(&self) -> bool {
        false
    }

    /// The declared reflex layer. Brains and diagnostic scripts receive only body
    /// constraints; the live teachers opt into the complete heuristic layer.
    fn layer(&self) -> Layer {
        Layer::Brain
    }

    /// Whether this controller's body is **at a carcass** after its last [`drive`]: eating
    /// carrion at its mouth. The lanternjaw's heuristic answers it, and the tick's carcass
    /// yield (roster R3) reads it; every other controller answers `false`.
    ///
    /// [`drive`]: Controller::drive
    fn at_carcass(&self) -> bool {
        false
    }

    /// HUNGRY-SEARCH: what this controller's last answer was for ([`Seek`]), read by the
    /// tick's hungry search (`step::rove`). The foraging heuristics answer it; every other
    /// controller — a trained policy, a hunter, a test's script — answers
    /// [`Seek::Unknown`], and the search leaves it alone.
    fn seek(&self) -> Seek {
        Seek::Unknown
    }

    /// **Whether this controller reads the v3 blocks** (contract v3) — the slots after its
    /// manifest's v2 layout ([`Manifest::tail_offset`]). A policy does, so the default is
    /// yes; the observation-only heuristics read only their v2 slots and answer no, and the
    /// step then leaves their v3 blocks zero (unless the executed-action tap records them).
    /// Encoding reads only, so the answer changes the cost, never the world.
    fn reads_tail(&self) -> bool {
        true
    }
}

/// **The reflex contract** (v3 B, P4). Both layers retain sleep (gliders only
/// perched), DROP, stamina, eating latch, pup strike refusal, sunlit lure refusal,
/// TRAPS and LEADS-PAY, plus the body's motion, wounds and retraction physics.
/// Only `Heuristic` adds flee/refuge/hunker choice, herding, search, fruit walks,
/// roosts, shallows, snail trips, fed perching, nests, light steering, patrol,
/// carcass yield, pack tactics and stop-and-go cadence. Hybrid options are P8.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Layer {
    Heuristic,
    #[default]
    Brain,
}

/// HUNGRY-SEARCH: what a foraging heuristic's last answer was for ([`Controller::seek`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Seek {
    /// Not a forager's answer, or none yet.
    #[default]
    Unknown,
    /// Food at the mouth: it stops to feed (or tastes and bites).
    Taste,
    /// Steering by food it senses but cannot yet bite: food in view, a scent rising or
    /// falling, the turn back toward a patch just left.
    Lead,
    /// Its own search: nothing to eat in sense and nothing in the way (the no-signal
    /// wander, or a scent with no gradient to follow).
    Search,
    /// In the way: an escape from a refused motor or a drop, a blocked turn, a push out.
    Obstacle,
}

/// The shared action adapter: transfer, clamp, deadband — in that order, identically
/// for training and viewing. Non-finite logits and out-of-range bounded actions are
/// rejected to the deadband's zero rather than trained through.
pub fn resolve_actions(response: Response, manifest: &Manifest) -> Actions {
    resolve(response, manifest).actions
}

/// **The adapter per manifest action** (contract v3): every action the manifest declares is
/// transferred (logits), clamped into its bounds and deadbanded — the `[0, 1]` ones below
/// the deadband to 0, the signed ones on their magnitude — and routed by its name; the
/// option head's logits follow the actions, and the option is the largest (the first on a
/// tie; a non-finite logit never wins). Bounded responses choose no option; a plain
/// `Bounded` response has zero body actions.
pub fn resolve(response: Response, manifest: &Manifest) -> Resolved {
    let deadband = manifest.deadband;
    let mut out = Resolved::default();
    let mut put = |name: &str, v: f64| match name {
        "forward_effort" => out.actions.forward = v,
        "turn_effort" => out.actions.turn = v,
        "feed_effort" => out.actions.feed = v,
        "sprint_effort" => out.body.sprint = v,
        "hunker_effort" => out.body.hunker = v,
        "retract_effort" => out.body.retract = v,
        "lure_effort" => out.body.lure = v,
        "lift_effort" => out.body.lift = v,
        other => panic!("manifest action {other} has no slot in the response"),
    };
    let band = |v: f64, t: Transfer| match t {
        Transfer::Sigmoid => deadband_low(v, deadband),
        Transfer::Tanh => deadband_sym(v, deadband),
    };
    match response {
        Response::Bounded(a) | Response::Body(a, _) => {
            let b = match response {
                Response::Body(_, b) => b,
                _ => BodyActions::default(),
            };
            for act in manifest.actions {
                let v = match act.name {
                    "forward_effort" => a.forward,
                    "turn_effort" => a.turn,
                    "feed_effort" => a.feed,
                    "sprint_effort" => b.sprint,
                    "hunker_effort" => b.hunker,
                    "retract_effort" => b.retract,
                    "lure_effort" => b.lure,
                    "lift_effort" => b.lift,
                    other => panic!("manifest action {other} has no bounded response slot"),
                };
                // Plain bounded actions historically leave body slots entirely zero.
                put(act.name, band(clean(v, act.low, act.high), act.transfer));
            }
        }
        Response::Logits(l) => {
            for (k, act) in manifest.actions.iter().enumerate() {
                put(act.name, band(transfer(l[k], act.transfer).clamp(act.low, act.high), act.transfer));
            }
            let head = &l[manifest.actions.len()..manifest.outputs()];
            out.option = head
                .iter()
                .enumerate()
                .filter(|(_, v)| v.is_finite() && **v != f64::MIN)
                .fold(None, |best: Option<(usize, f64)>, (k, &v)| match best {
                    Some((_, b)) if b >= v => best,
                    _ => Some((k, v)),
                })
                .map(|(k, _)| k);
        }
    }
    out
}

/// A finite value clamped into the action's declared bounds; anything non-finite is 0.
fn clean(v: f64, low: f64, high: f64) -> f64 {
    if !v.is_finite() {
        return 0.0;
    }
    v.clamp(low, high)
}

/// One manifest transfer; non-finite or absent (`f64::MIN`) in, zero out.
fn transfer(logit: f64, t: Transfer) -> f64 {
    if !logit.is_finite() || logit == f64::MIN {
        return 0.0;
    }
    match t {
        Transfer::Sigmoid => 1.0 / (1.0 + (-logit).exp()),
        Transfer::Tanh => logit.tanh(),
    }
}

/// Forward and feed deadband: anything below it is no action.
fn deadband_low(v: f64, deadband: f64) -> f64 {
    if v < deadband { 0.0 } else { v }
}

/// Turn deadband, on the absolute value, keeping the sign.
fn deadband_sym(v: f64, deadband: f64) -> f64 {
    if v.abs() < deadband { 0.0 } else { v }
}

/// The controller table: one controller per founder body that has one, keyed by animal
/// id in id order. Controller memory is **driver-owned session state, not world
/// state**: it is skipped by the fauna snapshot (a loaded layer's founders rest until
/// their driver installs controllers again), and cloning a fauna starts the copy
/// without it — cloning a body does not clone a mind.
#[derive(Default)]
pub struct FounderControllers {
    entries: Vec<(u64, Box<dyn Controller>)>,
}

impl FounderControllers {
    /// Install or replace one body's controller, keeping the id order.
    pub fn set(&mut self, id: u64, controller: Box<dyn Controller>) {
        match self.entries.binary_search_by_key(&id, |e| e.0) {
            Ok(at) => self.entries[at].1 = controller,
            Err(at) => self.entries.insert(at, (id, controller)),
        }
    }

    /// Take a body's controller back, if it has one.
    pub fn take(&mut self, id: u64) -> Option<Box<dyn Controller>> {
        let at = self.entries.binary_search_by_key(&id, |e| e.0).ok()?;
        Some(self.entries.remove(at).1)
    }

    /// Drive one body's controller with an observation, if it has one.
    pub fn drive(&mut self, id: u64, observation: &[f64]) -> Option<Response> {
        let at = self.entries.binary_search_by_key(&id, |e| e.0).ok()?;
        Some(self.entries[at].1.drive(observation))
    }

    pub(crate) fn taste_only(&self, id: u64) -> bool {
        self.entries.binary_search_by_key(&id, |e| e.0)
            .is_ok_and(|at| self.entries[at].1.taste_only())
    }

    /// Each of `ids`' controllers, mutably and at once (`None` for a body without one),
    /// in `ids`' order: the controller stage drives them side by side. `ids` are distinct.
    pub(crate) fn slots_mut(&mut self, ids: &[u64]) -> Vec<Option<&mut Box<dyn Controller>>> {
        let mut want: Vec<(usize, usize)> = ids
            .iter()
            .enumerate()
            .filter_map(|(k, id)| {
                self.entries.binary_search_by_key(id, |e| e.0).ok().map(|at| (at, k))
            })
            .collect();
        want.sort_unstable();
        let mut slots: Vec<Option<&mut Box<dyn Controller>>> = ids.iter().map(|_| None).collect();
        let mut next = want.into_iter().peekable();
        for (at, entry) in self.entries.iter_mut().enumerate() {
            if next.peek().is_some_and(|&(w, _)| w == at) {
                let (_, k) = next.next().expect("peeked");
                slots[k] = Some(&mut entry.1);
            }
        }
        slots
    }

    /// Whether a body's controller reads its v3 blocks ([`Controller::reads_tail`]); false
    /// with no controller.
    pub fn reads_tail(&self, id: u64) -> bool {
        self.entries
            .binary_search_by_key(&id, |e| e.0)
            .ok()
            .is_some_and(|at| self.entries[at].1.reads_tail())
    }

    /// The body's declared layer; an absent controller has no heuristic reflexes.
    pub fn layer(&self, id: u64) -> Layer {
        self.entries
            .binary_search_by_key(&id, |e| e.0)
            .ok()
            .map_or(Layer::Brain, |at| self.entries[at].1.layer())
    }

    /// What a body's controller's last answer was for ([`Controller::seek`]).
    pub fn seek(&self, id: u64) -> Seek {
        self.entries
            .binary_search_by_key(&id, |e| e.0)
            .ok()
            .map_or(Seek::Unknown, |at| self.entries[at].1.seek())
    }

    /// Whether a body's controller is at a carcass ([`Controller::at_carcass`]).
    pub fn at_carcass(&self, id: u64) -> bool {
        self.entries
            .binary_search_by_key(&id, |e| e.0)
            .ok()
            .is_some_and(|at| self.entries[at].1.at_carcass())
    }

    /// Whether this body has a controller.
    pub fn has(&self, id: u64) -> bool {
        self.entries.binary_search_by_key(&id, |e| e.0).is_ok()
    }

    /// How many bodies currently have a controller.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

impl Clone for FounderControllers {
    /// Controller memory is not world state and is not cloned: the copy starts
    /// mindless, the same way a snapshot does. Installing fresh controllers is what a
    /// fresh episode does.
    fn clone(&self) -> Self {
        FounderControllers::default()
    }
}

impl std::fmt::Debug for FounderControllers {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let ids: Vec<u64> = self.entries.iter().map(|e| e.0).collect();
        f.debug_tuple("FounderControllers").field(&ids).finish()
    }
}

/// **How to build a fresh controller of one founder kind.** A recipe, not a mind: the
/// factory is asked for a *new* controller with new memory every time it is used, so no
/// two bodies can ever share one.
///
/// This exists because a founder body can be **born** (`step::births`) as well as
/// introduced. A newborn inherits its parent's lineage marker and nothing else — not its
/// memory, and not its controller — so the layer has to be able to make the newborn one
/// of the parent's kind without knowing what kind of controller the driver chose. The
/// driver registers one factory per founder ([`crate::Fauna::set_founder_factory`]) and
/// every founder born after that gets a controller from it.
///
/// Any `Fn() -> Box<dyn Controller>` that is `Send + Sync` is a factory, so the ordinary
/// registration is a closure: `|| Box::new(BlindForager::new())`, or a closure over a
/// loaded policy that hands out a fresh network with a zeroed hidden state.
pub trait ControllerFactory: Send + Sync {
    /// A brand-new controller of this kind, with its own memory.
    fn make(&self) -> Box<dyn Controller>;
}

impl<F> ControllerFactory for F
where
    F: Fn() -> Box<dyn Controller> + Send + Sync,
{
    fn make(&self) -> Box<dyn Controller> {
        self()
    }
}

/// One registered [`ControllerFactory`] per founder lineage, in [`Founder::index`] order.
///
/// Unlike [`FounderControllers`], this **is** cloned with the layer: a factory is the
/// recipe for a mind and not a mind, so a copy of a layer that breeds still knows what to
/// give its newborns. It is still skipped by the snapshot — a recipe is not world state
/// and a `dyn Fn` does not serialize — so a loaded layer's driver registers factories
/// again, which is the same contract the controllers themselves have.
#[derive(Clone, Default)]
pub struct FounderFactories {
    per_founder: [Option<std::sync::Arc<dyn ControllerFactory>>; Founder::COUNT],
}

impl FounderFactories {
    /// Register (or replace) the factory for one lineage.
    pub fn set(&mut self, founder: Founder, factory: std::sync::Arc<dyn ControllerFactory>) {
        self.per_founder[founder.index()] = Some(factory);
    }

    /// Forget one lineage's factory. Bodies of that kind born afterwards get no
    /// controller and rest, exactly as an unregistered lineage's always did.
    pub fn clear(&mut self, founder: Founder) {
        self.per_founder[founder.index()] = None;
    }

    /// Whether this lineage has a factory registered.
    pub fn has(&self, founder: Founder) -> bool {
        self.per_founder[founder.index()].is_some()
    }

    /// A fresh controller for this lineage, or `None` when none is registered.
    pub fn make(&self, founder: Founder) -> Option<Box<dyn Controller>> {
        self.per_founder[founder.index()].as_ref().map(|f| f.make())
    }
}

impl std::fmt::Debug for FounderFactories {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let registered: Vec<&'static str> = Founder::ALL
            .into_iter()
            .filter(|founder| self.has(*founder))
            .map(Founder::name)
            .collect();
        f.debug_tuple("FounderFactories")
            .field(&registered)
            .finish()
    }
}

/// A diagnostic controller that cycles a fixed script of already-bounded actions, one
/// per drive. It is the plumbing test's controller — motion, contact and feeding
/// behaviour are driven with it — and not a foraging heuristic: it reads no senses.
#[derive(Clone, Debug, Default)]
pub struct Scripted {
    script: Vec<Actions>,
    next: usize,
    lift: f64,
}

impl Scripted {
    /// Cycle `script` forever; an empty script rests.
    pub fn new(script: Vec<Actions>) -> Scripted {
        Scripted {
            script,
            next: 0,
            lift: 0.0,
        }
    }

    /// The same script holding a constant `lift` effort ([`Controller::lift`]) beside it:
    /// a flyer's test control (F1).
    pub fn lifting(mut self, lift: f64) -> Scripted {
        self.lift = lift;
        self
    }
}

impl Controller for Scripted {
    fn drive(&mut self, _observation: &[f64]) -> Response {
        if self.script.is_empty() {
            return if self.lift == 0.0 { Response::Bounded(Actions::REST) } else {
                Response::with_body(Actions::REST, BodyActions { lift: self.lift, ..BodyActions::default() })
            };
        }
        let action = self.script[self.next % self.script.len()];
        self.next += 1;
        if self.lift == 0.0 { Response::Bounded(action) } else {
            Response::with_body(action, BodyActions { lift: self.lift, ..BodyActions::default() })
        }
    }

    fn reset(&mut self) {
        self.next = 0;
    }
}

/// The named module a controller reads from its observation by manifest slot.
fn module_slot(manifest: &Manifest, name: &str) -> crate::manifest::Module {
    *manifest
        .modules
        .iter()
        .find(|m| m.name == name)
        .unwrap_or_else(|| panic!("manifest has no {name} module"))
}

// The teachers' reflexes (escape, correlated walk, feed gate, ambush) are the knobs of
// [`TeacherConfig`](crate::TeacherConfig): heuristic internals, not model numbers — they
// shape a teacher's behaviour, and the live animals run on them until a policy is
// installed.

/// A **correlated walk** for when a teacher has no signal: hold the heading for a random
/// number of samples, then turn by a bounded random angle (a random number of samples at
/// a fixed effort, either way), and hold again. The randomness is the controller's own
/// memory — seeded from the bits of the first observation it is handed, so two bodies
/// with different senses walk different walks — and nothing here reads a coordinate.
/// Blocks and hazards are the caller's escape and contact rules, which run first.
#[derive(Clone, Debug, Default)]
struct Wander {
    state: u64,
    seeded: bool,
    /// Samples left holding the heading.
    hold: u32,
    /// Samples left turning, and which way.
    turning: u32,
    sign: f64,
    /// How many leading observation channels the seed folds (0: all): the lineage's v2
    /// layout ([`Manifest::tail_offset`]), so a walk's seed is the same whether or not the
    /// v3 blocks were encoded after it.
    seed_len: usize,
}

impl Wander {
    /// A fresh walk whose seed folds the first `seed_len` channels.
    fn over(seed_len: usize) -> Wander {
        Wander { seed_len, ..Wander::default() }
    }

    fn next(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        cubarium_voxel::mix(self.state)
    }

    fn draw(&mut self, (lo, hi): (u32, u32)) -> u32 {
        lo + (self.next() % u64::from(hi - lo + 1)) as u32
    }

    fn hold(&mut self, cfg: &TeacherConfig) -> u32 {
        self.draw((cfg.wander_hold_min, cfg.wander_hold_max))
    }

    /// Seed from the observation on the first call after a reset.
    fn seed(&mut self, o: &[f64], cfg: &TeacherConfig) {
        if self.seeded {
            return;
        }
        self.seeded = true;
        let seen = if self.seed_len == 0 { o } else { &o[..self.seed_len.min(o.len())] };
        self.state = seen.iter().fold(0x5EED_0F_A11C_E5u64, |h, v| {
            (h ^ v.to_bits()).wrapping_mul(0x0100_0000_01B3)
        });
        self.hold = self.hold(cfg);
    }

    /// This sample's wander turn effort.
    fn turn(&mut self, o: &[f64], cfg: &TeacherConfig) -> f64 {
        self.seed(o, cfg);
        if self.turning > 0 {
            self.turning -= 1;
            if self.turning == 0 {
                self.hold = self.hold(cfg);
            }
            return self.sign * cfg.wander_turn;
        }
        if self.hold > 0 {
            self.hold -= 1;
            return 0.0;
        }
        self.sign = if self.next() & 1 == 0 { 1.0 } else { -1.0 };
        self.turning = self.draw((cfg.wander_turn_samples_min, cfg.wander_turn_samples_max)) - 1;
        if self.turning == 0 {
            self.hold = self.hold(cfg);
        }
        self.sign * cfg.wander_turn
    }

    /// Whichever way the walk last turned: the side an escape turns to when nothing else
    /// picks one.
    fn side(&self) -> f64 {
        if self.sign == 0.0 { 1.0 } else { self.sign }
    }

    /// A block or a signal interrupts the walk: hold again afterwards.
    fn interrupt(&mut self, cfg: &TeacherConfig) {
        self.turning = 0;
        if self.seeded {
            self.hold = self.hold(cfg);
        }
    }
}

/// The escape reflex both teachers share: returns `Some((forward, turn))` while an
/// escape is running, arming it when `hazard` is seen.
fn escape(cfg: &TeacherConfig, escape: &mut u32, hazard: bool, turn: f64) -> Option<(f64, f64)> {
    if hazard {
        *escape = cfg.escape_samples;
        return Some((0.0, turn));
    }
    if *escape > 0 {
        *escape -= 1;
        return Some((cfg.escape_creep, turn));
    }
    None
}

/// **Each lineage's own observation-only heuristic**, on the layer's reflex numbers
/// (`teachers`, [`crate::FaunaConfig::teachers`]): the one place a founder is mapped to
/// its heuristic. The seeder's birth factories, an introduced body's controller, the
/// search's bystanders and its heuristic control all build from here.
pub fn heuristic(founder: Founder, teachers: TeacherConfig) -> Box<dyn Controller> {
    match founder {
        Founder::Blind => Box::new(BlindForager::with_config(teachers)),
        Founder::Browser => Box::new(BrowserForager::with_config(teachers)),
        // The ambush teacher (predation §2.12): it has no trained centre, so this drives
        // every lanternjaw, hatched or introduced.
        // The detritus web D2: the patrol over it (`crate::hunt`), while the world's
        // `hunt.patrol` is on.
        Founder::Lanternjaw => Box::new(LanternjawPatrol::with_config(teachers)),
        // Roster R1: the shredder's forager on the capgnawer's own `Chem(fungal)`, and the
        // browser's forager on the loftstrider's banded cone and high mouth.
        Founder::Capgnawer => Box::new(BlindForager::for_founder(Founder::Capgnawer, teachers)),
        Founder::Loftstrider => {
            Box::new(BrowserForager::for_founder(Founder::Loftstrider, teachers))
        }
        // Roster R2b: the blind forager on the ripple-snail's own `Chem(film)`.
        Founder::RippleSnail => {
            Box::new(BlindForager::for_founder(Founder::RippleSnail, teachers))
        }
        // F1: the hover-sip teacher.
        Founder::Bellwing => Box::new(HoverSip::with_config(teachers)),
        // F3: the seedporter's fruit forager (the body climbs, perches and glides).
        Founder::Seedporter => Box::new(FruitForager::with_config(teachers)),
        // R3: the chorister's pack hunter — the lanternjaw's ambush without a lure, under
        // the step's scripted pack tactic (`crate::pack`).
        Founder::Chorister => Box::new(PackHunter::with_config(teachers)),
    }
}

/// Observation-only **blind foraging** heuristic: go up the litter cue's response and
/// trend while they support it, feed when the mouth tastes litter, turn away from contact
/// and cover ground with a remembered, alternating turn preference when there is no
/// signal. It reads only the observation vector — never a coordinate, a site, a route or
/// a stock total (`design/voxel-senses-phase1-tests.md` §2's observation-only control).
///
/// Since contract v2 it **backs off a refused motor**: when `Self.motor_delivery` says
/// most of what it asked for was not delivered, it stops pushing and turns away for a
/// few samples, rather than holding full forward against a drop.
#[derive(Clone, Debug)]
pub struct BlindForager {
    /// The remembered turn preference a falling cue turns by: the controller's own
    /// memory, never a world direction.
    turn_bias: f64,
    /// The no-signal search (S3).
    wander: Wander,
    /// Samples left in the current escape.
    escape: u32,
    /// Whether the mouth tasted food on the last sample (stay on food), and the samples
    /// left of the turn back after leaving an exhausted patch.
    on_food: bool,
    turn_back: u32,
    contact: crate::manifest::Module,
    taste: crate::manifest::Module,
    chem: crate::manifest::Module,
    /// What the last answer was for (HUNGRY-SEARCH, [`Controller::seek`]).
    seek: Seek,
    /// The taste it holds still to feed on: [`TeacherConfig::capgnawer_stay_worth`] for the
    /// capgnawer, whose taste reads a bite's worth; 0 (any food it feeds on) for the rest.
    stay_taste: f64,
    /// The reflexes' numbers ([`TeacherConfig`]).
    cfg: TeacherConfig,
}

impl BlindForager {
    /// The blind founder's heuristic, pre-indexed against its manifest, on the default
    /// [`TeacherConfig`].
    pub fn new() -> BlindForager {
        BlindForager::with_config(TeacherConfig::default())
    }

    /// The blind founder's heuristic on `cfg` (a world's [`crate::FaunaConfig::teachers`]).
    pub fn with_config(cfg: TeacherConfig) -> BlindForager {
        BlindForager::for_founder(Founder::Blind, cfg)
    }

    /// The same forager on any blind-layout lineage's manifest: it follows that
    /// manifest's one `Chem` module, whatever field it reads (the shredder's detritus, the
    /// capgnawer's fungal field).
    pub fn for_founder(founder: Founder, cfg: TeacherConfig) -> BlindForager {
        let manifest = founder.manifest();
        let chem = *manifest
            .modules
            .iter()
            .find(|m| m.name.starts_with("Chem("))
            .unwrap_or_else(|| panic!("the {} manifest has no Chem module", founder.name()));
        BlindForager {
            stay_taste: if founder == Founder::Capgnawer { cfg.capgnawer_stay_worth } else { 0.0 },
            cfg,
            turn_bias: 1.0,
            wander: Wander::over(manifest.tail_offset()),
            escape: 0,
            on_food: false,
            turn_back: 0,
            contact: module_slot(&manifest, "Contact(4)"),
            taste: module_slot(&manifest, "Taste(1)"),
            chem,
            seek: Seek::Unknown,
        }
    }
}

impl Default for BlindForager {
    fn default() -> Self {
        BlindForager::new()
    }
}

impl Controller for BlindForager {
    fn reads_tail(&self) -> bool {
        false
    }

    fn drive(&mut self, o: &[f64]) -> Response {
        let c = &self.contact;
        let (front, left, right) = (o[c.offset], o[c.offset + 1], o[c.offset + 2]);
        let (taste_resp, taste_valid) = (o[self.taste.offset], o[self.taste.offset + 2]);
        let (chem_resp, chem_trend, chem_valid) = (
            o[self.chem.offset],
            o[self.chem.offset + 1],
            o[self.chem.offset + 2],
        );

        // On a terrain face: nothing is under the body — on the
        // ground there always is. Keep climbing at full effort: the turn is not read on a
        // face, and slowing for "a wall ahead" would only leave it hanging there.
        let (underside, contact_valid) = (o[c.offset + 3], o[c.offset + 4]);
        if contact_valid > 0.5 && underside < 0.5 {
            self.seek = Seek::Unknown;
            return Response::Bounded(Actions {
                forward: 1.0,
                turn: 0.0,
                feed: 0.0,
            });
        }
        let cfg = self.cfg;
        let front_blocked = front > 0.5;
        let forward = if front_blocked { cfg.blocked_forward } else { 1.0 };
        // Feed on food the mouth tastes above `TeacherConfig::feed_gate`. Taste uses
        // the lineage's food-unit reference, independent of body mass; a film-only
        // eater without an override uses film capacity per site. Failed attempts off
        // the stock are free — the gate costs nothing but keeps feeding honest.
        let feed = if taste_valid > 0.5 && taste_resp > cfg.feed_gate {
            1.0
        } else {
            0.0
        };
        // A refused motor: stop pushing and turn away — from the contacted side if there
        // is one, else the remembered way.
        let away = if left > right {
            -1.0
        } else if right > left {
            1.0
        } else {
            self.wander.side()
        };
        if let Some((forward, turn)) = escape(&cfg, &mut self.escape, o[7] < cfg.motor_refused, away) {
            self.wander.interrupt(&cfg);
            self.on_food = false;
            self.seek = Seek::Obstacle;
            return Response::Bounded(Actions {
                forward,
                turn,
                feed,
            });
        }
        // **Stay on food** (the detritus web, §3.1A): tasting food, it holds still and
        // feeds; the face under its mouth falling below the gate is an exhausted patch,
        // and it walks on with a short turn back toward where the food was. Food under its
        // stay taste (the capgnawer's litter, CAPGNAWER-CAPS) it eats on the move: the
        // search below steers, with `feed` on.
        if feed > 0.5 && taste_resp >= self.stay_taste {
            self.on_food = true;
            self.turn_back = 0;
            self.wander.interrupt(&cfg);
            self.seek = Seek::Taste;
            return Response::Bounded(Actions {
                forward: cfg.forage_stay_forward.clamp(0.0, 1.0),
                turn: 0.0,
                feed,
            });
        }
        if std::mem::take(&mut self.on_food) {
            self.turn_back = cfg.forage_turn_back_samples;
        }
        if self.turn_back > 0 && !front_blocked {
            self.turn_back -= 1;
            self.wander.interrupt(&cfg);
            self.seek = Seek::Lead;
            return Response::Bounded(Actions {
                forward,
                turn: self.turn_bias * cfg.blind_turn,
                feed,
            });
        }
        let turn = if front_blocked {
            // A wall is a physical feature, not a target: turn away from the contacted
            // side, or fall back on the walk's last side.
            self.wander.interrupt(&cfg);
            self.seek = Seek::Obstacle;
            if left > right {
                -1.0
            } else if right > left {
                1.0
            } else {
                self.wander.side() * cfg.blind_turn
            }
        } else if chem_valid > 0.5 && chem_resp >= cfg.chem_response_gate {
            self.wander.interrupt(&cfg);
            // A scent with a gradient is a lead; one without (a flat trend) is not, and
            // the hungry search may take over there (HUNGRY-SEARCH: capgnawer juveniles
            // in a fungal scent held one heading between two walls that turned them back,
            // for minutes).
            self.seek = if chem_trend.abs() > cfg.chem_trend_gate { Seek::Lead } else { Seek::Search };
            if chem_trend > cfg.chem_trend_gate {
                // A rising signal: hold the heading the body already faces.  The
                // Stage-A arena deliberately starts aimed at the food; even the old
                // small steering term bent that direct approach into a circle before
                // the mouth could reach the patch.
                0.0
            } else if chem_trend < -cfg.chem_trend_gate {
                // A falling signal: change course to a remembered new preference.
                -self.turn_bias * cfg.blind_turn
            } else {
                // The initial chemical sample has no temporal history, hence a flat
                // trend.  It still means "continue this observable approach", not
                // "begin a remembered search arc".
                0.0
            }
        } else {
            // No signal: cover ground with a correlated walk (S3).
            self.seek = Seek::Search;
            self.wander.turn(o, &cfg)
        };
        Response::Bounded(Actions {
            forward,
            turn,
            feed,
        })
    }

    fn reset(&mut self) {
        self.turn_bias = 1.0;
        self.wander = Wander::over(self.wander.seed_len);
        self.escape = 0;
        self.on_food = false;
        self.turn_back = 0;
        self.seek = Seek::Unknown;
    }

    fn layer(&self) -> Layer {
        Layer::Heuristic
    }

    fn seek(&self) -> Seek {
        self.seek
    }
}

/// Observation-only **sighted browser** heuristic: gaze at the cone sector with the most
/// foliage, hold a centre-sector heading, creep around front contact, and feed only when
/// the mouth's taste reports foliage. No coordinate, site, route or stock total is read.
///
/// Since contract v2 it **turns away from a hazard** before trying anything else: a
/// refused motor, or a forward sector whose downward rays read a drop — more of them
/// clear than flat ground leaves clear, or the floor rows' hits far off. Water in a
/// hollow reads the same way, its surface being below the bank.
#[derive(Clone, Debug)]
pub struct BrowserForager {
    /// The remembered wander preference, flipped every three no-signal samples; also the
    /// side an escape turns to. The browser keeps this search: the correlated walk (S3)
    /// measured 0.846 → 0.708 on its held-out landscapes and was reverted for it.
    wander_bias: f64,
    wander_ticks: u32,
    escape: u32,
    /// JUVENILE-FOOD: radians turned on the spot while the front has stayed blocked, and
    /// the samples left of a push straight on once that reaches a full turn.
    blocked_turned: f64,
    push: u32,
    /// The manifest's `Self.delivered_turn` scale, radians per unit.
    turn_reference_rad: f64,
    contact: crate::manifest::Module,
    taste: crate::manifest::Module,
    cone: crate::manifest::Module,
    /// Whether the adult's steepest ray reaches flat ground inside the cone's range, so
    /// that the forward sector's floor rows mean "flat" ([`TeacherConfig::flat_clear`]).
    /// True for the grazer (eye 0.3 m); false for the loftstrider (eye 1.5 m, the
    /// −40° ray ends 1.29 m down), whose drop reflex is off and whose refused motor still
    /// turns it away. Geometry, not a knob.
    reads_drops: bool,
    /// What the last answer was for (HUNGRY-SEARCH, [`Controller::seek`]).
    seek: Seek,
    /// The reflexes' numbers ([`TeacherConfig`]).
    cfg: TeacherConfig,
}

impl BrowserForager {
    /// The browser founder's heuristic, pre-indexed against its manifest, on the default
    /// [`TeacherConfig`].
    pub fn new() -> BrowserForager {
        BrowserForager::with_config(TeacherConfig::default())
    }

    /// The browser founder's heuristic on `cfg` (a world's [`crate::FaunaConfig::teachers`]).
    pub fn with_config(cfg: TeacherConfig) -> BrowserForager {
        BrowserForager::for_founder(Founder::Browser, cfg)
    }

    /// The same forager on any sighted lineage's manifest (the grazer's, the
    /// loftstrider's banded cone).
    pub fn for_founder(founder: Founder, cfg: TeacherConfig) -> BrowserForager {
        let manifest = founder.manifest();
        let p = crate::FounderPhysiology::frozen(founder);
        let eye = p.eye_height_fraction * p.adult_height_m;
        let steepest = manifest
            .ray_pitch_offsets_deg
            .iter()
            .fold(0.0f64, |a, &d| a.min(d))
            .abs()
            .to_radians();
        let reads_drops = steepest > 0.0 && eye / steepest.sin() <= manifest.cone_range_m;
        BrowserForager {
            cfg,
            wander_bias: 1.0,
            wander_ticks: 0,
            escape: 0,
            blocked_turned: 0.0,
            push: 0,
            turn_reference_rad: manifest.turn_reference_rad,
            contact: module_slot(&manifest, "Contact(4)"),
            taste: module_slot(&manifest, "Taste(1)"),
            cone: module_slot(&manifest, "Cone(3, foliage/body)"),
            reads_drops,
            seek: Seek::Unknown,
        }
    }
}

impl Default for BrowserForager {
    fn default() -> Self {
        BrowserForager::new()
    }
}

impl Controller for BrowserForager {
    fn reads_tail(&self) -> bool {
        false
    }

    fn drive(&mut self, o: &[f64]) -> Response {
        let front = o[self.contact.offset];
        let taste_resp = o[self.taste.offset];
        let taste_valid = o[self.taste.offset + 2];
        let base = self.cone.offset;
        // Per sector, the foliage fraction sits at base + k*6 + 2.
        let frac = [o[base + 2], o[base + 8], o[base + 14]];
        let cfg = self.cfg;
        let feed = if taste_valid > 0.5 && taste_resp > cfg.feed_gate {
            1.0
        } else {
            0.0
        };
        // The forward sector (s1) read against flat ground: its downward rows escaping,
        // or its only hits far below.
        let (clear, proximity) = (o[base + 6], o[base + 7]);
        let drop = self.reads_drops
            && (clear > cfg.flat_clear + cfg.half_ray
                || (clear >= cfg.flat_clear - cfg.half_ray && proximity < cfg.drop_proximity));
        // JUVENILE-FOOD: **a full turn on the spot is no box**. The contact probe sits a
        // step's height up (`step_up_m`: three voxels at 0.125 m), over a small body's
        // head, so under a low overhang it touches the ceiling at every heading; and on a
        // narrow ledge the forward sector reads the drop beside the path at every heading.
        // The blocked turn and the drop escape below circled such bodies on the spot for
        // good (frondgrazer juveniles, s1 minute 63: 4 of 11 under one overhang and 3 on
        // ledges, reserve 0). So once the front (or a drop reading) has held through a
        // whole delivered turn while the motor still delivers, it walks straight on for
        // `escape_samples`: the motion itself refuses a real wall or a drop deeper than
        // the step, and that refusal turns it away through the escape.
        let refused = o[7] < cfg.motor_refused;
        if refused {
            self.push = 0;
            self.blocked_turned = 0.0;
        } else if front > 0.5 || drop {
            self.blocked_turned += o[6].abs() * self.turn_reference_rad;
        } else {
            self.blocked_turned = 0.0;
        }
        if self.push == 0 && self.blocked_turned >= std::f64::consts::TAU {
            self.blocked_turned = 0.0;
            self.push = cfg.escape_samples.max(1);
            self.escape = 0;
        }
        if self.push > 0 {
            self.push -= 1;
            self.wander_ticks = 0;
            self.seek = Seek::Obstacle;
            return Response::Bounded(Actions {
                forward: 1.0,
                turn: 0.0,
                feed,
            });
        }
        let away = self.wander_bias;
        if let Some((forward, turn)) = escape(&cfg, &mut self.escape, refused || drop, away) {
            self.wander_ticks = 0;
            self.seek = Seek::Obstacle;
            return Response::Bounded(Actions {
                forward,
                turn,
                feed,
            });
        }
        // JUVENILE-FOOD: **stay on food**, as the blind forager does. It used to walk on at
        // full forward while it cropped, so its mouth left the stand after a bite or two
        // and it tasted food on 18% of its hungry samples (juveniles, whose mouth reaches
        // least, worst; the C1 brain slows to ~0.17 and tastes it on 59%). Holding still
        // while it tastes food also makes it the cropping grazer the scan pause and the
        // giving-up rule (`crate::forage`) read; that rule walks it off a spent stand.
        if feed > 0.5 {
            self.wander_ticks = 0;
            self.seek = Seek::Taste;
            return Response::Bounded(Actions {
                forward: cfg.forage_stay_forward.clamp(0.0, 1.0),
                turn: 0.0,
                feed,
            });
        }

        let turn;
        // Gaze: turn toward the richest sector — sector 0 is −60° (left of forward), so
        // facing it needs a negative turn effort; sector 2 (+60°) needs positive.
        if frac[0] > cfg.browser_sector_gate
            || frac[1] > cfg.browser_sector_gate
            || frac[2] > cfg.browser_sector_gate
        {
            self.wander_ticks = 0;
            self.seek = Seek::Lead;
            if frac[0] >= frac[1] && frac[0] >= frac[2] {
                turn = -cfg.browser_gaze_turn;
                self.wander_bias = -1.0;
            } else if frac[2] > frac[1] {
                turn = cfg.browser_gaze_turn;
                self.wander_bias = 1.0;
            } else {
                turn = 0.0;
            }
        } else if front > 0.5 {
            // FROZEN-PREY: a wall ahead with nothing in view. The flip below swung the
            // heading back and forth through about 35° against it, creeping at
            // `blocked_forward`, and the turn it delivered kept the motor's delivery over
            // `motor_refused`, so no escape came: frondgrazers stood so for 50 sim minutes,
            // starving. As the blind forager does, it turns away from the contacted side,
            // or on its remembered side, without flipping, until the front clears.
            self.wander_ticks = 0;
            self.seek = Seek::Obstacle;
            let (left, right) = (o[self.contact.offset + 1], o[self.contact.offset + 2]);
            if left > right {
                self.wander_bias = -1.0;
            } else if right > left {
                self.wander_bias = 1.0;
            }
            turn = self.wander_bias * cfg.browser_wander_turn;
        } else {
            self.seek = Seek::Search;
            self.wander_ticks += 1;
            if self.wander_ticks >= cfg.browser_wander_flip {
                self.wander_ticks = 0;
                self.wander_bias = -self.wander_bias;
            }
            turn = self.wander_bias * cfg.browser_wander_turn;
        }
        let forward = if front > 0.5 { cfg.blocked_forward } else { 1.0 };
        Response::Bounded(Actions {
            forward,
            turn,
            feed,
        })
    }

    fn reset(&mut self) {
        self.wander_bias = 1.0;
        self.wander_ticks = 0;
        self.escape = 0;
        self.blocked_turned = 0.0;
        self.push = 0;
        self.seek = Seek::Unknown;
    }

    fn layer(&self) -> Layer {
        Layer::Heuristic
    }

    fn seek(&self) -> Seek {
        self.seek
    }
}

/// The seedporter's **fruit forager** (F3, Brief F3's heuristic), observation-only. The
/// body does the climbing, perching and gliding (`step`'s glider act): walking into a
/// tall stem climbs it to the crown, forward effort on a perch glides off (or climbs down
/// when the drop is short), and a glide is steered by `turn`. So the forager only:
///
/// 1. **eats** (stops, `feed`) while fruit is in the mouth's reach (`Taste`) — until the
///    source is empty. Whether it wants food is the body's eating latch, which no sense
///    reads: since the minimal gut a fed body's reserve sits at its cap, so the old
///    reserve line (`seedporter_full_at`) is gone;
/// 2. else **turns toward** the cone sector with the most fruit in view and goes forward
///    — walking to a stem and up it, or off a crown in a glide aimed at it;
/// 3. else **wanders** forward (on a perch, that glides it off or takes it down), backing
///    off a refused motor.
///
/// Night and downpours are the body's roost reflex, not this. It flees like the grazer.
#[derive(Clone, Debug)]
pub struct FruitForager {
    wander_bias: f64,
    wander_ticks: u32,
    escape: u32,
    selfm: crate::manifest::Module,
    taste: crate::manifest::Module,
    cone: crate::manifest::Module,
    /// What the last answer was for (HUNGRY-SEARCH, [`Controller::seek`]).
    seek: Seek,
    cfg: TeacherConfig,
}

impl FruitForager {
    /// The seedporter's forager on `cfg` (a world's [`crate::FaunaConfig::teachers`]).
    pub fn with_config(cfg: TeacherConfig) -> FruitForager {
        let manifest = Founder::Seedporter.manifest();
        FruitForager {
            cfg,
            wander_bias: 1.0,
            wander_ticks: 0,
            escape: 0,
            selfm: module_slot(&manifest, "Self"),
            taste: module_slot(&manifest, "Taste(1)"),
            cone: module_slot(&manifest, "Cone(3, foliage/body)"),
            seek: Seek::Unknown,
        }
    }
}

impl Controller for FruitForager {
    fn reads_tail(&self) -> bool {
        false
    }

    fn drive(&mut self, o: &[f64]) -> Response {
        let cfg = self.cfg;
        // The body's latch decides whether a bite is taken (the minimal gut).
        let full = false;
        let taste = o[self.taste.offset];
        let tasted = o[self.taste.offset + 2] > 0.5 && taste > cfg.feed_gate;
        if tasted && !full {
            self.wander_ticks = 0;
            self.seek = Seek::Taste;
            return Response::Bounded(Actions {
                forward: 0.0,
                turn: 0.0,
                feed: 1.0,
            });
        }
        let away = self.wander_bias;
        if let Some((forward, turn)) = escape(&cfg, &mut self.escape, o[self.selfm.offset + 7] < cfg.motor_refused, away) {
            self.wander_ticks = 0;
            self.seek = Seek::Obstacle;
            return Response::Bounded(Actions { forward, turn, feed: 0.0 });
        }
        let base = self.cone.offset;
        // Per sector, the fruit fraction sits in the browser's foliage slot, base + k*6 + 2.
        let frac = [o[base + 2], o[base + 8], o[base + 14]];
        let turn = if frac.iter().any(|&f| f > cfg.browser_sector_gate) {
            self.wander_ticks = 0;
            self.seek = Seek::Lead;
            if frac[0] >= frac[1] && frac[0] >= frac[2] {
                self.wander_bias = -1.0;
                -cfg.browser_gaze_turn
            } else if frac[2] > frac[1] {
                self.wander_bias = 1.0;
                cfg.browser_gaze_turn
            } else {
                0.0
            }
        } else {
            self.seek = Seek::Search;
            self.wander_ticks += 1;
            if self.wander_ticks >= cfg.browser_wander_flip {
                self.wander_ticks = 0;
                self.wander_bias = -self.wander_bias;
            }
            self.wander_bias * cfg.browser_wander_turn
        };
        // Full forward even against a contact: a wall in front is often the vine it climbs
        // (the body attaches), and a refused motor is the escape's.
        Response::Bounded(Actions {
            forward: 1.0,
            turn,
            feed: if full { 0.0 } else { 1.0 },
        })
    }

    fn reset(&mut self) {
        self.wander_bias = 1.0;
        self.wander_ticks = 0;
        self.escape = 0;
        self.seek = Seek::Unknown;
    }

    fn layer(&self) -> Layer {
        Layer::Heuristic
    }

    fn seek(&self) -> Seek {
        self.seek
    }
}

/// The bellwing's **hover-sip teacher** (F1, Brief F1 item 8), observation-only:
///
/// - a bloom in the mouth's reach (`Taste`): hover (or stay perched) and sip — the body's
///   eating latch decides whether the sip is taken (the minimal gut);
/// - the rest when full (rise into cover close overhead, else come down, and rest perched
///   until hungry) is kept but **no longer triggered**: since the minimal gut a fed body's
///   reserve sits at its cap, so the old reserve lines (`bellwing_full_at`,
///   `bellwing_hungry_at`) are gone;
/// - a bloom in the fan: turn toward the sector with the most, fly at it, and come down
///   when it is near;
/// - nothing in sight: a correlated wander at its search height within the band;
/// - blocked: climb while there is room to rise (under the band's top, open overhead),
///   else turn away;
/// - grounded by the weather (a downpour, or rain over its threshold; backlog §12): stop
///   under cover, else head for the most foliage in the fan, else keep moving while the
///   sky is open overhead (the body is brought down regardless, or up under cover it can
///   reach); at night the body roosts by itself.
pub struct HoverSip {
    wander: Wander,
    escape: u32,
    selfm: crate::manifest::Module,
    air: crate::manifest::Module,
    taste: crate::manifest::Module,
    fan: crate::manifest::Module,
    /// Resting: full, and not yet hungry again.
    resting: bool,
    lift: f64,
    /// Samples since the last decision, and that decision: the teacher re-plans at the
    /// walkers' cadence (0.25 s, [`HOVER_SIP_REPLAN`] ticks) so its reflex numbers —
    /// the wander's holds and turns in samples — mean what they mean for the walkers;
    /// a bloom at the mouth stops it at once.
    since: u32,
    last: Actions,
    /// What the last decision was for (HUNGRY-SEARCH, [`Controller::seek`]).
    seek: Seek,
    cfg: TeacherConfig,
}

/// Ticks between the hover-sip teacher's decisions: the walkers' controller period.
const HOVER_SIP_REPLAN: u32 = 5;

/// `Self.altitude` within this of 1 is at the band's top: a float tolerance, not a knob.
const BAND_TOP_TOLERANCE: f64 = 1e-3;

impl HoverSip {
    pub fn with_config(cfg: TeacherConfig) -> HoverSip {
        let m = Founder::Bellwing.manifest();
        HoverSip {
            wander: Wander::over(m.tail_offset()),
            escape: 0,
            selfm: module_slot(&m, "Self"),
            air: module_slot(&m, "Air"),
            taste: module_slot(&m, "Taste(1)"),
            fan: module_slot(&m, "Fan(3, bloom/foliage/body)"),
            resting: false,
            lift: 0.0,
            since: 0,
            last: Actions::REST,
            seek: Seek::Unknown,
            cfg,
        }
    }

    fn act(&mut self, forward: f64, turn: f64, feed: f64, lift: f64) -> Response {
        self.lift = lift;
        self.since = 0;
        self.last = Actions { forward, turn, feed };
        Response::with_body(self.last, BodyActions { lift: self.lift, ..BodyActions::default() })
    }
}

impl Controller for HoverSip {
    fn reads_tail(&self) -> bool {
        false
    }

    fn taste_only(&self) -> bool {
        // Between replans, drive either holds its answer or stops for a fresh taste.
        // Neither branch consumes Self, Air or Fan. The mouth still samples each tick.
        self.since + 1 < HOVER_SIP_REPLAN && self.last != Actions::REST
    }

    fn drive(&mut self, o: &[f64]) -> Response {
        let cfg = self.cfg;
        let s = self.selfm.offset;
        let (reserve, delivery, altitude, ceiling) = (o[s + 1], o[s + 7], o[s + 8], o[s + 9]);
        let perched = o[self.air.offset] > 0.5;
        let hanging = o[self.air.offset + 1] > 0.5;
        let overhead = o[self.air.offset + 2];
        let downpour = o[self.air.offset + 4] > 0.5;
        let taste = if o[self.taste.offset + 2] > 0.5 { o[self.taste.offset] } else { 0.0 };
        let f = self.fan.offset;
        let bloom = [o[f + 2], o[f + 10], o[f + 18]];
        let bloom_prox = [o[f + 3], o[f + 11], o[f + 19]];
        // The minimal gut: satiety is the body's eating latch, not the reserve.
        let _ = reserve;
        self.resting = false;
        // Hold the search height, never climbing into cover close overhead (it would hang
        // there): it goes around.
        // Between decisions it holds the last one, unless a bloom is at its mouth.
        let sip_now = taste > cfg.feed_gate && !self.resting && self.last.feed <= 0.0;
        self.since += 1;
        if self.since < HOVER_SIP_REPLAN && !sip_now && self.since > 0 && self.last != Actions::REST {
            return Response::with_body(self.last, BodyActions { lift: self.lift, ..BodyActions::default() });
        }
        // Under cover close overhead it holds the middle of the gap between the floor and
        // the cover instead (as far down as its search height): it never climbs into the
        // cover, and never scrapes along the floor, where a one-voxel lip blocks it —
        // a hatchling hatched on its face under an overhang hung there, turning in place,
        // until it starved (bellwing-finding follow-up 2).
        let hold = |alt: f64| {
            let lift = (cfg.bellwing_altitude_gain * (cfg.bellwing_cruise_altitude - alt)).clamp(-1.0, 1.0);
            if overhead < cfg.bellwing_roost_ceiling {
                lift.min((cfg.bellwing_altitude_gain * 0.5 * (overhead - alt)).clamp(-1.0, 1.0))
            } else {
                lift
            }
        };

        // A bloom at the mouth: sip.
        self.seek = Seek::Unknown;
        if taste > cfg.feed_gate && !self.resting {
            self.wander.interrupt(&cfg);
            self.seek = Seek::Taste;
            return self.act(0.0, 0.0, 1.0, 0.0);
        }
        // Grounded by the weather (the `downpour` channel: a downpour, or rain over its
        // threshold): down regardless (the body rises under cover it can reach); stop under
        // cover, else keep going toward the most foliage in its fan — a crown to hang under
        // or land on — or wander on while the sky is open overhead.
        if downpour {
            if perched || ceiling < 1.0 || overhead < 1.0 {
                return self.act(0.0, 0.0, 0.0, -1.0);
            }
            let foliage = [o[f + 4], o[f + 12], o[f + 20]];
            let best = (0..3).max_by(|&a, &b| foliage[a].total_cmp(&foliage[b])).unwrap_or(1);
            if foliage[best] > cfg.bellwing_bloom_gate {
                self.wander.interrupt(&cfg);
                let turn = match best {
                    0 => -cfg.bellwing_gaze_turn,
                    2 => cfg.bellwing_gaze_turn,
                    _ => 0.0,
                };
                return self.act(1.0, turn, 0.0, -1.0);
            }
            let turn = self.wander.turn(o, &cfg);
            return self.act(1.0, turn, 0.0, -1.0);
        }
        if perched {
            if self.resting {
                return self.act(0.0, 0.0, 0.0, 0.0);
            }
            // Take off: drop from an underside; out from under low cover; else up.
            return if hanging {
                self.act(1.0, 0.0, 0.0, -1.0)
            } else if overhead < cfg.bellwing_roost_ceiling {
                self.act(1.0, 0.0, 0.0, 0.0)
            } else {
                self.act(0.0, 0.0, 0.0, 1.0)
            };
        }
        // Aloft and full: go to rest.
        if self.resting {
            let lift = if overhead < cfg.bellwing_roost_ceiling { 1.0 } else { -1.0 };
            return self.act(0.0, 0.0, 0.0, lift);
        }
        // Blocked by a wall with room to rise — under the band's top and open overhead —
        // climb it, pressing on: a step taller than its search height is no wall to a
        // flyer whose band clears it (bellwing-finding item 1: turning away from every
        // one held the bellwings to the low strip, under flowers on the terraces).
        let blocked = delivery < cfg.motor_refused;
        if blocked && altitude < 1.0 - BAND_TOP_TOLERANCE && overhead >= cfg.bellwing_roost_ceiling {
            self.escape = 0;
            self.seek = Seek::Obstacle;
            return self.act(cfg.bellwing_wander_forward, 0.0, 0.0, 1.0);
        }
        // Blocked with no room to rise (the band's top, cover overhead, the strip's
        // edge): turn away, holding its height.
        let side = self.wander.side();
        if let Some((forward, turn)) = escape(&cfg, &mut self.escape, blocked, side) {
            self.wander.interrupt(&cfg);
            self.seek = Seek::Obstacle;
            return self.act(forward, turn, 0.0, hold(altitude));
        }
        // A bloom in the fan: at it.
        let best = (0..3).max_by(|&a, &b| bloom[a].total_cmp(&bloom[b])).unwrap_or(1);
        if bloom[best] > cfg.bellwing_bloom_gate {
            self.wander.interrupt(&cfg);
            self.seek = Seek::Lead;
            let turn = match best {
                0 => -cfg.bellwing_gaze_turn,
                2 => cfg.bellwing_gaze_turn,
                _ => 0.0,
            };
            let near = bloom_prox[best] > cfg.bellwing_bloom_near;
            let (forward, lift) = if near { (0.3, -0.6) } else { (1.0, hold(altitude)) };
            return self.act(forward, turn, 1.0, lift);
        }
        self.seek = Seek::Search;
        let turn = self.wander.turn(o, &cfg);
        self.act(cfg.bellwing_wander_forward, turn, 0.0, hold(altitude))
    }

    fn reset(&mut self) {
        self.wander = Wander::over(self.wander.seed_len);
        self.escape = 0;
        self.resting = false;
        self.lift = 0.0;
        self.since = 0;
        self.last = Actions::REST;
        self.seek = Seek::Unknown;
    }

    fn layer(&self) -> Layer {
        Layer::Heuristic
    }

    fn seek(&self) -> Seek {
        self.seek
    }
}

// The lanternjaw's ambush teacher (P1, predation §2.12's "ambush teacher"). Heuristic
// internals: nothing in the world reads them.

/// What the ambush teacher is doing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Ambush {
    /// Walking to a crown the grazers feed on.
    Seek,
    /// Stopped beside it with the lure lit, or stalking a body it sees.
    Wait,
    /// Leaving a spot that gave nothing.
    Leave,
    /// Eating, or full.
    Idle,
}

/// **The lanternjaw's ambush teacher** (predation §2.12, the brief's item 9), on the
/// interim lanternjaw manifest (the Browser's layout) and nothing else: walk to a crown —
/// where grazers feed and, at night, sleep — and wait beside it with the lure lit; turn
/// to a body it sees and creep toward it slower than 0.1 BL/s, so it stays a still shape;
/// hold `feed` while a body is close ahead, which the body turns into a strike when a
/// prey is in the capsule and into a mouthful of carrion when one is under the mouth; eat
/// the carcass its kill left. A fed body's mouth, lure and strike are shut by its eating
/// latch, not by the teacher. A fruitless wait of a minute sends it on to another crown.
#[derive(Clone, Debug)]
pub struct LanternjawAmbush {
    mode: Ambush,
    wait_left: u32,
    leave_left: u32,
    wander_bias: f64,
    wander_ticks: u32,
    escape: u32,
    lure: f64,
    /// Whether its last answer was eating carrion at its mouth ([`Controller::at_carcass`]).
    eating: bool,
    /// Whether it lights a lure at all (the chorister's does not).
    lit: bool,
    contact: crate::manifest::Module,
    taste: crate::manifest::Module,
    cone: crate::manifest::Module,
    /// The reflexes' numbers ([`TeacherConfig`]).
    cfg: TeacherConfig,
}

impl LanternjawAmbush {
    /// The lanternjaw's heuristic, pre-indexed against its manifest, on the default
    /// [`TeacherConfig`].
    pub fn new() -> LanternjawAmbush {
        LanternjawAmbush::with_config(TeacherConfig::default())
    }

    /// The lanternjaw's heuristic on `cfg` (a world's [`crate::FaunaConfig::teachers`]).
    pub fn with_config(cfg: TeacherConfig) -> LanternjawAmbush {
        LanternjawAmbush::for_founder(Founder::Lanternjaw, cfg)
    }

    /// The ambush on `founder`'s manifest (the lanternjaw's layout) and satiation line,
    /// its lure lit only for a hunter whose table pays one.
    fn for_founder(founder: Founder, cfg: TeacherConfig) -> LanternjawAmbush {
        let manifest = founder.manifest();
        let hunter = crate::FounderPhysiology::frozen(founder).active_hunter();
        LanternjawAmbush {
            cfg,
            mode: Ambush::Seek,
            wait_left: 0,
            leave_left: 0,
            wander_bias: 1.0,
            wander_ticks: 0,
            escape: 0,
            lure: 0.0,
            eating: false,
            lit: hunter.is_some_and(|h| h.lure_power > 0.0),
            contact: module_slot(&manifest, "Contact(4)"),
            taste: module_slot(&manifest, "Taste(1)"),
            cone: module_slot(&manifest, "Cone(3, foliage/body)"),
        }
    }

    fn act(&mut self, mode: Ambush, forward: f64, turn: f64, feed: f64) -> Response {
        self.mode = mode;
        self.lure = if mode == Ambush::Wait && self.lit { 1.0 } else { 0.0 };
        self.eating = mode == Ambush::Idle && feed > 0.0;
        Response::with_body(Actions { forward, turn, feed }, BodyActions { lure: self.lure, ..BodyActions::default() })
    }
}

impl Default for LanternjawAmbush {
    fn default() -> Self {
        LanternjawAmbush::new()
    }
}

impl Controller for LanternjawAmbush {
    fn layer(&self) -> Layer {
        Layer::Heuristic
    }

    fn reads_tail(&self) -> bool {
        false
    }

    fn drive(&mut self, o: &[f64]) -> Response {
        let cfg = self.cfg;
        let front = o[self.contact.offset];
        let (taste_cue, taste_valid) = (o[self.taste.offset], o[self.taste.offset + 2]);
        let base = self.cone.offset;
        let body = [o[base + 4], o[base + 10], o[base + 16]];
        let body_near = o[base + 11];
        let foliage = [o[base + 2], o[base + 8], o[base + 14]];
        let foliage_near = o[base + 9];

        // Carrion at the mouth: eat it. Whether it is wanted is the body's: since the
        // minimal gut a fed body's mouth and strike are shut by its eating latch, which no
        // sense reads, so the teacher asks and the body decides.
        if taste_valid > 0.5 && taste_cue > 0.0 {
            return self.act(Ambush::Idle, 0.0, 0.0, 1.0);
        }
        // A body in sight: face it, and creep up on one ahead with feed held.
        if body.iter().any(|&b| b > 0.0) {
            self.wait_left = cfg.wait_samples;
            if body[1] > 0.0 && body[1] >= body[0] && body[1] >= body[2] {
                let forward = if body_near >= cfg.strike_proximity || front > 0.5 {
                    0.0
                } else {
                    cfg.stalk
                };
                return self.act(Ambush::Wait, forward, 0.0, 1.0);
            }
            let turn = if body[0] >= body[2] { -cfg.ambush_face_turn } else { cfg.ambush_face_turn };
            return self.act(Ambush::Wait, 0.0, turn, 1.0);
        }
        // The browser teacher's escape from a refused motor or a drop ahead.
        let (clear, proximity) = (o[base + 6], o[base + 7]);
        let drop = clear > cfg.flat_clear + cfg.half_ray
            || (clear >= cfg.flat_clear - cfg.half_ray && proximity < cfg.drop_proximity);
        let away = self.wander_bias;
        if let Some((forward, turn)) =
            escape(&cfg, &mut self.escape, o[7] < cfg.motor_refused || drop, away)
        {
            return self.act(Ambush::Seek, forward, turn, 0.0);
        }
        match self.mode {
            Ambush::Wait if self.wait_left > 0 => {
                self.wait_left -= 1;
                if self.wait_left == 0 {
                    self.leave_left = cfg.leave_samples;
                    self.wander_bias = -self.wander_bias;
                    return self.act(Ambush::Leave, 1.0, self.wander_bias, 0.0);
                }
                return self.act(Ambush::Wait, 0.0, 0.0, 0.0);
            }
            Ambush::Leave if self.leave_left > 0 => {
                self.leave_left -= 1;
                let forward = if front > 0.5 { cfg.blocked_forward } else { 1.0 };
                return self.act(Ambush::Leave, forward, cfg.ambush_leave_turn * self.wander_bias, 0.0);
            }
            _ => {}
        }
        // Seek a crown; stop beside one and wait there.
        if foliage[1] > cfg.browser_sector_gate && foliage_near >= cfg.near_crown {
            self.wait_left = cfg.wait_samples;
            return self.act(Ambush::Wait, 0.0, 0.0, 0.0);
        }
        let forward = if front > 0.5 { cfg.blocked_forward } else { 1.0 };
        let turn = if foliage.iter().any(|&f| f > cfg.browser_sector_gate) {
            self.wander_ticks = 0;
            if foliage[0] >= foliage[1] && foliage[0] >= foliage[2] {
                -cfg.ambush_gaze_turn
            } else if foliage[2] > foliage[1] {
                cfg.ambush_gaze_turn
            } else {
                0.0
            }
        } else {
            self.wander_ticks += 1;
            if self.wander_ticks >= cfg.ambush_wander_flip {
                self.wander_ticks = 0;
                self.wander_bias = -self.wander_bias;
            }
            self.wander_bias * cfg.ambush_wander_turn
        };
        self.act(Ambush::Seek, forward, turn, 0.0)
    }

    fn reset(&mut self) {
        let lit = self.lit;
        *self = LanternjawAmbush::with_config(self.cfg);
        self.lit = lit;
    }

    fn at_carcass(&self) -> bool {
        self.eating
    }
}

/// **The lanternjaw's heuristic** (the detritus web D2): the P1 ambush
/// ([`LanternjawAmbush`]) under the step's **patrol** ([`Layer::Heuristic`],
/// `crate::hunt`), which reads the world and overrides it — patch to patch along its prey's
/// food, a short still wait with the lure lit, a fruitless patch left and remembered, a
/// turn to prey tremor, and attacks by its search image. The ambush's own answer stands
/// where the patrol stands back: eating carrion at its mouth, and the whole behaviour when
/// the world's [`crate::HuntConfig::patrol`] is off (the baseline arm).
#[derive(Clone, Debug)]
pub struct LanternjawPatrol {
    ambush: LanternjawAmbush,
    /// Whether the carrion its mouth tastes is over the teachers' `feed_gate`: the patrol
    /// stands back for a carcass, not for crumbs (the ambush eats any carrion it tastes,
    /// and a smear of it held patrolling lanternjaws in place all night, 2026-10-05).
    meal: bool,
}

impl LanternjawPatrol {
    /// The lanternjaw's heuristic on `cfg` (a world's [`crate::FaunaConfig::teachers`]).
    pub fn with_config(cfg: TeacherConfig) -> LanternjawPatrol {
        LanternjawPatrol {
            ambush: LanternjawAmbush::with_config(cfg),
            meal: false,
        }
    }
}

impl Controller for LanternjawPatrol {
    fn reads_tail(&self) -> bool {
        false
    }

    fn drive(&mut self, o: &[f64]) -> Response {
        let t = self.ambush.taste.offset;
        self.meal = o[t + 2] > 0.5 && o[t] > self.ambush.cfg.feed_gate;
        self.ambush.drive(o)
    }

    fn reset(&mut self) {
        self.ambush.reset();
        self.meal = false;
    }

    fn at_carcass(&self) -> bool {
        self.ambush.at_carcass() && self.meal
    }

    fn layer(&self) -> Layer {
        Layer::Heuristic
    }
}

/// **The chorister's heuristic** (roster R3, Brief R3 "Heuristic"): the lanternjaw's
/// ambush without a lure — alone it hunts like the lanternjaw — under the step's scripted
/// **pack tactic** ([`Layer::Heuristic`], `crate::pack`), which reads the pack and
/// overrides it: drive and flank a pack target, give a chase up, eat the kill, walk home
/// to rest, keep the pack together, never path into another pack's territory, and keep a
/// pup at its den. Observation-only itself, as every heuristic is.
#[derive(Clone, Debug)]
pub struct PackHunter {
    ambush: LanternjawAmbush,
}

impl PackHunter {
    /// The chorister's heuristic on `cfg` (a world's [`crate::FaunaConfig::teachers`]).
    pub fn with_config(cfg: TeacherConfig) -> PackHunter {
        PackHunter {
            ambush: LanternjawAmbush::for_founder(Founder::Chorister, cfg),
        }
    }
}

impl Controller for PackHunter {
    fn reads_tail(&self) -> bool {
        false
    }

    fn drive(&mut self, o: &[f64]) -> Response {
        self.ambush.drive(o)
    }

    fn reset(&mut self) {
        self.ambush.reset();
    }

    fn layer(&self) -> Layer {
        Layer::Heuristic
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn absent_body_logits_are_refused_instead_of_resolving_to_half_effort() {
        use super::*;
        for founder in Founder::ALL {
            let got = resolve(Response::logits(&[0.0, 0.0, 0.0]), &founder.manifest());
            assert_eq!(got.body, BodyActions::default(), "{founder:?}");
            assert_eq!(got.option, None);
        }
    }

    #[test]
    fn a_held_hover_sip_answer_consumes_only_taste() {
        use super::*;
        let cfg = TeacherConfig { feed_gate: 0.2, ..TeacherConfig::default() };
        let mut full = HoverSip::with_config(cfg);
        let mut trimmed = HoverSip::with_config(cfg);
        let manifest = Founder::Bellwing.manifest();
        let air = module_slot(&manifest, "Air").offset;
        let taste = module_slot(&manifest, "Taste(1)").offset;
        let fan = module_slot(&manifest, "Fan(3, bloom/foliage/body)");
        let mut skipped = 0;
        let mut tasted_while_skipped = false;
        for sample in 0..24 {
            let mut observation = vec![0.0; manifest.inputs()];
            observation[7] = 1.0;
            observation[8] = 0.5;
            observation[9] = 1.0;
            observation[air + 2] = 1.0;
            // A new mouthful interrupts a held flight; losing it still holds the sip
            // until the ordinary replan. Alternate a lead and a rain-grounding sample.
            observation[taste] = if sample % 7 == 2 { 1.0 } else { 0.0 };
            observation[taste + 2] = 1.0;
            observation[air + 4] = f64::from(sample % 3 == 0);
            observation[fan.offset + 2] = 1.0;
            observation[fan.offset + 4] = 1.0;
            let mut reduced = observation.clone();
            if trimmed.taste_only() {
                skipped += 1;
                tasted_while_skipped |= observation[taste] > cfg.feed_gate;
                reduced.fill(0.0);
                reduced[taste..taste + 3].copy_from_slice(&observation[taste..taste + 3]);
            }
            assert_eq!(full.drive(&observation), trimmed.drive(&reduced), "sample {sample}");
            assert_eq!(full.seek(), trimmed.seek());
        }
        assert!(skipped > 0 && tasted_while_skipped);
    }

    use super::*;
    use crate::manifest::Founder;

    /// S3: with no signal the shredder's teacher walks a correlated walk — they hold their heading
    /// for at least `wander_hold_min` samples between turns, a turn lasts at most
    /// `wander_turn_samples_max` samples, both signs occur, and two bodies with different
    /// senses walk different walks.
    #[test]
    fn the_no_signal_walk_holds_heading_between_bounded_turns() {
        {
            let founder = Founder::Blind;
            let manifest = founder.manifest();
            let walk = |salt: f64| -> Vec<f64> {
                let mut c: Box<dyn Controller> = heuristic(founder, TeacherConfig::default());
                c.reset();
                let mut o = quiet(&manifest);
                o[0] = salt;
                (0..400)
                    .map(|_| match c.drive(&o) {
                        Response::Bounded(a) | Response::Body(a, _) => a.turn,
                        Response::Logits(_) => unreachable!("teachers answer bounded"),
                    })
                    .collect()
            };
            let turns = walk(0.3);
            let mut runs: Vec<(bool, u32)> = Vec::new();
            for t in &turns {
                let turning = *t != 0.0;
                match runs.last_mut() {
                    Some((k, n)) if *k == turning => *n += 1,
                    _ => runs.push((turning, 1)),
                }
            }
            let holds: Vec<u32> = runs[1..runs.len() - 1]
                .iter()
                .filter(|r| !r.0)
                .map(|r| r.1)
                .collect();
            assert!(
                holds.len() >= 5,
                "{founder:?}: several turns in 400 samples"
            );
            assert!(
                holds.iter().all(|&h| h >= TeacherConfig::default().wander_hold_min),
                "{founder:?}: the heading is held between turns: {holds:?}"
            );
            assert!(
                runs.iter()
                    .filter(|r| r.0)
                    .all(|r| r.1 <= TeacherConfig::default().wander_turn_samples_max),
                "{founder:?}: a turn is a bounded angle"
            );
            let turning = turns.iter().filter(|t| **t != 0.0).count();
            assert!(turning * 4 < turns.len(), "{founder:?}: turns are rare");
            assert!(turns.iter().any(|t| *t > 0.0) && turns.iter().any(|t| *t < 0.0));
            assert_ne!(turns, walk(0.7), "{founder:?}: another body, another walk");
        }
    }

    /// A zero observation with `motor_delivery` at 1 — "everything asked for was
    /// delivered", which is what the channel reads when nothing was asked — so a test
    /// about one signal does not also trip the escape reflex.
    fn quiet(manifest: &Manifest) -> Vec<f64> {
        let mut o = vec![0.0; manifest.inputs()];
        o[7] = 1.0;
        o
    }

    #[test]
    fn heuristics_keep_the_v2_answers_with_a_changing_v3_tail_and_after_reset() {
        let cfg = TeacherConfig {
            wander_hold_min: 2,
            wander_hold_max: 4,
            wander_turn_samples_min: 1,
            wander_turn_samples_max: 3,
            ..TeacherConfig::default()
        };
        for founder in Founder::ALL {
            let manifest = founder.manifest();
            let mut v2 = heuristic(founder, cfg);
            let mut v3 = heuristic(founder, cfg);
            assert!(!v3.reads_tail(), "{founder:?}");
            assert_eq!(v3.layer(), Layer::Heuristic, "every live teacher keeps the full layer");
            let mut observation = quiet(&manifest);
            observation[0] = 0.3;
            if founder == Founder::Bellwing {
                observation[8] = 0.5;
                observation[9] = 1.0;
                observation[module_slot(&manifest, "Air").offset + 2] = 1.0;
            }
            for sample in 0..80 {
                if sample == 40 {
                    v2.reset();
                    v3.reset();
                }
                for (k, v) in observation[manifest.tail_offset()..].iter_mut().enumerate() {
                    *v = ((sample + k) % 11) as f64 / 10.0;
                }
                assert_eq!(
                    v2.drive(&observation[..manifest.tail_offset()]),
                    v3.drive(&observation),
                    "{founder:?}, sample {sample}"
                );
                assert_eq!(v2.seek(), v3.seek());
                assert_eq!(v2.taste_only(), v3.taste_only());
            }
        }
    }

    /// The adapter is the policy boundary's other half: logits go through the
    /// manifest's transfers and then the deadband, bounded actions only through the
    /// deadband, and neither path can touch a world — the function takes no world.
    #[test]
    fn the_adapter_transfers_logits_and_deadbands_both_kinds() {
        let manifest = Founder::Blind.manifest();

        // Sigmoid logits, then the 0.05 deadband: a small logit is zeroed, a large one
        // saturates to 1.
        // σ(0) = 0.5 for forward and feed (above the deadband), and tanh(0) = 0 is
        // deadbanded to no turn.
        let a = resolve_actions(Response::logits(&[0.0, 0.0, 100.0]), &manifest);
        assert_eq!((a.forward, a.turn, a.feed), (0.5, 0.0, 1.0));
        // σ(0) = 0.5 exactly, so forward and feed are 0.5; turn tanh(5) ≈ 1.
        let a = resolve_actions(Response::logits(&[0.0, 5.0, 0.0]), &manifest);
        assert_eq!(a.forward, 0.5);
        assert_eq!(a.feed, 0.5);
        assert!(a.turn > 0.99);
        // tanh of a small logit is inside the deadband: no turn.
        let a = resolve_actions(Response::logits(&[10.0, 0.01, 10.0]), &manifest);
        assert_eq!(a.turn, 0.0);
        assert!(a.forward > 0.9 && a.feed > 0.9);

        // Bounded responses are not transferred again: the heuristic's 0.5 stays 0.5.
        let a = resolve_actions(
            Response::Bounded(Actions {
                forward: 0.5,
                turn: 0.5,
                feed: 0.5,
            }),
            &manifest,
        );
        assert_eq!(
            (a.forward, a.turn, a.feed),
            (0.5, 0.5, 0.5),
            "sigmoid must not be applied to already-bounded actions"
        );

        // Out-of-range and non-finite responses are rejected, not trained through.
        let a = resolve_actions(
            Response::Bounded(Actions {
                forward: f64::NAN,
                turn: 7.0,
                feed: -3.0,
            }),
            &manifest,
        );
        assert_eq!(
            (a.forward, a.turn, a.feed),
            (0.0, 1.0, 0.0),
            "NAN forward is zero, turn 7 clamps to 1, feed -3 clamps to 0"
        );
        let a = resolve_actions(
            Response::logits(&[f64::NAN, f64::INFINITY, f64::NEG_INFINITY]),
            &manifest,
        );
        assert_eq!(a, Actions::REST, "non-finite logits are zeroed");
    }

    /// The observation-only heuristics answer bounded actions and react to the senses:
    /// the blind one creeps around front contact and turns to a rising/falling cue, and
    /// the browser one gazes at the richest foliage sector of its cone. Only the vector.
    #[test]
    fn the_heuristics_read_only_the_observation_and_reply_bounded() {
        let blind_manifest = Founder::Blind.manifest();
        let (c, t, ch) = (
            module_slot(&blind_manifest, "Contact(4)"),
            module_slot(&blind_manifest, "Taste(1)"),
            module_slot(&blind_manifest, "Chem(detritus)"),
        );
        let mut blind = BlindForager::new();
        let mut o = quiet(&blind_manifest);

        // Front contact: creep, turn away from the contacted side (here, a remembered
        // preference since neither side reads harder).
        o[c.offset] = 1.0;
        let Response::Bounded(a) = blind.drive(&o) else {
            panic!("bounded");
        };
        assert!(a.forward < 1.0, "a walled body creeps");
        assert!(a.turn.abs() > 0.5, "it turns away from the contact");

        // A flat, strong, valid cue: hold the heading and keep moving.  This is the
        // first valid sample at an arena start, so a deterministic diagnostic must not
        // turn away from the prepared food-facing heading.
        let mut o = quiet(&blind_manifest);
        o[ch.offset] = 0.6;
        o[ch.offset + 2] = 1.0;
        let Response::Bounded(a) = blind.drive(&o) else {
            panic!("bounded");
        };
        assert_eq!(a.forward, 1.0);
        assert_eq!(a.turn, 0.0, "a held-heading response does not steer");

        // Litter in the mouth: feed.
        let mut o = quiet(&blind_manifest);
        o[t.offset] = 0.6;
        o[t.offset + 2] = 1.0;
        let Response::Bounded(a) = blind.drive(&o) else {
            panic!("bounded");
        };
        assert_eq!(a.feed, 1.0);

        // Browser: foliage richer on the left sector turns left, richer on the right
        // turns right, and the mouth's taste gates feeding.
        let browser_manifest = Founder::Browser.manifest();
        let (tb, cb) = (
            module_slot(&browser_manifest, "Taste(1)"),
            module_slot(&browser_manifest, "Cone(3, foliage/body)"),
        );
        let mut browser = BrowserForager::new();
        let mut o = quiet(&browser_manifest);
        o[cb.offset + 2] = 0.6; // left sector's foliage fraction
        let Response::Bounded(a) = browser.drive(&o) else {
            panic!("bounded");
        };
        assert!(a.turn < -0.5, "left foliage turns left, got {}", a.turn);
        assert_eq!(a.feed, 0.0, "no mouth contact yet");

        let mut o = quiet(&browser_manifest);
        o[cb.offset + 14] = 0.6; // right sector's foliage fraction
        let Response::Bounded(a) = browser.drive(&o) else {
            panic!("bounded");
        };
        assert!(a.turn > 0.5, "right foliage turns right, got {}", a.turn);

        let mut o = quiet(&browser_manifest);
        o[tb.offset] = 0.6;
        o[tb.offset + 2] = 1.0;
        let Response::Bounded(a) = browser.drive(&o) else {
            panic!("bounded");
        };
        assert_eq!(a.feed, 1.0, "foliage at the mouth feeds");
    }

    /// **FROZEN-PREY: a browser blocked ahead with nothing in view keeps turning one way**
    /// until its front clears — away from the contacted side, or its remembered side —
    /// where the open-ground wander flips every `browser_wander_flip` samples.
    #[test]
    fn a_blocked_browser_turns_one_way_until_its_front_clears() {
        let cfg = TeacherConfig::default();
        let manifest = Founder::Browser.manifest();
        let contact = module_slot(&manifest, "Contact(4)");
        let turns = |browser: &mut BrowserForager, o: &[f64], n: u32| -> Vec<f64> {
            (0..n)
                .map(|_| match browser.drive(o) {
                    Response::Bounded(a) | Response::Body(a, _) => a.turn,
                    _ => panic!("bounded"),
                })
                .collect()
        };
        let n = 4 * cfg.browser_wander_flip.max(1);
        // Open ground: the wander flips.
        let open = quiet(&manifest);
        let t = turns(&mut BrowserForager::new(), &open, n);
        assert!(t.iter().any(|&x| x > 0.0) && t.iter().any(|&x| x < 0.0), "open ground flips: {t:?}");
        // Blocked ahead, no side contact: one sign throughout.
        let mut blocked = quiet(&manifest);
        blocked[contact.offset] = 1.0;
        let t = turns(&mut BrowserForager::new(), &blocked, n);
        assert!(t.iter().all(|&x| x != 0.0 && x.signum() == t[0].signum()), "blocked keeps one side: {t:?}");
        // Blocked with contact on one side: it turns the way the blind forager turns from
        // that side, every sample.
        for (side, sign) in [(1usize, -1.0), (2usize, 1.0)] {
            let mut o = blocked.clone();
            o[contact.offset + side] = 1.0;
            let t = turns(&mut BrowserForager::new(), &o, n);
            assert!(t.iter().all(|&x| x.signum() == sign), "contact slot {side}: {t:?}");
        }
    }

    /// JUVENILE-FOOD: **a browser tasting food stays on it** — the configured stay-on-food
    /// forward, no turn, feeding — whatever its cone shows; tasting nothing (or crumbs under
    /// the gate) it walks its search as before.
    #[test]
    fn a_browser_tasting_food_holds_still_and_feeds() {
        for stay in [0.0, 0.3] {
            let cfg = TeacherConfig { forage_stay_forward: stay, ..TeacherConfig::default() };
            for founder in [Founder::Browser, Founder::Loftstrider] {
                let manifest = founder.manifest();
                let taste = module_slot(&manifest, "Taste(1)");
                let cone = module_slot(&manifest, "Cone(3, foliage/body)");
                let mut o = quiet(&manifest);
                // Foliage off to one side, which the gaze would otherwise turn to.
                o[cone.offset + 2] = 1.0;
                o[taste.offset + 2] = 1.0;
                o[taste.offset] = cfg.feed_gate * 2.0;
                let mut c = BrowserForager::for_founder(founder, cfg);
                let Response::Bounded(a) = c.drive(&o) else { panic!("bounded") };
                assert_eq!((a.forward, a.turn, a.feed), (stay, 0.0, 1.0), "{founder:?} tasting");
                o[taste.offset] = cfg.feed_gate * 0.5;
                let Response::Bounded(a) = c.drive(&o) else { panic!("bounded") };
                assert!(a.feed < 0.5 && a.forward > stay && a.turn != 0.0, "{founder:?} not tasting: {a:?}");
            }
        }
    }

    /// JUVENILE-FOOD: **a browser whose front (or drop reading) stays on through a whole
    /// delivered turn walks straight on** for `escape_samples` samples, then turns again;
    /// one that has not turned, or whose motor is refused, keeps turning, and a front that
    /// clears starts the count afresh.
    #[test]
    fn a_full_blocked_turn_ends_in_a_push_straight_on() {
        let cfg = TeacherConfig::default();
        let manifest = Founder::Browser.manifest();
        let contact = module_slot(&manifest, "Contact(4)");
        let act = |c: &mut BrowserForager, o: &[f64]| match c.drive(o) {
            Response::Bounded(a) => a,
            _ => panic!("bounded"),
        };
        let mut blocked = quiet(&manifest);
        blocked[contact.offset] = 1.0;
        // Each interval delivered the most turn the channel reports.
        let mut turning = blocked.clone();
        turning[6] = 1.0;
        let full = (std::f64::consts::TAU / manifest.turn_reference_rad).ceil() as usize;
        let mut c = BrowserForager::new();
        for k in 0..full - 1 {
            let a = act(&mut c, &turning);
            assert!(a.turn != 0.0 && a.forward < 1.0, "sample {k} still turns: {a:?}");
        }
        for k in 0..cfg.escape_samples.max(1) {
            let a = act(&mut c, &turning);
            assert_eq!((a.forward, a.turn), (1.0, 0.0), "push sample {k}");
        }
        let a = act(&mut c, &turning);
        assert!(a.turn != 0.0 && a.forward < 1.0, "turns again after the push: {a:?}");
        // No turn delivered, or a refused motor (a real obstacle): no push however long.
        let mut refused = turning.clone();
        refused[7] = 0.0;
        for o in [&blocked, &refused] {
            let mut c = BrowserForager::new();
            for _ in 0..4 * full {
                assert!(act(&mut c, o).forward < 1.0);
            }
        }
        // The grazer's drop reading (its forward sector's rows clear past flat ground)
        // counts as the blocked front does.
        let cone = module_slot(&manifest, "Cone(3, foliage/body)");
        let mut ledge = quiet(&manifest);
        ledge[6] = 1.0;
        ledge[cone.offset + 6] = 1.0;
        let mut c = BrowserForager::new();
        for k in 0..full - 1 {
            assert!(act(&mut c, &ledge).forward < 1.0, "sample {k} turns from the drop");
        }
        assert_eq!(act(&mut c, &ledge).forward, 1.0, "a whole turn of drops: straight on");
        // A clear front in between restarts the count.
        let mut c = BrowserForager::new();
        let open = quiet(&manifest);
        for _ in 0..3 {
            for _ in 0..full - 1 {
                assert!(act(&mut c, &turning).forward < 1.0);
            }
            act(&mut c, &open);
        }
    }

    /// The blind diagnostic keeps an observable approach straight, changes course
    /// after a falling cue, and only its own bounded wander memory affects no-signal
    /// turns.  Resetting that memory restores the fresh-episode response.
    #[test]
    fn blind_forager_trend_wander_and_reset_are_bounded() {
        let manifest = Founder::Blind.manifest();
        let (contact, chem) = (
            module_slot(&manifest, "Contact(4)"),
            module_slot(&manifest, "Chem(detritus)"),
        );
        let mut blind = BlindForager::new();

        let mut cue = quiet(&manifest);
        cue[chem.offset] = 0.6;
        cue[chem.offset + 2] = 1.0;

        // Flat is the first temporal sample; a rising sample preserves the
        // food-facing heading.  The old controller steered on both paths.
        for trend in [0.0, 0.1] {
            cue[chem.offset + 1] = trend;
            let Response::Bounded(actions) = blind.drive(&cue) else {
                panic!("blind diagnostic answers bounded actions");
            };
            assert_eq!(actions.forward, 1.0);
            assert_eq!(actions.turn, 0.0, "trend {trend} should hold heading");
        }

        // A falling cue changes course, while front contact still limits forward
        // effort and turns away from the contacted side.
        cue[chem.offset + 1] = -0.1;
        let Response::Bounded(actions) = blind.drive(&cue) else {
            panic!("blind diagnostic answers bounded actions");
        };
        assert_eq!(actions.turn, -0.8);
        cue.fill(0.0);
        cue[7] = 1.0;
        cue[contact.offset] = 1.0;
        cue[contact.offset + 1] = 0.8;
        cue[contact.offset + 2] = 0.1;
        let Response::Bounded(actions) = blind.drive(&cue) else {
            panic!("blind diagnostic answers bounded actions");
        };
        assert_eq!(actions.forward, 0.1);
        assert_eq!(actions.turn, -1.0);

        // No signal walks the correlated walk (S3): bounded, and `reset` restarts the
        // same walk from the same senses.
        cue.fill(0.0);
        cue[7] = 1.0;
        let walk = |c: &mut BlindForager| -> Vec<f64> {
            (0..60)
                .map(|_| {
                    let Response::Bounded(actions) = c.drive(&cue) else {
                        panic!("blind diagnostic answers bounded actions");
                    };
                    assert!((-1.0..=1.0).contains(&actions.turn));
                    actions.turn
                })
                .collect()
        };
        blind.reset();
        let first = walk(&mut blind);
        blind.reset();
        assert_eq!(walk(&mut blind), first, "reset restarts the walk");
    }

    /// The scripted diagnostic emits its bounded script in order, cycles, and `reset`
    /// rewinds it — the fresh-episode contract for controller memory.
    #[test]
    fn the_scripted_diagnostic_cycles_and_resets() {
        let mut c = Scripted::new(vec![
            Actions {
                forward: 1.0,
                turn: 0.0,
                feed: 0.0,
            },
            Actions {
                forward: 0.0,
                turn: -1.0,
                feed: 0.0,
            },
        ]);
        for round in 0..2 {
            for expected in [1.0, 0.0] {
                let Response::Bounded(a) = c.drive(&[0.0; 23]) else {
                    panic!("a scripted controller answers bounded");
                };
                assert_eq!(a.forward, expected, "round {round}");
            }
        }
        c.reset();
        let Response::Bounded(a) = c.drive(&[0.0; 23]) else {
            panic!("a scripted controller answers bounded");
        };
        assert_eq!(a.forward, 1.0, "reset rewound the script");
    }

    /// HUNGRY-SEARCH: each foraging heuristic says what its answer was for — its own
    /// search with nothing in sense, food when it tastes or steers by it, an obstacle when
    /// something is in its way — and a scent with no gradient is no lead.
    #[test]
    fn the_foragers_say_what_each_answer_was_for() {
        let cfg = TeacherConfig::default();
        let seek = |c: &mut dyn Controller, o: &[f64]| {
            c.drive(o);
            c.seek()
        };
        for founder in [Founder::Blind, Founder::Capgnawer, Founder::RippleSnail] {
            let manifest = founder.manifest();
            let (contact, taste) = (module_slot(&manifest, "Contact(4)"), module_slot(&manifest, "Taste(1)"));
            let chem = *manifest.modules.iter().find(|m| m.name.starts_with("Chem(")).expect("a Chem module");
            let mut c = BlindForager::for_founder(founder, cfg);
            assert_eq!(c.seek(), Seek::Unknown, "{founder:?} before its first answer");
            assert_eq!(seek(&mut c, &quiet(&manifest)), Seek::Search, "{founder:?} with nothing in sense");
            let mut o = quiet(&manifest);
            o[chem.offset] = 2.0 * cfg.chem_response_gate;
            o[chem.offset + 2] = 1.0;
            assert_eq!(seek(&mut c, &o), Seek::Search, "{founder:?} in a scent with no gradient");
            o[chem.offset + 1] = 2.0 * cfg.chem_trend_gate;
            assert_eq!(seek(&mut c, &o), Seek::Lead, "{founder:?} up a rising scent");
            let mut o = quiet(&manifest);
            // Food it stays on: the capgnawer's must taste of its stay worth
            // (CAPGNAWER-CAPS); weaker food it eats on the move while it searches.
            let stay = if founder == Founder::Capgnawer { cfg.capgnawer_stay_worth } else { 0.0 };
            o[taste.offset] = 2.0 * cfg.feed_gate.max(stay);
            o[taste.offset + 2] = 1.0;
            assert_eq!(seek(&mut c, &o), Seek::Taste, "{founder:?} tasting food");
            let mut c = BlindForager::for_founder(founder, cfg);
            let mut o = quiet(&manifest);
            o[contact.offset] = 1.0;
            assert_eq!(seek(&mut c, &o), Seek::Obstacle, "{founder:?} blocked ahead");
            let mut o = quiet(&manifest);
            o[7] = 0.0;
            assert_eq!(seek(&mut c, &o), Seek::Obstacle, "{founder:?} with its motor refused");
            c.reset();
            assert_eq!(c.seek(), Seek::Unknown, "{founder:?} after a reset");
        }
        for founder in [Founder::Browser, Founder::Loftstrider] {
            let manifest = founder.manifest();
            let (contact, taste) = (module_slot(&manifest, "Contact(4)"), module_slot(&manifest, "Taste(1)"));
            let cone = module_slot(&manifest, "Cone(3, foliage/body)");
            let mut c = BrowserForager::for_founder(founder, cfg);
            assert_eq!(seek(&mut c, &quiet(&manifest)), Seek::Search, "{founder:?} with nothing in sense");
            let mut o = quiet(&manifest);
            o[cone.offset + 2] = 2.0 * cfg.browser_sector_gate;
            assert_eq!(seek(&mut c, &o), Seek::Lead, "{founder:?} with foliage in view");
            let mut o = quiet(&manifest);
            o[taste.offset] = 2.0 * cfg.feed_gate;
            o[taste.offset + 2] = 1.0;
            assert_eq!(seek(&mut c, &o), Seek::Taste, "{founder:?} tasting food");
            let mut o = quiet(&manifest);
            o[contact.offset] = 1.0;
            assert_eq!(seek(&mut c, &o), Seek::Obstacle, "{founder:?} blocked ahead");
            let mut o = quiet(&manifest);
            o[7] = 0.0;
            assert_eq!(seek(&mut c, &o), Seek::Obstacle, "{founder:?} with its motor refused");
        }
        let manifest = Founder::Seedporter.manifest();
        let cone = module_slot(&manifest, "Cone(3, foliage/body)");
        let mut c = FruitForager::with_config(cfg);
        assert_eq!(seek(&mut c, &quiet(&manifest)), Seek::Search, "seedporter with nothing in sense");
        let mut o = quiet(&manifest);
        o[cone.offset + 8] = 2.0 * cfg.browser_sector_gate;
        assert_eq!(seek(&mut c, &o), Seek::Lead, "seedporter with fruit in view");
        let manifest = Founder::Bellwing.manifest();
        let fan = module_slot(&manifest, "Fan(3, bloom/foliage/body)");
        let mut c = HoverSip::with_config(cfg);
        assert_eq!(seek(&mut c, &quiet(&manifest)), Seek::Search, "bellwing with nothing in sense");
        let mut c = HoverSip::with_config(cfg);
        let mut o = quiet(&manifest);
        o[fan.offset + 10] = 2.0 * cfg.bellwing_bloom_gate;
        assert_eq!(seek(&mut c, &o), Seek::Lead, "bellwing with a bloom in its fan");
        // Not a forager: the search leaves it alone.
        let mut c = Scripted::new(vec![Actions { forward: 1.0, turn: 0.0, feed: 0.0 }]);
        assert_eq!(seek(&mut c, &quiet(&manifest)), Seek::Unknown, "a scripted body");
    }
}
