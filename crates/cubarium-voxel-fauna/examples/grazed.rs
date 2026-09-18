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
    assert_eq!(
        founders_a, founders_b,
        "the two arms must be the same conditioned world"
    );
    println!(
        "\nfounders (identical in both arms): {}",
        show_founders(&founders_a)
    );

    report(&plain, &grazed, seconds);
    println!(
        "\nwall time: {:.1} s for two arms of {seconds} coupled seconds",
        started.elapsed().as_secs_f64()
    );
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
    /// Where the cropping was attempted, and what was in reach of it.
    attempts: CropAttempts,
    /// What actually left a stand, attributed to the face it was taken from.
    receipts: Receipts,
}

/// **Crop-attempt and reachable-stand observations (Astra R9.5, corrected by R10.5).**
/// Round 5c measured that 87 % of the intake came off bloomcrown, which a ground browser
/// can only eat from a face above the stand's own, and recorded nothing about *which*
/// faces did it. This is the first half of that record, and its name is what it measures.
///
/// The unit is a **crop-attempt animal-tick**: an animal whose state after the tick is
/// `State::Cropping` had a whole bite in reach when it was planned, but the withdrawal it
/// then made can come back empty if an earlier animal in id order emptied the same stand
/// first, so this is attempts and reach and **not** intake. The `(species, height)` rows
/// are deduplicated per animal-tick — two stands of the same species at the same height in
/// one reach set are one observation, not two, which is what R10.5 found double-counted.
/// Intake is [`Receipts`], keyed on withdrawals that actually returned something.
#[derive(Debug, Default)]
struct CropAttempts {
    /// Crop-attempt animal-ticks by the height of the face the animal stood on.
    by_face_y: Vec<(u32, u64)>,
    /// Every distinct face a crop was attempted from, sorted.
    faces: Vec<Site>,
    /// Crop-attempt animal-ticks by plant species and by `stand.y - face.y`: negative is
    /// food below the eater, zero level with it, positive above. One per animal-tick per
    /// distinct `(species, dy)`, however many stands in reach share it.
    by_dy: Vec<(Species, i64, u64)>,
}

impl CropAttempts {
    /// One crop-attempt animal-tick on `face`, with the stands that were in reach of it.
    fn record(&mut self, face: Site, reach: &[(Site, Species)]) {
        match self.by_face_y.binary_search_by_key(&face.y, |&(y, _)| y) {
            Ok(i) => self.by_face_y[i].1 += 1,
            Err(i) => self.by_face_y.insert(i, (face.y, 1)),
        }
        if let Err(i) = self.faces.binary_search(&face) {
            self.faces.insert(i, face);
        }
        // Deduplicated: the observation is "this species, this far up, was in reach",
        // which two stands cannot make twice in one animal-tick.
        let mut seen: Vec<(Species, i64)> = Vec::new();
        for &(site, species) in reach {
            let key = (species, i64::from(site.y) - i64::from(face.y));
            if let Err(i) = seen.binary_search(&key) {
                seen.insert(i, key);
            }
        }
        for key in seen {
            match self
                .by_dy
                .binary_search_by_key(&key, |&(sp, dy, _)| (sp, dy))
            {
                Ok(i) => self.by_dy[i].2 += 1,
                Err(i) => self.by_dy.insert(i, (key.0, key.1, 1)),
            }
        }
    }
}

/// **Actual intake, from successful withdrawal receipts (Astra R10.5).** What a stand lost
/// in a tick is exactly what the animals took out of it — the plant layer has already
/// grown by the time they eat — so the foliage a stand loses is a receipt, and one with no
/// organic matter in it is not recorded at all.
///
/// Attribution is **exact where it is unambiguous and refused where it is not**: a stand
/// that exactly one cropping animal could reach that tick lost what that animal took, so
/// the loss is booked to that animal's face; a stand two or more cropping animals could
/// reach is `contested` and booked to no face, because splitting it would mean
/// reproducing `crop`'s own spending order in the harness. The report prints both, so the
/// share the attribution covers is visible rather than assumed.
#[derive(Debug, Default)]
struct Receipts {
    /// Organic matter and receipts by the height of the face it was taken from.
    by_face_y: Vec<(u32, f64, u64)>,
    /// Organic matter by plant species and by `stand.y - face.y`.
    by_dy: Vec<(Species, i64, f64)>,
    /// Attributed to one face, and left unattributed — because more than one eater could
    /// have taken it, or because an eater that could have is no longer there to ask
    /// (Astra R11.4).
    attributed: f64,
    contested: f64,
}

