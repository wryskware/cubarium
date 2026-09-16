use cubarium_surface::Topology;
use serde::{Deserialize, Serialize};

use cubarium_surface::CUBE_CELL_COUNT;

use crate::accounting::{EnergyCorrection, EnergyLedgers, Ledger};
use crate::care::CareState;
use crate::config::WorldConfig;
use crate::dormancy::ApexDormancyState;
use crate::encounter::ApexEncounterState;
use crate::fields::{EcologyV1State, Fields};
use crate::genome::Phenotype;
use crate::genome::{Genome, MAX_FORMS};
use crate::habitat::Weather;
use crate::hunter::{FixedHunterProfile, HunterState};
use crate::ids::Slots;
use crate::organism::Organism;
use crate::neural::NeuralState;
use crate::quiet::QuietState;

use super::*;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct WorldState {
    pub config: WorldConfig,
    pub tick: u64,
    pub fields: Fields,
    pub weather: Weather,
    pub organisms: Slots<Organism>,
    /// Cumulative counters since world creation.
    pub births_total: u64,
    pub deaths_total: [u64; 3],
    pub cap_rejections_total: u64,
    /// Material admitted from outside (founders and any future stimuli), for the invariant.
    pub external_material_in: f64,
    /// Running energy audit.
    pub light_in_total: f64,
    pub heat_out_total: f64,
    /// Running water budget (`design/water.md`): `Σw == rain_in_total − evap_out_total`
    /// to rounding at every tick of a world created dry.
    #[serde(default)]
    pub rain_in_total: f64,
    #[serde(default)]
    pub evap_out_total: f64,
    /// Optional care (`design/7_Research/care-contract-2026-09-12.md`): the sequence cursor,
    /// any shower in progress, and the six ledgers the mass, energy and water identities
    /// carry. Schema 8 is schema 7 plus exactly this field, which is what makes
    /// [`crate::snapshot::WorldStateV7`] a byte-exact projection.
    #[serde(default)]
    pub care: CareState,
    /// Persisted signed Neumaier corrections for `light_in_total` and `heat_out_total`
    /// (`crate::accounting`, `design/7_Research/accounting-compensation-handoff-2026-09-13.md`).
    /// The raw counters above are written exactly as before; the corrected cumulative total
    /// of either ledger is `raw + correction`, read through [`WorldState::energy_ledgers`].
    ///
    /// **Appended last**: schema 9 is schema 8 plus exactly this field, which is what makes
    /// [`crate::snapshot::WorldStateV8`] a byte-exact projection. Zero for a world migrated
    /// from schema 7 or 8: the correction begins at migration and claims no repair of
    /// rounding already lost.
    #[serde(default)]
    pub energy_correction: EnergyCorrection,
    /// The opt-in paid hunter extension (`crate::hunter`): the trial profile, its members and
    /// their carried guts, the material and energy it imported, and its own counters.
    ///
    /// **Appended last**: schema 10 is schema 9 plus exactly this field, which is what makes
    /// [`crate::snapshot::WorldStateV9`] a byte-exact projection. Empty and inert for a world
    /// migrated from schema 9, 8 or 7 — a migration never introduces a predator — and empty
    /// for every world that never opts in.
    #[serde(default)]
    pub hunters: HunterState,
    /// The opt-in ordinary quiet extension (`crate::quiet`). Appended in schema 13, outside
    /// every legacy nested organism and config payload. Off with no entries by default, and an
    /// Off world never enters a quiet code path.
    #[serde(default)]
    pub quiet: QuietState,
    /// Opt-in underground dormancy for paid apex offspring. Appended in schema 14 so every
    /// earlier organism, hunter and quiet payload retains its exact wire shape.
    #[serde(default)]
    pub apex_dormancy: ApexDormancyState,
    /// Independently opt-in adult apex pairing and combat. Schema 14 is still unreleased and
    /// carries both milestone extensions; schema 13 migrates each one Off.
    #[serde(default)]
    pub apex_encounters: ApexEncounterState,
    /// The opt-in recurrent-policy extension (`crate::neural`). Appended in schema 15, after
    /// every earlier extension, so [`crate::snapshot::WorldStateV14`] is a byte-exact prefix.
    ///
    /// Empty by default and empty for every world migrated from schema 14 or earlier: **an
    /// existing world therefore loads with every organism legacy-controlled**, and an animal
    /// is neural only because something explicitly inserted an entry for it.
    #[serde(default)]
    pub neural: NeuralState,
    /// Ecology v1's pools and counters (`crate::fields::EcologyV1State`,
    /// `design/ecology-v1-contract.md` §3, §14). Appended **last**, after every earlier
    /// extension, which is what makes schema 15's payload a byte-exact prefix of schema 16's.
    ///
    /// No `#[serde(default)]`, and no migration: schema 16 refuses every older snapshot by
    /// name (§15.1), so a world either carries these pools or does not load at all. Nothing
    /// is ever synthesised for a world that never had wood.
    pub ecology: EcologyV1State,
}

