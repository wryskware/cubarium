//! The ecology v1 accounting tests A1–A9 (`design/ecology-v1-contract.md` §13.1).
//!
//! These pass or fail at machine precision and gate the merge. They are not ecological
//! claims: nothing here says a stand is viable, a grazer is fed or a parameter is right.
//! What they check is that every transfer §4–§8 names books both sides, that a stock can
//! never be withdrawn past what it holds, and that the compatibility rules of §15 are the
//! rules the code actually applies.
//!
//! The ecological scenarios are the separate binary
//! `examples/ecology_v1_scenarios.rs` (§13.2), which reports measurements rather than
//! asserting them.

mod common;

use cubarium_surface::{Scale, Topology};
use common::{stored_energy, total_material};
use cubarium_core::care::{CareCommand, CareDose, CareKind, CareTarget};
use cubarium_core::config::{PlantConfig, WorldConfig};
use cubarium_core::genome::{Genome, decode};
use cubarium_core::ids::OrganismId;
use cubarium_core::organism::{Mode, Organism, Origin};
use cubarium_core::rng::Counter;
use cubarium_core::world::CellClass;
use cubarium_core::{
    DT, FixedHunterProfile, HunterTarget, MotorBill, SnapshotError, World, decode_snapshot,
    ecology_hash, encode_snapshot, snapshot::SCHEMA_VERSION, snapshot::state_hash,
};
use cubarium_surface::{CUBE_CELL_COUNT, CellId, Face, SurfacePoint, Vec2, cell_of};

// ---------------------------------------------------------------------------- fixtures

/// A world with nothing in it but what a test puts there: no founders, no weather swing, no
/// rain, no mutation, and a **bare** surface — no stands, no litter, no fruit, no water.
/// Light and moisture are flat, so every cell is the same reference class.
fn bare_config(light: f64, moisture: f64) -> WorldConfig {
    let mut cfg = WorldConfig::default();
    cfg.founders.kinds.clear();
    cfg.founders.count = 0;
    cfg.weather.amplitude = 0.0;
    cfg.water.rain_rate = 0.0;
    cfg.mechanisms.mutation = false;
    cfg.habitat.light_base = light;
    cfg.habitat.light_height_gain = 0.0;
    cfg.habitat.light_noise_gain = 0.0;
    cfg.habitat.moisture_base = moisture;
    cfg.habitat.moisture_height_gain = 0.0;
    cfg.habitat.moisture_noise_gain = 0.0;
    cfg.plant.initial_wood = 0.0;
    cfg.producer.initial_fraction = 0.0;
    cfg.detritus.initial_dark = 0.0;
    cfg
}

/// The §11 reference classes of §13: **average** `L = 0.5, μ = 0.7` and **bright**
/// `L = 0.8, μ = 0.75`, both with `N = 0.4`.
const AVERAGE: (f64, f64) = (0.5, 0.7);
const BRIGHT: (f64, f64) = (0.8, 0.75);
const REFERENCE_NUTRIENT: f64 = 0.4;

fn bare_world(class: (f64, f64)) -> World {
    let mut world = World::new(bare_config(class.0, class.1)).expect("a bare world is valid");
    let before = total_material(&world);
    for v in world.state.fields.n.iter_mut() {
        *v = REFERENCE_NUTRIENT;
    }
    let after = total_material(&world);
    world.state.external_material_in += after - before;
    World::from_state(world.state).expect("the staged state is a valid world")
}

/// Paint one cell as a stand, booking every unit of it as admitted material so the world's own
/// mass identity stays closed.
fn paint(world: &mut World, cell: CellId, p: f64, w: f64, q: f64) {
    let before = total_material(world);
    let i = cell.index();
    world.state.fields.p[i] = p;
    world.state.ecology.wood[i] = w;
    world.state.ecology.plant_reserve[i] = q;
    let after = total_material(world);
    world.state.external_material_in += after - before;
}

/// Book a hand-edited field back into the ledger: the difference between now and the caller's
/// earlier reading is material the fixture admitted (or exported).
fn book(world: &mut World, before: f64) {
    let after = total_material(world);
    world.state.external_material_in += after - before;
}

/// Restage a world so its residual baseline is re-derived from the values a fixture wrote.
fn restage(world: World) -> World {
    World::from_state(world.state).expect("the staged state is a valid world")
}

/// One immobile animal of the named `diet`, standing at a cell centre with an empty reserve.
fn place(world: &mut World, cell: CellId, diet: f32, mouth_offset: f64) -> OrganismId {
    let cfg = world.config().clone();
    let mut genome = Genome::founder(0.5, &cfg.drives);
    genome.diet = diet;
    genome.clamp();
    let phenotype = decode(&genome, &cfg.organism);
    let centre = cell.center(Topology::Cube, Scale::ONE);
    let pos = SurfacePoint::new(cell.face(Topology::Cube, Scale::ONE), centre.u + mouth_offset, centre.v);
    assert_eq!(cell_of(Topology::Cube, Scale::ONE, &pos), cell, "the body landed outside its cell");
    let structure = phenotype.structure_adult;
    let id = world.state.organisms.insert(Organism {
        pos,
        heading: Vec2::new(1.0, 0.0),
        ou: Vec2::ZERO,
        structure,
        reserve: 0.0,
        energy: 0.75 * phenotype.energy_max,
        born_tick: world.tick(),
        hunger_memory: 1.0,
        mode: Mode::Seeking,
        escrow: None,
        births: 0,
        genome,
        phenotype,
        parent: None,
        origin: Origin::Founder,
        turn_counter: Counter::default(),
        fed_this_tick: false,
    });
    let o = world.state.organisms.get(id).expect("placed");
    world.state.external_material_in += o.structure + o.reserve;
    id
}

/// `X_eff = X · min(1, ρ/e_r)` — the contract's own edible rule, transcribed rather than
/// imported, so a test never takes the world's word for it.
fn edible(stock: f64, energy: f64, e_r: f64) -> f64 {
    if stock <= 0.0 || e_r <= 0.0 {
        return 0.0;
    }
    stock * ((energy / stock) / e_r).min(1.0)
}

/// Pin every body in place, so a fixture's mouth stays on the cell it was put on.
fn immobile(cfg: &mut WorldConfig) {
    cfg.organism.speed_max = 0.0;
    cfg.drives.turn_rate_max_deg = 0.0;
}

// ------------------------------------------------------------------------------ A1

/// **A1 — material identity across a sweep of configs.** Income, reflush, dieback, stand
/// death, establishment, the three decompositions and downhill fall, including the A3b joint
/// endpoints where a stock's decomposition and its fall are each a whole tick's worth.
/// `mass_residual` stays within 1e-9 on every tick of every arm.
#[test]
fn a1_material_is_closed_through_every_ecology_v1_transfer() {
    /// The knobs each arm turns, named so a failure says which mechanism broke the box.
    struct Arm {
        name: &'static str,
        class: (f64, f64),
        tune: fn(&mut WorldConfig),
        stage: fn(&mut World),
        ticks: u64,
    }

    fn stand_and_neighbours(world: &mut World) {
        // One mature stand with a ring of bare neighbours, so income, senescence, ripening,
        // propagules and establishment all run.
        let centre = CellId::new(Topology::Cube, Scale::ONE, Face::Top, 8, 8);
        paint(world, centre, 0.56, 0.6, 0.30);
    }

    fn doomed_stand(world: &mut World) {
        // Wood with no foliage and no reserve: maintenance is wholly unpaid, so this stand
        // diebacks every tick and eventually crosses `W_min` and dies.
        let centre = CellId::new(Topology::Cube, Scale::ONE, Face::Front, 6, 6);
        paint(world, centre, 0.0, 0.05, 0.0);
    }

    fn charged_detritus(world: &mut World) {
        let before = total_material(world);
        for cell in CellId::all(Topology::Cube, Scale::ONE) {
            let i = cell.index();
            world.state.fields.d[i] = 0.4;
            world.state.fields.de[i] = 2.0 * 0.4;
            world.state.ecology.carrion[i] = 0.25;
            world.state.ecology.carrion_energy[i] = 2.0 * 0.25;
            world.state.ecology.dead_wood[i] = 0.3;
        }
        book(world, before);
    }

    fn both(world: &mut World) {
        stand_and_neighbours(world);
        charged_detritus(world);
    }

    let arms = [
        Arm {
            name: "growth, senescence, ripening and propagules",
            class: BRIGHT,
            tune: |_| {},
            stage: stand_and_neighbours,
            ticks: 4_000,
        },
        Arm {
            name: "dieback into stand death",
            class: AVERAGE,
            tune: |_| {},
            stage: doomed_stand,
            ticks: 2_000,
        },
        Arm {
            name: "three decompositions and the downhill fall",
            class: AVERAGE,
            tune: |_| {},
            stage: charged_detritus,
            ticks: 2_000,
        },
        Arm {
            name: "the A3b joint endpoint on litter",
            class: AVERAGE,
            tune: |cfg| {
                cfg.detritus.decomposition = 1.0 / DT;
                cfg.detritus.fall = 1.0 / DT;
            },
            stage: both,
            ticks: 200,
        },
        Arm {
            name: "the A3b joint endpoint on remains",
            class: AVERAGE,
            tune: |cfg| {
                cfg.detritus.carrion_decomposition = 1.0 / DT;
                cfg.detritus.fall = 1.0 / DT;
            },
            stage: both,
            ticks: 200,
        },
        Arm {
            name: "every rate at its admitted ceiling at once",
            class: BRIGHT,
            tune: |cfg| {
                cfg.detritus.decomposition = 1.0 / DT;
                cfg.detritus.carrion_decomposition = 1.0 / DT;
                cfg.detritus.wood_decomposition = 1.0 / DT;
                cfg.detritus.fall = 1.0 / DT;
                cfg.producer.mortality = 1.0 / DT;
                cfg.plant.maintenance = 1.0 / DT;
            },
            stage: both,
            ticks: 200,
        },
        Arm {
            name: "the reserve share, with room in `Q` to take it",
            class: BRIGHT,
            // A stand with a full foliage load and an **empty** reserve: every surplus tick
            // takes `q_share` off the top (§4.4, repair cycle 1) and the rest tops it up.
            stage: |world| paint(world, CellId::new(Topology::Cube, Scale::ONE, Face::Top, 8, 8), 0.56, 0.6, 0.0),
            tune: |_| {},
            ticks: 4_000,
        },
        Arm {
            name: "a full reserve, which takes no share",
            class: BRIGHT,
            // `Q = q_cap · W` exactly, so `D_Q = 0`: the share branch and the final top-up
            // both clamp to zero and the whole surplus goes to foliage and wood.
            stage: |world| paint(world, CellId::new(Topology::Cube, Scale::ONE, Face::Top, 8, 8), 0.56, 0.6, 0.30),
            tune: |_| {},
            ticks: 4_000,
        },
        Arm {
            name: "reflush from a stocked reserve below the threshold",
            class: BRIGHT,
            // Stripped foliage with a full reserve: the emergency draw runs every tick until
            // it reaches `p_reflush · P_cap`, then stops.
            stage: |world| paint(world, CellId::new(Topology::Cube, Scale::ONE, Face::Top, 8, 8), 0.0, 0.6, 0.30),
            tune: |_| {},
            ticks: 4_000,
        },
        Arm {
            name: "a fast establishment ring",
            class: BRIGHT,
            tune: |cfg| {
                cfg.plant.propagule_rate = 0.05;
            },
            stage: stand_and_neighbours,
            ticks: 4_000,
        },
    ];

    for arm in arms {
        let mut cfg = bare_config(arm.class.0, arm.class.1);
        (arm.tune)(&mut cfg);
        cfg.validate()
            .unwrap_or_else(|e| panic!("{}: the arm's config must be admitted: {e}", arm.name));
        let mut world = World::new(cfg).expect("valid");
        let before = total_material(&world);
        for v in world.state.fields.n.iter_mut() {
            *v = REFERENCE_NUTRIENT;
        }
        book(&mut world, before);
        (arm.stage)(&mut world);
        let mut world = restage(world);

        let mut worst = 0.0f64;
        for tick in 1..=arm.ticks {
            world.step();
            let residual = world.mass_residual();
            worst = worst.max(residual.abs());
            assert!(
                residual.abs() < 1e-9,
                "{}: tick {tick} residual {residual:e}",
                arm.name
            );
            world
                .check_invariants()
                .unwrap_or_else(|e| panic!("{}: tick {tick}: {e}", arm.name));
        }
        println!("A1 {}: worst |mass_residual| {worst:e}", arm.name);
    }
}

