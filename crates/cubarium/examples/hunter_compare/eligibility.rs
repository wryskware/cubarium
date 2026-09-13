//! End-of-step reproductive opportunity diagnostics, not mutation-site evidence.
//! A gate can change during physiology; these observations never claim an actual
//! funding attempt. All durations use member-ticks, not world-ticks.

use cubarium_core::{WorldState, hunter};
use serde_json::{Value, json};

pub const BLOCKERS: [&str; 11] = [
    "escrow",
    "gut",
    "target",
    "hunting",
    "juvenile",
    "reserve_gate",
    "energy_gate",
    "age",
    "cooldown",
    "funding_material",
    "funding_energy",
];

fn blocked(
    state: &WorldState,
    m: &hunter::HunterMember,
    o: &cubarium_core::organism::Organism,
    profile: &hunter::FixedHunterProfile,
) -> [bool; 11] {
    let cfg = &state.config.organism;
    let funding_m = cfg.child_structure_fraction * o.phenotype.structure_adult
        + cfg.child_reserve_fraction * o.phenotype.reserve_max;
    let funding_e = cfg.build_cost * cfg.child_structure_fraction * o.phenotype.structure_adult
        + cfg.child_energy_fraction * o.phenotype.energy_max;
    [
        o.escrow.is_some(),
        m.carrying(),
        m.target.is_some(),
        m.phase.hunting(),
        o.structure < o.phenotype.structure_adult - hunter::TOLERANCE,
        o.reserve < profile.reproduce_reserve_fraction * o.phenotype.reserve_max,
        o.energy < profile.reproduce_energy_fraction * o.phenotype.energy_max,
        o.age_ticks(state.tick) as f64 * cubarium_core::DT < profile.reproduce_min_age_seconds,
        state.tick < m.next_reproduction_tick,
        o.reserve < funding_m,
        o.energy < funding_e,
    ]
}

pub fn members(state: &WorldState) -> Vec<Value> {
    let Some(profile) = state.hunters.profile() else {
        return Vec::new();
    };
    let cfg = &state.config.organism;
    state.hunters.members.iter().map(|m| {
        let o = state.organisms.get(m.id).expect("validated hunter membership");
        let child_s = cfg.child_structure_fraction * o.phenotype.structure_adult;
        let child_r = cfg.child_reserve_fraction * o.phenotype.reserve_max;
        let child_e = cfg.child_energy_fraction * o.phenotype.energy_max;
        let funding_m = child_s + child_r;
        let funding_e = cfg.build_cost * child_s + child_e;
        let age = o.age_ticks(state.tick) as f64 * cubarium_core::DT;
        let blockers = blocked(state,m,o,profile);
        json!({"id":m.id,"parent":o.parent,"age_seconds":age,
            "structure":o.structure,"adult_structure":o.phenotype.structure_adult,
            "reserve":o.reserve,"reserve_max":o.phenotype.reserve_max,
            "energy":o.energy,"energy_max":o.phenotype.energy_max,
            "reserve_gate":profile.reproduce_reserve_fraction * o.phenotype.reserve_max,
            "energy_gate":profile.reproduce_energy_fraction * o.phenotype.energy_max,
            "funding_material":funding_m,"funding_energy":funding_e,
            "gut_material":m.gut_material,"gut_energy":m.gut_energy,
            "phase":m.phase,"escrow":o.escrow,
            "local_gate_open":!blockers[..9].iter().any(|b|*b),
            "gate_and_stocks_open":!blockers.iter().any(|b|*b),
            "blockers":BLOCKERS.iter().zip(blockers).filter_map(|(name,b)|b.then_some(*name)).collect::<Vec<_>>()})
    }).collect()
}

#[derive(Default)]
pub struct Eligibility {
    member_samples: u64,
    gate_open: u64,
    gate_and_stocks_open: u64,
    age_and_size_ready: u64,
    ready_reserve_open: u64,
    ready_energy_open: u64,
    ready_both_open: u64,
    blockers: [u64; 11],
    max_reserve_fraction: Option<f64>,
    max_energy_fraction: Option<f64>,
}

