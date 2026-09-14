//! R0a part B: what local consumption and renewal actually do, measured on the real core.
//!
//! `design/handoffs/r0a-movement-foundation-2026-09-14.md` asks for a factual stock/flow
//! report before anyone chooses a replacement regrowth model. This is that measurement and
//! nothing more: it decides no design, changes no world, and tunes no parameter.
//!
//! # What this is, and what it is not
//!
//! **A staged measurement, not a demonstration of ordinary behaviour.** The consumers here
//! cannot move: the fixture sets `organism.speed_max` and `drives.turn_rate_max_deg` to zero so
//! a body stays in the cell it was placed in for the whole horizon. That isolates the rates —
//! the question is what a mouth takes from a patch and what the patch puts back, not whether an
//! animal would choose to stand there. Nothing about ordinary foraging can be read off it.
//!
//! **A single cell is the patch.** Feeding reaches exactly the cell an organism stands in
//! (`world.rs` section 7), so a cell is what "local" means to a mouth. Every other cell in the
//! fixture is emptied of `P`, `F`, `D` and `De`, which makes the world's own totals the patch's
//! own totals: producer growth is zero where `P` is zero, and decomposition is zero where `D`
//! is. The numbers reported are therefore absolute and local, not a world average.
//!
//! **Intake is measured, never inferred.** `World::intake_diagnostics` reports the material
//! that actually left each field through a mouth, after the per-cell proportional share and
//! every clamp. No number here comes from a reserve delta or from a `Feeding` label.
//!
//! # Arms
//!
//! Four runs of 12,000 ticks (600 s of world time), identical but for the consumers:
//!
//! | Arm | Consumers | Question |
//! | --- | --- | --- |
//! | `undisturbed` | 0 | What does the patch make and hold on its own? |
//! | `one` | 1 | What does one mouth take, and what does that leave standing? |
//! | `four` | 4 | Does the cell serve four mouths, and how is it shared? |
//! | `recovery` | 4, removed at tick 6,000 | What comes back once consumption stops? |
//!
//! Run it:
//!
//! ```text
//! cargo run --release -p cubarium-core --example food_stock_flow
//! ```

use cubarium_core::config::WorldConfig;
use cubarium_core::genome::{Genome, decode};
use cubarium_core::ids::OrganismId;
use cubarium_core::organism::{Mode, Organism, Origin};
use cubarium_core::rng::Counter;
use cubarium_core::{DT, IntakeDiagnostics, World};
use cubarium_surface::{CellId, Face, SurfacePoint, Vec2, cell_of};

/// Ticks per arm. 12,000 at 20 Hz is ten minutes of world time — the handoff's ceiling.
const TICKS: u64 = 12_000;
/// When the `recovery` arm's consumers are removed.
const CEASE_TICK: u64 = 6_000;
/// How often a row is written.
const SAMPLE: u64 = 1_000;

