//! Contract checks for changing the mass of an already saved lineage.

use super::*;
use cubarium_voxel::{Command as WorldCommand, Config as VoxelConfig, Material};

fn plain() -> World {
    let mut world = World::empty(VoxelConfig {
        width: 24,
        height: 12,
        depth: 12,
        voxel_m: 1.0,
        day: cubarium_voxel::weather::DayClock {
            mode: cubarium_voxel::weather::DayMode::Off,
            ..Default::default()
        },
        ..VoxelConfig::default()
    });
    for x in 0..24 {
        for z in 0..12 {
            world.apply(WorldCommand::SetMaterial {
                x,
                y: 1,
                z,
                material: Material::Soil,
            });
        }
    }
    world
}

fn currencies(t: Taken) -> [f64; 3] {
    [t.organic, t.mineral, t.energy]
}

fn stocks(a: &Animal) -> Vec<f64> {
    let mut values = vec![a.body, a.reserve, a.mineral, a.energy, a.reproduction.brood];
    values.extend(currencies(a.gut));
    for pool in a.stomach.pools {
        values.extend(currencies(pool));
    }
    for lot in a.gut_seeds.lots {
        values.extend([lot.organic, lot.mineral, lot.energy]);
    }
    if let Some(e) = a.reproduction.escrow {
        values.extend([e.organic, e.mineral, e.energy]);
    }
    values
}

fn totals(f: &Fauna) -> [f64; 3] {
    let v = f.view();
    [v.organic(), v.mineral(), v.energy()]
}

fn introduced(f: &Fauna) -> [f64; 3] {
    [
        f.ledger.introduced_organic_in,
        f.ledger.introduced_mineral_in,
        f.ledger.introduced_energy_in,
    ]
}

fn close(actual: f64, expected: f64) {
    assert!(
        (actual - expected).abs() <= 1e-12 * expected.abs().max(1e-12),
        "{actual} != {expected}"
    );
}

fn books_close(f: &Fauna) {
    let expected = [
        f.ledger.expected_organic(),
        f.ledger.expected_mineral(),
        f.ledger.expected_energy(),
    ];
    for (held, booked) in totals(f).into_iter().zip(expected) {
        close(held, booked);
    }
}

fn saved_fixture() -> Fauna {
    let world = plain();
    let mut f = Fauna::new(FaunaConfig::default());
    for lineage in Founder::ALL {
        let body = f.config.founder(lineage).core.body_max;
        let site = Site { x: 3, y: 1, z: 3 };
        assert!(f.introduce_body(
            &world,
            site,
            Species::Frondgrazer,
            Some(lineage),
            body,
            None,
            0.0
        ));
        let a = f.animals.last_mut().unwrap();
        // Deliberately different inventories make omitted compartments visible.
        a.gut = Taken {
            organic: 0.11 * body,
            mineral: 0.013 * body,
            energy: 0.17 * body,
        };
        for (i, pool) in a.stomach.pools.iter_mut().enumerate() {
            let share = (i + 1) as f64 * body / 100.0;
            *pool = Taken {
                organic: share,
                mineral: share / 9.0,
                energy: share * 3.0,
            };
        }
        for (i, lot) in a.gut_seeds.lots.iter_mut().enumerate() {
            let share = (i + 1) as f64 * body / 200.0;
            *lot = GutSeedLot {
                plant: Some(SeedPlant::Latticevine),
                organic: share,
                mineral: share / 7.0,
                energy: share * 4.0,
                source: site,
                opened_tick: 0,
                due_tick: 100,
            };
        }
        a.reproduction.escrow = Some(Escrow {
            organic: 0.07 * body,
            mineral: 0.009 * body,
            energy: 0.19 * body,
            ticks: 7,
            young: 2,
        });
        a.reproduction.brood = 0.2 * a.reserve;
        a.reproduction.given = 0.23 * body;
        a.wound = 0.125;
        f.clutches.push(Clutch {
            site,
            count: 2,
            organic: 0.03 * body,
            mineral: 0.005 * body,
            energy: 0.09 * body,
            laid_tick: 0,
            lineage,
            dormancy: None,
            wet_ticks: 3,
            dry_ticks: 4,
            face: None,
        });
    }
    // The fixture's extra stores are introduced, just as manually founding them is.
    let held = totals(&f);
    f.ledger.introduced_organic_in = held[0];
    f.ledger.introduced_mineral_in = held[1];
    f.ledger.introduced_energy_in = held[2];
    let back = Fauna::load(&f.save()).expect("saved inventories round-trip");
    books_close(&back);
    back
}

fn scaled_config(f: &Fauna, ratio: f64) -> FaunaConfig {
    let mut config = f.config.clone();
    for lineage in Founder::ALL {
        config.founders[lineage.index()].scale_mass(ratio);
    }
    config
}

