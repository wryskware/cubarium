//! Transport-free study readouts. Poll life events every tick, sample activity at 1 Hz,
//! and take minute snapshots. No simulation rule or saved-world schema is changed.
//!
//! Body-time means individual seconds (not organic mass). Minute energy compares only
//! identities alive at both boundaries, using `Animal::stored_energy` exactly as booked
//! by fauna, including gut, stomach, seeds and escrow. Ledger deltas are organic units.
//! LifeRecorder infers parents/causes from simultaneous transitions; these are observed
//! histories, not authoritative ancestry. Meal intake is its inferred stomach increases,
//! including open bouts at death/shutdown; simultaneous digestion can underestimate it.

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
}

#[derive(Clone, Debug)]
pub struct AnimalVitals {
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
    pub net_energy_per_body_hour: Option<f64>,
    pub path_m_per_body: Option<f64>,
    pub displacement_m_per_body: Option<f64>,
    pub stuck_n: usize,
    pub stuck_observed_n: usize,
    pub intake_organic: f64,
    pub upkeep_organic: f64,
    pub motor_organic: f64,
}

#[derive(Clone, Debug)]
pub struct PlantVitals {
    pub minute: f64,
    pub species: String,
    pub stands: usize,
    pub vigour: Vec<f64>,
    pub cured_n: usize,
    pub drought_n: usize,
    pub aeration_n: usize,
    pub stressed_n: usize,
}

#[derive(Debug)]
struct Track {
    row: LifeRow,
    open_intake: [f64; Food::COUNT],
    position: [f64; 3],
    history: VecDeque<(f64, [f64; 3])>,
    /// Energy, position and cumulative path at the previous minute boundary.
    boundary: Option<(f64, [f64; 3], f64)>,
}

/// Owns all histories. Keep it across checkpoints; create a new one after loading.
pub struct Recorder {
    options: Options,
    life: LifeRecorder,
    tracks: BTreeMap<u64, Track>,
    dead: Vec<u64>,
    activity: BTreeMap<String, Activity>,
    ledger: [[f64; 3]; Founder::COUNT],
    last_minute: f64,
    last_sample: Option<f64>,
}

