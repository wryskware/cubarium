//! **The policy-gradient learner's knobs** (PG-PILOT): one `[<lineage>.pg]` table of the kin
//! file, read only by a lineage on [`crate::es::voxel::kin::Comparison::PolicyGradient`].
//! Training-side, like the twins layout: nothing in the sim or a save.
//!
//! Discount and GAE's λ are given as **time horizons**, because a body's decisions are not
//! evenly spaced (4 Hz scheduled, plus forced ones, and none while it sleeps): between two
//! decisions `Δt` seconds apart the return is discounted by `γ = exp(−Δt / discount_horizon_s)`
//! and the advantage's trace by `λ = exp(−Δt / lambda_horizon_s)`.
//!
//! SCORE-V2 (`design/handoffs/score-v2-2026-10-15.md`) adds the score's two halves and the
//! pull toward the clone:
//!
//! - [`JudgeKnobs`] (`[<lineage>.pg.judge]`): the **judge**'s life values, outcomes only
//!   ([`super::judge`]); every comparison and brain-vs-rules check reads the judge;
//! - [`TeacherKnobs`] (`[<lineage>.pg.teacher]`): the **teacher**'s drive weights, the
//!   judge per step plus hunger, wound load and sleep debt in potential form (one weight
//!   each, the same default for every lineage);
//! - `clone_kl` and `clone_kl_fade_generations`: the KL from the clone's intent
//!   distribution on the same input, beside the entropy bonus.
//!
//! A kin file written before SCORE-V2 that names none of these gets the defaults below;
//! `config/training/pg-pilot-2026-10-14.toml` pins every one of them off, so pilot v1's
//! file still reproduces its own reward.

use cubarium_voxel::registry::{Range, Row};
use serde::{Deserialize, Serialize};

/// One lineage's policy-gradient knobs.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct PgKnobs {
    /// Return discount horizon, simulated seconds: a reward `Δt` seconds after a decision
    /// counts `exp(−Δt / discount_horizon_s)` toward its return.
    pub discount_horizon_s: f64,
    /// GAE trace horizon, simulated seconds: λ between two decisions `Δt` apart is
    /// `exp(−Δt / lambda_horizon_s)` (long: closer to the Monte-Carlo return; short: leans on
    /// the value baseline).
    pub lambda_horizon_s: f64,
    /// PPO's ratio clip `ε` (dimensionless): a decision's probability ratio to the policy that
    /// sampled it counts only inside `[1 − ε, 1 + ε]` in the direction its advantage pushes.
    pub clip: f64,
    /// Entropy bonus weight (per decision, on the normalised-advantage scale): keeps the
    /// sampled choice from collapsing before the advantages say so.
    pub entropy: f64,
    /// Adam learning rate of the top's intent head (the lineage's own optimizer state).
    pub policy_lr: f64,
    /// Adam learning rate of the value baseline.
    pub value_lr: f64,
    /// Hidden width of the value baseline's one tanh layer (count). The baseline reads the
    /// top's input: the frozen bottom's GRU state, the held intent and the time held.
    pub value_hidden: usize,
    /// Passes over one generation's decisions (count).
    pub epochs: usize,
    /// Minibatches per pass, one Adam step of each net per minibatch (count).
    pub minibatches: usize,
    /// Random half-splits averaged into the split-half cosine readout (count; readout only).
    pub splits: usize,
    /// **The pull toward the clone** (SCORE-V2; research topic 6(c)): weight of
    /// `KL(π_clone ‖ π)` per decision, on the normalised-advantage scale, where `π_clone` is
    /// the intent proposal of the centre the lineage started from ([`super::PgState::anchor`])
    /// on the same input at the same temperature. 0: no pull.
    pub clone_kl: f64,
    /// Generations over which `clone_kl` fades linearly to 0 (count; 0: constant).
    pub clone_kl_fade_generations: f64,
    /// The judge's life values (`[<lineage>.pg.judge]`).
    pub judge: JudgeKnobs,
    /// The teacher's drive weights (`[<lineage>.pg.teacher]`).
    pub teacher: TeacherKnobs,
}

