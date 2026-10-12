//! Fuel is mineral-free food energy: it pays running costs before tissue, while
//! the bite's mineral still bounds every unit added to body and reserve.

use super::predation_fixture::{assert_closes, clock_off, introduce};
use super::{assimilate, drain_riders, grow, kill, maintenance, respire, Respiration};
use crate::{
    Actions, Attachment, Command, Departure, Fauna, FaunaConfig, Food, Founder, MobilityMode, Pose,
    Scripted, StartingStores,
};
use cubarium_voxel::World;
use cubarium_voxel_flora::{Flora, FloraConfig, Site, Taken};

fn config(founder: Founder, fuel_cap: f64) -> FaunaConfig {
    let mut c = FaunaConfig::default();
    for p in &mut c.founders {
        p.gut.s0 = 0.0;
        p.core.lifespan_median_s = 0.0;
    }
    let p = &mut c.founders[founder.index()];
    p.core.fuel_cap = fuel_cap;
    p.core.reserve_cap = 2.0;
    p.core.n_tissue = 0.1;
    p.core.energy_density = 2.0;
    p.core.growth_max_per_s = 0.0;
    p.core.maintenance_per_s = 0.0;
    p.yield_by_food[Food::Nectar.index()] = 0.8;
    c
}

/// A measured bite introduced directly, as in the dung tests. All three incoming
/// currencies are booked, so the existing conservation helper can check each law.
fn feed(fauna: &mut Fauna, i: usize, t: Taken, yield_fraction: f64) -> f64 {
    fauna.ledger.introduced_organic_in += t.organic;
    fauna.ledger.introduced_mineral_in += t.mineral;
    fauna.ledger.introduced_energy_in += t.energy;
    let sc = super::body::effective_config(&fauna.config, &fauna.animals[i]);
    assimilate(fauna, i, &sc, t, yield_fraction)
}

fn empty_reserve() -> StartingStores {
    StartingStores {
        body: 1.0,
        reserve: 0.0,
    }
}

fn close(got: f64, want: f64) {
    assert!(
        (got - want).abs() <= 1e-12 * want.abs().max(1e-6),
        "{got} against {want}"
    );
}

/// The twins have identical nectar, motion and upkeep. Their chosen test physiology
/// makes the unfunded sugar sufficient to pay their airborne running costs.
fn flyer(fuel_cap: f64) -> (World, Flora, Fauna) {
    let world = clock_off(41);
    let mut c = config(Founder::Bellwing, fuel_cap);
    let p = &mut c.founders[Founder::Bellwing.index()];
    p.core.maintenance_per_s = 0.02;
    let f = p.flight.as_mut().expect("a flyer");
    f.flight_factor = 3.0;
    f.hover_factor = 3.0;
    let mut fauna = Fauna::new(c);
    fauna.set_births_enabled(false);
    assert!(fauna.apply(
        &world,
        Command::IntroduceAloft {
            pose: Pose {
                x: 2.0,
                z: 1.5,
                y_m: 1.25,
                heading_rad: std::f64::consts::FRAC_PI_2,
            },
            founder: Founder::Bellwing,
            stores: StartingStores {
                body: 1.0,
                reserve: 0.1
            },
        }
    ));
    let id = fauna.animals[0].id;
    assert!(fauna.set_controller(
        id,
        Box::new(
            Scripted::new(vec![Actions {
                forward: 1.0,
                turn: 0.0,
                feed: 0.0
            }])
            .lifting(0.0)
        )
    ));
    (world, Flora::new(FloraConfig::default()), fauna)
}

fn nectar_meal(fauna: &Fauna, body: f64) -> Taken {
    let n = fauna.config.founder(Founder::Bellwing).core.n_tissue;
    Taken {
        organic: body * 0.01,
        mineral: n * body * 0.0025,
        energy: body * 0.07,
    }
}

