//! **TEST-PASS-4 (independent), GLOWCAP's plant-layer rules**: the substrate gate at
//! exactly `establish_substrate_min` (litter and dead wood together) and a hair under; the
//! switches off restoring the old rules (`substrate_required = false`: a dying or dying-back
//! fungus leaves wood as dead wood; `mycelium_graze_fraction = 0`: no mycelium offered);
//! caps first (a sliver of edible cap hides the mycelium); a satiated budgeted fungus taking
//! nothing and a rate-bound one taking its rate; the books closing when a fungus's return to
//! litter overflows the litter's energy cap; a dormant fungus's die-back; and a postcard
//! round trip of the new species fields and readouts at the current schema.
//!
//! Every expectation is computed from the config the case sets; no preset is pinned.

use super::*;
use cubarium_voxel::{Command as WorldCommand, Config as VoxelConfig, Material, World};

const FUNGUS: Species = Species::Glowcap;

fn site() -> Site {
    Site { x: 1, y: 2, z: 0 }
}

/// Three soil columns at field capacity, 1 m voxels.
fn world() -> World {
    let mut w = World::empty(VoxelConfig { width: 3, height: 7, depth: 1, voxel_m: 1.0, seed: 97, ..VoxelConfig::default() });
    let water = w.config().materials.field_capacity(Material::Soil) * w.config().materials.pore_capacity(Material::Soil) * w.config().voxel_volume();
    for x in 0..3 {
        for y in 1..=2 {
            w.apply(WorldCommand::AddWater { x, y, z: 0, volume_m3: water });
            w.apply(WorldCommand::SetMaterial { x, y, z: 0, material: Material::Soil });
        }
    }
    w
}

/// No decomposition, no seeds, a fungus whose every rate the case reads off this config.
fn config() -> FloraConfig {
    let mut c = FloraConfig::for_voxel_size(1.0);
    c.decomposition = 0.0;
    c.wood_decomposition = 0.0;
    c.carrion_decomposition = 0.0;
    c.upkeep_q10 = 1.0;
    c.initial_mineral = 1.0;
    c.step_every = 1;
    for sp in Species::ALL {
        let s = c.species_mut(sp);
        s.propagule_rate = 0.0;
        s.seed_attrition_per_s = 0.0;
        s.burr_pickup = 0.0;
        s.cure_after_s = 0.0;
    }
    let s = c.species_mut(FUNGUS);
    s.substrate_budgeted = true;
    s.substrate_required = true;
    s.mycelium_graze_fraction = 0.5;
    s.rooting_radius = 0;
    s.rooting_depth = 1;
    s.wood_max = 1.0;
    s.alpha = 0.5;
    s.reserve_cap = 0.2;
    s.maintenance = 0.0;
    s.dieback = 1.0;
    s.alive_min = 0.3;
    s.foliage_rate = 0.0;
    s.wood_rate = 0.0;
    s.senescence = 0.0;
    s.dark_uptake_per_s = 0.0;
    s.substrate_uptake_per_s = 1.0;
    s.substrate_yield = 0.5;
    s.energy_density = 2.0;
    s.defence_per_bite = 0.0;
    s.establish_substrate_min = 0.1;
    s.establish_water_min = 0.0;
    s.establish_saturated_max = 1.0;
    s.wilt_water = 0.0;
    s.full_water = 0.5;
    c.litter_energy_cap = c.glowcap.energy_density;
    c
}

fn seed(c: FloraConfig, w: &World, wood: f64) -> Flora {
    let mut f = Flora::new(c);
    assert!(f.apply(w, Command::SeedOnFace { site: site(), species: FUNGUS, wood }));
    f
}

fn litter(f: &mut Flora, organic: f64) {
    let s = &f.config.glowcap;
    let (n, e) = (s.n_tissue, s.energy_density);
    assert!(f.deposit(site(), Deposit { kind: DepositKind::Litter, organic, mineral: n * organic, energy: e * organic }));
}

