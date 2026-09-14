use cubarium_core::encounter::{ApexEncounterEvent, ApexEncounterState, CombatResponse};
use cubarium_core::genome::{Genome, decode};
use cubarium_core::hunter::{FixedHunterProfile, HunterMember, HunterTarget};
use cubarium_core::organism::{Mode, Organism, Origin};
use cubarium_core::rng::Counter;
use cubarium_core::{DT, World, WorldConfig, WorldState, decode_snapshot, encode_snapshot};
use cubarium_surface::{Face, SurfacePoint, Vec2};

const A: SurfacePoint = SurfacePoint {
    face: Face::Top,
    u: 24.0,
    v: 24.0,
};
const B: SurfacePoint = SurfacePoint {
    face: Face::Top,
    u: 24.5,
    v: 24.0,
};

fn world_with(cap: u32, attacks: bool, gestation_ticks: u64, interval_ticks: u64) -> World {
    let mut cfg = WorldConfig::default();
    cfg.capacity.max_organisms = cap;
    cfg.founders.kinds.clear();
    cfg.founders.count = 0;
    cfg.weather.amplitude = 0.0;
    cfg.water.rain_rate = 0.0;
    cfg.habitat.light_base = 0.0;
    cfg.habitat.light_height_gain = 0.0;
    cfg.habitat.light_noise_gain = 0.0;
    cfg.producer.growth = 0.0;
    cfg.producer.mortality = 0.0;
    cfg.detritus.decomposition = 0.0;
    cfg.detritus.fall = 0.0;
    cfg.organism.maintenance = 0.0;
    cfg.organism.move_cost = 0.0;
    cfg.organism.sense_cost = 0.0;
    cfg.organism.oxidation_rate = 0.0;
    cfg.organism.growth_rate = 0.0;
    let mut world = World::new(cfg).expect("quiet world");
    world.state.apex_encounters = ApexEncounterState::paired_v1();
    let mut profile = FixedHunterProfile::lanternjaw_trial(world.config());
    profile.attacks_enabled = attacks;
    profile.reproduce_min_age_seconds = 0.0;
    profile.gestation_seconds = gestation_ticks as f64 * DT;
    profile.reproduce_interval_seconds = interval_ticks as f64 * DT;
    world
        .start_hunter_trial(
            profile,
            HunterTarget {
                face: A.face.index() as u8,
                u: A.u,
                v: A.v,
            },
        )
        .expect("first hunter");
    world
}

fn fill(world: &mut World, id: cubarium_core::OrganismId) {
    let o = world.state.organisms.get_mut(id).unwrap();
    let before = o.reserve;
    o.reserve = o.phenotype.reserve_max;
    o.energy = o.phenotype.energy_max;
    world.state.external_material_in += o.reserve - before;
}

fn second_hunter(
    world: &mut World,
    first: cubarium_core::OrganismId,
    pos: SurfacePoint,
    size: f32,
) -> cubarium_core::OrganismId {
    let cfg = world.config().clone();
    let mut o = world.state.organisms.get(first).unwrap().clone();
    o.pos = pos;
    o.genome.size = size;
    o.genome.clamp();
    o.phenotype = decode(&o.genome, &cfg.organism);
    o.phenotype.extent = world.hunters().profile().unwrap().body_extent_px;
    o.structure = o.phenotype.structure_adult;
    o.reserve = o.phenotype.reserve_max;
    o.energy = o.phenotype.energy_max;
    o.born_tick = world.tick();
    o.escrow = None;
    o.births = 0;
    o.parent = None;
    o.origin = Origin::Founder;
    o.turn_counter = Counter::default();
    let imported = o.structure + o.reserve;
    let id = world.state.organisms.insert(o);
    assert!(
        world
            .state
            .hunters
            .insert_member(HunterMember::new(id, world.tick()))
    );
    world.state.external_material_in += imported;
    id
}

fn first_id(world: &World) -> cubarium_core::OrganismId {
    world.hunters().members[0].id
}

