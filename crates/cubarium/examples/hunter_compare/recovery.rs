//! Observer-only recovery measurements. No World, RNG, or mutable simulation state.
//! Capture batches must precede the same tick's census; all times are absolute ticks.

use serde::Serialize;
use std::collections::{BTreeSet, VecDeque};

pub const ARMS: usize = 6;
pub const CADENCE: u64 = 200;
pub const CAPTURE_BIN: u64 = 12_000;
pub const LOCAL_HORIZON: u64 = 72_000;
pub const LOCAL_HOLD: u64 = 1_200;
pub const MILESTONES: [u64; 4] = [1_200, 6_000, 18_000, 72_000];
pub type Counts = [Vec<u32>; ARMS];

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Serialize)]
pub struct CaptureId {
    pub hunter_slot: u32,
    pub hunter_generation: u32,
    pub attempt: u64,
}

#[derive(Clone, Debug, Serialize)]
pub struct Capture {
    pub arm: usize,
    pub cell: usize,
    pub id: CaptureId,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LocalStatus {
    Recovered,
    NoMeasuredDeficit,
    InsufficientPre,
    NotRecoveredBy3600s,
    RightCensored,
}

#[derive(Clone, Debug, Serialize)]
pub struct Milestone {
    pub requested_tick: u64,
    /// Last cadence sample at or before the requested endpoint; never future data.
    pub observed_tick: u64,
    pub counts: [u64; ARMS],
    /// None when the selected capture lacks a complete pre-reference.
    pub differences_in_change: Option<[f64; ARMS]>,
}

#[derive(Clone, Debug, Serialize)]
pub struct LocalRecord {
    pub capture: Capture,
    pub capture_tick: u64,
    pub bin: u64,
    pub pre_samples: usize,
    pub pre_means: Option<[f64; ARMS]>,
    pub initial_post_step_counts: [u64; ARMS],
    pub target: Option<u64>,
    pub status: LocalStatus,
    pub crossing_tick: Option<u64>,
    pub confirmation_tick: Option<u64>,
    pub observed_until_tick: u64,
    pub closed_tick: u64,
    pub termination_reason: Option<String>,
    /// Other captures in these cells, separated by arm; excludes the index capture.
    pub recurrent_exposures: [u64; ARMS],
    pub milestones: Vec<Milestone>,
}

struct Window {
    record: LocalRecord,
    cells: Vec<usize>,
    deadline: u64,
    qualifying_since: Option<u64>,
    last_sample: Option<(u64, [u64; ARMS])>,
    next_milestone: usize,
}

/// Six prior samples, at most seven active windows per on-arm, and streaming outputs.
pub struct LocalRecovery {
    opening: u64,
    neighborhoods: Vec<Vec<usize>>,
    on_arms: BTreeSet<usize>,
    history: VecDeque<(u64, Counts)>,
    last_sample: Option<u64>,
    last_batch: Option<u64>,
    last_bin: [Option<u64>; ARMS],
    windows: Vec<Window>,
    pub captures_seen: [u64; ARMS],
    pub selected_captures: [u64; ARMS],
    finished: bool,
}

fn add_tick(tick: u64, delta: u64) -> Result<u64, String> {
    tick.checked_add(delta)
        .ok_or_else(|| "recovery tick overflow".into())
}

fn totals(cells: &[usize], counts: &Counts) -> [u64; ARMS] {
    std::array::from_fn(|arm| cells.iter().map(|&cell| u64::from(counts[arm][cell])).sum())
}

impl LocalRecovery {
    pub fn new(
        opening_tick: u64,
        neighborhoods: Vec<Vec<usize>>,
        on_arms: Vec<usize>,
    ) -> Result<Self, String> {
        let n = neighborhoods.len();
        if n == 0 || n > 65_536 {
            return Err("invalid recovery cell count".into());
        }
        for (center, cells) in neighborhoods.iter().enumerate() {
            let unique: BTreeSet<_> = cells.iter().copied().collect();
            if unique.len() != cells.len()
                || !unique.contains(&center)
                || cells.iter().any(|&c| c >= n)
            {
                return Err(
                    "neighborhood must contain its center and unique in-range cells".into(),
                );
            }
        }
        let arms: BTreeSet<_> = on_arms.iter().copied().collect();
        if arms.is_empty() || arms.len() != on_arms.len() || arms.iter().any(|&a| a >= ARMS) {
            return Err("invalid on-arm indices".into());
        }
        Ok(Self {
            opening: opening_tick,
            neighborhoods,
            on_arms: arms,
            history: VecDeque::new(),
            last_sample: None,
            last_batch: None,
            last_bin: [None; ARMS],
            windows: Vec::new(),
            captures_seen: [0; ARMS],
            selected_captures: [0; ARMS],
            finished: false,
        })
    }