fn close(f: &Flora) {
    let v = f.view();
    for (held, expected, what) in [
        (v.organic(), v.ledger.expected_organic(), "organic"),
        (v.mineral(), v.ledger.expected_mineral(), "mineral"),
        (v.energy(), v.ledger.expected_energy(), "energy"),
    ] {
        assert!((held - expected).abs() < 1e-10 * held.abs().max(1.0), "{what}: held {held}, expected {expected}");
    }
}

/// Take `share` of the fungus's reserve out, booked removed.
fn drain_reserve(f: &mut Flora, share: f64) {
    let e = f.config.glowcap.energy_density;
    let i = find_stand(&f.config, &f.stands, site(), FaceLayer::Fungus).unwrap();
    let r = share * f.stands[i].reserve;
    f.stands[i].reserve -= r;
    f.ledger.removed_organic_out += r;
    f.ledger.removed_energy_out += e * r;
}

fn crop(f: &mut Flora) {
    f.take_foliage_of(site(), FaceLayer::Fungus, f64::MAX, &(i64::MIN..=i64::MAX)).expect("caps above the refuge");
}

#[test]
fn the_substrate_gate_opens_at_exactly_its_minimum_of_litter_and_dead_wood_together() {
    let w = world();
    let sc = config().glowcap;
    let min = sc.establish_substrate_min;
    let gate = |wood: f64, litter: f64| step::establishment_gates_with_sky(&w.view(), site(), &sc, 0.0, wood, litter).substrate_ok;
    assert!(gate(0.0, min), "litter exactly at the minimum");
    assert!(gate(min, 0.0), "dead wood exactly at the minimum");
    assert!(gate(0.5 * min, 0.5 * min), "the two pools together");
    assert!(!gate(0.0, min * (1.0 - 1e-9)), "a hair under");
    assert!(!gate(0.5 * min, 0.5 * min * (1.0 - 1e-9)), "a hair under, together");
}

#[test]
fn with_substrate_not_required_a_dying_fungus_leaves_its_wood_as_dead_wood() {
    let w = world();
    for required in [true, false] {
        let mut c = config();
        c.glowcap.substrate_required = required;
        let mut f = seed(c, &w, 1.0);
        let s = f.stands.remove(0);
        step::die(&f.config, &w.view(), &s, &mut f.ground, &mut f.ledger, DeathCause::Starved, &mut f.death_log);
        let g = f.view().ground_at(site()).unwrap().clone();
        if required {
            assert_eq!(g.dead_wood, 0.0);
            assert!((g.litter - s.material()).abs() < 1e-12);
        } else {
            assert!((g.dead_wood - s.wood).abs() < 1e-12, "the old rule: the wood is a log");
            assert!((g.litter - (s.material() - s.wood)).abs() < 1e-12, "and the rest is litter");
        }
        close(&f);
    }
}

#[test]
fn with_substrate_not_required_a_fungus_dies_back_into_dead_wood() {
    let mut w = world();
    for required in [true, false] {
        let mut c = config();
        c.glowcap.substrate_required = required;
        c.glowcap.maintenance = 0.1 / DT;
        let sc = c.glowcap.clone();
        let mut f = seed(c, &w, sc.wood_max);
        drain_reserve(&mut f, 1.0);
        let before = *f.view().fungus_at(site()).unwrap();
        let bill = sc.maintenance * before.wood * DT;
        let lost = sc.dieback * bill;
        assert!(before.wood - lost > sc.alive_min, "fixture: a partial die-back");
        f.step(&mut w);
        let after = *f.view().fungus_at(site()).unwrap();
        assert!((before.wood - after.wood - lost).abs() < 1e-12, "required={required}: the configured die-back");
        let g = f.view().ground_at(site()).unwrap().clone();
        let (to_wood, to_litter) = if required { (0.0, lost) } else { (lost, 0.0) };
        assert!((g.dead_wood - to_wood).abs() < 1e-12 && (g.litter - to_litter).abs() < 1e-12, "required={required}: {g:?}");
        close(&f);
    }
}