#[test]
fn fuel_conserves_all_three_currencies_through_feeding_flight_and_carcass() {
    let (world, mut flora, mut fauna) = flyer(0.2);
    let body = fauna.animals[0].body;
    let start = fauna.animals[0].pose;
    let y = fauna
        .config
        .founder(Founder::Bellwing)
        .yield_for(Food::Nectar);
    let meal = nectar_meal(&fauna, body);
    for _ in 0..160 {
        feed(&mut fauna, 0, meal, y);
        let fuel = fauna.animals[0].fuel;
        fauna.step_with(&world, &mut flora, 1);
        assert!(fauna.animals[0].fuel < fuel, "upkeep and flight spend fuel");
        assert_closes(&fauna, "a fed flying tick");
    }
    let a = fauna.animals[0];
    assert!(
        a.pose.x != start.x || a.pose.z != start.z,
        "the animal moved"
    );
    assert_eq!(a.mobility.mode, MobilityMode::Fly);
    assert!(a.fuel > 0.0, "the corpse must carry fuel");
    assert!(fauna.ledger.upkeep_by_founder[Founder::Bellwing.index()] > 0.0);
    assert!(fauna.ledger.motor_by_founder[Founder::Bellwing.index()] > 0.0);
    let (organic, mineral, energy) = {
        let v = fauna.view();
        (v.organic(), v.mineral(), v.energy())
    };
    kill(
        &mut fauna,
        &mut flora,
        0,
        Departure::Killed {
            by: Founder::Lanternjaw,
        },
        Some(&world.view()),
    );
    assert!(fauna.animals.is_empty());
    let v = flora.view();
    let g = v.ground_at(a.site).expect("the fuel-bearing carcass");
    close(g.carrion, organic);
    close(g.carrion_mineral, mineral);
    close(g.carrion_energy, energy);
    // Once no animals remain, cancellation dust in the cumulative ledger must be
    // measured against the matter handled, rather than against zero final holdings.
    let empty = fauna.view();
    for (held, expected, scale) in [
        (
            empty.organic(),
            empty.ledger.expected_organic(),
            empty.ledger.introduced_organic_in,
        ),
        (
            empty.mineral(),
            empty.ledger.expected_mineral(),
            empty.ledger.introduced_mineral_in,
        ),
        (
            empty.energy(),
            empty.ledger.expected_energy(),
            empty.ledger.introduced_energy_in,
        ),
    ] {
        assert!((held - expected).abs() <= 1e-12 * scale.abs().max(1e-6));
    }
    for (held, expected) in [
        (v.organic(), v.ledger.expected_organic()),
        (v.mineral(), v.ledger.expected_mineral()),
        (v.energy(), v.ledger.expected_energy()),
    ] {
        close(held, expected);
    }
}

#[test]
fn fuel_never_borrows_mineral_and_tissue_stays_within_the_food_mineral_budget() {
    let world = clock_off(42);
    let mut c = config(Founder::Browser, 0.5);
    c.founders[Founder::Browser.index()].core.growth_max_per_s = 1.0;
    let mut fauna = Fauna::new(c);
    introduce(
        &mut fauna,
        &world,
        Founder::Browser,
        StartingStores {
            body: 0.5,
            reserve: 0.0,
        },
    );
    let before = fauna.animals[0];
    let n = fauna.config.founder(Founder::Browser).core.n_tissue;
    let bite = before.body * 0.01;
    let y = 0.8;
    let sugar = Taken {
        organic: bite,
        mineral: 0.0,
        energy: bite * 7.0,
    };
    assert_eq!(feed(&mut fauna, 0, sugar, y), 0.0);
    assert!(fauna.animals[0].fuel > 0.0);
    assert_eq!(fauna.animals[0].mineral, before.mineral);
    assert_eq!(
        fauna.animals[0].body + fauna.animals[0].reserve,
        before.body + before.reserve
    );
    let gut_mineral = fauna.animals[0].gut.mineral;
    let cost = fauna.animals[0].fuel / 2.0;
    respire(&mut fauna, 0, cost, Respiration::Motor);
    assert_eq!(fauna.animals[0].mineral, before.mineral);
    assert_eq!(
        fauna.animals[0].gut.mineral, gut_mineral,
        "fuel sheds no mineral"
    );

    let meal = Taken {
        organic: bite,
        mineral: n * bite * 0.25,
        energy: bite * 7.0,
    };
    let mut eaten_mineral = 0.0;
    for _ in 0..24 {
        feed(&mut fauna, 0, meal, y);
        eaten_mineral += meal.mineral;
        grow(&mut fauna);
        let a = fauna.animals[0];
        let built = a.body + a.reserve - before.body - before.reserve;
        assert!(built <= eaten_mineral / n + 1e-12 * before.body);
        close(a.mineral - before.mineral, built * n);
        assert_closes(&fauna, "fuel and tissue have separate mineral budgets");
    }
    assert!(
        fauna.animals[0].body > before.body,
        "mineral-funded tissue grew"
    );
}

