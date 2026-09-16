use std::f64::consts::TAU;

use cubarium_surface::{Vec2, cell_of, travel_into, unfold_with};

use crate::accounting::{self, EnergyCorrection};
use crate::care::{self};
use crate::config::WorldConfig;
use crate::controller::{Decision, Observation, TurnGate, decide_quiet, turn_toward};
use crate::dormancy::{
    ApexDormancyEvent, EMERGENCE_ENERGY_FRACTION, EMERGENCE_RESERVE_FRACTION,
    MAINTENANCE_PER_STRUCTURE_SECOND, PREY_RADIUS_PX, PREY_REQUIRED, RECHECK_TICKS, SUSTAIN_TICKS,
};
use crate::encounter::{
    self, ApexContribution, ApexEncounterEvent, CombatResponse, PairedGestation, PairedParentage,
};
use crate::events::LifeEvent;
use crate::genome::decode;
use crate::hunter::{self, AttemptOutcome, HunterEvent, HunterPhase};
use crate::ids::{OrganismId, Slots};
use crate::motor::{self, MotorBill, MotorLimits, MotorRequest};
use crate::neural;
use crate::organism::{DeathCause, Escrow, Mode, Organism, Origin};
use crate::pairs::Body;
use crate::quiet::{QuietEvent, QuietOverride, QuietPause, QuietReason, QuietState};
use crate::rng::{Counter, Stream, draw, normal, unit};
use crate::water;
use crate::{DT, pairs};

use super::*;

use super::budget;
use super::invariants::edible_detritus;

/// `D_eff + C_eff` for one cell: the edible portion of the litter plus the edible portion of
/// the animal remains (`design/ecology-v1-contract.md` §3.1, §6.2, §9). Each stock has its
/// own density and its own `min(1, ρ/e_r)` factor, so a cell full of energy-poor litter beside
/// a fresh carcass reads as the carcass, which is what a scavenge bite would actually get.
pub(crate) fn edible_here(
    fields: &crate::fields::Fields,
    eco: &crate::fields::EcologyV1State,
    cell: usize,
    e_r: f64,
) -> f64 {
    edible_detritus(fields.d[cell], fields.de[cell], e_r)
        + edible_detritus(eco.carrion[cell], eco.carrion_energy[cell], e_r)
}
// The per-tick energy and water audits are debug-only, and so is the helper they read: a
// release build has neither, and importing it unconditionally does not compile there.
#[cfg(debug_assertions)]
use super::invariants::stored_energy;
use super::lifecycle::{normalize_or_zero, sense_depth, share, ticks_from_seconds, up_direction};
use super::state::{TickCounters, WorldState, strike_closing_px};

// --- ordinary quiet (`crate::quiet`) ------------------------------------------------------
//
// Both helpers are observer-shaped: they read the organism, never charge it, and consume no RNG
// draw. They run only in a world whose policy is enabled.

/// What the controller should do with this organism's decision at boundary `now`.
///
/// Expiry and early abort both remove the entry and return an override that substitutes the
/// **underlying** ordinary mode without imposing anything, so the parent uses the ordinary
/// controller on that same tick and re-enters the hysteresis from where it really was.
fn quiet_hold(
    quiet: &mut QuietState,
    events: &mut Vec<QuietEvent>,
    id: OrganismId,
    o: &Organism,
    cfg: &crate::config::OrganismConfig,
    now: u64,
    dt: f64,
) -> Option<QuietOverride> {
    let index = quiet.pauses.iter().position(|p| p.parent == id)?;
    let p = quiet.pauses[index];
    let released = Some(QuietOverride {
        underlying: p.underlying,
        hold: false,
    });
    if !p.holds(now) {
        // The promised window is over. Release at its exact end, not one tick late.
        quiet.pauses.remove(index);
        events.push(QuietEvent::End {
            tick: now,
            parent: p.parent,
            child: p.child,
            completed_ticks: p.completed(now),
            underlying: p.underlying,
        });
        return released;
    }
    // The conservative budget is re-tested before every held decision, over the decisions that
    // remain plus one ordinary tick. A parent that can no longer cover it stops resting now.
    let abort = |events: &mut Vec<QuietEvent>, quiet: &mut QuietState, reason: QuietReason| {
        quiet.pauses.remove(index);
        events.push(QuietEvent::Abort {
            tick: now,
            parent: p.parent,
            child: p.child,
            completed_ticks: p.completed(now),
            reason,
        });
        Some(QuietOverride {
            underlying: p.underlying,
            hold: false,
        })
    };
    let Some(horizon) = crate::quiet::horizon_seconds(p.remaining(now), dt) else {
        return abort(events, quiet, QuietReason::Overflow);
    };
    let Some(budget) = crate::quiet::Budget::of(o, cfg, horizon) else {
        return abort(events, quiet, QuietReason::InvalidInputs);
    };
    if !budget.affordable(o) {
        return abort(events, quiet, QuietReason::UnaffordableRemaining);
    }
    Some(QuietOverride {
        underlying: p.underlying,
        hold: true,
    })
}

/// Offer a pause to the parent of an actual paid insertion at completed boundary `birth_tick`.
///
/// Every refusal is recorded and the opportunity is then forgotten: a hungry parent is never
/// made to wait for permission to act, and nothing is retried on a later tick.
#[allow(clippy::too_many_arguments)]
fn quiet_admit(
    quiet: &mut QuietState,
    events: &mut Vec<QuietEvent>,
    organisms: &Slots<Organism>,
    cfg: &crate::config::OrganismConfig,
    cap: usize,
    birth_tick: u64,
    dt: f64,
    parent: OrganismId,
    child: OrganismId,
    hunter_parent: bool,
) {
    let mut refuse = |reason: QuietReason| {
        events.push(QuietEvent::Refuse {
            tick: birth_tick,
            parent,
            child,
            reason,
        });
    };
    if hunter_parent {
        // `QuietState::validate` already refuses this combination outright; the record exists so
        // a harness sees why nothing happened rather than inferring it from silence.
        return refuse(QuietReason::HunterMember);
    }
    let Some(o) = organisms.get(parent) else {
        return refuse(QuietReason::ParentGone);
    };
    if quiet.pauses.iter().any(|p| p.parent == parent) {
        return refuse(QuietReason::AlreadyPaused);
    }
    if quiet.pauses.len() >= cap {
        return refuse(QuietReason::Bounded);
    }
    let Some(end_tick) = birth_tick.checked_add(crate::quiet::POST_BIRTH_PAUSE_TICKS) else {
        return refuse(QuietReason::Overflow);
    };
    let Some(horizon) = crate::quiet::horizon_seconds(crate::quiet::POST_BIRTH_PAUSE_TICKS, dt)
    else {
        return refuse(QuietReason::Overflow);
    };
    let Some(budget) = crate::quiet::Budget::of(o, cfg, horizon) else {
        return refuse(QuietReason::InvalidInputs);
    };
    if !budget.affordable(o) {
        return refuse(QuietReason::Unaffordable);
    }
    // The mode the parent is actually in as this tick closes: the ordinary value release will
    // resume from.
    let pause = QuietPause {
        parent,
        child,
        start_tick: birth_tick,
        end_tick,
        underlying: o.mode,
    };
    let at = quiet.pauses.partition_point(|p| {
        (p.parent.slot, p.parent.generation) < (parent.slot, parent.generation)
    });
    quiet.pauses.insert(at, pause);
    events.push(QuietEvent::Begin {
        tick: birth_tick,
        parent,
        child,
        end_tick,
        underlying: pause.underlying,
    });
}

