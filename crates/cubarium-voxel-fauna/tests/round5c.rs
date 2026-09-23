//! Round 5c: the **frondgrazer**, one short function test per rule of its tick.
//!
//! Conventions are the flora crate's `round5a.rs`: `voxel_m` is 1 m, soil is made by
//! adding water to an air cell and converting it, and the **world is never stepped**
//! except where a test says "coupled" — so a fixture's water and a stand's foliage are the
//! condition the test says they are and nothing else moves them.
//!
//! Where a test needs a rate the placeholders do not give, it sets that rate in its **own**
//! [`FaunaConfig`] and says why. None of those is read back as a placeholder, and no test
//! here asserts anything about viability, population or carrying capacity: the model says
//! nothing about them.

use cubarium_voxel::{Command as WorldCommand, Config as VoxelConfig, Material, World};
use cubarium_voxel_fauna::{
    Command, DT, Fauna, FaunaConfig, Reproduction, Species, SpeciesConfig, State,
};
use cubarium_voxel_flora::{Command as FloraCommand, Flora, FloraConfig, Site, Species as Plant};

// ------------------------------------------------------------------- fixtures

/// One air voxel turned into `material` holding exactly `pore` of that material's own
/// pore capacity. `round5a.rs`'s `fill`.
fn fill(w: &mut World, x: i64, y: u32, z: u32, material: Material, pore: f64) {
    let want = pore * material.pore_capacity() * w.config().voxel_volume();
    if want > 0.0 {
        let got = w.apply(WorldCommand::AddWater {
            x,
            y,
            z,
            volume_m3: want,
        });
        assert!((got - want).abs() < 1e-12, "the void took {got} of {want}");
    }
    w.apply(WorldCommand::SetMaterial { x, y, z, material });
}

fn empty_world(width: u32, seed: u64) -> World {
    World::empty(VoxelConfig {
        width,
        height: 10,
        depth: 1,
        voxel_m: 1.0,
        seed,
        ..VoxelConfig::default()
    })
}

/// A flat plain: every column solid to `top`, so every column's support face is `top` in
/// open sky. `pore` is the soil wetness the plant gates read.
fn plain(width: u32, top: u32, pore: f64, seed: u64) -> World {
    let mut w = empty_world(width, seed);
    for x in 0..width as i64 {
        for y in 1..=top {
            fill(&mut w, x, y, 0, Material::Soil, pore);
        }
    }
    w
}

/// Raise one column of a [`plain`] to `top`, so its support face is higher than its
/// neighbours': the step a `climb` either allows or refuses.
fn raise(w: &mut World, x: i64, from: u32, top: u32, pore: f64) {
    for y in from + 1..=top {
        fill(w, x, y, 0, Material::Soil, pore);
    }
}

fn at(x: u32, y: u32) -> Site {
    Site { x, y, z: 0 }
}

/// A springturf founder at half its own `wood_max`, which is the harness's founder size:
/// wood 0.03, foliage `α · W` = 0.06, and a crown **one** voxel above its support face
/// (`crown_voxels(0.03)` is `round(0.75)` = 1), so a browser standing on the next face
/// along with `up: 1` can reach it and one standing two voxels lower cannot.
fn turf(flora: &mut Flora, world: &World, x: i64) {
    let wood = 0.5 * flora.config().springturf.wood_max;
    assert!(
        flora.apply(
            world,
            FloraCommand::Seed {
                x,
                z: 0,
                species: Plant::Springturf,
                wood
            }
        ),
        "a founder at x {x}"
    );
}

fn grazer(fauna: &mut Fauna, world: &World, x: i64, body: f64) -> u64 {
    let before = fauna.view().ledger.births;
    assert!(
        fauna.apply(
            world,
            Command::Introduce {
                x,
                z: 0,
                species: Species::Frondgrazer,
                body
            }
        ),
        "a grazer at x {x}"
    );
    before
}

fn config_with(edit: impl FnOnce(&mut SpeciesConfig)) -> FaunaConfig {
    let mut c = FaunaConfig::default();
    edit(c.species_mut(Species::Frondgrazer));
    c
}

/// A grazer **hungry on its first tick** (package G's satiety): an upkeep of
/// `reserve_cap / DT` spends the whole reserve `Introduce` gives it in that tick's
/// maintenance, before it eats, so the bite it takes is a whole hungry one; and no
/// diminishing bite (`bite_half_stock` 0), so that bite is exactly `bite_per_s · DT`. For
/// the cases about what one bite is and what it builds, which were written when a full
/// animal still ate a whole mouthful.
fn hungry(s: &mut SpeciesConfig) {
    s.maintenance_per_s = s.reserve_cap / DT;
    s.bite_half_stock = 0.0;
}

/// The gestation rule at its **shortest**: a one-tick hold, a one-tick gestation and a
/// one-tick interval, so a birth lands on the tick a body becomes eligible and the
/// arithmetic of one paid parcel is visible in one step.
///
/// The shipped placeholders are minutes long
/// (`design/handoffs/voxel-reproduction-2026-09-21.md`), and these cases are about
/// **what a birth costs and what a newborn holds**, not about how long the parent had
/// to wait for it. Running them on the shipped intervals would be a 10,000-tick test of
/// the same three numbers. The floor is dropped to zero for the same reason: the cases
/// were written against `reserve >= birth_cost`, which is exactly the eligibility this
/// leaves.
fn fast_births(s: &mut SpeciesConfig) {
    s.reproduction = Reproduction {
        surplus_floor: 0.0,
        surplus_hold_s: DT,
        gestation_s: DT,
        birth_interval_s: DT,
        ..Reproduction::LIVE_BIRTH_PLACEHOLDER
    };
}

/// The animal layer's three residuals, in the shape the flora tests use.
fn assert_fauna_residuals(fauna: &Fauna, when: &str) {
    let v = fauna.view();
    let (o, n, e) = (
        v.organic() - v.ledger.expected_organic(),
        v.mineral() - v.ledger.expected_mineral(),
        v.energy() - v.ledger.expected_energy(),
    );
    assert!(
        o.abs() <= 1e-9 * v.organic().abs().max(1.0),
        "{when}: fauna organic residual {o}"
    );
    assert!(
        n.abs() <= 1e-9 * v.mineral().abs().max(1.0),
        "{when}: fauna mineral residual {n}"
    );
    assert!(
        e.abs() <= 1e-9 * v.energy().abs().max(1.0),
        "{when}: fauna energy residual {e}"
    );
}

fn assert_flora_residuals(flora: &Flora, when: &str) {
    let v = flora.view();
    let (o, n, e) = (
        v.organic() - v.ledger.expected_organic(),
        v.mineral() - v.ledger.expected_mineral(),
        v.energy() - v.ledger.expected_energy(),
    );
    assert!(
        o.abs() <= 1e-9 * v.organic().abs().max(1.0),
        "{when}: flora organic residual {o}"
    );
    assert!(
        n.abs() <= 1e-9 * v.mineral().abs().max(1.0),
        "{when}: flora mineral residual {n}"
    );
    assert!(
        e.abs() <= 1e-9 * v.energy().abs().max(1.0),
        "{when}: flora energy residual {e}"
    );
}

