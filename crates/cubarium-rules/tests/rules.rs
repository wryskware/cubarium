//! The water rules on the host: each rule on the smallest thing that shows it works, and
//! the drivers against each other. Exchange proposals have a fixed summation order;
//! changing the worker schedule must preserve every water cell's bits.

use cubarium_rules::water::host::{ExchangeBackend, Serial};
use cubarium_rules::water::{self, Grid, Params};

// ------------------------------------------------------------------ store arithmetic

#[test]
fn fill_and_empty_stop_at_the_store_bounds_and_return_what_moved() {
    let (after, moved) = water::fill(0.75, 0.5, 1.0);
    assert_eq!(after, 1.0);
    assert!((moved - 0.25).abs() < 1e-15);
    let (after, moved) = water::empty(0.25, 0.5, 2.0);
    assert!((after - 0.0).abs() < 1e-15);
    assert!((moved - 0.5).abs() < 1e-15, "a quarter of a 2 m³ store is 0.5 m³");
    assert_eq!(water::transfer_want(3.0, 2.0, 1.0), 1.0);
}
// ------------------------------------------------------------------ the exchange

/// A water grid the exchange's drivers step: fills, and per-column void and wet masks, the
/// wet ones kept by the driver.
#[derive(Clone)]
struct Pond {
    grid: Grid,
    free: Vec<f64>,
    void: Vec<u128>,
    wet: Vec<u128>,
}

impl Pond {
    fn cell(&self, x: usize, y: usize, z: usize) -> usize {
        y * self.grid.plane() + z * self.grid.width + x
    }
    fn total(&self) -> f64 {
        self.free.iter().sum()
    }
    fn masks_from_free(&self) -> Vec<u128> {
        let plane = self.grid.plane();
        let mut m = vec![0u128; plane];
        for (i, &f) in self.free.iter().enumerate() {
            if f > 0.0 {
                m[i % plane] |= 1u128 << (i / plane);
            }
        }
        m
    }
    /// One exchange; the driver keeps the wet masks and reports the change in the count.
    fn step(&mut self, b: &mut dyn ExchangeBackend, p: &Params) {
        let before: u32 = self.wet.iter().map(|m| m.count_ones()).sum();
        let delta = b.exchange(self.grid, &mut self.free, &self.void, &mut self.wet, p);
        let after: u32 = self.wet.iter().map(|m| m.count_ones()).sum();
        assert_eq!(after as isize - before as isize, delta, "the reported count change");
    }
    /// Seeded rock and water: a floor, rock pillars and ledges, fills in `0..=1`.
    fn seeded(width: usize, depth: usize, height: usize, seed: u64) -> Pond {
        let grid = Grid { width, depth, height,
        };
        let plane = grid.plane();
        let mut s = seed;
        let mut next = move || {
            s = s.wrapping_add(0x9E37_79B9_7F4A_7C15);
            let mut z = s;
            z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
            z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
            ((z ^ (z >> 31)) >> 11) as f64 / (1u64 << 53) as f64
        };
        let mut solid = vec![false; grid.cells()];
        let mut free = vec![0.0; grid.cells()];
        for col in 0..plane {
            let ground = 1 + (next() * 3.0) as usize;
            for y in 0..height {
                let i = y * plane + col;
                if y < ground || (y > ground + 2 && next() < 0.08) {
                    solid[i] = true;
                } else if next() < 0.35 {
                    free[i] = if next() < 0.3 { 1.0 } else { next() };
                }
            }
        }
        let mut void = vec![0u128; plane];
        for (i, &s) in solid.iter().enumerate() {
            if !s {
                void[i % plane] |= 1u128 << (i / plane);
            }
        }
        let mut pond = Pond { grid, free, void, wet: Vec::new(),
        };
        pond.wet = pond.masks_from_free();
        pond
    }
}

const P: Params = Params {
    min_spread: 1e-6,
    transfer_cap: 0.0,
};

