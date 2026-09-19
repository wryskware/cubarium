//! The controller bodies behind the fauna's **own** controller seam.
//!
//! P1-B landed the boundary this crate used to mirror: `cubarium_voxel_fauna::Controller`
//! (`drive(&mut self, observation) -> Response` + `reset()`), one shared adapter
//! (`resolve_actions`: the manifest's transfers and deadband, identical for training and
//! viewing), and a per-body controller table the fauna tick's controller stage drives.
//! This module now only **supplies bodies** for that seam:
//!
//! - [`GruPolicy`]: a validated [`ShapePolicy`] as a `Controller` returning
//!   [`Response::Logits`] — the shape-aware runtime the P1-B module doc reserves for
//!   P1-D. The adapter is *not* duplicated here: the fauna resolves logits through
//!   `resolve_actions`, so training and viewing decode identically by construction.
//! - [`NoIntake`] and [`StationaryFeeding`]: the two disclosed diagnostic controls, as
//!   bounded responses.
//! - The observation-only heuristic slot: the fauna's own `BlindForager` /
//!   `BrowserForager` — real observation-only controllers P1-B shipped; this crate does
//!   not keep a second heuristic.
//!
//! The policy boundary is enforced by the fauna's trait shape: a controller receives the
//! observation vector and its own memory and nothing else. What this module adds is the
//! digest discipline: a [`GruPolicy`] is built only through
//! [`tensor::shape_policy`] against the founder's manifest digest, so a policy authored
//! against one schema is refused before any episode runs.
//!
//! # Who samples, who holds, who resolves
//!
//! The **fauna tick** samples each due founder's observation (its own
//! `body::observation`), drives the installed controller, resolves the response through
//! the shared adapter, and holds the actions in the body's `founder_state.held` until
//! the next due tick. The **driver** installs a fresh controller per episode and calls
//! `reset()` — fresh memory by construction and by contract — then reads outcomes from
//! the world and ledger. It never samples, holds or resolves.

use cubarium_core::neural::ShapePolicy;
use cubarium_core::neural::gru::HIDDEN;
use cubarium_voxel_fauna::{BlindForager, BrowserForager, Controller, Founder, Response};

use super::super::tensor;
use super::voxel_schema_digest;

/// A GRU policy as a fauna controller: validated weights, one hidden state, raw logits
/// out. The fauna's shared adapter transfers and deadbands them.
#[derive(Clone, Debug)]
pub struct GruPolicy<const I: usize> {
    policy: ShapePolicy<I, 3>,
    hidden: [f64; HIDDEN],
}

impl<const I: usize> GruPolicy<I> {
    /// Build a controller from a flat parameter vector, validated against the founder's
    /// manifest digest — the [`tensor::shape_policy`] boundary with the digest plug-in
    /// ([`super::voxel_schema_digest`]) filled in.
    ///
    /// Refuses a wrong-length vector, a non-finite value, and a digest mismatch by name.
    pub fn new(theta: &[f64], digest: u64) -> Result<GruPolicy<I>, String> {
        Ok(GruPolicy {
            policy: tensor::shape_policy::<I, 3>(theta, digest)?,
            hidden: [0.0; HIDDEN],
        })
    }

    /// The validated policy this controller runs.
    pub fn policy(&self) -> &ShapePolicy<I, 3> {
        &self.policy
    }
}

impl<const I: usize> Controller for GruPolicy<I> {
    fn drive(&mut self, observation: &[f64]) -> Response {
        let mut x = [0.0; I];
        x.copy_from_slice(observation);
        let logits = self.policy.weights.forward(&x, &mut self.hidden);
        Response::Logits(logits)
    }

    fn reset(&mut self) {
        self.hidden = [0.0; HIDDEN];
    }
}

/// The blind founder's GRU runtime.
pub type GruBlind = GruPolicy<23>;
/// The browser's GRU runtime.
pub type GruBrowser = GruPolicy<37>;

/// Stand still, take nothing: what the starting stores alone buy, and the check that no
/// free intake or scoring artifact exists.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct NoIntake;

impl Controller for NoIntake {
    fn drive(&mut self, _observation: &[f64]) -> Response {
        Response::Bounded(cubarium_voxel_fauna::Actions::REST)
    }

    fn reset(&mut self) {}
}

/// Stand still and feed continuously: the most favourable stationary strategy that
/// exists, to show why remaining alive or feeding in place is not foraging from an
/// off-food start.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct StationaryFeeding;

