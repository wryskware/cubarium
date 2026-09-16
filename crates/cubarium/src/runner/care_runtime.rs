use anyhow::Result;
use cubarium_core::care::{CareCommand, CareKind, CareOutcome, CareReceipt, CareTarget};
use cubarium_core::{CareApplied, FixedHunterProfile, HunterTarget, World};

use crate::care;
use crate::care_effects;
use crate::cli::Run;
use crate::state;

fn care_epoch() -> String {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    format!("{nanos:x}-{}", std::process::id())
}

/// One command as the core wants it. The host keeps whole-pixel targets because that is
/// what a click on the net *is*; the core takes chart coordinates as `f64`.
pub(super) fn to_core(command: &care::PlannedCommand) -> CareCommand {
    CareCommand {
        seq: command.seq,
        apply_after_tick: command.apply_after_tick,
        kind: match command.kind {
            care::CareKind::Feed => CareKind::Feed,
            care::CareKind::Rain => CareKind::Rain,
            care::CareKind::Clean => CareKind::Clean,
            care::CareKind::SpawnApex => unreachable!("apex introductions use apply_planned"),
        },
        target: CareTarget {
            face: command.target.face,
            u: f64::from(command.target.u),
            v: f64::from(command.target.v),
        },
        // The journaled amount, carried through verbatim. The host never substitutes a
        // default here: a replayed command applies what its record says it asked for.
        dose: command.dose,
    }
}

pub(super) fn apply_planned(world: &mut World, command: &care::PlannedCommand) -> CareReceipt {
    if command.kind != care::CareKind::SpawnApex {
        return world.apply_care(&to_core(command));
    }
    let tick = world.tick();
    let receipt = |outcome| CareReceipt {
        seq: command.seq,
        tick,
        outcome,
    };
    if command.seq != world.care().admitted_seq.wrapping_add(1) {
        return receipt(CareOutcome::Rejected("out of order".into()));
    }
    if tick != command.apply_after_tick {
        return receipt(CareOutcome::Rejected("wrong boundary".into()));
    }
    // Sequence admission must run in release builds too, even when spawning is refused.
    let admitted = world.void_care(command.seq);
    debug_assert!(admitted);
    let to_target = |target: care::CareTarget| HunterTarget {
        face: target.face,
        u: f64::from(target.u),
        v: f64::from(target.v),
    };
    let mut targets = vec![to_target(command.target)];
    if let Some(target) = command.second_target {
        targets.push(to_target(target));
    }
    let mut profile = FixedHunterProfile::lanternjaw_trial(world.config());
    profile.seek_reserve_fraction = 0.80;
    profile.perch_reserve_fraction = 0.90;
    profile = profile.charge80();
    match world.introduce_hunters(profile, &targets) {
        Ok(founders) => receipt(CareOutcome::Applied(CareApplied {
            material_in: founders.iter().map(|r| r.material_in).sum(),
            energy_in: founders.iter().map(|r| r.energy_in).sum(),
            cells: founders.len() as u32,
            ..CareApplied::default()
        })),
        Err(reason) => receipt(CareOutcome::Rejected(reason)),
    }
}

/// The receipt's quantities, for the journal's diagnostic record and for the viewer.
fn applied_json(command: &care::PlannedCommand, outcome: &CareOutcome) -> serde_json::Value {
    match outcome.applied() {
        None => serde_json::Value::Null,
        Some(q) if command.kind == care::CareKind::SpawnApex => serde_json::json!({
            "material_in": q.material_in, "energy_in": q.energy_in, "founders": q.cells,
        }),
        Some(q) => care_applied_json(q),
    }
}

pub(super) fn care_applied_json(q: &CareApplied) -> serde_json::Value {
    serde_json::json!({
            "material_in": q.material_in,
            "energy_in": q.energy_in,
            "water_depth": q.water_depth,
            "material_out": q.material_out,
            "energy_out": q.energy_out,
            "cells": q.cells,
            "ends_tick": q.ends_tick,
    })
}

/// Everything `--care` adds to the loop: the service the HTTP server talks to, the journal
/// worker, the recovered schedule, and the hold.
///
/// The whole of the contract's admission protocol lives in [`CareRuntime::boundary`],
/// which the loop calls at each tick boundary and obeys: while it answers `false` the world
/// does not advance, and everything else — rendering, `/frame`, `/status`, `/care/status` —
/// keeps running.
pub(super) struct CareRuntime {
    pub(super) service: care::CareService,
    pub(super) worker: care::JournalWorker,
    /// Receipt-driven presentation only; never persisted or consulted by ecology.
    pub(super) effects: care_effects::CareEffects,
    /// The next sequence number to allocate: after the journal's maximum, not after the
    /// snapshot's cursor. A record in the journal has reserved its sequence already.
    pub(super) next_seq: u64,
    /// Recovered commands still to apply, in sequence order.
    pub(super) replay: std::collections::VecDeque<care::PlannedCommand>,
    /// Handed to the journal worker, not yet acknowledged.
    pub(super) inflight: Vec<care::PlannedCommand>,
    /// The boundary the world is held at.
    pub(super) holding_at: Option<u64>,
    /// Set once an uncertain write has happened. The world never advances again: the only
    /// way out is a clean stop, which still writes the final snapshot at this boundary.
    pub(super) failed: bool,
    /// False without `--care`. Recovery of *already accepted* history never depends on this
    /// flag — a durable command at `B ≥ S` must be replayed whether or not this run is
    /// willing to accept new ones, or restarting without the flag would advance past `B`
    /// and checkpoint a history the journal disagrees with. The flag only decides whether
    /// anything *new* may be admitted.
    pub(super) intake_allowed: bool,
    /// A hold ended this iteration, so the clock owes itself a re-base.
    pub(super) released: bool,
}

