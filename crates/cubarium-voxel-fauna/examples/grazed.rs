//! The grazed meadow: one conditioned springturf-plus-bloomcrown world, run twice —
//! plant-only, and with `n` frondgrazers introduced at the halfway point.
//!
//! ```text
//! cargo run --release -p cubarium-voxel-fauna --example grazed -- grazed [seconds] [n] [seed] [noise_seed]
//! ```
//!
//! **What this is.** A probe that every quantity an animal spends is paid and observable:
//! bodies, reserves, bites, steps, births, deaths, and the two ledgers closing against each
//! other. It is **not** a study. There is no replication (one seed, one landform), no dose
//! series (one `n`), no control over introduction time, and **no viability or carrying-
//! capacity claim anywhere** — Astra's round 7 left population targets and carrying
//! capacity unclaimed and this harness does not take them.
//!
//! **How to read the two arms.** They are separate `World` and `Flora` instances built from
//! the same seed and given the same founders, which the run asserts rather than assumes. So
//! an arm-to-arm difference is a **treatment effect through within-arm mediators** — the
//! grazed arm's own water table, shade field and germination lotteries all move once foliage
//! is taken — and not a coupling between the arms. That is also why a species no grazer ever
//! bit can differ between them.
//!
//! Every condition below is **declared** and printed with the run: the rain, the founder
//! rule, the grazer placement rule, the introduction time and the counts. None of them is a
//! model rule, and the animal's own numbers are the untuned placeholders of
//! `SpeciesConfig::frondgrazer` (`design/backlog.md` §1).

use std::time::Instant;

use cubarium_voxel::{Command as WorldCommand, Config as VoxelConfig, Material, World};
use cubarium_voxel_fauna::{
    Command as FaunaCommand, DT, Fauna, FaunaConfig, Species as Beast, State, TICK_HZ,
};
use cubarium_voxel_flora::{
    Command as FloraCommand, Flora, FloraConfig, Site, Species, can_establish,
};

/// Ticks of world-only warm-up before the founders land: `two_producers.rs`'s condition,
/// so the two harnesses condition a world the same way.
const WARMUP_TICKS: u32 = 1000;
/// Rain onto exposed top faces, metres per second. `two_producers.rs`'s number and for its
/// reason: under the outlet's export capacity, so the input is bounded below the exit.
const HARNESS_RAIN_M_PER_S: f64 = 0.0002;
/// A founder starts at this fraction of its own species' `wood_max`, which is exactly
/// `donor_min` for all five presets. `two_producers.rs`'s condition.
const FOUNDER_FRACTION: f64 = 0.5;
const FOUNDERS_PER_SPECIES: usize = 8;
/// The two producers of this meadow: the pioneer turf a ground browser can actually reach,
/// and the tall one it mostly cannot.
const PATCH: [Species; 2] = [Species::Springturf, Species::Bloomcrown];

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let rest: Vec<String> = match args.first().map(String::as_str) {
        Some("grazed") => args[1..].to_vec(),
        _ => args.clone(),
    };
    let seconds: f64 = arg(&rest, 0).unwrap_or(400.0);
    let grazers: usize = arg(&rest, 1).unwrap_or(4);
    let seed: u64 = arg(&rest, 2).unwrap_or(1);
    let noise_seed: u64 = arg(&rest, 3).unwrap_or(0);
    let started = Instant::now();

    println!("# grazed: {seconds} s, {grazers} grazers, seed {seed}, noise_seed {noise_seed}");
    println!(
        "conditions (declared, not model rules): rain {HARNESS_RAIN_M_PER_S} m/s with the \
         outlet open, {WARMUP_TICKS} warm-up ticks, {FOUNDERS_PER_SPECIES} founders per \
         species at {FOUNDER_FRACTION} of its own wood_max, springturf and bloomcrown only"
    );
    println!(
        "founder rule: the gate-passing skyline sites of that species, in skyline order \
         (low to high), sampled by a fixed stride — this harness's own rule, not \
         two_producers.rs's Habitat table"
    );
    println!(
        "grazer rule: introduced at the halfway point on gate-passing **open-soil** sites \
         (support face is Material::Soil and the site passes springturf's own establishment \
         predicate), by the same stride; body = body_max, reserve full"
    );

    let (plain, founders_a) = arm(seconds, 0, seed, noise_seed);
    let (grazed, founders_b) = arm(seconds, grazers, seed, noise_seed);
    assert_eq!(founders_a, founders_b, "the two arms must be the same conditioned world");
    println!("\nfounders (identical in both arms): {}", show_founders(&founders_a));

    report(&plain, &grazed, seconds);
    println!("\nwall time: {:.1} s for two arms of {seconds} coupled seconds", started.elapsed().as_secs_f64());
}

