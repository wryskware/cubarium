//! The **replacement-control study harness**: three predeclared introduction sites, three
//! matched arms per site branched from one conditioned state, and a declared observation cap
//! the run prints before it starts.
//!
//! ```text
//! cargo run --release -p cubarium-voxel-flora --example replacement -- \
//!     <resident> <newcomer> [conditioning_s] [seed] [noise_seed] [--cap seconds]
//! ```
//!
//! Astra designed this study in R5.4 and corrected it in R7.2
//! (`design/7_Research/astra-voxel-first-wave-review-2026-09-16.md`); the brief that
//! condensed it is `design/handoffs/voxel-replacement-study-brief-2026-09-17.md`. What it is
//! **not**: a preset gate, a clearance condition, a coexistence verdict, or the long control
//! and probe themselves. It is the harness, and one short smoke of it.
//!
//! What it does, in the order it does it:
//!
//! 1. **Prints the predeclared cap**, from the presets' own arithmetic
//!    ([`harness::observation_cap`]) — the time from a newborn to a donor at the growth cap
//!    plus one fully funded package, for both species of the pair, with the larger of the
//!    two as the pair's cap. A window shorter than that can only report **unresolved at
//!    cap**, and this harness says so instead of inferring exclusion.
//! 2. **Conditions one state**: the generated world under the harness's rain, warmed up, with
//!    `Provision::AtCreation` so that every support face holds the same mineral inventory in
//!    every arm however far an arm's own plants spread, then the resident's founder cohort on
//!    its own contract habitat, stepped for the conditioning duration with the water budget
//!    — accepted rain, export, **storage change**, head — and the eligible sets printed at
//!    every interval, not only the head (R5.4).
//! 3. **Predeclares the sites, once.** Three suitable, unoccupied introduction sites for the
//!    newcomer, from the conditioned state, through the model's own gates and the placement
//!    helpers, each with its eligible dispersal recipients counted and printed. The list is
//!    declared here and handed to every arm; no arm re-derives it (R7.2), which matters
//!    because the exclusion arm's own removals change what the pool would offer.
//! 4. **Runs the arms.** Per site: resident plus newcomer, and newcomer with the resident
//!    excluded — its stands and its seed banks removed with `Clear`-style bookings and its
//!    water, litter and soil mineral retained. Plus **one** resident-only control: it
//!    introduces nothing, so with the same conditioned state and the same forcing three
//!    copies of it would be bit-identical, and it is run once and reported once.
//! 5. **Observes by identity.** Per tick: newly seen newcomer birth IDs, losses and
//!    survivors separately, and every donor's funding and delivery events by identity — so
//!    that a founder donating repeatedly cannot pass as descendant reproduction, and the
//!    first birth is separate from a descendant reaching donor size and funding a package
//!    itself.
//! 6. **Refuses rather than mislabelling.** An arm whose resident was never planted or has
//!    disappeared is printed as a refusal and is never called an invasion.
//!
//! Every experiment condition it declares is printed with the run: the rain, the warm-up, the
//! founder treatment, the background canopy if one is needed, the conditioning duration and
//! the cap. None of them is a model rule.

#[path = "harness/mod.rs"]
mod harness;
use harness::*;

use cubarium_voxel::World;
use cubarium_voxel_flora::{
    Command, Flora, FloraConfig, Provision, Site, Species, SpeciesConfig, Stand, Trophic,
};

/// How many introduction sites are predeclared. Three, as R5.4 asks: "one successful site is
/// a possible refuge, not evidence that every introduction can invade".
const SITES: usize = 3;

/// The interval of the conditioning report and of every arm's table, in seconds.
const INTERVAL_S: f64 = 100.0;