    fn check_counts(&self, counts: &Counts) -> Result<(), String> {
        if counts
            .iter()
            .any(|arm| arm.len() != self.neighborhoods.len())
        {
            return Err("six-arm cell census has wrong shape".into());
        }
        Ok(())
    }

    fn check_open(&self, tick: u64) -> Result<(), String> {
        if self.finished || tick < self.opening {
            return Err("recovery observer closed or tick before opening".into());
        }
        Ok(())
    }

    /// One batch for all captures at this tick, sorted internally for stable tie selection.
    /// This must run before observe_sample(tick, ..). Empty batches are optional.
    pub fn record_captures(
        &mut self,
        tick: u64,
        captures: &[Capture],
        post: &Counts,
    ) -> Result<Vec<LocalRecord>, String> {
        self.check_open(tick)?;
        self.check_counts(post)?;
        if self.last_batch.is_some_and(|t| tick <= t) || self.last_sample.is_some_and(|t| tick <= t)
        {
            return Err("capture batches must increase and precede same-tick samples".into());
        }
        let next_sample = match self.last_sample {
            Some(t) => add_tick(t, CADENCE)?,
            None => self.opening,
        };
        if tick > next_sample {
            return Err("capture batch skipped a required census sample".into());
        }
        add_tick(tick, LOCAL_HORIZON)?;
        let mut unique = BTreeSet::new();
        for c in captures {
            if !self.on_arms.contains(&c.arm)
                || c.cell >= self.neighborhoods.len()
                || !unique.insert((c.arm, c.id))
            {
                return Err("invalid or duplicate capture in batch".into());
            }
        }
        self.last_batch = Some(tick);
        let out = self.expire_before(tick);
        for window in &mut self.windows {
            for c in captures {
                if window.cells.contains(&c.cell) {
                    window.record.recurrent_exposures[c.arm] += 1;
                }
            }
        }
        let mut sorted: Vec<_> = captures.iter().collect();
        sorted.sort_by_key(|c| (c.arm, c.id));
        for c in sorted {
            self.captures_seen[c.arm] += 1;
            let bin = (tick - self.opening) / CAPTURE_BIN;
            if self.last_bin[c.arm] == Some(bin) {
                continue;
            }
            self.last_bin[c.arm] = Some(bin);
            self.selected_captures[c.arm] += 1;
            let cells = self.neighborhoods[c.cell].clone();
            let initial = totals(&cells, post);
            let complete_pre = tick - self.opening >= 6 * CADENCE
                && self.history.len() == 6
                && self
                    .history
                    .back()
                    .is_some_and(|(last, _)| *last < tick && tick - last <= CADENCE);
            let pre = complete_pre.then(|| {
                let mut sums = [0_u64; ARMS];
                for (_, sample) in &self.history {
                    let values = totals(&cells, sample);
                    for a in 0..ARMS {
                        sums[a] += values[a];
                    }
                }
                sums.map(|sum| sum as f64 / 6.0)
            });
            let target = pre.map(|means| means[c.arm].ceil() as u64);
            let status = if !complete_pre {
                LocalStatus::InsufficientPre
            } else if target == Some(0) || initial[c.arm] >= target.unwrap() {
                LocalStatus::NoMeasuredDeficit
            } else {
                LocalStatus::RightCensored
            };
            let mut record = LocalRecord {
                capture: c.clone(),
                capture_tick: tick,
                bin,
                pre_samples: self.history.len(),
                pre_means: pre,
                initial_post_step_counts: initial,
                target,
                status,
                crossing_tick: None,
                confirmation_tick: None,
                observed_until_tick: tick,
                closed_tick: tick,
                termination_reason: None,
                recurrent_exposures: [0; ARMS],
                milestones: Vec::new(),
            };
            for other in captures {
                if (other.arm != c.arm || other.id != c.id) && cells.contains(&other.cell) {
                    record.recurrent_exposures[other.arm] += 1;
                }
            }
            self.windows.push(Window {
                record,
                cells,
                deadline: tick + LOCAL_HORIZON,
                qualifying_since: None,
                last_sample: None,
                next_milestone: 0,
            });
        }
        debug_assert!(self.windows.len() <= self.on_arms.len() * 7);
        Ok(out)
    }

