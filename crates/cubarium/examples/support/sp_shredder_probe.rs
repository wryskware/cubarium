//! Opt-in encounter diagnostics for a checkpoint probe; no simulation changes.
use cubarium_voxel_fauna::{Departure, Founder, MobilityMode, perception};
use cubarium_voxel_sim::Sim;
use std::collections::BTreeSet;

#[derive(Default)]
pub struct Probe {
    samples: u64,
    ground: u64,
    hungry_ground: u64,
    seen: BTreeSet<(u64, u64)>,
    ground_seen: BTreeSet<(u64, u64)>,
    profitable: BTreeSet<(u64, u64)>,
    kills0: u64,
}

impl Probe {
    pub fn new(sim: &Sim) -> Option<Self> {
        std::env::var_os("SP_SHREDDER_PROBE").map(|_| Self {
            kills0: sim.fauna().view().ledger.departed_founder(
                Founder::Blind,
                Departure::Killed {
                    by: Founder::Seedporter,
                },
            ),
            ..Self::default()
        })
    }

    /// Sample once a simulated second. Pair counts are unique over this run;
    /// sight records retain the perception system's ordinary memory.
    pub fn sample(&mut self, sim: &Sim) {
        let fauna = sim.fauna();
        let view = fauna.view();
        for a in view
            .animals
            .iter()
            .filter(|a| a.founder == Some(Founder::Seedporter))
        {
            self.samples += 1;
            let ground = a.mobility.mode == MobilityMode::Ground;
            self.ground += u64::from(ground);
            self.hungry_ground += u64::from(ground && a.eating);
            if let Some(p) = fauna.percept(a.id) {
                for t in p.tracks().iter().filter(|t| {
                    t.known
                        && usize::from(t.lineage) == Founder::Blind.index()
                        && t.modes & perception::SIGHT != 0
                }) {
                    self.seen.insert((a.id, t.id));
                    if ground {
                        self.ground_seen.insert((a.id, t.id));
                        if view
                            .animals
                            .iter()
                            .find(|b| b.id == t.id)
                            .is_some_and(|b| cubarium_voxel_fauna::is_prey_of(fauna.config(), a, b))
                        {
                            self.profitable.insert((a.id, t.id));
                        }
                    }
                }
            }
        }
    }

    pub fn report(&self, sim: &Sim) {
        let kills = sim.fauna().view().ledger.departed_founder(
            Founder::Blind,
            Departure::Killed {
                by: Founder::Seedporter,
            },
        ) - self.kills0;
        eprintln!(
            "sp_shredder_probe: body_seconds={} ground_seconds={} hungry_ground_seconds={} seen_pairs={} ground_seen_pairs={} ground_profitable_pairs={} catches={kills}",
            self.samples,
            self.ground,
            self.hungry_ground,
            self.seen.len(),
            self.ground_seen.len(),
            self.profitable.len()
        );
    }
}
