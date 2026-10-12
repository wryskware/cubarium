//! Juvenile trace: what a lineage's growing bodies do with their time, beside its adults.
//!
//! ```text
//! cargo run --release -p cubarium --example juvenile_trace -- load=DIR minutes=15 [heuristic] [threads=2]
//! ```
//!
//! Loads a saved run (`voxel_census save=` / a staged checkpoint), installs the drivers as
//! `voxel_census load=` does (default heuristics; `heuristic` is a legacy no-op),
//! steps `minutes` simulated minutes and reads every founder body every tick.
//! Per lineage and stage (juvenile / adult) it prints, on stdout, once at the end:
//! - `awake_s`: awake body-seconds; `eat`: share of them with the eating latch on;
//! - `ask`: share of the latch-on controller samples whose held `feed` is on (the mouth
//!   tasted food); `bite`: share of the latch-on samples that took a bite (`Cropping`);
//! - `fwd`: mean held forward effort while the latch is on; `fwd_feed`: the same over
//!   samples that asked to feed (a stay-on-food forager holds it near 0);
//! - `stom`: mean stomach fill over its capacity; `full`: mean fullness;
//! - `in_rate`: assimilated organic per body-second, per unit of body, read off the gut
//!   (each pool's `S · speed / τ` at its mineral-limited yield), and `gut_max`: its ceiling,
//!   `yield · K/τ` per unit body for the lineage's first diet food — so `gut_use =
//!   in_rate / gut_max` is how much of its gut it uses;
//! - `grow`: body built per body-second, over `growth_max_per_s` (equals fullness: growth
//!   runs at `growth_max_per_s · fullness`);
//! - `idle`: latch-on samples held still without feeding; `delivery`: motion delivered over
//!   asked; `fwd_bins`: latch-on non-feeding samples by held forward (0 / ≤0.15 / ≤0.45 /
//!   more); `steer`: the layer that set the held actions.
//!
//! On stderr: the bloom stands' nectar and the sips (start → end), each lineage's stomach
//! mineral over organic against its `n_tissue`, and with `dump=N` every N-th latch-on
//! sample of each juvenile — with `adults`, of every body — (pose, held actions, steer,
//! mobility, fullness, what its controller said the answer was for: `seek`); with `probe`,
//! the walkable surface around each juvenile at the end (`lineage=NAME` narrows all of it).
//! With `ids=A,B,..` (and `every=N` samples): every N-th controller sample of those bodies,
//! whatever their stage or latch (`body` lines: pose, held actions, steer, state, mobility,
//! the DROP trapped clock, the FORAGE-LEAVE stay, the hunt mode), and at the end each one's
//! motor at 16 headings, whether it stands clear, and the air over the columns round it;
//! `drop` adds its route-graph standing and DROP's jump (`RouteGraph::drop_way`).
//!
//! Measurement only: it changes nothing in the run.

use cubarium::voxel::install_founders_with;
use cubarium_observatory::query::{LifeStage, stage_of};
use cubarium_voxel_fauna::{Founder, State, Steer, TICK_HZ};
use cubarium_voxel_sim::{Sim, SimConfig};
use std::collections::HashMap;

#[derive(Default, Clone, Copy)]
struct Acc {
    awake_ticks: f64,
    eat_ticks: f64,
    samples_eat: f64,
    asked: f64,
    bit: f64,
    fwd_sum: f64,
    fwd_feed_sum: f64,
    stom_sum: f64,
    full_sum: f64,
    body_sum: f64,
    intake: f64,
    grown: f64,
    ticks: f64,
    /// Latch-on samples held still (forward under 0.05) without feeding.
    idle: f64,
    /// Latch-on samples by the layer that set the held actions ([`Steer::ALL`] order).
    steer: [f64; Steer::ALL.len()],
    /// Latch-on, not-feeding samples by held forward: 0, (0, 0.15], (0.15, 0.45], more.
    fwd_bins: [f64; 4],
    /// Motion asked and delivered (equivalent metres), for the delivery ratio.
    asked_m: f64,
    got_m: f64,
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let get = |k: &str| args.iter().find_map(|a| a.strip_prefix(k).map(str::to_string));
    let dir = get("load=").expect("load=DIR");
    let minutes: f64 = get("minutes=").map_or(10.0, |s| s.parse().expect("minutes="));
    let threads: usize = get("threads=").map_or(2, |s| s.parse().expect("threads="));
    // `heuristic` remains accepted as a legacy no-op.
    let only: Option<String> = get("lineage=");
    // `dump=N`: also print every N-th latch-on controller sample of each juvenile (stderr).
    let dump: u64 = get("dump=").map_or(0, |s| s.parse().expect("dump="));
    let mut dumped: HashMap<u64, u64> = HashMap::new();
    // `adults`: the dump covers the adults too.
    let adults = args.iter().any(|a| a == "adults");
    // `ids=A,B,...`: every controller sample of these bodies, whatever their stage or latch
    // (stderr `body ...` lines): pose, held actions, steer, state and mobility.
    let ids: Vec<u64> = get("ids=").map_or_else(Vec::new, |s| s.split(',').map(|x| x.parse().expect("ids=")).collect());
    let every: u64 = get("every=").map_or(1, |s| s.parse().expect("every="));
    cubarium_voxel::set_thread_override(threads);

