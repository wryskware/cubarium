//! R0b: can physical travel pay for itself, and does a continuous feeding request pin a patch?
//!
//! `design/handoffs/r0b-opus-2026-09-14.md` §2. This is a *measurement*, on the real core,
//! under the corrected motor envelope of §1. It chooses nothing: no cropping floor, no
//! regrowth change, no coefficient change, no behaviour. Every number it prints is read from
//! the world, not inferred from a `Feeding` label or a reserve delta.
//!
//! Run it:
//!
//! ```text
//! cargo run --release -p cubarium-core --example mobile_grazing
//! ```
//!
//! # The fixture
//!
//! A **3×3 patch** of cells centred on `CellId::new(Face::Top, 8, 8)` — nine cells, 4 px each,
//! in the middle of the top face where light is strongest. Every *other* cell in the world is
//! emptied of `P`, `F`, `D` and `De` and the removal is booked as an export, so the world's own
//! totals are the patch's own totals and `check_invariants` still holds. Identical in all four
//! arms; the only difference between arms is what the one grazer asks for.
//!
//! Everything else is the world's ordinary configuration: producer growth, regrowth,
//! decomposition, nutrients, cell resolution, handling prices and the type-II intake term are
//! untouched. Weather amplitude and rain are zero and mutation is off, so light, moisture and
//! genotype are matched tick for tick across arms. Births are disabled *equally* in all four
//! arms through the diagnostic seam's `bud: Some(false)`, not by changing any reproduction
//! rule: this is an individual-capability fixture, and a birth would move the ledger.
//!
//! The grazer is one unit adult founder (`Genome::founder(0.5, …)`, the same genotype R0a's
//! `food_stock_flow.rs` used), placed at the centre of the patch's south-west cell with half a
//! reserve and three quarters of a battery, `hunger_memory` 1 and mode `Seeking`. Its material
//! is booked into `external_material_in`, so the fixture's mass box closes.
//!
//! # The arms
//!
//! | Arm | Motion | Feeding request |
//! | --- | --- | --- |
//! | `gated-still` | none (effort 0) | the controller's own, behind the legacy `feed_min` gate |
//! | `open-still` | none (effort 0) | `graze_effort = 1` every tick, gate bypassed |
//! | `mobile` | scripted ring tour | `graze_effort = 1` every tick, gate bypassed |
//! | `no-intake` | `mobile`'s recorded intent, replayed | none |
//!
//! `gated-still` is the reference: it is R0a's stationary measurement re-run under the
//! corrected envelope. `open-still` isolates *the gate* — same body, same cell, same stillness,
//! only the behavioural leave rule removed — so the difference between it and `gated-still` is
//! what a continuous request does to a patch. `mobile` adds physical travel, and nothing else.
//!
//! **The script is a probe, not behaviour.** It states an intent through
//! `crate::diagnostic::ScriptedIntent` and the ordinary machinery answers it: the heading goes
//! through `motor::resolve` under `|v| + r · |ω| ≤ u`, so the grazer turns at the rate its body
//! and energy allow and walks the 4 px between cell centres at the speed it can pay for.
//! There is no teleport and no free alignment. The intake effort goes through the world's own
//! per-cell share, type-II term and reserve headroom; no capacity or funding constraint is
//! removed to let it eat.
//!
//! **The tour.** The eight *border* cells of the patch, in a closed ring of neighbours:
//! SW → S → SE → E → NE → N → NW → W → SW. The grazer holds a cell until its `P` falls below
//! `LEAVE_BELOW` (0.20 material, the world's own `feed_min`, so the leave rule is the same
//! number the legacy gate uses), then asks to face the next cell's centre and walks there. The
//! **centre cell is never visited**, which makes it a matched undisturbed control inside the
//! same patch, under the same light and the same nutrient field.
//!
//! **The no-intake control** replays `mobile`'s recorded per-tick intent — the same requested
//! heading and the same effort, tick for tick — with every intake effort at zero. That is the
//! closest matched baseline available: it pays the same upkeep and asks for the same motion, so
//! its survival time is what the starting stores alone buy at this activity level. Its
//! *resolved* motion diverges from `mobile`'s once its energy budget starts binding, which is
//! the point at which the two bodies genuinely differ; that divergence is reported, not hidden.
//!
//! # What is not measured
//!
//! Anything after the grazer starves in an arm is censored at that tick. A revisit that
//! arrives before a cell has recovered censors that cell's recovery estimate, and the report
//! says which cells are censored. One patch, one light level, one genotype, one seed: this is
//! a capability measurement, not a world average and not an ecological claim.

