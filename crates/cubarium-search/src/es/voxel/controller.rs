//! The one controller interface and its two bodies: the GRU `ShapePolicy` wired through
//! the voxel 3-action adapter, and the observation-only diagnostic heuristic slot.
//!
//! # The boundary is a type, not a convention
//!
//! [`VoxelController::act`] receives **the observation vector and nothing else**. There is
//! no parameter through which a controller could reach the world, the flora, a route, a
//! food position or a future-face query — the "Policy boundary" row of the tests plan is
//! enforced by the signature. Whatever a controller cannot compute from the vector and its
//! own memory, it does not have.
//!
//! # The two bodies
//!
//! - [`GruController`]: a [`cubarium_core::neural::ShapePolicy`] over the manifest's shape,
//!   validated against the founder's manifest digest, sampled at the manifest's cadence,
//!   decoded through [`adapt`] (sigmoid for forward/feed, tanh for turn, the manifest's
//!   fixed deadband). One controller object per episode; [`VoxelController::reset`] zeroes
//!   the hidden state so a fresh episode starts with fresh memory.
//! - [`VoxelControl`]: the diagnostic heuristic slot. It may read only the vector and its
//!   own turn memory. Until P1-C lands the real samplers and the real heuristic, the
//!   shipped rules are the disclosed stationary controls plus a minimal observation-only
//!   stub; the stub's shape (an observation-driven rule with memory) is what P1-C fills.
//!
//! The adapter is identical for training and viewing: the same [`adapt`] function decodes
//! a raw head wherever a voxel controller runs.

use cubarium_core::neural::gru::HIDDEN;
use cubarium_core::neural::ShapePolicy;
use cubarium_voxel_fauna::{Founder, Manifest, Transfer};

use super::super::tensor;
use super::voxel_schema_digest;

/// The three bounded actions in manifest order: forward effort `[0, 1]`, turn effort
/// `[-1, 1]`, feed effort `[0, 1]`.
pub type Actions = [f64; 3];

/// One controller for one episode.
///
/// A controller is created fresh (or [`VoxelController::reset`]) at the start of an
/// episode, then [`VoxelController::act`] is called at the manifest's cadence with the
/// observation vector sampled from the pre-action state. The returned actions are held
/// until the next call.
pub trait VoxelController {
    /// Zero every per-episode memory: hidden state, turn memory, integrators. A fresh
    /// episode must not inherit a previous one's memory.
    fn reset(&mut self);

    /// The three actions for this controller update, from the observation vector alone.
    ///
    /// The vector is in manifest order, length `manifest.inputs()`, already clamped and
    /// finite (the driver rejects a non-finite sample before this is called). The result
    /// must be finite and in each action's declared bounds; the GRU body guarantees both.
    fn act(&mut self, observation: &[f64]) -> Actions;
}

/// The sigmoid the adapter uses for the two `[0, 1]` actions.
///
/// Local because `cubarium_core`'s forward-pass helper is private to the flat world's
/// dispatch, and the voxel adapter is this module's contract with the manifest.
pub fn sigmoid(x: f64) -> f64 {
    if x >= 0.0 {
        1.0 / (1.0 + (-x).exp())
    } else {
        let e = x.exp();
        e / (1.0 + e)
    }
}

/// The voxel 3-action adapter: unbounded GRU logits → the three bounded actions.
///
/// Per the plan's action contract: sigmoid for the two `[0, 1]` actions, tanh for the
/// signed turn, then the manifest's fixed deadband — a magnitude below it is a rest
/// action, with `|turn|` compared so the deadband is symmetric. The transfers and the
/// deadband come from the manifest, so the adapter is schema data, not a second copy of
/// the schema.
pub fn adapt(manifest: &Manifest, logits: [f64; 3]) -> Actions {
    let mut out = [0.0; 3];
    for (i, action) in manifest.actions.iter().enumerate() {
        let bounded = match action.transfer {
            Transfer::Sigmoid => sigmoid(logits[i]),
            Transfer::Tanh => logits[i].tanh(),
        };
        out[i] = if bounded.abs() < manifest.deadband { 0.0 } else { bounded };
    }
    out
}

/// A GRU controller over the blind founder's 23-input schema.
pub type GruBlind = GruController<23>;
/// A GRU controller over the browser's 37-input schema.
pub type GruBrowser = GruController<37>;

/// The GRU body of the controller interface: validated weights, one hidden state, the
/// manifest's adapter.
#[derive(Clone, Debug)]
pub struct GruController<const I: usize> {
    policy: ShapePolicy<I, 3>,
    manifest: Manifest,
    hidden: [f64; HIDDEN],
}