    let driven = cubarium::voxel::policy_founders(&[]);
    let saved = cubarium_voxel_sim::saved::load(std::path::Path::new(&dir), &driven)
        .unwrap_or_else(|e| panic!("load: {e}"));
    let (world, mut flora, mut fauna, minute) = (saved.world, saved.flora, saved.fauna, saved.minute);
    // `set=PATH=VALUE` (repeatable): flora/fauna parameters over the save's own configs,
    // as `voxel_census load= set=` resolves them.
    let sets: Vec<String> = args.iter().filter_map(|a| a.strip_prefix("set=").map(str::to_string)).collect();
    if !sets.is_empty() {
        let loaded = cubarium_voxel_sim::params::Params::over_loaded(world.config().clone(), flora.config(), fauna.config().clone(), &sets)
            .unwrap_or_else(|e| panic!("the parameter set: {e}"));
        flora.replace_config(loaded.flora.clone()).unwrap_or_else(|e| panic!("set= flora: {e}"));
        fauna.replace_config(loaded.params.fauna.clone()).unwrap_or_else(|e| panic!("set= fauna: {e}"));
        eprintln!("set {sets:?}");
    }
    install_founders_with(&mut fauna, &[]).expect("default heuristic controllers");
    eprintln!(
        "loaded {dir} at minute {minute}: animals {} ({})",
        fauna.view().animals.len(),
        "heuristic"
    );
    let mut sim = Sim::ambient(world, flora, fauna, SimConfig::experiment());

