//! **The phase-one controller boundary** — the one seam between a body and whatever
//! drives it (`design/voxel-senses-phase1-plan.md`, "Phase-one body and action contract").
//!
//! A [`Controller`] receives **only the observation vector and its own private memory**
//! and answers with the three local actions. It sees no world, no flora stock, no site,
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

/// What a controller answers with. The distinction is the whole adapter: bounded
/// responses are deadbanded as they are, logits are transferred first, and both paths
/// are the same code in training and viewing.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Response {
    /// Actions already inside the manifest's bounds, from a diagnostic controller.
    Bounded(Actions),
    /// Unbounded logits in manifest action order, from a network controller. The
    /// adapter applies the manifest's transfer per action before the deadband.
    Logits([f64; 3]),
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

    /// The **lure** effort (`[0, 1]`) this controller holds after its last [`drive`]: the
    /// lanternjaw manifest's fourth action (predation §2.2). An interim seam — P2's
    /// contract v3 carries it in the response — so every controller that is not a
    /// lanternjaw's answers 0, and the body only reads it for a carnivore.
    ///
    /// [`drive`]: Controller::drive
    fn lure(&self) -> f64 {
        0.0
    }

    /// The **lift** effort (`[-1, 1]`, vertical) this controller holds after its last
    /// [`drive`]: a flyer manifest's fourth action (F1, predation-and-flight §3.3), the
    /// lure's seam until P2's contract v3 carries it in the response. Every controller
    /// that is not a flyer's answers 0, and the body only reads it for a flyer.
    ///
    /// [`drive`]: Controller::drive
    fn lift(&self) -> f64 {
        0.0
    }

    /// Whether the tick applies the **prey flee rule** to this controller's body
    /// (predation §2.7) while a hunter is in the world: the prey heuristics say yes, and
    /// nothing else does — a trained policy's behaviour changes only through the world,
    /// and a test's scripted control is left exactly as scripted.
    fn flees(&self) -> bool {
        false
    }

    /// Whether the tick applies the **scripted pack tactic** (roster R3) to this
    /// controller's body: the chorister's heuristic says yes, and nothing else does. The
    /// tactic reads the pack (den, target, packmates) and overrides the held actions as the
    /// flee rule does; a trained policy or a test's scripted control is left as it is.
    fn pack_tactic(&self) -> bool {
        false
    }

    /// Whether this controller's body is **at a carcass** after its last [`drive`]: eating
    /// carrion at its mouth. The lanternjaw's heuristic answers it, and the tick's carcass
    /// yield (roster R3) reads it; every other controller answers `false`.
    ///
    /// [`drive`]: Controller::drive
    fn at_carcass(&self) -> bool {
        false
    }
}

/// The shared action adapter: transfer, clamp, deadband — in that order, identically
/// for training and viewing. Non-finite logits and out-of-range bounded actions are
/// rejected to the deadband's zero rather than trained through.
pub fn resolve_actions(response: Response, manifest: &Manifest) -> Actions {
    let deadband = manifest.deadband;
    match response {
        Response::Bounded(a) => Actions {
            forward: deadband_low(
                clean(a.forward, manifest.actions[0].low, manifest.actions[0].high),
                deadband,
            ),
            turn: deadband_sym(
                clean(a.turn, manifest.actions[1].low, manifest.actions[1].high),
                deadband,
            ),
            feed: deadband_low(
                clean(a.feed, manifest.actions[2].low, manifest.actions[2].high),
                deadband,
            ),
        },
        Response::Logits(l) => Actions {
            forward: deadband_low(transfer(l[0], manifest.actions[0].transfer), deadband),
            turn: deadband_sym(transfer(l[1], manifest.actions[1].transfer), deadband),
            feed: deadband_low(transfer(l[2], manifest.actions[2].transfer), deadband),
        },
    }
}

/// A finite value clamped into the action's declared bounds; anything non-finite is 0.
fn clean(v: f64, low: f64, high: f64) -> f64 {
    if !v.is_finite() {
        return 0.0;
    }
    v.clamp(low, high)
}