/// **The judge's life values** (SCORE-V2, Wrysk's "Score topic 2"): what a death costs on top
/// of the stored energy it takes with it, and what an own young born adds, per lineage and
/// per life stage ([`super::judge::LifeStage`]), in body references (`body_ref`, the
/// lineage manifest's `body_reference`). The learned value baseline
/// sits underneath.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct JudgeKnobs {
    /// A young's life value (below breeding size, or riding a host), body references: lost
    /// when a young dies, added to its mother when one is born to her.
    pub life_young: f64,
    /// A grown body's life value (at breeding size, carrying no offspring), body references.
    pub life_adult: f64,
    /// A breeding adult's life value (gestating, or holding a brood for its next clutch),
    /// body references.
    pub life_breeding: f64,
}

impl Default for JudgeKnobs {
    /// Small beside what a death already costs (the body's stored energy, about one body
    /// reference for a grown body), larger for the young and for breeding adults.
    fn default() -> JudgeKnobs {
        JudgeKnobs { life_young: 0.5, life_adult: 0.25, life_breeding: 0.5 }
    }
}

impl JudgeKnobs {
    /// Every life value 0: the judge is the condition and kin terms alone (pilot v1).
    pub fn off() -> JudgeKnobs {
        JudgeKnobs { life_young: 0.0, life_adult: 0.0, life_breeding: 0.0 }
    }

    pub fn is_off(&self) -> bool {
        *self == JudgeKnobs::off()
    }

    /// The life value of a body in `stage`, body references.
    pub fn life(&self, stage: super::judge::LifeStage) -> f64 {
        match stage {
            super::judge::LifeStage::Young => self.life_young,
            super::judge::LifeStage::Adult => self.life_adult,
            super::judge::LifeStage::Breeding => self.life_breeding,
        }
    }
}

/// **The teacher's drives** (SCORE-V2, Wrysk's "Score topic 3", option B): each drive enters
/// a decision's reward as `weight × (drive before − drive after)` (potential form, so over a
/// window it telescopes to `weight × (start − end)` and cannot pay for a loop). One weight
/// per drive, the same default for every lineage; temperament weights come later. Each
/// drive is dimensionless ([`super::judge::drives`]); the weights are in body references
/// per unit of drive.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct TeacherKnobs {
    /// Hunger, `1 − S/K` (stomach fill against capacity), body references per unit.
    pub hunger: f64,
    /// Wound load, the wound stock in `[0, 1)`, body references per unit.
    pub wound: f64,
    /// Sleep debt, time since its last deep bout over its sleep-need ceiling, body
    /// references per unit.
    pub sleep: f64,
}

impl Default for TeacherKnobs {
    fn default() -> TeacherKnobs {
        TeacherKnobs { hunger: 0.05, wound: 0.1, sleep: 0.05 }
    }
}

impl TeacherKnobs {
    /// Every drive weight 0: the teacher is the judge per step (pilot v1).
    pub fn off() -> TeacherKnobs {
        TeacherKnobs { hunger: 0.0, wound: 0.0, sleep: 0.0 }
    }

    /// The weights in [`super::judge::DRIVE_NAMES`] order.
    pub fn weights(&self) -> [f64; super::judge::DRIVES] {
        [self.hunger, self.wound, self.sleep]
    }
}

impl Default for PgKnobs {
    fn default() -> PgKnobs {
        PgKnobs {
            discount_horizon_s: 300.0,
            lambda_horizon_s: 60.0,
            clip: 0.2,
            entropy: 0.01,
            policy_lr: 3e-4,
            value_lr: 1e-3,
            value_hidden: 32,
            epochs: 4,
            minibatches: 4,
            splits: 16,
            clone_kl: 0.1,
            clone_kl_fade_generations: 0.0,
            judge: JudgeKnobs::default(),
            teacher: TeacherKnobs::default(),
        }
    }
}

