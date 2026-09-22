//! Hydraulic erosion on a [`Heightfield`], in metres.
//!
//! A CPU solver with a fixed work budget. **Geological time is iterations**, never
//! ticks: nothing in the world experiences this, and the model water each iteration
//! rains, routes and discards is a modelling tool, not the live inventory. Slice 3 fills
//! the basins from the world's own water budget.
//!
//! One iteration is: uniform model rain; a Priority-Flood to find how high each closed
//! depression fills before it spills; routing down the derived drainage surface;
//! stream-power entrainment proportional to slope times discharge, taking loose sediment
//! first and then bedrock divided by its hardness; transport; deposition wherever the
//! carrying capacity has fallen below the load; and relaxation of sediment steeper than
//! the angle of repose.
//!
//! `x` wraps in every pass. Front and back are walls. **A ring has no exterior drain**:
//! there is no boundary to seed the flood from and no edge to export to, so the flood is
//! seeded at the single lowest column and water that reaches a closed basin stops there
//! and drops what it carried. Nothing is exported, which is why removed always equals
//! deposited.
//!
//! Every iteration keeps the books: material removed from bedrock and from sediment
//! equals material deposited plus material still in transport, no layer goes negative,
//! and bedrock never rises. The totals live on the heightfield so the tests and the
//! `erosion_map` example can read them.

use std::collections::BinaryHeap;

use crate::generate::Heightfield;
use crate::recipe::Erosion;

/// The gradient the drainage surface gives a flat, metres per sample. Small enough that
/// no terrain notices it, large enough to order a lake's cells toward its spill.
const DRAIN_EPS: f64 = 1e-6;

/// Run `e.iterations` of erosion over `field`.
///
/// `hardness(x, z, at_m)` reads the recipe's hardness field at the height it is asked
/// for — the bedrock surface, which is the cell the water is cutting. The solver never
/// sees the recipe, so a test can hand it a constant.
///
/// `soft_hardness` is the line between the two per-iteration caps
/// ([`Erosion::max_cut_soft_m`] and [`Erosion::max_cut_hard_m`]). The caller passes
/// `hollows.soft_hardness`, so the rock the water cuts fast is the same rock the carve
/// later notches out: one definition of soft, two uses.
///
/// `iterations = 0` is the identity: the layers, the budget and the spill levels are
/// left exactly as they were. The hardness and the hard-cap flags are read either way —
/// they describe the field, they are not something erosion did to it.
pub fn erode(
    field: &mut Heightfield,
    e: &Erosion,
    soft_hardness: f64,
    hardness: impl Fn(usize, usize, f64) -> f64,
) {
    let (w, d) = (field.width, field.depth);
    let n = w * d;
    if n == 0 {
        return;
    }
    read_hardness(field, &hardness);
    if e.iterations == 0 {
        return;
    }

    let cell = field.cell_m;
    let mut surface = vec![0.0f64; n];
    let mut filled = vec![0.0f64; n];
    let mut drain = vec![0.0f64; n];
    let mut seen = vec![false; n];
    let mut heap: BinaryHeap<Node> = BinaryHeap::with_capacity(n);
    let mut order: Vec<u32> = (0..n as u32).collect();
    let mut load = vec![0.0f64; n];
    let mut delta = vec![0.0f64; n];

    for _ in 0..e.iterations {
        for i in 0..n {
            surface[i] = field.bedrock_m[i] + field.sediment_m[i];
        }
        flood(
            &surface,
            w,
            d,
            &mut filled,
            &mut drain,
            &mut seen,
            &mut heap,
        );
        field.spill_m.copy_from_slice(&filled);

        // Upstream first, so a column's discharge and load are complete when it is cut.
        order.sort_by(|&a, &b| drain[b as usize].total_cmp(&drain[a as usize]));
        for (i, item) in field.discharge.iter_mut().enumerate() {
            *item = e.rain;
            load[i] = 0.0;
        }

        let (mut cut_bedrock, mut cut_sediment, mut dropped) = (0.0, 0.0, 0.0);
        for &oi in &order {
            let i = oi as usize;
            let down = receiver(i, w, d, &drain);
            // The real surface, not the drainage one: inside a lake the slope is flat,
            // the capacity is nothing, and the load falls out where it arrived.
            let slope = match down {
                Some(j) => ((surface[i] - surface[j]) / cell).max(0.0),
                None => 0.0,
            };
            let capacity = e.capacity * field.discharge[i] * slope;
            // Layer-aware: the cap is the bed's, read where the water is cutting.
            let hard = field.hardness[i].clamp(0.0, 1.0);
            let cap = if hard <= soft_hardness {
                e.max_cut_soft_m
            } else {
                e.max_cut_hard_m
            };

            if load[i] < capacity {
                let want = ((capacity - load[i]) * e.erode).min(cap);
                let from_sediment = want.min(field.sediment_m[i]);
                field.sediment_m[i] -= from_sediment;
                cut_sediment += from_sediment;
                let mut taken = from_sediment;
                let left = want - from_sediment;
                if left > 0.0 {
                    // Hardness slows the bed, it never stops the water.
                    let from_bedrock = left / (1.0 + e.bedrock_resistance * hard);
                    field.bedrock_m[i] -= from_bedrock;
                    cut_bedrock += from_bedrock;
                    taken += from_bedrock;
                }
                load[i] += taken;
            } else {
                let drop = (load[i] - capacity) * e.deposit;
                field.sediment_m[i] += drop;
                dropped += drop;
                load[i] -= drop;
            }
            match down {
                Some(j) => {
                    field.discharge[j] += field.discharge[i];
                    load[j] += load[i];
                    load[i] = 0.0;
                }
                None => {
                    // The bottom of a ring: nowhere left to go, so it all lands here.
                    field.sediment_m[i] += load[i];
                    dropped += load[i];
                    load[i] = 0.0;
                }
            }
        }

        field.budget.removed_bedrock_m += cut_bedrock;
        field.budget.removed_sediment_m += cut_sediment;
        field.budget.deposited_m += dropped;
        field.budget.in_transport_m = load.iter().sum();

        for _ in 0..e.repose_sweeps {
            relax(field, e.repose, &mut surface, &mut delta);
        }
        read_hardness(field, &hardness);
    }
    // One last flood, so the spill levels describe the field as it now stands rather
    // than as it stood before the final iteration cut into it.
    for i in 0..n {
        surface[i] = field.bedrock_m[i] + field.sediment_m[i];
    }
    flood(
        &surface,
        w,
        d,
        &mut filled,
        &mut drain,
        &mut seen,
        &mut heap,
    );
    field.spill_m.copy_from_slice(&filled);
}

