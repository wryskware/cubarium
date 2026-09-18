//! The **replacement-control study harness**: three predeclared introduction sites, three
//! matched arms per site branched from one conditioned state, and a declared observation cap
//! the run prints before it starts.
//!
//! ```text
//! cargo run --release -p cubarium-voxel-flora --example replacement -- \
//!     [full|pilot] <resident> <newcomer> [conditioning_budget_s] [seed] [noise_seed] \
//!     [--cap <seconds>]
//! ```
//!
//! `--cap` **consumes its own value**, so shortening a run cannot change its seeds (R10.4),
//! and a duration that is negative, non-finite or unparseable is a printed refusal rather than
//! a silent default.
//!
//! Astra designed this study in R5.4 and corrected it in R7.2
//! (`design/7_Research/astra-voxel-first-wave-review-2026-09-16.md`); the brief that
//! condensed it is `design/handoffs/voxel-replacement-study-brief-2026-09-17.md`. What it is
//! **not**: a preset gate, a clearance condition, a coexistence verdict, or the long control
//! and probe themselves. It is the harness, and one short smoke of it.
//!
//! What it does, in the order it does it:
//!
//! 1. **Prints the predeclared cap and, separately, the earliest-possible replacement
//!    timeline.** The cap ([`harness::observation_cap`]) is a *newborn's* clock — newborn to
//!    donor at the growth cap plus one funded package. This study introduces a **founder
//!    with a zero parcel and no newborn**, so the thing it could actually observe is
//!    [`harness::replacement_timeline`]'s four stages: the founder's first package, the
//!    germination tick, the newborn's capped growth, and that descendant's own package —
//!    3,308.30 s for the original pair against the published cap's 3,008.15 s (Astra R10.1).
//!    The **authorised stopping budget** is printed as a third, separate number: a stopping
//!    rule, not a sufficient window. A `full` study whose budget is at or below the bound is
//!    **refused**, because no arm of it could resolve a replacement; `replacement pilot …`
//!    runs one positive-control arm instead, whose job is to measure `G`.
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
    Command, DeliveryReceipt, Flora, FloraConfig, Provision, Site, Species, Stand, Trophic,
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

/// The default stopping budget, as a multiple of the pair's earliest-possible replacement
/// bound. **A named placeholder and nothing measured it**: it exists so that a run with no
/// `--cap` stops somewhere above the bound rather than at the published cap, which R10.1
/// showed cannot observe a replacement at all. R5.4's real window — 3 x the measured `G` —
/// can only be chosen after a pilot has measured one.
const DEFAULT_BUDGET_FACTOR: f64 = 1.5;

/// The share of the conditioning budget phase A — the hydrology alone — may spend before
/// phase B gets the rest. **A named placeholder.** One budget covers both phases, and without
/// a split a hydrology that never settles would spend all of it and leave the resident
/// unconditioned: half each keeps both phases' verdicts readable, and the total stays finite
/// (R10.2).
const HYDROLOGY_SHARE: f64 = 0.5;

/// What a run is for. `full` is the seven-arm study; `pilot` is R10.1's **positive-control
/// pilot**, one arm whose job is to measure `G`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Mode {
    Full,
    Pilot,
}

impl Mode {
    fn label(self) -> &'static str {
        match self {
            Mode::Full => "control study (seven arms)",
            Mode::Pilot => "positive-control **pilot** (one arm, to measure G)",
        }
    }
}

/// The two durations an arm is read against, kept together so that no report can quote one
/// without the other: the **stopping budget** this run was given, and the
/// **earliest-possible bound** the pair's own presets impose (Astra R10.1).
#[derive(Clone, Copy, Debug)]
struct Budget {
    window: f64,
    bound: f64,
}

impl Budget {
    /// The sentence an expiry earns. Never "exclusion", and never silent about the bound.
    fn unresolved(&self) -> String {
        if self.window <= self.bound {
            format!(
                "**unresolved at the stopping budget** ({:.0} s, which is {:.1} % of the \
                 earliest-possible bound {:.2} s — a replacement could not have completed \
                 here however the arm behaved)",
                self.window,
                100.0 * self.window / self.bound,
                self.bound
            )
        } else {
            format!(
                "**unresolved at the stopping budget** ({:.0} s against an earliest-possible \
                 bound of {:.2} s — long enough in principle, so this is a measurement and not \
                 an arithmetic impossibility, and it is still not evidence of exclusion)",
                self.window, self.bound
            )
        }
    }
}