    let ticks = (minutes * 60.0 * TICK_HZ as f64).round() as u64;
    let dt_tick = 1.0 / TICK_HZ as f64;
    // The nectar supply: stand nectar held (bloomcrowns), stands holding any, and what the
    // bellwings ate, at the start and the end.
    let nectar = |sim: &Sim| {
        let fv = sim.flora().view();
        let held: f64 = fv.stands.iter().map(|s| s.nectar).sum();
        let holding = fv.stands.iter().filter(|s| s.nectar > 0.0).count();
        let l = sim.fauna().view().ledger;
        let nectar = cubarium_voxel_fauna::Food::Nectar.index();
        (held, holding, l.eaten_by_food[nectar], l.sips_by_bloom, l.sip_misses_by_founder[Founder::Bellwing.index()])
    };
    let n0 = nectar(&sim);
    let light0 = sim.fauna().view().ledger.light_budget[Founder::Bellwing.index()];
    // Per body: its body at the last tick (growth), and its motion feedback so far.
    let mut last: HashMap<u64, f64> = HashMap::new();
    let mut moved: HashMap<u64, (f64, f64)> = HashMap::new();
    let mut acc: HashMap<(Founder, LifeStage), Acc> = HashMap::new();
    for _ in 0..ticks {
        sim.step();
        let fauna = sim.fauna();
        let config = fauna.config();
        for a in fauna.view().animals {
            let Some(f) = a.founder else { continue };
            if ids.contains(&a.id) && a.age_ticks % (f.manifest().cadence_ticks() * every) == 0 {
                let h = a.founder_state.held;
                let m = a.mobility;
                let fbk = a.founder_state.feedback;
                // The light reading (S3): the still beacon it steers by, `kind@(x,z,y)` and its
                // horizontal distance, or `-`; the open inspection's age in ticks.
                let light = fauna.percept(a.id).map_or_else(String::new, |p| {
                    let l = p.light;
                    let beacon = if l.beacon {
                        let ring = f64::from(sim.world().view().config.width) * sim.world().view().config.voxel_m;
                        let mut dx = (l.beacon_x - a.pose.x).rem_euclid(ring);
                        if dx > ring / 2.0 {
                            dx -= ring;
                        }
                        format!("{}@({:.2},{:.2},{:.2})d={:.2}", l.beacon_kind, l.beacon_x, l.beacon_z, l.beacon_y, dx.hypot(l.beacon_z - a.pose.z))
                    } else {
                        "-".to_string()
                    };
                    let visit = if p.visit.active { (fauna.view().tick.saturating_sub(p.visit.since)).to_string() } else { "-".to_string() };
                    // The class expectations (near-IR, cyan), the nearest lit lure and the nearest
                    // nectar-holding stand (horizontal metres, its site's cell height), for the
                    // light and smell leads.
                    let now = fauna.view().tick;
                    let (nir, cyan) = (
                        cubarium_voxel_fauna::light::expectation_of(&config.light, p, 0, now),
                        cubarium_voxel_fauna::light::expectation_of(&config.light, p, 1, now),
                    );
                    let wv = sim.world().view();
                    let v = wv.config.voxel_m;
                    let ring = f64::from(wv.config.width) * v;
                    let hd = |x: f64, z: f64| {
                        let mut dx = (x - a.pose.x).rem_euclid(ring);
                        if dx > ring / 2.0 {
                            dx -= ring;
                        }
                        dx.hypot(z - a.pose.z)
                    };
                    let lure = cubarium_voxel_fauna::light::lures(fauna, &wv)
                        .iter()
                        .map(|b| (hd(b.x, b.z), b.y_m))
                        .min_by(|p, q| p.0.total_cmp(&q.0))
                        .map_or("-".to_string(), |(d, y)| format!("{d:.2}/y{y:.2}"));
                    let fv = sim.flora().view();
                    let stand = fv
                        .stands
                        .iter()
                        .filter(|s| s.nectar > 0.0)
                        .map(|s| {
                            let head = fv.profile_layers(s).into_iter().filter(|l| l.kind.bears_foliage()).map(|l| l.cell).max().unwrap_or(-1);
                            (hd((f64::from(s.site.x) + 0.5) * v, (f64::from(s.site.z) + 0.5) * v), s.site.y, head, s.nectar)
                        })
                        .min_by(|p, q| p.0.total_cmp(&q.0))
                        .map_or("-".to_string(), |(d, y, h, n)| format!("{d:.2}/c{y}/head{h}/{n:.4}"));
                    format!(
                        " amb={:.3} beacon={beacon} visit={visit} marks={} expect={nir:.2}/{cyan:.2} lure={lure} stand={stand}",
                        l.ambient,
                        p.marks.iter().filter(|m| m.until_tick > now).count()
                    )
                });
                eprintln!(
                    "body t={} id={} {} x={:.3} z={:.3} y={} ym={:.3} hd={:.2} fwd={:.2} turn={:.2} feed={:.0} lift={:.1} steer={:?} state={:?} mode={:?} eat={} emerg={} full={:.3} body={:.3} thirsty={} ashore={} dry={:.2} retract={} climb={} perch={} falling={} att={:.4} got={:.4} trapped={:?} wade={:?} stay={:?} hunt={:?}{light}",
                    fauna.view().tick, a.id, f.name(), a.pose.x, a.pose.z, a.site.y, a.pose.y_m, a.pose.heading_rad,
                    h.forward, h.turn, h.feed, a.founder_state.lift, fauna.steer_of(a.id), a.state, m.mode,
                    a.eating, a.emergency, a.reserve / config.founder(f).core.reserve_of(a.body).max(1e-12), a.body,
                    m.thirsty, m.ashore, m.dryness, m.retract_ticks, m.climb.is_some(), m.perch.is_some(), m.falling,
                    fbk.attempted_equivalent, fbk.delivered_equivalent, fauna.trapped_of(a.id), fauna.wade_state(a.id), fauna.forage_stays().into_iter().find(|s| s.0 == a.id), a.hunt.mode
                );
            }
            if only.as_deref().is_some_and(|l| l != f.name()) {
                continue;
            }
            let stage = stage_of(config, a);
            if !matches!(stage, LifeStage::Juvenile | LifeStage::Adult) {
                continue;
            }
            let phys = config.founder(f);
            let e = acc.entry((f, stage)).or_default();
            let fbk = a.founder_state.feedback;
            let (pa, pg) = moved.get(&a.id).copied().unwrap_or((fbk.attempted_equivalent, fbk.delivered_equivalent));
            if fbk.attempted_equivalent + 1e-15 < pa {
                e.asked_m += fbk.attempted_equivalent;
                e.got_m += fbk.delivered_equivalent;
            } else {
                e.asked_m += fbk.attempted_equivalent - pa;
                e.got_m += fbk.delivered_equivalent - pg;
            }
            moved.insert(a.id, (fbk.attempted_equivalent, fbk.delivered_equivalent));
            let prev_body = last.get(&a.id).copied().unwrap_or(a.body);
            last.insert(a.id, a.body);
            e.ticks += 1.0;
            // The feedback is zeroed at each sampling, inside the tick, so a lineage
            // sampled every tick never shows it here: the intake is the gut's own rate
            // instead — every pool's digestion, `S · speed / τ`, at its mineral-limited
            // yield (the reserve's room never binds a body this far under its cap).
            let speed = phys.gut.throughput_factor(a.body, phys.core.body_max);
            for food in cubarium_voxel_fauna::Food::ALL {
                let p = a.stomach.pools[food.index()];
                if !(p.organic > 0.0) {
                    continue;
                }
                let funded = if phys.core.n_tissue > 0.0 {
                    p.mineral / (phys.core.n_tissue * p.organic)
                } else {
                    f64::INFINITY
                };
                let y = phys.yield_for(food).max(0.0);
                let raw = p.organic * speed / phys.gut.digestion_s[food.index()] * dt_tick;
                let tissue = raw * y.min(funded);
                let fuel_room = (phys.core.fuel_cap * a.body - a.fuel).max(0.0);
                e.intake += tissue + (raw * y - tissue).min(fuel_room);
            }
            if a.body > prev_body {
                e.grown += a.body - prev_body;
            }
            e.body_sum += a.body;
            let cap = phys.core.reserve_of(a.body);
            e.full_sum += if cap > 0.0 { (a.reserve / cap).clamp(0.0, 1.0) } else { 0.0 };
            let k = phys.gut.capacity(a.body);
            e.stom_sum += if k > 0.0 { a.stomach.organic() / k } else { 0.0 };
            if a.state == State::Asleep {
                continue;
            }
            e.awake_ticks += 1.0;
            if !a.eating {
                continue;
            }
            e.eat_ticks += 1.0;
            let held = a.founder_state.held;
            e.fwd_sum += held.forward;
            if a.age_ticks % f.manifest().cadence_ticks() == 0 {
                e.samples_eat += 1.0;
                if dump > 0 && (stage == LifeStage::Juvenile || adults) {
                    let n = dumped.entry(a.id).or_default();
                    if *n % dump == 0 {
                        eprintln!(
                            "dump {:?} t={} id={} x={:.2} z={:.2} y={} hd={:.2} fwd={:.2} turn={:.2} feed={:.0} steer={:?} state={:?} body={:.3} full={:.3} mode={:?} rain={} emerg={} lift={:.1} stom={:.2} seek={:?}",
                            stage, fauna.view().tick, a.id, a.pose.x, a.pose.z, a.site.y, a.pose.heading_rad,
                            held.forward, held.turn, held.feed, fauna.steer_of(a.id), a.state, a.body,
                            a.reserve / phys.core.reserve_of(a.body).max(1e-12),
                            a.mobility.mode, a.mobility.rain_grounded, a.emergency, a.founder_state.lift,
                            a.stomach.organic() / phys.gut.capacity(a.body).max(1e-30),
                            fauna.seek_of(a.id)
                        );
                    }
                    *n += 1;
                }
                if held.forward < 0.05 && held.feed < 0.5 {
                    e.idle += 1.0;
                }
                if let Some(st) = fauna.steer_of(a.id) {
                    if let Some(k) = Steer::ALL.iter().position(|x| *x == st) {
                        e.steer[k] += 1.0;
                    }
                }
                if held.feed <= 0.5 {
                    let b = if held.forward <= 0.0 { 0 } else if held.forward <= 0.15 { 1 } else if held.forward <= 0.45 { 2 } else { 3 };
                    e.fwd_bins[b] += 1.0;
                }
                if held.feed > 0.5 {
                    e.asked += 1.0;
                    e.fwd_feed_sum += held.forward;
                }
                if a.state == State::Cropping {
                    e.bit += 1.0;
                }
            }
        }
    }
    // With `ids=`: each traced body's motor at the end, at 16 headings (`+` a sub-step of
    // half a radius taken, `.` refused), whether it stands clear now, and the air over each
    // column round it (voxels to the first solid above its surface, `+` for 30 or more).
    if !ids.is_empty() {
        let w = sim.world().view();
        let c = w.config;
        for a in sim.fauna().view().animals.iter().filter(|a| ids.contains(&a.id)) {
            let Some(f) = a.founder else { continue };
            let phys = sim.fauna().config().founder(f);
            let g = phys.body_at(a.body);
            let (r, head) = (g.footprint_radius(), g.headroom_voxels(c.voxel_m));
            let mut ring = String::new();
            let mut clear = false;
            for step in [0.5 * r, 0.02] {
                ring.push(' ');
                for k in 0..16 {
                    let h = k as f64 * std::f64::consts::TAU / 16.0;
                    let (ok, now) = cubarium_voxel_fauna::motor_probe(&w, a.pose.x, a.pose.z, (h.sin(), h.cos()), a.site.y, step, &g, phys.wade_depth_m(&g), phys.step_limits(c.voxel_m));
                    ring.push(if ok { '+' } else { '.' });
                    clear = now;
                }
            }
            eprintln!(
                "motor id={} {} r={r:.3} headroom={head} site=({},{},{}) pose=({:.3},{:.3}) water={:.3} wade={:.3} clear_now={clear} headings(0=+z,4=+x; half radius, 2 cm)={ring}",
                a.id, f.name(), a.site.x, a.site.y, a.site.z, a.pose.x, a.pose.z,
                w.water_depth_m(i64::from(a.site.x), a.site.y, a.site.z), phys.wade_depth_m(&g)
            );
            if args.iter().any(|a| a == "drop") {
                let cfg = sim.fauna().config();
                let graph = cubarium_voxel_fauna::RouteGraph::for_founder(&w, phys, cfg.founding.clearance_margin_m);
                let v = c.voxel_m;
                let step = phys.step_limits(v);
                let reach = cubarium_voxel_fauna::step_voxels(phys.drop.reach_m(phys, &g), v);
                let own = (r / v).ceil() as u32;
                let cols = (cfg.teachers.way_on_m / v).ceil() as u32;
                eprintln!("graph: node_near={:?} way_on={:?}", graph.node_near(a.site, (a.pose.x, a.pose.z), own), graph.way_on(&w, a.site, (a.pose.x, a.pose.z), r, head, step, phys.wade_depth_m(&g), own, cols));
                if let Some(n) = graph.node_near(a.site, (a.pose.x, a.pose.z), own) {
                    let comp = graph.components[n];
                    let size = graph.components.iter().filter(|&&k| k == comp).count();
                    let mut sizes: HashMap<usize, usize> = HashMap::new();
                    for &k in &graph.components {
                        *sizes.entry(k).or_default() += 1;
                    }
                    let biggest = sizes.values().copied().max().unwrap_or(0);
                    eprintln!("graph: node {n} at {:?} component {comp} of {size} nodes (largest {biggest}, {} components)", graph.faces[n], sizes.len());
                }
                let way = graph.drop_way(&w, a.site, (a.pose.x, a.pose.z), r, head, step, reach, phys.wade_depth_m(&g), own, cols);
                eprintln!("drop_way: {way:?}");
                let window = 2 * (cfg.teachers.way_on_m / v).ceil() as usize + 1;
                let sc = (cfg.life.shallows_search_m / v).ceil() as u32;
                for (open_min, under, max_under) in [(window * window, phys.drown_depth_m(&g), 40), (window * window, phys.drown_depth_m(&g), 0), (1, 10.0, 0)] {
                    let wade = graph.wade_way(&w, a.site, r, head, step, (phys.wade_depth_m(&g), under, max_under), own, open_min, sc, &mut |_| true);
                    eprintln!("wade_way open_min={open_min} under={under:.3} max_under={max_under}: {:?}", wade.map(|(f, d, u)| (f.len(), f.last().copied(), d, u)));
                }
            }
            let (cx, cz) = ((a.pose.x / c.voxel_m).floor() as i64, (a.pose.z / c.voxel_m).floor() as i64);
            for dz in -4i64..=4 {
                let mut line = String::new();
                for dx in -4i64..=4 {
                    let (x, z) = ((cx + dx).rem_euclid(i64::from(c.width)), cz + dz);
                    if z < 0 || z >= i64::from(c.depth) {
                        line.push_str("  | ");
                        continue;
                    }
                    let y0 = i64::from(a.site.y);
                    let solid = |yy: i64| yy >= 0 && yy < i64::from(c.height) && w.is_solid(x, yy as u32, z as u32);
                    let surf = (0..=9i64).flat_map(|d| [d, -d]).find(|d| solid(y0 + d) && !solid(y0 + d + 1));
                    let cell = match surf {
                        None => "##".to_string(),
                        Some(d) => {
                            let gap = (1..30i64).find(|g| solid(y0 + d + g)).map_or("+".to_string(), |g| (g - 1).to_string());
                            format!("{d:+}/{gap}")
                        }
                    };
                    line.push_str(&format!("{cell:>6}{}", if dx == 0 && dz == 0 { '*' } else { ' ' }));
                }
                eprintln!("  {line}");
            }
            // `water=R`: a (2R+1)² window of columns round it, each its top surface within
            // 12 layers of its own (relative layers) and the standing water there in cm
            // (`h:cm`, `h.` for dry; `#` no surface in that span); `|` past the strip's ends.
            if let Some(rad) = get("water=").map(|s| s.parse::<i64>().expect("water=")) {
                eprintln!("water id={} wade_cm={:.1} (rows +z downward, columns +x)", a.id, 100.0 * phys.wade_depth_m(&g));
                let y0 = i64::from(a.site.y);
                for dz in -rad..=rad {
                    let mut line = String::new();
                    for dx in -rad..=rad {
                        let (x, z) = ((cx + dx).rem_euclid(i64::from(c.width)), cz + dz);
                        if z < 0 || z >= i64::from(c.depth) {
                            line.push_str("    |");
                            continue;
                        }
                        let solid = |yy: i64| yy >= 0 && yy < i64::from(c.height) && w.is_solid(x, yy as u32, z as u32);
                        let top = (-12..=12i64).rev().find(|d| solid(y0 + d) && !solid(y0 + d + 1));
                        let cell = match top {
                            None => "#".to_string(),
                            Some(d) => {
                                let cm = 100.0 * w.water_depth_m(x, (y0 + d) as u32, z as u32);
                                if cm < 0.5 { format!("{d:+}.") } else { format!("{d:+}:{cm:.0}") }
                            }
                        };
                        line.push_str(&format!("{cell:>5}{}", if dx == 0 && dz == 0 { '*' } else { ' ' }));
                    }
                    eprintln!("  {line}");
                }
            }
            // `wmap=R`: the same window one character a column: `.` dry, `w` water it wades,
            // `o` up to twice that, `O` deeper, `#` no surface within 40 layers, `*` itself;
            // then the surface layer relative to its own (`0`-`9` up, `a`-`z` down 1..26).
            if let Some(rad) = get("wmap=").map(|s| s.parse::<i64>().expect("wmap=")) {
                let wade = phys.wade_depth_m(&g);
                let y0 = i64::from(a.site.y);
                eprintln!("wmap id={} wade_cm={:.1} centre col ({cx},{cz}) rows z from {} downward", a.id, 100.0 * wade, (cz - rad).max(0));
                let mut hl = Vec::new();
                for dz in -rad..=rad {
                    let z = cz + dz;
                    if z < 0 || z >= i64::from(c.depth) {
                        continue;
                    }
                    let (mut line, mut hline) = (String::new(), String::new());
                    for dx in -rad..=rad {
                        let x = (cx + dx).rem_euclid(i64::from(c.width));
                        let solid = |yy: i64| yy >= 0 && yy < i64::from(c.height) && w.is_solid(x, yy as u32, z as u32);
                        let top = (-40..=40i64).rev().find(|d| solid(y0 + d) && !solid(y0 + d + 1));
                        let (ch, hc) = match top {
                            None => ('#', '#'),
                            Some(d) => {
                                let m = w.water_depth_m(x, (y0 + d) as u32, z as u32);
                                let ch = if m < 0.005 { '.' } else if m <= wade { 'w' } else if m <= 2.0 * wade { 'o' } else { 'O' };
                                let hc = if d >= 0 { char::from_digit(d.min(9) as u32, 10).unwrap_or('9') } else { (b'a' + ((-d).min(26) - 1) as u8) as char };
                                (ch, hc)
                            }
                        };
                        line.push(if dx == 0 && dz == 0 { '*' } else { ch });
                        hline.push(hc);
                    }
                    eprintln!("  {line}");
                    hl.push(hline);
                }
                for h in hl {
                    eprintln!("  {h}");
                }
            }
        }
    }
    // `probe`: the ground around each juvenile at the end, a 9 x 9 grid of columns.
    if args.iter().any(|a| a == "probe") {
        let w = sim.world().view();
        let c = w.config;
        for a in sim.fauna().view().animals {
            let Some(f) = a.founder else { continue };
            if only.as_deref().is_some_and(|l| l != f.name()) || stage_of(sim.fauna().config(), a) != LifeStage::Juvenile {
                continue;
            }
            let (cx, cz) = ((a.pose.x / c.voxel_m).floor() as i64, (a.pose.z / c.voxel_m).floor() as i64);
            eprintln!("probe id={} at col ({cx},{cz}) y={} body={:.3}", a.id, a.site.y, a.body);
            for dz in -4i64..=4 {
                let mut line = String::new();
                for dx in -4i64..=4 {
                    let (x, z) = (cx + dx, cz + dz);
                    if z < 0 || z >= i64::from(c.depth) {
                        line.push('|');
                        continue;
                    }
                    let x = x.rem_euclid(i64::from(c.width));
                    // The walkable surface nearest the standing face (solid with air above),
                    // relative to it: `+3` a three-voxel riser, `-5` a five-voxel drop, `##`
                    // none within nine.
                    let y = i64::from(a.site.y);
                    let solid = |yy: i64| yy >= 0 && yy < i64::from(c.height) && w.is_solid(x, yy as u32, z as u32);
                    let surf = (0..=9i64)
                        .flat_map(|d| [d, -d])
                        .find(|d| solid(y + d) && !solid(y + d + 1));
                    let cell = surf.map_or("##".to_string(), |d| format!("{d:+}"));
                    line.push_str(&format!("{cell:>3}"));
                    line.push(' ');
                    if dx == 0 && dz == 0 {
                        line.push('*');
                        continue;
                    }
                    line.push(' ');
                }
                eprintln!("  {line}");
            }
        }
    }
    let dt = 1.0 / TICK_HZ as f64;
    let n1 = nectar(&sim);
    // The bellwing's light layer: its samples by mode over the run (share of them).
    {
        let l1 = sim.fauna().view().ledger.light_budget[Founder::Bellwing.index()];
        let names = cubarium_voxel_fauna::light::LightMode::NAMES;
        let total: u64 = (0..names.len()).map(|k| l1[k] - light0[k]).sum();
        let parts: Vec<String> = names.iter().enumerate().map(|(k, n)| format!("{n}={} ({:.3})", l1[k] - light0[k], (l1[k] - light0[k]) as f64 / total.max(1) as f64)).collect();
        eprintln!("bellwing light layer: {total} samples: {}", parts.join(", "));
    }
    // The stomach's mineral over its organic, per food, against the eater's `n_tissue`:
    // under it the food cannot fund the tissue its yield would build.
    for f in [Founder::Bellwing, Founder::Browser, Founder::Capgnawer, Founder::Loftstrider] {
        let n_tissue = sim.fauna().config().founder(f).core.n_tissue;
        let mut by_food = [(0.0f64, 0.0f64); cubarium_voxel_fauna::Food::COUNT];
        for a in sim.fauna().view().animals.iter().filter(|a| a.founder == Some(f)) {
            for (k, p) in a.stomach.pools.iter().enumerate() {
                by_food[k].0 += p.organic;
                by_food[k].1 += p.mineral;
            }
        }
        for food in cubarium_voxel_fauna::Food::ALL {
            let (o, m) = by_food[food.index()];
            if o > 0.0 {
                eprintln!("stomach: {} {} organic {o:.3e} mineral/organic {:.4} (n_tissue {n_tissue})", f.name(), food.name(), m / o);
            }
        }
    }
    {
        let fv = sim.flora().view();
        let (mut n, mut m) = (0.0, 0.0);
        for st in fv.stands.iter().filter(|st| st.nectar > 0.0) {
            n += st.nectar;
            m += st.mineral;
        }
        eprintln!("bloom stands: nectar {n:.3e}, their mineral {m:.3e}");
    }
    eprintln!(
        "nectar: stand stock {:.4e} -> {:.4e} on {} -> {} stands; nectar eaten {:.4e} ({:.3e}/s); sips stand {} vine {}; bellwing misses {}",
        n0.0, n1.0, n0.1, n1.1, n1.2 - n0.2, (n1.2 - n0.2) / (minutes * 60.0),
        n1.3[0] - n0.3[0], n1.3[1] - n0.3[1], n1.4 - n0.4
    );
    let fauna = sim.fauna();
    // LEADS-PAY: the leads each lineage gave up over the run.
    let gave_up = fauna.lead_give_ups();
    let gave_up: Vec<String> = Founder::ALL.iter().filter(|f| gave_up[f.index()] > 0).map(|f| format!("{} {}", f.name(), gave_up[f.index()])).collect();
    eprintln!("leads given up: {}", if gave_up.is_empty() { "none".to_string() } else { gave_up.join(", ") });
    println!("lineage,stage,body_mean,awake_s,eat,ask,bite,fwd,fwd_feed,stom,full,in_rate,gut_max,gut_use,grow,idle,delivery,fwd_bins,steer");
    let mut keys: Vec<_> = acc.keys().copied().collect();
    keys.sort_by_key(|(f, s)| (f.index(), *s as u8));
    for key in keys {
        let e = acc[&key];
        let (f, stage) = key;
        let phys = fauna.config().founder(f);
        let food = phys.eating_diet().foods().next();
        let gut_max = food.map_or(0.0, |food| {
            phys.yield_for(food) * phys.gut.capacity(1.0) / phys.gut.digestion_s[food.index()]
        });
        let body_s = e.ticks * dt;
        let in_rate = e.intake / (e.body_sum * dt).max(1e-12);
        let steer: Vec<String> = Steer::ALL
            .iter()
            .zip(e.steer)
            .filter(|(_, n)| *n > 0.0)
            .map(|(s, n)| format!("{}={:.2}", s.name(), n / e.samples_eat.max(1.0)))
            .collect();
        println!(
            "{},{},{:.3},{:.0},{:.3},{:.3},{:.3},{:.3},{:.3},{:.3},{:.4},{:.3e},{:.3e},{:.3},{:.3},{:.3},{:.3},{},{}",
            f.name(),
            stage.name(),
            e.body_sum / e.ticks.max(1.0),
            e.awake_ticks * dt,
            e.eat_ticks / e.awake_ticks.max(1.0),
            e.asked / e.samples_eat.max(1.0),
            e.bit / e.samples_eat.max(1.0),
            e.fwd_sum / e.eat_ticks.max(1.0),
            e.fwd_feed_sum / e.asked.max(1.0),
            e.stom_sum / e.ticks.max(1.0),
            e.full_sum / e.ticks.max(1.0),
            in_rate,
            gut_max,
            if gut_max > 0.0 { in_rate / gut_max } else { 0.0 },
            e.grown / body_s.max(1e-12) / phys.core.growth_max_per_s.max(1e-30),
            e.idle / e.samples_eat.max(1.0),
            e.got_m / e.asked_m.max(1e-12),
            {
                let n: f64 = e.fwd_bins.iter().sum::<f64>().max(1.0);
                e.fwd_bins.iter().map(|x| format!("{:.2}", x / n)).collect::<Vec<_>>().join("/")
            },
            steer.join(" "),
        );
    }
}
