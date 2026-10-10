//! Independent mechanism tests: hand-built groups and a compressed breeding clock.
//! None of the assertions depend on shipped ecological tuning.

use super::predation_fixture::*;
use super::{disperse, grow};
use crate::{
    Dispersal, DispersalAt, Escrow, Fauna, FaunaConfig, Founder, GutSeedLot, Reproduction,
    SeedPlant, StartingStores,
};
use cubarium_voxel::weather::{Climate, DayClock, DayMode};
use cubarium_voxel::World;
use cubarium_voxel_flora::{Flora, FloraConfig, Site, Taken};

const L: Founder = Founder::Browser;
const V: Founder = Founder::Chorister;

fn configuration(founder: Founder, maximum: u32, at: DispersalAt) -> FaunaConfig {
    let mut c = FaunaConfig::default();
    let p = &mut c.founders[founder.index()];
    p.dispersal = Dispersal {
        enabled: true,
        group_max: maximum,
        at,
    };
    p.herding.young_only = true;
    p.herding.comfort_m = 0.75;
    p.core.growth_max_per_s = p.core.birth_body / super::DT;
    c
}

fn flora() -> Flora {
    Flora::new(FloraConfig {
        litter_energy_cap: 1000.0,
        ..FloraConfig::default()
    })
}

fn member(fauna: &mut Fauna, world: &World, founder: Founder, x: f64, z: f64, young: bool) -> u64 {
    let id = introduce(fauna, world, founder, StartingStores::FULL);
    place(fauna, id, x, z, EAST);
    if young {
        let birth = fauna.config.founder(founder).core.birth_body;
        let a = animal_mut(fauna, id);
        a.body = birth * 0.9;
        a.reserve = birth * 2.0;
    }
    id
}

fn grow_and_disperse(fauna: &mut Fauna, world: &World, flora: &mut Flora) -> Vec<u64> {
    let crossed = grow(fauna);
    disperse(fauna, &world.view(), flora, &crossed, DispersalAt::Adult);
    crossed
}

fn alive(fauna: &Fauna, id: u64) -> bool {
    fauna.view().animal(id).is_some()
}

fn near(left: f64, right: f64) {
    assert!(
        (left - right).abs() <= 1e-11 * left.abs().max(right.abs()).max(1.0),
        "{left} differs from {right}"
    );
}

#[test]
fn adulthood_dispersal_requires_strictly_more_than_the_group_limit() {
    for maximum in [1, 2, 3] {
        let world = clock_off(501);
        let mut f = Fauna::new(configuration(L, maximum, DispersalAt::Adult));
        let mut plants = flora();
        let parent = member(&mut f, &world, L, 2.0, 1.5, false);
        let calf = member(&mut f, &world, L, 2.1, 1.5, true);
        let crossed = grow_and_disperse(&mut f, &world, &mut plants);
        assert_eq!(crossed, vec![calf]);
        assert!(alive(&f, parent), "an established adult is never selected");
        assert_eq!(alive(&f, calf), maximum >= 2);
        assert_eq!(
            f.ledger.dispersed_by_founder[L.index()],
            u64::from(maximum < 2)
        );
        assert_eq!(f.ledger.deaths_accounted(), 0);
    }
}

#[test]
fn a_group_over_its_limit_does_not_remove_established_adults_on_a_later_tick() {
    let world = clock_off(502);
    let mut f = Fauna::new(configuration(L, 1, DispersalAt::Adult));
    let mut plants = flora();
    let parent = member(&mut f, &world, L, 2.0, 1.5, false);
    let adult = member(&mut f, &world, L, 2.1, 1.5, false);
    assert!(grow_and_disperse(&mut f, &world, &mut plants).is_empty());
    assert!(alive(&f, parent) && alive(&f, adult));
    assert_eq!(f.ledger.dispersed_by_founder[L.index()], 0);
}

#[test]
fn simultaneous_maturations_a_calf_that_stays_counts_for_the_next() {
    let world = clock_off(503);
    let mut f = Fauna::new(configuration(L, 2, DispersalAt::Adult));
    let mut plants = flora();
    let parent = member(&mut f, &world, L, 2.0, 1.5, false);
    let one = member(&mut f, &world, L, 2.1, 1.5, true);
    let two = member(&mut f, &world, L, 2.2, 1.5, true);
    let crossed = grow_and_disperse(&mut f, &world, &mut plants);
    assert_eq!(crossed, vec![one, two]);
    assert!(alive(&f, parent));
    assert!(alive(&f, one), "parent plus the first calf is the limit");
    assert!(!alive(&f, two), "the first calf now counts as established");
    assert_eq!(f.ledger.dispersed_by_founder[L.index()], 1);
}

