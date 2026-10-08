//! **The loop's side**: what a host calls between its ticks to record life histories and,
//! about once a simulated second, hand a [`Publication`] to whoever reads it (the HTTP
//! server, an in-game UI). Transport-free; the host owns when it is called.
//!
//! Cost: [`Observer::after_tick`] is one pass over the animals (the [`LifeRecorder`]'s
//! merge), and [`Observer::publish`] one pass over the animals, the stands, the ground
//! sites and the wet cells. Both measure themselves; [`RunState`] reports it.

use std::time::Instant;

use cubarium_voxel::{DT, World};
use cubarium_voxel_fauna::Fauna;
use cubarium_voxel_flora::Flora;
use serde::{Deserialize, Serialize};

use crate::life::{LifeEvent, LifeRecorder};
use crate::query::{self, AnimalDetail, Environment, Populations, WorldClock, WorldSummary};

/// The host's run controls as it holds them now.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Controls {
    /// World ticks per clock tick: 1 is real time.
    pub speed: f64,
    pub paused: bool,
    /// The range a speed command is clamped to.
    pub min_speed: f64,
    pub max_speed: f64,
}

/// How the run is going: the controls, and what the loop measured since the last
/// publication.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct RunState {
    #[serde(flatten)]
    pub controls: Controls,
    /// Mean milliseconds a tick took to step.
    pub ms_per_tick: f64,
    /// Ticks stepped per wall second, and simulated seconds per wall second (the speed
    /// the loop actually kept up).
    pub ticks_per_s: f64,
    pub sim_s_per_s: f64,
    /// Frames drawn per wall second.
    pub fps: f64,
    /// Wall seconds since the observer was made.
    pub uptime_s: f64,
    /// The observer's own cost: microseconds per tick recording life histories, the last
    /// publication's build, and both as a share of the stepping time.
    pub observe_us_per_tick: f64,
    pub publish_us: f64,
    pub overhead_share: f64,
}

/// Everything a reader needs about the world now.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Snapshot {
    /// Counts publications from 1.
    pub seq: u64,
    pub summary: WorldSummary,
    pub clock: WorldClock,
    pub run: RunState,
    pub populations: Populations,
    pub environment: Environment,
    /// Every living body, by id.
    pub animals: Vec<AnimalDetail>,
}

/// A snapshot and the life events recorded since the last one.
#[derive(Clone, Debug, PartialEq)]
pub struct Publication {
    pub snapshot: Snapshot,
    pub events: Vec<LifeEvent>,
}

/// When the observer publishes.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Cadence {
    /// Simulated seconds between publications while running.
    pub sim_s: f64,
    /// And at least this many wall seconds, so a fast-forwarded run is not published
    /// sixty-four times a wall second.
    pub min_wall_s: f64,
    /// Wall seconds between publications while paused (the run state still moves).
    pub paused_wall_s: f64,
}

impl Default for Cadence {
    fn default() -> Cadence {
        Cadence { sim_s: 1.0, min_wall_s: 0.25, paused_wall_s: 1.0 }
    }
}

/// The loop-side recorder and publisher.
#[derive(Debug)]
pub struct Observer {
    label: String,
    cadence: Cadence,
    recorder: LifeRecorder,
    wet: Vec<usize>,
    started: Instant,
    last_tick: Option<u64>,
    last_wall: Instant,
    seq: u64,
    forced: bool,
    // Since the last publication.
    ticks: u64,
    tick_ns: u64,
    frames: u64,
    observe_ns: u64,
    last_publish_ns: u64,
}

impl Observer {
    /// An observer of a world the host calls `label`.
    pub fn new(label: impl Into<String>, cadence: Cadence) -> Observer {
        let now = Instant::now();
        Observer {
            label: label.into(),
            cadence,
            recorder: LifeRecorder::new(),
            wet: Vec::new(),
            started: now,
            last_tick: None,
            last_wall: now,
            seq: 0,
            forced: true,
            ticks: 0,
            tick_ns: 0,
            frames: 0,
            observe_ns: 0,
            last_publish_ns: 0,
        }
    }

    /// After every step: record what the animals did, and count the tick and what it
    /// cost (`tick_ns`, the host's own measure of the step).
    pub fn after_tick(&mut self, fauna: &Fauna, tick_ns: u64) {
        let at = Instant::now();
        self.recorder.observe(fauna);
        self.observe_ns += at.elapsed().as_nanos() as u64;
        self.ticks += 1;
        self.tick_ns += tick_ns;
    }

    /// After every frame drawn.
    pub fn after_frame(&mut self) {
        self.frames += 1;
    }

    /// Publish at the next chance whatever the cadence says (a command changed the
    /// controls, say).
    pub fn request(&mut self) {
        self.forced = true;
    }

    /// Whether a publication is due at world tick `tick`.
    pub fn due(&self, tick: u64, now: Instant, paused: bool) -> bool {
        let wall = now.duration_since(self.last_wall).as_secs_f64();
        if self.forced {
            return true;
        }
        if paused {
            return wall >= self.cadence.paused_wall_s;
        }
        let sim = self.last_tick.map_or(f64::INFINITY, |t| tick.saturating_sub(t) as f64 * DT);
        (sim >= self.cadence.sim_s && wall >= self.cadence.min_wall_s) || tick < self.last_tick.unwrap_or(0)
    }

    /// Build the publication: the queries over the three layers and the events since the
    /// last one.
    pub fn publish(&mut self, world: &World, flora: &Flora, fauna: &Fauna, controls: Controls, now: Instant) -> Publication {
        let at = Instant::now();
        if !self.recorder.started() {
            // Published before any tick: the bodies standing now are the record's start.
            self.recorder.observe(fauna);
        }
        let wall = now.duration_since(self.last_wall).as_secs_f64().max(1e-9);
        self.seq += 1;
        let populations = query::populations(flora, fauna);
        let environment = query::environment_with(world, &mut self.wet);
        let view = fauna.view();
        let animals = view.animals.iter().filter_map(|a| query::animal(fauna, a.id)).collect();
        let ms_per_tick = if self.ticks > 0 { self.tick_ns as f64 / self.ticks as f64 / 1e6 } else { 0.0 };
        let observe_us = if self.ticks > 0 { self.observe_ns as f64 / self.ticks as f64 / 1e3 } else { 0.0 };
        let mut snapshot = Snapshot {
            seq: self.seq,
            summary: query::summary(world, &self.label),
            clock: query::clock(world),
            run: RunState {
                controls,
                ms_per_tick,
                ticks_per_s: self.ticks as f64 / wall,
                sim_s_per_s: self.ticks as f64 * DT / wall,
                fps: self.frames as f64 / wall,
                uptime_s: now.duration_since(self.started).as_secs_f64(),
                observe_us_per_tick: observe_us,
                publish_us: 0.0,
                overhead_share: 0.0,
            },
            populations,
            environment,
            animals,
        };
        let publish_ns = at.elapsed().as_nanos() as u64;
        snapshot.run.publish_us = publish_ns as f64 / 1e3;
        snapshot.run.overhead_share = if self.tick_ns > 0 {
            (self.observe_ns + self.last_publish_ns) as f64 / self.tick_ns as f64
        } else {
            0.0
        };
        self.last_publish_ns = publish_ns;
        self.last_tick = Some(world.tick());
        self.last_wall = now;
        self.forced = false;
        self.ticks = 0;
        self.tick_ns = 0;
        self.frames = 0;
        self.observe_ns = 0;
        Publication { snapshot, events: self.recorder.take_events() }
    }
}
