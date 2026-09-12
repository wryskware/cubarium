//! Independent verification of the 2026-09-12 redesign slices: the stratified cube's
//! detritus fall, `design/water.md`, `design/fauna-v2.md`, and the controller v2 rules of
//! `design/m2-world-spec.md`.
//!
//! Every expectation below is transcribed from those documents, never from the
//! implementation. Where a rule is a formula the test recomputes the formula and compares;
//! where a rule is a prohibition ("nothing moves upward", "a v1 genome is stamped v2") the
//! test builds the situation the prohibition is about and checks the world never does it.
//!
//! Run in release (`cargo test -p cubarium-core --release --test redesign_rules`): several
//! fixtures write fields and water directly, and keep the world's own audit counters honest
//! by hand so the debug-build assertions would also pass.

mod common;

use cubarium_core::config::{FounderKind, WorldConfig};
use cubarium_core::genome::{self, FORM_UNSET, Genome, MAX_FORMS};
use cubarium_core::ids::OrganismId;
use cubarium_core::organism::Mode;
use cubarium_core::{DT, World};
use cubarium_surface::{CELL_COUNT, CellId, Face, SurfacePoint, Vec2};

// ---------------------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------------------

/// A world in which nothing happens on its own: no growth, no mortality, no decomposition,
/// no ripening, no drop, no fall, no diffusion, no weather, no rain, no flow, no
/// evaporation, and nobody alive. Each test switches back on exactly the one process it is
/// about, so a change it observes can only have come from that process.
fn quiet_config() -> WorldConfig {
    let mut cfg = WorldConfig::default();
    cfg.producer.growth = 0.0;
    cfg.producer.mortality = 0.0;
    cfg.producer.initial_fraction = 0.0;
    cfg.detritus.decomposition = 0.0;
    cfg.detritus.fall = 0.0;
    cfg.detritus.initial_dark = 0.0;
    cfg.nutrient.diffusion = 0.0;
    cfg.nutrient.initial = 0.0;
    cfg.fruit.ripen = 0.0;
    cfg.fruit.drop = 0.0;
    cfg.water.rain_rate = 0.0;
    cfg.water.flow = 0.0;
    cfg.water.evap = 0.0;
    cfg.weather.amplitude = 0.0;
    cfg.founders.count = 0;
    cfg.founders.kinds = Vec::new();
    cfg
}

/// [`quiet_config`] plus a habitat that is the same everywhere, so the light and moisture a
/// formula needs are the two numbers passed in rather than a noise field a test would have
/// to re-derive.
fn flat_habitat(cfg: &mut WorldConfig, light: f64, moisture: f64) {
    cfg.habitat.light_base = light;
    cfg.habitat.light_height_gain = 0.0;
    cfg.habitat.light_noise_gain = 0.0;
    cfg.habitat.moisture_base = moisture;
    cfg.habitat.moisture_height_gain = 0.0;
    cfg.habitat.moisture_noise_gain = 0.0;
    cfg.weather.amplitude = 0.0;
}

/// Silence every steering term except the food gradients and the depth drive, so a test can
/// attribute a turn to the one term it left on.
fn deterministic_steering(cfg: &mut WorldConfig) {
    cfg.drives.turn_noise = 0.0;
    cfg.drives.w_persist = 0.0;
}

/// One founder of a kind that fixes the genome loci a test cares about. Everything else
/// takes the v1 founder value.
fn probe_kind(diet: f32, depth: f32, speed: f32, size: f32, swim: f32, form: u8) -> FounderKind {
    FounderKind {
        name: "probe".to_string(),
        count: 1,
        diet: Some(diet),
        depth: Some(depth),
        speed: Some(speed),
        size: Some(size),
        metabolism: Some(1.0),
        swim: Some(swim),
        hue: Some(0.5),
        form: Some(form),
    }
}

fn sole_id(world: &World) -> OrganismId {
    let ids: Vec<OrganismId> = world.state.organisms.iter().map(|(id, _)| id).collect();
    assert_eq!(ids.len(), 1, "fixture expects exactly one organism");
    ids[0]
}

/// Uniform per-cell material, with the world's external-material ledger corrected so the
/// mass invariant still reads true after the injection.
fn set_fields(world: &mut World, n: f64, p: f64, d: f64, de: f64, f: f64) {
    let before: f64 = common::total_material(world);
    let fields = &mut world.state.fields;
    for c in 0..CELL_COUNT {
        fields.n[c] = n;
        fields.p[c] = p;
        fields.d[c] = d;
        fields.de[c] = de;
        fields.f[c] = f;
    }
    let after: f64 = common::total_material(world);
    world.state.external_material_in += after - before;
}

/// Uniform water, with the rain ledger corrected the same way so `Σw = rain_in − evap_out`
/// still holds after the injection.
fn set_water(world: &mut World, depth: f64) {
    let before: f64 = world.state.fields.w.iter().sum();
    for c in 0..CELL_COUNT {
        world.state.fields.w[c] = depth;
    }
    let after: f64 = world.state.fields.w.iter().sum();
    world.state.rain_in_total += after - before;
}

fn set_cell_water(world: &mut World, cell: CellId, depth: f64) {
    let before = world.state.fields.w[cell.index()];
    world.state.fields.w[cell.index()] = depth;
    world.state.rain_in_total += depth - before;
}

/// Embedded height of a cell center (Top = 1, rim = −1), the `h` of the design documents.
fn cell_height(c: CellId) -> f64 {
    c.center().embed()[1]
}

/// A deterministic pseudo-random stream, so conservation is not checked on a suspiciously
/// smooth field.
fn splitmix(state: &mut u64) -> f64 {
    *state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let mut z = *state;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^= z >> 31;
    (z >> 11) as f64 / (1u64 << 53) as f64
}

/// A ragged litter field: `D` pseudo-random, `De` a pseudo-random fraction of its cap, so
/// the two move in different proportions from cell to cell.
fn ragged_litter(world: &mut World, seed: u64) {
    let cap = world.state.config.detritus.energy_cap;
    let before: f64 = common::total_material(world);
    let mut s = seed;
    for c in 0..CELL_COUNT {
        let d = 0.05 + 2.0 * splitmix(&mut s);
        let de = cap * d * splitmix(&mut s);
        world.state.fields.d[c] = d;
        world.state.fields.de[c] = de;
    }
    let after: f64 = common::total_material(world);
    world.state.external_material_in += after - before;
}

// ---------------------------------------------------------------------------------------
// Detritus fall (`design/stratified-world.md` mechanism 2, spec "Conversion table")
// ---------------------------------------------------------------------------------------

