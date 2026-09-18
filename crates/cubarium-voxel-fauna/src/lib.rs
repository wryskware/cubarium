//! Consumers on the voxel strip. One species — the **frondgrazer**, a ground browser that
//! stands on a support face, crops the foliage it can reach, walks toward the best food it
//! can sense, and pays for every unit of it — living as [`Animal`]s beside a
//! [`cubarium_voxel_flora::Flora`] on a [`cubarium_voxel::World`].
//!
//! This crate never draws, never reads the clock and never touches the world's stores: it
//! reads the world through [`cubarium_voxel::VoxelView`] (support faces, heights, water
//! depth, the `x` wrap) and changes the plant layer only through round 5a's bounded
//! transfers — [`cubarium_voxel_flora::Flora::take_foliage`] and
//! [`cubarium_voxel_flora::Flora::deposit`], with
//! [`cubarium_voxel_flora::FloraView::reachable_foliage`] as the geometry of what a
//! browser standing somewhere can actually get at.
//!
//! # What this layer is, and is not
//!
//! It is a **heuristic** animal, not a policy and not the cube's hunter: there is no
//! network, no learning, no drive system and no game hook. Every quantity is paid out of a
//! stock this layer holds, and every unit that leaves it is a named boundary flow of
//! [`FaunaLedger`], so the plant layer's `consumed_*` and `deposited_*` and this layer's
//! `eaten_*` and `deposited_*` are the same transfers seen from two sides and the two
//! ledgers close together.
//!
//! It says **nothing** about populations, carrying capacity or viability. Astra's round 7
//! left those unclaimed on purpose and this round does not claim them: what a run can show
//! is that the flows are paid and observable.
//!
//! # Stated assumptions of this round
//!
//! 1. **No mating system.** An adult that has the reserve pays the whole `birth_cost`
//!    itself and a newborn appears beside it. One specimen is therefore a lineage here,
//!    which `design/theoretical-biosphere-2026-09-16.md` says a population is not; a
//!    two-parent contract (encounter, pairing, gestation) is a later round and not a
//!    missing line of code.
//! 2. **Growth is unrated and unpriced.** Assimilated matter fills the body up to
//!    `body_max` and then the reserve up to `reserve_cap · body`; there is no growth rate
//!    and no construction cost, because [`SpeciesConfig::yield_fraction`] already carries
//!    the loss between what was eaten and what was built.
//! 3. **Dung is mineral only.** The undigested fraction of a bite is *respired*, per the
//!    round's brief, and what is excreted is the mineral a bite carried in excess of the
//!    tissue it built — as a litter deposit, because litter is where this round puts dung.
//! 4. **No sub-tick motion.** An animal is on one support face per tick. How a body moves
//!    between faces is the art thread's question (`design/voxel-art-direction.md`, when it
//!    lands), and `pace-in-body-lengths` applies to it then, not here.
//! 5. **The interim look is not the look.** The presenter draws a 2×1×2 block in a
//!    placeholder colour (`crates/cubarium/src/voxel/animal.rs`), named interim in the
//!    code, until the art direction says what a consumer looks like.
//!
//! Every number in [`SpeciesConfig::frondgrazer`] is an untuned **placeholder** listed in
//! `design/backlog.md` §1. Nothing here is tuned and no result in this round depends on a
//! value.

#![forbid(unsafe_code)]

mod snapshot;
mod step;

use cubarium_voxel::{VoxelView, World};
use cubarium_voxel_flora::{Deposit, DepositKind, Flora, Site, Taken};
use serde::{Deserialize, Serialize};

pub use cubarium_voxel::{DT, TICK_HZ};
pub use cubarium_voxel_flora::Reach;
pub use snapshot::SCHEMA;

/// The consumers of the voxel ecology. One, so far: see
/// [`SpeciesConfig::frondgrazer`] for the sentence of ecology its numbers encode.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Species {
    /// The paid ground browser of the grazed meadow: crops reachable foliage, walks to
    /// better food, drowns in standing water it cannot wade.
    Frondgrazer,
}