impl WorldState {
    /// Range checks after decode: finite numbers, nonnegative stocks, canonical positions,
    /// unit headings (renormalize if within 1e-6, else invalid), population ≤ cap, escrows
    /// nonnegative, genome fields in range, config valid.
    ///
    /// `validate` only reports; [`World::from_state`] renormalizes headings that are within
    /// tolerance before calling it, so a heading that still fails here is genuinely invalid.
    pub fn validate(&self) -> Result<(), String> {
        self.config.validate()?;
        let cap = self.config.capacity.max_organisms as usize;
        if self.organisms.len() > cap {
            return Err(format!(
                "population {} exceeds cap {cap}",
                self.organisms.len()
            ));
        }
        for (name, v) in [
            ("external_material_in", self.external_material_in),
            ("light_in_total", self.light_in_total),
            ("heat_out_total", self.heat_out_total),
        ] {
            if !v.is_finite() {
                return Err(format!("{name} is not finite"));
            }
        }
        for (name, v) in [
            ("n", &self.fields.n),
            ("p", &self.fields.p),
            ("d", &self.fields.d),
            ("de", &self.fields.de),
        ] {
            if v.len() != CUBE_CELL_COUNT {
                return Err(format!(
                    "field {name} has {} cells, expected {CUBE_CELL_COUNT}",
                    v.len()
                ));
            }
        }
        self.fields.check(self.config.detritus.energy_cap)?;
        self.ecology.check(self.config.detritus.carrion_energy_cap)?;
        self.care.validate(self.tick)?;
        self.hunters
            .validate(self.tick, &self.organisms, &self.config)?;
        self.apex_dormancy
            .validate(self.tick, cap, &self.organisms, &self.hunters)?;
        self.apex_encounters
            .validate(self.tick, cap, &self.organisms, &self.hunters)?;
        // The ordinary quiet extension, against this world's own clock, capacity and live
        // organisms — and against the hunter extension, which this slice refuses to combine
        // with an enabled policy (`crate::quiet`).
        self.quiet.validate(
            self.tick,
            self.config.capacity.max_organisms as usize,
            self.hunters.profile.is_some()
                || self.hunters.founders_placed > 0
                || self.hunters.control_deposited
                || !self.hunters.members.is_empty(),
            |id| self.organisms.get(id).is_some(),
        )?;
        self.neural.validate()?;
        for (id, _) in &self.neural.animals {
            if self.organisms.get(*id).is_none() {
                return Err(format!(
                    "neural animal {}:{} has no organism",
                    id.slot, id.generation
                ));
            }
            // Two combinations have no contract yet and are refused rather than silently
            // half-applied. A neural animal ignores the quiet pause and every legacy
            // behavioural override, so a world that also runs one is not describable; and the
            // apex sensory/action extensions (gut, handling, strike readiness, attack
            // semantics) do not exist, so a member cannot speak this interface at all.
            if self.hunters.contains(*id) {
                return Err(format!(
                    "neural animal {}:{} is an apex member: the apex sensory and action \
                     extensions do not exist in this interface version",
                    id.slot, id.generation
                ));
            }
        }
        if !self.neural.animals.is_empty() && self.quiet.policy.enabled() {
            return Err(
                "the ordinary quiet extension and neural animals cannot be enabled together: \
                 a neural animal does not observe the post-birth pause"
                    .into(),
            );
        }
        // The corrections are signed, so they are checked as the *combined* totals they are
        // part of: finite corrections, and finite nonnegative corrected cumulative flows.
        // Nothing is clamped or reset — an unusable accounting state fails the load.
        self.energy_ledgers().validate()?;

        for blob in self
            .weather
            .light
            .iter()
            .chain(self.weather.moisture.iter())
        {
            if !blob
                .center
                .iter()
                .chain(blob.axis.iter())
                .all(|c| c.is_finite())
                || !blob.rate.is_finite()
            {
                return Err("weather blob has non-finite geometry".into());
            }
        }

        for (id, o) in self.organisms.iter() {
            let who = format!("organism {}:{}", id.slot, id.generation);
            for (name, v) in [("structure", o.structure), ("reserve", o.reserve)] {
                if !v.is_finite() || v < 0.0 {
                    return Err(format!("{who}: {name} = {v}"));
                }
            }
            if !o.energy.is_finite() || o.energy < 0.0 {
                return Err(format!("{who}: energy = {}", o.energy));
            }
            if !o.hunger_memory.is_finite() {
                return Err(format!("{who}: hunger memory is not finite"));
            }
            if !o.pos.is_canonical(Topology::Cube) {
                return Err(format!("{who}: position {:?} is not canonical", o.pos));
            }
            if !o.heading.is_finite() || (o.heading.length() - 1.0).abs() > HEADING_TOLERANCE {
                return Err(format!(
                    "{who}: heading {:?} is not a unit vector",
                    o.heading
                ));
            }
            if !o.ou.is_finite() {
                return Err(format!("{who}: OU vector is not finite"));
            }
            if o.born_tick > self.tick {
                return Err(format!(
                    "{who}: born at {} after tick {}",
                    o.born_tick, self.tick
                ));
            }
            if let Some(e) = &o.escrow {
                for (name, v) in [
                    ("structure", e.structure),
                    ("reserve", e.reserve),
                    ("energy", e.energy),
                ] {
                    if !v.is_finite() || v < 0.0 {
                        return Err(format!("{who}: escrow {name} = {v}"));
                    }
                }
                if e.started_tick > self.tick {
                    return Err(format!(
                        "{who}: escrow started at {} after tick {}",
                        e.started_tick, self.tick
                    ));
                }
                check_genome(&e.genome, &format!("{who} escrow"))?;
            }
            check_genome(&o.genome, &who)?;
        }
        Ok(())
    }