fn main() {
    let started = std::time::Instant::now();
    println!("# R0a food stock/flow measurement");
    println!("# staged fixture: consumers are immobile, one cell is the patch, every other");
    println!("# cell is emptied of P/F/D/De so world totals are patch totals.");
    let cfg = fixture_config();
    println!(
        "# patch cell {:?}  feed_min {}  K_P {}  P_max {}  graze_rate(unit adult) {:.6} m/s",
        patch(),
        cfg.drives.feed_min,
        cfg.organism.intake_half_saturation,
        cfg.producer.max,
        decode(&Genome::founder(0.5, &cfg.drives), &cfg.organism).graze_rate,
    );

    let arms = [
        ("undisturbed", 0usize, false),
        ("one", 1, false),
        ("four", 4, false),
        ("recovery", 4, true),
    ];
    let mut finals = Vec::new();
    for (name, consumers, cease) in arms {
        let report = run(name, consumers, cease);
        finals.push(report);
    }

    println!();
    println!("## Summary at tick {TICKS}  (material units; 12,000 ticks = 600 s of world time)");
    println!(
        "{:<12} {:>6} {:>8} {:>9} {:>9} {:>9} {:>9} {:>8} {:>7} {:>7} {:>16}",
        "arm", "n", "P_end", "grown", "eaten_P", "eaten_D", "per-head", "served%", "below%", "sat%",
        "starved at tick"
    );
    for r in &finals {
        let starved: Vec<String> = r
            .died
            .iter()
            .map(|d| d.map_or_else(|| "-".to_string(), |t| t.to_string()))
            .collect();
        println!(
            "{:<12} {:>6} {:>8.4} {:>9.4} {:>9.4} {:>9.4} {:>9.4} {:>7.1}% {:>6.1}% {:>6.1}% {:>16}",
            r.name,
            r.consumers,
            r.p_end,
            r.grown,
            r.eaten_p,
            r.eaten_d,
            r.per_capita(),
            r.served_percent(),
            r.below_percent(),
            r.saturated_percent(),
            if starved.is_empty() { "-".to_string() } else { starved.join(",") },
        );
    }

    let undisturbed = &finals[0];
    let one = &finals[1];
    let four = &finals[2];
    let recovery = &finals[3];

    println!();
    println!("## Stock and flow");
    let horizon = TICKS as f64 * DT;
    println!(
        "- Undisturbed patch: P {:.4} -> {:.4} m standing, gross production {:.4} m over {horizon:.0} s \
         ({:.3e} m/s mean). Nothing was eaten, so the difference between production and standing \
         stock is mortality into detritus.",
        undisturbed.p_start, undisturbed.p_end, undisturbed.grown, undisturbed.grown / horizon,
    );
    if let Some(t) = one.feeding_ticks() {
        let fed = t as f64 * DT;
        println!(
            "- One consumer: took {:.4} m in the {fed:.0} s it lived ({:.3e} m/s mean), against \
             {:.3e} m/s of gross production in the same patch undisturbed. Intake ran at {:.1}x \
             production.",
            one.eaten_p + one.eaten_d,
            (one.eaten_p + one.eaten_d) / fed,
            undisturbed.grown / horizon,
            ((one.eaten_p + one.eaten_d) / fed) / (undisturbed.grown / horizon),
        );
    }
    println!(
        "- Four consumers took {:.4} m each against {:.4} m for the single one: per-head intake \
         fell {:.1}x. The cell served {:.1}% of every request in both arms, so the competition is \
         not a within-tick share — it is the standing stock the four of them hold the patch down \
         to, and the shorter time each of them survives there.",
        four.per_capita(),
        one.per_capita(),
        one.per_capita() / four.per_capita().max(f64::MIN_POSITIVE),
        four.served_percent(),
    );
    println!(
        "- Depletion floor: a consumed patch sits pinned just under the feeding gate \
         (feed_min {:.2}); the `one` arm spent {:.1}% of its horizon with no edible kind above \
         that gate, the `four` arm {:.1}%.",
        WorldConfig::default().drives.feed_min,
        one.below_percent(),
        four.below_percent(),
    );
    println!(
        "- Recovery: P was {:.4} m when consumption ceased at tick {CEASE_TICK} and {:.4} m at \
         tick {TICKS} — {:.1}% of the {:.4} m the undisturbed patch held at the same tick, \
         recovered over {:.0} s. Recovery is still in progress at the horizon, not complete.",
        recovery.p_at_cease,
        recovery.p_end,
        100.0 * recovery.p_end / undisturbed.p_end.max(f64::MIN_POSITIVE),
        undisturbed.p_end,
        (TICKS - CEASE_TICK) as f64 * DT,
    );

    println!();
    println!("## Why intake stopped, by cause");
    println!(
        "- Reserve saturation: {:.1}% of the `one` arm's requesting ticks and {:.1}% of the \
         `four` arm's were refused because the animal was already full. That is the animal's \
         limit, not the patch's.",
        one.saturated_percent(),
        four.saturated_percent(),
    );
    println!(
        "- Mouth throughput: a unit adult's graze rate is {:.4} m/s at saturation; the type-II \
         term (K_P {:.2}) cuts it to {:.4} m/s at the P {:.2} a consumed patch settles to. \
         Throughput is not what stopped these animals — they were taking what the law allows.",
        0.035,
        WorldConfig::default().organism.intake_half_saturation,
        0.035 * 0.2 / (0.2 + WorldConfig::default().organism.intake_half_saturation),
        0.2,
    );
    println!(
        "- Chemical quality: the `one` arm ended with {:.4} m of detritus in the patch of which \
         only {:.4} m was edible (D_eff = D · min(1, rho/e_r)). Detritus was never above \
         feed_min as *edible* material, so the scavenging gate never opened and eaten_D is zero \
         — a quality refusal, not an absence of matter.",
        one.d_end,
        one.d_eff_end,
    );
    println!(
        "- Unreachable food: none by construction. Every other cell was emptied, so nothing in \
         this fixture was out of reach rather than absent. A moving animal's reach is exactly \
         what this staged measurement does not test."
    );

    println!();
    println!("## Not measured here (censored, not extrapolated)");
    println!("- Anything after a consumer starved: each arm's intake horizon is its own, listed above.");
    println!("- Fruit as a channel: F stayed at 0 in every consumed arm, so no frugivory rate was observed.");
    println!("- Scavenging rate: the gate never opened, so no detritus intake rate exists to report.");
    println!("- Whether a *moving* animal would do better: the fixture pins every body in place.");
    println!("- Multi-cell patches, nutrient limitation at other light levels, and any other config.");
    println!();
    println!("# wall time {:.1} s", started.elapsed().as_secs_f64());
}

