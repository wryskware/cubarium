//! Read-only audit of the hunter reproduction transactions
//! (`design/7_Research/hunter-experiment-contract-2026-09-13.md` section 6).
//!
//! Observer only: it never mutates a `World`, never draws, and never relaxes a bound. It reads
//! the transaction records core emits at its own mutation sites (`HunterEvent::Reproduction`,
//! `cubarium_core::hunter::Reproduction`), the ordinary life events of the same tick, and the
//! completed state, and it refuses anything that does not reconcile.
//!
//! **What is independent here and what is not.** The *quantities* are core's: they are read at
//! the assignment that moved them, and no observer outside the tick can measure them, because
//! oxidation, growth, movement and a death can all touch the same parent in the same tick. What
//! this audit contributes is independent *recomputation of the identities those quantities must
//! satisfy* — from the world's own config and the parent's own decoded phenotype — plus
//! reconciliation against facts it can see for itself: the escrows actually held after the
//! step, the offspring and birth records of the same tick, and the parent's death and cause.
//! A number that only ever appears in one event field is reported as core's evidence, never as
//! an independently measured transfer.
//!
//! The bookkeeping is bounded: one open escrow per living parent, a size cache pruned to the
//! living lineage, and scalar counters. No event history is retained.

use anyhow::{Result, bail, ensure};
use cubarium_core::hunter::{FundingBlocked, Reproduction};
use cubarium_core::organism::DeathCause;
use cubarium_core::{HunterEvent, LifeEvent, OrganismId, WorldState};
use serde::Serialize;
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};

/// Absolute slack for an identity recomputed from the same `f64` inputs in a different
/// association order. Sums of two or three terms at these magnitudes differ by at most a few
/// ulps; anything larger is a real disagreement, not rounding.
const EPS: f64 = 1e-12;

/// `a == b` to within [`EPS`] scaled by the magnitude involved.
fn close(a: f64, b: f64) -> bool {
    let scale = a.abs().max(b.abs()).max(1.0);
    (a - b).abs() <= EPS * scale
}

fn finite_nonnegative(name: &str, v: f64) -> Result<()> {
    ensure!(v.is_finite(), "{name} is not finite: {v}");
    ensure!(v >= 0.0, "{name} is negative: {v}");
    Ok(())
}

fn id_json(id: OrganismId) -> Value {
    json!({ "slot": id.slot, "generation": id.generation })
}

/// One gestation this audit is carrying: what core said it bought, and when.
#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
struct Open {
    started_tick: u64,
    funded_tick: u64,
    structure: f64,
    reserve: f64,
    energy: f64,
}

/// The decoded sizes a funding's fractions are checked against. Cached per living member so a
/// parent that funds and dies inside one tick can still be validated.
#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
struct Sizes {
    structure_adult: f64,
    reserve_max: f64,
    energy_max: f64,
}

/// Counts and aggregates, by transaction. Every quantity is core's reported one; the audit adds
/// only the arithmetic that checks it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize)]
struct Totals {
    funded: u64,
    born: u64,
    refunded: u64,
    miscarried: u64,
    not_funded_cap: u64,
    not_funded_stocks: u64,
    /// `escrow S + R` summed over fundings, and the energy the parent gave up with it.
    funded_material: f64,
    funded_energy: f64,
    /// `build_cost · S` at funding, and `e_r · S` at birth.
    build_heat: f64,
    birth_heat: f64,
    born_material: f64,
    born_energy: f64,
    refunded_material: f64,
    refunded_energy: f64,
    /// The escrow terms exported to the litter, and how the detritus cap split them.
    miscarried_material: f64,
    miscarried_energy: f64,
    miscarried_stored: f64,
    miscarried_heat: f64,
}

/// The configured quantities a transaction is checked against, read once at the opening.
#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
struct Rates {
    reserve_energy_density: f64,
    build_cost: f64,
    child_structure_fraction: f64,
    child_reserve_fraction: f64,
    child_energy_fraction: f64,
    detritus_energy_cap: f64,
}

impl Rates {
    fn read(s: &WorldState) -> Self {
        let o = &s.config.organism;
        Self {
            reserve_energy_density: o.reserve_energy_density,
            build_cost: o.build_cost,
            child_structure_fraction: o.child_structure_fraction,
            child_reserve_fraction: o.child_reserve_fraction,
            child_energy_fraction: o.child_energy_fraction,
            detritus_energy_cap: s.config.detritus.energy_cap,
        }
    }
}

/// The identity records of one tick, indexed once so each transaction can be reconciled
/// against them: which child each offspring record names, which parent each birth names, and
/// what each death was caused by.
#[derive(Debug, Default)]
struct TickIndex {
    offspring: BTreeMap<OrganismId, OrganismId>,
    births: BTreeMap<OrganismId, OrganismId>,
    deaths: BTreeMap<OrganismId, DeathCause>,
}

impl TickIndex {
    fn read(events: &[HunterEvent], life: &[LifeEvent]) -> Result<Self> {
        let mut index = TickIndex::default();
        for event in events {
            if let HunterEvent::Offspring { parent, child, .. } = event {
                ensure!(
                    index.offspring.insert(*child, *parent).is_none(),
                    "duplicate offspring record for {child:?}"
                );
            }
        }
        for event in life {
            match event {
                LifeEvent::Birth { id, parent, .. } => {
                    ensure!(index.births.insert(*id, *parent).is_none(), "duplicate birth event for {id:?}");
                }
                LifeEvent::Death { id, cause, .. } => {
                    ensure!(index.deaths.insert(*id, *cause).is_none(), "duplicate death event for {id:?}");
                }
            }
        }
        Ok(index)
    }
}

/// The audit itself. One per arm, attached at a comparison opening and fed every completed tick.
#[derive(Debug)]
pub struct ReproductionAudit {
    rates: Rates,
    opening_tick: u64,
    last_tick: u64,
    ticks: u64,
    /// At most one outstanding gestation per parent, by full ID.
    open: BTreeMap<OrganismId, Open>,
    /// Decoded sizes of the living lineage, pruned every tick to members and open parents.
    sizes: BTreeMap<OrganismId, Sizes>,
    totals: Totals,
}