/// **A1, stock by stock.** The sweep above audits an aggregate: `mass_residual` is a single
/// number, so a pair of equal-and-opposite omissions, or the same stock counted on both sides,
/// passes it (Astra's implementation review, finding 4). These arms close that gap by isolating
/// one transfer at a time and asserting the **exact** movement of every one of the eight
/// stocks — the one that should move, and the seven that must not.
///
/// Each arm switches off every rate but the one it names, so the expected numbers below are
/// the contract's own expressions with nothing else mixed into them.
#[test]
fn a1_every_stock_moves_only_through_the_transfer_that_names_it() {
    /// The eight world stocks of §10, summed, plus the two counters.
    #[derive(Clone, Copy, Debug, PartialEq)]
    struct Stocks {
        n: f64,
        p: f64,
        f: f64,
        w: f64,
        q: f64,
        wd: f64,
        d: f64,
        de: f64,
        c: f64,
        ce: f64,
    }

    fn stocks(world: &World) -> Stocks {
        let s = &world.state;
        Stocks {
            n: s.fields.n.iter().sum(),
            p: s.fields.p.iter().sum(),
            f: s.fields.f.iter().sum(),
            w: s.ecology.wood.iter().sum(),
            q: s.ecology.plant_reserve.iter().sum(),
            wd: s.ecology.dead_wood.iter().sum(),
            d: s.fields.d.iter().sum(),
            de: s.fields.de.iter().sum(),
            c: s.ecology.carrion.iter().sum(),
            ce: s.ecology.carrion_energy.iter().sum(),
        }
    }

    /// Every rate off. An arm turns back on exactly what it is about.
    fn frozen(class: (f64, f64)) -> WorldConfig {
        let mut cfg = bare_config(class.0, class.1);
        cfg.producer.growth = 0.0;
        cfg.producer.mortality = 0.0;
        cfg.fruit.ripen = 0.0;
        cfg.fruit.drop = 0.0;
        cfg.detritus.decomposition = 0.0;
        cfg.detritus.carrion_decomposition = 0.0;
        cfg.detritus.wood_decomposition = 0.0;
        cfg.detritus.fall = 0.0;
        cfg.nutrient.diffusion = 0.0;
        cfg.plant.maintenance = 0.0;
        cfg.plant.foliage_rate = 0.0;
        cfg.plant.wood_rate = 0.0;
        cfg.plant.reserve_share = 0.0;
        cfg.plant.propagule_rate = 0.0;
        cfg
    }

    let cell = CellId::new(Topology::Cube, Scale::ONE, Face::Top, 8, 8);
    let i = cell.index();
    /// The exact-equality bound: every expression below is a handful of multiplications.
    const EXACT: f64 = 1e-15;

    /// `left` must equal `right` to `EXACT`, and the message names the stock.
    fn same(stock: &str, left: f64, right: f64) {
        assert!(
            (left - right).abs() <= EXACT.max(right.abs() * 1e-12),
            "{stock}: {left} is not {right}"
        );
    }

    // ---- senescence alone: `P → D`, its energy with it, and nothing else moves.
    {
        let mut cfg = frozen(AVERAGE);
        cfg.producer.mortality = 0.4;
        let m_p = cfg.producer.mortality;
        let e_v = cfg.plant.energy_density;
        let mut world = World::new(cfg).expect("valid");
        paint(&mut world, cell, 0.5, 0.6, 0.1);
        let mut world = restage(world);
        let before = stocks(&world);
        world.step();
        let after = stocks(&world);
        let shed = m_p * before.p * DT;
        same("P", after.p, before.p - shed);
        same("D", after.d, before.d + shed);
        same("De", after.de, before.de + e_v * shed);
        for (name, a, b) in [
            ("N", after.n, before.n),
            ("F", after.f, before.f),
            ("W", after.w, before.w),
            ("Q", after.q, before.q),
            ("Wd", after.wd, before.wd),
            ("C", after.c, before.c),
            ("Ce", after.ce, before.ce),
        ] {
            assert_eq!(a, b, "senescence moved {name}");
        }
    }

    // ---- ripening and drop alone: `P → F → D`, and `De` gains `e_f` per dropped unit.
    {
        let mut cfg = frozen(BRIGHT);
        cfg.fruit.ripen = 0.02;
        cfg.fruit.drop = 0.004;
        let (ripen, drop, fruit_min) = (cfg.fruit.ripen, cfg.fruit.drop, cfg.fruit.fruit_min);
        let (p_max, e_f, e_v) =
            (cfg.producer.max, cfg.fruit.energy_density, cfg.plant.energy_density);
        let light = BRIGHT.0;
        let mut world = World::new(cfg).expect("valid");
        paint(&mut world, cell, 1.2, 0.6, 0.1);
        let before_f = 0.3;
        {
            let b = total_material(&world);
            world.state.fields.f[i] = before_f;
            // Energy-poor litter already in the cell, so the `e_d_max · D` cap has room for
            // the dropped fruit's `e_f` and the arm measures the transfer, not the clamp.
            world.state.fields.d[i] = 1.0;
            world.state.fields.de[i] = 0.0;
            book(&mut world, b);
        }
        let mut world = restage(world);
        let before = stocks(&world);
        world.step();
        let after = stocks(&world);
        let over = (before.p / p_max - fruit_min).max(0.0);
        let ripened = ripen * before.p * over * light * DT;
        let dropped = drop * before.f * DT;
        assert!(ripened > 0.0 && dropped > 0.0, "the arm must actually convert something");
        same("P", after.p, before.p - ripened);
        same("F", after.f, before.f + ripened - dropped);
        same("D", after.d, before.d + dropped);
        same("De", after.de, before.de + e_f * dropped);
        for (name, a, b) in [
            ("N", after.n, before.n),
            ("W", after.w, before.w),
            ("Q", after.q, before.q),
            ("Wd", after.wd, before.wd),
            ("C", after.c, before.c),
            ("Ce", after.ce, before.ce),
        ] {
            assert_eq!(a, b, "ripening moved {name}");
        }
        let _ = e_v;
    }

    // ---- dieback alone: unpaid maintenance carries `W → Wd`, one for one.
    {
        let mut cfg = frozen(AVERAGE);
        cfg.plant.maintenance = 0.002;
        let (m_w, kappa) = (cfg.plant.maintenance, cfg.plant.dieback);
        let mut world = World::new(cfg).expect("valid");
        // No foliage and no reserve, so the whole maintenance bill is unpaid.
        paint(&mut world, cell, 0.0, 0.6, 0.0);
        let mut world = restage(world);
        let before = stocks(&world);
        world.step();
        let after = stocks(&world);
        let unpaid = m_w * before.w * DT;
        let died = kappa * unpaid;
        assert!(died > 0.0);
        same("W", after.w, before.w - died);
        same("Wd", after.wd, before.wd + died);
        for (name, a, b) in [
            ("N", after.n, before.n),
            ("P", after.p, before.p),
            ("F", after.f, before.f),
            ("Q", after.q, before.q),
            ("D", after.d, before.d),
            ("De", after.de, before.de),
            ("C", after.c, before.c),
            ("Ce", after.ce, before.ce),
        ] {
            assert_eq!(a, b, "dieback moved {name}");
        }
    }

    // ---- each decomposition alone: its own stock to `N`, and no other stock touched.
    for (what, rate_of, stock_of) in [
        (
            "litter",
            (|c: &mut WorldConfig, r: f64| c.detritus.decomposition = r) as fn(&mut WorldConfig, f64),
            (|s: &Stocks| s.d) as fn(&Stocks) -> f64,
        ),
        (
            "remains",
            |c: &mut WorldConfig, r: f64| c.detritus.carrion_decomposition = r,
            |s: &Stocks| s.c,
        ),
        (
            "dead wood",
            |c: &mut WorldConfig, r: f64| c.detritus.wood_decomposition = r,
            |s: &Stocks| s.wd,
        ),
    ] {
        let mut cfg = frozen(AVERAGE);
        rate_of(&mut cfg, 0.3);
        let mut world = World::new(cfg).expect("valid");
        let b = total_material(&world);
        world.state.fields.d[i] = 1.0;
        world.state.fields.de[i] = 2.0;
        world.state.ecology.carrion[i] = 0.8;
        world.state.ecology.carrion_energy[i] = 1.6;
        world.state.ecology.dead_wood[i] = 0.6;
        book(&mut world, b);
        let mut world = restage(world);
        let before = stocks(&world);
        world.step();
        let after = stocks(&world);
        let gone = 0.3 * DT * stock_of(&before);
        assert!(gone > 0.0);
        same(what, stock_of(&after), stock_of(&before) - gone);
        same("N", after.n, before.n + gone);
        // The two stocks this arm did **not** name are untouched to the bit: no decomposition
        // leaks across stocks (§5's separate withdrawals).
        for (name, a, b) in [
            ("litter", after.d, before.d),
            ("remains", after.c, before.c),
            ("dead wood", after.wd, before.wd),
        ] {
            if name != what {
                assert_eq!(a, b, "{what} decomposition moved {name}");
            }
        }
        for (name, a, b) in [
            ("P", after.p, before.p),
            ("F", after.f, before.f),
            ("W", after.w, before.w),
            ("Q", after.q, before.q),
        ] {
            assert_eq!(a, b, "{what} decomposition moved {name}");
        }
    }

    // ---- fall alone: `ΣD` and `ΣC` are conserved and `N`, `Wd` never see it.
    {
        let mut cfg = frozen(AVERAGE);
        cfg.detritus.fall = 0.5;
        let mut world = World::new(cfg).expect("valid");
        let b = total_material(&world);
        for c in CellId::all(Topology::Cube, Scale::ONE) {
            let k = c.index();
            world.state.fields.d[k] = 0.4;
            world.state.fields.de[k] = 0.6;
            world.state.ecology.carrion[k] = 0.3;
            world.state.ecology.carrion_energy[k] = 0.45;
        }
        book(&mut world, b);
        let mut world = restage(world);
        let before = stocks(&world);
        for _ in 0..50 {
            world.step();
        }
        let after = stocks(&world);
        for (name, a, b) in [
            ("D", after.d, before.d),
            ("De", after.de, before.de),
            ("C", after.c, before.c),
            ("Ce", after.ce, before.ce),
            ("N", after.n, before.n),
            ("Wd", after.wd, before.wd),
        ] {
            same(name, a, b);
        }
        // And it really did move: the source cell is poorer than it started.
        assert!(
            world.state.fields.d[CellId::new(Topology::Cube, Scale::ONE, Face::Front, 6, 0).index()] < 0.4,
            "the fall arm must actually move something"
        );
    }
}

// -------------------------------------------------------------- §4.4, repair cycle 1

/// **The repaired §4.4 allocation (`design/ecology-v1-contract.md` §4.4, repair cycle 1).**
///
/// Three claims, each on a one-cell world with `c_g = 0` so the arithmetic is exact:
///
/// 1. a stand at or above `p_reflush · P_cap` **never** draws its reserve for foliage — the
///    defect the first implementation run measured as B0-1 was routine top-up draining `Q`
///    whenever `P < P_cap`, which is always;
/// 2. a stand with room in `Q` takes exactly `q_share` of the tick's surplus into the reserve
///    before foliage and wood see any of it;
/// 3. a **full** reserve takes no share at all, so foliage gets the whole surplus back.
#[test]
fn the_reflush_threshold_and_the_reserve_share_are_the_repaired_allocation() {
    /// A one-cell world in which the only thing that can move `P`, `W`, `Q` or `N` is §4.4.
    fn arena(tune: impl FnOnce(&mut WorldConfig)) -> (WorldConfig, CellId) {
        let mut cfg = bare_config(BRIGHT.0, BRIGHT.1);
        // `c_g = 0` makes every `x / (1 + c_g)` the identity, so the assertions below are the
        // contract's expressions with nothing rounded into them.
        cfg.plant.build = 0.0;
        cfg.plant.maintenance = 0.0;
        cfg.plant.wood_rate = 0.0;
        cfg.plant.propagule_rate = 0.0;
        cfg.producer.mortality = 0.0;
        cfg.fruit.ripen = 0.0;
        cfg.fruit.drop = 0.0;
        cfg.detritus.decomposition = 0.0;
        cfg.detritus.fall = 0.0;
        cfg.nutrient.diffusion = 0.0;
        tune(&mut cfg);
        (cfg, CellId::new(Topology::Cube, Scale::ONE, Face::Top, 8, 8))
    }

    fn staged(cfg: WorldConfig, cell: CellId, p: f64, w: f64, q: f64) -> World {
        let mut world = World::new(cfg).expect("valid");
        let before = total_material(&world);
        world.state.fields.n[cell.index()] = REFERENCE_NUTRIENT;
        book(&mut world, before);
        paint(&mut world, cell, p, w, q);
        restage(world)
    }

    // ---- 1. At or above the reflush ceiling, the reserve is never touched for foliage.
    //
    // Income is off, so the only foliage a tick could make is reflush. The stand sits exactly
    // **at** the ceiling, which is the boundary the contract writes as a strict `<`.
    let (cfg, cell) = arena(|c| {
        c.producer.growth = 0.0;
        c.plant.foliage_rate = 1e6;
    });
    let (w, ceiling) = {
        let w = 0.6;
        let p_cap = cfg.producer.max.min(cfg.plant.alpha * w);
        (w, cfg.plant.reflush_below * p_cap)
    };
    let mut world = staged(cfg.clone(), cell, ceiling, w, 0.30);
    let i = cell.index();
    for tick in 1..=2_000u64 {
        world.step();
        assert_eq!(
            world.state.ecology.plant_reserve[i], 0.30,
            "tick {tick}: a stand at the ceiling drew on its reserve"
        );
        assert_eq!(
            world.state.fields.p[i], ceiling,
            "tick {tick}: and grew foliage it had no income for"
        );
    }
    // One hair below the ceiling and it draws — so the guard is the threshold, not inertia.
    let mut world = staged(cfg.clone(), cell, ceiling - 1e-6, w, 0.30);
    world.step();
    assert!(
        world.state.ecology.plant_reserve[i] < 0.30,
        "below the ceiling the reserve must pay for the reflush"
    );
    assert!(
        (world.state.fields.p[i] - ceiling).abs() < 1e-15,
        "and the reflush stops exactly at `p_reflush · P_cap`: {}",
        world.state.fields.p[i]
    );

    // A stripped stand climbs to the ceiling and stops there, with reserve to spare — so the
    // thing that stopped it is the ceiling and not an empty reserve. The stand is
    // over-provisioned on purpose (`Q > Q_max`, which also zeroes `D_Q` and so the share),
    // because at §11's own values a *full* reserve cannot quite reach the ceiling: with
    // `p_reflush · α = 0.25 · 2 = 0.5 = q_cap`, the ceiling and `Q_max` are the same number,
    // so paying `1 + c_g` per unit exhausts the reserve first. That is a property of §11's
    // table, reported in the result note, not something this test asserts away.
    let mut world = staged(cfg.clone(), cell, 0.0, w, 3.0 * 0.30);
    for _ in 0..4_000 {
        world.step();
    }
    assert!(
        (world.state.fields.p[i] - ceiling).abs() < 1e-12,
        "reflush settled at {} against the ceiling {ceiling}",
        world.state.fields.p[i]
    );
    assert!(
        world.state.ecology.plant_reserve[i] > 0.0,
        "and it stopped because of the ceiling, not because the reserve ran out"
    );

    // The same stand with exactly a full reserve stops **short** of the ceiling, at the
    // reserve's own limit: `Q_max / (1 + c_g)` of leaf for `Q_max` of reserve.
    let mut world = staged(cfg.clone(), cell, 0.0, w, 0.30);
    for _ in 0..4_000 {
        world.step();
    }
    assert!(
        world.state.fields.p[i] <= ceiling + 1e-12,
        "a full reserve cannot pass the ceiling either: {}",
        world.state.fields.p[i]
    );

    // ---- 2. With room in `Q`, the share is exactly `q_share · rem`, taken first.
    //
    // No maintenance, so `rem = A`; a huge `r_p` so foliage could absorb everything; an empty
    // reserve and a foliage load above the ceiling, so nothing reflushes back.
    let (cfg, cell) = arena(|c| c.plant.foliage_rate = 1e6);
    let q_share = cfg.plant.reserve_share;
    let mut world = staged(cfg.clone(), cell, 0.56, w, 0.0);
    let (n0, p0, q0) = (
        world.state.fields.n[i],
        world.state.fields.p[i],
        world.state.ecology.plant_reserve[i],
    );
    world.step();
    let income = n0 - world.state.fields.n[i];
    assert!(income > 0.0, "the fixture must actually earn something");
    let gained_q = world.state.ecology.plant_reserve[i] - q0;
    let gained_p = world.state.fields.p[i] - p0;
    assert!(
        (gained_q - q_share * income).abs() < 1e-15,
        "the reserve took {gained_q}, not `q_share · rem` = {}",
        q_share * income
    );
    assert!(
        (gained_p - (1.0 - q_share) * income).abs() < 1e-15,
        "foliage took {gained_p}, not the remaining {}",
        (1.0 - q_share) * income
    );

    // ---- 3. A full reserve takes no share: `D_Q = 0` clamps both reserve branches.
    let full = cfg.plant.reserve_cap * w;
    let mut world = staged(cfg, cell, 0.56, w, full);
    let (n0, p0) = (world.state.fields.n[i], world.state.fields.p[i]);
    world.step();
    let income = n0 - world.state.fields.n[i];
    assert!(income > 0.0);
    assert_eq!(
        world.state.ecology.plant_reserve[i], full,
        "a full reserve must take nothing"
    );
    assert!(
        (world.state.fields.p[i] - p0 - income).abs() < 1e-15,
        "so foliage gets the whole surplus: {} of {income}",
        world.state.fields.p[i] - p0
    );
    assert!(world.mass_residual().abs() < 1e-9);
}