fn place_bystander(world: &mut World) {
    let cfg = world.config().clone();
    let genome = Genome::founder(0.5, &cfg.drives);
    let phenotype = decode(&genome, &cfg.organism);
    let structure = phenotype.structure_adult;
    let reserve = 0.1;
    world.state.organisms.insert(Organism {
        pos: SurfacePoint::new(Face::Top, 50.0, 50.0),
        heading: Vec2::new(1.0, 0.0),
        ou: Vec2::ZERO,
        structure,
        reserve,
        energy: 0.2,
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
    world.state.external_material_in += structure + reserve;
}

#[test]
fn both_adults_pay_and_both_genomes_contribute_to_a_persisted_paid_child() {
    let mut world = world_with(8, false, 1, 100);
    let a = first_id(&world);
    let organism_cfg = world.config().organism.clone();
    let hunter_extent = world.hunters().profile().unwrap().body_extent_px;
    {
        let o = world.state.organisms.get_mut(a).unwrap();
        o.genome.size = 1.0;
        o.genome.hue = 0.2;
        o.genome.clamp();
        o.phenotype = decode(&o.genome, &organism_cfg);
        o.phenotype.extent = hunter_extent;
        o.structure = o.phenotype.structure_adult;
    }
    fill(&mut world, a);
    let b = second_hunter(&mut world, a, B, 2.0);
    world.state.organisms.get_mut(b).unwrap().genome.hue = 0.8;
    let a_before = world.state.organisms.get(a).unwrap().clone();
    let b_before = world.state.organisms.get(b).unwrap().clone();

    world.step();
    let events = world.drain_apex_encounter_events();
    let ApexEncounterEvent::Mated {
        carrier,
        partner,
        carrier_paid,
        partner_paid,
        ..
    } = events[0]
    else {
        panic!("{events:?}")
    };
    assert_eq!((carrier, partner), (a, b));
    assert_eq!(carrier_paid, partner_paid);
    assert_eq!(
        a_before.reserve - world.state.organisms.get(a).unwrap().reserve,
        carrier_paid.material()
    );
    assert_eq!(
        b_before.reserve - world.state.organisms.get(b).unwrap().reserve,
        partner_paid.material()
    );
    assert_eq!(
        a_before.energy - world.state.organisms.get(a).unwrap().energy,
        carrier_paid.energy + carrier_paid.build_heat
    );
    assert_eq!(
        b_before.energy - world.state.organisms.get(b).unwrap().energy,
        partner_paid.energy + partner_paid.build_heat
    );
    world.step();
    let born = world
        .drain_apex_encounter_events()
        .into_iter()
        .find_map(|e| match e {
            ApexEncounterEvent::Born { child, .. } => Some(child),
            _ => None,
        })
        .expect("paired birth");
    let child = world.state.organisms.get(born).unwrap();
    assert_eq!(child.genome.size, 1.5);
    assert_eq!(child.genome.hue, 0.5);
    assert_eq!(world.apex_encounters().parents_of(born), Some((a, b)));
    let (_, loaded) = decode_snapshot(&encode_snapshot(&world.state, "paired")).unwrap();
    assert_eq!(loaded.apex_encounters.parents_of(born), Some((a, b)));
    assert!(
        world.drain_hunter_events().is_empty(),
        "paired facts never claim one-parent transactions"
    );
    world.check_invariants().unwrap();
}

#[test]
fn paired_policy_blocks_solitary_reproduction_and_reuses_neither_parent_during_cooldown() {
    let mut world = world_with(8, false, 1, 100);
    let a = first_id(&world);
    fill(&mut world, a);
    for _ in 0..5 {
        world.step();
    }
    assert_eq!(world.population(), 1);
    assert!(world.state.organisms.get(a).unwrap().escrow.is_none());

    let far = SurfacePoint::new(Face::Top, 50.0, 50.0);
    let b = second_hunter(&mut world, a, far, 2.0);
    world.step();
    assert_eq!(
        world.apex_encounters().matings_total,
        0,
        "distance blocks pairing"
    );
    world.state.organisms.get_mut(b).unwrap().pos = B;
    world.step();
    assert_eq!(world.apex_encounters().matings_total, 1);
    world.step();
    let child = world.apex_encounters().parentage[0].child;
    // Even with the new child made artificially ready and nearby, both parents are cooling down.
    let o = world.state.organisms.get_mut(child).unwrap();
    o.structure = o.phenotype.structure_adult;
    o.reserve = o.phenotype.reserve_max;
    o.energy = o.phenotype.energy_max;
    for _ in 0..20 {
        world.step();
        world.drain_apex_encounter_events();
    }
    assert_eq!(world.apex_encounters().matings_total, 1);
    assert!(world.hunters().member(a).unwrap().next_reproduction_tick > world.tick());
    assert!(world.hunters().member(b).unwrap().next_reproduction_tick > world.tick());
}

#[test]
fn schema_thirteen_migrates_encounters_off_and_enabled_state_has_no_old_projection() {
    let cfg = WorldConfig::default();
    let current = World::new(cfg).unwrap().state;
    let old = cubarium_core::snapshot::v13::project(&current).expect("off state projects");
    let migrated = WorldState::from(old);
    assert_eq!(migrated.apex_encounters, ApexEncounterState::default());

    let enabled = world_with(4, false, 1, 10).state;
    assert!(cubarium_core::snapshot::v13::project(&enabled).is_none());
}

#[test]
fn cap_rejection_restores_each_live_contributors_share_without_refunding_build_heat() {
    let mut world = world_with(3, false, 2, 100);
    let a = first_id(&world);
    fill(&mut world, a);
    let b = second_hunter(&mut world, a, B, 2.0);
    let before_a = world.state.organisms.get(a).unwrap().clone();
    let before_b = world.state.organisms.get(b).unwrap().clone();
    world.step();
    let paid = world.apex_encounters().gestations[0].carrier_paid;
    place_bystander(&mut world);
    world.step();
    world.step();
    let refunded = world
        .drain_apex_encounter_events()
        .into_iter()
        .find(|e| matches!(e, ApexEncounterEvent::Refunded { .. }))
        .expect("paired refund");
    assert!(
        matches!(refunded, ApexEncounterEvent::Refunded { partner_refund_to, .. } if partner_refund_to == b)
    );
    assert_eq!(
        world.state.organisms.get(a).unwrap().reserve,
        before_a.reserve
    );
    assert_eq!(
        world.state.organisms.get(b).unwrap().reserve,
        before_b.reserve
    );
    assert_eq!(
        world.state.organisms.get(a).unwrap().energy,
        before_a.energy - paid.build_heat
    );
    assert_eq!(
        world.state.organisms.get(b).unwrap().energy,
        before_b.energy - paid.build_heat
    );
    assert!(world.apex_encounters().gestations.is_empty());
    world.check_invariants().unwrap();
}

#[test]
fn carrier_death_exports_the_whole_joint_escrow_and_keeps_accounting_closed() {
    let mut world = world_with(4, false, 20, 100);
    let a = first_id(&world);
    fill(&mut world, a);
    let b = second_hunter(&mut world, a, B, 2.0);
    world.step();
    world.drain_apex_encounter_events();
    let escrow = world
        .state
        .organisms
        .get(a)
        .unwrap()
        .escrow
        .clone()
        .unwrap();
    let o = world.state.organisms.get_mut(a).unwrap();
    o.reserve = 0.0;
    o.energy = 0.0;
    world.step();
    let event = world
        .drain_apex_encounter_events()
        .into_iter()
        .find(|e| matches!(e, ApexEncounterEvent::Miscarried { .. }))
        .expect("joint miscarriage");
    assert!(
        matches!(event, ApexEncounterEvent::Miscarried { carrier, partner, material, .. }
        if carrier == a && partner == b && material == escrow.structure + escrow.reserve)
    );
    assert!(world.state.organisms.get(a).is_none());
    assert!(world.state.organisms.get(b).is_some());
    assert!(world.apex_encounters().gestations.is_empty());
    world.check_invariants().unwrap();
}

#[test]
fn partner_death_leaves_the_prepaid_gestation_and_stale_safe_parentage_intact() {
    let mut world = world_with(4, false, 2, 100);
    let a = first_id(&world);
    fill(&mut world, a);
    let b = second_hunter(&mut world, a, B, 2.0);
    world.step();
    world.drain_apex_encounter_events();
    let partner = world.state.organisms.get_mut(b).unwrap();
    partner.reserve = 0.0;
    partner.energy = 0.0;
    world.step();
    assert!(world.state.organisms.get(b).is_none());
    assert!(world.apex_encounters().gestation(a).is_some());
    world.step();
    let child = world
        .drain_apex_encounter_events()
        .into_iter()
        .find_map(|event| match event {
            ApexEncounterEvent::Born { child, .. } => Some(child),
            _ => None,
        })
        .expect("the already-paid gestation completes");
    assert_eq!(world.apex_encounters().parents_of(child), Some((a, b)));
    world.check_invariants().unwrap();
}

#[test]
fn unavailable_mates_retreat_or_retaliate_with_paid_consequences() {
    let mut retreat = world_with(4, true, 20, 100);
    let large = first_id(&retreat);
    fill(&mut retreat, large);
    let small = second_hunter(&mut retreat, large, B, 0.5);
    retreat.state.organisms.get_mut(large).unwrap().reserve = 0.0;
    retreat.step();
    assert!(retreat.drain_apex_encounter_events().iter().any(|e| matches!(e,
        ApexEncounterEvent::Combat { attacker, defender, response: CombatResponse::Retreated,
            attacker_energy_paid, defender_energy_paid, attacker_injury: 0.0, defender_injury: 0.0, .. }
        if *attacker == large && *defender == small && *attacker_energy_paid > 0.0 && *defender_energy_paid > 0.0)));

    let mut retaliation = world_with(4, true, 20, 100);
    let a = first_id(&retaliation);
    fill(&mut retaliation, a);
    let b = second_hunter(&mut retaliation, a, B, 2.0);
    retaliation.state.organisms.get_mut(a).unwrap().reserve = 0.0;
    let a_structure = retaliation.state.organisms.get(a).unwrap().structure;
    let b_structure = retaliation.state.organisms.get(b).unwrap().structure;
    retaliation.step();
    assert!(
        retaliation
            .drain_apex_encounter_events()
            .iter()
            .any(|e| matches!(e,
        ApexEncounterEvent::Combat { response: CombatResponse::Retaliated,
            attacker_injury, defender_injury, .. }
        if *attacker_injury > 0.0 && *defender_injury > 0.0))
    );
    // Hunter physiology may immediately rebuild a paid sliver after the wound, but neither
    // participant can finish the tick at its opening structure.
    assert!(retaliation.state.organisms.get(a).unwrap().structure < a_structure);
    assert!(retaliation.state.organisms.get(b).unwrap().structure < b_structure);
    retaliation.check_invariants().unwrap();
}

#[test]
fn a_dormant_paid_child_is_never_an_encounter_participant_or_combat_target() {
    let mut world = world_with(6, false, 1, 100);
    world.state.apex_dormancy = cubarium_core::ApexDormancyState::underground_v1();
    let a = first_id(&world);
    fill(&mut world, a);
    let _b = second_hunter(&mut world, a, B, 2.0);
    world.step();
    world.drain_apex_encounter_events();
    world.step();
    let child = world.apex_encounters().parentage[0].child;
    assert!(world.apex_dormancy().contains(child));
    {
        let o = world.state.organisms.get_mut(child).unwrap();
        o.structure = o.phenotype.structure_adult;
        o.reserve = o.phenotype.reserve_max;
        o.energy = o.phenotype.energy_max;
    }
    world
        .state
        .hunters
        .profile
        .as_mut()
        .unwrap()
        .attacks_enabled = true;
    world.state.organisms.get_mut(a).unwrap().reserve = 0.0;
    let before = world.state.organisms.get(child).unwrap().structure;
    world.step();
    assert_eq!(world.state.organisms.get(child).unwrap().structure, before);
    assert!(world.drain_apex_encounter_events().iter().all(|e| !matches!(e,
        ApexEncounterEvent::Combat { attacker, defender, .. } if *attacker == child || *defender == child)));
}