/// How many per-identity events one arm prints before it stops printing them and reports the
/// counts alone. A window with more than this many births is not the window this harness was
/// built to read, and the counts are still exact.
const MAX_EVENTS: usize = 60;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let flags: Vec<&String> = args.iter().filter(|a| a.starts_with("--")).collect();
    let positional: Vec<String> =
        args.iter().filter(|a| !a.starts_with("--")).cloned().collect();

    let resident = species_arg(&positional, 0);
    let newcomer = species_arg(&positional, 1);
    let conditioning_s: f64 = arg(&positional, 2).unwrap_or(1_000.0);
    let seed: u64 = arg(&positional, 3).unwrap_or(1);
    let noise_seed: u64 = arg(&positional, 4).unwrap_or(101);
    let mut cap_override: Option<f64> = None;
    for (i, flag) in args.iter().enumerate() {
        if flag == "--cap" {
            cap_override = args.get(i + 1).and_then(|s| s.parse().ok());
            assert!(cap_override.is_some(), "`--cap` wants a number of seconds after it");
        } else if flag.starts_with("--") && flag != "--cap" {
            panic!("unknown flag {flag:?}; the only one is `--cap <seconds>`");
        }
    }
    let _ = flags;

    if resident == newcomer {
        println!(
            "REFUSED: a replacement control needs two different species, and both roles were \
             given {}.",
            resident.name()
        );
        return;
    }
    // Astra R9.7. A saprotroph's establishment predicate reads the dead wood of its own
    // mycelium box, so its introduction sites have to be chosen with the substrate-aware
    // gates over declared logs — which is a different site-selection rule, not a different
    // argument to this one. Refused here, with the reason printed, rather than run on a
    // pool that is empty by construction.
    for (role, species) in [("resident", resident), ("newcomer", newcomer)] {
        if species == Species::Glowcap {
            println!(
                "REFUSED: {} is a {:?} and this study does not admit it. Its substrate gate \
                 reads the dead wood of its own mycelium box, so a fresh world offers it no \
                 site at all and its introduction sites would have to be predeclared over \
                 **declared logs** with the substrate-aware gates \
                 (`establishment_gates_on_substrate`), which is a site-selection rule this \
                 package did not build (Astra R9.7). Nothing here is a claim about the \
                 species.",
                species.name(),
                Trophic::Saprotroph
            );
            let _ = role;
            return;
        }
    }

    let config = FloraConfig { provision: Provision::AtCreation, ..FloraConfig::default() };
    let caps = (
        observation_cap(resident, config.species(resident)),
        observation_cap(newcomer, config.species(newcomer)),
    );
    let declared = pair_cap(&caps.0, &caps.1);
    let window = cap_override.unwrap_or(declared);

    println!(
        "replacement control: resident {}, newcomer {}; seed {seed}, noise_seed {noise_seed}; \
         {conditioning_s:.0} s of conditioning then {window:.0} s of observation per arm",
        resident.name(),
        newcomer.name()
    );
    println!(
        "experiment conditions, none of them a model rule: rain {HARNESS_RAIN_M_PER_S} m/s on \
         exposed top faces, {WARMUP_TICKS} ticks ({:.0} s) of world warm-up before anything is \
         planted, {FOUNDERS_PER_SPECIES} resident founders, mineral provisioning \
         {:?} (every support face at creation, one booking)",
        WARMUP_TICKS as f64 * cubarium_voxel::DT,
        config.provision
    );
    println!(
        "founder treatment (Astra R9.7): **0.5 · wood_max** with ordinary `Seed` foliage \
         `alpha · W`, full reserve, zero parcel and no bank of its own species — {} for {} and \
         {} for {}. A flat 0.3 is that rule for the original pair and five times springturf's \
         whole mature wood, so the rule and not the number is what is fixed across the arms.",
        founder_wood(config.species(resident)),
        resident.name(),
        founder_wood(config.species(newcomer)),
        newcomer.name()
    );

    println!("\npredeclared observation cap, from the presets' own arithmetic:");
    println!("  {}", caps.0.line(config.species(resident)));
    println!("  {}", caps.1.line(config.species(newcomer)));
    println!(
        "  pair cap: **{declared:.2} s** (the larger of the two, so one window can resolve \
         either direction)"
    );
    if window + 1e-9 < declared {
        println!(
            "  the window is **shortened to {window:.0} s** by `--cap`, which is {:.1} % of the \
             predeclared cap. A window this short **cannot resolve a replacement**: whatever it \
             ends with, an arm that has not completed one is reported as **unresolved at cap** \
             and exclusion is not inferred from it (R5.4).",
            100.0 * window / declared
        );
    } else {
        println!("  the window is the predeclared cap and was not shortened.");
    }

    // ----------------------------------------------------------- the conditioned state
    let Some(conditioned) = condition(&config, resident, newcomer, conditioning_s, seed, noise_seed)
    else {
        return;
    };

    // ------------------------------------------------------------- the declared sites
    let declared_sites = predeclare_sites(&conditioned, newcomer);
    if declared_sites.is_empty() {
        println!(
            "\nREFUSED: the conditioned state offers {} no suitable unoccupied site at all — \
             no column passes its own establishment predicate and its habitat rule with nothing \
             standing on it. There is no introduction to make and no arm to run; a founder off \
             its own predicate is not a founder of anything (Astra R7.4).",
            newcomer.name()
        );
        return;
    }

    // -------------------------------------------------------------------- the arms
    println!("\n=== arms ===");
    println!(
        "Every arm below is a **clone** of the one conditioned state above — the same world, \
         the same water, the same litter, the same mineral inventory, the same resident cohort \
         and the same phase — so a difference between two arms is the treatment and nothing \
         else."
    );
    let control = run_arm(&conditioned, Treatment::ResidentOnly, None, window);
    for (n, site) in declared_sites.iter().enumerate() {
        println!("\n---------- site {} of {}: {}", n + 1, declared_sites.len(), site.label());
        run_arm(&conditioned, Treatment::Both, Some(site), window);
        run_arm(&conditioned, Treatment::NewcomerAlone, Some(site), window);
    }

    println!("\n=== what this can and cannot say ===");
    println!(
        "The conditioned state this all branched from planted {} {} founders{} and stood at {} \
         stands of that species at the introduction, its own descendants included.",
        conditioned.resident_planted,
        resident.name(),
        match conditioned.background {
            Some((species, n)) => format!(
                " under a declared background canopy of {n} {} founders, held constant across \
                 every arm",
                species.name()
            ),
            None => String::new(),
        },
        conditioned.resident_alive
    );
    println!(
        "The resident-only control ended with {} resident stands against the {} it was \
         conditioned with{}.",
        control.resident_end,
        conditioned.resident_alive,
        if control.resident_end < conditioned.resident_alive {
            " — a **declining** resident, and R5.4 is explicit that a newcomer increasing \
             against one of those is not coexistence evidence"
        } else {
            ""
        }
    );
    println!(
        "Coexistence evidence for these conditions would be a positive increase while rare \
         **through replacement** — a descendant of the founder reaching donor size and funding \
         its own package, beyond the founder's reserve subsidy — in **both** directions while \
         the resident persists. One birth is not it, the founder surviving is not it, and a \
         declining resident is not it. One successful site is a possible refuge. This \
         invocation runs one direction; the other is the same command with the two species \
         exchanged, and the probe window R5.4 asks for is at least 3 x the larger measured \
         replacement time G, which only the controls can establish."
    );
}

// ============================================================ arguments

fn arg<T: std::str::FromStr>(args: &[String], i: usize) -> Option<T> {
    args.get(i).and_then(|s| s.parse().ok())
}

/// A species named on the command line, through the model's own `Species::parse`. There is
/// no default: a replacement control is about two named species and a silent fallback would
/// make a run mean something other than what it was asked for.
fn species_arg(args: &[String], i: usize) -> Species {
    let names: Vec<&str> = Species::ALL.iter().map(|s| s.name()).collect();
    match args.get(i) {
        None => panic!(
            "replacement <resident> <newcomer> [conditioning_s] [seed] [noise_seed] [--cap s]; \
             the species are {}",
            names.join(", ")
        ),
        Some(name) => Species::parse(name)
            .unwrap_or_else(|| panic!("unknown species {name:?}; the six are {}", names.join(", "))),
    }
}

// ============================================================ the conditioned state

/// One conditioned state, and everything about it an arm or the report needs. The arms are
/// clones of the `world` and `flora` in here.
struct Conditioned {
    world: World,
    flora: Flora,
    skyline: Vec<Site>,
    resident: Species,
    newcomer: Species,
    /// How many resident founders were actually planted, and how many are alive at the end
    /// of conditioning: the two numbers a refusal is decided on.
    resident_planted: usize,
    resident_alive: usize,
    /// The background canopy this study declared, if the resident's habitat needed one.
    background: Option<(Species, usize)>,
}

