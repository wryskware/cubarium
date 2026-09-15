//! The ecology v1 ecological scenarios B0–B7 (`design/ecology-v1-contract.md` §13.2).
//!
//! # What this is, and what it is not
//!
//! **Measurements with an expected direction, not assertions.** The accounting tests
//! (`tests/ecology_v1.rs`, §13.1) pass or fail at machine precision and gate the merge. These
//! do not: each scenario prints what the simulator did and names what the contract expected,
//! and a mechanism that behaves contrary to §13.2 is a finding to report, not a number to tune
//! before review.
//!
//! **Censored at 36,000 ticks (30 min simulated), never extended.** A quantity not reached by
//! the horizon is reported as `censored`. No scenario runs longer to make something converge.
//!
//! **B0 is the anchor.** The §11 steady state is a hand calculation; B0 measures what the
//! simulator's own lone stand does from the §11 seed, and every later scenario paints *B0's
//! measured* values rather than the hand table. A large gap between the two is itself the
//! finding B0 exists to produce.
//!
//! **Staged fixtures.** Following `examples/food_stock_flow.rs`: no founders, no weather swing,
//! no rain, no mutation, and a world stripped to the named cells. Light and moisture are flat
//! per run, so every cell is the run's one reference class — **average** `L = 0.5, μ = 0.7` or
//! **bright** `L = 0.8, μ = 0.75` — with `N = 0.4`. Every hand-written stock is booked into the
//! world's own material ledger, so the mass identity stays closed and `check_invariants` holds
//! throughout.
//!
//! Run it:
//!
//! ```text
//! cargo run -p cubarium-core --release --example ecology_v1_scenarios -- b0
//! ```
//!
//! Subcommands: `b0 b1a b1b b2 b3 b4a b4b b5 b6a b6b b7`, or `all`.

use std::collections::BTreeSet;

use cubarium_core::config::WorldConfig;
use cubarium_core::genome::{Genome, decode};
use cubarium_core::ids::OrganismId;
use cubarium_core::organism::{Mode, Organism, Origin};
use cubarium_core::rng::Counter;
use cubarium_core::world::CellClass;
use cubarium_core::{DT, World};
use cubarium_surface::{CellId, Face, SurfacePoint, Vec2, cell_of};

/// §13: every scenario runs at most 36,000 ticks (30 min simulated) and is censored there.
const HORIZON: u64 = 36_000;
/// §13.2 B1a's own, shorter horizon.
const B1A_TICKS: u64 = 12_000;
/// How often a trajectory row is printed.
const SAMPLE: u64 = 1_000;

// ------------------------------------------------------------------ reference classes

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Class {
    Average,
    Bright,
}

impl Class {
    fn light(self) -> f64 {
        match self {
            Class::Average => 0.5,
            Class::Bright => 0.8,
        }
    }

    fn moisture(self) -> f64 {
        match self {
            Class::Average => 0.7,
            Class::Bright => 0.75,
        }
    }

    fn name(self) -> &'static str {
        match self {
            Class::Average => "average",
            Class::Bright => "bright",
        }
    }
}

const NUTRIENT: f64 = 0.4;

/// The staged config: the world's ordinary ecology on a flat habitat of one reference class,
/// with nothing in it that the scenario did not put there.
fn staged(light: f64, moisture: f64) -> WorldConfig {
    let mut c = WorldConfig::default();
    c.founders.kinds.clear();
    c.founders.count = 0;
    c.weather.amplitude = 0.0;
    c.water.rain_rate = 0.0;
    c.mechanisms.mutation = false;
    c.habitat.light_base = light;
    c.habitat.light_height_gain = 0.0;
    c.habitat.light_noise_gain = 0.0;
    c.habitat.moisture_base = moisture;
    c.habitat.moisture_height_gain = 0.0;
    c.habitat.moisture_noise_gain = 0.0;
    // A bare surface: the scenario paints what it means to measure.
    c.plant.initial_wood = 0.0;
    c.producer.initial_fraction = 0.0;
    c.detritus.initial_dark = 0.0;
    c
}

fn class_config(class: Class) -> WorldConfig {
    staged(class.light(), class.moisture())
}

// ------------------------------------------------------------------------- bookkeeping

/// Every unit of material the world holds, ecology v1's pools included.
fn material(world: &World) -> f64 {
    let s = &world.state;
    s.fields.n.iter().sum::<f64>()
        + s.fields.p.iter().sum::<f64>()
        + s.fields.d.iter().sum::<f64>()
        + s.fields.f.iter().sum::<f64>()
        + s.ecology.total_material()
        + s.organisms.iter().map(|(_, o)| o.material()).sum::<f64>()
        + s.hunters.gut_material_total()
}

/// Book a hand edit as admitted (or exported) material, so the world's mass identity closes.
fn book(world: &mut World, before: f64) {
    let after = material(world);
    world.state.external_material_in += after - before;
}

fn nutrient_everywhere(world: &mut World) {
    let before = material(world);
    for v in world.state.fields.n.iter_mut() {
        *v = NUTRIENT;
    }
    book(world, before);
}

fn restage(world: World) -> World {
    let world = World::from_state(world.state).expect("the staged state is a valid world");
    world.check_invariants().expect("the staged world is consistent");
    world
}

// --------------------------------------------------------------------------- the stand

/// A stand's four stocks, as B0 measured them (or as a scenario paints them).
#[derive(Clone, Copy, Debug, Default)]
struct Stand {
    p: f64,
    w: f64,
    q: f64,
    f: f64,
}

impl Stand {
    /// The §11 seed: `W_0 = 0.5 · W_max · L · μ`, `P_0 = 0.4 · P_cap(W_0)`,
    /// `Q_0 = 0.5 · q_cap · W_0`, no fruit.
    fn seed(_cfg: &WorldConfig, class: Class) -> Stand {
        // `staged` zeroes the seeding knobs so a scenario can paint a bare surface, so the §11
        // seed is read from the crate defaults instead: this is §11's `W_0`, not the fixture's
        // own (bare) opening.
        let d = WorldConfig::default();
        let w = d.plant.initial_wood * d.plant.wood_max * class.light() * class.moisture();
        let p_cap = d.producer.max.min(d.plant.alpha * w);
        Stand {
            p: d.producer.initial_fraction * p_cap,
            w,
            q: 0.5 * d.plant.reserve_cap * w,
            f: 0.0,
        }
    }

    /// The §11 hand table's "mature stand", for comparison only.
    fn hand(class: Class) -> Stand {
        match class {
            Class::Average => Stand { p: 0.50, w: 0.33, q: 0.16, f: 0.03 },
            Class::Bright => Stand { p: 0.56, w: 0.60, q: 0.30, f: 0.16 },
        }
    }