#[test]
fn the_serial_exchange_conserves_water_and_keeps_the_wet_masks() {
    let mut pond = Pond::seeded(12, 5, 16, 7);
    let total = pond.total();
    let mut serial = Serial::default();
    for step in 0..150 {
        pond.step(&mut serial, &P);
        assert!(pond.free.iter().all(|f| (0.0..=1.0).contains(f)), "step {step}: a fill left 0..=1");
        assert_eq!(pond.wet, pond.masks_from_free(), "step {step}: the driver kept the wet masks");
        let drift = (pond.total() - total).abs();
        assert!(drift <= 1e-12 * total, "step {step}: water drifted by {drift}");
    }
    // It moved: the water is not where it started.
    let start = Pond::seeded(12, 5, 16, 7);
    let moved: f64 = pond.free.iter().zip(&start.free).map(|(a, b)| (a - b).abs()).sum();
    assert!(moved > 1.0, "the exchange moved water ({moved})");
}

/// A full column beside a dry one: one substep moves water across the shared faces and
/// down nothing (the floor is rock), and what one column loses the other gains. Levelling
/// itself needs `fall` between exchanges and is `cubarium-voxel`'s settle tests' business.
#[test]
fn a_full_column_pushes_into_its_dry_neighbour() {
    // 3 × 1 × 10: rock at y = 0, column 2 rock to the top; column 0 holds six full cells.
    let grid = Grid {
        width: 3,
        depth: 1,
        height: 10,
    };
    let mut pond = Pond {
        grid,
        free: vec![0.0; grid.cells()],
        void: vec![run(1, 9), run(1, 9), 0],
        wet: Vec::new(),
    };
    for y in 1..7 {
        let i = pond.cell(0, y, 0);
        pond.free[i] = 1.0;
    }
    pond.wet = pond.masks_from_free();
    pond.step(&mut Serial::default(), &P);
    let column = |x: usize| (1..10).map(|y| pond.free[pond.cell(x, y, 0)]).sum::<f64>();
    assert!(column(1) > 0.5, "the dry column received {}", column(1));
    assert!((column(0) + column(1) - 6.0).abs() < 1e-12);
    assert_eq!(pond.wet, pond.masks_from_free());
    // Every row of the pushing column offered across its face: the lowest row the most.
    let low = pond.free[pond.cell(1, 1, 0)];
    assert!(low > 0.0 && pond.free[pond.cell(1, 6, 0)] < low);
}

fn run(y: usize, len: usize) -> u128 {
    water::run_bits(y, len)
}

#[cfg(feature = "par")]
#[test]
fn the_parallel_exchange_agrees_with_the_serial_one() {
    use cubarium_rules::water::host::Parallel;
    let mut a = Pond::seeded(24, 6, 20, 11);
    let mut b = a.clone();
    let total = a.total();
    let (mut serial, mut parallel) = (Serial::default(), Parallel::new(4));
    for step in 0..120 {
        a.step(&mut serial, &P);
        b.step(&mut parallel, &P);
        let worst = a.free.iter().zip(&b.free).map(|(x, y)| (x - y).abs()).fold(0.0, f64::max);
        assert!(worst <= 1e-9, "step {step}: serial and parallel differ by {worst}");
        assert!((b.total() - total).abs() <= 1e-12 * total, "step {step}: parallel drifted");
        assert_eq!(b.wet, b.masks_from_free(), "step {step}: parallel wet masks");
    }
}

#[cfg(feature = "par")]
#[test]
fn parallel_exchange_keeps_serial_proposal_order_on_different_pools() {
    use cubarium_rules::water::host::Parallel;
    let mut expected = Pond::seeded(24, 6, 20, 11);
    let mut four = expected.clone();
    let mut eight = expected.clone();
    let (mut serial, mut p4, mut p8) =
        (Serial::default(), Parallel::new(4), Parallel::new(8));
    for tick in 0..40 {
        expected.step(&mut serial, &P);
        four.step(&mut p4, &P);
        eight.step(&mut p8, &P);
        for (name, pond) in [("four", &four), ("eight", &eight)] {
            for (cell, (&a, &b)) in expected.free.iter().zip(&pond.free).enumerate() {
                assert_eq!(a.to_bits(), b.to_bits(), "tick {tick}, {name}, cell {cell}");
            }
            assert_eq!(expected.wet, pond.wet, "tick {tick}, {name}");
        }
    }
}