impl World {
    /// One tick in the normative order of `design/m2-world-spec.md` "Tick order".
    /// Returns the per-tick counters (also accumulated internally for telemetry).
    pub fn step(&mut self) -> &TickCounters {
        let dt = DT;
        // The audit reads the *corrected* ledgers: the identity it checks is the one the
        // world actually books, and over a long run the raw counters no longer are.
        #[cfg(debug_assertions)]
        let audit = (
            stored_energy(&self.state),
            self.state.energy_ledgers(),
            self.state.care.feed_energy_in,
            self.state.care.clean_energy_out,
            self.state.hunters.imported_energy(),
        );
        #[cfg(debug_assertions)]
        let water_audit = (
            self.state.fields.w.iter().sum::<f64>(),
            self.state.rain_in_total,
            self.state.evap_out_total,
        );
        {
            let World {
                state,
                graph,
                habitat,
                images,
                light,
                moisture,
                rain_source,
                rain,
                manual_rain,
                rain_envelope,
                scratch,
                water_scratch,
                sense_rings,
                neighbors,
                travel_buf,
                eco_scratch,
                moved,
                events,
                hunter_events,
                quiet_events,
                apex_dormancy_events,
                apex_encounter_events,
                counters,
                charging,
                intake,
                budgets,
                strikes,
                apex_opportunity,
                neural_timing,
                scripted,
                motor_model,
                apex_turn_radius,
                apex_motor_model,
                initial_material: _,
            } = &mut *self;
            // The motor contract in force this tick, read once. `Sweep` is the shipped
            // contract and the default (`crate::motor::MotorModel`).
            let motor_model = *motor_model;
            // And which radius an apex member's grasp puts in that contract's rotation term
            // (`crate::motor::ApexTurnRadius`). `Grasp` is the shipped rule and the default.
            let apex_turn_radius = *apex_turn_radius;
            // And the contract a *hunter member* runs when it is not the world's own
            // (`crate::motor::model_for_body`). `None` is the default and means every body runs
            // `motor_model`; an override reaches nothing without apex contact geometry.
            let apex_motor_model = *apex_motor_model;
            let WorldState {
                config,
                tick,
                fields,
                weather,
                organisms,
                births_total,
                deaths_total,
                cap_rejections_total,
                external_material_in: _,
                light_in_total,
                heat_out_total,
                rain_in_total,
                evap_out_total,
                care,
                // Destructured field by field so the heat closure and the light accumulation
                // borrow disjoint components.
                energy_correction:
                    EnergyCorrection {
                        light_in: light_in_correction,
                        heat_out: heat_out_correction,
                    },
                hunters,
                quiet,
                apex_dormancy,
                apex_encounters,
                neural,
                ecology,
            } = state;
            let cfg: &WorldConfig = config;
            // The world's shape and scale, read once: every geometric call below uses these
            // rather than assuming the cube (`design/flat-world-plan-2026-09-16.md` §2).
            let topo = cfg.topology;
            let world_scale = cfg.world_scale;
            let cell_count = graph.cell_count();
            let org_cfg = &cfg.organism;
            let e_r = org_cfg.reserve_energy_density;
            let k_p = org_cfg.intake_half_saturation;
            let e_f = cfg.fruit.energy_density;
            let e_v = cfg.plant.energy_density;
            let e_c_max = cfg.detritus.carrion_energy_cap;
            let gate = TurnGate::from_config(org_cfg);
            // One boolean, read once: an Off world never touches a quiet code path again.
            let quiet_on = quiet.active();
            // Read once. Off remains a branch-free no-op at every lifecycle mutation site.
            let apex_dormancy_on = apex_dormancy.active();
            // Independent from offspring dormancy: either policy can be screened alone.
            let apex_encounters_on = apex_encounters.active();
            let now = *tick;
            let seed = cfg.seed;
            // Every heat payment of the tick goes through here: the transient counter keeps
            // its existing arithmetic and reset semantics, and the persisted raw total gets
            // exactly the addition it always got, with the bits it drops booked into the
            // correction (`crate::accounting`).
            let mut heat = |amount: f64| {
                counters.heat_out += amount;
                accounting::accumulate(heat_out_total, heat_out_correction, amount);
            };
            // Open a per-body budget record for anything that does not have one yet, before
            // this tick mutates a single store, so a record's `start_*` is the body's opening
            // inventory and its identity closes over whole ticks (`crate::world::budget`).
            // Off by default: one boolean test and no allocation in an ordinary world.
            if budgets.enabled() {
                for (id, o) in organisms.iter() {
                    budgets.ensure(id, o, now);
                }
            }
            // Open this tick's intake row for the one traced body, before anything moves
            // (`crate::world::budget`). Off by default: one `Option` test and no allocation.
            budgets.open_row(now);
            // The raw linear head the traced body's policy produced this tick, if it ran one.
            // Written by stage 5 and read by stage 7; `None` on a tick that reused the held
            // action, and for every body that is not neural.
            let mut traced_head: Option<[f64; crate::neural::action::ACT_LEN]> = None;

            // 1. Admit due stimuli. The queue is empty in M2; the hook is the journal.

            // 2. Weather advance, then derive light, moisture and the rain source per cell.
            weather.advance(&cfg.weather, seed, now);
            weather.sample(
                &cfg.weather,
                habitat,
                light,
                moisture,
                rain_source,
                cfg.habitat.moisture_min,
            );

            // 2b. Water: rain, downhill flow, evaporation (`design/water.md`). Before the
            //     field reactions, so growth sees this tick's wetness and flooding.
            //
            //     Manual rain (the care contract) joins the natural rate here and nowhere
            //     else, so `rain[c]` publishes what actually falls and the depth is inside
            //     `rain_in_total` once. With no shower running the water stage is handed
            //     `None` and executes the pre-care arithmetic operation for operation.
            let manual: Option<&[f64]> = if care.showers.is_empty() {
                None
            } else {
                manual_rain.fill(0.0);
                for shower in care.showers.iter() {
                    let k = shower.delivered as usize;
                    let Some(&e) = rain_envelope.get(k) else {
                        continue;
                    };
                    // The shower's own persisted dose, not whatever a panel now offers.
                    let depth = shower.dose().scale(care::RAIN_DEPTH_TOTAL);
                    for (c, w) in shower.cells.iter().zip(shower.weights.iter()) {
                        manual_rain[usize::from(*c)] += depth * w * e;
                    }
                }
                Some(&**manual_rain)
            };
            let water_ledger = water::step(
                &mut fields.w,
                &cfg.water,
                water::Drivers {
                    terrain: &habitat.terrain,
                    light,
                    rain_source,
                },
                manual,
                rain,
                graph,
                water_scratch,
            );
            counters.rain_in += water_ledger.rain_in;
            counters.evap_out += water_ledger.evap_out;
            *rain_in_total += water_ledger.rain_in;
            *evap_out_total += water_ledger.evap_out;
            if !care.showers.is_empty() {
                // Book the manual share of the depth that actually landed, then retire the
                // shower once its 120th sample has been delivered.
                care.rain_depth_in += water_ledger.manual_in;
                for shower in care.showers.iter_mut() {
                    shower.delivered += 1;
                }
                care.showers.retain(|s| s.delivered < care::RAIN_TICKS);
            }

            // 3. Field reactions: the eight subphases 3a-3h of
            //    `design/ecology-v1-contract.md` §4.0, in `Fields::react`.
            let ledger = fields.react(ecology, cfg, light, moisture, graph, scratch, eco_scratch);
            intake.producer_growth += ledger.producer_growth;
            intake.plant_income += ledger.plant_income;
            intake.plant_maintenance_unpaid += ledger.plant_maintenance_unpaid;
            intake.propagule_sent += ledger.propagule_sent;
            counters.light_in += ledger.light_in;
            accounting::accumulate(light_in_total, light_in_correction, ledger.light_in);
            heat(ledger.heat_out);

            // 4. Pair pass.
            let mut bodies: Vec<Body> = Vec::with_capacity(organisms.len());
            for (id, o) in organisms.iter() {
                if apex_dormancy_on && apex_dormancy.contains(id) {
                    continue;
                }
                bodies.push(Body {
                    id,
                    pos: o.pos,
                    sense_radius: o.phenotype.sense_radius,
                    extent: o.phenotype.extent,
                });
            }
            pairs::build(
                topo,
                &bodies,
                images,
                cfg.capacity.max_neighbors as usize,
                neighbors,
            );

            // 5. Observe and decide (pure per organism, from the pre-movement world).
            let mut decisions: Vec<(OrganismId, Decision)> = Vec::with_capacity(organisms.len());
            // Scratch for the recurrent sampler, reused across animals so a neural world does
            // not allocate per body per tick. Empty and untouched in every legacy world.
            let mut sensed_cells: Vec<neural::SensedCell> = Vec::new();
            let mut sensed_bodies: Vec<neural::SensedBody> = Vec::new();
            for (id, o) in organisms.iter_mut() {
                // A concealed offspring has no surface observation, controller draw, movement,
                // intake or ordinary physiology. Its dedicated paid lifecycle runs below.
                if apex_dormancy_on && apex_dormancy.contains(id) {
                    continue;
                }
                // **Per-animal dispatch.** An animal is neural exactly when the extension
                // holds an entry for its full id. It runs the contract's own pipeline and
                // never touches the legacy controller: no mode hysteresis, no hunger memory,
                // no steering weights, no OU draw, no turn gate, no `feed_min`, no quiet hold.
                // The world's physics and physiology below are untouched.
                if !neural.animals.is_empty()
                    && let Some(index) = neural.index_of(id)
                {
                    let decision = neural_decision(
                        neural,
                        neural_timing,
                        index,
                        o,
                        now,
                        dt,
                        cfg,
                        e_r,
                        fields,
                        ecology,
                        light,
                        images,
                        &sense_rings[cell_of(topo, world_scale, &o.pos).index()],
                        neighbors.lists.get(id.slot as usize).map_or(&[][..], |l| &l[..]),
                        &mut sensed_cells,
                        &mut sensed_bodies,
                        if budgets.traced() == Some(id) { Some(&mut traced_head) } else { None },
                        motor_model,
                    );
                    decisions.push((id, decision));
                    continue;
                }
                let cell = cell_of(topo, world_scale, &o.pos);
                let here = cell.index();
                let chart = o.pos.chart();
                let mut obs = Observation {
                    p_here: fields.p[here],
                    f_here: fields.f[here],
                    // `D_eff + C_eff` (§9): litter and remains are one detrital channel to a
                    // mouth and to an eye, each contributing only its own edible portion.
                    d_here: edible_here(fields, ecology, here, e_r),
                    height: topo.height(&o.pos),
                    up: up_direction(topo, o.pos.face),
                    ..Observation::default()
                };
                // Sensing reaches `ceil(r_sense / 4)` graph hops (`design/fauna-v2.md`): each
                // sensed cell contributes its finite-difference slope toward it.
                let depth = sense_depth(o.phenotype.sense_radius);
                for ring in &sense_rings[here][..depth] {
                    for neighbor in ring {
                        let center = neighbor.center(topo, world_scale);
                        let Some(view) = unfold_with(topo, 
                            &images[topo.chart_index(o.pos.face)],
                            o.pos,
                            center,
                            CELL_UNFOLD_RADIUS,
                        ) else {
                            continue;
                        };
                        let Some(dir) = (view.local - chart).normalized() else {
                            continue;
                        };
                        if view.distance <= GRADIENT_EPS || view.distance.is_nan() {
                            continue;
                        }
                        let slope = dir * (1.0 / view.distance);
                        let there = neighbor.index();
                        let edible = edible_here(fields, ecology, there, e_r);
                        obs.grad_p += slope * (fields.p[there] - obs.p_here);
                        obs.grad_f += slope * (fields.f[there] - obs.f_here);
                        obs.grad_d += slope * (edible - obs.d_here);
                    }
                }
                obs.grad_p = normalize_or_zero(obs.grad_p);
                obs.grad_f = normalize_or_zero(obs.grad_f);
                obs.grad_d = normalize_or_zero(obs.grad_d);

                if let Some(list) = neighbors.lists.get(id.slot as usize) {
                    for n in list {
                        let extent_sum = o.phenotype.extent + n.extent;
                        if n.distance >= extent_sum + 1.0 {
                            continue;
                        }
                        let delta = chart - n.local;
                        let len_sq = delta.length_sq();
                        if len_sq > GRADIENT_EPS {
                            obs.repulsion += delta * (extent_sum / len_sq);
                        }
                    }
                }

                // Two standard normals cost two counters each.
                let cx = o.turn_counter.take();
                let _ = o.turn_counter.take();
                let cy = o.turn_counter.take();
                let _ = o.turn_counter.take();
                let key = u64::from(id.slot);
                obs.noise = Vec2::new(
                    normal(seed, Stream::OrganismTurn, key, cx),
                    normal(seed, Stream::OrganismTurn, key, cy),
                );

                // The ordinary-quiet override (`crate::quiet`). `None` in every Off world,
                // and `decide_quiet(.., None)` is `decide` expression for expression, so the
                // reference trajectory is untouched. The noise pair above is drawn either way.
                let quiet_override = if quiet_on {
                    quiet_hold(quiet, quiet_events, id, o, org_cfg, now, dt)
                } else {
                    None
                };
                let decision = decide_quiet(
                    o,
                    &obs,
                    now,
                    dt,
                    cfg.mechanisms.grazing,
                    cfg.mechanisms.scavenging,
                    gate,
                    quiet_override,
                );
                // Carry the ordinary mode forward, once, so release resumes the real hysteresis
                // rather than the imposed Resting.
                if quiet_override.is_some_and(|q| q.hold)
                    && let Some(p) = quiet.pauses.iter_mut().find(|p| p.parent == id)
                {
                    p.underlying = decision.underlying_mode;
                }
                decisions.push((id, decision));
            }

            // 5a. Transient diagnostic intent overrides (`crate::diagnostic`). Empty in every
            //     ordinary world — the world never sets one — so this is a single `is_empty`
            //     test and the reference trajectory is untouched. A script states an *intent*
            //     and nothing more: the resolver below still bounds it by `|v| + r · |ω| ≤ u`,
            //     the intake pass still applies the cell's share, the type-II term and the
            //     reserve headroom, and gestation still faces capacity and funding. Running it
            //     here, before the hunter, escape and encounter passes, keeps a legitimate
            //     override winning over a script exactly as it wins over the controller.
            if !scripted.is_empty() {
                for (id, d) in &mut decisions {
                    if let Some((_, intent)) = scripted.iter().find(|(s, _)| s == id) {
                        intent.apply(d);
                    }
                }
            }

            // 5b. Hunters (`crate::hunter`), when any member exists: advance each member's
            //     local phase from the pre-movement world, charge a strike in full at its
            //     entry, and turn the phase into a movement intent. Prey that actually senses
            //     a hunter pursuing *it* gets an escape term. Both are skipped whole when the
            //     extension is empty, so a world without hunters runs the pre-hunter tick
            //     operation for operation, with no additional draws.
            //
            //     A boost is an absolute speed ceiling in px/s, still divided by wading and
            //     still capped by the movement energy the creature actually has.
            let mut boosts: Vec<(OrganismId, f64)> = Vec::new();

            // 5a. Adult apex encounters. Work from canonical unordered neighbor pairs and mark
            // both participants used before applying an action: no self-pair, reverse duplicate,
            // distant mating, or second partner reuse can occur in this tick. The ordinary
            // hunter reproduction stream remains strictly one-parent; joint transactions are
            // published only through `ApexEncounterEvent`.
            if apex_encounters_on
                && !hunters.members.is_empty()
                && let Some(profile) = hunters.profile.clone()
            {
                let mut pairs = Vec::new();
                for m in &hunters.members {
                    if m.phase != HunterPhase::Perched
                        || (apex_dormancy_on && apex_dormancy.contains(m.id))
                        || apex_encounters
                            .gestations
                            .iter()
                            .any(|g| g.carrier == m.id || g.partner == m.id)
                    {
                        continue;
                    }
                    let Some(a) = organisms.get(m.id) else {
                        continue;
                    };
                    if a.structure < a.phenotype.structure_adult - hunter::TOLERANCE {
                        continue;
                    }
                    if let Some(sensed) = neighbors.lists.get(m.id.slot as usize) {
                        for n in sensed {
                            if m.id >= n.id || !hunters.contains(n.id) {
                                continue;
                            }
                            let Some(other) = hunters.member(n.id) else {
                                continue;
                            };
                            if other.phase != HunterPhase::Perched
                                || (apex_dormancy_on && apex_dormancy.contains(n.id))
                                || apex_encounters
                                    .gestations
                                    .iter()
                                    .any(|g| g.carrier == n.id || g.partner == n.id)
                            {
                                continue;
                            }
                            let Some(b) = organisms.get(n.id) else {
                                continue;
                            };
                            if b.structure < b.phenotype.structure_adult - hunter::TOLERANCE {
                                continue;
                            }
                            pairs.push((n.distance, m.id, n.id));
                        }
                    }
                }
                pairs.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)).then(a.2.cmp(&b.2)));
                // **The opportunity census** (`crate::encounter::ApexOpportunity`). Counted
                // over every member the world holds, not over the candidate list above, so a
                // world where two adults were never ready at the same instant is
                // distinguishable from one where they were ready and never met. Reads only
                // values the pass already reads and writes nothing the tick consults.
                {
                    let mut alive = 0u32;
                    let mut adults = 0u32;
                    let mut perched = 0u32;
                    let mut ready_ids: Vec<OrganismId> = Vec::new();
                    for (index, m) in hunters.members.iter().enumerate() {
                        let Some(o) = organisms.get(m.id) else { continue };
                        alive += 1;
                        let dormant = apex_dormancy_on && apex_dormancy.contains(m.id);
                        let adult =
                            o.structure >= o.phenotype.structure_adult - hunter::TOLERANCE;
                        if dormant || !adult {
                            continue;
                        }
                        adults += 1;
                        if m.phase != HunterPhase::Perched {
                            continue;
                        }
                        perched += 1;
                        let committed = apex_encounters
                            .gestations
                            .iter()
                            .any(|g| g.carrier == m.id || g.partner == m.id);
                        if !committed
                            && hunter::may_reproduce(&profile, o, &hunters.members[index], now, dt)
                        {
                            ready_ids.push(m.id);
                        }
                    }
                    let mut closest: Option<f64> = None;
                    for (i, a) in ready_ids.iter().enumerate() {
                        for b in &ready_ids[i + 1..] {
                            let (Some(oa), Some(ob)) = (organisms.get(*a), organisms.get(*b))
                            else {
                                continue;
                            };
                            if let Some(d) = cubarium_surface::surface_distance(topo, 
                                oa.pos,
                                ob.pos,
                                cubarium_surface::MAX_LOCAL_RADIUS,
                            ) {
                                closest = Some(closest.map_or(d, |m: f64| m.min(d)));
                            }
                        }
                    }
                    apex_opportunity.observe(
                        alive,
                        adults,
                        perched,
                        u32::try_from(ready_ids.len()).unwrap_or(u32::MAX),
                        closest,
                    );
                }
                let mut used: Vec<OrganismId> = Vec::new();
                let gestation_ticks = ticks_from_seconds(profile.gestation_seconds, dt);
                let interval_ticks = ticks_from_seconds(profile.reproduce_interval_seconds, dt);
                let encounter_cooldown = gestation_ticks.saturating_add(interval_ticks);
                let recovery_ticks = ticks_from_seconds(profile.recovery_seconds, dt).max(1);

