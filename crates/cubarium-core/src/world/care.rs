use cubarium_surface::CellId;

use crate::accounting::EnergyLedgers;
use crate::care::{
    self, ActiveShower, CareApplied, CareCommand, CareDose, CareKind, CareOutcome, CareReceipt,
    CareState,
};

use super::*;

impl World {
    /// The care ledgers, the sequence cursor, and any shower in progress.
    pub fn care(&self) -> &CareState {
        &self.state.care
    }

    /// The compensated cumulative energy ledgers
    /// ([`WorldState::energy_ledgers`], `crate::accounting`). Hold a reading before an
    /// interval and call [`EnergyLedgers::net_since`] to get the energy booked over it
    /// without re-rounding the two large totals.
    pub fn energy_ledgers(&self) -> EnergyLedgers {
        self.state.energy_ledgers()
    }

    /// Apply one care command at a held boundary
    /// (`design/7_Research/care-contract-2026-09-12.md`).
    ///
    /// Admission is strict and total:
    /// - only `seq == care.admitted_seq + 1` is accepted; any other `seq` is
    ///   `Rejected("out of order")` **without changing a single value**;
    /// - `world.tick()` must equal `cmd.apply_after_tick`, else `Rejected("wrong boundary")`,
    ///   again without changing anything (the host treats it as a recovery failure);
    /// - a command with the expected `seq` at the right boundary **always** consumes that
    ///   seq, including when its own validation rejects it, so replay is a pure function of
    ///   the journal.
    ///
    /// Feed and Clean change the fields immediately. Rain registers a shower whose first
    /// sample falls in the step `B → B + 1`.
    pub fn apply_care(&mut self, cmd: &CareCommand) -> CareReceipt {
        let tick = self.state.tick;
        let receipt = |outcome| CareReceipt {
            seq: cmd.seq,
            tick,
            outcome,
        };
        if cmd.seq != self.state.care.admitted_seq.wrapping_add(1) {
            return receipt(CareOutcome::Rejected("out of order".into()));
        }
        if tick != cmd.apply_after_tick {
            return receipt(CareOutcome::Rejected("wrong boundary".into()));
        }
        // From here the seq is spent whatever happens next.
        self.state.care.admitted_seq = cmd.seq;
        // A dose outside the documented range is refused, not clamped — and it still spends
        // its sequence, like every other admitted-then-rejected command.
        if let Err(reason) = cmd.dose.validate("care dose") {
            return receipt(CareOutcome::Rejected(reason));
        }
        let Some(center) = cmd.target.resolve(self.topology(), self.scale()) else {
            return receipt(CareOutcome::Rejected("invalid target".into()));
        };
        let outcome = match cmd.kind {
            CareKind::Feed => self.feed(center, cmd.dose),
            CareKind::Rain => self.shower(cmd.seq, tick, center, cmd.dose),
            CareKind::Clean => self.clean(center, cmd.dose),
        };
        receipt(outcome)
    }

    /// Consume `seq` with no effect. The host calls this for a record it durably aborted, so
    /// a replay of the journal reaches the same cursor as the run that wrote it. Returns
    /// false (and changes nothing) when `seq` is not the next one.
    pub fn void_care(&mut self, seq: u64) -> bool {
        if seq != self.state.care.admitted_seq.wrapping_add(1) {
            return false;
        }
        self.state.care.admitted_seq = seq;
        true
    }

    // ------------------------------------------------------------------ hunters