// ------------------------------------------------------------------- the bite

/// A grazer on the face beside a turf takes exactly one bite off it — `bite_per_s · dt`,
/// neither more nor less — and the two layers book the same three numbers.
#[test]
fn a_grazer_beside_reachable_foliage_crops_exactly_one_bite() {
    let world = plain(8, 2, 0.3, 5);
    let mut flora = Flora::new(FloraConfig::default());
    turf(&mut flora, &world, 3);
    let mut fauna = Fauna::new(config_with(hungry));
    let id = grazer(&mut fauna, &world, 2, 0.02);

    let sc = *fauna.config().species(Species::Frondgrazer);
    let bite = sc.bite_per_s * DT;
    let before = flora
        .view()
        .stand_at(at(3, 2))
        .expect("the founder")
        .foliage;
    assert!(before > bite, "the fixture must hold more than one bite");

    fauna.step(&world, &mut flora);

    let after = flora
        .view()
        .stand_at(at(3, 2))
        .expect("the founder")
        .foliage;
    assert!(
        (before - after - bite).abs() < 1e-15,
        "the stand lost {} not {bite}",
        before - after
    );
    assert_eq!(fauna.view().animal(id).unwrap().state, State::Cropping);
    assert_eq!(fauna.view().ledger.bites, 1);
    // The two ledgers are the same transfer from two sides, to the bit.
    let (fv, av) = (flora.view(), fauna.view());
    assert_eq!(fv.ledger.consumed_organic_out, av.ledger.eaten_organic_in);
    assert_eq!(fv.ledger.consumed_mineral_out, av.ledger.eaten_mineral_in);
    assert_eq!(fv.ledger.consumed_energy_out, av.ledger.eaten_energy_in);
    assert_eq!(av.ledger.eaten_organic_in, bite);
    assert_residual_pair(&flora, &fauna, "after one bite");
}

fn assert_residual_pair(flora: &Flora, fauna: &Fauna, when: &str) {
    assert_flora_residuals(flora, when);
    assert_fauna_residuals(fauna, when);
}

/// Foliage two voxels above the face a browser stands on is not food, whatever else is
/// true of it: the reach box is `up: 1`. It is not eaten, and the step that would put the
/// animal on the stand's own face is refused by `climb` as well, so nothing at all
/// happens.
#[test]
fn foliage_two_voxels_up_is_not_eaten() {
    let mut world = plain(8, 2, 0.3, 5);
    raise(&mut world, 3, 2, 4, 0.3);
    let mut flora = Flora::new(FloraConfig::default());
    turf(&mut flora, &world, 3);
    assert_eq!(
        flora.view().stands[0].site,
        at(3, 4),
        "the founder is on the raised face"
    );
    let mut fauna = Fauna::new(FaunaConfig::default());
    let id = grazer(&mut fauna, &world, 2, 0.02);

    let before = flora.view().stands[0].foliage;
    for _ in 0..20 {
        fauna.step(&world, &mut flora);
    }

    assert_eq!(flora.view().stands[0].foliage, before, "nothing was taken");
    assert_eq!(fauna.view().ledger.eaten_organic_in, 0.0);
    assert_eq!(fauna.view().ledger.bites, 0);
    assert_eq!(
        fauna.view().animal(id).unwrap().site,
        at(2, 2),
        "and it could not climb up"
    );
    assert_eq!(fauna.view().ledger.steps, 0);
}

// ------------------------------------------------ R9.1: the mineral budget of a bite

/// A flora config whose springturf tissue holds `n_tissue` of mineral per unit, so a test
/// can say what a bite carries. Nothing else moves, and the **animal's** knobs are the
/// placeholders throughout.
fn flora_with_plant_mineral(n_tissue: f64) -> Flora {
    let mut config = FloraConfig::default();
    config.species_mut(Plant::Springturf).n_tissue = n_tissue;
    Flora::new(config)
}

/// **R9.1, the zero-mineral bite.** A turf whose tissue holds no mineral at all is food
/// with no nutrient in it: the grazer takes its whole mouthful, respires every unit of it
/// with the energy that came in it, pays its upkeep out of its reserve, and **builds
/// nothing** — although it is itself carrying `n_tissue · organic` of mineral from the day
/// it was introduced. That inventory is not a reserve growth may draw on.
#[test]
fn a_mineral_free_bite_builds_nothing_and_is_respired_whole() {
    let world = plain(8, 2, 0.3, 5);
    let mut flora = flora_with_plant_mineral(0.0);
    turf(&mut flora, &world, 3);
    let mut fauna = Fauna::new(config_with(hungry));
    let id = grazer(&mut fauna, &world, 2, 0.02);
    let sc = *fauna.config().species(Species::Frondgrazer);
    let before = *fauna.view().animal(id).unwrap();
    assert!(
        before.mineral > 0.0,
        "it arrived with mineral in its tissue"
    );

    fauna.step(&world, &mut flora);

    let v = fauna.view();
    let a = *v.animal(id).unwrap();
    let bite = sc.bite_per_s * DT;
    assert!(
        (v.ledger.eaten_organic_in - bite).abs() < 1e-18,
        "it ate one whole bite"
    );
    assert_eq!(
        v.ledger.eaten_mineral_in, 0.0,
        "and the bite carried no mineral"
    );
    assert!(
        (a.body - before.body).abs() <= 1e-15,
        "nothing was built: {} -> {}",
        before.body,
        a.body
    );
    let upkeep = sc.maintenance_per_s * before.body * DT;
    assert!(
        (a.reserve - (before.reserve - upkeep)).abs() <= 1e-12 * before.reserve,
        "the reserve paid the upkeep and gained nothing: {a:?}"
    );
    assert_eq!(
        a.mineral, before.mineral,
        "its own mineral is an inventory, not a reserve"
    );
    assert!(
        (v.ledger.respired_out - (bite + upkeep)).abs() <= 1e-12 * bite,
        "the whole bite and the upkeep were respired: {}",
        v.ledger.respired_out
    );
    // Its own energy fell only by the upkeep's own heat, at its own density: none of the
    // bite's energy stayed, because none of the bite's organic matter did.
    let e_density = before.energy / before.organic();
    assert!(
        (a.energy - (before.energy - e_density * upkeep)).abs() <= 1e-12 * before.energy,
        "{a:?}"
    );
    let heat = v.ledger.eaten_energy_in + e_density * upkeep;
    assert!(
        (v.ledger.heat_out - heat).abs() <= 1e-12 * heat,
        "heat {}",
        v.ledger.heat_out
    );
    assert_eq!(
        v.ledger.deposited_mineral_out, 0.0,
        "there was nothing to excrete"
    );
    assert_residual_pair(&flora, &fauna, "after a mineral-free bite");
}

