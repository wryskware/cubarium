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
//! - **Diagnostic controllers** (the P1-C heuristics; [`Scripted`] here for tests)
//!   return [`Response::Bounded`] — actions already inside the manifest's bounds.
//! - **A GRU policy** returns [`Response::Logits`] — the raw network outputs. The
//!   adapter ([`resolve_actions`]) applies the manifest's transfers (sigmoid for
//!   forward/feed, tanh for turn) and the fixed 0.05 deadband to logits, and the same
//!   deadband to bounded actions, **identically in training and viewing**: the arena
//!   tick, the ES episode driver and the viewer all resolve through this one function.
//!
//! The shape-aware GRU runtime itself is P1-D (`crates/cubarium-search`): it plugs in as
//! a `Controller` whose `drive` runs one GRU forward step over the observation and wraps
//! the three outputs in [`Response::Logits`], keeping its hidden state as its private
//! memory and clearing it in `reset` when a fresh episode starts.

use serde::{Deserialize, Serialize};

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
/// behaviour are driven with it — and **not** the P1-C foraging heuristic, which reads
/// the senses this phase does not have yet.
#[derive(Clone, Debug, Default)]
pub struct Scripted {
    script: Vec<Actions>,
    next: usize,
}

impl Scripted {
    /// Cycle `script` forever; an empty script rests.
    pub fn new(script: Vec<Actions>) -> Scripted {
        Scripted { script, next: 0 }
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
}

/// The named module a controller reads from its observation by manifest slot.
fn module_slot(manifest: &Manifest, name: &str) -> crate::manifest::Module {
    *manifest
        .modules
        .iter()
        .find(|m| m.name == name)
        .unwrap_or_else(|| panic!("manifest has no {name} module"))
}

// The teachers' escape reflex (contract v2, P5-A item 9). Heuristic internals, not model
// numbers: they shape a teacher's behaviour and nothing in the world reads them.

/// `Self.motor_delivery` below this is a refused motor: a wall, a drop past the climb,
/// water past the wade depth, or the strip's edge.
const MOTOR_REFUSED: f64 = 0.5;
/// Controller samples an escape lasts once triggered: a full-effort turn, the first
/// sample standing still and the rest creeping.
const ESCAPE_SAMPLES: u32 = 3;
/// Forward effort while turning away, after the first sample.
const ESCAPE_CREEP: f64 = 0.3;
/// The share of a sector's 15 rays that are clear on flat open ground: the two
/// downward pitch rows (−40°, −20°) hit the floor and the other nine do not. More clear
/// than that, by half a ray, means a downward ray found nothing within range ahead.
const FLAT_CLEAR: f64 = 9.0 / 15.0;
const HALF_RAY: f64 = 0.5 / 15.0;
/// A forward sector whose only hits are the floor rows, reading this far away on
/// average, is looking over an edge rather than at the ground ahead: the floor rows of
/// an adult on flat ground read ≈0.83, a one-voxel step down ≈0.65 (walkable), a drop of
/// two voxels ≈0.47.
const DROP_PROXIMITY: f64 = 0.55;

// The teachers' no-signal search (P5-C S3): a **correlated walk**. Heuristic internals,
// backlog §1 placeholders: nothing in the world reads them.

/// Samples a wander holds its heading, drawn uniformly from this range: 2–6 s at the
/// 0.25 s cadence.
const WANDER_HOLD: (u32, u32) = (8, 24);
/// Samples a wander turn lasts, drawn uniformly from this range, at [`WANDER_TURN`]
/// effort: a bounded random angle.
const WANDER_TURN_SAMPLES: (u32, u32) = (1, 4);
/// Turn effort while a wander turn runs.
const WANDER_TURN: f64 = 0.8;

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
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    fn draw(&mut self, (lo, hi): (u32, u32)) -> u32 {
        lo + (self.next() % u64::from(hi - lo + 1)) as u32
    }

    /// Seed from the observation on the first call after a reset.
    fn seed(&mut self, o: &[f64]) {
        if self.seeded {
            return;
        }
        self.seeded = true;
        self.state = o.iter().fold(0x5EED_0F_A11C_E5u64, |h, v| {
            (h ^ v.to_bits()).wrapping_mul(0x0100_0000_01B3)
        });
        self.hold = self.draw(WANDER_HOLD);
    }

    /// This sample's wander turn effort.
    fn turn(&mut self, o: &[f64]) -> f64 {
        self.seed(o);
        if self.turning > 0 {
            self.turning -= 1;
            if self.turning == 0 {
                self.hold = self.draw(WANDER_HOLD);
            }
            return self.sign * WANDER_TURN;
        }
        if self.hold > 0 {
            self.hold -= 1;
            return 0.0;
        }
        self.sign = if self.next() & 1 == 0 { 1.0 } else { -1.0 };
        self.turning = self.draw(WANDER_TURN_SAMPLES) - 1;
        if self.turning == 0 {
            self.hold = self.draw(WANDER_HOLD);
        }
        self.sign * WANDER_TURN
    }

    /// Whichever way the walk last turned: the side an escape turns to when nothing else
    /// picks one.
    fn side(&self) -> f64 {
        if self.sign == 0.0 { 1.0 } else { self.sign }
    }

    /// A block or a signal interrupts the walk: hold again afterwards.
    fn interrupt(&mut self) {
        self.turning = 0;
        if self.seeded {
            self.hold = self.draw(WANDER_HOLD);
        }
    }
}