    /// Both compensated cumulative energy ledgers: the raw counters paired with their
    /// persisted corrections (`crate::accounting`).
    ///
    /// This is the accurate cumulative representation. `light_in_total` and `heat_out_total`
    /// on their own remain exactly what every earlier build wrote and are kept as explicit
    /// diagnostics and wire values, not as fully accurate totals.
    pub fn energy_ledgers(&self) -> EnergyLedgers {
        EnergyLedgers {
            light_in: Ledger {
                raw: self.light_in_total,
                correction: self.energy_correction.light_in,
            },
            heat_out: Ledger {
                raw: self.heat_out_total,
                correction: self.energy_correction.heat_out,
            },
        }
    }

    /// Corrected cumulative light admitted, `light_in_total + correction`.
    pub fn light_in_corrected(&self) -> f64 {
        self.energy_ledgers().light_in.total()
    }

    /// Corrected cumulative heat dissipated, `heat_out_total + correction`.
    pub fn heat_out_corrected(&self) -> f64 {
        self.energy_ledgers().heat_out.total()
    }

    /// Corrected cumulative `light_in − heat_out`. For an interval, hold an
    /// [`EnergyLedgers`] reading and use [`EnergyLedgers::net_since`] instead: differencing
    /// two corrected totals rounds the interval's flow away again.
    pub fn net_energy_in_corrected(&self) -> f64 {
        self.energy_ledgers().net()
    }
}