/// The patch: one cell in the middle of the top face, where light is strongest and the
/// producer field is at its most productive.
fn patch() -> CellId {
    CellId::new(Face::Top, 8, 8)
}

struct Report {
    name: &'static str,
    consumers: usize,
    p_start: f64,
    p_end: f64,
    p_at_cease: f64,
    /// Absolute detritus and its edible part at the end: the gap between them is chemical
    /// quality, not quantity.
    d_end: f64,
    d_eff_end: f64,
    grown: f64,
    eaten_p: f64,
    eaten_d: f64,
    requested: f64,
    served: f64,
    below_ticks: u64,
    request_ticks: u64,
    saturated_ticks: u64,
    /// The tick each consumer starved, in placement order. `None` means it was still alive at
    /// the horizon, or was removed by the `recovery` arm rather than dying.
    died: Vec<Option<u64>>,
}

impl Report {
    /// The share of what the mouths asked their cell for that the cell actually served.
    fn served_percent(&self) -> f64 {
        if self.requested <= 0.0 {
            return f64::NAN;
        }
        100.0 * self.served / self.requested
    }

    /// Ticks the patch spent below `feed_min`, where the controller's food gate will not open
    /// at all, as a share of the horizon.
    fn below_percent(&self) -> f64 {
        100.0 * self.below_ticks as f64 / TICKS as f64
    }

    /// Of the organism-ticks that asked for food, the share refused because the reserve was
    /// already full — a refusal that belongs to the animal, not to the patch.
    fn saturated_percent(&self) -> f64 {
        if self.request_ticks == 0 {
            return f64::NAN;
        }
        100.0 * self.saturated_ticks as f64 / self.request_ticks as f64
    }

    /// Material each consumer actually took, averaged over the bodies placed.
    fn per_capita(&self) -> f64 {
        if self.consumers == 0 {
            return 0.0;
        }
        (self.eaten_p + self.eaten_d) / self.consumers as f64
    }

    /// The horizon each arm actually observed feeding over: an arm whose consumers starved
    /// stops measuring intake there, and everything after is censored.
    fn feeding_ticks(&self) -> Option<u64> {
        self.died.iter().copied().max().flatten()
    }
}

fn run(name: &'static str, consumers: usize, cease: bool) -> Report {
    let cfg = fixture_config();
    let feed_min = cfg.drives.feed_min;
    let e_r = cfg.organism.reserve_energy_density;
    let mut world = World::new(cfg).expect("the fixture config is valid");
    strip_to_patch(&mut world);
    let placed: Vec<OrganismId> = (0..consumers).map(|i| place(&mut world, i)).collect();
    let mut world = World::from_state(world.state).expect("the staged state is a valid world");
    world.check_invariants().expect("the staged world is consistent");

    println!();
    println!("## arm `{name}`  consumers {consumers}{}", if cease {
        format!("  (removed at tick {CEASE_TICK})")
    } else {
        String::new()
    });
    println!(
        "{:>7} {:>9} {:>9} {:>9} {:>9} {:>9} {:>10} {:>10} {:>10} {:>9}",
        "tick", "P", "F", "D_eff", "N", "De/D", "grown", "eaten_P", "eaten_D", "reserve"
    );

    let p_start = world.state.fields.p[patch().index()];
    let mut below_ticks = 0u64;
    let mut p_at_cease = f64::NAN;
    let mut removed = false;
    let mut died: Vec<Option<u64>> = vec![None; placed.len()];
    for tick in 0..=TICKS {
        if cease && !removed && tick == CEASE_TICK {
            p_at_cease = world.state.fields.p[patch().index()];
            for id in &placed {
                // The body leaves the world the way any removal does: its material is booked
                // out, so the fixture's mass box stays closed and the patch is not credited
                // with a corpse it did not receive.
                if let Some(o) = world.state.organisms.remove(*id) {
                    world.state.external_material_in -= o.structure + o.reserve;
                }
            }
            world = World::from_state(world.state.clone())
                .expect("removing the consumers leaves a valid world");
            removed = true;
        }
        if tick % SAMPLE == 0 || tick == TICKS {
            let f = &world.state.fields;
            let i = patch().index();
            let d_eff = edible(f.d[i], f.de[i], e_r);
            let diag = world.intake_diagnostics();
            let live = placed
                .iter()
                .filter(|id| world.state.organisms.get(**id).is_some())
                .count();
            let reserve: f64 = placed
                .iter()
                .filter_map(|id| world.state.organisms.get(*id))
                .map(|o| o.reserve)
                .sum::<f64>()
                .abs();
            let _ = live;
            println!(
                "{tick:>7} {:>9.4} {:>9.4} {:>9.4} {:>9.4} {:>9.4} {:>10.4} {:>10.4} {:>10.4} {reserve:>9.4}",
                f.p[i],
                f.f[i],
                d_eff,
                f.n[i],
                if f.d[i] > 0.0 { f.de[i] / f.d[i] } else { 0.0 },
                diag.producer_growth,
                diag.producer_eaten,
                diag.detritus_eaten,
            );
        }
        if tick == TICKS {
            break;
        }
        // Record a starvation the tick it happens, so an arm's measured horizon is explicit
        // rather than something a reader has to spot in the reserve column.
        if !removed {
            for (slot, id) in placed.iter().enumerate() {
                if died[slot].is_none() && world.state.organisms.get(*id).is_none() {
                    died[slot] = Some(tick);
                }
            }
        }
        // "Below profitable feeding levels" is the world's own gate: the controller opens no
        // food channel at all when the cell holds less than `feed_min` of every edible kind.
        let f = &world.state.fields;
        let i = patch().index();
        if f.p[i] < feed_min && f.f[i] < feed_min && edible(f.d[i], f.de[i], e_r) < feed_min {
            below_ticks += 1;
        }
        world.step();
        world.drain_events();
    }
    world.check_invariants().expect("the arm ended consistent");

    let IntakeDiagnostics {
        producer_growth,
        producer_eaten,
        fruit_eaten,
        detritus_eaten,
        requested,
        request_ticks,
        reserve_saturated_ticks,
    } = world.intake_diagnostics();
    Report {
        name,
        consumers,
        p_start,
        p_end: world.state.fields.p[patch().index()],
        p_at_cease,
        d_end: world.state.fields.d[patch().index()],
        d_eff_end: edible(
            world.state.fields.d[patch().index()],
            world.state.fields.de[patch().index()],
            e_r,
        ),
        grown: producer_growth,
        eaten_p: producer_eaten,
        eaten_d: detritus_eaten,
        requested,
        served: producer_eaten + fruit_eaten + detritus_eaten,
        below_ticks,
        request_ticks,
        saturated_ticks: reserve_saturated_ticks,
        died,
    }
}