#[test]
fn the_herd_is_every_adult_chained_within_comfort_and_only_adults_count() {
    // comfort_m is 0.75. Adults at 2.0, 2.6 and 3.2 form one herd by chain although the
    // ends are 1.2 m apart; the calf at 3.7 is beside the last only.
    for (adults, maximum, leaves) in [(3usize, 3u32, true), (3, 4, false), (2, 3, false), (2, 2, true)] {
        let world = clock_off(504);
        let mut f = Fauna::new(configuration(L, maximum, DispersalAt::Adult));
        let mut plants = flora();
        let xs = [3.2, 2.6, 2.0];
        let kept: Vec<u64> = xs[..adults]
            .iter()
            .map(|&x| member(&mut f, &world, L, x, 1.5, false))
            .collect();
        let calf = member(&mut f, &world, L, 3.7, 1.5, true);
        // Dependent young in the herd never count, however many.
        for x in [2.3, 2.9, 3.4] {
            let y = member(&mut f, &world, L, x, 1.5, true);
            animal_mut(&mut f, y).reserve = 0.0;
        }
        let crossed = grow_and_disperse(&mut f, &world, &mut plants);
        assert_eq!(crossed, vec![calf]);
        assert!(kept.iter().all(|&id| alive(&f, id)), "adults never leave");
        assert_eq!(!alive(&f, calf), leaves, "{adults} adults, limit {maximum}");
        assert_eq!(f.ledger.deaths_accounted(), 0);
    }
}

#[test]
fn separate_herds_are_counted_separately_and_x_wraps() {
    let world = clock_off(505);
    let ring = f64::from(world.config().width) * world.config().voxel_m;
    let mut f = Fauna::new(configuration(L, 2, DispersalAt::Adult));
    let mut plants = flora();
    // Herd one straddles the seam: adults at 0.1 and ring - 0.3, a calf at ring - 0.8.
    let a = member(&mut f, &world, L, 0.1, 1.5, false);
    let b = member(&mut f, &world, L, ring - 0.3, 1.5, false);
    let seam_calf = member(&mut f, &world, L, ring - 0.8, 1.5, true);
    // Herd two, far away: one adult and its calf.
    let c = member(&mut f, &world, L, 6.0, 1.5, false);
    let far_calf = member(&mut f, &world, L, 6.3, 1.5, true);
    grow_and_disperse(&mut f, &world, &mut plants);
    assert!(alive(&f, a) && alive(&f, b) && alive(&f, c));
    assert!(!alive(&f, seam_calf), "two adults across the seam plus the calf exceed 2");
    assert!(alive(&f, far_calf), "the far herd has one adult plus its calf");
    assert_eq!(f.ledger.dispersed_by_founder[L.index()], 1);
}

#[test]
fn herd_labels_join_chains_and_wrap_x() {
    let world = clock_off(512);
    let ring = f64::from(world.config().width) * world.config().voxel_m;
    let mut f = Fauna::new(FaunaConfig::default());
    for x in [0.2, ring - 0.2, 5.0, 5.5, 6.0, 9.0] {
        member(&mut f, &world, L, x, 1.5, false);
    }
    let refs: Vec<&crate::Animal> = f.animals.iter().collect();
    assert_eq!(crate::herd_labels(&refs, 0.6, ring), vec![0, 0, 2, 2, 2, 5]);
    assert_eq!(crate::herd_labels(&refs, 0.3, ring), vec![0, 1, 2, 3, 4, 5]);
}

