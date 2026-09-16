//! Stub: the movement measures, declared so the definition tests in
//! `crates/cubarium-search/tests/movement_measures.rs` compile and fail before any of this is
//! implemented. Replaced in the next commit.

pub const DEPLETION_FRACTION: f64 = crate::evaluate::DEPLETION_FRACTION;
pub const RECOVERY_FRACTION: f64 = crate::evaluate::RECOVERY_FRACTION;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Visit {
    pub cell: u16,
    pub start_probe: u64,
    pub probes: u64,
}

pub fn visits(_probes: &[Option<u16>]) -> Vec<Visit> {
    Vec::new()
}

pub fn residence_ticks(_visits: &[Visit], _probe_ticks: u64) -> Vec<u64> {
    Vec::new()
}

pub fn revisit_intervals(_visits: &[Visit], _probe_ticks: u64) -> Vec<u64> {
    Vec::new()
}

#[derive(Clone, Debug, Default)]
pub struct BodyTrack;

impl BodyTrack {
    pub fn observe(&mut self, _probe_index: u64, _cell: u16) {}
    pub fn finish(&mut self) {}
    pub fn probes_seen(&self) -> u64 {
        0
    }
    pub fn distinct_cells(&self) -> usize {
        0
    }
    pub fn visit_count(&self) -> u64 {
        0
    }
    pub fn mean_residence_ticks(&self, _probe_ticks: u64) -> Option<f64> {
        None
    }
    pub fn mean_revisit_ticks(&self, _probe_ticks: u64) -> Option<f64> {
        None
    }
}

#[derive(Clone, Debug)]
pub struct CrossingCounter;

impl CrossingCounter {
    pub fn new(_p_ref: &[f64], _deplete: f64, _recover: f64) -> Self {
        CrossingCounter
    }
    pub fn observe(&mut self, _p: &[f64]) {}
    pub fn watched(&self) -> u32 {
        0
    }
    pub fn depletions(&self) -> u64 {
        0
    }
    pub fn recoveries(&self) -> u64 {
        0
    }
    pub fn cells_depleted(&self) -> u32 {
        0
    }
    pub fn cells_recovered(&self) -> u32 {
        0
    }
    pub fn cells_cycled(&self) -> u32 {
        0
    }
    pub fn max_cycles(&self) -> u32 {
        0
    }
    pub fn depleted_now(&self) -> u32 {
        0
    }
}

pub fn diet_bin(_diet: f64) -> usize {
    0
}