impl Species {
    pub const ALL: [Species; 1] = [Species::Frondgrazer];
    pub const COUNT: usize = Species::ALL.len();

    /// Index into a per-species array. Stable, like the flora's.
    pub fn index(self) -> usize {
        match self {
            Species::Frondgrazer => 0,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Species::Frondgrazer => "frondgrazer",
        }
    }

    /// The name a command line or a stdin line gives, or `None`.
    pub fn parse(s: &str) -> Option<Species> {
        Species::ALL.into_iter().find(|sp| sp.name().eq_ignore_ascii_case(s))
    }
}

/// What an animal did on the tick just stepped. A record of the act, not a mode it is put
/// into: the next tick reads its stocks and its surroundings again from scratch.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum State {
    /// It had at least one bite within reach and took it.
    Cropping,
    /// It had nothing in reach, something in sense, and a face strictly closer to it that
    /// it could climb and wade. Also the state of an animal that wanted to walk and was
    /// not due a step this tick.
    Walking,
    /// Nothing in reach and nothing in sense, or nowhere closer it could stand.
    Resting,
}

/// One animal: an identity, the support face it stands on, and the three currencies it
/// holds.
///
/// `body` is structure and `reserve` is what maintenance is paid out of; `mineral` and
/// `energy` are the stocks that came in with its food. Energy is a **stock and not
/// `energy_density · organic`** (the flora's dead pools are kept the same way and for the
/// same reason): a bite carries the energy the plant layer handed over at *its* density,
/// so holding that number is what makes the energy residual close for any food, instead of
/// requiring this species' own density to be no greater than its food's.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Animal {
    /// This animal's identity, from [`FaunaLedger::births`]: every animal this layer has
    /// ever created has its own, introduced ones included, and it never changes or
    /// repeats.
    pub id: u64,
    pub species: Species,
    /// The support face it stands on. Its body is drawn in the void above this.
    pub site: Site,
    /// Organic structure. Below `body_min` it is dead.
    pub body: f64,
    /// Organic matter maintenance is paid from before the body is.
    pub reserve: f64,
    /// The mineral in its tissue: an inventory, moved by the same fraction rule the plant
    /// layer uses for every outflow, never created and never respired.
    pub mineral: f64,
    /// The energy in its tissue, as its food handed it over.
    pub energy: f64,
    /// Ticks since it was born or introduced. Also its step phase: an animal steps on the
    /// ticks where `age_ticks` is a multiple of its species' step period, so two animals
    /// born on different ticks do not step in lockstep.
    pub age_ticks: u64,
    pub state: State,
}

impl Animal {
    /// Organic matter in the animal: structure plus reserve. What a corpse deposits.
    pub fn organic(&self) -> f64 {
        self.body + self.reserve
    }
}