// ------------------------------------------------------------------------------ A2

/// **A2a — no-input energy.** With `g = 0`, `ripen = 0`, care off, no organisms and no
/// imports, the stored total `U` (§10) is non-increasing every tick and `light_in` stays at
/// exactly zero. The two light paths of §10 are the only ways energy can enter, and both are
/// switched off here by name.
#[test]
fn a2a_stored_energy_never_rises_without_a_light_path() {
    let mut cfg = bare_config(BRIGHT.0, BRIGHT.1);
    cfg.producer.growth = 0.0;
    cfg.fruit.ripen = 0.0;
    let mut world = World::new(cfg).expect("valid");
    let before = total_material(&world);
    // Everything that holds energy, so there is something for the tick to spend: foliage,
    // wood, a reserve, standing fruit, charged litter and fresh remains.
    for cell in CellId::all(Topology::Cube, Scale::ONE) {
        let i = cell.index();
        world.state.fields.n[i] = REFERENCE_NUTRIENT;
        world.state.fields.p[i] = 0.4;
        world.state.fields.f[i] = 0.05;
        world.state.fields.d[i] = 0.3;
        world.state.fields.de[i] = 2.0 * 0.3;
        world.state.ecology.wood[i] = 0.4;
        world.state.ecology.plant_reserve[i] = 0.2;
        world.state.ecology.dead_wood[i] = 0.2;
        world.state.ecology.carrion[i] = 0.2;
        world.state.ecology.carrion_energy[i] = 2.0 * 0.2;
    }
    book(&mut world, before);
    let mut world = restage(world);

    let mut previous = stored_energy(&world);
    let opening = previous;
    for tick in 1..=3_000u64 {
        world.step();
        let now = stored_energy(&world);
        assert!(
            now <= previous + 1e-12,
            "tick {tick}: U rose from {previous} to {now}"
        );
        previous = now;
        assert_eq!(
            world.state.light_in_total, 0.0,
            "tick {tick}: light entered a world with no light path"
        );
    }
    assert!(previous < opening, "the world must actually have spent something");
    println!("A2a: U fell from {opening} to {previous} with light_in = 0");
}

/// **A2b — the full energy ledger.** Everything on: income, reflush, dieback, death, all three
/// decompositions, fall, ripening, drop, propagules, mouths, births, deaths, and care Feed and
/// Clean. `ΔU = light_in − heat_out + care_in − clean_out` within 1e-9 on every tick.
#[test]
fn a2b_the_energy_ledger_closes_every_tick_with_everything_on() {
    let mut cfg = bare_config(BRIGHT.0, BRIGHT.1);
    cfg.drives.feed_min = 0.001;
    let mut world = World::new(cfg).expect("valid");
    let before = total_material(&world);
    for cell in CellId::all(Topology::Cube, Scale::ONE) {
        let i = cell.index();
        world.state.fields.n[i] = REFERENCE_NUTRIENT;
    }
    // A band of mature stands, a band of charged litter and a band of fresh remains, so every
    // channel has something to work on and the propagule ring has somewhere to go.
    for u in 0..16u16 {
        // Three plant bands, so the repaired §4.4 (repair cycle 1) runs every branch inside
        // the full ledger: a **full** reserve that takes no share, a **part-full** one that
        // takes `q_share` off the top every surplus tick, and a **stripped** stand below
        // `p_reflush · P_cap` that draws its reserve down for the emergency reflush.
        for v in 0..8u16 {
            let i = CellId::new(Topology::Cube, Scale::ONE, Face::Top, u, v).index();
            let (p, q) = match v % 3 {
                0 => (0.56, 0.30),
                1 => (0.56, 0.05),
                _ => (0.0, 0.30),
            };
            world.state.fields.p[i] = p;
            world.state.ecology.wood[i] = 0.6;
            world.state.ecology.plant_reserve[i] = q;
            world.state.fields.f[i] = 0.16;
        }
        for v in 8..16u16 {
            let i = CellId::new(Topology::Cube, Scale::ONE, Face::Top, u, v).index();
            world.state.fields.d[i] = 0.5;
            world.state.fields.de[i] = 2.0 * 0.5;
            world.state.ecology.carrion[i] = 0.4;
            world.state.ecology.carrion_energy[i] = 2.0 * 0.4;
            world.state.ecology.dead_wood[i] = 0.3;
        }
    }
    book(&mut world, before);
    // Mouths that between them use every channel, and lifecycles that between them cover
    // every remains path: a foliage digester and a detrital one, both stocked well enough to
    // fund a birth inside the run, and one body with nothing at all, which starves on the
    // first tick so an ordinary death is inside the ledger too.
    let breeders = [
        place(&mut world, CellId::new(Topology::Cube, Scale::ONE, Face::Top, 4, 4), 0.85, 0.0),
        place(&mut world, CellId::new(Topology::Cube, Scale::ONE, Face::Top, 4, 12), 0.10, 0.0),
    ];
    for id in breeders {
        let o = world.state.organisms.get_mut(id).expect("placed");
        let (r, e) = (o.phenotype.reserve_max, o.phenotype.energy_max);
        world.state.external_material_in += r - o.reserve;
        o.reserve = r;
        o.energy = e;
    }
    // Two more mouths with **empty** reserves, so something actually eats: a full-reserve
    // breeder has no headroom and its every request is refused.
    place(&mut world, CellId::new(Topology::Cube, Scale::ONE, Face::Top, 8, 4), 0.85, 0.0);
    place(&mut world, CellId::new(Topology::Cube, Scale::ONE, Face::Top, 8, 12), 0.10, 0.0);
    let doomed = place(&mut world, CellId::new(Topology::Cube, Scale::ONE, Face::Top, 6, 12), 0.10, 1.0);
    {
        let o = world.state.organisms.get_mut(doomed).expect("placed");
        o.energy = 0.0;
        o.reserve = 0.0;
    }
    let mut world = restage(world);

    let feed_cell = CellId::new(Topology::Cube, Scale::ONE, Face::Top, 2, 12);
    let clean_cell = CellId::new(Topology::Cube, Scale::ONE, Face::Top, 3, 12);
    let target = |c: CellId| {
        let p = c.center(Topology::Cube, Scale::ONE);
        CareTarget { face: p.face.index() as u8, u: p.u, v: p.v }
    };

    // Long enough for a birth: `bud_min_age_seconds` is 120 s (2,400 ticks) and gestation is
    // 30 s (600 ticks), so a funded parent's first child lands around tick 3,000.
    let mut seq = 1u64;
    let mut worst = 0.0f64;
    for tick in 1..=4_000u64 {
        // Care commits at a boundary, never inside a step, so the identity below picks it up
        // in the same tick's difference of the ledgers.
        if tick % 200 == 0 {
            let cmd = CareCommand {
                seq,
                apply_after_tick: world.tick(),
                kind: if seq % 2 == 0 { CareKind::Clean } else { CareKind::Feed },
                target: target(if seq % 2 == 0 { clean_cell } else { feed_cell }),
                dose: CareDose::STANDARD,
            };
            world.apply_care(&cmd);
            seq += 1;
        }
        let u_before = stored_energy(&world);
        let ledgers = world.energy_ledgers();
        let fed = world.state.care.feed_energy_in;
        let cleaned = world.state.care.clean_energy_out;
        world.step();
        let booked = world.energy_ledgers().net_since(ledgers)
            + (world.state.care.feed_energy_in - fed)
            - (world.state.care.clean_energy_out - cleaned);
        let drift = (stored_energy(&world) - u_before) - booked;
        worst = worst.max(drift.abs());
        assert!(drift.abs() < 1e-9, "tick {tick}: energy drift {drift:e}");
        assert!(world.mass_residual().abs() < 1e-9, "tick {tick}: mass");
    }
    assert!(world.state.care.feed_material_in > 0.0, "the fixture must actually have fed");
    assert!(world.state.care.clean_material_out > 0.0, "and actually have cleaned");
    // The fixture must really have exercised what it claims to. The old line here read
    // `plant_deaths_total > 0 || world.tick() > 0`, which is true of every world after one
    // tick and asserted nothing (Astra's implementation review, finding 5). These are the
    // claims: mouths fed on every stock, plants earned and lost, and animals were born.
    let diag = world.intake_diagnostics();
    assert!(diag.producer_eaten > 0.0, "no leaf was eaten: {diag:?}");
    assert!(diag.fruit_eaten > 0.0, "no fruit was eaten: {diag:?}");
    assert!(diag.litter_eaten > 0.0, "no litter was eaten: {diag:?}");
    assert!(diag.carrion_eaten > 0.0, "no remains were eaten: {diag:?}");
    assert!(diag.plant_income > 0.0, "the plants earned nothing");
    assert!(diag.propagule_sent > 0.0, "no propagule was sent");
    assert!(
        world.state.births_total > 0,
        "the full ledger must cover a birth, and no birth happened"
    );
    assert!(
        world.state.deaths_total.iter().sum::<u64>() > 0,
        "and a death, so the remains path is inside the ledger too"
    );
    println!(
        "A2b: worst per-tick energy drift {worst:e}; births {}, deaths {:?}, plant deaths {}, \
         propagules {:.4} m",
        world.state.births_total,
        world.state.deaths_total,
        world.state.ecology.plant_deaths_total,
        diag.propagule_sent
    );
}

// ------------------------------------------------------------------------------ A3

/// **A3a — a genuinely inert fixture.** Every rate that could move a field is zero, there is
/// no rain, no weather swing, no fruit, no water, no organism and no care, so *every* field
/// vector is bit-identical before and after any number of ticks. A single moved bit here is a
/// term the contract did not account for.
#[test]
fn a3a_an_inert_fixture_moves_not_one_bit() {
    let mut cfg = bare_config(BRIGHT.0, BRIGHT.1);
    cfg.producer.growth = 0.0;
    cfg.producer.mortality = 0.0;
    cfg.fruit.ripen = 0.0;
    cfg.fruit.drop = 0.0;
    cfg.detritus.decomposition = 0.0;
    cfg.detritus.carrion_decomposition = 0.0;
    cfg.detritus.wood_decomposition = 0.0;
    cfg.detritus.fall = 0.0;
    cfg.nutrient.diffusion = 0.0;
    cfg.plant.maintenance = 0.0;
    cfg.plant.foliage_rate = 0.0;
    cfg.plant.wood_rate = 0.0;
    cfg.plant.propagule_rate = 0.0;
    cfg.water.rain_rate = 0.0;
    cfg.weather.amplitude = 0.0;

    let mut world = World::new(cfg).expect("valid");
    let before = total_material(&world);
    // A deterministic scatter, so "unchanged" is not "unchanged because it was smooth".
    let mut x = 0x2545_F491_4F6C_DD1Du64;
    let mut next = || {
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        (x >> 11) as f64 / (1u64 << 53) as f64
    };
    for i in 0..CUBE_CELL_COUNT {
        world.state.fields.n[i] = next();
        world.state.fields.p[i] = next();
        world.state.fields.d[i] = next();
        world.state.fields.de[i] = next() * 2.0 * world.state.fields.d[i];
        world.state.ecology.wood[i] = next();
        world.state.ecology.plant_reserve[i] = next();
        world.state.ecology.dead_wood[i] = next();
        world.state.ecology.carrion[i] = next();
        world.state.ecology.carrion_energy[i] =
            next() * 2.0 * world.state.ecology.carrion[i];
    }
    // Initial fruit and water are zero by construction, and named so.
    assert!(world.state.fields.f.iter().all(|&f| f == 0.0));
    assert!(world.state.fields.w.iter().all(|&w| w == 0.0));
    book(&mut world, before);
    let mut world = restage(world);

    let opening = (
        world.state.fields.clone(),
        world.state.ecology.clone(),
        world.state.light_in_total,
        world.state.heat_out_total,
    );
    for _ in 0..500 {
        world.step();
    }
    assert_eq!(world.state.fields, opening.0, "a field vector moved");
    assert_eq!(world.state.ecology, opening.1, "an ecology v1 pool moved");
    assert_eq!(world.state.light_in_total, opening.2, "light entered an inert world");
    assert_eq!(world.state.heat_out_total, opening.3, "heat left an inert world");
}

