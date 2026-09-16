//! Run a headless world to steady state and print what it settled at.
//!
//! FW-2's ring verification (`design/flat-world-plan-2026-09-16.md` §9): a ring world runs,
//! at `S = 1` (320×180) and at `S = 2` (640×360), and the cube is unchanged beside it. The
//! numbers are populations and field totals, not a picture — FW-3 and FW-5 own the picture.
//!
//! Usage: `cargo run --release --example ring_steady_state [ticks] [seed]`

use cubarium_core::{World, WorldConfig};
use cubarium_surface::{Scale, Topology};

fn summarise(name: &str, topo: Topology, scale: Scale, ticks: u64, seed: u64) {
    let config = WorldConfig {
        seed,
        topology: topo,
        world_scale: scale,
        ..WorldConfig::default()
    };
    let cells = topo.cell_count(scale);
    let start = std::time::Instant::now();
    let mut world = match World::new(config) {
        Ok(w) => w,
        Err(e) => {
            println!("{name}: refused: {e}");
            return;
        }
    };
    let founders = world.state.organisms.len();
    let mut min_pop = founders;
    let mut max_pop = founders;
    for t in 1..=ticks {
        world.step();
        let n = world.state.organisms.len();
        min_pop = min_pop.min(n);
        max_pop = max_pop.max(n);
        if t % (ticks / 6).max(1) == 0 {
            println!(
                "  {name} tick {t:>6}  pop {n:>4}  P {:>10.2}  D {:>10.2}  N {:>10.2}  \
                 W {:>9.2}  wood {:>10.2}",
                total(&world.state.fields.p),
                total(&world.state.fields.d),
                total(&world.state.fields.n),
                total(&world.state.fields.w),
                total(&world.state.ecology.wood),
            );
        }
    }
    let s = &world.state;
    let elapsed = start.elapsed();
    println!(
        "{name}: {cells} cells, {founders} founders -> pop {} (min {min_pop}, max {max_pop}), \
         births {}, deaths {:?}, mass residual {:.3e}, {:.0} ticks/s",
        s.organisms.len(),
        s.births_total,
        s.deaths_total,
        world.mass_residual(),
        ticks as f64 / elapsed.as_secs_f64(),
    );
    println!(
        "{name}: totals P {:.2} F {:.2} D {:.2} De {:.2} N {:.2} w {:.2} | wood {:.2} \
         reserve {:.2} dead wood {:.2} carrion {:.2}",
        total(&s.fields.p),
        total(&s.fields.f),
        total(&s.fields.d),
        total(&s.fields.de),
        total(&s.fields.n),
        total(&s.fields.w),
        total(&s.ecology.wood),
        total(&s.ecology.plant_reserve),
        total(&s.ecology.dead_wood),
        total(&s.ecology.carrion),
    );
    // Where the water and the foliage ended up along the height axis: the ring's top cell row
    // is the canopy and never drains, its bottom row has nothing below it (plan §5).
    let (cx, cy) = topo.cells(scale, cubarium_surface::Face::Front);
    if matches!(topo, Topology::Ring { .. }) {
        // Row-major within the one chart: index = cy·cx + cx.
        let row = |r: u16, v: &[f64]| -> f64 {
            (0..cx)
                .map(|c| v[usize::from(r) * usize::from(cx) + usize::from(c)])
                .sum()
        };
        println!(
            "{name}: top row  w {:.3}  P {:.3} | bottom row  w {:.3}  P {:.3}",
            row(0, &s.fields.w),
            row(0, &s.fields.p),
            row(cy - 1, &s.fields.w),
            row(cy - 1, &s.fields.p),
        );
    }
}

fn total(v: &[f64]) -> f64 {
    v.iter().sum()
}

fn main() {
    let mut args = std::env::args().skip(1);
    let ticks: u64 = args.next().map_or(3000, |s| s.parse().expect("ticks"));
    let seed: u64 = args.next().map_or(1, |s| s.parse().expect("seed"));
    summarise("cube      ", Topology::Cube, Scale::ONE, ticks, seed);
    summarise(
        "ring 320x180 S=1",
        Topology::Ring { w: 320, h: 180 },
        Scale::ONE,
        ticks,
        seed,
    );
    summarise(
        "ring 640x360 S=2",
        Topology::Ring { w: 640, h: 360 },
        Scale::new(2.0),
        ticks,
        seed,
    );
}
