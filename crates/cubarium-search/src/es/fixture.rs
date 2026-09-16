//! The foraging fixtures: four frozen training layouts, eight held-out layouts, and the
//! construction that turns a layout into a real world.
//!
//! A layout is an **isolated capability episode**, not an ecosystem: one ordinary mature
//! grazer of one fixed genotype, identical stores and body in every episode, births disabled
//! equally through the diagnostic seam, and no other organism anywhere. Everything else is the
//! world's ordinary configuration — the current pace, the energy prices, the repaired
//! starvation and maturity rules, sensing costs, the real fields with their real renewal.
//! Nothing is subsidised: no survival floor, no regrowth change, no respawn, no care
//! replenishment, and no scripted motion for a learned candidate.
//!
//! # What a layout is
//!
//! Every field cell in the world is emptied of `P`, `F`, `D` and `De` — booked as an export so
//! the world's material box still closes and `check_invariants` still holds — and then the
//! layout's own patches are painted back in. So a layout's food is exactly what it declares,
//! and the world's totals are the layout's totals.
//!
//! A layout declares:
//!
//! - an **opening patch** centred on the grazer's start cell, at a stated fraction of `P_max`;
//!   some layouts open weak or depleted, which is a reason to leave, not a foodless scene;
//! - one or more **later patches** elsewhere on the face, reachable on the body's own paid
//!   motion inside the horizon;
//! - a **start cell** and a **start heading**, which differ between layouts, so a policy
//!   cannot memorise one bearing.
//!
//! The policy is given no destination and no global food information: it sees only the
//! 70-scalar observation the contract defines, whose food channels reach three cell hops at
//! most. Where the later food is has to be found.
//!
//! # Freezing
//!
//! `layout_hash` is FNV-1a over a canonical one-line description of the layout together with
//! the serialized `WorldConfig` it is built on. It is recorded with every result, so a later
//! run cannot quietly redefine the task and compare numbers across the change.

use std::f64::consts::FRAC_1_SQRT_2;
use std::path::Path;
use std::sync::Arc;

use cubarium_core::config::WorldConfig;
use cubarium_core::ids::OrganismId;
use cubarium_core::neural::ActionAdapter;
use cubarium_core::{DT, MotorModel, World};
use cubarium_surface::{CellId, Face, Vec2, cell_of};
use serde::{Deserialize, Serialize};

use super::rng::{gaussian, stream};

/// World ticks in the initial evaluation horizon: 36,000 at 20 Hz is 1,800 s.
pub const HORIZON_TICKS: u64 = 36_000;

/// The genotype every episode uses: the unit adult founder R0a, R0b and R0d measured.
/// `Genome::founder` takes a *hue*; everything else about the genome is that constructor's
/// fixed unit adult, so one number names the whole genotype.
///
/// The number itself now lives in the core beside the founding routine, so the display's
/// seeding control and this fixture cannot drift apart; this is the same constant.
pub use cubarium_core::TRAINING_FOUNDER_HUE as FOUNDER_HUE;

/// Starting reserve as a fraction of `reserve_max` (headroom to store, so intake is never
/// refused for a reason that has nothing to do with behaviour).
pub use cubarium_core::TRAINING_START_RESERVE as START_RESERVE;
/// Starting energy as a fraction of `energy_max`.
pub use cubarium_core::TRAINING_START_ENERGY as START_ENERGY;

/// The **ecology** a fixture set is built on: the base [`WorldConfig`] every layout starts
/// from, with a label and a hash.
///
/// A layout was always a set of overrides on top of *some* configuration; before ecology v1's
/// calibration that configuration was silently `WorldConfig::default()`. It is now named, so a
/// score, a protocol hash and an exported policy all say which ecology they belong to, and a
/// policy trained in one ecology cannot be quietly evaluated in another.
///
/// `hash` is [`crate::calibrate::config_hash`] itself — called, not reimplemented — so a
/// configuration exported by `calibrate-export` carries **one** number everywhere it appears:
/// in the calibration's note, in this protocol, and in the exported policy. (That function is
/// a 64-bit multiply-xor fingerprint over the canonical JSON with its own constant; it is not
/// bit-for-bit the FNV-1a that [`fnv1a`] computes, and the two must not be swapped for each
/// other on the grounds that both are "the hash".)
#[derive(Clone, Debug, PartialEq)]
pub struct Ecology {
    /// A short name for the configuration: `default`, or the TOML file's stem.
    pub label: String,
    /// FNV-1a over the base config's canonical JSON.
    pub hash: u64,
    /// The configuration a layout's own overrides are applied on top of.
    pub base: Arc<WorldConfig>,
}