/// Condition the hydrology and the resident, and refuse rather than hand back a state no arm
/// can be read from: `None` is a printed refusal.
fn condition(
    config: &FloraConfig,
    resident: Species,
    newcomer: Species,
    seconds: f64,
    seed: u64,
    noise_seed: u64,
) -> Option<Conditioned> {
    println!("\n=== conditioning ===");
    let mut world = prepared_world(seed, noise_seed);
    let skyline = skyline_of(&world);
    let mut flora = Flora::in_world(&world, config.clone());
    println!(
        "{} skyline columns; {} support faces provisioned with {} of mineral each at creation, \
         booked once as seeded_mineral_in {:.1} — a **fixed per-site inventory**, so no arm can \
         import mineral by colonising further (R5.4)",
        skyline.len(),
        flora.view().ground.len(),
        config.initial_mineral,
        flora.view().ledger.seeded_mineral_in
    );

    // ---- the declared background canopy, if the resident's own habitat needs one.
    //
    // R7.2: a velvetpad resident asks `UnderACrown` for its sites and an empty world has no
    // crown, so the probe used to run with **zero residents** and could then report newcomer
    // recruitment in that empty treatment. The background canopy is declared here, planted
    // **before** the branch so that it is identical in every arm including the exclusion one,
    // and it is never the resident or the newcomer: the resident must not secretly require
    // the competitor being excluded.
    let background = if habitat_of(resident) == Habitat::UnderACrown {
        let Some(species) = background_canopy(resident, newcomer) else {
            println!(
                "REFUSED: {} needs a crown over it and no declared background species is \
                 available that is neither the resident nor the newcomer.",
                resident.name()
            );
            return None;
        };
        let planted = plant_cohort(&mut flora, &world, &skyline, species, "background canopy");
        println!(
            "declared background canopy: {planted} {} founders, an **experiment condition** \
             held constant across every arm — the resident's own habitat rule reads crowns, and \
             a resident selected from an empty canopy is the R7.2 defect. It is neither the \
             resident nor the newcomer, so no arm removes it.",
            species.name()
        );
        if planted == 0 {
            println!(
                "REFUSED: the declared background canopy could not be planted, so {} has no \
                 crown to sit under and its cohort would be selected from an empty layer (R7.2).",
                resident.name()
            );
            return None;
        }
        Some((species, planted))
    } else {
        None
    };

    // ---- the resident cohort, on its own contract habitat.
    let resident_planted = plant_cohort(&mut flora, &world, &skyline, resident, "resident");
    if resident_planted == 0 {
        println!(
            "\nREFUSED: **{} was never planted.** Not one of the {} skyline columns passes its \
             own establishment predicate and its habitat rule {:?} with nothing standing on it, \
             so this state has no resident population. An arm run from it would be a newcomer \
             introduced into an empty treatment, and R7.2 refuses to label that an invasion.",
            resident.name(),
            skyline.len(),
            habitat_of(resident)
        );
        return None;
    }

    // ---- the conditioning run, with the water budget and the eligible sets per interval.
    let mut mark = water_mark(&world, 0.0);
    let start = mark;
    let mut elapsed = 0.0;
    println!("\nconditioning run ({seconds:.0} s), reported every {INTERVAL_S:.0} s:");
    while elapsed < seconds - 1e-9 {
        let chunk = INTERVAL_S.min(seconds - elapsed);
        step_coupled(&mut flora, &mut world, chunk);
        elapsed += chunk;
        let now = water_mark(&world, elapsed);
        println!("  t {elapsed:>7.0} s: {}", water_budget_line(&mark, &now));
        println!(
            "  t {elapsed:>7.0} s: {} stands {}, eligible columns {} / {}; {} eligible columns \
             {} — interval storage, habitat and head, not only head (R5.4)",
            resident.name(),
            count(&flora, resident),
            eligible_count(&world, &flora, &skyline, resident),
            skyline.len(),
            newcomer.name(),
            eligible_count(&world, &flora, &skyline, newcomer)
        );
        mark = now;
    }
    let end = water_mark(&world, elapsed);
    println!("  whole run: {}", water_budget_line(&start, &end));

    let resident_alive = count(&flora, resident);
    let (banks, banked) = banked(&flora, resident);
    let v = flora.view();
    println!(
        "conditioned: **{resident_alive} {} stands** alive — {resident_planted} founders planted \
         and {} establishments since, so the count includes the resident's own descendants — \
         seed bank on {banks} sites holding {banked:.5}; deaths {}, births {}",
        resident.name(),
        v.ledger.establishments,
        v.ledger.deaths,
        v.ledger.births
    );
    println!("  residuals: {}", residual_line(&flora));

    if resident_alive == 0 {
        println!(
            "\nREFUSED: **{} has disappeared.** {resident_planted} founders were planted and \
             none of them is alive at the end of conditioning, so at the moment the newcomer \
             would be introduced there is no resident population. This is printed as a refusal \
             and **not** run as an arm: a newcomer introduced into an empty treatment is not an \
             invasion of anything (Astra R7.2).",
            resident.name()
        );
        return None;
    }

    Some(Conditioned {
        world,
        flora,
        skyline,
        resident,
        newcomer,
        resident_planted,
        resident_alive,
        background,
    })
}

/// The declared background canopy species for a resident whose habitat is `UnderACrown`: the
/// first species in the model's own fixed order that is neither of the study's two, is a
/// plant, and does not itself need a crown to sit under. Deterministic, and printed.
fn background_canopy(resident: Species, newcomer: Species) -> Option<Species> {
    Species::ALL.into_iter().find(|&s| {
        s != resident
            && s != newcomer
            && habitat_of(s) != Habitat::UnderACrown
            && FloraConfig::default().species(s).trophic == Trophic::Photo
    })
}

/// One cohort of `species` on its own contract habitat: `FOUNDERS_PER_SPECIES` founders on a
/// keyed spread of the sites that pass the **model's own** establishment predicate and the
/// harness's habitat rule, with nothing standing on them. No fallback to sites that fail the
/// predicate (Astra R7.4), and the per-founder gate values printed by identity (R7.4).
fn plant_cohort(
    flora: &mut Flora,
    world: &World,
    skyline: &[Site],
    species: Species,
    role: &str,
) -> usize {
    let pool = habitat(world, flora, skyline, species);
    let free: Vec<Site> = pool
        .into_iter()
        .filter(|s| flora.view().stand_at(*s).is_none())
        .collect();
    if free.is_empty() {
        println!(
            "{role} {}: 0 of {} skyline columns pass its establishment predicate and its \
             habitat rule {:?} unoccupied — **nothing planted**",
            species.name(),
            skyline.len(),
            habitat_of(species)
        );
        return 0;
    }
    let stride = (free.len() / FOUNDERS_PER_SPECIES).max(1);
    let wood = founder_wood(flora.config().species(species));
    let mut planted: Vec<(u64, Site)> = Vec::new();
    for site in free.iter().step_by(stride).take(FOUNDERS_PER_SPECIES) {
        let ok = flora.apply(
            world,
            Command::Seed { x: site.x as i64, z: site.z, species, wood },
        );
        if ok {
            let id = flora.view().stand_at(*site).expect("just planted").id;
            planted.push((id, *site));
        }
    }
    println!(
        "{role} {}: {} founders at wood {wood} on {} candidates of {} skyline columns (habitat \
         rule {:?})",
        species.name(),
        planted.len(),
        free.len(),
        skyline.len(),
        habitat_of(species)
    );
    for (id, site) in &planted {
        println!(
            "  {} #{id} ({:>3},{:>3}) y{:<2} — {}",
            species.name(),
            site.x,
            site.z,
            site.y,
            gate_line(world, flora, species, *site)
        );
    }
    planted.len()
}

fn eligible_count(world: &World, flora: &Flora, skyline: &[Site], species: Species) -> usize {
    skyline.iter().filter(|s| passes(world, flora, species, **s)).count()
}

fn residual_line(flora: &Flora) -> String {
    let v = flora.view();
    let (o, n, e) = (
        v.organic() - v.ledger.expected_organic(),
        v.mineral() - v.ledger.expected_mineral(),
        v.energy() - v.ledger.expected_energy(),
    );
    format!("organic {o:+.2e}, mineral {n:+.2e}, energy {e:+.2e} (stocks {:.4}, {:.4}, {:.4})", v.organic(), v.mineral(), v.energy())
}

// ============================================================ the declared sites