/// One manifest transfer applied to a logit; non-finite in, zero out.
fn transfer(logit: f64, t: Transfer) -> f64 {
    if !logit.is_finite() {
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

    /// The lure a body's controller holds after its last drive; 0 with no controller.
    pub fn lure(&self, id: u64) -> f64 {
        self.entries
            .binary_search_by_key(&id, |e| e.0)
            .ok()
            .map_or(0.0, |at| self.entries[at].1.lure())
    }

    /// The lift a body's controller holds after its last drive; 0 with no controller.
    pub fn lift(&self, id: u64) -> f64 {
        self.entries
            .binary_search_by_key(&id, |e| e.0)
            .ok()
            .map_or(0.0, |at| self.entries[at].1.lift())
    }

    /// Whether a body's controller opts into the prey flee rule ([`Controller::flees`]).
    pub fn flees(&self, id: u64) -> bool {
        self.entries
            .binary_search_by_key(&id, |e| e.0)
            .ok()
            .is_some_and(|at| self.entries[at].1.flees())
    }

    /// Whether a body's controller opts into the pack tactic ([`Controller::pack_tactic`]).
    pub fn pack_tactic(&self, id: u64) -> bool {
        self.entries
            .binary_search_by_key(&id, |e| e.0)
            .ok()
            .is_some_and(|at| self.entries[at].1.pack_tactic())
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
            return Response::Bounded(Actions::REST);
        }
        let action = self.script[self.next % self.script.len()];
        self.next += 1;
        Response::Bounded(action)
    }

    fn reset(&mut self) {
        self.next = 0;
    }

    fn lift(&self) -> f64 {
        self.lift
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
}

impl Wander {
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
        self.state = o.iter().fold(0x5EED_0F_A11C_E5u64, |h, v| {
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
        Founder::Lanternjaw => Box::new(LanternjawAmbush::with_config(teachers)),
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
    contact: crate::manifest::Module,
    taste: crate::manifest::Module,
    chem: crate::manifest::Module,
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
            cfg,
            turn_bias: 1.0,
            wander: Wander::default(),
            escape: 0,
            contact: module_slot(&manifest, "Contact(4)"),
            taste: module_slot(&manifest, "Taste(1)"),
            chem,
        }
    }
}

impl Default for BlindForager {
    fn default() -> Self {
        BlindForager::new()
    }
}

impl Controller for BlindForager {
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
            return Response::Bounded(Actions {
                forward: 1.0,
                turn: 0.0,
                feed: 0.0,
            });
        }
        let cfg = self.cfg;
        let front_blocked = front > 0.5;
        let forward = if front_blocked { cfg.blocked_forward } else { 1.0 };
        // Feed on any food the mouth tastes ([`TeacherConfig::feed_gate`], 0.02: "any
        // food"; taste's reference is `emit_reference × mass_scale`, so at the shipped x30
        // a whole springturf or a starter litter tile tastes ~0.05; the ripple-snail's is its
        // own `taste_reference`, the film capacity per site). Failed attempts off
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
            return Response::Bounded(Actions {
                forward,
                turn,
                feed,
            });
        }
        let turn = if front_blocked {
            // A wall is a physical feature, not a target: turn away from the contacted
            // side, or fall back on the walk's last side.
            self.wander.interrupt(&cfg);
            if left > right {
                -1.0
            } else if right > left {
                1.0
            } else {
                self.wander.side() * cfg.blind_turn
            }
        } else if chem_valid > 0.5 && chem_resp >= cfg.chem_response_gate {
            self.wander.interrupt(&cfg);
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
        self.wander = Wander::default();
        self.escape = 0;
    }

    fn flees(&self) -> bool {
        true
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
    contact: crate::manifest::Module,
    taste: crate::manifest::Module,
    cone: crate::manifest::Module,
    /// Whether the adult's steepest ray reaches flat ground inside the cone's range, so
    /// that the forward sector's floor rows mean "flat" ([`TeacherConfig::flat_clear`]).
    /// True for the grazer (eye 0.3 m); false for the loftstrider (eye 1.5 m, the
    /// −40° ray ends 1.29 m down), whose drop reflex is off and whose refused motor still
    /// turns it away. Geometry, not a knob.
    reads_drops: bool,
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
            contact: module_slot(&manifest, "Contact(4)"),
            taste: module_slot(&manifest, "Taste(1)"),
            cone: module_slot(&manifest, "Cone(3, foliage/body)"),
            reads_drops,
        }
    }
}