    /// Samples begin at opening_tick and then arrive without gaps every 200 ticks.
    pub fn observe_sample(
        &mut self,
        tick: u64,
        counts: &Counts,
    ) -> Result<Vec<LocalRecord>, String> {
        self.check_open(tick)?;
        self.check_counts(counts)?;
        let wanted = match self.last_sample {
            Some(t) => add_tick(t, CADENCE)?,
            None => self.opening,
        };
        if tick != wanted || self.last_batch.is_some_and(|t| t > tick) {
            return Err(
                "recovery samples must be contiguous and not precede a later capture batch".into(),
            );
        }
        let mut out = Vec::new();
        let mut keep = Vec::new();
        for mut w in self.windows.drain(..) {
            Self::milestones_until(&mut w, tick.saturating_sub(1));
            if tick > w.deadline {
                out.push(Self::close(w, LocalStatus::NotRecoveredBy3600s, None));
                continue;
            }
            let values = totals(&w.cells, counts);
            w.last_sample = Some((tick, values));
            w.record.observed_until_tick = tick;
            Self::milestones_until(&mut w, tick);
            if w.record.status == LocalStatus::RightCensored
                && values[w.record.capture.arm] >= w.record.target.unwrap()
            {
                let start = *w.qualifying_since.get_or_insert(tick);
                if tick - start >= LOCAL_HOLD {
                    w.record.crossing_tick = Some(start);
                    w.record.confirmation_tick = Some(tick);
                    w.record.closed_tick = tick;
                    w.record.status = LocalStatus::Recovered;
                }
            } else if w.record.status == LocalStatus::RightCensored {
                w.qualifying_since = None;
            }
            if tick == w.deadline {
                out.push(Self::close(w, LocalStatus::NotRecoveredBy3600s, None));
            } else {
                keep.push(w);
            }
        }
        self.windows = keep;
        self.history.push_back((tick, counts.clone()));
        if self.history.len() > 6 {
            self.history.pop_front();
        }
        self.last_sample = Some(tick);
        Ok(out)
    }

    fn milestones_until(w: &mut Window, through: u64) {
        while w.next_milestone < MILESTONES.len() {
            let requested = w.record.capture_tick + MILESTONES[w.next_milestone];
            if requested > through {
                break;
            }
            if let Some((observed, counts)) = w.last_sample {
                if observed <= requested {
                    w.record.milestones.push(Milestone {
                        requested_tick: requested,
                        observed_tick: observed,
                        counts,
                        differences_in_change: w.record.pre_means.map(|means| {
                            let treated =
                                counts[w.record.capture.arm] as f64 - means[w.record.capture.arm];
                            std::array::from_fn(|a| treated - (counts[a] as f64 - means[a]))
                        }),
                    });
                }
            }
            w.next_milestone += 1;
        }
    }

    fn close(mut w: Window, status: LocalStatus, reason: Option<&str>) -> LocalRecord {
        if w.record.status == LocalStatus::RightCensored {
            w.record.status = status;
            w.record.crossing_tick = w.qualifying_since;
        }
        w.record.closed_tick = w.deadline;
        w.record.termination_reason = reason.map(str::to_owned);
        w.record
    }