use std::time::{Duration, Instant};

use cubarium_core::config::WorldConfig;
use cubarium_core::diagnostic::ScriptedIntent;
use cubarium_core::genome::{Genome, decode};
use cubarium_core::ids::OrganismId;
use cubarium_core::motor::{self, MotorBill, ROTATION_COST_SCALE};
use cubarium_core::organism::{Mode, Organism, Origin};
use cubarium_core::rng::Counter;
use cubarium_core::{DT, IntakeDiagnostics, World};
use cubarium_surface::{CellId, Face, Vec2, cell_of};

/// Ticks per arm: 36,000 at 20 Hz is 1,800 s of world time — the handoff's ceiling.
const TICKS: u64 = 36_000;
/// How often a row is written.
const SAMPLE: u64 = 3_000;
/// The disclosed leave rule: the grazer holds a cell until its producer stock falls below this.
/// 0.20 is the world's own `drives.feed_min`, so the script leaves exactly where the legacy
/// behavioural gate would have closed.
const LEAVE_BELOW: f64 = 0.20;
/// The whole diagnostic's wall-time budget, retries included. A run that reaches it stops and
/// says what is missing rather than being tuned until it fits.
const WALL_BUDGET: Duration = Duration::from_secs(120);

/// The patch centre. Light is strongest in the middle of the top face.
fn centre() -> CellId {
    CellId::new(Face::Top, 8, 8)
}

/// The nine cells of the patch, centre last.
fn patch() -> Vec<CellId> {
    let mut cells: Vec<CellId> = ring().to_vec();
    cells.push(centre());
    cells
}

/// The tour: the eight border cells as a closed ring of neighbours, starting south-west.
fn ring() -> [CellId; 8] {
    [
        CellId::new(Face::Top, 7, 7),
        CellId::new(Face::Top, 8, 7),
        CellId::new(Face::Top, 9, 7),
        CellId::new(Face::Top, 9, 8),
        CellId::new(Face::Top, 9, 9),
        CellId::new(Face::Top, 8, 9),
        CellId::new(Face::Top, 7, 9),
        CellId::new(Face::Top, 7, 8),
    ]
}

fn main() {
    let started = Instant::now();
    println!("# R0b mobile-grazing measurement (corrected motor envelope)");
    println!("# 3x3 patch on Face::Top around {:?}; every other cell emptied of P/F/D/De,", centre());
    println!("# so world totals are patch totals. Births disabled in every arm.");
    let cfg = fixture_config();
    let unit = decode(&Genome::founder(0.5, &cfg.drives), &cfg.organism);
    println!(
        "# leave_below {LEAVE_BELOW}  feed_min {}  K_P {}  P_max {}  graze_rate {:.6} m/s",
        cfg.drives.feed_min, cfg.organism.intake_half_saturation, cfg.producer.max, unit.graze_rate,
    );
    println!(
        "# grazer: extent {:.3} px  speed_max {:.3} px/s  turn_rate_max {:.1} deg/s  \
         pivot ceiling at full effort {:.4} deg/s  upkeep {:.6} energy/s",
        unit.extent,
        unit.speed_max,
        f64::from(unit.drives.turn_rate_max_deg),
        (unit.speed_max / unit.extent).to_degrees(),
        (unit.maintenance * unit.structure_adult + cfg.organism.sense_cost * unit.sense_radius),
    );

    // `mobile` runs first so its intent schedule can be replayed by the matched control.
    let mobile = run(Arm::Mobile, &[], started);
    let schedule = mobile.schedule.clone();
    let gated = run(Arm::GatedStill, &[], started);
    let open = run(Arm::OpenStill, &[], started);
    let control = run(Arm::NoIntake, &schedule, started);

    let arms = [&gated, &open, &mobile, &control];
    summary(&arms);
    println!();
    println!("# wall time {:.1} s of the {} s budget", started.elapsed().as_secs_f64(), WALL_BUDGET.as_secs());
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Arm {
    GatedStill,
    OpenStill,
    Mobile,
    NoIntake,
}

impl Arm {
    fn name(self) -> &'static str {
        match self {
            Arm::GatedStill => "gated-still",
            Arm::OpenStill => "open-still",
            Arm::Mobile => "mobile",
            Arm::NoIntake => "no-intake",
        }
    }

    fn mobile(self) -> bool {
        self == Arm::Mobile
    }
}

