//! The water tick's cost, serial against the pool, and the two drivers' water after a run
//! through a shower (`design/handoffs/voxel-water-parallel-2026-09-24.md`).
//!
//! `water_bench [seeds=1,2,3] [threads=16] [bench_ticks=100] [parity_ticks=1200]
//! [out_dir=.] [world=terrarium]` — `world` is `terrarium` (the desktop terrarium, 256 × 128
//! × 48 at 0.125 m) or a shipped preset (`small`, `default`, `wide`).
//!
//! Per seed: run the world in until its first shower, keeping the last state 500 ticks or
//! less before it (**steady**) and the state 300 ticks into it (**shower**); both are
//! cached in `out_dir`. Each state is then stepped `bench_ticks` by one thread and by
//! `threads`, timing every phase. Parity: each driver from the steady state through the
//! shower for `parity_ticks`; standing volume, pooled cells, falling cells and the lake's
//! level against the seed-to-seed spread, and a picture pair for the first seed.
//! Development measurement only.

use std::time::Instant;

use cubarium_voxel::water;
use cubarium_voxel::{Config, Landform, Preset, Terrarium, World, hydrate};

fn config(world: &str, seed: u64) -> Config {
    if world == "terrarium" {
        return Config {
            width: 256,
            height: 128,
            depth: 48,
            voxel_m: 0.125,
            water_substeps: 4,
            seed,
            landform: Landform::Terrarium(
                Terrarium::preset("terrarium").expect("the terrarium preset"),
            ),
            ..Config::default()
        };
    }
    Config {
        seed,
        ..Preset::find(world)
            .unwrap_or_else(|| panic!("no preset {world}"))
            .config()
    }
}

/// The phases a tick times, in [`water::step`]'s order.
const PHASES: [&str; 10] = [
    "begin",
    "rain",
    "evaporate",
    "infiltrate",
    "fall",
    "exchange",
    "drain",
    "water_table",
    "spring",
    "outlet",
];

/// One water tick, `water::step`'s phases, each timed into `ms`. Returns the tick's ms.
fn tick(w: &mut World, threads: usize, ms: &mut [f64; 10]) -> f64 {
    let t0 = Instant::now();
    let mut lap = Instant::now();
    let mut mark = |k: usize, lap: &mut Instant| {
        let now = Instant::now();
        ms[k] += (now - *lap).as_secs_f64() * 1e3;
        *lap = now;
    };
    water::begin(w);
    mark(0, &mut lap);
    water::rain(w);
    mark(1, &mut lap);
    water::evaporate(w);
    mark(2, &mut lap);
    let substeps = w.config().water_substeps.max(1);
    let sub_dt = cubarium_voxel::DT / substeps as f64;
    for _ in 0..substeps {
        water::infiltrate(w, sub_dt, threads);
        mark(3, &mut lap);
        water::fall(w, threads);
        mark(4, &mut lap);
        water::exchange(w, threads);
        mark(5, &mut lap);
    }
    water::drain(w, threads);
    mark(6, &mut lap);
    water::water_table(w, threads);
    mark(7, &mut lap);
    water::spring(w);
    mark(8, &mut lap);
    water::outlet(w);
    mark(9, &mut lap);
    w.advance_tick();
    t0.elapsed().as_secs_f64() * 1e3
}

/// What the parity compares.
#[derive(Clone, Copy, Debug, Default)]
struct Stats {
    /// Free water standing in the world, m³.
    standing_m3: f64,
    /// Cells at least half full resting on rock or full water: pooled.
    pooled: usize,
    /// The lake's mean surface, cells above y = 0.
    lake_level: f64,
    /// Wet cells over a void cell with room: water in transit.
    falling: usize,
    residual: f64,
}