    fn expire_before(&mut self, tick: u64) -> Vec<LocalRecord> {
        let mut out = Vec::new();
        let mut keep = Vec::new();
        for mut w in self.windows.drain(..) {
            if w.deadline < tick {
                let deadline = w.deadline;
                Self::milestones_until(&mut w, deadline);
                out.push(Self::close(w, LocalStatus::NotRecoveredBy3600s, None));
            } else {
                keep.push(w);
            }
        }
        self.windows = keep;
        out
    }

    pub fn finish(&mut self, tick: u64, reason: &str) -> Result<Vec<LocalRecord>, String> {
        self.check_open(tick)?;
        if self.last_sample.is_some_and(|t| tick < t) || self.last_batch.is_some_and(|t| tick < t) {
            return Err("finish precedes observed data".into());
        }
        let next_sample = match self.last_sample {
            Some(t) => add_tick(t, CADENCE)?,
            None => self.opening,
        };
        if tick > next_sample {
            return Err("finish skipped a required census sample".into());
        }
        let mut out = self.expire_before(tick);
        for mut w in self.windows.drain(..) {
            Self::milestones_until(&mut w, tick);
            let status = if tick >= w.deadline {
                LocalStatus::NotRecoveredBy3600s
            } else {
                LocalStatus::RightCensored
            };
            w.deadline = tick.min(w.deadline);
            out.push(Self::close(w, status, Some(reason)));
        }
        self.finished = true;
        Ok(out)
    }

    pub fn active_windows(&self) -> usize {
        self.windows.len()
    }
    pub fn retained_samples(&self) -> usize {
        self.history.len()
    }
}

pub const WHOLE_HOLD: u64 = 36_000;
pub const WHOLE_HORIZON: u64 = 432_000;

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum WholeStatus {
    AbsentAtOpening,
    NoDecline,
    Recovered,
    NotRecoveredBy6h,
    RightCensored,
}

#[derive(Clone, Debug, Serialize)]
pub struct WholeRecord {
    pub channel: usize,
    pub opening_count: u32,
    pub minimum_count: u32,
    pub first_zero_tick: Option<u64>,
    pub first_decline_tick: Option<u64>,
    pub crossing_tick: Option<u64>,
    pub confirmation_tick: Option<u64>,
    pub status: WholeStatus,
    /// Coverage for minimum/zero/below-half statistics, which continue after recovery.
    pub census_until_tick: u64,
    /// Coverage of the first-decline recovery window, not necessarily the whole run.
    pub observed_until_tick: u64,
    pub closed_tick: u64,
    pub termination_reason: String,
    pub ticks_below_half: u64,
}

struct WholeChannel {
    record: WholeRecord,
    candidate: Option<u64>,
    deadline: Option<u64>,
}

/// One per arm; caller labels channels (e.g. total prey followed by eight forms).
/// Tickwise counts give exact decline/zero/crossing ticks and detect between-sample dips.
pub struct WholeRecovery {
    opening: u64,
    last_tick: u64,
    channels: Vec<WholeChannel>,
}

impl WholeRecovery {
    pub fn new(opening_tick: u64, counts: &[u32]) -> Result<Self, String> {
        if counts.is_empty() || counts.len() > 65 {
            return Err("invalid whole recovery channel count".into());
        }
        add_tick(opening_tick, WHOLE_HORIZON)?;
        Ok(Self {
            opening: opening_tick,
            last_tick: opening_tick,
            channels: counts
                .iter()
                .enumerate()
                .map(|(channel, &count)| WholeChannel {
                    record: WholeRecord {
                        channel,
                        opening_count: count,
                        minimum_count: count,
                        first_zero_tick: (count == 0).then_some(opening_tick),
                        first_decline_tick: None,
                        crossing_tick: None,
                        confirmation_tick: None,
                        status: if count == 0 {
                            WholeStatus::AbsentAtOpening
                        } else {
                            WholeStatus::NoDecline
                        },
                        census_until_tick: opening_tick,
                        observed_until_tick: opening_tick,
                        closed_tick: opening_tick,
                        termination_reason: String::new(),
                        ticks_below_half: 0,
                    },
                    candidate: None,
                    deadline: None,
                })
                .collect(),
        })
    }

