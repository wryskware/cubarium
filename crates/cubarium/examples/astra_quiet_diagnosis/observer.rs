//! Read-only completed-tick observations, not reconstructed feeding/oxidation transactions.
use anyhow::{Result, ensure};
use cubarium_core::{
    DT, OrganismId, WorldState,
    organism::{Mode, Organism},
};
use serde::Serialize;
use serde_json::{Value, json};
use std::collections::BTreeMap;

#[derive(Clone, Serialize)]
pub struct Point {
    tick: u64,
    reserve: f64,
    reserve_fraction: f64,
    energy_fraction: f64,
    structure: f64,
    adult_fraction: f64,
    hunger: f64,
    memory: f64,
    seek_off: f64,
    seek_on: f64,
    tau: f64,
    bud_reserve: f64,
    bud_energy: f64,
    bud_min_age_seconds: f64,
    mode: Mode,
    fed: bool,
    escrow: Option<u64>,
}
impl Point {
    fn of(o: &Organism, tick: u64) -> Self {
        Self {
            tick,
            reserve: o.reserve,
            reserve_fraction: o.reserve / o.phenotype.reserve_max,
            energy_fraction: o.energy / o.phenotype.energy_max,
            structure: o.structure,
            adult_fraction: o.structure / o.phenotype.structure_adult,
            hunger: o.hunger(),
            memory: o.hunger_memory,
            seek_off: f64::from(o.phenotype.drives.seek_off),
            seek_on: f64::from(o.phenotype.drives.seek_on),
            tau: f64::from(o.phenotype.drives.tau_hunger_seconds),
            bud_reserve: f64::from(o.phenotype.drives.bud_reserve),
            bud_energy: f64::from(o.phenotype.drives.bud_energy),
            bud_min_age_seconds: f64::from(o.phenotype.drives.bud_min_age_seconds),
            mode: o.mode,
            fed: o.fed_this_tick,
            escrow: o.escrow.as_ref().map(|e| e.started_tick),
        }
    }
    fn next_memory(&self) -> f64 {
        self.memory + (1.0 - (-DT / self.tau.max(1e-9)).exp()) * (self.hunger - self.memory)
    }
    fn next_resting(&self) -> bool {
        if self.mode == Mode::Resting {
            self.next_memory() <= self.seek_on
        } else {
            self.next_memory() < self.seek_off
        }
    }
}