impl Controller for StationaryFeeding {
    fn drive(&mut self, _observation: &[f64]) -> Response {
        Response::Bounded(cubarium_voxel_fauna::Actions {
            forward: 0.0,
            turn: 0.0,
            feed: 1.0,
        })
    }

    fn reset(&mut self) {}
}

/// What drives the body for one episode. Built per episode through
/// [`EpisodeDriver::fresh`] so the controller's memory — hidden state, turn preference —
/// is never carried between episodes.
#[derive(Clone, Debug)]
pub struct EpisodeDriver {
    founder: Founder,
    kind: EpisodeKind,
}

/// The controller body a driver runs.
#[derive(Clone, Debug)]
pub enum EpisodeKind {
    /// The seeded or perturbed GRU centre, by shape.
    Gru(EpisodeGru),
    /// The disclosed diagnostic controls and the observation-only heuristic.
    Control(VoxelControl),
}

/// The shape of an [`EpisodeKind::Gru`]: the two voxel schemas this driver runs.
#[derive(Clone, Debug)]
pub enum EpisodeGru {
    Blind(Box<ShapePolicy<23, 3>>),
    Browser(Box<ShapePolicy<37, 3>>),
}

/// The diagnostic controllers the commands name.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VoxelControl {
    NoIntake,
    StationaryFeeding,
    /// The observation-only heuristic: the fauna's own `BlindForager` (blind founder) or
    /// `BrowserForager` (browser) — real chem/gaze heuristics shipped with the seam,
    /// interchangeable here behind the same [`Controller`] trait.
    Heuristic,
}

impl EpisodeDriver {
    /// A GRU driver from a flat parameter vector, for `founder`, validated against that
    /// founder's manifest digest.
    pub fn gru(theta: &[f64], founder: Founder) -> Result<EpisodeDriver, String> {
        let digest = voxel_schema_digest(founder);
        let gru = match founder {
            Founder::Blind => {
                EpisodeGru::Blind(Box::new(tensor::shape_policy::<23, 3>(theta, digest)?))
            }
            Founder::Browser => {
                EpisodeGru::Browser(Box::new(tensor::shape_policy::<37, 3>(theta, digest)?))
            }
        };
        Ok(EpisodeDriver {
            founder,
            kind: EpisodeKind::Gru(gru),
        })
    }

    /// A diagnostic driver for `founder`.
    pub fn control(control: VoxelControl, founder: Founder) -> EpisodeDriver {
        EpisodeDriver {
            founder,
            kind: EpisodeKind::Control(control),
        }
    }

    /// The founder this driver runs.
    pub fn founder(&self) -> Founder {
        self.founder
    }

    /// The controller body a driver runs, for diagnostics and tests.
    pub fn kind(&self) -> &EpisodeKind {
        &self.kind
    }

    /// A fresh controller for one episode: new memory by construction, then an explicit
    /// `reset()` — the fauna's fresh-episode contract names `Controller::reset`, and
    /// doing both is the point of the check.
    pub fn fresh(&self) -> Box<dyn Controller> {
        let mut controller: Box<dyn Controller> = match &self.kind {
            EpisodeKind::Gru(EpisodeGru::Blind(policy)) => Box::new(GruPolicy::<23> {
                policy: (**policy).clone(),
                hidden: [0.0; HIDDEN],
            }),
            EpisodeKind::Gru(EpisodeGru::Browser(policy)) => Box::new(GruPolicy::<37> {
                policy: (**policy).clone(),
                hidden: [0.0; HIDDEN],
            }),
            EpisodeKind::Control(VoxelControl::NoIntake) => Box::new(NoIntake),
            EpisodeKind::Control(VoxelControl::StationaryFeeding) => Box::new(StationaryFeeding),
            // The heuristic slot is the fauna's own observation-only heuristics: the
            // body the driver runs decides which one fits its founder.
            EpisodeKind::Control(VoxelControl::Heuristic) => match self.founder {
                Founder::Blind => Box::new(BlindForager::new()),
                Founder::Browser => Box::new(BrowserForager::new()),
            },
        };
        controller.reset();
        controller
    }