/// One predeclared introduction site: the site itself, its gate values as the predicate read
/// them, and how many eligible dispersal recipients a donor standing there would have.
struct DeclaredSite {
    site: Site,
    gates: String,
    /// Support faces inside the newcomer's own `hop` of this site, one per column, excluding
    /// the site itself: every face a package from here could land on.
    candidates: usize,
    /// How many of those pass the newcomer's own establishment predicate in the conditioned
    /// state: where a package from here could actually germinate.
    recipients: usize,
}

impl DeclaredSite {
    fn label(&self) -> String {
        format!(
            "({},{}) y{} — {} eligible recipients of {} candidate faces in hop",
            self.site.x, self.site.z, self.site.y, self.recipients, self.candidates
        )
    }
}

/// The site list, declared **once** from the conditioned state and handed to every arm.
///
/// R7.2 and package L's note: an arm must not re-derive it. The exclusion arm removes the
/// resident's stands, which changes the crowns, the occupancy and therefore the pool — so a
/// re-derived list would silently give the exclusion arm different ground from the arm it is
/// supposed to be matched with, and the comparison would be between two different
/// experiments.
fn predeclare_sites(c: &Conditioned, newcomer: Species) -> Vec<DeclaredSite> {
    println!("\n=== predeclared introduction sites ===");
    let pool = habitat(&c.world, &c.flora, &c.skyline, newcomer);
    let free: Vec<Site> =
        pool.into_iter().filter(|s| c.flora.view().stand_at(*s).is_none()).collect();
    println!(
        "{} passes its own establishment predicate on {} of {} skyline columns in the \
         conditioned state; {} of them are unoccupied and pass its habitat rule {:?}",
        newcomer.name(),
        eligible_count(&c.world, &c.flora, &c.skyline, newcomer),
        c.skyline.len(),
        free.len(),
        habitat_of(newcomer)
    );
    if free.is_empty() {
        return Vec::new();
    }
    // A spread of the whole ordered pool rather than its first few, so the three sites are
    // "across the eligible band" the pool's own habitat order lays out (R5.4).
    let stride = (free.len() / SITES).max(1);
    let chosen: Vec<Site> = free.iter().copied().step_by(stride).take(SITES).collect();
    let sites: Vec<DeclaredSite> = chosen
        .into_iter()
        .map(|site| {
            let (candidates, recipients) = recipients_of(&c.world, &c.flora, newcomer, site);
            DeclaredSite {
                site,
                gates: gate_line(&c.world, &c.flora, newcomer, site),
                candidates,
                recipients,
            }
        })
        .collect();
    for (n, s) in sites.iter().enumerate() {
        println!(
            "  site {} ({:>3},{:>3}) y{:<2}: {} eligible recipients of {} candidate faces \
             within hop {}",
            n + 1,
            s.site.x,
            s.site.z,
            s.site.y,
            s.recipients,
            s.candidates,
            c.flora.config().species(newcomer).hop
        );
        println!("           gates: {}", s.gates);
    }
    if sites.len() < SITES {
        println!(
            "  only {} of the {SITES} sites this study asks for exist in the conditioned state; \
             what is here is what is run, and no site is invented (R7.4).",
            sites.len()
        );
    }
    let _ = c.resident;
    sites
}

/// The dispersal side of a site: the support faces a package from it could land on — the
/// **highest** face of each column within `hop`, never the donor's own site, which is
/// `dispersal_target`'s own candidate rule — and how many of them the newcomer could
/// actually germinate on.
fn recipients_of(world: &World, flora: &Flora, species: Species, site: Site) -> (usize, usize) {
    let view = world.view();
    let hop = flora.config().species(species).hop as i64;
    let mut seen: Vec<Site> = Vec::new();
    for dz in -hop..=hop {
        let z = site.z as i64 + dz;
        if z < 0 || z >= view.config.depth as i64 {
            continue;
        }
        for dx in -hop..=hop {
            let x = site.x as i64 + dx;
            let Some(face) = cubarium_voxel_flora::highest_support(&view, x, z as u32) else {
                continue;
            };
            if face == site || seen.contains(&face) {
                continue;
            }
            seen.push(face);
        }
    }
    let ok = seen.iter().filter(|s| passes(world, flora, species, **s)).count();
    (seen.len(), ok)
}

// ============================================================ the arms

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Treatment {
    /// The control: the conditioned state, stepped for the window with **nothing
    /// introduced**. It cannot differ by site, so it is run once.
    ResidentOnly,
    /// The resident plus one newcomer founder at the declared site.
    Both,
    /// One newcomer founder at the declared site with the resident **excluded**: its stands
    /// and its seed banks removed with `Clear`-style bookings, its water, litter and soil
    /// mineral retained.
    NewcomerAlone,
}

impl Treatment {
    fn label(self) -> &'static str {
        match self {
            Treatment::ResidentOnly => "resident only (the control, introduced nothing)",
            Treatment::Both => "resident plus newcomer",
            Treatment::NewcomerAlone => "newcomer with the resident excluded",
        }
    }
}

struct ArmOutcome {
    resident_end: usize,
}