/// **R9.1, the partially funded bite.** The placeholders themselves: a plant's foliage
/// holds `n_tissue` 0.02 and this animal's tissue wants 0.05, so a `1e-4` bite brings
/// `2e-6` of mineral and funds `4e-5` of tissue where `yield_fraction` 0.5 would have
/// built `5e-5`. The growth is exactly `mineral / n_tissue`, the difference is respired
/// with its energy, and every unit of the bite's mineral ends up in the new tissue with
/// nothing left over to excrete. **No grazer knob is touched.**
#[test]
fn a_partly_mineralised_bite_builds_exactly_what_its_mineral_funds() {
    let world = plain(8, 2, 0.3, 5);
    let mut flora = Flora::new(FloraConfig::default());
    turf(&mut flora, &world, 3);
    let mut fauna = Fauna::new(config_with(hungry));
    let id = grazer(&mut fauna, &world, 2, 0.02);
    let sc = *fauna.config().species(Species::Frondgrazer);
    let before = *fauna.view().animal(id).unwrap();
    assert!(before.body < sc.body_max, "there is room to grow into");

    fauna.step(&world, &mut flora);

    let v = fauna.view();
    let a = *v.animal(id).unwrap();
    let bite = v.ledger.eaten_organic_in;
    let mineral = v.ledger.eaten_mineral_in;
    assert!(
        (bite - 1e-4).abs() < 1e-18 && (mineral - 2e-6).abs() < 1e-18,
        "{bite} / {mineral}"
    );
    let funded = mineral / sc.n_tissue;
    assert!(
        (funded - 4e-5).abs() <= 1e-12 * funded && funded < sc.yield_fraction * bite,
        "the mineral funds {funded} of the {} the yield would build",
        sc.yield_fraction * bite
    );
    // All of it went into the reserve, which the upkeep had just emptied: intake is
    // stored, and structure is built out of the reserve at the growth rate (package G),
    // which found nothing to build from before this bite.
    let built = a.reserve;
    assert!(
        (built - funded).abs() <= 1e-9 * funded,
        "it stored {built} and not {funded}"
    );
    assert!(
        (a.body - before.body).abs() <= 1e-15,
        "the body waits for growth"
    );
    // The mineral: all of it kept, none excreted, none created.
    assert!((a.mineral - (before.mineral + mineral)).abs() <= 1e-12 * a.mineral);
    assert_eq!(
        v.ledger.deposited_mineral_out, 0.0,
        "nothing was left over to excrete"
    );
    // And what was not built was respired with the energy that came in with it.
    let upkeep = sc.maintenance_per_s * before.body * DT;
    assert!(
        (v.ledger.respired_out - (bite - built + upkeep)).abs() <= 1e-12 * bite,
        "respired {} of {}",
        v.ledger.respired_out,
        bite - built + upkeep
    );
    assert_residual_pair(&flora, &fauna, "after a partly funded bite");
}

// ------------------------------------------------------- maintenance and death

/// The upkeep is paid out of the reserve while there is one, and out of the body after
/// that. The body does not move while the reserve is paying.
#[test]
fn maintenance_drains_the_reserve_and_then_the_body() {
    let world = plain(4, 2, 0.3, 5);
    let mut flora = Flora::new(FloraConfig::default());
    // 4 /s against the placeholder 0.001 /s, so one tick's upkeep is a readable 0.004 of
    // organic matter instead of 5e-7. What is tested is the order of payment; the body is
    // 0.02, under `birth_body`, so nothing else is spending the reserve.
    // No growth (package G builds structure out of the reserve), so the reserve pays the
    // upkeep and nothing else.
    let mut fauna = Fauna::new(config_with(|s| {
        s.maintenance_per_s = 4.0;
        s.growth_max_per_s = 0.0;
    }));
    let id = grazer(&mut fauna, &world, 1, 0.02);
    let start = *fauna.view().animal(id).unwrap();
    assert_eq!(
        start.reserve, 0.01,
        "a full reserve is `reserve_cap · body`"
    );

    for tick in 1..=2 {
        fauna.step(&world, &mut flora);
        let a = *fauna.view().animal(id).unwrap();
        assert_eq!(
            a.body, start.body,
            "tick {tick}: the body has not been touched"
        );
        assert!(
            (a.reserve - (0.01 - 0.004 * f64::from(tick))).abs() < 1e-15,
            "{a:?}"
        );
        assert_eq!(a.mineral, start.mineral, "respiration moves no mineral");
        assert_eq!(
            a.state,
            State::Resting,
            "no food in reach and none in sense"
        );
    }
    // The third tick's 0.004 cannot all come out of the 0.002 that is left, so the body
    // pays the difference.
    fauna.step(&world, &mut flora);
    let a = *fauna.view().animal(id).unwrap();
    assert_eq!(a.reserve, 0.0);
    assert!(
        (a.body - 0.018).abs() < 1e-15,
        "the body paid the difference: {a:?}"
    );
    // Three ticks of the whole upkeep: the shortfall is paid, not skipped.
    assert!((fauna.view().ledger.respired_out - 0.012).abs() < 1e-15);
    assert_fauna_residuals(&fauna, "three ticks of upkeep");
}

/// A grazer with nothing to eat dies when its body falls below `body_min`, and what is
/// left of it is on its own site as carrion with the mineral it never respired.
#[test]
fn a_starving_grazer_dies_at_body_min_and_leaves_its_carrion() {
    let world = plain(4, 2, 0.3, 5);
    let mut flora = Flora::new(FloraConfig::default());
    // 5 /s against the placeholder 0.001 /s: the point is the death, not how long a
    // grazer takes to starve, and 5 /s reaches it in a dozen ticks.
    let mut fauna = Fauna::new(config_with(|s| s.maintenance_per_s = 5.0));
    // 0.02 is under `birth_body`: a starving adult that could still afford a birth would
    // leave a newborn to starve beside it, and this test is about one death.
    let id = grazer(&mut fauna, &world, 1, 0.02);
    let site = fauna.view().animal(id).unwrap().site;
    let mineral = fauna.view().animal(id).unwrap().mineral;
    assert!(mineral > 0.0);

    let mut ticks = 0;
    while fauna.view().animal(id).is_some() && ticks < 200 {
        fauna.step(&world, &mut flora);
        ticks += 1;
    }
    assert!(ticks < 200, "it starved inside the window");
    assert_eq!(fauna.view().ledger.deaths, 1);
    assert!(fauna.view().animals.is_empty());

    let g = flora
        .view()
        .ground_at(site)
        .expect("a deposit provisions the site");
    let body_min = fauna.config().species(Species::Frondgrazer).body_min;
    assert!(
        g.carrion > 0.0 && g.carrion < body_min,
        "the corpse is what was left: {}",
        g.carrion
    );
    assert!(
        (g.carrion_mineral - mineral).abs() < 1e-15,
        "with all of its mineral"
    );
    assert_eq!(fauna.view().ledger.deposited_organic_out, g.carrion);
    assert_eq!(fauna.view().ledger.deposited_mineral_out, g.carrion_mineral);
    assert!(fauna.view().organic() == 0.0 && fauna.view().mineral() == 0.0);
    assert_residual_pair(&flora, &fauna, "after the death");
}