/// One tick of recorded intent: what the `mobile` arm asked for, so the control can ask for it
/// too. This is the *request*, never the resolved motion.
#[derive(Clone, Copy)]
struct Intent {
    heading: Vec2,
    effort: f64,
}

struct Report {
    name: &'static str,
    /// Ticks actually simulated (a starved arm stops there; so does the wall-time budget).
    ticks: u64,
    stopped_early: Option<&'static str>,
    died_at: Option<u64>,
    /// The first tick at which the usable store could no longer pay one tick of upkeep. This,
    /// not `died_at`, is when a body stops being able to fund itself — see the note in
    /// `summary` about why the world's own starvation predicate can miss it.
    starved_at: Option<u64>,
    schedule: Vec<Intent>,
    p_start: [f64; 9],
    p_end: [f64; 9],
    intake: IntakeDiagnostics,
    store_start: f64,
    store_end: f64,
    store_min: f64,
    upkeep_paid: f64,
    travel_paid: f64,
    rotation_paid: f64,
    travelled_px: f64,
    turned_rad: f64,
    transitions: u64,
    /// One entry per completed tour of the ring: `(usable-store delta e, producer eaten m,
    /// seconds, whether the store sat at its ceiling for any of it)`.
    cycle_yield: Vec<(f64, f64, f64, bool)>,
    /// `(cell, P when abandoned, P at revisit or horizon, seconds away, censored by a revisit?)`.
    recovery: Vec<(CellId, f64, f64, f64, bool)>,
    centre_start: f64,
    centre_end: f64,
}

impl Report {
    fn usable_delta(&self) -> f64 {
        self.store_end - self.store_start
    }
    fn seconds(&self) -> f64 {
        self.ticks as f64 * DT
    }
}