    /// "Scatter food": charged organic crumbs into `D` and `De`. Per cell `D += m·w_c` and
    /// `De += rho·(m·w_c)` with `rho = detritus.energy_cap`, so `De ≤ energy_cap · D` is
    /// preserved cell by cell and existing scavenging diets can eat all of it. No organism
    /// state is touched: they find it by the sensing they already have.
    fn feed(&mut self, center: CellId, dose: CareDose) -> CareOutcome {
        // The dose scales the nominal total this command moves, and nothing else: the
        // footprint, the weights, the energy density and the allowance rule are unchanged.
        let m = dose.scale(care::FEED_MATERIAL);
        // The bound is the nominal dose. The ledger books the actual f64 sums, which trail
        // the nominal total by a few ulps (a rim footprint sums `3 · w_c` to
        // `3.0000000000000004`), so without the tolerance the allowance would silently buy
        // one fewer feed at the rim than in the interior.
        //
        // A dose the remaining allowance cannot cover is **refused**, not quietly served
        // smaller: the viewer asked for an amount, and a smaller one is a different answer.
        if self.state.care.allowance_used + m > care::FEED_ALLOWANCE + care::ALLOWANCE_TOLERANCE {
            return CareOutcome::Rejected("allowance exhausted".into());
        }
        let rho = care::feed_energy_density(&self.state.config);
        let footprint = care::footprint(&self.graph, center, care::FEED_HOPS);
        let (mut material, mut energy) = (0.0, 0.0);
        for (cell, w) in &footprint {
            let add = m * w;
            let add_energy = rho * add;
            self.state.fields.d[cell.index()] += add;
            self.state.fields.de[cell.index()] += add_energy;
            material += add;
            energy += add_energy;
        }
        self.state.care.feed_material_in += material;
        self.state.care.feed_energy_in += energy;
        self.state.care.allowance_used += material;
        CareOutcome::Applied(CareApplied {
            material_in: material,
            energy_in: energy,
            cells: footprint.len() as u32,
            ..CareApplied::default()
        })
    }

    /// "Shower": register the one active shower. Cells and weights are resolved now, so a
    /// snapshot taken mid-shower resumes at the next undelivered sample over the same
    /// footprint. The weather source is never mutated.
    fn shower(&mut self, seq: u64, tick: u64, center: CellId, dose: CareDose) -> CareOutcome {
        if !self.state.care.showers.is_empty() {
            return CareOutcome::Rejected("shower active".into());
        }
        let footprint = care::footprint(&self.graph, center, care::RAIN_HOPS);
        let cells = footprint.len() as u32;
        self.state.care.showers.push(ActiveShower {
            seq,
            apply_after_tick: tick,
            cells: footprint.iter().map(|(c, _)| c.0).collect(),
            weights: footprint.iter().map(|(_, w)| *w).collect(),
            delivered: 0,
            // Persisted with the shower: the amount that was asked for is the amount its
            // remaining samples deliver, across any number of restarts.
            dose_permille: dose.permille(),
        });
        CareOutcome::Applied(CareApplied {
            // The scheduled total; what lands is booked tick by tick in `rain_depth_in`.
            water_depth: dose.scale(care::RAIN_DEPTH_TOTAL),
            cells,
            ends_tick: Some(tick + u64::from(care::RAIN_TICKS)),
            ..CareApplied::default()
        })
    }

    /// "Clean up litter": export `D` and the chemical energy it carried, at the cell's own
    /// energy density, so `De ≤ energy_cap · D` still holds. `N`, `P`, `F`, `w` and every
    /// organism are untouched, and the exported energy is an export, not heat dissipated
    /// inside the world.
    fn clean(&mut self, center: CellId, dose: CareDose) -> CareOutcome {
        // The dose scales the maximum export. The per-cell half-of-what-is-there limit, the
        // footprint and the proportional energy export are untouched, so a larger dose still
        // cannot sterilize a cell.
        let cap = dose.scale(care::CLEAN_MATERIAL);
        let footprint = care::footprint(&self.graph, center, care::CLEAN_HOPS);
        let (mut material, mut energy) = (0.0, 0.0);
        for (cell, w) in &footprint {
            let i = cell.index();
            let d = self.state.fields.d[i];
            if d <= 0.0 {
                continue;
            }
            let take = (care::CLEAN_FRACTION * d).min(cap * w);
            if take <= 0.0 {
                continue;
            }
            let de = self.state.fields.de[i];
            let removed = (de * (take / d)).min(de).max(0.0);
            self.state.fields.d[i] = d - take;
            self.state.fields.de[i] = de - removed;
            material += take;
            energy += removed;
        }
        if material <= 0.0 {
            return CareOutcome::Rejected("nothing to remove".into());
        }
        self.state.care.clean_material_out += material;
        self.state.care.clean_energy_out += energy;
        // A clean gives the allowance back, but never turns into credit.
        self.state.care.allowance_used = (self.state.care.allowance_used - material).max(0.0);
        let q = CareApplied {
            material_out: material,
            energy_out: energy,
            cells: footprint.len() as u32,
            ..CareApplied::default()
        };
        if material + care::WEIGHT_TOLERANCE < cap {
            CareOutcome::Partial(q)
        } else {
            CareOutcome::Applied(q)
        }
    }
}