    /// The driver's name, for job labels and reports.
    pub fn name(&self) -> String {
        match &self.kind {
            EpisodeKind::Gru(_) => "gru".into(),
            EpisodeKind::Control(VoxelControl::NoIntake) => "no-intake".into(),
            EpisodeKind::Control(VoxelControl::StationaryFeeding) => "stationary-feeding".into(),
            EpisodeKind::Control(VoxelControl::Heuristic) => "heuristic".into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cubarium_voxel_fauna::Actions;

    /// The GRU body answers raw logits — resolution is the fauna's shared adapter, not
    /// this crate's — and reset() clears its memory.
    #[test]
    fn the_gru_returns_logits_and_reset_clears_its_memory() {
        let theta = tensor::initial_center_shape::<23, 3>(11);
        let mut run =
            GruPolicy::<23>::new(&theta, voxel_schema_digest(Founder::Blind)).expect("policy");
        let mut fresh =
            GruPolicy::<23>::new(&theta, voxel_schema_digest(Founder::Blind)).expect("policy");
        let obs = [0.25; 23];
        let Response::Logits(first) = run.drive(&obs) else {
            panic!("the GRU answers logits");
        };
        for v in first {
            assert!(v.is_finite(), "logits are raw and finite");
        }
        // Memory is its own: the same observation twice evolves the hidden state.
        let Response::Logits(second) = run.drive(&obs) else {
            panic!("logits");
        };
        assert_ne!(
            first, second,
            "the GRU is recurrent on its own hidden state"
        );
        // Evolve far from the fresh state, reset, and compare against a fresh body.
        for _ in 0..12 {
            let _ = run.drive(&obs);
        }
        run.reset();
        assert_eq!(
            run.drive(&obs),
            fresh.drive(&obs),
            "a reset controller must act as a fresh one"
        );
    }

    /// The two disclosed controls are bounded responses, deadbanded by the fauna's own
    /// adapter into rest or full feed.
    #[test]
    fn the_controls_answer_bounded_rest_and_full_feed() {
        let manifest = Founder::Blind.manifest();
        let mut no = NoIntake;
        let resolved = cubarium_voxel_fauna::resolve_actions(no.drive(&[0.5; 23]), &manifest);
        assert_eq!(resolved, Actions::REST);
        let mut feed = StationaryFeeding;
        let resolved = cubarium_voxel_fauna::resolve_actions(feed.drive(&[0.5; 23]), &manifest);
        assert_eq!(
            resolved,
            Actions {
                forward: 0.0,
                turn: 0.0,
                feed: 1.0
            }
        );
        no.reset();
        feed.reset();
    }

    /// A driver hands out fresh memory per episode: two `fresh()` bodies are equal at
    /// their first drive, whatever the prototype has already done.
    #[test]
    fn fresh_hands_out_fresh_memory_per_episode() {
        let driver = EpisodeDriver::gru(&tensor::initial_center_shape::<23, 3>(13), Founder::Blind)
            .expect("policy");
        let mut a = driver.fresh();
        let mut b = driver.fresh();
        let obs = [0.1; 23];
        // Evolve `a` far from fresh.
        for _ in 0..9 {
            let _ = a.drive(&obs);
        }
        let mut a = driver.fresh();
        assert_eq!(a.drive(&obs), b.drive(&obs), "fresh is fresh, twice");
    }

    /// Shape acceptance and incompatible-policy rejection at the digest boundary.
    #[test]
    fn the_two_shapes_are_accepted_and_the_mismatched_policy_is_refused() {
        let blind = tensor::initial_center_shape::<23, 3>(7);
        let browser = tensor::initial_center_shape::<37, 3>(7);
        let browser_digest = voxel_schema_digest(Founder::Browser);

        let d = EpisodeDriver::gru(&blind, Founder::Blind).expect("blind accepts");
        assert_eq!(d.founder(), Founder::Blind);
        let d = EpisodeDriver::gru(&browser, Founder::Browser).expect("browser accepts");
        assert_eq!(d.founder(), Founder::Browser);

        // A blind-shaped theta refused as a browser policy: wrong length, named.
        let err = EpisodeDriver::gru(&blind, Founder::Browser).expect_err("wrong length");
        assert!(err.contains("5571"), "{err}");

        // A right-length policy stamped with a corrupted digest is refused by the
        // boundary the world uses: `ShapePolicy::new` (the authoring stamp) against
        // `validate` (the expected digest).
        let claimed = ShapePolicy::new(
            tensor::unflatten_shape::<37, 3>(&browser).expect("shape"),
            browser_digest ^ 1,
        );
        let err = claimed.validate(browser_digest).expect_err("digest");
        assert!(err.contains("schema_digest"), "{err}");
    }
}
