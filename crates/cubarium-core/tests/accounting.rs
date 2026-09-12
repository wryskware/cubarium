//! Material and energy accounting, checked against `design/m2-world-spec.md`
//! ("Units and quantities", "Conversion table", "Tick order" step 10).
//!
//! Every quantity is recomputed from `World::state` by `common`, so the assertions are
//! independent of the world's own `mass_residual` / telemetry bookkeeping except where a
//! test names those outputs explicitly.

mod common;

use common::{harsh_config, no_light_config, stocks, stored_energy, total_material};
use cubarium_core::{World, WorldConfig};

/// Spec: "`M` is constant to rounding except for named external material sources (none in
/// M2 besides the initial seed)", with the invariant "checked every tick in debug and
/// every 60 s in release".
#[test]
fn material_is_closed_over_six_thousand_ticks() {
    let mut world = World::new(WorldConfig::default()).expect("defaults are a valid world");

    // `mass_residual` is documented as drift from the initial seed plus admitted material,
    // so it must read ~0 from the very first tick.
    let initial = total_material(&world);
    // The founders are already standing in `initial`, so only material admitted *after*
    // creation may move the box.
    let admitted_at_creation = world.state.external_material_in;
    let mut worst_residual = 0.0f64;
    let mut worst_recomputed = 0.0f64;

    for tick in 1..=6_000u64 {
        world.step();

        let residual = world.mass_residual();
        assert!(
            residual.is_finite(),
            "tick {tick}: mass_residual is not finite: {residual}"
        );
        worst_residual = worst_residual.max(residual.abs());

        // Independent recomputation: the closed box may only move by the material the
        // world says it admitted from outside.
        let drift = total_material(&world)
            - initial
            - (world.state.external_material_in - admitted_at_creation);
        worst_recomputed = worst_recomputed.max(drift.abs());

        if tick % 100 == 0 {
            world.check_invariants().unwrap_or_else(|e| panic!("tick {tick}: {e}"));
        }
    }

    assert!(
        worst_residual < 1e-9,
        "worst |mass_residual| over 6000 ticks was {worst_residual:e}, expected < 1e-9"
    );
    assert!(
        worst_recomputed < 1e-9,
        "worst independently recomputed material drift was {worst_recomputed:e}, expected < 1e-9"
    );
}

/// Spec: energy "enters from light, leaves as heat". With no light there is no source, so
/// the stored total `Σ(e_p·P + De) + Σ_org(E + e_r·R) + escrow` can only fall.
#[test]
fn stored_energy_never_increases_without_light() {
    let mut world = World::new(no_light_config()).expect("a lightless world is still valid");

    let mut previous = stored_energy(&world);
    let mut worst_rise = 0.0f64;
    let mut worst_rise_tick = 0u64;

    for tick in 1..=3_000u64 {
        world.step();
        let now = stored_energy(&world);
        let rise = now - previous;
        if rise > worst_rise {
            worst_rise = rise;
            worst_rise_tick = tick;
        }
        previous = now;
    }

    assert_eq!(
        world.state.light_in_total, 0.0,
        "a lightless habitat reported light_in_total = {}",
        world.state.light_in_total
    );
    assert!(
        worst_rise <= 1e-12,
        "stored energy rose by {worst_rise:e} at tick {worst_rise_tick} with no light source"
    );
}

/// Spec: "every tick `ΔE_total = light_in − heat_out` to rounding. Both sides are logged."
#[test]
fn the_energy_audit_balances_every_tick() {
    let mut world = World::new(WorldConfig::default()).expect("defaults are a valid world");

    let mut previous = stored_energy(&world);
    let mut cumulative_delta = 0.0f64;
    let mut cumulative_net = 0.0f64;
    let mut worst_tick_error = 0.0f64;
    let mut worst_tick = 0u64;

    for tick in 1..=2_000u64 {
        world.step();
        // `telemetry()` is documented to reset the per-sample counters, so sampling every
        // tick yields this tick's `light_in` / `heat_out`.
        let sample = world.telemetry();
        assert_eq!(sample.tick, tick, "telemetry tick label");

        let now = stored_energy(&world);
        let delta = now - previous;
        let net = sample.light_in - sample.heat_out;
        let error = (delta - net).abs();
        if error > worst_tick_error {
            worst_tick_error = error;
            worst_tick = tick;
        }
        cumulative_delta += delta;
        cumulative_net += net;
        previous = now;

        assert!(
            sample.light_in >= 0.0 && sample.heat_out >= 0.0,
            "tick {tick}: light_in {} / heat_out {} must be nonnegative flows",
            sample.light_in,
            sample.heat_out
        );
    }

    assert!(
        worst_tick_error < 1e-9,
        "worst per-tick audit error {worst_tick_error:e} at tick {worst_tick}, expected < 1e-9"
    );
    let cumulative_error = (cumulative_delta - cumulative_net).abs();
    assert!(
        cumulative_error < 1e-9,
        "cumulative ΔE_total {cumulative_delta} vs light_in − heat_out {cumulative_net} \
         differ by {cumulative_error:e}, expected < 1e-9"
    );

    // The world's own running audit must tell the same story as the per-tick samples.
    let running = world.state.light_in_total - world.state.heat_out_total;
    assert!(
        (running - cumulative_net).abs() < 1e-9,
        "running audit {running} disagrees with the summed samples {cumulative_net}"
    );
}