impl Eligibility {
    pub fn observe(&mut self, state: &WorldState) {
        let Some(profile) = state.hunters.profile() else {
            return;
        };
        // No JSON allocation on the per-tick path; the sparse census uses the
        // same classification. No per-ID lifetime history is retained.
        for m in &state.hunters.members {
            let o = state
                .organisms
                .get(m.id)
                .expect("validated hunter membership");
            let blockers = blocked(state, m, o, profile);
            self.member_samples += 1;
            self.gate_open += u64::from(!blockers[..9].iter().any(|b| *b));
            self.gate_and_stocks_open += u64::from(!blockers.iter().any(|b| *b));
            // Age and adult structure are distinct from stocks, quiet behavior,
            // and an actual funding. Keep this conditional denominator explicit.
            if !blockers[4] && !blockers[7] {
                self.age_and_size_ready += 1;
                self.ready_reserve_open += u64::from(!blockers[5]);
                self.ready_energy_open += u64::from(!blockers[6]);
                self.ready_both_open += u64::from(!blockers[5] && !blockers[6]);
            }
            for (i, b) in blockers.iter().enumerate() {
                self.blockers[i] += u64::from(*b);
            }
            for (peak, stock, cap) in [
                (
                    &mut self.max_reserve_fraction,
                    o.reserve,
                    o.phenotype.reserve_max,
                ),
                (
                    &mut self.max_energy_fraction,
                    o.energy,
                    o.phenotype.energy_max,
                ),
            ] {
                if cap > 0.0 {
                    let fraction = stock / cap;
                    *peak = Some(peak.map_or(fraction, |old| old.max(fraction)));
                }
            }
        }
    }