/// How far a stored heading may drift from unit length before it is invalid.

pub(super) struct HunterFounder {
    pub(super) phenotype: Phenotype,
    pub(super) structure: f64,
    pub(super) reserve: f64,
    pub(super) energy: f64,
    pub(super) material_in: f64,
    pub(super) energy_in: f64,
}

pub(super) fn strike_closing_px(profile: &FixedHunterProfile) -> f64 {
    profile.strike_speed_px_s * profile.strike_seconds
}

/// Bounds from `design/m2-world-spec.md` "Organism representation" and `genome`'s docs.
pub(crate) fn check_genome(g: &Genome, who: &str) -> Result<(), String> {
    let range = |name: &str, v: f32, lo: f32, hi: f32| -> Result<(), String> {
        if !v.is_finite() || v < lo || v > hi {
            Err(format!("{who}: genome {name} = {v}, expected [{lo}, {hi}]"))
        } else {
            Ok(())
        }
    };
    if g.version != Genome::VERSION {
        return Err(format!(
            "{who}: genome version {} is not {}",
            g.version,
            Genome::VERSION
        ));
    }
    range("size", g.size, 0.5, 2.0)?;
    range("metabolism", g.metabolism, 0.5, 2.0)?;
    range("sense", g.sense, 2.0, 12.0)?;
    range("reserve", g.reserve, 0.5, 2.0)?;
    range("mouth", g.mouth, 0.2, 1.0)?;
    range("speed", g.speed, 0.3, 1.0)?;
    range("hue", g.hue, 0.0, 1.0)?;
    range("diet", g.diet, 0.0, 1.0)?;
    range("depth", g.depth, 0.0, 1.0)?;
    range("swim", g.swim, 0.0, 1.0)?;
    if g.form >= MAX_FORMS {
        return Err(format!(
            "{who}: genome form {} is not below {MAX_FORMS}",
            g.form
        ));
    }
    let d = &g.drives;
    for (name, v) in [
        ("w_food", d.w_food),
        ("w_detritus", d.w_detritus),
        ("w_persist", d.w_persist),
        ("w_crowd", d.w_crowd),
        ("w_depth", d.w_depth),
        ("turn_noise", d.turn_noise),
    ] {
        range(name, v, 0.0, 2.0)?;
    }
    for (name, v) in [
        ("seek_on", d.seek_on),
        ("seek_off", d.seek_off),
        ("feed_min", d.feed_min),
        ("rest_effort", d.rest_effort),
        ("feed_effort", d.feed_effort),
        ("bud_reserve", d.bud_reserve),
        ("bud_energy", d.bud_energy),
    ] {
        range(name, v, 0.0, 1.0)?;
    }
    if d.seek_off >= d.seek_on {
        return Err(format!(
            "{who}: seek_off {} is not below seek_on {}",
            d.seek_off, d.seek_on
        ));
    }
    range("tau_hunger_seconds", d.tau_hunger_seconds, 1.0, 60.0)?;
    for (name, v) in [
        ("bud_min_age_seconds", d.bud_min_age_seconds),
        ("turn_rate_max_deg", d.turn_rate_max_deg),
    ] {
        if !v.is_finite() || v < 0.0 {
            return Err(format!("{who}: genome {name} = {v}"));
        }
    }
    Ok(())
}

