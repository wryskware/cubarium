//! Population bookkeeping over twenty simulated minutes, from
//! `design/m2-world-spec.md` ("Capacity and IDs", "Controller" budding, "Tick order"
//! step 9) and the doc comments on `World` and `Telemetry`.

use cubarium_core::{TICK_HZ, World, WorldConfig};
use cubarium_surface::{CELL_COUNT, cell_of};

/// Twenty simulated minutes.
const TWENTY_MINUTES: u64 = 20 * 60 * TICK_HZ as u64;

/// Every organism must remain a well-formed inhabitant of the surface:
/// canonical position, unit heading, and a cell that exists.
fn check_organisms(world: &World, tick: u64) {
    for (id, o) in world.state.organisms.iter() {
        assert!(
            o.pos.is_canonical(),
            "tick {tick}: organism {id:?} left the surface at {:?}",
            o.pos
        );
        let len = o.heading.length();
        assert!(
            (len - 1.0).abs() <= 1e-6,
            "tick {tick}: organism {id:?} heading {:?} has length {len}",
            o.heading
        );
        let cell = cell_of(&o.pos);
        assert!(
            cell.index() < CELL_COUNT,
            "tick {tick}: organism {id:?} maps to cell {} of {CELL_COUNT}",
            cell.index()
        );
        assert_eq!(
            cell.face(),
            o.pos.face,
            "tick {tick}: organism {id:?} cell face disagrees with its chart"
        );
    }
}

/// Spec: cap 512 active; `births_total`, `deaths_total` and the live population must agree
/// with the founders admitted at creation.
#[test]
fn twenty_minutes_of_default_world_keeps_its_books() {
    let config = WorldConfig::default();
    let founders = u64::from(config.founders.count);
    let cap = config.capacity.max_organisms as usize;
    let mut world = World::new(config).expect("defaults are a valid world");

    let mut min_population = world.population();
    let mut max_population = world.population();

    for tick in 1..=TWENTY_MINUTES {
        world.step();
        let population = world.population();
        assert!(
            population <= cap,
            "tick {tick}: population {population} exceeds the cap {cap}"
        );
        min_population = min_population.min(population);
        max_population = max_population.max(population);

        if tick % 500 == 0 {
            check_organisms(&world, tick);
            world.check_invariants().unwrap_or_else(|e| panic!("tick {tick}: {e}"));
        }
    }
    check_organisms(&world, TWENTY_MINUTES);

    let deaths: u64 = world.state.deaths_total.iter().sum();
    let births = world.state.births_total;
    println!(
        "20 min @ defaults: births {births}, deaths starvation/age/collapse {:?} (total {deaths}), \
         cap rejections {}, population min {min_population} max {max_population} final {}",
        world.state.deaths_total,
        world.state.cap_rejections_total,
        world.population()
    );

    assert_eq!(
        births + founders - deaths,
        world.population() as u64,
        "births {births} + founders {founders} − deaths {deaths} does not equal the population {}",
        world.population()
    );
    assert!(
        births > 0,
        "no organism reproduced in twenty simulated minutes (bud_min_age is {} s); \
         population min {min_population} max {max_population}, deaths {:?}",
        world.config().drives.bud_min_age_seconds,
        world.state.deaths_total
    );
}

/// Spec, "Capacity and IDs": "conception is refused at the cap before escrow". With the cap
/// equal to the founder count the world is full from tick zero, so nothing is ever born and
/// the population can only fall.
#[test]
fn a_world_that_starts_full_never_gives_birth() {
    let mut config = WorldConfig::default();
    config.founders.count = 8;
    config.capacity.max_organisms = 8;
    let founders = u64::from(config.founders.count);
    let cap = config.capacity.max_organisms as usize;
    let mut world = World::new(config).expect("a full-from-birth world is still valid");
    assert_eq!(world.population(), cap);

    let mut max_population = world.population();
    for tick in 1..=TWENTY_MINUTES {
        world.step();
        max_population = max_population.max(world.population());
        assert!(
            world.population() <= cap,
            "tick {tick}: population {} exceeds the cap {cap}",
            world.population()
        );
    }

    let deaths: u64 = world.state.deaths_total.iter().sum();
    println!(
        "20 min @ cap == founders: births {}, deaths {:?}, cap rejections {}, max population \
         {max_population}, final {}",
        world.state.births_total,
        world.state.deaths_total,
        world.state.cap_rejections_total,
        world.population()
    );

    assert_eq!(
        world.state.births_total, 0,
        "a world at its cap from the first tick recorded {} births",
        world.state.births_total
    );
    assert_eq!(
        founders - deaths,
        world.population() as u64,
        "founders {founders} − deaths {deaths} does not equal the population {}",
        world.population()
    );
    check_organisms(&world, TWENTY_MINUTES);
}