impl Default for BrowserForager {
    fn default() -> Self {
        BrowserForager::new()
    }
}

impl Controller for BrowserForager {
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
        let away = self.wander_bias;
        if let Some((forward, turn)) =
            escape(&cfg, &mut self.escape, o[7] < cfg.motor_refused || drop, away)
        {
            self.wander_ticks = 0;
            return Response::Bounded(Actions {
                forward,
                turn,
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
            if frac[0] >= frac[1] && frac[0] >= frac[2] {
                turn = -cfg.browser_gaze_turn;
                self.wander_bias = -1.0;
            } else if frac[2] > frac[1] {
                turn = cfg.browser_gaze_turn;
                self.wander_bias = 1.0;
            } else {
                turn = 0.0;
            }
        } else {
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
    }

    fn flees(&self) -> bool {
        true
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
///    reserve line ([`TeacherConfig::seedporter_full_at`]) is no longer read;
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
        }
    }
}

impl Controller for FruitForager {
    fn drive(&mut self, o: &[f64]) -> Response {
        let cfg = self.cfg;
        // The body's latch decides whether a bite is taken (the minimal gut).
        let full = false;
        let taste = o[self.taste.offset];
        let tasted = o[self.taste.offset + 2] > 0.5 && taste > cfg.feed_gate;
        if tasted && !full {
            self.wander_ticks = 0;
            return Response::Bounded(Actions {
                forward: 0.0,
                turn: 0.0,
                feed: 1.0,
            });
        }
        let away = self.wander_bias;
        if let Some((forward, turn)) = escape(&cfg, &mut self.escape, o[self.selfm.offset + 7] < cfg.motor_refused, away) {
            self.wander_ticks = 0;
            return Response::Bounded(Actions { forward, turn, feed: 0.0 });
        }
        let base = self.cone.offset;
        // Per sector, the fruit fraction sits in the browser's foliage slot, base + k*6 + 2.
        let frac = [o[base + 2], o[base + 8], o[base + 14]];
        let turn = if frac.iter().any(|&f| f > cfg.browser_sector_gate) {
            self.wander_ticks = 0;
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
    }

    fn flees(&self) -> bool {
        true
    }
}

/// The bellwing's **hover-sip teacher** (F1, Brief F1 item 8), observation-only:
///
/// - a bloom in the mouth's reach (`Taste`): hover (or stay perched) and sip — the body's
///   eating latch decides whether the sip is taken (the minimal gut);
/// - the rest when full (rise into cover close overhead, else come down, and rest perched
///   until hungry) is kept but **no longer triggered**: since the minimal gut a fed body's
///   reserve sits at its cap, so [`TeacherConfig::bellwing_full_at`] and
///   [`TeacherConfig::bellwing_hungry_at`] are no longer read;
/// - a bloom in the fan: turn toward the sector with the most, fly at it, and come down
///   when it is near;
/// - nothing in sight: a correlated wander at its search height within the band;
/// - a downpour: keep moving while the sky is open overhead, stop over cover (the body
///   is brought down regardless); at night the body roosts by itself.
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
    cfg: TeacherConfig,
}

/// Ticks between the hover-sip teacher's decisions: the walkers' controller period.
const HOVER_SIP_REPLAN: u32 = 5;

impl HoverSip {
    pub fn with_config(cfg: TeacherConfig) -> HoverSip {
        let m = Founder::Bellwing.manifest();
        HoverSip {
            wander: Wander::default(),
            escape: 0,
            selfm: module_slot(&m, "Self"),
            air: module_slot(&m, "Air"),
            taste: module_slot(&m, "Taste(1)"),
            fan: module_slot(&m, "Fan(3, bloom/foliage/body)"),
            resting: false,
            lift: 0.0,
            since: 0,
            last: Actions::REST,
            cfg,
        }
    }