/// Per-tick counters exposed to telemetry and reset each sample.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TickCounters {
    pub births: u32,
    pub deaths: [u32; 3],
    pub cap_rejections: u32,
    pub light_in: f64,
    pub heat_out: f64,
    pub rain_in: f64,
    pub evap_out: f64,
    pub travel_fallbacks: u32,
    pub travel_ties: u32,
    /// Paid hunter attempts, successful captures and prey consumed this sample
    /// (`crate::hunter`). Zero in every world that never opted in.
    pub hunter_attacks: u32,
    pub hunter_captures: u32,
    pub deaths_predation: u32,
}

/// Read-only stock/flow diagnostics for the R0a food measurement
/// (`design/handoffs/r0a-movement-foundation-2026-09-14.md`, part B): what the world's
/// producers actually made and what its mouths actually took, as opposed to what a reserve
/// delta or a `Feeding` label suggests.
///
/// **Scope.** Totals over every cell and every organism, in material units, accumulated inside
/// the passes that already compute them. Intake is the material that actually left a field,
/// after the per-cell proportional share and after every clamp — never a request, never an
/// assimilated fraction, never a reserve difference.
///
/// **Time.** From the moment this `World` value was constructed to now, exactly as
/// [`ChargingDiagnostics`] documents. Transient: never persisted, never hashed, never read
/// back by the tick, and zero again after a reload.
///
/// **Cost.** Seven running scalars written inside branches the step already takes. No per-tick
/// history, no RNG, nothing read or written that the step did not already touch, so recording
/// this cannot move the simulation.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct IntakeDiagnostics {
    /// Gross producer material grown, before mortality, ripening or grazing.
    pub producer_growth: f64,
    /// Material that actually left `P`, `F`, `D` and `C` through a mouth: the **served
    /// bite**, before the digestible share, the assimilated share or the feces
    /// (`design/ecology-v1-contract.md` §6.4). Litter and remains are recorded apart, so a
    /// scenario knows which stock a detrital digester actually lived on.
    pub producer_eaten: f64,
    pub fruit_eaten: f64,
    pub litter_eaten: f64,
    pub carrion_eaten: f64,
    /// Feces: the material a mouth returned to `D` because its machinery could not digest
    /// it, `(1 − η_m′)·cap·q + (1 − cap)·q` summed over served bites.
    pub undigested: f64,
    /// `Σ A` (§4.2), `Σ unpaid` (§4.3) and `Σ s` (§4.8) over the run: what the plants
    /// actually took out of `N`, what their maintenance could not cover, and what donors
    /// spent on propagules.
    pub plant_income: f64,
    pub plant_maintenance_unpaid: f64,
    pub propagule_sent: f64,

    /// **The complete animal energy bill of the run**, in `e`, accumulated inside the pass
    /// that charges it — so it is what the world actually booked, not a reconstruction.
    ///
    /// `body_bill_total` is what every body **owed**: `MotorBill::total_cost`, which is
    /// `(maintenance · S + move_cost · S · billed_motion(speed, sweep) + sense_cost ·
    /// r_sense) · dt` — maintenance, sensing and **both** halves of the motor bill, the
    /// translation and the rotational sweep. `body_bill_paid` is what it **actually paid**,
    /// after the clamp to the energy it could raise; the two differ only for a body that could
    /// not cover its own upkeep, which is a body that dies this tick. `body_bill_upkeep` is
    /// the mandatory half alone (`MotorBill::upkeep`), so the motor half is the difference, to
    /// floating-point association.
    ///
    /// **Every body that was billed is in here, including one removed later in the same
    /// tick** — a starving body, a captured prey, a miscarrying parent — because the charge is
    /// recorded where it is levied, in step 6, and removals commit in step 9. A concealed apex
    /// offspring's dormancy upkeep is counted too, at its own site.
    ///
    /// Transient like every other field here: never persisted, never hashed, never read back
    /// by the tick, and zero again after a reload. Recording it changes no bill.
    pub body_bill_total: f64,
    pub body_bill_paid: f64,
    pub body_bill_upkeep: f64,
    /// What the mouths asked their cells for, before the proportional share. Larger than the
    /// three totals above exactly when a cell could not serve everyone standing in it, which
    /// is what makes competition visible rather than inferred.
    pub requested: f64,
    /// Organism-ticks that asked for food, and the subset that got none of what they asked
    /// for because their reserve was already full.
    pub request_ticks: u64,
    pub reserve_saturated_ticks: u64,
}

