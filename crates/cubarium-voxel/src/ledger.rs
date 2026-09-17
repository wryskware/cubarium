use serde::{Deserialize, Serialize};

/// Cumulative external water fluxes since the world began, in cubic metres. Every
/// change in total stored water is explained by these; internal transfers (falling,
/// spreading, infiltration, drainage, springs) never appear here.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Ledger {
    pub rain_in: f64,
    pub user_in: f64,
    pub evaporation_out: f64,
    pub outlet_out: f64,
    /// Water removed because a terrain edit left it nowhere to go. Should stay zero;
    /// reported rather than hidden.
    pub displaced_out: f64,
    /// Total stored water at creation, so `residual` can be computed from a snapshot.
    pub initial_stored: f64,
}

impl Ledger {
    pub fn net_in(&self) -> f64 {
        self.rain_in + self.user_in - self.evaporation_out - self.outlet_out - self.displaced_out
    }

    /// What the stores should hold now given the fluxes alone. `stored - expected` is
    /// the raw conservation residual: `free` and `pore` are `f64`, so nothing corrects
    /// it and any term in it is a real leak.
    pub fn expected_stored(&self) -> f64 {
        self.initial_stored + self.net_in()
    }
}