// ------------------------------------------------------------------- the walk

/// A step crosses no face taller than `climb` and no water deeper than `wade_depth_m`,
/// and the rule is not vacuous: the same grazer does step onto a face exactly `climb`
/// above the one it is standing on.
#[test]
fn a_step_crosses_neither_a_wall_nor_a_pool() {
    // The wall: the target's face is two voxels up, `climb` is 1.
    let mut world = plain(12, 2, 0.3, 5);
    raise(&mut world, 5, 2, 4, 0.3);
    let mut flora = Flora::new(FloraConfig::default());
    turf(&mut flora, &world, 5);
    let mut fauna = Fauna::new(FaunaConfig::default());
    let id = grazer(&mut fauna, &world, 3, 0.02);
    for _ in 0..120 {
        fauna.step(&world, &mut flora);
    }
    // It walks up to the foot of the wall and no further: x 4 is one step from x 3, and
    // x 5 is two voxels up.
    let stopped = fauna.view().animal(id).unwrap().site;
    assert_eq!(
        stopped,
        at(4, 2),
        "it stands at the foot of the wall: {stopped:?}"
    );

    // The pool: one column of standing water deeper than it will wade, between it and the
    // only food in the world.
    let mut world = plain(12, 2, 0.3, 7);
    let mut flora = Flora::new(FloraConfig::default());
    turf(&mut flora, &world, 6);
    let sc = *FaunaConfig::default().species(Species::Frondgrazer);
    let depth = 2.0 * sc.wade_depth_m;
    world.apply(WorldCommand::AddWater {
        x: 4,
        y: 3,
        z: 0,
        volume_m3: depth,
    });
    assert!(
        world.view().water_depth_m(4, 2, 0) > sc.wade_depth_m,
        "the fixture's pool must be deeper than the wade: {}",
        world.view().water_depth_m(4, 2, 0)
    );
    let mut fauna = Fauna::new(FaunaConfig::default());
    let id = grazer(&mut fauna, &world, 3, 0.02);
    for _ in 0..120 {
        fauna.step(&world, &mut flora);
    }
    assert_eq!(
        fauna.view().animal(id).unwrap().site,
        at(3, 2),
        "it stayed out of the pool"
    );
    assert_eq!(fauna.view().ledger.bites, 0, "and never reached the food");

    // Not vacuous: one voxel up is exactly `climb` and is stepped onto.
    let mut world = plain(12, 2, 0.3, 5);
    raise(&mut world, 5, 2, 3, 0.3);
    let mut flora = Flora::new(FloraConfig::default());
    turf(&mut flora, &world, 5);
    let mut fauna = Fauna::new(FaunaConfig::default());
    let id = grazer(&mut fauna, &world, 3, 0.02);
    for _ in 0..120 {
        fauna.step(&world, &mut flora);
    }
    let reached = fauna.view().animal(id).unwrap().site;
    assert!(
        reached.y == 3 || reached == at(4, 2),
        "it climbed the one-voxel step: {reached:?}"
    );
    assert!(fauna.view().ledger.bites > 0, "and ate when it got there");
}

// ------------------------------------------------- R9.5: sensing in face coordinates

/// A one-layer bloomcrown at a fifth of its own `wood_max`: wood 0.12, foliage `α · W` =
/// 0.24, and on package L's ladder a 0.5 m crown whose cell is **two** 0.25 m voxels above
/// its support face, so a browser with `reach.up` 1 can only eat it from a face at least
/// one voxel higher than the stand's. (Half its `wood_max` was two voxels before the
/// ladder; it is three now.)
fn bloom(flora: &mut Flora, world: &World, x: i64) {
    let wood = 0.2 * flora.config().bloomcrown.wood_max;
    assert!(
        flora.apply(
            world,
            FloraCommand::Seed {
                x,
                z: 0,
                species: Plant::Bloomcrown,
                wood
            }
        ),
        "a bloomcrown founder at x {x}"
    );
    assert_eq!(
        flora.config().bloomcrown.crown_voxels(wood, flora.config().voxel_m),
        2,
        "the fixture's premise"
    );
}

/// **R9.5, Astra's case: a feeding face outside the sensing radius is not selectable.**
/// Width 40, depth 1, a grazer on `(10,2)`, a bloomcrown founder on `(18,2)` — eight
/// columns away, inside `sense_radius` 8 — and the **only** face its crown can be eaten
/// from is the raised `(19,3)`, nine columns away and outside it. The old stand-centre
/// prefilter admitted that face because the *stand* was inside the radius; the face-domain
/// rule refuses it, and the animal has nothing to walk to.
///
/// Three arms: the refusal, the same fixture translated across the `x` seam, and the same
/// fixture at `sense_radius` 9 — this test's own number — where the face *is* a candidate
/// and the walk happens, so the refusal is about the radius and not about the geometry.
#[test]
fn a_feeding_face_outside_the_sensing_radius_is_not_selectable() {
    /// One arm: a grazer at `gx`, a bloomcrown at `bx`, the sole raised face at `bx + 1`.
    /// Returns where the animal ended up, its state, and the bites and steps it took.
    fn arm(gx: i64, bx: i64, radius: u32) -> (Site, State, u64, u64) {
        let mut world = plain(40, 2, 0.3, 5);
        let raised = (bx + 1).rem_euclid(40);
        raise(&mut world, raised, 2, 3, 0.3);
        // The premise below is "the crown is food from exactly one face", which is a
        // claim about the **sensing radius** and needs a plant with one disc: an adult
        // bloomcrown's basal rosette is food from every face beside it, which is the
        // layers package's point and `tests/plant_layers.rs`'s subject.
        let mut flora = Flora::new(FloraConfig::default().one_layer_species());
        bloom(&mut flora, &world, bx);
        let mut fauna = Fauna::new(config_with(|s| s.sense_radius = radius));
        let sc = *fauna.config().species(Species::Frondgrazer);
        let id = grazer(&mut fauna, &world, gx, 0.02);

        // The premise, from the model's own reach query: the crown is food from the raised
        // face and from nowhere else in the world.
        let fv = flora.view();
        let from_raised = fv
            .reachable_foliage(&world.view(), at(raised as u32, 3), sc.reach)
            .len();
        assert_eq!(
            from_raised, 1,
            "the crown is reachable from the raised face"
        );
        for x in 0..40u32 {
            if x == raised as u32 {
                continue;
            }
            assert!(
                fv.reachable_foliage(&world.view(), at(x, 2), sc.reach)
                    .is_empty(),
                "({x},2) must not reach the crown"
            );
        }

        for _ in 0..200 {
            fauna.step(&world, &mut flora);
        }
        let a = *fauna
            .view()
            .animal(id)
            .expect("it neither starved nor drowned");
        (
            a.site,
            a.state,
            fauna.view().ledger.bites,
            fauna.view().ledger.steps,
        )
    }

    // The refusal: nine columns is outside a radius of eight, so it rests where it stands.
    let (site, state, bites, steps) = arm(10, 18, 8);
    assert_eq!(site, at(10, 2), "it did not move: {site:?}");
    assert_eq!(state, State::Resting);
    assert_eq!((bites, steps), (0, 0), "nothing sensed, nothing eaten");

    // The same case translated across the seam: `x` wraps, so 32 → 1 is nine columns the
    // short way round and the answer cannot depend on where the seam is.
    let (site, state, bites, steps) = arm(32, 0, 8);
    assert_eq!(site, at(32, 2), "across the seam too: {site:?}");
    assert_eq!(state, State::Resting);
    assert_eq!((bites, steps), (0, 0));

    // Not vacuous: at a radius of nine the raised face is a candidate, and the animal
    // walks to it and eats.
    let (site, _, bites, steps) = arm(10, 18, 9);
    assert!(
        steps > 0 && bites > 0,
        "at radius 9 it walked and ate: {site:?} {steps} {bites}"
    );
    assert_eq!(site.y, 3, "and it is standing on the raised face: {site:?}");
}

