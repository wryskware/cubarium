//! Surface water: rain, downhill flow, standing pools, evaporation (`design/water.md`).
//!
//! Water is not material. It is an open quantity with an audited budget, like energy:
//! rain adds it, evaporation removes it, and every tick `Δ Σw = rain_in − evap_out` to
//! rounding. Nothing here touches `N`, `P`, `D` or `De`; the coupling to the ecology runs
//! the other way, through `Fields::react` (wet growth, drowning) and movement (wading).

use cubarium_surface::{CELL_COUNT, FieldGraph, ScalarField};

use crate::DT;
use crate::config::WaterConfig;

/// Largest flux fraction of a source cell's depth one edge may carry in one substep. A cell
/// has at most four edges, so four quarters cannot drain it below zero.
const EDGE_CAP: f64 = 0.25;

/// The static and per-tick drivers of one water step, all per cell in `CellId` order.
#[derive(Clone, Copy)]
pub struct Drivers<'a> {
    /// Terrain height `z` (`Habitat::terrain`).
    pub terrain: &'a [f64; CELL_COUNT],
    /// This tick's light `L`, for evaporation.
    pub light: &'a [f64; CELL_COUNT],
    /// This tick's moisture weather blob sum `B`, for rain.
    pub rain_source: &'a [f64; CELL_COUNT],
}

/// What one tick of water did, for the audit and telemetry.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct WaterLedger {
    /// Depth added by rain, summed over cells.
    pub rain_in: f64,
    /// Depth removed by evaporation, summed over cells.
    pub evap_out: f64,
    /// The share of `rain_in` that came from the manual rate (the care contract's shower),
    /// summed over cells. Always zero when no manual array is supplied. Manual water is
    /// inside `rain_in` once; this is the attribution, not a second budget.
    pub manual_in: f64,
}

/// One tick of water, in the order rain → flow → evaporation, every stage reading the
/// values the previous stage left (the stages are sequential by design: rain that falls
/// this tick may already run downhill this tick).
///
/// Normative (`design/water.md`):
/// - rain: `w += rain_rate · max(0, B − rain_threshold) · dt` per cell, `B` the moisture
///   weather blob sum; `rain[c]` receives that rate in d/s for the render view.
/// - flow: for every graph edge `(a, b)` with surface levels `s = z + depth_gain · w`,
///   `q = k · (s_a − s_b)` from the higher cell to the lower, `k = flow · dt / substeps`,
///   capped at `EDGE_CAP · w_source` of the source's pre-substep depth; substeps
///   `ceil(flow · dt / EDGE_CAP)`, so the coefficient per substep never exceeds a quarter.
///   No flux crosses the open rim (the graph has no edges there).
/// - evaporation: `w −= min(w, evap · max(L, evap_floor) · w · dt)`; the floor keeps the
///   dark soil drying slowly, so a moat cannot fill without bound.
///
/// `manual` is the care contract's shower: an optional per-cell rate in d/s added to the
/// natural rate at the rain stage, so `rain[c]` publishes what actually falls and the depth
/// is inside `rain_in` once. `None` is not "an array of zeros": it takes the pre-care branch
/// and executes the original arithmetic operation for operation, which is what lets a world
/// that is never given care replay bit for bit.
///
/// Returns the ledger; `Σw_after − Σw_before == rain_in − evap_out` to rounding.
pub fn step(
    w: &mut [f64],
    cfg: &WaterConfig,
    drivers: Drivers<'_>,
    manual: Option<&[f64; CELL_COUNT]>,
    rain: &mut [f32; CELL_COUNT],
    graph: &FieldGraph,
    scratch: &mut ScalarField,
) -> WaterLedger {
    debug_assert_eq!(w.len(), CELL_COUNT);
    let Drivers { terrain, light, rain_source } = drivers;
    let mut ledger = WaterLedger::default();

    // Rain.
    match manual {
        None => {
            for i in 0..CELL_COUNT {
                let excess = rain_source[i] - cfg.rain_threshold;
                let rate = if excess > 0.0 { cfg.rain_rate * excess } else { 0.0 };
                rain[i] = rate as f32;
                let added = rate * DT;
                w[i] += added;
                ledger.rain_in += added;
            }
        }
        Some(manual) => {
            for i in 0..CELL_COUNT {
                let excess = rain_source[i] - cfg.rain_threshold;
                let natural = if excess > 0.0 { cfg.rain_rate * excess } else { 0.0 };
                let rate = natural + manual[i];
                rain[i] = rate as f32;
                let added = rate * DT;
                w[i] += added;
                ledger.rain_in += added;
                // What the manual rate actually put in this cell, given the combined
                // rounding. Exactly `manual[i] · DT` wherever nothing natural is falling.
                ledger.manual_in += added - natural * DT;
            }
        }
    }

    // Flow.
    let total = cfg.flow * DT;
    if total.is_finite() && total > 0.0 {
        let substeps = (total / EDGE_CAP).ceil().max(1.0) as u32;
        let k = total / f64::from(substeps);
        for _ in 0..substeps {
            scratch.values.copy_from_slice(w);
            for &(a, b) in graph.edges() {
                let (ia, ib) = (a.index(), b.index());
                let (wa, wb) = (scratch.values[ia], scratch.values[ib]);
                let sa = terrain[ia] + cfg.depth_gain * wa;
                let sb = terrain[ib] + cfg.depth_gain * wb;
                let (from, to, flux) = if sa > sb {
                    (ia, ib, (k * (sa - sb)).min(EDGE_CAP * wa))
                } else if sb > sa {
                    (ib, ia, (k * (sb - sa)).min(EDGE_CAP * wb))
                } else {
                    continue;
                };
                if flux <= 0.0 {
                    continue;
                }
                w[from] -= flux;
                w[to] += flux;
            }
            // Four quarter-caps sum to the whole depth exactly in real arithmetic; in
            // floating point a drained cell can land a few ulps below zero. Snap those to
            // dry ground (the budget error is at denormal scale, far inside the audit
            // tolerance); anything larger is a bug.
            for depth in w.iter_mut() {
                if *depth < 0.0 {
                    debug_assert!(*depth > -1e-12, "flow over-drained a cell: {depth}");
                    *depth = 0.0;
                }
            }
        }
    }

    // Evaporation.
    if cfg.evap > 0.0 {
        for i in 0..CELL_COUNT {
            let lost = (cfg.evap * light[i].max(cfg.evap_floor) * w[i] * DT).min(w[i]).max(0.0);
            w[i] -= lost;
            ledger.evap_out += lost;
        }
    }

    ledger
}