#[test]
fn detritus_fall_conserves_total_detritus_and_its_energy() {
    let mut cfg = quiet_config();
    cfg.detritus.fall = 0.02;
    let mut world = World::new(cfg).expect("config");
    ragged_litter(&mut world, 0x5EED_0001);

    let d0: f64 = world.state.fields.d.iter().sum();
    let de0: f64 = world.state.fields.de.iter().sum();
    assert!(d0 > 100.0, "fixture should hold real litter, got {d0}");

    for tick in 1..=600 {
        world.step();
        let d: f64 = world.state.fields.d.iter().sum();
        let de: f64 = world.state.fields.de.iter().sum();
        assert!(
            (d - d0).abs() <= 1e-13 * d0,
            "tick {tick}: ΣD drifted from {d0} to {d} (Δ = {})",
            d - d0
        );
        assert!(
            (de - de0).abs() <= 1e-13 * de0,
            "tick {tick}: ΣDe drifted from {de0} to {de} (Δ = {})",
            de - de0
        );
    }
}

#[test]
fn detritus_fall_matches_the_spec_downhill_transfer() {
    let mut cfg = quiet_config();
    cfg.detritus.fall = 0.02;
    let fraction = cfg.detritus.fall * DT;
    let mut world = World::new(cfg).expect("config");
    ragged_litter(&mut world, 0x5EED_0002);

    let neighbors = world.cell_neighbors();
    let d_before = world.state.fields.d.clone();
    let de_before = world.state.fields.de.clone();

    // `design/m2-world-spec.md`: "downhill = the graph neighbor with the lowest embedded y
    // if strictly lower; none on the top face or the bottom row". Derived here, not read
    // from the world.
    let mut want_d = d_before.clone();
    let mut want_de = de_before.clone();
    for c in 0..CELL_COUNT {
        let Some(down) = spec_downhill(&neighbors, c) else { continue };
        want_d[c] -= fraction * d_before[c];
        want_de[c] -= fraction * de_before[c];
        want_d[down] += fraction * d_before[c];
        want_de[down] += fraction * de_before[c];
    }

    world.step();

    for c in 0..CELL_COUNT {
        let tol = 1e-12 * want_d[c].abs().max(1.0);
        assert!(
            (world.state.fields.d[c] - want_d[c]).abs() <= tol,
            "cell {c}: D is {} but the spec's fall gives {}",
            world.state.fields.d[c],
            want_d[c]
        );
        let tol = 1e-12 * want_de[c].abs().max(1.0);
        assert!(
            (world.state.fields.de[c] - want_de[c]).abs() <= tol,
            "cell {c}: De is {} but the spec's fall gives {}",
            world.state.fields.de[c],
            want_de[c]
        );
    }
}

/// The design's downhill rule, transcribed: the graph neighbour with the lowest embedded
/// height when that height is strictly lower, and nothing at all on the top face.
fn spec_downhill(neighbors: &[[Option<u16>; 4]], c: usize) -> Option<usize> {
    let cell = CellId(c as u16);
    if cell.face() == Face::Top {
        return None;
    }
    let own = cell_height(cell);
    let mut best: Option<(usize, f64)> = None;
    for n in neighbors[c].iter().flatten() {
        let y = cell_height(CellId(*n));
        if best.is_none_or(|(_, by)| y < by) {
            best = Some((*n as usize, y));
        }
    }
    match best {
        Some((idx, y)) if y < own - 1e-9 => Some(idx),
        _ => None,
    }
}

#[test]
fn detritus_never_moves_upward_or_onto_the_top_face() {
    let mut cfg = quiet_config();
    cfg.detritus.fall = 0.02;
    let mut world = World::new(cfg).expect("config");
    world.state.external_material_in += 1.0;

    // One cell at a time holds the world's whole litter, so every arrival after one tick is
    // an individual transfer out of that cell and can be named. (A net-delta sweep over a
    // full field cannot do this: a cell above may receive more than it sheds.)
    let neighbours = world.cell_neighbors();
    for (source, outgoing) in neighbours.iter().enumerate() {
        for c in 0..CELL_COUNT {
            world.state.fields.d[c] = 0.0;
            world.state.fields.de[c] = 0.0;
        }
        world.state.fields.d[source] = 1.0;
        world.state.fields.de[source] = 1.0;

        world.step();

        let cell = CellId(source as u16);
        let own_h = cell_height(cell);
        let total: f64 = world.state.fields.d.iter().sum();
        assert!((total - 1.0).abs() <= 1e-12, "source {source}: ΣD became {total}");

        for c in 0..CELL_COUNT {
            let got = world.state.fields.d[c];
            if c == source || got == 0.0 {
                continue;
            }
            let target = CellId(c as u16);
            assert!(
                outgoing.iter().flatten().any(|n| *n as usize == c),
                "source {source} sent detritus to {c}, which is not one of its graph neighbours"
            );
            assert_ne!(target.face(), Face::Top, "source {source} sent detritus onto the top face (cell {c})");
            assert!(
                cell_height(target) < own_h - 1e-9,
                "source {source} (h = {own_h}) sent {got} upward to cell {c} (h = {})",
                cell_height(target)
            );
        }

        if cell.face() == Face::Top || cell.cy() == 15 {
            assert_eq!(
                world.state.fields.d[source], 1.0,
                "cell {source} has no downhill neighbour (top face or bottom row) and must keep its litter"
            );
        }
    }
}

#[test]
fn falling_detritus_never_raises_the_litters_mean_height() {
    let mut cfg = quiet_config();
    cfg.detritus.fall = 0.02;
    let mut world = World::new(cfg).expect("config");
    ragged_litter(&mut world, 0x5EED_0003);

    let top_cells: Vec<usize> = (0..CELL_COUNT).filter(|c| CellId(*c as u16).face() == Face::Top).collect();
    let top_before: f64 = top_cells.iter().map(|c| world.state.fields.d[*c]).sum();
    let moment = |d: &Vec<f64>| -> f64 { d.iter().enumerate().map(|(c, x)| x * cell_height(CellId(c as u16))).sum() };
    let mut previous = moment(&world.state.fields.d);
    let start = previous;

    for tick in 1..=200u32 {
        world.step();
        let now = moment(&world.state.fields.d);
        assert!(now <= previous + 1e-12, "tick {tick}: Σ D·h rose from {previous} to {now}");
        previous = now;
    }
    assert!(previous < start - 1e-6, "the litter should actually have settled downward: {start} → {previous}");

    let top_after: f64 = top_cells.iter().map(|c| world.state.fields.d[*c]).sum();
    assert!(
        (top_after - top_before).abs() <= 1e-12 * top_before.max(1.0),
        "the canopy's detritus changed from {top_before} to {top_after}: nothing may fall onto or off the top face"
    );
}