    fn of(world: &World, cell: CellId) -> Stand {
        let i = cell.index();
        Stand {
            p: world.state.fields.p[i],
            w: world.state.ecology.wood[i],
            q: world.state.ecology.plant_reserve[i],
            f: world.state.fields.f[i],
        }
    }

    fn material(self) -> f64 {
        self.p + self.w + self.q + self.f
    }
}

/// Paint one cell as a stand, booking the material.
fn paint(world: &mut World, cell: CellId, s: Stand) {
    let before = material(world);
    let i = cell.index();
    world.state.fields.p[i] = s.p;
    world.state.fields.f[i] = s.f;
    world.state.ecology.wood[i] = s.w;
    world.state.ecology.plant_reserve[i] = s.q;
    book(world, before);
}

// -------------------------------------------------------------------------- the animal

/// Place one legacy animal of the named `diet` at a cell centre, hungry and empty.
fn place(world: &mut World, cell: CellId, diet: f32, nth: usize) -> OrganismId {
    place_genome(world, cell, nth, |g| g.diet = diet)
}

/// Place one legacy animal built from the named default founder kind — its `diet`, `size`,
/// `metabolism`, `speed`, `depth` and `swim`, not only its diet, so B5 measures the animal the
/// contract's §6.1 table names rather than a stand-in for it.
fn place_kind(world: &mut World, cell: CellId, kind_name: &str, nth: usize) -> OrganismId {
    let kinds = cubarium_core::config::FounderKind::defaults();
    let kind = kinds
        .iter()
        .find(|k| k.name == kind_name)
        .unwrap_or_else(|| panic!("the default roster has a {kind_name}"))
        .clone();
    place_genome(world, cell, nth, move |g| {
        if let Some(x) = kind.diet {
            g.diet = x;
        }
        if let Some(x) = kind.size {
            g.size = x;
        }
        if let Some(x) = kind.metabolism {
            g.metabolism = x;
        }
        if let Some(x) = kind.speed {
            g.speed = x;
        }
        if let Some(x) = kind.depth {
            g.depth = x;
        }
        if let Some(x) = kind.swim {
            g.swim = x;
        }
    })
}