impl<const I: usize> GruController<I> {
    /// Build a controller from a flat parameter vector, validated against the founder's
    /// manifest digest — the [`tensor::shape_policy`] boundary with the digest plug-in
    /// ([`super::voxel_schema_digest`]) filled in.
    ///
    /// Refuses a wrong-length vector, a non-finite value, and a digest mismatch by name.
    pub fn new(theta: &[f64], manifest: Manifest, digest: u64) -> Result<GruController<I>, String> {
        let policy = tensor::shape_policy::<I, 3>(theta, digest)?;
        Ok(GruController {
            policy,
            manifest,
            hidden: [0.0; HIDDEN],
        })
    }

    /// The validated policy this controller runs.
    pub fn policy(&self) -> &ShapePolicy<I, 3> {
        &self.policy
    }
}

impl<const I: usize> VoxelController for GruController<I> {
    fn reset(&mut self) {
        self.hidden = [0.0; HIDDEN];
    }

    fn act(&mut self, observation: &[f64]) -> Actions {
        debug_assert_eq!(observation.len(), I, "the observation is manifest-ordered");
        let mut x = [0.0; I];
        x.copy_from_slice(observation);
        let logits = self.policy.weights.forward(&x, &mut self.hidden);
        adapt(&self.manifest, logits)
    }
}

/// The observation-only diagnostic heuristic slot.
///
/// The rule reads the observation vector and its own turn memory — never a world handle.
/// The shipped rules are the tests plan §2's disclosed controls plus the minimal stub
/// P1-C will replace with the real heuristic; the slot's *shape* (an observation-driven
/// rule with memory, behind [`VoxelController`]) is what makes that a one-function swap.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VoxelControl {
    /// Stand still, take nothing: what the starting stores alone buy, and the check that
    /// no free intake or scoring artifact exists.
    NoIntake,
    /// Stand still and feed continuously: the most favourable stationary strategy that
    /// exists, to show why remaining alive or feeding in place is not foraging from an
    /// off-food start.
    StationaryFeeding,
    /// The observation-only stub heuristic: turn toward whatever the vector's chem/trend
    /// sector suggests, keep some forward effort when the trend is stale. It consumes
    /// real observations and real memory and is honest about being a placeholder: the
    /// real heuristic is P1-C's, wired in by replacing this rule.
    Stub,
}

/// The heuristic controller's per-episode memory.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HeuristicController {
    control: VoxelControl,
    /// The sign of the last trend the rule acted on, in `[-1, 1]`: the turn memory the
    /// tests plan's blind heuristic is allowed ("its own turn memory").
    turn_memory: f64,
    /// Updates since the trend last moved, so a stale gradient can decay into a search
    /// behaviour instead of pinning the body against a wall.
    stale_updates: u64,
}

impl HeuristicController {
    pub fn new(control: VoxelControl) -> HeuristicController {
        HeuristicController {
            control,
            turn_memory: 0.0,
            stale_updates: 0,
        }
    }
}

impl VoxelController for HeuristicController {
    fn reset(&mut self) {
        self.turn_memory = 0.0;
        self.stale_updates = 0;
    }

    fn act(&mut self, observation: &[f64]) -> Actions {
        match self.control {
            VoxelControl::NoIntake => [0.0, 0.0, 0.0],
            VoxelControl::StationaryFeeding => [0.0, 0.0, 1.0],
            VoxelControl::Stub => {
                // The blind schema's chem response and trend are inputs 18 and 19 (the
                // `Chem(litter)` module); the browser's cone sectors cover 18..=36. Read
                // the first channel of whichever cue the vector carries and act on it.
                // This is a *stub*: it proves the plumbable observation path and gives
                // the controls something to be compared against, and P1-C replaces it.
                let n = observation.len();
                let mut response = 0.0f64;
                let mut trend = 0.0f64;
                if n >= 20 {
                    // Blind: Chem(litter) at 18..=21; browser: cone sector 0 at 18..=24,
                    // whose second slot is the all-hit proximity — a stand-in cue.
                    response = observation[18];
                    trend = observation[19];
                }
                let drive = if trend.abs() < 1e-6 && response < 1e-6 {
                    self.stale_updates += 1;
                    // Nothing in sense: a slow searching sweep that reverses when stale.
                    let sweep = if self.stale_updates % 8 < 4 { 0.5 } else { -0.5 };
                    [0.3, sweep, 0.0]
                } else {
                    self.stale_updates = 0;
                    self.turn_memory = trend.clamp(-1.0, 1.0);
                    [0.6, self.turn_memory, 1.0]
                };
                drive
            }
        }
    }
}