#[test]
fn dispersal_transfers_every_inventory_to_the_leavers_face_without_an_external_term() {
    let world = clock_off(505);
    let mut f = Fauna::new(configuration(L, 1, DispersalAt::Adult));
    let mut plants = flora();
    member(&mut f, &world, L, 2.0, 1.5, false);
    let calf = member(&mut f, &world, L, 2.1, 1.5, true);
    let a = animal_mut(&mut f, calf);
    a.gut = Taken {
        organic: 0.07,
        mineral: 0.02,
        energy: 0.03,
    };
    a.stomach.pools[crate::Food::Foliage.index()] = Taken {
        organic: 0.11,
        mineral: 0.04,
        energy: 0.05,
    };
    a.gut_seeds.lots[0] = GutSeedLot {
        plant: Some(SeedPlant::Latticevine),
        organic: 0.13,
        mineral: 0.06,
        energy: 0.07,
        source: a.site,
        ..GutSeedLot::default()
    };
    a.reproduction.escrow = Some(Escrow {
        organic: 0.17,
        mineral: 0.08,
        energy: 0.09,
        ..Escrow::default()
    });
    let held = animal(&f, calf);
    let before = [f.view().organic(), f.view().mineral(), f.view().energy()];
    let seeded_mineral = plants.view().ledger.seeded_mineral_in;
    let removed = [
        f.ledger.removed_organic_out,
        f.ledger.removed_mineral_out,
        f.ledger.removed_energy_out,
    ];
    grow_and_disperse(&mut f, &world, &mut plants);
    let g = plants.view().ground_at(held.site).unwrap().clone();
    let parcel = [
        held.stored_organic(),
        held.stored_mineral(),
        held.stored_energy(),
    ];
    let after = [f.view().organic(), f.view().mineral(), f.view().energy()];
    for i in 0..3 {
        near(before[i] - after[i], parcel[i]);
    }
    near(g.litter, parcel[0]);
    near(g.litter_mineral, parcel[1]);
    near(plants.view().ledger.seeded_mineral_in, seeded_mineral);
    near(g.litter_energy, parcel[2]);
    near(f.ledger.deposited_organic_out, parcel[0]);
    near(f.ledger.deposited_mineral_out, parcel[1]);
    near(f.ledger.deposited_energy_out, parcel[2]);
    assert_eq!(
        [
            f.ledger.removed_organic_out,
            f.ledger.removed_mineral_out,
            f.ledger.removed_energy_out
        ],
        removed
    );
    assert_eq!(f.ledger.deaths_accounted(), 0);
    assert_eq!(f.ledger.dispersed_by_founder[L.index()], 1);
}

#[test]
fn disabling_dispersal_matches_growth_without_a_dispersal_call() {
    let world = clock_off(506);
    let mut config = configuration(L, 1, DispersalAt::Adult);
    config.founders[L.index()].dispersal.enabled = false;
    let mut f = Fauna::new(config);
    member(&mut f, &world, L, 2.0, 1.5, false);
    member(&mut f, &world, L, 2.1, 1.5, true);
    let mut baseline = Fauna::load(&f.save()).expect("fixture loads");
    let mut plants = flora();
    for _ in 0..3 {
        grow(&mut baseline);
        grow_and_disperse(&mut f, &world, &mut plants);
        assert_eq!(
            f.save(),
            baseline.save(),
            "off follows the ordinary growth path"
        );
        assert_eq!(f.ledger.dispersed_by_founder[L.index()], 0);
        assert!(plants.view().ground.is_empty());
    }
}

fn independence(f: &mut Fauna, world: &World, plants: &mut Flora) {
    let before: Vec<u64> = f
        .animals
        .iter()
        .filter(|a| a.pack.is_some_and(|p| p.pup))
        .map(|a| a.id)
        .collect();
    crate::pack::upkeep(f, &world.view());
    let transitioned: Vec<u64> = before
        .into_iter()
        .filter(|&id| {
            f.view()
                .animal(id)
                .is_some_and(|a| a.pack.is_some_and(|p| !p.pup))
        })
        .collect();
    disperse(
        f,
        &world.view(),
        plants,
        &transitioned,
        DispersalAt::Independent,
    );
}

#[test]
fn repeated_veilhound_cohorts_keep_the_breeder_and_limit_independent_members() {
    let world = clock_off(507);
    let mut c = configuration(V, 3, DispersalAt::Independent);
    c.founders[V.index()].core.reserve_cap = 100.0;
    let p = c.founders[V.index()].pack.as_mut().unwrap();
    p.pack_size = 2;
    p.independence_age_s = 0.1;
    let mut f = Fauna::new(c);
    assert!(f.apply(
        &world,
        crate::Command::FoundPack {
            den: Site {
                x: 8,
                y: SURFACE_Y,
                z: 6
            },
            founder: V,
            stores: StartingStores::FULL,
            heading_rad: EAST
        }
    ));
    let pack = f.packs()[0].id;
    let breeder = f.pack(pack).unwrap().breeder.unwrap();
    let established: Vec<u64> = f.animals.iter().map(|a| a.id).collect();
    let mut plants = flora();
    for _ in 0..3 {
        f.set_reserve_fraction(breeder, 1.0);
        let pups = f.whelp(&world.view(), pack, 2).unwrap();
        independence(&mut f, &world, &mut plants);
        assert!(
            pups.iter().all(|&id| alive(&f, id)),
            "dependent pups remain"
        );
        let dependent = f
            .animals
            .iter()
            .filter(|a| a.pack.is_some_and(|p| p.pup))
            .count();
        assert!(f.animals.len() <= 3 + dependent);
        for id in pups {
            animal_mut(&mut f, id).age_ticks = 3;
        }
        independence(&mut f, &world, &mut plants);
        assert!(f.animals.len() <= 3);
        assert!(established.iter().all(|&id| alive(&f, id)));
        assert_eq!(f.pack(pack).unwrap().breeder, Some(breeder));
    }
    assert!(f.ledger.dispersed_by_founder[V.index()] > 0);
    assert_eq!(f.ledger.deaths_accounted(), 0);
}

