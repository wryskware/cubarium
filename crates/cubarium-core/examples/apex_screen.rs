use std::collections::{BTreeMap, BTreeSet};

use cubarium_core::encounter::{ApexEncounterEvent, ApexEncounterState};
use cubarium_core::genome::{Genome, decode};
use cubarium_core::hunter::{FixedHunterProfile, HunterEvent, HunterMember, HunterTarget};
use cubarium_core::organism::{Mode, Organism, Origin};
use cubarium_core::rng::Counter;
use cubarium_core::{ApexDormancyEvent, ApexDormancyState, LifeEvent, World, WorldConfig};
use cubarium_surface::{Face, SurfacePoint, Vec2};

const TICKS: u64 = 12_000;
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

#[derive(Default)]
struct Counts {
    matings: u64,
    births: u64,
    emergences: u64,
    deaths: u64,
    predator_deaths: u64,
    prey_deaths: u64,
    combats: u64,
    matured_offspring: BTreeSet<cubarium_core::OrganismId>,
    another_generation_emerged: bool,
}

fn target(pos: SurfacePoint) -> HunterTarget {
    HunterTarget {
        face: pos.face.index() as u8,
        u: pos.u,
        v: pos.v,
    }
}

fn fill(world: &mut World, id: cubarium_core::OrganismId) {
    let organism = world
        .state
        .organisms
        .get_mut(id)
        .expect("staged hunter is alive");
    let before = organism.reserve;
    organism.structure = organism.phenotype.structure_adult;
    organism.reserve = organism.phenotype.reserve_max;
    organism.energy = organism.phenotype.energy_max;
    world.state.external_material_in += organism.reserve - before;
}

fn add_second_hunter(
    world: &mut World,
    first: cubarium_core::OrganismId,
) -> cubarium_core::OrganismId {
    let mut organism = world
        .state
        .organisms
        .get(first)
        .expect("first hunter")
        .clone();
    organism.pos = B;
    organism.born_tick = world.tick();
    organism.escrow = None;
    organism.births = 0;
    organism.parent = None;
    organism.origin = Origin::Founder;
    organism.turn_counter = Counter::default();
    let imported = organism.structure + organism.reserve;
    let id = world.state.organisms.insert(organism);
    assert!(
        world
            .state
            .hunters
            .insert_member(HunterMember::new(id, world.tick()))
    );
    world.state.external_material_in += imported;
    id
}