                for (distance, a_id, b_id) in pairs {
                    if used.contains(&a_id) || used.contains(&b_id) {
                        apex_opportunity.candidate(encounter::PairOutcome::PartnerUsed);
                        continue;
                    }
                    let (Some(a_index), Some(b_index)) =
                        (hunters.index_of(a_id), hunters.index_of(b_id))
                    else {
                        continue;
                    };
                    let (Some(a), Some(b)) = (organisms.get(a_id), organisms.get(b_id)) else {
                        continue;
                    };
                    let a_ready = hunters.members[a_index].phase == HunterPhase::Perched
                        && hunter::may_reproduce(&profile, a, &hunters.members[a_index], now, dt)
                        && !apex_encounters.gestations.iter().any(|g| g.partner == a_id);
                    let b_ready = hunters.members[b_index].phase == HunterPhase::Perched
                        && hunter::may_reproduce(&profile, b, &hunters.members[b_index], now, dt)
                        && !apex_encounters.gestations.iter().any(|g| g.partner == b_id);

                    // The first of the four cheap terms that fails, in the `if`'s own
                    // evaluation order. The remaining two outcomes — the contribution, and a
                    // conception — are recorded inside the branch, where the world computes
                    // them; nothing here re-derives a due or draws anything.
                    if let Some(blocked) = if distance > encounter::MATING_RADIUS_PX {
                        Some(encounter::PairOutcome::Radius)
                    } else if !a_ready {
                        Some(encounter::PairOutcome::ReadyA)
                    } else if !b_ready {
                        Some(encounter::PairOutcome::ReadyB)
                    } else if organisms.len() >= cfg.capacity.max_organisms as usize {
                        Some(encounter::PairOutcome::Capacity)
                    } else {
                        None
                    } {
                        apex_opportunity.candidate(blocked);
                    }
                    if distance <= encounter::MATING_RADIUS_PX
                        && a_ready
                        && b_ready
                        && organisms.len() < cfg.capacity.max_organisms as usize
                    {
                        let genome = encounter::recombine(&a.genome, &b.genome, a_id, b_id);
                        let phenotype = decode(&genome, org_cfg);
                        let structure =
                            org_cfg.child_structure_fraction * phenotype.structure_adult;
                        let reserve = org_cfg.child_reserve_fraction * phenotype.reserve_max;
                        let energy = org_cfg.child_energy_fraction * phenotype.energy_max;
                        let build = org_cfg.build_cost * structure;
                        let paid = ApexContribution {
                            structure: structure * 0.5,
                            reserve: reserve * 0.5,
                            energy: energy * 0.5,
                            build_heat: build * 0.5,
                        };
                        let material_due = paid.material();
                        let energy_due = paid.energy + paid.build_heat;
                        if a.reserve >= material_due
                            && b.reserve >= material_due
                            && a.energy >= energy_due
                            && b.energy >= energy_due
                        {
                            apex_opportunity.candidate(encounter::PairOutcome::Mated);
                            // End the immutable reads before mutating the two distinct slots.
                            let child_genome = genome.digest();
                            let a = organisms
                                .get_mut(a_id)
                                .expect("paired adult remained alive");
                            a.reserve -= material_due;
                            a.energy -= energy_due;
                            if let Some(rec) = budgets.at(a_id) {
                                rec.reproduction_material += material_due;
                                rec.reproduction_energy += energy_due;
                            }
                            a.escrow = Some(Escrow {
                                structure,
                                reserve,
                                energy,
                                started_tick: now,
                                genome,
                            });
                            let b = organisms
                                .get_mut(b_id)
                                .expect("paired adult remained alive");
                            b.reserve -= material_due;
                            b.energy -= energy_due;
                            if let Some(rec) = budgets.at(b_id) {
                                rec.reproduction_material += material_due;
                                rec.reproduction_energy += energy_due;
                            }
                            hunters.members[a_index].next_reproduction_tick =
                                now.saturating_add(encounter_cooldown);
                            hunters.members[b_index].next_reproduction_tick =
                                now.saturating_add(encounter_cooldown);
                            let record = PairedGestation {
                                carrier: a_id,
                                partner: b_id,
                                started_tick: now,
                                carrier_paid: paid,
                                partner_paid: paid,
                            };
                            assert!(apex_encounters.insert_gestation(record));
                            apex_encounters.matings_total += 1;
                            heat(build);
                            apex_encounter_events.push(ApexEncounterEvent::Mated {
                                tick: now + 1,
                                carrier: a_id,
                                partner: b_id,
                                carrier_paid: paid,
                                partner_paid: paid,
                                child_genome,
                            });
                            used.extend([a_id, b_id]);
                            continue;
                        }
                        apex_opportunity.candidate(encounter::PairOutcome::Contribution);
                    }

                    // A failed mutual funding is not a free attack. Combat is an alternative
                    // only when mating itself was unavailable and the profile permits attacks.
                    if distance > encounter::COMBAT_RADIUS_PX
                        || !profile.attacks_enabled
                        || (a_ready && b_ready)
                    {
                        continue;
                    }
                    let a_hungry =
                        a.reserve < profile.seek_reserve_fraction * a.phenotype.reserve_max;
                    let b_hungry =
                        b.reserve < profile.seek_reserve_fraction * b.phenotype.reserve_max;
                    let (attacker, defender, attacker_index, defender_index) =
                        if a_hungry || (!b_hungry && !a_ready) {
                            (a_id, b_id, a_index, b_index)
                        } else {
                            (b_id, a_id, b_index, a_index)
                        };
                    let Some(attacker_o) = organisms.get(attacker) else {
                        continue;
                    };
                    if attacker_o.energy < profile.strike_energy_cost {
                        continue;
                    }
                    let defender_o = organisms.get(defender).expect("paired defender is live");
                    let retreat_cost = profile.strike_energy_cost * 0.5;
                    let retreats = defender_o.structure < attacker_o.structure
                        && defender_o.energy >= retreat_cost;
                    let retaliates = !retreats && defender_o.energy >= profile.strike_energy_cost;
                    let attack_injury =
                        encounter::INJURY_ADULT_FRACTION * attacker_o.phenotype.structure_adult;
                    let defend_injury =
                        encounter::INJURY_ADULT_FRACTION * defender_o.phenotype.structure_adult;
                    let mut attacker_injury = 0.0;
                    let mut defender_injury = 0.0;
                    let defender_energy_paid;
                    let response;

                    {
                        let o = organisms
                            .get_mut(attacker)
                            .expect("attacker remained alive");
                        o.energy -= profile.strike_energy_cost;
                    }
                    if let Some(rec) = budgets.at(attacker) {
                        rec.other_energy_paid += profile.strike_energy_cost;
                    }
                    heat(profile.strike_energy_cost);
                    if retreats {
                        let o = organisms
                            .get_mut(defender)
                            .expect("defender remained alive");
                        o.energy -= retreat_cost;
                        if let Some(rec) = budgets.at(defender) {
                            rec.other_energy_paid += retreat_cost;
                        }
                        heat(retreat_cost);
                        defender_energy_paid = retreat_cost;
                        response = CombatResponse::Retreated;
                        apex_encounters.retreats_total += 1;
                        if let Some(n) = neighbors
                            .lists
                            .get(defender.slot as usize)
                            .and_then(|list| list.iter().find(|n| n.id == attacker))
                            && let Some(d) = decisions.iter_mut().find(|(id, _)| *id == defender)
                        {
                            // A *request* to face away from the attacker. It is resolved,
                            // paid for and bounded by the same envelope as every other
                            // motion in section 6; a retreat is not a free instant turn.
                            d.1.heading = (organisms.get(defender).expect("defender").pos.chart()
                                - n.local)
                                .normalized()
                                .unwrap_or(d.1.heading);
                            d.1.turn_rate_max =
                                d.1.turn_rate_max
                                    .max(profile.escape_turn_rate_deg.to_radians());
                            boosts.push((
                                defender,
                                profile.escape_speed_multiple
                                    * organisms
                                        .get(defender)
                                        .expect("defender")
                                        .phenotype
                                        .speed_max,
                            ));
                        }
                    } else {
                        let o = organisms
                            .get_mut(defender)
                            .expect("defender remained alive");
                        defender_injury = defend_injury.min(o.structure);
                        o.structure -= defender_injury;
                        fields.d[cell_of(topo, world_scale, &o.pos).index()] += defender_injury;
                        if retaliates {
                            o.energy -= profile.strike_energy_cost;
                            heat(profile.strike_energy_cost);
                            defender_energy_paid = profile.strike_energy_cost;
                            response = CombatResponse::Retaliated;
                            apex_encounters.retaliations_total += 1;
                            let o = organisms
                                .get_mut(attacker)
                                .expect("attacker remained alive");
                            attacker_injury = attack_injury.min(o.structure);
                            o.structure -= attacker_injury;
                            fields.d[cell_of(topo, world_scale, &o.pos).index()] += attacker_injury;
                            if let Some(rec) = budgets.at(attacker) {
                                rec.injury_structure += attacker_injury;
                            }
                            if let Some(rec) = budgets.at(defender) {
                                rec.other_energy_paid += profile.strike_energy_cost;
                            }
                        } else {
                            defender_energy_paid = 0.0;
                            response = CombatResponse::Injured;
                        }
                        if let Some(rec) = budgets.at(defender) {
                            rec.injury_structure += defender_injury;
                        }
                    }
                    hunters.members[attacker_index].enter(
                        HunterPhase::Recovering,
                        now,
                        now + recovery_ticks,
                        0,
                    );
                    hunters.members[defender_index].enter(
                        HunterPhase::Recovering,
                        now,
                        now + recovery_ticks,
                        0,
                    );
                    apex_encounters.combats_total += 1;
                    apex_encounters.injury_material_total += attacker_injury + defender_injury;
                    apex_encounter_events.push(ApexEncounterEvent::Combat {
                        tick: now + 1,
                        attacker,
                        defender,
                        response,
                        attacker_energy_paid: profile.strike_energy_cost,
                        defender_energy_paid,
                        attacker_injury,
                        defender_injury,
                    });
                    used.extend([a_id, b_id]);
                }
            }
            if !hunters.members.is_empty()
                && let Some(profile) = hunters.profile.as_ref()
            {
                let windup_ticks = ticks_from_seconds(profile.windup_seconds, dt).max(1);
                let strike_ticks = ticks_from_seconds(profile.strike_seconds, dt).max(1);
                let _recovery_ticks = ticks_from_seconds(profile.recovery_seconds, dt).max(1);
                let meal_ticks = ticks_from_seconds(profile.meal_recovery_seconds, dt).max(1);
                let stalk_ticks = ticks_from_seconds(profile.stalk_timeout_seconds, dt).max(1);
                let mut charges: Vec<(OrganismId, f64)> = Vec::new();
                let mut threats: Vec<(OrganismId, OrganismId)> = Vec::new();

                for index in 0..hunters.members.len() {
                    // The record is `Copy`: the whole phase machine runs on this copy, so the
                    // arena and the member list can be read freely, and only the finished
                    // member is written back.
                    let mut m = hunters.members[index];
                    if apex_dormancy_on && apex_dormancy.contains(m.id) {
                        continue;
                    }
                    let Some(o) = organisms.get(m.id) else {
                        continue;
                    };
                    let headroom = profile.gut_capacity_material - m.gut_material;
                    let empty: &[pairs::Neighbor] = &[];
                    let sensed = neighbors
                        .lists
                        .get(m.id.slot as usize)
                        .map_or(empty, |l| l.as_slice());
                    let eligible = |id: OrganismId| -> bool {
                        !hunters.contains(id)
                            && organisms.get(id).is_some_and(|prey| {
                                hunter::prey_is_eligible(profile, o, prey, headroom, e_r)
                            })
                    };

                    // A target that died, had its slot reused, became a hunter, grew out of
                    // the window or no longer fits the gut is dropped deterministically. A
                    // strike that has already been paid for keeps its (now empty) handle and
                    // resolves as a failed attempt after movement.
                    if let Some(t) = m.target
                        && !eligible(t)
                    {
                        m.target = None;
                        if matches!(m.phase, HunterPhase::Stalking | HunterPhase::Windup) {
                            m.enter(HunterPhase::Perched, now, now, 0);
                        }
                    }
                    // Losing local sensing of the target ends a stalk or a windup too.
                    if matches!(m.phase, HunterPhase::Stalking | HunterPhase::Windup)
                        && m.target.is_some_and(|t| !sensed.iter().any(|n| n.id == t))
                    {
                        m.enter(HunterPhase::Perched, now, now, 0);
                    }

                    // Timed phases expire; a finished meal becomes a pause.
                    match m.phase {
                        HunterPhase::Handling if !m.carrying() => {
                            // The meal is over: the recoil that follows is a *finished meal*,
                            // which `entered_from` now records for the adapter.
                            let episode = m.episode;
                            m.enter(HunterPhase::Recovering, now, now + meal_ticks, episode);
                        }
                        HunterPhase::Recovering if now >= m.phase_ends_tick => {
                            m.enter(HunterPhase::Perched, now, now, 0);
                        }
                        _ => {}
                    }

                    // Satiety and a carried meal both end a hunt: the hysteresis is between
                    // `seek_reserve_fraction` and `perch_reserve_fraction` of `R_max`.
                    let full = o.reserve > profile.perch_reserve_fraction * o.phenotype.reserve_max;
                    if m.phase.hunting() && (m.carrying() || full) {
                        m.enter(HunterPhase::Perched, now, now, 0);
                    }

                    let hungry =
                        o.reserve < profile.seek_reserve_fraction * o.phenotype.reserve_max;
                    let may_hunt = profile.attacks_enabled && !m.carrying() && hungry;
                    // The nearest eligible prey this hunter actually senses; the neighbour
                    // list is already sorted by `(distance, id)`, so this is "nearest, ties by
                    // full ID" and never a global population scan.
                    let nearest = || sensed.iter().find(|n| eligible(n.id)).map(|n| n.id);
                    // The geometry this member hunts with right now, at its own body scale.
                    let geometry = hunter::ContactGeometry::of(profile, o);
                    // How far from the grasp centre a prey may be and still be worth cocking
                    // at: the tolerance plus the gap the paid strike can close.
                    let admission = |prey: &Organism| -> Option<hunter::ContactMeasure> {
                        hunter::measure_contact(
                            topo,
                            images,
                            o.pos,
                            o.heading,
                            &geometry,
                            prey.pos,
                            prey.phenotype.extent,
                        )
                    };
                    let closing = strike_closing_px(profile);

                    match m.phase {
                        HunterPhase::Perched => {
                            if may_hunt && let Some(t) = nearest() {
                                m.enter(HunterPhase::Stalking, now, now, 0);
                                m.target = Some(t);
                            }
                        }
                        HunterPhase::Stalking => {
                            if now.saturating_sub(m.phase_started_tick) >= stalk_ticks {
                                m.enter(HunterPhase::Perched, now, now, 0);
                            } else if m.target.is_none() {
                                match nearest() {
                                    Some(t) => m.target = Some(t),
                                    None => m.enter(HunterPhase::Perched, now, now, 0),
                                }
                            }
                            // The gesture starts only when the prey is inside the grasp the
                            // paid strike can actually close on — measured from the hunter
                            // root in the renderer's own body basis, never from a separately
                            // transported anchor's chart.
                            if m.phase == HunterPhase::Stalking
                                && let Some(t) = m.target
                                && let Some(prey) = organisms.get(t)
                                && admission(prey)
                                    .is_some_and(|c| c.effector_distance <= c.tolerance + closing)
                            {
                                m.enter(HunterPhase::Windup, now, now + windup_ticks, 0);
                                m.target = Some(t);
                                // The intent frame: where both bodies stood when this member
                                // committed to the gesture. Inert and opt-in
                                // (`crate::hunter::StrikeRecorder`); one `bool` test otherwise.
                                if strikes.enabled() {
                                    strikes.open_intent(
                                        m.id,
                                        hunter::StrikeFrame::gather(
                                            topo,
                                            images,
                                            profile,
                                            now,
                                            o,
                                            organisms.get(t).map(|prey| (t, prey)),
                                        ),
                                    );
                                }
                            }
                        }
                        HunterPhase::Windup if now >= m.phase_ends_tick => {
                            // The whole strike cost must be available *before* attempting.
                            if let Some(t) = m.target
                                && o.energy >= profile.strike_energy_cost
                            {
                                charges.push((m.id, profile.strike_energy_cost));
                                m.attack_counter += 1;
                                hunters.attacks_total += 1;
                                counters.hunter_attacks += 1;
                                // The episode key: the counter this attempt's draws and its
                                // events all carry, after the increment that opened it.
                                let episode = m.attack_counter;
                                m.enter(HunterPhase::Strike, now, now + strike_ticks, episode);
                                m.target = Some(t);
                                // The strike frame: where both bodies stood when the burst was
                                // paid for, i.e. after `windup_seconds` of the hunter holding
                                // and the prey fleeing.
                                if strikes.enabled() {
                                    strikes.begin_strike(
                                        m.id,
                                        episode,
                                        hunter::StrikeFrame::gather(
                                            topo,
                                            images,
                                            profile,
                                            now,
                                            o,
                                            organisms.get(t).map(|prey| (t, prey)),
                                        ),
                                    );
                                }
                            } else {
                                // Refused before payment: no energy, no draw, no attempt.
                                hunter_events.push(HunterEvent::Attempt {
                                    tick: now + 1,
                                    hunter: m.id,
                                    target: m.target,
                                    outcome: hunter::AttemptOutcome::Unaffordable,
                                    energy_paid: 0.0,
                                    attack_counter: None,
                                    evidence: m
                                        .target
                                        .and_then(|t| organisms.get(t).map(|prey| (t, prey)))
                                        .map(|(t, prey)| {
                                            hunter::ContactEvidence::gather(
                                                topo, images, profile, o, t, prey,
                                            )
                                        }),
                                });
                                m.enter(HunterPhase::Perched, now, now, 0);
                            }
                        }
                        _ => {}
                    }

                    // The intent: face the target, close the distance while the grasp is still
                    // ahead of the prey, hold while cocking, and burst during the strike.
                    // Steering uses the neighbour list's own unfolded position.
                    if let Some(t) = m.target
                        && m.phase.hunting()
                        && let Some(n) = sensed.iter().find(|n| n.id == t)
                        && let Some(toward) = (n.local - o.pos.chart()).normalized()
                    {
                        // The pursuit stopping distance, reconciled with the effector rather
                        // than left implicit: a prey already *inside* the reach envelope is not
                        // approached further — walking onto it would put it behind the claws.
                        //
                        // R0d: this used to read `body.x < capture_offset_body.x - tolerance
                        // - closing`, which is the *far* side of the lunge, not the reach
                        // envelope the comment names — it was false for a prey sitting in the
                        // claws and so never held anything. It went unnoticed only because
                        // `closing = strike_speed_px_s · strike_seconds` was 1 px, inside the
                        // 2.6 px tolerance. The pace calibration makes the lunge 16.67 px, so
                        // a member charged straight through point-blank prey and missed.
                        //
                        // Until 2026-09-16 the rule was nevertheless still a one-sided
                        // **forward half-space**, `body.x < capture_offset_body.x + tolerance`,
                        // and not the envelope: a prey *short* of the claws satisfied it as
                        // readily as one inside them. The strike record measured it true at the
                        // burst's start on 408 of 449 paid attempts, which drops the member to
                        // `rest_effort` and suppresses the burst it has just paid for
                        // (`design/7_Research/ecology-v1-apex-reach-2026-09-16.md` §5), and the
                        // paired intervention took held-at-burst from 89.4 % to 5.4 %, contacts
                        // 88 → 140 and captures 38 → 67
                        // (`design/7_Research/ecology-v1-apex-predicate-2026-09-16.md`).
                        //
                        // **The shipped rule is now the envelope.** `PursuitStop` names both
                        // readings, `ContactMeasure::pursuit_holds` is the one place either is
                        // written, and an ordinary world runs `ReachEnvelope` without being
                        // told; the half-space is the opt-in that reproduces a retained row
                        // (`crate::World::set_pursuit_stop`,
                        // `crates/cubarium-core/tests/pursuit_predicate_adoption.rs`). A world's
                        // bytes do not carry the rule, so the adoption is schema 17 and a
                        // schema-16 world is refused rather than silently re-ruled.
                        let stop = strikes.pursuit_stop();
                        let inside = organisms
                            .get(t)
                            .and_then(admission)
                            .is_some_and(|c| geometry.pursuit_holds(stop, &c));
                        let hold = inside || m.phase == HunterPhase::Windup;
                        if let Some(d) = decisions.iter_mut().find(|(id, _)| *id == m.id) {
                            // The intent to face the target, **not** the heading it ends the
                            // tick with. This late override used to write the heading
                            // directly, so a member could spin to any bearing in one tick
                            // however large its body was; section 6 now turns it by what its
                            // own geometry and energy allow.
                            d.1.heading = toward;
                            d.1.effort = if hold {
                                f64::from(o.phenotype.drives.rest_effort)
                            } else {
                                1.0
                            };
                            if m.phase == HunterPhase::Strike && !inside {
                                boosts.push((m.id, profile.strike_speed_px_s));
                            }
                        }
                        threats.push((t, m.id));
                    }
                    // A member never grazes and never eats fruit; detritus scavenging is the
                    // profile's explicit allocation of its handling capacity, and only with no
                    // meal and no hunt in progress. Budding is the profile's own gate, applied
                    // in the physiology pass, so the controller's request never stands.
                    if let Some(d) = decisions.iter_mut().find(|(id, _)| *id == m.id) {
                        let may_scavenge = profile.scavenge_fraction > 0.0
                            && !m.carrying()
                            && m.target.is_none()
                            && !m.phase.hunting();
                        d.1.fruit_effort = 0.0;
                        d.1.graze_effort = 0.0;
                        d.1.scavenge_effort = if may_scavenge {
                            d.1.scavenge_effort * profile.scavenge_fraction
                        } else {
                            0.0
                        };
                        d.1.bud = false;
                        if !m.phase.hunting() && d.1.scavenge_effort <= 0.0 {
                            // Perched, recovering or handling with nothing to forage: rest in
                            // place rather than wander at seeking effort.
                            d.1.mode = Mode::Resting;
                            d.1.effort = f64::from(o.phenotype.drives.rest_effort);
                        }
                    }
                    // A member that left the gesture without paying for a burst has no
                    // attempt to record; a member still cocking or still lunging keeps its
                    // open frames. A no-op when nothing is open, and when recording is off.
                    if strikes.enabled()
                        && !matches!(m.phase, HunterPhase::Windup | HunterPhase::Strike)
                    {
                        strikes.abandon(m.id);
                    }
                    hunters.members[index] = m;
                }

                // The strike cost, charged in full at entry, before any outcome is known.
                for (id, cost) in charges {
                    if let Some(o) = organisms.get_mut(id) {
                        let paid = cost.min(o.energy).max(0.0);
                        o.energy -= paid;
                        if let Some(rec) = budgets.at(id) {
                            rec.other_energy_paid += paid;
                        }
                        heat(paid);
                    }
                }

                // 5c. Escape: only prey that is actually being pursued and actually senses
                //     its pursuer turns away, within a documented escape turn limit, and may
                //     briefly ask for more speed than its own maximum.
                let escape_turn = profile.escape_turn_rate_deg.to_radians() * dt;
                for (prey_id, hunter_id) in threats {
                    // A neural animal owns its own flight: the automatic escape turn and dash
                    // are a legacy behavioural override and must not steer it (contract §6).
                    // Its own sensing still reports the pursuer through the body sectors.
                    if neural.contains(prey_id) {
                        continue;
                    }
                    let Some(prey) = organisms.get(prey_id) else {
                        continue;
                    };
                    let Some(list) = neighbors.lists.get(prey_id.slot as usize) else {
                        continue;
                    };
                    let Some(n) = list.iter().find(|n| n.id == hunter_id) else {
                        continue;
                    };
                    let Some(d) = decisions.iter_mut().find(|(id, _)| *id == prey_id) else {
                        continue;
                    };
                    // Away from the sensed pursuer, at the profile's escape angular ceiling
                    // rather than the prey's ordinary one. Still only a request: section 6
                    // decides how much of it the body and its energy actually deliver.
                    d.1.heading = turn_toward(d.1.heading, prey.pos.chart() - n.local, escape_turn);
                    d.1.turn_rate_max = d.1.turn_rate_max.max(escape_turn / dt);
                    boosts.push((
                        prey_id,
                        profile.escape_speed_multiple * prey.phenotype.speed_max,
                    ));
                }
            }

            // 6. Resolve every motor request, move, transport tangents, and pay for the
            //    motion, the sensing and the maintenance.
            //
            //    This is the single boundary of `crate::motor` (milestone R0a): whatever the
            //    controller, apex pursuit, escape or an encounter retreat asked for above is
            //    an *intent*, and nothing downstream of here may assign a heading. The body's
            //    own radius, its angular ceiling and the movement energy left after upkeep
            //    decide how much of the intent is delivered, under `|v| + r · |ω| ≤ u`.
            //
            //    Payment order matches the physiology the world already used: unavoidable
            //    upkeep (`maintenance · S + sense_cost · r_sense`) is reserved first, and only
            //    the remainder buys motion. Translation and turning are charged exactly once
            //    each, through one term at the world's existing `move_cost`, with rotation
            //    priced at `motor::ROTATION_COST_SCALE` — so a body that only translates pays
            //    the pre-R0a bill to the bit, and a body with no movement energy left now
            //    holds still instead of moving for free.
            moved.resize_with(organisms.slot_count(), Vec::new);
            for segments in moved.iter_mut() {
                segments.clear();
            }
            // **Starvation is decided here, before intake settles.** A body starves when the
            // energy it can raise *this* tick — what it holds, plus everything one tick of
            // oxidation can convert out of its reserve (`Organism::raisable_energy`) — cannot
            // cover this tick's mandatory upkeep. The old rule (`energy <= 0 && reserve <= 0`,
            // evaluated after the settlement) could never fire while a geometric reserve decay
            // and an infinitesimal bite kept both stores positive: R0b found broke bodies at
            // `energy = 5.5e-57` still alive and still grazing. Recorded per slot now and
            // applied in the physiology pass below, so no amount of food arriving later in the
            // tick can reverse a body that already failed to pay for being alive.
            let mut starving = vec![false; organisms.slot_count()];
            // Reserve material each body oxidised *in the settlement below* to cover a bill its
            // stored energy could not. The physiology pass subtracts it from that body's
            // per-tick oxidation allowance, so the world's `oxidation_rate · dt` ceiling is a
            // ceiling on the tick, not on each pass separately.
            let mut settled_oxidation = vec![0.0f64; organisms.slot_count()];
            for (id, d) in &decisions {
                // The apex contact geometry, read before the mutable borrow: a member's claws
                // reach well past its lobes and are the part of it that actually sweeps.
                let apex_geometry = hunters
                    .profile
                    .as_ref()
                    .filter(|_| hunters.contains(*id))
                    .and_then(|profile| {
                        organisms
                            .get(*id)
                            .map(|o| hunter::ContactGeometry::of(profile, o))
                    });
                // The motor contract **this body** runs: the apex override where one is set and
                // this body has the contact geometry only a hunter member is handed, the world's
                // own otherwise. All four of this block's motor reads — the envelope radius, the
                // resolver, the bill and the rotation price the budget split uses — take it from
                // here, and for `apex_motor_model == None` it is `motor_model` for every body,
                // arithmetic for arithmetic.
                let body_model =
                    motor::model_for_body(motor_model, apex_motor_model, apex_geometry.as_ref());
                let Some(o) = organisms.get_mut(*id) else {
                    continue;
                };
                o.mode = d.mode;
                o.hunger_memory = d.hunger_memory;
                o.fed_this_tick = false;
                // Wading (`design/water.md`, `design/fauna-v2.md`): speed is divided by
                // `1 + w · (1 − swim)` of the cell the organism stands in before it moves; a
                // swimmer ignores the pool.
                let wading = 1.0 + fields.w[cell_of(topo, world_scale, &o.pos).index()] * (1.0 - o.phenotype.swim);
                let mut speed_cap = d.effort * o.phenotype.speed_max / wading;
                // A hunter's burst and a threatened prey's dash are the only boosts, and both
                // raise `speed_cap`. Since R0b that *is* the whole motor budget, so a
                // legitimate burst lifts turning as well as travel and a Mode label can
                // neither supply free rotation nor suppress an escape. The energy still binds.
                // The list is empty in every world without hunters.
                if let Some((_, wanted)) = boosts.iter().find(|(b, _)| *b == *id) {
                    speed_cap = speed_cap.max(wanted / wading);
                }
                let bill = MotorBill::of(o, cfg);
                if o.raisable_energy(org_cfg, dt) < bill.upkeep(dt)
                    && let Some(flag) = starving.get_mut(id.slot as usize)
                {
                    *flag = true;
                }
                let limits = MotorLimits {
                    // The radius the model in force puts in the rotation term: the outermost
                    // contacting point (an apex member's grasp included) under `Sweep`, the
                    // disc's own radius of gyration with the grasp dropped under `Inertial`.
                    radius_px: motor::turn_radius_px_in_with(
                        o,
                        apex_geometry.as_ref(),
                        body_model,
                        apex_turn_radius,
                    ),
                    turn_rate_max: d.turn_rate_max,
                    speed_cap,
                    motor_budget: bill.affordable_motor(o.energy, dt),
                    dt,
                };
                let request = MotorRequest {
                    heading: d.heading,
                    // A recurrent policy asks for a share of its capability as travel and
                    // leaves the rest for the pivot; every legacy caller asks for all of it.
                    // `resolve` clamps this to `speed_cap` either way.
                    speed: d.speed_request.unwrap_or(speed_cap),
                };
                let motion = motor::resolve_in(o.heading, &request, &limits, body_model);
                if !neural.animals.is_empty()
                    && let Some(a) = neural.get_mut(*id)
                {
                    // Feedback accumulates every physics tick and is consumed at the animal's
                    // own controller tick. `ResolvedMotion.turn` excludes transport by
                    // construction, and `neural::action::resolved_turn` puts it in the same
                    // clockwise sign convention as the `turn` channel.
                    let requested = neural::action::Envelope {
                        speed_max: o.phenotype.speed_max,
                        wading,
                        radius_px: limits.radius_px,
                        turn_rate_max: d.turn_rate_max,
                        u_full: limits.available(),
                        dt,
                    }
                    .requested_magnitude(&a.held_action());
                    a.feedback.speed_sum += motion.speed;
                    a.feedback.turn_sum += neural::action::resolved_turn(&motion);
                    a.feedback.req_mag += requested;
                    a.feedback.res_mag += motion.motor_magnitude();
                    a.feedback.ticks = a.feedback.ticks.saturating_add(1);
                }
                travel_into(topo, o.pos, motion.heading * (motion.speed * dt), travel_buf);
                o.pos = travel_buf.end;
                // Transport is a change of chart, applied to the *resolved* heading: it costs
                // nothing and consumes no turn budget.
                o.heading = travel_buf
                    .map
                    .apply(motion.heading)
                    .normalized()
                    .unwrap_or(motion.heading);
                o.ou = travel_buf.map.apply(d.ou);
                counters.travel_ties += travel_buf.ties;
                counters.travel_fallbacks += u32::from(travel_buf.fallback);
                if let Some(segments) = moved.get_mut(id.slot as usize) {
                    segments.extend_from_slice(&travel_buf.segments);
                }
                // **The bill is collected from the resources solvency counted.** The
                // starvation predicate above admits a body that can raise this tick's upkeep
                // out of `energy + one tick of oxidation`; taking only `min(cost, energy)`
                // forgave the difference and then handed the body the whole oxidation credit
                // in the physiology pass, so a body with no stored energy survived without
                // paying. Stored energy goes first; exactly the shortfall is then oxidised out
                // of the reserve, at the world's own rate, density and efficiency, inside the
                // same per-tick allowance the physiology pass uses.
                //
                // Only the *mandatory* half can reach the reserve: `motor_budget` is sized from
                // stored energy alone (`affordable_motor`), so a body short of upkeep has
                // `u = 0` and stands still, and the shortfall is never motion. Movement is paid
                // from the battery, as it always was.
                let cost = bill.total_cost_in(motion.speed, motion.sweep, dt, body_model);
                // The complete bill, recorded where it is levied: maintenance, sensing and
                // both halves of the motor charge, for every body billed this tick — including
                // one that is removed later in the same tick, because removals commit in step
                // 9 and this is step 6 (`crate::world::IntakeDiagnostics`).
                intake.body_bill_total += cost;
                intake.body_bill_upkeep += bill.upkeep(dt);
                // The same charge, per body, with its three terms kept apart. Translation and
                // the rotational sweep are priced exactly as `MotorBill::total_cost` prices
                // them — `move_cost · S · |v| · dt` and `move_cost · S · k · sweep · dt` — so
                // the split is the bill's own, not a second formula (`crate::world::budget`).
                if let Some(rec) = budgets.at(*id) {
                    let per_motor = bill.move_cost * bill.structure * dt;
                    rec.billed_ticks += 1;
                    rec.bill_total += cost;
                    rec.upkeep_billed += bill.upkeep(dt);
                    rec.motor_translation_billed += per_motor * motion.speed.max(0.0);
                    rec.motor_turn_billed +=
                        per_motor * body_model.rotation_price() * motion.sweep.max(0.0);
                }
                if let Some(row) = budgets.row_of(*id) {
                    row.bill_total += cost;
                }
                let mut collected = cost.min(o.energy).max(0.0);
                o.energy -= collected;
                let shortfall = (cost - collected).max(0.0);
                if shortfall > 0.0 {
                    let allowance = (org_cfg.oxidation_rate * dt).min(o.reserve.max(0.0)).max(0.0);
                    let per_unit = e_r * org_cfg.oxidation_efficiency;
                    if allowance > 0.0 && per_unit > 0.0 {
                        // Burn only what the shortfall needs, never the whole allowance.
                        let burned = (shortfall / per_unit).min(allowance);
                        o.reserve -= burned;
                        fields.n[cell_of(topo, world_scale, &o.pos).index()] += burned;
                        // The same transaction the physiology pass runs: the reserve material
                        // carried `e_r` per unit, `η_ox` of it becomes usable, the rest is heat.
                        let released = e_r * burned;
                        let room = (o.phenotype.energy_max - o.energy).max(0.0);
                        let gained = (released * org_cfg.oxidation_efficiency).min(room);
                        o.energy += gained;
                        heat(released - gained);
                        if let Some(rec) = budgets.at(*id) {
                            rec.oxidation_reserve_burned += burned;
                            rec.oxidation_battery_credit += gained;
                        }
                        if let Some(already) = settled_oxidation.get_mut(id.slot as usize) {
                            *already = burned;
                        }
                        // Spend what the bill still owes; anything the burn over-delivered
                        // stays in the battery rather than evaporating.
                        let more = shortfall.min(o.energy).max(0.0);
                        o.energy -= more;
                        collected += more;
                    }
                }
                heat(collected);
                // What it could actually raise. Less than `cost` only for a body that failed
                // to pay for being alive, which the starvation predicate above has already
                // marked for removal.
                intake.body_bill_paid += collected;
                if let Some(rec) = budgets.at(*id) {
                    rec.bill_paid += collected;
                }
                if let Some(row) = budgets.row_of(*id) {
                    row.bill_paid += collected;
                }
            }

            // 6b. Capture settlement, from the common post-movement state and before any
            //     field feeding or physiology: every paid attempt that ends this tick is
            //     resolved once, at most one hunter claims each prey, and a claimed prey is
            //     removed exactly once with exactly one death event. Losing contenders paid
            //     at strike entry and are not refunded.
            if !hunters.members.is_empty()
                && let Some(profile) = hunters.profile.clone()
            {
                let recovery_ticks = ticks_from_seconds(profile.recovery_seconds, dt).max(1);
                // Attempt priority is its own seeded draw, so a contested prey is not decided
                // by slot order; the full ID breaks a tie. Each attempt carries the target it
                // was made against, so a settlement earlier in this loop cannot turn a
                // contender's claim into "the target was lost".
                let mut attempts: Vec<(u64, OrganismId, usize, Option<OrganismId>)> = Vec::new();
                for (index, m) in hunters.members.iter().enumerate() {
                    if m.phase == HunterPhase::Strike && now + 1 >= m.phase_ends_tick {
                        let key = hunter::draw_key(m.id);
                        let counter = hunter::priority_counter(m.attack_counter);
                        attempts.push((
                            draw(seed, Stream::Hunt, key, counter),
                            m.id,
                            index,
                            m.target,
                        ));
                    }
                }
                attempts.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(&b.1)));

                let mut claimed: Vec<OrganismId> = Vec::new();
                for (_, hunter_id, index, aimed_at) in attempts {
                    let m = hunters.members[index];
                    let Some(hunter_o) = organisms.get(hunter_id) else {
                        continue;
                    };
                    let headroom = profile.gut_capacity_material - m.gut_material;
                    let mut caught: Option<(OrganismId, f64, f64)> = None;
                    // The settlement evidence is gathered here, from the common post-movement
                    // state, **before** anything is removed or any target is cleared: once the
                    // prey is gone no later view can recover where it stood.
                    let mut evidence: Option<hunter::ContactEvidence> = None;
                    let outcome = match aimed_at {
                        None => AttemptOutcome::TargetLost,
                        Some(t) if claimed.contains(&t) => {
                            // A contender that lost the claim still records what it saw.
                            if let Some(prey) = organisms.get(t) {
                                evidence = Some(hunter::ContactEvidence::gather(
                                    topo, images, &profile, hunter_o, t, prey,
                                ));
                            }
                            AttemptOutcome::TargetClaimed
                        }
                        Some(t) => match organisms.get(t) {
                            // A stale handle has no prey position: it is never filled in from
                            // whoever reused the slot.
                            None => AttemptOutcome::TargetLost,
                            Some(prey) => {
                                let seen = hunter::ContactEvidence::gather(
                                    topo, images, &profile, hunter_o, t, prey,
                                );
                                evidence = Some(seen);
                                if hunters.contains(t)
                                    || !hunter::prey_is_eligible(
                                        &profile, hunter_o, prey, headroom, e_r,
                                    )
                                {
                                    AttemptOutcome::Ineligible
                                } else if !seen.measure.is_some_and(|c| c.in_contact()) {
                                    // Contact is re-evaluated here, from the hunter root in the
                                    // renderer's body basis, after *both* creatures moved.
                                    AttemptOutcome::OutOfReach
                                } else if seen.capture_center.is_none() {
                                    // In reach of a grasp that is not on the surface where the
                                    // artwork draws it: off the open rim, or a vertex whose
                                    // images disagree. The strike is paid; no capture is made
                                    // from a point that cannot be drawn (the safe first policy
                                    // of the animation contract).
                                    AttemptOutcome::GraspUnmapped
                                } else {
                                    let chance = hunter::capture_probability(
                                        &profile,
                                        hunter_o.structure,
                                        prey.structure,
                                    );
                                    let roll = unit(
                                        seed,
                                        Stream::Hunt,
                                        hunter::draw_key(hunter_id),
                                        hunter::capture_counter(m.attack_counter),
                                    );
                                    if roll < chance {
                                        let (material, energy) = hunter::prey_inventory(prey, e_r);
                                        caught = Some((t, material, energy));
                                        claimed.push(t);
                                        AttemptOutcome::Captured
                                    } else {
                                        AttemptOutcome::Missed
                                    }
                                }
                            }
                        },
                    };
                    hunter_events.push(HunterEvent::Attempt {
                        tick: now + 1,
                        hunter: hunter_id,
                        target: aimed_at,
                        outcome,
                        energy_paid: profile.strike_energy_cost,
                        // The paid attempt's key: this member's counter after the increment
                        // that opened the attempt, the same one its draws used.
                        attack_counter: Some(m.attack_counter),
                        evidence,
                    });
                    // The resolution frame, read from the same common post-movement state the
                    // settlement above used, and before anything is removed. Closing the
                    // record derives the classification from the three frames and nothing
                    // else; it re-measures nothing and mutates nothing the tick reads.
                    if strikes.enabled() {
                        let resolution = hunter::StrikeFrame::gather(
                            topo,
                            images,
                            &profile,
                            now + 1,
                            hunter_o,
                            aimed_at.and_then(|t| organisms.get(t).map(|prey| (t, prey))),
                        );
                        strikes.close(
                            hunter_id,
                            m.attack_counter,
                            resolution,
                            outcome,
                            profile.strike_energy_cost,
                            profile.windup_seconds,
                            profile.strike_seconds,
                            |from, to| {
                                hunter::surface_reach(
                                    topo,
                                    images,
                                    from,
                                    to,
                                    topo.max_local_radius(),
                                )
                            },
                        );
                    }

                    match caught {
                        Some((prey_id, material, energy)) => {
                            // One removal, one death event, and no detritus: the whole body
                            // and its escrow moved into the gut, energy included. This is an
                            // internal transfer with no source ledger and no detritus cap.
                            let prey = organisms.remove(prey_id).expect("the claim was checked");
                            budgets.close(prey_id, &prey, now + 1, DeathCause::Predation);
                            // The private recurrent state goes with the body here too. An
                            // *ordinary* neural body is legitimate prey for a legacy hunter —
                            // refusing a neural apex says nothing about that combination — and
                            // this is the second boundary that removes an organism. Leaving the
                            // entry behind left the world failing its own `validate` with
                            // "neural animal has no organism" after an ordinary interaction.
                            neural.remove(prey_id);
                            // Every other member invalidates the handle in the same breath, so
                            // no unpaid hunt can chase a body that left the arena. A paid
                            // contender remains in Strike and settles from the attempt snapshot.
                            hunters.forget_target(prey_id, now + 1);
                            let member = &mut hunters.members[index];
                            member.gut_material += material;
                            member.gut_energy += energy;
                            // Settlement happens *after* movement, so the phase it opens starts
                            // at the boundary this tick completes — the same `now + 1` the
                            // capture record is stamped with. Entering at `now` would publish a
                            // recoil that began before the contact it recoils from.
                            member.enter(HunterPhase::Handling, now + 1, now + 1, m.attack_counter);
                            hunters.captures_total += 1;
                            hunters.predation_deaths_total += 1;
                            counters.hunter_captures += 1;
                            counters.deaths_predation += 1;
                            events.push(LifeEvent::Death {
                                tick: now + 1,
                                id: prey_id,
                                age_ticks: prey.age_ticks(now + 1),
                                cause: DeathCause::Predation,
                                births: prey.births,
                                genome: prey.genome.digest(),
                            });
                            hunter_events.push(HunterEvent::Capture {
                                tick: now + 1,
                                hunter: hunter_id,
                                prey: prey_id,
                                material,
                                energy,
                                attack_counter: m.attack_counter,
                                evidence: evidence.expect("a capture always measured its prey"),
                            });
                        }
                        None => {
                            // The recoil that follows came from a fully extended strike, which
                            // `entered_from` and `episode` now say out loud.
                            hunters.members[index].enter(
                                HunterPhase::Recovering,
                                now + 1,
                                now + 1 + recovery_ticks,
                                m.attack_counter,
                            );
                        }
                    }
                }
            }

            // 7. Settle feeding once per cell, proportionally, from the pre-transfer stocks
            //    (`design/ecology-v1-contract.md` §6.2–6.4).
            //
            //    One mouth, one rate. Every channel bites at the phenotype's `mouth_rate`,
            //    and the three efforts are normalised **world-side** so that graze + fruit +
            //    scavenge ≤ 1 for every decision — the neural squash already does this, but a
            //    legacy decision can set all three to 1 at once, and a scripted intent can
            //    set anything. The trade-off between foods is therefore in yield, once, not
            //    in rate and yield twice.
            //
            //    The scavenge channel serves litter and remains together: the request is made
            //    against `D_eff + C_eff` and the served bite is split between the two stocks
            //    in proportion to their edible shares.
            let mut fruit = vec![0.0f64; cell_count];
            let mut graze = vec![0.0f64; cell_count];
            let mut scavenge = vec![0.0f64; cell_count];
            // The pre-settlement detrital picture of every contested cell: each stock's
            // edible portion, and each stock's energy density. Captured before a single
            // transfer is applied, so every mouth in a cell reads the same food.
            let mut litter_eff = vec![0.0f64; cell_count];
            let mut carrion_eff = vec![0.0f64; cell_count];
            let mut litter_density = vec![0.0f64; cell_count];
            let mut carrion_density = vec![0.0f64; cell_count];
            let mut requests: Vec<(usize, OrganismId, f64, f64, f64)> = Vec::new();
            // 7t. The per-tick intake trace (`crate::world::budget`), for the one traced body
            //     and nobody else: the cell it stands on, that cell's four edible stocks and
            //     whether each clears `drives.feed_min`, the three decoded efforts (and the raw
            //     head behind them), the bite each mouth asks for, and what bound it. It reads
            //     the same pre-settlement stocks every mouth below reads and writes nothing
            //     any body or field can see. Off in every ordinary world.
            if let Some(target) = budgets.traced()
                && let Some(o) = organisms.get(target)
                && let Some((_, d)) = decisions.iter().find(|(id, _)| *id == target)
            {
                let here = cell_of(topo, world_scale, &o.pos);
                let cell = here.index();
                let stock = [
                    fields.p[cell],
                    fields.f[cell],
                    edible_detritus(fields.d[cell], fields.de[cell], e_r),
                    edible_detritus(ecology.carrion[cell], ecology.carrion_energy[cell], e_r),
                ];
                budgets.observe_intake(
                    target,
                    here.0,
                    stock,
                    cfg.drives.feed_min,
                    [d.graze_effort, d.fruit_effort, d.scavenge_effort],
                    o.phenotype.cap_foliage,
                    o.phenotype.cap_detrital,
                    o.phenotype.mouth_rate,
                    k_p,
                    dt,
                    o.reserve,
                    o.phenotype.reserve_max,
                    o.energy,
                    o.phenotype.energy_max,
                    traced_head,
                );
            }
            for (id, d) in &decisions {
                if d.fruit_effort <= 0.0 && d.graze_effort <= 0.0 && d.scavenge_effort <= 0.0 {
                    continue;
                }
                let Some(o) = organisms.get(*id) else {
                    continue;
                };
                // §6.2: a channel whose machinery does not exist is refused here, in the
                // world, whatever asked for it.
                let want_fruit =
                    if o.phenotype.cap_foliage > 0.0 { d.fruit_effort.max(0.0) } else { 0.0 };
                let want_graze =
                    if o.phenotype.cap_foliage > 0.0 { d.graze_effort.max(0.0) } else { 0.0 };
                let want_scavenge =
                    if o.phenotype.cap_detrital > 0.0 { d.scavenge_effort.max(0.0) } else { 0.0 };
                let asked = want_fruit + want_graze + want_scavenge;
                if asked <= 0.0 {
                    continue;
                }
                // §6.3: one mouth. Normalise only when the efforts overcommit it, so a
                // single-channel feeder is untouched.
                let norm = if asked > 1.0 { 1.0 / asked } else { 1.0 };
                let cell = cell_of(topo, world_scale, &o.pos).index();
                let headroom = (o.phenotype.reserve_max - o.reserve).max(0.0);
                // Type-II intake: what a mouth can take falls off as the cell empties, so a
                // poor cell is poor food even to an organism standing in it. `K_P = 0` gives
                // back the linear law exactly. The rate is the same for every food.
                let rate = o.phenotype.mouth_rate;
                let bite = |effort: f64, room: f64, food: f64| {
                    if effort <= 0.0 {
                        return 0.0;
                    }
                    let total = food + k_p;
                    let saturation = if total > 0.0 { food / total } else { 0.0 };
                    (rate * effort * dt * saturation).clamp(0.0, room.max(0.0))
                };
                let d_eff = edible_detritus(fields.d[cell], fields.de[cell], e_r);
                let c_eff =
                    edible_detritus(ecology.carrion[cell], ecology.carrion_energy[cell], e_r);
                // Fruit settles first and takes its headroom, grazing next, scavenging
                // gets the rest, so intake alone can never push the reserve past `R_max`.
                // All read the cell's pre-settlement stock.
                let f = bite(want_fruit * norm, headroom, fields.f[cell]);
                let g = bite(want_graze * norm, headroom - f, fields.p[cell]);
                let s = bite(want_scavenge * norm, headroom - f - g, d_eff + c_eff);
                intake.request_ticks += 1;
                if f <= 0.0 && g <= 0.0 && s <= 0.0 {
                    // Asked, got nothing. A full reserve is the one refusal the organism is
                    // carrying rather than the cell: name it, so an empty patch and a full
                    // animal are not reported as the same outcome.
                    intake.reserve_saturated_ticks += u64::from(headroom <= 0.0);
                    continue;
                }
                intake.requested += f + g + s;
                fruit[cell] += f;
                graze[cell] += g;
                scavenge[cell] += s;
                requests.push((cell, *id, f, g, s));
            }
            // Turn the per-cell request sums into proportional shares, and capture the two
            // detrital stocks' edible portions and densities, all before a single transfer is
            // applied. Sharing the scavenge requests against `D_eff + C_eff` is what bounds
            // each stock's own withdrawal: `Σq · X_eff/(D_eff + C_eff) ≤ X_eff ≤ X`.
            let mut contested: Vec<usize> = requests.iter().map(|r| r.0).collect();
            contested.sort_unstable();
            contested.dedup();
            for &cell in &contested {
                fruit[cell] = share(fruit[cell], fields.f[cell]);
                graze[cell] = share(graze[cell], fields.p[cell]);
                let (litter, litter_e) = (fields.d[cell], fields.de[cell]);
                let (remains, remains_e) =
                    (ecology.carrion[cell], ecology.carrion_energy[cell]);
                litter_eff[cell] = edible_detritus(litter, litter_e, e_r);
                carrion_eff[cell] = edible_detritus(remains, remains_e, e_r);
                litter_density[cell] = if litter > 0.0 { litter_e / litter } else { 0.0 };
                carrion_density[cell] = if remains > 0.0 { remains_e / remains } else { 0.0 };
                scavenge[cell] = share(scavenge[cell], litter_eff[cell] + carrion_eff[cell]);
            }

            let eta_m = org_cfg.assimilation_material;
            let eta_e = org_cfg.assimilation_energy;
            for &(cell, id, f, g, s) in &requests {
                let Some(o) = organisms.get_mut(id) else {
                    continue;
                };
                let cap_h = o.phenotype.cap_foliage;
                let cap_d = o.phenotype.cap_detrital;
                let mut eaten = 0.0;
                let mut ate = [0.0f64; 3];
                // §6.4, once, for every food: the stock loses the whole bite `q` and the
                // energy `ρ·q` it carried; the digestible portion `q_d = cap·q` is
                // assimilated at `η_m′` and stores `e_r` per unit in the reserve, `η_e` of
                // the difference reaches the battery and the rest is heat; what the mouth
                // could not digest — the un-assimilated part of `q_d` **and** the whole
                // indigestible `(1 − cap)·q` — is energy-free feces in `D`, and the energy
                // that indigestible part carried is heat. For `cap = 1` this is exactly the
                // pre-ecology-v1 law.
                //
                // Written out three times rather than through a closure because each food
                // draws its material and its energy from a different pair of fields, and the
                // borrow of those fields is what the accounting identity is about.
                if f > 0.0 {
                    let q = (f * fruit[cell]).clamp(0.0, fields.f[cell]);
                    if q > 0.0 {
                        let rho = e_f;
                        fields.f[cell] -= q;
                        if let Some(rec) = eco_scratch.plant_budget_mut() {
                            rec.cells[cell].withdrawal_fruit += q;
                        }
                        let q_d = cap_h * q;
                        let to_reserve = eta_m * q_d;
                        o.reserve += to_reserve;
                        let spare = rho * q_d - e_r * to_reserve;
                        let room = (o.phenotype.energy_max - o.energy).max(0.0);
                        let gained = (eta_e * spare).clamp(0.0, room);
                        o.energy += gained;
                        heat(spare - gained);
                        let feces = (1.0 - eta_m) * q_d + (1.0 - cap_h) * q;
                        fields.d[cell] += feces;
                        heat(rho * (1.0 - cap_h) * q);
                        intake.fruit_eaten += q;
                        intake.undigested += feces;
                        if let Some(rec) = budgets.at(id) {
                            rec.served[budget::FRUIT] += q;
                            rec.digestible[budget::FRUIT] += q_d;
                            rec.reserve_credit[budget::FRUIT] += to_reserve;
                            rec.battery_credit[budget::FRUIT] += gained;
                        }
                        if let Some(row) = budgets.row_of(id) {
                            row.credit(
                                budget::FRUIT,
                                q,
                                q_d,
                                to_reserve,
                                gained,
                                (eta_e * spare).max(0.0) - gained,
                            );
                        }
                        ate[1] += q;
                        eaten += q;
                    }
                }
                if g > 0.0 {
                    let q = (g * graze[cell]).clamp(0.0, fields.p[cell]);
                    if q > 0.0 {
                        let rho = e_v;
                        fields.p[cell] -= q;
                        // Workstream M's exact per-cell withdrawal: the cell the mouth is
                        // standing in, at the site the stock actually loses the bite.
                        if let Some(rec) = eco_scratch.plant_budget_mut() {
                            rec.cells[cell].withdrawal_foliage += q;
                        }
                        let q_d = cap_h * q;
                        let to_reserve = eta_m * q_d;
                        o.reserve += to_reserve;
                        let spare = rho * q_d - e_r * to_reserve;
                        let room = (o.phenotype.energy_max - o.energy).max(0.0);
                        let gained = (eta_e * spare).clamp(0.0, room);
                        o.energy += gained;
                        heat(spare - gained);
                        let feces = (1.0 - eta_m) * q_d + (1.0 - cap_h) * q;
                        fields.d[cell] += feces;
                        heat(rho * (1.0 - cap_h) * q);
                        intake.producer_eaten += q;
                        intake.undigested += feces;
                        if let Some(rec) = budgets.at(id) {
                            rec.served[budget::FOLIAGE] += q;
                            rec.digestible[budget::FOLIAGE] += q_d;
                            rec.reserve_credit[budget::FOLIAGE] += to_reserve;
                            rec.battery_credit[budget::FOLIAGE] += gained;
                        }
                        if let Some(row) = budgets.row_of(id) {
                            row.credit(
                                budget::FOLIAGE,
                                q,
                                q_d,
                                to_reserve,
                                gained,
                                (eta_e * spare).max(0.0) - gained,
                            );
                        }
                        ate[0] += q;
                        eaten += q;
                    }
                }
                if s > 0.0 {
                    // One scavenge bite, split between the two stocks by their edible shares.
                    let (d_eff, c_eff) = (litter_eff[cell], carrion_eff[cell]);
                    let total_eff = d_eff + c_eff;
                    let served = (s * scavenge[cell]).clamp(0.0, total_eff);
                    if served > 0.0 && total_eff > 0.0 {
                        let q_litter = (served * (d_eff / total_eff)).min(fields.d[cell]);
                        let q_remains =
                            (served - served * (d_eff / total_eff)).min(ecology.carrion[cell]);
                        if q_litter > 0.0 {
                            let q = q_litter;
                            // Energy leaves at the stock's own pre-settlement density; the
                            // clamp is a safety net that the `D_eff ≤ D` bound makes inert,
                            // and everything below is derived from what actually left, so a
                            // clamp could not leak a joule.
                            let carried = (litter_density[cell] * q).min(fields.de[cell]);
                            let rho = carried / q;
                            fields.d[cell] -= q;
                            fields.de[cell] -= carried;
                            if let Some(rec) = eco_scratch.plant_budget_mut() {
                                rec.cells[cell].withdrawal_litter += q;
                            }
                            let eta =
                                if e_r > 0.0 { eta_m * (rho / e_r).min(1.0) } else { eta_m };
                            let q_d = cap_d * q;
                            let to_reserve = eta * q_d;
                            o.reserve += to_reserve;
                            let spare = rho * q_d - e_r * to_reserve;
                            let room = (o.phenotype.energy_max - o.energy).max(0.0);
                            let gained = (eta_e * spare).clamp(0.0, room);
                            o.energy += gained;
                            heat(spare - gained);
                            let feces = (1.0 - eta) * q_d + (1.0 - cap_d) * q;
                            fields.d[cell] += feces;
                            heat(rho * (1.0 - cap_d) * q);
                            intake.litter_eaten += q;
                            intake.undigested += feces;
                            if let Some(rec) = budgets.at(id) {
                                rec.served[budget::LITTER] += q;
                                rec.digestible[budget::LITTER] += q_d;
                                rec.reserve_credit[budget::LITTER] += to_reserve;
                                rec.battery_credit[budget::LITTER] += gained;
                            }
                            if let Some(row) = budgets.row_of(id) {
                                row.credit(
                                    budget::LITTER,
                                    q,
                                    q_d,
                                    to_reserve,
                                    gained,
                                    (eta_e * spare).max(0.0) - gained,
                                );
                            }
                            ate[2] += q;
                            eaten += q;
                        }
                        if q_remains > 0.0 {
                            let q = q_remains;
                            let carried = (carrion_density[cell] * q)
                                .min(ecology.carrion_energy[cell]);
                            let rho = carried / q;
                            ecology.carrion[cell] -= q;
                            ecology.carrion_energy[cell] -= carried;
                            if let Some(rec) = eco_scratch.plant_budget_mut() {
                                rec.cells[cell].withdrawal_carrion += q;
                            }
                            let eta =
                                if e_r > 0.0 { eta_m * (rho / e_r).min(1.0) } else { eta_m };
                            let q_d = cap_d * q;
                            let to_reserve = eta * q_d;
                            o.reserve += to_reserve;
                            let spare = rho * q_d - e_r * to_reserve;
                            let room = (o.phenotype.energy_max - o.energy).max(0.0);
                            let gained = (eta_e * spare).clamp(0.0, room);
                            o.energy += gained;
                            heat(spare - gained);
                            // Feces are litter whatever was eaten: a scavenger's droppings
                            // are not a carcass (§5, "Sources into each stock").
                            let feces = (1.0 - eta) * q_d + (1.0 - cap_d) * q;
                            fields.d[cell] += feces;
                            heat(rho * (1.0 - cap_d) * q);
                            intake.carrion_eaten += q;
                            intake.undigested += feces;
                            if let Some(rec) = budgets.at(id) {
                                rec.served[budget::CARRION] += q;
                                rec.digestible[budget::CARRION] += q_d;
                                rec.reserve_credit[budget::CARRION] += to_reserve;
                                rec.battery_credit[budget::CARRION] += gained;
                            }
                            if let Some(row) = budgets.row_of(id) {
                                row.credit(
                                    budget::CARRION,
                                    q,
                                    q_d,
                                    to_reserve,
                                    gained,
                                    (eta_e * spare).max(0.0) - gained,
                                );
                            }
                            ate[2] += q;
                            eaten += q;
                        }
                    }
                }
                o.fed_this_tick = eaten > 0.0;
                // What this mouth actually removed from the fields, for the `ate` channels.
                // Material removed, not material requested: a poor cell reports the poor bite.
                if !neural.animals.is_empty()
                    && let Some(a) = neural.get_mut(id)
                {
                    for (acc, q) in a.feedback.ate.iter_mut().zip(ate) {
                        *acc += q;
                    }
                }
            }

            // 7b. Handling and digestion. A carried carcass is homogeneous: a portion `q`
            //     leaves the gut with exactly the energy it was carrying, stores what its
            //     density pays for, rejects the rest as energy-free detritus, and sends the
            //     spare energy to the battery or to heat. Handling is paid first and in full,
            //     or nothing is digested this tick.
            if !hunters.members.is_empty()
                && let Some(profile) = hunters.profile.as_ref()
            {
                let metabolism = hunter::Metabolism::of(cfg);
                let handling = profile.handling_cost_per_second * dt;
                let meal_ticks = ticks_from_seconds(profile.meal_recovery_seconds, dt).max(1);
                for index in 0..hunters.members.len() {
                    let m = hunters.members[index];
                    if apex_dormancy_on && apex_dormancy.contains(m.id) {
                        continue;
                    }
                    if !m.carrying() {
                        continue;
                    }
                    let Some(o) = organisms.get_mut(m.id) else {
                        continue;
                    };
                    let paid = handling.min(o.energy).max(0.0);
                    o.energy -= paid;
                    if let Some(rec) = budgets.at(m.id) {
                        rec.other_energy_paid += paid;
                    }
                    heat(paid);
                    if paid < handling {
                        // It could not carry its meal this tick; the gut keeps everything and
                        // ordinary oxidation may refill the battery for the next one.
                        continue;
                    }
                    let reserve_room = (o.phenotype.reserve_max - o.reserve).max(0.0);
                    let energy_room = (o.phenotype.energy_max - o.energy).max(0.0);
                    let step = hunter::digest_step(
                        profile,
                        dt,
                        m.gut_material,
                        m.gut_energy,
                        metabolism,
                        reserve_room,
                        energy_room,
                    );
                    if step.material > 0.0 {
                        o.reserve += step.to_reserve;
                        o.energy += step.energy_gain;
                        if let Some(rec) = budgets.at(m.id) {
                            rec.gut_reserve_credit += step.to_reserve;
                            rec.gut_battery_credit += step.energy_gain;
                        }
                        // §8: a digestion reject is feces, and feces are litter, energy-free.
                        fields.d[cell_of(topo, world_scale, &o.pos).index()] += step.to_detritus;
                        heat(step.heat);
                        let member = &mut hunters.members[index];
                        member.gut_material -= step.material;
                        member.gut_energy -= step.carried;
                    }
                    // A finished meal ends exactly empty: the last few ulps of energy leave as
                    // heat rather than sitting in a gut with no material to carry them.
                    let member = &mut hunters.members[index];
                    if member.gut_material <= hunter::GUT_RESIDUE {
                        let residue = member.gut_energy;
                        member.gut_material = 0.0;
                        member.gut_energy = 0.0;
                        if residue > 0.0 {
                            heat(residue);
                        }
                        // The meal ended *this* tick, so the pause starts now: a member is
                        // never left handling nothing, not even for the rest of the tick.
                        if member.phase == HunterPhase::Handling {
                            let episode = member.episode;
                            // Digestion is a post-movement pass too: the pause starts at the
                            // boundary this tick completes, and runs its whole advertised span.
                            member.enter(
                                HunterPhase::Recovering,
                                now + 1,
                                now + 1 + meal_ticks,
                                episode,
                            );
                        }
                    }
                }
            }

            // 8. Physiology: oxidation, growth, gestation, budding, death checks.
            let gestation_ticks = ticks_from_seconds(org_cfg.gestation_seconds, dt);
            // A member gestates on its own, much longer clock and grows on its own, much
            // slower ceiling; every other creature keeps the world config's.
            let hunter_gestation_ticks = hunters
                .profile
                .as_ref()
                .map(|p| ticks_from_seconds(p.gestation_seconds, dt))
                .unwrap_or(gestation_ticks);
            let max_age_ticks = ticks_from_seconds(org_cfg.max_age_seconds, dt);
            let cap = cfg.capacity.max_organisms as usize;
            let population = organisms.len();
            let mut births: Vec<OrganismId> = Vec::new();
            let mut deaths: Vec<(OrganismId, DeathCause)> = Vec::new();

            // Dormancy is a paid lifecycle, not free storage. Count suitable juvenile prey at
            // the concealed organism's persisted surface location, charge the low upkeep first,
            // then recheck both the current abundance and post-payment reserves before waking.
            if apex_dormancy_on {
                let evaluations: Vec<(OrganismId, u32)> = apex_dormancy
                    .dormant
                    .iter()
                    .map(|d| {
                        let suitable = organisms.get(d.id).map_or(0, |buried| {
                            organisms
                                .iter()
                                .filter(|(prey_id, prey)| {
                                    !hunters.contains(*prey_id)
                                        && prey.structure < 0.7 * prey.phenotype.structure_adult
                                        && hunter::prey_is_eligible(
                                            hunters.profile.as_ref().expect(
                                                "validated active dormancy has a hunter profile",
                                            ),
                                            buried,
                                            prey,
                                            hunters
                                                .profile
                                                .as_ref()
                                                .expect("profile")
                                                .gut_capacity_material,
                                            e_r,
                                        )
                                        && hunter::surface_reach(
                            topo,
                                            images,
                                            buried.pos,
                                            prey.pos,
                                            PREY_RADIUS_PX,
                                        )
                                        .is_some()
                                })
                                .count()
                        });
                        (d.id, u32::try_from(suitable).unwrap_or(u32::MAX))
                    })
                    .collect();
                let mut emerged = Vec::new();
                for (id, suitable_prey) in evaluations {
                    let Some(index) = apex_dormancy.index_of(id) else {
                        continue;
                    };
                    let Some(o) = organisms.get_mut(id) else {
                        continue;
                    };
                    let cost = MAINTENANCE_PER_STRUCTURE_SECOND * o.structure * dt;
                    let paid = cost.min(o.energy).max(0.0);
                    o.energy -= paid;
                    if let Some(rec) = budgets.at(id) {
                        rec.billed_ticks += 1;
                        rec.bill_total += cost;
                        rec.upkeep_billed += cost;
                        rec.bill_paid += paid;
                    }
                    heat(paid);
                    // A concealed offspring is a body too: its dormancy upkeep is part of the
                    // run's complete animal bill. It has no motor half — it does not move — so
                    // the whole charge is mandatory.
                    intake.body_bill_total += cost;
                    intake.body_bill_upkeep += cost;
                    intake.body_bill_paid += paid;
                    apex_dormancy.maintenance_energy_paid_total += paid;
                    let record = &mut apex_dormancy.dormant[index];
                    record.maintenance_energy_paid += paid;
                    record.suitable_prey_ticks = if suitable_prey >= PREY_REQUIRED {
                        record.suitable_prey_ticks.saturating_add(1)
                    } else {
                        0
                    };

                    if o.energy <= 0.0 {
                        deaths.push((id, DeathCause::Starvation));
                        apex_dormancy.exhausted_total += 1;
                        apex_dormancy_events.push(ApexDormancyEvent::Exhausted {
                            tick: now + 1,
                            id,
                            dormant_ticks: (now + 1).saturating_sub(record.entered_tick),
                            maintenance_energy_paid: record.maintenance_energy_paid,
                        });
                        continue;
                    }

                    if now + 1 >= record.next_check_tick {
                        let stocks_ready = o.reserve
                            >= EMERGENCE_RESERVE_FRACTION * o.phenotype.reserve_max
                            && o.energy >= EMERGENCE_ENERGY_FRACTION * o.phenotype.energy_max;
                        if record.suitable_prey_ticks >= SUSTAIN_TICKS && stocks_ready {
                            emerged.push((id, suitable_prey));
                        } else {
                            record.next_check_tick = (now + 1).saturating_add(RECHECK_TICKS);
                        }
                    }
                }
                for (id, suitable_prey) in emerged {
                    let record = apex_dormancy
                        .remove(id)
                        .expect("an emergence candidate is still dormant");
                    let dormant_ticks = (now + 1).saturating_sub(record.entered_tick);
                    if let Some(o) = organisms.get_mut(id) {
                        // Pause the active age clock exactly as long as development was paused.
                        o.born_tick = o.born_tick.saturating_add(dormant_ticks).min(now + 1);
                    }
                    apex_dormancy.emerged_total += 1;
                    apex_dormancy_events.push(ApexDormancyEvent::Emerged {
                        tick: now + 1,
                        id,
                        dormant_ticks,
                        suitable_prey,
                        maintenance_energy_paid: record.maintenance_energy_paid,
                    });
                }
            }
            for (id, d) in &decisions {
                let Some(o) = organisms.get_mut(*id) else {
                    continue;
                };
                // Membership, not `genome.form`, is what makes a predator.
                let member = hunters.index_of(*id);

                // *When* reserve is converted into battery charge is a **member policy**; what
                // that conversion does is not. An authoritative member carrying semantic
                // profile version 4 activates below a fixed fraction of `E_max` at every age
                // and phase; everything else — ordinary organisms, and members carrying
                // version 3 — uses the world's configured threshold, resolved through the same
                // accessor so the two cannot drift apart (`crate::hunter::OxidationPolicy`).
                let reference = org_cfg.oxidation_threshold;
                let threshold = match (member, hunters.profile.as_ref()) {
                    (Some(_), Some(profile)) => profile.oxidation_threshold(org_cfg),
                    _ => reference,
                };
                if o.energy < threshold * o.phenotype.energy_max && o.reserve > 0.0 {
                    // Decided *before* the transaction changes anything, so the test is the
                    // one a configured-threshold member would actually have failed — not a
                    // subtraction reconstructed afterwards from rounded values.
                    let above_reference = o.energy >= reference * o.phenotype.energy_max;
                    // Whatever the settlement already oxidised out of this body comes off its
                    // allowance: `oxidation_rate · dt` bounds the tick, not each pass.
                    let already = settled_oxidation
                        .get(id.slot as usize)
                        .copied()
                        .unwrap_or(0.0);
                    let burned = (org_cfg.oxidation_rate * dt - already).max(0.0).min(o.reserve);
                    o.reserve -= burned;
                    fields.n[cell_of(topo, world_scale, &o.pos).index()] += burned;
                    // The reserve material carried `e_r` per unit; `η_ox` of it becomes usable.
                    let released = e_r * burned;
                    let room = (o.phenotype.energy_max - o.energy).max(0.0);
                    let gained = (released * org_cfg.oxidation_efficiency).min(room);
                    o.energy += gained;
                    heat(released - gained);
                    if let Some(rec) = budgets.at(*id) {
                        rec.oxidation_reserve_burned += burned;
                        rec.oxidation_battery_credit += gained;
                    }
                    // Bounded diagnostics, after the transaction and out of its way: this is
                    // the branch a member under the configured threshold would not have taken.
                    // Reads nothing new, writes no world state, consumes no draw
                    // (`ChargingDiagnostics`).
                    if above_reference && burned > 0.0 {
                        charging.extra_transactions += 1;
                        charging.extra_reserve_burned += burned;
                        charging.extra_energy_gained += gained;
                        charging.extra_heat += released - gained;
                    }
                }

                if o.structure < o.phenotype.structure_adult
                    && o.reserve > org_cfg.growth_reserve_min * o.phenotype.reserve_max
                {
                    let growth_rate = match (member, hunters.profile.as_ref()) {
                        (Some(_), Some(profile)) => profile.juvenile_growth_rate,
                        _ => org_cfg.growth_rate,
                    };
                    let mut grown = (growth_rate * dt)
                        .min(o.phenotype.structure_adult - o.structure)
                        .min(o.reserve);
                    // Building is paid for up front: what the energy cannot cover is not built.
                    if org_cfg.build_cost > 0.0 {
                        grown = grown.min(o.energy / org_cfg.build_cost);
                    }
                    if grown > 0.0 {
                        o.reserve -= grown;
                        o.structure += grown;
                        let cost = (org_cfg.build_cost * grown).min(o.energy);
                        o.energy -= cost;
                        if let Some(rec) = budgets.at(*id) {
                            rec.growth_material += grown;
                            rec.growth_energy += cost;
                        }
                        // Structure holds no chemical energy: the reserve's energy is released.
                        heat(cost + e_r * grown);
                    }
                }

                let gestation = if member.is_some() {
                    hunter_gestation_ticks
                } else {
                    gestation_ticks
                };
                let due = o
                    .escrow
                    .as_ref()
                    .is_some_and(|e| now.saturating_sub(e.started_tick) >= gestation);
                // A hunter's one paid offspring is gated by its profile and its own local
                // state; the ordinary controller's `bud` never applies to a member.
                // **The retained maturity gate.** `decide_quiet` refuses a budding request on
                // five conditions (`controller.rs`): the quiet hold, `escrow.is_none()`, the
                // two drive *thresholds* `bud_reserve`/`bud_energy`, and `bud_min_age_seconds`.
                // A neural body skips that function entirely, and the two thresholds are
                // exactly the behavioural preference the policy is meant to own. The age gate
                // is not: the brief keeps present physical maturity, funding and gestation
                // constraints in this slice, so the world enforces it at admission instead.
                // `escrow.is_none()`, funding and capacity are already world conditions below.
                //
                // This is a strict no-op for a legacy body: `d.bud` already carries the same
                // test, and a scripted diagnostic intent can only suppress `bud`, never set it.
                let mature = o.age_ticks(now) as f64 * dt
                    >= f64::from(o.phenotype.drives.bud_min_age_seconds);
                let bud = match (member, hunters.profile.as_ref()) {
                    (Some(index), Some(profile)) if !apex_encounters_on => {
                        hunter::may_reproduce(profile, o, &hunters.members[index], now, dt)
                    }
                    (Some(_), Some(_)) => false,
                    _ => d.bud && mature,
                };
                if due {
                    births.push(*id);
                } else if bud && o.escrow.is_none() {
                    if population + births.len() < cap {
                        let structure =
                            org_cfg.child_structure_fraction * o.phenotype.structure_adult;
                        let reserve = org_cfg.child_reserve_fraction * o.phenotype.reserve_max;
                        let energy = org_cfg.child_energy_fraction * o.phenotype.energy_max;
                        let build = org_cfg.build_cost * structure;
                        if o.reserve >= structure + reserve && o.energy >= build + energy {
                            // Read on either side of the assignment that moves them: this is the
                            // transaction, not a post-step difference (`crate::hunter`).
                            let (reserve_before, energy_before) = (o.reserve, o.energy);
                            o.reserve -= structure + reserve;
                            o.energy -= build + energy;
                            if let Some(rec) = budgets.at(*id) {
                                rec.reproduction_material += structure + reserve;
                                rec.reproduction_energy += build + energy;
                            }
                            heat(build);
                            let genome = o.genome.clone();
                            o.escrow = Some(Escrow {
                                structure,
                                reserve,
                                energy,
                                started_tick: now,
                                genome,
                            });
                            if member.is_some() {
                                hunter_events.push(HunterEvent::Reproduction {
                                    tick: now + 1,
                                    hunter: *id,
                                    record: hunter::Reproduction::Funded {
                                        key: hunter::EscrowKey {
                                            parent: *id,
                                            started_tick: now,
                                        },
                                        parent_reserve_before: reserve_before,
                                        parent_reserve_after: o.reserve,
                                        parent_energy_before: energy_before,
                                        parent_energy_after: o.energy,
                                        escrow_structure: structure,
                                        escrow_reserve: reserve,
                                        escrow_energy: energy,
                                        build_heat: build,
                                    },
                                });
                            }
                        } else if member.is_some() {
                            // Ready by its own gate, but the stocks could not cover the child:
                            // no escrow exists and nothing moved.
                            hunter_events.push(HunterEvent::Reproduction {
                                tick: now + 1,
                                hunter: *id,
                                record: hunter::Reproduction::NotFunded {
                                    parent: *id,
                                    reason: hunter::FundingBlocked::Stocks,
                                },
                            });
                        }
                    } else {
                        counters.cap_rejections += 1;
                        *cap_rejections_total += 1;
                        if member.is_some() {
                            // The cap refused the gestation before it began: no escrow was ever
                            // created, so there is no transaction to close later.
                            hunter_events.push(HunterEvent::Reproduction {
                                tick: now + 1,
                                hunter: *id,
                                record: hunter::Reproduction::NotFunded {
                                    parent: *id,
                                    reason: hunter::FundingBlocked::Cap,
                                },
                            });
                        }
                    }
                }

                let cause = if starving.get(id.slot as usize).copied().unwrap_or(false) {
                    Some(DeathCause::Starvation)
                } else if o.age_ticks(now) >= max_age_ticks {
                    Some(DeathCause::Age)
                } else if o.structure < org_cfg.min_structure {
                    Some(DeathCause::Collapse)
                } else {
                    None
                };
                if let Some(cause) = cause {
                    deaths.push((*id, cause));
                    // A parent that dies this tick miscarries: the escrow goes to detritus.
                    births.retain(|b| b != id);
                }
            }

            // 9. Commit: remove the dead, then place births against the freed capacity.
            // The detritus energy cap is no longer read here: every deposit this pass makes
            // is animal remains, under `e_c_max`.
            for (id, cause) in &deaths {
                let Some(o) = organisms.remove(*id) else {
                    continue;
                };
                budgets.close(*id, &o, now + 1, *cause);
                // The private recurrent state goes at the same boundary the body does. The
                // entry is keyed by the full id, so a reused slot could not inherit it even if
                // this removal were ever missed.
                neural.remove(*id);
                let cell = cell_of(topo, world_scale, &o.pos).index();
                // The body: structure carries no energy, the reserve carries `e_r` per unit.
                // Ecology v1 §8: an ordinary death is **animal remains**, not plant litter.
                // A detrital digester can eat it; a foliage digester cannot; decomposition
                // takes it back to `N` on its own, faster, rate.
                let material = o.structure + o.reserve;
                let energy = o.energy + e_r * o.reserve;
                ecology.carrion[cell] += material;
                let kept = energy.min(e_c_max * material);
                ecology.carrion_energy[cell] += kept;
                heat(energy - kept);
                // A gestation that never finished decays with its own clamp.
                if let Some(es) = &o.escrow {
                    let paired = if apex_encounters_on {
                        apex_encounters.gestation(*id).copied()
                    } else {
                        None
                    };
                    let material = es.structure + es.reserve;
                    let energy = e_r * material + es.energy;
                    ecology.carrion[cell] += material;
                    let kept = energy.min(e_c_max * material);
                    ecology.carrion_energy[cell] += kept;
                    heat(energy - kept);
                    // The escrow's own terms, kept apart from the body above and the gut below:
                    // a miscarriage is not the whole corpse.
                    if let Some(pair) = paired {
                        apex_encounters.miscarriages_total += 1;
                        apex_encounter_events.push(ApexEncounterEvent::Miscarried {
                            tick: now + 1,
                            carrier: pair.carrier,
                            partner: pair.partner,
                            cause: *cause,
                            material,
                            energy,
                            energy_stored: kept,
                            energy_heat: energy - kept,
                        });
                        let _ = apex_encounters.take_gestation(*id);
                    } else if hunters.contains(*id) {
                        hunter_events.push(HunterEvent::Reproduction {
                            tick: now + 1,
                            hunter: *id,
                            record: hunter::Reproduction::Miscarried {
                                key: hunter::EscrowKey {
                                    parent: *id,
                                    started_tick: es.started_tick,
                                },
                                cause: *cause,
                                material,
                                energy,
                                energy_stored: kept,
                                energy_heat: energy - kept,
                            },
                        });
                    }
                }
                let slot = match cause {
                    DeathCause::Starvation => 0,
                    DeathCause::Age => 1,
                    DeathCause::Collapse => 2,
                    // Predation never reaches this loop: a consumed prey is settled and
                    // removed by the hunter pass, which books it in the extension's own
                    // counter and leaves the three natural counters alone.
                    DeathCause::Predation => {
                        unreachable!("predation is settled by the hunter pass")
                    }
                };
                counters.deaths[slot] += 1;
                deaths_total[slot] += 1;
                // Whether this was prey or another hunter, any unpaid pursuit ends at the
                // removal boundary. A paid strike keeps its phase long enough to settle as a
                // lost target on its own boundary.
                hunters.forget_target(*id, now + 1);
                // A member's carried meal goes where its body went: into the cell's detritus,
                // keeping at most what the detritus cap allows and releasing the rest as heat.
                // Nothing a hunter was holding disappears because its record was removed.
                if let Some(gone) = hunters.remove_member(*id, now + 1) {
                    let mut stored = 0.0;
                    if gone.gut_material > 0.0 || gone.gut_energy > 0.0 {
                        // A carried carcass released by its hunter's death is still a body:
                        // it lands in `C` with the hunter's own (§5, §8).
                        ecology.carrion[cell] += gone.gut_material;
                        stored = gone.gut_energy.min(e_c_max * gone.gut_material);
                        ecology.carrion_energy[cell] += stored;
                        heat(gone.gut_energy - stored);
                    }
                    hunters.hunter_deaths_total += 1;
                    hunter_events.push(HunterEvent::Death {
                        tick: now + 1,
                        id: *id,
                        cause: *cause,
                        gut_material: gone.gut_material,
                        gut_energy: gone.gut_energy,
                        gut_energy_stored: stored,
                    });
                }
                apex_dormancy.remove(*id);
                apex_encounters.remove_parentage(*id);
                events.push(LifeEvent::Death {
                    tick: now + 1,
                    id: *id,
                    age_ticks: o.age_ticks(now + 1),
                    cause: *cause,
                    births: o.births,
                    genome: o.genome.digest(),
                });
            }

            // Every removal path — starvation, age, collapse, a settled capture — has now
            // happened. A pause whose parent is gone is aborted here, once, before any birth
            // can add a new one (`crate::quiet`).
            if quiet_on && !quiet.pauses.is_empty() {
                let mut kept = Vec::with_capacity(quiet.pauses.len());
                for p in std::mem::take(&mut quiet.pauses) {
                    if organisms.get(p.parent).is_some() {
                        kept.push(p);
                    } else {
                        quiet_events.push(QuietEvent::Abort {
                            tick: now + 1,
                            parent: p.parent,
                            child: p.child,
                            completed_ticks: p.completed(now + 1),
                            reason: QuietReason::ParentGone,
                        });
                    }
                }
                quiet.pauses = kept;
            }

            for parent_id in &births {
                let full = organisms.len() >= cap;
                // A member's child inherits the lineage, not the appearance: membership is
                // granted explicitly below, and its genome is copied exactly.
                let hunter_parent = hunters.index_of(*parent_id);
                let paired = if apex_encounters_on {
                    apex_encounters.gestation(*parent_id).copied()
                } else {
                    None
                };
                if full && let Some(pair) = paired {
                    let Some(escrow) = organisms
                        .get_mut(*parent_id)
                        .and_then(|parent| parent.escrow.take())
                    else {
                        continue;
                    };
                    // Each surviving contributor receives its own inventory share. A partner
                    // that died after paying cannot safely be addressed; only that stale share
                    // falls back to the carrier. Build heat is already in the heat ledger.
                    {
                        let carrier = organisms
                            .get_mut(*parent_id)
                            .expect("a due paired carrier is alive");
                        carrier.reserve += pair.carrier_paid.material();
                        carrier.energy += pair.carrier_paid.energy;
                    }
                    // A refund is negative outlay, not income: the contributor's own
                    // reproduction line goes back down by exactly what it got back. The build
                    // heat it also paid is not refunded and stays on the line.
                    if let Some(rec) = budgets.at(*parent_id) {
                        rec.reproduction_material -= pair.carrier_paid.material();
                        rec.reproduction_energy -= pair.carrier_paid.energy;
                    }
                    let partner_refund_to = if organisms.get(pair.partner).is_some() {
                        let partner = organisms
                            .get_mut(pair.partner)
                            .expect("checked live partner");
                        partner.reserve += pair.partner_paid.material();
                        partner.energy += pair.partner_paid.energy;
                        if let Some(rec) = budgets.at(pair.partner) {
                            rec.reproduction_material -= pair.partner_paid.material();
                            rec.reproduction_energy -= pair.partner_paid.energy;
                        }
                        pair.partner
                    } else {
                        let carrier = organisms
                            .get_mut(*parent_id)
                            .expect("a due paired carrier is alive");
                        carrier.reserve += pair.partner_paid.material();
                        carrier.energy += pair.partner_paid.energy;
                        // A dead partner's share falls back to the carrier, which never paid
                        // it: that is income to this body, so it reduces its own line by the
                        // same amount the identity's `Δ(S + R)` grew.
                        if let Some(rec) = budgets.at(*parent_id) {
                            rec.reproduction_material -= pair.partner_paid.material();
                            rec.reproduction_energy -= pair.partner_paid.energy;
                        }
                        *parent_id
                    };
                    debug_assert!(
                        (escrow.structure
                            - pair.carrier_paid.structure
                            - pair.partner_paid.structure)
                            .abs()
                            < hunter::TOLERANCE
                    );
                    debug_assert!(
                        (escrow.reserve - pair.carrier_paid.reserve - pair.partner_paid.reserve)
                            .abs()
                            < hunter::TOLERANCE
                    );
                    debug_assert!(
                        (escrow.energy - pair.carrier_paid.energy - pair.partner_paid.energy).abs()
                            < hunter::TOLERANCE
                    );
                    counters.cap_rejections += 1;
                    *cap_rejections_total += 1;
                    apex_encounters.refunds_total += 1;
                    let _ = apex_encounters.take_gestation(*parent_id);
                    apex_encounter_events.push(ApexEncounterEvent::Refunded {
                        tick: now + 1,
                        carrier: pair.carrier,
                        partner: pair.partner,
                        carrier_refund: pair.carrier_paid,
                        partner_refund: pair.partner_paid,
                        partner_refund_to,
                    });
                    continue;
                }
                let placement = {
                    let Some(parent) = organisms.get_mut(*parent_id) else {
                        continue;
                    };
                    let Some(escrow) = parent.escrow.take() else {
                        continue;
                    };
                    if full {
                        // A refused birth returns its escrow to the parent untouched.
                        let (reserve_before, energy_before) = (parent.reserve, parent.energy);
                        parent.reserve += escrow.structure + escrow.reserve;
                        parent.energy += escrow.energy;
                        if let Some(rec) = budgets.at(*parent_id) {
                            rec.reproduction_material -= escrow.structure + escrow.reserve;
                            rec.reproduction_energy -= escrow.energy;
                        }
                        counters.cap_rejections += 1;
                        *cap_rejections_total += 1;
                        if hunter_parent.is_some() && paired.is_none() {
                            // A refund, not a miscarriage: every unit went back where it came
                            // from, and nothing was burned or dropped.
                            hunter_events.push(HunterEvent::Reproduction {
                                tick: now + 1,
                                hunter: *parent_id,
                                record: hunter::Reproduction::Refunded {
                                    key: hunter::EscrowKey {
                                        parent: *parent_id,
                                        started_tick: escrow.started_tick,
                                    },
                                    refunded_structure: escrow.structure,
                                    refunded_reserve: escrow.reserve,
                                    refunded_energy: escrow.energy,
                                    parent_reserve_before: reserve_before,
                                    parent_reserve_after: parent.reserve,
                                    parent_energy_before: energy_before,
                                    parent_energy_after: parent.energy,
                                },
                            });
                        }
                        // …and the parent waits a gestation before trying again, so a world at
                        // its cap cannot spin a hunter through a free birth attempt per tick.
                        if let Some(index) = hunter_parent {
                            hunters.members[index].next_reproduction_tick =
                                now + 1 + hunter_gestation_ticks;
                        }
                        continue;
                    }
                    let index = u64::from(parent.births);
                    parent.births += 1;
                    let key = u64::from(parent_id.slot);
                    // Each birth owns a block of `BIRTH_DRAWS` counters: placement first,
                    // then mutation, so neither can collide with the next birth's draws.
                    let base = index * BIRTH_DRAWS;
                    let direction =
                        Vec2::from_screen_angle(unit(seed, Stream::Birth, key, base) * TAU);
                    let heading =
                        Vec2::from_screen_angle(unit(seed, Stream::Birth, key, base + 1) * TAU);
                    (
                        parent.pos,
                        direction,
                        heading,
                        escrow,
                        parent.age_ticks(now + 1),
                        parent.births,
                        key,
                        base + 2,
                    )
                };
                let (
                    from,
                    direction,
                    heading,
                    escrow,
                    parent_age_ticks,
                    parent_births,
                    key,
                    mut counter,
                ) = placement;
                travel_into(topo, from, direction * cfg.drives.birth_offset_px, travel_buf);
                counters.travel_ties += travel_buf.ties;
                counters.travel_fallbacks += u32::from(travel_buf.fallback);
                let heading = travel_buf
                    .map
                    .apply(heading)
                    .normalized()
                    .unwrap_or(Vec2::new(1.0, 0.0));
                let mut genome = escrow.genome.clone();
                // Sparse mutation (`design/fauna-v2.md`): the draws follow the placement draws
                // in the parent's birth stream; `form` never changes; an exact copy records
                // nothing.
                // Hunter mutation is outside this experiment: a member's child carries the
                // profile's fixed genome exactly, even where ordinary prey mutate.
                let mutations = if cfg.mechanisms.mutation && hunter_parent.is_none() {
                    let draws = genome.mutate(cfg.mutation.probability, cfg.mutation.step, || {
                        let u = unit(seed, Stream::Birth, key, counter);
                        counter += 1;
                        u
                    });
                    genome.clamp();
                    draws
                } else {
                    Vec::new()
                };
                let mut phenotype = decode(&genome, org_cfg);
                if hunter_parent.is_some()
                    && let Some(profile) = hunters.profile.as_ref()
                {
                    // The assembled body's tested support, exactly as the founder got it.
                    phenotype.extent = profile.body_extent_px;
                }
                // The escrowed material that becomes structure gives up its reserve energy.
                heat(e_r * escrow.structure);
                let child = Organism {
                    pos: travel_buf.end,
                    heading,
                    ou: Vec2::ZERO,
                    structure: escrow.structure,
                    reserve: escrow.reserve,
                    energy: escrow.energy,
                    born_tick: now + 1,
                    hunger_memory: (1.0 - escrow.reserve / phenotype.reserve_max).clamp(0.0, 1.0),
                    mode: Mode::Resting,
                    escrow: None,
                    births: 0,
                    genome,
                    phenotype,
                    parent: Some(*parent_id),
                    origin: Origin::Descendant,
                    turn_counter: Counter::default(),
                    fed_this_tick: false,
                };
                let genome_digest = child.genome.digest();
                let origin = child.origin;
                let child_id = organisms.insert(child);
                // A neural offspring copies the parent's **policy** and starts with fresh
                // private state: zero hidden, nothing held, no feedback, its own birth-tick
                // phase. Weight mutation, two-parent neural mating and any change to body
                // inheritance belong to a later slice; nothing here mutates weights.
                if let Some(parent_policy) = neural.get(*parent_id).map(|a| a.policy) {
                    neural.insert(
                        child_id,
                        neural::AnimalState::fresh(now + 1, parent_policy),
                    );
                }
                if let Some(index) = hunter_parent {
                    // The funded descendant joins the lineage with no target, an empty gut and
                    // a fresh attack counter, and the parent starts its recovery interval.
                    hunters.insert_member(hunter::HunterMember::new(child_id, now + 1));
                    if apex_dormancy_on
                        && let Some(child) = organisms.get(child_id)
                        && let Some(event) =
                            apex_dormancy.admit(*parent_id, child_id, now + 1, child)
                    {
                        apex_dormancy_events.push(event);
                    }
                    hunters.hunter_births_total += 1;
                    let interval = hunters
                        .profile
                        .as_ref()
                        .map(|p| ticks_from_seconds(p.reproduce_interval_seconds, dt))
                        .unwrap_or(0);
                    // `insert_member` may have shifted the parent's row; find it again.
                    let _ = index;
                    if let Some(parent_index) = hunters.index_of(*parent_id) {
                        hunters.members[parent_index].next_reproduction_tick = now + 1 + interval;
                    }
                    if let Some(pair) = paired {
                        // Both identities remain attached to the living child. The ordinary
                        // life record keeps its established primary-parent shape; this event is
                        // the reconciling two-parent identity and inventory record.
                        apex_encounters.insert_parentage(PairedParentage {
                            child: child_id,
                            carrier: pair.carrier,
                            partner: pair.partner,
                        });
                        let _ = apex_encounters.take_gestation(*parent_id);
                        apex_encounters.births_total += 1;
                        if let Some(partner_index) = hunters.index_of(pair.partner) {
                            hunters.members[partner_index].next_reproduction_tick =
                                now + 1 + interval;
                        }
                        apex_encounter_events.push(ApexEncounterEvent::Born {
                            tick: now + 1,
                            carrier: pair.carrier,
                            partner: pair.partner,
                            child: child_id,
                            structure: escrow.structure,
                            reserve: escrow.reserve,
                            energy: escrow.energy,
                            birth_heat: e_r * escrow.structure,
                        });
                    } else {
                        hunter_events.push(HunterEvent::Offspring {
                            tick: now + 1,
                            parent: *parent_id,
                            child: child_id,
                        });
                        // The transaction beside the identity link: the child's actual opening
                        // inventory is the escrow's, and the structural material gave up its
                        // reserve energy as heat on the way.
                        hunter_events.push(HunterEvent::Reproduction {
                            tick: now + 1,
                            hunter: *parent_id,
                            record: hunter::Reproduction::Born {
                                key: hunter::EscrowKey {
                                    parent: *parent_id,
                                    started_tick: escrow.started_tick,
                                },
                                child: child_id,
                                child_structure: escrow.structure,
                                child_reserve: escrow.reserve,
                                child_energy: escrow.energy,
                                birth_heat: e_r * escrow.structure,
                            },
                        });
                    }
                }
                // The trigger is this successful core commit, not an observer's post-step
                // inference: the child is in the arena and the parent survived to see it.
                if quiet_on {
                    quiet_admit(
                        quiet,
                        quiet_events,
                        organisms,
                        org_cfg,
                        cap,
                        now + 1,
                        dt,
                        *parent_id,
                        child_id,
                        hunter_parent.is_some(),
                    );
                }
                events.push(LifeEvent::Birth {
                    tick: now + 1,
                    id: child_id,
                    parent: *parent_id,
                    parent_age_ticks,
                    parent_births,
                    genome: genome_digest,
                    origin,
                    mutations,
                });
                let slot = child_id.slot as usize;
                if moved.len() <= slot {
                    moved.resize_with(slot + 1, Vec::new);
                }
                moved[slot].clear();
                counters.births += 1;
                *births_total += 1;
            }

            // Bring every surviving record's terminal stores up to this tick's close, so a
            // reader that stops mid-run sees a closed identity rather than a stale one. One
            // pass over the living, and nothing at all when recording is off.
            if budgets.enabled() {
                for (id, o) in organisms.iter() {
                    budgets.mark(id, o);
                }
            }
            // Keep this tick's intake row, once the served bites it needs are known.
            budgets.close_row();

            *tick += 1;
        }

        // 10. Invariants; the render view and telemetry are pulled by the host.
        #[cfg(debug_assertions)]
        {
            if let Err(e) = self.check_invariants() {
                panic!("invariant violated after tick {}: {e}", self.state.tick);
            }
            // The energy audit is an exact identity: every joule is light, heat, stored, or
            // (with care) fed in or cleaned out. Feed and clean commit at a boundary, never
            // inside a step, so their terms are zero here; they are written out anyway so
            // the identity the contract states is the identity the code checks.
            //
            // The hunter founder and control imports are boundary commits for the same reason;
            // a capture, a digestion and a hunter's death are all internal transfers and move
            // no term of this identity except heat.
            let (before, opening, fed, cleaned, imported) = audit;
            let booked = self.state.energy_ledgers().net_since(opening)
                + (self.state.care.feed_energy_in - fed)
                - (self.state.care.clean_energy_out - cleaned)
                + (self.state.hunters.imported_energy() - imported);
            let drift = (stored_energy(&self.state) - before) - booked;
            assert!(
                drift.abs() < AUDIT_TOLERANCE,
                "energy audit drifted by {drift:e} in tick {}",
                self.state.tick
            );
            // The water budget is the same kind of identity: `Δ Σw == rain_in − evap_out`.
            let (w_before, rain, evap) = water_audit;
            let w_booked = (self.state.rain_in_total - rain) - (self.state.evap_out_total - evap);
            let w_drift = (self.state.fields.w.iter().sum::<f64>() - w_before) - w_booked;
            assert!(
                w_drift.abs() < AUDIT_TOLERANCE,
                "water budget drifted by {w_drift:e} in tick {}",
                self.state.tick
            );
        }
        &self.counters
    }
}