#[test]
fn a_side_face_column_empties_into_the_bottom_row() {
    let mut cfg = quiet_config();
    // `fall · DT ≤ 1` is the configuration bound, so at the bound a cell hands its whole
    // litter to its downhill neighbour each tick and the column drains one row per tick.
    cfg.detritus.fall = 1.0 / DT;
    let mut world = World::new(cfg).expect("config");

    let top = CellId::new(Face::Front, 8, 0);
    let floor = CellId::new(Face::Front, 8, 15);
    let before: f64 = common::total_material(&world);
    world.state.fields.d[top.index()] = 1.0;
    world.state.fields.de[top.index()] = 1.0;
    let after: f64 = common::total_material(&world);
    world.state.external_material_in += after - before;

    for _ in 0..15 {
        world.step();
    }

    assert_eq!(world.state.fields.d[floor.index()], 1.0, "the column should have landed whole on the bottom row");
    assert_eq!(world.state.fields.de[floor.index()], 1.0, "its energy travels in the same proportion");
    for c in 0..CELL_COUNT {
        if c == floor.index() {
            continue;
        }
        assert_eq!(world.state.fields.d[c], 0.0, "cell {c} still holds detritus after the column drained");
    }

    // The bottom row has no downhill neighbour: the litter stays there.
    for _ in 0..60 {
        world.step();
    }
    assert_eq!(world.state.fields.d[floor.index()], 1.0, "the bottom row must keep what reaches it");
}

#[test]
fn a_fall_of_zero_leaves_the_fields_bit_identical() {
    let mut cfg = quiet_config();
    cfg.detritus.fall = 0.0;
    let mut world = World::new(cfg).expect("config");
    ragged_litter(&mut world, 0x5EED_0004);

    let d0 = world.state.fields.d.clone();
    let de0 = world.state.fields.de.clone();

    for _ in 0..500 {
        world.step();
    }

    for c in 0..CELL_COUNT {
        assert_eq!(world.state.fields.d[c].to_bits(), d0[c].to_bits(), "cell {c}: D moved with fall = 0");
        assert_eq!(world.state.fields.de[c].to_bits(), de0[c].to_bits(), "cell {c}: De moved with fall = 0");
    }

    // The control: the same fixture with the default fall does move, so the equality above
    // is the `fall = 0` rule and not a fall step that never runs.
    let mut cfg = quiet_config();
    cfg.detritus.fall = 0.02;
    let mut moving = World::new(cfg).expect("config");
    ragged_litter(&mut moving, 0x5EED_0004);
    moving.step();
    assert!(
        (0..CELL_COUNT).any(|c| moving.state.fields.d[c].to_bits() != d0[c].to_bits()),
        "control: with fall = 0.02 the same litter must move"
    );
}

// ---------------------------------------------------------------------------------------
// Water (`design/water.md`)
// ---------------------------------------------------------------------------------------

#[test]
fn the_water_budget_closes_over_two_thousand_default_ticks() {
    let mut world = World::new(WorldConfig::default()).expect("config");
    for tick in 1..=2000u32 {
        world.step();
        let sum: f64 = world.state.fields.w.iter().sum();
        let ledger = world.state.rain_in_total - world.state.evap_out_total;
        let tol = 1e-11 * sum.abs().max(1.0);
        assert!(
            (sum - ledger).abs() <= tol,
            "tick {tick}: Σw = {sum} but rain_in − evap_out = {ledger} (residual {})",
            sum - ledger
        );
        assert!(
            (world.water_residual() - (sum - ledger)).abs() <= tol,
            "tick {tick}: the world's own residual {} disagrees with the recomputed one {}",
            world.water_residual(),
            sum - ledger
        );
    }
    let rain = world.state.rain_in_total;
    assert!(rain > 0.0, "a default world must actually rain somewhere in 100 s, got rain_in_total = {rain}");
}

#[test]
fn water_is_never_negative_in_a_default_world() {
    let mut world = World::new(WorldConfig::default()).expect("config");
    for tick in 1..=2000u32 {
        world.step();
        for (c, w) in world.state.fields.w.iter().enumerate() {
            assert!(w.is_finite() && *w >= 0.0, "tick {tick}: cell {c} holds w = {w}");
        }
    }
}

#[test]
fn without_rain_the_surface_stays_dry_forever() {
    let mut cfg = WorldConfig::default();
    cfg.water.rain_rate = 0.0;
    let mut world = World::new(cfg).expect("config");

    for tick in 1..=2000u32 {
        world.step();
        for (c, w) in world.state.fields.w.iter().enumerate() {
            assert_eq!(*w, 0.0, "tick {tick}: cell {c} (face {:?}) got wet with rain_rate = 0", CellId(c as u16).face());
        }
    }
    assert_eq!(world.state.rain_in_total, 0.0, "no rain may be booked at rain_rate = 0");
    assert_eq!(world.state.evap_out_total, 0.0, "a dry surface cannot evaporate");

    // The control: the same world with the design's rain rate does get wet within the same
    // window, so the dryness above is `rain_rate = 0` and not a water step that never runs.
    let mut wet = World::new(WorldConfig::default()).expect("config");
    for _ in 0..400 {
        wet.step();
    }
    let sum: f64 = wet.state.fields.w.iter().sum();
    assert!(sum > 0.0, "control: a default world must be raining somewhere by 20 s, got Σw = {sum}");
}