impl ReproductionAudit {
    /// Attach at an opening where **no member holds an escrow**.
    ///
    /// A mid-gestation attachment is refused rather than guessed: the debits that funded an
    /// escrow already in flight happened before this audit existed, and no post-step state can
    /// recover them. Ordinary non-member organisms may gestate freely; this audit is about the
    /// hunter lineage.
    pub fn new(s: &WorldState) -> Result<Self> {
        let mut sizes = BTreeMap::new();
        for member in &s.hunters.members {
            let o = s
                .organisms
                .get(member.id)
                .ok_or_else(|| anyhow::anyhow!("hunter member {:?} is not a live organism", member.id))?;
            ensure!(
                o.escrow.is_none(),
                "this opening is mid-gestation: hunter {:?} already holds an escrow started at \
                 tick {}, whose funding debits happened before the audit and cannot be recovered \
                 from post-step state",
                member.id,
                o.escrow.as_ref().map_or(0, |e| e.started_tick)
            );
            sizes.insert(
                member.id,
                Sizes {
                    structure_adult: o.phenotype.structure_adult,
                    reserve_max: o.phenotype.reserve_max,
                    energy_max: o.phenotype.energy_max,
                },
            );
        }
        Ok(Self {
            rates: Rates::read(s),
            opening_tick: s.tick,
            last_tick: s.tick,
            ticks: 0,
            open: BTreeMap::new(),
            sizes,
            totals: Totals::default(),
        })
    }

    /// Observe one **completed** tick: the records drained after that step, the ordinary life
    /// events of the same step, and the state the step produced.
    ///
    /// Ticks must arrive strictly consecutively. Nothing is committed until the whole tick
    /// validates, so a rejected tick leaves the audit exactly as it was.
    pub fn observe(&mut self, events: &[HunterEvent], life: &[LifeEvent], state: &WorldState) -> Result<()> {
        ensure!(
            state.tick == self.last_tick + 1,
            "reproduction audit needs consecutive ticks: expected {}, got {}",
            self.last_tick + 1,
            state.tick
        );
        ensure!(
            Rates::read(state) == self.rates,
            "the world's reproduction rates changed under the audit at tick {}",
            state.tick
        );
        let tick = state.tick;

        // Scratch copies: the tick is committed only if all of it validates.
        let mut open = self.open.clone();
        let mut sizes = self.sizes.clone();
        let mut totals = self.totals;

        // Every record in this batch belongs to this tick.
        for event in events {
            ensure!(event.tick() == tick, "hunter record {event:?} is not from tick {tick}");
        }
        for event in life {
            ensure!(event.tick() == tick, "life event {event:?} is not from tick {tick}");
        }

        // The identity records this tick's transactions must reconcile against.
        let index = TickIndex::read(events, life)?;

        // The transactions, in the order core committed them. A funding and its loss can share a
        // tick, so a closure may refer to a key opened earlier in this very batch.
        let mut born_here: BTreeSet<OrganismId> = BTreeSet::new();
        for event in events {
            let HunterEvent::Reproduction { hunter, record, .. } = event else { continue };
            ensure!(
                *hunter == record.parent(),
                "record {record:?} is wrapped for hunter {hunter:?} but names parent {:?}",
                record.parent()
            );
            match *record {
                Reproduction::Funded { .. } => {
                    self.funded(&mut open, &sizes, &mut totals, tick, *record, state)?
                }
                Reproduction::Born { .. } => {
                    let child = self.born(&mut open, &mut totals, tick, *record, state, &index)?;
                    ensure!(born_here.insert(child), "two births reported for child {child:?}");
                }
                Reproduction::Refunded { .. } => self.refunded(&mut open, &mut totals, *record)?,
                Reproduction::Miscarried { .. } => {
                    self.miscarried(&mut open, &mut totals, *record, &index)?
                }
                Reproduction::NotFunded { parent, reason } => {
                    ensure!(
                        !open.contains_key(&parent),
                        "{parent:?} reported a blocked funding while already gestating"
                    );
                    match reason {
                        FundingBlocked::Cap => totals.not_funded_cap += 1,
                        FundingBlocked::Stocks => totals.not_funded_stocks += 1,
                    }
                }
            }
        }

        // Every offspring record of this tick is a reported birth transaction, and vice versa.
        for (child, parent) in &index.offspring {
            ensure!(
                born_here.contains(child),
                "offspring {child:?} of {parent:?} has no reported birth transaction"
            );
        }
        // Every ordinary birth whose parent belongs to the lineage is one too.
        for (child, parent) in &index.births {
            let lineage = state.hunters.contains(*parent) || self.sizes.contains_key(parent);
            if lineage {
                ensure!(
                    born_here.contains(child),
                    "hunter {parent:?} bore {child:?} with no reported birth transaction"
                );
            }
        }

        // What the audit believes is outstanding must be exactly what the world is holding.
        self.reconcile_open(&open, state)?;

        // Bounded caches: the living lineage and whoever still owes a gestation.
        sizes.retain(|id, _| state.hunters.contains(*id) || open.contains_key(id));
        for member in &state.hunters.members {
            if let Some(o) = state.organisms.get(member.id) {
                sizes.insert(
                    member.id,
                    Sizes {
                        structure_adult: o.phenotype.structure_adult,
                        reserve_max: o.phenotype.reserve_max,
                        energy_max: o.phenotype.energy_max,
                    },
                );
            }
        }

        self.open = open;
        self.sizes = sizes;
        self.totals = totals;
        self.last_tick = tick;
        self.ticks += 1;
        Ok(())
    }

    fn funded(
        &self,
        open: &mut BTreeMap<OrganismId, Open>,
        sizes: &BTreeMap<OrganismId, Sizes>,
        totals: &mut Totals,
        tick: u64,
        record: Reproduction,
        state: &WorldState,
    ) -> Result<()> {
        let Reproduction::Funded {
            key,
            parent_reserve_before,
            parent_reserve_after,
            parent_energy_before,
            parent_energy_after,
            escrow_structure,
            escrow_reserve,
            escrow_energy,
            build_heat,
        } = record
        else {
            bail!("not a funding: {record:?}")
        };
        for (name, v) in [
            ("parent_reserve_before", parent_reserve_before),
            ("parent_reserve_after", parent_reserve_after),
            ("parent_energy_before", parent_energy_before),
            ("parent_energy_after", parent_energy_after),
            ("escrow_structure", escrow_structure),
            ("escrow_reserve", escrow_reserve),
            ("escrow_energy", escrow_energy),
            ("build_heat", build_heat),
        ] {
            finite_nonnegative(name, v)?;
        }
        ensure!(
            key.started_tick + 1 == tick,
            "a gestation funded in the step that completed tick {tick} must have started at {}, not {}",
            tick - 1,
            key.started_tick
        );
        ensure!(
            !open.contains_key(&key.parent),
            "{:?} funded a second gestation while one was outstanding",
            key.parent
        );

        // The identities, recomputed here rather than taken on trust.
        let material = escrow_structure + escrow_reserve;
        ensure!(
            close(parent_reserve_before - parent_reserve_after, material),
            "funding debit {} does not match the escrow material {material}",
            parent_reserve_before - parent_reserve_after
        );
        let e_r = self.rates.reserve_energy_density;
        let lost = (e_r * parent_reserve_before + parent_energy_before)
            - (e_r * parent_reserve_after + parent_energy_after);
        let held = e_r * material + escrow_energy;
        ensure!(
            close(lost, held + build_heat),
            "funding energy {lost} is not the escrow's {held} plus build heat {build_heat}"
        );
        ensure!(
            close(build_heat, self.rates.build_cost * escrow_structure),
            "build heat {build_heat} is not build_cost {} on structure {escrow_structure}",
            self.rates.build_cost
        );

        // The child inventory is the world's configured fractions of the parent's own decoded
        // size — from the live organism, or from the cached size if it died in this same tick.
        let sizes = state
            .organisms
            .get(key.parent)
            .map(|o| Sizes {
                structure_adult: o.phenotype.structure_adult,
                reserve_max: o.phenotype.reserve_max,
                energy_max: o.phenotype.energy_max,
            })
            .or_else(|| sizes.get(&key.parent).copied())
            .ok_or_else(|| {
                anyhow::anyhow!("no known size for {:?}: its funding fractions cannot be checked", key.parent)
            })?;
        for (name, reported, expected) in [
            ("structure", escrow_structure, self.rates.child_structure_fraction * sizes.structure_adult),
            ("reserve", escrow_reserve, self.rates.child_reserve_fraction * sizes.reserve_max),
            ("energy", escrow_energy, self.rates.child_energy_fraction * sizes.energy_max),
        ] {
            ensure!(
                close(reported, expected),
                "escrow {name} {reported} is not the configured fraction of the parent's own size ({expected})"
            );
        }

        totals.funded += 1;
        totals.funded_material += material;
        totals.funded_energy += held;
        totals.build_heat += build_heat;
        open.insert(
            key.parent,
            Open {
                started_tick: key.started_tick,
                funded_tick: tick,
                structure: escrow_structure,
                reserve: escrow_reserve,
                energy: escrow_energy,
            },
        );
        Ok(())
    }

