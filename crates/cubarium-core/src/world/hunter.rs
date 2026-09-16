use std::f64::consts::TAU;

use cubarium_surface::{Vec2, cell_of};

use crate::DT;
use crate::accounting::{self};
use crate::dormancy::{ApexDormancyEvent, ApexDormancyPolicy, ApexDormancyState};
use crate::encounter::{ApexEncounterEvent, ApexEncounterPolicy, ApexEncounterState};
use crate::genome::decode;
use crate::hunter::{
    self, FixedHunterProfile, HunterControlReceipt, HunterEvent, HunterFounderReceipt, HunterState,
    HunterTarget, HunterView,
};
use crate::organism::{Mode, Organism, Origin};
use crate::quiet::{QuietEvent, QuietState};
use crate::rng::{Counter, Stream, unit};

use super::*;

use super::lifecycle::ticks_from_seconds;
use super::state::{ChargingDiagnostics, HunterFounder, IntakeDiagnostics};

impl World {
    /// What the member oxidation policy did that this world's configured threshold would not
    /// have done, since this `World` value was built. Read-only, transient and process-scoped;
    /// see [`ChargingDiagnostics`] for the exact transaction scope and time window.
    pub fn charging_diagnostics(&self) -> ChargingDiagnostics {
        self.charging
    }

    /// What this world's producers actually made and its mouths actually took, since this
    /// `World` value was built. Read-only, transient and process-scoped; see
    /// Install the transient diagnostic intent overrides for the ticks that follow
    /// (`crate::diagnostic`). Replaces any previous list; an empty list is off, which is
    /// where every ordinary world stays. Nothing here is persisted, and an override is an
    /// intent that the ordinary resolver, the world's intake law and the funding checks still
    /// answer — see [`crate::diagnostic::ScriptedIntent`].
    pub fn set_scripted_intents(
        &mut self,
        intents: Vec<(crate::ids::OrganismId, crate::diagnostic::ScriptedIntent)>,
    ) {
        self.scripted = intents;
    }

    /// Remove every diagnostic intent override.
    pub fn clear_scripted_intents(&mut self) {
        self.scripted.clear();
    }

    /// [`IntakeDiagnostics`] for the exact scope and time window.
    pub fn intake_diagnostics(&self) -> IntakeDiagnostics {
        self.intake
    }

    /// [`crate::world::state::NeuralTiming`] accumulated since this `World` was constructed.
    /// Bracket a run with two readings and take `since` to time one arm.
    pub fn neural_timing(&self) -> crate::world::state::NeuralTiming {
        self.neural_timing
    }

    /// The oxidation activation threshold, as a fraction of `E_max`, that an **authoritative
    /// member** of this world runs under — the world's configured one when there is no
    /// profile, or no raised policy on it. Published so a recorded experiment can state the
    /// resolved number it actually ran, rather than a reader inferring it from a version.
    pub fn member_oxidation_threshold(&self) -> f64 {
        let org = &self.state.config.organism;
        match self.state.hunters.profile.as_ref() {
            Some(profile) => profile.oxidation_threshold(org),
            None => org.oxidation_threshold,
        }
    }

    /// Record one [`crate::hunter::StrikeRecord`] per paid attempt, or stop recording.
    ///
    /// **Off by default and inert.** The flag is read only at the three sites in the strike
    /// path that build a frame; it consumes no draw, moves no value the simulation reads back,
    /// and is never persisted or hashed. Turning it off drops everything in flight and
    /// everything undrained, so a half-recorded attempt is never published as a record.
    ///
    /// Independent of [`crate::World::record_body_budgets`]: the ledger says what a body
    /// raised and spent over its life, this says what one of its lunges was aimed at.
    pub fn record_strike_attempts(&mut self, on: bool) {
        self.strikes.set_enabled(on);
    }

    /// Whether this world is recording per-attempt strike records.
    pub fn records_strike_attempts(&self) -> bool {
        self.strikes.enabled()
    }