#[test]
fn water_on_a_side_face_runs_downhill() {
    let mut cfg = quiet_config();
    cfg.water.flow = 3.0;
    // Bare embedded height as the terrain, so "downhill" is unambiguously "lower `h`" and
    // no basin can trap the flow part way.
    cfg.habitat.basin_gain = 0.0;
    let mut world = World::new(cfg).expect("config");

    let source = CellId::new(Face::Front, 8, 4);
    // Shallow enough that `depth_gain · w` (0.1) never exceeds the 0.125 of `h` between two
    // vertically adjacent side cells, so no surface level can ever tilt uphill.
    set_cell_water(&mut world, source, 0.25);

    let total0: f64 = world.state.fields.w.iter().sum();
    let mean_height = |w: &Vec<f64>| -> f64 {
        let mass: f64 = w.iter().sum();
        w.iter().enumerate().map(|(c, x)| x * cell_height(CellId(c as u16))).sum::<f64>() / mass
    };
    let mut previous = mean_height(&world.state.fields.w);
    let start = previous;

    for tick in 1..=2000u32 {
        world.step();
        let total: f64 = world.state.fields.w.iter().sum();
        assert!(
            (total - total0).abs() <= 1e-12 * total0,
            "tick {tick}: flow is a transfer, but Σw went from {total0} to {total}"
        );
        let now = mean_height(&world.state.fields.w);
        assert!(
            now <= previous + 1e-12,
            "tick {tick}: the water's mean height rose from {previous} to {now}"
        );
        previous = now;
    }

    let floor = cell_height(CellId::new(Face::Front, 8, 15));
    assert!(
        previous < floor + 0.2,
        "after 100 s the water should stand on the floor row (h = {floor}); its mean height is {previous}, from {start}"
    );
}

#[test]
fn evaporation_keeps_its_floor_where_there_is_no_light() {
    let mut cfg = quiet_config();
    flat_habitat(&mut cfg, 0.0, 0.5);
    cfg.water.evap = 0.008;
    cfg.water.flow = 0.0;
    let evap = cfg.water.evap;
    let floor = cfg.water.evap_floor;
    let mut world = World::new(cfg).expect("config");

    set_water(&mut world, 0.5);

    // `design/water.md`: `evap · max(L, evap_floor) · w` per second, with `L = 0` here.
    let per_tick = 1.0 - evap * floor * DT;
    let mut want = 0.5;
    for tick in 1..=200u32 {
        world.step();
        want *= per_tick;
        for c in 0..CELL_COUNT {
            let got = world.state.fields.w[c];
            assert!(
                (got - want).abs() <= 1e-12 * want,
                "tick {tick}, cell {c}: w = {got}, but the evaporation floor gives {want}"
            );
        }
    }
    assert!(want < 0.5, "the floor must actually dry the dark: w only fell to {want}");

    let sum: f64 = world.state.fields.w.iter().sum();
    let ledger = world.state.rain_in_total - world.state.evap_out_total;
    assert!((sum - ledger).abs() <= 1e-9 * sum, "the evaporated depth must be booked: Σw = {sum}, ledger = {ledger}");
}

// ---------------------------------------------------------------------------------------
// Algae (`design/water.md` "Algae", spec "Conversion table")
// ---------------------------------------------------------------------------------------

/// A lightless world with a uniform moisture of 0.5, three isolated cells stocked with
/// nutrient and producer, and only growth switched on. Returns the world and the
/// (dry, wet, flooded) cells.
fn algae_fixture() -> (World, CellId, CellId, CellId) {
    let mut cfg = quiet_config();
    flat_habitat(&mut cfg, 0.0, 0.5);
    cfg.producer.growth = 0.008;
    let mut world = World::new(cfg).expect("config");

    let dry = CellId::new(Face::Front, 2, 2);
    let wet = CellId::new(Face::Front, 6, 6);
    let flooded = CellId::new(Face::Front, 10, 10);

    let before: f64 = common::total_material(&world);
    for c in [dry, wet, flooded] {
        world.state.fields.n[c.index()] = 0.5;
        world.state.fields.p[c.index()] = 0.3;
    }
    let after: f64 = common::total_material(&world);
    world.state.external_material_in += after - before;

    let flood = world.state.config.water.flood;
    let algae_depth = world.state.config.water.algae_depth;
    set_cell_water(&mut world, wet, algae_depth);
    set_cell_water(&mut world, flooded, 2.0 * flood);

    (world, dry, wet, flooded)
}

#[test]
fn a_dark_wet_cell_grows_producers_at_the_algae_rate() {
    let (mut world, _, wet, _) = algae_fixture();
    let cfg = world.state.config.clone();

    // The spec's growth law with the algae light floor and the wet moisture, evaluated here.
    let p = 0.3f64;
    let n = 0.5f64;
    let w = cfg.water.algae_depth;
    let l_eff = (cfg.water.algae_light * (w / cfg.water.algae_depth).min(1.0)).max(0.0);
    let w_eff = (0.5 + cfg.water.wet_gain * w.min(1.0)).clamp(cfg.habitat.moisture_min, 1.0);
    let want = (cfg.producer.growth * l_eff * w_eff * p * (1.0 - p / cfg.producer.max) * (n / (n + cfg.nutrient.half_saturation)) * DT)
        .min(cfg.producer.uptake_max * n * DT)
        .min(n);
    assert!(want > 0.0, "the fixture must actually grow: Δ = {want}");

    world.step();

    let got = world.state.fields.p[wet.index()] - p;
    assert!(
        (got - want).abs() <= 1e-12 * want,
        "a dark pool grew ΔP = {got}, but the algae law gives {want}"
    );
    let n_now = world.state.fields.n[wet.index()];
    assert!((n_now - (n - want)).abs() <= 1e-12 * n, "growth must take its material from N: N = {n_now}");
}

#[test]
fn a_dark_dry_cell_grows_nothing() {
    let (mut world, dry, wet, _) = algae_fixture();
    let p0 = world.state.fields.p[dry.index()];
    let n0 = world.state.fields.n[dry.index()];
    let wet0 = world.state.fields.p[wet.index()];
    for _ in 0..200 {
        world.step();
    }
    assert!(
        world.state.fields.p[wet.index()] > wet0,
        "control: the wet cell of the same fixture must grow, or this test proves nothing"
    );
    assert_eq!(
        world.state.fields.p[dry.index()].to_bits(),
        p0.to_bits(),
        "a dry cell in the dark has no light at all and must not grow"
    );
    assert_eq!(world.state.fields.n[dry.index()].to_bits(), n0.to_bits(), "and must not spend nutrient");
}

#[test]
fn a_flooded_cell_grows_nothing() {
    let (mut world, _, wet, flooded) = algae_fixture();
    let p0 = world.state.fields.p[flooded.index()];
    let wet0 = world.state.fields.p[wet.index()];
    let w = world.state.fields.w[flooded.index()];
    let flood = world.state.config.water.flood;
    assert!(w > flood, "the fixture must actually be flooded: w = {w}, flood = {flood}");
    // The flooded cell is lit by its own algae mat (w ≥ algae_depth), so only `drown` can
    // explain a standstill. `drown = max(0, 1 − (w − flood)/flood)` is zero at w = 2·flood.
    for _ in 0..200 {
        world.step();
    }
    assert!(
        world.state.fields.p[wet.index()] > wet0,
        "control: the shallow-pool cell of the same fixture must grow, or this test proves nothing"
    );
    assert_eq!(
        world.state.fields.p[flooded.index()].to_bits(),
        p0.to_bits(),
        "standing water deeper than 2·flood is bare water, however well the algae light it"
    );
}