#[allow(clippy::too_many_lines)]
fn run(arm: Arm, replay: &[Intent], started: Instant) -> Report {
    let cfg = fixture_config();
    let e_r = cfg.organism.reserve_energy_density;
    let mut world = World::new(cfg).expect("the fixture config is valid");
    strip_to_patch(&mut world);
    let id = place(&mut world);
    let mut world = World::from_state(world.state).expect("the staged state is a valid world");
    world.check_invariants().expect("the staged world is consistent");

    let (bill, extent) = {
        let o = world.state.organisms.get(id).expect("placed");
        (MotorBill::of(o, world.config()), o.phenotype.extent)
    };
    let usable = |w: &World| -> f64 {
        w.state
            .organisms
            .get(id)
            .map_or(0.0, |o| o.energy + e_r * o.reserve)
    };

    println!();
    println!("## arm `{}`", arm.name());
    println!(
        "{:>7} {:>9} {:>9} {:>9} {:>9} {:>10} {:>10} {:>9} {:>9} {:>9} {:>6}",
        "tick", "P_here", "P_patch", "P_centre", "N_patch", "grown", "eaten_P", "E", "R", "px", "cell"
    );

    let cells = patch();
    let p_of = |w: &World, c: CellId| w.state.fields.p[c.index()];
    let mut p_start = [0.0; 9];
    for (i, c) in cells.iter().enumerate() {
        p_start[i] = p_of(&world, *c);
    }
    let centre_start = p_of(&world, centre());

    let mut report = Report {
        name: arm.name(),
        ticks: 0,
        stopped_early: None,
        died_at: None,
        starved_at: None,
        schedule: Vec::new(),
        p_start,
        p_end: [0.0; 9],
        intake: IntakeDiagnostics::default(),
        store_start: usable(&world),
        store_end: 0.0,
        store_min: usable(&world),
        upkeep_paid: 0.0,
        travel_paid: 0.0,
        rotation_paid: 0.0,
        travelled_px: 0.0,
        turned_rad: 0.0,
        transitions: 0,
        cycle_yield: Vec::new(),
        recovery: Vec::new(),
        centre_start,
        centre_end: 0.0,
    };

    // Tour state. `target` is the ring index the grazer is heading for; it dwells once it is
    // standing in that cell, and advances when the cell falls below the leave rule.
    let mut target = 0usize;
    let mut laps = 0u32;
    let mut lap_opening = report.store_start;
    let mut lap_eaten = 0.0;
    let mut lap_tick = 0u64;
    let mut lap_saturated = false;
    let saturation_ceiling = {
        let o = world.state.organisms.get(id).expect("placed");
        o.phenotype.energy_max + e_r * o.phenotype.reserve_max
    };
    // The recovery estimate, one row per ring cell, opened the **first** time that cell is
    // abandoned and closed at the first revisit (censored: the mouth arrived and the cell
    // stopped recovering) or at the horizon. Later abandonments do not reopen it, so the
    // estimate is always "what came back after the first visit", never a mixture of laps.
    let mut left_at: [Option<(f64, u64)>; 8] = [None; 8];
    let mut recovered: Vec<(CellId, f64, f64, f64, bool)> = Vec::new();
    let mut closed = [false; 8];
    // A cell only starts recovering once the mouth has actually walked out of it.
    let mut gone = [false; 8];
    let mut last_cell = cell_of(&world.state.organisms.get(id).expect("placed").pos);

    for tick in 0..TICKS {
        if started.elapsed() >= WALL_BUDGET {
            report.stopped_early = Some("wall-time budget reached");
            break;
        }
        let Some(o) = world.state.organisms.get(id) else {
            report.died_at = Some(tick);
            break;
        };
        let here = cell_of(&o.pos);
        let heading_before = o.heading;
        let pos_before = o.pos;

        // The intent this tick.
        let intent = if arm == Arm::NoIntake {
            replay
                .get(tick as usize)
                .copied()
                .unwrap_or(Intent { heading: heading_before, effort: 0.0 })
        } else if arm.mobile() {
            let goal = ring()[target];
            if here == goal {
                // Dwelling: stand still and crop. Leaving is the disclosed threshold.
                Intent { heading: heading_before, effort: 0.0 }
            } else {
                // Travelling: face the goal's centre and walk. `toward` is a *request*; the
                // resolver decides how much of the turn and how much of the walk happen.
                let toward = (goal.center().chart() - pos_before.chart())
                    .normalized()
                    .unwrap_or(heading_before);
                Intent { heading: toward, effort: 1.0 }
            }
        } else {
            Intent { heading: heading_before, effort: 0.0 }
        };
        if arm.mobile() {
            report.schedule.push(intent);
        }

        let scripted = ScriptedIntent {
            heading: Some(intent.heading),
            effort: Some(intent.effort),
            graze_effort: match arm {
                // The legacy gate stays in charge here: the controller's own answer stands.
                Arm::GatedStill => None,
                Arm::OpenStill | Arm::Mobile => Some(1.0),
                Arm::NoIntake => Some(0.0),
            },
            fruit_effort: if arm == Arm::NoIntake { Some(0.0) } else { None },
            scavenge_effort: if arm == Arm::NoIntake { Some(0.0) } else { None },
            mode: None,
            bud: Some(false),
        };
        world.set_scripted_intents(vec![(id, scripted)]);

        if tick % SAMPLE == 0 {
            let patch_p: f64 = cells.iter().map(|c| p_of(&world, *c)).sum();
            let patch_n: f64 = cells.iter().map(|c| world.state.fields.n[c.index()]).sum();
            let diag = world.intake_diagnostics();
            println!(
                "{tick:>7} {:>9.4} {:>9.4} {:>9.4} {:>9.4} {:>10.4} {:>10.4} {:>9.3e} {:>9.3e} {:>9.2} {:>6}",
                p_of(&world, here),
                patch_p,
                p_of(&world, centre()),
                patch_n,
                diag.producer_growth,
                diag.producer_eaten,
                world.state.organisms.get(id).map_or(0.0, |o| o.energy),
                world.state.organisms.get(id).map_or(0.0, |o| o.reserve),
                report.travelled_px,
                ring().iter().position(|c| *c == here).map_or(-1i32, |i| i as i32),
            );
        }

        world.step();
        world.drain_events();
        report.ticks = tick + 1;

        let Some(o) = world.state.organisms.get(id) else {
            report.died_at = Some(tick + 1);
            break;
        };
        // Actual motion, read from the published path and the heading the world stored — not
        // from the request. The patch is interior to one face, so no seam transport occurs and
        // every radian here is a physical turn.
        let travelled: f64 = world
            .moved_segments(id)
            .iter()
            .map(cubarium_surface::PathSegment::length)
            .sum();
        let turn = signed_turn(heading_before, o.heading).abs();
        report.travelled_px += travelled;
        report.turned_rad += turn;
        report.upkeep_paid += bill.upkeep(DT);
        report.travel_paid += bill.motor_cost(travelled / DT, 0.0, DT);
        report.rotation_paid += bill.motor_cost(0.0, extent * turn / DT, DT);
        let store = usable(&world);
        report.store_min = report.store_min.min(store);
        if report.starved_at.is_none() && store < bill.upkeep(DT) {
            report.starved_at = Some(tick + 1);
        }
        if store >= saturation_ceiling * (1.0 - 1e-9) {
            lap_saturated = true;
        }

        let now = cell_of(&o.pos);
        if now != last_cell {
            report.transitions += 1;
            last_cell = now;
        }
        if arm.mobile() {
            let goal = ring()[target];
            if now == goal && p_of(&world, goal) < LEAVE_BELOW {
                if left_at[target].is_none() {
                    left_at[target] = Some((p_of(&world, goal), tick + 1));
                }
                target = (target + 1) % ring().len();
                if target == 0 {
                    laps += 1;
                    let eaten = world.intake_diagnostics().producer_eaten;
                    report.cycle_yield.push((
                        store - lap_opening,
                        eaten - lap_eaten,
                        (tick + 1 - lap_tick) as f64 * DT,
                        lap_saturated,
                    ));
                    lap_opening = store;
                    lap_eaten = eaten;
                    lap_tick = tick + 1;
                    lap_saturated = false;
                }
            }
            // A recovery window opens when the body has actually walked out of the cell, and
            // closes — censored — the moment it walks back in.
            for r in 0..ring().len() {
                if left_at[r].is_some() && !gone[r] && now != ring()[r] {
                    gone[r] = true;
                }
            }
            if let Some(r) = ring().iter().position(|c| *c == now)
                && !closed[r]
                && gone[r]
                && let Some((p_left, t_left)) = left_at[r]
            {
                recovered.push((
                    ring()[r],
                    p_left,
                    p_of(&world, ring()[r]),
                    (tick + 1 - t_left) as f64 * DT,
                    true,
                ));
                closed[r] = true;
            }
        }
    }
    let _ = laps;

    // A cell never revisited gives an uncensored recovery estimate at the horizon.
    for (r, entry) in left_at.iter().enumerate() {
        if let Some((p_left, t_left)) = entry
            && !closed[r]
        {
            recovered.push((
                ring()[r],
                *p_left,
                p_of(&world, ring()[r]),
                report.ticks.saturating_sub(*t_left) as f64 * DT,
                false,
            ));
        }
    }
    report.recovery = recovered;
    for (i, c) in cells.iter().enumerate() {
        report.p_end[i] = p_of(&world, *c);
    }
    report.centre_end = p_of(&world, centre());
    report.store_end = usable(&world);
    report.intake = world.intake_diagnostics();
    world.clear_scripted_intents();
    world.check_invariants().expect("the arm ended consistent");
    report
}