/// Finite and nonnegative everywhere.
pub fn check(w: &[f64]) -> Result<(), String> {
    if w.len() != CELL_COUNT {
        return Err(format!("w has {} cells, expected {CELL_COUNT}", w.len()));
    }
    for (i, &x) in w.iter().enumerate() {
        if !x.is_finite() {
            return Err(format!("w[{i}] is not finite: {x}"));
        }
        if x < 0.0 {
            return Err(format!("w[{i}] is negative: {x}"));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::WorldConfig;
    use crate::habitat::Habitat;
    use cube_proto::Face;
    use cubarium_surface::CellId;

    struct Harness {
        cfg: WaterConfig,
        terrain: Box<[f64; CELL_COUNT]>,
        light: Box<[f64; CELL_COUNT]>,
        source: Box<[f64; CELL_COUNT]>,
        rain: Box<[f32; CELL_COUNT]>,
        graph: FieldGraph,
        scratch: ScalarField,
    }

    impl Harness {
        fn new(basin_gain: f64) -> Harness {
            let mut world = WorldConfig::default();
            world.habitat.basin_gain = basin_gain;
            let habitat = Habitat::new(&world.habitat, 3);
            Harness {
                cfg: world.water,
                terrain: habitat.terrain.clone(),
                light: habitat.light_base.clone(),
                source: Box::new([0.0; CELL_COUNT]),
                rain: Box::new([0.0; CELL_COUNT]),
                graph: FieldGraph::new(),
                scratch: ScalarField::zeros(),
            }
        }

        fn step(&mut self, w: &mut [f64]) -> WaterLedger {
            self.step_with(w, None)
        }

        fn step_with(&mut self, w: &mut [f64], manual: Option<&[f64; CELL_COUNT]>) -> WaterLedger {
            step(
                w,
                &self.cfg,
                Drivers { terrain: &self.terrain, light: &self.light, rain_source: &self.source },
                manual,
                &mut self.rain,
                &self.graph,
                &mut self.scratch,
            )
        }
    }

    fn total(w: &[f64]) -> f64 {
        w.iter().sum()
    }

    fn face_rows(face: Face, cy: u8) -> impl Iterator<Item = usize> {
        (0..16u8).map(move |cx| CellId::new(face, cx, cy).index())
    }

    #[test]
    fn rain_falls_only_where_the_blob_sum_exceeds_the_threshold() {
        let mut h = Harness::new(0.0);
        h.cfg.flow = 0.0;
        h.cfg.evap = 0.0;
        let wet = CellId::new(Face::Front, 3, 3).index();
        let damp = CellId::new(Face::Front, 9, 9).index();
        h.source[wet] = h.cfg.rain_threshold + 0.5;
        h.source[damp] = h.cfg.rain_threshold; // exactly at the threshold: dry
        let mut w = vec![0.0; CELL_COUNT];
        let ledger = h.step(&mut w);
        let expected = h.cfg.rain_rate * 0.5 * DT;
        assert!((w[wet] - expected).abs() < 1e-15, "{}", w[wet]);
        assert!((f64::from(h.rain[wet]) - h.cfg.rain_rate * 0.5).abs() < 1e-6);
        assert_eq!(w[damp], 0.0);
        assert_eq!(h.rain[damp], 0.0);
        assert_eq!(w.iter().filter(|&&x| x > 0.0).count(), 1);
        assert!((ledger.rain_in - expected).abs() < 1e-15);
        assert_eq!(ledger.evap_out, 0.0);

        // No rain rate, no rain, whatever the weather.
        h.cfg.rain_rate = 0.0;
        h.source.fill(1.0);
        let mut dry = vec![0.0; CELL_COUNT];
        let ledger = h.step(&mut dry);
        assert!(dry.iter().all(|&x| x == 0.0));
        assert_eq!(ledger.rain_in, 0.0);
        assert!(h.rain.iter().all(|&r| r == 0.0));
    }

    #[test]
    fn a_manual_rate_joins_the_natural_one_and_is_published_and_booked() {
        let mut h = Harness::new(0.0);
        h.cfg.flow = 0.0;
        h.cfg.evap = 0.0;
        let wet = CellId::new(Face::Front, 3, 3).index();
        let dry = CellId::new(Face::Front, 9, 9).index();
        h.source[wet] = h.cfg.rain_threshold + 0.5;
        let natural = h.cfg.rain_rate * 0.5;
        let mut manual = Box::new([0.0f64; CELL_COUNT]);
        manual[wet] = 0.25;
        manual[dry] = 0.125;

        let mut w = vec![0.0; CELL_COUNT];
        let ledger = h.step_with(&mut w, Some(&manual));
        // The published rate is the sum, so the visible shower and the deposited water agree.
        assert!((f64::from(h.rain[wet]) - (natural + 0.25)).abs() < 1e-6, "{}", h.rain[wet]);
        assert!((f64::from(h.rain[dry]) - 0.125).abs() < 1e-9, "{}", h.rain[dry]);
        assert!((w[wet] - (natural + 0.25) * DT).abs() < 1e-15);
        assert!((w[dry] - 0.125 * DT).abs() < 1e-15);
        // Manual water is inside `rain_in` once, and attributed exactly where nothing
        // natural falls.
        assert!((ledger.rain_in - (natural + 0.375) * DT).abs() < 1e-15);
        assert!((ledger.manual_in - 0.375 * DT).abs() < 1e-15);
        assert_eq!(ledger.evap_out, 0.0);

        // `None` and an all-zero array agree on the water; only the branch differs.
        let zeros = Box::new([0.0f64; CELL_COUNT]);
        let mut a = vec![0.0; CELL_COUNT];
        let la = h.step_with(&mut a, None);
        let mut b = vec![0.0; CELL_COUNT];
        let lb = h.step_with(&mut b, Some(&zeros));
        assert_eq!(a, b);
        assert_eq!(la.rain_in, lb.rain_in);
        assert_eq!(la.manual_in, 0.0);
        assert_eq!(lb.manual_in, 0.0);
    }

    #[test]
    fn flow_conserves_water_and_never_goes_negative() {
        let mut h = Harness::new(0.06);
        h.cfg.evap = 0.0;
        // A scattered deterministic field, including some very shallow cells.
        let mut x: u64 = 0x9E37_79B9_7F4A_7C15;
        let mut w: Vec<f64> = (0..CELL_COUNT)
            .map(|_| {
                x ^= x << 13;
                x ^= x >> 7;
                x ^= x << 17;
                let u = (x >> 11) as f64 / (1u64 << 53) as f64;
                if u < 0.3 { 0.0 } else { u * 2.0 }
            })
            .collect();
        let before = total(&w);
        for _ in 0..400 {
            let ledger = h.step(&mut w);
            assert_eq!(ledger, WaterLedger::default(), "flow is a pure transfer");
            assert!(w.iter().all(|&x| x >= 0.0), "negative depth");
            assert!(w.iter().all(|&x| x.is_finite()));
        }
        assert!((total(&w) - before).abs() < 1e-12 * before.max(1.0), "{} vs {before}", total(&w));
    }

    #[test]
    fn water_on_a_top_row_side_cell_reaches_the_bottom_row_within_bounded_ticks() {
        let mut h = Harness::new(0.0);
        h.cfg.evap = 0.0;
        let mut w = vec![0.0; CELL_COUNT];
        w[CellId::new(Face::Right, 6, 0).index()] = 1.0;
        let mut ticks = 0;
        loop {
            h.step(&mut w);
            ticks += 1;
            let bottom: f64 = face_rows(Face::Right, 15).map(|i| w[i]).sum();
            if bottom > 0.9 {
                break;
            }
            assert!(ticks < 20_000, "water never reached the bottom row");
        }
        // Report the count in the test name's assertion so a regression is visible.
        assert!(ticks <= 2400, "reached the bottom row after {ticks} ticks (> 2 simulated minutes)");
        eprintln!("water reached the bottom row of the face after {ticks} ticks");
        assert!((total(&w) - 1.0).abs() < 1e-12);
    }

    #[test]
    fn water_on_a_flat_plateau_equalizes_and_then_stops() {
        let mut h = Harness::new(0.0);
        h.cfg.evap = 0.0;
        // Fill a 4×4 block of the top face and let it spread over the level face. The
        // top face has no downhill neighbors on the side faces (z drops there, so water
        // *does* leave the top over the seams — so pin it: run with only the top face's
        // interior by giving side faces a huge terrain wall).
        for i in 0..CELL_COUNT {
            if CellId(i as u16).face() != Face::Top {
                h.terrain[i] = 10.0;
            }
        }
        let mut w = vec![0.0; CELL_COUNT];
        for cy in 6..10u8 {
            for cx in 6..10u8 {
                w[CellId::new(Face::Top, cx, cy).index()] = 1.0;
            }
        }
        let before = total(&w);
        for _ in 0..40_000 {
            h.step(&mut w);
        }
        let top: Vec<f64> = (0..CELL_COUNT)
            .filter(|&i| CellId(i as u16).face() == Face::Top)
            .map(|i| w[i])
            .collect();
        let mean = top.iter().sum::<f64>() / top.len() as f64;
        assert!((mean - before / 256.0).abs() < 1e-9);
        assert!(top.iter().all(|&x| (x - mean).abs() < 1e-6), "not level: {:?}", top.iter().cloned().fold((f64::MAX, f64::MIN), |(lo, hi), x| (lo.min(x), hi.max(x))));
        assert!(w.iter().enumerate().all(|(i, &x)| CellId(i as u16).face() == Face::Top || x == 0.0));
        // A level field is a fixed point, bit for bit.
        let snapshot = w.clone();
        h.step(&mut w);
        assert_eq!(w, snapshot);
    }

    #[test]
    fn evaporation_follows_light_and_depth_down_to_the_floor_in_the_dark() {
        let mut h = Harness::new(0.0);
        h.cfg.flow = 0.0;
        h.cfg.evap_floor = 0.2;
        h.light.fill(0.0);
        let bright = CellId::new(Face::Top, 2, 2).index();
        let dim = CellId::new(Face::Top, 4, 4).index();
        let dark = CellId::new(Face::Top, 6, 6).index();
        h.light[bright] = 1.0;
        h.light[dim] = 0.5;
        let mut w = vec![0.0; CELL_COUNT];
        w[bright] = 2.0;
        w[dim] = 2.0;
        w[dark] = 2.0;
        let ledger = h.step(&mut w);
        let full = h.cfg.evap * 2.0 * DT;
        assert!((w[bright] - (2.0 - full)).abs() < 1e-15);
        assert!((w[dim] - (2.0 - full / 2.0)).abs() < 1e-15);
        // In the dark, evaporation is `evap · evap_floor · w`, not zero.
        assert!((w[dark] - (2.0 - full * 0.2)).abs() < 1e-15, "{}", w[dark]);
        assert!((ledger.evap_out - (1.0 + 0.5 + 0.2) * full).abs() < 1e-15);
        assert_eq!(ledger.rain_in, 0.0);
        // Light below the floor is lifted to it: a cell at L = 0.1 dries like one at 0.2.
        let mut h2 = Harness::new(0.0);
        h2.cfg.flow = 0.0;
        h2.cfg.evap_floor = 0.2;
        h2.light.fill(0.1);
        let mut w2 = vec![1.0; CELL_COUNT];
        let l2 = h2.step(&mut w2);
        assert!((l2.evap_out - CELL_COUNT as f64 * h2.cfg.evap * 0.2 * DT).abs() < 1e-9);
        // A zero floor restores the bare light law.
        h2.cfg.evap_floor = 0.0;
        let mut w3 = vec![1.0; CELL_COUNT];
        let l3 = h2.step(&mut w3);
        assert!((l3.evap_out - CELL_COUNT as f64 * h2.cfg.evap * 0.1 * DT).abs() < 1e-9);
    }

    #[test]
    fn check_rejects_negative_and_non_finite_depths() {
        assert!(check(&vec![0.0; CELL_COUNT]).is_ok());
        let mut w = vec![0.0; CELL_COUNT];
        w[5] = -1e-3;
        assert!(check(&w).unwrap_err().contains("negative"));
        w[5] = f64::NAN;
        assert!(check(&w).unwrap_err().contains("finite"));
        assert!(check(&[0.0; 3]).is_err());
    }
}