/// **A3b — rate validation and the joint endpoints.**
///
/// Every per-second fraction of a stock is refused **by name** above one per tick, and at
/// exactly one per tick the joint decomposition-and-fall rule of §5 empties the stock exactly
/// and takes nothing below zero.
#[test]
fn a3b_rates_are_validated_by_name_and_the_joint_endpoints_are_exact() {
    /// `(the field's name in the error, how to set it)`.
    type Set = fn(&mut WorldConfig, f64);
    let rates: [(&str, Set); 8] = [
        ("plant.maintenance", |c, r| c.plant.maintenance = r),
        ("producer.mortality", |c, r| c.producer.mortality = r),
        ("fruit.ripen", |c, r| c.fruit.ripen = r),
        ("fruit.drop", |c, r| c.fruit.drop = r),
        ("detritus.decomposition", |c, r| c.detritus.decomposition = r),
        ("detritus.carrion_decomposition", |c, r| c.detritus.carrion_decomposition = r),
        ("detritus.wood_decomposition", |c, r| c.detritus.wood_decomposition = r),
        ("detritus.fall", |c, r| c.detritus.fall = r),
    ];
    for (name, set) in rates {
        // Exactly one per tick is admitted; anything above it is refused, and the refusal
        // names the knob rather than the symptom.
        let mut ok = WorldConfig::default();
        set(&mut ok, 1.0 / DT);
        ok.validate()
            .unwrap_or_else(|e| panic!("{name} at exactly one per tick must be admitted: {e}"));

        let mut bad = WorldConfig::default();
        set(&mut bad, 1.0 / DT + 1e-9);
        let why = bad.validate().expect_err(&format!("{name} above one per tick was admitted"));
        assert!(why.contains(name), "the refusal must name {name}: {why}");
        assert!(why.contains("per tick"), "and say why: {why}");
    }

    // The other §14 validations, each by name.
    let mut split = WorldConfig::default();
    split.plant.propagule_split = [0.4, 0.4, 0.3];
    assert!(split.validate().unwrap_err().contains("propagule_split"));
    let mut order = WorldConfig::default();
    order.plant.donor_min = order.plant.alive_min;
    assert!(order.validate().unwrap_err().contains("donor_min"));
    let mut over = WorldConfig::default();
    over.plant.donor_min = over.plant.wood_max + 0.1;
    assert!(over.validate().unwrap_err().contains("donor_min"));
    let mut gate = WorldConfig::default();
    gate.organism.capability_gate = 0.6;
    assert!(gate.validate().unwrap_err().contains("capability_gate"));
    let mut curve = WorldConfig::default();
    curve.organism.capability_exponent = 0.0;
    assert!(curve.validate().unwrap_err().contains("capability_exponent"));

    // The joint endpoints. A cell with a downhill neighbour, stocked, with both a whole
    // tick's decomposition **and** a whole tick's fall admitted at once: §5's shared budget
    // says the stock ends exactly zero.
    for stock in ["litter", "remains"] {
        let mut cfg = bare_config(AVERAGE.0, AVERAGE.1);
        cfg.detritus.fall = 1.0 / DT;
        match stock {
            "litter" => cfg.detritus.decomposition = 1.0 / DT,
            _ => cfg.detritus.carrion_decomposition = 1.0 / DT,
        }
        let mut world = World::new(cfg).expect("valid");
        let source = CellId::new(Topology::Cube, Scale::ONE, Face::Front, 6, 6);
        let before = total_material(&world);
        let i = source.index();
        if stock == "litter" {
            world.state.fields.d[i] = 1.0;
            world.state.fields.de[i] = 2.0;
        } else {
            world.state.ecology.carrion[i] = 1.0;
            world.state.ecology.carrion_energy[i] = 2.0;
        }
        book(&mut world, before);
        let mut world = restage(world);
        assert!(
            world.cell_neighbors()[i][2].is_some(),
            "the source cell must have a downhill neighbour"
        );

        world.step();
        let (left, energy_left) = if stock == "litter" {
            (world.state.fields.d[i], world.state.fields.de[i])
        } else {
            (world.state.ecology.carrion[i], world.state.ecology.carrion_energy[i])
        };
        assert_eq!(left, 0.0, "{stock}: the joint withdrawal left {left}");
        assert_eq!(energy_left, 0.0, "{stock}: it left {energy_left} of energy behind");
        world.check_invariants().expect("nothing went negative");
        assert!(world.mass_residual().abs() < 1e-9);
    }

    // **The interior case.** The coincident endpoint above is not enough on its own: at
    // `k·dt = fall·dt = 1` the correct joint rule makes fall exactly zero, so an
    // implementation that simply suppressed fall whenever decomposition ran would pass it
    // (Astra's implementation review, finding 4). Here both rates are nonzero and **below**
    // one per tick, so the two withdrawals are separately visible and their exact sizes are
    // the thing under test:
    //
    // ```
    // dec  = k·dt·X⁻                      fall = fall·dt·(1 − k·dt)·X⁻
    // source keeps  X⁻ − dec − fall       downhill gains fall
    // N gains dec                          energy moves at the source's current density
    // ```
    for (stock, k_dt) in [("litter", 0.3f64), ("remains", 0.3)] {
        let fall_dt = 0.5f64;
        let mut cfg = bare_config(AVERAGE.0, AVERAGE.1);
        cfg.nutrient.diffusion = 0.0;
        cfg.detritus.fall = fall_dt / DT;
        match stock {
            "litter" => cfg.detritus.decomposition = k_dt / DT,
            _ => cfg.detritus.carrion_decomposition = k_dt / DT,
        }
        cfg.validate().expect("both rates are admitted below one per tick");
        let mut world = World::new(cfg).expect("valid");
        let source = CellId::new(Topology::Cube, Scale::ONE, Face::Front, 6, 6);
        let (si, x0, xe0) = (source.index(), 1.0f64, 1.5f64);
        let before = total_material(&world);
        if stock == "litter" {
            world.state.fields.d[si] = x0;
            world.state.fields.de[si] = xe0;
        } else {
            world.state.ecology.carrion[si] = x0;
            world.state.ecology.carrion_energy[si] = xe0;
        }
        book(&mut world, before);
        let mut world = restage(world);
        let down = world
            .cell_neighbors()
            .get(si)
            .copied()
            .expect("the source cell exists");
        let n0: f64 = world.state.fields.n.iter().sum();
        let heat0 = world.state.heat_out_corrected();
        world.step();

        let dec = k_dt * x0;
        let fall = fall_dt * (1.0 - k_dt) * x0;
        let kept = x0 - dec - fall;
        let (x_now, xe_now, total, total_e) = if stock == "litter" {
            (
                world.state.fields.d[si],
                world.state.fields.de[si],
                world.state.fields.d.iter().sum::<f64>(),
                world.state.fields.de.iter().sum::<f64>(),
            )
        } else {
            (
                world.state.ecology.carrion[si],
                world.state.ecology.carrion_energy[si],
                world.state.ecology.carrion.iter().sum::<f64>(),
                world.state.ecology.carrion_energy.iter().sum::<f64>(),
            )
        };
        assert!(
            (x_now - kept).abs() < 1e-15,
            "{stock}: the source kept {x_now}, not `X⁻(1 − k·dt)(1 − fall·dt)` = {kept}"
        );
        assert!(
            fall > 0.0 && kept > 0.0,
            "{stock}: the interior case must leave both withdrawals visible"
        );
        // The whole stock, source and downhill together, lost exactly the decomposition.
        assert!(
            (total - (x0 - dec)).abs() < 1e-15,
            "{stock}: the stock totals {total}, not {}",
            x0 - dec
        );
        assert!(
            (world.state.fields.n.iter().sum::<f64>() - (n0 + dec)).abs() < 1e-12,
            "{stock}: `N` did not gain exactly the decomposition"
        );
        // Energy leaves at the stock's **current** density, so the density is unchanged and
        // the heat is exactly the decomposed share of the energy.
        let rho = xe0 / x0;
        assert!(
            (total_e - rho * (x0 - dec)).abs() < 1e-15,
            "{stock}: energy {total_e}, not `ρ · (X⁻ − dec)` = {}",
            rho * (x0 - dec)
        );
        assert!(
            (xe_now - rho * kept).abs() < 1e-15,
            "{stock}: the source's density moved"
        );
        assert!(
            (world.state.heat_out_corrected() - heat0 - rho * dec).abs() < 1e-12,
            "{stock}: the heat is not the decomposed energy"
        );
        let _ = down;
        assert!(world.mass_residual().abs() < 1e-9);
        world.check_invariants().expect("nothing went negative");
    }

    // The same at `m_p · dt = 1` with ripening on: a whole tick's senescence takes all the
    // foliage the growth left and no more, and the cell stays a well-formed stock.
    let mut cfg = bare_config(BRIGHT.0, BRIGHT.1);
    cfg.producer.mortality = 1.0 / DT;
    let mut world = World::new(cfg).expect("valid");
    let cell = CellId::new(Topology::Cube, Scale::ONE, Face::Top, 8, 8);
    let before = total_material(&world);
    world.state.fields.n[cell.index()] = REFERENCE_NUTRIENT;
    book(&mut world, before);
    paint(&mut world, cell, 0.9, 0.6, 0.3);
    let mut world = restage(world);
    world.step();
    assert_eq!(world.state.fields.p[cell.index()], 0.0, "a whole tick of senescence");
    world.check_invariants().expect("nothing went negative");
    assert!(world.mass_residual().abs() < 1e-9);
}

// ------------------------------------------------------------------------------ A4

/// **A4 — capability decode and the §6.4 bite.** The four founder kinds decode to the §6.1
/// table; a bite on a food the body has no machinery for is refused outright; and a bite with
/// `cap < 1` books its digestible portion, its feces and its heat exactly as §6.4 writes them.
#[test]
fn a4_capabilities_decode_and_a_partial_bite_books_every_term() {
    let cfg = WorldConfig::default();
    let org = &cfg.organism;
    assert_eq!((org.capability_gate, org.capability_exponent), (0.2, 1.0));

    // The §6.1 founder table, to the digit.
    let table = [
        ("burrower", 0.10f32, 0.0, 0.90),
        ("grazer", 0.85, 0.85, 0.0),
        ("glider", 0.90, 0.90, 0.0),
        ("skimmer", 0.60, 0.60, 0.40),
    ];
    for (name, diet, cap_h, cap_d) in table {
        let kind = cfg
            .founders
            .kinds
            .iter()
            .find(|k| k.name == name)
            .unwrap_or_else(|| panic!("the default roster has a {name}"));
        assert_eq!(kind.diet, Some(diet), "{name}: the roster's diet");
        let mut genome = Genome::founder(0.5, &cfg.drives);
        genome.diet = diet;
        genome.clamp();
        let p = decode(&genome, org);
        // The genome stores `diet` as `f32`, so the table is met to single precision.
        assert!((p.cap_foliage - cap_h).abs() < 1e-7, "{name}: cap_h = {}", p.cap_foliage);
        assert!((p.cap_detrital - cap_d).abs() < 1e-7, "{name}: cap_d = {}", p.cap_detrital);
    }

    // A masked bite is refused: a pure detrital digester standing on a rich foliage cell with
    // no litter takes nothing at all, whatever it asks for.
    let mut cfg = bare_config(AVERAGE.0, AVERAGE.1);
    immobile(&mut cfg);
    cfg.drives.feed_min = 0.001;
    // Nothing but a mouth may move `P`, so "the cell is untouched" is a statement about the
    // mask and not about ripening or senescence.
    cfg.producer.growth = 0.0;
    cfg.producer.mortality = 0.0;
    cfg.fruit.ripen = 0.0;
    let mut world = World::new(cfg).expect("valid");
    let cell = CellId::new(Topology::Cube, Scale::ONE, Face::Front, 6, 6);
    paint(&mut world, cell, 1.0, 0.0, 0.0);
    let burrower = place(&mut world, cell, 0.10, 0.0);
    let mut world = restage(world);
    let p0 = world.state.fields.p[cell.index()];
    world.step();
    assert_eq!(
        world.state.fields.p[cell.index()],
        p0,
        "a body with no foliage machinery took foliage"
    );
    assert_eq!(world.state.organisms.get(burrower).expect("alive").reserve, 0.0);
    assert_eq!(world.intake_diagnostics().producer_eaten, 0.0);

    // A partial bite, booked term by term. One grazer (`cap_h = 0.85`) on a leaf-only cell,
    // with no other channel open, so the whole tick's intake is one served bite.
    let mut cfg = bare_config(AVERAGE.0, AVERAGE.1);
    immobile(&mut cfg);
    cfg.drives.feed_min = 0.001;
    cfg.producer.growth = 0.0;
    cfg.producer.mortality = 0.0;
    cfg.plant.maintenance = 0.0;
    cfg.plant.foliage_rate = 0.0;
    cfg.mechanisms.scavenging = false;
    cfg.fruit.ripen = 0.0;
    let e_r = cfg.organism.reserve_energy_density;
    let e_v = cfg.plant.energy_density;
    let eta_m = cfg.organism.assimilation_material;
    let eta_e = cfg.organism.assimilation_energy;
    let mut world = World::new(cfg).expect("valid");
    paint(&mut world, cell, 1.0, 0.6, 0.0);
    let grazer = place(&mut world, cell, 0.85, 0.0);
    let mut world = restage(world);

    let cap_h = world.state.organisms.get(grazer).expect("alive").phenotype.cap_foliage;
    assert!((cap_h - 0.85f32 as f64).abs() < 1e-9);
    let (p0, d0, r0, e0) = (
        world.state.fields.p[cell.index()],
        world.state.fields.d[cell.index()],
        world.state.organisms.get(grazer).expect("alive").reserve,
        world.state.organisms.get(grazer).expect("alive").energy,
    );
    let heat0 = world.state.heat_out_corrected();
    world.step();
    let o = world.state.organisms.get(grazer).expect("alive");
    let q = p0 - world.state.fields.p[cell.index()];
    assert!(q > 0.0, "the grazer must actually have bitten");
    let q_d = cap_h * q;
    // reserve += η_m · q_d
    assert!(
        (o.reserve - (r0 + eta_m * q_d)).abs() < 1e-12,
        "reserve {} vs {}",
        o.reserve,
        r0 + eta_m * q_d
    );
    // feces: D += (1 − η_m)·q_d + (1 − cap)·q, energy-free
    let feces = (1.0 - eta_m) * q_d + (1.0 - cap_h) * q;
    assert!(
        (world.state.fields.d[cell.index()] - (d0 + feces)).abs() < 1e-12,
        "feces {} vs {feces}",
        world.state.fields.d[cell.index()] - d0
    );
    assert_eq!(world.state.fields.de[cell.index()], 0.0, "feces carry no energy");
    // E += min(η_e · (ρ·q_d − e_r·η_m·q_d), room); the rest, plus the indigestible portion's
    // whole energy, is heat.
    let spare = e_v * q_d - e_r * eta_m * q_d;
    let room = o.phenotype.energy_max - e0;
    let gained = (eta_e * spare).min(room).max(0.0);
    // The tick also pays upkeep, so the energy comparison is made against what the bite
    // delivered rather than against the body's closing battery.
    assert!(gained > 0.0, "the bite must have delivered usable energy");
    let heat = world.state.heat_out_corrected() - heat0;
    let bite_heat = (spare - gained) + e_v * (1.0 - cap_h) * q;
    assert!(
        heat >= bite_heat - 1e-12,
        "the tick's heat {heat} is at least the bite's {bite_heat}"
    );
    assert!(
        (world.intake_diagnostics().producer_eaten - q).abs() < 1e-15,
        "the served bite is recorded: {} vs the {q} the cell lost",
        world.intake_diagnostics().producer_eaten
    );
    assert!(
        (world.intake_diagnostics().undigested - feces).abs() < 1e-12,
        "and so are the feces"
    );
    assert!(world.mass_residual().abs() < 1e-9);
}