impl CareRuntime {
    /// True while the world is held at a boundary and must not advance. The presentation
    /// freezes with it: see the render arm of the loop.
    pub(super) fn is_holding(&self) -> bool {
        self.failed || self.holding_at.is_some()
    }

    /// True when the world may step. Never blocks: a pending commit is discovered by
    /// polling the worker, once per loop iteration.
    pub(super) fn boundary(&mut self, world: &mut World) -> Result<bool> {
        if self.failed {
            return Ok(false);
        }
        self.drain_acks(world)?;
        if self.failed || self.holding_at.is_some() {
            return Ok(false);
        }
        // The recovered schedule owns the boundary until it is exhausted. Intake is gated
        // `replaying` meanwhile, so nothing new can interleave with it.
        if self.apply_replay(world)? {
            return Ok(true);
        }
        if !self.replay.is_empty() || !self.intake_allowed {
            return Ok(true);
        }
        let boundary = world.tick();
        let planned = self.service.drain_prepared(self.next_seq, boundary);
        if planned.is_empty() {
            return Ok(true);
        }
        self.next_seq += planned.len() as u64;
        self.inflight = planned.clone();
        self.holding_at = Some(boundary);
        self.service.hold_at(boundary);
        eprintln!(
            "cubarium: holding at tick {boundary} while {} care command(s) are made durable",
            planned.len()
        );
        if !self.worker.submit(care::JournalJob::Accept(planned)) {
            self.fail(boundary, "the care journal worker is gone");
        }
        Ok(false)
    }

    /// Apply every recovered command whose boundary is this one. Returns true when at least
    /// one was applied.
    pub(super) fn apply_replay(&mut self, world: &mut World) -> Result<bool> {
        let mut applied = false;
        while let Some(head) = self.replay.front() {
            if head.apply_after_tick > world.tick() {
                break;
            }
            // The schedule was validated whole before the first step, so this cannot happen
            // from a well-formed journal; if it does, something else is wrong and stopping
            // is the only answer that does not invent history.
            anyhow::ensure!(
                head.apply_after_tick == world.tick(),
                "care recovery: seq {} belongs at boundary {} but the world is already at \
                 tick {}",
                head.seq,
                head.apply_after_tick,
                world.tick()
            );
            let command = self.replay.pop_front().expect("just inspected");
            let receipt = apply_planned(world, &command);
            if let Some(reason) = receipt.outcome.reason()
                && (reason == "out of order" || reason == "wrong boundary")
            {
                anyhow::bail!(
                    "care recovery: the world refused seq {} at boundary {} with \"{reason}\". \
                     A replayed command the core will not admit is a recovery error, not \
                     something to skip past.",
                    command.seq,
                    command.apply_after_tick
                );
            }
            eprintln!(
                "cubarium: replayed care seq {} ({}) at tick {}: {}",
                command.seq,
                command.kind.as_str(),
                receipt.tick,
                receipt.outcome.as_str()
            );
            self.record(&command, &receipt);
            applied = true;
        }
        if applied && self.replay.is_empty() && self.intake_allowed {
            eprintln!("cubarium: the recovered care schedule is exhausted; care is open");
            self.service.open_intake();
        }
        Ok(applied)
    }

    /// Answer the journal worker. This is where an acknowledgement turns into application.
    pub(super) fn drain_acks(&mut self, world: &mut World) -> Result<()> {
        while let Some(ack) = self.worker.poll_ack() {
            match ack {
                care::JournalAck::Accepted {
                    commands,
                    result: Ok(()),
                } => {
                    // Durable. Only now does anything reach the world.
                    self.service.commit_accepted(&commands);
                    for command in &commands {
                        let receipt = apply_planned(world, command);
                        self.record(command, &receipt);
                    }
                    self.inflight.clear();
                    self.holding_at = None;
                    self.released = true;
                    self.service.release_hold();
                }
                care::JournalAck::Accepted {
                    commands,
                    result: Err(e),
                } if e.is_full() => {
                    // Nothing was attempted, so nothing is uncertain: the sequence numbers
                    // go back, the clients are told, and the world resumes. This is the one
                    // journal failure that does not stop the world.
                    eprintln!("cubarium: care refused: {e}");
                    self.next_seq = self.next_seq.saturating_sub(commands.len() as u64);
                    for command in &commands {
                        self.service.record_outcome(
                            command.seq,
                            "rejected",
                            "journal full",
                            serde_json::Value::Null,
                        );
                    }
                    self.inflight.clear();
                    self.holding_at = None;
                    self.released = true;
                    self.service.release_hold();
                }
                care::JournalAck::Accepted { result: Err(e), .. } => {
                    self.fail(world.tick(), format!("{e}"));
                }
                care::JournalAck::Outcome { result: Err(e) } => {
                    if e.is_full() {
                        // Diagnostic only; replay never needs it.
                        eprintln!("cubarium: could not record a care outcome: {e}");
                    } else {
                        // The hold was already released when this was submitted, so the
                        // world has advanced. It holds *here*, at the tick it has reached.
                        self.fail(world.tick(), format!("{e}"));
                    }
                }
                care::JournalAck::Outcome { result: Ok(()) } => {}
            }
        }
        Ok(())
    }

