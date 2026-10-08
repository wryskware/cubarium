//! Transport-free study readouts. Poll life events every tick, sample activity at 1 Hz,
//! and take minute snapshots. No simulation rule or saved-world schema is changed.
//!
//! Body-time means individual seconds (not organic mass). Ledger deltas are organic units.
//! LifeRecorder infers parents/causes from simultaneous transitions; these are observed
//! histories, not authoritative ancestry. Meal intake is its inferred stomach increases,
//! including open bouts at death/shutdown; simultaneous digestion can underestimate it.
//!
//! Two energy readouts per lineage and minute, both per body-hour:
//! - `survivor_stock_change_per_body_hour`: only identities alive at both minute ends,
//!   gross `Animal::stored_energy` (energy currency, gut, stomach, seeds and escrow
//!   included), no offspring transfer. It hides deaths and charges reproduction.
//! - `cohort_energy_change`: the conserved budget of the cohort alive at the minute's
//!   start, in organic units (the currency `reproduction.given` is booked in): `Σ(E_end −
//!   E_start)` with `E = body + reserve + escrow.organic` (the training reward's `E`) and
//!   `E_end = 0` for a body that died in the minute (its whole store counted as lost),
//!   plus the organic matter each cohort body passed to offspring in the minute; over
//!   cohort bodies × minute hours.
//!
//! Distances wrap in x around the ring (`ring_m`, the world's width × voxel size), so a
//! seam crossing is a short step. `displacement_m_per_body` is net displacement from the
//! minute's start; `delivered_bl_per_s` is that displacement in each survivor's current
//! body lengths, summed and divided by the survivors' awake body-seconds in the minute
//! (pacing on the spot scores near zero; `moving_share` stays the state-based share).
//!
//! **Stuck** (per body, at each minute): the trailing window (`stuck_window_min`) must be
//! fully observed; at every 1 Hz sample in it and now the body was *active* — awake, not
//! voluntarily resting (asleep, hunkered, or standing in `State::Resting` with neither
//! the eating nor the emergency latch on), not perched, not attached — and it
//! swallowed nothing in the window (no stomach pool rose: an actual bite, not the eating
//! latch). Such a body is *eligible*; it is stuck if its wrapped net displacement over the
//! window is under `stuck_body_lengths` current body lengths. A lineage with no eligible
//! body writes `stuck_n` blank (NA) beside `stuck_observed_n = 0`.
//!
//! Every row carries `segment`: 0 for a fresh founding, then +1 per load or resume (the
//! caller decides; see `voxel_census`).

use std::collections::{BTreeMap, VecDeque};
use std::io::{self, Write};

use cubarium_voxel_fauna::{Animal, DT, Fauna, Food, Founder, MobilityMode, State, Steer};
use cubarium_voxel_flora::{Flora, Species as Plant};

use crate::life::{LifeEvent, LifeRecorder, What};
use crate::query::{self, LifeStage};

/// Diagnostic choices, independent of ecological configuration.
#[derive(Clone, Copy, Debug)]
pub struct Options {
    /// Required observed history, simulated minutes; incomplete windows never count.
    pub stuck_window_min: f64,
    /// Net displacement below this many current body lengths counts as stuck.
    pub stuck_body_lengths: f64,
}

#[derive(Clone, Debug, Default)]
pub struct Activity {
    pub body_s: f64,
    pub awake_body_s: f64,
    pub moving_s: f64,
    pub feeding_s: f64,
    pub resting_s: f64,
    pub fleeing_s: f64,
    pub hunting_s: f64,
    pub climbing_s: f64,
    pub gliding_s: f64,
    pub flying_s: f64,
}

/// One body, with observed birth dates distinguished from inferred ages at a load.
#[derive(Clone, Debug)]
pub struct LifeRow {
    pub id: u64,
    pub lineage: String,
    pub native: bool,
    pub origin: String,
    pub parent_id: Option<u64>,
    pub generation: u64,
    pub observed_minute: f64,
    pub born_minute: f64,
    pub matured_minute: Option<f64>,
    pub independent_minute: Option<f64>,
    pub died_minute: Option<f64>,
    pub last_seen_minute: f64,
    pub cause: String,
    pub killer_lineage: String,
    pub adult: bool,
    pub meals: u64,
    pub intake_organic: [f64; Food::COUNT],
    pub kills: u64,
    pub offspring: u64,
    pub organic_given: f64,
    pub peak_size: f64,
    pub min_fullness: f64,
    pub path_m: f64,
    pub activity: Activity,
    pub segment: u32,
}

#[derive(Clone, Debug)]
pub struct AnimalVitals {
    pub segment: u32,
    pub minute: f64,
    pub lineage: String,
    pub n: usize,
    /// Rider, pup, juvenile, adult (query's LifeStage order).
    pub n_by_stage: [usize; 4],
    pub native_n: usize,
    pub fullness: Vec<f64>,
    pub stomach_fill: Vec<f64>,
    pub wound_mean: f64,
    pub wounded_n: usize,
    pub size_mean: f64,
    pub energy_per_body_mean: f64,
    pub emergency_n: usize,
    pub eating_n: usize,
    pub asleep_n: usize,
    pub gestating_or_clutch_n: usize,
    pub breeding_eligible_n: usize,
    pub activity: Activity,
    pub energy_body_hours: f64,
    pub survivor_stock_change_per_body_hour: Option<f64>,
    /// Organic units per cohort body-hour (module docs).
    pub cohort_energy_change: Option<f64>,
    pub path_m_per_body: Option<f64>,
    pub displacement_m_per_body: Option<f64>,
    pub delivered_bl_per_s: Option<f64>,
    pub stuck_n: usize,
    pub stuck_observed_n: usize,
    pub intake_organic: f64,
    pub upkeep_organic: f64,
    pub motor_organic: f64,
}

#[derive(Clone, Debug)]
pub struct PlantVitals {
    pub segment: u32,
    pub minute: f64,
    pub species: String,
    pub stands: usize,
    pub vigour: Vec<f64>,
    pub cured_n: usize,
    pub drought_n: usize,
    pub aeration_n: usize,
    pub stressed_n: usize,
}

/// One 1 Hz stuck-history sample.
#[derive(Clone, Copy, Debug)]
struct Sample {
    seconds: f64,
    pos: [f64; 3],
    /// Awake, not voluntarily resting, not perched, not attached ([`active`]).
    active: bool,
    /// The body's cumulative inferred swallowing at this sample, organic.
    swallowed: f64,
}