    /// **Which pursuit stopping rule this world's hunt-intent pass runs.** Off-by-default in
    /// the only sense that matters: an untouched world is
    /// [`crate::hunter::PursuitStop::ForwardHalfSpace`], the shipped rule, and is byte-identical
    /// to a world that never heard of this switch.
    ///
    /// The rule decides, for a member hunting a target it senses, whether the prey counts as
    /// "inside" — and a member for which it is true drops to its `rest_effort` and is pushed
    /// **no strike burst**, whatever it has already paid. The shipped rule is a one-sided
    /// forward half-space; the variant is the reach envelope the predicate's own comment names
    /// (`crate::hunter::ContactMeasure::in_contact`). It was measured true at the burst's start
    /// on 408 of 449 paid attempts of the two-apex death arm
    /// (`design/7_Research/ecology-v1-apex-reach-2026-09-16.md`), and this switch exists so the
    /// paired arm can be run without changing the rule for anyone
    /// (`design/7_Research/ecology-v1-apex-predicate-2026-09-16.md`).
    ///
    /// **This is not a [`crate::WorldConfig`] field and must not become one** until it is a
    /// decision: a config field would change `calibrate::config_hash` for every existing TOML.
    /// It is transient, never persisted, never hashed, and set per `World` value.
    pub fn set_pursuit_stop(&mut self, stop: crate::hunter::PursuitStop) {
        self.strikes.set_pursuit_stop(stop);
    }

    /// The rule [`crate::World::set_pursuit_stop`] installed, or the shipped default.
    pub fn pursuit_stop(&self) -> crate::hunter::PursuitStop {
        self.strikes.pursuit_stop()
    }

    /// The strike records closed since the last drain, oldest first, with how many were
    /// dropped because more than [`crate::hunter::MAX_STRIKE_RECORDS`] accumulated undrained,
    /// and how many resolved with no intent frame (only possible across a mid-attempt
    /// switch-on).
    pub fn drain_strike_records(&mut self) -> (Vec<crate::hunter::StrikeRecord>, u64, u64) {
        self.strikes.drain()
    }

    /// The hunter extension: profile, members, guts, imports and counters (`crate::hunter`).
    pub fn hunters(&self) -> &HunterState {
        &self.state.hunters
    }

    /// Start the opt-in hunter trial: validate everything, then place **exactly one** founder
    /// of the fixed lineage at `target` and book what it imported.
    ///
    /// Refused, without changing a single value, when the extension is already initialized,
    /// when this world is a budget-matched control, when the profile or the target is invalid,
    /// when the body does not fit the world's own `body_extent_max`, or when there is no
    /// organism capacity left. The founder inventory is **derived from this world's config**
    /// (`S = S_adult`, `R = fraction · R_max`, `E = fraction · E_max`), never hardcoded, and
    /// its material and energy are booked once in the extension's import ledgers — never again
    /// in `external_material_in`, and never by resetting the world's opening history.
    ///
    /// One `Stream::Hunt` draw (key [`hunter::FOUNDER_DRAW_KEY`], counter 0) picks the founder
    /// heading; nothing else in the world consumes a draw here.
    pub fn start_hunter_trial(
        &mut self,
        profile: FixedHunterProfile,
        target: HunterTarget,
    ) -> Result<HunterFounderReceipt, String> {
        if self.state.quiet.active() {
            return Err(
                "this world runs an ordinary quiet policy; the hunter extension is not combined \
                 with it in this slice, because threat and escape interactions need their own \
                 explicit contract"
                    .into(),
            );
        }
        if self.state.hunters.control_deposited {
            return Err("this world is a budget-matched control and must not gain a hunter".into());
        }
        if self.state.hunters.profile.is_some() || self.state.hunters.founders_placed > 0 {
            return Err("the hunter extension is already initialized".into());
        }
        let founder = self.derive_hunter_founder(&profile)?;
        let Some(pos) = target.resolve() else {
            return Err(format!(
                "hunter founder target {target:?} is not on the surface"
            ));
        };
        let cap = self.state.config.capacity.max_organisms as usize;
        if self.state.organisms.len() >= cap {
            return Err(format!(
                "no organism capacity for a hunter founder (population {cap})"
            ));
        }

        // Everything above validates; only now does anything change.
        let tick = self.state.tick;
        let heading = Vec2::from_screen_angle(
            unit(
                self.state.config.seed,
                Stream::Hunt,
                hunter::FOUNDER_DRAW_KEY,
                0,
            ) * TAU,
        );
        let hunger_memory = (1.0 - founder.reserve / founder.phenotype.reserve_max).clamp(0.0, 1.0);
        let id = self.state.organisms.insert(Organism {
            pos,
            heading,
            ou: Vec2::ZERO,
            structure: founder.structure,
            reserve: founder.reserve,
            energy: founder.energy,
            born_tick: tick,
            hunger_memory,
            mode: Mode::Resting,
            escrow: None,
            births: 0,
            genome: profile.genome.clone(),
            phenotype: founder.phenotype.clone(),
            parent: None,
            origin: Origin::Founder,
            turn_counter: Counter::default(),
            fed_this_tick: false,
        });
        let receipt = HunterFounderReceipt {
            id,
            tick,
            pos,
            structure: founder.structure,
            reserve: founder.reserve,
            energy: founder.energy,
            material_in: founder.material_in,
            energy_in: founder.energy_in,
            extent: founder.phenotype.extent,
            sense_radius: founder.phenotype.sense_radius,
            geometry: hunter::ContactGeometry {
                // The founder is an adult, so this is the profile's own geometry at scale 1;
                // it is published here so the art adapter and the harness read the same
                // numbers the world will test contact with.
                scale: 1.0,
                capture_offset_body: profile.capture_offset_body,
                capture_reach_px: profile.capture_reach_px,
                ingestion_offset_body: profile.ingestion_offset_body,
                visual_query_extent_px: profile.visual_query_extent_px,
            },
        };
        self.state.hunters.profile = Some(profile);
        self.state
            .hunters
            .insert_member(hunter::HunterMember::new(id, tick));
        self.state.hunters.founder_material_in += founder.material_in;
        self.state.hunters.founder_energy_in += founder.energy_in;
        self.state.hunters.founders_placed += 1;
        // The import is booked, so the closed box has not moved: a control run and a hunter
        // run are comparable on the same residual.
        debug_assert!(
            self.mass_residual().abs() < 1e-9,
            "founding moved the mass residual"
        );
        Ok(receipt)
    }