#[test]
fn saved_lineage_stocks_and_eggs_scale_up_and_down_by_the_mass_ratio() {
    for ratio in [2.5, 0.4] {
        let mut loaded = saved_fixture();
        let animals = loaded.animals.clone();
        let clutches = loaded.clutches.clone();
        let config = scaled_config(&loaded, ratio);
        let ratios = Founder::ALL
            .map(|f| config.founder(f).mass_scale / loaded.config.founder(f).mass_scale);
        loaded.replace_config(config).unwrap();
        for (before, after) in animals.iter().zip(&loaded.animals) {
            let k = ratios[before.founder.unwrap().index()];
            for (old, new) in stocks(before).into_iter().zip(stocks(after)) {
                close(new, old * k);
            }
            assert_eq!(
                after.reproduction.escrow.unwrap().ticks,
                before.reproduction.escrow.unwrap().ticks
            );
            assert_eq!(
                after.reproduction.escrow.unwrap().young,
                before.reproduction.escrow.unwrap().young
            );
            assert_eq!(
                after.gut_seeds.lots[0].due_tick,
                before.gut_seeds.lots[0].due_tick
            );
            assert_eq!(after.wound.to_bits(), before.wound.to_bits());
            assert_eq!(
                after.reproduction.given.to_bits(),
                before.reproduction.given.to_bits(),
                "past throughput is not held matter"
            );
        }
        for (before, after) in clutches.iter().zip(&loaded.clutches) {
            let k = ratios[before.lineage.index()];
            for (old, new) in [before.organic, before.mineral, before.energy]
                .into_iter()
                .zip([after.organic, after.mineral, after.energy])
            {
                close(new, old * k);
            }
            assert_eq!(
                (
                    after.count,
                    after.laid_tick,
                    after.wet_ticks,
                    after.dry_ticks
                ),
                (
                    before.count,
                    before.laid_tick,
                    before.wet_ticks,
                    before.dry_ticks
                )
            );
        }
    }
}

#[test]
fn rescaled_saved_stocks_book_the_signed_net_change_in_each_currency() {
    for ratio in [2.5, 0.4] {
        let mut loaded = saved_fixture();
        let held_before = totals(&loaded);
        let in_before = introduced(&loaded);
        let config = scaled_config(&loaded, ratio);
        loaded.replace_config(config).unwrap();
        for ((old, new), (old_in, new_in)) in held_before
            .into_iter()
            .zip(totals(&loaded))
            .zip(in_before.into_iter().zip(introduced(&loaded)))
        {
            close(new_in - old_in, new - old);
            assert_eq!((new_in - old_in).is_sign_positive(), ratio > 1.0);
        }
        books_close(&loaded);
        books_close(&Fauna::load(&loaded.save()).unwrap());
    }
}

#[test]
fn an_unchanged_mass_preserves_saved_stocks_bit_for_bit() {
    let mut loaded = saved_fixture();
    loaded.animals[0].gut.energy = -0.0;
    let animals = loaded.animals.clone();
    let clutches = loaded.clutches.clone();
    let ledger = loaded.ledger.clone();
    loaded.replace_config(loaded.config.clone()).unwrap();
    for (before, after) in animals.iter().zip(&loaded.animals) {
        assert_eq!(
            stocks(before)
                .iter()
                .map(|x| x.to_bits())
                .collect::<Vec<_>>(),
            stocks(after)
                .iter()
                .map(|x| x.to_bits())
                .collect::<Vec<_>>()
        );
    }
    assert_eq!(loaded.animals, animals);
    assert_eq!(loaded.clutches, clutches);
    assert_eq!(loaded.ledger, ledger);
}

#[test]
fn changing_one_lineage_preserves_other_lineages_and_their_eggs() {
    let mut loaded = saved_fixture();
    let animals = loaded.animals.clone();
    let clutches = loaded.clutches.clone();
    let mut config = loaded.config.clone();
    config.founders[Founder::Browser.index()].scale_mass(3.0);
    loaded.replace_config(config).unwrap();
    for (before, after) in animals.iter().zip(&loaded.animals) {
        if before.founder != Some(Founder::Browser) {
            assert_eq!(before, after);
        }
    }
    for (before, after) in clutches.iter().zip(&loaded.clutches) {
        if before.lineage != Founder::Browser {
            assert_eq!(before, after);
        }
    }
    books_close(&loaded);
}

fn loaded_bodies_survive(ratio: f64) {
    let world = plain();
    let mut fauna = Fauna::new(FaunaConfig::default());
    let core = fauna.config.founder(Founder::Browser).core;
    for x in [3, 8] {
        assert!(fauna.introduce_body(
            &world,
            Site { x, y: 1, z: 3 },
            Species::Frondgrazer,
            Some(Founder::Browser),
            core.body_max,
            None,
            0.0
        ));
    }
    let mut loaded = Fauna::load(&fauna.save()).unwrap();
    let mut config = loaded.config.clone();
    config.founders[Founder::Browser.index()].scale_mass(ratio);
    loaded.replace_config(config).unwrap();
    let minimum = loaded.config.founder(Founder::Browser).core.body_min;
    assert!(loaded.animals.iter().all(|a| a.body >= minimum));
    let ids: Vec<_> = loaded.animals.iter().map(|a| a.id).collect();
    loaded.set_births_enabled(false);
    loaded.step_with(
        &world,
        &mut Flora::new(cubarium_voxel_flora::FloraConfig::default()),
        1,
    );
    for id in ids {
        assert!(
            loaded.view().animal(id).is_some(),
            "the rescaled saved body survives one tick"
        );
    }
    books_close(&loaded);
}

#[test]
fn saved_bodies_remain_above_the_new_minimum_and_alive_after_scaling_up() {
    loaded_bodies_survive(40.0);
}

#[test]
fn saved_bodies_remain_above_the_new_minimum_and_alive_after_scaling_down() {
    loaded_bodies_survive(0.25);
}