#[test]
fn dependent_pups_do_not_count_and_an_independent_pup_counts_established_plus_itself() {
    for (maximum, leaves) in [(3u32, false), (2, true)] {
        let world = clock_off(513);
        let mut c = configuration(V, maximum, DispersalAt::Independent);
        c.founders[V.index()].core.reserve_cap = 100.0;
        let p = c.founders[V.index()].pack.as_mut().unwrap();
        p.pack_size = 2;
        p.independence_age_s = 0.1;
        let mut f = Fauna::new(c);
        assert!(f.apply(
            &world,
            crate::Command::FoundPack {
                den: Site { x: 8, y: SURFACE_Y, z: 6 },
                founder: V,
                stores: StartingStores::FULL,
                heading_rad: EAST
            }
        ));
        let pack = f.packs()[0].id;
        let breeder = f.pack(pack).unwrap().breeder.unwrap();
        let adults: Vec<u64> = f.animals.iter().map(|a| a.id).collect();
        assert_eq!(adults.len(), 2);
        f.set_reserve_fraction(breeder, 1.0);
        let pups = f.whelp(&world.view(), pack, 4).unwrap();
        assert!(pups.len() > 1, "a litter larger than one");
        let mut plants = flora();
        // One pup reaches independence; its siblings stay dependent at the den.
        animal_mut(&mut f, pups[0]).age_ticks = 3;
        independence(&mut f, &world, &mut plants);
        assert!(pups[1..].iter().all(|&id| alive(&f, id)), "dependent pups never leave");
        assert_eq!(!alive(&f, pups[0]), leaves, "2 adults plus itself against limit {maximum}");
        assert!(adults.iter().all(|&id| alive(&f, id)));
        assert_eq!(f.pack(pack).unwrap().breeder, Some(breeder));
    }
}

#[test]
fn loftstrider_season_limits_conceptions_and_births_follow_gestation() {
    // 1.2 s per year lets four years fit into fewer than 100 fixture ticks.
    let mut world = World::empty(cubarium_voxel::Config {
        width: 16,
        height: 8,
        depth: 12,
        voxel_m: VOXEL_M,
        seed: 508,
        day: DayClock {
            mode: DayMode::Off,
            length_min: 0.02,
            ..DayClock::default()
        },
        climate: Climate {
            dry_season: false,
            dry_season_every_days: 1.0,
            dry_season_phase_days: 0.0,
            ..Climate::default()
        },
        ..cubarium_voxel::Config::default()
    });
    for z in 0..12 {
        for x in 0..16 {
            for y in 1..=SURFACE_Y {
                world.apply(cubarium_voxel::Command::SetMaterial {
                    x,
                    y,
                    z,
                    material: cubarium_voxel::Material::Soil,
                });
            }
        }
    }
    let mut c = FaunaConfig::default();
    let rule = Reproduction {
        surplus_hold_s: 0.05,
        surplus_floor: 0.0,
        gestation_s: 0.4 * world.config().climate.year_s(&world.config().day),
        birth_interval_s: 0.05,
        litters_per_year: 1,
        breeding_phase: 0.125,
        breeding_window: 0.25,
        ..Reproduction::LIVE_BIRTH_PLACEHOLDER
    };
    c.founders[L.index()].core.reproduction = rule;
    let mut f = Fauna::new(c);
    let parent = introduce(&mut f, &world, L, StartingStores::FULL);
    let mut openings = Vec::new();
    let mut births = Vec::new();
    let year_s = world.config().climate.year_s(&world.config().day);
    let year_ticks = (year_s / super::DT).round() as u64;
    for tick in 0..4 * year_ticks {
        let index = f.animals.iter().position(|a| a.id == parent).unwrap();
        let sc = crate::body::effective_config(&f.config, &f.animals[index]);
        f.animals[index].reserve = sc.reproduction.package_cost(&sc) * 10.0;
        super::advance_eligibility(&mut f, index, &sc, &rule);
        let opened = f.ledger.gestations_opened;
        let mut newborns = Vec::new();
        let year = world.view().year();
        super::gestate(
            &mut f,
            index,
            &sc,
            &rule,
            (world.config().seed, year),
            &mut newborns,
        );
        if f.ledger.gestations_opened > opened {
            let y = year.unwrap();
            let distance = (y.phase - rule.breeding_phase).rem_euclid(1.0);
            assert!(
                distance < rule.breeding_window,
                "conception outside the window"
            );
            openings.push((tick, y.number));
        }
        if !newborns.is_empty() {
            births.push(tick);
        }
        world.advance_tick();
    }
    assert_eq!(
        openings.len(),
        4,
        "one well-funded conception in each fixture year"
    );
    assert_eq!(births.len(), openings.len());
    for ((opened, year), born) in openings.iter().zip(&births) {
        assert_eq!(*born - *opened, rule.gestation_ticks() - 1);
        assert!(
            openings.iter().filter(|(_, y)| y == year).count() <= rule.litters_per_year as usize
        );
        let phase = (*born % year_ticks) as f64 / year_ticks as f64;
        assert!(
            (phase - rule.breeding_phase).rem_euclid(1.0) >= rule.breeding_window,
            "the gate controls conception; birth can follow outside the window"
        );
    }
}