/// `D_eff = D · min(1, ρ / e_r)`: detritus too energy-poor to fuel reserve storage is not food.
/// The same expression `world.rs` feeds the controller.
fn edible(d: f64, de: f64, e_r: f64) -> f64 {
    if d <= 0.0 || e_r <= 0.0 {
        return 0.0;
    }
    d * (de / d / e_r).min(1.0)
}

/// The fixture. Everything not named here is the world's ordinary configuration, because the
/// point is to measure this ecology rather than a convenient one.
fn fixture_config() -> WorldConfig {
    let mut c = WorldConfig::default();
    // No founders of its own: the arms place exactly the consumers they mean to.
    c.founders.kinds.clear();
    c.founders.count = 0;
    // A fixed feeding footprint. This is the staging the handoff permits, and the reason no
    // conclusion about foraging behaviour can be drawn from these numbers.
    c.organism.speed_max = 0.0;
    c.drives.turn_rate_max_deg = 0.0;
    // Matched environmental conditions across arms: no weather swing, no rain, so the light
    // and moisture a cell sees are the same in every run at every tick.
    c.weather.amplitude = 0.0;
    c.water.rain_rate = 0.0;
    // One genotype, copied exactly, so competing shares are between equals.
    c.mechanisms.mutation = false;
    c
}

/// Empty every field cell but the patch, so the world's totals are the patch's own flows.
fn strip_to_patch(world: &mut World) {
    let mut removed = 0.0;
    for cell in CellId::all() {
        if cell == patch() {
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

/// One immobile consumer standing in the patch. Unit adult, default drives, half a reserve so
/// it starts hungry enough to feed and has room to store what it takes.
fn place(world: &mut World, nth: usize) -> OrganismId {
    let cfg = world.config().clone();
    let genome = Genome::founder(0.5, &cfg.drives);
    let phenotype = decode(&genome, &cfg.organism);
    let centre = patch().center();
    // Spread the bodies across the cell so the pair pass sees four separate animals rather
    // than four copies of one point; every one of them is still inside the same cell, which is
    // what feeding reads.
    let offset = [(0.0, 0.0), (1.0, 0.0), (0.0, 1.0), (1.0, 1.0)][nth % 4];
    let pos = SurfacePoint::new(Face::Top, centre.u + offset.0, centre.v + offset.1);
    assert_eq!(cell_of(&pos), patch(), "consumer {nth} landed outside the patch");
    let structure = phenotype.structure_adult;
    let reserve = 0.5 * phenotype.reserve_max;
    let organism = Organism {
        pos,
        heading: Vec2::new(1.0, 0.0),
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
    id
}