impl Default for Ecology {
    fn default() -> Self {
        Ecology::defaults()
    }
}

impl Ecology {
    /// The shipped defaults: what every R2 fixture was built on, unchanged.
    pub fn defaults() -> Ecology {
        Ecology::of("default", WorldConfig::default())
    }

    /// A named base configuration.
    pub fn of(label: &str, base: WorldConfig) -> Ecology {
        let hash = crate::calibrate::config_hash(&base);
        Ecology { label: label.to_string(), hash, base: Arc::new(base) }
    }

    /// Read a `cubarium run --config` TOML — for instance the calibration's selected
    /// `fast-leaf.toml` — and freeze it as this fixture set's ecology. The config is validated
    /// here, so a file the core would refuse is refused before any episode runs.
    pub fn load(path: &Path) -> Result<Ecology, String> {
        let text = std::fs::read_to_string(path)
            .map_err(|e| format!("reading {}: {e}", path.display()))?;
        let config: WorldConfig = toml::from_str(&text)
            .map_err(|e| format!("{} is not a world config: {e}", path.display()))?;
        config
            .validate()
            .map_err(|e| format!("{} is not a config the core accepts: {e}", path.display()))?;
        let label = path
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| path.display().to_string());
        Ok(Ecology::of(&label, config))
    }

    /// `--config <toml>` when it was given, the shipped defaults when it was not.
    pub fn from_option(path: Option<&Path>) -> Result<Ecology, String> {
        match path {
            Some(p) => Ecology::load(p),
            None => Ok(Ecology::defaults()),
        }
    }

    /// The hash as it is printed and recorded everywhere: sixteen lower-case hex digits.
    pub fn hex(&self) -> String {
        format!("{:016x}", self.hash)
    }
}

/// One painted square of producer material: `(2·half + 1)²` cells centred on `(cx, cy)`,
/// filled to `fill · P_max`.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Patch {
    pub cx: u8,
    pub cy: u8,
    pub half: u8,
    pub fill: f64,
}

impl Patch {
    /// The patch's cells, row-major, clipped to the face.
    pub fn cells(self, face: Face) -> Vec<CellId> {
        let mut out = Vec::new();
        let lo_x = self.cx.saturating_sub(self.half);
        let lo_y = self.cy.saturating_sub(self.half);
        let hi_x = (self.cx + self.half).min(15);
        let hi_y = (self.cy + self.half).min(15);
        for y in lo_y..=hi_y {
            for x in lo_x..=hi_x {
                out.push(CellId::new(face, x, y));
            }
        }
        out
    }
}

/// A frozen layout: the world seed, where the grazer starts and faces, and the food.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Layout {
    pub name: String,
    /// `WorldConfig::seed`: the world's own randomness. Both signs of a perturbation pair see
    /// this same value, so a candidate is never scored against a different world than its
    /// mirror.
    pub seed: u64,
    pub start: (u8, u8),
    /// Start heading in chart `(u, v)`, normalised on construction.
    pub heading: (f64, f64),
    /// The opening patch first, then the later food, in the order the disclosed control tours
    /// them.
    pub patches: Vec<Patch>,
    /// The base configuration this layout's own overrides are applied on top of.
    ///
    /// Deliberately **not** serialized: a layout's JSON is a description of its geometry, and
    /// the ecology it was run in is recorded once, by label and hash, in the protocol and in
    /// the exported policy. Nothing in this workspace deserializes a `Layout`; if something
    /// ever does, it gets the shipped defaults and must set this itself.
    #[serde(skip)]
    pub ecology: Ecology,
    /// The motor contract every episode on this layout runs under
    /// (`cubarium_core::MotorModel`, `crate::World::set_motor_model`).
    ///
    /// Deliberately **not** serialized, for the same reason `ecology` is not: a layout's JSON
    /// is a description of its geometry, and the contract it was run under is recorded once,
    /// by name, in the protocol and in the exported policy. `Sweep` is the shipped contract
    /// and the default, so every fixture set built without naming one is the set that has
    /// always existed.
    #[serde(skip)]
    pub motor: MotorModel,
    /// The **action adapter** every episode on this layout decodes a raw head with
    /// (`cubarium_core::neural::ActionAdapter`, `crate::World::set_action_adapter`).
    ///
    /// Deliberately **not** serialized, for the same reason `ecology` and `motor` are not: a
    /// layout's JSON is a description of its geometry, and the adapter it was run under is
    /// recorded once, by name, in the protocol and in the exported policy. `cub-act-1` is the
    /// shipped adapter and the default, so every fixture set built without naming one is the
    /// set that has always existed.
    #[serde(skip)]
    pub adapter: ActionAdapter,
}