fn run_arm(
    c: &Conditioned,
    treatment: Treatment,
    site: Option<&DeclaredSite>,
    window: f64,
) -> ArmOutcome {
    let (resident, newcomer) = (c.resident, c.newcomer);
    println!("\n-- arm: {}", treatment.label());
    // The branch: a clone of the one conditioned state, which `tests/replacement.rs` pins as
    // stepping identically.
    let mut world = c.world.clone();
    let mut flora = c.flora.clone();

    if treatment == Treatment::NewcomerAlone && !exclude_resident(&mut flora, &world, resident) {
        println!(
            "   REFUSED: the resident could not be excluded from this arm, so it is not the \
             exclusion arm it claims to be and is not reported as one."
        );
        return ArmOutcome { resident_end: count(&flora, resident) };
    }

    let founder = match (treatment, site) {
        (Treatment::ResidentOnly, _) => None,
        (_, None) => unreachable!("an introduction arm needs a declared site"),
        (_, Some(s)) => {
            let wood = founder_wood(flora.config().species(newcomer));
            let ok = flora.apply(
                &world,
                Command::Seed {
                    x: s.site.x as i64,
                    z: s.site.z,
                    species: newcomer,
                    wood,
                },
            );
            if !ok {
                println!(
                    "   REFUSED: one {} founder could not be seeded at the declared site \
                     ({},{}): the site is occupied or has no support face. The declared list is \
                     never re-derived per arm, so this arm is refused rather than moved.",
                    newcomer.name(),
                    s.site.x,
                    s.site.z
                );
                return ArmOutcome { resident_end: count(&flora, resident) };
            }
            let stand = *flora.view().stand_at(s.site).expect("just planted");
            println!(
                "   one {} founder #{} at ({},{}) y{}: wood {}, foliage {:.4}, reserve {:.4}, \
                 parcel {:.4}; bank of its own species on {} sites ({:.5})",
                newcomer.name(),
                stand.id,
                s.site.x,
                s.site.z,
                s.site.y,
                stand.wood,
                stand.foliage,
                stand.reserve,
                stand.parcel,
                banked(&flora, newcomer).0,
                banked(&flora, newcomer).1
            );
            println!("   gates at the declared site: {}", s.gates);
            Some(stand.id)
        }
    };

    let started = std::time::Instant::now();
    let mut watch = Watch::new(&flora, newcomer, founder);
    let mut mark = water_mark(&world, 0.0);
    let ticks = (window * cubarium_voxel::TICK_HZ as f64).round() as u64;
    let per_interval = (INTERVAL_S * cubarium_voxel::TICK_HZ as f64).round() as u64;

    println!(
        "   {:>7} {:>9} {:>9} {:>7} {:>7} {:>9} {:>8} {:>10} {:>7} {:>9} {:>8}",
        "t (s)",
        "resident",
        "newcomer",
        "births",
        "losses",
        "survivors",
        "deliv.",
        "fnd.parcel",
        "banks",
        "banked",
        "head (m)"
    );
    for tick in 1..=ticks {
        world.step();
        flora.step(&mut world);
        watch.observe(&flora, tick);
        if tick % per_interval == 0 || tick == ticks {
            let t = tick as f64 * cubarium_voxel::DT;
            println!(
                "   {t:>7.0} {:>9} {:>9} {:>7} {:>7} {:>9} {:>8} {:>10.5} {:>7} {:>9.5} {:>8.3}",
                count(&flora, resident),
                count(&flora, newcomer),
                watch.births(),
                watch.losses(),
                watch.survivors(&flora),
                watch.deliveries(),
                watch.founder_parcel(&flora),
                banked(&flora, newcomer).0,
                banked(&flora, newcomer).1,
                world.aquifer_head_m()
            );
        }
    }
    let end = water_mark(&world, window);
    println!("   water: {}", water_budget_line(&mark, &end));
    mark = end;
    let _ = mark;
    println!("   residuals: {}", residual_line(&flora));
    println!(
        "   {:.1} s of wall time for {ticks} coupled ticks",
        started.elapsed().as_secs_f64()
    );

    watch.report(&flora, resident, newcomer, window, treatment, c.resident_alive);
    ArmOutcome { resident_end: count(&flora, resident) }
}

/// Remove the resident from a branched state: every stand of it with `Command::Clear`, and
/// every seed bank of it with `Command::ClearBank`, both booked as `removed_*`. The water,
/// the litter, the dead wood and the soil mineral are **retained**, and the totals are
/// printed before and after so that "retained" is a measured thing and not a claim.
///
/// `false` — a refusal, not a silent arm — if anything of the resident survives the
/// removals: an exclusion arm that did not exclude is not an arm.
fn exclude_resident(flora: &mut Flora, world: &World, resident: Species) -> bool {
    let before = Stocks::of(flora, world);
    let stands: Vec<(u64, Site)> = flora
        .view()
        .stands
        .iter()
        .filter(|s| s.species == resident)
        .map(|s| (s.id, s.site))
        .collect();
    let mut cleared = 0usize;
    for (id, site) in &stands {
        if flora.apply(world, Command::Clear { x: site.x as i64, z: site.z })
            && !flora.view().stands.iter().any(|s| s.id == *id)
        {
            cleared += 1;
        }
    }
    let banked: Vec<Site> = flora
        .view()
        .ground
        .iter()
        .filter(|g| g.seed_organic(resident) > 0.0)
        .map(|g| g.site)
        .collect();
    let mut banks_cleared = 0usize;
    for site in &banked {
        if flora.apply(world, Command::ClearBank { x: site.x as i64, z: site.z, species: resident })
        {
            banks_cleared += 1;
        }
    }
    let after = Stocks::of(flora, world);
    println!(
        "   excluded {}: {cleared} of {} stands cleared, {banks_cleared} of {} seed banks \
         removed, all of it booked out as removed_* (organic {:+.4}, mineral {:+.4})",
        resident.name(),
        stands.len(),
        banked.len(),
        after.removed_organic - before.removed_organic,
        after.removed_mineral - before.removed_mineral
    );
    println!(
        "   retained and matched: stored water {:.4} -> {:.4} m3, head {:.3} -> {:.3} m, litter \
         {:.5} -> {:.5}, dead wood {:.5} -> {:.5}, soil mineral {:.4} -> {:.4}, mineral \
         imported {:.1} -> {:.1}",
        before.stored, after.stored, before.head, after.head, before.litter, after.litter,
        before.dead_wood, after.dead_wood, before.soil_mineral, after.soil_mineral,
        before.imported_mineral, after.imported_mineral
    );
    let alive = count(flora, resident);
    let banks_left =
        flora.view().ground.iter().filter(|g| g.seed_organic(resident) > 0.0).count();
    if alive > 0 || banks_left > 0 {
        println!(
            "   the exclusion is incomplete: {alive} stands and {banks_left} banks of {} are \
             still there",
            resident.name()
        );
        return false;
    }
    // The retention, as an assertion and not a hope. Water and the ground pools are matched
    // by construction — nothing here touches them — and the imported mineral cannot move at
    // all under `Provision::AtCreation`.
    let moved = [
        ("stored water", after.stored - before.stored),
        ("head", after.head - before.head),
        ("litter", after.litter - before.litter),
        ("dead wood", after.dead_wood - before.dead_wood),
        ("soil mineral", after.soil_mineral - before.soil_mineral),
        ("imported mineral", after.imported_mineral - before.imported_mineral),
    ];
    for (what, delta) in moved {
        if delta.abs() > 1e-9 {
            println!("   the exclusion moved the {what} by {delta:+.6}, which it must not");
            return false;
        }
    }
    true
}

/// The stocks an exclusion arm has to retain, plus the two removal counters it is allowed to
/// move.
struct Stocks {
    stored: f64,
    head: f64,
    litter: f64,
    dead_wood: f64,
    soil_mineral: f64,
    imported_mineral: f64,
    removed_organic: f64,
    removed_mineral: f64,
}

impl Stocks {
    fn of(flora: &Flora, world: &World) -> Stocks {
        let v = flora.view();
        Stocks {
            stored: world.view().stored_m3(),
            head: world.aquifer_head_m(),
            litter: v.ground.iter().map(|g| g.litter).sum(),
            dead_wood: v.ground.iter().map(|g| g.dead_wood).sum(),
            soil_mineral: v.ground.iter().map(|g| g.mineral).sum(),
            imported_mineral: v.ledger.seeded_mineral_in + v.ledger.deposited_mineral_in,
            removed_organic: v.ledger.removed_organic_out,
            removed_mineral: v.ledger.removed_mineral_out,
        }
    }
}

// ============================================================ observation by identity

/// One newcomer stand's whole record: when it was first seen, when it first reached donor
/// size, what it has been funded and what it has delivered.
struct Record {
    id: u64,
    born_tick: u64,
    /// `None` for the founder, which was planted and not born.
    descendant: bool,
    donor_tick: Option<u64>,
    funded: f64,
    funded_ticks: u64,
    deliveries: Vec<(u64, Site, f64)>,
    lost_tick: Option<u64>,
    parcel: f64,
}

