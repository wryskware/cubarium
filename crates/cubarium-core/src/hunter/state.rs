use cubarium_surface::Topology;
use serde::{Deserialize, Serialize};

use cubarium_surface::{Face, SurfacePoint};

use crate::config::WorldConfig;
use crate::ids::{OrganismId, Slots};
use crate::organism::Organism;

use super::*;

/// `Stream::Hunt` key of the one founder placement draw (its heading). Every other draw in
/// the stream is keyed by a hunter's packed full ID, which can never be this value because a
/// live generation is never `u32::MAX` in the same slot as `u32::MAX`.
pub const FOUNDER_DRAW_KEY: u64 = u64::MAX;

/// A hunter's full ID packed into a draw key: `(slot << 32) | generation`.
pub fn draw_key(id: OrganismId) -> u64 {
    (u64::from(id.slot) << 32) | u64::from(id.generation)
}

/// Draw counters of one paid attempt: the contested-claim priority, then the capture roll.
/// Two draws per paid attempt and none for anything else (`crate::rng::Stream::Hunt`).
pub fn priority_counter(attack_counter: u64) -> u64 {
    attack_counter.saturating_mul(2)
}

pub fn capture_counter(attack_counter: u64) -> u64 {
    attack_counter.saturating_mul(2) + 1
}

/// The semantic role the art direction selected, independent of `genome.form`. The renderer
/// maps the role to a body; the world never reads it.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum HunterRole {
    /// Fable's Lanternjaw, the body Wrysk selected.
    Lanternjaw,
}

impl HunterRole {
    pub fn as_str(self) -> &'static str {
        match self {
            HunterRole::Lanternjaw => "lanternjaw",
        }
    }
}

/// Where a hunter founder or its budget-matched control deposit goes: a canonical surface
/// point, `face` a chart of the world's topology and `u`/`v` inside that chart's extent.
///
/// The bounds are the **topology's**, not the cube's 64 (`design/flat-world-plan-2026-09-16.md`
/// §5, repair 3 finding 2): on a 320×180 ring a target at `u = 200` is on the world and a
/// target on `Face::Top` is not. This is the core type, with `f64` coordinates; the host's
/// `CareTarget` in `crates/cubarium/src/care/mod.rs` is a different type with the same name.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct HunterTarget {
    pub face: u8,
    pub u: f64,
    pub v: f64,
}

impl HunterTarget {
    /// The surface point named, or `None` when the chart is not one this topology has or the
    /// coordinates are outside its extent. Never panics on hostile input.
    pub fn resolve(&self, topo: Topology) -> Option<SurfacePoint> {
        let face = Face::from_index(self.face)?;
        if !topo.has_chart(face) {
            return None;
        }
        if !self.u.is_finite() || !self.v.is_finite() {
            return None;
        }
        let (ext_u, ext_v) = topo.extent(face);
        if self.u < 0.0 || self.u >= ext_u || self.v < 0.0 || self.v >= ext_v {
            return None;
        }
        Some(SurfacePoint::new(face, self.u, self.v).canonicalize(topo))
    }
}

/// Where one hunter is in its local hunt. Phases are per member and never global.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum HunterPhase {
    /// Resting in place: fed, recovering from nothing, or with attacks disabled.
    Perched,
    /// Moving toward one locally sensed eligible prey.
    Stalking,
    /// The folded-to-open gesture. Grants no capture.
    Windup,
    /// The paid burst. The attempt resolves after both creatures have moved.
    Strike,
    /// Paid for an attempt and failed, or just finished a meal.
    Recovering,
    /// Carrying and digesting a carcass. No attacks and no intake from the fields.
    Handling,
}

impl HunterPhase {
    /// Phases that end at a stored tick. The others end on a condition (satiety, contact, an
    /// empty gut) and store `phase_ends_tick == phase_started_tick`.
    pub fn is_timed(self) -> bool {
        matches!(
            self,
            HunterPhase::Windup | HunterPhase::Strike | HunterPhase::Recovering
        )
    }

    /// Phases in which the hunter is pursuing a specific prey.
    pub fn hunting(self) -> bool {
        matches!(
            self,
            HunterPhase::Stalking | HunterPhase::Windup | HunterPhase::Strike
        )
    }