#[test]
fn a_return_to_litter_that_overflows_the_litter_energy_cap_still_closes_the_books() {
    let mut w = world();
    let mut c = config();
    c.glowcap.maintenance = 0.1 / DT;
    // The litter can hold only a quarter of the fungus's energy density.
    c.litter_energy_cap = 0.25 * c.glowcap.energy_density;
    let sc = c.glowcap.clone();
    let mut f = seed(c.clone(), &w, sc.wood_max);
    drain_reserve(&mut f, 1.0);
    let heat0 = f.view().ledger.heat_out;
    f.step(&mut w);
    let g = f.view().ground_at(site()).unwrap().clone();
    assert!(g.litter > 0.0 && g.litter_energy <= f.config.litter_energy_cap * g.litter + 1e-12, "fixture: the cap binds");
    assert!(f.view().ledger.heat_out > heat0, "the overflow is heat");
    close(&f);
    // And a whole fungus dying into a capped litter pool.
    let mut f = seed(c, &w, sc.wood_max);
    let s = f.stands.remove(0);
    step::die(&f.config, &w.view(), &s, &mut f.ground, &mut f.ledger, DeathCause::Starved, &mut f.death_log);
    close(&f);
}

#[test]
fn a_zero_mycelium_fraction_offers_no_mycelium_once_the_caps_are_gone() {
    let w = world();
    let mut c = config();
    c.glowcap.mycelium_graze_fraction = 0.0;
    let mut f = seed(c, &w, 1.0);
    crop(&mut f);
    let s = *f.view().fungus_at(site()).unwrap();
    assert!(f.view().edible_foliage(&s) <= 1e-12, "fixture: cropped");
    assert_eq!(f.view().edible_mycelium(&s), 0.0);
    let ledger = f.view().ledger.clone();
    assert!(f.take_mycelium(site(), 1.0).is_none(), "nothing to take");
    assert_eq!(f.view().fungus_at(site()).unwrap().wood, s.wood);
    assert_eq!(f.view().ledger, &ledger);
    close(&f);
}

#[test]
fn a_sliver_of_regrown_cap_hides_the_mycelium() {
    let mut w = world();
    let mut c = config();
    // Caps regrow from the reserve at a small configured rate.
    c.glowcap.foliage_rate = 1e-3 / DT;
    let mut f = seed(c, &w, 1.0);
    crop(&mut f);
    let s = *f.view().fungus_at(site()).unwrap();
    assert!(f.view().edible_mycelium(&s) > 0.0, "fixture: cropped, the mycelium is offered");
    f.step(&mut w);
    let s = *f.view().fungus_at(site()).unwrap();
    let caps = f.view().edible_foliage(&s);
    assert!(caps > 0.0 && caps < 0.01 * s.wood, "fixture: a sliver of regrown cap: {caps}");
    assert_eq!(f.view().edible_mycelium(&s), 0.0, "caps first");
    assert!(f.take_mycelium(site(), 1.0).is_none());
    close(&f);
}

#[test]
fn mycelium_is_only_taken_from_a_fungus() {
    let w = world();
    let mut f = Flora::new(config());
    assert!(f.apply(&w, Command::SeedOnFace { site: site(), species: Species::Springturf, wood: f.config.springturf.wood_max }));
    f.take_foliage_of(site(), FaceLayer::Plant, f64::MAX, &(i64::MIN..=i64::MAX));
    assert!(f.take_mycelium(site(), 1.0).is_none(), "a plant's wood is not mycelium");
    for want in [0.0, -1.0, f64::NAN, f64::INFINITY] {
        let mut g = seed(config(), &w, 1.0);
        crop(&mut g);
        assert!(g.take_mycelium(site(), want).is_none(), "want {want}");
    }
    close(&f);
}