/// Every newcomer identity this arm has seen, and the funding and delivery events of each.
///
/// Astra's R5.4 asks for three separate numbers — births, losses and survivors — because a
/// descendant born and dead inside the window is a real birth that a final-state reading
/// calls "no recruitment". And it asks for the funding and delivery events **by identity**,
/// because a founder that keeps donating produces births that are not descendant
/// reproduction: a replacement needs a *descendant* to reach donor size and fund its own
/// package.
struct Watch {
    species: Species,
    founder: Option<u64>,
    /// Sorted by id, so a lookup is a binary search and no hash iteration can reach the
    /// model.
    records: Vec<Record>,
    alive: Vec<u64>,
    /// The newcomer's bank per ground slot at the last delivery, to name the recipient of the
    /// next one. Under `Provision::AtCreation` the ground vector never changes shape, so the
    /// slots line up.
    bank: Vec<f64>,
    events: usize,
    truncated: usize,
}

impl Watch {
    fn new(flora: &Flora, species: Species, founder: Option<u64>) -> Watch {
        let mut watch = Watch {
            species,
            founder,
            records: Vec::new(),
            alive: Vec::new(),
            bank: bank_of(flora, species),
            events: 0,
            truncated: 0,
        };
        for stand in flora.view().stands.iter().filter(|s| s.species == species) {
            watch.records.push(Record {
                id: stand.id,
                born_tick: 0,
                descendant: Some(stand.id) != founder,
                donor_tick: None,
                funded: 0.0,
                funded_ticks: 0,
                deliveries: Vec::new(),
                lost_tick: None,
                parcel: stand.parcel,
            });
            watch.alive.push(stand.id);
        }
        watch.records.sort_by_key(|r| r.id);
        watch.alive.sort_unstable();
        watch
    }

    /// One tick's observation: births, losses, funding and deliveries.
    fn observe(&mut self, flora: &Flora, tick: u64) {
        let donor_min = flora.config().species(self.species).donor_min;
        let package = package_of(flora.config().species(self.species));
        let species = self.species;
        let mut now: Vec<u64> = Vec::new();
        let mut delivered: Vec<(u64, f64)> = Vec::new();
        // Copied out before the walk: an event is printed through `&mut self`, and the view
        // borrows the layer for as long as it is held.
        let stands: Vec<Stand> =
            flora.view().stands.iter().filter(|s| s.species == species).copied().collect();
        for stand in &stands {
            now.push(stand.id);
            let i = match self.records.binary_search_by_key(&stand.id, |r| r.id) {
                Ok(i) => i,
                Err(i) => {
                    // A birth: an identity this arm has never seen alive before.
                    self.records.insert(
                        i,
                        Record {
                            id: stand.id,
                            born_tick: tick,
                            descendant: true,
                            donor_tick: None,
                            funded: 0.0,
                            funded_ticks: 0,
                            deliveries: Vec::new(),
                            lost_tick: None,
                            parcel: stand.parcel,
                        },
                    );
                    self.event(format!(
                        "birth: {} #{} at ({},{}) y{}, wood {:.5}",
                        self.species.name(),
                        stand.id,
                        stand.site.x,
                        stand.site.z,
                        stand.site.y,
                        stand.wood
                    ), tick);
                    i
                }
            };
            // Funding and delivery, from the parcel this stand holds. A tick funds
            // `propagule_rate · dt / (1 + c_g)` into the parcel and sends **one** whole
            // package or none, so a parcel that fell can only have delivered one.
            let delta = stand.parcel - self.records[i].parcel;
            self.records[i].parcel = stand.parcel;
            if delta < 0.0 {
                self.records[i].funded += delta + package;
                self.records[i].funded_ticks += 1;
                delivered.push((stand.id, package));
            } else if delta > 0.0 {
                self.records[i].funded += delta;
                self.records[i].funded_ticks += 1;
            }
            if self.records[i].donor_tick.is_none() && stand.wood >= donor_min {
                self.records[i].donor_tick = Some(tick);
                self.event(
                    format!(
                        "donor size: {} #{} reached wood {:.5} >= donor_min {donor_min} — it can \
                         now set material aside{}",
                        self.species.name(),
                        stand.id,
                        stand.wood,
                        if self.records[i].descendant {
                            ", and it is a **descendant**"
                        } else {
                            " (this is the introduced founder, not a descendant)"
                        }
                    ),
                    tick,
                );
            }
        }
        // Deliveries name their recipient by diffing the bank, which only has to be walked on
        // a tick that actually delivered.
        if !delivered.is_empty() {
            let bank = bank_of(flora, self.species);
            let mut best: Option<(usize, f64)> = None;
            for (slot, value) in bank.iter().enumerate() {
                let grew = value - self.bank.get(slot).copied().unwrap_or(0.0);
                if grew > 0.0 && best.is_none_or(|(_, b)| grew > b) {
                    best = Some((slot, grew));
                }
            }
            let where_ = best.map(|(slot, grew)| (flora.view().ground[slot].site, grew));
            self.bank = bank;
            for (id, package) in delivered {
                let (site, grew) = where_.unwrap_or((Site { x: 0, y: 0, z: 0 }, 0.0));
                if let Ok(i) = self.records.binary_search_by_key(&id, |r| r.id) {
                    self.records[i].deliveries.push((tick, site, package));
                    let descendant = self.records[i].descendant;
                    self.event(
                        format!(
                            "delivery: {} #{id} sent one {package:.4} package to ({},{}) y{} \
                             (that bank grew by {grew:.5}) — {}",
                            self.species.name(),
                            site.x,
                            site.z,
                            site.y,
                            if descendant {
                                "**a descendant funding its own package**"
                            } else {
                                "the introduced founder donating again, which is not descendant \
                                 reproduction"
                            }
                        ),
                        tick,
                    );
                }
            }
        }
        // Losses: an identity that was alive at the end of the last tick and is not now.
        now.sort_unstable();
        let previous = std::mem::take(&mut self.alive);
        for id in &previous {
            if now.binary_search(id).is_err() {
                if let Ok(i) = self.records.binary_search_by_key(id, |r| r.id) {
                    if self.records[i].lost_tick.is_none() {
                        self.records[i].lost_tick = Some(tick);
                        let descendant = self.records[i].descendant;
                        self.event(
                            format!(
                                "loss: {} #{id} is gone{}",
                                self.species.name(),
                                if descendant { "" } else { " — the introduced founder" }
                            ),
                            tick,
                        );
                    }
                }
            }
        }
        self.alive = now;
    }

    fn event(&mut self, line: String, tick: u64) {
        if self.events < MAX_EVENTS {
            self.events += 1;
            println!("   t {:>8.2} s  {line}", tick as f64 * cubarium_voxel::DT);
        } else {
            self.truncated += 1;
        }
    }