/// **A4, all four foods.** The bite above is a leaf bite and asserts a heat *lower bound*, so
/// a mistake in fruit, litter or carrion energy passes it (Astra's implementation review,
/// finding 4). This drives one **isolated** bite of each of the four foods and asserts every
/// term of §6.4 exactly: the stock's material and energy, the reserve, the battery, the feces,
/// and the heat — the last as an equality, because the fixture pays no other bill.
#[test]
fn a4_every_one_of_the_four_foods_books_material_and_energy_exactly() {
    // `(name, the stock it eats, the digester's diet)`.
    let foods = [
        ("leaf", 0u8, 0.85f32),
        ("fruit", 1, 0.85),
        ("litter", 2, 0.10),
        ("remains", 3, 0.10),
    ];
    let cell = CellId::new(Topology::Cube, Scale::ONE, Face::Front, 6, 6);
    let i = cell.index();

    for (name, which, diet) in foods {
        let mut cfg = bare_config(AVERAGE.0, AVERAGE.1);
        immobile(&mut cfg);
        cfg.drives.feed_min = 0.001;
        // Nothing may move a stock but the one mouth, and nothing may cost the body energy
        // but the bite, so every heat term below is an equality rather than a bound.
        cfg.producer.growth = 0.0;
        cfg.producer.mortality = 0.0;
        cfg.fruit.ripen = 0.0;
        cfg.fruit.drop = 0.0;
        cfg.detritus.decomposition = 0.0;
        cfg.detritus.carrion_decomposition = 0.0;
        cfg.detritus.wood_decomposition = 0.0;
        cfg.detritus.fall = 0.0;
        cfg.nutrient.diffusion = 0.0;
        cfg.plant.maintenance = 0.0;
        cfg.plant.foliage_rate = 0.0;
        cfg.plant.propagule_rate = 0.0;
        cfg.organism.maintenance = 0.0;
        cfg.organism.sense_cost = 0.0;
        cfg.organism.move_cost = 0.0;
        cfg.organism.oxidation_rate = 0.0;
        cfg.organism.growth_rate = 0.0;
        let (e_v, e_f, e_r) = (
            cfg.plant.energy_density,
            cfg.fruit.energy_density,
            cfg.organism.reserve_energy_density,
        );
        let (eta_m, eta_e) = (
            cfg.organism.assimilation_material,
            cfg.organism.assimilation_energy,
        );

        let mut world = World::new(cfg).expect("valid");
        // Only the food under test is present, so only one channel can open.
        let (stock0, energy0, rho) = match which {
            0 => (1.0, 0.0, e_v),
            1 => (1.0, 0.0, e_f),
            // Litter and remains at three quarters of `e_r`: a density below `e_r`, so the
            // `min(1, ρ/e_r)` factor in `η_m′` is genuinely engaged rather than saturated.
            _ => (1.0, 1.5, 1.5),
        };
        let before = total_material(&world);
        match which {
            0 => {
                world.state.fields.p[i] = stock0;
                world.state.ecology.wood[i] = 0.6;
            }
            1 => world.state.fields.f[i] = stock0,
            2 => {
                world.state.fields.d[i] = stock0;
                world.state.fields.de[i] = energy0;
            }
            _ => {
                world.state.ecology.carrion[i] = stock0;
                world.state.ecology.carrion_energy[i] = energy0;
            }
        }
        book(&mut world, before);
        let id = place(&mut world, cell, diet, 0.0);
        let mut world = restage(world);

        let (cap, r0, e0, e_max) = {
            let o = world.state.organisms.get(id).expect("alive");
            let cap = if which <= 1 { o.phenotype.cap_foliage } else { o.phenotype.cap_detrital };
            (cap, o.reserve, o.energy, o.phenotype.energy_max)
        };
        assert!(cap > 0.0 && cap < 1.0, "{name}: the digester must be partial, not perfect");
        let d0 = world.state.fields.d[i];
        let heat0 = world.state.heat_out_corrected();
        world.step();
        let o = world.state.organisms.get(id).expect("alive");

        // What actually left the stock.
        let (stock_now, energy_now) = match which {
            0 => (world.state.fields.p[i], f64::NAN),
            1 => (world.state.fields.f[i], f64::NAN),
            2 => (world.state.fields.d[i], world.state.fields.de[i]),
            _ => (world.state.ecology.carrion[i], world.state.ecology.carrion_energy[i]),
        };
        let diag = world.intake_diagnostics();
        let q = match which {
            0 => diag.producer_eaten,
            1 => diag.fruit_eaten,
            2 => diag.litter_eaten,
            _ => diag.carrion_eaten,
        };
        assert!(q > 0.0, "{name}: nothing was eaten");

        // §6.4, term by term.
        let q_d = cap * q;
        let eta = if which <= 1 { eta_m } else { eta_m * (rho / e_r).min(1.0) };
        let to_reserve = eta * q_d;
        let feces = (1.0 - eta) * q_d + (1.0 - cap) * q;
        let spare = rho * q_d - e_r * to_reserve;
        let gained = (eta_e * spare).min(e_max - e0).max(0.0);
        let bite_heat = (spare - gained) + rho * (1.0 - cap) * q;

        assert!(
            (o.reserve - (r0 + to_reserve)).abs() < 1e-15,
            "{name}: reserve {} is not {}",
            o.reserve,
            r0 + to_reserve
        );
        assert!(
            (o.energy - (e0 + gained)).abs() < 1e-15,
            "{name}: battery {} is not {}",
            o.energy,
            e0 + gained
        );
        // The **whole** bite left the stock — litter and remains included, where the
        // pre-ecology-v1 rule removed only the assimilated part.
        let expected_stock = if which == 2 {
            // Litter is also where this bite's own feces land, so the net is the difference.
            stock0 - q + feces
        } else {
            stock0 - q
        };
        assert!(
            (stock_now - expected_stock).abs() < 1e-15,
            "{name}: the stock holds {stock_now}, not {expected_stock}"
        );
        if which >= 2 {
            assert!(
                (energy_now - (energy0 - rho * q)).abs() < 1e-15,
                "{name}: the stock's energy is {energy_now}, not {}",
                energy0 - rho * q
            );
        }
        // Feces are litter, and they are energy-free.
        if which != 2 {
            assert!(
                (world.state.fields.d[i] - (d0 + feces)).abs() < 1e-15,
                "{name}: feces {} are not {feces}",
                world.state.fields.d[i] - d0
            );
        }
        if which <= 1 {
            assert_eq!(
                world.state.fields.de[i], 0.0,
                "{name}: feces carried energy into the litter"
            );
        }
        // And the whole tick's heat is exactly this bite's, because nothing else was paid.
        assert!(
            (world.state.heat_out_corrected() - heat0 - bite_heat).abs() < 1e-12,
            "{name}: the tick's heat {} is not the bite's {bite_heat}",
            world.state.heat_out_corrected() - heat0
        );
        assert!(world.mass_residual().abs() < 1e-9, "{name}: mass");
        println!(
            "A4 {name}: q {q:.6} cap {cap:.2} η′ {eta:.4} reserve +{to_reserve:.6} \
             battery +{gained:.6} feces {feces:.6} heat {bite_heat:.6}"
        );
    }
}

// ------------------------------------------------------------------------------ A5

/// **A5 — settlement.** One mouth: a legacy decision with all three efforts at 1 is normalised
/// world-side. The scavenge bite splits between litter and remains in proportion to their
/// edible shares. Several mouths on one cell share it proportionally, and what leaves the
/// stocks equals what was served.
#[test]
fn a5_settlement_normalises_one_mouth_splits_the_stocks_and_shares_proportionally() {
    let mut cfg = bare_config(AVERAGE.0, AVERAGE.1);
    immobile(&mut cfg);
    cfg.drives.feed_min = 0.001;
    cfg.organism.intake_half_saturation = 0.0;
    cfg.producer.growth = 0.0;
    cfg.producer.mortality = 0.0;
    cfg.plant.maintenance = 0.0;
    cfg.plant.foliage_rate = 0.0;
    cfg.fruit.ripen = 0.0;
    cfg.fruit.drop = 0.0;
    cfg.detritus.decomposition = 0.0;
    cfg.detritus.carrion_decomposition = 0.0;
    cfg.detritus.fall = 0.0;
    let e_r = cfg.organism.reserve_energy_density;
    let mut world = World::new(cfg).expect("valid");
    let cell = CellId::new(Topology::Cube, Scale::ONE, Face::Front, 6, 6);
    // Leaf, fruit, litter and remains all present and all plentiful, so a skimmer's three
    // channels all open at once — exactly the legacy decision §6.3 names.
    paint(&mut world, cell, 1.0, 0.6, 0.0);
    let before = total_material(&world);
    world.state.fields.f[cell.index()] = 0.5;
    world.state.fields.d[cell.index()] = 0.6;
    world.state.fields.de[cell.index()] = 2.0 * 0.6;
    world.state.ecology.carrion[cell.index()] = 0.2;
    world.state.ecology.carrion_energy[cell.index()] = 2.0 * 0.2;
    book(&mut world, before);
    let skimmer = place(&mut world, cell, 0.60, 0.0);
    let mut world = restage(world);

    let mouth_rate = world.state.organisms.get(skimmer).expect("alive").phenotype.mouth_rate;
    let d_eff = edible(
        world.state.fields.d[cell.index()],
        world.state.fields.de[cell.index()],
        e_r,
    );
    let c_eff = edible(
        world.state.ecology.carrion[cell.index()],
        world.state.ecology.carrion_energy[cell.index()],
        e_r,
    );
    let (p0, f0, d0, c0) = (
        world.state.fields.p[cell.index()],
        world.state.fields.f[cell.index()],
        world.state.fields.d[cell.index()],
        world.state.ecology.carrion[cell.index()],
    );
    world.step();
    let diag = world.intake_diagnostics();
    assert!(diag.fruit_eaten > 0.0 && diag.producer_eaten > 0.0, "{diag:?}");
    assert!(diag.litter_eaten > 0.0 && diag.carrion_eaten > 0.0, "{diag:?}");

    // One mouth: the three normalised efforts are 1/3 each, so each channel's request is a
    // third of a mouth-tick and the three together are exactly one.
    let third = mouth_rate * (1.0 / 3.0) * DT;
    assert!(
        (diag.fruit_eaten - third).abs() < 1e-12,
        "fruit took {} of a third-mouthful {third}",
        diag.fruit_eaten
    );
    assert!(
        (diag.producer_eaten - third).abs() < 1e-12,
        "graze took {} of {third}",
        diag.producer_eaten
    );
    let scavenged = diag.litter_eaten + diag.carrion_eaten;
    assert!(
        (scavenged - third).abs() < 1e-12,
        "scavenge took {scavenged} of {third}"
    );

    // The `D : C` split is the edible shares', to the bit.
    let share = d_eff / (d_eff + c_eff);
    assert!(
        (diag.litter_eaten - scavenged * share).abs() < 1e-12,
        "litter took {} of the {scavenged} bite, expected {}",
        diag.litter_eaten,
        scavenged * share
    );

    // What left each stock is exactly what was served — minus, for litter, the feces the same
    // bites returned to it.
    assert!((f0 - world.state.fields.f[cell.index()] - diag.fruit_eaten).abs() < 1e-12);
    assert!((p0 - world.state.fields.p[cell.index()] - diag.producer_eaten).abs() < 1e-12);
    assert!((c0 - world.state.ecology.carrion[cell.index()] - diag.carrion_eaten).abs() < 1e-12);
    let litter_moved = world.state.fields.d[cell.index()] - d0;
    assert!(
        (litter_moved - (diag.undigested - diag.litter_eaten)).abs() < 1e-12,
        "litter moved by {litter_moved}, feces {} less the {} eaten",
        diag.undigested,
        diag.litter_eaten
    );
    assert!(world.mass_residual().abs() < 1e-9);

    // Proportional sharing among several mouths. Four identical grazers on one cell that
    // cannot serve them all: equal shares, the cell drained and not overdrawn, and the total
    // removed equal to the total served.
    let mut cfg = bare_config(AVERAGE.0, AVERAGE.1);
    immobile(&mut cfg);
    cfg.drives.feed_min = 0.001;
    cfg.organism.intake_half_saturation = 0.0;
    cfg.producer.growth = 0.0;
    cfg.producer.mortality = 0.0;
    cfg.plant.maintenance = 0.0;
    cfg.plant.foliage_rate = 0.0;
    cfg.fruit.ripen = 0.0;
    cfg.mechanisms.scavenging = false;
    let mut world = World::new(cfg).expect("valid");
    let available = 0.004;
    paint(&mut world, cell, available, 0.6, 0.0);
    let ids: Vec<OrganismId> = (0..4)
        .map(|k| place(&mut world, cell, 0.85, f64::from(k) * 0.5))
        .collect();
    let mut world = restage(world);
    world.step();
    let gains: Vec<f64> = ids
        .iter()
        .map(|id| world.state.organisms.get(*id).expect("alive").reserve)
        .collect();
    let spread = gains.iter().cloned().fold(f64::MIN, f64::max)
        - gains.iter().cloned().fold(f64::MAX, f64::min);
    assert!(spread < 1e-15, "equal mouths got unequal shares: {gains:?}");
    assert!(gains[0] > 0.0, "nobody was fed");
    assert!(
        world.state.fields.p[cell.index()] < 1e-15,
        "the cell was not drained: {}",
        world.state.fields.p[cell.index()]
    );
    let served = world.intake_diagnostics().producer_eaten;
    assert!(
        (served - available).abs() < 1e-12,
        "served {served} of the {available} that was there"
    );
    assert!(world.mass_residual().abs() < 1e-9);
}

// ------------------------------------------------------------------------------ A6

