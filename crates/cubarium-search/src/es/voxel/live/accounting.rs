//! Absolute persistence and exact recruitment, independent of paired scoring.

use cubarium_voxel_fauna::{Fauna, Founder};
use cubarium_voxel_flora::{FloraConfig, Species};
use cubarium_voxel_sim::Sim;
use serde::{Deserialize, Serialize};

/// The declared task, captured before placement or burn-in can reduce it.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct RequiredRoster {
    pub animals: [bool; Founder::COUNT],
    pub plants: [bool; Species::COUNT],
    pub latticevine: bool,
}

impl RequiredRoster {
    pub fn declared(founders: &[usize; Founder::COUNT], flora: &FloraConfig) -> Self {
        Self {
            animals: founders.map(|n| n > 0),
            plants: Species::ALL.map(|s| {
                let p = flora.species(s);
                p.founder_per_m2 > 0.0 && p.founder_cap > 0
            }),
            latticevine: flora.latticevine.founders > 0,
        }
    }

    /// Loaded starts can contain deliberately constructed bodies/banks beyond the
    /// founding table. They add requirements; absence never removes a declared one.
    pub fn include_present(&mut self, p: &Presence) {
        for i in 0..Founder::COUNT {
            self.animals[i] |= p.bodies[i] + p.eggs[i] > 0;
        }
        for i in 0..Species::COUNT {
            self.plants[i] |= p.active_plants[i] + p.dormant_plants[i] + p.seed_banks[i] > 0;
        }
        self.latticevine |= p.vines > 0 || p.vine_banks > 0;
    }

    pub fn losses(&self, p: &Presence) -> Vec<String> {
        let mut out = Vec::new();
        for f in Founder::ALL {
            if self.animals[f.index()] && p.bodies[f.index()] + p.eggs[f.index()] == 0 {
                out.push(f.name().into());
            }
        }
        for s in Species::ALL {
            let i = s.index();
            if self.plants[i] && p.active_plants[i] + p.dormant_plants[i] + p.seed_banks[i] == 0 {
                out.push(s.name().into());
            }
        }
        if self.latticevine && p.vines + p.vine_banks == 0 {
            out.push("latticevine".into());
        }
        out
    }
}

/// Living stands, cured stands and viable seed cohorts are distinct readouts.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Presence {
    pub bodies: [usize; Founder::COUNT],
    pub eggs: [usize; Founder::COUNT],
    pub active_plants: [usize; Species::COUNT],
    pub dormant_plants: [usize; Species::COUNT],
    pub seed_banks: [usize; Species::COUNT],
    pub vines: usize,
    pub vine_banks: usize,
}

impl Presence {
    pub fn of(sim: &Sim) -> Self {
        let mut p = Self::default();
        let fa = sim.fauna().view();
        for a in fa.animals {
            if let Some(f) = a.founder {
                p.bodies[f.index()] += 1;
            }
        }
        for f in Founder::ALL {
            p.eggs[f.index()] = fa.eggs_by_founder(f) as usize;
        }
        let fl = sim.flora().view();
        for s in fl.stands {
            if s.cured {
                p.dormant_plants[s.species.index()] += 1;
            } else {
                p.active_plants[s.species.index()] += 1;
            }
        }
        for g in fl.ground {
            for c in &g.seeds {
                let sc = fl.config.species(c.species);
                let package = if sc.propagule_split[0] > 0.0 {
                    f64::from(sc.seed_mass.max(1)) * sc.alive_min / sc.propagule_split[0]
                } else {
                    0.0
                };
                // Match the seed bank's whole-package rounding; mineral is carried,
                // not an establishment gate.
                if package > 0.0
                    && (c.organic / package).round() >= 1.0
                    && fl.tick.saturating_sub(c.bin_start_tick) as f64 * cubarium_voxel::DT
                        <= sc.seed_max_age_s
                {
                    p.seed_banks[c.species.index()] += 1;
                }
            }
            if g.vine_seeds.as_ref().is_some_and(|c| {
                let vc = &fl.config.latticevine;
                let package = cubarium_voxel_flora::vine_seed_package(vc);
                c.organic >= package - 1e-15
                    && fl.tick.saturating_sub(c.landed_tick) as f64 * cubarium_voxel::DT
                        <= vc.seed_max_age_s
            }) {
                p.vine_banks += 1;
            }
        }
        p.vines = fl.cover.vines().len();
        p
    }
}

/// Cumulative events; interval differences survive young dying between observations.
#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize)]
pub struct Recruitment {
    pub births: [u64; Founder::COUNT],
    pub live_births: [u64; Founder::COUNT],
    pub hatchings: [u64; Founder::COUNT],
    pub eggs_laid: [u64; Founder::COUNT],
    pub maturations: [u64; Founder::COUNT],
}