    /// Add one or two mature founders to an existing or new hunter lineage.
    ///
    /// Unlike the singleton trial initializer, this permits repeated introductions. All
    /// targets, profile compatibility and capacity are validated before any state changes,
    /// so a two-founder request is never partially applied. Successful introductions enable
    /// paired encounters and underground offspring; the new adults remain active on surface.
    pub fn introduce_hunters(
        &mut self,
        profile: FixedHunterProfile,
        targets: &[HunterTarget],
    ) -> Result<Vec<HunterFounderReceipt>, String> {
        self.introduce_hunters_with_age(profile, targets, 0.0)
    }

    /// [`World::introduce_hunters`], with the founders placed as if they had **already lived**
    /// for `age_seconds`.
    ///
    /// The only value this moves is the placed body's `born_tick`, which is the world's single
    /// source of age (`Organism::age_ticks`). Stores, geometry, headings, draws, member
    /// records, imports and receipts are what the age-zero door produces, so `age_seconds ==
    /// 0.0` is that door byte for byte — the one `introduce_hunters` itself now calls.
    ///
    /// It exists for one measurement. The apex opportunity audit found that two introduced
    /// adults were never simultaneously able to reproduce because every one of them died at
    /// 43–59 % of the `reproduce_min_age_seconds` its own profile demands
    /// (`design/7_Research/ecology-v1-budget-2026-09-16.md`). Separating "the age gate is what
    /// binds" from "something after the age gate also binds" needs a founder that is already
    /// past that gate and nothing else changed. No constant moved to provide it, and the
    /// display's ordinary spawn control does not use it.
    ///
    /// Refused, without changing a single value, for everything
    /// [`World::introduce_hunters`] refuses, and additionally when `age_seconds` is not a
    /// finite non-negative number, when it exceeds this world's `organism.max_age_seconds`
    /// (a founder that is already dead of old age), or when it exceeds the world's own age —
    /// a body cannot have been born before the world began, and `WorldState::validate` holds
    /// every organism to `born_tick <= tick`.
    pub fn introduce_hunters_with_age(
        &mut self,
        profile: FixedHunterProfile,
        targets: &[HunterTarget],
        age_seconds: f64,
    ) -> Result<Vec<HunterFounderReceipt>, String> {
        if !(1..=2).contains(&targets.len()) {
            return Err(
                "an interactive hunter introduction must contain one or two founders".into(),
            );
        }
        if self.state.hunters.control_deposited {
            return Err("this world is a budget-matched control and must not gain a hunter".into());
        }
        if let Some(installed) = self.state.hunters.profile.as_ref()
            && installed != &profile
        {
            return Err("the requested hunter profile does not match the installed lineage".into());
        }
        if !age_seconds.is_finite() || age_seconds < 0.0 {
            return Err(format!(
                "a founder's age at introduction must be a finite, non-negative number of \
                 seconds, not {age_seconds}"
            ));
        }
        let lifespan_seconds = self.state.config.organism.max_age_seconds;
        if age_seconds > lifespan_seconds {
            return Err(format!(
                "a founder introduced at {age_seconds} s would already be past this world's \
                 {lifespan_seconds} s lifespan"
            ));
        }
        let age_ticks = ticks_from_seconds(age_seconds, DT);
        if age_ticks > self.state.tick {
            return Err(format!(
                "a founder introduced at {age_seconds} s ({age_ticks} ticks) into tick {} would \
                 have been born before this world began; introduce it no earlier than tick \
                 {age_ticks}",
                self.state.tick
            ));
        }
        let founder = self.derive_hunter_founder(&profile)?;
        let positions = targets
            .iter()
            .map(|target| {
                target.resolve().ok_or_else(|| {
                    format!("hunter founder target {target:?} is not on the surface")
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        let cap = self.state.config.capacity.max_organisms as usize;
        if self.state.organisms.len().saturating_add(positions.len()) > cap {
            return Err(format!(
                "no organism capacity for {} hunter founder(s) (population {}, capacity {cap})",
                positions.len(),
                self.state.organisms.len()
            ));
        }

        let tick = self.state.tick;
        // The world's only source of age. `age_ticks <= tick` was checked above, so this is a
        // real tick of this world and never a body born before it existed.
        let born_tick = tick - age_ticks;
        let first_draw = u64::from(self.state.hunters.founders_placed);
        let mut receipts = Vec::with_capacity(positions.len());
        for (offset, pos) in positions.into_iter().enumerate() {
            let heading = Vec2::from_screen_angle(
                unit(
                    self.state.config.seed,
                    Stream::Hunt,
                    hunter::FOUNDER_DRAW_KEY,
                    first_draw + offset as u64,
                ) * TAU,
            );
            let hunger_memory =
                (1.0 - founder.reserve / founder.phenotype.reserve_max).clamp(0.0, 1.0);
            let id = self.state.organisms.insert(Organism {
                pos,
                heading,
                ou: Vec2::ZERO,
                structure: founder.structure,
                reserve: founder.reserve,
                energy: founder.energy,
                born_tick,
                hunger_memory,
                mode: Mode::Resting,
                escrow: None,
                births: 0,
                genome: profile.genome.clone(),
                phenotype: founder.phenotype.clone(),
                parent: None,
                origin: Origin::Founder,
                turn_counter: Counter::default(),
                fed_this_tick: false,
            });
            self.state
                .hunters
                .insert_member(hunter::HunterMember::new(id, tick));
            receipts.push(HunterFounderReceipt {
                id,
                tick,
                pos,
                structure: founder.structure,
                reserve: founder.reserve,
                energy: founder.energy,
                material_in: founder.material_in,
                energy_in: founder.energy_in,
                extent: founder.phenotype.extent,
                sense_radius: founder.phenotype.sense_radius,
                geometry: hunter::ContactGeometry {
                    scale: 1.0,
                    capture_offset_body: profile.capture_offset_body,
                    capture_reach_px: profile.capture_reach_px,
                    ingestion_offset_body: profile.ingestion_offset_body,
                    visual_query_extent_px: profile.visual_query_extent_px,
                },
            });
        }
        let count = receipts.len() as u32;
        self.state.hunters.profile = Some(profile);
        self.state.hunters.founder_material_in += founder.material_in * f64::from(count);
        self.state.hunters.founder_energy_in += founder.energy_in * f64::from(count);
        self.state.hunters.founders_placed += count;
        // Adding founders enables behavior without resetting existing offspring or escrows.
        self.state.apex_dormancy.policy = ApexDormancyPolicy::UndergroundV1;
        self.state.apex_encounters.policy = ApexEncounterPolicy::PairedV1;
        debug_assert!(
            self.mass_residual().abs() < 1e-9,
            "founding moved the mass residual"
        );
        Ok(receipts)
    }

    /// The budget-matched control: deposit the **same derived founder inventory** as local
    /// detritus at `target` and place no hunter, so a paired arm can separate "a predator was
    /// added" from "this much material and energy was added".
    ///
    /// The deposit respects the per-cell `De ≤ energy_cap · D` bound; energy the cap cannot
    /// hold leaves as real heat, compensated like every other heat payment, rather than
    /// silently vanishing. Refused, without changing anything, for a world that already has a
    /// profile or a previous deposit.
    pub fn deposit_hunter_budget_control(
        &mut self,
        profile: FixedHunterProfile,
        target: HunterTarget,
    ) -> Result<HunterControlReceipt, String> {
        if self.state.quiet.active() {
            return Err(
                "this world runs an ordinary quiet policy; the hunter extension is not combined \
                 with it in this slice, because threat and escape interactions need their own \
                 explicit contract"
                    .into(),
            );
        }
        if self.state.hunters.profile.is_some() || self.state.hunters.founders_placed > 0 {
            return Err("the hunter extension is already initialized".into());
        }
        if self.state.hunters.control_deposited {
            return Err("this world already holds a budget-matched control deposit".into());
        }
        let founder = self.derive_hunter_founder(&profile)?;
        let Some(pos) = target.resolve() else {
            return Err(format!(
                "hunter control target {target:?} is not on the surface"
            ));
        };

        let cell = cell_of(&pos);
        let at = cell.index();
        let cap = self.state.config.detritus.energy_cap;
        let material = founder.material_in;
        let energy = founder.energy_in;
        // `De ≤ energy_cap · D` per cell, measured against the cell as it will be.
        let room = (cap * (self.state.fields.d[at] + material) - self.state.fields.de[at]).max(0.0);
        let stored = energy.min(room);
        self.state.fields.d[at] += material;
        self.state.fields.de[at] += stored;
        let spilled = energy - stored;
        if spilled > 0.0 {
            accounting::accumulate(
                &mut self.state.heat_out_total,
                &mut self.state.energy_correction.heat_out,
                spilled,
            );
        }
        self.state.hunters.profile = Some(profile);
        self.state.hunters.control_material_in += material;
        self.state.hunters.control_energy_in += energy;
        self.state.hunters.control_deposited = true;
        debug_assert!(
            self.mass_residual().abs() < 1e-9,
            "the control deposit moved the mass residual"
        );
        Ok(HunterControlReceipt {
            tick: self.state.tick,
            cell: cell.0,
            material_in: material,
            energy_in: energy,
            energy_stored: stored,
            energy_heat: spilled,
        })
    }

    /// The founder inventory this world's own config gives the profile, with every derived
    /// number validated before anything is placed or deposited.
    fn derive_hunter_founder(&self, profile: &FixedHunterProfile) -> Result<HunterFounder, String> {
        profile.validate()?;
        let org_cfg = &self.state.config.organism;
        if profile.body_extent_px > org_cfg.body_extent_max {
            return Err(format!(
                "hunter body extent {} exceeds this world's body_extent_max {}",
                profile.body_extent_px, org_cfg.body_extent_max
            ));
        }
        let mut phenotype = decode(&profile.genome, org_cfg);
        // The assembled body is longer than the decoded lobes: members carry the profile's
        // tested support, and no other creature's radius moves.
        phenotype.extent = profile.body_extent_px;
        let structure = phenotype.structure_adult;
        let reserve = profile.founder_reserve_fraction * phenotype.reserve_max;
        let energy = profile.founder_energy_fraction * phenotype.energy_max;
        let material_in = structure + reserve;
        let energy_in = energy + org_cfg.reserve_energy_density * reserve;
        for (name, v) in [
            ("structure", structure),
            ("reserve", reserve),
            ("energy", energy),
            ("material", material_in),
            ("energy_in", energy_in),
        ] {
            if !v.is_finite() || v < 0.0 {
                return Err(format!("derived hunter founder {name} = {v}"));
            }
        }
        if structure < org_cfg.min_structure {
            return Err(format!(
                "derived hunter founder structure {structure} would collapse at once"
            ));
        }
        Ok(HunterFounder {
            phenotype,
            structure,
            reserve,
            energy,
            material_in,
            energy_in,
        })
    }

    /// One hunter per live member, keyed by full ID, with the transported jaw anchor contact
    /// is actually tested at. Empty for every world without hunters, and published separately
    /// from [`World::render_view`] so the existing view structs keep their fields.
    pub fn hunter_view(&self) -> Vec<HunterView> {
        let Some(profile) = self.state.hunters.profile() else {
            return Vec::new();
        };
        let gestation_ticks = ticks_from_seconds(profile.gestation_seconds, DT);
        let tick = self.state.tick;
        self.state
            .hunters
            .members
            .iter()
            .filter_map(|m| {
                if self.state.apex_dormancy.contains(m.id) {
                    return None;
                }
                let o = self.state.organisms.get(m.id)?;
                let geometry = hunter::ContactGeometry::of(profile, o);
                Some(HunterView {
                    id: m.id,
                    role: profile.role,
                    phase: m.phase,
                    phase_progress: m.progress(tick),
                    phase_started_tick: m.phase_started_tick,
                    phase_ends_tick: m.phase_ends_tick,
                    entered_from: m.entered_from,
                    episode: m.episode,
                    attack_counter: m.attack_counter,
                    pos: o.pos,
                    heading: o.heading,
                    geometry,
                    body_scale: geometry.scale,
                    capture_center: hunter::body_point(
                        &self.images,
                        o.pos,
                        o.heading,
                        geometry.capture_offset_body,
                    ),
                    ingestion_center: hunter::body_point(
                        &self.images,
                        o.pos,
                        o.heading,
                        geometry.ingestion_offset_body,
                    ),
                    target: m.target.filter(|t| self.state.organisms.get(*t).is_some()),
                    structure: o.structure,
                    structure_adult: o.phenotype.structure_adult,
                    extent: o.phenotype.extent,
                    juvenile: o.structure < 0.7 * o.phenotype.structure_adult,
                    gut_material: m.gut_material,
                    gut_energy: m.gut_energy,
                    gut_fraction: (m.gut_material / profile.gut_capacity_material).clamp(0.0, 1.0)
                        as f32,
                    gut_capacity: profile.gut_capacity_material,
                    gestation: o.escrow.as_ref().map(|e| {
                        if gestation_ticks == 0 {
                            1.0
                        } else {
                            (tick.saturating_sub(e.started_tick) as f64 / gestation_ticks as f64)
                                .clamp(0.0, 1.0) as f32
                        }
                    }),
                })
            })
            .collect()
    }

    /// Take the hunter records committed since the last drain, in commit order. Transient:
    /// nothing in the world reads them back, and a host that never drains simply lets the
    /// buffer grow. Each consumed prey also produced one ordinary `LifeEvent::Death` with
    /// [`DeathCause::Predation`].
    pub fn drain_hunter_events(&mut self) -> Vec<HunterEvent> {
        std::mem::take(&mut self.hunter_events)
    }

    /// Underground entry, emergence and exhaustion records since the last drain.
    pub fn drain_apex_dormancy_events(&mut self) -> Vec<ApexDormancyEvent> {
        std::mem::take(&mut self.apex_dormancy_events)
    }

    /// The persisted opt-in policy, dormant identities and accounting totals.
    pub fn apex_dormancy(&self) -> &ApexDormancyState {
        &self.state.apex_dormancy
    }

    /// Paid pairing, birth/refund/loss, and combat records since the last drain.
    pub fn drain_apex_encounter_events(&mut self) -> Vec<ApexEncounterEvent> {
        std::mem::take(&mut self.apex_encounter_events)
    }

    /// The independently opt-in adult apex encounter state.
    pub fn apex_encounters(&self) -> &ApexEncounterState {
        &self.state.apex_encounters
    }

    /// Ordinary-quiet begin/refuse/end/abort records since the last drain (`crate::quiet`).
    ///
    /// Transient measurement output for a harness: never checkpointed, never hashed, never read
    /// back by the tick, and never routed to the ambient display. Always empty in an Off world.
    pub fn drain_quiet_events(&mut self) -> Vec<QuietEvent> {
        std::mem::take(&mut self.quiet_events)
    }

    /// The opt-in ordinary quiet extension: policy and any held pauses (`crate::quiet`).
    pub fn quiet(&self) -> &QuietState {
        &self.state.quiet
    }
}