/// Transient per-run timing of the recurrent stage (`crate::neural`): the sampler, the GRU
/// forward pass and the action adapter, each measured at its own call rather than inferred
/// from the difference between a legacy world and a neural one — two worlds that also differ
/// in their trajectories, their controllers and the work those imply.
///
/// Never persisted, never hashed, never read by the simulation, and only written inside the
/// neural branch, which a world with no neural animal never enters.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct NeuralTiming {
    /// Wall nanoseconds inside the 70-scalar sampler, and the number of times it ran.
    pub sampler_nanos: u64,
    pub sampler_calls: u64,
    /// Wall nanoseconds inside `Gru32::forward`, and the number of forward passes.
    pub inference_nanos: u64,
    pub inference_calls: u64,
    /// Wall nanoseconds inside the action adapter: squash, deadband, masks, mouth
    /// normalisation and this tick's `MotorRequest`. Runs on **every** tick, not only
    /// controller ticks, because a held action is rebuilt from the current heading each tick.
    pub adapter_nanos: u64,
    pub adapter_calls: u64,
}

impl NeuralTiming {
    /// The difference between two readings, for a probe that brackets a run.
    pub fn since(&self, earlier: &NeuralTiming) -> NeuralTiming {
        NeuralTiming {
            sampler_nanos: self.sampler_nanos.saturating_sub(earlier.sampler_nanos),
            sampler_calls: self.sampler_calls.saturating_sub(earlier.sampler_calls),
            inference_nanos: self.inference_nanos.saturating_sub(earlier.inference_nanos),
            inference_calls: self.inference_calls.saturating_sub(earlier.inference_calls),
            adapter_nanos: self.adapter_nanos.saturating_sub(earlier.adapter_nanos),
            adapter_calls: self.adapter_calls.saturating_sub(earlier.adapter_calls),
        }
    }
}

/// Read-only diagnostics for the paid-charging experiment: what the member oxidation policy
/// did that the world's own configured threshold would **not** have done
/// (`design/7_Research/astra-hunter-paid-charging-proposal-2026-09-13.md`).
///
/// **Scope.** A transaction is counted here when, and only when, an authoritative hunter
/// member's oxidation ran at a resolved threshold *above* the world's configured one **and**
/// its energy was at or above the configured threshold — that is, the exact transactions a
/// [`crate::hunter::OxidationPolicy::Configured`] member would not have performed in the same
/// world at the same instant. Oxidation below the configured threshold is ordinary physiology
/// and is not counted, by either policy. Ordinary organisms are never counted.
///
/// **Time.** Totals run from the moment this `World` value was constructed — by
/// [`World::new`] or [`World::from_state`] — to now. They are **transient**: never persisted,
/// never hashed, never read back by the tick. A reloaded world starts them at zero, so a
/// reader must treat them as a property of one process's run, not of the world's history.
///
/// **Cost.** Four running scalars and a counter, written only inside a branch the step already
/// takes. No per-tick history is accumulated, no RNG is consumed and no world state is read or
/// written that the step did not already read or write, so recording this cannot move the
/// simulation.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ChargingDiagnostics {
    /// Oxidation transactions performed only because the member's threshold was raised.
    pub extra_transactions: u64,
    /// Reserve material burned in those transactions (m). It went to the local `N` cell, as
    /// every other oxidation's does.
    pub extra_reserve_burned: f64,
    /// Battery charge gained in them (e), after efficiency and the headroom cap.
    pub extra_energy_gained: f64,
    /// Conversion heat released by them (e): `e_r · burned − gained`, as always.
    pub extra_heat: f64,
}