    pub fn as_str(self) -> &'static str {
        match self {
            HunterPhase::Perched => "perched",
            HunterPhase::Stalking => "stalking",
            HunterPhase::Windup => "windup",
            HunterPhase::Strike => "strike",
            HunterPhase::Recovering => "recovering",
            HunterPhase::Handling => "handling",
        }
    }
}

/// One member of the lineage. The organism itself is an ordinary [`Organism`] in the arena;
/// this record is everything the hunt adds.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct HunterMember {
    /// Slot **and** generation: a stale handle never resolves to a reused slot.
    pub id: OrganismId,
    pub phase: HunterPhase,
    pub phase_started_tick: u64,
    /// End tick of a timed phase; equal to `phase_started_tick` for the others.
    pub phase_ends_tick: u64,
    /// The prey being pursued, by full ID. Cleared deterministically when it goes stale.
    pub target: Option<OrganismId>,
    /// Paid attempts this member has made: the counter of its `Stream::Hunt` draws.
    pub attack_counter: u64,
    /// Earliest tick a new gestation may start.
    pub next_reproduction_tick: u64,
    /// The carried carcass: material and the energy actually removed with it.
    pub gut_material: f64,
    pub gut_energy: f64,
    /// The phase this member was in when it entered its current one.
    ///
    /// Persisted because durations alone cannot tell a recoil apart: `Recovering` entered from
    /// `Strike` follows a fully extended miss, from `Handling` a finished meal, and `Perched`
    /// entered from `Windup` an aborted or unaffordable attempt. The adapter reads a fact, it
    /// does not guess one from clocks that happen to differ (`lanternjaw-core-art-integration
    /// -gaps-2026-09-13.md`). This is a semantic transition origin, not an animation state.
    pub entered_from: HunterPhase,
    /// The attack episode the current phase belongs to: the `attack_counter` of the paid
    /// attempt that produced it, or 0 when it belongs to no paid attempt. Windup is not yet a
    /// paid attempt and carries 0 until the strike is charged.
    pub episode: u64,
}

impl HunterMember {
    /// A newly placed member: perched, no target, empty gut.
    pub fn new(id: OrganismId, tick: u64) -> HunterMember {
        HunterMember {
            id,
            phase: HunterPhase::Perched,
            phase_started_tick: tick,
            phase_ends_tick: tick,
            target: None,
            attack_counter: 0,
            next_reproduction_tick: 0,
            gut_material: 0.0,
            gut_energy: 0.0,
            entered_from: HunterPhase::Perched,
            episode: 0,
        }
    }

    /// Enter `phase` at `tick`, ending at `ends` (pass `tick` for an untimed phase), recording
    /// the phase it came from and the attack episode it belongs to.
    pub fn enter(&mut self, phase: HunterPhase, tick: u64, ends: u64, episode: u64) {
        self.entered_from = self.phase;
        self.phase = phase;
        self.phase_started_tick = tick;
        self.phase_ends_tick = ends.max(tick);
        self.episode = episode;
        if !phase.hunting() {
            self.target = None;
        }
    }

    /// Progress through a timed phase in `[0, 1]`, `None` for the others.
    pub fn progress(&self, tick: u64) -> Option<f32> {
        if !self.phase.is_timed() {
            return None;
        }
        let span = self.phase_ends_tick.saturating_sub(self.phase_started_tick);
        if span == 0 {
            return Some(1.0);
        }
        let done = tick.saturating_sub(self.phase_started_tick).min(span);
        Some((done as f64 / span as f64) as f32)
    }

    pub fn carrying(&self) -> bool {
        self.gut_material > 0.0
    }
}