/// **R9.5, the other direction: a stand outside the radius whose feeding face is inside it
/// is found.** A springturf founder on `(19,2)` is nine columns from a grazer on `(10,2)`
/// and the stand-centre prefilter dropped it, although `(18,2)` — eight columns away, well
/// inside `sense_radius` 8 — reaches it. Scoring candidate faces finds it, and the animal
/// walks over and eats.
#[test]
fn a_stand_outside_the_radius_is_found_through_a_face_inside_it() {
    let world = plain(40, 2, 0.3, 5);
    let mut flora = Flora::new(FloraConfig::default());
    turf(&mut flora, &world, 19);
    let mut fauna = Fauna::new(FaunaConfig::default());
    let sc = *fauna.config().species(Species::Frondgrazer);
    let id = grazer(&mut fauna, &world, 10, 0.02);
    // The premise: nothing is in reach where it stands, the stand is nine columns away,
    // and the nearest face that reaches it is eight.
    let fv = flora.view();
    assert!(
        fv.reachable_foliage(&world.view(), at(10, 2), sc.reach)
            .is_empty()
    );
    assert_eq!(
        fv.reachable_foliage(&world.view(), at(18, 2), sc.reach)
            .len(),
        1
    );
    assert_eq!(sc.sense_radius, 8, "the placeholder this case is about");

    for _ in 0..200 {
        fauna.step(&world, &mut flora);
    }

    let a = *fauna.view().animal(id).expect("it is still alive");
    assert!(fauna.view().ledger.steps > 0, "it walked: {a:?}");
    assert!(
        fauna.view().ledger.bites > 0,
        "and it ate when it got there"
    );
    assert_residual_pair(
        &flora,
        &fauna,
        "after walking to a stand outside the radius",
    );
}

// ----------------------------------------------------------------- the newborn

/// A birth is paid out of the parent's reserve, exactly `birth_cost` of it, and the
/// newborn stands on its parent's face at `body_min` with the remainder as its reserve and
/// its share of the parent's mineral and energy. Nothing crosses the layer's boundary.
///
/// On [`fast_births`], so the whole cost is one instalment on the tick the parent
/// becomes eligible and this stays a test of what a birth **costs**.
#[test]
fn a_birth_pays_birth_cost_and_the_newborn_is_at_body_min() {
    let world = plain(4, 2, 0.3, 5);
    let mut flora = Flora::new(FloraConfig::default());
    let mut fauna = Fauna::new(config_with(fast_births));
    let parent = grazer(&mut fauna, &world, 1, 0.05);
    let before = *fauna.view().animal(parent).unwrap();
    let sc = *fauna.config().species(Species::Frondgrazer);
    assert!(before.body >= sc.birth_body && before.reserve >= sc.birth_cost);
    let stock = fauna.view().organic();

    fauna.step(&world, &mut flora);

    let after = *fauna.view().animal(parent).unwrap();
    let upkeep = sc.maintenance_per_s * before.body * DT;
    assert!(
        (after.reserve - (before.reserve - upkeep - sc.birth_cost)).abs() < 1e-15,
        "the reserve paid the upkeep and the whole birth cost: {after:?}"
    );
    assert_eq!(fauna.view().animals.len(), 2);
    assert_eq!(fauna.view().ledger.born, 1);
    let newborn = fauna
        .view()
        .animals
        .iter()
        .find(|a| a.id != parent)
        .copied()
        .unwrap();
    assert_eq!(newborn.body, sc.body_min);
    assert!((newborn.reserve - (sc.birth_cost - sc.body_min)).abs() < 1e-15);
    // **The named exception to `reserve_cap` (Astra R9.4).** `birth_cost - body_min` is
    // 0.005 and `reserve_cap · body_min` is 0.0025, so a newborn starts with twice the
    // reserve its own body would take in as intake. That is deliberate: the parcel is what
    // the parent actually paid out of its reserve, and clamping it would destroy organic
    // matter. `reserve_cap` is the ceiling on **new intake** and not a storage bound, and
    // `a_newborn_s_endowment_sits_above_the_intake_ceiling_and_is_spent_normally` pins
    // both halves of that contract.
    assert!(
        newborn.reserve > sc.reserve_of(newborn.body),
        "the endowment is above the intake ceiling by design: {} against {}",
        newborn.reserve,
        sc.reserve_of(newborn.body)
    );
    assert_eq!(newborn.site, before.site);
    assert_eq!(newborn.age_ticks, 0);
    // The mineral and the energy left the parent by the same fraction rule the organic
    // matter did, and the layer's total moved only by the tick's own respiration.
    // The fraction is of the parent as the birth found it: after this tick's upkeep, which
    // is paid first.
    let f = sc.birth_cost / (before.organic() - upkeep);
    assert!((newborn.mineral - before.mineral * f).abs() < 1e-15 * before.mineral);
    assert!(
        (newborn.mineral + after.mineral - before.mineral).abs() < 1e-18,
        "mineral moved"
    );
    let heat = upkeep * before.energy / before.organic();
    assert!(
        (newborn.energy + after.energy - (before.energy - heat)).abs() < 1e-15,
        "and the energy, less the upkeep's own heat"
    );
    assert!(
        (fauna.view().organic() - (stock - upkeep)).abs() < 1e-15,
        "a birth is internal"
    );
    assert_eq!(
        fauna.view().ledger.introduced_organic_in,
        stock,
        "and nothing was introduced"
    );
    assert_fauna_residuals(&fauna, "after a birth");
}