/// One neural animal's observe-and-decide, in tick order (`crate::neural`, contract §1–5).
///
/// Every legacy behavioural rule is absent by construction here, not switched off downstream:
/// there is no hysteresis, no hunger memory, no steering weight, no OU draw, no turn gate and
/// no `feed_min`. What remains is the world's own physics, which stage 6 and everything after
/// it apply exactly as they do to a legacy body.
///
/// The cadence is the animal's own: on its controller tick it consumes the interval's
/// accumulated feedback, samples the world, advances the GRU and latches a new held action; on
/// the other tick it rebuilds this tick's request from the action already in force and the
/// heading the body has *now*, so a held turn keeps turning across the interval and across a
/// seam instead of chasing a bearing in a chart that no longer exists.
#[allow(clippy::too_many_arguments)]
fn neural_decision(
    neural: &mut crate::neural::NeuralState,
    timing: &mut super::state::NeuralTiming,
    index: usize,
    o: &Organism,
    now: u64,
    dt: f64,
    cfg: &WorldConfig,
    e_r: f64,
    fields: &crate::fields::Fields,
    eco: &crate::fields::EcologyV1State,
    light: &[f64],
    images: &[Vec<cubarium_surface::ChartImage>],
    rings: &[Vec<cubarium_surface::CellId>; super::SENSE_DEPTH_MAX],
    neighbours: &[crate::pairs::Neighbor],
    cells: &mut Vec<crate::neural::SensedCell>,
    bodies: &mut Vec<crate::neural::SensedBody>,
    head_sink: Option<&mut Option<[f64; crate::neural::action::ACT_LEN]>>,
    motor_model: crate::motor::MotorModel,
) -> Decision {
    use crate::neural::action::{Action7, Capability, Envelope};

    let (topo, world_scale) = (cfg.topology, cfg.world_scale);
    let here = cell_of(topo, world_scale, &o.pos).index();
    let omega_max = f64::from(o.phenotype.drives.turn_rate_max_deg).to_radians();
    // The radius the model in force puts in the rotation term, so what the policy is told
    // about its own turning capability is what the resolver will actually grant it. Under
    // `Inertial` this is the disc's radius of gyration, and `omega_attain = u_full / r_g` is
    // exactly the pivot the quadrature envelope allows at zero speed.
    let radius_px = motor::turn_radius_px_in(o, None, motor_model);
    // `motor_avail` and `ω_attain` share the world's own affordability calculation rather than
    // re-deriving an approximate energy bill: `u_full = min(v_max / wading, affordable_motor)`
    // at the energy the body holds *before* this tick's payment, which is the state stage 6
    // will price the move against.
    let wading = 1.0 + fields.w[here] * (1.0 - o.phenotype.swim);
    let bill = MotorBill::of(o, cfg);
    let u_full = (o.phenotype.speed_max / wading).min(bill.affordable_motor(o.energy, dt));
    let envelope = Envelope {
        speed_max: o.phenotype.speed_max,
        wading,
        radius_px,
        turn_rate_max: omega_max,
        u_full,
        dt,
    };

    if neural.animals[index].1.updates_on(now) {
        // ---- sample ----
        let sampler_start = std::time::Instant::now();
        let animal = &neural.animals[index].1;
        let feedback = animal.feedback.channels(
            o.phenotype.mouth_rate,
            o.phenotype.speed_max,
            radius_px,
            dt,
        );
        let observation = sample_observation(
            o, now, dt, cfg, e_r, fields, eco, light, images, rings, neighbours, u_full,
            feedback, cells, bodies,
        );
        timing.sampler_nanos = timing
            .sampler_nanos
            .saturating_add(sampler_start.elapsed().as_nanos() as u64);
        timing.sampler_calls = timing.sampler_calls.saturating_add(1);

        // ---- infer ----
        let policy_index = neural.animals[index].1.policy as usize;
        let head = match neural.policies.get(policy_index) {
            Some(policy) => {
                let mut hidden = [0.0f64; crate::neural::HIDDEN];
                let animal = &neural.animals[index].1;
                hidden.copy_from_slice(&animal.hidden);
                let inference_start = std::time::Instant::now();
                let y = policy.weights.forward(observation.as_slice(), &mut hidden);
                timing.inference_nanos = timing
                    .inference_nanos
                    .saturating_add(inference_start.elapsed().as_nanos() as u64);
                timing.inference_calls = timing.inference_calls.saturating_add(1);
                neural.animals[index].1.hidden.copy_from_slice(&hidden);
                y
            }
            // A validated world cannot reach this; a dangling index holds the last action
            // rather than inventing one.
            None => {
                let held = neural.animals[index].1.held;
                neural.animals[index].1.feedback = crate::neural::Feedback::default();
                return decision_from(Action7(held), o, &envelope, omega_max);
            }
        };

        let capability = Capability::ordinary(
            o.phenotype.cap_foliage,
            o.phenotype.cap_detrital,
            cfg.mechanisms.grazing,
            cfg.mechanisms.scavenging,
        );
        // The one read-only diagnostic in this function: the raw head before squashing, for
        // the traced body only (`crate::world::budget`). Nothing downstream reads it.
        if let Some(sink) = head_sink {
            *sink = Some(head);
        }
        let squash_start = std::time::Instant::now();
        let held = Action7::squash(&head, &capability).0;
        timing.adapter_nanos = timing
            .adapter_nanos
            .saturating_add(squash_start.elapsed().as_nanos() as u64);
        neural.animals[index].1.held = held;
        neural.animals[index].1.feedback = crate::neural::Feedback::default();
    }

    let adapter_start = std::time::Instant::now();
    let decision = decision_from(neural.animals[index].1.held_action(), o, &envelope, omega_max);
    timing.adapter_nanos = timing
        .adapter_nanos
        .saturating_add(adapter_start.elapsed().as_nanos() as u64);
    timing.adapter_calls = timing.adapter_calls.saturating_add(1);
    decision
}