/// A body's state at the previous minute boundary.
#[derive(Clone, Copy, Debug)]
struct Boundary {
    stored_energy: f64,
    /// `body + reserve + escrow.organic` (the cohort budget's `E`).
    organic: f64,
    pos: [f64; 3],
    path_m: f64,
    awake_s: f64,
    given: f64,
}

#[derive(Debug)]
struct Track {
    row: LifeRow,
    open_intake: [f64; Food::COUNT],
    position: [f64; 3],
    /// Stomach pools (organic) at the last tick, and the sum of their rises since the
    /// body was first observed: what it swallowed, never the eating latch.
    stomach: [f64; Food::COUNT],
    swallowed: f64,
    history: VecDeque<Sample>,
    boundary: Option<Boundary>,
}

/// Owns all histories. Keep it across checkpoints; create a new one after loading.
pub struct Recorder {
    options: Options,
    /// The ring's circumference in x, metres (`width × voxel_m`); 0 disables wrapping.
    ring_m: f64,
    segment: u32,
    life: LifeRecorder,
    tracks: BTreeMap<u64, Track>,
    dead: Vec<u64>,
    /// Cohort bodies that died since the last boundary: lineage, start `E`, given since.
    died_in_minute: Vec<(String, f64, f64)>,
    activity: BTreeMap<String, Activity>,
    ledger: [[f64; 3]; Founder::COUNT],
    last_minute: f64,
    last_sample: Option<f64>,
}