fn summary(arms: &[&Report; 4]) {
    println!();
    println!("## Summary  (material `m`, energy `e`, world seconds; 20 ticks = 1 s)");
    println!(
        "{:<12} {:>7} {:>8} {:>9} {:>9} {:>9} {:>9} {:>9} {:>9} {:>6} {:>7} {:>9}",
        "arm", "s", "eaten_P", "grown", "d_store", "upkeep", "travel", "rotate", "px", "cells", "turns", "survival"
    );
    for r in arms {
        println!(
            "{:<12} {:>7.0} {:>8.4} {:>9.4} {:>+9.4} {:>9.4} {:>9.5} {:>9.5} {:>9.2} {:>6} {:>7.2} {:>9}",
            r.name,
            r.seconds(),
            r.intake.producer_eaten,
            r.intake.producer_growth,
            r.usable_delta(),
            r.upkeep_paid,
            r.travel_paid,
            r.rotation_paid,
            r.travelled_px,
            r.transitions,
            r.turned_rad.to_degrees() / 360.0,
            match (r.starved_at, r.died_at) {
                (_, Some(t)) => format!("died {t}"),
                (Some(t), None) => format!("broke {t}"),
                (None, None) => "funded".to_string(),
            },
        );
    }
    println!();
    println!("## Patch stocks (9 cells; the last column is the never-visited centre cell)");
    for r in arms {
        let start: f64 = r.p_start.iter().sum();
        let end: f64 = r.p_end.iter().sum();
        println!(
            "- {:<12} P_patch {start:.4} -> {end:.4} m; centre {:.4} -> {:.4} m; \
             fruit eaten {:.4}; detritus eaten {:.4}",
            r.name, r.centre_start, r.centre_end, r.intake.fruit_eaten, r.intake.detritus_eaten
        );
    }
    println!();
    println!("## Requests and refusals");
    for r in arms {
        let served = r.intake.producer_eaten + r.intake.fruit_eaten + r.intake.detritus_eaten;
        let served_pct = if r.intake.requested > 0.0 {
            format!("{:.1}%", 100.0 * served / r.intake.requested)
        } else {
            "n/a".to_string()
        };
        let sat_pct = if r.intake.request_ticks > 0 {
            format!(
                "{:.1}%",
                100.0 * r.intake.reserve_saturated_ticks as f64 / r.intake.request_ticks as f64
            )
        } else {
            "n/a".to_string()
        };
        println!(
            "- {:<12} requesting ticks {:>6}; served {served_pct} of what was asked; \
             refused-because-full {sat_pct}; usable store {:.4} -> {:.4} (min {:.4})",
            r.name, r.intake.request_ticks, r.store_start, r.store_end, r.store_min
        );
    }

    let mobile = arms[2];
    println!();
    println!("## Mobile arm: the cycle");
    if mobile.cycle_yield.is_empty() {
        println!("- No complete tour of the ring finished inside the horizon.");
    } else {
        for (i, (delta, eaten, secs, saturated)) in mobile.cycle_yield.iter().enumerate() {
            println!(
                "- tour {}: {secs:>6.0} s, ate {eaten:.4} m of P, usable store {delta:+.4} e{}",
                i + 1,
                if *saturated { "  (store at its ceiling for part of it: the yield is censored from above)" } else { "" }
            );
        }
    }
    println!(
        "- {} cell transitions, {:.2} full turns of the body, {:.2} px travelled in {:.0} s.",
        mobile.transitions,
        mobile.turned_rad.to_degrees() / 360.0,
        mobile.travelled_px,
        mobile.seconds()
    );
    println!(
        "- Motor spend: {:.5} e on travel + {:.5} e on rotation against {:.4} e of upkeep \
         ({:.1}% of the whole bill is motion).",
        mobile.travel_paid,
        mobile.rotation_paid,
        mobile.upkeep_paid,
        100.0 * (mobile.travel_paid + mobile.rotation_paid)
            / (mobile.travel_paid + mobile.rotation_paid + mobile.upkeep_paid).max(1e-12)
    );
    println!("- Rotation is priced at ROTATION_COST_SCALE = {ROTATION_COST_SCALE}.");
    println!();
    println!("## Mobile arm: recovery of abandoned cells");
    if mobile.recovery.is_empty() {
        println!("- No cell was abandoned, so there is no recovery estimate.");
    }
    for (cell, left, now, away, censored) in &mobile.recovery {
        println!(
            "- {cell:?}: left at P {left:.4} m, {away:>6.0} s away, {} at P {now:.4} m{}",
            if *censored { "found on return" } else { "at the horizon" },
            if *censored { " (censored: the mouth arrived and it stopped recovering)" } else { "" }
        );
    }
    println!();
    println!("## Survival: `broke` is not the same as `died`");
    println!("- `broke T` is the first tick the usable store could no longer pay one tick of");
    println!("  upkeep. `died T` is the world's own starvation predicate, `E <= 0 && R <= 0`.");
    println!("- In the two ungated arms the body goes broke and then does *not* die, because");
    println!("  intake is settled before the death check: a cell stripped to ~1e-20 m still");
    println!("  serves an infinitesimal bite every tick, and that bite lands as the only energy");
    println!("  the body has when the check runs. The result is a body funding itself on a");
    println!("  geometrically shrinking trickle. This is a property of the existing world, not");
    println!("  of the motor correction; the legacy `feed_min` gate normally prevents it by");
    println!("  refusing to open on a cell that poor. R0b measures it and changes nothing:");
    println!("  a cropping floor was explicitly out of scope for this milestone.");
    println!();
    println!("## Not measured (censored, not extrapolated)");
    println!("- Anything after the grazer starved in an arm: each arm's horizon is its own.");
    println!("- Whether any of this is sustainable: one grazer, one patch, one seed, no births.");
    println!("- World-average gross production: only these nine cells produce anything here.");
    println!("- The `no-intake` arm's *resolved* motion once its energy budget binds; only its");
    println!("  requested intent is matched to `mobile`'s, tick for tick.");
}

