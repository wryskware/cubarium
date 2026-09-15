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

use common::{stored_energy, total_material};
use cubarium_core::care::{CareCommand, CareDose, CareKind, CareTarget};
use cubarium_core::config::{PlantConfig, WorldConfig};
use cubarium_core::genome::{Genome, decode};
use cubarium_core::ids::OrganismId;
use cubarium_core::organism::{Mode, Organism, Origin};
use cubarium_core::rng::Counter;
use cubarium_core::world::CellClass;
use cubarium_core::{
    DT, FixedHunterProfile, HunterTarget, SnapshotError, World, decode_snapshot, encode_snapshot,
    snapshot::SCHEMA_VERSION, snapshot::state_hash,
};
use cubarium_surface::{CELL_COUNT, CellId, Face, SurfacePoint, Vec2, cell_of};

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
    let centre = cell.center();
    let pos = SurfacePoint::new(cell.face(), centre.u + mouth_offset, centre.v);
    assert_eq!(cell_of(&pos), cell, "the body landed outside its cell");
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
        let centre = CellId::new(Face::Top, 8, 8);
        paint(world, centre, 0.56, 0.6, 0.30);
    }

    fn doomed_stand(world: &mut World) {
        // Wood with no foliage and no reserve: maintenance is wholly unpaid, so this stand
        // diebacks every tick and eventually crosses `W_min` and dies.
        let centre = CellId::new(Face::Front, 6, 6);
        paint(world, centre, 0.0, 0.05, 0.0);
    }

    fn charged_detritus(world: &mut World) {
        let before = total_material(world);
        for cell in CellId::all() {
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
            stage: |world| paint(world, CellId::new(Face::Top, 8, 8), 0.56, 0.6, 0.0),
            tune: |_| {},
            ticks: 4_000,
        },
        Arm {
            name: "a full reserve, which takes no share",
            class: BRIGHT,
            // `Q = q_cap · W` exactly, so `D_Q = 0`: the share branch and the final top-up
            // both clamp to zero and the whole surplus goes to foliage and wood.
            stage: |world| paint(world, CellId::new(Face::Top, 8, 8), 0.56, 0.6, 0.30),
            tune: |_| {},
            ticks: 4_000,
        },
        Arm {
            name: "reflush from a stocked reserve below the threshold",
            class: BRIGHT,
            // Stripped foliage with a full reserve: the emergency draw runs every tick until
            // it reaches `p_reflush · P_cap`, then stops.
            stage: |world| paint(world, CellId::new(Face::Top, 8, 8), 0.0, 0.6, 0.30),
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
        (cfg, CellId::new(Face::Top, 8, 8))
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
    for cell in CellId::all() {
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
    for cell in CellId::all() {
        let i = cell.index();
        world.state.fields.n[i] = REFERENCE_NUTRIENT;
    }
    // A band of mature stands, a band of charged litter and a band of fresh remains, so every
    // channel has something to work on and the propagule ring has somewhere to go.
    for u in 0..16u8 {
        // Three plant bands, so the repaired §4.4 (repair cycle 1) runs every branch inside
        // the full ledger: a **full** reserve that takes no share, a **part-full** one that
        // takes `q_share` off the top every surplus tick, and a **stripped** stand below
        // `p_reflush · P_cap` that draws its reserve down for the emergency reflush.
        for v in 0..8u8 {
            let i = CellId::new(Face::Top, u, v).index();
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
        for v in 8..16u8 {
            let i = CellId::new(Face::Top, u, v).index();
            world.state.fields.d[i] = 0.5;
            world.state.fields.de[i] = 2.0 * 0.5;
            world.state.ecology.carrion[i] = 0.4;
            world.state.ecology.carrion_energy[i] = 2.0 * 0.4;
            world.state.ecology.dead_wood[i] = 0.3;
        }
    }
    book(&mut world, before);
    // Two mouths that between them use every channel: a foliage digester on the stands and a
    // detrital one on the litter-and-remains band.
    place(&mut world, CellId::new(Face::Top, 4, 4), 0.85, 0.0);
    place(&mut world, CellId::new(Face::Top, 4, 12), 0.10, 0.0);
    let mut world = restage(world);

    let feed_cell = CellId::new(Face::Top, 2, 12);
    let clean_cell = CellId::new(Face::Top, 3, 12);
    let target = |c: CellId| {
        let p = c.center();
        CareTarget { face: p.face.index() as u8, u: p.u, v: p.v }
    };

    let mut seq = 1u64;
    let mut worst = 0.0f64;
    for tick in 1..=2_000u64 {
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
    assert!(world.state.ecology.plant_deaths_total > 0 || world.tick() > 0);
    println!("A2b: worst per-tick energy drift {worst:e}");
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
    for i in 0..CELL_COUNT {
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
        let source = CellId::new(Face::Front, 6, 6);
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

    // The same at `m_p · dt = 1` with ripening on: a whole tick's senescence takes all the
    // foliage the growth left and no more, and the cell stays a well-formed stock.
    let mut cfg = bare_config(BRIGHT.0, BRIGHT.1);
    cfg.producer.mortality = 1.0 / DT;
    let mut world = World::new(cfg).expect("valid");
    let cell = CellId::new(Face::Top, 8, 8);
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
    let cell = CellId::new(Face::Front, 6, 6);
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
    let cell = CellId::new(Face::Front, 6, 6);
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
    let cell = CellId::new(Face::Front, 6, 6);
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
    let d0: f64 = world.state.fields.d.iter().sum();
    world.step();
    assert!(world.state.organisms.get(id).is_none(), "it must actually die");
    assert!(
        (world.state.ecology.carrion[cell.index()] - body).abs() < 1e-12,
        "the body landed as {} of remains, not {body}",
        world.state.ecology.carrion[cell.index()]
    );
    assert_eq!(world.state.fields.d.iter().sum::<f64>(), d0, "and not in the litter");
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
    let d0: f64 = world.state.fields.d.iter().sum();
    world.step();
    assert!(world.state.organisms.get(id).is_none());
    assert!(
        (world.state.ecology.carrion[cell.index()] - (body + escrow_material)).abs() < 1e-12,
        "body {body} plus escrow {escrow_material} vs {}",
        world.state.ecology.carrion[cell.index()]
    );
    assert_eq!(world.state.fields.d.iter().sum::<f64>(), d0, "no part of it became litter");

    // --- a hunter's digestion reject is feces and lands in `D`; its death lands in `C`.
    let mut cfg = bare_config(AVERAGE.0, AVERAGE.1);
    immobile(&mut cfg);
    cfg.detritus.decomposition = 0.0;
    cfg.detritus.carrion_decomposition = 0.0;
    let mut world = World::new(cfg).expect("valid");
    let profile = FixedHunterProfile::lanternjaw_trial(world.config());
    let spot = CellId::new(Face::Top, 8, 8).center();
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
    let here = cell_of(&world.state.organisms.get(hunter).expect("alive").pos).index();
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

    // Its death routes the body — and anything still in the gut — to `C`.
    let body = {
        let o = world.state.organisms.get(hunter).expect("alive");
        o.structure + o.reserve
    };
    let d_before = world.state.fields.d[here];
    {
        let o = world.state.organisms.get_mut(hunter).expect("alive");
        o.energy = 0.0;
        o.reserve = 0.0;
    }
    let mut world = restage(world);
    world.step();
    assert!(world.state.organisms.get(hunter).is_none(), "the member must die");
    assert!(
        world.state.ecology.carrion[here] >= body - 1e-9,
        "a dead hunter is remains: {} vs a body of {body}",
        world.state.ecology.carrion[here]
    );
    assert!(
        (world.state.fields.d[here] - d_before).abs() < 1e-12,
        "none of it became litter"
    );

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

/// **A7 — snapshot.** Schema 16 round-trips with the documented header, every older schema is
/// refused with its own version, and a relabelled schema 16 payload is refused too.
///
/// The old-schema **fixtures** are covered by their own refusal tests beside the suites that
/// used to migrate them (`care.rs`, `care_dose_migration.rs`, `hunter_migration.rs`,
/// `quiet_migration.rs`, `hunter_charging.rs`, `astra_quiet_policy.rs`,
/// `continuation_fixtures.rs`); this is the rule itself.
#[test]
fn a7_schema_sixteen_round_trips_and_every_older_schema_is_refused_by_name() {
    assert_eq!(SCHEMA_VERSION, 16);
    let mut world = bare_world(BRIGHT);
    paint(&mut world, CellId::new(Face::Top, 8, 8), 0.56, 0.6, 0.30);
    let mut world = restage(world);
    for _ in 0..50 {
        world.step();
    }

    let bytes = encode_snapshot(&world.state, "ecology-v1");
    assert_eq!(&bytes[..4], b"CUBW");
    assert_eq!(u32::from_le_bytes(bytes[4..8].try_into().unwrap()), SCHEMA_VERSION);
    let (meta, back) = decode_snapshot(&bytes).expect("schema 16 round-trips");
    assert_eq!(meta.schema, SCHEMA_VERSION);
    assert_eq!(meta.build_id, "ecology-v1");
    assert_eq!(back, world.state, "the round trip is lossless");
    assert_eq!(state_hash(&back), state_hash(&world.state));
    // The new pools really are in the payload.
    assert!(back.ecology.wood.iter().any(|&w| w > 0.0));
    assert_eq!(back.ecology.wood.len(), CELL_COUNT);

    for old in 7..SCHEMA_VERSION {
        let mut relabelled = bytes.clone();
        relabelled[4..8].copy_from_slice(&old.to_le_bytes());
        assert_eq!(
            decode_snapshot(&relabelled),
            Err(SnapshotError::UnsupportedSchema(old)),
            "schema {old} must be refused by name"
        );
    }

    // A relabelled schema 16 payload: a longer body under the current version number is
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
        "a relabelled schema 16 payload must be refused"
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
                paint(&mut w, CellId::new(Face::Top, 8, 8), 0.0, 0.18, 0.0);
                restage(w)
            },
            4_000,
        ),
        (
            "an establishment ring",
            || {
                let mut w = bare_world(BRIGHT);
                for (du, dv) in [(0i16, 0i16), (1, 0), (-1, 0), (0, 1), (0, -1)] {
                    let c = CellId::new(
                        Face::Top,
                        (8 + du) as u8,
                        (8 + dv) as u8,
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
                for cell in CellId::all() {
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
                let cell = CellId::new(Face::Top, 8, 8);
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
    let donor = CellId::new(Face::Top, 8, 8);
    let recipient = CellId::new(Face::Top, 8, 9);
    paint(&mut world, donor, 0.56, 0.6, 0.30);
    // The donor's other three neighbours are **alive but not donors**: wood above `W_min` so
    // they are not recipients, and below `W_est` so they send nothing of their own. That makes
    // the centre stand the only donor in the world and gives it exactly one recipient, so every
    // number below is one transfer rather than a sum over a ring.
    for c in [
        CellId::new(Face::Top, 8, 7),
        CellId::new(Face::Top, 7, 8),
        CellId::new(Face::Top, 9, 8),
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