fn main() {
    let argv: Vec<String> = std::env::args().skip(1).collect();
    let invocation = match parse(&argv) {
        Ok(invocation) => invocation,
        Err(why) => {
            println!("REFUSED: {why}\n{USAGE}");
            return;
        }
    };
    let Invocation { mode, resident, newcomer, conditioning_s, seed, noise_seed, cap_override } =
        invocation;

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
    let timelines = (
        replacement_timeline(resident, config.species(resident)),
        replacement_timeline(newcomer, config.species(newcomer)),
    );
    let bound = pair_bound(&timelines.0, &timelines.1);
    // The **authorised stopping budget**, which is not the bound and not a sufficient
    // window: it is when this run stops and prints what it saw.
    let window = cap_override.unwrap_or(DEFAULT_BUDGET_FACTOR * bound);
    let budget = Budget { window, bound };

    println!(
        "replacement {}: resident {}, newcomer {}; seed {seed}, noise_seed {noise_seed}; a \
         conditioning **budget** of {conditioning_s:.0} s and a stopping budget of \
         {window:.0} s per arm",
        mode.label(),
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
    println!(
        "\nearliest-possible **replacement timeline** (Astra R10.1) — from what this study \
         actually introduces, one founder with a **zero parcel** and no newborn of its own \
         anywhere, which the published cap above does not cost:"
    );
    println!("  {}", timelines.0.line());
    println!("  {}", timelines.1.line());
    println!(
        "  pair bound: **{bound:.2} s**, against the published pair cap's {declared:.2} s. It is \
         a lower bound on a lower bound — perfect funding, an immediate germination on a \
         passing site and a saturated growth cap the whole way — so a budget above it makes \
         nothing likely, and a budget below it makes a replacement **impossible**."
    );
    println!(
        "\nauthorised stopping budget: **{window:.0} s** per arm ({}), which is {:.1} % of the \
         bound. A stopping rule and not a sufficient window: on expiry every arm reports \
         **unresolved**, and exclusion is never inferred from expiry (R5.4/R10.1).",
        match cap_override {
            Some(_) => "given with `--cap`",
            None => "the default, DEFAULT_BUDGET_FACTOR x the bound — a named placeholder, \
                     nothing measured it",
        },
        100.0 * window / bound
    );
    if window <= bound {
        match mode {
            Mode::Full => {
                println!(
                    "\nREFUSED: the stopping budget {window:.0} s is **at or below the \
                     earliest-possible bound {bound:.2} s**, so no arm of this study could \
                     observe the replacement it is named after, and seven arms of it would buy \
                     nothing but expiry (Astra R10.1). Either raise the budget above the bound, \
                     or run `replacement pilot {} {} ...` — one positive-control arm, whose job \
                     is to **measure G** so that a real budget can be chosen.",
                    resident.name(),
                    newcomer.name()
                );
                return;
            }
            Mode::Pilot => println!(
                "  the pilot runs anyway, on purpose: its job is to measure G if it can, and a \
                 pilot that expires below the bound is the expected outcome and is reported as \
                 **unresolved** rather than as exclusion."
            ),
        }
    }

    // ----------------------------------------------------------- the conditioned state
    let Some(conditioned) = condition(&config, resident, newcomer, conditioning_s, seed, noise_seed)
    else {
        return;
    };
    // R10.2: an unresolved conditioning is not a conditioned state. A control refuses on it; a
    // pilot may run on it, loudly, because a pilot is measuring G and is not a comparison.
    if let Some(why) = &conditioned.unresolved {
        match mode {
            Mode::Full => {
                println!(
                    "\nREFUSED: {why}\nThe arms of a control study are matched *and* settled or \
                     they are not a control, so none were run. Raise the conditioning budget, or \
                     run `replacement pilot …` and read its arm for what it is."
                );
                return;
            }
            Mode::Pilot => println!(
                "\nthe pilot runs on an **unresolved conditioning**, stamped here and in its own \
                 verdict: whatever G it measures belongs to a setting that was still moving, and \
                 it is not a control measurement (R10.2)."
            ),
        }
    }

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
    let control = match mode {
        Mode::Pilot => {
            let site = &declared_sites[0];
            println!(
                "pilot: **one positive-control arm** — {} alone at the first declared site {}, \
                 with the resident excluded so that nothing else can be the cause of what it \
                 does. Its only question is **G**, the replacement time R5.4 asks for before a \
                 probe window of 3 x G can mean anything. No control, no comparison and no \
                 second site.",
                newcomer.name(),
                site.label()
            );
            run_arm(&conditioned, Treatment::NewcomerAlone, Some(site), budget)
        }
        Mode::Full => {
            let control = run_arm(&conditioned, Treatment::ResidentOnly, None, budget);
            for (n, site) in declared_sites.iter().enumerate() {
                println!(
                    "\n---------- site {} of {}: {}",
                    n + 1,
                    declared_sites.len(),
                    site.label()
                );
                run_arm(&conditioned, Treatment::Both, Some(site), budget);
                run_arm(&conditioned, Treatment::NewcomerAlone, Some(site), budget);
            }
            control
        }
    };

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
    if mode == Mode::Pilot {
        println!(
            "This was a **pilot**: one positive-control arm, no control to compare it with and \
             one site. If it measured a G above, that number is what a real budget and R5.4's \
             3 x G probe window can be chosen from — for this site, this seed and these \
             conditions. If it did not, the pair's earliest-possible bound is {bound:.2} s and \
             the budget it was given was {window:.0} s.",
        );
        return;
    }
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

const USAGE: &str = "usage: replacement [full|pilot] <resident> <newcomer> \
                     [conditioning_budget_s] [seed] [noise_seed] [--cap <seconds>]";

/// Everything one invocation says, parsed once.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Invocation {
    mode: Mode,
    resident: Species,
    newcomer: Species,
    conditioning_s: f64,
    seed: u64,
    noise_seed: u64,
    cap_override: Option<f64>,
}

/// The command line, as one fallible function of the argument vector — which is what makes
/// R10.4's cases testable.
///
/// **An option consumes its own value.** The old parser filtered `--`-prefixed tokens out of
/// the positional list and left the *value* in it, so `replacement bloomcrown umbrellafrond
/// 1000 --cap 300` read 300 as the **world seed** and quietly ran a different world from the
/// one the reader thinks was shortened (Astra R10.4). Shortening a run must not change its
/// experimental keys.
///
/// Two more silent fallbacks are refused here rather than defaulted: a positional that is not
/// a number at all, and a duration that is negative or not finite. A `--cap 0` is refused too
/// — an arm that runs no ticks is not an arm — while a **zero conditioning budget is
/// allowed**, because it is a legitimate way to ask what an unconditioned state looks like and
/// it reports "conditioning unresolved" like any other expiry.
fn parse(argv: &[String]) -> Result<Invocation, String> {
    let mut positional: Vec<&str> = Vec::new();
    let mut cap_override: Option<f64> = None;
    let mut i = 0usize;
    while i < argv.len() {
        let token = argv[i].as_str();
        match token {
            "--cap" => {
                let value = argv
                    .get(i + 1)
                    .ok_or_else(|| "`--cap` wants a number of seconds after it".to_string())?;
                let seconds: f64 = value
                    .parse()
                    .map_err(|_| format!("`--cap {value}` is not a number of seconds"))?;
                if !seconds.is_finite() || seconds <= 0.0 {
                    return Err(format!(
                        "`--cap {seconds}` is not a duration: a stopping budget has to be a \
                         finite, positive number of seconds, and an arm that runs no ticks is \
                         not an arm"
                    ));
                }
                cap_override = Some(seconds);
                i += 2; // the option **and** its value
            }
            other if other.starts_with("--") => {
                return Err(format!("unknown option {other:?}; the only one is `--cap <seconds>`"));
            }
            other => {
                positional.push(other);
                i += 1;
            }
        }
    }

    // `pilot` (or an explicit `full`) as the first word, before the two species.
    let (mode, positional) = match positional.first().copied() {
        Some("pilot") => (Mode::Pilot, &positional[1..]),
        Some("full") => (Mode::Full, &positional[1..]),
        _ => (Mode::Full, &positional[..]),
    };

    let species = |i: usize, role: &str| -> Result<Species, String> {
        let names: Vec<&str> = Species::ALL.iter().map(|s| s.name()).collect();
        match positional.get(i) {
            None => Err(format!("no {role} species was named; the six are {}", names.join(", "))),
            Some(name) => Species::parse(name)
                .ok_or_else(|| format!("unknown species {name:?}; the six are {}", names.join(", "))),
        }
    };
    let resident = species(0, "resident")?;
    let newcomer = species(1, "newcomer")?;

    let number = |i: usize, what: &str| -> Result<Option<f64>, String> {
        match positional.get(i) {
            None => Ok(None),
            Some(text) => text
                .parse::<f64>()
                .map(Some)
                .map_err(|_| format!("{what} {text:?} is not a number")),
        }
    };
    let conditioning_s = number(2, "the conditioning budget")?.unwrap_or(1_000.0);
    if !conditioning_s.is_finite() || conditioning_s < 0.0 {
        return Err(format!(
            "a conditioning budget of {conditioning_s} is not a duration: it has to be a finite, \
             nonnegative number of seconds"
        ));
    }
    let integer = |i: usize, what: &str| -> Result<Option<u64>, String> {
        match positional.get(i) {
            None => Ok(None),
            Some(text) => {
                text.parse::<u64>().map(Some).map_err(|_| format!("{what} {text:?} is not a seed"))
            }
        }
    };
    let seed = integer(3, "the world seed")?.unwrap_or(1);
    let noise_seed = integer(4, "the noise seed")?.unwrap_or(101);
    if let Some(extra) = positional.get(5) {
        return Err(format!("{extra:?} is one argument too many"));
    }

    Ok(Invocation { mode, resident, newcomer, conditioning_s, seed, noise_seed, cap_override })
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
    /// `Some` when a conditioning phase's budget expired before its tolerances held: the
    /// printed **conditioning unresolved** text. A control must refuse on it; a pilot may
    /// run on it and says so (R10.2).
    unresolved: Option<String>,
}

/// Condition the **hydrology first**, then the resident, and check the coupled setting again
/// before handing anything back — Astra's R10.2. `None` is a printed refusal; a state that
/// comes back with `unresolved` set is one the caller must refuse for a control.
fn condition(
    config: &FloraConfig,
    resident: Species,
    newcomer: Species,
    budget_s: f64,
    seed: u64,
    noise_seed: u64,
) -> Option<Conditioned> {
    let tol = Tolerances::default();
    println!("\n=== conditioning ===");
    println!("  {}", tol.line());
    println!(
        "  conditioning **budget** {budget_s:.0} s, spent in two phases: the hydrology alone, \
         and then the coupled setting with the resident in it. Elapsed time is not a \
         conditioned habitat (R10.2), so each phase ends when its tolerances hold — or the \
         budget expires and the run says **conditioning unresolved**. Phase A may spend at \
         most {:.0} s of it (HYDROLOGY_SHARE, a placeholder), so that an unsettling hydrology \
         cannot leave the resident unconditioned.",
        budget_s * HYDROLOGY_SHARE
    );
    let mut world = prepared_world(seed, noise_seed);
    let skyline = skyline_of(&world);
    let mut flora = Flora::in_world(&world, config.clone());
    println!(
        "  {} skyline columns; {} support faces provisioned with {} of mineral each at \
         creation, booked once as seeded_mineral_in {:.1} — a **fixed per-site inventory**, so \
         no arm can import mineral by colonising further (R5.4)",
        skyline.len(),
        flora.view().ground.len(),
        config.initial_mineral,
        flora.view().ledger.seeded_mineral_in
    );

    // ---- phase A: the hydrology alone, **before any founder is selected**.
    //
    // The plant layer is stepped throughout and simply has nothing in it yet, so the phase
    // order and the tick clock are the same ones a planted run walks. Founder selection reads
    // the gates, the gates read the water, and the water is still moving: selecting first was
    // the R10.2 defect, and the fix is to settle first and select from what settled. That the
    // later state offers bloomcrown more sites is **not** the reason — a convenient planting
    // moment is not a conditioned one, and early planting remains its own succession
    // experiment.
    let (records_a, settle_a, spent_a) = settle_phase(
        &mut flora,
        &mut world,
        &skyline,
        &[resident, newcomer],
        budget_s * HYDROLOGY_SHARE,
        0.0,
        &tol,
        "A, the hydrology alone",
    );
    let mut unresolved = phase_verdict("A, the hydrology alone", &records_a, settle_a, &tol);

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

    // ---- the resident cohort, selected from the state phase A left.
    let resident_planted = plant_cohort(&mut flora, &world, &skyline, resident, "resident");
    if let Some(why) = refusal(resident, resident_planted, resident_planted, skyline.len()) {
        println!("\n{why}");
        return None;
    }

    // ---- phase B: the coupled setting, with the resident in it and perturbing it.
    let (records_b, settle_b, spent_b) = settle_phase(
        &mut flora,
        &mut world,
        &skyline,
        &[resident, newcomer],
        budget_s,
        spent_a,
        &tol,
        "B, the coupled setting with the resident",
    );
    let verdict_b =
        phase_verdict("B, the coupled setting with the resident", &records_b, settle_b, &tol);
    unresolved = unresolved.or(verdict_b);
    println!(
        "  conditioning spent {:.0} s of its {budget_s:.0} s budget over the two phases",
        spent_a + spent_b
    );

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

    if let Some(why) = refusal(resident, resident_planted, resident_alive, skyline.len()) {
        println!("\n{why}");
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
        unresolved,
    })
}