/// Turn the action in force into this tick's `Decision`, from the body's *current* heading.
fn decision_from(
    action: crate::neural::Action7,
    o: &Organism,
    envelope: &crate::neural::action::Envelope,
    omega_max: f64,
) -> Decision {
    let request = envelope.request(o.heading, &action);
    Decision {
        // A neural animal has no mode: the label the renderer and the telemetry read is the
        // honest description of what it is doing, never an input to anything it decides.
        mode: if action.active() {
            Mode::Seeking
        } else {
            Mode::Resting
        },
        underlying_mode: if action.active() {
            Mode::Seeking
        } else {
            Mode::Resting
        },
        heading: request.heading,
        turn_rate_max: omega_max,
        ou: o.ou,
        effort: action.effort(),
        fruit_effort: action.0[crate::neural::action::FRUIT],
        graze_effort: action.0[crate::neural::action::GRAZE],
        scavenge_effort: action.0[crate::neural::action::SCAVENGE],
        bud: action.reproduce(),
        hunger_memory: o.hunger_memory,
        speed_request: Some(request.speed),
    }
}

/// Build one animal's 70-scalar observation from the pre-movement world (contract §1–2).
///
/// Split out of [`neural_decision`] so that a development accessor
/// (`World::neural_observation`) can produce exactly the vector the controller would see,
/// without stepping and without a second copy of the geometry.
#[allow(clippy::too_many_arguments)]
pub(super) fn sample_observation(
    o: &Organism,
    now: u64,
    dt: f64,
    cfg: &WorldConfig,
    e_r: f64,
    fields: &crate::fields::Fields,
    eco: &crate::fields::EcologyV1State,
    light: &[f64],
    images: &[Vec<cubarium_surface::ChartImage>],
    rings: &[Vec<cubarium_surface::CellId>; super::SENSE_DEPTH_MAX],
    neighbours: &[crate::pairs::Neighbor],
    u_full: f64,
    feedback: [f64; 6],
    cells: &mut Vec<crate::neural::SensedCell>,
    bodies: &mut Vec<crate::neural::SensedBody>,
) -> crate::neural::Observation70 {
    let org_cfg = &cfg.organism;
    let (topo, world_scale) = (cfg.topology, cfg.world_scale);
    let here = cell_of(topo, world_scale, &o.pos).index();
    let chart = o.pos.chart();
    cells.clear();
    bodies.clear();
    let depth = sense_depth(o.phenotype.sense_radius);
    for (hop, ring) in rings[..depth].iter().enumerate() {
        for neighbor in ring {
            let center = neighbor.center(topo, world_scale);
            let Some(view) =
                unfold_with(topo, &images[topo.chart_index(o.pos.face)], o.pos, center, CELL_UNFOLD_RADIUS)
            else {
                continue;
            };
            if view.distance <= GRADIENT_EPS || view.distance.is_nan() {
                continue;
            }
            let there = neighbor.index();
            cells.push(crate::neural::SensedCell {
                offset: view.local - chart,
                near: hop == 0,
                p: fields.p[there],
                f: fields.f[there],
                // `design/ecology-v1-contract.md` §9: the detrital channels read the two
                // stocks' edible portions together. The layout is unchanged — this scalar is
                // still "edible detrital material here" — so the policy interface does not
                // move even though the world behind it does.
                d_eff: edible_here(fields, eco, there, e_r),
            });
        }
    }
    let mut crowd = Vec2::ZERO;
    for n in neighbours {
        bodies.push(crate::neural::SensedBody {
            offset: n.local - chart,
            distance: n.distance,
            extent: n.extent,
        });
        // The same overlap term the world already computes, on the same condition.
        let extent_sum = o.phenotype.extent + n.extent;
        if n.distance >= extent_sum + 1.0 {
            continue;
        }
        let delta = chart - n.local;
        let len_sq = delta.length_sq();
        if len_sq > GRADIENT_EPS {
            crowd += delta * (extent_sum / len_sq);
        }
    }

    let self_state = crate::neural::SelfState {
        p_here: fields.p[here],
        f_here: fields.f[here],
        d_here: edible_here(fields, eco, here, e_r),
        p_max: cfg.producer.max,
        crowd,
        water: fields.w[here],
        w_flood: cfg.water.flood,
        light: light[here],
        height: topo.height(&o.pos),
        up: up_direction(topo, o.pos.face),
        extent: o.phenotype.extent,
        sense_radius: o.phenotype.sense_radius,
        reserve: o.reserve,
        reserve_max: o.phenotype.reserve_max,
        energy: o.energy,
        energy_max: o.phenotype.energy_max,
        structure: o.structure,
        structure_adult: o.phenotype.structure_adult,
        gestating: o.escrow.is_some(),
        age_seconds: o.age_ticks(now) as f64 * dt,
        max_age_seconds: org_cfg.max_age_seconds,
        motor_avail: if o.phenotype.speed_max > 0.0 {
            u_full / o.phenotype.speed_max
        } else {
            0.0
        },
        feedback,
    };
    crate::neural::obs::observe(o.heading, cells, bodies, &self_state)
}