fn place_genome(
    world: &mut World,
    cell: CellId,
    nth: usize,
    edit: impl FnOnce(&mut Genome),
) -> OrganismId {
    let cfg = world.config().clone();
    let mut genome = Genome::founder(0.5, &cfg.drives);
    edit(&mut genome);
    genome.clamp();
    let phenotype = decode(&genome, &cfg.organism);
    let centre = cell.center();
    let offset = [(0.0, 0.0), (1.0, 0.0), (0.0, 1.0), (1.0, 1.0)][nth % 4];
    let pos = SurfacePoint::new(cell.face(), centre.u + offset.0, centre.v + offset.1);
    assert_eq!(cell_of(&pos), cell, "the body landed outside its cell");
    let id = world.state.organisms.insert(Organism {
        pos,
        heading: Vec2::new(1.0, 0.0),
        ou: Vec2::ZERO,
        structure: phenotype.structure_adult,
        reserve: 0.5 * phenotype.reserve_max,
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

/// Pin every body where it stands: the staged measurement of §13, not a foraging claim.
fn pinned(cfg: &mut WorldConfig) {
    cfg.organism.speed_max = 0.0;
    cfg.drives.turn_rate_max_deg = 0.0;
}

/// The grazer's own food gate has to be reachable on the stocks a scenario paints.
fn hungry_gate(cfg: &mut WorldConfig) {
    cfg.drives.feed_min = 0.001;
}

// ------------------------------------------------------------------------- reporting

fn censored(reached: Option<u64>) -> String {
    match reached {
        Some(t) => format!("{t} ({:.0} s)", t as f64 * DT),
        None => format!("censored at {HORIZON}"),
    }
}

fn head(name: &str, what: &str) {
    println!();
    println!("## {name} — {what}");
}

fn row_header() {
    println!(
        "{:>7} {:>8} {:>8} {:>8} {:>8} {:>10} {:>10} {:>10}",
        "tick", "P", "W", "Q", "F", "income", "senescence", "ripening"
    );
}

/// One trajectory row. `income`, `senescence` and `ripening` are **rates over the sample
/// interval**, in m/s, so they are directly comparable with §11's per-second arithmetic.
#[allow(clippy::too_many_arguments)]
fn row(tick: u64, s: Stand, income: f64, senescence: f64, ripening: f64) {
    println!(
        "{tick:>7} {:>8.4} {:>8.4} {:>8.4} {:>8.4} {:>10.3e} {:>10.3e} {:>10.3e}",
        s.p, s.w, s.q, s.f, income, senescence, ripening
    );
}

fn compare(label: &str, measured: Stand, hand: Stand) {
    println!(
        "- {label}: measured P {:.4} W {:.4} Q {:.4} F {:.4}   |   §11 hand table P {:.2} W {:.2} \
         Q {:.2} F {:.2}   |   ΔP {:+.4} ΔW {:+.4}",
        measured.p,
        measured.w,
        measured.q,
        measured.f,
        hand.p,
        hand.w,
        hand.q,
        hand.f,
        measured.p - hand.p,
        measured.w - hand.w,
    );
}

// ------------------------------------------------------------------------------- B0

/// B0's outcome: the terminal stand, whether it was still moving, and what the patch around it
/// did (a lone stand is a **donor**, so §4.8 is running and its neighbours can establish).
struct Baseline {
    terminal: Stand,
    /// `|ΔP|` and `|ΔW|` over the last sample interval, as a per-second rate.
    drift_p: f64,
    drift_w: f64,
    established: u64,
    propagule_sent: f64,
}

fn run_b0(class: Class, verbose: bool) -> Baseline {
    let cfg = class_config(class);
    let seed = Stand::seed(&cfg, class);
    let mut world = World::new(cfg).expect("the staged config is valid");
    nutrient_everywhere(&mut world);
    let focal = CellId::new(Face::Top, 8, 8);
    paint(&mut world, focal, seed);
    let mut world = restage(world);

    if verbose {
        head(
            &format!("B0 {}", class.name()),
            "one lone stand, seeded at the §11 `W_0`, no animals, 36,000 ticks",
        );
        println!(
            "seed: P {:.4} W {:.4} Q {:.4} (§11: W_0 = initial_wood · W_max · L · μ)",
            seed.p, seed.w, seed.q
        );
        println!(
            "note: §4.8 is part of the model and is **on**. A lone stand that clears `W_est` and \
             its reserve floor spends on its bare neighbours, so the patch can spread; the \
             propagule spend and the number of cells that crossed `W_min` are reported below."
        );
        row_header();
    }

    let mut previous = Stand::of(&world, focal);
    let mut last_sample = previous;
    let mut last_diag = world.intake_diagnostics();
    let mut drift_p = 0.0;
    let mut drift_w = 0.0;
    for tick in 1..=HORIZON {
        world.step();
        if tick % SAMPLE == 0 {
            let now = Stand::of(&world, focal);
            let diag = world.intake_diagnostics();
            let seconds = SAMPLE as f64 * DT;
            let income = (diag.plant_income - last_diag.plant_income) / seconds;
            let grown = (diag.producer_growth - last_diag.producer_growth) / seconds;
            // Senescence and ripening are not recorded separately, so they are reconstructed
            // from the identity the tick applies: what the cell gained and what it kept.
            let senescence = world.config().producer.mortality * now.p;
            let over = (now.p / world.config().producer.max - world.config().fruit.fruit_min)
                .max(0.0);
            let ripening = world.config().fruit.ripen * now.p * over * class.light();
            if verbose {
                row(tick, now, income, senescence, ripening);
            }
            let _ = grown;
            drift_p = (now.p - last_sample.p).abs() / seconds;
            drift_w = (now.w - last_sample.w).abs() / seconds;
            last_sample = now;
            last_diag = diag;
            previous = now;
        }
    }
    world.check_invariants().expect("B0 ends consistent");
    let terminal = Stand::of(&world, focal);
    let _ = previous;
    let established = world.state.ecology.recolonisations_total;
    let propagule_sent = world.intake_diagnostics().propagule_sent;

    if verbose {
        compare(&format!("B0 {}", class.name()), terminal, Stand::hand(class));
        println!(
            "- still moving at the horizon: |ΔP| {drift_p:.3e} m/s, |ΔW| {drift_w:.3e} m/s over \
             the last {SAMPLE} ticks. **This is a finite-horizon baseline, not proof of \
             equilibrium**; the horizon is not extended to make it converge."
        );
        println!(
            "- propagules: {propagule_sent:.4} m of reserve sent, {established} cell(s) crossed \
             `W_min`."
        );
    }

    Baseline { terminal, drift_p, drift_w, established, propagule_sent }
}

fn b0() {
    println!("# B0 — stand baseline (§13.2)");
    println!(
        "The §11 table is a hand calculation; these are the simulator's own numbers, and they \
         are what every later scenario paints and is judged against."
    );
    let avg = run_b0(Class::Average, true);
    let bright = run_b0(Class::Bright, true);
    println!();
    println!("## B0 summary at the 36,000-tick horizon");
    println!(
        "{:<8} {:>8} {:>8} {:>8} {:>8} {:>12} {:>12} {:>8} {:>10}",
        "class", "P", "W", "Q", "F", "|dP/dt|", "|dW/dt|", "estab.", "sent"
    );
    for (class, b) in [(Class::Average, &avg), (Class::Bright, &bright)] {
        println!(
            "{:<8} {:>8.4} {:>8.4} {:>8.4} {:>8.4} {:>12.3e} {:>12.3e} {:>8} {:>10.4}",
            class.name(),
            b.terminal.p,
            b.terminal.w,
            b.terminal.q,
            b.terminal.f,
            b.drift_p,
            b.drift_w,
            b.established,
            b.propagule_sent
        );
    }
    println!();
    println!("Expected direction (§13.2): approaches the §11 table — average `P ≈ 0.5, W ≈ 0.33`;");
    println!("bright `P ≈ 0.56, W = 0.6`. The comparison rows above are the finding.");
}

/// The measured mature stand for a class, from a B0 run. Every scenario that paints a "mature
/// stand" uses this rather than the §11 hand table.
fn mature(class: Class) -> Stand {
    run_b0(class, false).terminal
}

// ------------------------------------------------------------------------------ B1a

fn b1a() {
    println!("# B1a — one pinned grazer, one bright mature stand, 12,000 ticks (§13.2)");
    let stand = mature(Class::Bright);
    println!(
        "painted from B0 bright: P {:.4} W {:.4} Q {:.4} F {:.4}",
        stand.p, stand.w, stand.q, stand.f
    );

    let mut cfg = class_config(Class::Bright);
    pinned(&mut cfg);
    hungry_gate(&mut cfg);
    let mut world = World::new(cfg).expect("valid");
    nutrient_everywhere(&mut world);
    let cell = CellId::new(Face::Top, 8, 8);
    paint(&mut world, cell, stand);
    let grazer = place(&mut world, cell, 0.85, 0);
    let mut world = restage(world);

    let reserve_max = world.state.organisms.get(grazer).expect("alive").phenotype.reserve_max;
    println!();
    println!(
        "{:>7} {:>8} {:>8} {:>8} {:>8} {:>9} {:>10} {:>10} {:>8}",
        "tick", "P", "W", "Q", "F", "reserve", "leaf eaten", "fruit", "sat%"
    );
    let mut died = None;
    let mut stand_died = None;
    for tick in 0..=B1A_TICKS {
        if tick % SAMPLE == 0 {
            let s = Stand::of(&world, cell);
            let diag = world.intake_diagnostics();
            let reserve = world.state.organisms.get(grazer).map(|o| o.reserve);
            let sat = if diag.request_ticks > 0 {
                100.0 * diag.reserve_saturated_ticks as f64 / diag.request_ticks as f64
            } else {
                f64::NAN
            };
            println!(
                "{tick:>7} {:>8.4} {:>8.4} {:>8.4} {:>8.4} {:>9} {:>10.4} {:>10.4} {:>7.1}%",
                s.p,
                s.w,
                s.q,
                s.f,
                reserve.map_or("dead".to_string(), |r| format!("{r:.4}")),
                diag.producer_eaten,
                diag.fruit_eaten,
                sat
            );
        }
        if tick == B1A_TICKS {
            break;
        }
        world.step();
        world.drain_events();
        if died.is_none() && world.state.organisms.get(grazer).is_none() {
            died = Some(tick + 1);
        }
        if stand_died.is_none() && world.state.ecology.plant_deaths_total > 0 {
            stand_died = Some(tick + 1);
        }
    }
    world.check_invariants().expect("B1a ends consistent");
    let diag = world.intake_diagnostics();
    let s = Stand::of(&world, cell);
    let seconds = B1A_TICKS as f64 * DT;

    println!();
    println!("## B1a result");
    // The rate is over the window the grazer actually fed in — its own horizon, censored at
    // the run's — not over the whole run, which would average a live mouth with a dead one.
    let fed_ticks = died.unwrap_or(B1A_TICKS);
    let fed_seconds = fed_ticks as f64 * DT;
    println!(
        "- intake: leaf {:.4} m, fruit {:.4} m, feces {:.4} m over the {fed_seconds:.0} s it \
         lived; mean bite rate {:.3e} m/s against §11's sustainable bright yield of 6.4e-4 m/s \
         — a ratio of {:.1}×.",
        diag.producer_eaten,
        diag.fruit_eaten,
        diag.undigested,
        (diag.producer_eaten + diag.fruit_eaten) / fed_seconds,
        (diag.producer_eaten + diag.fruit_eaten) / fed_seconds / 6.4e-4,
    );
    let _ = seconds;
    println!(
        "- reserve-saturated ticks: {} of {} requesting ticks ({:.1}%).",
        diag.reserve_saturated_ticks,
        diag.request_ticks,
        100.0 * diag.reserve_saturated_ticks as f64 / diag.request_ticks.max(1) as f64
    );
    println!("- R_max was {reserve_max:.4} m.");
    println!(
        "- stand at the horizon: P {:.4} W {:.4} Q {:.4} Wd {:.4}; stand death at {}.",
        s.p,
        s.w,
        s.q,
        world.state.ecology.dead_wood[cell.index()],
        censored(stand_died)
    );
    println!("- grazer starved at {}.", censored(died));
    println!(
        "Expected direction (§13.2): the bite exceeds the yield ~40×, the reserve saturates \
         first, the stand is stripped, reflushes from `Q` and dies, and the grazer then \
         starves. Coexistence is **not** expected."
    );
}

// ------------------------------------------------------------------------------ B1b

fn region(centre: CellId, half: i32) -> Vec<CellId> {
    let mut out = Vec::new();
    let (cu, cv) = (i32::from(centre.cx()), i32::from(centre.cy()));
    for du in -half..=half {
        for dv in -half..=half {
            let (u, v) = (cu + du, cv + dv);
            if (0..16).contains(&u) && (0..16).contains(&v) {
                out.push(CellId::new(centre.face(), u as u8, v as u8));
            }
        }
    }
    out
}

fn run_b1b(class: Class) {
    let stand = mature(class);
    let mut cfg = class_config(class);
    hungry_gate(&mut cfg);
    let mut world = World::new(cfg).expect("valid");
    nutrient_everywhere(&mut world);
    let centre = CellId::new(Face::Top, 8, 8);
    let cells = region(centre, 2);
    for c in &cells {
        paint(&mut world, *c, stand);
    }
    let painted: f64 = cells.len() as f64 * stand.material();
    let grazer = place(&mut world, centre, 0.85, 0);
    let mut world = restage(world);

    let mut visited: BTreeSet<u16> = BTreeSet::new();
    let mut died = None;
    println!();
    println!(
        "### B1b {} — 5 × 5 mature region ({} cells), one mobile legacy grazer",
        class.name(),
        cells.len()
    );
    println!(
        "{:>7} {:>10} {:>9} {:>9} {:>10} {:>10} {:>8}",
        "tick", "Σ P", "min P", "reserve", "leaf eaten", "Σ W", "cells"
    );
    for tick in 0..=HORIZON {
        if let Some(o) = world.state.organisms.get(grazer) {
            visited.insert(cell_of(&o.pos).0);
        }
        if tick % (SAMPLE * 4) == 0 {
            let p_total: f64 = cells.iter().map(|c| world.state.fields.p[c.index()]).sum();
            let p_min = cells
                .iter()
                .map(|c| world.state.fields.p[c.index()])
                .fold(f64::INFINITY, f64::min);
            let w_total: f64 = cells.iter().map(|c| world.state.ecology.wood[c.index()]).sum();
            let reserve = world.state.organisms.get(grazer).map_or(f64::NAN, |o| o.reserve);
            let diag = world.intake_diagnostics();
            println!(
                "{tick:>7} {p_total:>10.4} {p_min:>9.4} {reserve:>9.4} {:>10.4} {w_total:>10.4} \
                 {:>8}",
                diag.producer_eaten + diag.fruit_eaten,
                visited.len()
            );
        }
        if tick == HORIZON {
            break;
        }
        world.step();
        world.drain_events();
        if died.is_none() && world.state.organisms.get(grazer).is_none() {
            died = Some(tick + 1);
        }
    }
    world.check_invariants().expect("B1b ends consistent");
    let diag = world.intake_diagnostics();
    let p_total: f64 = cells.iter().map(|c| world.state.fields.p[c.index()]).sum();
    let p_min = cells
        .iter()
        .map(|c| world.state.fields.p[c.index()])
        .fold(f64::INFINITY, f64::min);
    let yield_per_stand = match class {
        Class::Average => 1.5e-4,
        Class::Bright => 6.4e-4,
    };
    println!(
        "- region production (§11): {} × {:.1e} = {:.4e} m/s against a resting need of \
         5.4e-3 m/s (6.9e-3 cruising).",
        cells.len(),
        yield_per_stand,
        cells.len() as f64 * yield_per_stand
    );
    println!(
        "- cells visited {}, region Σ P {p_total:.4} (painted {:.4}), minimum stand P {p_min:.4}, \
         stand deaths {}, grazer starved at {}.",
        visited.len(),
        painted,
        world.state.ecology.plant_deaths_total,
        censored(died)
    );
    println!(
        "- intake: leaf {:.4} m, fruit {:.4} m, litter {:.4} m, remains {:.4} m.",
        diag.producer_eaten, diag.fruit_eaten, diag.litter_eaten, diag.carrion_eaten
    );
}

fn b1b() {
    println!("# B1b — one mobile legacy grazer on a 5 × 5 mature region, 36,000 ticks (§13.2)");
    println!(
        "Expected direction: bright — coexistence with retained foliage is **expected**, and a \
         failure there is a finding. Average — the grazer is expected to run the region down. \
         Both are reported either way; §13.2 marks B1b unresolved by design."
    );
    run_b1b(Class::Bright);
    run_b1b(Class::Average);
}

// ------------------------------------------------------------------------------- B2

fn b2() {
    println!("# B2 — depletion and relocation: three bright mature stands, three cells apart");
    let stand = mature(Class::Bright);
    let mut cfg = class_config(Class::Bright);
    hungry_gate(&mut cfg);
    let mut world = World::new(cfg).expect("valid");
    nutrient_everywhere(&mut world);
    let stands = [
        CellId::new(Face::Top, 5, 8),
        CellId::new(Face::Top, 8, 8),
        CellId::new(Face::Top, 11, 8),
    ];
    for c in stands {
        paint(&mut world, c, stand);
    }
    let grazer = place(&mut world, stands[1], 0.85, 0);
    let mut world = restage(world);

    let mut visited: BTreeSet<u16> = BTreeSet::new();
    let mut ticks_on: [u64; 3] = [0; 3];
    let mut minima = [f64::INFINITY; 3];
    let mut q_at_departure: [Option<f64>; 3] = [None; 3];
    let mut on: Option<usize> = None;
    let mut died = None;
    for tick in 0..HORIZON {
        if let Some(o) = world.state.organisms.get(grazer) {
            let here = cell_of(&o.pos);
            visited.insert(here.0);
            let index = stands.iter().position(|c| *c == here);
            if index != on {
                if let Some(left) = on {
                    q_at_departure[left] = Some(world.state.ecology.plant_reserve[stands[left].index()]);
                }
                on = index;
            }
            if let Some(i) = index {
                ticks_on[i] += 1;
            }
        }
        for (i, c) in stands.iter().enumerate() {
            minima[i] = minima[i].min(world.state.fields.p[c.index()]);
        }
        world.step();
        world.drain_events();
        if died.is_none() && world.state.organisms.get(grazer).is_none() {
            died = Some(tick + 1);
        }
    }
    world.check_invariants().expect("B2 ends consistent");

    println!();
    println!(
        "{:<10} {:>10} {:>10} {:>12} {:>10} {:>10}",
        "stand", "ticks on", "min P", "Q at exit", "P now", "Q now"
    );
    for (i, c) in stands.iter().enumerate() {
        println!(
            "{:<10} {:>10} {:>10.4} {:>12} {:>10.4} {:>10.4}",
            format!("({},{})", c.cx(), c.cy()),
            ticks_on[i],
            minima[i],
            q_at_departure[i].map_or("—".to_string(), |q| format!("{q:.4}")),
            world.state.fields.p[c.index()],
            world.state.ecology.plant_reserve[c.index()],
        );
    }
    let k_p = world.config().organism.intake_half_saturation;
    println!(
        "- cells visited {}; the type-II floor `K_P` is {k_p} m, so a stand at `P ≈ K_P` serves \
         half a mouthful.",
        visited.len()
    );
    println!("- grazer starved at {}.", censored(died));
    println!(
        "Expected direction (§13.2): the grazer leaves a stand near the type-II floor and moves \
         on; whether a stand recovers before it returns is **reported, not assumed** — the \
         `P now` column against `min P` is that report."
    );
}

// ------------------------------------------------------------------------------- B3

fn run_b3(class: Class, stand: Stand, label: &str) {
    let mut cfg = class_config(class);
    let mut world = World::new(cfg.clone()).expect("valid");
    nutrient_everywhere(&mut world);
    let cell = CellId::new(Face::Top, 8, 8);
    paint(&mut world, cell, stand);
    // The defoliation, by fiat, as an **internal transfer**: the foliage becomes litter in its
    // own cell, carrying `e_v` per unit under the `e_d_max` cap, exactly as senescence does.
    let e_v = cfg.plant.energy_density;
    let e_d_max = cfg.detritus.energy_cap;
    let i = cell.index();
    let moved = world.state.fields.p[i];
    world.state.fields.p[i] = 0.0;
    world.state.fields.d[i] += moved;
    let want = e_v * moved;
    let room = (e_d_max * world.state.fields.d[i] - world.state.fields.de[i]).max(0.0);
    world.state.fields.de[i] += want.min(room);
    // The energy the cap could not hold is heat, booked so the world's own audit closes.
    world.state.heat_out_total += want - want.min(room);
    let mut world = restage(world);
    let _ = &mut cfg;

    let target_half = 0.5 * stand.p;
    let target_nine = 0.9 * stand.p;
    let n0 = world.state.fields.n[i];
    let mut q_min = world.state.ecology.plant_reserve[i];
    let mut n_min = n0;
    let mut half = None;
    let mut nine = None;

    println!();
    println!(
        "### B3 {} ({label}) — stripped at tick 0 ({:.4} m of foliage to litter), no animals",
        class.name(),
        moved
    );
    println!(
        "{:>7} {:>8} {:>8} {:>8} {:>8} {:>9}",
        "tick", "P", "W", "Q", "N", "P/P*"
    );
    for tick in 0..=HORIZON {
        let p = world.state.fields.p[i];
        let q = world.state.ecology.plant_reserve[i];
        q_min = q_min.min(q);
        n_min = n_min.min(world.state.fields.n[i]);
        if half.is_none() && p >= target_half {
            half = Some(tick);
        }
        if nine.is_none() && p >= target_nine {
            nine = Some(tick);
        }
        if tick % (SAMPLE * 3) == 0 {
            println!(
                "{tick:>7} {p:>8.4} {:>8.4} {q:>8.4} {:>8.4} {:>9.3}",
                world.state.ecology.wood[i],
                world.state.fields.n[i],
                p / stand.p
            );
        }
        if tick == HORIZON {
            break;
        }
        world.step();
    }
    world.check_invariants().expect("B3 ends consistent");
    println!(
        "- reached 0.5·P* ({target_half:.4}) at {}; 0.9·P* ({target_nine:.4}) at {}.",
        censored(half),
        censored(nine)
    );
    println!(
        "- reserve dipped to {q_min:.4} of the painted {:.4} and now holds {:.4}; `N` bottomed \
         at {n_min:.4} from {n0:.4}.",
        stand.q,
        world.state.ecology.plant_reserve[i]
    );
}

fn b3() {
    println!("# B3 — plant recovery after defoliation (§13.2)");
    println!(
        "Expected direction: reflush from `Q` first (a visible fall in the reserve), then \
         income-limited. Bright reaches 0.9·P* in ~10 min; average is expected to sit near \
         breakeven and be **reported censored**."
    );
    println!(
        "**Two arms per class.** The recovery §13.2 describes is paid out of `Q`, and B0's \
         measured reserve is far below the §11 hand table's. Both stands are run so the \
         difference is measured rather than assumed."
    );
    for class in [Class::Bright, Class::Average] {
        run_b3(class, mature(class), "B0 measured");
        run_b3(class, Stand::hand(class), "§11 hand table");
    }
}

// ------------------------------------------------------------------------------ B4a

/// B4a's terminal world, so B4b can start from the stand it actually killed.
struct Killed {
    dead_wood: f64,
    q_zero: Option<u64>,
    dieback: Option<u64>,
    death: Option<u64>,
    grazers: Vec<Option<u64>>,
}

fn run_b4a(verbose: bool) -> Killed {
    let stand = mature(Class::Bright);
    let mut cfg = class_config(Class::Bright);
    pinned(&mut cfg);
    hungry_gate(&mut cfg);
    // Isolated: §4.8 needs a **living** donor, and the fixture has none, so the stand it kills
    // stays dead. Painting one cell in an otherwise bare world is exactly that isolation.
    let mut world = World::new(cfg).expect("valid");
    nutrient_everywhere(&mut world);
    let cell = CellId::new(Face::Top, 8, 8);
    paint(&mut world, cell, stand);
    let ids: Vec<OrganismId> = (0..3).map(|k| place(&mut world, cell, 0.85, k)).collect();
    let mut world = restage(world);

    let i = cell.index();
    let mut q_zero = None;
    let mut dieback = None;
    let mut death = None;
    let mut grazers = vec![None; ids.len()];
    if verbose {
        println!();
        println!(
            "{:>7} {:>8} {:>8} {:>8} {:>8} {:>9} {:>7}",
            "tick", "P", "W", "Q", "Wd", "Σreserve", "alive"
        );
    }
    for tick in 0..=HORIZON {
        if verbose && tick % SAMPLE == 0 {
            let reserve: f64 = ids
                .iter()
                .filter_map(|id| world.state.organisms.get(*id))
                .map(|o| o.reserve)
                .sum();
            let alive = ids
                .iter()
                .filter(|id| world.state.organisms.get(**id).is_some())
                .count();
            println!(
                "{tick:>7} {:>8.4} {:>8.4} {:>8.4} {:>8.4} {reserve:>9.4} {alive:>7}",
                world.state.fields.p[i],
                world.state.ecology.wood[i],
                world.state.ecology.plant_reserve[i],
                world.state.ecology.dead_wood[i],
            );
        }
        if q_zero.is_none() && world.state.ecology.plant_reserve[i] <= 1e-12 {
            q_zero = Some(tick);
        }
        if dieback.is_none() && world.state.ecology.dead_wood[i] > 1e-12 {
            dieback = Some(tick);
        }
        if death.is_none() && world.state.ecology.plant_deaths_total > 0 {
            death = Some(tick);
        }
        if tick == HORIZON {
            break;
        }
        world.step();
        world.drain_events();
        for (k, id) in ids.iter().enumerate() {
            if grazers[k].is_none() && world.state.organisms.get(*id).is_none() {
                grazers[k] = Some(tick + 1);
            }
        }
    }
    world.check_invariants().expect("B4a ends consistent");
    let out = Killed {
        dead_wood: world.state.ecology.dead_wood[i],
        q_zero,
        dieback,
        death,
        grazers,
    };
    if verbose {
        println!(
            "- `Q` reached zero at {}; dieback opened at {}; the stand died at {}.",
            censored(out.q_zero),
            censored(out.dieback),
            censored(out.death)
        );
        println!(
            "- dead wood left standing: {:.4} m. Living `W` {:.4}, `P` {:.4}.",
            out.dead_wood,
            world.state.ecology.wood[i],
            world.state.fields.p[i]
        );
        let fates: Vec<String> = out.grazers.iter().map(|d| censored(*d)).collect();
        println!("- grazers starved at: {}.", fates.join(", "));
        println!(
            "Expected direction (§13.2): `Q → 0`, `W → Wd`, no regrowth (there is no donor), and \
             the grazers starve."
        );
    }
    out
}

fn b4a() {
    println!("# B4a — three pinned grazers on one isolated bright mature stand (§13.2)");
    run_b4a(true);
}

// ------------------------------------------------------------------------------ B4b

fn run_b4b(killed: &Killed, stand: Stand, label: &str) {
    println!();
    println!("### B4b — ring painted from {label}");
    let mut cfg = class_config(Class::Bright);
    let mut world = World::new(cfg.clone()).expect("valid");
    nutrient_everywhere(&mut world);
    let centre = CellId::new(Face::Top, 8, 8);
    let ring: Vec<CellId> = region(centre, 1).into_iter().filter(|c| *c != centre).collect();
    for c in &ring {
        paint(&mut world, *c, stand);
    }
    // The killed stand's remains: its dead wood, and nothing living.
    let before = material(&world);
    world.state.ecology.dead_wood[centre.index()] = killed.dead_wood;
    book(&mut world, before);
    let mut world = restage(world);
    let _ = &mut cfg;

    let i = centre.index();
    println!(
        "centre opens bare with {:.4} m of B4a's dead wood; the ring is {} bright mature stands.",
        killed.dead_wood,
        ring.len()
    );
    println!();
    println!(
        "{:>7} {:>8} {:>8} {:>8} {:>8} {:>10}",
        "tick", "W", "P", "Q", "Wd", "class"
    );
    let mut established = None;
    for tick in 0..=HORIZON {
        if tick % (SAMPLE * 3) == 0 {
            let w = world.state.ecology.wood[i];
            println!(
                "{tick:>7} {w:>8.4} {:>8.4} {:>8.4} {:>8.4} {:>10}",
                world.state.fields.p[i],
                world.state.ecology.plant_reserve[i],
                world.state.ecology.dead_wood[i],
                match CellClass::of(w, world.config().plant.alive_min) {
                    CellClass::Bare => "bare",
                    CellClass::Establishing => "establishing",
                    CellClass::Alive => "alive",
                }
            );
        }
        if established.is_none() && world.state.ecology.recolonisations_total > 0 {
            established = Some(tick);
        }
        if tick == HORIZON {
            break;
        }
        world.step();
    }
    world.check_invariants().expect("B4b ends consistent");
    let w = world.state.ecology.wood[i];
    let p = world.state.fields.p[i];
    println!();
    println!("- established (crossed `W_min`) at {}.", censored(established));
    println!(
        "- at the horizon: W {:.4} = {:.1}% of the B0 bright value {:.4}; P {:.4} = {:.1}% of \
         {:.4}. Rebuilding beyond that is **censored at 30 min**.",
        w,
        100.0 * w / stand.w,
        stand.w,
        p,
        100.0 * p / stand.p,
        stand.p
    );
}

fn b4b() {
    println!("# B4b — recovery after death: a killed cell inside a living ring of eight (§13.2)");
    println!(
        "Expected direction (§13.2): establishes within a minute with eight donors; rebuilding \
         is reported censored at the horizon."
    );
    println!(
        "**Two arms, and the difference between them is a finding.** §13 defines a \"mature \
         stand\" as the §11 steady-state values, and then says later scenarios paint *B0's \
         measured* values instead. Under §11's provisional numbers those are not the same stand: \
         B0's measured reserve sits below `q_prop · Q_max`, so a B0 stand is not a **donor** at \
         all and §4.8 never fires. Both arms are run and reported."
    );
    let killed = run_b4a(false);
    run_b4b(&killed, mature(Class::Bright), "B0's measured bright stand");
    run_b4b(&killed, Stand::hand(Class::Bright), "the §11 hand table's bright stand");
}

// ------------------------------------------------------------------------------- B5

fn b5() {
    println!("# B5 — dietary exclusion (§13.2)");
    println!(
        "Conversions are frozen (`m_p = ripen = drop = k_d = k_c = k_w = fall = 0`) so a cell's \
         food keeps its identity for the whole run, and every body is pinned. This tests \
         **dependence**, not desirability."
    );
    let kinds: [(&str, f32); 4] =
        [("burrower", 0.10), ("grazer", 0.85), ("glider", 0.90), ("skimmer", 0.60)];
    let foods = ["a: foliage", "b: charged litter", "c: a placed carcass"];

    println!();
    println!(
        "{:<10} {:>8} {:>8} | {:<26} {:<26} {:<26}",
        "kind", "cap_h", "cap_d", foods[0], foods[1], foods[2]
    );
    for (name, diet) in kinds {
        let _ = diet;
        let mut cells = Vec::new();
        let (mut caps, mut cells_out) = ((0.0, 0.0), Vec::new());
        for (food, _) in foods.iter().enumerate() {
            let mut cfg = class_config(Class::Bright);
            pinned(&mut cfg);
            hungry_gate(&mut cfg);
            cfg.producer.growth = 0.0;
            cfg.producer.mortality = 0.0;
            cfg.fruit.ripen = 0.0;
            cfg.fruit.drop = 0.0;
            cfg.detritus.decomposition = 0.0;
            cfg.detritus.carrion_decomposition = 0.0;
            cfg.detritus.wood_decomposition = 0.0;
            cfg.detritus.fall = 0.0;
            cfg.plant.propagule_rate = 0.0;
            let mut world = World::new(cfg).expect("valid");
            nutrient_everywhere(&mut world);
            let cell = CellId::new(Face::Top, 8, 8);
            let before = material(&world);
            match food {
                0 => {
                    world.state.fields.p[cell.index()] = 1.0;
                    world.state.ecology.wood[cell.index()] = 0.6;
                }
                1 => {
                    world.state.fields.d[cell.index()] = 2.0;
                    world.state.fields.de[cell.index()] = 2.0 * 2.0;
                }
                _ => {
                    // A placed carcass, booked as external material exactly as a founder is.
                    world.state.ecology.carrion[cell.index()] = 2.0;
                    world.state.ecology.carrion_energy[cell.index()] = 2.0 * 2.0;
                }
            }
            book(&mut world, before);
            let id = place_kind(&mut world, cell, name, 0);
            let mut world = restage(world);
            caps = {
                let p = &world.state.organisms.get(id).expect("alive").phenotype;
                (p.cap_foliage, p.cap_detrital)
            };

            let mut died = None;
            for tick in 0..HORIZON {
                world.step();
                world.drain_events();
                if world.state.organisms.get(id).is_none() {
                    died = Some(tick + 1);
                    break;
                }
            }
            let diag = world.intake_diagnostics();
            let reserve = world.state.organisms.get(id).map_or(0.0, |o| o.reserve);
            let served = match food {
                0 => diag.producer_eaten + diag.fruit_eaten,
                1 => diag.litter_eaten,
                _ => diag.carrion_eaten,
            };
            cells.push(format!(
                "{:<12} R {:.3} ate {:.3}",
                died.map_or("survived".to_string(), |t| format!("died {t}")),
                reserve,
                served
            ));
            cells_out.push(served);
        }
        println!(
            "{name:<10} {:>8.2} {:>8.2} | {:<26} {:<26} {:<26}",
            caps.0, caps.1, cells[0], cells[1], cells[2]
        );
        let _ = cells_out;
    }
    println!();
    println!(
        "Expected direction (§13.2): grazer and glider starve on (b) and (c); the burrower \
         starves on (a); the skimmer lives on all three at lower intake."
    );
}

// ------------------------------------------------------------------------------ B6

fn run_b6(renewal: bool, half: i32, label: &str) {
    let stand = mature(Class::Bright);
    let mut cfg = class_config(Class::Bright);
    hungry_gate(&mut cfg);
    if !renewal {
        cfg.producer.growth = 0.0;
        cfg.fruit.ripen = 0.0;
        cfg.producer.mortality = 0.0;
    }
    let mut world = World::new(cfg).expect("valid");
    nutrient_everywhere(&mut world);
    let centre = CellId::new(Face::Top, 8, 8);
    let cells = region(centre, half);
    for c in &cells {
        paint(&mut world, *c, stand);
    }
    place(&mut world, centre, 0.85, 0);
    place(&mut world, centre, 0.85, 1);
    let mut world = restage(world);

    let opening_u = world.state.net_energy_in_corrected();
    let mut peak = world.population();
    let mut extinct = None;
    let mut births = 0u64;
    println!();
    println!(
        "### {label} — {} × {} region ({} cells), renewal {}",
        2 * half + 1,
        2 * half + 1,
        cells.len(),
        if renewal { "on" } else { "off" }
    );
    println!(
        "{:>7} {:>6} {:>8} {:>8} {:>10} {:>10} {:>10}",
        "tick", "pop", "births", "deaths", "Σ P", "eaten", "escrows"
    );
    for tick in 0..=HORIZON {
        if tick % (SAMPLE * 4) == 0 {
            let p_total: f64 = cells.iter().map(|c| world.state.fields.p[c.index()]).sum();
            let diag = world.intake_diagnostics();
            let escrows = world
                .state
                .organisms
                .iter()
                .filter(|(_, o)| o.escrow.is_some())
                .count();
            println!(
                "{tick:>7} {:>6} {:>8} {:>8} {p_total:>10.4} {:>10.4} {escrows:>10}",
                world.population(),
                world.state.births_total,
                world.state.deaths_total.iter().sum::<u64>(),
                diag.producer_eaten + diag.fruit_eaten + diag.litter_eaten + diag.carrion_eaten,
            );
        }
        if tick == HORIZON {
            break;
        }
        world.step();
        world.drain_events();
        peak = peak.max(world.population());
        births = world.state.births_total;
        if extinct.is_none() && world.population() == 0 {
            extinct = Some(tick + 1);
        }
    }
    world.check_invariants().expect("B6 ends consistent");
    let diag = world.intake_diagnostics();
    println!(
        "- births {births}, deaths {:?}, peak population {peak}, extinct at {}.",
        world.state.deaths_total,
        censored(extinct)
    );
    println!(
        "- intake: leaf {:.4} m, fruit {:.4} m, litter {:.4} m, remains {:.4} m; plant income \
         {:.4} m.",
        diag.producer_eaten,
        diag.fruit_eaten,
        diag.litter_eaten,
        diag.carrion_eaten,
        diag.plant_income
    );
    let net = world.state.net_energy_in_corrected() - opening_u;
    println!(
        "- cumulative `light_in − heat_out` over the run: {net:+.4} e{}",
        if renewal { "" } else { " — with renewal off this must not be positive" }
    );
    if renewal {
        println!(
            "- §11 comparison: {} × 6.4e-4 = {:.4e} m/s of production against 1.4e-2 m/s for two \
             cruising grazers — a ratio of {:.2}.",
            cells.len(),
            cells.len() as f64 * 6.4e-4,
            cells.len() as f64 * 6.4e-4 / 1.4e-2
        );
    }
}

fn b6a() {
    println!("# B6a — reproduction on a finite input: 3 × 3 bright region, renewal off (§13.2)");
    println!(
        "Expected direction: births while the stock lasts, then starvation to zero; no birth \
         without an escrow debit; `U` non-increasing."
    );
    run_b6(false, 1, "B6a");
}

fn b6b() {
    println!("# B6b — reproduction on a renewing patch: 7 × 7 bright region (§13.2)");
    println!(
        "Expected direction: 49 × 6.4e-4 ≈ 0.031 m/s against 0.014 for two cruising grazers \
         leaves a surplus, so **some** births are expected; whether starvation follows is \
         measured, not assumed. §13.2 marks B6b unresolved by design."
    );
    run_b6(true, 3, "B6b");
}

// ------------------------------------------------------------------------------- B7

fn run_b7(dim: bool, stand: Stand, label: &str) {
    // §13.2: the donor is a bright mature stand; the bare cell is either bright or at
    // `L_eff · μ = 0.2`. Light and moisture are flat per world, so the dim arm runs the whole
    // world at the dim class and paints the donor with the bright stand it measured.
    let (light, moisture) = if dim { (0.4, 0.5) } else { (Class::Bright.light(), Class::Bright.moisture()) };
    let mut cfg = staged(light, moisture);
    cfg.plant.propagule_rate = WorldConfig::default().plant.propagule_rate;
    let mut world = World::new(cfg).expect("valid");
    nutrient_everywhere(&mut world);
    let donor = CellId::new(Face::Top, 8, 8);
    let bare = CellId::new(Face::Top, 8, 9);
    paint(&mut world, donor, stand);
    let mut world = restage(world);

    let i = bare.index();
    println!();
    println!(
        "### B7 {} — one bright mature donor ({label}) beside one bare cell (L·μ = {:.2})",
        if dim { "dim recipient" } else { "bright recipient" },
        light * moisture
    );
    println!(
        "{:>7} {:>8} {:>8} {:>8} {:>10} {:>12}",
        "tick", "W", "P", "Q", "A (m/s)", "class"
    );
    let mut established = None;
    let mut last_income = 0.0;
    let mut last_tick = 0u64;
    let mut died_back = false;
    for tick in 0..=HORIZON {
        if tick % SAMPLE == 0 {
            let w = world.state.ecology.wood[i];
            let diag = world.intake_diagnostics();
            let seconds = (tick - last_tick).max(1) as f64 * DT;
            let income = (diag.plant_income - last_income) / seconds;
            println!(
                "{tick:>7} {w:>8.4} {:>8.4} {:>8.4} {income:>10.3e} {:>12}",
                world.state.fields.p[i],
                world.state.ecology.plant_reserve[i],
                match CellClass::of(w, world.config().plant.alive_min) {
                    CellClass::Bare => "bare",
                    CellClass::Establishing => "establishing",
                    CellClass::Alive => "alive",
                }
            );
            last_income = diag.plant_income;
            last_tick = tick;
        }
        if established.is_none() && world.state.ecology.recolonisations_total > 0 {
            established = Some(tick);
        }
        if !died_back && world.state.ecology.dead_wood[i] > 1e-12 {
            died_back = true;
        }
        if tick == HORIZON {
            break;
        }
        world.step();
    }
    world.check_invariants().expect("B7 ends consistent");
    println!(
        "- established at {}; dieback in the recipient: {}; stand deaths in the world: {}.",
        censored(established),
        if died_back { "yes" } else { "no" },
        world.state.ecology.plant_deaths_total
    );
    println!(
        "- recipient at the horizon: W {:.4} P {:.4} Q {:.4}; donor: W {:.4} P {:.4} Q {:.4}.",
        world.state.ecology.wood[i],
        world.state.fields.p[i],
        world.state.ecology.plant_reserve[i],
        world.state.ecology.wood[donor.index()],
        world.state.fields.p[donor.index()],
        world.state.ecology.plant_reserve[donor.index()],
    );
}

fn b7() {
    println!("# B7 — establishment (§13.2)");
    println!(
        "Expected direction: the bright cell establishes (~5 min with one donor, §11) and shows \
         positive, growing foliage and income; the dim cell establishes and is expected to die \
         back. Both are reported."
    );
    println!(
        "**Two arms per light class.** §13 calls a \"mature stand\" the §11 steady state and \
         then says later scenarios paint B0's measured values; under §11's provisional numbers \
         those differ in the one stock §4.8 reads, the reserve. Both are run."
    );
    for (stand, label) in [
        (mature(Class::Bright), "B0 measured"),
        (Stand::hand(Class::Bright), "§11 hand table"),
    ] {
        run_b7(false, stand, label);
        run_b7(true, stand, label);
    }
}

// ------------------------------------------------------------------------------ main

fn main() {
    let started = std::time::Instant::now();
    let which: Vec<String> = std::env::args().skip(1).collect();
    if which.is_empty() {
        eprintln!(
            "usage: ecology_v1_scenarios <b0|b1a|b1b|b2|b3|b4a|b4b|b5|b6a|b6b|b7|all> ...\n\
             Run b0 first: its measured terminal states are what the later scenarios paint."
        );
        std::process::exit(2);
    }
    let all = ["b0", "b1a", "b1b", "b2", "b3", "b4a", "b4b", "b5", "b6a", "b6b", "b7"];
    let names: Vec<String> = if which.iter().any(|w| w == "all") {
        all.iter().map(|s| (*s).to_string()).collect()
    } else {
        which
    };
    for name in &names {
        match name.as_str() {
            "b0" => b0(),
            "b1a" => b1a(),
            "b1b" => b1b(),
            "b2" => b2(),
            "b3" => b3(),
            "b4a" => b4a(),
            "b4b" => b4b(),
            "b5" => b5(),
            "b6a" => b6a(),
            "b6b" => b6b(),
            "b7" => b7(),
            other => {
                eprintln!("unknown scenario {other}");
                std::process::exit(2);
            }
        }
    }
    println!();
    println!("# wall time {:.1} s", started.elapsed().as_secs_f64());
}