#[test]
fn nectar_fuel_preserves_more_body_and_reserve_under_the_same_flight_costs() {
    let (world, mut flora_fuel, mut fueled) = flyer(0.2);
    let (_, mut flora_no_fuel, mut unfueled) = flyer(0.0);
    let body = fueled.animals[0].body;
    let meal = nectar_meal(&fueled, body);
    let y = fueled
        .config
        .founder(Founder::Bellwing)
        .yield_for(Food::Nectar);
    assert!(
        meal.mineral / fueled.config.founder(Founder::Bellwing).core.n_tissue < y * meal.organic
    );
    for _ in 0..120 {
        feed(&mut fueled, 0, meal, y);
        feed(&mut unfueled, 0, meal, y);
        fueled.step_with(&world, &mut flora_fuel, 1);
        unfueled.step_with(&world, &mut flora_no_fuel, 1);
    }
    let (a, b) = (fueled.animals[0], unfueled.animals[0]);
    assert!(a.body + a.reserve > b.body + b.reserve);
    assert!(a.fuel > 0.0);
    assert_eq!(b.fuel, 0.0);
    close(
        fueled.ledger.upkeep_by_founder[Founder::Bellwing.index()],
        unfueled.ledger.upkeep_by_founder[Founder::Bellwing.index()],
    );
    close(
        fueled.ledger.motor_by_founder[Founder::Bellwing.index()],
        unfueled.ledger.motor_by_founder[Founder::Bellwing.index()],
    );
    assert_closes(&fueled, "the fueled twin");
    assert_closes(&unfueled, "the unfueled twin");
}

#[test]
fn a_tick_of_maintenance_spends_fuel_before_reserve_and_sheds_no_mineral() {
    let world = clock_off(43);
    let mut c = config(Founder::Browser, 0.5);
    c.founders[Founder::Browser.index()].core.maintenance_per_s = 0.02;
    let mut fauna = Fauna::new(c);
    introduce(
        &mut fauna,
        &world,
        Founder::Browser,
        StartingStores {
            body: 1.0,
            reserve: 0.25,
        },
    );
    let bite = fauna.animals[0].body * 0.1;
    feed(
        &mut fauna,
        0,
        Taken {
            organic: bite,
            mineral: 0.0,
            energy: bite * 7.0,
        },
        0.8,
    );
    let before = fauna.animals[0];
    let sc = fauna.config.founder(Founder::Browser).core;
    let cost = sc.maintenance_per_s * before.body * crate::DT;
    assert!(
        before.fuel >= cost,
        "the fixture can pay entirely from fuel"
    );
    let heat = fauna.ledger.heat_out;
    maintenance(&mut fauna);
    let a = fauna.animals[0];
    assert!(a.fuel < before.fuel);
    assert_eq!(a.reserve, before.reserve);
    assert_eq!(a.body, before.body);
    assert_eq!(a.energy, before.energy, "the tissue energy was untouched");
    assert_eq!(a.mineral, before.mineral);
    assert_eq!(a.gut.mineral, before.gut.mineral);
    close(
        fauna.ledger.heat_out - heat,
        cost * before.fuel_energy / before.fuel,
    );
    close(
        fauna.ledger.upkeep_by_founder[Founder::Browser.index()],
        cost,
    );
    assert_closes(&fauna, "one fuel-paid upkeep tick");
}

#[test]
fn fuel_overflow_is_digestion_respiration_at_the_bites_own_energy_density() {
    let world = clock_off(44);
    let mut fauna = Fauna::new(config(Founder::Browser, 0.1));
    introduce(&mut fauna, &world, Founder::Browser, empty_reserve());
    let a = fauna.animals[0];
    let cap = fauna.config.founder(Founder::Browser).core.fuel_cap * a.body;
    let t = Taken {
        organic: a.body,
        mineral: 0.0,
        energy: a.body * 7.0,
    };
    let y = 0.8;
    for _ in 0..3 {
        let before = fauna.animals[0];
        let (digestion, heat) = (fauna.ledger.respired_digestion_out, fauna.ledger.heat_out);
        feed(&mut fauna, 0, t, y);
        let a = fauna.animals[0];
        assert!(a.fuel <= cap + 1e-12 * cap);
        let stored = a.fuel - before.fuel;
        let overflow = y * t.organic - stored;
        assert!(overflow > 0.0, "the fixture overfills the cap");
        close(fauna.ledger.respired_digestion_out - digestion, overflow);
        close(
            fauna.ledger.heat_out - heat,
            overflow * t.energy / t.organic,
        );
        close(
            a.fuel_energy - before.fuel_energy,
            stored * t.energy / t.organic,
        );
        assert_eq!(a.mineral, before.mineral);
        assert_closes(&fauna, "fuel overflow");
    }
}