// ---------------------------------------------------------------------------------------
// Fruit (`design/fauna-v2.md` "Fruit")
// ---------------------------------------------------------------------------------------

#[test]
fn the_closed_box_holds_with_the_fruit_pool() {
    let mut world = World::new(WorldConfig::default()).expect("config");
    let m0 = common::total_material(&world);
    assert!(m0 > 0.0);
    for tick in 1..=2000u32 {
        world.step();
        let m = common::total_material(&world);
        assert!(
            (m - m0).abs() <= 1e-11 * m0,
            "tick {tick}: Σ(N+P+D+F) + organisms drifted from {m0} to {m} (Δ = {})",
            m - m0
        );
    }
    let fruit: f64 = world.state.fields.f.iter().sum();
    assert!(fruit > 0.0, "a default world must ripen some fruit in 100 s, got ΣF = {fruit}");
}

#[test]
fn fruit_ripens_only_above_the_fruit_minimum() {
    let mut cfg = quiet_config();
    flat_habitat(&mut cfg, 0.8, 0.5);
    cfg.fruit.ripen = 0.02;
    let light = 0.8;
    let mut world = World::new(cfg).expect("config");
    let cfg = world.state.config.clone();
    let threshold = cfg.fruit.fruit_min * cfg.producer.max;

    let below = CellId::new(Face::Front, 2, 2);
    let at = CellId::new(Face::Front, 6, 6);
    let above = CellId::new(Face::Front, 10, 10);
    let p_below = threshold - 0.01;
    let p_above = 0.6;

    let before: f64 = common::total_material(&world);
    world.state.fields.p[below.index()] = p_below;
    world.state.fields.p[at.index()] = threshold;
    world.state.fields.p[above.index()] = p_above;
    let after: f64 = common::total_material(&world);
    world.state.external_material_in += after - before;

    world.step();

    assert_eq!(world.state.fields.f[below.index()], 0.0, "a cell under fruit_min · P_max must not fruit");
    assert_eq!(world.state.fields.f[at.index()], 0.0, "the (·)⁺ in the rate makes the threshold itself yield nothing");

    let want = cfg.fruit.ripen * p_above * (p_above / cfg.producer.max - cfg.fruit.fruit_min) * light * DT;
    let got = world.state.fields.f[above.index()];
    assert!(
        (got - want).abs() <= 1e-12 * want,
        "a rich lit cell ripened ΔF = {got}, but the spec's rate gives {want}"
    );
    assert!(
        (world.state.fields.p[above.index()] - (p_above - want)).abs() <= 1e-12 * p_above,
        "ripening is `P → F`: P should have fallen by exactly ΔF"
    );

    for _ in 0..400 {
        world.step();
    }
    assert_eq!(world.state.fields.f[below.index()], 0.0, "the poor cell must stay fruitless");
    assert_eq!(world.state.fields.f[at.index()], 0.0, "and so must the cell exactly at the threshold");
}

/// One stationary probe organism sitting on a stocked cell, hungry enough to feed but with
/// only `headroom` of reserve capacity left, so the order in which it requests its foods is
/// visible in the deltas.
fn feeding_probe(diet: f32, fruit_here: f64, headroom: f64) -> (World, OrganismId, CellId) {
    let mut cfg = quiet_config();
    flat_habitat(&mut cfg, 0.5, 0.5);
    deterministic_steering(&mut cfg);
    cfg.drives.w_depth = 0.0;
    // The slowest genome the range allows, so a feeding body cannot leave its cell.
    cfg.founders.kinds = vec![probe_kind(diet, 0.5, 0.3, 1.0, 0.0, 0)];
    let mut world = World::new(cfg).expect("config");

    let id = sole_id(&world);
    let cell = CellId::new(Face::Front, 8, 8);
    let centre = cell.center();
    {
        let o = world.state.organisms.get_mut(id).expect("probe");
        o.pos = centre;
        o.heading = Vec2::new(1.0, 0.0);
        o.ou = Vec2::ZERO;
        o.reserve = o.phenotype.reserve_max - headroom;
        o.energy = o.phenotype.energy_max;
        o.hunger_memory = 0.9;
        o.mode = Mode::Seeking;
    }

    let before: f64 = common::total_material(&world);
    world.state.fields.p[cell.index()] = 0.6;
    world.state.fields.f[cell.index()] = fruit_here;
    let after: f64 = common::total_material(&world);
    world.state.external_material_in += after - before;

    (world, id, cell)
}

#[test]
fn a_frugivore_takes_fruit_before_producer() {
    // Headroom smaller than one tick's fruit request, so whichever food is served first
    // consumes the whole of it.
    let headroom = 0.0005;
    let (mut world, _, cell) = feeding_probe(0.7, 1.0, headroom);
    let p0 = world.state.fields.p[cell.index()];
    let f0 = world.state.fields.f[cell.index()];
    world.step();
    let dp = world.state.fields.p[cell.index()] - p0;
    let df = world.state.fields.f[cell.index()] - f0;

    // The control: the same organism on the same cell with no fruit does eat the producer,
    // so a zero ΔP above is a priority, not an inability to graze.
    let (mut control, _, ccell) = feeding_probe(0.7, 0.0, headroom);
    let cp0 = control.state.fields.p[ccell.index()];
    control.step();
    let control_dp = control.state.fields.p[ccell.index()] - cp0;

    assert!(
        control_dp < -1e-9,
        "control: a hungry grazer on a stocked cell must graze, but ΔP = {control_dp}"
    );
    assert!(df < -1e-9, "the frugivore should have taken fruit, but ΔF = {df}");
    assert_eq!(dp, 0.0, "fruit is requested first and used the whole headroom, so ΔP must be exactly 0 (got {dp})");
    assert!(
        (df.abs() - headroom).abs() <= 1e-12,
        "the fruit request is capped by the reserve headroom {headroom}, but ΔF = {df}"
    );
}