fn stats(w: &World) -> Stats {
    let c = w.config();
    let v = w.view();
    let plane = c.width as usize * c.depth as usize;
    let mut s = Stats::default();
    for (i, &f) in v.free.iter().enumerate() {
        if !(f > 1e-9) {
            continue;
        }
        s.standing_m3 += f * c.voxel_volume();
        if i < plane {
            continue;
        }
        let below = i - plane;
        let solid = v.material[below].is_solid();
        let full_below = v.free[below] >= 1.0 - 1e-9;
        if f >= 0.5 && (solid || full_below) {
            s.pooled += 1;
        }
        if !solid && v.free[below] < 1.0 - 1e-9 {
            s.falling += 1;
        }
    }
    let lake = hydrate::lake(w);
    s.lake_level = if lake.surface_cells.is_empty() {
        0.0
    } else {
        lake.surface_cells
            .iter()
            .map(|&i| c.coords(i).1 as f64 + v.free[i])
            .sum::<f64>()
            / lake.surface_cells.len() as f64
    };
    s.residual = v.stored_m3() - v.ledger.expected_stored();
    s
}

/// The front elevation, x across and y up, two pixels a cell: water by its deepest fill
/// along z, rock where the whole row is solid.
fn picture(w: &World, path: &std::path::Path) {
    let c = w.config();
    let v = w.view();
    let (wd, ht, dp) = (c.width as usize, c.height as usize, c.depth as usize);
    let s = 2;
    let (iw, ih) = (wd * s, ht * s);
    let mut rgb = vec![0u8; iw * ih * 3];
    for y in 0..ht {
        for x in 0..wd {
            let (mut fill, mut solid) = (0.0f64, 0usize);
            for z in 0..dp {
                let i = (y * dp + z) * wd + x;
                if v.material[i].is_solid() {
                    solid += 1;
                } else {
                    fill = fill.max(v.free[i]);
                }
            }
            let px = if fill > 0.01 {
                let k = fill.min(1.0);
                [
                    (20.0 * (1.0 - k)) as u8,
                    (90.0 + 60.0 * k) as u8,
                    (160.0 + 95.0 * k) as u8,
                ]
            } else if solid == dp {
                [90, 84, 78]
            } else {
                let g = (18 + 40 * solid / dp) as u8;
                [g, g, g + 6]
            };
            for dy in 0..s {
                for dx in 0..s {
                    let (qx, qy) = (x * s + dx, (ht - 1 - y) * s + dy);
                    rgb[(qy * iw + qx) * 3..(qy * iw + qx) * 3 + 3].copy_from_slice(&px);
                }
            }
        }
    }
    let f = std::fs::File::create(path).expect("the picture file");
    let mut e = png::Encoder::new(std::io::BufWriter::new(f), iw as u32, ih as u32);
    e.set_color(png::ColorType::Rgb);
    e.set_depth(png::BitDepth::Eight);
    e.write_header().unwrap().write_image_data(&rgb).unwrap();
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let seeds: Vec<u64> = args
        .get(1)
        .map_or("1,2,3", |s| s.as_str())
        .split(',')
        .map(|s| s.parse().unwrap())
        .collect();
    let threads: usize = args.get(2).map_or(16, |s| s.parse().unwrap());
    let bench_ticks: usize = args.get(3).map_or(100, |s| s.parse().unwrap());
    let parity_ticks: usize = args.get(4).map_or(1200, |s| s.parse().unwrap());
    let out = std::path::PathBuf::from(args.get(5).map_or(".", |s| s.as_str()));
    let world = args.get(6).map_or("terrarium", |s| s.as_str()).to_string();
    std::fs::create_dir_all(&out).unwrap();
    water::prepare_pool(threads);
    println!("world {world}, {threads} threads, {bench_ticks} bench ticks, {parity_ticks} parity ticks");

    let drivers = [("serial", 1usize), ("parallel", threads)];
    let mut parity: Vec<(u64, [Stats; 2])> = Vec::new();
    for &seed in &seeds {
        let (steady_path, shower_path) = (
            out.join(format!("{world}-steady-seed{seed}.snapshot")),
            out.join(format!("{world}-shower-seed{seed}.snapshot")),
        );
        let (steady, shower) = if let (Ok(a), Ok(b)) =
            (std::fs::read(&steady_path), std::fs::read(&shower_path))
        {
            (
                World::load(&a).expect("a steady snapshot"),
                World::load(&b).expect("a shower snapshot"),
            )
        } else {
            let t = Instant::now();
            let mut w = World::new(config(&world, seed));
            eprintln!("seed {seed}: built in {:.1?}", t.elapsed());
            let t = Instant::now();
            let mut steady = w.clone();
            let mut ticks = 0usize;
            let mut scratch = [0.0; 10];
            while !w.view().is_raining() && ticks < 20_000 {
                if ticks % 500 == 0 {
                    steady = w.clone();
                }
                tick(&mut w, threads, &mut scratch);
                ticks += 1;
            }
            eprintln!(
                "seed {seed}: first shower at tick {ticks} ({:.1?})",
                t.elapsed()
            );
            for _ in 0..300 {
                tick(&mut w, threads, &mut scratch);
            }
            std::fs::write(&steady_path, steady.save()).unwrap();
            std::fs::write(&shower_path, w.save()).unwrap();
            (steady, w)
        };
        let raining = shower.view().is_raining();

        for (label, state) in [("steady", &steady), ("shower", &shower)] {
            let s = stats(state);
            println!(
                "seed {seed} {label}: {:.3} m³ standing, {} pooled, {} falling, lake {:.3}{}",
                s.standing_m3,
                s.pooled,
                s.falling,
                s.lake_level,
                if label == "shower" && !raining {
                    " (the shower had ended)"
                } else {
                    ""
                }
            );
            for (name, n) in drivers {
                let mut w = state.clone();
                let mut ms = [0.0; 10];
                // Two warm-up ticks (scratch sized), then timed.
                for _ in 0..2 {
                    tick(&mut w, n, &mut ms);
                }
                ms = [0.0; 10];
                let mut all = 0.0;
                for _ in 0..bench_ticks {
                    all += tick(&mut w, n, &mut ms);
                }
                let per = |x: f64| x / bench_ticks as f64;
                let phases: Vec<String> = PHASES
                    .iter()
                    .zip(ms)
                    .filter(|(_, m)| per(*m) >= 0.005)
                    .map(|(p, m)| format!("{p} {:.2}", per(m)))
                    .collect();
                println!(
                    "  {label:6} {name:8} x{n:<2}: water tick {:6.2} ms | {}",
                    per(all),
                    phases.join(", ")
                );
            }
        }

        // Parity: each driver from the steady state, through the shower, `parity_ticks`.
        let mut both = [Stats::default(); 2];
        for (k, (name, n)) in drivers.into_iter().enumerate() {
            let t = Instant::now();
            let mut w = steady.clone();
            let mut worst = 0.0f64;
            let mut ms = [0.0; 10];
            for t in 0..parity_ticks {
                tick(&mut w, n, &mut ms);
                if t % 100 == 99 {
                    let v = w.view();
                    worst = worst.max((v.stored_m3() - v.ledger.expected_stored()).abs());
                }
            }
            both[k] = stats(&w);
            let s = both[k];
            println!(
                "  parity {name:8}: {:.4} m³ standing, {:6} pooled, {:6} falling, lake {:.4}, \
                 worst ledger residual {worst:.1e} m³ ({:.1?})",
                s.standing_m3,
                s.pooled,
                s.falling,
                s.lake_level,
                t.elapsed()
            );
            if seed == seeds[0] {
                picture(&w, &out.join(format!("{world}-seed{seed}-{name}.png")));
            }
        }
        parity.push((seed, both));
    }

    // Driver-to-driver differences against the seed-to-seed spread.
    let field = |s: &Stats, f: usize| match f {
        0 => s.standing_m3,
        1 => s.pooled as f64,
        2 => s.falling as f64,
        _ => s.lake_level,
    };
    for (f, name) in ["standing m³", "pooled cells", "falling cells", "lake level"]
        .iter()
        .enumerate()
    {
        let serial: Vec<f64> = parity.iter().map(|(_, s)| field(&s[0], f)).collect();
        let mean = serial.iter().sum::<f64>() / serial.len() as f64;
        let spread = serial.iter().map(|x| (x - mean).abs()).fold(0.0, f64::max);
        let worst = parity
            .iter()
            .map(|(_, s)| (field(&s[1], f) - field(&s[0], f)).abs())
            .fold(0.0, f64::max);
        println!(
            "{name:14}: serial mean {mean:.4}, seed spread ±{spread:.4}; largest |parallel − serial| {worst:.4}"
        );
    }
}