/// One species' numbers. **Every one of them is an untuned placeholder**; see
/// [`SpeciesConfig::frondgrazer`] and `design/backlog.md` §1.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct SpeciesConfig {
    /// Organic matter respired per unit of body per second, standing still. Paid from the
    /// reserve, then from the body.
    pub maintenance_per_s: f64,
    /// Organic matter a mouth can take per second, spread over the stands in reach in site
    /// order. The bound on what a bite *asks* for; the plant layer bounds what it gets.
    pub bite_per_s: f64,
    /// The fraction of a bite that becomes tissue. The rest is respired, with the energy
    /// that was in it leaving as heat and the mineral staying behind.
    pub yield_fraction: f64,
    /// Mineral per unit of tissue built — the density of what it builds, and therefore the
    /// **budget** its growth fits inside: a bite can build at most `mineral / n_tissue` of
    /// tissue, whatever `yield_fraction` would otherwise have assimilated, and the
    /// unfunded organic matter is respired with its energy (Astra R9.1). Mineral a bite
    /// carries in excess of what was built is excreted as a litter deposit.
    ///
    /// There is **no mineral reserve**: [`Animal::mineral`] is an inventory of the tissue's
    /// own content and growth never draws on it, so a mineral-free bite builds nothing
    /// however much mineral the animal is carrying. At the placeholders the budget binds
    /// on every bite — a plant's foliage holds 0.02 against this 0.05 — so a frondgrazer
    /// turns 40 % of what it eats into tissue and not the 50 % `yield_fraction` names.
    pub n_tissue: f64,
    /// Structure it grows to and no further.
    pub body_max: f64,
    /// Structure below which it is dead. Also a newborn's body.
    pub body_min: f64,
    /// Structure at which it can pay for a birth.
    pub birth_body: f64,
    /// Organic matter a birth costs its parent, all of it out of the reserve. At least
    /// `body_min`, because that is the newborn's body; the remainder is the newborn's
    /// reserve.
    pub birth_cost: f64,
    /// The ceiling on **new intake** into the reserve, per unit of body: assimilated
    /// matter stops going to the reserve at `reserve_cap · body` and is respired instead
    /// ([`step`]'s assimilation). It is deliberately **not** a universal storage bound
    /// (Astra R9.4).
    ///
    /// **The one exception, named.** A paid birth endowment may sit above it. A newborn is
    /// `body_min` of body and `birth_cost − body_min` of reserve, which at the
    /// placeholders is `0.005` against a cap of `reserve_cap · body_min` = `0.0025`: the
    /// parcel is what the parent actually paid for out of its own reserve, so clamping it
    /// would either destroy organic matter or need a conserving birth allocation, which is
    /// a life-history decision and not a repair. While a reserve is above the ceiling no
    /// intake raises it — `room` is zero and the surplus is respired — and maintenance
    /// spends it exactly as it spends any other reserve, so the excess is transient and
    /// never grows.
    pub reserve_cap: f64,
    /// How far it can get at food from the face it stands on.
    pub reach: Reach,
    /// The largest height difference, in voxels, between the face it stands on and a face
    /// it can step onto.
    pub climb: u32,
    /// The deepest standing water, in metres, it will step into.
    pub wade_depth_m: f64,
    /// The depth of standing water on its own face that kills it.
    pub drown_depth_m: f64,
    /// Seconds between steps. Quantised to whole ticks, never below one.
    pub step_period_s: f64,
    /// How far, in voxels of wrapped `x` and of `z` each measured on its own, a **face it
    /// could stand on** may be and still be a candidate for the walk. The domain is the
    /// candidate face's and not the food's (Astra R9.5): a crown is found by scoring the
    /// faces in this box with [`cubarium_voxel_flora::FloraView::reachable_foliage`], so a
    /// stand outside the box whose feeding face is inside it *is* found, and a stand inside
    /// it whose only feeding face is outside is not.
    pub sense_radius: u32,
    /// Energy per unit of organic matter an **introduced** animal arrives with. Used for
    /// nothing else: every other unit of energy in this layer came in with a bite at the
    /// food's own density, and a newborn takes its parent's pro rata.
    pub energy_density: f64,
}