fn position(a: &Animal) -> [f64; 3] {
    [a.pose.x, a.pose.y_m, a.pose.z]
}
/// Straight-line distance with x wrapped around a ring of `ring_m` metres (no wrap when
/// `ring_m` is not positive).
fn distance(a: [f64; 3], b: [f64; 3], ring_m: f64) -> f64 {
    let mut dx = (a[0] - b[0]).abs();
    if ring_m > 0.0 {
        dx = dx.rem_euclid(ring_m);
        dx = dx.min(ring_m - dx);
    }
    (dx * dx + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt()
}
fn organic_store(a: &Animal) -> f64 {
    a.body + a.reserve + a.reproduction.escrow.map_or(0.0, |e| e.organic)
}
fn stomach_pools(a: &Animal) -> [f64; Food::COUNT] {
    a.stomach.pools.map(|p| p.organic)
}
/// What went into the stomach between two readings: the pools' rises (digestion only
/// lowers a pool).
fn swallowed_between(before: &[f64; Food::COUNT], now: &[f64; Food::COUNT]) -> f64 {
    before.iter().zip(now).map(|(b, n)| (n - b).max(0.0)).sum()
}
/// Eligible for the stuck rule's exposure: awake and not voluntarily resting. Asleep
/// (`rest`), hunkered, perched and attached bodies are resting by choice or carried.
/// `State::Resting` is fauna's "nothing in reach or sense, or nowhere closer to stand",
/// not a chosen rest: it counts as voluntary only for a body with no appetite (eating
/// and emergency latches off); a hungry body standing there is exposed.
fn active(a: &Animal) -> bool {
    a.rest.is_none()
        && a.state != State::Asleep
        && a.attached.is_none()
        && !a.hunkered
        && a.mobility.mode != MobilityMode::Perched
        && (a.state != State::Resting || a.eating || a.emergency)
}
fn ratio(n: f64, d: f64) -> f64 {
    if d > 0.0 { n / d } else { 0.0 }
}
fn mean(v: &[f64]) -> Option<f64> {
    (!v.is_empty()).then(|| v.iter().sum::<f64>() / v.len() as f64)
}
/// Linearly interpolated quantile, including the singleton and empty cases.
pub fn quantile(v: &[f64], q: f64) -> Option<f64> {
    if v.is_empty() {
        return None;
    }
    let mut v = v.to_vec();
    v.sort_by(f64::total_cmp);
    let k = q.clamp(0.0, 1.0) * (v.len() - 1) as f64;
    let lo = k.floor() as usize;
    let hi = k.ceil() as usize;
    Some(v[lo] + (v[hi] - v[lo]) * (k - lo as f64))
}

fn stage_index(s: LifeStage) -> usize {
    match s {
        LifeStage::Rider => 0,
        LifeStage::Pup => 1,
        LifeStage::Juvenile => 2,
        LifeStage::Adult => 3,
    }
}

/// Append a sample and drop all but the latest one at or before the window's start.
fn push_sample(history: &mut VecDeque<Sample>, sample: Sample, window_s: f64) {
    let cutoff = sample.seconds - window_s;
    history.push_back(sample);
    while history.len() > 1 && history[1].seconds <= cutoff {
        history.pop_front();
    }
}

/// The stuck rule (module docs) on a body's trailing history ending at `now`: `None`
/// when the body is not eligible (incomplete window, inactive at any sample or now, or
/// it swallowed in the window), else whether its wrapped net displacement is under the
/// threshold.
#[allow(clippy::too_many_arguments)]
fn stuck(
    history: &VecDeque<Sample>,
    now: f64,
    pos: [f64; 3],
    active_now: bool,
    swallowed_now: f64,
    length: f64,
    options: Options,
    ring_m: f64,
) -> Option<bool> {
    let first = history.front()?;
    if now - first.seconds + f64::EPSILON < options.stuck_window_min * 60.0 {
        return None;
    }
    if !active_now || history.iter().any(|s| !s.active) || swallowed_now > first.swallowed {
        return None;
    }
    Some(distance(first.pos, pos, ring_m) < options.stuck_body_lengths * length)
}

impl Recorder {
    /// `ring_m`: the world's x circumference (`width × voxel_m`); `segment`: written on
    /// every row (module docs).
    pub fn new(options: Options, ring_m: f64, segment: u32, fauna: &Fauna, minute: f64) -> Self {
        assert!(options.stuck_window_min.is_finite() && options.stuck_window_min > 0.0);
        assert!(options.stuck_body_lengths.is_finite() && options.stuck_body_lengths >= 0.0);
        let mut r = Self {
            options,
            ring_m,
            segment,
            life: LifeRecorder::new(),
            tracks: BTreeMap::new(),
            dead: Vec::new(),
            died_in_minute: Vec::new(),
            activity: BTreeMap::new(),
            ledger: [[0.0; 3]; Founder::COUNT],
            last_minute: minute,
            last_sample: None,
        };
        r.observe(fauna, (minute * 60.0 / DT).round() as u64);
        r.sample(fauna, minute * 60.0);
        r.reset_boundary(fauna, minute);
        r
    }

    /// Cheap per-tick life polling and continuous 3-D path integration. Activity and
    /// quantile readouts remain 1 Hz. Nothing in this method mutates fauna.
    pub fn observe(&mut self, fauna: &Fauna, tick: u64) {
        let minute = tick as f64 * DT / 60.0;
        for a in fauna.view().animals {
            let pos = position(a);
            let track = self.tracks.entry(a.id).or_insert_with(|| {
                let row = query::animal_row(fauna, a);
                Track {
                    row: LifeRow {
                        id: a.id,
                        lineage: row.species,
                        native: a.reproduction.native,
                        origin: "observed".into(),
                        parent_id: None,
                        generation: 0,
                        observed_minute: minute,
                        born_minute: minute - a.age_ticks as f64 * DT / 60.0,
                        matured_minute: None,
                        independent_minute: None,
                        died_minute: None,
                        last_seen_minute: minute,
                        cause: String::new(),
                        killer_lineage: String::new(),
                        adult: row.stage == LifeStage::Adult,
                        meals: 0,
                        intake_organic: [0.0; Food::COUNT],
                        kills: 0,
                        offspring: 0,
                        organic_given: a.reproduction.given,
                        peak_size: row.size,
                        min_fullness: row.fullness,
                        path_m: 0.0,
                        activity: Activity::default(),
                        segment: self.segment,
                    },
                    open_intake: [0.0; Food::COUNT],
                    position: pos,
                    stomach: stomach_pools(a),
                    swallowed: 0.0,
                    history: VecDeque::new(),
                    boundary: None,
                }
            });
            track.row.path_m += distance(track.position, pos, self.ring_m);
            track.position = pos;
            let stomach = stomach_pools(a);
            track.swallowed += swallowed_between(&track.stomach, &stomach);
            track.stomach = stomach;
            track.row.last_seen_minute = minute;
            track.row.organic_given = a.reproduction.given;
        }
        // Preserve open-bout intake before disappeared bodies leave the recorder.
        for (id, meal) in self.life.unfinished_meals() {
            if let Some(t) = self.tracks.get_mut(&id) {
                t.open_intake = meal;
            }
        }
        self.life.observe(fauna, tick);
        for event in self.life.take_events() {
            self.event(event);
        }
    }

    fn event(&mut self, event: LifeEvent) {
        let minute = event.tick as f64 * DT / 60.0;
        let generation = match &event.what {
            What::Born {
                parent: Some(id), ..
            } => self.tracks.get(id).map(|p| p.row.generation + 1),
            _ => None,
        };
        let killer = match &event.what {
            What::Died { by: Some(id), .. } => self.tracks.get(id).map(|p| p.row.lineage.clone()),
            _ => None,
        };
        let Some(track) = self.tracks.get_mut(&event.animal) else {
            return;
        };
        let r = &mut track.row;
        match event.what {
            What::Born { parent, .. } => {
                r.origin = "born".into();
                r.born_minute = minute;
                r.parent_id = parent;
                r.generation = generation.unwrap_or(0);
            }
            What::Matured => {
                r.matured_minute = Some(minute);
                r.adult = true;
            }
            What::Independent => r.independent_minute = Some(minute),
            What::Meal { foods, .. } => {
                r.meals += 1;
                track.open_intake = [0.0; Food::COUNT];
                for amount in foods {
                    if let Some(food) = Food::ALL.into_iter().find(|f| f.name() == amount.food) {
                        r.intake_organic[food.index()] += amount.organic;
                    }
                }
            }
            What::Kill { .. } => r.kills += 1,
            What::GaveBirth { young } => r.offspring += young.len() as u64,
            What::LaidEggs { eggs, .. } => r.offspring += u64::from(eggs),
            What::Died { cause, .. } => {
                add_open_intake(r, track.open_intake);
                r.died_minute = Some(minute);
                r.killer_lineage = killer
                    .unwrap_or_else(|| cause.strip_prefix("killed_by_").unwrap_or("").to_string());
                r.cause = cause;
                self.dead.push(r.id);
                track.history.clear();
                if let Some(b) = track.boundary.take() {
                    self.died_in_minute.push((
                        r.lineage.clone(),
                        b.organic,
                        r.organic_given - b.given,
                    ));
                }
            }
            _ => {}
        }
    }

    /// Sample the interval ending at `seconds`; the first call establishes a baseline.
    /// Call at every simulated second, and at an irregular final boundary if necessary.
    pub fn sample(&mut self, fauna: &Fauna, seconds: f64) {
        let dt = self.last_sample.map_or(0.0, |s| seconds - s);
        assert!(dt >= 0.0);
        self.last_sample = Some(seconds);
        for a in fauna.view().animals {
            let row = query::animal_row(fauna, a);
            let t = self.tracks.get_mut(&a.id).expect("observe before sample");
            t.row.peak_size = t.row.peak_size.max(row.size);
            t.row.min_fullness = t.row.min_fullness.min(row.fullness);
            t.row.adult = row.stage == LifeStage::Adult;
            push_sample(
                &mut t.history,
                Sample {
                    seconds,
                    pos: position(a),
                    active: active(a),
                    swallowed: t.swallowed,
                },
                self.options.stuck_window_min * 60.0,
            );
            let dt = dt.min((seconds - t.row.observed_minute * 60.0).max(0.0));
            let awake = a.state != State::Asleep && a.rest.is_none();
            let moving = matches!(a.state, State::Walking | State::Climbing)
                || matches!(a.mobility.mode, MobilityMode::Fly | MobilityMode::Glide);
            let feeding = row.activity.contains("feeding");
            for sum in [
                &mut t.row.activity,
                self.activity.entry(row.species).or_default(),
            ] {
                sum.body_s += dt;
                if !awake {
                    continue;
                }
                sum.awake_body_s += dt;
                sum.moving_s += if moving { dt } else { 0.0 };
                sum.feeding_s += if feeding { dt } else { 0.0 };
                sum.resting_s += if a.state == State::Resting { dt } else { 0.0 };
                sum.fleeing_s += if fauna.steer_of(a.id) == Some(Steer::Flee) {
                    dt
                } else {
                    0.0
                };
                sum.hunting_s +=
                    if a.strike.is_some() || fauna.steer_of(a.id) == Some(Steer::Patrol) {
                        dt
                    } else {
                        0.0
                    };
                sum.climbing_s +=
                    if a.state == State::Climbing || a.mobility.mode == MobilityMode::Climb {
                        dt
                    } else {
                        0.0
                    };
                sum.gliding_s += if a.mobility.mode == MobilityMode::Glide {
                    dt
                } else {
                    0.0
                };
                sum.flying_s += if a.mobility.mode == MobilityMode::Fly {
                    dt
                } else {
                    0.0
                };
            }
        }
    }

    fn reset_boundary(&mut self, fauna: &Fauna, minute: f64) {
        for a in fauna.view().animals {
            let t = self.tracks.get_mut(&a.id).expect("observed body");
            t.boundary = Some(Boundary {
                stored_energy: a.stored_energy(),
                organic: organic_store(a),
                pos: position(a),
                path_m: t.row.path_m,
                awake_s: t.row.activity.awake_body_s,
                given: a.reproduction.given,
            });
        }
        self.died_in_minute.clear();
        let l = fauna.view().ledger;
        for f in Founder::ALL {
            let i = f.index();
            self.ledger[i] = [
                l.swallowed_by_founder[i],
                l.upkeep_by_founder[i],
                l.motor_by_founder[i],
            ];
        }
        self.last_minute = minute;
        self.activity.clear();
    }

    /// Current snapshot and interval totals; taking a snapshot starts the next interval.
    /// Empty distribution means and quantiles serialize as blank, never fabricated zero.
    pub fn minute(
        &mut self,
        flora: &Flora,
        fauna: &Fauna,
        minute: f64,
    ) -> (Vec<AnimalVitals>, Vec<PlantVitals>) {
        let mut animals = Vec::new();
        let mut lineages: BTreeMap<String, Option<Founder>> = Founder::ALL
            .into_iter()
            .map(|f| (f.name().into(), Some(f)))
            .collect();
        for a in fauna.view().animals {
            if a.founder.is_none() {
                lineages.insert(a.species.name().into(), None);
            }
        }
        let elapsed = (minute - self.last_minute) / 60.0;
        for (lineage, founder) in lineages {
            let mut r = AnimalVitals {
                segment: self.segment,
                minute,
                lineage,
                n: 0,
                n_by_stage: [0; 4],
                native_n: 0,
                fullness: Vec::new(),
                stomach_fill: Vec::new(),
                wound_mean: 0.0,
                wounded_n: 0,
                size_mean: 0.0,
                energy_per_body_mean: 0.0,
                emergency_n: 0,
                eating_n: 0,
                asleep_n: 0,
                gestating_or_clutch_n: 0,
                breeding_eligible_n: 0,
                activity: Activity::default(),
                energy_body_hours: 0.0,
                survivor_stock_change_per_body_hour: None,
                cohort_energy_change: None,
                path_m_per_body: None,
                displacement_m_per_body: None,
                delivered_bl_per_s: None,
                stuck_n: 0,
                stuck_observed_n: 0,
                intake_organic: 0.0,
                upkeep_organic: 0.0,
                motor_organic: 0.0,
            };
            let mut survivors = 0;
            let (mut energy, mut path, mut displacement) = (0.0, 0.0, 0.0);
            let (mut cohort, mut cohort_n) = (0.0, 0usize);
            let (mut delivered_bl, mut delivered_awake_s) = (0.0, 0.0);
            for (lineage, start, given) in &self.died_in_minute {
                if *lineage == r.lineage {
                    cohort_n += 1;
                    cohort += given - start;
                }
            }
            for a in fauna.view().animals {
                let t = &self.tracks[&a.id];
                if t.row.lineage != r.lineage {
                    continue;
                }
                let row = query::animal_row(fauna, a);
                r.n += 1;
                r.n_by_stage[stage_index(row.stage)] += 1;
                r.native_n += usize::from(a.reproduction.native);
                r.fullness.push(row.fullness);
                let capacity = a
                    .founder
                    .map_or(0.0, |f| fauna.config().founder(f).gut.capacity(a.body));
                r.stomach_fill.push(ratio(a.stomach.organic(), capacity));
                r.wound_mean += row.wound;
                r.wounded_n += usize::from(row.wound > 0.0);
                r.size_mean += row.size;
                r.energy_per_body_mean += ratio(row.energy, a.body);
                r.emergency_n += usize::from(a.emergency);
                r.eating_n += usize::from(a.eating);
                r.asleep_n += usize::from(a.state == State::Asleep || a.rest.is_some());
                // Fauna has gestation escrow and deposited clutches, no carried-clutch state.
                r.gestating_or_clutch_n += usize::from(a.reproduction.escrow.is_some());
                r.breeding_eligible_n += usize::from(a.reproduction.eligible_now);
                let length = cubarium_voxel_fauna::body_of(fauna.config(), a).map(|b| b.length_m);
                if let Some(b) = t.boundary {
                    survivors += 1;
                    energy += a.stored_energy() - b.stored_energy;
                    path += t.row.path_m - b.path_m;
                    let moved = distance(b.pos, position(a), self.ring_m);
                    displacement += moved;
                    cohort_n += 1;
                    cohort += organic_store(a) - b.organic + (a.reproduction.given - b.given);
                    if let Some(length) = length.filter(|l| *l > 0.0) {
                        delivered_bl += moved / length;
                        delivered_awake_s += t.row.activity.awake_body_s - b.awake_s;
                    }
                }
                if let Some(length) = length
                    && let Some(is_stuck) = stuck(
                        &t.history,
                        minute * 60.0,
                        position(a),
                        active(a),
                        t.swallowed,
                        length,
                        self.options,
                        self.ring_m,
                    )
                {
                    r.stuck_observed_n += 1;
                    r.stuck_n += usize::from(is_stuck);
                }
            }
            r.wound_mean = ratio(r.wound_mean, r.n as f64);
            r.size_mean = ratio(r.size_mean, r.n as f64);
            r.energy_per_body_mean = ratio(r.energy_per_body_mean, r.n as f64);
            r.activity = self.activity.remove(&r.lineage).unwrap_or_default();
            r.energy_body_hours = survivors as f64 * elapsed;
            if r.energy_body_hours > 0.0 {
                r.survivor_stock_change_per_body_hour = Some(energy / r.energy_body_hours);
            }
            let cohort_hours = cohort_n as f64 * elapsed;
            if cohort_hours > 0.0 {
                r.cohort_energy_change = Some(cohort / cohort_hours);
            }
            if survivors > 0 {
                r.path_m_per_body = Some(path / survivors as f64);
                r.displacement_m_per_body = Some(displacement / survivors as f64);
            }
            if delivered_awake_s > 0.0 {
                r.delivered_bl_per_s = Some(delivered_bl / delivered_awake_s);
            }
            if let Some(f) = founder {
                let i = f.index();
                let l = fauna.view().ledger;
                r.intake_organic = l.swallowed_by_founder[i] - self.ledger[i][0];
                r.upkeep_organic = l.upkeep_by_founder[i] - self.ledger[i][1];
                r.motor_organic = l.motor_by_founder[i] - self.ledger[i][2];
            }
            animals.push(r);
        }
        let view = flora.view();
        let plants = Plant::ALL
            .into_iter()
            .map(|s| {
                let mut r = PlantVitals {
                    segment: self.segment,
                    minute,
                    species: s.name().into(),
                    stands: 0,
                    vigour: Vec::new(),
                    cured_n: 0,
                    drought_n: 0,
                    aeration_n: 0,
                    stressed_n: 0,
                };
                for stand in view.stands.iter().filter(|a| a.species == s) {
                    r.stands += 1;
                    r.vigour.push(ratio(
                        stand.reserve,
                        view.config.species(s).reserve_cap * stand.wood,
                    ));
                    r.cured_n += usize::from(stand.cured);
                    r.drought_n += usize::from(stand.dry_s > 0.0);
                    r.aeration_n += usize::from(stand.aeration_stress > 0.0);
                    r.stressed_n += usize::from(
                        stand.cured || stand.dry_s > 0.0 || stand.aeration_stress > 0.0,
                    );
                }
                r
            })
            .collect();
        self.reset_boundary(fauna, minute);
        (animals, plants)
    }

    pub fn lives(&self) -> impl Iterator<Item = LifeRow> + '_ {
        self.tracks.values().map(|t| {
            let mut row = t.row.clone();
            if row.died_minute.is_none() {
                let meal = self.life.unfinished_meal(row.id);
                add_open_intake(&mut row, meal);
            }
            row
        })
    }
    /// Deaths are emitted once; final living rows are written separately at shutdown.
    pub fn take_dead(&mut self) -> Vec<LifeRow> {
        std::mem::take(&mut self.dead)
            .into_iter()
            .map(|id| self.tracks[&id].row.clone())
            .collect()
    }
}