/// **R9.4: `reserve_cap` is the ceiling on new intake, and a newborn's endowment sits
/// above it.** Two halves of one contract.
///
/// The ceiling: an adult at `body_max` with a full reserve eats only the little its upkeep
/// has just made room for (package G's satiety), so its reserve never passes
/// `reserve_cap · body` and nothing it eats is respired as surplus.
///
/// The exception: a newborn's paid endowment starts at twice it, no intake raises that
/// excess — a body above its ceiling is sated — and maintenance and growth spend it like
/// any other reserve: no clamp anywhere, which would have destroyed organic matter the
/// parent paid for.
#[test]
fn a_newborn_s_endowment_sits_above_the_intake_ceiling_and_is_spent_normally() {
    let sc = *FaunaConfig::default().species(Species::Frondgrazer);

    // ---- the ceiling, on an adult that is already full.
    let world = plain(8, 2, 0.3, 5);
    let mut flora = Flora::new(FloraConfig::default());
    turf(&mut flora, &world, 3);
    // A birth cost this adult can never afford, so the only things moving its reserve are
    // the upkeep and the intake. (`validate` refuses a zero cost: a newborn needs a body.)
    let mut fauna = Fauna::new(config_with(|s| s.birth_cost = 1.0));
    let id = grazer(&mut fauna, &world, 2, sc.body_max);
    assert_eq!(
        fauna.view().animal(id).unwrap().reserve,
        sc.reserve_of(sc.body_max)
    );
    for tick in 1..=5 {
        fauna.step(&world, &mut flora);
        let a = *fauna.view().animal(id).unwrap();
        assert_eq!(
            a.body, sc.body_max,
            "tick {tick}: it had nowhere to put a body"
        );
        assert!(
            a.reserve <= sc.reserve_of(a.body) + 1e-18,
            "tick {tick}: intake never takes the reserve past the ceiling: {a:?}"
        );
    }
    let l = *fauna.view().ledger;
    assert!(l.bites > 0, "it was eating the whole time");
    let assimilated =
        (sc.yield_fraction * l.eaten_organic_in).min(l.eaten_mineral_in / sc.n_tissue);
    assert!(
        (l.respired_digestion_out - (l.eaten_organic_in - assimilated)).abs()
            <= 1e-12 * l.eaten_organic_in,
        "and digestion respired only the undigested share, none as surplus: {} of {}",
        l.respired_digestion_out,
        l.eaten_organic_in
    );
    assert_residual_pair(&flora, &fauna, "an adult held at the ceiling");

    // ---- the exception, on a newborn in a world with nothing to eat. No growth, so the
    // reserve pays the upkeep and nothing else.
    let world = plain(4, 2, 0.3, 5);
    let mut flora = Flora::new(FloraConfig::default());
    let mut fauna = Fauna::new(config_with(|s| {
        fast_births(s);
        s.growth_max_per_s = 0.0;
    }));
    let parent = grazer(&mut fauna, &world, 1, sc.body_max);
    fauna.step(&world, &mut flora);
    let newborn = fauna
        .view()
        .animals
        .iter()
        .find(|a| a.id != parent)
        .copied()
        .expect("a birth");
    let cap = sc.reserve_of(newborn.body);
    assert!(
        (newborn.reserve - 0.005).abs() < 1e-18 && (cap - 0.0025).abs() < 1e-18,
        "the placeholders' own numbers: {} above {cap}",
        newborn.reserve
    );

    // Maintenance spends the excess at the ordinary rate, with no clamp at the ceiling.
    let mut reserve = newborn.reserve;
    for tick in 1..=3 {
        fauna.step(&world, &mut flora);
        let a = *fauna.view().animal(newborn.id).unwrap();
        let upkeep = sc.maintenance_per_s * a.body * DT;
        assert!(
            (a.reserve - (reserve - upkeep)).abs() <= 1e-12 * reserve,
            "tick {tick}: the reserve fell by the upkeep and nothing else: {a:?}"
        );
        assert!(
            a.reserve > cap,
            "tick {tick}: still above the ceiling, not clamped to it"
        );
        assert_eq!(
            a.body, newborn.body,
            "and the body is untouched while the reserve pays"
        );
        reserve = a.reserve;
    }
    assert_fauna_residuals(&fauna, "a newborn above the ceiling");

    // And no intake raises the excess: with food in reach a newborn above its ceiling is
    // sated and takes nothing, its reserve only falls, and what it loses beyond the
    // upkeep is the structure growth builds out of it.
    let world = plain(8, 2, 0.3, 5);
    let mut flora = Flora::new(FloraConfig::default());
    turf(&mut flora, &world, 3);
    let mut fauna = Fauna::new(config_with(fast_births));
    let parent = grazer(&mut fauna, &world, 2, sc.body_max);
    fauna.step(&world, &mut flora);
    let newborn = fauna
        .view()
        .animals
        .iter()
        .find(|a| a.id != parent)
        .copied()
        .expect("a birth");
    let mut last = newborn;
    for tick in 1..=5 {
        fauna.step(&world, &mut flora);
        let a = *fauna.view().animal(newborn.id).unwrap();
        assert!(
            a.reserve < last.reserve,
            "tick {tick}: the excess never rose: {a:?}"
        );
        assert!(
            a.body > last.body,
            "tick {tick}: growth built structure out of the reserve"
        );
        assert_eq!(a.state, State::Resting, "tick {tick}: sated, it does not crop");
        assert!(
            a.reserve > sc.reserve_of(a.body),
            "tick {tick}: and it is still above it"
        );
        last = a;
    }
    assert_residual_pair(&flora, &fauna, "a fed newborn above the ceiling");
}

/// **R9.4's cheap case: one full default adult buys two young with nothing to eat.**
/// Two paid births out of one full reserve, an interval tick between them, and nothing
/// afterwards — `0.025` of reserve against `birth_cost` 0.01 twice, with the upkeep on
/// top. Neither newborn reproduces: `body_min` is under `birth_body`. On
/// [`fast_births`], so the reserve and not the clock is what stops the third.
#[test]
fn a_full_adult_buys_two_young_with_nothing_to_eat() {
    let world = plain(4, 2, 0.3, 5);
    let mut flora = Flora::new(FloraConfig::default());
    let mut fauna = Fauna::new(config_with(fast_births));
    let sc = *fauna.config().species(Species::Frondgrazer);
    let parent = grazer(&mut fauna, &world, 1, sc.body_max);
    let stock = fauna.view().organic();

    fauna.step(&world, &mut flora);
    assert_eq!(fauna.view().ledger.born, 1, "the first young");
    // The interval: the tick after a birth is refractory, whatever the reserve holds.
    fauna.step(&world, &mut flora);
    assert_eq!(
        fauna.view().ledger.born,
        1,
        "the tick after a birth is the interval, and it pays for nothing"
    );
    fauna.step(&world, &mut flora);
    assert_eq!(
        fauna.view().ledger.born,
        2,
        "and the second, out of the same reserve"
    );
    assert_eq!(fauna.view().animals.len(), 3);
    let after = *fauna.view().animal(parent).unwrap();
    assert!(
        after.reserve < sc.birth_cost && after.reserve > 0.0,
        "what is left cannot buy a third: {after:?}"
    );

    for _ in 0..4 {
        fauna.step(&world, &mut flora);
    }
    assert_eq!(
        fauna.view().ledger.born,
        2,
        "and no later tick buys another"
    );
    assert_eq!(fauna.view().ledger.bites, 0, "with nothing eaten anywhere");
    // Two births are internal: the layer's organic matter moved only by respiration.
    assert!(fauna.view().organic() < stock && fauna.view().organic() > 0.9 * stock);
    assert_fauna_residuals(&fauna, "two young out of one reserve");
}

