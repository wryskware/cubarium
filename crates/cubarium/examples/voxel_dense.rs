//! Dense-world tick cost: grow a host world, snapshot it, and profile a snapshot.
//! Measurement only — no rule and no number of the ecology is in here.
//!
//! ```text
//! # found the desktop terrarium and grow it, saving the three layers every 15 minutes
//! cargo run --release -p cubarium --example voxel_dense -- grow \
//!     config=config/desktop/terrarium.toml minutes=120 every=15 save=runs/dense [threads=16]
//! # resume a snapshot and grow it further
//! cargo run --release -p cubarium --example voxel_dense -- grow load=runs/dense/m090 \
//!     minutes=120 every=15 save=runs/dense
//! # the tick profile of a snapshot: per-phase ms/tick over `ticks` ticks
//! cargo run --release -p cubarium --example voxel_dense -- profile load=runs/dense/m090 \
//!     ticks=600 [threads=16]
//! ```
//!
//! A snapshot is a directory holding `world.bin`, `flora.bin` and `fauna.bin` — each
//! layer's own always-fresh envelope — plus `minute`. Loading re-installs the built-in
//! founder centres and re-settles the senses, which is what the host does at start.

use std::path::{Path, PathBuf};
use std::time::Instant;

use cubarium::voxel::{habitat, install_default_founders};
use cubarium_voxel::World;
use cubarium_voxel::profile::{self, Phase};
use cubarium_voxel_fauna::{Fauna, Senses, TICK_HZ};
use cubarium_voxel_flora::{Flora, FloraConfig};
use cubarium_voxel_sim::{Sim, SimConfig};

const TICKS_PER_MIN: u64 = 60 * TICK_HZ as u64;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mode = args.first().cloned().unwrap_or_default();
    let get = |key: &str| -> Option<String> {
        args.iter()
            .find_map(|a| a.strip_prefix(&format!("{key}=")).map(str::to_string))
    };
    let threads: usize = get("threads").map_or(16, |s| s.parse().expect("threads=N"));
    let (mut sim, minute0) = if let Some(dir) = get("load") {
        load(Path::new(&dir), threads)
    } else {
        let config = get("config").expect("config=<toml> or load=<dir>");
        let seed: u64 = get("seed").map_or(1, |s| s.parse().expect("seed=N"));
        found(Path::new(&config), seed, threads)
    };
    if let Some(n) = get("densify") {
        densify(&mut sim, n.parse().expect("densify=N"));
    }
    match mode.as_str() {
        "grow" => {
            let minutes: u64 = get("minutes").map_or(60, |s| s.parse().expect("minutes=N"));
            let every: u64 = get("every").map_or(15, |s| s.parse().expect("every=N"));
            let save = get("save").map(PathBuf::from);
            grow(sim, minute0, minutes, every, save.as_deref());
        }
        "profile" => {
            let ticks: u64 = get("ticks").map_or(600, |s| s.parse().expect("ticks=N"));
            profile_run(sim, ticks);
        }
        _ => panic!("usage: voxel_dense grow|profile (config=<toml>|load=<dir>) ..."),
    }
}

fn found(config: &Path, seed: u64, threads: usize) -> (Sim, u64) {
    let cfg = cubarium::voxel::load_config(config).expect("a host config");
    let founded = cubarium::voxel::ambient_habitat(
        &cfg.world,
        seed,
        FloraConfig::for_voxel_size,
        habitat::FOUNDER_COUNTS,
    );
    eprintln!(
        "founded seed {} ({}), stands {}",
        founded.seed,
        if founded.accepted { "accepted" } else { "NOT accepted" },
        founded.seeded.stands
    );
    (assemble(founded.world, founded.flora, founded.fauna, threads), 0)
}

fn assemble(mut world: World, flora: Flora, mut fauna: Fauna, threads: usize) -> Sim {
    install_default_founders(&mut fauna).expect("the built-in centres validate");
    let mut senses = Senses::new();
    senses.settle(&world.view(), &flora.view());
    if world.config().closed_water_budget && !world.outlet_open() {
        world.apply(cubarium_voxel::Command::SetOutlet { open: true });
    }
    Sim::new(world, flora, fauna, SimConfig { threads }, Some(senses))
}

