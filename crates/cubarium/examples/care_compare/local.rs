//! Fixed, paired observation regions. Never selects a more responsive site after care.
use anyhow::{Context, Result, ensure};
use cubarium_core::{CareTarget, OrganismId, WorldState, organism::Mode};
use cubarium_surface::{CellId, FieldGraph, cell_of};
use serde::Serialize;
use serde_json::{Value, json};
use std::collections::BTreeSet;

pub const HOPS: usize = 3;

#[derive(Clone, Copy, Default, Serialize)]
struct Counts {
    population: u64,
    resting: u64,
    seeking: u64,
    feeding_mode: u64,
    fed_this_tick: u64,
    gestating: u64,
    by_form: [u64; 8],
}

impl Counts {
    fn add(&mut self, other: Self) {
        self.population += other.population;
        self.resting += other.resting;
        self.seeking += other.seeking;
        self.feeding_mode += other.feeding_mode;
        self.fed_this_tick += other.fed_this_tick;
        self.gestating += other.gestating;
        for (a, b) in self.by_form.iter_mut().zip(other.by_form) {
            *a += b;
        }
    }
}

struct Region {
    target: CareTarget,
    cells: BTreeSet<CellId>,
    totals: Counts,
    cohort: Option<BTreeSet<OrganismId>>,
    cohort_opening: Value,
    cohort_totals: Counts,
}

impl Region {
    fn counts(&self, state: &WorldState) -> (Counts, usize) {
        self.counts_selected(state, false)
    }

    fn counts_selected(&self, state: &WorldState, fixed_cohort: bool) -> (Counts, usize) {
        let mut counts = Counts::default();
        let mut occupied = BTreeSet::new();
        for (id, organism) in state.organisms.iter() {
            let cell = cell_of(&organism.pos);
            let selected = if fixed_cohort {
                self.cohort
                    .as_ref()
                    .is_some_and(|cohort| cohort.contains(&id))
            } else {
                self.cells.contains(&cell)
            };
            if !selected {
                continue;
            }
            occupied.insert(cell);
            counts.population += 1;
            counts.by_form[usize::from(organism.phenotype.form).min(7)] += 1;
            match organism.mode {
                Mode::Resting => counts.resting += 1,
                Mode::Seeking => counts.seeking += 1,
                Mode::Feeding => counts.feeding_mode += 1,
            }
            counts.fed_this_tick += u64::from(organism.fed_this_tick);
            counts.gestating += u64::from(organism.escrow.is_some());
        }
        (counts, occupied.len())
    }

    fn sample(&self, state: &WorldState) -> Value {
        let (instant, occupied) = self.counts(state);
        let sum = |values: &[f64]| self.cells.iter().map(|c| values[c.index()]).sum::<f64>();
        json!({"target":self.target,"cells":self.cells.iter().map(|c| c.index()).collect::<Vec<_>>(),
            "instant":instant,"occupied_cells":occupied,"cumulative_member_ticks":self.totals,
            "first_pulse_cohort":self.cohort.as_ref().map(|cohort| json!({
                "opening_count":cohort.len(),"living_anywhere":self.counts_selected(state, true).0,
                "cumulative_member_ticks_anywhere":self.cohort_totals})),
            "nutrient":sum(&state.fields.n),"producer":sum(&state.fields.p),
            "fruit":sum(&state.fields.f),"litter":sum(&state.fields.d),
            "litter_energy":sum(&state.fields.de),"water":sum(&state.fields.w)})
    }
}

pub struct LocalObserver {
    opening: u64,
    last_tick: u64,
    regions: Vec<Region>,
}

impl LocalObserver {
    pub fn new(state: &WorldState, targets: &[CareTarget]) -> Result<Self> {
        let graph = FieldGraph::new();
        let mut regions = Vec::new();
        for &target in targets {
            let mut cells = BTreeSet::from([target.resolve().context("invalid local target")?]);
            for _ in 0..HOPS {
                let neighbors: Vec<_> = cells
                    .iter()
                    .flat_map(|c| graph.neighbors(*c).iter().flatten().copied())
                    .collect();
                cells.extend(neighbors);
            }
            regions.push(Region {
                target,
                cells,
                totals: Counts::default(),
                cohort: None,
                cohort_opening: Value::Null,
                cohort_totals: Counts::default(),
            });
        }
        Ok(Self {
            opening: state.tick,
            last_tick: state.tick,
            regions,
        })
    }

    /// Counts every completed tick, even when the report samples less frequently.
    pub fn observe(&mut self, state: &WorldState) -> Result<()> {
        ensure!(
            self.last_tick.checked_add(1) == Some(state.tick),
            "local observer skipped/repeated a tick"
        );
        for region in &mut self.regions {
            region.totals.add(region.counts(state).0);
            region
                .cohort_totals
                .add(region.counts_selected(state, true).0);
        }
        self.last_tick = state.tick;
        Ok(())
    }

