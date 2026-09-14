use cubarium_core::{
    ApexDormancyState, ApexEncounterState, FixedHunterProfile, HunterTarget, World, WorldConfig,
    snapshot::state_hash,
};

fn live_profile(world: &World) -> FixedHunterProfile {
    let mut profile = FixedHunterProfile::lanternjaw_trial(world.config());
    profile.seek_reserve_fraction = 0.80;
    profile.perch_reserve_fraction = 0.90;
    profile.charge80()
}

fn target(face: u8, u: f64, v: f64) -> HunterTarget {
    HunterTarget { face, u, v }
}

#[test]
fn repeated_introductions_are_active_deterministic_and_fully_accounted() {
    let mut a = World::new(WorldConfig::default()).unwrap();
    let mut b = World::new(WorldConfig::default()).unwrap();
    let opening_material = a.hunters().founder_material_in;
    let opening_energy = a.hunters().founder_energy_in;

    let first = a
        .introduce_hunters(live_profile(&a), &[target(0, 7.0, 11.0)])
        .unwrap();
    let pair = a
        .introduce_hunters(
            live_profile(&a),
            &[target(1, 23.0, 31.0), target(4, 55.0, 3.0)],
        )
        .unwrap();
    b.introduce_hunters(live_profile(&b), &[target(0, 7.0, 11.0)])
        .unwrap();
    b.introduce_hunters(
        live_profile(&b),
        &[target(1, 23.0, 31.0), target(4, 55.0, 3.0)],
    )
    .unwrap();

    assert_eq!(first.len(), 1);
    assert_eq!(pair.len(), 2);
    assert_eq!(a.hunters().founders_placed, 3);
    assert_eq!(a.hunters().members.len(), 3);
    assert_eq!(a.state.apex_dormancy, ApexDormancyState::underground_v1());
    assert_eq!(a.state.apex_encounters, ApexEncounterState::paired_v1());
    assert!(
        first
            .iter()
            .chain(&pair)
            .all(|r| !a.state.apex_dormancy.contains(r.id))
    );
    let material_each = first[0].material_in;
    let energy_each = first[0].energy_in;
    assert!(
        (a.hunters().founder_material_in - opening_material - 3.0 * material_each).abs() < 1e-9
    );
    assert!((a.hunters().founder_energy_in - opening_energy - 3.0 * energy_each).abs() < 1e-9);
    assert!(a.mass_residual().abs() < 1e-9);
    assert_eq!(state_hash(&a.state), state_hash(&b.state));
}

#[test]
fn a_pair_that_will_not_fit_is_rejected_without_a_partial_founder() {
    let mut world = World::new(WorldConfig::default()).unwrap();
    world.state.config.capacity.max_organisms = world.state.organisms.len() as u32 + 1;
    let before = state_hash(&world.state);
    let err = world
        .introduce_hunters(
            live_profile(&world),
            &[target(2, 8.0, 9.0), target(3, 10.0, 11.0)],
        )
        .unwrap_err();
    assert!(err.contains("capacity"), "{err}");
    assert_eq!(state_hash(&world.state), before);
    assert_eq!(world.hunters().founders_placed, 0);
}