    /// Hand a receipt to the waiting client and to the journal's diagnostic record.
    fn record(&mut self, command: &care::PlannedCommand, receipt: &CareReceipt) {
        // Both fresh durable application and boundary-correct journal replay enter
        // here. Snapshot history is not replayed, so old inputs do not retrigger.
        if command.kind != care::CareKind::SpawnApex {
            self.effects.observe(&to_core(command), receipt);
        }
        let reason = receipt.outcome.reason().unwrap_or_default().to_string();
        let applied = applied_json(command, &receipt.outcome);
        self.service.record_outcome(
            command.seq,
            receipt.outcome.as_str(),
            &reason,
            applied.clone(),
        );
        self.worker
            .submit(care::JournalJob::Outcome(vec![care::OutcomeRecord {
                seq: command.seq,
                tick: receipt.tick,
                outcome: receipt.outcome.as_str(),
                reason,
                applied,
            }]));
    }

    /// An uncertain write. The world holds where it is for the rest of the process.
    ///
    /// `observed_at` is the world's *actual* tick right now, which is not always the
    /// acceptance boundary: a diagnostic outcome record is written after the hold was
    /// released, so its failure can arrive with the world several ticks further on. The
    /// world holds at the tick it has actually reached, and that is the tick the final
    /// snapshot will carry — claiming otherwise would send an operator looking for a
    /// checkpoint that does not exist.
    pub(super) fn fail(&mut self, observed_at: u64, reason: impl Into<String>) {
        let reason = reason.into();
        if self.failed {
            return;
        }
        self.failed = true;
        self.holding_at = Some(observed_at);
        self.service.hold_at(observed_at);
        eprintln!(
            "cubarium: care failed, observed at tick {observed_at}: {reason}\n\
             cubarium: the record may or may not be on disk, so the world will not advance \
             past tick {observed_at}. Nothing more will be appended to the journal in this \
             process. Stop cleanly (Ctrl-C) — the final snapshot is written at tick \
             {observed_at} — and restart: recovery applies whatever accepted records \
             survived, each at its own boundary."
        );
        self.service.fail(reason);
    }

    /// Whether a hold ended since this was last asked, so the clock can re-base instead of
    /// fast-forwarding through the time the world spent waiting on a disk.
    pub(super) fn take_released(&mut self) -> bool {
        std::mem::take(&mut self.released)
    }
}

/// Open the journal, validate the recovered schedule, and build the runtime. Everything
/// that can refuse to start does so here, before a log, a worker or a sink is opened.
pub(super) fn open_care(
    run: &Run,
    world: &World,
    build_id: &str,
    lock: &std::sync::Arc<state::StateLock>,
) -> Result<CareRuntime> {
    let epoch = care_epoch();
    let (topology, scale) = (world.topology(), world.scale());
    let journal = care::Journal::open(&run.state, &epoch, build_id, topology)?;
    let admitted = world.care().admitted_seq;
    let plan = journal.replay_plan(admitted, world.tick())?;
    // After the journal's maximum, never after the snapshot's cursor: the journal has
    // already reserved those numbers even where the world has not reached them yet.
    let next_seq = journal.max_seq().unwrap_or(admitted).max(admitted) + 1;
    if !plan.is_empty() {
        eprintln!(
            "cubarium: {} care command(s) to replay from {}, seq {}..={} at boundaries {}..={}",
            plan.len(),
            journal.path().display(),
            plan[0].seq,
            plan[plan.len() - 1].seq,
            plan[0].apply_after_tick,
            plan[plan.len() - 1].apply_after_tick,
        );
    }
    let status = journal.status();
    let worker = care::JournalWorker::spawn_holding(journal, Some(std::sync::Arc::clone(lock)));
    let service = care::CareService::for_world(epoch, status, topology);
    Ok(CareRuntime {
        service,
        worker,
        effects: care_effects::CareEffects::new(topology, scale),
        next_seq,
        replay: plan.into(),
        inflight: Vec::new(),
        holding_at: None,
        failed: false,
        intake_allowed: run.care,
        released: false,
    })
}