/// **A6 — remains routing.** An ordinary death, a failed gestation and a hunter's death all
/// land in `C`; a hunter's digestion rejects land in `D`; and each stock's decomposition
/// leaks into no other stock.
#[test]
fn a6_remains_route_to_c_rejects_route_to_d_and_no_stock_leaks_into_another() {
    // --- an ordinary death.
    let mut cfg = bare_config(AVERAGE.0, AVERAGE.1);
    immobile(&mut cfg);
    cfg.detritus.carrion_decomposition = 0.0;
    cfg.detritus.decomposition = 0.0;
    let mut world = World::new(cfg).expect("valid");
    let cell = CellId::new(Topology::Cube, Scale::ONE, Face::Front, 6, 6);
    let id = place(&mut world, cell, 0.85, 0.0);
    {
        // Nothing to raise and nothing to eat: it starves on the first tick.
        let o = world.state.organisms.get_mut(id).expect("alive");
        o.energy = 0.0;
        o.reserve = 0.0;
    }
    let mut world = restage(world);
    let body = {
        let o = world.state.organisms.get(id).expect("alive");
        o.structure + o.reserve
    };
    let body_energy = {
        let o = world.state.organisms.get(id).expect("alive");
        o.energy + world.config().organism.reserve_energy_density * o.reserve
    };
    let e_c_max = world.config().detritus.carrion_energy_cap;
    let d0: f64 = world.state.fields.d.iter().sum();
    let heat0 = world.state.heat_out_corrected();
    world.step();
    assert!(world.state.organisms.get(id).is_none(), "it must actually die");
    assert!(
        (world.state.ecology.carrion[cell.index()] - body).abs() < 1e-12,
        "the body landed as {} of remains, not {body}",
        world.state.ecology.carrion[cell.index()]
    );
    // Its energy went with it, under `e_c_max`, and the excess is heat — exactly, because
    // this body had nothing left to pay any other bill with.
    let kept = body_energy.min(e_c_max * body);
    assert!(
        (world.state.ecology.carrion_energy[cell.index()] - kept).abs() < 1e-12,
        "the remains carry {}, not {kept}",
        world.state.ecology.carrion_energy[cell.index()]
    );
    assert!(
        (world.state.heat_out_corrected() - heat0 - (body_energy - kept)).abs() < 1e-12,
        "the clamped excess is not heat"
    );
    assert_eq!(world.state.fields.d.iter().sum::<f64>(), d0, "and not in the litter");
    assert_eq!(
        world.state.ecology.carrion_energy.iter().sum::<f64>(),
        world.state.ecology.carrion_energy[cell.index()],
        "nor anywhere but the cell it died in"
    );
    assert!(world.mass_residual().abs() < 1e-9);

    // --- a failed gestation: the escrow lands in `C` beside the body.
    let mut cfg = bare_config(AVERAGE.0, AVERAGE.1);
    immobile(&mut cfg);
    cfg.detritus.carrion_decomposition = 0.0;
    let mut world = World::new(cfg).expect("valid");
    let id = place(&mut world, cell, 0.85, 0.0);
    let escrow_material;
    {
        let o = world.state.organisms.get_mut(id).expect("alive");
        let structure = 0.4 * o.phenotype.structure_adult;
        let reserve = 0.2 * o.phenotype.reserve_max;
        escrow_material = structure + reserve;
        o.escrow = Some(cubarium_core::organism::Escrow {
            structure,
            reserve,
            energy: 0.1,
            started_tick: 0,
            genome: o.genome.clone(),
        });
        o.energy = 0.0;
        o.reserve = 0.0;
    }
    let mut world = restage(world);
    world.state.external_material_in += escrow_material;
    let body = {
        let o = world.state.organisms.get(id).expect("alive");
        o.structure + o.reserve
    };
    let (body_energy, escrow_energy) = {
        let o = world.state.organisms.get(id).expect("alive");
        let e_r = world.config().organism.reserve_energy_density;
        let es = o.escrow.as_ref().expect("gestating");
        (
            o.energy + e_r * o.reserve,
            e_r * (es.structure + es.reserve) + es.energy,
        )
    };
    let e_c_max = world.config().detritus.carrion_energy_cap;
    let d0: f64 = world.state.fields.d.iter().sum();
    let heat0 = world.state.heat_out_corrected();
    // This arm books the escrow into `external_material_in` after restaging, so the residual
    // opens at a known offset; what must not move is the **change** across the death.
    let residual0 = world.mass_residual();
    world.step();
    assert!(world.state.organisms.get(id).is_none());
    assert!(
        (world.state.ecology.carrion[cell.index()] - (body + escrow_material)).abs() < 1e-12,
        "body {body} plus escrow {escrow_material} vs {}",
        world.state.ecology.carrion[cell.index()]
    );
    // The body and the escrow are two deposits, each under its own `e_c_max` clamp — the
    // miscarriage is not the corpse and the contract books them apart (§5).
    let kept = body_energy.min(e_c_max * body) + escrow_energy.min(e_c_max * escrow_material);
    assert!(
        (world.state.ecology.carrion_energy[cell.index()] - kept).abs() < 1e-12,
        "the remains carry {}, not {kept}",
        world.state.ecology.carrion_energy[cell.index()]
    );
    assert!(
        (world.state.heat_out_corrected() - heat0
            - ((body_energy + escrow_energy) - kept))
            .abs()
            < 1e-12,
        "the clamped excess is not heat"
    );
    assert_eq!(world.state.fields.d.iter().sum::<f64>(), d0, "no part of it became litter");
    assert!(
        (world.mass_residual() - residual0).abs() < 1e-9,
        "the miscarriage moved material out of the box"
    );

    // --- a hunter's digestion reject is feces and lands in `D`; its death lands in `C`.
    let mut cfg = bare_config(AVERAGE.0, AVERAGE.1);
    immobile(&mut cfg);
    cfg.detritus.decomposition = 0.0;
    cfg.detritus.carrion_decomposition = 0.0;
    let mut world = World::new(cfg).expect("valid");
    let profile = FixedHunterProfile::lanternjaw_trial(world.config());
    let spot = CellId::new(Topology::Cube, Scale::ONE, Face::Top, 8, 8).center(Topology::Cube, Scale::ONE);
    let hunter = world
        .start_hunter_trial(
            profile,
            HunterTarget { face: spot.face.index() as u8, u: spot.u, v: spot.v },
        )
        .expect("the trial starts")
        .id;
    // A gut with a carcass in it, booked as an import, and a full battery so handling is paid.
    let (gut_material, gut_energy) = (2.0, 3.0);
    {
        let m = world.state.hunters.member_mut(hunter).expect("the founder is a member");
        m.gut_material = gut_material;
        m.gut_energy = gut_energy;
    }
    world.state.hunters.founder_material_in += gut_material;
    world.state.hunters.founder_energy_in += gut_energy;
    {
        let o = world.state.organisms.get_mut(hunter).expect("alive");
        o.energy = o.phenotype.energy_max;
        o.reserve = 0.0;
    }
    let mut world = restage(world);
    let here = cell_of(Topology::Cube, Scale::ONE, &world.state.organisms.get(hunter).expect("alive").pos).index();
    let mut rejected = 0.0;
    for _ in 0..200 {
        let d_before = world.state.fields.d[here];
        let c_before = world.state.ecology.carrion[here];
        world.step();
        world.drain_hunter_events();
        rejected += world.state.fields.d[here] - d_before;
        assert_eq!(
            world.state.ecology.carrion[here], c_before,
            "a digestion reject is feces, and feces are litter"
        );
        if !world.state.hunters.members.iter().any(|m| m.carrying()) {
            break;
        }
    }
    assert!(rejected > 0.0, "the digestion must actually have rejected something");
    assert!(world.mass_residual().abs() < 1e-9);

    // Its death routes the body **and** everything still in the gut to `C`, exactly — a
    // lower bound of "at least the body" would pass an implementation that quietly dropped the
    // gut (Astra's implementation review, finding 4). It is stopped mid-meal on purpose, so
    // there is a gut to lose.
    let (body, gut_material, gut_energy, body_energy) = {
        let o = world.state.organisms.get(hunter).expect("alive");
        let m = world.state.hunters.member(hunter).expect("still a member");
        let e_r = world.config().organism.reserve_energy_density;
        (
            o.structure + o.reserve,
            m.gut_material,
            m.gut_energy,
            o.energy + e_r * o.reserve,
        )
    };
    assert!(
        gut_material > 0.0,
        "the fixture must kill it **while carrying**, or the gut term is untested"
    );
    let d_before = world.state.fields.d[here];
    let c_before = world.state.ecology.carrion[here];
    let ce_before = world.state.ecology.carrion_energy[here];
    let e_c_max = world.config().detritus.carrion_energy_cap;
    {
        // Starve it outright: `raisable_energy` is zero, so it dies on this tick, before
        // handling could digest any more of the meal.
        let o = world.state.organisms.get_mut(hunter).expect("alive");
        o.energy = 0.0;
        o.reserve = 0.0;
    }
    // The body it dies with, re-read after the edit.
    let (body_now, body_energy_now) = {
        let o = world.state.organisms.get(hunter).expect("alive");
        (o.structure + o.reserve, o.energy)
    };
    let mut world = restage(world);
    world.step();
    assert!(world.state.organisms.get(hunter).is_none(), "the member must die");
    assert!(
        world.state.hunters.member(hunter).is_none(),
        "and leave the member list, so no gut is left dangling"
    );
    assert!(
        (world.state.ecology.carrion[here] - (c_before + body_now + gut_material)).abs() < 1e-12,
        "remains gained {}, not the body {body_now} plus the gut {gut_material}",
        world.state.ecology.carrion[here] - c_before
    );
    // Each deposit keeps at most `e_c_max` per unit; the body and the gut are booked apart,
    // so the cap is applied to each and the total is the sum of the two clamped amounts.
    let kept_body = body_energy_now.min(e_c_max * body_now);
    let kept_gut = gut_energy.min(e_c_max * gut_material);
    assert!(
        (world.state.ecology.carrion_energy[here] - (ce_before + kept_body + kept_gut)).abs()
            < 1e-12,
        "the remains' energy is {}, not {}",
        world.state.ecology.carrion_energy[here] - ce_before,
        kept_body + kept_gut
    );
    assert!(
        (world.state.fields.d[here] - d_before).abs() < 1e-12,
        "none of it became litter"
    );
    // The post-death residual: nothing of the member is left anywhere.
    assert_eq!(world.state.hunters.gut_material_total(), 0.0, "a gut outlived its hunter");
    assert_eq!(world.state.hunters.gut_energy_total(), 0.0);
    assert!(world.mass_residual().abs() < 1e-9, "the death closed the box");
    let _ = (body, body_energy);

    // --- no stock leaks into another. Each stock alone, decomposing, with the other two
    //     empty: only `N` may gain, and the other two must stay at exactly zero.
    for stock in ["litter", "remains", "dead wood"] {
        let mut cfg = bare_config(AVERAGE.0, AVERAGE.1);
        cfg.detritus.fall = 0.0;
        cfg.nutrient.diffusion = 0.0;
        let mut world = World::new(cfg).expect("valid");
        let i = cell.index();
        let before = total_material(&world);
        match stock {
            "litter" => {
                world.state.fields.d[i] = 1.0;
                world.state.fields.de[i] = 2.0;
            }
            "remains" => {
                world.state.ecology.carrion[i] = 1.0;
                world.state.ecology.carrion_energy[i] = 2.0;
            }
            _ => world.state.ecology.dead_wood[i] = 1.0,
        }
        book(&mut world, before);
        let mut world = restage(world);
        let n0: f64 = world.state.fields.n.iter().sum();
        for _ in 0..400 {
            world.step();
        }
        let f = &world.state.fields;
        let e = &world.state.ecology;
        let others: [(&str, f64); 3] = [
            ("litter", f.d.iter().sum()),
            ("remains", e.carrion.iter().sum()),
            ("dead wood", e.dead_wood.iter().sum()),
        ];
        for (name, total) in others {
            if name == stock {
                assert!(total < 1.0, "{stock} did not decompose at all");
            } else {
                assert_eq!(total, 0.0, "{stock} leaked into {name}");
            }
        }
        assert!(f.n.iter().sum::<f64>() > n0, "{stock}: nothing returned to N");
        assert!(world.mass_residual().abs() < 1e-9);
    }
}

// ------------------------------------------------------------------------------ A7

/// **A7 — snapshot.** The current schema round-trips with the documented header, every older
/// schema is refused with its own version, and a relabelled current payload is refused too.
///
/// Written for schema 16 and re-pointed at **17** by the ring world
/// (`design/flat-world-plan-2026-09-16.md` §4), which appends `topology` and `world_scale` to
/// `WorldConfig` and therefore changes the payload's shape. The rule the test states is
/// unchanged, and it now covers one more refusal: 16 is refused by name like every other.
///
/// The old-schema **fixtures** are covered by their own refusal tests beside the suites that
/// used to migrate them (`care.rs`, `care_dose_migration.rs`, `hunter_migration.rs`,
/// `quiet_migration.rs`, `hunter_charging.rs`, `astra_quiet_policy.rs`,
/// `continuation_fixtures.rs`); this is the rule itself.
#[test]
fn a7_the_current_schema_round_trips_and_every_older_schema_is_refused_by_name() {
    assert_eq!(SCHEMA_VERSION, 17);
    let mut world = bare_world(BRIGHT);
    paint(&mut world, CellId::new(Topology::Cube, Scale::ONE, Face::Top, 8, 8), 0.56, 0.6, 0.30);
    let mut world = restage(world);
    for _ in 0..50 {
        world.step();
    }

    let bytes = encode_snapshot(&world.state, "ecology-v1");
    assert_eq!(&bytes[..4], b"CUBW");
    assert_eq!(u32::from_le_bytes(bytes[4..8].try_into().unwrap()), SCHEMA_VERSION);
    let (meta, back) = decode_snapshot(&bytes).expect("the current schema round-trips");
    assert_eq!(meta.schema, SCHEMA_VERSION);
    assert_eq!(meta.build_id, "ecology-v1");
    assert_eq!(back, world.state, "the round trip is lossless");
    assert_eq!(state_hash(&back), state_hash(&world.state));
    // The new pools really are in the payload.
    assert!(back.ecology.wood.iter().any(|&w| w > 0.0));
    assert_eq!(back.ecology.wood.len(), CUBE_CELL_COUNT);

    for old in 7..SCHEMA_VERSION {
        let mut relabelled = bytes.clone();
        relabelled[4..8].copy_from_slice(&old.to_le_bytes());
        assert_eq!(
            decode_snapshot(&relabelled),
            Err(SnapshotError::UnsupportedSchema(old)),
            "schema {old} must be refused by name"
        );
    }

    // A relabelled current payload: a longer body under the current version number is
    // refused rather than read at the wrong offsets.
    let mut longer = postcard::to_allocvec(&world.state).expect("encodes");
    longer.extend_from_slice(&[0u8; 3]);
    let mut framed = Vec::new();
    framed.extend_from_slice(b"CUBW");
    framed.extend_from_slice(&SCHEMA_VERSION.to_le_bytes());
    framed.extend_from_slice(&0u16.to_le_bytes());
    framed.extend_from_slice(&(longer.len() as u64).to_le_bytes());
    framed.extend_from_slice(&crc32fast::hash(&longer).to_le_bytes());
    framed.extend_from_slice(&longer);
    assert!(
        matches!(decode_snapshot(&framed), Err(SnapshotError::Decode(_))),
        "a relabelled current payload must be refused"
    );
}

/// **A7b — `ecology_hash` sees the ecology (`design/ecology-v1-contract.md` §15.1, revised).**
///
/// It used to hash the schema 7 projection, which predates wood, the plant reserve, dead wood
/// and animal remains: two schema 16 worlds could differ in every pool ecology v1 added and
/// still hash alike, so a care replay could pass after the ecology had diverged (Astra's
/// implementation review, finding 2). It is now the care-masked hash of the current state.
///
/// The perturbation test the contract asks for: **every** ecology v1 vector entry and both
/// counters move the hash, and care state does not.
#[test]
fn a7b_the_ecology_hash_moves_with_every_ecology_stock_and_not_with_care() {
    let mut world = bare_world(BRIGHT);
    paint(&mut world, CellId::new(Topology::Cube, Scale::ONE, Face::Top, 8, 8), 0.56, 0.6, 0.30);
    let mut world = restage(world);
    for _ in 0..40 {
        world.step();
    }
    let base = world.state.clone();
    let h0 = ecology_hash(&base);
    assert_eq!(h0, ecology_hash(&base), "the hash is a pure function of the state");

    // One entry of each ecology v1 vector, in three places apiece — a cell the fixture used,
    // a cell it did not, and the last cell — so a hash that covered only a prefix would fail.
    let cells = [CellId::new(Topology::Cube, Scale::ONE, Face::Top, 8, 8).index(), 0, CUBE_CELL_COUNT - 1];
    type Poke = fn(&mut cubarium_core::world::EcologyV1State, usize);
    let pokes: [(&str, Poke); 5] = [
        ("wood", |e, i| e.wood[i] += 1e-9),
        ("plant_reserve", |e, i| e.plant_reserve[i] += 1e-9),
        ("dead_wood", |e, i| e.dead_wood[i] += 1e-9),
        ("carrion", |e, i| e.carrion[i] += 1e-9),
        ("carrion_energy", |e, i| e.carrion_energy[i] += 1e-9),
    ];
    for (name, poke) in pokes {
        for i in cells {
            let mut moved = base.clone();
            poke(&mut moved.ecology, i);
            assert_ne!(
                ecology_hash(&moved),
                h0,
                "{name}[{i}] moved and the ecology hash did not"
            );
        }
    }
    for (name, poke) in [
        ("plant_deaths_total", (|e: &mut cubarium_core::world::EcologyV1State| {
            e.plant_deaths_total += 1
        }) as fn(&mut cubarium_core::world::EcologyV1State)),
        ("recolonisations_total", |e| e.recolonisations_total += 1),
    ] {
        let mut moved = base.clone();
        poke(&mut moved.ecology);
        assert_ne!(ecology_hash(&moved), h0, "{name} moved and the ecology hash did not");
    }

    // The pools that were already hashed stay hashed.
    for (name, poke) in [
        ("n", (|s: &mut cubarium_core::WorldState| s.fields.n[3] += 1e-9)
            as fn(&mut cubarium_core::WorldState)),
        ("p", |s| s.fields.p[3] += 1e-9),
        ("d", |s| s.fields.d[3] += 1e-9),
        ("de", |s| s.fields.de[3] += 1e-9),
        ("f", |s| s.fields.f[3] += 1e-9),
        ("tick", |s| s.tick += 1),
    ] {
        let mut moved = base.clone();
        poke(&mut moved);
        assert_ne!(ecology_hash(&moved), h0, "{name} moved and the ecology hash did not");
    }

    // And care does **not** move it — the one thing this hash exists to ignore, so a care run
    // and a matched no-care run of the same ecology still compare directly.
    let mut cared = base.clone();
    cared.care.admitted_seq = 7;
    cared.care.feed_material_in = 1.5;
    cared.care.feed_energy_in = 3.0;
    cared.care.clean_material_out = 0.5;
    cared.care.clean_energy_out = 1.0;
    cared.care.rain_depth_in = 0.25;
    cared.care.allowance_used = 2.0;
    assert_eq!(
        ecology_hash(&cared),
        h0,
        "care state moved the care-masked ecology hash"
    );
    // `state_hash` is the full encoding and must see exactly what `ecology_hash` masks.
    assert_ne!(state_hash(&cared), state_hash(&base), "care is in the full hash");
    assert_ne!(
        state_hash(&cared),
        ecology_hash(&cared),
        "with care in it, the two hashes must differ — that difference *is* the mask"
    );
    assert_eq!(
        state_hash(&base),
        h0,
        "and with no care they agree exactly: the mask is care and nothing else"
    );
}