/// Book one tick's withdrawals. `taken` is every stand that lost foliage, with the species
/// it is and the organic matter it lost; `croppers` are the animals whose state after the
/// tick was `Cropping`, with the reach sets read before any of them ate; `gone` are the
/// pre-tick animals that are **no longer in the world** at the end of it.
///
/// **A disappeared eater takes its receipts with it (Astra R11.4).** The animal layer acts
/// before it removes its dead, so a grazer can eat a whole mouthful and then starve or
/// drown in the same tick — and it leaves no state for the harness to read, not even a
/// final `State`. Any stand such an animal could reach is therefore **unattributed**:
/// giving that loss to a survivor standing at a different height would put another
/// animal's intake on the wrong face, which is exactly the reading this table exists for.
/// It is deliberately conservative — an animal the terrain removed in step 1 never ate at
/// all, and the harness cannot tell the two exits apart from outside — and it costs only
/// attribution coverage, which the report prints.
fn attribute(
    receipts: &mut Receipts,
    taken: &[(Site, Species, f64)],
    croppers: &[&Reading],
    gone: &[&Reading],
) {
    for &(site, species, organic) in taken {
        if !(organic > 0.0) {
            continue;
        }
        if gone.iter().any(|g| g.reach.iter().any(|&(s, _)| s == site)) {
            receipts.contested += organic;
            continue;
        }
        let mut reached_by = croppers
            .iter()
            .filter(|c| c.reach.iter().any(|&(s, _)| s == site));
        let Some(one) = reached_by.next() else {
            // Nobody who cropped could reach it: not this harness's to attribute either.
            receipts.contested += organic;
            continue;
        };
        if reached_by.next().is_some() {
            receipts.contested += organic;
            continue;
        }
        let face = one.face;
        receipts.attributed += organic;
        match receipts
            .by_face_y
            .binary_search_by_key(&face.y, |&(y, _, _)| y)
        {
            Ok(i) => {
                receipts.by_face_y[i].1 += organic;
                receipts.by_face_y[i].2 += 1;
            }
            Err(i) => receipts.by_face_y.insert(i, (face.y, organic, 1)),
        }
        let key = (species, i64::from(site.y) - i64::from(face.y));
        match receipts
            .by_dy
            .binary_search_by_key(&key, |&(sp, dy, _)| (sp, dy))
        {
            Ok(i) => receipts.by_dy[i].2 += organic,
            Err(i) => receipts.by_dy.insert(i, (key.0, key.1, organic)),
        }
    }
}

/// One animal's pre-bite reading: which face it stands on and what its species' reach box
/// finds from there, with the species of each stand, read before anything has eaten.
struct Reading {
    id: u64,
    face: Site,
    reach: Vec<(Site, Species)>,
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
    let basin_floor_m = World::new(dry.clone())
        .outlet_cell()
        .map_or(0.0, |(_, y, _)| y as f64)
        * dry.voxel_m;
    let config = VoxelConfig {
        initial_aquifer_head_m: basin_floor_m + 1.0,
        ..dry
    };
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
        av.animals
            .iter()
            .filter(|a| of(a.id, founders))
            .fold(Cohort::default(), |mut c, a| {
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
            FloraCommand::Seed {
                x: i64::from(site.x),
                z: site.z,
                species,
                wood,
            },
        );
    }
    let mut fauna = Fauna::new(FaunaConfig::default());
    let sites = grazer_sites(&world, &flora, &skyline, grazers);

    let ticks = (seconds * f64::from(TICK_HZ)).round() as u64;
    let half = ticks / 2;
    let every = 100 * u64::from(TICK_HZ);
    let mut samples = vec![sample(0.0, &flora, &fauna, grazers as u64)];
    let mut attempts = CropAttempts::default();
    let mut receipts = Receipts::default();
    let reach = fauna.config().species(Beast::Frondgrazer).reach;