/// Spec: `P`, `D`, `N`, `De` are stocks and "Never produces negatives"; maintenance is
/// `paid = min(cost · dt, E)` "so `E ≥ 0` always"; reserve and structure are material.
///
/// The run is carried past 3,000 ticks because starvation under this config first strikes
/// around tick 3,025; the extra ticks make sure the death path is exercised too.
#[test]
fn every_stock_stays_nonnegative_while_the_world_starves() {
    let mut world = World::new(harsh_config()).expect("the harsh config is a valid world");

    let mut worst: Option<(u64, &'static str, f64)> = None;
    for tick in 1..=4_500u64 {
        world.step();
        for (name, value) in stocks(&world) {
            assert!(value.is_finite(), "tick {tick}: {name} is not finite ({value})");
            if value < 0.0 && worst.is_none_or(|(_, _, w)| value < w) {
                worst = Some((tick, name, value));
            }
        }
        if tick % 100 == 0 {
            world.check_invariants().unwrap_or_else(|e| panic!("tick {tick}: {e}"));
        }
    }

    if let Some((tick, name, value)) = worst {
        panic!("stock {name} went negative ({value:e}) at tick {tick}");
    }

    let deaths: u64 = world.state.deaths_total.iter().sum();
    assert!(
        deaths > 0,
        "the harsh config was meant to starve the founders but recorded no deaths \
         (population {}), so the death path went unchecked",
        world.population()
    );
}

/// Spec, "Death": `S + R → D` in the cell, `De += min(E + e_r·R, e_d_max·(S + R))`, the
/// rest heat. Material is therefore conserved across the death and detritus grows by the
/// dead organism's material.
#[test]
fn death_moves_organism_material_into_detritus() {
    let mut world = World::new(harsh_config()).expect("the harsh config is a valid world");

    let mut checked = 0usize;
    for tick in 1..=8_000u64 {
        let before_material = total_material(&world);
        let before_population = world.population();
        let before_detritus: f64 = world.state.fields.d.iter().sum();
        let before_organism: f64 =
            world.state.organisms.iter().map(|(_, o)| o.material()).sum::<f64>();

        world.step();

        if world.population() >= before_population {
            continue;
        }

        let after_material = total_material(&world);
        let after_detritus: f64 = world.state.fields.d.iter().sum();
        let after_organism: f64 =
            world.state.organisms.iter().map(|(_, o)| o.material()).sum::<f64>();

        assert!(
            (after_material - before_material).abs() < 1e-9,
            "tick {tick}: {} organisms died and material moved by {:e}",
            before_population - world.population(),
            after_material - before_material
        );
        let lost = before_organism - after_organism;
        let gained = after_detritus - before_detritus;
        assert!(
            lost > 0.0,
            "tick {tick}: population fell but organism material did not ({lost:e})"
        );
        // Detritus also decomposes to `N` and is grazed within the same tick, so it need
        // not gain the whole amount; it must not *lose* material while a corpse arrives.
        assert!(
            gained > -1e-9,
            "tick {tick}: organism material {lost:e} vanished instead of becoming detritus \
             (detritus moved by {gained:e})"
        );

        // Every remaining stock is still a well-formed stock right after a death.
        world.check_invariants().unwrap_or_else(|e| panic!("tick {tick} after a death: {e}"));

        checked += 1;
        if checked == 3 {
            break;
        }
    }

    assert!(
        checked > 0,
        "no tick in 8,000 saw the population fall, so the death path went unchecked \
         (population {}, deaths {:?})",
        world.population(),
        world.state.deaths_total
    );
    println!("checked material conservation across {checked} ticks that lost organisms");
}