#[test]
fn an_organism_below_the_fruit_diet_never_reduces_fruit() {
    let mut base = quiet_config();
    flat_habitat(&mut base, 0.5, 0.5);
    deterministic_steering(&mut base);
    base.drives.w_depth = 0.0;

    let run = |diet: f32| -> (f64, f64) {
        let mut cfg = base.clone();
        cfg.founders.kinds = vec![probe_kind(diet, 0.5, 0.3, 1.0, 0.0, 0)];
        let mut world = World::new(cfg).expect("config");
        let id = sole_id(&world);
        {
            let o = world.state.organisms.get_mut(id).expect("probe");
            o.reserve = 0.2 * o.phenotype.reserve_max;
            o.energy = o.phenotype.energy_max;
            o.hunger_memory = 0.9;
            o.mode = Mode::Seeking;
        }
        // Fruit and producer everywhere, so the answer cannot depend on where it walks.
        set_fields(&mut world, 0.0, 0.6, 0.0, 0.0, 1.0);
        let f0: f64 = world.state.fields.f.iter().sum();
        for _ in 0..400 {
            world.step();
        }
        (f0, world.state.fields.f.iter().sum())
    };

    let (f0_low, f1_low) = run(0.3);
    assert_eq!(f1_low, f0_low, "fruit is only for `diet ≥ 0.5`: a diet-0.3 organism moved ΣF from {f0_low} to {f1_low}");

    let (f0_high, f1_high) = run(0.7);
    assert!(f1_high < f0_high - 1e-9, "a diet-0.7 organism must eat fruit: ΣF went {f0_high} → {f1_high}");
}

// ---------------------------------------------------------------------------------------
// Genome v2 (`design/fauna-v2.md` "Genome v2")
// ---------------------------------------------------------------------------------------

#[test]
fn a_v1_genome_loads_with_the_documented_defaults_and_becomes_version_two() {
    // A v1 encoding: the seven v1 loci and the v1 drives, with none of the v2 fields.
    let v1 = r#"
version = 1
size = 1.0
metabolism = 1.0
sense = 6.0
reserve = 1.0
mouth = 1.0
speed = 1.0
hue = 0.8

[drives]
w_food = 1.0
w_detritus = 0.4
w_persist = 0.3
w_crowd = 0.6
seek_on = 0.3
seek_off = 0.1
feed_min = 0.2
rest_effort = 0.05
feed_effort = 0.2
bud_reserve = 0.7
bud_energy = 0.3
bud_min_age_seconds = 120.0
tau_hunger_seconds = 10.0
turn_rate_max_deg = 90.0
turn_noise = 0.6
"#;
    let mut g: Genome = toml::from_str(v1).expect("a v1 genome must still decode");

    assert_eq!(g.version, 1, "the fixture is a v1 encoding");
    assert_eq!(g.diet, 0.7, "`diet` defaults to 0.7 on a v1 upgrade");
    assert_eq!(g.depth, 0.5, "`depth` defaults to 0.5");
    assert_eq!(g.swim, 0.0, "`swim` defaults to 0");
    assert_eq!(g.drives.w_depth, 1.0, "`w_depth` defaults to 1.0");
    assert_eq!(g.form, FORM_UNSET, "a v1 encoding carries no form until it is upgraded");

    assert!(g.upgrade(), "upgrading a v1 genome must change something");
    assert_eq!(g.version, Genome::VERSION, "an upgraded genome is stamped version 2");
    assert_eq!(g.version, 2);
    assert_eq!(g.form, genome::form_of_hue(0.8), "`form` takes the hue tercile: hue 0.8 is the third rig");
    assert_eq!(g.form, 2);
    assert_eq!(g.diet, 0.7);
    assert_eq!(g.depth, 0.5);
    assert_eq!(g.swim, 0.0);
    assert_eq!(g.drives.w_depth, 1.0);

    assert!(!g.upgrade(), "upgrading an already-v2 genome must be a no-op");
}

#[test]
fn the_genome_clamps_diet_depth_and_swim_into_their_ranges() {
    let mut g = Genome::founder(0.5, &WorldConfig::default().drives);

    g.diet = 5.0;
    g.depth = -3.0;
    g.swim = 2.0;
    assert!(g.clamp(), "out-of-range v2 loci must be clamped");
    assert_eq!(g.diet, 1.0, "`diet` is 0–1");
    assert_eq!(g.depth, 0.0, "`depth` is 0–1");
    assert_eq!(g.swim, 1.0, "`swim` is 0–1");

    g.diet = 0.0;
    g.depth = 1.0;
    g.swim = 0.5;
    assert!(!g.clamp(), "values already inside the range must be left alone");
    assert_eq!((g.diet, g.depth, g.swim), (0.0, 1.0, 0.5));

    g.drives.w_depth = 9.0;
    assert!(g.clamp(), "`w_depth` is 0–2");
    assert_eq!(g.drives.w_depth, 2.0);
}

#[test]
fn mutation_never_changes_form() {
    let cfg = WorldConfig::default();
    let mut s = 0xF0_0BA5u64;
    for form in 0..MAX_FORMS {
        for _ in 0..2000 {
            let mut g = Genome::founder(0.5, &cfg.drives);
            g.form = form;
            let changes = g.mutate(1.0, 0.4, || splitmix(&mut s));
            assert_eq!(g.form, form, "`form` is never mutated: it changed under {changes:?}");
            for m in &changes {
                assert_ne!(m.locus, "form", "`form` must never appear as a mutated locus");
                assert!(
                    genome::MUTABLE_LOCI.contains(&m.locus),
                    "{} is not one of the loci the design names",
                    m.locus
                );
            }
        }
    }
}

#[test]
fn births_never_leave_the_founder_forms() {
    let mut cfg = WorldConfig::default();
    cfg.mechanisms.mutation = true;
    cfg.seed = 11;
    let founder_forms: Vec<u8> = cfg.founders.kinds.iter().map(|k| k.form.expect("default kinds fix form")).collect();
    let mut world = World::new(cfg).expect("config");

    for _ in 0..12_000 {
        world.step();
    }

    assert!(world.state.births_total > 0, "the run must contain births for this to mean anything");
    let mut seen_descendant = false;
    for (id, o) in world.state.organisms.iter() {
        assert!(
            founder_forms.contains(&o.genome.form),
            "organism {id:?} has form {} outside the founder forms {founder_forms:?}",
            o.genome.form
        );
        assert_eq!(o.genome.version, Genome::VERSION, "every live genome is version 2");
        if o.parent.is_some() {
            seen_descendant = true;
        }
    }
    assert!(seen_descendant, "the population should contain at least one descendant after 10 simulated minutes");
}

// ---------------------------------------------------------------------------------------
// Founder kinds (`design/fauna-v2.md` "Founders come in kinds")
// ---------------------------------------------------------------------------------------