#[test]
fn a_satiated_budgeted_fungus_takes_no_substrate_and_a_rate_bound_one_takes_its_rate() {
    let mut w = world();
    let stock = 1.0;
    // Satiated: no upkeep, nothing to build, a full reserve, full caps.
    let mut c = config();
    let sc = c.glowcap.clone();
    let mut f = seed(c.clone(), &w, sc.wood_max);
    {
        let i = find_stand(&f.config, &f.stands, site(), FaceLayer::Fungus).unwrap();
        let s = f.stands[i];
        assert!(s.reserve >= sc.reserve_cap * s.wood - 1e-12 && s.foliage >= sc.alpha * s.wood - 1e-12, "fixture: a full fungus: {s:?}");
    }
    litter(&mut f, stock);
    f.step(&mut w);
    assert_eq!(f.view().ledger.substrate_uptake[FUNGUS.index()], 0.0, "no demand, no uptake");
    assert_eq!(f.view().ground_at(site()).unwrap().litter, stock);
    close(&f);
    // Rate-bound: a large upkeep, a tiny uptake rate.
    c.glowcap.maintenance = 0.1 / DT;
    c.glowcap.substrate_uptake_per_s = 1e-3;
    let sc = c.glowcap.clone();
    let mut f = seed(c, &w, sc.wood_max);
    litter(&mut f, stock);
    let wood = f.view().fungus_at(site()).unwrap().wood;
    let raw = sc.substrate_uptake_per_s * wood * DT;
    assert!(raw < sc.maintenance * wood * DT / sc.substrate_yield, "fixture: the rate binds");
    f.step(&mut w);
    let took = f.view().ledger.substrate_uptake[FUNGUS.index()];
    assert!((took - raw).abs() < 1e-15, "took {took}, rate {raw}");
    close(&f);
}

#[test]
fn a_fungus_flagged_dormant_still_dies_back_into_litter() {
    // A saprotroph given a cure and flagged cured never takes the dormant tick (curing is a
    // photosynthetic stand's), so its die-back still follows GLOWCAP's litter rule.
    let mut w = world();
    let mut c = config();
    c.glowcap.maintenance = 0.1 / DT;
    c.glowcap.cure_upkeep = 1.0;
    c.glowcap.cure_after_s = 1.0;
    c.glowcap.cure_resume_water = 2.0;
    let sc = c.glowcap.clone();
    let mut f = seed(c, &w, sc.wood_max);
    drain_reserve(&mut f, 1.0);
    let i = find_stand(&f.config, &f.stands, site(), FaceLayer::Fungus).unwrap();
    f.stands[i].cured = true;
    f.step(&mut w);
    let s = *f.view().fungus_at(site()).expect("still alive");
    assert!(s.cured, "fixture: it stayed dormant");
    let g = f.view().ground_at(site()).unwrap().clone();
    assert!(g.litter > 0.0 || g.dead_wood > 0.0, "fixture: it died back: {g:?}");
    assert_eq!(g.dead_wood, 0.0, "mycelium cannot become a reusable wood log: {g:?}");
    close(&f);
}

#[test]
fn glowcap_species_fields_and_readouts_survive_a_postcard_round_trip() {
    let w = world();
    let mut c = config();
    c.glowcap.substrate_budgeted = false;
    c.glowcap.substrate_required = true;
    c.glowcap.mycelium_graze_fraction = 0.375;
    let mut f = seed(c, &w, 1.0);
    f.ledger.substrate_by_pool = [0.25, 0.5];
    f.ledger.decomposed_by_pool = [0.125, 0.0625, 0.03125];
    f.ledger.mycelium_taken[FUNGUS.index()] = 0.75;
    f.ledger.stand_respired[FUNGUS.index()] = 1.5;
    let back = snapshot::decode(&snapshot::encode(&f)).expect("round trip");
    let (a, b) = (&back.config.glowcap, &f.config.glowcap);
    assert_eq!((a.substrate_budgeted, a.substrate_required, a.mycelium_graze_fraction), (b.substrate_budgeted, b.substrate_required, b.mycelium_graze_fraction));
    assert_eq!(back.ledger, f.ledger);
    assert_eq!(back.stands, f.stands);
}