    /// Descendant births: every identity of this species this arm has seen that is not the
    /// introduced founder, alive or dead.
    fn births(&self) -> usize {
        self.records.iter().filter(|r| r.descendant).count()
    }

    fn losses(&self) -> usize {
        self.records.iter().filter(|r| r.descendant && r.lost_tick.is_some()).count()
    }

    fn survivors(&self, flora: &Flora) -> usize {
        let v = flora.view();
        self.records
            .iter()
            .filter(|r| r.descendant && v.stands.iter().any(|s| s.id == r.id))
            .count()
    }

    fn deliveries(&self) -> usize {
        self.records.iter().map(|r| r.deliveries.len()).sum()
    }

    fn founder_parcel(&self, flora: &Flora) -> f64 {
        let Some(id) = self.founder else { return 0.0 };
        flora.view().stands.iter().find(|s| s.id == id).map_or(0.0, |s| s.parcel)
    }

    /// The arm's verdict, in R5.4's own words, and never a coexistence claim.
    fn report(
        &self,
        flora: &Flora,
        resident: Species,
        newcomer: Species,
        window: f64,
        treatment: Treatment,
        resident_start: usize,
    ) {
        if self.truncated > 0 {
            println!(
                "   ({} further events were not printed; the counts below are still exact)",
                self.truncated
            );
        }
        let founder_alive = self
            .founder
            .is_some_and(|id| flora.view().stands.iter().any(|s| s.id == id));
        let births = self.births();
        let survivors = self.survivors(flora);
        let losses = self.losses();
        let resident_now = count(flora, resident);

        if treatment == Treatment::ResidentOnly {
            println!(
                "   control: {} went {resident_start} -> {resident_now} stands over \
                 {window:.0} s with nothing introduced; establishments {}, deaths {}. A resident \
                 that is itself declining cannot be invaded 'against' (R5.4), which is what this \
                 arm is for.",
                resident.name(),
                flora.view().ledger.establishments,
                flora.view().ledger.deaths
            );
            return;
        }

        println!(
            "   by identity: {births} descendant birth(s), {losses} of them lost before the end, \
             {survivors} surviving; the introduced founder {}",
            if founder_alive { "survived the whole window" } else { "died during the window" }
        );
        // The founder's own donations, separately from any descendant's.
        for r in &self.records {
            if r.deliveries.is_empty() && r.funded == 0.0 {
                continue;
            }
            println!(
                "     #{} ({}): funded {:.5} over {} ticks, {} deliveries{}",
                r.id,
                if r.descendant { "descendant" } else { "introduced founder" },
                r.funded,
                r.funded_ticks,
                r.deliveries.len(),
                match r.donor_tick {
                    Some(t) => format!(
                        ", reached donor size at t {:.2} s",
                        t as f64 * cubarium_voxel::DT
                    ),
                    None => String::new(),
                }
            );
        }
        let first_birth = self.records.iter().filter(|r| r.descendant).map(|r| r.born_tick).min();
        let replacement = self
            .records
            .iter()
            .filter(|r| r.descendant)
            .filter_map(|r| r.deliveries.first().map(|(t, _, _)| *t))
            .min();
        let descendant_donor = self
            .records
            .iter()
            .filter(|r| r.descendant)
            .filter_map(|r| r.donor_tick)
            .min();

        match (first_birth, descendant_donor, replacement) {
            (None, _, _) => println!(
                "   recruitment NOT OBSERVED within {window:.0} s: no {} identity but the \
                 introduced founder was ever alive. **Unresolved at cap** — the window is what \
                 it is and this is not evidence of exclusion (R5.4).",
                newcomer.name()
            ),
            (Some(b), None, _) => println!(
                "   first birth at t {:.2} s, and **no descendant reached donor size**: the \
                 replacement is NOT completed and this arm is **unresolved at cap** \
                 ({window:.0} s). A birth is not a generation.",
                b as f64 * cubarium_voxel::DT
            ),
            (Some(b), Some(d), None) => println!(
                "   first birth at t {:.2} s and a descendant at donor size from t {:.2} s, but \
                 **no descendant has funded a package** of its own: the replacement is NOT \
                 completed and this arm is **unresolved at cap** ({window:.0} s).",
                b as f64 * cubarium_voxel::DT,
                d as f64 * cubarium_voxel::DT
            ),
            (Some(b), Some(_), Some(g)) => println!(
                "   **replacement completed**: first birth at t {:.2} s and a descendant funded \
                 and delivered its own package at t {:.2} s, so the replacement time G for this \
                 site and direction is {:.2} s. Beyond the founder's reserve subsidy: the \
                 package was the descendant's own.",
                b as f64 * cubarium_voxel::DT,
                g as f64 * cubarium_voxel::DT,
                g as f64 * cubarium_voxel::DT
            ),
        }
        if treatment == Treatment::Both && resident_now == 0 {
            println!(
                "   REFUSED as an invasion: the resident **disappeared during this arm** \
                 ({} -> 0), so whatever the newcomer did here it did not do it against a \
                 persisting resident (Astra R7.2).",
                resident.name()
            );
        }
    }
}

/// `alive_min / w_frac`: the model's own package size, which is what a parcel has to hold
/// before anything leaves it.
fn package_of(sc: &SpeciesConfig) -> f64 {
    if sc.propagule_split[0] > 0.0 { sc.alive_min / sc.propagule_split[0] } else { 0.0 }
}

/// One species' seed-bank organic matter per ground slot, in the ground's own order.
fn bank_of(flora: &Flora, species: Species) -> Vec<f64> {
    flora.view().ground.iter().map(|g| g.seed_organic(species)).collect()
}

/// What one species' whole seed bank holds, and the two counts beside it: sites with
/// anything in them, and the total.
///
/// The `+ 0.0` is not decoration. `Iterator::sum` for `f64` folds from **-0.0**, so a bank
/// with no cohorts in it sums to negative zero and `{:.5}` prints it as `-0.00000` — which
/// in a study's own report reads as a small negative stock rather than as nothing at all.
/// Adding a positive zero normalises it and changes no other value.
fn banked(flora: &Flora, species: Species) -> (usize, f64) {
    let ground = flora.view().ground;
    let sites = ground.iter().filter(|g| g.seed_organic(species) > 0.0).count();
    (sites, ground.iter().map(|g| g.seed_organic(species)).sum::<f64>() + 0.0)
}

#[cfg(test)]
mod tests {
    //! The harness-level cases the brief lists: the predeclared site list is one list for
    //! every arm, a resident that was never planted or has disappeared is a refusal and not
    //! an arm, and the empty-layer velvetpad-resident selection check Astra's R7.2 asked for.
    //!
    //! Hand-built fixtures, never the generated world: these are tests of the selection and
    //! refusal **rules**, and a 128 x 24 world with a thousand warm-up ticks in it would make
    //! them a small experiment instead.

    use super::*;
    use cubarium_voxel::{Command as WorldCommand, Config as VoxelConfig, Material};

