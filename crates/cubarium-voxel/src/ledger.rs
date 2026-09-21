use serde::{Deserialize, Serialize};

/// Cumulative external water fluxes since the world began, in cubic metres. Every
/// change in total stored water is explained by these; internal transfers (falling,
/// spreading, infiltration, drainage, springs) never appear here.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Ledger {
    pub rain_in: f64,
    /// Water the user's commands put in, **signed**: an `AddWater` or a positive
    /// `ChargeAquifer` adds, a `ChargeAquifer` withdrawal subtracts. Refused amounts are
    /// never booked here.
    pub user_in: f64,
    /// Water lifted off open free-water surfaces. In the **open** budget it leaves the
    /// world; in the closed one it is the flow *into* [`crate::World::atmosphere_m3`]
    /// and is counted again in `atmosphere_in`. Either way it left the in-world stores,
    /// so [`Ledger::expected_stored`] subtracts it in both.
    pub evaporation_out: f64,
    /// Free water the named outlet exported. Out of the world in the open budget; into
    /// the atmosphere store in the closed one (the return loop: the river that leaves
    /// the strip comes back as weather, not as a second reservoir).
    pub outlet_out: f64,
    /// Pore water taken out of the soil by plants, cubic metres. The one loss term for
    /// [`crate::Command::WithdrawPore`]. Under the open budget transpired water leaves
    /// the world; under the closed one it is the flow into the atmosphere store, like
    /// evaporation.
    pub transpiration_out: f64,
    /// Water removed because a terrain edit left it nowhere to go. Should stay zero;
    /// reported rather than hidden.
    pub displaced_out: f64,
    /// Total stored water at creation, so `residual` can be computed from a snapshot.
    pub initial_stored: f64,
    /// Cumulative water deposited **into** the lumped atmosphere store: evaporation,
    /// transpiration and the outlet's export under a closed budget, plus whatever the
    /// user's lever added. Stays zero under the open budget, where those flows simply
    /// leave the world.
    pub atmosphere_in: f64,
    /// Cumulative water the showers drew back **out** of the atmosphere store and the
    /// sky cells accepted. Rain refused by a full world is refunded here, so this is
    /// what actually fell.
    pub atmosphere_out: f64,
    /// The share of `atmosphere_in` the user's lever put there — `user_in`'s counterpart
    /// for the store. Reporting only; the conservation arithmetic uses `atmosphere_in`.
    pub user_atmosphere_in: f64,
    /// Atmosphere store at creation, so the atmosphere residual can be computed from a
    /// snapshot exactly as `initial_stored` does it for the in-world stores.
    pub initial_atmosphere: f64,
    /// Showers **started** since the world began. A count, not a flux.
    pub showers: u64,
}

impl Ledger {
    pub fn net_in(&self) -> f64 {
        self.rain_in + self.user_in
            - self.evaporation_out
            - self.outlet_out
            - self.transpiration_out
            - self.displaced_out
    }

    /// What the stores should hold now given the fluxes alone. `stored - expected` is
    /// the raw conservation residual: `free` and `pore` are `f64`, so nothing corrects
    /// it and any term in it is a real leak.
    ///
    /// The **in-world** stores only — free, pore and the aquifer — under either budget.
    /// A closed budget does not change this identity: evaporation, transpiration and the
    /// outlet really did leave free and pore water behind, whether they left the world or
    /// only moved to the atmosphere store.
    pub fn expected_stored(&self) -> f64 {
        self.initial_stored + self.net_in()
    }

    /// What the lumped atmosphere store should hold. Zero for an open-budget world,
    /// which never deposits into it.
    pub fn expected_atmosphere(&self) -> f64 {
        self.initial_atmosphere + self.atmosphere_in - self.atmosphere_out
    }

    /// Every store the world owns, as the fluxes explain it. Under a closed budget every
    /// internal term cancels and this collapses to the water the world started with plus
    /// the user's additions less `displaced_out`: nothing else leaves.
    pub fn expected_total(&self) -> f64 {
        self.expected_stored() + self.expected_atmosphere()
    }
}