impl SpeciesConfig {
    /// The paid ground browser of the biosphere's grazed meadow
    /// (`design/theoretical-biosphere-2026-09-16.md` §5, branch 1): a small body that
    /// eats low foliage it can stand under, walks to the next patch when its own is bare,
    /// cannot climb a step taller than itself, wades a film and drowns in a pool.
    ///
    /// **Every number is a placeholder and nothing measured any of them.** They are
    /// chosen to encode that sentence at the scale the round-4 presets already set: a
    /// springturf founder carries `α · W` = 0.06 of foliage, so a body of 0.05 and a
    /// mouth of 0.002 /s put one animal and one turf in the same order of magnitude, and
    /// `bite_per_s` is deliberately the same rate as round 5a's harvest probe so that the
    /// two are comparable.
    pub fn frondgrazer() -> SpeciesConfig {
        SpeciesConfig {
            maintenance_per_s: 0.001,
            bite_per_s: 0.002,
            yield_fraction: 0.5,
            n_tissue: 0.05,
            body_max: 0.05,
            body_min: 0.005,
            birth_body: 0.03,
            birth_cost: 0.01,
            reserve_cap: 0.5,
            reach: Reach { horizontal: 1, up: 1 },
            climb: 1,
            wade_depth_m: 0.05,
            drown_depth_m: 0.2,
            step_period_s: 1.0,
            sense_radius: 8,
            energy_density: 2.0,
        }
    }

    /// Whole ticks between steps, never zero.
    pub fn step_period_ticks(&self) -> u64 {
        let t = (self.step_period_s * f64::from(TICK_HZ)).round();
        if !t.is_finite() || t < 1.0 { 1 } else { t as u64 }
    }

    /// The reserve intake into a body of this size stops at: `reserve_cap · body`. A
    /// ceiling on new intake and not a storage bound — a newborn's paid endowment starts
    /// above it (see [`SpeciesConfig::reserve_cap`]) — so a caller must not read this as
    /// "what it is holding" or clamp to it.
    pub fn reserve_of(&self, body: f64) -> f64 {
        self.reserve_cap * body
    }

    /// Structural refusals only, in the shape of [`cubarium_voxel_flora::FloraConfig`]'s:
    /// finite nonnegative numbers, and the four body thresholds in an order that can
    /// build a living animal. **Not** a plausibility check — the placeholders above are
    /// the values known to be arbitrary, and refusing them here would be tuning in a
    /// validator's clothes.
    pub fn validate(&self, name: &str) -> Result<(), String> {
        let fields: [(&str, f64); 12] = [
            ("maintenance_per_s", self.maintenance_per_s),
            ("bite_per_s", self.bite_per_s),
            ("yield_fraction", self.yield_fraction),
            ("n_tissue", self.n_tissue),
            ("body_max", self.body_max),
            ("body_min", self.body_min),
            ("birth_body", self.birth_body),
            ("birth_cost", self.birth_cost),
            ("reserve_cap", self.reserve_cap),
            ("wade_depth_m", self.wade_depth_m),
            ("drown_depth_m", self.drown_depth_m),
            ("energy_density", self.energy_density),
        ];
        for (field, v) in fields {
            if !v.is_finite() || v < 0.0 {
                return Err(format!("{name}.{field} must be finite and nonnegative, not {v}"));
            }
        }
        if !(self.yield_fraction <= 1.0) {
            return Err(format!("{name}.yield_fraction must be at most 1, not {}", self.yield_fraction));
        }
        if !(self.step_period_s.is_finite() && self.step_period_s > 0.0) {
            return Err(format!("{name}.step_period_s must be positive, not {}", self.step_period_s));
        }
        if !(self.body_min > 0.0) {
            return Err(format!("{name}.body_min must be positive, not {}", self.body_min));
        }
        if self.body_min > self.birth_body || self.birth_body > self.body_max {
            return Err(format!(
                "{name} needs body_min <= birth_body <= body_max, not {} <= {} <= {}",
                self.body_min, self.birth_body, self.body_max
            ));
        }
        if self.birth_cost < self.body_min {
            return Err(format!(
                "{name}.birth_cost {} cannot build a newborn of body_min {}",
                self.birth_cost, self.body_min
            ));
        }
        Ok(())
    }
}

/// The whole layer's numbers: one [`SpeciesConfig`] per species, and nothing else. There is
/// no global constant in this layer — no decomposition, no cap, no lottery — because every
/// rule it has belongs to an animal.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct FaunaConfig {
    pub frondgrazer: SpeciesConfig,
}