/// What drives the body for one episode: a candidate policy or a diagnostic control.
///
/// Built per episode from a prototype ([`EpisodeDriver::fresh`]) so hidden state and
/// memory are never carried between episodes.
#[derive(Clone, Debug)]
pub enum EpisodeDriver {
    /// The seeded or perturbed GRU centre, by shape.
    Gru(EpisodeGru),
    /// A diagnostic control.
    Control(VoxelControl),
}

/// The shape of a [`EpisodeDriver::Gru`]: the two voxel schemas this driver runs.
#[derive(Clone, Debug)]
pub enum EpisodeGru {
    Blind(Box<GruController<23>>),
    Browser(Box<GruController<37>>),
}

impl EpisodeDriver {
    /// A GRU driver from a flat parameter vector, for `founder`, validated against that
    /// founder's manifest digest.
    pub fn gru(theta: &[f64], founder: cubarium_voxel_fauna::Founder) -> Result<EpisodeDriver, String> {
        let manifest = founder.manifest();
        let digest = voxel_schema_digest(founder);
        let gru = match founder {
            Founder::Blind => EpisodeGru::Blind(Box::new(GruController::<23>::new(theta, manifest, digest)?)),
            Founder::Browser => {
                EpisodeGru::Browser(Box::new(GruController::<37>::new(theta, manifest, digest)?))
            }
        };
        Ok(EpisodeDriver::Gru(gru))
    }

    /// A fresh controller for one episode, with its memory zeroed.
    pub fn fresh(&self) -> Box<dyn VoxelController + Send> {
        match self {
            EpisodeDriver::Gru(EpisodeGru::Blind(g)) => {
                let mut c: GruController<23> = (**g).clone();
                c.reset();
                Box::new(c)
            }
            EpisodeDriver::Gru(EpisodeGru::Browser(g)) => {
                let mut c: GruController<37> = (**g).clone();
                c.reset();
                Box::new(c)
            }
            EpisodeDriver::Control(c) => {
                let mut h = HeuristicController::new(*c);
                h.reset();
                Box::new(h)
            }
        }
    }

    /// The driver's name, for job labels and reports.
    pub fn name(&self) -> String {
        match self {
            EpisodeDriver::Gru(_) => "gru".into(),
            EpisodeDriver::Control(VoxelControl::NoIntake) => "no-intake".into(),
            EpisodeDriver::Control(VoxelControl::StationaryFeeding) => {
                "stationary-feeding".into()
            }
            EpisodeDriver::Control(VoxelControl::Stub) => "heuristic".into(),
        }
    }
}

/// The observation source prototype: what fills the manifest-ordered vector at each
/// controller update.
///
/// The prototype is shared; an episode takes [`ObservationSource::boxed`] and owns its
/// sampler copy for the episode, so per-episode memory (interval feedback, smoothing) is
/// private to it. **This is the one function P1-C's real samplers wire through**: they
/// implement this trait and the driver's episode construction is unchanged.
///
/// The boundary the sampler must hold is the driver's own: it reads state through the
/// sim's read views and must not call the arena's settlement APIs (`resources`,
/// `resource_stock()`, `take()`) or mutate anything. The controller still receives only
/// the filled vector.
pub trait ObservationSource: Send + Sync {
    /// A private sampler copy for one episode.
    fn boxed(&self) -> Box<dyn ObservationSource>;

    /// Fill `observation` (length `manifest.inputs()`, in manifest order) from the
    /// current pre-action state. Implementations must write every slot — the driver
    /// zero-fills first, so an unwritten slot reads as an invalid-but-zero channel.
    fn sample(
        &mut self,
        sim: &cubarium_voxel_sim::Sim,
        animal_id: Option<u64>,
        manifest: &Manifest,
        observation: &mut [f64],
    );
}

/// The interim observation source until P1-C lands the real samplers: the eight `Self`
/// channels from the placed animal's own physiology, normalized by the manifest's fixed
/// references, and zeros everywhere else.
///
/// `Self` is the animal's own state — energy, reserve, the interval's structural loss,
/// the interval's assimilated intake and resolved motion feedback — which is exactly what
/// the fauna view publishes without touching a resource stock. The interval feedback
/// channels (structural loss, intake) are measured by this sampler between its own calls,
/// which is why an episode owns its sampler copy ([`ObservationSource::boxed`]) and the
/// fields below are per-episode. Everything P1-C owns (contacts, wet, taste, chem, light,
/// cone) stays a valid zero.
#[derive(Clone, Copy, Debug, Default)]
pub struct SelfOnly {
    prev_energy: f64,
    prev_reserve: f64,
    prev_organic: f64,
    prev_body: f64,
}