/// Several worlds of different sizes stepping the exchange at once in one process, each
/// on its own `Parallel` over the one shared pool (the new-world picker founding its
/// candidates): each lands where it lands stepped alone on one thread, to rounding, and
/// keeps its water. And one `Parallel` stepping two worlds of different sizes in turn (a
/// thread's backend in `cubarium-voxel`, which serves whatever world that thread steps)
/// resizes its scratch and agrees too.
#[cfg(feature = "par")]
#[test]
fn worlds_of_different_sizes_exchange_at_once_on_one_pool() {
    use cubarium_rules::water::host::Parallel;
    const STEPS: usize = 80;
    let sizes = [(24, 6, 20, 11), (40, 3, 30, 12), (9, 11, 12, 13), (70, 5, 64, 14), (33, 7, 128, 15),
    ];
    let alone: Vec<Pond> = sizes
        .iter()
        .map(|&(w, d, h, seed)| {
            let mut pond = Pond::seeded(w, d, h, seed);
            let mut serial = Serial::default();
            for _ in 0..STEPS {
                pond.step(&mut serial, &P);
            }
            pond
        })
        .collect();
    let together: Vec<Pond> = std::thread::scope(|s| {
        let handles: Vec<_> = sizes
            .iter()
            .map(|&(w, d, h, seed)| {
                s.spawn(move || {
                    let mut pond = Pond::seeded(w, d, h, seed);
                    let total = pond.total();
                    let mut parallel = Parallel::new(4);
                    for step in 0..STEPS {
                        pond.step(&mut parallel, &P);
                        let drift = (pond.total() - total).abs();
                        assert!(drift <= 1e-12 * total, "{w}x{d}x{h} step {step}: drifted by {drift}");
                    }
                    pond
                })
            })
            .collect();
        handles.into_iter().map(|h| h.join().expect("a world's thread")).collect()
    });
    for (a, b) in alone.iter().zip(&together) {
        let worst = a.free.iter().zip(&b.free).map(|(x, y)| (x - y).abs()).fold(0.0, f64::max);
        assert!(worst <= 1e-9, "{:?}: alone and together differ by {worst}", a.grid);
        assert_eq!(b.wet, b.masks_from_free(), "{:?}: wet masks", b.grid);
    }

    // One backend, two sizes in turn.
    let (mut big, mut small) = (Pond::seeded(70, 5, 64, 14), Pond::seeded(9, 11, 12, 13));
    let mut shared = Parallel::new(4);
    for _ in 0..STEPS {
        big.step(&mut shared, &P);
        small.step(&mut shared, &P);
    }
    for (a, b) in [(&alone[3], &big), (&alone[2], &small)] {
        let worst = a.free.iter().zip(&b.free).map(|(x, y)| (x - y).abs()).fold(0.0, f64::max);
        assert!(worst <= 1e-9, "{:?}: one backend for two worlds differs by {worst}", a.grid);
    }
}

// ------------------------------------------------------------------ the column phases

/// Serial against the pool, so only with the rayon drivers (`par`, on in the workspace
/// through `cubarium-voxel`'s default `parallel`).
#[cfg(feature = "par")]
mod columns {
    use cubarium_rules::water::{self, Grid};
    use cubarium_rules::water::column::{Ground, Table};
    use cubarium_rules::water::host::{self, Exec, Water};

    /// Air, bedrock, rock and soil, as the column phases see them, fluxes over `dt`: soil
    /// drains fast enough to cross its field capacity within a few steps, rock barely.
    fn grounds(dt: f64) -> [Ground; 4] {
        let porous = |capacity: f64, field_capacity: f64, rate: f64| Ground {
            solid: true,
            bedrock: false,
            capacity,
            permeable: true,
            field_capacity,
            flux: rate * dt,
        };
        [
            Ground::default(),
            Ground {
                solid: true,
                bedrock: true,
                ..Ground::default()
            },
            porous(0.02, 0.5, 0.001),
            porous(0.35, 0.65, 0.02),
        ]
    }

    /// A column world with ground: bedrock at the floor, soil (and some rock) to a seeded
    /// height, air over it; seeded free water in the air, seeded pore water in the ground,
    /// the three sets as row masks, and an aquifer.
    #[derive(Clone)]
    struct Land {
        grid: Grid,
        kind: Vec<u8>,
        free: Vec<f64>,
        pore: Vec<f64>,
        wet: Vec<u128>,
        damp: Vec<u128>,
        drainable: Vec<u128>,
        aquifer: f64,
    }

    const VOXEL: f64 = 1.0;
    const VOXEL_M: f64 = 1.0;