impl Default for FaunaConfig {
    fn default() -> FaunaConfig {
        FaunaConfig { frondgrazer: SpeciesConfig::frondgrazer() }
    }
}

impl FaunaConfig {
    pub fn species(&self, s: Species) -> &SpeciesConfig {
        match s {
            Species::Frondgrazer => &self.frondgrazer,
        }
    }

    pub fn species_mut(&mut self, s: Species) -> &mut SpeciesConfig {
        match s {
            Species::Frondgrazer => &mut self.frondgrazer,
        }
    }

    pub fn validate(&self) -> Result<(), String> {
        for s in Species::ALL {
            self.species(s).validate(s.name())?;
        }
        Ok(())
    }
}

/// Every unit of organic matter, mineral and energy that has crossed this layer's
/// boundary, and the counters of what happened.
///
/// The three currencies are the plant layer's ([`cubarium_voxel_flora::FloraLedger`]) and
/// mean the same thing. Internal moves — a bite becoming tissue, a reserve paying
/// maintenance, a parent paying for a newborn — never appear here.
///
/// **Two pairs of terms the brief did not name are here because the residuals do not close
/// without them.** An [`Command::Introduce`] creates an animal out of nothing, exactly as
/// the plant layer's `Seed` creates a founder stand, so it is booked `introduced_*_in`
/// the way that is booked `seeded_*_in`; and [`Command::Remove`] takes one out of the
/// world without a corpse, so it is booked `removed_*_out`. Both are named boundary flows
/// and neither is hidden.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct FaunaLedger {
    /// Organic matter, mineral and energy taken out of the plant layer by
    /// [`cubarium_voxel_flora::Flora::take_foliage`], cumulative over every bite. The
    /// sum of the [`cubarium_voxel_flora::Taken`]s the withdrawals returned, to the bit,
    /// so these three **equal** the plant layer's `consumed_*_out` for a run in which
    /// this layer is the only consumer.
    pub eaten_organic_in: f64,
    pub eaten_mineral_in: f64,
    pub eaten_energy_in: f64,
    /// Organic matter respired: maintenance, the undigested fraction of a bite, and
    /// anything assimilated that a full body and a full reserve could not hold. Mineral
    /// never crosses this boundary.
    pub respired_out: f64,
    /// The energy that left with it, at the density of the stock it left.
    pub heat_out: f64,
    /// Organic matter, mineral and energy handed back to the plant layer by
    /// [`cubarium_voxel_flora::Flora::deposit`]: a corpse as carrion, and excreted mineral
    /// as litter. Equal to the plant layer's `deposited_*_in` to the bit, for the same
    /// reason the `eaten_*` are.
    pub deposited_organic_out: f64,
    pub deposited_mineral_out: f64,
    pub deposited_energy_out: f64,
    /// What an [`Command::Introduce`] created out of nothing.
    pub introduced_organic_in: f64,
    pub introduced_mineral_in: f64,
    pub introduced_energy_in: f64,
    /// What a [`Command::Remove`] — or an animal whose support face the terrain took away
    /// — removed without a corpse.
    pub removed_organic_out: f64,
    pub removed_mineral_out: f64,
    pub removed_energy_out: f64,
    /// Every animal this layer has created, introduced ones included: a counter, not a
    /// flux, and the source of [`Animal::id`]. `births − introduced` is how many were
    /// born here.
    pub births: u64,
    /// Animals born of a parent's reserve.
    pub born: u64,
    /// Animals an [`Command::Introduce`] put into the world.
    pub introduced: u64,
    /// Animals that died: starved below `body_min`, or drowned.
    pub deaths: u64,
    /// Withdrawals that returned something.
    pub bites: u64,
    /// Bites and organic matter **by the plant species they came off**, indexed by
    /// [`cubarium_voxel_flora::Species::index`].
    ///
    /// The plant layer's `consumed_*` have no species in them and a reach box does not
    /// choose one (round 5a's note), and Astra's round 8 asks a consumer's report to say
    /// which species it actually ate: which stands a browser can reach depends on the
    /// face it stands on, so "what it ate" is a measurement and never a property of a
    /// species. This is that measurement, taken where the bite happens.
    pub bites_by_plant: [u64; cubarium_voxel_flora::Species::COUNT],
    pub eaten_by_plant: [f64; cubarium_voxel_flora::Species::COUNT],
    /// Steps taken, one voxel each.
    pub steps: u64,
}