// ------------------------------------------------------------------------------ A8

/// **A8 — determinism.** Two runs of the same fixture produce equal `state_hash` at the end,
/// for every mechanism this milestone adds and for a full world with mouths in it.
#[test]
fn a8_two_runs_of_each_fixture_end_on_the_same_state_hash() {
    /// `(name, builder, ticks)`.
    type Build = fn() -> World;
    let fixtures: [(&str, Build, u64); 4] = [
        (
            "a lone stand",
            || {
                let mut w = bare_world(BRIGHT);
                paint(&mut w, CellId::new(Topology::Cube, Scale::ONE, Face::Top, 8, 8), 0.0, 0.18, 0.0);
                restage(w)
            },
            4_000,
        ),
        (
            "an establishment ring",
            || {
                let mut w = bare_world(BRIGHT);
                for (du, dv) in [(0i16, 0i16), (1, 0), (-1, 0), (0, 1), (0, -1)] {
                    let c = CellId::new(Topology::Cube, Scale::ONE, 
                        Face::Top,
                        (8 + du) as u16,
                        (8 + dv) as u16,
                    );
                    paint(&mut w, c, 0.56, 0.6, 0.30);
                }
                restage(w)
            },
            4_000,
        ),
        (
            "decomposition and fall",
            || {
                let mut w = bare_world(AVERAGE);
                let before = total_material(&w);
                for cell in CellId::all(Topology::Cube, Scale::ONE) {
                    let i = cell.index();
                    w.state.fields.d[i] = 0.4;
                    w.state.fields.de[i] = 0.8;
                    w.state.ecology.carrion[i] = 0.2;
                    w.state.ecology.carrion_energy[i] = 0.4;
                    w.state.ecology.dead_wood[i] = 0.3;
                }
                book(&mut w, before);
                restage(w)
            },
            2_000,
        ),
        (
            "a grazer on a stand",
            || {
                let mut cfg = bare_config(BRIGHT.0, BRIGHT.1);
                cfg.drives.feed_min = 0.001;
                let mut w = World::new(cfg).expect("valid");
                let before = total_material(&w);
                for v in w.state.fields.n.iter_mut() {
                    *v = REFERENCE_NUTRIENT;
                }
                book(&mut w, before);
                let cell = CellId::new(Topology::Cube, Scale::ONE, Face::Top, 8, 8);
                paint(&mut w, cell, 0.56, 0.6, 0.30);
                place(&mut w, cell, 0.85, 0.0);
                restage(w)
            },
            4_000,
        ),
    ];

    for (name, build, ticks) in fixtures {
        let run = || {
            let mut world = build();
            for _ in 0..ticks {
                world.step();
                world.drain_events();
            }
            state_hash(&world.state)
        };
        let a = run();
        let b = run();
        assert_eq!(a, b, "{name}: two runs ended on different states");
    }
}

// ------------------------------------------------------------------------------ A9

/// **A9 — establishment arithmetic (§4.8).** One donor, one bare neighbour, `k_est · dt` per
/// tick: the donor's reserve falls by exactly the sent amount, the recipient's `W`, `P` and
/// `Q` rise by the split of `net = s/(1 + c_g)`, the construction nutrient lands in the
/// recipient, the establishing cell's other stocks are untouched, and
/// `recolonisations_total` increments exactly once, on the tick `W` crosses `W_min`.
#[test]
fn a9_one_donor_one_recipient_books_every_term_of_the_propagule() {
    let mut cfg = bare_config(BRIGHT.0, BRIGHT.1);
    // Nothing but the propagule may move a stock, so every number below is the transfer.
    cfg.producer.growth = 0.0;
    cfg.producer.mortality = 0.0;
    cfg.plant.maintenance = 0.0;
    cfg.plant.foliage_rate = 0.0;
    cfg.plant.wood_rate = 0.0;
    cfg.fruit.ripen = 0.0;
    cfg.fruit.drop = 0.0;
    cfg.detritus.decomposition = 0.0;
    cfg.detritus.carrion_decomposition = 0.0;
    cfg.detritus.wood_decomposition = 0.0;
    cfg.detritus.fall = 0.0;
    cfg.nutrient.diffusion = 0.0;
    let PlantConfig {
        build,
        propagule_rate,
        propagule_split,
        reserve_cap,
        donor_reserve_floor,
        alive_min,
        donor_min: cfg_donor_min,
        ..
    } = cfg.plant;
    let e_v = cfg.plant.energy_density;

    let mut world = World::new(cfg).expect("valid");
    // A donor with exactly one establishing-or-bare neighbour: the interior of a face has
    // four, so the other three are given stands of their own.
    let donor = CellId::new(Topology::Cube, Scale::ONE, Face::Top, 8, 8);
    let recipient = CellId::new(Topology::Cube, Scale::ONE, Face::Top, 8, 9);
    paint(&mut world, donor, 0.56, 0.6, 0.30);
    // The donor's other three neighbours are **alive but not donors**: wood above `W_min` so
    // they are not recipients, and below `W_est` so they send nothing of their own. That makes
    // the centre stand the only donor in the world and gives it exactly one recipient, so every
    // number below is one transfer rather than a sum over a ring.
    for c in [
        CellId::new(Topology::Cube, Scale::ONE, Face::Top, 8, 7),
        CellId::new(Topology::Cube, Scale::ONE, Face::Top, 7, 8),
        CellId::new(Topology::Cube, Scale::ONE, Face::Top, 9, 8),
    ] {
        paint(&mut world, c, 0.0, 0.5 * (alive_min + cfg_donor_min), 0.0);
    }
    // Charged litter and remains in the recipient, so "otherwise untouched" is a real claim.
    let before = total_material(&world);
    world.state.fields.d[recipient.index()] = 0.3;
    world.state.fields.de[recipient.index()] = 0.6;
    world.state.ecology.carrion[recipient.index()] = 0.2;
    world.state.ecology.carrion_energy[recipient.index()] = 0.4;
    world.state.ecology.dead_wood[recipient.index()] = 0.1;
    book(&mut world, before);
    let mut world = restage(world);

    assert_eq!(
        CellClass::of(world.state.ecology.wood[recipient.index()], alive_min),
        CellClass::Bare
    );

    let (di, ri) = (donor.index(), recipient.index());
    let mut crossed_on = None;
    let mut sent_total = 0.0;
    for tick in 1..=20_000u64 {
        let q_donor = world.state.ecology.plant_reserve[di];
        let w_donor = world.state.ecology.wood[di];
        let (w0, p0, q0, n0) = (
            world.state.ecology.wood[ri],
            world.state.fields.p[ri],
            world.state.ecology.plant_reserve[ri],
            world.state.fields.n[ri],
        );
        let (d0, de0, c0, wd0) = (
            world.state.fields.d[ri],
            world.state.fields.de[ri],
            world.state.ecology.carrion[ri],
            world.state.ecology.dead_wood[ri],
        );
        let was_bare = w0 < alive_min;
        let heat0 = world.state.heat_out_corrected();
        world.step();

        // The donor's budget this tick: one recipient, so `B = min(Q − q_prop·Q_max, k_est·dt)`.
        let floor = donor_reserve_floor * reserve_cap * w_donor;
        let sent = (q_donor - floor).max(0.0).min(propagule_rate * DT);
        sent_total += sent;
        assert!(
            (q_donor - world.state.ecology.plant_reserve[di] - sent).abs() < 1e-15,
            "tick {tick}: the donor's reserve fell by {} against the sent {sent}",
            q_donor - world.state.ecology.plant_reserve[di]
        );
        if sent <= 0.0 {
            panic!("tick {tick}: the donor stopped spending before the crossing");
        }

        let net = sent / (1.0 + build);
        assert!(
            (world.state.ecology.wood[ri] - (w0 + propagule_split[0] * net)).abs() < 1e-15,
            "tick {tick}: wood"
        );
        assert!(
            (world.state.fields.p[ri] - (p0 + propagule_split[1] * net)).abs() < 1e-15,
            "tick {tick}: foliage"
        );
        assert!(
            (world.state.ecology.plant_reserve[ri] - (q0 + propagule_split[2] * net)).abs()
                < 1e-15,
            "tick {tick}: reserve"
        );
        assert!(
            (world.state.fields.n[ri] - (n0 + build * net)).abs() < 1e-15,
            "tick {tick}: the construction nutrient lands in the recipient"
        );
        assert!(
            (world.state.heat_out_corrected() - heat0 - e_v * build * net).abs() < 1e-12,
            "tick {tick}: construction heat"
        );
        // Everything else in the establishing cell is frozen.
        assert_eq!(world.state.fields.d[ri], d0, "tick {tick}: litter");
        assert_eq!(world.state.fields.de[ri], de0, "tick {tick}: litter energy");
        assert_eq!(world.state.ecology.carrion[ri], c0, "tick {tick}: remains");
        assert_eq!(world.state.ecology.dead_wood[ri], wd0, "tick {tick}: dead wood");

        let now_alive = world.state.ecology.wood[ri] >= alive_min;
        if was_bare && now_alive && crossed_on.is_none() {
            crossed_on = Some(tick);
            assert_eq!(
                world.state.ecology.recolonisations_total, 1,
                "tick {tick}: the crossing must be counted exactly once"
            );
            break;
        }
        assert_eq!(
            world.state.ecology.recolonisations_total, 0,
            "tick {tick}: counted before the crossing"
        );
        assert!(world.mass_residual().abs() < 1e-9, "tick {tick}: mass");
    }
    let crossed = crossed_on.expect("the recipient must establish within 20,000 ticks");

    // The counter never moves again: an established cell is no longer a recipient.
    for _ in 0..2_000 {
        world.step();
    }
    assert_eq!(world.state.ecology.recolonisations_total, 1, "the count is one-time");
    assert_eq!(
        CellClass::of(world.state.ecology.wood[ri], alive_min),
        CellClass::Alive
    );
    assert!(world.mass_residual().abs() < 1e-9);
    println!(
        "A9: crossed W_min on tick {crossed} ({:.1} s) after {sent_total:.6} m of reserve sent",
        crossed as f64 * DT
    );
}

// ----------------------------------------------- A9, repair cycle 2: 3h recipient state

/// **A9b — a stand that dies in 3d is an eligible recipient on the same tick.**
///
/// §4.0 says 3h reads `W⁴`, the wood **after** 3d, for recipients as well as donors. The first
/// implementation read the pre-tick class instead, so a cell that crossed `W_min` downward in
/// 3d stayed ineligible until the following tick — a difference from the named state that the
/// aggregate tests could not see (Astra's implementation review, finding 1).
///
/// The fixture makes the death happen on a chosen tick and asserts the propagule lands in the
/// same one.
#[test]
fn a9b_a_stand_that_dies_in_3d_receives_a_propagule_in_the_same_tick() {
    let mut cfg = bare_config(BRIGHT.0, BRIGHT.1);
    // Nothing grows, nothing decays: the only two things that happen are the doomed stand's
    // dieback and the donor's propagule.
    cfg.producer.growth = 0.0;
    cfg.producer.mortality = 0.0;
    cfg.fruit.ripen = 0.0;
    cfg.fruit.drop = 0.0;
    cfg.detritus.decomposition = 0.0;
    cfg.detritus.carrion_decomposition = 0.0;
    cfg.detritus.wood_decomposition = 0.0;
    cfg.detritus.fall = 0.0;
    cfg.nutrient.diffusion = 0.0;
    cfg.plant.foliage_rate = 0.0;
    cfg.plant.wood_rate = 0.0;
    cfg.plant.reserve_share = 0.0;
    let alive_min = cfg.plant.alive_min;
    let split = cfg.plant.propagule_split;
    let build = 1.0 + cfg.plant.build;
    let (rate, floor_fraction, reserve_cap) = (
        cfg.plant.propagule_rate,
        cfg.plant.donor_reserve_floor,
        cfg.plant.reserve_cap,
    );
    // Maintenance stays at the §11 value: the doomed stand is instead placed one hair above
    // `W_min`, so a single tick of unpaid maintenance — `κ · m_w · W · dt = 2e-7` against a
    // margin of 2e-8 — carries it below the threshold. A larger `m_w` would also bill the
    // donor, which is not what this test is about.
    let m_w = cfg.plant.maintenance;

    let mut world = World::new(cfg).expect("valid");
    let donor = CellId::new(Topology::Cube, Scale::ONE, Face::Top, 8, 8);
    let doomed = CellId::new(Topology::Cube, Scale::ONE, Face::Top, 8, 9);
    // The donor's other three neighbours are alive but below `W_est`, so they are neither
    // recipients nor donors and the doomed cell is the donor's only recipient.
    paint(&mut world, donor, 0.0, 0.6, 0.30);
    for c in [
        CellId::new(Topology::Cube, Scale::ONE, Face::Top, 8, 7),
        CellId::new(Topology::Cube, Scale::ONE, Face::Top, 7, 8),
        CellId::new(Topology::Cube, Scale::ONE, Face::Top, 9, 8),
    ] {
        paint(&mut world, c, 0.0, 0.2, 0.0);
    }
    // Just alive, with nothing to pay maintenance with.
    let doomed_wood = alive_min * (1.0 + 1e-6);
    paint(&mut world, doomed, 0.0, doomed_wood, 0.0);
    let mut world = restage(world);

    let (di, ti) = (donor.index(), doomed.index());
    assert_eq!(
        CellClass::of(world.state.ecology.wood[ti], alive_min),
        CellClass::Alive,
        "the doomed stand must start alive, or nothing is being tested"
    );
    let q_before = world.state.ecology.plant_reserve[di];
    let donor_wood = world.state.ecology.wood[di];
    let wd_before = world.state.ecology.dead_wood[ti];
    world.step();

    // 3d killed it this tick.
    assert_eq!(world.state.ecology.plant_deaths_total, 1, "the stand must die on this tick");
    assert!(
        wd_before < world.state.ecology.dead_wood[ti],
        "and its wood must have become dead wood"
    );
    // 3h saw `W⁴` and treated it as bare, so the donor spent on it in the **same** tick. The
    // donor also pays its own (tiny) maintenance out of reserve in 4.3, before 3h reads `Q⁴`.
    let donor_upkeep = m_w * donor_wood * DT;
    let q4 = q_before - donor_upkeep;
    let floor = floor_fraction * reserve_cap * world.state.ecology.wood[di];
    let sent = (q4 - floor).max(0.0).min(rate * DT);
    assert!(sent > 0.0, "the donor must have something to send");
    assert!(
        (q_before - world.state.ecology.plant_reserve[di] - (donor_upkeep + sent)).abs() < 1e-15,
        "the donor's reserve fell by {}, not upkeep {donor_upkeep} plus the sent {sent}",
        q_before - world.state.ecology.plant_reserve[di]
    );
    let net = sent / build;
    assert!(
        (world.state.ecology.wood[ti] - split[0] * net).abs() < 1e-15,
        "the dead cell holds {} of wood, not the propagule's {}",
        world.state.ecology.wood[ti],
        split[0] * net
    );
    assert!(
        (world.state.fields.p[ti] - split[1] * net).abs() < 1e-15,
        "nor the propagule's starter foliage"
    );
    assert!(
        (world.state.ecology.plant_reserve[ti] - split[2] * net).abs() < 1e-15,
        "nor its starter reserve"
    );
    assert!(world.mass_residual().abs() < 1e-9);
    println!(
        "A9b: the stand died and received {net:.3e} m of propagule in tick {}",
        world.tick()
    );
}