#[test]
fn a_pack_breeder_is_protected_even_if_it_is_a_transition_candidate() {
    let world = clock_off(509);
    let mut c = configuration(V, 1, DispersalAt::Independent);
    c.founders[V.index()].pack.as_mut().unwrap().pack_size = 2;
    let mut f = Fauna::new(c);
    assert!(f.apply(
        &world,
        crate::Command::FoundPack {
            den: Site {
                x: 8,
                y: SURFACE_Y,
                z: 6
            },
            founder: V,
            stores: StartingStores::FULL,
            heading_rad: EAST,
        }
    ));
    let breeder = f.packs()[0].breeder.unwrap();
    let mut plants = flora();
    disperse(
        &mut f,
        &world.view(),
        &mut plants,
        &[breeder],
        DispersalAt::Independent,
    );
    assert!(alive(&f, breeder));
    assert_eq!(f.ledger.dispersed_by_founder[V.index()], 0);
    assert!(plants.view().ground.is_empty());
}

#[test]
fn litter_energy_overflow_is_heat_and_the_whole_closed_world_still_conserves() {
    let world = clock_off(510);
    let mut f = Fauna::new(configuration(L, 1, DispersalAt::Adult));
    let mut plants = Flora::new(FloraConfig {
        litter_energy_cap: 0.01,
        ..FloraConfig::default()
    });
    member(&mut f, &world, L, 2.0, 1.5, false);
    let calf = member(&mut f, &world, L, 2.1, 1.5, true);
    let a = animal_mut(&mut f, calf);
    a.energy = a.stored_organic() * 2.0;
    let held = animal(&f, calf);
    grow_and_disperse(&mut f, &world, &mut plants);
    let g = plants.view().ground_at(held.site).unwrap().clone();
    assert!(plants.view().ledger.heat_out > 0.0);
    near(
        g.litter_energy + plants.view().ledger.heat_out,
        held.stored_energy(),
    );
    near(g.litter, held.stored_organic());
    near(g.litter_mineral, held.stored_mineral());
    assert_eq!(f.ledger.removed_energy_out, 0.0);
}

#[test]
fn the_session_dispersal_counter_does_not_change_saved_state_bytes() {
    let world = clock_off(511);
    let mut f = Fauna::new(configuration(L, 1, DispersalAt::Adult));
    member(&mut f, &world, L, 2.0, 1.5, false);
    let before = f.save();
    f.ledger.dispersed_by_founder[L.index()] = 7;
    assert_eq!(
        f.save(),
        before,
        "the transient counter adds no saved state"
    );
    let resumed = Fauna::load(&f.save()).expect("unchanged state format loads");
    assert_eq!(
        resumed.ledger.dispersed_by_founder[L.index()],
        0,
        "the counter starts a new session at load"
    );
}