// -------------------------------------------------------------- the two ledgers

/// The union: over 100 coupled ticks with two grazers, one of which starves and deposits
/// a corpse, the plant layer's `consumed_*` are the animal layer's `eaten_*` and its
/// `deposited_*_in` are the animal layer's `deposited_*_out` — to the bit, because both
/// sides book the same three numbers of the same transfer — and both layers' residuals
/// are at float noise.
#[test]
fn the_two_ledgers_close_together_over_a_hundred_coupled_ticks() {
    let mut world = plain(16, 2, 0.3, 11);
    let mut flora = Flora::new(FloraConfig::default());
    for x in [3, 4, 9] {
        turf(&mut flora, &world, x);
    }
    // One ordinary grazer, and one whose own upkeep is 5 /s so that this run contains a
    // death and therefore a carrion deposit with positive organic matter in it. Two
    // species would be the honest way to say that; one config is what this round has, so
    // the second grazer is introduced into a second layer stepped against the same
    // world and plant layer — which is also the only way one fixture can hold both.
    let mut fauna = Fauna::new(FaunaConfig::default());
    let mut starver = Fauna::new(config_with(|s| s.maintenance_per_s = 5.0));
    grazer(&mut fauna, &world, 2, 0.02);
    grazer(&mut starver, &world, 13, 0.02);

    for _ in 0..100 {
        world.step();
        flora.step(&mut world);
        fauna.step(&world, &mut flora);
        starver.step(&world, &mut flora);
    }

    assert!(fauna.view().ledger.bites > 0, "the grazer ate");
    assert_eq!(starver.view().ledger.deaths, 1, "and the starver died");
    assert!(
        starver.view().ledger.deposited_organic_out > 0.0,
        "leaving a corpse with a body"
    );

    let fv = flora.view();
    let eaten = |o: fn(&cubarium_voxel_fauna::FaunaLedger) -> f64| {
        o(fauna.view().ledger) + o(starver.view().ledger)
    };
    assert_eq!(
        fv.ledger.consumed_organic_out,
        eaten(|l| l.eaten_organic_in)
    );
    assert_eq!(
        fv.ledger.consumed_mineral_out,
        eaten(|l| l.eaten_mineral_in)
    );
    assert_eq!(fv.ledger.consumed_energy_out, eaten(|l| l.eaten_energy_in));
    assert_eq!(
        fv.ledger.deposited_organic_in,
        eaten(|l| l.deposited_organic_out)
    );
    assert_eq!(
        fv.ledger.deposited_mineral_in,
        eaten(|l| l.deposited_mineral_out)
    );
    assert_eq!(
        fv.ledger.deposited_energy_in,
        eaten(|l| l.deposited_energy_out)
    );

    assert_flora_residuals(&flora, "100 coupled ticks");
    assert_fauna_residuals(&fauna, "100 coupled ticks");
    assert_fauna_residuals(&starver, "100 coupled ticks");
}

/// Dung: the mineral a bite carries in excess of the tissue it built is excreted as a
/// litter deposit on the animal's own face.
///
/// **What the placeholders do and do not make inert (Astra R9.2).** The excess depends on
/// the tissue **actually placed**, not on `yield_fraction`, so "there is never any excess
/// at the defaults" was wrong: it is true only of a *fresh founder's first bite*, where
/// the whole mouthful has somewhere to go. This test keeps that narrow arm, and adds the
/// two cases that do excrete — an animal with nowhere left to put the matter, at the
/// placeholders with **no knob touched**, and mineral-rich food with the grazer's own
/// numbers unchanged. The forced `n_tissue` 0 arm stays as the path's extreme.
#[test]
fn excess_mineral_is_excreted_as_litter() {
    let world = plain(8, 2, 0.3, 5);
    let mut flora = Flora::new(FloraConfig::default());
    turf(&mut flora, &world, 3);
    let mut fauna = Fauna::new(config_with(|s| s.n_tissue = 0.0));
    let id = grazer(&mut fauna, &world, 2, 0.02);
    let mineral_before = fauna.view().animal(id).unwrap().mineral;

    fauna.step(&world, &mut flora);

    let v = fauna.view();
    assert!(v.ledger.eaten_mineral_in > 0.0, "the bite carried mineral");
    assert_eq!(
        v.ledger.deposited_mineral_out, v.ledger.eaten_mineral_in,
        "all of it excreted"
    );
    assert_eq!(
        v.animal(id).unwrap().mineral,
        mineral_before,
        "the animal kept none of it"
    );
    let g = flora
        .view()
        .ground_at(at(2, 2))
        .expect("the dung provisioned the site");
    // A zero-organic deposit settles at once (Astra R8.2, round 5b): the mineral goes
    // straight to the site's soluble pool on top of the lazy provisioning, and nothing
    // waits in litter for organic matter that never comes.
    let provisioned = flora.config().initial_mineral;
    assert!((g.mineral - provisioned - v.ledger.deposited_mineral_out).abs() < 1e-15);
    assert_eq!(g.litter_mineral, 0.0, "nothing is stranded in litter");
    assert_eq!(
        g.litter, 0.0,
        "dung is mineral only this round: the rest was respired"
    );
    assert_residual_pair(&flora, &fauna, "after excretion");

    // The narrow true claim: a **fresh founder's first bite** carries no excess. Half a
    // bite assimilated is funded to 0.4 of it by a plant's own 0.02 of mineral, and a
    // young animal's body has room for all of that, so every unit of the mineral is
    // needed. This is not a property of the placeholders in general.
    let mut flora = Flora::new(FloraConfig::default());
    turf(&mut flora, &world, 3);
    let mut fauna = Fauna::new(FaunaConfig::default());
    grazer(&mut fauna, &world, 2, 0.02);
    fauna.step(&world, &mut flora);
    assert_eq!(
        fauna.view().ledger.deposited_mineral_out,
        0.0,
        "a fresh founder's first bite has somewhere to put every unit it can fund"
    );

    // A full animal no longer has a surplus to excrete (package G): at `body_max` with a
    // full reserve it takes only the little its upkeep made room for, every unit of it the
    // mineral funds is placed, and the bite's mineral is exactly what that tissue needs.
    let mut flora = Flora::new(FloraConfig::default());
    turf(&mut flora, &world, 3);
    let mut fauna = Fauna::new(FaunaConfig::default());
    let sc = *fauna.config().species(Species::Frondgrazer);
    let id = grazer(&mut fauna, &world, 2, sc.body_max);
    let before = *fauna.view().animal(id).unwrap();
    fauna.step(&world, &mut flora);
    let v = fauna.view();
    // Read the tissue placed off the ledger rather than off the animal: a birth is an
    // internal transfer, so `respired_out = upkeep + bite - placed` is the only reading
    // that is about the bite.
    let upkeep = sc.maintenance_per_s * before.body * DT;
    let placed = v.ledger.eaten_organic_in + upkeep - v.ledger.respired_out;
    assert!(
        v.ledger.bites > 0 && placed > 0.0 && placed <= upkeep,
        "it placed {placed}, within the {upkeep} its upkeep freed"
    );
    assert!(
        (placed - v.ledger.eaten_mineral_in / sc.n_tissue).abs() <= 1e-9 * placed,
        "all of what the mineral funds was placed"
    );
    assert!(
        v.ledger.deposited_mineral_out <= 1e-12 * v.ledger.eaten_mineral_in,
        "a full animal has nothing to excrete: {}",
        v.ledger.deposited_mineral_out
    );
    assert_eq!(
        v.ledger.born, 0,
        "and it paid for nothing: a full reserve is not a birth until the surplus has \
         stood for the hold"
    );
    assert_residual_pair(&flora, &fauna, "after a full animal's bite");

    // And mineral-rich food excretes with the **grazer's** numbers untouched: a turf whose
    // own `n_tissue` is 0.2 brings `2e-5` of mineral in a `1e-4` bite, ten times what the
    // tissue it funds can hold, and the remainder is dung.
    let mut flora = flora_with_plant_mineral(0.2);
    turf(&mut flora, &world, 3);
    let mut fauna = Fauna::new(FaunaConfig::default());
    let id = grazer(&mut fauna, &world, 2, 0.02);
    let before = *fauna.view().animal(id).unwrap();
    fauna.step(&world, &mut flora);
    let v = fauna.view();
    // Whatever the sated bite came to, the food is a fifth mineral.
    let eaten = v.ledger.eaten_organic_in;
    assert!(
        eaten > 0.0 && (v.ledger.eaten_mineral_in - 0.2 * eaten).abs() <= 1e-12 * eaten,
        "{} of {eaten}",
        v.ledger.eaten_mineral_in
    );
    // The yield is what binds here, not the mineral: half the bite is built, all of it
    // placed (read off the ledger: the reserve also pays for growth this tick).
    let upkeep = sc.maintenance_per_s * before.body * DT;
    let built = eaten + upkeep - v.ledger.respired_out;
    assert!((built - 0.5 * eaten).abs() <= 1e-9 * built, "it built {built}");
    let excess = v.ledger.eaten_mineral_in - sc.n_tissue * built;
    assert!(
        (v.ledger.deposited_mineral_out - excess).abs() <= 1e-9 * excess,
        "excreted {} of {excess}",
        v.ledger.deposited_mineral_out
    );
    let g = flora
        .view()
        .ground_at(at(2, 2))
        .expect("the dung provisioned the site");
    assert!(
        g.mineral > flora.config().initial_mineral,
        "and it is in the site's pool"
    );
    assert_residual_pair(&flora, &fauna, "after a mineral-rich bite");
}