    fn born(
        &self,
        open: &mut BTreeMap<OrganismId, Open>,
        totals: &mut Totals,
        tick: u64,
        record: Reproduction,
        state: &WorldState,
        index: &TickIndex,
    ) -> Result<OrganismId> {
        let Reproduction::Born { key, child, child_structure, child_reserve, child_energy, birth_heat } = record
        else {
            bail!("not a birth: {record:?}")
        };
        for (name, v) in [
            ("child_structure", child_structure),
            ("child_reserve", child_reserve),
            ("child_energy", child_energy),
            ("birth_heat", birth_heat),
        ] {
            finite_nonnegative(name, v)?;
        }
        let held = self.take(open, key, "birth")?;

        // The child is the escrow, and the structural material gave up its reserve energy.
        ensure!(close(child_structure, held.structure), "child structure {child_structure} is not the escrow's {}", held.structure);
        ensure!(close(child_reserve, held.reserve), "child reserve {child_reserve} is not the escrow's {}", held.reserve);
        ensure!(close(child_energy, held.energy), "child energy {child_energy} is not the escrow's {}", held.energy);
        ensure!(
            close(birth_heat, self.rates.reserve_energy_density * held.structure),
            "birth heat {birth_heat} is not e_r on the escrow's structure"
        );

        // The child is a real, new organism of this tick, by full ID, with that inventory.
        let o = state
            .organisms
            .get(child)
            .ok_or_else(|| anyhow::anyhow!("reported child {child:?} is not a live organism"))?;
        ensure!(o.born_tick == tick, "child {child:?} was born at {}, not {tick}", o.born_tick);
        ensure!(o.parent == Some(key.parent), "child {child:?} does not name {:?} as its parent", key.parent);
        ensure!(
            close(o.structure, child_structure) && close(o.reserve, child_reserve) && close(o.energy, child_energy),
            "child {child:?} does not hold the inventory the record reports"
        );
        ensure!(
            state.hunters.contains(child),
            "child {child:?} of a hunter was not granted membership"
        );

        // One-for-one with the identity records of the same tick.
        ensure!(
            index.offspring.get(&child) == Some(&key.parent),
            "no offspring record links {child:?} to {:?}",
            key.parent
        );
        ensure!(
            index.births.get(&child) == Some(&key.parent),
            "no ordinary birth event links {child:?} to {:?}",
            key.parent
        );

        totals.born += 1;
        totals.born_material += child_structure + child_reserve;
        totals.born_energy += self.rates.reserve_energy_density * child_reserve + child_energy;
        totals.birth_heat += birth_heat;
        Ok(child)
    }

    fn refunded(
        &self,
        open: &mut BTreeMap<OrganismId, Open>,
        totals: &mut Totals,
        record: Reproduction,
    ) -> Result<()> {
        let Reproduction::Refunded {
            key,
            refunded_structure,
            refunded_reserve,
            refunded_energy,
            parent_reserve_before,
            parent_reserve_after,
            parent_energy_before,
            parent_energy_after,
        } = record
        else {
            bail!("not a refund: {record:?}")
        };
        for (name, v) in [
            ("refunded_structure", refunded_structure),
            ("refunded_reserve", refunded_reserve),
            ("refunded_energy", refunded_energy),
            ("parent_reserve_before", parent_reserve_before),
            ("parent_reserve_after", parent_reserve_after),
            ("parent_energy_before", parent_energy_before),
            ("parent_energy_after", parent_energy_after),
        ] {
            finite_nonnegative(name, v)?;
        }
        let held = self.take(open, key, "refund")?;
        ensure!(close(refunded_structure, held.structure), "refunded structure is not the escrow's");
        ensure!(close(refunded_reserve, held.reserve), "refunded reserve is not the escrow's");
        ensure!(close(refunded_energy, held.energy), "refunded energy is not the escrow's");

        // Everything went back, and nothing was burned on the way.
        let material = refunded_structure + refunded_reserve;
        ensure!(
            close(parent_reserve_after - parent_reserve_before, material),
            "the refund returned {} of material, not {material}",
            parent_reserve_after - parent_reserve_before
        );
        ensure!(
            close(parent_energy_after - parent_energy_before, refunded_energy),
            "the refund returned {} of energy, not {refunded_energy}",
            parent_energy_after - parent_energy_before
        );

        totals.refunded += 1;
        totals.refunded_material += material;
        totals.refunded_energy += self.rates.reserve_energy_density * material + refunded_energy;
        Ok(())
    }