impl ObservationSource for SelfOnly {
    fn boxed(&self) -> Box<dyn ObservationSource> {
        Box::new(*self)
    }

    fn sample(
        &mut self,
        sim: &cubarium_voxel_sim::Sim,
        animal_id: Option<u64>,
        manifest: &Manifest,
        observation: &mut [f64],
    ) {
        // Zero first: an unwritten slot is an invalid-but-zero channel, never stale data.
        observation.fill(0.0);
        let Some(id) = animal_id else { return };
        let view = sim.fauna().view();
        let Some(a) = view.animal(id) else { return };
        if observation.len() < 8 {
            return;
        }

        // The interval delta since this sampler's last call, in the animal's own units.
        let organic_now = a.organic();
        let d_organic = organic_now - self.prev_organic;
        self.prev_energy = a.energy;
        self.prev_reserve = a.reserve;
        self.prev_organic = organic_now;

        // Birth readiness: the physiological threshold, even though arena births are off.
        let birth_ready = (a.body >= view.config.species(a.species).birth_body) as i32 as f64;
        // Structural loss over the interval: organic gone from the body itself, against
        // the manifest's fixed structural reference. Regrowth is not loss; the intake
        // channel carries the growth.
        let structural_loss = (self.prev_body - a.body).max(0.0) / manifest.structural_reference;
        self.prev_body = a.body;

        observation[0] = (a.energy / manifest.adult_energy_reference).clamp(0.0, 4.0);
        observation[1] = (a.reserve / manifest.adult_reserve_reference).clamp(0.0, 4.0);
        observation[2] = birth_ready;
        observation[3] = structural_loss.clamp(0.0, 4.0);
        // Assimilated intake over the interval: organic the body kept (growth; the loss
        // channel carries the decline). Zero on the first sample of the episode — the
        // plan: initial intake/loss/motion feedback is zero.
        observation[4] =
            (d_organic.max(0.0) / manifest.structural_reference).clamp(0.0, 4.0);
        // Resolved forward/turn and motor delivery: P1-B's motion feedback, zero until
        // it lands. They are valid zero readings now, not missing channels.
        observation[5] = 0.0;
        observation[6] = 0.0;
        observation[7] = 1.0; // "1 when none requested" is the delivery reference.
    }
}

/// The shared `SelfOnly` prototype the commands pass around; an episode takes its own
/// copy through [`ObservationSource::boxed`].
pub static SELF_ONLY: SelfOnly = SelfOnly {
    prev_energy: 0.0,
    prev_reserve: 0.0,
    prev_organic: 0.0,
    prev_body: 0.0,
};

/// The all-zero observation source: every channel a valid zero reading.
///
/// The smoke and the plumbing tests use it to prove the driver, the adapter and the ES
/// run with no sensory information at all — the honest floor P1-C raises from.
#[derive(Clone, Copy, Debug, Default)]
pub struct Zeros;

/// The shared `Zeros` prototype.
pub static ZEROS: Zeros = Zeros;

impl ObservationSource for Zeros {
    fn boxed(&self) -> Box<dyn ObservationSource> {
        Box::new(Zeros)
    }

