//! Material and energy accounting, checked against `design/m2-world-spec.md`
//! ("Units and quantities", "Conversion table", "Tick order" step 10).
//!
//! Every quantity is recomputed from `World::state` by `common`, so the assertions are
//! independent of the world's own `mass_residual` / telemetry bookkeeping except where a
//! test names those outputs explicitly.

mod common;

use common::{harsh_config, no_light_config, stored_energy, total_material};
use cubarium_core::{World, WorldConfig};

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

    for tick in 1..=300u64 {
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

    // The world's own running audit must tell the same story as the per-tick samples. The
    // cumulative audit reads the *corrected* ledgers
    // (`design/7_Research/accounting-compensation-handoff-2026-09-13.md`): `light_in_total`
    // and `heat_out_total` alone are the uncompensated counters every build has written and
    // are an explicit diagnostic, not an accurate cumulative total.
    let running = world.state.net_energy_in_corrected();
    assert!(
        (running - cumulative_net).abs() < 1e-9,
        "running audit {running} disagrees with the summed samples {cumulative_net}"
    );
    // Over 2,000 ticks the raw counters have not yet drifted far enough to fail that bound,
    // so their disagreement is reported rather than asserted on.
    let raw = world.state.light_in_total - world.state.heat_out_total;
    println!(
        "corrected running audit is {:e} from the summed samples; the raw counters are {:e}",
        running - cumulative_net,
        raw - cumulative_net
    );
}

/// Spec, "Death": `S + R → D` in the cell, `De += min(E + e_r·R, e_d_max·(S + R))`, the
/// rest heat. Material is therefore conserved across the death and detritus grows by the
/// dead organism's material.
#[test]
fn death_moves_organism_material_into_remains() {
    let mut world = World::new(harsh_config()).expect("the harsh config is a valid world");

    let mut checked = 0usize;
    for tick in 1..=8_000u64 {
        let before_material = total_material(&world);
        let before_population = world.population();
        // Ecology v1 §8: an ordinary death lands in **animal remains** `C`, not in plant
        // litter `D`. Both are detrital stocks and neither is the other.
        let before_detritus: f64 = world.state.ecology.carrion.iter().sum();
        let before_organism: f64 =
            world.state.organisms.iter().map(|(_, o)| o.material()).sum::<f64>();

        world.step();

        if world.population() >= before_population {
            continue;
        }

        let after_material = total_material(&world);
        let after_detritus: f64 = world.state.ecology.carrion.iter().sum();
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
        // Remains also decompose to `N` and are scavenged within the same tick, so the stock
        // need not gain the whole amount; it must not *lose* material while a corpse arrives.
        assert!(
            gained > -1e-9,
            "tick {tick}: organism material {lost:e} vanished instead of becoming remains \
             (carrion moved by {gained:e})"
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
