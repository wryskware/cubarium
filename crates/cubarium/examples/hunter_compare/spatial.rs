//! Read-only bridge from genuine settlement events to paired spatial observations.
//! The counter map retains only living full IDs; it is never fed back into World.

use super::recovery;
use anyhow::{Context, Result, ensure};
use cubarium_core::{
    HunterEvent, OrganismId, WorldState,
    hunter::{AttemptOutcome, body_point, measure_contact},
};
use cubarium_surface::{
    CELL_COUNT, CellId, ChartImage, Face, FieldGraph, MAX_SEAMS, cell_of, chart_images,
};
use std::collections::{BTreeMap, BTreeSet};

pub const ON_ARMS: [usize; 2] = [3, 5];

/// Radius three in the existing surface field graph, including the center.
pub fn neighborhoods() -> Vec<Vec<usize>> {
    let graph = FieldGraph::new();
    CellId::all()
        .map(|center| {
            let mut seen = BTreeSet::from([center]);
            let mut frontier = vec![center];
            for _ in 0..3 {
                let mut next = Vec::new();
                for cell in frontier {
                    for &neighbor in graph.neighbors(cell).iter().flatten() {
                        if seen.insert(neighbor) {
                            next.push(neighbor);
                        }
                    }
                }
                frontier = next;
            }
            seen.into_iter().map(CellId::index).collect()
        })
        .collect()
}

pub fn counts(states: [&WorldState; recovery::ARMS]) -> recovery::Counts {
    states.map(|state| {
        let mut counts = vec![0; CELL_COUNT];
        for (id, organism) in state.organisms.iter() {
            if !state.hunters.contains(id) {
                counts[cell_of(&organism.pos).index()] += 1;
            }
        }
        counts
    })
}

/// Immediate END-OF-STEP occupancy by form in the same cells in all six arms.
/// This is not the removed prey's inventory or the instant just before removal.
pub fn local_forms(
    states: [&WorldState; recovery::ARMS],
    cells: &[usize],
) -> [[u32; 8]; recovery::ARMS] {
    states.map(|state| {
        let mut counts = [0; 8];
        for (id, organism) in state.organisms.iter() {
            if !state.hunters.contains(id)
                && cells.binary_search(&cell_of(&organism.pos).index()).is_ok()
            {
                counts[usize::from(organism.phenotype.form).min(7)] += 1;
            }
        }
        counts
    })
}

#[derive(Clone)]
pub struct CaptureAudit {
    last_settled: BTreeMap<OrganismId, u64>,
    images: [Vec<ChartImage>; 5],
}

#[cfg(test)]
mod tests {
    use super::*;
    use cubarium_core::{FixedHunterProfile, HunterTarget, World, WorldConfig};
    use cubarium_surface::{Face, SurfacePoint, Vec2};

    /// Certain-capture, stationary-prey fixture, not a balance trial. The real
    /// controller pays for and settles its strike; no events are manufactured.
    fn capture_fixture() -> (WorldState, Vec<HunterEvent>, CaptureAudit) {
        let mut cfg = WorldConfig::default();
        cfg.founders.kinds.clear();
        cfg.founders.count = 1;
        let mut world = World::new(cfg).unwrap();
        {
            let prey = world.state.organisms.iter_mut().next().unwrap().1;
            prey.pos = SurfacePoint::new(Face::Top, 31.0, 20.0);
            prey.phenotype.speed_max = 0.0;
        }
        let mut profile = FixedHunterProfile::lanternjaw_trial(world.config());
        profile.capture_min = 1.0;
        profile.capture_max = 1.0;
        let founder = world
            .start_hunter_trial(
                profile,
                HunterTarget {
                    face: 4,
                    u: 20.0,
                    v: 20.0,
                },
            )
            .unwrap();
        {
            let hunter = world.state.organisms.get_mut(founder.id).unwrap();
            hunter.heading = Vec2::new(1.0, 0.0);
            let old = hunter.reserve;
            hunter.reserve = 0.1;
            world.state.external_material_in += 0.1 - old;
        }
        let mut audit = CaptureAudit::new(&world.state);
        for _ in 0..600 {
            world.step();
            world.drain_events();
            let events = world.drain_hunter_events();
            if events
                .iter()
                .any(|e| matches!(e, HunterEvent::Capture { .. }))
            {
                return (world.state, events, audit);
            }
            audit.observe(3, &events, &world.state).unwrap();
        }
        panic!("fixture did not produce a real capture");
    }