fn add_open_intake(row: &mut LifeRow, meal: [f64; Food::COUNT]) {
    if meal.iter().any(|v| *v > 0.0) {
        row.meals += 1;
    }
    for (sum, amount) in row.intake_organic.iter_mut().zip(meal) {
        *sum += amount;
    }
}

fn val(v: Option<f64>) -> String {
    v.map_or_else(String::new, |n| n.to_string())
}
fn csv(out: &mut impl Write, cells: impl IntoIterator<Item = String>) -> io::Result<()> {
    let mut first = true;
    for cell in cells {
        if !first {
            write!(out, ",")?;
        }
        first = false;
        if cell.contains([',', '"', '\n', '\r']) {
            write!(out, "\"{}\"", cell.replace('"', "\"\""))?;
        } else {
            write!(out, "{cell}")?;
        }
    }
    writeln!(out)
}

pub const VITALS_HEADER: &str = "minute,lineage,n,n_rider,n_pup,n_juvenile,n_adult,native_n,fullness_mean,fullness_p10,fullness_p50,fullness_p90,stomach_fill_mean,stomach_fill_p50,wound_mean,wounded_n,size_mean,energy_per_body_mean,emergency_n,eating_n,asleep_n,gestating_or_clutch_n,breeding_eligible_n,body_s,awake_body_s,moving_share,feeding_share,resting_share,fleeing_share,hunting_share,climbing_share,gliding_share,flying_share,energy_body_hours,survivor_stock_change_per_body_hour,path_m_per_body,displacement_m_per_body,stuck_n,stuck_observed_n,intake_organic,upkeep_organic,motor_organic,delivered_bl_per_s,cohort_energy_change,segment";
pub const PLANTS_HEADER: &str = "minute,species,stands,vigour_mean,vigour_p10,vigour_p50,cured_n,drought_n,aeration_n,stressed_n,segment";