/// The whole hunter extension, appended to `WorldState` at schema 10. Default is empty and
/// inert: no profile, no members, no imports, no counters.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct HunterState {
    /// `None` until an experiment initializes one. Present with an empty member list after
    /// every member has died, or in a budget-matched control arm.
    pub profile: Option<FixedHunterProfile>,
    /// Members sorted by full ID.
    pub members: Vec<HunterMember>,
    /// Material and energy admitted from outside for hunter founders, booked once here and
    /// never again in `external_material_in`.
    pub founder_material_in: f64,
    pub founder_energy_in: f64,
    /// The budget-matched control deposit: the same derived founder inventory placed as local
    /// `D`/`De` with no hunter.
    pub control_material_in: f64,
    pub control_energy_in: f64,
    pub control_deposited: bool,
    /// Founders placed by [`crate::World::start_hunter_trial`]; the initializer refuses a
    /// second one.
    pub founders_placed: u32,
    /// Paid attempts, successful captures, and prey consumed. Separate from the three
    /// persisted natural-death counters, which keep their shape and their wire position.
    pub attacks_total: u64,
    pub captures_total: u64,
    pub predation_deaths_total: u64,
    pub hunter_deaths_total: u64,
    pub hunter_births_total: u64,
}

impl HunterState {
    /// True when this world has hunters to run. An empty extension is skipped whole.
    pub fn active(&self) -> bool {
        self.profile.is_some() && !self.members.is_empty()
    }

    /// The profile of an active extension.
    pub fn profile(&self) -> Option<&FixedHunterProfile> {
        self.profile.as_ref()
    }

    /// Index of a member by full ID (binary search: members are sorted).
    pub fn index_of(&self, id: OrganismId) -> Option<usize> {
        self.members.binary_search_by(|m| m.id.cmp(&id)).ok()
    }

    pub fn contains(&self, id: OrganismId) -> bool {
        self.index_of(id).is_some()
    }

    pub fn member(&self, id: OrganismId) -> Option<&HunterMember> {
        self.index_of(id).map(|i| &self.members[i])
    }

    pub fn member_mut(&mut self, id: OrganismId) -> Option<&mut HunterMember> {
        self.index_of(id).map(|i| &mut self.members[i])
    }

    /// Insert a member keeping the list sorted; returns false if the ID is already a member.
    pub fn insert_member(&mut self, member: HunterMember) -> bool {
        match self.members.binary_search_by(|m| m.id.cmp(&member.id)) {
            Ok(_) => false,
            Err(at) => {
                self.members.insert(at, member);
                true
            }
        }
    }

    /// Remove a member and invalidate every other member's reference to it at `tick`, returning
    /// the record.
    pub fn remove_member(&mut self, id: OrganismId, tick: u64) -> Option<HunterMember> {
        let at = self.index_of(id)?;
        let gone = self.members.remove(at);
        self.forget_target(id, tick);
        Some(gone)
    }

    /// Invalidate every member's reference to a prey that left the arena at `tick`.
    ///
    /// An unpaid stalk or windup cannot continue without its prey, so it returns to the
    /// ordinary perched state at the removal boundary. A paid strike keeps its phase and later
    /// settles as a lost target; clearing it here must never refund or retry that attempt.
    pub fn forget_target(&mut self, id: OrganismId, tick: u64) {
        for m in &mut self.members {
            if m.target == Some(id) {
                m.target = None;
                if matches!(m.phase, HunterPhase::Stalking | HunterPhase::Windup) {
                    m.enter(HunterPhase::Perched, tick, tick, 0);
                }
            }
        }
    }

    pub fn gut_material_total(&self) -> f64 {
        self.members.iter().map(|m| m.gut_material).sum()
    }

    pub fn gut_energy_total(&self) -> f64 {
        self.members.iter().map(|m| m.gut_energy).sum()
    }

    /// Material and energy this extension has admitted from outside the world.
    pub fn imported_material(&self) -> f64 {
        self.founder_material_in + self.control_material_in
    }

    pub fn imported_energy(&self) -> f64 {
        self.founder_energy_in + self.control_energy_in
    }