fn arg<T: std::str::FromStr>(args: &[String], i: usize) -> Option<T> {
    args.get(i).and_then(|s| s.parse().ok())
}

/// A negative zero printed as one is a distraction: `-0.0 + 0.0` is `+0.0`.
fn nz(v: f64) -> f64 {
    v + 0.0
}

/// Producer stocks of one species at one moment.
#[derive(Clone, Copy, Debug, Default)]
struct Stocks {
    stands: usize,
    foliage: f64,
    reserve: f64,
    wood: f64,
}

/// One sample of an arm: the producers, and the animals.
#[derive(Clone, Copy, Debug, Default)]
struct Sample {
    t: f64,
    stocks: [Stocks; Species::COUNT],
    animals: usize,
    mean_body: f64,
    mean_reserve: f64,
    bites: u64,
    steps: u64,
    born: u64,
    deaths: u64,
    /// The **introduced founders** and their descendants, kept apart by identity (Astra
    /// R9.2): an animal's `id` never changes, and the founders hold the first `n` of them
    /// because an introduction is the first thing this harness creates. A falling
    /// population *mean* is composition and not shrinking individuals — 75 newborns at
    /// `body_min` move it on their own — so a starvation reading needs these rows and not
    /// that mean.
    founders: Cohort,
    descendants: Cohort,
}

/// One cohort of animals at one moment: how many, and the stocks they hold.
#[derive(Clone, Copy, Debug, Default)]
struct Cohort {
    animals: usize,
    body: f64,
    reserve: f64,
}

impl Cohort {
    fn mean_body(&self) -> f64 {
        self.body / self.animals.max(1) as f64
    }

    fn mean_reserve(&self) -> f64 {
        self.reserve / self.animals.max(1) as f64
    }
}

/// Everything one arm leaves behind.
struct Arm {
    label: &'static str,
    grazers: usize,
    introduced_at: f64,
    grazer_sites: Vec<Site>,
    samples: Vec<Sample>,
    /// Bites and organic matter by the plant species they came off.
    bites_by_plant: [u64; Species::COUNT],
    eaten_by_plant: [f64; Species::COUNT],
    /// `(organic, mineral, energy)` the animals took, and handed back.
    eaten: (f64, f64, f64),
    deposited: (f64, f64, f64),
    /// The plant layer's own view of the same two transfers.
    consumed: (f64, f64, f64),
    received: (f64, f64, f64),
    plant_establishments: u64,
    plant_deaths: u64,
    flora_residuals: (f64, f64, f64),
    fauna_residuals: (f64, f64, f64),
    flora_stocks: (f64, f64, f64),
    fauna_stocks: (f64, f64, f64),
    /// Where the cropping happened, and how high the food was above it.
    bites: BiteSites,
}

/// **Where the bites were taken, and from how high (Astra R9.5).** Round 5c measured that
/// 87 % of the intake came off bloomcrown, which a ground browser can only eat from a face
/// above the stand's own — and recorded nothing about *which* faces did it, so the
/// attribution was an inference. This is the record that makes it a measurement.
///
/// The unit is a **cropping animal-tick**, not a bite: one mouthful is spread over every
/// stand in reach in site order and the ledger counts bites by plant species, so splitting
/// a mouthful between two stands of different species would mean reproducing the model's
/// own spending order here. What is recorded instead is the face each animal cropped from
/// and the stands that were in reach **from that face, before it ate** — which is exactly
/// the set the mouthful was spent over. Per-bite attribution would need a field in
/// `FaunaLedger`, and that is a snapshot schema change.
#[derive(Default)]
struct BiteSites {
    /// Cropping animal-ticks by the height of the face the animal stood on.
    by_face_y: Vec<(u32, u64)>,
    /// Every distinct face cropped from, sorted.
    faces: Vec<Site>,
    /// Cropping animal-ticks by plant species and by `stand.y - face.y`: negative is food
    /// below the eater, zero is level with it, positive is above.
    by_dy: Vec<(Species, i64, u64)>,
}