#[derive(Default, Serialize)]
pub struct Counts {
    ticks: u64,
    fed: u64,
    resting: u64,
    feeding_mode: u64,
    seeking: u64,
    immature: u64,
    gestating: u64,
    reserve_zero: u64,
    fed_reserve_zero: u64,
    reserve_below_growth_gate: u64,
    reserve_at_bud_gate: u64,
    energy_at_bud_gate: u64,
    both_bud_stock_gates: u64,
    instant_hunger_below_off: u64,
    memory_below_off: u64,
    memory_above_on: u64,
    post_energy_below_oxidation_threshold: u64,
    fed_reserve_increased: u64,
    fed_reserve_not_increased: u64,
    survived_predecision_ticks: u64,
    rest_admission_gate_open: u64,
    active_to_rest_entries: u64,
    newborn_rest_entries: u64,
    observed_escrow_starts: u64,
    observed_escrow_closes: u64,
    growth_material: f64,
    escrow_start_material: f64,
    reserve_fraction_sum: f64,
    energy_fraction_sum: f64,
    memory_sum: f64,
}
#[derive(Serialize)]
struct Entry {
    id: OrganismId,
    form: u8,
    opening_member: bool,
    born_tick: u64,
    parent: Option<OrganismId>,
    initial: Point,
    last: Point,
    diet: f64,
    graze_rate: f64,
    scavenge_rate: f64,
    reserve_max: f64,
    energy_max: f64,
    maintenance: f64,
    sense_radius: f64,
    counts: Counts,
    min_memory: f64,
    max_reserve_fraction: f64,
    #[serde(skip)]
    rest: Option<(u64, u64, bool, &'static str)>,
}
impl Entry {
    fn new(id: OrganismId, o: &Organism, tick: u64, opening: bool) -> Self {
        let p = Point::of(o, tick);
        Self {
            id,
            form: o.phenotype.form,
            opening_member: opening,
            born_tick: o.born_tick,
            parent: o.parent,
            initial: p.clone(),
            last: p.clone(),
            diet: o.phenotype.diet,
            graze_rate: o.phenotype.graze_rate,
            scavenge_rate: o.phenotype.scavenge_rate,
            reserve_max: o.phenotype.reserve_max,
            energy_max: o.phenotype.energy_max,
            maintenance: o.phenotype.maintenance,
            sense_radius: o.phenotype.sense_radius,
            counts: Counts::default(),
            min_memory: p.memory,
            max_reserve_fraction: p.reserve_fraction,
            rest: if opening && p.mode == Mode::Resting {
                Some((tick, 0, true, "opening"))
            } else {
                None
            },
        }
    }
    fn close_rest(&mut self, tick: u64, reason: &str) -> Option<Value> {
        self.rest.take().map(|(start,len,left,entry)|json!({"kind":"rest_bout","id":self.id,
            "form":self.form,"start_tick":start,"end_boundary":tick,"observed_resting_ticks":len,
            "left_censored":left,"right_censored":reason=="horizon","entry":entry,"end_reason":reason}))
    }
}

pub struct Observer {
    opening: u64,
    last: u64,
    entries: BTreeMap<OrganismId, Entry>,
}
impl Observer {
    pub fn new(s: &WorldState) -> Self {
        Self {
            opening: s.tick,
            last: s.tick,
            entries: s
                .organisms
                .iter()
                .map(|(id, o)| (id, Entry::new(id, o, s.tick, true)))
                .collect(),
        }
    }
    pub fn observe(&mut self, s: &WorldState) -> Result<Vec<Value>> {
        ensure!(
            self.last.checked_add(1) == Some(s.tick),
            "observer skipped/repeated tick"
        );
        let mut records = Vec::new();
        for (&id, e) in &mut self.entries {
            if e.last.tick == self.last && s.organisms.get(id).is_none() {
                if let Some(r) = e.close_rest(s.tick, "death") {
                    records.push(r);
                }
            }
        }
        for (id, o) in s.organisms.iter() {
            let new = !self.entries.contains_key(&id);
            if new {
                ensure!(
                    self.entries.len() < 20000,
                    "bounded full-ID observer capacity exceeded"
                );
            }
            let e = self
                .entries
                .entry(id)
                .or_insert_with(|| Entry::new(id, o, s.tick, false));
            let p = Point::of(o, s.tick);
            let prior = if new { None } else { Some(e.last.clone()) };
            if let Some(a) = &prior {
                ensure!(a.tick == self.last, "stale full ID reappeared");
                ensure!(
                    a.next_memory().to_bits() == p.memory.to_bits(),
                    "memory update mismatch"
                );
                ensure!(
                    a.next_resting() == (p.mode == Mode::Resting),
                    "ordinary rest gate mismatch"
                );
                e.counts.survived_predecision_ticks += 1;
                e.counts.rest_admission_gate_open +=
                    u64::from(a.mode != Mode::Resting && a.next_memory() < a.seek_off);
                let grown = p.structure - a.structure;
                ensure!(grown >= 0.0, "unexpected ordinary structure loss");
                e.counts.growth_material += grown;
                if p.fed {
                    if p.reserve > a.reserve {
                        e.counts.fed_reserve_increased += 1;
                    } else {
                        e.counts.fed_reserve_not_increased += 1;
                    }
                }
                if a.escrow != p.escrow {
                    if a.escrow.is_some() {
                        e.counts.observed_escrow_closes += 1;
                    }
                    if p.escrow.is_some() {
                        e.counts.observed_escrow_starts += 1;
                        let es = o.escrow.as_ref().unwrap();
                        e.counts.escrow_start_material += es.structure + es.reserve;
                    }
                    records.push(json!({"kind":"observed_escrow_transition","id":id,"form":e.form,
                        "before":a,"after":p,"escrow_now":o.escrow,
                        "note":"post-step observation, not exact debit; same-tick funding/death can be absent"}));
                }
            } else {
                ensure!(
                    o.born_tick == s.tick,
                    "new full ID lacks current birth boundary"
                );
                records
                    .push(json!({"kind":"first_observed_newborn","id":id,"form":e.form,"point":p}));
            }
            if p.mode == Mode::Resting {
                if e.rest.is_none() {
                    let origin = if new { "newborn" } else { "active_to_rest" };
                    e.rest = Some((s.tick, 0, false, origin));
                    if new {
                        e.counts.newborn_rest_entries += 1;
                    } else {
                        e.counts.active_to_rest_entries += 1;
                    }
                }
                e.rest.as_mut().unwrap().1 += 1;
            } else if let Some(r) = e.close_rest(s.tick, "mode_exit") {
                records.push(r);
            }
            let c = &mut e.counts;
            c.ticks += 1;
            c.fed += u64::from(p.fed);
            c.resting += u64::from(p.mode == Mode::Resting);
            c.feeding_mode += u64::from(p.mode == Mode::Feeding);
            c.seeking += u64::from(p.mode == Mode::Seeking);
            c.immature += u64::from(p.adult_fraction < 1.0);
            c.gestating += u64::from(p.escrow.is_some());
            c.reserve_zero += u64::from(p.reserve == 0.0);
            c.fed_reserve_zero += u64::from(p.fed && p.reserve == 0.0);
            c.reserve_below_growth_gate +=
                u64::from(p.reserve_fraction <= s.config.organism.growth_reserve_min);
            c.reserve_at_bud_gate += u64::from(p.reserve_fraction >= p.bud_reserve);
            c.energy_at_bud_gate += u64::from(p.energy_fraction >= p.bud_energy);
            c.both_bud_stock_gates +=
                u64::from(p.reserve_fraction >= p.bud_reserve && p.energy_fraction >= p.bud_energy);
            c.instant_hunger_below_off += u64::from(p.hunger < p.seek_off);
            c.memory_below_off += u64::from(p.memory < p.seek_off);
            c.memory_above_on += u64::from(p.memory > p.seek_on);
            c.post_energy_below_oxidation_threshold +=
                u64::from(p.energy_fraction < s.config.organism.oxidation_threshold);
            c.reserve_fraction_sum += p.reserve_fraction;
            c.energy_fraction_sum += p.energy_fraction;
            c.memory_sum += p.memory;
            e.min_memory = e.min_memory.min(p.memory);
            e.max_reserve_fraction = e.max_reserve_fraction.max(p.reserve_fraction);
            e.last = p;
        }
        self.last = s.tick;
        Ok(records)
    }
    pub fn sample(&self, s: &WorldState) -> Result<Value> {
        ensure!(s.tick == self.last, "sample tick mismatch");
        Ok(
            json!({"tick":s.tick,"organisms":s.organisms.iter().map(|(id,o)|
            json!({"id":id,"form":o.phenotype.form,"position":o.pos,"point":Point::of(o,s.tick)})).collect::<Vec<_>>()}),
        )
    }
    pub fn finish(&mut self) -> (Value, Vec<Value>) {
        let bouts = self
            .entries
            .values_mut()
            .filter_map(|e| e.close_rest(self.last, "horizon"))
            .collect();
        (
            json!({"opening_tick":self.opening,"closing_tick":self.last,"ticks":self.last-self.opening,
            "individuals":self.entries.values().collect::<Vec<_>>()}),
            bouts,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cubarium_core::{World, WorldConfig};
    #[test]
    fn observation_is_inert_and_repeated_ticks_refuse() {
        let mut a = World::new(WorldConfig::default()).unwrap();
        let mut b = World::from_state(a.state.clone()).unwrap();
        let mut obs = Observer::new(&a.state);
        assert!(obs.observe(&a.state).is_err());
        for _ in 0..30 {
            a.step();
            b.step();
            obs.observe(&a.state).unwrap();
            obs.sample(&a.state).unwrap();
        }
        assert_eq!(a.state, b.state);
        assert!(obs.observe(&a.state).is_err());
    }
    #[test]
    fn strict_rest_gate_and_hysteresis_are_not_interchanged() {
        let s = World::new(WorldConfig::default()).unwrap().state;
        let (_, o) = s.organisms.iter().next().unwrap();
        let mut p = Point::of(o, 0);
        p.mode = Mode::Feeding;
        p.memory = 0.1;
        p.hunger = 0.1;
        p.seek_off = 0.1;
        p.seek_on = 0.3;
        assert!(!p.next_resting());
        p.memory = 0.09;
        p.hunger = 0.09;
        assert!(p.next_resting());
        p.mode = Mode::Resting;
        p.memory = 0.3;
        p.hunger = 0.3;
        assert!(p.next_resting());
        p.memory = 0.31;
        p.hunger = 0.31;
        assert!(!p.next_resting());
    }
    #[test]
    fn newborn_single_tick_rest_is_not_active_rest() {
        let mut w = World::new(WorldConfig::default()).unwrap();
        let mut obs = Observer::new(&w.state);
        let mut child = w.state.organisms.iter().next().unwrap().1.clone();
        child.born_tick = 1;
        child.mode = Mode::Resting;
        child.hunger_memory = child.hunger();
        w.step();
        w.state.organisms.insert(child);
        obs.observe(&w.state).unwrap();
        w.step();
        let records = obs.observe(&w.state).unwrap();
        assert!(
            records
                .iter()
                .any(|r| r["entry"] == "newborn" && r["observed_resting_ticks"] == 1)
        );
    }
}