/// The stand that carries `p` of foliage (`design/ecology-v1-contract.md` §14 "search"):
/// `W = P/α`, the least wood whose `P_cap = α·W` reaches the painted foliage, and
/// `Q = q_cap·W`, the reserve that structure holds. A zero-foliage cell is bare.
fn stand_of(p: f64, cfg: &WorldConfig) -> (f64, f64) {
    if !(p > 0.0) || cfg.plant.alpha <= 0.0 {
        return (0.0, 0.0);
    }
    let w = p / cfg.plant.alpha;
    (w, cfg.plant.reserve_cap * w)
}

impl Layout {
    pub fn face(&self) -> Face {
        Face::Top
    }

    pub fn start_cell(&self) -> CellId {
        CellId::new(self.face(), self.start.0, self.start.1)
    }

    pub fn heading_vec(&self) -> Vec2 {
        Vec2::new(self.heading.0, self.heading.1)
            .normalized()
            .expect("a layout heading is nonzero")
    }

    /// Every food cell, in patch order and row-major inside a patch. This is also the
    /// disclosed mobile control's tour order; it is a diagnostic route, never shown to a
    /// policy.
    pub fn route(&self) -> Vec<CellId> {
        let face = self.face();
        let mut out: Vec<CellId> = Vec::new();
        for p in &self.patches {
            for c in p.cells(face) {
                if !out.contains(&c) {
                    out.push(c);
                }
            }
        }
        out
    }

    /// Material painted into the layout, in `m`. Reported so a fixture's inputs are accounted
    /// for rather than assumed.
    ///
    /// Ecology v1 (`design/ecology-v1-contract.md` §14, "search"): a painted patch is a live
    /// **stand**, not a floating leaf, so each cell also carries the wood that could hold its
    /// foliage (`W = P/α`) and the reserve that structure holds (`Q = q_cap · W`). All three
    /// are imported material and all three are counted here.
    pub fn painted_material(&self, cfg: &WorldConfig) -> f64 {
        self.route()
            .iter()
            .map(|c| {
                let p = self.fill_of(*c) * cfg.producer.max;
                let (w, q) = stand_of(p, cfg);
                p + w + q
            })
            .sum()
    }

    fn fill_of(&self, cell: CellId) -> f64 {
        // Later patches overwrite earlier ones where they overlap, which is the order
        // `build` paints in.
        let face = self.face();
        let mut fill = 0.0;
        for p in &self.patches {
            if p.cells(face).contains(&cell) {
                fill = p.fill;
            }
        }
        fill
    }

    /// The canonical text the layout hash is taken over.
    pub fn canonical_text(&self, cfg: &WorldConfig) -> String {
        let patches: Vec<String> = self
            .patches
            .iter()
            .map(|p| format!("{},{},{},{:.6}", p.cx, p.cy, p.half, p.fill))
            .collect();
        format!(
            "cub-es-layout-1|name={}|seed={}|face=Top|start={},{}|heading={:.9},{:.9}\
             |patches={}|horizon={HORIZON_TICKS}|hue={FOUNDER_HUE}\
             |reserve={START_RESERVE}|energy={START_ENERGY}|config={}",
            self.name,
            self.seed,
            self.start.0,
            self.start.1,
            self.heading_vec().x,
            self.heading_vec().y,
            patches.join(";"),
            serde_json::to_string(cfg).expect("the world config serializes"),
        )
    }

    /// FNV-1a 64 over [`Layout::canonical_text`].
    pub fn hash(&self, cfg: &WorldConfig) -> u64 {
        fnv1a(self.canonical_text(cfg).as_bytes())
    }

    /// The same layout under a named motor contract (`cubarium_core::MotorModel`). A set of
    /// layouts carries one contract, exactly as it carries one ecology, and that is what the
    /// protocol and the exported policy record.
    pub fn with_motor(mut self, motor: MotorModel) -> Layout {
        self.motor = motor;
        self
    }

    /// The same layout under a named action adapter (`cubarium_core::neural::ActionAdapter`).
    /// A set of layouts carries one adapter, exactly as it carries one ecology and one motor
    /// contract, and that is what the protocol and the exported policy record.
    pub fn with_adapter(mut self, adapter: ActionAdapter) -> Layout {
        self.adapter = adapter;
        self
    }