    for tick in 0..ticks {
        world.step();
        flora.step(&mut world);
        // What each animal could reach from where it stands, read **before** it eats: the
        // set a mouthful is spent over, and the only moment it can be read.
        let before: Vec<Reading> = fauna
            .view()
            .animals
            .iter()
            .map(|a| Reading {
                id: a.id,
                face: a.site,
                reach: flora
                    .view()
                    .reachable_foliage(&world.view(), a.site, reach)
                    .into_iter()
                    .filter_map(|(site, _)| flora.view().stand_at(site).map(|s| (site, s.species)))
                    .collect(),
            })
            .collect();
        // And what those stands held, so that the foliage they lose in this step is a
        // receipt: nothing but an animal takes foliage between here and the next line.
        let mut held: Vec<(Site, Species, f64)> = Vec::new();
        for r in &before {
            for &(site, species) in &r.reach {
                if let Err(i) = held.binary_search_by_key(&site, |&(s, _, _)| s) {
                    let foliage = flora.view().stand_at(site).map_or(0.0, |s| s.foliage);
                    held.insert(i, (site, species, foliage));
                }
            }
        }
        fauna.step(&world, &mut flora);
        // An animal that cropped did not move, so the face it stands on now is the one the
        // pre-bite reach was read from.
        let croppers: Vec<&Reading> = before
            .iter()
            .filter(|r| fauna.view().animal(r.id).map(|a| a.state) == Some(State::Cropping))
            .collect();
        for r in &croppers {
            attempts.record(r.face, &r.reach);
        }
        // Anything that ate and then left: `step` acts before it removes its dead, so a
        // grazer can take a mouthful and drown in the same tick (R11.4).
        let gone: Vec<&Reading> = before
            .iter()
            .filter(|r| fauna.view().animal(r.id).is_none())
            .collect();
        let taken: Vec<(Site, Species, f64)> = held
            .iter()
            .map(|&(site, species, foliage)| {
                (
                    site,
                    species,
                    foliage - flora.view().stand_at(site).map_or(0.0, |s| s.foliage),
                )
            })
            .collect();
        attribute(&mut receipts, &taken, &croppers, &gone);
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
            samples.push(sample(
                (tick + 1) as f64 * DT,
                &flora,
                &fauna,
                grazers as u64,
            ));
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
        attempts,
        receipts,
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
    println!(
        "| t (s) | arm | {} |",
        PATCH
            .iter()
            .map(|s| s.name())
            .collect::<Vec<_>>()
            .join(" | ")
    );
    println!(
        "| --- | --- | {} |",
        PATCH.iter().map(|_| "---").collect::<Vec<_>>().join(" | ")
    );
    for arm in [plain, grazed] {
        for s in &arm.samples {
            let cells: Vec<String> = PATCH
                .iter()
                .map(|sp| {
                    let e = s.stocks[sp.index()];
                    format!(
                        "{} / {:.4} / {:.4} / {:.4}",
                        e.stands, e.foliage, e.reserve, e.wood
                    )
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
        let mean = |v: f64| {
            if s.animals == 0 {
                "—".to_string()
            } else {
                format!("{:.5}", nz(v))
            }
        };
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
            if c.animals == 0 {
                "—".to_string()
            } else {
                format!("{:.5}", nz(v))
            }
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
        let (b, o) = (
            grazed.bites_by_plant[sp.index()],
            grazed.eaten_by_plant[sp.index()],
        );
        if b > 0 {
            println!("| {} | {b} | {o:.6} |", sp.name());
        }
    }
    println!(
        "\n(a reach box does not choose a species: which stands a browser can eat depends on \
         the face it stands on, so this table is a measurement and not a property of any \
         preset.)"
    );

    println!("\n## crop attempts and what was in reach (crop-attempt animal-ticks)\n");
    println!("| eater's face y | animal-ticks | distinct faces at this height |");
    println!("| --- | --- | --- |");
    for &(y, n) in &grazed.attempts.by_face_y {
        let faces = grazed.attempts.faces.iter().filter(|f| f.y == y).count();
        println!("| {y} | {n} | {faces} |");
    }
    println!("\n| species in reach | stand y - face y | animal-ticks |");
    println!("| --- | --- | --- |");
    for &(sp, dy, n) in &grazed.attempts.by_dy {
        println!("| {} | {dy:+} | {n} |", sp.name());
    }
    println!(
        "\n(**attempts and reach, not intake**: an animal that planned a whole bite is \
         `Cropping` even if the stand was emptied by an earlier animal in id order the \
         same tick. One observation per animal-tick per distinct species and height, \
         however many stands in reach share it. `stand y - face y` is what a higher face \
         buys: a positive row is food above the eater, which is the only way a grown \
         bloomcrown is food at all.)"
    );

    println!("\n## actual intake, from withdrawal receipts (organic matter)\n");
    println!("| eater's face y | receipts | organic |");
    println!("| --- | --- | --- |");
    for &(y, o, n) in &grazed.receipts.by_face_y {
        println!("| {y} | {n} | {:.6} |", nz(o));
    }
    println!("\n| species eaten | stand y - face y | organic |");
    println!("| --- | --- | --- |");
    for &(sp, dy, o) in &grazed.receipts.by_dy {
        println!("| {} | {dy:+} | {:.6} |", sp.name(), nz(o));
    }
    let booked = grazed.receipts.attributed + grazed.receipts.contested;
    println!(
        "\nattributed {:.6} of {:.6} withdrawn ({:.1} %); contested {:.6} on stands more \
         than one cropping animal could reach, or that an animal which left the world this \
         tick could have reached; the animal ledger's own eaten organic is \
         {:.6}, so the receipts account for every unit that left a stand",
        grazed.receipts.attributed,
        booked,
        100.0 * grazed.receipts.attributed / booked.max(f64::MIN_POSITIVE),
        grazed.receipts.contested,
        grazed.eaten.0
    );
    println!(
        "(a receipt is foliage that actually left a stand, so an empty withdrawal is not in \
         here at all. Nothing but an animal takes foliage between the plant step and the \
         animal step, which is why the loss is the receipt; a stand exactly one cropping \
         animal could reach is booked to that animal's face, and one that several could — \
         or that an animal which died or drowned this tick could, having eaten before the \
         layer removed it — is left unattributed rather than split or handed to a survivor \
         at another height.)"
    );

    println!("\n## the two ledgers, and the residuals\n");
    for arm in [plain, grazed] {
        println!(
            "{}: eaten {:.6} / {:.7} / {:.6}, deposited {:.6} / {:.7} / {:.6}",
            arm.label,
            arm.eaten.0,
            arm.eaten.1,
            arm.eaten.2,
            arm.deposited.0,
            arm.deposited.1,
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
            nz(arm.flora_residuals.0),
            arm.flora_stocks.0,
            nz(arm.flora_residuals.1),
            arm.flora_stocks.1,
            nz(arm.flora_residuals.2),
            arm.flora_stocks.2
        );
        println!(
            "  fauna residual {:.3e} of {:.6} / {:.3e} of {:.7} / {:.3e} of {:.6}",
            nz(arm.fauna_residuals.0),
            nz(arm.fauna_stocks.0),
            nz(arm.fauna_residuals.1),
            nz(arm.fauna_stocks.1),
            nz(arm.fauna_residuals.2),
            nz(arm.fauna_stocks.2)
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

#[cfg(test)]
mod tests {
    use super::*;

    fn site(x: u32, y: u32) -> Site {
        Site { x, y, z: 0 }
    }

    fn reading(id: u64, face: Site, reach: &[(Site, Species)]) -> Reading {
        Reading {
            id,
            face,
            reach: reach.to_vec(),
        }
    }

    /// **R10.5, the attempts table.** Two stands of the same species at the same height in
    /// one reach set are **one** observation and not two, and two different heights are
    /// two. Before the repair every reachable stand added one, so a wide patch looked like
    /// several attempts.
    #[test]
    fn a_crop_attempt_counts_each_species_and_height_once() {
        let face = site(4, 2);
        let mut attempts = CropAttempts::default();
        attempts.record(
            face,
            &[
                (site(3, 2), Species::Springturf),
                (site(5, 2), Species::Springturf),
                (site(5, 1), Species::Springturf),
                (site(3, 2), Species::Bloomcrown),
            ],
        );
        assert_eq!(
            attempts.by_face_y,
            vec![(2, 1)],
            "one animal-tick, one face"
        );
        assert_eq!(
            attempts.by_dy,
            vec![
                (Species::Bloomcrown, 0, 1),
                (Species::Springturf, -1, 1),
                (Species::Springturf, 0, 1),
            ],
            "two stands level with the eater are one observation, and one below is another"
        );
    }

    /// **R10.5, the receipts table.** A withdrawal that returned nothing is not a receipt;
    /// a stand exactly one cropping animal could reach is that animal's intake, booked to
    /// the face it stood on; a stand two of them could reach is contested and booked to no
    /// face, so the report can say what share the attribution covers.
    #[test]
    fn a_receipt_is_attributed_only_where_one_eater_could_have_taken_it() {
        let low = reading(
            0,
            site(4, 2),
            &[
                (site(3, 2), Species::Springturf),
                (site(9, 2), Species::Springturf),
            ],
        );
        let high = reading(
            1,
            site(7, 3),
            &[
                (site(6, 2), Species::Bloomcrown),
                (site(9, 2), Species::Springturf),
            ],
        );
        let croppers = [&low, &high];

        let mut receipts = Receipts::default();
        attribute(
            &mut receipts,
            &[
                // One eater each: attributed, and the bloomcrown to the higher face.
                (site(3, 2), Species::Springturf, 1e-4),
                (site(6, 2), Species::Bloomcrown, 2e-4),
                // Both could reach it: contested.
                (site(9, 2), Species::Springturf, 5e-5),
                // An empty withdrawal is not a receipt at all.
                (site(5, 2), Species::Springturf, 0.0),
            ],
            &croppers,
            &[],
        );

        assert_eq!(receipts.by_face_y, vec![(2, 1e-4, 1), (3, 2e-4, 1)]);
        assert_eq!(
            receipts.by_dy,
            vec![
                (Species::Bloomcrown, -1, 2e-4),
                (Species::Springturf, 0, 1e-4)
            ],
            "the bloomcrown was taken from a face one voxel above it"
        );
        assert!(
            (receipts.attributed - 3e-4).abs() < 1e-18,
            "{}",
            receipts.attributed
        );
        assert_eq!(receipts.contested, 5e-5);

        // A stand nobody who cropped could reach is not attributed either.
        let mut orphan = Receipts::default();
        attribute(
            &mut orphan,
            &[(site(20, 2), Species::Springturf, 7e-5)],
            &croppers,
            &[],
        );
        assert_eq!((orphan.attributed, orphan.contested), (0.0, 7e-5));
        assert!(orphan.by_face_y.is_empty());
    }

    /// **R11.4: an animal that ate and then drowned does not hand its intake to a
    /// survivor.** Two grazers on faces of different heights can reach one stand: the
    /// lower one crops it and drowns in the same tick — `step` acts before it removes its
    /// dead — and the harness sees only the survivor, whose face is a voxel higher. That
    /// loss is now **unattributed** rather than booked to the survivor's height, which
    /// would have put one animal's mouthful on another animal's face in the very table
    /// that exists to read heights.
    ///
    /// The control is the same geometry with nobody gone: a survivor that merely rested
    /// does not block the attribution, so the rule keys on *disappearing* and not on
    /// having failed to crop.
    #[test]
    fn a_receipt_an_animal_that_left_the_world_could_have_taken_is_unattributed() {
        let stand = site(5, 2);
        let lower = reading(0, stand, &[(stand, Species::Springturf)]);
        let higher = reading(1, site(6, 3), &[(stand, Species::Springturf)]);
        let taken = [(stand, Species::Springturf, 1e-4)];

        // The lower one ate and drowned; the higher one survived and cropped.
        let mut drowned = Receipts::default();
        attribute(&mut drowned, &taken, &[&higher], &[&lower]);
        assert_eq!((drowned.attributed, drowned.contested), (0.0, 1e-4));
        assert!(
            drowned.by_face_y.is_empty() && drowned.by_dy.is_empty(),
            "nothing may land on the survivor's face: {drowned:?}",
        );

        // The control: the lower one is still there, resting, and the survivor's receipt
        // is its own.
        let mut alive = Receipts::default();
        attribute(&mut alive, &taken, &[&higher], &[]);
        assert_eq!((alive.attributed, alive.contested), (1e-4, 0.0));
        assert_eq!(alive.by_face_y, vec![(3, 1e-4, 1)]);
        assert_eq!(alive.by_dy, vec![(Species::Springturf, -1, 1e-4)]);

        // And a stand the departed animal could not reach is unaffected by its exit.
        let far = site(20, 2);
        let mut mixed = Receipts::default();
        let reacher = reading(2, site(20, 2), &[(far, Species::Springturf)]);
        attribute(
            &mut mixed,
            &[(far, Species::Springturf, 3e-5)],
            &[&reacher],
            &[&lower],
        );
        assert_eq!((mixed.attributed, mixed.contested), (3e-5, 0.0));
    }
}