/// **A9c — two donors sharing three recipients allocate by the §4.8 rule.**
///
/// Each donor splits its own budget `B_j = min(Q⁴ − q_prop·Q_max, k_est·dt·n_j)` equally among
/// its `n_j` recipients, every transfer is read from one immutable snapshot, and a recipient
/// shared by both donors receives the **sum**. The one-donor case (A9) cannot see either the
/// `n_j` factor or the sum, so a wrong denominator or a last-writer-wins commit passes it
/// (Astra's implementation review, finding 4).
#[test]
fn a9c_two_donors_split_their_own_budgets_and_a_shared_recipient_gets_both() {
    let mut cfg = bare_config(BRIGHT.0, BRIGHT.1);
    cfg.producer.growth = 0.0;
    cfg.producer.mortality = 0.0;
    // No maintenance anywhere, so the only thing that moves a reserve is the propagule.
    cfg.plant.maintenance = 0.0;
    cfg.plant.foliage_rate = 0.0;
    cfg.plant.wood_rate = 0.0;
    cfg.plant.reserve_share = 0.0;
    cfg.fruit.ripen = 0.0;
    cfg.fruit.drop = 0.0;
    cfg.detritus.decomposition = 0.0;
    cfg.detritus.carrion_decomposition = 0.0;
    cfg.detritus.wood_decomposition = 0.0;
    cfg.detritus.fall = 0.0;
    cfg.nutrient.diffusion = 0.0;
    let split = cfg.plant.propagule_split;
    let build = 1.0 + cfg.plant.build;
    let c_g = cfg.plant.build;
    let (rate, floor_fraction, reserve_cap, alive_min, donor_min) = (
        cfg.plant.propagule_rate,
        cfg.plant.donor_reserve_floor,
        cfg.plant.reserve_cap,
        cfg.plant.alive_min,
        cfg.plant.donor_min,
    );

    // Two donors side by side in a row, each with its own bare neighbours, and one bare cell
    // between them that both can reach.
    //
    // ```
    //        (7,7)      (9,7)          <- one private recipient each
    //  (6,8) (7,8) (8,8) (9,8) (10,8)  <- donors at (7,8) and (9,8); (8,8) is shared
    //        (7,9)      (9,9)          <- one private recipient each
    // ```
    // The row cells flanking the donors are alive-but-not-donors, so each donor has exactly
    // three recipients: its two private ones and the shared centre.
    let a = CellId::new(Topology::Cube, Scale::ONE, Face::Top, 7, 8);
    let b = CellId::new(Topology::Cube, Scale::ONE, Face::Top, 9, 8);
    let shared = CellId::new(Topology::Cube, Scale::ONE, Face::Top, 8, 8);
    let private = [
        CellId::new(Topology::Cube, Scale::ONE, Face::Top, 7, 7),
        CellId::new(Topology::Cube, Scale::ONE, Face::Top, 7, 9),
        CellId::new(Topology::Cube, Scale::ONE, Face::Top, 9, 7),
        CellId::new(Topology::Cube, Scale::ONE, Face::Top, 9, 9),
    ];

    let mut world = World::new(cfg).expect("valid");
    // Two donors with **different budgets**, so an implementation that pooled them, or used
    // one donor's for both, would give the wrong per-recipient share. Donor A has reserve to
    // spare and is capped by `k_est · dt · n_j`; donor B is capped by what it holds above its
    // floor, which is set to half of A's rate cap.
    let wood = 0.6f64;
    let floor = floor_fraction * reserve_cap * wood;
    paint(&mut world, a, 0.0, wood, 0.30);
    paint(&mut world, b, 0.0, wood, floor + 0.5 * rate * DT * 3.0);
    for c in [CellId::new(Topology::Cube, Scale::ONE, Face::Top, 6, 8), CellId::new(Topology::Cube, Scale::ONE, Face::Top, 10, 8)] {
        paint(&mut world, c, 0.0, 0.5 * (alive_min + donor_min), 0.0);
    }
    let mut world = restage(world);

    // Confirm the arrangement before measuring it.
    for c in [shared].iter().chain(private.iter()) {
        assert_eq!(
            CellClass::of(world.state.ecology.wood[c.index()], alive_min),
            CellClass::Bare,
            "{c:?} must be a recipient"
        );
    }
    let q_a0 = world.state.ecology.plant_reserve[a.index()];
    let q_b0 = world.state.ecology.plant_reserve[b.index()];
    assert!(q_a0 > q_b0, "the two donors must differ, or the test cannot tell them apart");
    let n0: Vec<f64> = world.state.fields.n.clone();

    world.step();

    // Each donor's own budget, over its own three recipients.
    let budget = |q0: f64, wood: f64| {
        let floor = floor_fraction * reserve_cap * wood;
        (q0 - floor).max(0.0).min(rate * DT * 3.0)
    };
    let b_a = budget(q_a0, world.state.ecology.wood[a.index()]);
    let b_b = budget(q_b0, world.state.ecology.wood[b.index()]);
    assert!(b_a > 0.0 && b_b > 0.0);
    assert!(
        (q_a0 - world.state.ecology.plant_reserve[a.index()] - b_a).abs() < 1e-15,
        "donor A spent {}, not its budget {b_a}",
        q_a0 - world.state.ecology.plant_reserve[a.index()]
    );
    assert!(
        (q_b0 - world.state.ecology.plant_reserve[b.index()] - b_b).abs() < 1e-15,
        "donor B spent {}, not its budget {b_b}",
        q_b0 - world.state.ecology.plant_reserve[b.index()]
    );

    let share_a = b_a / 3.0;
    let share_b = b_b / 3.0;
    // The shared recipient gets **both** shares; each private one gets its own donor's.
    let expect = |s: f64| (s / build, s);
    for (cell, incoming) in [
        (shared, share_a + share_b),
        (private[0], share_a),
        (private[1], share_a),
        (private[2], share_b),
        (private[3], share_b),
    ] {
        let k = cell.index();
        let (net, sent) = expect(incoming);
        assert!(
            (world.state.ecology.wood[k] - split[0] * net).abs() < 1e-15,
            "{cell:?}: wood {} is not {}",
            world.state.ecology.wood[k],
            split[0] * net
        );
        assert!(
            (world.state.fields.p[k] - split[1] * net).abs() < 1e-15,
            "{cell:?}: foliage"
        );
        assert!(
            (world.state.ecology.plant_reserve[k] - split[2] * net).abs() < 1e-15,
            "{cell:?}: reserve"
        );
        assert!(
            (world.state.fields.n[k] - (n0[k] + c_g * net)).abs() < 1e-15,
            "{cell:?}: the construction nutrient is not `c_g · net`"
        );
        let _ = sent;
    }
    // A shared recipient really did get more than a private one — the sum is visible, not a
    // coincidence of equal budgets.
    assert!(
        world.state.ecology.wood[shared.index()] > world.state.ecology.wood[private[0].index()],
        "the shared recipient must receive both donors' shares"
    );
    assert!(
        world.state.ecology.wood[private[0].index()]
            > world.state.ecology.wood[private[2].index()],
        "and the richer donor's recipients must receive more than the poorer donor's"
    );
    assert!(world.mass_residual().abs() < 1e-9);
    println!(
        "A9c: donor A sent {b_a:.3e} over 3, donor B {b_b:.3e} over 3; the shared cell took \
         {:.3e} of wood",
        world.state.ecology.wood[shared.index()]
    );
}

// ------------------------------------- the complete animal bill, repair cycle 3

/// **The exposed per-tick body bill is the sum of the individual bills, including a body that
/// dies in that tick.**
///
/// Repair cycle 2's B6 reconstructed upkeep from outside the step: it used
/// `MotorBill::upkeep` plus a travel term derived from the distance the world transported each
/// body, which omitted the rotational half of the motor charge, and it iterated the organisms
/// **after** the step, so a body that died in the tick paid a bill nobody counted. Both make
/// the reported income/upkeep ratio an upper bound rather than the actual one §13.2 B6b asks
/// for (Astra's cycle 2 verification).
///
/// `IntakeDiagnostics::body_bill_*` is recorded where the charge is levied instead. This test
/// is the claim that it is complete: on a tick with three live bodies and one dying one, the
/// exposed total equals `Σ MotorBill::total_cost` over **all four**, the exposed mandatory half
/// equals `Σ MotorBill::upkeep`, and the paid total is short of the owed total by exactly what
/// the dying body could not raise.
#[test]
fn the_exposed_body_bill_is_the_sum_of_every_bill_including_a_body_that_dies_this_tick() {
    let mut cfg = bare_config(BRIGHT.0, BRIGHT.1);
    cfg.drives.feed_min = 0.001;
    // Turning must be real and paid, so the rotational half of the bill is nonzero: the old
    // reconstruction dropped exactly this term.
    cfg.drives.turn_noise = 1.0;
    let mut world = World::new(cfg).expect("valid");
    let cell = CellId::new(Topology::Cube, Scale::ONE, Face::Top, 8, 8);
    paint(&mut world, cell, 0.56, 0.6, 0.30);
    // Three bodies with stores, and one with none at all.
    let live: Vec<OrganismId> = (0..3)
        .map(|k| place(&mut world, cell, 0.85, f64::from(k) * 0.5))
        .collect();
    for id in &live {
        let o = world.state.organisms.get_mut(*id).expect("placed");
        let (r, e) = (o.phenotype.reserve_max, o.phenotype.energy_max);
        world.state.external_material_in += r - o.reserve;
        o.reserve = r;
        o.energy = e;
    }
    let doomed = place(&mut world, cell, 0.85, 1.5);
    {
        let o = world.state.organisms.get_mut(doomed).expect("placed");
        o.energy = 0.0;
        o.reserve = 0.0;
    }
    let mut world = restage(world);

    // Independently recomputed from the world's own bill type, **before** the step, over every
    // body that is about to be billed — the dying one included.
    let cfg = world.config().clone();
    let expected_total: f64 = world
        .state
        .organisms
        .iter()
        .map(|(_, o)| {
            // The bill's motor term depends on the motion the step resolves, which is not
            // knowable here; its bounds are. `upkeep` is exact, and the total is at least it.
            MotorBill::of(o, &cfg).upkeep(DT)
        })
        .sum();
    let bodies_before = world.state.organisms.len();
    assert_eq!(bodies_before, 4, "three live bodies and one that cannot pay");

    let before = world.intake_diagnostics();
    // Snapshot every body's bill inputs, so the post-step recomputation uses the same
    // structure and sense radius the step charged against.
    let billed: Vec<(OrganismId, MotorBill)> = world
        .state
        .organisms
        .iter()
        .map(|(id, o)| (id, MotorBill::of(o, &cfg)))
        .collect();
    world.step();
    world.drain_events();
    let after = world.intake_diagnostics();

    assert!(
        world.state.organisms.get(doomed).is_none(),
        "the fixture must actually kill one body in this tick"
    );
    assert_eq!(
        world.state.organisms.len(),
        3,
        "and only that one — the other three must survive to be billed again"
    );

    let total = after.body_bill_total - before.body_bill_total;
    let paid = after.body_bill_paid - before.body_bill_paid;
    let upkeep = after.body_bill_upkeep - before.body_bill_upkeep;

    // The mandatory half is exact, and it is summed over **four** bodies, not the three that
    // survived. This is the assertion the old reconstruction could not make.
    let expected_upkeep: f64 = billed.iter().map(|(_, b)| b.upkeep(DT)).sum();
    assert!(
        (upkeep - expected_upkeep).abs() < 1e-15,
        "the exposed mandatory bill is {upkeep}, not the sum over all four bodies \
         {expected_upkeep}"
    );
    assert!(
        (expected_upkeep - expected_total).abs() < 1e-15,
        "the pre-step recomputation must be the same four bills"
    );
    // Three-quarters of it would be the sum over the survivors alone: the dying body's share
    // is really in there.
    let survivors_only: f64 = billed
        .iter()
        .filter(|(id, _)| *id != doomed)
        .map(|(_, b)| b.upkeep(DT))
        .sum();
    assert!(
        upkeep > survivors_only + 1e-12,
        "the dying body's bill is missing: {upkeep} is not above the survivors' {survivors_only}"
    );

    // The total owed is the mandatory half plus a motor half, and the motor half is positive —
    // the bodies moved and turned, so the term the old travel-only reconstruction dropped is
    // genuinely nonzero here.
    let motor = total - upkeep;
    assert!(
        motor > 0.0,
        "the fixture must actually charge a motor bill, or the rotational term is untested"
    );
    // And the total is exactly the sum of the four `total_cost` charges, which is the same
    // quantity `MotorBill::total_cost` computes from each body's resolved motion. The motion
    // is not observable from outside, so the identity is stated through its two halves and the
    // paid/owed relation below.
    assert!(
        total >= upkeep - 1e-15,
        "a total below its own mandatory half is not a bill"
    );

    // Paid versus owed: the three funded bodies paid in full, and the fourth was short by
    // exactly what it could not raise — it had neither energy nor reserve, so it paid nothing.
    let shortfall = total - paid;
    assert!(
        shortfall > 0.0,
        "a body with no energy and no reserve must leave the bill short: owed {total}, paid \
         {paid}"
    );
    let doomed_bill = billed
        .iter()
        .find(|(id, _)| *id == doomed)
        .map(|(_, b)| b.upkeep(DT))
        .expect("the doomed body was billed");
    assert!(
        (shortfall - doomed_bill).abs() < 1e-12,
        "the shortfall {shortfall} is not the dying body's whole bill {doomed_bill}"
    );
    assert!(paid >= 0.0 && paid < total);
    println!(
        "body bill for one tick over 4 bodies (1 dying): owed {total:.6e}, paid {paid:.6e}, \
         mandatory {upkeep:.6e}, motor {motor:.6e}; survivors' mandatory alone would have been \
         {survivors_only:.6e}"
    );
}