    /// A flat soil strip, `width` columns of one row of depth, bedrock at `y = 0` and soil at
    /// `y = 1..=2` holding `pore` of soil's own capacity, in open sky. Never stepped, so the
    /// pore fraction is the condition it says it is.
    fn strip(width: u32, pore: f64) -> World {
        let mut w = World::empty(VoxelConfig {
            width,
            height: 10,
            depth: 1,
            voxel_m: 1.0,
            seed: 3,
            ..VoxelConfig::default()
        });
        for x in 0..width as i64 {
            for y in 1..=2u32 {
                let want = pore * Material::Soil.pore_capacity() * w.config().voxel_volume();
                w.apply(WorldCommand::AddWater { x, y, z: 0, volume_m3: want });
                w.apply(WorldCommand::SetMaterial { x, y, z: 0, material: Material::Soil });
            }
        }
        w
    }

    fn eager() -> FloraConfig {
        FloraConfig { provision: Provision::AtCreation, ..FloraConfig::default() }
    }

    /// **The R7.2 empty-layer check.** A velvetpad resident asks `UnderACrown` for its sites,
    /// and on a layer with no stands in it `canopy_over` has no crown to offer: the pool is
    /// **empty**, so a study that selected its resident from an empty layer would plant zero
    /// residents and could then report the newcomer recruiting in an empty treatment. With the
    /// declared background canopy standing first, the same rule offers sites.
    #[test]
    fn a_velvetpad_resident_has_no_site_at_all_on_an_empty_layer() {
        let world = strip(8, 0.5);
        let skyline = skyline_of(&world);
        let mut flora = Flora::in_world(&world, eager());

        // The gates themselves are open here: it is the habitat rule that has nothing.
        assert_eq!(
            skyline.iter().filter(|s| passes(&world, &flora, Species::Velvetpad, **s)).count(),
            skyline.len(),
            "the fixture's premise: every column passes velvetpad's own predicate"
        );
        assert!(
            habitat(&world, &flora, &skyline, Species::Velvetpad).is_empty(),
            "an empty layer has no crown, so UnderACrown can offer nothing"
        );

        // The declared background canopy, which is what the study plants first.
        let background =
            background_canopy(Species::Velvetpad, Species::Umbrellafrond).expect("a background");
        assert_eq!(background, Species::Bloomcrown, "the first eligible species in ALL order");
        let wood = founder_wood(flora.config().species(background));
        assert!(flora.apply(
            &world,
            Command::Seed { x: 4, z: 0, species: background, wood }
        ));
        let under = habitat(&world, &flora, &skyline, Species::Velvetpad);
        assert!(
            !under.is_empty(),
            "with a crown standing, the same rule offers {} sites",
            under.len()
        );
        assert!(
            under.iter().all(|s| s.x.abs_diff(4) <= 1),
            "and they are the columns that crown actually covers: {under:?}"
        );
    }

    /// **A resident that was never planted is a refusal.** The velvetpad case again, through
    /// `plant_cohort`: no site, nothing planted, zero — which `condition` turns into the
    /// printed refusal instead of an arm.
    #[test]
    fn a_resident_with_no_contract_site_plants_nothing() {
        let world = strip(8, 0.5);
        let skyline = skyline_of(&world);
        let mut flora = Flora::in_world(&world, eager());
        let planted = plant_cohort(&mut flora, &world, &skyline, Species::Velvetpad, "resident");
        assert_eq!(planted, 0, "no crown, no site, no founder");
        assert_eq!(count(&flora, Species::Velvetpad), 0);
    }

    /// **A resident that disappears before the introduction is a refusal too.** Bloomcrown
    /// drowns in more than 0.05 m of standing water, so a strip flooded after planting loses
    /// its whole cohort in one tick — and what the study checks at the moment of introduction
    /// is the **living** count, not the planted one.
    #[test]
    fn a_resident_that_has_disappeared_is_caught_at_the_introduction() {
        let mut world = strip(8, 0.5);
        let skyline = skyline_of(&world);
        let mut flora = Flora::in_world(&world, eager());
        let planted = plant_cohort(&mut flora, &world, &skyline, Species::Bloomcrown, "resident");
        assert!(planted > 0, "the premise: the cohort is planted");

        // A metre of free water over every support face: well past bloomcrown's 0.05 m.
        for x in 0..8i64 {
            world.apply(WorldCommand::AddWater { x, y: 3, z: 0, volume_m3: 0.9 });
        }
        flora.step(&mut world);
        assert_eq!(
            count(&flora, Species::Bloomcrown),
            0,
            "the fixture's premise: the cohort drowned"
        );
        // This is the condition `condition` refuses on: planted > 0 and alive == 0.
        assert!(planted > 0 && count(&flora, Species::Bloomcrown) == 0);
    }

    /// **One site list, for every arm.** The list is declared from the conditioned state and
    /// handed to the arms; this pins both halves of that — the same conditioned state always
    /// declares the same list, and an arm's own state would declare a **different** one, which
    /// is why no arm may re-derive it (R7.2, package L's note).
    #[test]
    fn the_declared_site_list_is_one_list_and_an_arm_would_derive_another() {
        let world = strip(12, 0.5);
        let skyline = skyline_of(&world);
        let mut flora = Flora::in_world(&world, eager());
        // A bloomcrown resident cohort, then umbrellafrond's sites among what is left.
        let planted = plant_cohort(&mut flora, &world, &skyline, Species::Bloomcrown, "resident");
        assert!(planted > 0);
        let c = Conditioned {
            world: world.clone(),
            flora: flora.clone(),
            skyline: skyline.clone(),
            resident: Species::Bloomcrown,
            newcomer: Species::Umbrellafrond,
            resident_planted: planted,
            resident_alive: count(&flora, Species::Bloomcrown),
            background: None,
        };
        let declared: Vec<Site> =
            predeclare_sites(&c, Species::Umbrellafrond).into_iter().map(|s| s.site).collect();
        let again: Vec<Site> =
            predeclare_sites(&c, Species::Umbrellafrond).into_iter().map(|s| s.site).collect();
        assert_eq!(declared, again, "one conditioned state declares one list");
        assert!(!declared.is_empty(), "the fixture must offer the newcomer somewhere");

        // The exclusion arm's own state: the resident removed, which frees its columns and
        // takes its crowns away. Re-deriving there is a different list, and that is the defect
        // the declared list exists to prevent.
        let mut arm = flora.clone();
        assert!(exclude_resident(&mut arm, &world, Species::Bloomcrown));
        let arm_state = Conditioned {
            world: world.clone(),
            flora: arm,
            skyline,
            resident: Species::Bloomcrown,
            newcomer: Species::Umbrellafrond,
            resident_planted: planted,
            resident_alive: 0,
            background: None,
        };
        let rederived: Vec<Site> = predeclare_sites(&arm_state, Species::Umbrellafrond)
            .into_iter()
            .map(|s| s.site)
            .collect();
        assert_ne!(
            declared, rederived,
            "the exclusion arm's pool differs, so a re-derived list would move the founder"
        );
    }
}