impl FaunaLedger {
    pub fn expected_organic(&self) -> f64 {
        self.introduced_organic_in + self.eaten_organic_in
            - self.respired_out
            - self.deposited_organic_out
            - self.removed_organic_out
    }

    pub fn expected_mineral(&self) -> f64 {
        self.introduced_mineral_in + self.eaten_mineral_in
            - self.deposited_mineral_out
            - self.removed_mineral_out
    }

    pub fn expected_energy(&self) -> f64 {
        self.introduced_energy_in + self.eaten_energy_in
            - self.heat_out
            - self.deposited_energy_out
            - self.removed_energy_out
    }
}

/// Read-only access for drawing, inspection and a harness.
#[derive(Clone, Copy, Debug)]
pub struct FaunaView<'a> {
    pub config: &'a FaunaConfig,
    pub tick: u64,
    /// Sorted by [`Animal::id`], which never changes: an animal keeps its place in this
    /// slice for its whole life, whatever it walks over.
    pub animals: &'a [Animal],
    pub ledger: &'a FaunaLedger,
}

impl<'a> FaunaView<'a> {
    /// The animal with this identity, if it is still alive.
    pub fn animal(&self, id: u64) -> Option<&'a Animal> {
        self.animals.binary_search_by_key(&id, |a| a.id).ok().map(|i| &self.animals[i])
    }

    /// Every animal standing on this face, in id order. More than one is allowed: a
    /// newborn appears on its parent's face.
    pub fn animals_at(&self, site: Site) -> impl Iterator<Item = &'a Animal> {
        self.animals.iter().filter(move |a| a.site == site)
    }

    /// Organic matter in every animal: structure plus reserve.
    pub fn organic(&self) -> f64 {
        self.animals.iter().map(Animal::organic).sum()
    }

    /// Mineral in every animal's tissue.
    pub fn mineral(&self) -> f64 {
        self.animals.iter().map(|a| a.mineral).sum()
    }

    /// Energy in every animal's tissue.
    pub fn energy(&self) -> f64 {
        self.animals.iter().map(|a| a.energy).sum()
    }
}

/// A frontend, a harness or a test changes this layer only through these.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum Command {
    /// Put an animal of `body` on the **highest** support face of column `(x, z)`, with a
    /// full reserve, its species' `n_tissue` of mineral and its `energy_density` of
    /// energy — a body grown outside the system, booked `introduced_*_in`. Refused if the
    /// column has no support face or `body` is below the species' `body_min`.
    ///
    /// More than one animal may stand on a face, so this is never refused for being
    /// occupied.
    Introduce { x: i64, z: u32, species: Species, body: f64 },
    /// Take every animal off the highest support face of column `(x, z)`, booking their
    /// material as `removed_*_out`. No corpse: this is a frontend's undo, not a death.
    /// Refused if there is none.
    Remove { x: i64, z: u32 },
}

/// The animal layer. Owns its animals; borrows the world and the plant layer per call.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Fauna {
    config: FaunaConfig,
    tick: u64,
    /// Sorted by id, so every pass over them is in one stable order however they move.
    /// A `Vec` with a binary search, never a `HashMap`: this layer iterates nothing
    /// unordered.
    animals: Vec<Animal>,
    ledger: FaunaLedger,
}