/// The signed shortest angle from `a` to `b`, radians in `(−π, π]`.
fn signed_turn(a: Vec2, b: Vec2) -> f64 {
    use std::f64::consts::{PI, TAU};
    let d = b.screen_angle() - a.screen_angle();
    let d = (d + PI).rem_euclid(TAU) - PI;
    if d.is_finite() { d } else { 0.0 }
}

/// The fixture. Everything not named here is the world's ordinary configuration.
fn fixture_config() -> WorldConfig {
    let mut c = WorldConfig::default();
    // No founders of its own: the arms place exactly the one grazer they mean to.
    c.founders.kinds.clear();
    c.founders.count = 0;
    // Matched environmental conditions across arms: no weather swing, no rain, so the light
    // and moisture a cell sees are the same in every run at every tick.
    c.weather.amplitude = 0.0;
    c.water.rain_rate = 0.0;
    // One genotype, exactly.
    c.mechanisms.mutation = false;
    c
}

/// Empty every field cell outside the 3×3 patch, so the world's totals are the patch's.
fn strip_to_patch(world: &mut World) {
    let keep = patch();
    let mut removed = 0.0;
    for cell in CellId::all() {
        if keep.contains(&cell) {
            continue;
        }
        let i = cell.index();
        let f = &mut world.state.fields;
        removed += f.p[i] + f.d[i] + f.f[i];
        f.p[i] = 0.0;
        f.f[i] = 0.0;
        f.d[i] = 0.0;
        f.de[i] = 0.0;
    }
    // Booked as an export, so the fixture's material box closes and `check_invariants` holds.
    world.state.external_material_in -= removed;
}