impl AnimalVitals {
    pub fn write_csv(&self, out: &mut impl Write) -> io::Result<()> {
        let r = self;
        let a = &r.activity;
        let mut c = vec![r.minute.to_string(), r.lineage.clone(), r.n.to_string()];
        c.extend(r.n_by_stage.map(|n| n.to_string()));
        c.push(r.native_n.to_string());
        c.extend(
            [
                mean(&r.fullness),
                quantile(&r.fullness, 0.1),
                quantile(&r.fullness, 0.5),
                quantile(&r.fullness, 0.9),
                mean(&r.stomach_fill),
                quantile(&r.stomach_fill, 0.5),
            ]
            .map(val),
        );
        c.extend([
            val((r.n > 0).then_some(r.wound_mean)),
            r.wounded_n.to_string(),
            val((r.n > 0).then_some(r.size_mean)),
            val((r.n > 0).then_some(r.energy_per_body_mean)),
            r.emergency_n.to_string(),
            r.eating_n.to_string(),
            r.asleep_n.to_string(),
            r.gestating_or_clutch_n.to_string(),
            r.breeding_eligible_n.to_string(),
            a.body_s.to_string(),
            a.awake_body_s.to_string(),
        ]);
        c.extend(
            [
                a.moving_s,
                a.feeding_s,
                a.resting_s,
                a.fleeing_s,
                a.hunting_s,
                a.climbing_s,
                a.gliding_s,
                a.flying_s,
            ]
            .map(|n| val((a.awake_body_s > 0.0).then(|| n / a.awake_body_s))),
        );
        c.extend([
            r.energy_body_hours.to_string(),
            val(r.survivor_stock_change_per_body_hour),
            val(r.path_m_per_body),
            val(r.displacement_m_per_body),
            val((r.stuck_observed_n > 0).then_some(r.stuck_n as f64)),
            r.stuck_observed_n.to_string(),
            r.intake_organic.to_string(),
            r.upkeep_organic.to_string(),
            r.motor_organic.to_string(),
            val(r.delivered_bl_per_s),
            val(r.cohort_energy_change),
            r.segment.to_string(),
        ]);
        csv(out, c)
    }
}
impl PlantVitals {
    pub fn write_csv(&self, out: &mut impl Write) -> io::Result<()> {
        csv(
            out,
            [
                self.minute.to_string(),
                self.species.clone(),
                self.stands.to_string(),
                val(mean(&self.vigour)),
                val(quantile(&self.vigour, 0.1)),
                val(quantile(&self.vigour, 0.5)),
                self.cured_n.to_string(),
                self.drought_n.to_string(),
                self.aeration_n.to_string(),
                self.stressed_n.to_string(),
                self.segment.to_string(),
            ],
        )
    }
}
impl LifeRow {
    pub fn write_header(out: &mut impl Write) -> io::Result<()> {
        let mut c: Vec<String> = "id,lineage,native,origin,parent_id,generation,observed_minute,born_minute,matured_minute,independent_minute,died_minute,last_seen_minute,alive,cause,killer_lineage,adult,meals".split(',').map(str::to_string).collect();
        c.extend(Food::ALL.map(|f| format!("intake_{}", f.name())));
        c.extend("kills,offspring,organic_given,peak_size,min_fullness,path_m,awake_body_s,moving_share,feeding_share,segment".split(',').map(str::to_string));
        csv(out, c)
    }
    pub fn write_csv(&self, out: &mut impl Write) -> io::Result<()> {
        let r = self;
        let mut c = vec![
            r.id.to_string(),
            r.lineage.clone(),
            r.native.to_string(),
            r.origin.clone(),
            r.parent_id.map_or_else(String::new, |n| n.to_string()),
            r.generation.to_string(),
            r.observed_minute.to_string(),
            r.born_minute.to_string(),
            val(r.matured_minute),
            val(r.independent_minute),
            val(r.died_minute),
            r.last_seen_minute.to_string(),
            r.died_minute.is_none().to_string(),
            r.cause.clone(),
            r.killer_lineage.clone(),
            r.adult.to_string(),
            r.meals.to_string(),
        ];
        c.extend(r.intake_organic.map(|n| n.to_string()));
        c.extend([
            r.kills.to_string(),
            r.offspring.to_string(),
            r.organic_given.to_string(),
            r.peak_size.to_string(),
            r.min_fullness.to_string(),
            r.path_m.to_string(),
            r.activity.awake_body_s.to_string(),
        ]);
        c.extend(
            [r.activity.moving_s, r.activity.feeding_s]
                .map(|n| val((r.activity.awake_body_s > 0.0).then(|| n / r.activity.awake_body_s))),
        );
        c.push(r.segment.to_string());
        csv(out, c)
    }
}