#[test]
fn mineral_funded_food_never_accumulates_fuel() {
    let world = clock_off(45);
    let mut fauna = Fauna::new(config(Founder::Browser, 0.5));
    introduce(&mut fauna, &world, Founder::Browser, empty_reserve());
    let n = fauna.config.founder(Founder::Browser).core.n_tissue;
    let bite = fauna.animals[0].body * 0.01;
    let t = Taken {
        organic: bite,
        mineral: n * bite,
        energy: bite * 3.0,
    };
    let before = fauna.animals[0];
    for _ in 0..20 {
        feed(&mut fauna, 0, t, 0.8);
        assert_eq!(fauna.animals[0].fuel, 0.0);
        assert_eq!(fauna.animals[0].fuel_energy, 0.0);
        assert_closes(&fauna, "mineral funds the food yield");
    }
    assert!(fauna.animals[0].reserve > before.reserve);
}

#[test]
fn every_running_cost_spends_fuel_without_charging_the_brood() {
    for kind in [
        Respiration::Maintenance,
        Respiration::Motor,
        Respiration::Digestion,
        Respiration::Gestation,
        Respiration::Hunt,
        Respiration::Heal,
        Respiration::Provision,
    ] {
        let world = clock_off(46);
        let mut fauna = Fauna::new(config(Founder::Browser, 0.5));
        introduce(
            &mut fauna,
            &world,
            Founder::Browser,
            StartingStores {
                body: 1.0,
                reserve: 0.5,
            },
        );
        let bite = fauna.animals[0].body * 0.1;
        feed(
            &mut fauna,
            0,
            Taken {
                organic: bite,
                mineral: 0.0,
                energy: bite * 7.0,
            },
            0.8,
        );
        fauna.animals[0].reproduction.brood = fauna.animals[0].reserve / 2.0;
        let before = fauna.animals[0];
        let cost = before.fuel / 2.0;
        let (paid, from_body) = respire(&mut fauna, 0, cost, kind);
        let a = fauna.animals[0];
        close(paid, cost);
        assert_eq!(from_body, 0.0);
        assert_eq!(a.reserve, before.reserve);
        assert_eq!(
            a.reproduction.brood, before.reproduction.brood,
            "fuel is outside the brood's reserve share"
        );
        assert_eq!(a.mineral, before.mineral);
        assert_eq!(a.gut.mineral, before.gut.mineral);
        assert_closes(&fauna, "each running cost pays from fuel");
    }
}