    impl Land {
        fn seeded(width: usize, depth: usize, height: usize, seed: u64) -> Land {
            let grid = Grid { width, depth, height,
            };
            let (plane, n) = (grid.plane(), grid.cells());
            let mut s = seed;
            let mut next = move || {
                s = s.wrapping_add(0x9E37_79B9_7F4A_7C15);
                let mut z = s;
                z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
                z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
                ((z ^ (z >> 31)) >> 11) as f64 / (1u64 << 53) as f64
            };
            let (mut kind, mut free, mut pore) = (vec![0u8; n], vec![0.0; n], vec![0.0; n]);
            for col in 0..plane {
                let ground = 2 + (next() * 5.0) as usize;
                for y in 0..height {
                    let i = y * plane + col;
                    if y == 0 {
                        kind[i] = 1;
                    } else if y < ground {
                        kind[i] = if next() < 0.15 { 2 } else { 3 };
                        pore[i] = next();
                    } else if next() < 0.3 {
                        free[i] = if next() < 0.3 { 1.0 } else { next() };
                    }
                }
            }
            let mut land = Land {
                grid,
                kind,
                free,
                pore,
                wet: Vec::new(),
                damp: Vec::new(),
                drainable: Vec::new(),
                aquifer: 40.0,
            };
            (land.wet, land.damp, land.drainable) = land.sets_from_arrays();
            land
        }

        /// The three sets as the arrays say they should be.
        fn sets_from_arrays(&self) -> (Vec<u128>, Vec<u128>, Vec<u128>) {
            let plane = self.grid.plane();
            let words = self.grid.mask_words();
            let g = grounds(1.0);
            let (mut wet, mut damp, mut drainable) =
                (vec![0u128; plane * words], vec![0u128; plane * words], vec![0u128; plane * words],
            );
            for i in 0..self.grid.cells() {
                let y = i / plane;
                let col = self.grid.mask_index(i % plane, y);
                let bit = 1u128 << (y % water::MASK_ROWS);
                let k = g[self.kind[i] as usize];
                if !k.solid && self.free[i] > 0.0 {
                    wet[col] |= bit;
                }
                if k.capacity > 0.0 && self.pore[i] > 0.0 {
                    damp[col] |= bit;
                    let unit = k.capacity * VOXEL;
                    if k.permeable && self.pore[i] * VOXEL * k.capacity - k.field_capacity * unit > 0.0 {
                        drainable[col] |= bit;
                    }
                }
            }
            (wet, damp, drainable)
        }

        /// Free, pore and aquifer water, m³.
        fn total(&self) -> f64 {
            let g = grounds(1.0);
            let mut t = self.aquifer;
            for i in 0..self.grid.cells() {
                let k = g[self.kind[i] as usize];
                if !k.solid {
                    t += self.free[i] * VOXEL;
                }
                t += self.pore[i] * VOXEL * k.capacity;
            }
            t
        }

        /// One tick of the column phases on `exec`: four substeps of infiltration and fall,
        /// then drainage and the water table. Keeps the sets and checks the counts they
        /// report.
        fn tick(&mut self, exec: Exec<'_>) {
            let counts = |l: &Land| {
                [&l.wet, &l.damp, &l.drainable]
                    .map(|v| v.iter().map(|m| m.count_ones() as isize).sum::<isize>())
            };
            let check = |before: [isize; 3], after: [isize; 3], d: [isize; 3], phase: &str| {
                for k in 0..3 {
                    assert_eq!(after[k] - before[k], d[k], "{phase}: set {k}'s reported count");
                }
            };
            let kind = self.kind.clone();
            let (sub, tick) = (grounds(0.25), grounds(1.0));
            let (sub, ground) = (|i: usize| sub[kind[i] as usize], |i: usize| tick[kind[i] as usize],
            );
            for _ in 0..4 {
                let before = counts(self);
                let d = host::infiltrate(exec, self.water(), &sub);
                check(before, counts(self), d, "infiltrate");
                let before = counts(self);
                let d = host::fall(exec, self.water(), &ground);
                check(before, counts(self), d, "fall");
            }
            let table = 3.2;
            let before = counts(self);
            let (d, gained) = host::drain(exec, self.water(), &ground, VOXEL_M, table);
            check(before, counts(self), d, "drain");
            self.aquifer += gained;
            let before = counts(self);
            let charged = self.aquifer;
            let t = Table::new(table, VOXEL_M, self.grid.height);
            let (d, taken) = host::water_table(exec, self.water(), &ground, t, charged);
            check(before, counts(self), d, "water table");
            self.aquifer = (charged - taken).max(0.0);
        }