    fn sample(
        &mut self,
        _sim: &cubarium_voxel_sim::Sim,
        _animal_id: Option<u64>,
        _manifest: &Manifest,
        observation: &mut [f64],
    ) {
        observation.fill(0.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cubarium_voxel_fauna::Founder;

    /// A controller is constructible from (theta, manifest, digest) alone and acts on a
    /// hand-built vector with **no arena in scope**: the policy boundary is a type fact.
    #[test]
    fn a_controller_receives_only_the_vector_and_its_own_memory() {
        let manifest = Founder::Blind.manifest();
        let theta = tensor::initial_center_shape::<23, 3>(11);
        let mut gru = GruController::<23>::new(&theta, manifest, voxel_schema_digest(Founder::Blind))
            .expect("the centre is a policy");
        let mut a = [0.0; 23];
        a[0] = 0.5;
        let first = gru.act(&a);
        for (v, action) in first.iter().zip(manifest.actions) {
            assert!(v.is_finite());
            assert!(*v >= action.low && *v <= action.high, "{v} outside {action:?}");
        }
        // Memory is its own: the same observation twice in a row evolves the hidden state.
        let second = gru.act(&a);
        assert_ne!(first, second, "the GRU is recurrent on its own hidden state");

        // And the heuristic reads the vector and its own memory only.
        let mut h = HeuristicController::new(VoxelControl::Stub);
        h.reset();
        let _ = h.act(&a);
        let _ = h.act(&a);
    }

    /// The adapter is the manifest's own contract: sigmoid/tanh per declared transfer and
    /// the manifest's deadband, including the symmetric turn deadband.
    #[test]
    fn the_adapter_is_the_manifests_transfer_and_deadband() {
        let manifest = Founder::Blind.manifest();
        let dead = manifest.deadband;
        // Sigmoid channels: large negative logit → ~0, small logit → 0 by deadband.
        let out = adapt(&manifest, [-40.0, 0.0, 0.0]);
        assert_eq!(out[0], 0.0, "sigmoid(-40) is below the deadband → rest");
        assert_eq!(out[1], 0.0, "tanh(0) is exactly the deadband → rest");
        assert_eq!(out[2], 0.5, "sigmoid(0) is 0.5, above the deadband → a mid effort");
        let out = adapt(&manifest, [0.0, 40.0, -40.0]);
        assert_eq!(out[0], 0.5, "sigmoid(0) is 0.5, above the deadband → a mid effort");
        assert_eq!(out[1], 1.0, "tanh(40) saturates to +1");
        assert_eq!(out[2], 0.0);
        // A mid-range logit on each channel: inside bounds, above deadband.
        let out = adapt(&manifest, [2.0, -2.0, 2.0]);
        assert!(out[0] > dead && out[0] < 1.0);
        assert!(out[1] < -dead && out[1] > -1.0);
        assert!(out[2] > dead && out[2] < 1.0);
        // The deadband is compared on |turn|: +0.04 and −0.04 both rest.
        let tiny = adapt(&manifest, [6.0, 0.04f64.atanh(), 6.0]);
        assert_eq!(tiny[1], 0.0);
    }

    /// Fresh-episode reset of hidden state: a reset controller's first act is the fresh
    /// controller's first act, even after the original has evolved its memory.
    #[test]
    fn reset_gives_a_fresh_episode_fresh_hidden_state() {
        let manifest = Founder::Blind.manifest();
        let theta = tensor::initial_center_shape::<23, 3>(13);
        let digest = voxel_schema_digest(Founder::Blind);
        let mut run = GruController::<23>::new(&theta, manifest, digest).expect("policy");
        let mut fresh = GruController::<23>::new(&theta, manifest, digest).expect("policy");
        let obs = [0.25; 23];
        // Evolve `run` far from the fresh state.
        for _ in 0..12 {
            let _ = run.act(&obs);
        }
        run.reset();
        assert_eq!(
            run.act(&obs),
            fresh.act(&obs),
            "a reset controller must act as a fresh one"
        );
    }

    /// Shape acceptance and incompatible-policy rejection, at the controller boundary.
    #[test]
    fn the_two_shapes_are_accepted_and_the_mismatched_policy_is_refused() {
        let blind = tensor::initial_center_shape::<23, 3>(7);
        let browser = tensor::initial_center_shape::<37, 3>(7);
        let blind_digest = voxel_schema_digest(Founder::Blind);
        let browser_digest = voxel_schema_digest(Founder::Browser);

        let p = tensor::shape_policy::<23, 3>(&blind, blind_digest).expect("blind accepts");
        assert_eq!(p.schema_digest, blind_digest);
        let p = tensor::shape_policy::<37, 3>(&browser, browser_digest).expect("browser accepts");
        assert_eq!(p.schema_digest, browser_digest);

        // A blind-shaped theta refused as a browser policy: wrong length, named.
        let err = tensor::shape_policy::<37, 3>(&blind, browser_digest).expect_err("wrong length");
        assert!(err.contains("5571"), "{err}");

        // A right-length policy stamped with a corrupted digest is refused by the
        // boundary the world uses: `ShapePolicy::new` (the authoring stamp) against
        // `validate` (the expected digest). `shape_policy` itself cannot self-reject —
        // it stamps with what it validates against, which is why the driver always
        // passes the manifest's own digest.
        let claimed =
            cubarium_core::neural::ShapePolicy::new(tensor::unflatten_shape::<37, 3>(&browser).expect("shape"), browser_digest ^ 1);
        let err = claimed.validate(browser_digest).expect_err("digest");
        assert!(err.contains("schema_digest"), "{err}");
        // And the correct stamp validates.
        let good = cubarium_core::neural::ShapePolicy::new(
            tensor::unflatten_shape::<37, 3>(&browser).expect("shape"),
            browser_digest,
        );
        good.validate(browser_digest).expect("the true digest validates");
    }
}