/// What a resumed segment keeps of an existing readout file (`resume=`): the header and
/// every row the checkpoint at `minute` already accounts for. `kind` says which file.
/// Errors when the existing header is not `header` (a different binary wrote it).
/// Returns the kept text and whether a row at exactly `minute` survived (the resumed
/// segment then does not write its boundary row again).
pub fn keep_through(existing: &str, header: &str, kind: Readout, minute: f64) -> Result<(String, bool), String> {
    let mut lines = existing.lines();
    let found = lines.next().unwrap_or("");
    if found != header {
        return Err(format!("existing header differs from this binary's ({found:?})"));
    }
    let cols: Vec<&str> = header.split(',').collect();
    let col = |name: &str| cols.iter().position(|c| *c == name).expect("readout column");
    let mut out = format!("{header}\n");
    let mut at_minute = false;
    for line in lines.filter(|l| !l.trim().is_empty()) {
        // Lineage and species names hold no commas; life rows quote nothing numeric.
        let cells: Vec<&str> = line.split(',').collect();
        let num = |i: usize| cells.get(i).and_then(|c| c.parse::<f64>().ok());
        let keep = match kind {
            Readout::Minutes => match num(col("minute")) {
                Some(m) => {
                    at_minute |= m == minute;
                    m <= minute
                }
                None => false,
            },
            // A death the checkpoint holds, or a living row written when that segment
            // ended at or before it.
            Readout::Life => match num(col("died_minute")) {
                Some(d) => d <= minute,
                None => num(col("last_seen_minute")).is_some_and(|m| m <= minute),
            },
        };
        if keep {
            out.push_str(line);
            out.push('\n');
        }
    }
    Ok((out, at_minute))
}

/// Which readout [`keep_through`] filters.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Readout {
    /// `vitals.csv` or `plants_vitals.csv`: rows keyed by `minute`.
    Minutes,
    /// `life.csv`: rows keyed by `died_minute`, else `last_seen_minute`.
    Life,
}