impl Recruitment {
    pub fn of(fauna: &Fauna) -> Self {
        let l = fauna.view().ledger;
        Self {
            births: fauna.born_by_founder(),
            hatchings: l.eggs_hatched_by_founder,
            eggs_laid: l.eggs_laid_by_founder,
            maturations: l.matured_by_founder,
            ..Self::default()
        }
    }

    pub fn since(&self, start: &Self) -> Self {
        let diff = |a: &[u64; Founder::COUNT], b: &[u64; Founder::COUNT]| {
            std::array::from_fn(|i| a[i].saturating_sub(b[i]))
        };
        let births = diff(&self.births, &start.births);
        let hatchings = diff(&self.hatchings, &start.hatchings);
        Self {
            births,
            hatchings,
            live_births: diff(&births, &hatchings),
            eggs_laid: diff(&self.eggs_laid, &start.eggs_laid),
            maturations: diff(&self.maturations, &start.maturations),
        }
    }
}

/// Keep historical losses even if a later introduced cohort restores presence.
pub fn retain_losses(lost: &mut Vec<String>, new: Vec<String>) {
    for name in new {
        if !lost.contains(&name) {
            lost.push(name);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::es::voxel::live::search::{Band, BandSpec, Trace, penalty};

    #[test]
    fn absent_founders_are_required_and_banks_are_separate_presence() {
        let mut roster = RequiredRoster::default();
        roster.animals[Founder::Blind.index()] = true;
        roster.plants[Species::Springturf.index()] = true;
        let mut p = Presence::default();
        assert_eq!(roster.losses(&p).len(), 2);
        p.eggs[Founder::Blind.index()] = 2;
        p.dormant_plants[Species::Springturf.index()] = 1;
        assert!(roster.losses(&p).is_empty());
        p.dormant_plants[Species::Springturf.index()] = 0;
        p.seed_banks[Species::Springturf.index()] = 1;
        assert!(roster.losses(&p).is_empty());
        let mut lost = vec![];
        p.eggs[Founder::Blind.index()] = 0;
        retain_losses(&mut lost, roster.losses(&p));
        p.bodies[Founder::Blind.index()] = 1;
        retain_losses(&mut lost, roster.losses(&p));
        assert_eq!(lost, [Founder::Blind.name()]);
    }

    #[test]
    fn absolute_term_survives_readout_exclusions_and_can_be_disabled() {
        let spec = BandSpec::default()
            .with_targets(
                &"[targets]\nversion=3\nreadout_only=['littershredder']\nrequired_loss_weight=73\n"
                    .parse()
                    .unwrap(),
            )
            .unwrap();
        let band = Band::for_founding(1, false, &spec);
        let mut t = Trace {
            lost_required: 2,
            required_loss_weight: spec.required_loss_weight,
            counts_readout: true,
            ..Trace::default()
        };
        let p = penalty(&t, &band);
        assert_eq!(p.absolute_persist, 2.0 * spec.required_loss_weight);
        assert_eq!(p.total, p.absolute_persist + p.lively);
        t.required_loss_weight = 0.0;
        assert_eq!(penalty(&t, &band).absolute_persist, 0.0);
        assert_eq!(t.lost_required, 2, "diagnostics keep losses visible");
    }

    #[test]
    fn exact_events_do_not_depend_on_surviving_bodies() {
        let i = Founder::Blind.index();
        let mut a = Recruitment::default();
        a.births[i] = 20;
        a.hatchings[i] = 9;
        let mut b = a;
        b.births[i] += 5;
        b.hatchings[i] += 2;
        b.eggs_laid[i] += 7;
        b.maturations[i] += 1;
        let d = b.since(&a);
        assert_eq!(
            (
                d.births[i],
                d.live_births[i],
                d.hatchings[i],
                d.eggs_laid[i],
                d.maturations[i]
            ),
            (5, 3, 2, 7, 1)
        );
    }

    #[test]
    fn provenance_reports_installed_and_frozen_values() {
        let start = super::super::ecology::EcologyParams::defaults().layers(0.25);
        let mut candidate = start.clone();
        let f = Founder::Blind.index();
        candidate.fauna.founders[f].core.maintenance_per_s *= 1.5;
        candidate.fauna.founders[f].core.reserve_cap *= 2.0;
        let applied = super::super::start::rate_override(&start, &candidate).unwrap();
        let rows = super::super::start::parameter_changes(&start, &candidate, &applied.installed);
        assert!(rows.iter().any(|r| r.path.ends_with("maintenance_per_s") && r.candidate == r.installed && r.start != r.installed));
        assert!(rows.iter().any(|r| r.path.ends_with("reserve_cap") && r.start == r.installed && r.candidate != r.installed));
        let json = serde_json::to_value(&rows).unwrap();
        assert!(json[0].get("installed").is_some());
    }
}