    /// Freeze IDs BEFORE the first intervention in both arms. Follow those exact IDs
    /// anywhere thereafter; descendants and reused slots cannot replace lost members.
    pub fn mark_first_pulse(&mut self, state: &WorldState) -> Result<()> {
        ensure!(
            state.tick == self.last_tick,
            "pulse cohort is not at its observed boundary"
        );
        ensure!(
            self.regions.iter().all(|r| r.cohort.is_none()),
            "pulse cohort already frozen"
        );
        for region in &mut self.regions {
            let members: Vec<_> = state
                .organisms
                .iter()
                .filter(|(_, o)| region.cells.contains(&cell_of(&o.pos)))
                .collect();
            region.cohort = Some(members.iter().map(|(id, _)| *id).collect());
            region.cohort_opening = json!({"tick":state.tick,"target":region.target,
                "members":members.iter().map(|(id,o)| json!({"id":id,"position":o.pos,
                    "mode":o.mode,"form":o.phenotype.form,"diet":o.phenotype.diet,
                    "hunger_memory":o.hunger_memory,"reserve_headroom":o.phenotype.reserve_max-o.reserve,
                    "sense_radius":o.phenotype.sense_radius})).collect::<Vec<_>>()});
        }
        Ok(())
    }

    pub fn cohorts(&self) -> Value {
        json!(
            self.regions
                .iter()
                .map(|r| &r.cohort_opening)
                .collect::<Vec<_>>()
        )
    }

    pub fn sample(&self, state: &WorldState) -> Result<Value> {
        ensure!(
            state.tick == self.last_tick,
            "local sample is not at its observed tick"
        );
        Ok(json!({"tick":state.tick,"elapsed":state.tick-self.opening,
            "regions":self.regions.iter().map(|r| r.sample(state)).collect::<Vec<_>>()}))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cubarium_core::{World, WorldConfig};
    use cubarium_surface::SurfacePoint;
    use cube_proto::Face;

    #[test]
    fn neighborhoods_cross_the_seam_and_never_duplicate_a_cell() {
        let state = World::new(WorldConfig::default()).unwrap().state;
        let observer = LocalObserver::new(
            &state,
            &[
                CareTarget {
                    face: 0,
                    u: 63.5,
                    v: 32.0,
                },
                CareTarget {
                    face: 0,
                    u: 32.0,
                    v: 63.5,
                },
            ],
        )
        .unwrap();
        assert_eq!(observer.regions[0].cells.len(), 25);
        assert!(
            observer.regions[0]
                .cells
                .iter()
                .any(|c| c.face() == Face::Right)
        );
        assert_eq!(observer.regions[1].cells.len(), 16);
        assert!(
            observer.regions[1]
                .cells
                .iter()
                .all(|c| c.face() == Face::Front)
        );
    }

    #[test]
    fn member_ticks_count_real_intake_separately_from_feeding_mode() {
        let mut state = World::new(WorldConfig::default()).unwrap().state;
        let target = CareTarget {
            face: 0,
            u: 32.0,
            v: 32.0,
        };
        let ids: Vec<_> = state.organisms.iter().map(|(id, _)| id).collect();
        for id in &ids {
            state.organisms.get_mut(*id).unwrap().pos = SurfacePoint::new(Face::Back, 32.0, 32.0);
        }
        for (id, mode, fed) in [
            (ids[0], Mode::Feeding, false),
            (ids[1], Mode::Resting, true),
        ] {
            let o = state.organisms.get_mut(id).unwrap();
            o.pos = target.resolve().unwrap().center();
            o.mode = mode;
            o.fed_this_tick = fed;
        }
        let hash = cubarium_core::snapshot::state_hash(&state);
        let mut observer = LocalObserver::new(&state, &[target]).unwrap();
        let start = observer.sample(&state).unwrap();
        assert_eq!(start["regions"][0]["instant"]["population"], 2);
        assert_eq!(
            start["regions"][0]["cumulative_member_ticks"]["population"],
            0
        );
        assert_eq!(hash, cubarium_core::snapshot::state_hash(&state));
        observer.mark_first_pulse(&state).unwrap();
        assert!(observer.mark_first_pulse(&state).is_err());
        for _ in 0..3 {
            state.tick += 1;
            observer.observe(&state).unwrap();
        }
        let sample = observer.sample(&state).unwrap();
        let totals = &sample["regions"][0]["cumulative_member_ticks"];
        assert_eq!(totals["population"], 6);
        assert_eq!(totals["feeding_mode"], 3);
        assert_eq!(totals["fed_this_tick"], 3);
        assert_eq!(totals["resting"], 3);
        // Moving away does not erase a fixed cohort member or add the replacement.
        state.organisms.get_mut(ids[0]).unwrap().pos = SurfacePoint::new(Face::Back, 32.0, 32.0);
        state.organisms.get_mut(ids[2]).unwrap().pos = target.resolve().unwrap().center();
        state.tick += 1;
        observer.observe(&state).unwrap();
        let cohort = observer.sample(&state).unwrap()["regions"][0]["first_pulse_cohort"].clone();
        assert_eq!(cohort["opening_count"], 2);
        assert_eq!(cohort["living_anywhere"]["population"], 2);
        assert_eq!(cohort["cumulative_member_ticks_anywhere"]["population"], 8);
        assert!(observer.observe(&state).is_err());
        state.tick += 2;
        assert!(observer.observe(&state).is_err());
        assert!(observer.sample(&state).is_err());
    }
}