/// The `life.csv` header as one line.
pub fn life_header() -> String {
    let mut bytes = Vec::new();
    LifeRow::write_header(&mut bytes).expect("in-memory write");
    String::from_utf8(bytes).expect("ascii header").trim_end().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The small test world's ring: 12 cells of 0.25 m.
    const RING_M: f64 = 3.0;

    fn opts(window_s: f64) -> Options {
        Options {
            stuck_window_min: window_s / 60.0,
            stuck_body_lengths: 1.0,
        }
    }
    fn at(seconds: f64, x: f64, active: bool, swallowed: f64) -> Sample {
        Sample {
            seconds,
            pos: [x, 0.0, 0.0],
            active,
            swallowed,
        }
    }
    fn history(samples: &[Sample], window_s: f64) -> VecDeque<Sample> {
        let mut h = VecDeque::new();
        for s in samples {
            push_sample(&mut h, *s, window_s);
        }
        h
    }

    #[test]
    fn quantiles_interpolate_known_inputs() {
        assert_eq!(quantile(&[], 0.5), None);
        assert_eq!(quantile(&[7.0], 0.1), Some(7.0));
        assert_eq!(quantile(&[10.0, 0.0, 5.0], 0.5), Some(5.0));
        assert_eq!(quantile(&[10.0, 0.0], 0.1), Some(1.0));
    }

    #[test]
    fn a_hungry_blocked_body_is_stuck_and_one_that_bites_is_not_eligible() {
        let o = opts(2.0);
        // Pressed against a wall: awake and active the whole window, never moving.
        let h = history(&[at(0.0, 1.0, true, 0.0), at(1.0, 1.0, true, 0.0), at(2.0, 1.0, true, 0.0)], 2.0);
        assert_eq!(stuck(&h, 2.0, [1.0, 0.0, 0.0], true, 0.0, 0.5, o, RING_M), Some(true));
        // The same body taking a bite in the window: feeding, so not exposed.
        let h = history(&[at(0.0, 1.0, true, 0.0), at(1.0, 1.0, true, 0.2), at(2.0, 1.0, true, 0.2)], 2.0);
        assert_eq!(stuck(&h, 2.0, [1.0, 0.0, 0.0], true, 0.2, 0.5, o, RING_M), None);
        // A body that travelled more than its length is eligible and not stuck.
        let h = history(&[at(0.0, 1.0, true, 0.0), at(2.0, 1.6, true, 0.0)], 2.0);
        assert_eq!(stuck(&h, 2.0, [1.6, 0.0, 0.0], true, 0.0, 0.5, o, RING_M), Some(false));
    }

    #[test]
    fn a_just_woken_body_is_eligible_only_after_a_full_awake_window() {
        let o = opts(2.0);
        let mut h = history(&[at(0.0, 1.0, false, 0.0), at(1.0, 1.0, true, 0.0), at(2.0, 1.0, true, 0.0)], 2.0);
        // The window starts while it was asleep.
        assert_eq!(stuck(&h, 2.0, [1.0, 0.0, 0.0], true, 0.0, 0.5, o, RING_M), None);
        push_sample(&mut h, at(3.0, 1.0, true, 0.0), 2.0);
        assert_eq!(stuck(&h, 3.0, [1.0, 0.0, 0.0], true, 0.0, 0.5, o, RING_M), Some(true));
        // Lying down now ends exposure, whatever the window says.
        assert_eq!(stuck(&h, 3.0, [1.0, 0.0, 0.0], false, 0.0, 0.5, o, RING_M), None);
        // An incomplete window never counts.
        let h = history(&[at(0.0, 1.0, true, 0.0), at(1.0, 1.0, true, 0.0)], 2.0);
        assert_eq!(stuck(&h, 1.0, [1.0, 0.0, 0.0], true, 0.0, 0.5, o, RING_M), None);
    }

    #[test]
    fn distances_wrap_at_the_seam_and_a_closed_loop_has_no_displacement() {
        let (a, b) = ([RING_M - 0.1, 0.0, 0.0], [0.1, 0.0, 0.0]);
        let step = distance(a, b, RING_M);
        assert!((step - 0.2).abs() < 1e-9, "seam step {step}");
        assert!((distance(a, b, 0.0) - (RING_M - 0.2)).abs() < 1e-9);
        // A body crossing the seam in a short hop is stuck-sized, not ring-sized.
        let o = opts(2.0);
        let h = history(&[at(0.0, a[0], true, 0.0), at(2.0, b[0], true, 0.0)], 2.0);
        assert_eq!(stuck(&h, 2.0, b, true, 0.0, 0.5, o, RING_M), Some(true));
        // A loop around a square: path is its perimeter, displacement zero.
        let corners = [[1.0, 0.0, 1.0], [2.0, 0.0, 1.0], [2.0, 0.0, 2.0], [1.0, 0.0, 2.0], [1.0, 0.0, 1.0]];
        let path: f64 = corners.windows(2).map(|w| distance(w[0], w[1], RING_M)).sum();
        assert!(path > 0.0);
        assert_eq!(distance(corners[0], corners[4], RING_M), 0.0);
    }

    #[test]
    fn swallowing_is_the_stomach_pools_rises_only() {
        let mut before = [0.0; Food::COUNT];
        let mut now = [0.0; Food::COUNT];
        before[0] = 0.5;
        now[0] = 0.4; // digestion
        assert_eq!(swallowed_between(&before, &now), 0.0);
        now[Food::COUNT - 1] = 0.3; // a bite of another food
        assert_eq!(swallowed_between(&before, &now), 0.3);
    }

    #[test]
    fn a_real_bite_removes_a_body_from_stuck_exposure() {
        // Driverless founders stand in `State::Resting` with the eating latch on: hungry
        // and going nowhere, so exposed (the latch is set on their first step).
        let (world, mut flora, mut fauna) = crate::tests::small_world();
        fauna.step(&world, &mut flora);
        let mut recorder = Recorder::new(opts(2.0), RING_M, 0, &fauna, 0.0);
        recorder.sample(&fauna, 1.0);
        recorder.sample(&fauna, 2.0);
        let eligible = |fauna: &Fauna| {
            fauna
                .view()
                .animals
                .iter()
                .filter(|a| active(a) && cubarium_voxel_fauna::body_of(fauna.config(), a).is_some())
                .count()
        };
        let before = eligible(&fauna);
        assert!(before > 0, "the small world needs an active body for this test");
        let (rows, _) = recorder.minute(&flora, &fauna, 2.0 / 60.0);
        let browser = |rows: &[AnimalVitals]| rows.iter().find(|r| r.lineage == Founder::Browser.name()).unwrap().clone();
        assert_eq!(browser(&rows).stuck_observed_n, before);
        assert_eq!(browser(&rows).stuck_n, before, "nobody moved");

        let id = fauna.view().animals.iter().find(|a| active(a)).unwrap().id;
        assert!(fauna.set_arrival_stomach(id, 1.0));
        recorder.observe(&fauna, 1);
        assert!(recorder.tracks[&id].swallowed > 0.0, "the stomach rose");
        recorder.sample(&fauna, 3.0);
        let (rows, _) = recorder.minute(&flora, &fauna, 3.0 / 60.0);
        assert_eq!(browser(&rows).stuck_observed_n, before - 1);
    }

    #[test]
    fn a_load_writes_its_boundary_row_and_a_death_is_a_cohort_loss() {
        let (world, flora, mut fauna) = crate::tests::small_world();
        let load_minute = 7.0;
        let segment = 2;
        let mut recorder = Recorder::new(opts(60.0), RING_M, segment, &fauna, load_minute);
        let (rows, plants) = recorder.minute(&flora, &fauna, load_minute);
        let browser = rows.iter().find(|r| r.lineage == Founder::Browser.name()).unwrap();
        assert_eq!(browser.minute, load_minute);
        assert_eq!(browser.n, fauna.view().animals.len());
        assert!(rows.iter().all(|r| r.segment == segment));
        assert!(plants.iter().all(|p| p.segment == segment && p.minute == load_minute));
        // No body was exposed for a stuck window yet: NA, not zero.
        let mut bytes = Vec::new();
        browser.write_csv(&mut bytes).unwrap();
        let line = String::from_utf8(bytes).unwrap();
        let cells: Vec<_> = line.trim().split(',').collect();
        let header: Vec<_> = VITALS_HEADER.split(',').collect();
        assert_eq!(cells.len(), header.len());
        assert_eq!(cells[header.iter().position(|c| *c == "stuck_n").unwrap()], "");
        assert_eq!(cells[header.iter().position(|c| *c == "segment").unwrap()], segment.to_string());

        let n = fauna.view().animals.len();
        let lost: f64 = fauna.view().animals.iter().filter(|a| a.site.x == 2 && a.site.z == 3 && a.attached.is_none()).map(organic_store).sum();
        assert!(fauna.apply(&world, cubarium_voxel_fauna::Command::Remove { x: 2, z: 3 }));
        recorder.observe(&fauna, (load_minute * 60.0 / DT) as u64 + 1);
        let end = load_minute + 1.0;
        recorder.sample(&fauna, end * 60.0);
        let (rows, _) = recorder.minute(&flora, &fauna, end);
        let browser = rows.iter().find(|r| r.lineage == Founder::Browser.name()).unwrap();
        // Nobody stepped: survivors' stocks are unchanged and only the death moves the
        // cohort budget, by the dead body's whole store over the cohort's body-hours.
        assert_eq!(browser.survivor_stock_change_per_body_hour, Some(0.0));
        let expected = -lost / (n as f64 / 60.0);
        let got = browser.cohort_energy_change.unwrap();
        assert!((got - expected).abs() < 1e-9 * expected.abs().max(1.0), "{got} vs {expected}");
    }

    #[test]
    fn a_resume_keeps_what_the_checkpoint_holds_so_each_minute_appears_once() {
        let header = "minute,lineage,segment";
        // Segment 0 checkpointed at minute 4 and wrote minutes 5 and 6 before it crashed.
        let existing: String = (0..=6).map(|m| format!("{m},browser,0\n")).collect();
        let (kept, at_minute) =
            keep_through(&format!("{header}\n{existing}"), header, Readout::Minutes, 4.0).unwrap();
        assert!(at_minute);
        // Segment 1 resumes at 4, skips its boundary row (already present) and appends.
        let resumed: String = (5..=6).map(|m| format!("{m},browser,1\n")).collect();
        let joined = format!("{kept}{resumed}");
        let minutes: Vec<u32> = joined.lines().skip(1).map(|l| l.split(',').next().unwrap().parse().unwrap()).collect();
        assert_eq!(minutes, (0..=6).collect::<Vec<_>>());
        assert_eq!(joined.lines().filter(|l| *l == header).count(), 1);
        assert!(keep_through("other\n", header, Readout::Minutes, 4.0).is_err());

        let life = life_header();
        let cols: Vec<&str> = life.split(',').collect();
        let row = |died: &str, last_seen: f64| {
            let mut c = vec![String::new(); cols.len()];
            c[cols.iter().position(|c| *c == "died_minute").unwrap()] = died.into();
            c[cols.iter().position(|c| *c == "last_seen_minute").unwrap()] = last_seen.to_string();
            c.join(",")
        };
        let existing = [row("3", 3.0), row("5", 5.0), row("", 4.0), row("", 6.0)].join("\n");
        let (kept, _) = keep_through(&format!("{life}\n{existing}\n"), &life, Readout::Life, 4.0).unwrap();
        let kept: Vec<_> = kept.lines().skip(1).map(str::to_string).collect();
        assert_eq!(kept, vec![row("3", 3.0), row("", 4.0)]);
    }

    #[test]
    fn life_csv_follows_hand_built_bodies_and_events() {
        let (_, flora, fauna) = crate::tests::small_world();
        let mut recorder = Recorder::new(opts(2.0), RING_M, 0, &fauna, 0.0);
        recorder.sample(&fauna, 1.0);
        let (rows, plants) = recorder.minute(&flora, &fauna, 1.0 / 60.0);
        let browser = rows
            .iter()
            .find(|r| r.lineage == Founder::Browser.name())
            .unwrap();
        assert_eq!(browser.n, fauna.view().animals.len());
        assert_eq!(browser.survivor_stock_change_per_body_hour, Some(0.0));
        assert_eq!(browser.cohort_energy_change, Some(0.0));
        assert_eq!(browser.path_m_per_body, Some(0.0));
        assert_eq!(browser.activity.body_s, fauna.view().animals.len() as f64);
        assert_eq!(
            plants.iter().map(|r| r.stands).sum::<usize>(),
            flora.view().stands.len()
        );
        let mut bytes = Vec::new();
        plants[0].write_csv(&mut bytes).unwrap();
        assert_eq!(
            String::from_utf8(bytes).unwrap().trim().split(',').count(),
            PLANTS_HEADER.split(',').count()
        );

        let id = fauna.view().animals[0].id;
        let partial_intake = 0.03;
        recorder.tracks.get_mut(&id).unwrap().open_intake[Food::Litter.index()] = partial_intake;
        recorder.event(LifeEvent {
            tick: 2,
            animal: id,
            what: What::Died {
                cause: "drowned".into(),
                by: None,
            },
        });
        let dead = recorder.take_dead();
        assert_eq!(dead.len(), 1);
        assert_eq!(dead[0].cause, "drowned");
        assert_eq!(dead[0].intake_organic[Food::Litter.index()], partial_intake);
        assert_eq!(dead[0].meals, 1);
        assert_eq!(
            recorder
                .lives()
                .find(|r| r.id == id)
                .unwrap()
                .intake_organic[Food::Litter.index()],
            partial_intake
        );
        assert!(recorder.take_dead().is_empty());
        let mut bytes = Vec::new();
        LifeRow::write_header(&mut bytes).unwrap();
        dead[0].write_csv(&mut bytes).unwrap();
        let csv = String::from_utf8(bytes).unwrap();
        let lines: Vec<_> = csv.lines().collect();
        assert_eq!(lines.len(), 2);
        let header: Vec<_> = lines[0].split(',').collect();
        let row: Vec<_> = lines[1].split(',').collect();
        assert_eq!(header.len(), row.len());
        assert_eq!(
            row[header.iter().position(|s| *s == "cause").unwrap()],
            "drowned"
        );
        assert_eq!(
            row[header.iter().position(|s| *s == "alive").unwrap()],
            "false"
        );
        assert_eq!(row[header.iter().position(|s| *s == "segment").unwrap()], "0");
    }

    #[test]
    fn a_real_departure_is_recorded_once_with_its_ledger_cause() {
        let (world, _, mut fauna) = crate::tests::small_world();
        let mut recorder = Recorder::new(opts(60.0), RING_M, 0, &fauna, 0.0);
        let before = fauna.view().animals.len();
        assert!(fauna.apply(&world, cubarium_voxel_fauna::Command::Remove { x: 2, z: 3 }));
        recorder.observe(&fauna, 1);
        let dead = recorder.take_dead();
        assert_eq!(dead.len(), before - fauna.view().animals.len());
        assert!(
            dead.iter()
                .all(|r| r.cause == "removed" && r.died_minute.is_some())
        );
        recorder.observe(&fauna, 2);
        assert!(recorder.take_dead().is_empty());
    }
}