#[test]
fn the_default_config_places_twenty_four_founders_by_kind() {
    let mut world = World::new(WorldConfig::default()).expect("config");
    assert_eq!(world.population(), 24, "burrower 4 + grazer 10 + glider 5 + skimmer 5");

    let mut by_form = [0u32; MAX_FORMS as usize];
    for (_, o) in world.state.organisms.iter() {
        by_form[o.genome.form as usize] += 1;
    }
    // lantern = grazer 0, sail = glider 1, mossback = burrower 2, skimmer 3.
    assert_eq!(by_form, [10, 5, 4, 5, 0, 0, 0, 0], "counts after the 2026-09-12 rebalance");
    assert_eq!(world.telemetry().population_by_form, by_form, "telemetry must report the same census");

    // The diets that make the kinds different kinds.
    let mut diets: Vec<(u8, f32)> = world.state.organisms.iter().map(|(_, o)| (o.genome.form, o.genome.diet)).collect();
    diets.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.partial_cmp(&b.1).unwrap()));
    diets.dedup();
    assert_eq!(
        diets,
        vec![(0u8, 0.85f32), (1, 0.90), (2, 0.10), (3, 0.60)],
        "each form carries its kind's diet"
    );
}

#[test]
fn founder_kinds_are_deterministic_for_one_seed() {
    let cfg = WorldConfig { seed: 424_242, ..WorldConfig::default() };
    let a = World::new(cfg.clone()).expect("config");
    let b = World::new(cfg).expect("config");

    let census = |w: &World| -> Vec<(u8, f32, f32, Face, u64, u64)> {
        w.state
            .organisms
            .iter()
            .map(|(_, o)| (o.genome.form, o.genome.diet, o.genome.swim, o.pos.face, o.pos.u.to_bits(), o.pos.v.to_bits()))
            .collect()
    };
    assert_eq!(census(&a), census(&b), "two worlds of one seed must place identical founders");

    let other = WorldConfig { seed: 424_243, ..WorldConfig::default() };
    let c = World::new(other).expect("config");
    assert_ne!(census(&a), census(&c), "a different seed must place them differently");
}

#[test]
fn an_empty_kinds_list_falls_back_to_the_founder_count() {
    let mut cfg = WorldConfig::default();
    cfg.founders.kinds = Vec::new();
    cfg.founders.count = 7;
    let world = World::new(cfg).expect("config");
    assert_eq!(world.population(), 7, "with no kinds, `founders.count` is the founder count again");
    for (_, o) in world.state.organisms.iter() {
        assert_eq!(o.genome.diet, 0.7, "the v1 path's founders take the v1 defaults");
        assert_eq!(o.genome.depth, 0.5);
        assert_eq!(o.genome.swim, 0.0);
    }
}

// ---------------------------------------------------------------------------------------
// Controller v2 (`design/fauna-v2.md` "Controller v2", spec "Controller")
// ---------------------------------------------------------------------------------------

/// A single organism on a flat, frozen world: uniform producer below `feed_min` so it stays
/// Seeking, no detritus, no fruit, no water, no noise, and no steering term except the ones
/// the caller leaves on.
fn steering_probe(cfg_tweak: impl FnOnce(&mut WorldConfig), kind: FounderKind, at: SurfacePoint, heading: Vec2) -> (World, OrganismId) {
    let mut cfg = quiet_config();
    flat_habitat(&mut cfg, 0.5, 0.5);
    deterministic_steering(&mut cfg);
    cfg.founders.kinds = vec![kind];
    cfg_tweak(&mut cfg);
    let mut world = World::new(cfg).expect("config");

    // Below `feed_min` (0.2), so no food gate can fire and the mode stays Seeking.
    set_fields(&mut world, 0.0, 0.1, 0.0, 0.0, 0.0);

    let id = sole_id(&world);
    let o = world.state.organisms.get_mut(id).expect("probe");
    o.pos = at;
    o.heading = heading.normalized().expect("unit heading");
    o.ou = Vec2::ZERO;
    o.reserve = 0.2 * o.phenotype.reserve_max;
    o.energy = o.phenotype.energy_max;
    o.hunger_memory = 0.9;
    o.mode = Mode::Seeking;
    (world, id)
}

#[test]
fn a_resting_organism_holds_its_heading() {
    let mut cfg = quiet_config();
    flat_habitat(&mut cfg, 0.5, 0.5);
    cfg.founders.kinds = vec![probe_kind(0.7, 0.5, 1.0, 1.0, 0.0, 0)];
    assert_eq!(cfg.organism.rest_turn_fraction, 0.0, "the default rest gate is what this test is about");
    let mut world = World::new(cfg).expect("config");
    // Rich fields and a full reserve: the steering vector is not zero, but the gate is.
    set_fields(&mut world, 0.5, 0.6, 0.5, 0.5, 0.5);

    let id = sole_id(&world);
    let heading = {
        let o = world.state.organisms.get_mut(id).expect("probe");
        o.pos = SurfacePoint::new(Face::Front, 32.0, 32.0);
        o.heading = Vec2::new(0.6, 0.8);
        o.ou = Vec2::new(0.4, -0.7);
        o.reserve = o.phenotype.reserve_max;
        o.energy = o.phenotype.energy_max;
        o.hunger_memory = 0.0;
        o.mode = Mode::Resting;
        o.heading
    };

    for tick in 1..=200u32 {
        world.step();
        let o = world.state.organisms.get(id).expect("alive");
        assert_eq!(o.mode, Mode::Resting, "tick {tick}: a sated organism stays Resting");
        assert_eq!(o.pos.face, Face::Front, "tick {tick}: the move must stay on one face for this test to apply");
        assert_eq!(
            (o.heading.x.to_bits(), o.heading.y.to_bits()),
            (heading.x.to_bits(), heading.y.to_bits()),
            "tick {tick}: a resting body must hold its heading exactly, but it is {:?}",
            o.heading
        );
    }
}

/// Embedded height of a probe after `ticks` steps, for a genome with the given `depth`.
fn depth_drive_run(depth: f32, ticks: u32) -> (f64, f64) {
    let start = SurfacePoint::new(Face::Front, 32.0, 48.0);
    let (mut world, id) = steering_probe(
        |_| {},
        probe_kind(0.85, depth, 1.0, 1.0, 0.0, 0),
        start,
        // Across the slope, so any vertical progress is the depth drive's doing.
        Vec2::new(1.0, 0.0),
    );
    let h0 = world.state.organisms.get(id).expect("probe").pos.embed()[1];
    for _ in 0..ticks {
        world.step();
    }
    let o = world.state.organisms.get(id).expect("alive");
    assert_eq!(o.pos.face, Face::Front, "the probe must stay on the side face it started on");
    (h0, o.pos.embed()[1])
}