fn position(a: &Animal) -> [f64; 3] {
    [a.pose.x, a.pose.y_m, a.pose.z]
}
fn distance(a: [f64; 3], b: [f64; 3]) -> f64 {
    a.into_iter()
        .zip(b)
        .map(|(x, y)| (x - y).powi(2))
        .sum::<f64>()
        .sqrt()
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

/// A complete trailing window and a currently active body are both required. The
/// caller excludes feeding, sleeping, voluntarily resting/perching and attached bodies.
fn stuck(
    history: &VecDeque<(f64, [f64; 3])>,
    now: f64,
    pos: [f64; 3],
    length: f64,
    options: Options,
) -> Option<bool> {
    let &(then, before) = history.front()?;
    if now - then + f64::EPSILON < options.stuck_window_min * 60.0 {
        return None;
    }
    Some(distance(before, pos) < options.stuck_body_lengths * length)
}

impl Recorder {
    pub fn new(options: Options, fauna: &Fauna, minute: f64) -> Self {
        assert!(options.stuck_window_min.is_finite() && options.stuck_window_min > 0.0);
        assert!(options.stuck_body_lengths.is_finite() && options.stuck_body_lengths >= 0.0);
        let mut r = Self {
            options,
            life: LifeRecorder::new(),
            tracks: BTreeMap::new(),
            dead: Vec::new(),
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
                    },
                    open_intake: [0.0; Food::COUNT],
                    position: pos,
                    history: VecDeque::new(),
                    boundary: None,
                }
            });
            track.row.path_m += distance(track.position, pos);
            track.position = pos;
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
                track.boundary = None;
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
            t.history.push_back((seconds, position(a)));
            let cutoff = seconds - self.options.stuck_window_min * 60.0;
            while t.history.len() > 1 && t.history[1].0 <= cutoff {
                t.history.pop_front();
            }
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
            t.boundary = Some((a.stored_energy(), position(a), t.row.path_m));
        }
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
                net_energy_per_body_hour: None,
                path_m_per_body: None,
                displacement_m_per_body: None,
                stuck_n: 0,
                stuck_observed_n: 0,
                intake_organic: 0.0,
                upkeep_organic: 0.0,
                motor_organic: 0.0,
            };
            let mut survivors = 0;
            let (mut energy, mut path, mut displacement) = (0.0, 0.0, 0.0);
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
                if let Some((before, pos, distance_before)) = t.boundary {
                    survivors += 1;
                    energy += a.stored_energy() - before;
                    path += t.row.path_m - distance_before;
                    displacement += distance(pos, position(a));
                }
                if !a.eating
                    && a.rest.is_none()
                    && a.attached.is_none()
                    && !matches!(a.state, State::Asleep | State::Resting)
                    && a.mobility.mode != MobilityMode::Perched
                    && let Some(body) = cubarium_voxel_fauna::body_of(fauna.config(), a)
                    && let Some(is_stuck) = stuck(
                        &t.history,
                        minute * 60.0,
                        position(a),
                        body.length_m,
                        self.options,
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
                r.net_energy_per_body_hour = Some(energy / r.energy_body_hours);
            }
            if survivors > 0 {
                r.path_m_per_body = Some(path / survivors as f64);
                r.displacement_m_per_body = Some(displacement / survivors as f64);
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

pub const VITALS_HEADER: &str = "minute,lineage,n,n_rider,n_pup,n_juvenile,n_adult,native_n,fullness_mean,fullness_p10,fullness_p50,fullness_p90,stomach_fill_mean,stomach_fill_p50,wound_mean,wounded_n,size_mean,energy_per_body_mean,emergency_n,eating_n,asleep_n,gestating_or_clutch_n,breeding_eligible_n,body_s,awake_body_s,moving_share,feeding_share,resting_share,fleeing_share,hunting_share,climbing_share,gliding_share,flying_share,energy_body_hours,net_energy_per_body_hour,path_m_per_body,displacement_m_per_body,stuck_n,stuck_observed_n,intake_organic,upkeep_organic,motor_organic";
pub const PLANTS_HEADER: &str = "minute,species,stands,vigour_mean,vigour_p10,vigour_p50,cured_n,drought_n,aeration_n,stressed_n";

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
            val(r.net_energy_per_body_hour),
            val(r.path_m_per_body),
            val(r.displacement_m_per_body),
            r.stuck_n.to_string(),
            r.stuck_observed_n.to_string(),
            r.intake_organic.to_string(),
            r.upkeep_organic.to_string(),
            r.motor_organic.to_string(),
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
            ],
        )
    }
}
impl LifeRow {
    pub fn write_header(out: &mut impl Write) -> io::Result<()> {
        let mut c: Vec<String> = "id,lineage,native,origin,parent_id,generation,observed_minute,born_minute,matured_minute,independent_minute,died_minute,last_seen_minute,alive,cause,killer_lineage,adult,meals".split(',').map(str::to_string).collect();
        c.extend(Food::ALL.map(|f| format!("intake_{}", f.name())));
        c.extend("kills,offspring,organic_given,peak_size,min_fullness,path_m,awake_body_s,moving_share,feeding_share".split(',').map(str::to_string));
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
        csv(out, c)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quantiles_interpolate_known_inputs() {
        assert_eq!(quantile(&[], 0.5), None);
        assert_eq!(quantile(&[7.0], 0.1), Some(7.0));
        assert_eq!(quantile(&[10.0, 0.0, 5.0], 0.5), Some(5.0));
        assert_eq!(quantile(&[10.0, 0.0], 0.1), Some(1.0));
    }

    #[test]
    fn stuck_requires_complete_window_and_displacement_under_length() {
        let opts = Options {
            stuck_window_min: 2.0 / 60.0,
            stuck_body_lengths: 1.0,
        };
        let history = VecDeque::from([(0.0, [0.0; 3]), (1.0, [0.0; 3])]);
        assert_eq!(stuck(&history, 1.0, [0.0; 3], 0.5, opts), None);
        assert_eq!(stuck(&history, 2.0, [0.0; 3], 0.5, opts), Some(true));
        assert_eq!(
            stuck(&history, 2.0, [1.0, 0.0, 0.0], 0.5, opts),
            Some(false)
        );
    }

    #[test]
    fn snapshots_and_life_csv_follow_hand_built_bodies_and_events() {
        let (_, flora, fauna) = crate::tests::small_world();
        let options = Options {
            stuck_window_min: 2.0 / 60.0,
            stuck_body_lengths: 1.0,
        };
        let mut recorder = Recorder::new(options, &fauna, 0.0);
        recorder.sample(&fauna, 1.0);
        let (rows, plants) = recorder.minute(&flora, &fauna, 1.0 / 60.0);
        let browser = rows
            .iter()
            .find(|r| r.lineage == Founder::Browser.name())
            .unwrap();
        assert_eq!(browser.n, fauna.view().animals.len());
        assert_eq!(browser.net_energy_per_body_hour, Some(0.0));
        assert_eq!(browser.path_m_per_body, Some(0.0));
        assert_eq!(browser.activity.body_s, fauna.view().animals.len() as f64);
        let mut bytes = Vec::new();
        browser.write_csv(&mut bytes).unwrap();
        assert_eq!(
            String::from_utf8(bytes).unwrap().trim().split(',').count(),
            VITALS_HEADER.split(',').count()
        );
        assert_eq!(
            plants.iter().map(|r| r.stands).sum::<usize>(),
            flora.view().stands.len()
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
    }

    #[test]
    fn a_real_departure_is_recorded_once_with_its_ledger_cause() {
        let (world, _, mut fauna) = crate::tests::small_world();
        let options = Options {
            stuck_window_min: 1.0,
            stuck_body_lengths: 1.0,
        };
        let mut recorder = Recorder::new(options, &fauna, 0.0);
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