/// One conditioning phase: step in [`INTERVAL_S`] intervals, record what each one read, print
/// it, and stop when the tolerances hold on `tol.intervals` consecutive intervals or the
/// budget runs out. Returns the records, the verdict and the seconds this phase spent.
///
/// `already_spent` is what the earlier phase took out of the same budget, so the two phases
/// share one finite budget rather than each getting its own.
#[allow(clippy::too_many_arguments)]
fn settle_phase(
    flora: &mut Flora,
    world: &mut World,
    skyline: &[Site],
    species: &[Species],
    budget_s: f64,
    already_spent: f64,
    tol: &Tolerances,
    label: &str,
) -> (Vec<IntervalRecord>, Settle, f64) {
    println!("\nphase {label}, reported every {INTERVAL_S:.0} s:");
    let remaining = (budget_s - already_spent).max(0.0);
    if remaining <= 0.0 {
        println!("  the conditioning budget was already spent before this phase began");
        return (Vec::new(), Settle::Expired, 0.0);
    }
    let mut records: Vec<IntervalRecord> = Vec::new();
    let mut previous: Vec<Vec<Site>> =
        species.iter().map(|&s| eligible_sites(world, flora, skyline, s)).collect();
    let mut mark = water_mark(world, 0.0);
    let start = mark;
    let mut spent = 0.0;
    let mut verdict = Settle::Running;
    while spent < remaining - 1e-9 {
        let chunk = INTERVAL_S.min(remaining - spent);
        step_coupled(flora, world, chunk);
        spent += chunk;
        let now = water_mark(world, spent);
        let dt = (now.seconds - mark.seconds).max(1e-12);
        let sets: Vec<Vec<Site>> =
            species.iter().map(|&s| eligible_sites(world, flora, skyline, s)).collect();
        let record = IntervalRecord {
            seconds: spent,
            rain_rate: (now.rain_in - mark.rain_in) / dt,
            storage_rate: (now.stored - mark.stored) / dt,
            head_m: now.head_m,
            head_delta: now.head_m - mark.head_m,
            eligible: sets.iter().map(|s| s.len()).collect(),
            turnover: sets
                .iter()
                .zip(previous.iter())
                .map(|(now, before)| {
                    if records.is_empty() { None } else { Some(turnover_of(before, now)) }
                })
                .collect(),
        };
        // The next interval's turnover is measured against **this** interval's sets, not
        // against the phase's first ones: a set that drifts a little every interval is not a
        // stationary one, and comparing everything with the start would call it stationary
        // once it stopped moving relative to a state it had long left.
        previous = sets;
        println!("  t {spent:>7.0} s: {}", water_budget_line(&mark, &now));
        let habitat: Vec<String> = species
            .iter()
            .zip(record.eligible.iter())
            .zip(record.turnover.iter())
            .map(|((s, n), t)| {
                format!(
                    "{} {n}/{}{}",
                    s.name(),
                    skyline.len(),
                    match t {
                        Some(t) => format!(" (turnover {:.1} %)", 100.0 * t),
                        None => String::new(),
                    }
                )
            })
            .collect();
        println!(
            "  t {spent:>7.0} s: stands {}; eligible columns {} — {}",
            flora.view().stands.len(),
            habitat.join(", "),
            tol.why(&record)
        );
        records.push(record);
        mark = now;
        verdict = conditioning_verdict(&records, tol, remaining);
        if verdict != Settle::Running {
            break;
        }
    }
    println!("  phase {label}, whole phase: {}", water_budget_line(&start, &mark));
    (records, verdict, spent)
}