    pub fn observe_tick(&mut self, tick: u64, counts: &[u32]) -> Result<(), String> {
        if tick != add_tick(self.last_tick, 1)? || counts.len() != self.channels.len() {
            return Err("whole recovery requires contiguous tickwise counts of fixed shape".into());
        }
        add_tick(tick, WHOLE_HORIZON)?;
        for (c, &count) in self.channels.iter_mut().zip(counts) {
            let r = &mut c.record;
            r.census_until_tick = tick;
            r.minimum_count = r.minimum_count.min(count);
            if count == 0 && r.first_zero_tick.is_none() {
                r.first_zero_tick = Some(tick);
            }
            if u64::from(count) * 2 < u64::from(r.opening_count) {
                r.ticks_below_half += 1;
            }
            if r.opening_count == 0
                || r.status == WholeStatus::Recovered
                || r.status == WholeStatus::NotRecoveredBy6h
            {
                continue;
            }
            if r.first_decline_tick.is_none() && u64::from(count) * 2 < u64::from(r.opening_count) {
                r.first_decline_tick = Some(tick);
                c.deadline = Some(tick + WHOLE_HORIZON);
                r.status = WholeStatus::RightCensored;
            }
            let Some(deadline) = c.deadline else {
                continue;
            };
            if tick <= deadline {
                r.observed_until_tick = tick;
                if u64::from(count) * 10 >= u64::from(r.opening_count) * 9 {
                    let start = *c.candidate.get_or_insert(tick);
                    if tick - start >= WHOLE_HOLD && (tick - self.opening) % CADENCE == 0 {
                        r.crossing_tick = Some(start);
                        r.confirmation_tick = Some(tick);
                        r.closed_tick = tick;
                        r.status = WholeStatus::Recovered;
                    }
                } else {
                    c.candidate = None;
                }
            }
            if tick >= deadline && r.status != WholeStatus::Recovered {
                r.status = WholeStatus::NotRecoveredBy6h;
                r.crossing_tick = c.candidate;
                r.closed_tick = deadline;
            }
        }
        self.last_tick = tick;
        Ok(())
    }