    #[test]
    fn graph_neighborhoods_cross_seams_but_not_the_open_rim() {
        let neighborhoods = neighborhoods();
        let interior = CellId::new(Face::Front, 8, 8);
        assert_eq!(neighborhoods[interior.index()].len(), 25);
        let rim = CellId::new(Face::Front, 8, 15);
        assert_eq!(neighborhoods[rim.index()].len(), 16);
        assert!(
            neighborhoods[rim.index()]
                .iter()
                .all(|&i| CellId(i as u16).face() == Face::Front)
        );
        let seam = CellId::new(Face::Front, 8, 0);
        assert!(
            neighborhoods[seam.index()]
                .iter()
                .any(|&i| CellId(i as u16).face() == Face::Top)
        );
        let vertex = CellId::new(Face::Front, 0, 0);
        let faces: BTreeSet<_> = neighborhoods[vertex.index()]
            .iter()
            .map(|&i| CellId(i as u16).face().index())
            .collect();
        assert_eq!(
            faces,
            BTreeSet::from([Face::Front.index(), Face::Left.index(), Face::Top.index()])
        );
        for (center, cells) in neighborhoods.iter().enumerate() {
            assert!(cells.binary_search(&center).is_ok());
            assert!(cells.windows(2).all(|w| w[0] < w[1]));
            for &cell in cells {
                assert!(neighborhoods[cell].binary_search(&center).is_ok());
            }
        }
    }

    #[test]
    fn real_capture_uses_removed_prey_location_and_does_not_mutate_world() {
        let (state, events, mut audit) = capture_fixture();
        let before = cubarium_core::snapshot::state_hash(&state);
        let captures = audit.observe(3, &events, &state).unwrap();
        let evidence = events
            .iter()
            .find_map(|e| match e {
                HunterEvent::Capture { evidence, .. } => Some(evidence),
                _ => None,
            })
            .unwrap();
        assert_eq!(captures.len(), 1);
        assert_eq!(captures[0].cell, cell_of(&evidence.prey_pos).index());
        assert_ne!(captures[0].cell, cell_of(&evidence.hunter_pos).index());
        assert_eq!(captures[0].id.attempt, 1);
        assert_eq!(before, cubarium_core::snapshot::state_hash(&state));
        assert!(
            audit.observe(3, &events, &state).is_err(),
            "cross-batch repeated key must fail"
        );
    }

    #[test]
    fn missing_mismatched_and_duplicated_paid_evidence_are_refused_atomically() {
        let (state, events, audit) = capture_fixture();
        let capture = events
            .iter()
            .find(|e| matches!(e, HunterEvent::Capture { .. }))
            .unwrap()
            .clone();
        let mut bad = Vec::new();
        bad.push(vec![capture.clone()]);
        bad.push(
            events
                .iter()
                .filter(|e| !matches!(e, HunterEvent::Capture { .. }))
                .cloned()
                .collect(),
        );
        let mut duplicate = events.clone();
        duplicate.push(capture);
        bad.push(duplicate);
        for alteration in 0..4 {
            let mut altered = events.clone();
            for e in &mut altered {
                if let HunterEvent::Capture {
                    attack_counter,
                    evidence,
                    ..
                } = e
                {
                    match alteration {
                        0 => *attack_counter += 1,
                        1 => evidence.prey.generation += 1,
                        2 => evidence.prey_pos.u = f64::NAN,
                        _ => evidence.capture_center = None,
                    }
                }
            }
            bad.push(altered);
        }
        for malformed in bad {
            let mut a = audit.clone();
            assert!(a.observe(3, &malformed, &state).is_err());
            assert_eq!(a.last_settled, audit.last_settled);
            assert!(a.observe(3, &events, &state).is_ok());
        }
        assert!(audit.clone().observe(2, &events, &state).is_err());
    }

    #[test]
    fn matching_but_corrupted_pose_records_do_not_authorize_a_wrong_recovery_cell() {
        let (state, events, mut audit) = capture_fixture();
        let mut altered = events.clone();
        for event in &mut altered {
            let evidence = match event {
                HunterEvent::Capture { evidence, .. } => Some(evidence),
                HunterEvent::Attempt { evidence, .. } => evidence.as_mut(),
                _ => None,
            };
            if let Some(evidence) = evidence {
                evidence.prey_pos.u += 4.0;
            }
        }
        // Both copies still agree, and their old cached measure says in-contact.
        // Recompute from the reported root/prey/geometry, rather than trust that cache.
        assert!(audit.observe(3, &altered, &state).is_err());
        assert!(audit.observe(3, &events, &state).is_ok());
    }