fn load(dir: &Path, threads: usize) -> (Sim, u64) {
    let read = |name: &str| std::fs::read(dir.join(name)).expect("a snapshot file");
    let world = World::load(&read("world.bin")).expect("world");
    let flora = cubarium_voxel_flora::snapshot::decode(&read("flora.bin")).expect("flora");
    let fauna = Fauna::load(&read("fauna.bin")).expect("fauna");
    let minute: u64 = String::from_utf8(read("minute"))
        .ok()
        .and_then(|s| s.trim().parse().ok())
        .unwrap_or(0);
    eprintln!(
        "loaded {} at minute {minute}: stands {} ground {} animals {}",
        dir.display(),
        flora.view().stands.len(),
        flora.view().ground.len(),
        fauna.view().animals.len()
    );
    // The world was saved with its outlet as it was; `assemble` only opens a closed
    // world's outlet that is shut, as the host does.
    (assemble(world, flora, fauna, threads), minute)
}

fn save_to(dir: &Path, sim: &Sim, minute: u64) {
    let (world, flora, fauna) = sim.layers();
    let sub = dir.join(format!("m{minute:03}"));
    std::fs::create_dir_all(&sub).expect("snapshot dir");
    std::fs::write(sub.join("world.bin"), world.save()).expect("write world");
    std::fs::write(
        sub.join("flora.bin"),
        cubarium_voxel_flora::snapshot::encode(flora),
    )
    .expect("write flora");
    std::fs::write(sub.join("fauna.bin"), fauna.save()).expect("write fauna");
    std::fs::write(sub.join("minute"), minute.to_string()).expect("write minute");
    eprintln!(
        "saved {} (stands {})",
        sub.display(),
        flora.view().stands.len()
    );
}

/// Plant up to `target` stands in all, on free support faces whose gates pass for a
/// species, at a keyed size between `alive_min` and 60 % of `wood_max`: a densely grown
/// world without hours of growth. Faces are visited in a keyed shuffle so the new stands
/// are spread over the whole world, not packed into its first rows.
fn densify(sim: &mut Sim, target: usize) {
    use cubarium_voxel_flora::{Command, Site, Species, establishment_gates};
    let mut faces: Vec<Site> = Vec::new();
    {
        let view = sim.world().view();
        let c = view.config;
        for z in 0..c.depth {
            for x in 0..c.width {
                for y in 0..c.height {
                    if view.is_support(i64::from(x), y, z) {
                        faces.push(Site { x, y, z });
                    }
                }
            }
        }
    }
    let mut state = 0x9E37_79B9_7F4A_7C15u64;
    let mut next = move || {
        state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    };
    for i in (1..faces.len()).rev() {
        let j = (next() % (i as u64 + 1)) as usize;
        faces.swap(i, j);
    }
    let before = sim.flora().view().stands.len();
    let mut planted = 0;
    for site in faces {
        if before + planted >= target {
            break;
        }
        if sim.flora().view().stand_at(site).is_some() {
            continue;
        }
        let start = (next() % Species::ALL.len() as u64) as usize;
        let choice = (0..Species::ALL.len()).map(|k| Species::ALL[(start + k) % Species::ALL.len()]).find(|&sp| {
            let sc = sim.flora().view().config.species(sp);
            establishment_gates(&sim.world().view(), site, sc).passes()
        });
        let Some(species) = choice else { continue };
        let sc = sim.flora().view().config.species(species).clone();
        let u = (next() >> 11) as f64 / (1u64 << 53) as f64;
        let wood = sc.alive_min + u * (0.6 * sc.wood_max - sc.alive_min).max(0.0);
        let ok = sim.with_layers_mut(|world, flora, _| {
            flora.apply(world, Command::SeedOnFace { site, species, wood })
        });
        if ok {
            planted += 1;
        }
    }
    eprintln!(
        "densified: {before} -> {} stands",
        sim.flora().view().stands.len()
    );
}

/// The phases a row reports, as (label, phase). Nested totals are listed for reading,
/// leaves beneath them.
const ROW: [(&str, Phase); 10] = [
    ("flora", Phase::FloraStep),
    ("light", Phase::Light),
    ("drink", Phase::Drink),
    ("feed", Phase::Feed),
    ("grow", Phase::Grow),
    ("bank", Phase::SeedBank),
    ("prop", Phase::Propagate),
    ("fauna", Phase::FaunaStep),
    ("sense", Phase::FaunaSense),
    ("act", Phase::FaunaAct),
];

fn grow(mut sim: Sim, minute0: u64, minutes: u64, every: u64, save: Option<&Path>) {
    let mut last = [0u64; Phase::COUNT];
    let mut since = Instant::now();
    for m in 1..=minutes {
        for _ in 0..TICKS_PER_MIN {
            sim.step();
        }
        let ms = since.elapsed().as_secs_f64() * 1e3 / TICKS_PER_MIN as f64;
        since = Instant::now();
        let minute = minute0 + m;
        let mut line = format!(
            "min={minute} stands={} ground={} animals={} tick_ms={ms:.2}",
            sim.flora().view().stands.len(),
            sim.flora().view().ground.len(),
            sim.fauna().view().animals.len(),
        );
        for (label, p) in ROW {
            let n = profile::nanos(p);
            let d = n.saturating_sub(last[p.index()]);
            line.push_str(&format!(" {label}={:.2}", d as f64 / TICKS_PER_MIN as f64 / 1e6));
        }
        for p in Phase::ALL {
            last[p.index()] = profile::nanos(p);
        }
        println!("{line}");
        if let Some(dir) = save
            && every > 0
            && minute % every == 0
        {
            save_to(dir, &sim, minute);
        }
    }
}