/// The escape reflex both teachers share: returns `Some((forward, turn))` while an
/// escape is running, arming it when `hazard` is seen.
fn escape(escape: &mut u32, hazard: bool, turn: f64) -> Option<(f64, f64)> {
    if hazard {
        *escape = ESCAPE_SAMPLES;
        return Some((0.0, turn));
    }
    if *escape > 0 {
        *escape -= 1;
        return Some((ESCAPE_CREEP, turn));
    }
    None
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
}

impl BlindForager {
    /// The blind founder's heuristic, pre-indexed against its manifest.
    pub fn new() -> BlindForager {
        let manifest = Founder::Blind.manifest();
        BlindForager {
            turn_bias: 1.0,
            wander: Wander::default(),
            escape: 0,
            contact: module_slot(&manifest, "Contact(4)"),
            taste: module_slot(&manifest, "Taste(1)"),
            chem: module_slot(&manifest, "Chem(detritus)"),
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

        // On a terrain face (package mobility): nothing is under the body — on the
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
        let front_blocked = front > 0.5;
        let forward = if front_blocked { 0.1 } else { 1.0 };
        // The feed gate is calibrated to the field's own scale: a settled full tile
        // reads ≈0.29 at the receptor (bilinear over the node's neighbourhood), so a
        // gate near 0.2 is "the cue here is near-source". Failed attempts off the stock
        // are free — the gate costs nothing but keeps feeding honest.
        let feed = if taste_valid > 0.5 && taste_resp > 0.2 {
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
        if let Some((forward, turn)) = escape(&mut self.escape, o[7] < MOTOR_REFUSED, away) {
            self.wander.interrupt();
            return Response::Bounded(Actions {
                forward,
                turn,
                feed,
            });
        }
        let turn = if front_blocked {
            // A wall is a physical feature, not a target: turn away from the contacted
            // side, or fall back on the walk's last side.
            self.wander.interrupt();
            if left > right {
                -1.0
            } else if right > left {
                1.0
            } else {
                self.wander.side() * 0.8
            }
        } else if chem_valid > 0.5 && chem_resp >= 0.02 {
            self.wander.interrupt();
            if chem_trend > 0.05 {
                // A rising signal: hold the heading the body already faces.  The
                // Stage-A arena deliberately starts aimed at the food; even the old
                // small steering term bent that direct approach into a circle before
                // the mouth could reach the patch.
                0.0
            } else if chem_trend < -0.05 {
                // A falling signal: change course to a remembered new preference.
                -self.turn_bias * 0.8
            } else {
                // The initial chemical sample has no temporal history, hence a flat
                // trend.  It still means "continue this observable approach", not
                // "begin a remembered search arc".
                0.0
            }
        } else {
            // No signal: cover ground with a correlated walk (S3).
            self.wander.turn(o)
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
}

impl BrowserForager {
    /// The browser founder's heuristic, pre-indexed against its manifest.
    pub fn new() -> BrowserForager {
        let manifest = Founder::Browser.manifest();
        BrowserForager {
            wander_bias: 1.0,
            wander_ticks: 0,
            escape: 0,
            contact: module_slot(&manifest, "Contact(4)"),
            taste: module_slot(&manifest, "Taste(1)"),
            cone: module_slot(&manifest, "Cone(3, foliage/body)"),
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
        let feed = if taste_valid > 0.5 && taste_resp > 0.25 {
            1.0
        } else {
            0.0
        };
        // The forward sector (s1) read against flat ground: its downward rows escaping,
        // or its only hits far below.
        let (clear, proximity) = (o[base + 6], o[base + 7]);
        let drop = clear > FLAT_CLEAR + HALF_RAY
            || (clear >= FLAT_CLEAR - HALF_RAY && proximity < DROP_PROXIMITY);
        let away = self.wander_bias;
        if let Some((forward, turn)) = escape(&mut self.escape, o[7] < MOTOR_REFUSED || drop, away)
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
        if frac[0] > 0.02 || frac[1] > 0.02 || frac[2] > 0.02 {
            self.wander_ticks = 0;
            if frac[0] >= frac[1] && frac[0] >= frac[2] {
                turn = -0.8;
                self.wander_bias = -1.0;
            } else if frac[2] > frac[1] {
                turn = 0.8;
                self.wander_bias = 1.0;
            } else {
                turn = 0.0;
            }
        } else {
            self.wander_ticks += 1;
            if self.wander_ticks >= 3 {
                self.wander_ticks = 0;
                self.wander_bias = -self.wander_bias;
            }
            turn = self.wander_bias * 0.6;
        }
        let forward = if front > 0.5 { 0.1 } else { 1.0 };
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::manifest::Founder;

    /// S3: with no signal the shredder's teacher walks a correlated walk — they hold their heading
    /// for at least `WANDER_HOLD.0` samples between turns, a turn lasts at most
    /// `WANDER_TURN_SAMPLES.1` samples, both signs occur, and two bodies with different
    /// senses walk different walks.
    #[test]
    fn the_no_signal_walk_holds_heading_between_bounded_turns() {
        {
            let founder = Founder::Blind;
            let manifest = founder.manifest();
            let walk = |salt: f64| -> Vec<f64> {
                let mut c: Box<dyn Controller> = match founder {
                    Founder::Blind => Box::new(BlindForager::new()),
                    Founder::Browser => Box::new(BrowserForager::new()),
                };
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
                holds.iter().all(|&h| h >= WANDER_HOLD.0),
                "{founder:?}: the heading is held between turns: {holds:?}"
            );
            assert!(
                runs.iter()
                    .filter(|r| r.0)
                    .all(|r| r.1 <= WANDER_TURN_SAMPLES.1),
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