/// One unit adult in the patch's south-west cell: half a reserve so it starts hungry with room
/// to store, three quarters of a battery, and the same genotype R0a measured.
fn place(world: &mut World) -> OrganismId {
    let cfg = world.config().clone();
    let genome = Genome::founder(0.5, &cfg.drives);
    let phenotype = decode(&genome, &cfg.organism);
    let pos = ring()[0].center();
    assert_eq!(cell_of(&pos), ring()[0], "the grazer landed outside its starting cell");
    let structure = phenotype.structure_adult;
    let reserve = 0.5 * phenotype.reserve_max;
    let organism = Organism {
        pos,
        // Facing the second cell of the tour, so the first hop needs no turn at all and the
        // first measured cycle is not dominated by an arbitrary initial orientation.
        heading: (ring()[1].center().chart() - pos.chart())
            .normalized()
            .expect("neighbouring cell centres differ"),
        ou: Vec2::ZERO,
        structure,
        reserve,
        energy: 0.75 * phenotype.energy_max,
        born_tick: 0,
        hunger_memory: 1.0,
        mode: Mode::Seeking,
        escrow: None,
        births: 0,
        genome,
        phenotype,
        parent: None,
        origin: Origin::Founder,
        turn_counter: Counter::default(),
        fed_this_tick: false,
    };
    let id = world.state.organisms.insert(organism);
    world.state.external_material_in += structure + reserve;
    let _ = motor::REFERENCE_RADIUS_PX;
    id
}