    /// Build the world this layout describes, and return it with the grazer's id.
    ///
    /// The world is constructed with `World::new`, staged, and then re-validated through
    /// `World::from_state`, exactly as the core's own fixtures do. No second simulator exists:
    /// every tick after this is `World::step`.
    pub fn build(&self) -> Result<(World, OrganismId), String> {
        let cfg = self.config();
        let face = self.face();
        let mut world = World::new(cfg).map_err(|e| format!("layout {}: {e}", self.name))?;

        // 1. Empty the surface, booking the removal as an export. Ecology v1's pools go too:
        //    an unpainted cell is bare ground, with no stand, no dead wood and no remains.
        let mut removed = 0.0;
        for cell in CellId::all() {
            let i = cell.index();
            let f = &mut world.state.fields;
            removed += f.p[i] + f.d[i] + f.f[i];
            f.p[i] = 0.0;
            f.f[i] = 0.0;
            f.d[i] = 0.0;
            f.de[i] = 0.0;
            let e = &mut world.state.ecology;
            removed += e.wood[i] + e.plant_reserve[i] + e.dead_wood[i] + e.carrion[i];
            e.wood[i] = 0.0;
            e.plant_reserve[i] = 0.0;
            e.dead_wood[i] = 0.0;
            e.carrion[i] = 0.0;
            e.carrion_energy[i] = 0.0;
        }
        world.state.external_material_in -= removed;

        // 2. Paint the layout's patches, booking the addition as an import. Each painted
        //    cell is a live stand: the foliage the layout names, the wood that can carry it
        //    and the reserve that wood holds (§14 "search"). Without the wood the cell would
        //    be a dead one whose foliage falls to litter over the episode.
        let cfg_snapshot = world.state.config.clone();
        let p_max = cfg_snapshot.producer.max;
        let mut added = 0.0;
        for patch in &self.patches {
            for cell in patch.cells(face) {
                let i = cell.index();
                let p = patch.fill * p_max;
                let (w, q) = stand_of(p, &cfg_snapshot);
                added += p - world.state.fields.p[i];
                added += w - world.state.ecology.wood[i];
                added += q - world.state.ecology.plant_reserve[i];
                world.state.fields.p[i] = p;
                world.state.ecology.wood[i] = w;
                world.state.ecology.plant_reserve[i] = q;
            }
        }
        world.state.external_material_in += added;

        // 3. One mature grazer, identical in every episode of every layout.
        let id = self.place(&mut world);
        let mut world = World::from_state(world.state)
            .map_err(|e| format!("layout {}: staged state invalid: {e}", self.name))?;
        // The motor contract, on the **final** world and before the first tick. Transient, so
        // it is not in the staged state and `Sweep` changes nothing.
        world.set_motor_model(self.motor);
        // And the action adapter, on the same world and before the first tick, for the same
        // reason: transient, so it is not in the staged state and `cub-act-1` changes nothing.
        world.set_action_adapter(self.adapter);
        world
            .check_invariants()
            .map_err(|e| format!("layout {}: staged world inconsistent: {e}", self.name))?;
        Ok((world, id))
    }

    /// The config every layout shares, differing only in `seed`.
    ///
    /// It starts from [`Layout::ecology`]'s base — the shipped defaults, or the calibrated
    /// ecology a `--config` named — and then applies the layout's own overrides, which are the
    /// isolated-episode conditions and nothing else.
    pub fn config(&self) -> WorldConfig {
        let mut c = (*self.ecology.base).clone();
        // No founders of its own: the layout places exactly the one grazer it means to.
        c.founders.kinds.clear();
        c.founders.count = 0;
        // Matched conditions across candidates: no weather swing and no rain, so light and
        // moisture are the same tick for tick for every policy on this layout.
        c.weather.amplitude = 0.0;
        c.water.rain_rate = 0.0;
        // One genotype, exactly: no mutation anywhere.
        c.mechanisms.mutation = false;
        c.seed = self.seed;
        c
    }

    /// The one mature grazer, founded through the core's own
    /// [`World::found_training_animal`] — the same routine, body for body, that the
    /// display's `--neural` seeding uses. There is one definition of this animal, not two.
    fn place(&self, world: &mut World) -> OrganismId {
        let pos = self.start_cell().center();
        assert_eq!(cell_of(&pos), self.start_cell(), "the grazer landed outside its start cell");
        world
            .found_training_animal(pos, self.heading_vec())
            .expect("a fresh layout world has room for its one grazer")
    }
}

/// The four **training** layouts. Frozen: these exact values were fixed before any control or
/// smoke was measured, and the hash in the result document is over this text.
///
/// They differ in start heading, opening strength and the geometry of the later food:
///
/// | layout | opening | later food | start heading |
/// | --- | --- | --- | --- |
/// | `t1-corridor` | 3×3 at 0.55 | two 3×3 patches due east, 20 px apart | east |
/// | `t2-weak-open` | 3×3 at 0.30 | 3×3 north, then 3×3 north-west | north |
/// | `t3-scatter` | 3×3 at 0.70 | a single-cell cue, then a 5×5 field | west |
/// | `t4-ring` | 3×3 at 0.45 | two 3×3 corners and two single-cell cues | south-east |
pub fn training_layouts() -> Vec<Layout> {
    training_layouts_on(&Ecology::defaults())
}