    #[test]
    fn local_census_excludes_hunters_by_membership_and_matches_forms() {
        let (state, _, _) = capture_fixture();
        let states = [&state; 6];
        let counts = counts(states);
        let all: Vec<_> = (0..CELL_COUNT).collect();
        let forms = local_forms(states, &all);
        let expected = (state.organisms.len() - state.hunters.members.len()) as u32;
        for arm in 0..6 {
            assert_eq!(counts[arm].iter().sum::<u32>(), expected);
            assert_eq!(forms[arm].iter().sum::<u32>(), expected);
        }
        assert!(state.hunters.members.len() > 0);
    }

    #[test]
    fn capture_then_handling_restart_and_observer_reads_preserve_continuation() {
        let (state, events, mut audit) = capture_fixture();
        audit.observe(3, &events, &state).unwrap();
        let bytes = cubarium_core::encode_snapshot(&state, "spatial-observer-test");
        let (_, decoded) = cubarium_core::decode_snapshot(&bytes).unwrap();
        let mut observed = World::from_state(state).unwrap();
        let mut shadow = World::from_state(decoded).unwrap();
        for elapsed in 1..=1200 {
            observed.step();
            shadow.step();
            let events = observed.drain_hunter_events();
            assert_eq!(events, shadow.drain_hunter_events());
            audit.observe(3, &events, &observed.state).unwrap();
            if elapsed % 20 == 0 {
                let _ = counts([&observed.state; 6]);
                let _ = local_forms([&observed.state; 6], &[0, 1, 2]);
            }
            if elapsed % 200 == 0 {
                let _ = counts([&shadow.state; 6]);
                assert_eq!(
                    cubarium_core::snapshot::state_hash(&observed.state),
                    cubarium_core::snapshot::state_hash(&shadow.state)
                );
            }
            observed.drain_events();
            shadow.drain_events();
        }
    }
}

impl CaptureAudit {
    /// Trial opening has no in-flight attacks. This observer is not restart history.
    pub fn new(state: &WorldState) -> Self {
        Self {
            last_settled: state
                .hunters
                .members
                .iter()
                .map(|m| (m.id, m.attack_counter))
                .collect(),
            images: std::array::from_fn(|face| {
                let mut images = Vec::new();
                chart_images(
                    Face::from_index(face as u8).unwrap(),
                    MAX_SEAMS,
                    &mut images,
                );
                images
            }),
        }
    }