#[test]
fn the_depth_drive_carries_a_canopy_genome_upward() {
    let (h0, h1) = depth_drive_run(1.0, 400);
    assert!(
        h1 > h0 + 0.05,
        "`depth = 1` means `h_pref = 1`: the probe should have climbed, but went from {h0} to {h1}"
    );
}

#[test]
fn the_depth_drive_carries_a_soil_genome_downward() {
    let (h0, h1) = depth_drive_run(0.0, 400);
    assert!(
        h1 < h0 - 0.05,
        "`depth = 0` means `h_pref = −1`: the probe should have sunk, but went from {h0} to {h1}"
    );
}

/// A rich cell exactly two graph hops away in `+u`, with the whole adjacent ring flat, so
/// only a sensor that reaches past its neighbours can find it.
fn two_hop_patch(sense_radius: f64, ticks: u32) -> (SurfacePoint, SurfacePoint, Vec2, Vec2) {
    let own = CellId::new(Face::Front, 8, 8);
    let rich = CellId::new(Face::Front, 10, 8);
    let (mut world, id) = steering_probe(
        |cfg| {
            cfg.organism.sense_radius = sense_radius;
            cfg.drives.w_depth = 0.0; // only `∇P` may steer
        },
        probe_kind(0.85, 0.5, 1.0, 1.0, 0.0, 0),
        own.center(),
        // Pointing away from the patch: reaching it requires a deliberate turn.
        Vec2::new(-1.0, 0.0),
    );
    world.state.fields.p[rich.index()] = 1.4;
    world.state.external_material_in += 1.4 - 0.1;

    // The scenario the claim depends on: the patch is exactly two graph hops out, and every
    // cell of the adjacent ring carries the own cell's value, so a one-hop sensor sees a
    // perfectly flat neighbourhood.
    let neighbours = world.cell_neighbors();
    let ring1: Vec<usize> = neighbours[own.index()].iter().flatten().map(|n| *n as usize).collect();
    assert_eq!(ring1.len(), 4, "the probe's cell should be interior");
    assert!(!ring1.contains(&rich.index()), "the patch must not be adjacent");
    assert!(
        ring1.iter().any(|a| neighbours[*a].iter().flatten().any(|n| *n as usize == rich.index())),
        "the patch must be reachable in exactly two hops"
    );
    for a in &ring1 {
        assert_eq!(
            world.state.fields.p[*a], world.state.fields.p[own.index()],
            "the adjacent ring must be flat, or a one-hop sensor could steer too"
        );
    }

    let start = world.state.organisms.get(id).expect("probe").pos;
    let heading0 = world.state.organisms.get(id).expect("probe").heading;
    for _ in 0..ticks {
        world.step();
    }
    let o = world.state.organisms.get(id).expect("alive");
    (start, o.pos, heading0, o.heading)
}

#[test]
fn sensing_reaches_a_patch_two_cells_away() {
    let (start, end, _, _) = two_hop_patch(6.0, 600);
    assert_eq!(end.face, Face::Front);
    assert!(
        end.u > start.u + 2.0,
        "a 6 px sensor sees two cells out and should head for the patch at u = 42; it went from u = {} to u = {}",
        start.u,
        end.u
    );
    assert!((end.v - start.v).abs() < 2.0, "the patch is straight along +u, so v should barely move: {} → {}", start.v, end.v);
}

#[test]
fn a_one_hop_sensor_ignores_a_patch_two_cells_away() {
    let (start, end, heading0, heading1) = two_hop_patch(2.0, 600);
    assert_eq!(
        (heading1.x.to_bits(), heading1.y.to_bits()),
        (heading0.x.to_bits(), heading0.y.to_bits()),
        "with a flat one-hop ring and no other drive the steering vector is zero, so the heading is kept exactly"
    );
    assert!(
        end.u < start.u,
        "a 2 px sensor reaches one hop, sees nothing, and keeps walking away: u went {} → {}",
        start.u,
        end.u
    );
}

// ---------------------------------------------------------------------------------------
// Wading (`design/water.md` "Wading", `design/fauna-v2.md` `swim`)
// ---------------------------------------------------------------------------------------

/// Distance walked in `ticks` by an identical probe, under `depth` of water with the given
/// `swim`. The fields are flat and the noise is off, so all three runs make the same
/// decisions and only the wading divisor differs.
fn wading_run(depth: f64, swim: f32, ticks: u32) -> (f64, Mode) {
    let start = SurfacePoint::new(Face::Front, 32.0, 32.0);
    let (mut world, id) = steering_probe(
        |cfg| cfg.drives.w_depth = 0.0,
        probe_kind(0.85, 0.5, 1.0, 1.0, swim, 0),
        start,
        Vec2::new(1.0, 0.0),
    );
    set_water(&mut world, depth);
    for _ in 0..ticks {
        world.step();
    }
    let o = world.state.organisms.get(id).expect("alive");
    assert_eq!(o.pos.face, Face::Front, "the probe must stay on one face");
    assert_eq!(
        (o.heading.x.to_bits(), o.heading.y.to_bits()),
        (1.0f64.to_bits(), 0.0f64.to_bits()),
        "a flat world steers nothing: the heading must be held so the three runs are comparable"
    );
    (o.pos.u - start.u, o.mode)
}

#[test]
fn wading_halves_the_distance_of_a_non_swimmer() {
    let (dry, dry_mode) = wading_run(0.0, 0.0, 200);
    let (wet, wet_mode) = wading_run(1.0, 0.0, 200);
    assert_eq!(dry_mode, wet_mode, "the two runs must make the same decisions");
    assert!(dry > 0.5, "the dry probe must actually travel: {dry} px");
    assert!(
        (wet - dry / 2.0).abs() <= 1e-9 * dry,
        "`speed / (1 + w · (1 − swim))` at w = 1, swim = 0 halves the distance: dry {dry} px, wet {wet} px"
    );
}

#[test]
fn a_swimmer_ignores_the_pool() {
    let (dry, dry_mode) = wading_run(0.0, 1.0, 200);
    let (wet, wet_mode) = wading_run(1.0, 1.0, 200);
    assert_eq!(dry_mode, wet_mode, "the two runs must make the same decisions");
    assert!(
        (wet - dry).abs() <= 1e-12 * dry,
        "a swimmer's divisor is `1 + w · (1 − 1) = 1`: dry {dry} px, wet {wet} px"
    );
}