/// The four training layouts on a named ecology. The geometry is the frozen one; only the
/// configuration the overrides are applied to changes, and that change moves every layout
/// hash and the protocol hash with it.
pub fn training_layouts_on(ecology: &Ecology) -> Vec<Layout> {
    let e = || ecology.clone();
    vec![
        Layout {
            name: "t1-corridor".into(),
            seed: 20_260_915_001,
            start: (3, 8),
            heading: (1.0, 0.0),
            patches: vec![
                Patch { cx: 3, cy: 8, half: 1, fill: 0.55 },
                Patch { cx: 8, cy: 8, half: 1, fill: 1.0 },
                Patch { cx: 13, cy: 8, half: 1, fill: 1.0 },
            ],
            ecology: e(),
            motor: MotorModel::default(),
        adapter: ActionAdapter::default(),
        },
        Layout {
            name: "t2-weak-open".into(),
            seed: 20_260_915_002,
            start: (8, 12),
            heading: (0.0, -1.0),
            patches: vec![
                Patch { cx: 8, cy: 12, half: 1, fill: 0.30 },
                Patch { cx: 8, cy: 7, half: 1, fill: 1.0 },
                Patch { cx: 4, cy: 3, half: 1, fill: 1.0 },
            ],
            ecology: e(),
            motor: MotorModel::default(),
        adapter: ActionAdapter::default(),
        },
        Layout {
            name: "t3-scatter".into(),
            seed: 20_260_915_003,
            start: (12, 4),
            heading: (-1.0, 0.0),
            patches: vec![
                Patch { cx: 12, cy: 4, half: 1, fill: 0.70 },
                Patch { cx: 9, cy: 6, half: 0, fill: 1.0 },
                Patch { cx: 5, cy: 9, half: 2, fill: 1.0 },
            ],
            ecology: e(),
            motor: MotorModel::default(),
        adapter: ActionAdapter::default(),
        },
        Layout {
            name: "t4-ring".into(),
            seed: 20_260_915_004,
            start: (8, 8),
            heading: (FRAC_1_SQRT_2, FRAC_1_SQRT_2),
            patches: vec![
                Patch { cx: 8, cy: 8, half: 1, fill: 0.45 },
                Patch { cx: 12, cy: 12, half: 1, fill: 1.0 },
                Patch { cx: 12, cy: 4, half: 0, fill: 1.0 },
                Patch { cx: 4, cy: 4, half: 1, fill: 1.0 },
                Patch { cx: 4, cy: 12, half: 0, fill: 1.0 },
            ],
            ecology: e(),
            motor: MotorModel::default(),
        adapter: ActionAdapter::default(),
        },
    ]
}

/// The eight **held-out** layouts, from the same declared rules with their own seeds.
///
/// Declared rules: a 3×3 opening patch at a fill drawn uniformly from `[0.30, 0.75]`, two 3×3
/// later patches at full `P_max` placed at least four cells from the opening and from each
/// other, a start at the opening's centre, and a start heading from the eight compass
/// directions. Everything else — genotype, stores, horizon, config — is identical to training.
///
/// **These are not evaluated in R2a.** They are constructed and hashed so the split is on
/// record before any policy exists, and they are kept out of optimisation, of any calibration
/// and of candidate selection. Their evaluation belongs to the learning assignment.
pub fn holdout_layouts() -> Vec<Layout> {
    holdout_layouts_on(&Ecology::defaults())
}

/// The eight held-out layouts on a named ecology.
pub fn holdout_layouts_on(ecology: &Ecology) -> Vec<Layout> {
    (0..8u64).map(|i| holdout_layout(i, ecology)).collect()
}