#[test]
fn a_parasite_takes_fuel_as_fuel_and_respires_only_its_cap_overflow() {
    for room_share in [0.5, 2.0] {
        let world = clock_off(47);
        let mut c = config(Founder::Browser, 0.5);
        c.parasite_take_body_per_day = c.life.ecological_day_s * 0.2;
        c.parasite_host_bear_per_day = c.life.ecological_day_s * 0.2;
        let mut fauna = Fauna::new(c);
        let host = introduce(
            &mut fauna,
            &world,
            Founder::Browser,
            StartingStores {
                body: 1.0,
                reserve: 0.25,
            },
        );
        introduce(
            &mut fauna,
            &world,
            Founder::Lanternjaw,
            StartingStores {
                body: 0.25,
                reserve: 0.25,
            },
        );
        fauna.animals[1].attached = Some(Attachment { host });
        let bite = fauna.animals[0].body * 0.1;
        feed(
            &mut fauna,
            0,
            Taken {
                organic: bite,
                mineral: 0.0,
                energy: bite * 7.0,
            },
            0.8,
        );
        let (h0, r0) = (fauna.animals[0], fauna.animals[1]);
        let c = *fauna.config;
        let want = crate::rider_take_per_s(&c, r0.body)
            .min(c.parasite_host_bear_per_day / c.life.ecological_day_s * h0.body)
            * crate::DT;
        assert!(
            want > 0.0 && h0.fuel > want,
            "the host's fuel pays the whole drain"
        );
        let cap = room_share * want;
        fauna.config.founders[Founder::Lanternjaw.index()]
            .core
            .fuel_cap = cap / r0.body;
        let (digestion, heat, drain) = (
            fauna.ledger.respired_digestion_out,
            fauna.ledger.heat_out,
            fauna.ledger.parasite_taken_organic,
        );

        drain_riders(&mut fauna);

        let (h1, r1) = (fauna.animals[0], fauna.animals[1]);
        let stored = want.min(cap);
        let density = h0.fuel_energy / h0.fuel;
        close(h0.fuel - h1.fuel, want);
        close(r1.fuel - r0.fuel, stored);
        close(h0.fuel_energy - h1.fuel_energy, want * density);
        close(r1.fuel_energy - r0.fuel_energy, stored * density);
        assert!(r1.fuel <= cap + 1e-12 * cap);
        assert_eq!(h1.reserve, h0.reserve);
        assert_eq!(
            r1.reserve, r0.reserve,
            "mineral-free drain cannot build reserve"
        );
        assert_eq!(h1.body, h0.body);
        assert_eq!(r1.body, r0.body);
        assert_eq!(h1.mineral, h0.mineral);
        assert_eq!(r1.mineral, r0.mineral);
        assert_eq!(h1.energy, h0.energy);
        assert_eq!(r1.energy, r0.energy);
        close(fauna.ledger.parasite_taken_organic - drain, want);
        close(
            fauna.ledger.respired_digestion_out - digestion,
            want - stored,
        );
        close(fauna.ledger.heat_out - heat, (want - stored) * density);
        assert_closes(&fauna, "a parasite's fuel transfer and overflow");
    }
}

#[test]
fn mixed_fuel_and_reserve_spending_charges_only_the_reserve_share_to_brood() {
    let world = clock_off(48);
    let mut c = config(Founder::Browser, 0.5);
    c.founders[Founder::Browser.index()]
        .core
        .reproduction
        .brood_share = 0.25;
    let mut fauna = Fauna::new(c);
    introduce(
        &mut fauna,
        &world,
        Founder::Browser,
        StartingStores {
            body: 1.0,
            reserve: 0.5,
        },
    );
    let bite = fauna.animals[0].body * 0.1;
    feed(
        &mut fauna,
        0,
        Taken {
            organic: bite,
            mineral: 0.0,
            energy: bite * 7.0,
        },
        1.0,
    );
    fauna.animals[0].reproduction.brood = fauna.animals[0].reserve / 2.0;
    let before = fauna.animals[0];
    let reserve_paid = before.reserve / 4.0;
    let cost = before.fuel + reserve_paid;
    let tissue = before.body + before.reserve;
    let tissue_energy_density = before.energy / tissue;
    let tissue_mineral_density = before.mineral / tissue;
    let brood_share = fauna
        .config
        .founder(Founder::Browser)
        .core
        .reproduction
        .brood_share;
    let heat = fauna.ledger.heat_out;

    let (paid, from_body) = respire(&mut fauna, 0, cost, Respiration::Motor);

    let a = fauna.animals[0];
    close(paid, cost);
    assert_eq!(from_body, 0.0);
    assert_eq!(a.body, before.body);
    assert_eq!(a.fuel, 0.0);
    assert_eq!(a.fuel_energy, 0.0);
    close(before.reserve - a.reserve, reserve_paid);
    close(
        before.reproduction.brood - a.reproduction.brood,
        brood_share * reserve_paid,
    );
    close(
        before.energy - a.energy,
        reserve_paid * tissue_energy_density,
    );
    close(
        before.mineral - a.mineral,
        reserve_paid * tissue_mineral_density,
    );
    close(
        a.gut.mineral - before.gut.mineral,
        reserve_paid * tissue_mineral_density,
    );
    close(
        fauna.ledger.heat_out - heat,
        before.fuel_energy + reserve_paid * tissue_energy_density,
    );
    close(
        fauna.ledger.motor_by_founder[Founder::Browser.index()],
        cost,
    );
    assert_closes(&fauna, "mixed fuel and reserve spending");
}

