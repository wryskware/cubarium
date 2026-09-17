//! Two producers on the generated strip: founder stands of bloomcrown on the highest
//! support faces and of umbrellafrond in the lowest, then N seconds of coupled world
//! and plants.
//!
//! ```text
//! cargo run --release -p cubarium-voxel-flora --example two_producers -- [seconds] [seed] [noise_seed]
//! cargo run --release -p cubarium-voxel-flora --example two_producers -- compare [seconds] [seed] [noise_a] [noise_b] [control_seed]
//! ```
//!
//! The generated world starts bone dry and the plant model reads pore water, so the
//! example rains on it: `rain_m_per_s = 0.0005` is not weather, it is a tap, chosen to
//! bring flat ground to field capacity inside a 50-second warm-up without filling the
//! basin faster than the outlet can export. The outlet is opened for the same reason.
//!
//! What a single run prints: per-species stand counts, occupancy by support-height
//! quartile (the quartiles are of the *terrain's* own skyline, so "q4" is the top
//! quarter of the surface, not of the stands), the set of occupied skyline columns, and
//! the two flora residuals.
//!
//! `compare` is the decisive experiment of `design/voxel-ecology-sketch-2026-09-16.md`
//! §4: re-draw **only** the generator's final weak correlated noise with a new seed,
//! keep the landform, seed the same founder columns, and see whether each species
//! reoccupies the same ground. A control run with a different `seed` — a different
//! landform entirely — says what "unrelated" looks like on the same scale. If the
//! noise-pair overlap is no better than the control's, the terrain coupling is
//! decorative and the patches are reading the noise.

use cubarium_voxel::{Command as WorldCommand, Config as VoxelConfig, World};
use cubarium_voxel_flora::{Command, Flora, FloraConfig, Site, Species, Stage};

const WARMUP_TICKS: u32 = 1000;
const FOUNDERS_PER_SPECIES: usize = 8;
const FOUNDER_WOOD: f64 = 0.3;

/// A column a founder was planted in. The `y` is not part of it: under another noise
/// seed the same column's support face may sit a voxel higher or lower, and it is the
/// same place on the map.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Founder {
    species: Species,
    x: u32,
    z: u32,
}

/// Everything one run leaves behind for the comparison.
struct Outcome {
    seed: u64,
    noise_seed: u64,
    /// Occupied skyline columns per species, sorted, deduplicated: every stand.
    occupied: [Vec<(u32, u32)>; 2],
    /// The same, alive stands only.
    alive: [Vec<(u32, u32)>; 2],
    /// The columns of the lowest and the highest quarter of this run's own skyline: the
    /// hollows and the ridges, as pure landform, with no plant and no water in them.
    low_quartile: Vec<(u32, u32)>,
    high_quartile: Vec<(u32, u32)>,
    /// Per species, the skyline columns whose highest support passed the founder
    /// establishment predicate after the warm-up and before any plant acted: the
    /// species' *habitat*, pure terrain and water. The patch itself is bounded by the
    /// founders, which are held identical across runs on purpose, so this is the set
    /// that answers "do the hollows stay put" with the founders out of the way.
    eligible: [Vec<(u32, u32)>; 2],
    /// The founders this run's own selection rule would have planted, whatever it was
    /// actually handed: umbrellafrond from the bottom of its eligible skyline, bloomcrown
    /// from the top. Comparing these across runs asks where the *process* would put a
    /// species when it is free to choose, which the fixed-founder arms deliberately do
    /// not.
    own_founders: Vec<Founder>,
    /// What was actually planted.
    founders: Vec<Founder>,
    /// How many of the founder columns handed to this run no longer passed their
    /// species' establishment predicate, and were seeded anyway.
    off_predicate: usize,
}