fn holdout_layout(index: u64, ecology: &Ecology) -> Layout {
    let seed = 20_260_915_100 + index;
    let draw = |c: u64| super::rng::unit_at(seed, stream::ES_HOLDOUT, index, c);
    // Positions come from a 4-cell lattice inset from the rim, so a patch never clips the face
    // edge and two patches are always at least four cells apart.
    let lattice: Vec<(u8, u8)> =
        (0..4u8).flat_map(|y| (0..4u8).map(move |x| (2 + 4 * x, 2 + 4 * y))).collect();
    let mut taken: Vec<usize> = Vec::new();
    let pick = |c: u64, taken: &mut Vec<usize>| -> (u8, u8) {
        let free: Vec<usize> = (0..lattice.len()).filter(|i| !taken.contains(i)).collect();
        let k = free[((draw(c) * free.len() as f64) as usize).min(free.len() - 1)];
        taken.push(k);
        lattice[k]
    };
    let open = pick(0, &mut taken);
    let later_a = pick(1, &mut taken);
    let later_b = pick(2, &mut taken);
    let fill = 0.30 + 0.45 * draw(3);
    let compass = [
        (1.0, 0.0),
        (FRAC_1_SQRT_2, FRAC_1_SQRT_2),
        (0.0, 1.0),
        (-FRAC_1_SQRT_2, FRAC_1_SQRT_2),
        (-1.0, 0.0),
        (-FRAC_1_SQRT_2, -FRAC_1_SQRT_2),
        (0.0, -1.0),
        (FRAC_1_SQRT_2, -FRAC_1_SQRT_2),
    ];
    let heading = compass[((draw(4) * 8.0) as usize).min(7)];
    Layout {
        name: format!("h{}-holdout", index + 1),
        seed,
        start: open,
        heading,
        patches: vec![
            Patch { cx: open.0, cy: open.1, half: 1, fill: (fill * 1e6).round() / 1e6 },
            Patch { cx: later_a.0, cy: later_a.1, half: 1, fill: 1.0 },
            Patch { cx: later_b.0, cy: later_b.1, half: 1, fill: 1.0 },
        ],
        ecology: ecology.clone(),
        motor: MotorModel::default(),
        adapter: ActionAdapter::default(),
    }
}