    /// Range checks after decode, against the arena the extension belongs to.
    ///
    /// A **stale target is allowed**: a snapshot can be written on the tick its prey died, and
    /// a generation-checked handle to a dead organism is harmless — the tick clears it. What is
    /// refused is a member that does not exist, a duplicate or out-of-order member list, a
    /// gut past its bound, energy in an empty gut, a hunter targeting itself or another
    /// hunter, phases that contradict their stored ticks, and any import or counter that is
    /// not a finite nonnegative number.
    pub fn validate(
        &self,
        tick: u64,
        organisms: &Slots<Organism>,
        config: &WorldConfig,
    ) -> Result<(), String> {
        for (name, v) in [
            ("founder_material_in", self.founder_material_in),
            ("founder_energy_in", self.founder_energy_in),
            ("control_material_in", self.control_material_in),
            ("control_energy_in", self.control_energy_in),
        ] {
            if !v.is_finite() || v < 0.0 {
                return Err(format!("hunter {name} = {v}"));
            }
        }
        let Some(profile) = &self.profile else {
            if !self.members.is_empty() {
                return Err("hunter members exist without a profile".into());
            }
            if self.founders_placed > 0 || self.control_deposited {
                return Err("hunter imports were booked without a profile".into());
            }
            return Ok(());
        };
        profile.validate()?;
        if self.control_deposited && !self.members.is_empty() {
            return Err("a budget-matched control world must not hold hunter members".into());
        }
        if self.members.len() > config.capacity.max_organisms as usize {
            return Err(format!(
                "hunter members {} exceed the organism cap",
                self.members.len()
            ));
        }
        let mut previous: Option<OrganismId> = None;
        for m in &self.members {
            if let Some(p) = previous
                && m.id <= p
            {
                return Err(format!(
                    "hunter members are not sorted and unique: {:?} follows {:?}",
                    m.id, p
                ));
            }
            previous = Some(m.id);
            let who = format!("hunter {}:{}", m.id.slot, m.id.generation);
            let Some(_) = organisms.get(m.id) else {
                return Err(format!("{who} is not a live organism"));
            };
            for (name, v) in [
                ("gut_material", m.gut_material),
                ("gut_energy", m.gut_energy),
            ] {
                if !v.is_finite() || v < 0.0 {
                    return Err(format!("{who}: {name} = {v}"));
                }
            }
            if m.gut_material > profile.gut_capacity_material + TOLERANCE {
                return Err(format!(
                    "{who}: gut_material {} exceeds the profile capacity {}",
                    m.gut_material, profile.gut_capacity_material
                ));
            }
            if m.gut_material <= 0.0 && m.gut_energy > TOLERANCE {
                return Err(format!(
                    "{who}: an empty gut carries {} energy",
                    m.gut_energy
                ));
            }
            if m.episode > m.attack_counter {
                return Err(format!(
                    "{who}: phase episode {} is ahead of its attack counter {}",
                    m.episode, m.attack_counter
                ));
            }
            if m.episode > 0
                && !matches!(
                    m.phase,
                    HunterPhase::Strike | HunterPhase::Recovering | HunterPhase::Handling
                )
            {
                return Err(format!(
                    "{who}: phase {} claims attack episode {}",
                    m.phase.as_str(),
                    m.episode
                ));
            }
            if m.phase_started_tick > tick {
                return Err(format!(
                    "{who}: phase started at {} after tick {tick}",
                    m.phase_started_tick
                ));
            }
            if m.phase_ends_tick < m.phase_started_tick {
                return Err(format!("{who}: phase ends before it started"));
            }
            if m.phase.is_timed() && m.phase_ends_tick == m.phase_started_tick {
                return Err(format!(
                    "{who}: timed phase {} has no duration",
                    m.phase.as_str()
                ));
            }
            if !m.phase.is_timed() && m.phase_ends_tick != m.phase_started_tick {
                return Err(format!(
                    "{who}: untimed phase {} stores an end tick",
                    m.phase.as_str()
                ));
            }
            if m.phase == HunterPhase::Handling && !m.carrying() {
                return Err(format!("{who}: handling nothing"));
            }
            if m.phase == HunterPhase::Stalking && m.target.is_none() {
                return Err(format!("{who}: stalking no one"));
            }
            if !m.phase.hunting() && m.target.is_some() {
                return Err(format!("{who}: phase {} holds a target", m.phase.as_str()));
            }
            if let Some(target) = m.target {
                if target == m.id {
                    return Err(format!("{who} is hunting itself"));
                }
                // A stale handle is fine; a live one must not be another predator.
                if organisms.get(target).is_some() && self.contains(target) {
                    return Err(format!("{who} is hunting another hunter"));
                }
            }
        }
        Ok(())
    }
}