fn sp(species: Species) -> usize {
    match species {
        Species::Bloomcrown => 0,
        Species::Umbrellafrond => 1,
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.first().map(String::as_str) == Some("compare") {
        compare(&args[1..]);
        return;
    }
    let seconds: f64 = arg(&args, 0).unwrap_or(60.0);
    let seed: u64 = arg(&args, 1).unwrap_or(1);
    let noise_seed: u64 = arg(&args, 2).unwrap_or(0);
    run(seconds, seed, noise_seed, None, true);
}

fn arg<T: std::str::FromStr>(args: &[String], i: usize) -> Option<T> {
    args.get(i).and_then(|s| s.parse().ok())
}

/// One run: generate, warm up, seed founders, step the coupled world and plants.
///
/// `founders` is `None` for a run that picks its own — the base of a comparison, or a
/// single run — and `Some` for one that must plant the same columns as another, whether
/// or not its own terrain still likes them.
fn run(
    seconds: f64,
    seed: u64,
    noise_seed: u64,
    founders: Option<&[Founder]>,
    verbose: bool,
) -> Outcome {
    // Generate once to find where the generator put the basin, then generate the world
    // the run uses with the water table charged to a metre above that floor. Generation
    // is deterministic in the seed, so the second world is the first one with water in
    // it. The outlet cell is the lowest void cell of the receiving basin, so its `y` is
    // the basin floor.
    let dry = VoxelConfig { seed, noise_seed, rain_m_per_s: 0.0005, ..VoxelConfig::default() };
    let basin_floor_m =
        World::new(dry.clone()).outlet_cell().map_or(0.0, |(_, y, _)| y as f64) * dry.voxel_m;
    let config = VoxelConfig { initial_aquifer_head_m: basin_floor_m + 1.0, ..dry };
    let (width, depth) = (config.width, config.depth);
    let mut world = World::new(config.clone());
    world.apply(WorldCommand::SetOutlet { open: true });
    for _ in 0..WARMUP_TICKS {
        world.step();
    }

    // Every column's own highest support face, sorted low to high: the ridges are the
    // tail of this list and the hollows are its head.
    let mut skyline: Vec<Site> = Vec::new();
    for z in 0..depth {
        for x in 0..width as i64 {
            if let Some(site) = cubarium_voxel_flora::highest_support(&world.view(), x, z) {
                skyline.push(site);
            }
        }
    }
    skyline.sort_by_key(|s| (s.y, s.x, s.z));

    let mut flora = Flora::new(FloraConfig::default());
    let mut eligible = [Vec::new(), Vec::new()];
    for species in Species::ALL {
        let sc = flora.config().species(species);
        let mut set: Vec<(u32, u32)> =
            skyline.iter().filter(|s| passes(&world, sc, **s)).map(|s| (s.x, s.z)).collect();
        set.sort_unstable();
        eligible[sp(species)] = set;
    }
    let own_founders = pick_founders(&world, &flora, &skyline, verbose && founders.is_none());
    let (planted, off_predicate) = match founders {
        None => (own_founders.clone(), 0),
        Some(given) => (given.to_vec(), off_count(&world, &flora, given)),
    };
    let mut seeded = 0;
    for f in &planted {
        if flora.apply(
            &world,
            Command::Seed { x: f.x as i64, z: f.z, species: f.species, wood: FOUNDER_WOOD },
        ) {
            seeded += 1;
        }
    }
    if verbose && founders.is_some() {
        println!(
            "founders: the base run's {} columns replanted here, {off_predicate} of them no \
             longer passing their species' establishment predicate (seeded anyway)",
            planted.len()
        );
    }

    let ticks = (seconds * cubarium_voxel::TICK_HZ as f64).round() as u64;
    for _ in 0..ticks {
        world.step();
        flora.step(&mut world);
    }

    let mut occupied = [Vec::new(), Vec::new()];
    let mut alive = [Vec::new(), Vec::new()];
    for stand in flora.view().stands {
        let key = (stand.site.x, stand.site.z);
        occupied[sp(stand.species)].push(key);
        if stand.stage == Stage::Alive {
            alive[sp(stand.species)].push(key);
        }
    }
    for set in occupied.iter_mut().chain(alive.iter_mut()) {
        set.sort_unstable();
        set.dedup();
    }
    // The lowest quarter of this run's own skyline, by column.
    let cut = skyline.len() / 4;
    let mut low_quartile: Vec<(u32, u32)> = skyline[..cut].iter().map(|s| (s.x, s.z)).collect();
    low_quartile.sort_unstable();
    let mut high_quartile: Vec<(u32, u32)> =
        skyline[skyline.len() - cut..].iter().map(|s| (s.x, s.z)).collect();
    high_quartile.sort_unstable();

    let outcome = Outcome {
        seed,
        noise_seed,
        occupied,
        alive,
        low_quartile,
        high_quartile,
        eligible,
        own_founders,
        founders: planted,
        off_predicate,
    };
    if verbose {
        report(&world, &flora, &config, &skyline, seeded, ticks, seconds, basin_floor_m);
        print_columns(&outcome);
    }
    outcome
}

/// Founders go where their own species could establish: wet enough at the support face,
/// bright enough, and not already under water it cannot stand in. Without that filter
/// every founder of the high species lands on bare sloping rock and dies in the first
/// tick — see the commit message on why a slope never wets.
fn pick_founders(
    world: &World,
    flora: &Flora,
    skyline: &[Site],
    verbose: bool,
) -> Vec<Founder> {
    let mut out = Vec::new();
    for (species, from_the_top) in [(Species::Umbrellafrond, false), (Species::Bloomcrown, true)] {
        let sc = flora.config().species(species);
        let mut ok: Vec<Site> = skyline.iter().copied().filter(|s| passes(world, sc, *s)).collect();
        let n = ok.len();
        // A species with nowhere to establish is still seeded, on the sites its own
        // ordering prefers, so the run has two producers in it and the summary shows what
        // happens to it. The printed count is the honest one.
        if ok.is_empty() {
            ok = skyline.to_vec();
        }
        if from_the_top {
            ok.reverse();
        }
        let stride = (ok.len() / FOUNDERS_PER_SPECIES).max(1);
        let mut lo = u32::MAX;
        let mut hi = 0;
        for site in ok.iter().step_by(stride).take(FOUNDERS_PER_SPECIES) {
            out.push(Founder { species, x: site.x, z: site.z });
            lo = lo.min(site.y);
            hi = hi.max(site.y);
        }
        if verbose {
            println!(
                "{:>14}: {n} of {} skyline sites pass its establishment predicate; founders at y {lo}..{hi}{}",
                species.name(),
                skyline.len(),
                if n == 0 { " (seeded anyway, nowhere qualifies)" } else { "" }
            );
        }
    }
    out
}

/// The species' establishment predicate at one site, in the terms the plant layer uses.
fn passes(world: &World, sc: &cubarium_voxel_flora::SpeciesConfig, s: Site) -> bool {
    let view = world.view();
    let (x, y, z) = (s.x as i64, s.y, s.z);
    view.soil_below(x, y, z) >= 1
        && view.pore_at(x, y, z) >= sc.establish_pore_min
        && view.water_depth_m(x, y, z) <= sc.drown_depth_m
        && view.sky_visibility(x, y, z) >= sc.establish_light_min
}

/// How many of another run's founder columns this world no longer qualifies.
fn off_count(world: &World, flora: &Flora, founders: &[Founder]) -> usize {
    founders
        .iter()
        .filter(|f| {
            let sc = flora.config().species(f.species);
            match cubarium_voxel_flora::highest_support(&world.view(), f.x as i64, f.z) {
                None => true,
                Some(site) => !passes(world, sc, site),
            }
        })
        .count()
}

#[allow(clippy::too_many_arguments)]
fn report(
    world: &World,
    flora: &Flora,
    config: &VoxelConfig,
    skyline: &[Site],
    founders: usize,
    ticks: u64,
    seconds: f64,
    basin_floor_m: f64,
) {
    let view = flora.view();
    println!(
        "two_producers: {}x{}x{} seed {} noise_seed {}, rain {} m/s, outlet open",
        config.width, config.height, config.depth, config.seed, config.noise_seed,
        config.rain_m_per_s
    );
    println!(
        "water table charged to {:.2} m (basin floor {:.2} m + 1 m), now at {:.2} m",
        config.initial_aquifer_head_m,
        basin_floor_m,
        world.aquifer_head_m()
    );
    println!(
        "{WARMUP_TICKS} warm-up ticks, then {ticks} coupled ticks ({seconds:.0} s); {founders} founders at wood {FOUNDER_WOOD}"
    );

    for species in Species::ALL {
        let all: Vec<_> = view.stands.iter().filter(|s| s.species == species).collect();
        let alive = all.iter().filter(|s| s.stage == Stage::Alive).count();
        let wood: f64 = all.iter().map(|s| s.wood).sum();
        // Only the alive ones have read light and water: an establishing stand is frozen
        // and its two factors stay at zero.
        let live = all.iter().filter(|s| s.stage == Stage::Alive);
        let light: f64 = live.clone().map(|s| s.light).sum::<f64>() / alive.max(1) as f64;
        let moisture: f64 = live.map(|s| s.moisture).sum::<f64>() / alive.max(1) as f64;
        println!(
            "{:>14}: {} stands ({alive} alive, {} establishing), wood {wood:.4}, alive mean light {light:.3}, moisture {moisture:.3}",
            species.name(),
            all.len(),
            all.len() - alive
        );
    }

    // Quartiles of the terrain's own skyline, so occupancy is read against the shape of
    // the world rather than against where the stands happen to be.
    let bounds: Vec<u32> = (1..4).map(|q| skyline[skyline.len() * q / 4].y).collect();
    println!(
        "occupancy by skyline quartile (breaks at y = {}, {}, {}):",
        bounds[0], bounds[1], bounds[2]
    );
    for q in 0..4 {
        let lo = if q == 0 { 0 } else { bounds[q - 1] };
        let hi = if q == 3 { u32::MAX } else { bounds[q] };
        let columns = skyline.iter().filter(|s| s.y >= lo && s.y < hi).count();
        let counts: Vec<String> = Species::ALL
            .iter()
            .map(|&species| {
                let n = view
                    .stands
                    .iter()
                    .filter(|s| s.species == species && s.site.y >= lo && s.site.y < hi)
                    .count();
                format!("{} {n}", species.name())
            })
            .collect();
        println!("  q{} y {lo}..{}: {} columns, {}", q + 1, hi.min(999), columns, counts.join(", "));
    }

    let l = view.ledger;
    println!(
        "ledger: light_in {:.6} heat_out {:.6} transpired {:.6} m3 deaths {} establishments {}",
        l.light_in, l.heat_out, l.transpired_m3, l.deaths, l.establishments
    );
    println!(
        "residuals: material {:.3e} energy {:.3e} (stocks: material {:.4} energy {:.4})",
        view.material() - l.expected_material(),
        view.energy() - l.expected_energy(),
        view.material(),
        view.energy()
    );
    let water = world.view().stored_m3() - world.view().ledger.expected_stored();
    println!(
        "core water: stored {:.4} m3, residual {:.3e}, transpiration_out {:.6} m3 (flora says {:.6})",
        world.view().stored_m3(),
        water,
        world.view().ledger.transpiration_out,
        l.transpired_m3
    );
}

/// The occupied skyline columns themselves, so a run can be compared with another by
/// eye as well as by a number.
fn print_columns(outcome: &Outcome) {
    for species in Species::ALL {
        let all = &outcome.occupied[sp(species)];
        let alive = &outcome.alive[sp(species)];
        println!(
            "{:>14}: {} occupied columns ({} with a living stand), lowest skyline quartile {:.2}",
            species.name(),
            all.len(),
            alive.len(),
            quartile_fraction(all, &outcome.low_quartile)
        );
        println!("    {}", columns(all));
    }
}

/// The set, one entry per `x` with its `z` values collapsed into ranges: the full set,
/// short enough to read and to diff between two runs by eye.
fn columns(set: &[(u32, u32)]) -> String {
    if set.is_empty() {
        return "(none)".to_string();
    }
    let mut out: Vec<String> = Vec::new();
    let mut at = 0;
    while at < set.len() {
        let x = set[at].0;
        let mut end = at;
        while end < set.len() && set[end].0 == x {
            end += 1;
        }
        let zs = &set[at..end];
        let mut parts: Vec<String> = Vec::new();
        let mut i = 0;
        while i < zs.len() {
            let start = zs[i].1;
            let mut last = start;
            while i + 1 < zs.len() && zs[i + 1].1 == last + 1 {
                i += 1;
                last = zs[i].1;
            }
            parts.push(if last == start { format!("{start}") } else { format!("{start}-{last}") });
            i += 1;
        }
        out.push(format!("x{x}:z{}", parts.join(",")));
        at = end;
    }
    out.join(" ")
}

/// `|A ∩ B| / |A ∪ B|`, and `None` for two empty sets — which is not an overlap of one.
fn jaccard(a: &[(u32, u32)], b: &[(u32, u32)]) -> Option<f64> {
    let inter = a.iter().filter(|k| b.binary_search(k).is_ok()).count();
    let union = a.len() + b.len() - inter;
    if union == 0 { None } else { Some(inter as f64 / union as f64) }
}

fn show(v: Option<f64>) -> String {
    v.map_or_else(|| "n/a".to_string(), |x| format!("{x:.3}"))
}

fn quartile_fraction(set: &[(u32, u32)], low: &[(u32, u32)]) -> f64 {
    if set.is_empty() {
        return f64::NAN;
    }
    set.iter().filter(|k| low.binary_search(k).is_ok()).count() as f64 / set.len() as f64
}

/// One species' founder columns, sorted for `jaccard`.
fn founder_columns(founders: &[Founder], species: Species) -> Vec<(u32, u32)> {
    let mut out: Vec<(u32, u32)> =
        founders.iter().filter(|f| f.species == species).map(|f| (f.x, f.z)).collect();
    out.sort_unstable();
    out
}

/// The columns of a set that no founder was planted in: the part of the patch the run
/// itself produced, since the founder columns are identical in all three runs by
/// construction and would otherwise inflate every overlap.
fn spread(set: &[(u32, u32)], founders: &[Founder]) -> Vec<(u32, u32)> {
    set.iter().copied().filter(|&(x, z)| !founders.iter().any(|f| f.x == x && f.z == z)).collect()
}

/// The decisive experiment: the same landform under two noise seeds, against a different
/// landform as a control.
fn compare(args: &[String]) {
    let seconds: f64 = arg(args, 0).unwrap_or(300.0);
    let seed: u64 = arg(args, 1).unwrap_or(1);
    let noise_a: u64 = arg(args, 2).unwrap_or(101);
    let noise_b: u64 = arg(args, 3).unwrap_or(202);
    let control_seed: u64 = arg(args, 4).unwrap_or(7);

    println!("=== base: seed {seed}, noise_seed {noise_a} ===");
    let base = run(seconds, seed, noise_a, None, true);
    println!("\n=== re-drawn noise: seed {seed}, noise_seed {noise_b} (same landform) ===");
    let alt = run(seconds, seed, noise_b, Some(&base.founders), true);
    println!("\n=== control: seed {control_seed}, noise_seed {noise_a} (another landform) ===");
    let ctl = run(seconds, control_seed, noise_a, Some(&base.founders), true);

    println!("\n=== the comparison ===");
    println!(
        "seconds {seconds:.0}; base seed {} noise {}; re-drawn noise {}; control seed {} \
         (founder columns identical in all three: {} of them, off-predicate {} in the \
         re-draw and {} in the control)",
        base.seed, base.noise_seed, alt.noise_seed, ctl.seed,
        base.founders.len(), alt.off_predicate, ctl.off_predicate
    );
    // The landform itself, before any plant or any drop of water: if the ridges and the
    // hollows do not stay put under a re-drawn noise, nothing that follows them can.
    println!(
        "landform: the skyline's lowest quartile overlaps {} with the re-drawn noise and {} \
         with the control; its highest quartile {} and {}",
        show(jaccard(&base.low_quartile, &alt.low_quartile)),
        show(jaccard(&base.low_quartile, &ctl.low_quartile)),
        show(jaccard(&base.high_quartile, &alt.high_quartile)),
        show(jaccard(&base.high_quartile, &ctl.high_quartile))
    );
    for species in Species::ALL {
        let i = sp(species);
        let (b, a, c) = (&base.occupied[i], &alt.occupied[i], &ctl.occupied[i]);
        println!("{}:", species.name());
        println!(
            "  occupied columns: base {}, re-drawn noise {}, control {}",
            b.len(),
            a.len(),
            c.len()
        );
        println!(
            "  Jaccard base vs re-drawn noise {}   base vs control {}",
            show(jaccard(b, a)),
            show(jaccard(b, c))
        );
        let (bs, as_, cs) =
            (spread(b, &base.founders), spread(a, &base.founders), spread(c, &base.founders));
        println!(
            "  founders excluded ({} / {} / {} columns): noise {}   control {}",
            bs.len(),
            as_.len(),
            cs.len(),
            show(jaccard(&bs, &as_)),
            show(jaccard(&bs, &cs))
        );
        let (bl, al, cl) = (&base.alive[i], &alt.alive[i], &ctl.alive[i]);
        println!(
            "  living stands only ({} / {} / {} columns): noise {}   control {}",
            bl.len(),
            al.len(),
            cl.len(),
            show(jaccard(bl, al)),
            show(jaccard(bl, cl))
        );
        println!(
            "  in the lowest skyline quartile: base {:.2}, re-drawn noise {:.2}, control {:.2}",
            quartile_fraction(b, &base.low_quartile),
            quartile_fraction(a, &alt.low_quartile),
            quartile_fraction(c, &ctl.low_quartile)
        );
        // The patch is bounded by founders held identical on purpose, so the habitat is
        // the set that answers the sketch's question without them in the way.
        let (be, ae, ce) = (&base.eligible[i], &alt.eligible[i], &ctl.eligible[i]);
        println!(
            "  habitat, the establishment predicate alone ({} / {} / {} of {} columns): \
             noise {}   control {}",
            be.len(),
            ae.len(),
            ce.len(),
            base.low_quartile.len() * 4,
            show(jaccard(be, ae)),
            show(jaccard(be, ce))
        );
        println!(
            "  habitat in the lowest skyline quartile: base {:.2}, re-drawn noise {:.2}, control {:.2}",
            quartile_fraction(be, &base.low_quartile),
            quartile_fraction(ae, &alt.low_quartile),
            quartile_fraction(ce, &ctl.low_quartile)
        );
        // Where the selection rule would have put this species if each run had chosen
        // freely. It comes out near zero for both the noise pair and the control, and
        // that is a property of the *rule*, not of the terrain: it takes a strided
        // sample of the eligible sites sorted by height, and a one-voxel wobble reorders
        // near-ties, so the sample lands elsewhere while the set it samples barely moves
        // (compare the habitat line above). It is printed because it is the reason the
        // founders have to be held identical for the comparison to mean anything.
        let (bf, af, cf) = (
            founder_columns(&base.own_founders, species),
            founder_columns(&alt.own_founders, species),
            founder_columns(&ctl.own_founders, species),
        );
        println!(
            "  the strided founder sample its own rule would pick ({} columns, order-sensitive \
             by construction): noise {}   control {}",
            bf.len(),
            show(jaccard(&bf, &af)),
            show(jaccard(&bf, &cf))
        );
    }
}
