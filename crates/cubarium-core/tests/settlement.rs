//! Proportional settlement of feeding requests, from `design/m2-world-spec.md`:
//! "when the intake requests on a cell sum above the available `P` (or `D`), each consumer
//! receives `Q · q_i / Σq`. Requests are collected after movement and settled once from the
//! pre-transfer field", and the grazing row's `q = min(k_mouth · effort · dt, R_max − R)`.

mod common;

use common::total_material;
use cubarium_core::organism::Mode;
use cubarium_core::{World, WorldConfig};
use cubarium_surface::{CellId, Face, Vec2, cell_of};

/// The contested cell: Front, cell column 10, row 10.
fn arena() -> CellId {
    CellId::new(Face::Front, 10, 10)
}

/// Strip every field of food, then put `producer` into the arena cell only.
fn only_food_in_the_arena(world: &mut World, producer: f64) {
    for v in world.state.fields.p.iter_mut() {
        *v = 0.0;
    }
    for v in world.state.fields.d.iter_mut() {
        *v = 0.0;
    }
    for v in world.state.fields.de.iter_mut() {
        *v = 0.0;
    }
    world.state.fields.p[arena().index()] = producer;
}

/// Three equal, maximally hungry organisms in one cell whose producer cannot serve them all
/// must each receive the same share, and the cell must be drained to zero rather than
/// overdrawn.
#[test]
fn contested_producer_is_split_in_equal_proportion() {
    let mut config = WorldConfig::default();
    config.founders.count = 3;
    // The v1 founder path: one omnivore genotype, so the two channels below both request.
    config.founders.kinds.clear();
    // Defaults set `feed_min = 0.05 m`, which is far above the three organisms' combined
    // per-tick request of `3 · k_mouth · dt = 0.0075 m`; contention is unreachable without
    // lowering the threshold below the standing crop under test.
    config.drives.feed_min = 0.001;
    // Allocation is tested under the linear intake law: saturation scales requests but
    // never changes proportional settlement, and with K_P > 0 three mouths cannot
    // contest a 0.004 m cell at all.
    config.organism.intake_half_saturation = 0.0;
    let mut world = World::new(config).expect("three founders are a valid world");

    let center = arena().center();
    for (_, o) in world.state.organisms.iter_mut() {
        o.pos = center;
        o.heading = Vec2::new(1.0, 0.0);
        o.ou = Vec2::ZERO;
        // Empty reserve: maximum hunger, and the whole mouthful fits in `R_max − R`.
        o.reserve = 0.0;
        o.hunger_memory = 1.0;
        // Seeking with food underfoot becomes Feeding on the next decision.
        o.mode = Mode::Seeking;
    }
    let available = 0.004;
    only_food_in_the_arena(&mut world, available);

    let material_before = total_material(&world);
    world.step();
    let material_after = total_material(&world);

    let gains: Vec<f64> = world.state.organisms.iter().map(|(_, o)| o.reserve).collect();
    assert_eq!(gains.len(), 3, "the three founders must all survive the tick");
    for (_, o) in world.state.organisms.iter() {
        assert_eq!(o.mode, Mode::Feeding, "an organism standing on food is not Feeding");
        assert_eq!(
            cell_of(&o.pos),
            arena(),
            "an organism left the arena cell before settlement"
        );
    }

    let spread = gains.iter().cloned().fold(f64::MIN, f64::max)
        - gains.iter().cloned().fold(f64::MAX, f64::min);
    assert!(
        spread <= 1e-12,
        "equal contestants received unequal shares: {gains:?} (spread {spread:e})"
    );
    assert!(
        gains[0] > 0.0,
        "nobody was fed from a cell holding {available} m of producer"
    );

    let left = world.state.fields.p[arena().index()];
    assert!(
        (0.0..=1e-12).contains(&left),
        "the contested cell holds {left:e} after settlement, expected it drained to zero \
         without going negative"
    );
    assert!(
        (material_after - material_before).abs() < 1e-12,
        "settlement moved material by {:e}",
        material_after - material_before
    );

    // Grazing stores `η_m · q` in the reserve, so the material actually eaten is the sum of
    // the gains divided by `η_m`, and it must equal the producer the cell held when
    // settlement ran. Field reactions run earlier in the tick (tick order steps 3 and 7), so
    // that figure is `available` moved by at most one tick of growth and mortality.
    let assimilated: f64 = gains.iter().sum();
    let cfg = world.config();
    let eta = cfg.organism.assimilation_material;
    let eaten = assimilated / eta;
    let reaction_bound =
        (cfg.producer.growth + cfg.producer.mortality) * available * cubarium_core::DT;
    assert!(
        (eaten - available).abs() <= reaction_bound + 1e-12,
        "the three organisms between them ate {eaten:e} of the {available} on offer \
         (one tick of field reactions can move at most {reaction_bound:e})"
    );
}

/// A full reserve requests nothing: `q = min(k_mouth · effort · dt, R_max − R)` is zero when
/// `R == R_max`, so the cell it stands on is untouched.
#[test]
fn a_full_organism_requests_nothing() {
    let mut config = WorldConfig::default();
    config.founders.count = 1;
    config.founders.kinds.clear();
    // Freeze the producer field so any change to `P` can only come from intake: growth,
    // mortality and ripening (`design/fauna-v2.md` "Fruit", which takes `P` above
    // `fruit_min · P_max`) all off.
    config.producer.growth = 0.0;
    config.producer.mortality = 0.0;
    config.fruit.ripen = 0.0;
    let mut world = World::new(config).expect("one founder is a valid world");

    for (_, o) in world.state.organisms.iter_mut() {
        o.pos = arena().center();
        o.heading = Vec2::new(1.0, 0.0);
        o.ou = Vec2::ZERO;
        o.reserve = o.phenotype.reserve_max;
        // Claim to be starving anyway, so only the `R_max − R` term can hold intake back.
        o.hunger_memory = 1.0;
        o.mode = Mode::Seeking;
    }
    let stock = 0.5;
    only_food_in_the_arena(&mut world, stock);

    world.step();

    let (_, organism) = world.state.organisms.iter().next().expect("the founder survives");
    assert_eq!(
        organism.mode,
        Mode::Feeding,
        "the organism should still enter Feeding; only its request is empty"
    );
    assert_eq!(
        world.state.fields.p[arena().index()],
        stock,
        "a full organism drew {:e} from the cell",
        stock - world.state.fields.p[arena().index()]
    );
    assert!(
        organism.reserve <= organism.phenotype.reserve_max,
        "reserve {} exceeds R_max {}",
        organism.reserve,
        organism.phenotype.reserve_max
    );
}