    fn miscarried(
        &self,
        open: &mut BTreeMap<OrganismId, Open>,
        totals: &mut Totals,
        record: Reproduction,
        index: &TickIndex,
    ) -> Result<()> {
        let Reproduction::Miscarried { key, cause, material, energy, energy_stored, energy_heat } = record else {
            bail!("not a miscarriage: {record:?}")
        };
        for (name, v) in [
            ("material", material),
            ("energy", energy),
            ("energy_stored", energy_stored),
            ("energy_heat", energy_heat),
        ] {
            finite_nonnegative(name, v)?;
        }
        let held = self.take(open, key, "miscarriage")?;

        // The escrow's own terms, and only those: a corpse is not an escrow.
        let expected_material = held.structure + held.reserve;
        ensure!(
            close(material, expected_material),
            "miscarried material {material} is not the escrow's {expected_material}"
        );
        let e_r = self.rates.reserve_energy_density;
        let expected_energy = e_r * expected_material + held.energy;
        ensure!(
            close(energy, expected_energy),
            "miscarried energy {energy} is not the escrow's {expected_energy}"
        );
        ensure!(close(energy_stored + energy_heat, energy), "the exported energy is unaccounted for");
        let cap = self.rates.detritus_energy_cap * material;
        ensure!(
            close(energy_stored, energy.min(cap)),
            "the detritus cap split is {energy_stored}, not min({energy}, {cap})"
        );

        // A miscarriage is the parent's death, with the same cause, in the same tick.
        let reported = index
            .deaths
            .get(&key.parent)
            .ok_or_else(|| anyhow::anyhow!("{:?} miscarried without dying this tick", key.parent))?;
        ensure!(
            *reported == cause,
            "the miscarriage says {cause:?} and the death says {reported:?}"
        );

        totals.miscarried += 1;
        totals.miscarried_material += material;
        totals.miscarried_energy += energy;
        totals.miscarried_stored += energy_stored;
        totals.miscarried_heat += energy_heat;
        Ok(())
    }

    /// Close an outstanding gestation, refusing an unknown key or a mismatched start.
    fn take(
        &self,
        open: &mut BTreeMap<OrganismId, Open>,
        key: cubarium_core::EscrowKey,
        what: &str,
    ) -> Result<Open> {
        let held = open
            .get(&key.parent)
            .copied()
            .ok_or_else(|| anyhow::anyhow!("{what} closes a gestation this audit never saw funded: {key:?}"))?;
        ensure!(
            held.started_tick == key.started_tick,
            "{what} names gestation {} of {:?}, which is holding {} instead",
            key.started_tick,
            key.parent,
            held.started_tick
        );
        open.remove(&key.parent);
        Ok(held)
    }

    /// What the audit carries must be exactly what the world holds after the step: same parents,
    /// same start ticks, same inventories.
    fn reconcile_open(&self, open: &BTreeMap<OrganismId, Open>, state: &WorldState) -> Result<()> {
        let mut held: BTreeMap<OrganismId, (u64, f64, f64, f64)> = BTreeMap::new();
        for member in &state.hunters.members {
            let Some(o) = state.organisms.get(member.id) else {
                bail!("hunter member {:?} is not a live organism", member.id)
            };
            if let Some(e) = &o.escrow {
                held.insert(member.id, (e.started_tick, e.structure, e.reserve, e.energy));
            }
        }
        for (parent, escrow) in open {
            let (started, structure, reserve, energy) = *held
                .get(parent)
                .ok_or_else(|| anyhow::anyhow!("{parent:?} owes a gestation the world is not holding"))?;
            ensure!(started == escrow.started_tick, "{parent:?} holds a gestation from {started}, not {}", escrow.started_tick);
            ensure!(
                close(structure, escrow.structure) && close(reserve, escrow.reserve) && close(energy, escrow.energy),
                "{parent:?} holds an escrow the audit's numbers do not match"
            );
        }
        for parent in held.keys() {
            ensure!(open.contains_key(parent), "{parent:?} holds an escrow this audit never saw funded");
        }
        Ok(())
    }

    /// The report. Open gestations at the horizon are **censored**, not failures: a run can end
    /// with one in flight, and nothing about it is yet a transaction.
    pub fn summary(&self) -> Value {
        let t = self.totals;
        json!({
            "opening_tick": self.opening_tick,
            "last_complete_tick": self.last_tick,
            "ticks_observed": self.ticks,
            "counts": {
                "funded": t.funded,
                "born": t.born,
                "refunded": t.refunded,
                "miscarried": t.miscarried,
                "not_funded_cap": t.not_funded_cap,
                "not_funded_stocks": t.not_funded_stocks,
                "closed": t.born + t.refunded + t.miscarried,
                "open_at_horizon": self.open.len(),
            },
            "material": {
                "funded": t.funded_material,
                "born": t.born_material,
                "refunded": t.refunded_material,
                "miscarried": t.miscarried_material,
            },
            "energy": {
                "funded": t.funded_energy,
                "born": t.born_energy,
                "refunded": t.refunded_energy,
                "miscarried": t.miscarried_energy,
                "miscarried_retained_as_detritus": t.miscarried_stored,
                "miscarried_as_heat": t.miscarried_heat,
            },
            "heat": { "build": t.build_heat, "birth": t.birth_heat },
            "open_censored": self.open.iter().map(|(parent, e)| json!({
                "parent": id_json(*parent),
                "started_tick": e.started_tick,
                "funded_tick": e.funded_tick,
                "structure": e.structure,
                "reserve": e.reserve,
                "energy": e.energy,
            })).collect::<Vec<_>>(),
            "provenance": {
                "quantities": "core mutation evidence: read at the assignment that moved them",
                "checks": "independent identities recomputed from world config, the parent's own \
                           decoded size, the escrows actually held after the step, and the \
                           offspring, birth and death records of the same tick",
                "not_claimed": "no transfer is measured from an event field alone, and post-step \
                                parent stocks are never treated as an isolated funding difference",
            },
        })
    }

    /// Outstanding gestations right now, for a caller that wants to censor explicitly.
    pub fn open_gestations(&self) -> usize {
        self.open.len()
    }

