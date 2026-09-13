// Independent regression for frozen 9f4cf7d. Install unchanged as
// crates/cubarium-core/tests/astra_size_gate_review.rs in that source archive.
// This is a synthetic one-step boundary fixture, not an ecology experiment.
use cubarium_core::{World, WorldConfig};
use cubarium_core::hunter::{FixedHunterProfile, HunterTarget};
use cubarium_core::ids::OrganismId;

fn final_increment() -> (World, OrganismId, f64) {
    let mut cfg = WorldConfig::default();
    cfg.founders.kinds.clear();
    cfg.founders.count = 0;
    cfg.weather.amplitude = 0.0;
    cfg.water.rain_rate = 0.0;
    cfg.habitat.light_base = 0.0;
    cfg.habitat.light_height_gain = 0.0;
    cfg.habitat.light_noise_gain = 0.0;
    cfg.detritus.initial_dark = 0.0;
    cfg.detritus.decomposition = 0.0;
    cfg.detritus.fall = 0.0;
    cfg.organism.maintenance = 0.0;
    cfg.organism.move_cost = 0.0;
    cfg.organism.sense_cost = 0.0;
    let mut world = World::new(cfg).unwrap();
    let profile = FixedHunterProfile::lanternjaw_trial(world.config()).size_gate();
    let id = world.start_hunter_trial(profile, HunterTarget {
        face: 0, u: 22.0, v: 34.0,
    }).unwrap().id;
    let o = world.state.organisms.get_mut(id).unwrap();
    let before = o.structure + o.reserve;
    let adult = o.phenotype.structure_adult;
    o.structure = adult - 0.00002;
    o.reserve = 3.0;
    o.energy = 3.5; // Above charge80: no oxidation before the growth site.
    world.state.external_material_in += o.structure + o.reserve - before;
    world.enable_flow_ledger();
    (world, id, adult)
}

#[test]
fn adulthood_is_recorded_on_the_actual_final_increment_even_at_horizon() {
    let (mut world, id, adult) = final_increment();
    let boundary = world.state.tick + 1;
    world.step();
    assert_eq!(world.state.organisms.get(id).unwrap().structure, adult);
    let m = &world.flow_ledger().unwrap().members[&id];
    assert_eq!(m.growth.ticks, 1);
    assert_eq!(m.gate.first_adult_tick, Some(boundary),
        "Do not wait for another pre-growth observation to notice a completed increment");
}

#[test]
fn maximum_structure_includes_the_final_post_growth_stock() {
    let (mut world, id, adult) = final_increment();
    world.step();
    let m = &world.flow_ledger().unwrap().members[&id];
    assert_eq!(m.gate.max_structure, adult,
        "The last growth before horizon/death must not disappear from the maximum");
}

#[test]
fn actual_growth_ledger_records_the_unchanged_paid_transfer_and_heat() {
    let (mut world, id, adult) = final_increment();
    let before = world.state.organisms.get(id).unwrap().clone();
    let build = world.config().organism.build_cost;
    let e_r = world.config().organism.reserve_energy_density;
    world.step();
    let after = world.state.organisms.get(id).unwrap();
    let m = &world.flow_ledger().unwrap().members[&id];
    let ds = adult - before.structure;
    assert_eq!(m.growth.bound_by_remaining_structure, 1);
    assert!((m.growth.structure_gained - ds).abs() < 1e-12);
    assert!((before.reserve - after.reserve - ds).abs() < 1e-12);
    assert!((m.growth.reserve_spent - ds).abs() < 1e-12);
    assert!((m.growth.energy_cost - build * ds).abs() < 1e-12);
    assert!((before.energy - after.energy - build * ds).abs() < 1e-12);
    assert!((m.growth.heat - (build + e_r) * ds).abs() < 1e-12);
    assert_eq!(m.residual.violations, 0);
}