fn add_prey(world: &mut World, count: u32) {
    let cfg = world.config().clone();
    for i in 0..count {
        let mut genome = Genome::founder(0.4, &cfg.drives);
        genome.clamp();
        let mut phenotype = decode(&genome, &cfg.organism);
        phenotype.speed_max = 0.0;
        phenotype.maintenance = 0.0;
        let structure = 0.2;
        let reserve = 0.02;
        let pos = SurfacePoint::new(A.face, A.u + 2.0 + f64::from(i % 4), A.v + f64::from(i / 4))
            .canonicalize();
        world.state.organisms.insert(Organism {
            pos,
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
}

fn staged_opening() -> World {
    let mut cfg = WorldConfig::default();
    cfg.seed = 37;
    cfg.capacity.max_organisms = 64;
    cfg.founders.kinds.clear();
    cfg.founders.count = 0;
    let mut world = World::new(cfg).expect("valid fixed-seed world");
    let mut profile = FixedHunterProfile::lanternjaw_trial(world.config());
    profile.reproduce_min_age_seconds = 0.0;
    profile.gestation_seconds = 1.0;
    let first = world
        .start_hunter_trial(profile, target(A))
        .expect("first hunter")
        .id;
    fill(&mut world, first);
    let second = add_second_hunter(&mut world, first);
    fill(&mut world, second);
    add_prey(&mut world, 12);
    world.check_invariants().expect("staged opening is valid");
    world
}

fn census(world: &World) -> (usize, usize, usize) {
    let hunters: BTreeSet<_> = world.hunters().members.iter().map(|m| m.id).collect();
    let dormant = world.apex_dormancy().dormant.len();
    let active = hunters
        .iter()
        .filter(|id| {
            world.state.organisms.get(**id).is_some() && !world.apex_dormancy().contains(**id)
        })
        .count();
    let prey = world
        .state
        .organisms
        .iter()
        .filter(|(id, _)| !hunters.contains(id))
        .count();
    (prey, active, dormant)
}

fn run(name: &str, mut world: World, enable_candidate: bool) {
    if enable_candidate {
        world.state.apex_dormancy = ApexDormancyState::underground_v1();
        world.state.apex_encounters = ApexEncounterState::paired_v1();
    }
    let initial = census(&world);
    let founders: Vec<_> = world.hunters().members.iter().map(|m| m.id).collect();
    let mut predators: BTreeSet<_> = founders.iter().copied().collect();
    let mut depth: BTreeMap<_, u32> = founders.into_iter().map(|id| (id, 0)).collect();
    let mut counts = Counts::default();

    world.drain_events();
    world.drain_hunter_events();
    world.drain_apex_dormancy_events();
    world.drain_apex_encounter_events();
    world.drain_quiet_events();

    for _ in 0..TICKS {
        world.step();
        let life = world.drain_events();
        let hunter = world.drain_hunter_events();
        let dormant = world.drain_apex_dormancy_events();
        let encounter = world.drain_apex_encounter_events();
        world.drain_quiet_events();

        for event in &hunter {
            if let HunterEvent::Offspring { parent, child, .. } = *event {
                counts.births += 1;
                predators.insert(child);
                depth.insert(child, depth.get(&parent).copied().unwrap_or(0) + 1);
            }
        }
        for event in &encounter {
            match *event {
                ApexEncounterEvent::Mated { .. } => counts.matings += 1,
                ApexEncounterEvent::Born {
                    carrier,
                    partner,
                    child,
                    ..
                } => {
                    counts.births += 1;
                    predators.insert(child);
                    let d = depth
                        .get(&carrier)
                        .copied()
                        .unwrap_or(0)
                        .max(depth.get(&partner).copied().unwrap_or(0));
                    depth.insert(child, d + 1);
                }
                ApexEncounterEvent::Combat { .. } => counts.combats += 1,
                _ => {}
            }
        }
        for event in &dormant {
            if let ApexDormancyEvent::Emerged { id, .. } = *event {
                counts.emergences += 1;
                counts.another_generation_emerged |= depth.get(&id).copied().unwrap_or(0) >= 2;
            }
        }
        for event in &life {
            if let LifeEvent::Death { id, .. } = *event {
                counts.deaths += 1;
                if predators.contains(&id) {
                    counts.predator_deaths += 1;
                } else {
                    counts.prey_deaths += 1;
                }
            }
        }
        for id in predators
            .iter()
            .copied()
            .filter(|id| depth.get(id).copied().unwrap_or(0) > 0)
        {
            if let Some(organism) = world.state.organisms.get(id) {
                if !world.apex_dormancy().contains(id)
                    && organism.structure >= organism.phenotype.structure_adult
                {
                    counts.matured_offspring.insert(id);
                }
            }
        }
        world.check_invariants().expect("screen invariant");
    }

    let final_census = census(&world);
    println!(
        "{{\"arm\":\"{name}\",\"ticks\":{TICKS},\"initial\":{{\"prey\":{},\"predators_active\":{},\"predators_dormant\":{}}},\"final\":{{\"prey\":{},\"predators_active\":{},\"predators_dormant\":{}}},\"events\":{{\"matings\":{},\"births\":{},\"emergences\":{},\"deaths\":{},\"predator_deaths\":{},\"prey_deaths\":{},\"combats\":{}}},\"offspring_matured\":{},\"another_generation_emerged\":{},\"mass_residual\":{:.12}}}",
        initial.0,
        initial.1,
        initial.2,
        final_census.0,
        final_census.1,
        final_census.2,
        counts.matings,
        counts.births,
        counts.emergences,
        counts.deaths,
        counts.predator_deaths,
        counts.prey_deaths,
        counts.combats,
        counts.matured_offspring.len(),
        counts.another_generation_emerged,
        world.mass_residual(),
    );
}

fn main() {
    let opening = staged_opening();
    assert!(!opening.apex_dormancy().active() && !opening.apex_encounters().active());
    run(
        "baseline_policies_off",
        World::from_state(opening.state.clone()).unwrap(),
        false,
    );
    run(
        "dormancy_and_paired_v1",
        World::from_state(opening.state).unwrap(),
        true,
    );
}