fn read_hardness(field: &mut Heightfield, hardness: &impl Fn(usize, usize, f64) -> f64) {
    let w = field.width;
    for z in 0..field.depth {
        for x in 0..w {
            let i = z * w + x;
            field.hardness[i] = hardness(x, z, field.bedrock_m[i]);
        }
    }
}

/// The four neighbours of a sample: `x` wraps, front and back are walls.
fn neighbours(i: usize, w: usize, d: usize) -> [Option<usize>; 4] {
    let (x, z) = (i % w, i / w);
    [
        Some(z * w + (x + w - 1) % w),
        Some(z * w + (x + 1) % w),
        if z > 0 { Some((z - 1) * w + x) } else { None },
        if z + 1 < d {
            Some((z + 1) * w + x)
        } else {
            None
        },
    ]
}

/// Where a sample sends its water: the neighbour lowest on the drainage surface. `None`
/// only at the ring's single lowest column, which has nowhere to send it.
fn receiver(i: usize, w: usize, d: usize, drain: &[f64]) -> Option<usize> {
    let mut best: Option<usize> = None;
    for nb in neighbours(i, w, d).into_iter().flatten() {
        if drain[nb] < drain[i] && best.is_none_or(|b| drain[nb] < drain[b]) {
            best = Some(nb);
        }
    }
    best
}

/// A cell waiting in the flood, ordered lowest-first.
struct Node {
    filled: f64,
    drain: f64,
    i: usize,
}