    /// Non-consuming summary permits staged 2h/24h/72h checkpoints without resetting follow-up.
    pub fn summary(&self, reason: &str) -> Vec<WholeRecord> {
        self.channels
            .iter()
            .map(|c| {
                let mut r = c.record.clone();
                r.termination_reason = reason.into();
                if !matches!(
                    r.status,
                    WholeStatus::Recovered | WholeStatus::NotRecoveredBy6h
                ) {
                    r.observed_until_tick = self.last_tick;
                    r.closed_tick = self.last_tick;
                    r.crossing_tick = c.candidate;
                }
                r
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn counts(n: u32) -> Counts {
        std::array::from_fn(|_| vec![n, n])
    }
    fn capture(arm: usize, attempt: u64) -> Capture {
        Capture {
            arm,
            cell: 0,
            id: CaptureId {
                hunter_slot: 1,
                hunter_generation: 2,
                attempt,
            },
        }
    }
    fn prepared() -> LocalRecovery {
        let mut r = LocalRecovery::new(0, vec![vec![0], vec![1]], vec![3, 5]).unwrap();
        for tick in (0..=1200).step_by(200) {
            r.observe_sample(tick, &counts(2)).unwrap();
        }
        r
    }
    #[test]
    fn strict_prior_history_and_same_tick_order() {
        let mut r = prepared();
        let c = capture(3, 1);
        assert!(r.record_captures(1200, &[c.clone()], &counts(1)).is_err());
        r.record_captures(1400, &[c], &counts(1)).unwrap();
        r.observe_sample(1400, &counts(100)).unwrap();
        let result = r.finish(1400, "end").unwrap();
        assert_eq!(result[0].pre_means, Some([2.0; ARMS]));
        assert_eq!(result[0].status, LocalStatus::RightCensored);
    }
    #[test]
    fn first_capture_per_bin_has_stable_tie_and_counts_recurrences() {
        let mut r = prepared();
        r.record_captures(1201, &[capture(3, 9), capture(3, 2)], &counts(1))
            .unwrap();
        r.record_captures(1202, &[capture(3, 1)], &counts(1))
            .unwrap();
        let records = r.finish(1202, "end").unwrap();
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].capture.id.attempt, 2);
        assert_eq!(records[0].recurrent_exposures[3], 2);
        assert_eq!(r.captures_seen[3], 3);
        assert_eq!(r.selected_captures[3], 1);
    }
    #[test]
    fn missing_pre_and_no_deficit_are_not_recoveries() {
        let mut r = LocalRecovery::new(0, vec![vec![0], vec![1]], vec![3]).unwrap();
        assert!(
            r.record_captures(0, &[capture(3, 1)], &counts(1))
                .unwrap()
                .is_empty()
        );
        let records = r.finish(0, "end").unwrap();
        assert_eq!(records[0].status, LocalStatus::InsufficientPre);
        let mut r = prepared();
        assert!(
            r.record_captures(1201, &[capture(3, 1)], &counts(2))
                .unwrap()
                .is_empty()
        );
        let records = r.finish(1201, "end").unwrap();
        assert_eq!(records[0].status, LocalStatus::NoMeasuredDeficit);
    }
    #[test]
    fn hold_resets_and_unconfirmed_crossing_is_censored() {
        let mut r = prepared();
        r.record_captures(1201, &[capture(3, 1)], &counts(1))
            .unwrap();
        for tick in (1400..=2200).step_by(200) {
            assert!(r.observe_sample(tick, &counts(2)).unwrap().is_empty());
        }
        r.observe_sample(2400, &counts(1)).unwrap();
        for tick in (2600..=3600).step_by(200) {
            assert!(r.observe_sample(tick, &counts(2)).unwrap().is_empty());
        }
        let records = r.finish(3600, "end").unwrap();
        assert_eq!(records[0].crossing_tick, Some(2600));
        assert_eq!(records[0].confirmation_tick, None);
        assert_eq!(records[0].status, LocalStatus::RightCensored);
    }
    #[test]
    fn recovery_requires_full_hold_and_uses_each_controls_own_pre() {
        let mut r = prepared();
        r.record_captures(1201, &[capture(3, 1)], &counts(1))
            .unwrap();
        for tick in (1400..=2600).step_by(200) {
            assert!(r.observe_sample(tick, &counts(2)).unwrap().is_empty());
        }
        let records = r.finish(2600, "end").unwrap();
        assert_eq!(records[0].status, LocalStatus::Recovered);
        assert_eq!(records[0].confirmation_tick, Some(2600));
        assert_eq!(records[0].milestones[0].requested_tick, 2401);
        assert_eq!(records[0].milestones[0].observed_tick, 2400);
        assert_eq!(
            records[0].milestones[0].differences_in_change,
            Some([0.0; ARMS])
        );
    }
    #[test]
    fn horizon_is_not_extended_to_next_aligned_sample() {
        let mut r = prepared();
        r.record_captures(1201, &[capture(3, 1)], &counts(1))
            .unwrap();
        let mut completed = Vec::new();
        for tick in (1400..=73_400).step_by(200) {
            completed.extend(r.observe_sample(tick, &counts(1)).unwrap());
        }
        assert_eq!(completed.len(), 1);
        let record = &completed[0];
        assert_eq!(record.status, LocalStatus::NotRecoveredBy3600s);
        assert_eq!(record.closed_tick, 73_201);
        assert_eq!(record.observed_until_tick, 73_200);
        assert_eq!(record.milestones.len(), 4);
        assert_eq!(record.milestones[3].observed_tick, 73_200);
    }
    #[test]
    fn memory_is_bounded_even_with_dense_captures() {
        let mut r = prepared();
        let mut maximum = 0;
        for tick in 1201..=100_000 {
            r.record_captures(tick, &[capture(3, tick)], &counts(0))
                .unwrap();
            if tick % 200 == 0 {
                r.observe_sample(tick, &counts(1)).unwrap();
            }
            assert!(r.active_windows() <= 7);
            maximum = maximum.max(r.active_windows());
            assert!(r.retained_samples() <= 6);
        }
        assert_eq!(maximum, 7, "the bound must actually be exercised");
    }
    #[test]
    fn malformed_input_is_refused_before_observation() {
        assert!(LocalRecovery::new(0, vec![vec![0, 0]], vec![3]).is_err());
        let mut r = prepared();
        assert!(r.observe_sample(1600, &counts(2)).is_err());
        assert!(
            r.record_captures(1201, &[capture(3, 1), capture(3, 1)], &counts(1))
                .is_err()
        );
        assert!(r.record_captures(u64::MAX, &[], &counts(1)).is_err());
        assert_eq!(r.captures_seen, [0; ARMS]);
    }
    #[test]
    fn whole_thresholds_are_strict_and_zero_is_not_an_opening_loss() {
        let mut r = WholeRecovery::new(0, &[10, 0]).unwrap();
        r.observe_tick(1, &[5, 0]).unwrap();
        assert_eq!(r.summary("test")[0].first_decline_tick, None);
        r.observe_tick(2, &[4, 0]).unwrap();
        r.observe_tick(3, &[0, 0]).unwrap();
        let records = r.summary("end");
        assert_eq!(records[0].first_decline_tick, Some(2));
        assert_eq!(records[0].first_zero_tick, Some(3));
        assert_eq!(records[0].status, WholeStatus::RightCensored);
        assert_eq!(records[1].status, WholeStatus::AbsentAtOpening);
    }
    #[test]
    fn whole_recovery_counts_between_sample_dips_and_confirms_on_cadence() {
        let mut r = WholeRecovery::new(0, &[10]).unwrap();
        r.observe_tick(1, &[4]).unwrap();
        for tick in 2..=36_200 {
            r.observe_tick(tick, &[if tick == 199 { 8 } else { 9 }])
                .unwrap();
        }
        let record = &r.summary("end")[0];
        assert_eq!(record.status, WholeStatus::Recovered);
        assert_eq!(record.crossing_tick, Some(200));
        assert_eq!(record.confirmation_tick, Some(36_200));
    }
    #[test]
    fn whole_horizon_and_shape_are_explicit() {
        let mut r = WholeRecovery::new(0, &[10]).unwrap();
        assert!(r.observe_tick(2, &[4]).is_err());
        assert!(r.observe_tick(1, &[4, 0]).is_err());
        for tick in 1..=432_001 {
            r.observe_tick(tick, &[4]).unwrap();
        }
        let record = &r.summary("end")[0];
        assert_eq!(record.status, WholeStatus::NotRecoveredBy6h);
        assert_eq!(record.closed_tick, 432_001);
    }
    #[test]
    fn each_arm_uses_its_own_reference_and_graph_cells() {
        let mut r = LocalRecovery::new(0, vec![vec![0, 1], vec![1, 0]], vec![3]).unwrap();
        let mut prior = counts(2);
        prior[0] = vec![5, 5];
        for tick in (0..=1200).step_by(200) {
            r.observe_sample(tick, &prior).unwrap();
        }
        let mut after = prior.clone();
        after[3] = vec![1, 2];
        r.record_captures(1201, &[capture(3, 1)], &after).unwrap();
        after[0] = vec![4, 5];
        for tick in (1400..=2600).step_by(200) {
            r.observe_sample(tick, &after).unwrap();
        }
        let records = r.finish(2600, "end").unwrap();
        assert_eq!(records[0].pre_means.unwrap()[0], 10.0);
        assert_eq!(records[0].pre_means.unwrap()[3], 4.0);
        let change = records[0].milestones[0].differences_in_change.unwrap();
        assert_eq!(change[0], 0.0); // both treated and control lost one
        assert_eq!(change[1], -1.0); // this control stayed unchanged
        assert_eq!(change[3], 0.0); // own-arm reference is always zero
    }
    #[test]
    fn six_samples_without_sixty_seconds_are_insufficient() {
        let mut r = LocalRecovery::new(0, vec![vec![0], vec![1]], vec![3]).unwrap();
        for tick in (0..=1000).step_by(200) {
            r.observe_sample(tick, &counts(5)).unwrap();
        }
        assert!(
            r.record_captures(1001, &[capture(3, 1)], &counts(1))
                .unwrap()
                .is_empty()
        );
        let records = r.finish(1001, "end").unwrap();
        assert_eq!(records[0].pre_samples, 6);
        assert_eq!(records[0].status, LocalStatus::InsufficientPre);
        let mut r = LocalRecovery::new(0, vec![vec![0], vec![1]], vec![3]).unwrap();
        for tick in (0..=1200).step_by(200) {
            r.observe_sample(tick, &counts(5)).unwrap();
        }
        r.record_captures(1201, &[capture(3, 1)], &counts(1))
            .unwrap();
        assert_eq!(r.finish(1201, "end").unwrap()[0].target, Some(5));
    }
    #[test]
    fn confirmed_recovery_keeps_followup_and_later_exposures() {
        let mut r = prepared();
        r.record_captures(1201, &[capture(3, 1)], &counts(1))
            .unwrap();
        for tick in (1400..=2600).step_by(200) {
            r.observe_sample(tick, &counts(2)).unwrap();
        }
        assert_eq!(r.active_windows(), 1);
        r.record_captures(2601, &[capture(3, 2)], &counts(1))
            .unwrap();
        let records = r.finish(2601, "end").unwrap();
        assert_eq!(records[0].status, LocalStatus::Recovered);
        assert_eq!(records[0].confirmation_tick, Some(2600));
        assert_eq!(records[0].recurrent_exposures[3], 1);
    }
    #[test]
    fn whole_late_crossing_without_confirmation_stays_censored() {
        let mut r = WholeRecovery::new(0, &[10]).unwrap();
        for tick in 1..=432_001 {
            r.observe_tick(tick, &[if tick >= 396_001 { 9 } else { 4 }])
                .unwrap();
        }
        let record = &r.summary("end")[0];
        assert_eq!(record.crossing_tick, Some(396_001));
        assert_eq!(record.confirmation_tick, None);
        assert_eq!(record.status, WholeStatus::NotRecoveredBy6h);
        assert_eq!(record.census_until_tick, 432_001);
    }
    #[test]
    fn non_deficit_and_insufficient_windows_keep_all_milestones_and_exposures() {
        for insufficient in [true, false] {
            let (mut r, capture_tick, first_sample) = if insufficient {
                let mut observer = LocalRecovery::new(0, vec![vec![0], vec![1]], vec![3]).unwrap();
                observer.observe_sample(0, &counts(2)).unwrap();
                (observer, 1, 200)
            } else {
                (prepared(), 1201, 1400)
            };
            assert!(
                r.record_captures(capture_tick, &[capture(3, 1)], &counts(2))
                    .unwrap()
                    .is_empty()
            );
            let mut records = Vec::new();
            for tick in (first_sample..=capture_tick + LOCAL_HORIZON + CADENCE - 1).step_by(200) {
                if tick == first_sample {
                    records.extend(
                        r.record_captures(tick - 1, &[capture(3, 2)], &counts(1))
                            .unwrap(),
                    );
                }
                records.extend(r.observe_sample(tick, &counts(2)).unwrap());
            }
            assert_eq!(records.len(), 1);
            let record = &records[0];
            assert_eq!(
                record.status,
                if insufficient {
                    LocalStatus::InsufficientPre
                } else {
                    LocalStatus::NoMeasuredDeficit
                }
            );
            assert_eq!(record.confirmation_tick, None);
            assert_eq!(record.milestones.len(), 4);
            assert_eq!(record.recurrent_exposures[3], 1);
            assert_eq!(
                record.milestones[0].differences_in_change.is_none(),
                insufficient
            );
            assert_eq!(record.milestones[0].counts, [2; ARMS]);
        }
    }
}