impl PgKnobs {
    pub fn is_default(&self) -> bool {
        *self == PgKnobs::default()
    }

    /// `(γ, λ)` between two decisions `dt_s` seconds apart.
    pub fn factors(&self, dt_s: f64) -> (f64, f64) {
        let dt = dt_s.max(0.0);
        ((-dt / self.discount_horizon_s).exp(), (-dt / self.lambda_horizon_s).exp())
    }

    /// The clone pull's weight at `generation`: `clone_kl`, faded linearly to 0 over
    /// `clone_kl_fade_generations` (0: constant).
    pub fn clone_kl_at(&self, generation: u32) -> f64 {
        if self.clone_kl_fade_generations > 0.0 {
            self.clone_kl * (1.0 - f64::from(generation) / self.clone_kl_fade_generations).max(0.0)
        } else {
            self.clone_kl
        }
    }
}

/// The knobs' registry rows under `prefix` (the kin table mounts them at
/// `kin.lineages.*.pg`).
pub fn rows(prefix: &str) -> Vec<Row> {
    let row = |name: &str, unit: &'static str, range: Range, what: &'static str| {
        Row::new(format!("{prefix}.{name}"), unit, range, what)
    };
    vec![
        row("discount_horizon_s", "s", Range::positive(),
            "PG-PILOT: return discount horizon; a reward dt seconds after a decision counts exp(-dt/horizon)"),
        row("lambda_horizon_s", "s", Range::positive(),
            "PG-PILOT: GAE trace horizon; lambda between decisions dt apart is exp(-dt/horizon)"),
        row("clip", "ratio", Range::closed(0.0, 1.0).open_lo(),
            "PG-PILOT: PPO probability-ratio clip epsilon"),
        row("entropy", "weight", Range::nonneg(),
            "PG-PILOT: entropy bonus weight on the intent proposal, normalised-advantage scale"),
        row("policy_lr", "rate", Range::positive(),
            "PG-PILOT: Adam learning rate of the top's intent head"),
        row("value_lr", "rate", Range::positive(),
            "PG-PILOT: Adam learning rate of the value baseline"),
        row("value_hidden", "count", Range::closed(1.0, 1024.0),
            "PG-PILOT: hidden width of the value baseline's tanh layer"),
        row("epochs", "count", Range::closed(1.0, 64.0),
            "PG-PILOT: passes over a generation's decisions"),
        row("minibatches", "count", Range::closed(1.0, 4096.0),
            "PG-PILOT: minibatches per pass (one Adam step each)"),
        row("splits", "count", Range::closed(1.0, 1024.0),
            "PG-PILOT: random half-splits averaged into the split-half cosine readout"),
        row("clone_kl", "weight", Range::nonneg(),
            "SCORE-V2: weight of KL(clone || policy) on the intent proposal per decision, normalised-advantage scale; 0 = no pull"),
        row("clone_kl_fade_generations", "count", Range::nonneg(),
            "SCORE-V2: generations over which clone_kl fades linearly to 0; 0 = constant"),
        row("judge.life_young", "body_ref", Range::nonneg(),
            "SCORE-V2 judge: life value of a young (below breeding size), lost at its death and added to its mother at its birth"),
        row("judge.life_adult", "body_ref", Range::nonneg(),
            "SCORE-V2 judge: life value of a grown body carrying no offspring, lost at its death"),
        row("judge.life_breeding", "body_ref", Range::nonneg(),
            "SCORE-V2 judge: life value of a breeding adult (gestating or holding a brood), lost at its death"),
        row("teacher.hunger", "body_ref", Range::nonneg(),
            "SCORE-V2 teacher: weight of hunger (1 - stomach fill / capacity) as drive before - drive after"),
        row("teacher.wound", "body_ref", Range::nonneg(),
            "SCORE-V2 teacher: weight of wound load as drive before - drive after"),
        row("teacher.sleep", "body_ref", Range::nonneg(),
            "SCORE-V2 teacher: weight of sleep debt (time since the last deep bout / need_s) as drive before - drive after"),
    ]
}