    fn act(&mut self, forward: f64, turn: f64, feed: f64, lift: f64) -> Response {
        self.lift = lift;
        self.since = 0;
        self.last = Actions { forward, turn, feed };
        Response::Bounded(self.last)
    }
}

impl Controller for HoverSip {
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
            return Response::Bounded(self.last);
        }
        let hold = |alt: f64| {
            let lift = (cfg.bellwing_altitude_gain * (cfg.bellwing_cruise_altitude - alt)).clamp(-1.0, 1.0);
            if overhead < cfg.bellwing_roost_ceiling { lift.min(0.0) } else { lift }
        };

        // A bloom at the mouth: sip.
        if taste > cfg.feed_gate && !self.resting {
            self.wander.interrupt(&cfg);
            return self.act(0.0, 0.0, 1.0, 0.0);
        }
        // A downpour: down regardless; keep going while the sky is open overhead.
        if downpour {
            if perched || ceiling < 1.0 || overhead < 1.0 {
                return self.act(0.0, 0.0, 0.0, -1.0);
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
        // Blocked (a wall, the ceiling over lower ground): turn away and climb a little.
        let side = self.wander.side();
        if let Some((forward, turn)) = escape(&cfg, &mut self.escape, delivery < cfg.motor_refused, side) {
            self.wander.interrupt(&cfg);
            return self.act(forward, turn, 0.0, hold(altitude));
        }
        // A bloom in the fan: at it.
        let best = (0..3).max_by(|&a, &b| bloom[a].total_cmp(&bloom[b])).unwrap_or(1);
        if bloom[best] > cfg.bellwing_bloom_gate {
            self.wander.interrupt(&cfg);
            let turn = match best {
                0 => -cfg.bellwing_gaze_turn,
                2 => cfg.bellwing_gaze_turn,
                _ => 0.0,
            };
            let near = bloom_prox[best] > cfg.bellwing_bloom_near;
            let (forward, lift) = if near { (0.3, -0.6) } else { (1.0, hold(altitude)) };
            return self.act(forward, turn, 1.0, lift);
        }
        let turn = self.wander.turn(o, &cfg);
        self.act(cfg.bellwing_wander_forward, turn, 0.0, hold(altitude))
    }

    fn reset(&mut self) {
        self.wander = Wander::default();
        self.escape = 0;
        self.resting = false;
        self.lift = 0.0;
        self.since = 0;
        self.last = Actions::REST;
    }

    fn lift(&self) -> f64 {
        self.lift
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
        let hunter = crate::FounderPhysiology::frozen(founder).hunter;
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
        Response::Bounded(Actions {
            forward,
            turn,
            feed,
        })
    }
}

impl Default for LanternjawAmbush {
    fn default() -> Self {
        LanternjawAmbush::new()
    }
}

impl Controller for LanternjawAmbush {
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

    fn lure(&self) -> f64 {
        self.lure
    }

    fn at_carcass(&self) -> bool {
        self.eating
    }
}

/// **The chorister's heuristic** (roster R3, Brief R3 "Heuristic"): the lanternjaw's
/// ambush without a lure — alone it hunts like the lanternjaw — under the step's scripted
/// **pack tactic** ([`Controller::pack_tactic`], `crate::pack`), which reads the pack and
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
    fn drive(&mut self, o: &[f64]) -> Response {
        self.ambush.drive(o)
    }

    fn reset(&mut self) {
        self.ambush.reset();
    }

    fn pack_tactic(&self) -> bool {
        true
    }
}

#[cfg(test)]
mod tests {
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
                        Response::Bounded(a) => a.turn,
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
        let a = resolve_actions(Response::Logits([0.0, 0.0, 100.0]), &manifest);
        assert_eq!((a.forward, a.turn, a.feed), (0.5, 0.0, 1.0));
        // σ(0) = 0.5 exactly, so forward and feed are 0.5; turn tanh(5) ≈ 1.
        let a = resolve_actions(Response::Logits([0.0, 5.0, 0.0]), &manifest);
        assert_eq!(a.forward, 0.5);
        assert_eq!(a.feed, 0.5);
        assert!(a.turn > 0.99);
        // tanh of a small logit is inside the deadband: no turn.
        let a = resolve_actions(Response::Logits([10.0, 0.01, 10.0]), &manifest);
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
            Response::Logits([f64::NAN, f64::INFINITY, f64::NEG_INFINITY]),
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
        let mut walk = |c: &mut BlindForager| -> Vec<f64> {
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
}