    pub fn observe(
        &mut self,
        arm: usize,
        events: &[HunterEvent],
        state: &WorldState,
    ) -> Result<Vec<recovery::Capture>> {
        let mut next = self.last_settled.clone();
        let mut attempts = BTreeMap::new();
        for event in events {
            if let HunterEvent::Attempt {
                tick,
                hunter,
                target,
                outcome,
                energy_paid,
                attack_counter,
                evidence,
            } = event
            {
                ensure!(*tick == state.tick, "attempt settlement tick mismatch");
                ensure!(
                    energy_paid.is_finite() && *energy_paid >= 0.0,
                    "invalid attempt payment"
                );
                ensure!(
                    next.contains_key(hunter),
                    "attempt from an unobserved full hunter ID"
                );
                match attack_counter {
                    None => ensure!(
                        *outcome == AttemptOutcome::Unaffordable && *energy_paid == 0.0,
                        "unpaid attempt is not an unaffordable refusal"
                    ),
                    Some(counter) => {
                        ensure!(
                            *outcome != AttemptOutcome::Unaffordable,
                            "unpaid refusal has a paid key"
                        );
                        ensure!(
                            *counter > next[hunter],
                            "paid attempt key repeated or regressed"
                        );
                        let profile = state
                            .hunters
                            .profile
                            .as_ref()
                            .context("attempt without profile")?;
                        ensure!(
                            *energy_paid == profile.strike_energy_cost,
                            "attempt payment disagrees with profile"
                        );
                        next.insert(*hunter, *counter);
                        ensure!(
                            attempts
                                .insert((*hunter, *counter), (*tick, *target, *outcome, *evidence))
                                .is_none(),
                            "duplicate paid attempt"
                        );
                    }
                }
            }
        }
        let mut captures = Vec::new();
        let mut keys = BTreeSet::new();
        let mut prey_ids = BTreeSet::new();
        for event in events {
            if let HunterEvent::Capture {
                tick,
                hunter,
                prey,
                attack_counter,
                evidence,
                material,
                energy,
            } = event
            {
                ensure!(ON_ARMS.contains(&arm), "capture in a non-hunting control");
                ensure!(
                    keys.insert((*hunter, *attack_counter)) && prey_ids.insert(*prey),
                    "duplicate capture key or prey"
                );
                ensure!(
                    *tick == state.tick && evidence.prey == *prey,
                    "capture evidence identity mismatch"
                );
                ensure!(
                    material.is_finite() && *material > 0.0 && energy.is_finite() && *energy >= 0.0,
                    "invalid capture inventory"
                );
                ensure!(
                    evidence.prey_pos.is_canonical() && evidence.hunter_pos.is_canonical(),
                    "capture position is not finite/canonical"
                );
                ensure!(
                    evidence.grants_capture(),
                    "capture lacks validated contact geometry"
                );
                let geometry = &evidence.geometry;
                let profile = state
                    .hunters
                    .profile
                    .as_ref()
                    .context("capture without profile")?;
                ensure!(
                    geometry.scale.is_finite()
                        && geometry.scale >= profile.body_scale_min
                        && geometry.scale <= 1.0,
                    "capture scale outside admitted range"
                );
                ensure!(
                    geometry.capture_offset_body == profile.capture_offset_body * geometry.scale
                        && geometry.ingestion_offset_body
                            == profile.ingestion_offset_body * geometry.scale
                        && geometry.capture_reach_px == profile.capture_reach_px * geometry.scale
                        && geometry.visual_query_extent_px
                            == profile.visual_query_extent_px * geometry.scale,
                    "capture geometry disagrees with scaled profile"
                );
                ensure!(
                    evidence.prey_extent.is_finite()
                        && evidence.prey_extent >= 0.0
                        && evidence.hunter_heading.is_finite(),
                    "capture has invalid extent/heading"
                );
                // Consistency check using the shared read-only root-chart helpers;
                // not a separate proof/reimplementation of the geometry algorithm.
                ensure!(
                    evidence.measure
                        == measure_contact(
                            &self.images,
                            evidence.hunter_pos,
                            evidence.hunter_heading,
                            geometry,
                            evidence.prey_pos,
                            evidence.prey_extent
                        ),
                    "capture measure disagrees with recorded positions"
                );
                ensure!(
                    evidence.capture_center
                        == body_point(
                            &self.images,
                            evidence.hunter_pos,
                            evidence.hunter_heading,
                            geometry.capture_offset_body
                        )
                        && evidence.ingestion_center
                            == body_point(
                                &self.images,
                                evidence.hunter_pos,
                                evidence.hunter_heading,
                                geometry.ingestion_offset_body
                            ),
                    "capture centers disagree with recorded root geometry"
                );
                ensure!(
                    state.organisms.get(*prey).is_none(),
                    "captured full prey ID is still alive"
                );
                let attempt = attempts
                    .get(&(*hunter, *attack_counter))
                    .context("capture lacks same-key paid attempt")?;
                ensure!(
                    *attempt
                        == (
                            *tick,
                            Some(*prey),
                            AttemptOutcome::Captured,
                            Some(*evidence)
                        ),
                    "capture disagrees with paid attempt target/outcome/geometry"
                );
                captures.push(recovery::Capture {
                    arm,
                    cell: cell_of(&evidence.prey_pos).index(),
                    id: recovery::CaptureId {
                        hunter_slot: hunter.slot,
                        hunter_generation: hunter.generation,
                        attempt: *attack_counter,
                    },
                });
            }
        }
        ensure!(
            attempts
                .values()
                .filter(|a| a.2 == AttemptOutcome::Captured)
                .count()
                == captures.len(),
            "successful attempt lacks capture"
        );
        // Prune dead IDs only after checking their possible same-tick settlement.
        // Newborn IDs have their own generation and no prior settled attempt.
        next.retain(|id, _| state.hunters.contains(*id));
        for member in &state.hunters.members {
            let last = next.entry(member.id).or_insert(0);
            ensure!(
                *last <= member.attack_counter,
                "settled key exceeds persisted member counter"
            );
        }
        self.last_settled = next;
        Ok(captures)
    }
}