impl BiteSites {
    /// One cropping animal-tick on `face`, with the stands that were in reach of it.
    fn record(&mut self, face: Site, reach: &[(Site, f64)], flora: &Flora) {
        match self.by_face_y.binary_search_by_key(&face.y, |&(y, _)| y) {
            Ok(i) => self.by_face_y[i].1 += 1,
            Err(i) => self.by_face_y.insert(i, (face.y, 1)),
        }
        if let Err(i) = self.faces.binary_search(&face) {
            self.faces.insert(i, face);
        }
        for &(site, _) in reach {
            let Some(stand) = flora.view().stand_at(site) else { continue };
            let key = (stand.species, i64::from(site.y) - i64::from(face.y));
            match self.by_dy.binary_search_by_key(&key, |&(sp, dy, _)| (sp, dy)) {
                Ok(i) => self.by_dy[i].2 += 1,
                Err(i) => self.by_dy.insert(i, (key.0, key.1, 1)),
            }
        }
    }
}

/// `two_producers.rs`'s `prepared_world`: generate once to find the basin floor, then
/// generate the world the run uses with the table charged a metre above it, open the
/// outlet and warm it up. Copied rather than shared, because the flora crate's harness is
/// an example and not a library.
fn prepared_world(seed: u64, noise_seed: u64) -> World {
    let dry = VoxelConfig {
        seed,
        noise_seed,
        rain_m_per_s: HARNESS_RAIN_M_PER_S,
        ..VoxelConfig::default()
    };
    let basin_floor_m =
        World::new(dry.clone()).outlet_cell().map_or(0.0, |(_, y, _)| y as f64) * dry.voxel_m;
    let config = VoxelConfig { initial_aquifer_head_m: basin_floor_m + 1.0, ..dry };
    let mut world = World::new(config);
    world.apply(WorldCommand::SetOutlet { open: true });
    for _ in 0..WARMUP_TICKS {
        world.step();
    }
    world
}

/// Every column's highest support face, sorted low to high: hollows first, ridges last.
fn skyline_of(world: &World) -> Vec<Site> {
    let (width, depth) = (world.config().width, world.config().depth);
    let mut skyline: Vec<Site> = Vec::new();
    for z in 0..depth {
        for x in 0..width as i64 {
            if let Some(site) = cubarium_voxel_flora::highest_support(&world.view(), x, z) {
                skyline.push(site);
            }
        }
    }
    skyline.sort_by_key(|s| (s.y, s.x, s.z));
    skyline
}

/// A fixed stride sample of `want` items out of `pool`, in the pool's own order: the whole
/// pool spread over, and never the same end of it twice.
fn strided<T: Copy>(pool: &[T], want: usize) -> Vec<T> {
    if pool.is_empty() || want == 0 {
        return Vec::new();
    }
    let stride = (pool.len() / want).max(1);
    pool.iter().step_by(stride).take(want).copied().collect()
}

/// The founders, by the declared rule: each species' own gate-passing skyline sites, in
/// skyline order, strided — and never a site another founder already has.
fn pick_founders(world: &World, flora: &Flora, skyline: &[Site]) -> Vec<(Site, Species)> {
    let view = world.view();
    let mut taken: Vec<Site> = Vec::new();
    let mut out: Vec<(Site, Species)> = Vec::new();
    for species in PATCH {
        let sc = flora.config().species(species);
        let pool: Vec<Site> = skyline
            .iter()
            .copied()
            .filter(|&s| !taken.contains(&s) && can_establish(&view, s, sc))
            .collect();
        for site in strided(&pool, FOUNDERS_PER_SPECIES) {
            taken.push(site);
            out.push((site, species));
        }
    }
    out.sort_by_key(|&(site, sp)| (site, sp));
    out
}

/// The grazer faces, by the declared rule: gate-passing **open soil**, strided.
fn grazer_sites(world: &World, flora: &Flora, skyline: &[Site], n: usize) -> Vec<Site> {
    let view = world.view();
    let sc = flora.config().species(Species::Springturf);
    let pool: Vec<Site> = skyline
        .iter()
        .copied()
        .filter(|&s| {
            view.material_at(i64::from(s.x), s.y, s.z) == Material::Soil
                && can_establish(&view, s, sc)
        })
        .collect();
    let mut out = strided(&pool, n);
    out.sort_unstable();
    out
}

fn stocks_of(flora: &Flora) -> [Stocks; Species::COUNT] {
    let mut out = [Stocks::default(); Species::COUNT];
    for s in flora.view().stands {
        let e = &mut out[s.species.index()];
        e.stands += 1;
        e.foliage += s.foliage;
        e.reserve += s.reserve;
        e.wood += s.wood;
    }
    out
}

