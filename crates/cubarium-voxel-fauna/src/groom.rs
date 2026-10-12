//! SP-GROOM: picking a troop-mate's ectoparasite pays in prey and stops its drain.
//! No reward or score. A lost contact or any interruption discards the whole timer.

use crate::{Actions, Animal, Fauna, Founder, State};
use cubarium_voxel::VoxelView;
use cubarium_voxel_flora::{Flora, FloraView};
use serde::{Deserialize, Serialize};

/// Per-lineage grooming; a missing table is off, including in old named configs.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Grooming {
    /// Contact distance between body bases, metres; 0 disables grooming.
    /// Real primate grooming requires arm's reach, about one seedporter body length.
    pub groom_radius_m: f64,
    /// Continuous contact to locate and remove one parasite, seconds; 0 disables it.
    /// Bouts last minutes; finding and picking one item takes seconds to tens of seconds.
    pub groom_s: f64,
    /// Distance to notice an infested troop-mate, metres, also bounded by actual sight.
    /// Grooming partners are nearby troop members; this is a local visual approach.
    pub groom_seek_m: f64,
}

pub(crate) fn grooming_of(f: Founder) -> Grooming {
    if f == Founder::Seedporter {
        Grooming {
            groom_radius_m: 0.5,
            groom_s: 10.0,
            groom_seek_m: 4.0,
        }
    } else {
        Grooming::default()
    }
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub(crate) struct Contact {
    rider: u64,
    host: u64,
    ticks: u64,
}

fn eligible(fauna: &Fauna, a: &Animal) -> Option<Grooming> {
    let g = fauna.config.founder(a.founder?).grooming;
    if !(g.groom_radius_m > 0.0 && g.groom_s > 0.0)
        || a.state == State::Asleep
        || a.attached.is_some()
        || a.strike.is_some()
        || fauna.steer_of(a.id) == Some(crate::Steer::Flee)
        || fauna
            .percept(a.id)
            .is_some_and(|p| p.vigil.running(fauna.tick))
    {
        return None;
    }
    Some(g)
}

fn distance(a: &Animal, b: &Animal, ring: f64) -> f64 {
    let (dx, dz) = crate::pose::wrapped_delta(a.pose.x, a.pose.z, b.pose.x, b.pose.z, ring);
    dx.hypot(dz).hypot(a.pose.y_m - b.pose.y_m)
}

fn contact(fauna: &Fauna, a: &Animal, ring: f64, radius: f64) -> Option<(u64, u64)> {
    fauna
        .animals
        .iter()
        .filter_map(|r| {
            if r.founder != Some(Founder::Lanternjaw) {
                return None;
            }
            let host = fauna.view().animal(r.attached?.host)?;
            if host.id == a.id || host.founder != a.founder {
                return None;
            }
            let d = distance(a, host, ring);
            (d <= radius).then_some((d, host.id, r.id))
        })
        .min_by(|p, q| p.0.total_cmp(&q.0).then(p.1.cmp(&q.1)).then(p.2.cmp(&q.2)))
        .map(|(_, h, r)| (h, r))
}

/// Contact is an act every tick, including between controller samples.
pub(crate) fn holding(fauna: &Fauna, i: usize, view: &VoxelView<'_>) -> bool {
    let a = &fauna.animals[i];
    let Some(g) = eligible(fauna, a) else {
        return false;
    };
    let ring = f64::from(view.config.width) * view.config.voxel_m;
    contact(fauna, a, ring, g.groom_radius_m).is_some()
}

/// Reuse the existing directed walking action and target machinery, with fresh kin sight.
pub(crate) fn steer(
    fauna: &mut Fauna,
    i: usize,
    view: &VoxelView<'_>,
    fv: &FloraView<'_>,
    day: bool,
) -> Option<Actions> {
    let a = fauna.animals[i];
    let g = eligible(fauna, &a)?;
    let ring = f64::from(view.config.width) * view.config.voxel_m;
    if contact(fauna, &a, ring, g.groom_radius_m).is_some() {
        return Some(Actions::REST);
    }
    if a.emergency
        || a.bout.is_some_and(|b| b.kind == crate::BoutKind::Emergency)
        || !(g.groom_seek_m > 0.0)
    {
        return None;
    }
    let phys = fauna.config.founder(a.founder?);
    let seek = g.groom_seek_m.min(phys.senses.sight_range_m(day));
    let p = fauna.percept(a.id)?;
    let fresh = 2 * fauna.config.perception.stagger_ticks();
    let host = p
        .tracks()
        .iter()
        .filter(|t| {
            t.class == crate::perception::class::KIN
                && t.id != a.id
                && t.modes & crate::perception::SIGHT != 0
                && fauna.tick.saturating_sub(t.tick) <= fresh
        })
        .filter_map(|t| {
            let host = fauna.view().animal(t.id)?;
            if host.founder != a.founder {
                return None;
            }
            let d = distance(&a, host, ring);
            (d <= seek
                && fauna.animals.iter().any(|r| {
                    r.founder == Some(Founder::Lanternjaw)
                        && r.attached.is_some_and(|h| h.host == host.id)
                }))
            .then_some((d, host.id, host.site))
        })
        .min_by(|p, q| p.0.total_cmp(&q.0).then(p.1.cmp(&q.1)))?;
    let go = crate::step::groom_approach(fauna, i, view, fv, host, seek)?;
    fauna.walking_to(a.id, host.1, crate::Steer::Groom);
    Some(go)
}

/// After all hosts have moved and strikes landed, validate contact and book each removal.
/// Stable groomer id order means two groomers cannot eat the same rider.
pub(crate) fn step(fauna: &mut Fauna, view: &VoxelView<'_>, flora: &mut Flora) {
    let ring = f64::from(view.config.width) * view.config.voxel_m;
    let ids: Vec<_> = fauna
        .animals
        .iter()
        .filter(|a| {
            a.founder.is_some_and(|f| {
                let g = fauna.config.founder(f).grooming;
                g.groom_radius_m > 0.0 && g.groom_s > 0.0
            })
        })
        .map(|a| a.id)
        .collect();
    fauna
        .groom_contacts
        .retain(|id, _| ids.binary_search(id).is_ok());
    for id in ids {
        let Some(a) = fauna.view().animal(id).copied() else {
            continue;
        };
        let target = eligible(fauna, &a)
            .and_then(|g| contact(fauna, &a, ring, g.groom_radius_m).map(|t| (g, t)));
        let Some((g, (host, rider))) = target else {
            fauna.groom_contacts.remove(&id);
            continue;
        };
        let c = fauna.groom_contacts.entry(id).or_insert(Contact {
            host,
            rider,
            ticks: 0,
        });
        if (c.host, c.rider) != (host, rider) {
            *c = Contact {
                host,
                rider,
                ticks: 0,
            };
        }
        c.ticks = c.ticks.saturating_add(1);
        if c.ticks as f64 * crate::DT >= g.groom_s {
            fauna.groom_contacts.remove(&id);
            crate::step::groom_prey(fauna, flora, id, rider, view);
        }
    }
}