// ------------------------------------------------------------- the keyed stream

/// Walk a grazer standing exactly between two equal patches of food and record where it
/// goes: two runs of the same world are identical, and the same fixture under a different
/// **world seed** breaks the tie the other way. Nothing but the seed differs — the terrain
/// is built by hand and is the same in both.
#[test]
fn the_keyed_stream_repeats_and_a_different_world_seed_does_not() {
    fn path(seed: u64) -> Vec<Site> {
        let world = plain(11, 2, 0.3, seed);
        let mut flora = Flora::new(FloraConfig::default());
        turf(&mut flora, &world, 1);
        turf(&mut flora, &world, 9);
        let mut fauna = Fauna::new(FaunaConfig::default());
        let id = grazer(&mut fauna, &world, 5, 0.02);
        let mut out = Vec::new();
        for _ in 0..60 {
            fauna.step(&world, &mut flora);
            out.push(fauna.view().animal(id).map(|a| a.site).unwrap_or(at(0, 0)));
        }
        out
    }
    assert_eq!(path(5), path(5), "the same world is the same walk");
    assert_ne!(
        path(5),
        path(6),
        "a different world seed is a different tie-break"
    );
}

// ---------------------------------------------------------- config and snapshot

/// `validate()` covers every field it can refuse, and `Fauna::try_new` reports instead of
/// panicking.
#[test]
fn validate_covers_the_new_fields() {
    assert!(FaunaConfig::default().validate().is_ok());
    let cases: [(&str, fn(&mut SpeciesConfig)); 7] = [
        ("maintenance_per_s", |s| s.maintenance_per_s = -1.0),
        ("yield_fraction", |s| s.yield_fraction = 1.5),
        ("step_period_s", |s| s.step_period_s = 0.0),
        ("body_min", |s| s.body_min = 0.0),
        ("birth_body", |s| s.birth_body = 1.0),
        ("birth_cost", |s| s.birth_cost = 0.0),
        ("energy_density", |s| s.energy_density = f64::NAN),
    ];
    for (what, edit) in cases {
        let c = config_with(edit);
        let err = Fauna::try_new(c)
            .expect_err("a refusable field")
            .to_string();
        assert!(
            err.contains(what) || err.contains("body_min"),
            "{what}: {err}"
        );
    }
    // One step per tick at least, however small the period.
    assert_eq!(SpeciesConfig::frondgrazer().step_period_ticks(), 20);
    let mut sc = SpeciesConfig::frondgrazer();
    sc.step_period_s = 1e-9;
    assert_eq!(sc.step_period_ticks(), 1);
}

/// The snapshot round-trips, and a snapshot of another schema is refused rather than
/// migrated (`always-fresh-never-migrate`).
#[test]
fn the_snapshot_round_trips_and_another_schema_is_refused() {
    let world = plain(6, 2, 0.3, 5);
    let mut flora = Flora::new(FloraConfig::default());
    turf(&mut flora, &world, 3);
    let mut fauna = Fauna::new(FaunaConfig::default());
    grazer(&mut fauna, &world, 2, 0.02);
    for _ in 0..5 {
        fauna.step(&world, &mut flora);
    }

    let bytes = fauna.save();
    let back = Fauna::load(&bytes).expect("its own bytes");
    assert_eq!(back.view().animals, fauna.view().animals);
    assert_eq!(back.view().ledger, fauna.view().ledger);
    assert_eq!(back.tick(), fauna.tick());

    let mut other = bytes.clone();
    other[0] = other[0].wrapping_add(1);
    let err = format!(
        "{:#}",
        Fauna::load(&other).expect_err("another tag is refused")
    );
    assert!(err.contains("schema") || err.contains("corrupt"), "{err}");
}