    /// The last tick that validated completely.
    pub fn last_complete_tick(&self) -> u64 {
        self.last_tick
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cubarium_core::genome::{Genome, decode};
    use cubarium_core::hunter::{EscrowKey, FixedHunterProfile, HunterTarget};
    use cubarium_core::organism::{Mode, Organism, Origin};
    use cubarium_core::rng::Counter;
    use cubarium_core::snapshot::state_hash;
    use cubarium_core::{World, WorldConfig};
    use cubarium_surface::{Face, SurfacePoint, Vec2};

    const SPOT: SurfacePoint = SurfacePoint { face: Face::Top, u: 22.0, v: 34.0 };

    /// A world with nothing growing or decaying of its own, so a fixture's mechanics are the
    /// only thing moving. A label for the scenario, never a claim about balance.
    fn quiet(mut cfg: WorldConfig) -> World {
        cfg.founders.kinds.clear();
        cfg.founders.count = 0;
        cfg.weather.amplitude = 0.0;
        cfg.water.rain_rate = 0.0;
        cfg.habitat.light_base = 0.0;
        cfg.habitat.light_height_gain = 0.0;
        cfg.habitat.light_noise_gain = 0.0;
        cfg.detritus.initial_dark = 0.0;
        cfg.detritus.decomposition = 0.0;
        cfg.detritus.fall = 0.0;
        World::new(cfg).expect("a quiet world is valid")
    }

    /// The trial profile with its reproduction clocks wound down so a test can watch a whole
    /// gestation. No other value changes.
    fn breeder(world: &World) -> FixedHunterProfile {
        let mut p = FixedHunterProfile::lanternjaw_trial(world.config());
        p.reproduce_min_age_seconds = 0.0;
        p.gestation_seconds = 1.0;
        p.reproduce_interval_seconds = 2.0;
        p
    }

    fn target(pos: SurfacePoint) -> HunterTarget {
        HunterTarget { face: pos.face.index() as u8, u: pos.u, v: pos.v }
    }

    fn found(world: &mut World, profile: FixedHunterProfile) -> OrganismId {
        world.start_hunter_trial(profile, target(SPOT)).expect("the trial starts").id
    }

    fn feed_to_full(world: &mut World, id: OrganismId) {
        let o = world.state.organisms.get_mut(id).expect("alive");
        let before = o.reserve;
        o.reserve = o.phenotype.reserve_max;
        o.energy = o.phenotype.energy_max;
        world.state.external_material_in += o.reserve - before;
    }

    fn set_stocks(world: &mut World, id: OrganismId, reserve: f64, energy: f64) {
        let o = world.state.organisms.get_mut(id).expect("alive");
        let before = o.reserve;
        o.reserve = reserve;
        o.energy = energy;
        world.state.external_material_in += reserve - before;
    }

    /// A body too large to be prey, to occupy a slot against the organism cap.
    fn bystander(world: &mut World, pos: SurfacePoint) -> OrganismId {
        let cfg = world.config().clone();
        let mut genome = Genome::founder(0.5, &cfg.drives);
        genome.size = 2.0;
        genome.clamp();
        let mut phenotype = decode(&genome, &cfg.organism);
        phenotype.speed_max = 0.0;
        phenotype.structure_adult = 1.8;
        let id = world.state.organisms.insert(Organism {
            pos: pos.canonicalize(),
            heading: Vec2::new(1.0, 0.0),
            ou: Vec2::ZERO,
            structure: 1.8,
            reserve: 0.4,
            energy: 0.5,
            born_tick: world.tick(),
            hunger_memory: 0.0,
            mode: Mode::Resting,
            escrow: None,
            births: 0,
            genome,
            phenotype,
            parent: None,
            origin: Origin::Founder,
            turn_counter: Counter::default(),
            fed_this_tick: false,
        });
        world.state.external_material_in += 1.8 + 0.4;
        id
    }

    /// Step once and hand the completed tick to the audit, exactly as the harness will.
    fn feed(world: &mut World, audit: &mut ReproductionAudit) -> Result<(Vec<HunterEvent>, Vec<LifeEvent>)> {
        world.step();
        let hunter = world.drain_hunter_events();
        let life = world.drain_events();
        audit.observe(&hunter, &life, &world.state)?;
        Ok((hunter, life))
    }

    /// Run until `done`, collecting every hunter record seen, with the audit following along.
    fn run(world: &mut World, audit: &mut ReproductionAudit, ticks: u64, mut done: impl FnMut(&[HunterEvent]) -> bool) -> Vec<HunterEvent> {
        let mut seen = Vec::new();
        for _ in 0..ticks {
            let (hunter, _) = feed(world, audit).expect("a genuine tick must audit");
            seen.extend(hunter);
            if done(&seen) {
                return seen;
            }
        }
        panic!("nothing matched in {ticks} ticks")
    }

    fn is(record: &Reproduction, want: &str) -> bool {
        match record {
            Reproduction::Funded { .. } => want == "funded",
            Reproduction::Born { .. } => want == "born",
            Reproduction::Refunded { .. } => want == "refunded",
            Reproduction::Miscarried { .. } => want == "miscarried",
            Reproduction::NotFunded { .. } => want == "not_funded",
        }
    }

    fn saw(events: &[HunterEvent], want: &str) -> bool {
        events.iter().any(|e| matches!(e, HunterEvent::Reproduction { record, .. } if is(record, want)))
    }

    fn count(summary: &Value, what: &str) -> u64 {
        summary["counts"][what].as_u64().expect("a count")
    }

    fn number(summary: &Value, group: &str, what: &str) -> f64 {
        summary[group][what].as_f64().expect("a number")
    }

    // ------------------------------------------------------------ genuine streams

    #[test]
    fn a_funded_gestation_and_its_birth_are_audited() {
        let mut world = quiet(WorldConfig::default());
        let profile = breeder(&world);
        let parent = found(&mut world, profile.clone());
        feed_to_full(&mut world, parent);
        let mut audit = ReproductionAudit::new(&world.state).expect("a fresh opening");

        run(&mut world, &mut audit, 60, |seen| saw(seen, "funded"));
        assert_eq!(count(&audit.summary(), "funded"), 1);
        assert_eq!(count(&audit.summary(), "open_at_horizon"), 1, "the gestation is outstanding");

        run(&mut world, &mut audit, 200, |seen| saw(seen, "born"));
        let s = audit.summary();
        assert_eq!(count(&s, "born"), 1);
        assert_eq!(count(&s, "closed"), 1);
        assert_eq!(count(&s, "open_at_horizon"), 0, "the gestation closed");
        assert_eq!(s["last_complete_tick"].as_u64(), Some(world.tick()));

        // The aggregates are the world's own child fractions of the founder's decoded size.
        let org = world.config().organism.clone();
        let sizes = decode(&profile.genome, &org);
        let structure = org.child_structure_fraction * sizes.structure_adult;
        let reserve = org.child_reserve_fraction * sizes.reserve_max;
        let energy = org.child_energy_fraction * sizes.energy_max;
        assert!(close(number(&s, "material", "funded"), structure + reserve));
        assert!(close(number(&s, "material", "born"), structure + reserve));
        assert!(close(number(&s, "heat", "build"), org.build_cost * structure));
        assert!(close(number(&s, "heat", "birth"), org.reserve_energy_density * structure));
        assert!(close(
            number(&s, "energy", "funded"),
            org.reserve_energy_density * (structure + reserve) + energy
        ));
    }

    #[test]
    fn a_due_birth_refused_by_the_cap_is_audited_as_a_refund() {
        let mut cfg = WorldConfig::default();
        cfg.capacity.max_organisms = 3;
        let mut world = quiet(cfg);
        let profile = breeder(&world);
        let parent = found(&mut world, profile);
        feed_to_full(&mut world, parent);
        bystander(&mut world, SurfacePoint::new(Face::Back, 8.0, 8.0));
        let mut audit = ReproductionAudit::new(&world.state).expect("a fresh opening");

        run(&mut world, &mut audit, 60, |seen| saw(seen, "funded"));
        bystander(&mut world, SurfacePoint::new(Face::Left, 8.0, 8.0));
        run(&mut world, &mut audit, 200, |seen| saw(seen, "refunded"));

        let s = audit.summary();
        assert_eq!(count(&s, "funded"), 1);
        assert_eq!(count(&s, "refunded"), 1);
        assert_eq!(count(&s, "born"), 0);
        assert_eq!(count(&s, "open_at_horizon"), 0);
        // A refund returns the material; it is not a miscarriage and burns nothing.
        assert!(close(number(&s, "material", "refunded"), number(&s, "material", "funded")));
        assert_eq!(number(&s, "material", "miscarried"), 0.0);
    }

    #[test]
    fn a_funding_and_an_age_death_in_one_tick_are_audited_together() {
        let mut cfg = WorldConfig::default();
        cfg.organism.max_age_seconds = 2.0;
        let mut world = quiet(cfg);
        let profile = breeder(&world);
        let parent = found(&mut world, profile);
        feed_to_full(&mut world, parent);
        // Hold the gate shut until the tick it dies on.
        world.state.hunters.member_mut(parent).expect("a member").next_reproduction_tick = 40;
        let mut audit = ReproductionAudit::new(&world.state).expect("a fresh opening");

        let seen = run(&mut world, &mut audit, 80, |seen| saw(seen, "miscarried"));
        let ticks: Vec<u64> = seen
            .iter()
            .filter(|e| matches!(e, HunterEvent::Reproduction { record, .. } if is(record, "funded") || is(record, "miscarried")))
            .map(HunterEvent::tick)
            .collect();
        assert_eq!(ticks.len(), 2, "both facts must be in the stream");
        assert_eq!(ticks[0], ticks[1], "this fixture needs them in one tick");

        let s = audit.summary();
        assert_eq!(count(&s, "funded"), 1, "the audit saw the funding it needed to close");
        assert_eq!(count(&s, "miscarried"), 1);
        assert_eq!(count(&s, "open_at_horizon"), 0);
        assert!(number(&s, "energy", "miscarried") > 0.0);
        assert!(close(
            number(&s, "energy", "miscarried_retained_as_detritus") + number(&s, "energy", "miscarried_as_heat"),
            number(&s, "energy", "miscarried")
        ));
    }

    #[test]
    fn an_ordinary_miscarriage_is_audited_without_the_corpse() {
        let mut world = quiet(WorldConfig::default());
        let profile = breeder(&world);
        let parent = found(&mut world, profile);
        feed_to_full(&mut world, parent);
        let mut audit = ReproductionAudit::new(&world.state).expect("a fresh opening");
        run(&mut world, &mut audit, 60, |seen| saw(seen, "funded"));

        // Starve it while it gestates.
        let body = world.state.organisms.get(parent).expect("alive").structure;
        set_stocks(&mut world, parent, 0.0, 0.0);
        run(&mut world, &mut audit, 60, |seen| saw(seen, "miscarried"));

        let s = audit.summary();
        assert_eq!(count(&s, "miscarried"), 1);
        assert_eq!(count(&s, "born"), 0);
        let exported = number(&s, "material", "miscarried");
        assert!(exported > 0.0 && exported < body, "the escrow is not the corpse: {exported} vs {body}");
        assert!(close(exported, number(&s, "material", "funded")));
    }

    #[test]
    fn blocked_funding_is_counted_by_reason() {
        // The cap refuses the gestation before it begins: no escrow, no transaction.
        let mut cfg = WorldConfig::default();
        cfg.capacity.max_organisms = 2;
        let mut world = quiet(cfg);
        let profile = breeder(&world);
        let parent = found(&mut world, profile);
        feed_to_full(&mut world, parent);
        bystander(&mut world, SurfacePoint::new(Face::Back, 8.0, 8.0));
        let mut audit = ReproductionAudit::new(&world.state).expect("a fresh opening");
        run(&mut world, &mut audit, 60, |seen| saw(seen, "not_funded"));
        let s = audit.summary();
        assert!(count(&s, "not_funded_cap") >= 1);
        assert_eq!(count(&s, "not_funded_stocks"), 0);
        assert_eq!(count(&s, "funded"), 0, "nothing was ever funded");
        assert_eq!(count(&s, "open_at_horizon"), 0);

        // Its own stocks refuse it: the gate opens, the child cannot be paid for.
        let mut world = quiet(WorldConfig::default());
        let mut profile = breeder(&world);
        profile.reproduce_reserve_fraction = 0.0;
        profile.reproduce_energy_fraction = 0.0;
        let parent = found(&mut world, profile);
        set_stocks(&mut world, parent, 0.1, 0.2);
        let mut audit = ReproductionAudit::new(&world.state).expect("a fresh opening");
        run(&mut world, &mut audit, 60, |seen| saw(seen, "not_funded"));
        let s = audit.summary();
        assert!(count(&s, "not_funded_stocks") >= 1);
        assert_eq!(count(&s, "not_funded_cap"), 0);
        assert_eq!(count(&s, "funded"), 0);
    }

    // ------------------------------------------------------------ refusals

    /// A world paused with a genuine funding in hand, so a test can corrupt that one tick.
    struct Staged {
        world: World,
        audit: ReproductionAudit,
        hunter: Vec<HunterEvent>,
        life: Vec<LifeEvent>,
    }

    /// Step until the tick that carries a genuine `Born`, and hold that tick's batches back
    /// instead of feeding them: the audit is up to date through the tick before it.
    fn staged_birth() -> Staged {
        let mut world = quiet(WorldConfig::default());
        let profile = breeder(&world);
        let parent = found(&mut world, profile);
        feed_to_full(&mut world, parent);
        let mut audit = ReproductionAudit::new(&world.state).expect("a fresh opening");
        for _ in 0..400 {
            world.step();
            let hunter = world.drain_hunter_events();
            let life = world.drain_events();
            if saw(&hunter, "born") {
                return Staged { world, audit, hunter, life };
            }
            audit.observe(&hunter, &life, &world.state).expect("a genuine tick must audit");
        }
        panic!("no birth in 400 ticks")
    }

    /// The audit is unchanged by a refusal, and still accepts the genuine tick afterwards.
    fn refuses(mut staged: Staged, corrupt: impl Fn(&mut Vec<HunterEvent>), why: &str) {
        let before = (staged.audit.summary(), staged.audit.last_complete_tick(), staged.audit.open_gestations());
        let mut broken = staged.hunter.clone();
        corrupt(&mut broken);
        let err = staged
            .audit
            .observe(&broken, &staged.life, &staged.world.state)
            .expect_err(&format!("{why} must be refused"));
        assert!(!err.to_string().is_empty(), "{why}");
        let after = (staged.audit.summary(), staged.audit.last_complete_tick(), staged.audit.open_gestations());
        assert_eq!(before, after, "{why}: the refusal was not atomic");
        // The genuine batch still goes through, which is what "unchanged" has to mean.
        staged
            .audit
            .observe(&staged.hunter, &staged.life, &staged.world.state)
            .unwrap_or_else(|e| panic!("{why}: the genuine tick was rejected afterwards: {e}"));
    }

    fn born_record(events: &[HunterEvent]) -> (usize, OrganismId, Reproduction) {
        events
            .iter()
            .enumerate()
            .find_map(|(i, e)| match e {
                HunterEvent::Reproduction { hunter, record, .. } if is(record, "born") => Some((i, *hunter, *record)),
                _ => None,
            })
            .expect("a birth record")
    }

    #[test]
    fn a_missing_closure_is_refused_and_leaves_the_audit_unchanged() {
        // The birth vanishes from the stream: the world is holding no escrow, but the audit is.
        refuses(
            staged_birth(),
            |events| events.retain(|e| !matches!(e, HunterEvent::Reproduction { record, .. } if is(record, "born"))),
            "a birth dropped from the stream",
        );
    }

    #[test]
    fn a_duplicated_closure_is_refused() {
        refuses(
            staged_birth(),
            |events| {
                let (i, _, _) = born_record(events);
                let copy = events[i].clone();
                events.push(copy);
            },
            "the same birth twice",
        );
    }

    #[test]
    fn a_reordered_closure_is_refused() {
        // A funding and a closure can share a tick, but a closure can never precede its funding.
        let mut cfg = WorldConfig::default();
        cfg.organism.max_age_seconds = 2.0;
        let mut world = quiet(cfg);
        let profile = breeder(&world);
        let parent = found(&mut world, profile);
        feed_to_full(&mut world, parent);
        world.state.hunters.member_mut(parent).expect("a member").next_reproduction_tick = 40;
        let mut audit = ReproductionAudit::new(&world.state).expect("a fresh opening");
        for _ in 0..80 {
            world.step();
            let hunter = world.drain_hunter_events();
            let life = world.drain_events();
            if saw(&hunter, "miscarried") {
                let mut reversed = hunter.clone();
                reversed.reverse();
                let err = audit
                    .observe(&reversed, &life, &world.state)
                    .expect_err("a closure before its funding must be refused");
                assert!(err.to_string().contains("never saw funded"), "{err}");
                audit.observe(&hunter, &life, &world.state).expect("the genuine order audits");
                return;
            }
            audit.observe(&hunter, &life, &world.state).expect("a genuine tick must audit");
        }
        panic!("the fixture never produced a same-tick funding and loss");
    }

    #[test]
    fn a_duplicated_funding_is_refused() {
        let mut world = quiet(WorldConfig::default());
        let profile = breeder(&world);
        let parent = found(&mut world, profile);
        feed_to_full(&mut world, parent);
        let mut audit = ReproductionAudit::new(&world.state).expect("a fresh opening");
        for _ in 0..60 {
            world.step();
            let hunter = world.drain_hunter_events();
            let life = world.drain_events();
            if saw(&hunter, "funded") {
                let mut doubled = hunter.clone();
                let copy = hunter
                    .iter()
                    .find(|e| matches!(e, HunterEvent::Reproduction { record, .. } if is(record, "funded")))
                    .expect("the funding")
                    .clone();
                doubled.push(copy);
                let err = audit
                    .observe(&doubled, &life, &world.state)
                    .expect_err("two fundings for one parent must be refused");
                assert!(err.to_string().contains("second gestation"), "{err}");
                audit.observe(&hunter, &life, &world.state).expect("the genuine batch audits");
                return;
            }
            audit.observe(&hunter, &life, &world.state).expect("a genuine tick must audit");
        }
        panic!("no funding in 60 ticks");
    }

    #[test]
    fn a_wrong_generation_is_refused_on_either_side_of_the_transaction() {
        // A child whose generation is not the organism that was born.
        refuses(
            staged_birth(),
            |events| {
                let (i, hunter, record) = born_record(events);
                let Reproduction::Born { key, child, child_structure, child_reserve, child_energy, birth_heat } = record
                else {
                    unreachable!()
                };
                events[i] = HunterEvent::Reproduction {
                    tick: events[i].tick(),
                    hunter,
                    record: Reproduction::Born {
                        key,
                        child: OrganismId { slot: child.slot, generation: child.generation + 1 },
                        child_structure,
                        child_reserve,
                        child_energy,
                        birth_heat,
                    },
                };
            },
            "a child with a stale generation",
        );
        // A parent whose generation never funded anything.
        refuses(
            staged_birth(),
            |events| {
                let (i, _, record) = born_record(events);
                let Reproduction::Born { key, child, child_structure, child_reserve, child_energy, birth_heat } = record
                else {
                    unreachable!()
                };
                let stale = OrganismId { slot: key.parent.slot, generation: key.parent.generation + 1 };
                events[i] = HunterEvent::Reproduction {
                    tick: events[i].tick(),
                    hunter: stale,
                    record: Reproduction::Born {
                        key: EscrowKey { parent: stale, started_tick: key.started_tick },
                        child,
                        child_structure,
                        child_reserve,
                        child_energy,
                        birth_heat,
                    },
                };
            },
            "a parent with a stale generation",
        );
    }

    #[test]
    fn wrong_numbers_and_non_finite_numbers_are_refused() {
        // A birth heat that is not the reserve energy the structure gave up.
        refuses(
            staged_birth(),
            |events| {
                let (i, hunter, record) = born_record(events);
                let Reproduction::Born { key, child, child_structure, child_reserve, child_energy, birth_heat } = record
                else {
                    unreachable!()
                };
                events[i] = HunterEvent::Reproduction {
                    tick: events[i].tick(),
                    hunter,
                    record: Reproduction::Born {
                        key,
                        child,
                        child_structure,
                        child_reserve,
                        child_energy,
                        birth_heat: birth_heat * 1.5,
                    },
                };
            },
            "an invented birth heat",
        );
        // A child inventory that is not the escrow the audit is carrying.
        refuses(
            staged_birth(),
            |events| {
                let (i, hunter, record) = born_record(events);
                let Reproduction::Born { key, child, child_structure, child_reserve, child_energy, birth_heat } = record
                else {
                    unreachable!()
                };
                events[i] = HunterEvent::Reproduction {
                    tick: events[i].tick(),
                    hunter,
                    record: Reproduction::Born {
                        key,
                        child,
                        child_structure,
                        child_reserve: child_reserve + 0.25,
                        child_energy,
                        birth_heat,
                    },
                };
            },
            "a child inventory that is not the escrow",
        );
        // A non-finite quantity.
        refuses(
            staged_birth(),
            |events| {
                let (i, hunter, record) = born_record(events);
                let Reproduction::Born { key, child, child_reserve, child_energy, birth_heat, .. } = record else {
                    unreachable!()
                };
                events[i] = HunterEvent::Reproduction {
                    tick: events[i].tick(),
                    hunter,
                    record: Reproduction::Born {
                        key,
                        child,
                        child_structure: f64::NAN,
                        child_reserve,
                        child_energy,
                        birth_heat,
                    },
                };
            },
            "a NaN child structure",
        );
    }

    #[test]
    fn a_wrapper_that_disagrees_with_its_record_is_refused() {
        refuses(
            staged_birth(),
            |events| {
                let (i, hunter, record) = born_record(events);
                events[i] = HunterEvent::Reproduction {
                    tick: events[i].tick(),
                    hunter: OrganismId { slot: hunter.slot + 7, generation: hunter.generation },
                    record,
                };
            },
            "a wrapper naming a different hunter than its record",
        );
    }

    #[test]
    fn a_record_from_another_tick_is_refused() {
        refuses(
            staged_birth(),
            |events| {
                let (i, hunter, record) = born_record(events);
                events[i] = HunterEvent::Reproduction { tick: events[i].tick() - 1, hunter, record };
            },
            "a record stamped with another tick",
        );
    }

    #[test]
    fn a_gap_in_the_tick_sequence_is_refused() {
        let mut world = quiet(WorldConfig::default());
        let profile = breeder(&world);
        let parent = found(&mut world, profile);
        feed_to_full(&mut world, parent);
        let mut audit = ReproductionAudit::new(&world.state).expect("a fresh opening");
        world.step();
        let _ = world.drain_hunter_events();
        let _ = world.drain_events();
        world.step();
        let hunter = world.drain_hunter_events();
        let life = world.drain_events();
        let err = audit.observe(&hunter, &life, &world.state).expect_err("a skipped tick must be refused");
        assert!(err.to_string().contains("consecutive"), "{err}");
        assert_eq!(audit.last_complete_tick(), world.tick() - 2, "and nothing was recorded");
    }

    #[test]
    fn a_mid_gestation_opening_is_refused() {
        let mut world = quiet(WorldConfig::default());
        let profile = breeder(&world);
        let parent = found(&mut world, profile);
        feed_to_full(&mut world, parent);
        let mut audit = ReproductionAudit::new(&world.state).expect("a fresh opening");
        run(&mut world, &mut audit, 60, |seen| saw(seen, "funded"));
        assert!(world.state.organisms.get(parent).expect("alive").escrow.is_some());

        // Attaching a second audit here would have to invent the debits that funded it.
        let err = ReproductionAudit::new(&world.state).expect_err("a mid-gestation opening must be refused");
        assert!(err.to_string().contains("mid-gestation"), "{err}");
    }

    // ------------------------------------------------------------ bounds and non-interference

    /// The audit keeps one open gestation per living parent and a size cache pruned to the
    /// living lineage — never a history. Repeated outcomes must not grow it.
    #[test]
    fn memory_stays_bounded_over_repeated_outcomes() {
        let mut world = quiet(WorldConfig::default());
        let mut profile = breeder(&world);
        // Short clocks and a small world: fund, bear, fund again, over and over.
        profile.reproduce_interval_seconds = 1.0;
        let parent = found(&mut world, profile);
        feed_to_full(&mut world, parent);
        let mut audit = ReproductionAudit::new(&world.state).expect("a fresh opening");

        let mut high_water = 0;
        for tick in 0..1_500u64 {
            world.step();
            let hunter = world.drain_hunter_events();
            let life = world.drain_events();
            audit.observe(&hunter, &life, &world.state).expect("a genuine tick must audit");
            // Every parent keeps refilling, so the lineage keeps reproducing.
            if tick % 40 == 0 {
                let members: Vec<OrganismId> = world.state.hunters.members.iter().map(|m| m.id).collect();
                for id in members {
                    feed_to_full(&mut world, id);
                }
            }
            let members = world.state.hunters.members.len();
            assert!(audit.open.len() <= members, "one open gestation per member at most");
            assert!(
                audit.sizes.len() <= members + audit.open.len(),
                "the size cache is not pruned: {} against {members} members",
                audit.sizes.len()
            );
            high_water = high_water.max(audit.open.len() + audit.sizes.len());
        }
        let s = audit.summary();
        assert!(count(&s, "funded") >= 3, "this fixture should repeat: {s}");
        assert!(count(&s, "closed") >= 2);
        assert!(
            high_water <= 2 * (world.config().capacity.max_organisms as usize + 1),
            "the audit grew with history: {high_water}"
        );
    }

    /// Observing is reading. The same run with and without the audit — including a capture and
    /// a reproduction — must end on the same world, bit for bit.
    #[test]
    fn observing_does_not_move_the_world() {
        // The fixture: a hunter that hunts a placed prey and also breeds.
        fn scenario(audit: bool) -> (u64, u64) {
            let mut world = quiet(WorldConfig::default());
            let mut profile = breeder(&world);
            profile.capture_min = 1.0;
            profile.capture_max = 1.0;
            let parent = found(&mut world, profile.clone());
            // A frozen prey inside the claws, so the run settles a real capture too.
            let grasp = world.hunter_view()[0].capture_center.expect("a centre on the open top face");
            let cfg = world.config().clone();
            let mut genome = Genome::founder(0.5, &cfg.drives);
            genome.size = 0.5;
            genome.speed = 0.3;
            genome.clamp();
            let mut phenotype = decode(&genome, &cfg.organism);
            phenotype.speed_max = 0.0;
            phenotype.structure_adult = 0.5;
            world.state.organisms.insert(Organism {
                pos: grasp.canonicalize(),
                heading: Vec2::new(1.0, 0.0),
                ou: Vec2::ZERO,
                structure: 0.5,
                reserve: 0.3,
                energy: 0.4,
                born_tick: world.tick(),
                hunger_memory: 0.0,
                mode: Mode::Resting,
                escrow: None,
                births: 0,
                genome,
                phenotype,
                parent: None,
                origin: Origin::Founder,
                turn_counter: Counter::default(),
                fed_this_tick: false,
            });
            world.state.external_material_in += 0.8;
            feed_to_full(&mut world, parent);

            let mut watcher = audit.then(|| ReproductionAudit::new(&world.state).expect("a fresh opening"));
            for _ in 0..400 {
                world.step();
                // Both runs drain, so the comparison isolates the audit's own calls.
                let hunter = world.drain_hunter_events();
                let life = world.drain_events();
                if let Some(watcher) = watcher.as_mut() {
                    watcher.observe(&hunter, &life, &world.state).expect("a genuine tick must audit");
                    // Reading the report every tick must be just as inert.
                    let _ = watcher.summary();
                }
            }
            (state_hash(&world.state), world.hunters().captures_total)
        }

        let (watched, captures) = scenario(true);
        let (unwatched, same_captures) = scenario(false);
        assert_eq!(watched, unwatched, "observation moved the world");
        assert_eq!(captures, same_captures);
        assert!(captures >= 1, "this fixture should settle a capture: {captures}");
    }
}