/// One sample. `founders` is how many animals were introduced, which is also the number of
/// ids the founders hold: `0..founders`.
fn sample(t: f64, flora: &Flora, fauna: &Fauna, founders: u64) -> Sample {
    let av = fauna.view();
    let n = av.animals.len();
    let d = n.max(1) as f64;
    let cohort = |of: fn(u64, u64) -> bool| {
        av.animals.iter().filter(|a| of(a.id, founders)).fold(Cohort::default(), |mut c, a| {
            c.animals += 1;
            c.body += a.body;
            c.reserve += a.reserve;
            c
        })
    };
    Sample {
        t,
        stocks: stocks_of(flora),
        animals: n,
        mean_body: av.animals.iter().map(|a| a.body).sum::<f64>() / d,
        mean_reserve: av.animals.iter().map(|a| a.reserve).sum::<f64>() / d,
        bites: av.ledger.bites,
        steps: av.ledger.steps,
        born: av.ledger.born,
        deaths: av.ledger.deaths,
        founders: cohort(|id, n| id < n),
        descendants: cohort(|id, n| id >= n),
    }
}

/// One arm. `grazers` of 0 is the plant-only control; the two arms are otherwise the same
/// world, the same founders and the same rain.
fn arm(seconds: f64, grazers: usize, seed: u64, noise_seed: u64) -> (Arm, Vec<(Site, Species)>) {
    let mut world = prepared_world(seed, noise_seed);
    let skyline = skyline_of(&world);
    let mut flora = Flora::new(FloraConfig::default());
    let founders = pick_founders(&world, &flora, &skyline);
    for &(site, species) in &founders {
        let wood = FOUNDER_FRACTION * flora.config().species(species).wood_max;
        flora.apply(
            &world,
            FloraCommand::Seed { x: i64::from(site.x), z: site.z, species, wood },
        );
    }
    let mut fauna = Fauna::new(FaunaConfig::default());
    let sites = grazer_sites(&world, &flora, &skyline, grazers);

    let ticks = (seconds * f64::from(TICK_HZ)).round() as u64;
    let half = ticks / 2;
    let every = 100 * u64::from(TICK_HZ);
    let mut samples = vec![sample(0.0, &flora, &fauna, grazers as u64)];
    let mut bites = BiteSites::default();
    let reach = fauna.config().species(Beast::Frondgrazer).reach;

    for tick in 0..ticks {
        world.step();
        flora.step(&mut world);
        // What each animal could reach from where it stands, read **before** it eats: the
        // set a mouthful is spent over, and the only moment it can be read.
        let before: Vec<(u64, Site, Vec<(Site, f64)>)> = fauna
            .view()
            .animals
            .iter()
            .map(|a| (a.id, a.site, flora.view().reachable_foliage(&world.view(), a.site, reach)))
            .collect();
        fauna.step(&world, &mut flora);
        for (id, site, reached) in &before {
            // An animal that cropped did not move, so the face it stands on now is the one
            // the pre-bite reach was read from.
            if fauna.view().animal(*id).map(|a| a.state) == Some(State::Cropping) {
                bites.record(*site, reached, &flora);
            }
        }
        if tick + 1 == half {
            // The introduction is **between** ticks, like every other command in this
            // world: the tick that follows is the first one the animals act in.
            for &site in &sites {
                let body = fauna.config().species(Beast::Frondgrazer).body_max;
                let ok = fauna.apply(
                    &world,
                    FaunaCommand::Introduce {
                        x: i64::from(site.x),
                        z: site.z,
                        species: Beast::Frondgrazer,
                        body,
                    },
                );
                assert!(ok, "a grazer on a declared open-soil face {site:?}");
            }
        }
        if (tick + 1) % every == 0 {
            samples.push(sample((tick + 1) as f64 * DT, &flora, &fauna, grazers as u64));
        }
    }
    if samples.last().map(|s| s.t) != Some(ticks as f64 * DT) {
        samples.push(sample(ticks as f64 * DT, &flora, &fauna, grazers as u64));
    }

    let fv = flora.view();
    let av = fauna.view();
    let arm = Arm {
        label: if grazers == 0 { "plant-only" } else { "grazed" },
        grazers,
        introduced_at: half as f64 * DT,
        grazer_sites: sites,
        samples,
        bites_by_plant: av.ledger.bites_by_plant,
        eaten_by_plant: av.ledger.eaten_by_plant,
        eaten: (
            av.ledger.eaten_organic_in,
            av.ledger.eaten_mineral_in,
            av.ledger.eaten_energy_in,
        ),
        deposited: (
            av.ledger.deposited_organic_out,
            av.ledger.deposited_mineral_out,
            av.ledger.deposited_energy_out,
        ),
        consumed: (
            fv.ledger.consumed_organic_out,
            fv.ledger.consumed_mineral_out,
            fv.ledger.consumed_energy_out,
        ),
        received: (
            fv.ledger.deposited_organic_in,
            fv.ledger.deposited_mineral_in,
            fv.ledger.deposited_energy_in,
        ),
        plant_establishments: fv.ledger.establishments,
        plant_deaths: fv.ledger.deaths,
        flora_residuals: (
            fv.organic() - fv.ledger.expected_organic(),
            fv.mineral() - fv.ledger.expected_mineral(),
            fv.energy() - fv.ledger.expected_energy(),
        ),
        fauna_residuals: (
            av.organic() - av.ledger.expected_organic(),
            av.mineral() - av.ledger.expected_mineral(),
            av.energy() - av.ledger.expected_energy(),
        ),
        flora_stocks: (fv.organic(), fv.mineral(), fv.energy()),
        fauna_stocks: (av.organic(), av.mineral(), av.energy()),
        bites,
    };
    (arm, founders)
}