        fn water(&mut self) -> Water<'_> {
            Water {
                grid: self.grid,
                voxel: VOXEL,
                free: &mut self.free,
                pore: &mut self.pore,
                wet: &mut self.wet,
                damp: &mut self.damp,
                drainable: &mut self.drainable,
            }
        }
    }

    fn worst(a: &[f64], b: &[f64]) -> f64 {
        a.iter().zip(b).map(|(x, y)| (x - y).abs()).fold(0.0, f64::max)
    }

    /// Fall, infiltration, drainage and the water table on a pool follow the same phases
    /// on one thread (to rounding: the aquifer's share is summed per chunk), conserve
    /// water, and leave all three sets true to the arrays.
    #[test]
    fn the_column_phases_on_a_pool_agree_with_one_thread_and_conserve() {
        let pool = host::pool(4);
        let mut serial = Land::seeded(70, 5, 12, 3);
        // An aquifer that can pay for everything the table asks of it, so every tick's
        // water table runs split across the pool (the scarce case is the next test's).
        serial.aquifer = 1e4;
        let start = serial.clone();
        let mut pooled = serial.clone();
        let total = serial.total();
        for tick in 0..60 {
            serial.tick(Exec::serial());
            pooled.tick(Exec::on(&pool));
            for land in [&serial, &pooled] {
                let (wet, damp, drainable) = land.sets_from_arrays();
                assert_eq!(land.wet, wet, "tick {tick}: wet set");
                assert_eq!(land.damp, damp, "tick {tick}: damp set");
                assert_eq!(land.drainable, drainable, "tick {tick}: drainable set");
                let drift = (land.total() - total).abs();
                assert!(drift <= 1e-12 * total, "tick {tick}: water drifted by {drift}");
            }
            // Rounding, not bits: what drainage hands the aquifer is summed per chunk.
            assert!(worst(&serial.free, &pooled.free) <= 1e-9, "tick {tick}: free");
            assert!(worst(&serial.pore, &pooled.pore) <= 1e-9, "tick {tick}: pore");
            assert!(
                (serial.aquifer - pooled.aquifer).abs() <= 1e-9 * serial.aquifer.max(1.0),
                "tick {tick}: aquifer"
            );
        }
        // The phases did something: free water fell and soaked in, pores drained.
        assert!(worst(&serial.free, &start.free) > 0.1, "free water never moved");
        assert!(worst(&serial.pore, &start.pore) > 0.1, "pore water never moved");
        assert!((serial.aquifer - start.aquifer).abs() > 1.0, "the aquifer never moved");
    }

    /// A dense walk of the same primitives, independent of the host's word masks.
    fn dense_tick(l: &mut Land) {
        use water::column::{self, Cells, Sets};
        struct Ignore;
        impl Sets for Ignore {
            fn wet(&mut self, _: usize, _: usize, _: bool) {}
            fn damp(&mut self, _: usize, _: usize, _: bool) {}
            fn drainable(&mut self, _: usize, _: usize, _: bool) {}
        }
        let plane = l.grid.plane();
        let kind = &l.kind;
        let (sub, ground) = (grounds(0.25), grounds(1.0));
        let (sub, ground) = (
            |i: usize| sub[kind[i] as usize],
            |i: usize| ground[kind[i] as usize],
        );
        for _ in 0..4 {
            for infiltrate in [true, false] {
                for col in 0..plane {
                    let rows: Vec<_> = (0..l.grid.height)
                        .filter(|&y| {
                            !ground(y * plane + col).solid && l.free[y * plane + col] > 0.0
                        })
                        .collect();
                    let mut ignore = Ignore;
                    let mut c = Cells {
                        free: &mut l.free[..],
                        pore: &mut l.pore[..],
                        sets: &mut ignore,
                        voxel: VOXEL,
                        plane,
                    };
                    if infiltrate {
                        column::infiltrate(&mut c, col, rows.into_iter(), &sub);
                    } else {
                        column::fall(&mut c, col, rows.into_iter(), &ground);
                    }
                }
            }
        }
        let table = 3.2;
        for col in 0..plane {
            let rows: Vec<_> = (0..l.grid.height)
                .filter(|&y| {
                    let i = y * plane + col;
                    let g = ground(i);
                    g.capacity > 0.0 && g.permeable && l.pore[i] > g.field_capacity
                })
                .collect();
            let mut ignore = Ignore;
            let mut c = Cells {
                free: &mut l.free[..],
                pore: &mut l.pore[..],
                sets: &mut ignore,
                voxel: VOXEL,
                plane,
            };
            l.aquifer += column::drain(&mut c, col, rows.into_iter(), &ground, VOXEL_M, table);
        }
        let t = Table::new(table, VOXEL_M, l.grid.height);
        let charged = l.aquifer;
        let mut taken = 0.0;
        let mut ignore = Ignore;
        let mut c = Cells {
            free: &mut l.free[..],
            pore: &mut l.pore[..],
            sets: &mut ignore,
            voxel: VOXEL,
            plane,
        };
        for y in 0..t.band {
            if !t.submerged(y) {
                continue;
            }
            for col in 0..plane {
                let i = y * plane + col;
                taken += column::saturate(&mut c, (i, y, ground(i)), charged - taken);
            }
        }
        for y in 1..t.seep_top {
            for col in 0..plane {
                taken += column::seep(
                    &mut c,
                    y * plane + col,
                    y,
                    t.level(y),
                    y + 1 == l.grid.height,
                    &ground,
                    charged - taken,
                );
            }
        }
        l.aquifer = (charged - taken).max(0.0);
    }

    #[test]
    fn tall_column_phases_cross_words_and_parallel_chunks_like_a_dense_walk() {
        let pool = host::pool(4);
        for height in [129, 138, 168, 256, 259] {
            // Seventy columns exercise the final, short 64-column chunk too.
            let grid = Grid {
                width: 70,
                depth: 1,
                height,
            };
            let n = grid.cells();
            let mut serial = Land {
                grid,
                kind: vec![0; n],
                free: vec![0.0; n],
                pore: vec![0.0; n],
                wet: Vec::new(),
                damp: Vec::new(),
                drainable: Vec::new(),
                aquifer: 1e4,
            };
            for col in 0..grid.plane() {
                serial.kind[col] = 1;
                serial.kind[grid.plane() + col] = 3;
                for y in [127, 128, 255, 256, height - 1] {
                    if y >= height {
                        continue;
                    }
                    let i = y * grid.plane() + col;
                    if col % 3 == 0 {
                        serial.kind[i] = 3;
                        serial.pore[i] = 1.0;
                    } else {
                        serial.free[i] = 0.4;
                        if col % 3 == 1 {
                            serial.kind[i - grid.plane()] = 3;
                            serial.free[i - grid.plane()] = 0.0;
                        }
                    }
                }
            }
            (serial.wet, serial.damp, serial.drainable) = serial.sets_from_arrays();
            let (mut pooled, mut dense) = (serial.clone(), serial.clone());
            let total = serial.total();
            for _ in 0..2 {
                serial.tick(Exec::serial());
                pooled.tick(Exec::on(&pool));
                dense_tick(&mut dense);
                for l in [&serial, &pooled] {
                    assert!(
                        worst(&l.free, &dense.free) <= 1e-12,
                        "height {height}: free"
                    );
                    assert!(
                        worst(&l.pore, &dense.pore) <= 1e-12,
                        "height {height}: pore"
                    );
                    assert!((l.aquifer - dense.aquifer).abs() <= 1e-9);
                    assert!((l.total() - total).abs() <= 1e-9);
                    assert_eq!(
                        (l.wet.clone(), l.damp.clone(), l.drainable.clone()),
                        l.sets_from_arrays()
                    );
                }
            }
            // Saturating a high table exercises updates to every word in both modes.
            let grounds = grounds(1.0);
            let kind = serial.kind.clone();
            let ground = |i: usize| grounds[kind[i] as usize];
            let table = Table::new(height as f64 - 1.2, VOXEL_M, height);
            let (_, a) = host::water_table(Exec::serial(), serial.water(), &ground, table, 1e6);
            let (_, b) = host::water_table(Exec::on(&pool), pooled.water(), &ground, table, 1e6);
            assert!(worst(&serial.free, &pooled.free) <= 1e-12);
            assert!(worst(&serial.pore, &pooled.pore) <= 1e-12);
            assert!((a - b).abs() <= 1e-9);
            for l in [&serial, &pooled] {
                assert_eq!(
                    (l.wet.clone(), l.damp.clone(), l.drainable.clone()),
                    l.sets_from_arrays()
                );
            }
        }
    }

    /// Worlds of different sizes running the column phases at once on the one shared
    /// pool: each follows itself run alone on one thread, to rounding, keeps its water, and
    /// keeps its sets true to its arrays.
    #[test]
    fn worlds_of_different_sizes_run_the_column_phases_at_once_on_one_pool() {
        const TICKS: usize = 30;
        let sizes = [(70, 5, 12, 3), (130, 4, 20, 4), (17, 9, 9, 5), (64, 8, 128, 6),
        ];
        let alone: Vec<Land> = sizes
            .iter()
            .map(|&(w, d, h, seed)| {
                let mut land = Land::seeded(w, d, h, seed);
                land.aquifer = 1e4;
                for _ in 0..TICKS {
                    land.tick(Exec::serial());
                }
                land
            })
            .collect();
        let together: Vec<Land> = std::thread::scope(|s| {
            let handles: Vec<_> = sizes
                .iter()
                .map(|&(w, d, h, seed)| {
                    s.spawn(move || {
                        let pool = host::pool(4);
                        let mut land = Land::seeded(w, d, h, seed);
                        land.aquifer = 1e4;
                        let total = land.total();
                        for tick in 0..TICKS {
                            land.tick(Exec::on(&pool));
                            let drift = (land.total() - total).abs();
                            assert!(drift <= 1e-12 * total, "{w}x{d}x{h} tick {tick}: drifted by {drift}");
                            assert_eq!(
                                (land.wet.clone(), land.damp.clone(), land.drainable.clone()),
                                land.sets_from_arrays(),
                                "{w}x{d}x{h} tick {tick}: sets"
                            );
                        }
                        land
                    })
                })
                .collect();
            handles.into_iter().map(|h| h.join().expect("a world's thread")).collect()
        });
        for (a, b) in alone.iter().zip(&together) {
            assert!(worst(&a.free, &b.free) <= 1e-9, "{:?}: free", a.grid);
            assert!(worst(&a.pore, &b.pore) <= 1e-9, "{:?}: pore", a.grid);
            assert!((a.aquifer - b.aquifer).abs() <= 1e-9 * a.aquifer.max(1.0), "{:?}: aquifer", a.grid);
        }
    }

    /// When the aquifer cannot pay for every cell the table reaches, it is shared in the
    /// world's index order **by rule**, on a pool too: the pool's water table lands where
    /// the one-thread walk does, and the stock is spent.
    #[test]
    fn a_scarce_aquifer_is_shared_in_index_order_on_a_pool_too() {
        let pool = host::pool(4);
        let mut land = Land::seeded(70, 5, 12, 9);
        // Dry the ground out so the table has pores to fill everywhere it reaches.
        for p in &mut land.pore {
            *p *= 0.1;
        }
        (land.wet, land.damp, land.drainable) = land.sets_from_arrays();
        let charged = 0.05;
        let t = Table::new(4.2, VOXEL_M, land.grid.height);
        let table = grounds(1.0);
        let ground = |i: usize| table[land.kind[i] as usize];
        let run = |exec: Exec<'_>| {
            let mut l = land.clone();
            let (_, taken) = host::water_table(exec, l.water(), &ground, t, charged);
            (l, taken)
        };
        let (serial, taken_serial) = run(Exec::serial());
        let (pooled, taken_pooled) = run(Exec::on(&pool));
        assert!((taken_serial - charged).abs() <= 1e-12, "the stock was not spent: {taken_serial}");
        assert!((taken_pooled - taken_serial).abs() <= 1e-12);
        assert!(worst(&serial.pore, &pooled.pore) <= 1e-12, "the pool shared it differently");
        assert!(worst(&serial.free, &pooled.free) <= 1e-12);
        // Index order: the first cell the table filled is the lowest-indexed one it reaches.
        let filled: Vec<usize> = (0..land.grid.cells())
            .filter(|&i| serial.pore[i] > land.pore[i])
            .collect();
        let plane = land.grid.plane();
        assert!(filled.iter().all(|&i| i / plane < t.band), "a cell above the band filled");
        assert!(
            filled.last().copied().unwrap_or(0) < land.grid.cells() / 2,
            "a scarce stock reached the upper rows: {:?}",
            filled.last()
        );
    }
}