impl PartialEq for Node {
    fn eq(&self, other: &Node) -> bool {
        self.cmp(other) == std::cmp::Ordering::Equal
    }
}
impl Eq for Node {}
impl PartialOrd for Node {
    fn partial_cmp(&self, other: &Node) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for Node {
    // Reversed: `BinaryHeap` is a max-heap and the flood wants the lowest cell. Two
    // entries that compare equal here have the same filled and drain levels, so which
    // one is popped first cannot change the result -- that is what keeps the flood
    // independent of where the array happens to start.
    fn cmp(&self, other: &Node) -> std::cmp::Ordering {
        other
            .filled
            .total_cmp(&self.filled)
            .then(other.drain.total_cmp(&self.drain))
    }
}

/// Priority-Flood over a closed ring.
///
/// `filled[i]` is the level column `i` fills to before it spills — its own surface
/// outside a depression, the depression's spill level inside one. The real depressions
/// survive; they are recorded, not filled in. `drain[i]` is the derived drainage
/// surface: the same thing with a gradient no terrain would notice added along the
/// flood's own path, so every column but the sink has a strictly lower neighbour and a
/// lake routes toward its outlet instead of stalling on a flat.
///
/// There is no boundary to seed from: `x` wraps and both `z` edges are walls. The seed
/// is the ring's lowest column, which is the only place water can end up.
fn flood(
    surface: &[f64],
    w: usize,
    d: usize,
    filled: &mut [f64],
    drain: &mut [f64],
    seen: &mut [bool],
    heap: &mut BinaryHeap<Node>,
) {
    let n = w * d;
    seen.fill(false);
    heap.clear();
    let sink = (0..n)
        .min_by(|&a, &b| surface[a].total_cmp(&surface[b]))
        .expect("a non-empty field");
    seen[sink] = true;
    filled[sink] = surface[sink];
    drain[sink] = surface[sink];
    heap.push(Node {
        filled: surface[sink],
        drain: surface[sink],
        i: sink,
    });
    while let Some(c) = heap.pop() {
        for nb in neighbours(c.i, w, d).into_iter().flatten() {
            if seen[nb] {
                continue;
            }
            seen[nb] = true;
            filled[nb] = surface[nb].max(c.filled);
            drain[nb] = surface[nb].max(c.drain + DRAIN_EPS);
            heap.push(Node {
                filled: filled[nb],
                drain: drain[nb],
                i: nb,
            });
        }
    }
}

/// Loose sediment does not stand steeper than its angle of repose.
///
/// One simultaneous sweep: every pair's move is measured against the same surface and
/// applied afterwards, so the result does not depend on where the sweep started — which
/// is what a ring needs, there being no start. Exactly conservative: what one column
/// loses the next gains.
fn relax(field: &mut Heightfield, repose: f64, surface: &mut [f64], delta: &mut [f64]) {
    let (w, d, cell) = (field.width, field.depth, field.cell_m);
    let limit = repose * cell;
    for i in 0..w * d {
        surface[i] = field.bedrock_m[i] + field.sediment_m[i];
        delta[i] = 0.0;
    }
    for z in 0..d {
        for x in 0..w {
            let i = z * w + x;
            let pairs = [
                Some(z * w + (x + 1) % w),
                if z + 1 < d {
                    Some((z + 1) * w + x)
                } else {
                    None
                },
            ];
            for j in pairs.into_iter().flatten() {
                let diff = surface[i] - surface[j];
                if diff.abs() <= limit {
                    continue;
                }
                let (high, low) = if diff > 0.0 { (i, j) } else { (j, i) };
                // A quarter of the excess, and never more than a quarter of what the
                // column has: a cell is the high side of at most four pairs, so it
                // cannot promise away more sediment than it holds.
                let excess = diff.abs() - limit;
                let moved = (excess * 0.25).min(field.sediment_m[high] * 0.25);
                if moved > 0.0 {
                    delta[high] -= moved;
                    delta[low] += moved;
                }
            }
        }
    }
    for i in 0..w * d {
        field.sediment_m[i] = (field.sediment_m[i] + delta[i]).max(0.0);
    }
}

/// Flag the banks a hard cap stands on: hard rock at the surface with a neighbour cut at
/// least `drop_m` below it. One of the two sources of undercut sites
/// ([`crate::hollows::carve`]); the other is the hardness layering itself.
///
/// Not something erosion does to the field — a read of it — so it is its own call and
/// runs whatever the iteration budget was.
pub fn flag_hard_caps(field: &mut Heightfield, drop_m: f64, min_hardness: f64) {
    let (w, d) = (field.width, field.depth);
    let surface: Vec<f64> = (0..w * d).map(|i| field.surface_m(i)).collect();
    for i in 0..w * d {
        let capped = field.hardness[i] >= min_hardness
            && neighbours(i, w, d)
                .into_iter()
                .flatten()
                .any(|nb| surface[i] - surface[nb] >= drop_m);
        field.hard_cap[i] = capped;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::generate::Budget;

    /// A tiny field: `w x d` samples of `cell_m`, a surface from `elevation`, and a
    /// uniform mantle of sediment on top of the bedrock.
    fn field(
        w: usize,
        d: usize,
        cell_m: f64,
        mantle_m: f64,
        elevation: impl Fn(usize, usize) -> f64,
    ) -> Heightfield {
        let mut surface = vec![0.0; w * d];
        for z in 0..d {
            for x in 0..w {
                surface[z * w + x] = elevation(x, z);
            }
        }
        Heightfield {
            width: w,
            depth: d,
            cell_m,
            circumference_m: w as f64 * cell_m,
            bedrock_m: surface.iter().map(|e| e - mantle_m).collect(),
            sediment_m: vec![mantle_m; w * d],
            hardness: vec![0.5; w * d],
            discharge: vec![0.0; w * d],
            spill_m: surface,
            hard_cap: vec![false; w * d],
            pool_rock: vec![i32::MAX; w * d],
            pool_spill: vec![i32::MIN; w * d],
            budget: Budget::default(),
        }
    }

    fn budget() -> Erosion {
        Erosion {
            iterations: 50,
            ..Erosion::DEFAULT
        }
    }

    fn constant_hardness(h: f64) -> impl Fn(usize, usize, f64) -> f64 {
        move |_, _, _| h
    }

    /// A plane tilted across the ring, with a shallow dip so the flow has somewhere to
    /// go: `x = 2` is the low point and the descent to it crosses `x = 0`.
    fn ramp(w: usize) -> impl Fn(usize, usize) -> f64 {
        let w = w as f64;
        move |x, z| {
            // Distance around the ring from the low column, either way: a wedge whose
            // one descent runs through x = w-1 into x = 0, 1, 2.
            let from_low = ((x as f64 - 2.0) + w) % w;
            let around = from_low.min(w - from_low);
            3.0 + 0.25 * around + 0.02 * z as f64
        }
    }

    /// Removed equals deposited plus what is still in transport; no layer goes negative;
    /// bedrock never rises; and the total thickness of the two layers is untouched.
    #[test]
    fn erosion_moves_material_and_loses_none() {
        let start = field(16, 4, 0.25, 0.3, ramp(16));
        let mut f = start.clone();
        erode(&mut f, &budget(), 0.45, constant_hardness(0.5));

        let b = f.budget;
        let moved = b.removed_bedrock_m + b.removed_sediment_m;
        assert!(moved > 0.0, "50 iterations moved nothing");
        assert!(
            b.imbalance_m().abs() <= 1e-9 * moved.max(1.0),
            "{b:?} does not balance: {:.3e}",
            b.imbalance_m()
        );
        let total =
            |g: &Heightfield| -> f64 { (0..g.samples()).map(|i| g.surface_m(i)).sum::<f64>() };
        assert!(
            (total(&f) - total(&start)).abs() <= 1e-9 * total(&start).abs(),
            "the ring gained or lost ground"
        );
        for i in 0..f.samples() {
            assert!(f.sediment_m[i] >= 0.0, "negative sediment at {i}");
            assert!(f.sediment_m[i].is_finite() && f.bedrock_m[i].is_finite());
            assert!(
                f.bedrock_m[i] <= start.bedrock_m[i] + 1e-12,
                "bedrock rose at {i}"
            );
        }
    }

    /// The seam is not a wall: the descent that runs off the end of the array keeps
    /// going into `x = 0`, and what it carries lands there.
    #[test]
    fn flux_crosses_the_seam() {
        let start = field(16, 4, 0.25, 0.3, ramp(16));
        let mut f = start.clone();
        erode(&mut f, &budget(), 0.45, constant_hardness(0.5));
        let gained: f64 = (0..4)
            .map(|z| f.sediment_m[z * 16 + 2] - start.sediment_m[z * 16 + 2])
            .sum();
        assert!(
            gained > 0.0,
            "nothing was deposited past the seam: {gained:.4} m"
        );
        let lost: f64 = (0..4)
            .map(|z| start.sediment_m[z * 16 + 10] - f.sediment_m[z * 16 + 10])
            .sum();
        assert!(lost > 0.0, "nothing left the far side of the ring");
    }

    /// Shifting the ring and eroding is eroding and shifting.
    #[test]
    fn a_shifted_field_erodes_to_the_shifted_result() {
        let (w, d, shift) = (16usize, 4usize, 5usize);
        let e = ramp(w);
        let mut plain = field(w, d, 0.25, 0.3, &e);
        let mut shifted = field(w, d, 0.25, 0.3, |x, z| e((x + w - shift) % w, z));
        erode(&mut plain, &budget(), 0.45, constant_hardness(0.5));
        erode(&mut shifted, &budget(), 0.45, constant_hardness(0.5));
        for z in 0..d {
            for x in 0..w {
                let a = plain.surface_m(z * w + x);
                let b = shifted.surface_m(z * w + (x + shift) % w);
                assert!(
                    (a - b).abs() < 1e-9,
                    "({x}, {z}): {a:.9} against the shifted {b:.9}"
                );
            }
        }
    }

    /// A ring has no exterior drain. A bowl keeps what runs into it: its floor gains
    /// sediment, the ground above it loses some, and the level it would fill to before
    /// overflowing is its own sill — not some fictitious edge outlet.
    ///
    /// The ring's real sink is a deeper pit further along, which is what makes the bowl
    /// a closed *basin* rather than the bottom of the world; a bowl that is itself the
    /// lowest ground has nothing to spill over and a spill level would mean nothing.
    #[test]
    fn a_closed_basin_fills_its_floor_and_knows_its_spill_level() {
        let (w, d) = (16usize, 4usize);
        // Plateau at 5 m; a bowl at 3 m from x = 5 to 10; its sill at x = 11, 4.6 m,
        // then down over 4 m to the ring's own sink at 1 m. Every other way round to
        // that sink crosses the 5 m plateau, so the bowl spills at 4.6 m.
        let shape = |x: usize, _z: usize| match x {
            5..=10 => 3.0,
            11 => 4.6,
            12 => 4.0,
            13..=15 => 1.0,
            _ => 5.0,
        };
        let start = field(w, d, 0.25, 0.3, shape);
        let mut f = start.clone();
        // Fewer iterations than the other fixtures: run this long enough to move
        // material and short enough that the ring's own sink has not silted up into
        // something else, which would leave the bowl nothing to be a basin against.
        let e = Erosion {
            iterations: 12,
            ..budget()
        };
        erode(&mut f, &e, 0.45, constant_hardness(0.5));

        let floor: f64 = (5..=10)
            .map(|x| f.sediment_m[w + x] - start.sediment_m[w + x])
            .sum();
        assert!(floor > 0.0, "the basin floor gained nothing: {floor:.4} m");
        let above: f64 = (0..d)
            .flat_map(|z| (0..4).map(move |x| (x, z)))
            .map(|(x, z)| start.sediment_m[z * w + x] - f.sediment_m[z * w + x])
            .sum();
        assert!(
            above > 0.0,
            "the ground above the bowl lost nothing: {above:.4} m"
        );
        for i in 0..f.samples() {
            assert!(f.surface_m(i).is_finite(), "NaN at {i}");
        }
        // Water leaves over whichever row of the sill stands lowest, so that is the
        // one level the whole basin fills to.
        let sill = (0..d)
            .map(|z| f.surface_m(z * w + 11))
            .fold(f64::MAX, f64::min);
        for z in 0..d {
            let spill = f.spill_m[z * w + 7];
            assert!(
                (spill - sill).abs() < 1e-9,
                "z {z}: the basin spills at {spill:.4} m, its sill stands at {sill:.4} m"
            );
            assert!(
                spill > f.surface_m(z * w + 7),
                "z {z}: the spill level is not above the floor it would flood"
            );
        }
    }

    /// A hard band over a soft one becomes a bank.
    ///
    /// The channel cuts the soft rock as fast as the water can carry it away and the hard
    /// rock at a cap it cannot exceed, so where the bed has already worked down into the
    /// soft band it keeps going and the capped ground beside it does not. The same
    /// fixture in rock of one hardness gets the same water and stays the ramp it was.
    ///
    /// Two things the fixture has to do for the caps to mean anything, and neither of
    /// them is true of the presets. **Bare bedrock, no mantle**: with loose sediment over
    /// it the flow satisfies itself from the sediment first and the rock is never
    /// reached — measured on the presets, 0.34 m of bedrock incision over a hundred
    /// iterations, over the whole ring. **A low hard cap**: a cap only does anything
    /// while it is the binding constraint, and at the presets' 0.05 m the water never
    /// asks for that much, so the two caps are the same cap.
    #[test]
    fn a_cap_over_soft_rock_becomes_a_step_and_uniform_rock_does_not() {
        let (w, d, cell) = (16usize, 4usize, 0.25);
        let soft_hardness = 0.45;
        let run = |layered: bool| -> (Heightfield, Heightfield) {
            let start = field(w, d, cell, 0.0, ramp(w));
            let mut f = start.clone();
            let e = Erosion {
                iterations: 100,
                max_cut_hard_m: 0.001,
                ..Erosion::DEFAULT
            };
            // Hard above 4 m, soft below, so the upper ramp keeps its cap while the
            // mid-slope the channel runs down is in the soft band.
            erode(&mut f, &e, soft_hardness, move |_, _, at_m| {
                if !layered {
                    0.5
                } else if at_m >= 4.0 {
                    0.95
                } else {
                    0.15
                }
            });
            (start, f)
        };
        let worst_step = |f: &Heightfield, bedrock: bool| -> f64 {
            (0..d)
                .flat_map(|z| (0..w).map(move |x| (x, z)))
                .map(|(x, z)| {
                    let at = |i: usize| {
                        if bedrock {
                            f.bedrock_m[i]
                        } else {
                            f.surface_m(i)
                        }
                    };
                    (at(z * w + x) - at(z * w + (x + 1) % w)).abs()
                })
                .fold(0.0, f64::max)
                / cell
        };

        let (start, layered) = run(true);
        let (_, uniform) = run(false);
        let (bank, flat) = (worst_step(&layered, true), worst_step(&uniform, true));
        assert!(
            bank >= 2.0,
            "the cap made no bank: the worst bedrock step is {bank:.2} voxels"
        );
        assert!(
            bank >= flat * 1.7,
            "layered rock stepped {bank:.2} voxels against uniform rock's {flat:.2}: \
             not the layering doing it"
        );
        assert!(
            flat <= 1.25,
            "one hardness should leave the ramp a ramp, not a {flat:.2} voxel step"
        );
        assert!(
            worst_step(&layered, false) >= 1.7,
            "the bank does not reach the surface"
        );

        // And the books still balance under the two caps.
        let b = layered.budget;
        let moved = b.removed_bedrock_m + b.removed_sediment_m;
        assert!(
            b.imbalance_m().abs() <= 1e-9 * moved.max(1.0),
            "{b:?} does not balance"
        );
        let total = |g: &Heightfield| (0..g.samples()).map(|i| g.surface_m(i)).sum::<f64>();
        assert!(
            (total(&layered) - total(&start)).abs() <= 1e-9 * total(&start).abs().max(1.0),
            "the ring gained or lost ground"
        );
        for i in 0..layered.samples() {
            assert!(layered.sediment_m[i] >= 0.0, "negative sediment at {i}");
            assert!(layered.surface_m(i).is_finite());
            assert!(
                layered.bedrock_m[i] <= start.bedrock_m[i] + 1e-12,
                "bedrock rose at {i}"
            );
        }
    }

    /// Hardness limits what the water can cut.
    #[test]
    fn soft_bedrock_loses_more_than_hard() {
        let cut = |h: f64| -> f64 {
            let start = field(16, 4, 0.25, 0.05, ramp(16));
            let mut f = start.clone();
            erode(&mut f, &budget(), 0.45, constant_hardness(h));
            (0..f.samples())
                .map(|i| start.bedrock_m[i] - f.bedrock_m[i])
                .sum::<f64>()
        };
        let (soft, hard) = (cut(0.2), cut(0.9));
        assert!(soft > 0.0, "the soft slope lost no bedrock");
        assert!(
            soft > hard * 1.2,
            "soft rock lost {soft:.4} m, hard rock {hard:.4} m"
        );
    }

    /// Loose sediment does not stand in a wall.
    #[test]
    fn a_sediment_step_relaxes_to_the_angle_of_repose() {
        let (w, d, cell) = (16usize, 4usize, 0.25);
        let mut f = field(w, d, cell, 0.0, |_, _| 3.0);
        for z in 0..d {
            for x in 0..w / 2 {
                f.sediment_m[z * w + x] = 1.0;
            }
        }
        let e = Erosion {
            // Repose alone: no rain, so nothing but the relaxation moves.
            rain: 0.0,
            ..budget()
        };
        erode(&mut f, &e, 0.45, constant_hardness(0.5));
        let limit = e.repose * cell + cell;
        for z in 0..d {
            for x in 0..w {
                let here = f.surface_m(z * w + x);
                let right = f.surface_m(z * w + (x + 1) % w);
                assert!(
                    (here - right).abs() <= limit,
                    "({x}, {z}): {here:.3} m beside {right:.3} m exceeds the repose limit {limit:.3} m"
                );
            }
        }
    }

    /// Repose moves loose sediment and nothing else, so a bench face stands.
    ///
    /// Six voxels of bedrock step under a one-voxel veneer: afterwards the bedrock step
    /// is exactly where it was, and the veneer has run off the top of the face and
    /// piled at its foot. A relaxation that worked on the ground surface instead would
    /// grade the whole thing away, and there would be no cliffs anywhere.
    #[test]
    fn repose_moves_the_veneer_and_leaves_the_bedrock_face_standing() {
        let (w, d, cell) = (16usize, 4usize, 0.25);
        let step = 6.0 * cell;
        let mut f = field(
            w,
            d,
            cell,
            cell,
            |x, _| {
                if x < 8 { 3.0 + step } else { 3.0 }
            },
        );
        let before = f.clone();
        let e = Erosion {
            rain: 0.0,
            repose_sweeps: 4,
            ..budget()
        };
        erode(&mut f, &e, 0.45, constant_hardness(0.5));

        for i in 0..f.samples() {
            assert_eq!(
                f.bedrock_m[i].to_bits(),
                before.bedrock_m[i].to_bits(),
                "the bedrock moved at {i}"
            );
        }
        let top = f.sediment_m[7];
        let foot = f.sediment_m[8];
        assert!(
            top < before.sediment_m[7],
            "the veneer stayed on the lip: {top:.3} m"
        );
        assert!(
            foot > before.sediment_m[8],
            "nothing reached the foot of the face: {foot:.3} m"
        );
        // The face loses the veneer off its lip and the veneer piled at its foot, and
        // nothing else: two voxels out of six, not six out of six.
        let face = f.surface_m(7) - f.surface_m(8);
        assert!(
            face >= step - 2.0 * cell,
            "the face fell to {face:.3} m from {step:.3} m of bedrock"
        );
    }

    /// Zero iterations changes nothing, and the same input twice gives the same output.
    #[test]
    fn no_iterations_is_the_identity_and_the_solver_is_deterministic() {
        let start = field(16, 4, 0.25, 0.3, ramp(16));
        let mut idle = start.clone();
        erode(
            &mut idle,
            &Erosion {
                iterations: 0,
                ..budget()
            },
            0.45,
            constant_hardness(0.5),
        );
        assert_eq!(idle.bedrock_m, start.bedrock_m);
        assert_eq!(idle.sediment_m, start.sediment_m);
        assert_eq!(idle.budget, Budget::default());

        let mut a = start.clone();
        let mut b = start.clone();
        erode(&mut a, &budget(), 0.45, constant_hardness(0.5));
        erode(&mut b, &budget(), 0.45, constant_hardness(0.5));
        assert_eq!(a.bedrock_m, b.bedrock_m);
        assert_eq!(a.sediment_m, b.sediment_m);
    }

    /// The bank slice 2b undercuts: hard rock standing over a neighbour cut well below.
    #[test]
    fn a_hard_bank_over_a_cut_is_flagged() {
        let (w, d) = (16usize, 4usize);
        let mut f = field(w, d, 0.25, 0.0, |x, _| if x < 8 { 5.0 } else { 3.0 });
        f.hardness.fill(0.95);
        flag_hard_caps(&mut f, 0.75, 0.6);
        assert!(
            f.hard_cap[7],
            "the hard bank over a 2 m drop is not flagged"
        );
        assert!(!f.hard_cap[3], "flat hard ground is not a cap");
    }
}