/// FNV-1a 64, the same construction the core uses for the schema digest.
pub fn fnv1a(bytes: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in bytes {
        h ^= u64::from(*b);
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
}

/// A one-line summary of a layout for the protocol record.
pub fn describe(layout: &Layout) -> String {
    let cfg = layout.config();
    format!(
        "{:<13} seed {:>12}  start ({:>2},{:>2})  heading ({:>6.3},{:>6.3})  cells {:>3}  \
         opening fill {:.2}  material {:.3} m  hash {:#018x}",
        layout.name,
        layout.seed,
        layout.start.0,
        layout.start.1,
        layout.heading.0,
        layout.heading.1,
        layout.route().len(),
        layout.patches[0].fill,
        layout.painted_material(&cfg),
        layout.hash(&cfg),
    )
}

/// Seconds of world time in `ticks`.
pub fn seconds(ticks: u64) -> f64 {
    ticks as f64 * DT
}

/// Unused draw helper kept next to the holdout generator so its randomness is visibly the
/// trainer's, not the world's.
pub(crate) fn _gaussian_is_the_trainers(seed: u64, i: u64) -> f64 {
    gaussian(seed, stream::ES_HOLDOUT, 0, i)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn there_are_four_training_layouts_and_eight_held_out_ones_and_they_are_disjoint() {
        let train = training_layouts();
        let hold = holdout_layouts();
        assert_eq!(train.len(), 4);
        assert_eq!(hold.len(), 8);
        let mut seeds: Vec<u64> = train.iter().chain(&hold).map(|l| l.seed).collect();
        seeds.sort_unstable();
        let before = seeds.len();
        seeds.dedup();
        assert_eq!(seeds.len(), before, "every layout has its own world seed");
        let mut hashes: Vec<u64> =
            train.iter().chain(&hold).map(|l| l.hash(&l.config())).collect();
        hashes.sort_unstable();
        let before = hashes.len();
        hashes.dedup();
        assert_eq!(hashes.len(), before, "every layout hashes differently");
    }

    #[test]
    fn a_layout_is_frozen_the_hash_moves_when_anything_moves() {
        let l = &training_layouts()[0];
        let cfg = l.config();
        let base = l.hash(&cfg);
        assert_eq!(base, l.hash(&cfg), "the hash is a pure function of the layout and config");

        let mut moved = l.clone();
        moved.patches[0].fill += 1e-6;
        assert_ne!(moved.hash(&cfg), base);

        let mut other_cfg = cfg.clone();
        other_cfg.producer.max += 1e-9;
        assert_ne!(l.hash(&other_cfg), base, "the world config is inside the hash");
    }

    #[test]
    fn every_layout_builds_a_valid_world_with_exactly_one_grazer_on_its_start_cell() {
        for l in training_layouts().iter().chain(&holdout_layouts()) {
            let (world, id) = l.build().unwrap_or_else(|e| panic!("{}: {e}", l.name));
            assert_eq!(world.population(), 1, "{}", l.name);
            let o = world.state.organisms.get(id).expect("the grazer");
            assert_eq!(cell_of(&o.pos), l.start_cell(), "{}", l.name);
            assert!((o.structure - o.phenotype.structure_adult).abs() < 1e-12, "{}", l.name);
            assert!((o.heading.length() - 1.0).abs() < 1e-12, "{}", l.name);
            // The layout's food is exactly what it declares, and it is all it declares.
            // Ecology v1 (§14 "search"): a painted patch is a live stand, so the imported
            // material is the foliage **plus** the wood that carries it and that wood's
            // reserve; `painted_material` counts all three.
            let eco = &world.state.ecology;
            let painted: f64 = world.state.fields.p.iter().sum::<f64>()
                + eco.wood.iter().sum::<f64>()
                + eco.plant_reserve.iter().sum::<f64>();
            assert!(
                (painted - l.painted_material(&l.config())).abs() < 1e-9,
                "{}: painted {painted}",
                l.name
            );
            let cfg = l.config();
            for cell in CellId::all() {
                let i = cell.index();
                let p = world.state.fields.p[i];
                assert!(
                    (eco.wood[i] - p / cfg.plant.alpha).abs() < 1e-12,
                    "{}: cell {i} carries {} of wood for {p} of foliage",
                    l.name,
                    eco.wood[i]
                );
                assert!(
                    (eco.plant_reserve[i] - cfg.plant.reserve_cap * eco.wood[i]).abs() < 1e-12,
                    "{}: cell {i} reserve",
                    l.name
                );
            }
            assert!(world.state.fields.f.iter().all(|x| *x == 0.0), "{}", l.name);
            assert!(world.state.fields.d.iter().all(|x| *x == 0.0), "{}", l.name);
            assert!(eco.dead_wood.iter().all(|x| *x == 0.0), "{}", l.name);
            assert!(eco.carrion.iter().all(|x| *x == 0.0), "{}", l.name);
            world.check_invariants().unwrap_or_else(|e| panic!("{}: {e}", l.name));
        }
    }

    #[test]
    fn building_the_same_layout_twice_gives_the_same_world() {
        let l = &training_layouts()[2];
        let (a, _) = l.build().expect("built");
        let (b, _) = l.build().expect("built");
        assert_eq!(
            cubarium_core::snapshot::state_hash(&a.state),
            cubarium_core::snapshot::state_hash(&b.state)
        );
    }

    #[test]
    fn a_layout_opens_finite_and_keeps_reachable_later_food() {
        for l in training_layouts() {
            let opening = l.patches[0];
            assert!(
                opening.fill > 0.0 && opening.fill < 1.0,
                "{}: the opening is finite, not full and not foodless",
                l.name
            );
            assert_eq!((opening.cx, opening.cy), l.start, "{}: the grazer opens in a patch", l.name);
            assert!(l.patches.len() >= 2, "{}: there is later food", l.name);
            // Later food is outside the opening patch and inside the face.
            let open_cells = opening.cells(l.face());
            for p in &l.patches[1..] {
                for c in p.cells(l.face()) {
                    assert!(!open_cells.contains(&c), "{}: later food overlaps the opening", l.name);
                }
            }
        }
    }

    #[test]
    fn held_out_layouts_follow_the_declared_rules() {
        for l in holdout_layouts() {
            assert_eq!(l.patches.len(), 3);
            assert!((0.30..=0.75).contains(&l.patches[0].fill), "{}: {}", l.name, l.patches[0].fill);
            assert!(l.patches[1..].iter().all(|p| p.fill == 1.0));
            assert_eq!((l.patches[0].cx, l.patches[0].cy), l.start);
            for p in &l.patches {
                assert!(p.cx >= 1 && p.cx <= 14 && p.cy >= 1 && p.cy <= 14, "{}", l.name);
            }
            let centres: Vec<(u8, u8)> = l.patches.iter().map(|p| (p.cx, p.cy)).collect();
            for i in 0..centres.len() {
                for j in (i + 1)..centres.len() {
                    let dx = centres[i].0.abs_diff(centres[j].0);
                    let dy = centres[i].1.abs_diff(centres[j].1);
                    assert!(dx >= 4 || dy >= 4, "{}: patches {i} and {j} are adjacent", l.name);
                }
            }
        }
    }

    /// A different ecology, built the way `--config` builds one: the plant and producer
    /// constants a calibration moves, plus every field a layout is supposed to override.
    fn moved_ecology() -> Ecology {
        let mut c = WorldConfig::default();
        c.plant.foliage_rate = 0.006;
        c.plant.maintenance = 0.0001;
        c.plant.alpha = 2.5;
        c.producer.max = 1.8;
        // The three a layout must override, deliberately set to the opposite of what a layout
        // wants, plus a seed that is not any layout's.
        c.weather.amplitude = 0.3;
        c.water.rain_rate = 0.6;
        c.mechanisms.mutation = true;
        c.founders.count = 9;
        c.seed = 4_242;
        c.validate().expect("the moved ecology is one the core accepts");
        Ecology::of("moved", c)
    }

    /// The number this crate calls a config hash is **one** number. The fixture must not grow
    /// a second fingerprint that disagrees with the calibration's published one.
    #[test]
    fn an_ecologys_hash_is_the_calibrations_own_config_hash() {
        let eco = moved_ecology();
        assert_eq!(eco.hash, crate::calibrate::config_hash(&eco.base));
        assert_eq!(eco.hex(), format!("{:016x}", eco.hash));
        let defaults = Ecology::defaults();
        assert_eq!(defaults.hash, crate::calibrate::config_hash(&WorldConfig::default()));
        assert_ne!(defaults.hash, eco.hash, "a moved ecology is a different ecology");
        assert_eq!(defaults.label, "default");
    }

    /// `Layout::config` starts from the ecology it was given and still applies its own
    /// overrides: no founders, no weather swing, no rain, no mutation, its own seed.
    #[test]
    fn a_layout_starts_from_its_ecology_and_still_applies_its_own_overrides() {
        let eco = moved_ecology();
        for l in training_layouts_on(&eco).iter().chain(&holdout_layouts_on(&eco)) {
            let c = l.config();
            // From the ecology.
            assert_eq!(c.plant.foliage_rate, 0.006, "{}", l.name);
            assert_eq!(c.plant.maintenance, 0.0001, "{}", l.name);
            assert_eq!(c.plant.alpha, 2.5, "{}", l.name);
            assert_eq!(c.producer.max, 1.8, "{}", l.name);
            // The layout's own, on top.
            assert!(c.founders.kinds.is_empty(), "{}", l.name);
            assert_eq!(c.founders.count, 0, "{}", l.name);
            assert_eq!(c.weather.amplitude, 0.0, "{}", l.name);
            assert_eq!(c.water.rain_rate, 0.0, "{}", l.name);
            assert!(!c.mechanisms.mutation, "{}", l.name);
            assert_eq!(c.seed, l.seed, "{}", l.name);
        }
    }

    /// Every layout hash moves with the ecology, and the geometry is unchanged.
    #[test]
    fn the_layout_hashes_move_with_the_ecology_and_the_geometry_does_not() {
        let eco = moved_ecology();
        let default_set = training_layouts();
        let moved_set = training_layouts_on(&eco);
        for (d, m) in default_set.iter().zip(&moved_set) {
            assert_eq!(d.name, m.name);
            assert_eq!(d.patches, m.patches, "{}: the geometry is frozen", d.name);
            assert_eq!(d.start, m.start);
            assert_ne!(d.hash(&d.config()), m.hash(&m.config()), "{}", d.name);
        }
    }

    /// The whole point of the plumbing: a training layout on a calibrated ecology still builds
    /// one valid world holding one grazer whose painted patches are **live stands** under
    /// *that* ecology's plant constants — `P` at the declared fill of its `P_max`, `W = P/α`,
    /// `Q = q_cap·W` — not under the shipped defaults'.
    #[test]
    fn every_layout_on_a_moved_ecology_paints_live_stands_under_that_ecologys_constants() {
        let eco = moved_ecology();
        for l in training_layouts_on(&eco).iter().chain(&holdout_layouts_on(&eco)) {
            let (world, id) = l.build().unwrap_or_else(|e| panic!("{}: {e}", l.name));
            assert_eq!(world.population(), 1, "{}", l.name);
            assert!(world.state.organisms.get(id).is_some(), "{}", l.name);
            let cfg = l.config();
            let eco_state = &world.state.ecology;
            let mut live = 0usize;
            for cell in CellId::all() {
                let i = cell.index();
                let p = world.state.fields.p[i];
                assert!(
                    (eco_state.wood[i] - p / cfg.plant.alpha).abs() < 1e-12,
                    "{}: cell {i}",
                    l.name
                );
                assert!(
                    (eco_state.plant_reserve[i] - cfg.plant.reserve_cap * eco_state.wood[i]).abs()
                        < 1e-12,
                    "{}: cell {i}",
                    l.name
                );
                if p > 0.0 {
                    live += 1;
                }
            }
            assert_eq!(live, l.route().len(), "{}: every painted cell is live", l.name);
            // The opening patch is at its declared fraction of *this* ecology's P_max.
            let opening = l.patches[0];
            let centre = CellId::new(l.face(), opening.cx, opening.cy).index();
            assert!(
                (world.state.fields.p[centre] - opening.fill * cfg.producer.max).abs() < 1e-12,
                "{}",
                l.name
            );
            world.check_invariants().unwrap_or_else(|e| panic!("{}: {e}", l.name));
        }
    }
}