fn profile_run(mut sim: Sim, ticks: u64) {
    for _ in 0..20 {
        sim.step();
    }
    profile::reset();
    let cpu0 = thread_cpu_ns();
    let mut wall = 0.0;
    let mut worst = 0.0f64;
    for _ in 0..ticks {
        let at = Instant::now();
        sim.step();
        let t = at.elapsed().as_secs_f64();
        wall += t;
        worst = worst.max(t);
    }
    let cpu = thread_cpu_ns().saturating_sub(cpu0) as f64 / 1e6 / ticks as f64;
    let per = |p: Phase| profile::nanos(p) as f64 / ticks as f64 / 1e6;
    // On a shared machine the wall clock counts time spent waiting for a core; the main
    // thread's own CPU time does not. With `threads=1` every phase runs on this thread, so
    // `cpu_ms` is the tick's cost and `scale` converts a phase's wall share to it.
    println!(
        "stands={} ground={} animals={} ticks={ticks} tick_ms={:.2} worst_ms={:.2} \
         main_thread_cpu_ms={cpu:.2} scale={:.3}",
        sim.flora().view().stands.len(),
        sim.flora().view().ground.len(),
        sim.fauna().view().animals.len(),
        wall * 1e3 / ticks as f64,
        worst * 1e3,
        cpu / (wall * 1e3 / ticks as f64),
    );
    for p in Phase::ALL {
        if profile::calls(p) > 0 {
            println!("  {:<48} {:8.3} ms/tick", p.name(), per(p));
        }
    }
    // The water's leaves, the flora and fauna totals, and what is left of the tick.
    let water: f64 = [
        Phase::Begin,
        Phase::Rain,
        Phase::Evaporate,
        Phase::Infiltrate,
        Phase::Fall,
        Phase::Exchange,
        Phase::Drain,
        Phase::WaterTable,
        Phase::Spring,
        Phase::Outlet,
    ]
    .into_iter()
    .map(per)
    .sum();
    let (flora, fauna) = (per(Phase::FloraStep), per(Phase::FaunaStep));
    println!(
        "summary water={water:.2} flora={flora:.2} fauna={fauna:.2} other={:.2} ms/tick",
        wall * 1e3 / ticks as f64 - water - flora - fauna
    );
    // A digest of the state the run ended in, for comparing two builds on one snapshot:
    // the same rules give the same bits (the stands' observational `water_m3` is left out).
    {
        let (world, flora, fauna) = sim.layers();
        let f = flora.view();
        let sum = |g: &dyn Fn(&cubarium_voxel_flora::Stand) -> f64| -> f64 {
            f.stands.iter().map(g).sum()
        };
        let parts = [
            sum(&|s| s.wood),
            sum(&|s| s.foliage),
            sum(&|s| s.reserve),
            sum(&|s| s.mineral),
            sum(&|s| s.light),
            sum(&|s| s.moisture),
            f.ground.iter().map(|g| g.litter + g.dead_wood + g.mineral).sum(),
            f.ledger.fixed_in,
            f.ledger.respired_out,
            world.view().stored_m3(),
            fauna.view().animals.iter().map(|a| a.body).sum(),
        ];
        let bits: Vec<String> = parts.iter().map(|x| format!("{:016x}", x.to_bits())).collect();
        println!(
            "digest stands={} ground={} births={} deaths={} {}",
            f.stands.len(),
            f.ground.len(),
            f.ledger.births,
            f.ledger.deaths,
            bits.join(" ")
        );
    }
    for c in profile::Count::ALL {
        let n = profile::count(c);
        if n > 0 {
            println!("  {:<48} {:10.0} /tick", c.name(), n as f64 / ticks as f64);
        }
    }
}

/// This thread's CPU time in nanoseconds, from `/proc/thread-self/schedstat`; 0 where
/// that file does not exist.
fn thread_cpu_ns() -> u64 {
    std::fs::read_to_string("/proc/thread-self/schedstat")
        .ok()
        .and_then(|s| s.split_whitespace().next().and_then(|t| t.parse().ok()))
        .unwrap_or(0)
}