impl Fauna {
    /// An animal layer on a **valid** config. Panics with the offending field named if
    /// [`FaunaConfig::validate`] refuses it, exactly as
    /// [`cubarium_voxel_flora::Flora::new`] does and for the same reason: a preset that
    /// cannot build a living animal is a programming error in the preset.
    pub fn new(config: FaunaConfig) -> Fauna {
        match Fauna::try_new(config) {
            Ok(fauna) => fauna,
            Err(e) => panic!("invalid FaunaConfig — {e}"),
        }
    }

    /// [`Fauna::new`] without the panic.
    pub fn try_new(config: FaunaConfig) -> Result<Fauna, String> {
        config.validate()?;
        Ok(Fauna { config, tick: 0, animals: Vec::new(), ledger: FaunaLedger::default() })
    }

    pub fn config(&self) -> &FaunaConfig {
        &self.config
    }

    pub fn tick(&self) -> u64 {
        self.tick
    }

    pub fn view(&self) -> FaunaView<'_> {
        FaunaView {
            config: &self.config,
            tick: self.tick,
            animals: &self.animals,
            ledger: &self.ledger,
        }
    }

    /// Advance one tick against the world and the plant layer as they stand. Call **after**
    /// `World::step` and after `Flora::step`: the plant layer reads the water the core has
    /// just moved and grows, and then the animals eat what is there. The order within the
    /// tick is [`step`]'s module doc, and it is written down in exactly one place.
    pub fn step(&mut self, world: &World, flora: &mut Flora) {
        step::step(self, world, flora, 1);
    }

    /// [`Fauna::step`] with a thread count for the one phase that splits: `sense`, which
    /// only reads. `1` is [`Fauna::step`] exactly.
    ///
    /// **Execution, never a rule.** Every animal's plan is a pure function of the world,
    /// the plant layer and that animal, the plans are gathered in animal order whatever
    /// order the chunks finish in, and `act` still applies them serially in id order. So
    /// the thread count cannot reach a result, and with the `parallel` feature off it is
    /// ignored altogether (`design/handoffs/voxel-schedule-brief-2026-09-18.md`).
    pub fn step_with(&mut self, world: &World, flora: &mut Flora, threads: usize) {
        step::step(self, world, flora, threads);
    }

    /// Apply a command now, between ticks, the way the plant layer's are applied. Returns
    /// whether it was accepted.
    pub fn apply(&mut self, world: &World, command: Command) -> bool {
        match command {
            Command::Introduce { x, z, species, body } => {
                let view = world.view();
                let Some(site) = cubarium_voxel_flora::highest_support(&view, x, z) else {
                    return false;
                };
                let sc = *self.config.species(species);
                if !(body.is_finite() && body >= sc.body_min) {
                    return false;
                }
                let reserve = sc.reserve_of(body);
                let organic = body + reserve;
                let animal = Animal {
                    id: self.ledger.births,
                    species,
                    site,
                    body,
                    reserve,
                    mineral: sc.n_tissue * organic,
                    energy: sc.energy_density * organic,
                    age_ticks: 0,
                    state: State::Resting,
                };
                self.ledger.births += 1;
                self.ledger.introduced += 1;
                self.ledger.introduced_organic_in += organic;
                self.ledger.introduced_mineral_in += animal.mineral;
                self.ledger.introduced_energy_in += animal.energy;
                self.insert(animal);
                true
            }
            Command::Remove { x, z } => {
                let view = world.view();
                let Some(site) = cubarium_voxel_flora::highest_support(&view, x, z) else {
                    return false;
                };
                let before = self.animals.len();
                let mut removed = Vec::new();
                self.animals.retain(|a| {
                    if a.site == site {
                        removed.push(*a);
                        false
                    } else {
                        true
                    }
                });
                for a in removed {
                    self.book_removed(&a);
                }
                self.animals.len() != before
            }
        }
    }

    /// Save as postcard bytes behind this layer's own schema tag. A different tag is
    /// refused by [`Fauna::load`]; there is no migration, ever
    /// (`always-fresh-never-migrate`).
    pub fn save(&self) -> Vec<u8> {
        snapshot::encode(self)
    }

    /// Read a layer back. Refuses another schema tag, corrupt bytes, and a config no
    /// animal could live under.
    pub fn load(bytes: &[u8]) -> anyhow::Result<Fauna> {
        snapshot::decode(bytes)
    }

    /// Insert keeping the id order. An id is never reused, so this is always an insert.
    fn insert(&mut self, animal: Animal) {
        match self.animals.binary_search_by_key(&animal.id, |a| a.id) {
            Ok(_) => unreachable!("an animal id is never reused"),
            Err(at) => self.animals.insert(at, animal),
        }
    }

    /// One withdrawal on the boundary: the three `eaten_*` flows, and the same
    /// [`Taken`] back, so what this ledger books and what the animal received are one
    /// value and cannot drift from the plant layer's `consumed_*`.
    fn book_eaten(&mut self, taken: Taken) -> Taken {
        self.ledger.eaten_organic_in += taken.organic;
        self.ledger.eaten_mineral_in += taken.mineral;
        self.ledger.eaten_energy_in += taken.energy;
        self.ledger.bites += 1;
        taken
    }

    /// Material out of this layer and into the plant layer's pools, booked whichever way
    /// it went.
    ///
    /// A triplet whose three numbers are all zero is not a deposit and the plant layer
    /// refuses it; there is nothing to lose and nothing is booked. A triplet it refuses
    /// for any *other* reason would otherwise strand material on a dead animal, so it is
    /// booked out as `removed_*` instead — this layer never keeps an inert remainder.
    fn book_deposit(&mut self, flora: &mut Flora, site: Site, kind: DepositKind, d: Taken) {
        if d.organic <= 0.0 && d.mineral <= 0.0 && d.energy <= 0.0 {
            return;
        }
        let deposit = Deposit {
            kind,
            organic: d.organic.max(0.0),
            mineral: d.mineral.max(0.0),
            energy: d.energy.max(0.0),
        };
        if flora.deposit(site, deposit) {
            self.ledger.deposited_organic_out += deposit.organic;
            self.ledger.deposited_mineral_out += deposit.mineral;
            self.ledger.deposited_energy_out += deposit.energy;
        } else {
            self.ledger.removed_organic_out += deposit.organic;
            self.ledger.removed_mineral_out += deposit.mineral;
            self.ledger.removed_energy_out += deposit.energy;
        }
    }

    /// An animal leaving the world without a corpse.
    fn book_removed(&mut self, a: &Animal) {
        self.ledger.removed_organic_out += a.organic();
        self.ledger.removed_mineral_out += a.mineral;
        self.ledger.removed_energy_out += a.energy;
    }
}

/// Every support face of column `(x, z)` an animal on `from` could step onto, ascending in
/// `y`: within `climb` of `from.y`, and with no more than `wade_depth_m` of standing water
/// on it.
///
/// Public because the geometry is worth stating without a tick in the way — a test, or a
/// harness choosing where to introduce an animal, asks exactly this question.
pub fn steppable(
    view: &VoxelView<'_>,
    from: Site,
    x: i64,
    z: u32,
    sc: &SpeciesConfig,
) -> Vec<Site> {
    let c = view.config;
    if z >= c.depth {
        return Vec::new();
    }
    let climb = i64::from(sc.climb);
    let mut out = Vec::new();
    for y in 0..c.height {
        if !view.is_support(x, y, z) {
            continue;
        }
        if (i64::from(y) - i64::from(from.y)).abs() > climb {
            continue;
        }
        if view.water_depth_m(x, y, z) > sc.wade_depth_m {
            continue;
        }
        out.push(Site { x: x.rem_euclid(i64::from(c.width)) as u32, y, z });
    }
    out
}