/// One phase's verdict, printed. `Some(text)` is the **conditioning unresolved** line the
/// caller has to refuse a control on; `None` means the phase settled.
fn phase_verdict(
    label: &str,
    records: &[IntervalRecord],
    settle: Settle,
    tol: &Tolerances,
) -> Option<String> {
    match settle {
        Settle::Settled { at_s } => {
            println!(
                "  phase {label}: **settled** at t {at_s:.0} s — the tolerances held on {} \
                 consecutive intervals",
                tol.intervals
            );
            None
        }
        Settle::Running | Settle::Expired => {
            let last = records
                .last()
                .map(|r| tol.why(r))
                .unwrap_or_else(|| "no interval ran at all".to_string());
            let text = format!(
                "**CONDITIONING UNRESOLVED** in phase {label}: the budget ran out after {} \
                 interval(s) and the tolerances never held on {} consecutive ones. The last \
                 interval: {last}. This is not a conditioned habitat, and a longer fixed \
                 duration is not the answer to it (R10.2) — a species with no settled niche \
                 refusing is evidence, not a reason to loosen a gate.",
                records.len(),
                tol.intervals
            );
            println!("  {text}");
            Some(text)
        }
    }
}

/// **The refusal a conditioned state earns, if it earns one** — the R7.2 rule, as one
/// function, so that the decision is testable and cannot differ between the two places the
/// study asks it.
///
/// `None` means there is a resident population to run arms against. Otherwise the returned
/// text is printed and no arm is run: an arm whose resident was never planted, or whose
/// resident has disappeared by the moment of introduction, is a **newcomer in an empty
/// treatment**, and calling that an invasion is exactly the defect Astra found in the old
/// `chesson` probe.
fn refusal(resident: Species, planted: usize, alive: usize, skyline: usize) -> Option<String> {
    if planted == 0 {
        return Some(format!(
            "REFUSED: **{} was never planted.** Not one of the {skyline} skyline columns passes \
             its own establishment predicate and its habitat rule {:?} with nothing standing on \
             it, so this state has no resident population. An arm run from it would be a \
             newcomer introduced into an empty treatment, and R7.2 refuses to label that an \
             invasion.",
            resident.name(),
            habitat_of(resident)
        ));
    }
    if alive == 0 {
        return Some(format!(
            "REFUSED: **{} has disappeared.** {planted} founders were planted and none of them \
             is alive at the end of conditioning, so at the moment the newcomer would be \
             introduced there is no resident population. This is printed as a refusal and \
             **not** run as an arm: a newcomer introduced into an empty treatment is not an \
             invasion of anything (Astra R7.2).",
            resident.name()
        ));
    }
    None
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
    budget: Budget,
) -> ArmOutcome {
    let window = budget.window;
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
        let receipts = flora.take_deliveries();
        watch.observe(&flora, &receipts, tick);
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

    watch.report(&flora, resident, newcomer, budget, treatment, c.resident_alive);
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
    ///
    /// `receipts` is the model's own `Flora::deliveries()` for this tick — who sent what
    /// where, from the only place that knows (Astra R10.3). The parcel is still read, because
    /// it is the only source for how much a stand was **funded**; what it no longer does is
    /// guess a destination.
    fn observe(&mut self, flora: &Flora, receipts: &[DeliveryReceipt], tick: u64) {
        let donor_min = flora.config().species(self.species).donor_min;
        let species = self.species;
        let mut now: Vec<u64> = Vec::new();
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
            // **Funding** is the parcel's own change, corrected by what this stand actually
            // sent this tick — which the receipts say exactly, so a delivering tick's funding
            // is no longer reconstructed from an assumed package size.
            let sent: f64 =
                receipts.iter().filter(|r| r.donor == stand.id).map(|r| r.organic).sum();
            let funded = stand.parcel - self.records[i].parcel + sent;
            self.records[i].parcel = stand.parcel;
            if funded > 0.0 {
                self.records[i].funded += funded;
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
        // **Deliveries, from the model's own receipts.** No bank arithmetic, no argmax and no
        // fallback site: two donors delivering in one tick are two receipts with two
        // destinations, and a bank that was emptied by germination between two deliveries is
        // still named correctly, both of which the old bank-difference inference got wrong
        // (Astra R10.3).
        for r in receipts.iter().filter(|r| r.species == species) {
            let Ok(i) = self.records.binary_search_by_key(&r.donor, |rec| rec.id) else {
                // A donor this watch has never seen: impossible for its own species while the
                // watch is running, and reported rather than attributed to anyone.
                self.event(
                    format!(
                        "delivery from an unwatched {} donor #{}: one {:.4} package to \
                         ({},{}) y{}",
                        r.species.name(),
                        r.donor,
                        r.organic,
                        r.recipient.x,
                        r.recipient.z,
                        r.recipient.y
                    ),
                    tick,
                );
                continue;
            };
            self.records[i].deliveries.push((tick, r.recipient, r.organic));
            let descendant = self.records[i].descendant;
            self.event(
                format!(
                    "delivery: {} #{} sent one {:.4} package ({:.6} of mineral with it) to \
                     ({},{}) y{} — {}",
                    r.species.name(),
                    r.donor,
                    r.organic,
                    r.mineral,
                    r.recipient.x,
                    r.recipient.z,
                    r.recipient.y,
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
        budget: Budget,
        treatment: Treatment,
        resident_start: usize,
    ) {
        let window = budget.window;
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
                 introduced founder was ever alive. {}.",
                newcomer.name(),
                budget.unresolved()
            ),
            (Some(b), None, _) => println!(
                "   first birth at t {:.2} s, and **no descendant reached donor size**: the \
                 replacement is NOT completed and this arm is {}. A birth is not a generation.",
                b as f64 * cubarium_voxel::DT,
                budget.unresolved()
            ),
            (Some(b), Some(d), None) => println!(
                "   first birth at t {:.2} s and a descendant at donor size from t {:.2} s, but \
                 **no descendant has funded a package** of its own: the replacement is NOT \
                 completed and this arm is {}.",
                b as f64 * cubarium_voxel::DT,
                d as f64 * cubarium_voxel::DT,
                budget.unresolved()
            ),
            (Some(b), Some(_), Some(g)) => println!(
                "   **replacement completed**: first birth at t {:.2} s and a descendant funded \
                 and delivered its own package at t {:.2} s, so the measured replacement time \
                 **G = {:.2} s** for this site and direction, against an earliest-possible \
                 bound of {:.2} s. Beyond the founder's reserve subsidy: the package was the \
                 descendant's own. R5.4's probe window is at least 3 x G = {:.2} s.",
                b as f64 * cubarium_voxel::DT,
                g as f64 * cubarium_voxel::DT,
                g as f64 * cubarium_voxel::DT,
                budget.bound,
                3.0 * g as f64 * cubarium_voxel::DT
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

    /// **An option consumes its own value** (Astra R10.4). This is the case that made the
    /// item: `--cap 300` at the end of a three-positional command used to leave `300` in the
    /// positional list, where it became the **world seed** — so the shortened run was a
    /// different world from the one it claimed to shorten.
    #[test]
    fn a_cap_value_never_becomes_the_world_seed() {
        let argv = |s: &str| -> Vec<String> { s.split(' ').map(String::from).collect() };

        let shortened = parse(&argv("bloomcrown umbrellafrond 1000 --cap 300")).expect("parses");
        assert_eq!(shortened.cap_override, Some(300.0));
        assert_eq!(shortened.seed, 1, "the default seed, not the cap's value");
        assert_eq!(shortened.noise_seed, 101);
        assert_eq!(shortened.conditioning_s, 1_000.0);
        assert_eq!(shortened.mode, Mode::Full);

        // The published command supplies both seeds, and is unaffected either way.
        let published =
            parse(&argv("bloomcrown umbrellafrond 1000 1 101 --cap 300")).expect("parses");
        assert_eq!((published.seed, published.noise_seed), (1, 101));
        assert_eq!(published.cap_override, Some(300.0));

        // Explicit seeds are kept, wherever the option sits.
        let explicit = parse(&argv("bloomcrown umbrellafrond 500 7 11 --cap 60")).expect("parses");
        assert_eq!((explicit.seed, explicit.noise_seed), (7, 11));
        let leading = parse(&argv("--cap 60 bloomcrown umbrellafrond 500 7 11")).expect("parses");
        assert_eq!(leading, explicit, "an option before the positionals is the same run");

        // Omitted entirely: both seeds default and nothing is shifted.
        let bare = parse(&argv("bloomcrown umbrellafrond")).expect("parses");
        assert_eq!((bare.seed, bare.noise_seed, bare.cap_override), (1, 101, None));
        assert_eq!(bare.conditioning_s, 1_000.0);

        let pilot = parse(&argv("pilot bloomcrown umbrellafrond 400 2 3")).expect("parses");
        assert_eq!(pilot.mode, Mode::Pilot);
        assert_eq!((pilot.seed, pilot.noise_seed), (2, 3));
        assert_eq!(pilot.resident, Species::Bloomcrown);
        assert_eq!(pilot.newcomer, Species::Umbrellafrond);
    }

    /// **Durations that are not durations, and every other silent fallback, are refused.**
    #[test]
    fn a_bad_argument_is_a_refusal_and_never_a_default() {
        let bad = |s: &str| -> String {
            let argv: Vec<String> = s.split(' ').map(String::from).collect();
            parse(&argv).expect_err(&format!("{s:?} must be refused"))
        };

        assert!(bad("bloomcrown umbrellafrond -5").contains("not a duration"));
        assert!(bad("bloomcrown umbrellafrond nan").contains("not a duration"));
        assert!(bad("bloomcrown umbrellafrond inf").contains("not a duration"));
        assert!(bad("bloomcrown umbrellafrond soon").contains("not a number"));
        assert!(bad("bloomcrown umbrellafrond 100 --cap -5").contains("not a duration"));
        assert!(bad("bloomcrown umbrellafrond 100 --cap 0").contains("not an arm"));
        assert!(bad("bloomcrown umbrellafrond 100 --cap nan").contains("not a duration"));
        assert!(bad("bloomcrown umbrellafrond 100 --cap soon").contains("not a number"));
        assert!(bad("bloomcrown umbrellafrond 100 --cap").contains("wants a number"));
        assert!(bad("bloomcrown umbrellafrond --seed 4").contains("unknown option"));
        assert!(bad("bloomcrown gloomcrown").contains("unknown species"));
        assert!(bad("bloomcrown").contains("no newcomer species"));
        assert!(bad("pilot").contains("no resident species"));
        assert!(bad("bloomcrown umbrellafrond 100 1 101 extra").contains("one argument too many"));

        // A zero conditioning budget is **allowed**: it asks what an unconditioned state looks
        // like, and it reports "conditioning unresolved" like any other expiry.
        let argv: Vec<String> =
            "bloomcrown umbrellafrond 0".split(' ').map(String::from).collect();
        assert_eq!(parse(&argv).expect("parses").conditioning_s, 0.0);
    }

    /// **The two refusals, in the words they are printed in.** A resident that was never
    /// planted and a resident that has disappeared are different sentences and different
    /// causes, and neither of them is an arm.
    #[test]
    fn the_refusal_rule_names_the_two_cases_and_passes_a_living_resident() {
        let never = refusal(Species::Velvetpad, 0, 0, 3072).expect("never planted is a refusal");
        assert!(never.starts_with("REFUSED:"), "{never}");
        assert!(never.contains("was never planted"), "{never}");
        assert!(never.contains("UnderACrown"), "the habitat rule that had no site: {never}");
        assert!(!never.contains("invasion of anything"), "that is the other case: {never}");

        let gone = refusal(Species::Bloomcrown, 8, 0, 3072).expect("disappeared is a refusal");
        assert!(gone.contains("has disappeared"), "{gone}");
        assert!(gone.contains("8 founders were planted"), "{gone}");
        assert!(gone.contains("not an invasion of anything"), "{gone}");

        assert!(refusal(Species::Bloomcrown, 8, 1, 3072).is_none(), "one living stand is enough");
        assert!(refusal(Species::Bloomcrown, 8, 19, 3072).is_none());
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
            unresolved: None,
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
            unresolved: None,
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