    pub fn summary(&self) -> Value {
        json!({"basis":"end-of-step member-ticks; simultaneous blockers overlap; not mutation-site attempts; cap and queued births excluded",
            "member_ticks":self.member_samples,"local_gate_open_member_ticks":self.gate_open,
            "gate_and_stocks_open_member_ticks":self.gate_and_stocks_open,
            "age_and_size_ready_member_ticks":self.age_and_size_ready,
            "age_and_size_ready_reserve_gate_open_member_ticks":self.ready_reserve_open,
            "age_and_size_ready_energy_gate_open_member_ticks":self.ready_energy_open,
            "age_and_size_ready_both_stock_gates_open_member_ticks":self.ready_both_open,
            "blocker_member_ticks":BLOCKERS.iter().zip(self.blockers).map(|(k,v)|(k.to_string(),json!(v))).collect::<serde_json::Map<_,_>>(),
            "max_reserve_fraction":self.max_reserve_fraction,"max_energy_fraction":self.max_energy_fraction})
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cubarium_core::snapshot::state_hash;
    use cubarium_core::{FixedHunterProfile, HunterTarget, World, WorldConfig};

    fn fixture() -> World {
        let mut world = World::new(WorldConfig::default()).unwrap();
        let mut profile = FixedHunterProfile::lanternjaw_trial(world.config());
        profile.reproduce_min_age_seconds = 0.0;
        world
            .start_hunter_trial(
                profile,
                HunterTarget {
                    face: 4,
                    u: 22.0,
                    v: 32.0,
                },
            )
            .unwrap();
        let id = world.hunters().members[0].id;
        let o = world.state.organisms.get_mut(id).unwrap();
        o.reserve = o.phenotype.reserve_max;
        o.energy = o.phenotype.energy_max;
        world
    }

    #[test]
    fn boundary_gate_matches_core_but_reports_material_stock_obstacle_separately() {
        let mut world = fixture();
        assert_eq!(members(&world.state)[0]["local_gate_open"], true);
        assert_eq!(members(&world.state)[0]["gate_and_stocks_open"], true);
        world.state.config.organism.child_reserve_fraction = 10.0; // Diagnostic fixture, not an admitted config.
        let row = &members(&world.state)[0];
        assert_eq!(row["local_gate_open"], true);
        assert_eq!(row["gate_and_stocks_open"], false);
        assert_eq!(row["blockers"], json!(["funding_material"]));
        let m = &world.hunters().members[0];
        let o = world.state.organisms.get(m.id).unwrap();
        assert!(hunter::may_reproduce(
            world.hunters().profile().unwrap(),
            o,
            m,
            world.tick(),
            cubarium_core::DT
        ));
    }

    #[test]
    fn diagnostics_do_not_consume_rng_and_zero_members_do_not_look_eligible() {
        let empty = World::new(WorldConfig::default()).unwrap();
        let mut stats = Eligibility::default();
        stats.observe(&empty.state);
        assert_eq!(stats.summary()["member_ticks"], 0);
        assert_eq!(stats.summary()["max_reserve_fraction"], Value::Null);
        let mut world = fixture();
        let before = state_hash(&world.state);
        for _ in 0..120 {
            stats.observe(&world.state);
        }
        assert_eq!(before, state_hash(&world.state));
        assert_eq!(stats.summary()["member_ticks"], 120);
        let mut copy = World::from_state(world.state.clone()).unwrap();
        for _ in 0..400 {
            world.step();
            copy.step();
            stats.observe(&world.state);
        }
        assert_eq!(state_hash(&world.state), state_hash(&copy.state));
    }

    #[test]
    fn target_gut_and_cooldown_are_independent_obstacles() {
        let mut world = fixture();
        let id = world.hunters().members[0].id;
        world.state.hunters.members[0].target = Some(id);
        world.state.hunters.members[0].gut_material = 0.1;
        world.state.hunters.members[0].next_reproduction_tick = 1;
        let row = &members(&world.state)[0];
        assert_eq!(row["local_gate_open"], false);
        assert_eq!(row["blockers"], json!(["gut", "target", "cooldown"]));
        let mut stats = Eligibility::default();
        stats.observe(&world.state);
        assert_eq!(stats.summary()["blocker_member_ticks"]["gut"], 1);
        assert_eq!(stats.summary()["blocker_member_ticks"]["target"], 1);
        assert_eq!(stats.summary()["age_and_size_ready_member_ticks"], 1);
        assert_eq!(
            stats.summary()["age_and_size_ready_both_stock_gates_open_member_ticks"],
            1
        );
        assert_eq!(stats.summary()["local_gate_open_member_ticks"], 0);
    }

    #[test]
    fn youthful_stock_success_does_not_count_as_reproductive_age_opportunity() {
        let mut world = World::new(WorldConfig::default()).unwrap();
        let profile = FixedHunterProfile::lanternjaw_trial(world.config());
        world
            .start_hunter_trial(
                profile,
                HunterTarget {
                    face: 4,
                    u: 22.0,
                    v: 32.0,
                },
            )
            .unwrap();
        let id = world.hunters().members[0].id;
        let o = world.state.organisms.get_mut(id).unwrap();
        o.reserve = o.phenotype.reserve_max;
        o.energy = o.phenotype.energy_max;
        let mut stats = Eligibility::default();
        stats.observe(&world.state);
        assert_eq!(stats.summary()["age_and_size_ready_member_ticks"], 0);
        assert_eq!(
            stats.summary()["age_and_size_ready_both_stock_gates_open_member_ticks"],
            0
        );
        world.state.tick = 24000; // Boundary fixture: exactly1200s, not a balance run.
        stats.observe(&world.state);
        assert_eq!(stats.summary()["age_and_size_ready_member_ticks"], 1);
        assert_eq!(
            stats.summary()["age_and_size_ready_both_stock_gates_open_member_ticks"],
            1
        );
    }
}