#[test]
fn a_fueled_pup_survives_empty_reserve_and_stomach_while_fuel_pays_upkeep() {
    let world = clock_off(49);
    let mut c = config(Founder::Chorister, 0.5);
    c.founders[Founder::Chorister.index()]
        .core
        .maintenance_per_s = 0.02;
    let p = c.founders[Founder::Chorister.index()]
        .pack
        .as_mut()
        .expect("pack lineage");
    p.pack_size = 1;
    p.pup_starves_on_empty = true;
    p.provision_rate_per_s = 0.0;
    p.regurgitate_reserve_fraction = 0.0;
    let mut fauna = Fauna::new(c);
    fauna.set_births_enabled(false);
    assert!(fauna.apply(
        &world,
        Command::FoundPack {
            den: Site { x: 8, y: 2, z: 6 },
            founder: Founder::Chorister,
            stores: StartingStores::FULL,
            heading_rad: 0.0,
        }
    ));
    let pack = fauna.packs()[0].id;
    let pup = fauna
        .whelp(&world.view(), pack, 1)
        .expect("the breeder funds a pup")[0];
    let i = fauna
        .animals
        .iter()
        .position(|a| a.id == pup)
        .expect("new pup");
    let reserve = fauna.animals[i].reserve;
    let removed = super::take_from_reserve(&mut fauna.animals[i], reserve);
    fauna.ledger.removed_organic_out += removed.organic;
    fauna.ledger.removed_mineral_out += removed.mineral;
    fauna.ledger.removed_energy_out += removed.energy;
    let bite = fauna.animals[i].body * 0.1;
    feed(
        &mut fauna,
        i,
        Taken {
            organic: bite,
            mineral: 0.0,
            energy: bite * 7.0,
        },
        1.0,
    );
    assert!(fauna.set_controller(pup, Box::new(Scripted::new(vec![Actions::default()]))));
    let before = fauna.animals[i];
    assert_eq!(before.reserve, 0.0);
    assert_eq!(before.stomach.organic(), 0.0);
    assert!(before.pack.is_some_and(|m| m.pup));
    let upkeep = fauna.ledger.pup_upkeep_organic;
    let mut flora = Flora::new(FloraConfig::default());

    for _ in 0..8 {
        fauna.step_with(&world, &mut flora, 1);
        let a = fauna
            .view()
            .animal(pup)
            .copied()
            .expect("fuel keeps the pup alive");
        assert_eq!(a.reserve, 0.0);
        assert_eq!(a.stomach.organic(), 0.0);
        assert_eq!(a.body, before.body);
        assert!(a.fuel > 0.0 && a.fuel < before.fuel);
        assert_closes(&fauna, "a pup living on fuel");
    }
    assert!(fauna.ledger.pup_upkeep_organic > upkeep);
    assert_eq!(fauna.ledger.pups_starved, 0);
}

#[test]
fn lowering_the_configured_fuel_cap_respires_the_surplus_without_losing_matter() {
    let world = clock_off(50);
    let mut fauna = Fauna::new(config(Founder::Browser, 0.5));
    introduce(&mut fauna, &world, Founder::Browser, empty_reserve());
    let bite = fauna.animals[0].body * 0.1;
    feed(
        &mut fauna,
        0,
        Taken {
            organic: bite,
            mineral: 0.0,
            energy: bite * 7.0,
        },
        1.0,
    );
    let before = fauna.animals[0];
    let cap = before.fuel / 2.0;
    let mut c = *fauna.config;
    c.founders[Founder::Browser.index()].core.fuel_cap = cap / before.body;
    let (digestion, heat) = (fauna.ledger.respired_digestion_out, fauna.ledger.heat_out);

    fauna
        .replace_config(c)
        .expect("the reduced fuel cap is valid");

    let a = fauna.animals[0];
    close(a.fuel, cap);
    close(a.fuel_energy, before.fuel_energy * cap / before.fuel);
    assert_eq!(a.body, before.body);
    assert_eq!(a.reserve, before.reserve);
    assert_eq!(a.mineral, before.mineral);
    assert_eq!(a.energy, before.energy);
    assert_eq!(a.gut, before.gut);
    close(
        fauna.ledger.respired_digestion_out - digestion,
        before.fuel - cap,
    );
    close(
        fauna.ledger.heat_out - heat,
        before.fuel_energy - a.fuel_energy,
    );
    assert_closes(&fauna, "a reduced fuel cap");
}