fn show_founders(founders: &[(Site, Species)]) -> String {
    let mut parts: Vec<String> = Vec::new();
    for species in PATCH {
        let n = founders.iter().filter(|&&(_, s)| s == species).count();
        let where_: Vec<String> = founders
            .iter()
            .filter(|&&(_, s)| s == species)
            .map(|&(site, _)| format!("({},{})y{}", site.x, site.z, site.y))
            .collect();
        parts.push(format!("{} x{n} at {}", species.name(), where_.join(" ")));
    }
    parts.join("; ")
}

fn report(plain: &Arm, grazed: &Arm, seconds: f64) {
    println!(
        "\ngrazers: {} introduced at {:.0} s on {}",
        grazed.grazers,
        grazed.introduced_at,
        grazed
            .grazer_sites
            .iter()
            .map(|s| format!("({},{})y{}", s.x, s.z, s.y))
            .collect::<Vec<_>>()
            .join(" ")
    );

    println!("\n## the producers, both arms (stands / foliage / reserve / wood)\n");
    println!("| t (s) | arm | {} |", PATCH.iter().map(|s| s.name()).collect::<Vec<_>>().join(" | "));
    println!("| --- | --- | {} |", PATCH.iter().map(|_| "---").collect::<Vec<_>>().join(" | "));
    for arm in [plain, grazed] {
        for s in &arm.samples {
            let cells: Vec<String> = PATCH
                .iter()
                .map(|sp| {
                    let e = s.stocks[sp.index()];
                    format!("{} / {:.4} / {:.4} / {:.4}", e.stands, e.foliage, e.reserve, e.wood)
                })
                .collect();
            println!("| {:.0} | {} | {} |", s.t, arm.label, cells.join(" | "));
        }
    }

    println!("\n## the animals, grazed arm\n");
    println!("| t (s) | animals | mean body | mean reserve | bites | steps | born | deaths |");
    println!("| --- | --- | --- | --- | --- | --- | --- | --- |");
    for s in &grazed.samples {
        // No animals is no mean, and a mean of nothing printed as 0.00000 reads as a
        // measurement of a body that is not there.
        let mean = |v: f64| if s.animals == 0 { "—".to_string() } else { format!("{:.5}", nz(v)) };
        println!(
            "| {:.0} | {} | {} | {} | {} | {} | {} | {} |",
            s.t,
            s.animals,
            mean(s.mean_body),
            mean(s.mean_reserve),
            s.bites,
            s.steps,
            s.born,
            s.deaths
        );
    }

    println!("\n## the animals by identity: the introduced founders and their descendants\n");
    println!(
        "| t (s) | founders | founder mean body | founder mean reserve | descendants | \
         descendant mean body | descendant mean reserve |"
    );
    println!("| --- | --- | --- | --- | --- | --- | --- |");
    for s in &grazed.samples {
        let cell = |c: &Cohort, v: f64| {
            if c.animals == 0 { "—".to_string() } else { format!("{:.5}", nz(v)) }
        };
        println!(
            "| {:.0} | {} | {} | {} | {} | {} | {} |",
            s.t,
            s.founders.animals,
            cell(&s.founders, s.founders.mean_body()),
            cell(&s.founders, s.founders.mean_reserve()),
            s.descendants.animals,
            cell(&s.descendants, s.descendants.mean_body()),
            cell(&s.descendants, s.descendants.mean_reserve()),
        );
    }
    let config = FaunaConfig::default();
    let sc = config.species(Beast::Frondgrazer);
    println!(
        "\n(an id never changes, so these are the same animals at every row. A full default \
         adult's reserve is {:.4} against an upkeep of {:.1e} /s, which is {:.0} s of \
         standing still — and about {:.0} s once it has paid for two newborns at \
         {:.3} each. A falling population mean is composition before it is starvation.)",
        sc.reserve_of(sc.body_max),
        sc.maintenance_per_s * sc.body_max,
        sc.reserve_of(sc.body_max) / (sc.maintenance_per_s * sc.body_max),
        (sc.reserve_of(sc.body_max) - 2.0 * sc.birth_cost) / (sc.maintenance_per_s * sc.body_max),
        sc.birth_cost,
    );

    println!("\n## what was eaten, by the species it came off\n");
    println!("| species | bites | organic |");
    println!("| --- | --- | --- |");
    for sp in Species::ALL {
        let (b, o) = (grazed.bites_by_plant[sp.index()], grazed.eaten_by_plant[sp.index()]);
        if b > 0 {
            println!("| {} | {b} | {o:.6} |", sp.name());
        }
    }
    println!(
        "\n(a reach box does not choose a species: which stands a browser can eat depends on \
         the face it stands on, so this table is a measurement and not a property of any \
         preset.)"
    );

    println!("\n## where it was cropped from (cropping animal-ticks)\n");
    println!("| eater's face y | animal-ticks | distinct faces at this height |");
    println!("| --- | --- | --- |");
    for &(y, n) in &grazed.bites.by_face_y {
        let faces = grazed.bites.faces.iter().filter(|f| f.y == y).count();
        println!("| {y} | {n} | {faces} |");
    }
    println!("\n| species in reach | stand y - face y | animal-ticks |");
    println!("| --- | --- | --- |");
    for &(sp, dy, n) in &grazed.bites.by_dy {
        println!("| {} | {dy:+} | {n} |", sp.name());
    }
    println!(
        "\n(a cropping animal-tick, not a bite: one mouthful is spent over every stand in \
         reach in site order, so the faces and the reach sets are recorded and the split \
         between two stands is not. `stand y - face y` is what a higher face buys: a \
         positive row is food above the eater, which is the only way a grown bloomcrown is \
         food at all.)"
    );

    println!("\n## the two ledgers, and the residuals\n");
    for arm in [plain, grazed] {
        println!(
            "{}: eaten {:.6} / {:.7} / {:.6}, deposited {:.6} / {:.7} / {:.6}",
            arm.label, arm.eaten.0, arm.eaten.1, arm.eaten.2, arm.deposited.0, arm.deposited.1,
            arm.deposited.2
        );
        println!(
            "  union: consumed - eaten = {:.3e} / {:.3e} / {:.3e}; received - deposited = \
             {:.3e} / {:.3e} / {:.3e}",
            arm.consumed.0 - arm.eaten.0,
            arm.consumed.1 - arm.eaten.1,
            arm.consumed.2 - arm.eaten.2,
            arm.received.0 - arm.deposited.0,
            arm.received.1 - arm.deposited.1,
            arm.received.2 - arm.deposited.2,
        );
        println!(
            "  flora residual {:.3e} of {:.4} / {:.3e} of {:.4} / {:.3e} of {:.4}",
            nz(arm.flora_residuals.0), arm.flora_stocks.0, nz(arm.flora_residuals.1),
            arm.flora_stocks.1, nz(arm.flora_residuals.2), arm.flora_stocks.2
        );
        println!(
            "  fauna residual {:.3e} of {:.6} / {:.3e} of {:.7} / {:.3e} of {:.6}",
            nz(arm.fauna_residuals.0), nz(arm.fauna_stocks.0), nz(arm.fauna_residuals.1),
            nz(arm.fauna_stocks.1), nz(arm.fauna_residuals.2), nz(arm.fauna_stocks.2)
        );
        println!(
            "  plants: {} establishments, {} deaths",
            arm.plant_establishments, arm.plant_deaths
        );
    }
    println!(
        "\nBoth arms ran {seconds} coupled seconds from the same seed as separate World and \
         Flora instances, so any difference between them is the treatment reaching the \
         producers through this arm's own water, shade and lotteries."
    );
}
