//! **The fitted rates**: what emerges from behaviour in space and has no knob in the sim,
//! each recorded with the runs it came from, its spread and the form fitted. These are model
//! parameters, never sim knobs. `examples/fauna_model.rs fit` recomputes every one from the
//! logs and prints it beside the value recorded here.

use cubarium_voxel_fauna::{Food, Founder};

/// **A functional response** to one food class: the share of the mouth's full bite a feeding
/// body actually gets, against the world stock `E` of that food and the `n` bodies of its own
/// lineage, `ψ = psi_max · E / (E + half + interference · (n − 1))` — Holling type II in the
/// stock (Brose et al. 2006, Hill exponent 1) with Beddington–DeAngelis interference: a
/// stream's film or a litter bed shared by more mouths gives each less. It folds in
/// everything spatial: how often the mouth is at food, and the sim's per-site
/// `E_site / (E_site + bite_half_stock)`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Response {
    pub psi_max: f64,
    pub half: f64,
    pub interference: f64,
}

impl Response {
    /// A food the lineage always finds when the stock is there (an unfitted default).
    pub const ALWAYS: Response = Response { psi_max: 1.0, half: 0.0, interference: 0.0 };

    /// No interference.
    pub const fn holling(psi_max: f64, half: f64) -> Response {
        Response { psi_max, half, interference: 0.0 }
    }

    pub fn psi(&self, stock: f64, n: f64) -> f64 {
        if !(stock > 0.0) {
            return 0.0;
        }
        let crowd = self.interference.max(0.0) * (n - 1.0).max(0.0);
        (self.psi_max * stock / (stock + self.half.max(0.0) + crowd)).clamp(0.0, 1.0)
    }
}

/// One lineage's fitted rates.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LineageFit {
    /// Per [`Food`] class, in [`Food::ALL`] order; `None` is "not fitted" (read as
    /// [`Response::ALWAYS`] and flagged in reports).
    pub response: [Option<Response>; Food::COUNT],
    /// A dark-active lineage's share of **daytime** spent awake on a dark face (a cave, a
    /// hall, under an overhang), outside the emergency latch, which the model runs itself.
    pub day_dark_awake: f64,
    /// Mean motor effort while awake, `equivalent displacement / cruise` (the sim's motor
    /// ratio), so motor respiration is `motor_respiration_per_s · body · effort` awake.
    pub motor_effort: f64,
    /// **Nest crowding**: the mean number of the lineage's own bodies within its nest radius
    /// of a layer, per other living body of the lineage (the D1b readout `crowd` over
    /// `n − 1`). A layer's count is Poisson with mean `crowd_per_body · (n − 1 + eggs)`; a
    /// count above `nest_crowd_max` refuses the clutch.
    pub crowd_per_body: f64,
    /// **Nest-site aggregation**: how much denser a layer's own surroundings are than the
    /// average adult's (`crowd_per_body` is the average): the Poisson mean a laying body sees is
    /// `nest_aggregation · crowd_per_body · (n − 1 + eggs)`. Layers lay where earlier clutches
    /// hatched and the hatchlings stay.
    pub nest_aggregation: f64,
    /// The share of laid eggs lost before hatching (flooded or the face gone).
    pub egg_loss: f64,
    /// The share of laying attempts that find an egg site (the capgnawer's fungus within
    /// `egg_fungus_radius_m`); 1 for litter and film layers on patch.
    pub egg_site: f64,
    /// **As prey of the lanternjaw**: kills per hunting lanternjaw-second per prey body of
    /// this lineage at profitability 1 (mass action, Holling type I; the hunter's own gut
    /// latch makes it type II in the aggregate). 0: not taken.
    pub kill_rate: f64,
}

impl LineageFit {
    pub const UNFITTED: LineageFit = LineageFit {
        response: [None; Food::COUNT],
        day_dark_awake: 0.0,
        motor_effort: 0.5,
        crowd_per_body: 0.0,
        nest_aggregation: 1.0,
        egg_loss: 0.0,
        egg_site: 1.0,
        kill_rate: 0.0,
    };

    /// ψ for `food` at stock `stock` among `n` of its own lineage, the unfitted default being
    /// [`Response::ALWAYS`].
    pub fn psi(&self, food: Food, stock: f64, n: f64) -> f64 {
        self.response[food.index()].unwrap_or(Response::ALWAYS).psi(stock, n)
    }
}

/// Every lineage's fitted rates and the hunters' own.
#[derive(Clone, Debug, PartialEq)]
pub struct Fits {
    pub lineage: [LineageFit; Founder::COUNT],
    /// Lanternjaw strikes per kill (the motor and strike cost of a kill).
    pub strikes_per_kill: f64,
    /// A dormant egg's chance per dusk that its footfall threshold is met, no adult is near
    /// and a host stands within reach (all spatial), before the sim's own
    /// `hatch_chance_per_dusk`.
    pub bank_trigger_per_dusk: f64,
    /// **How long a hunter stays at its kill**, seconds (the mean of an exponential): its
    /// fresh kill is at its mouth, and it alone eats from it, until it moves on; what is left
    /// then goes to the carrion pool. Infinite: it never leaves (every kill is eaten or
    /// decays).
    pub carcass_stay_s: f64,
    /// **A kill's reach**: the share of a corpse's organic matter the hunter's mouth reaches
    /// and it alone eats; the rest goes to the carrion pool at the kill (left, scattered,
    /// scavenged). 1: the whole corpse is its own.
    pub kill_reach: f64,
    /// Whether each rate above came from telemetry (`true`) or is a default.
    pub fitted: [bool; Founder::COUNT],
}

impl Fits {
    /// Every lineage unfitted: ψ = 1 wherever food stands, no crowding, no predation.
    pub fn unfitted() -> Fits {
        Fits {
            lineage: [LineageFit::UNFITTED; Founder::COUNT],
            strikes_per_kill: 1.0,
            bank_trigger_per_dusk: 0.0,
            carcass_stay_s: f64::INFINITY,
            kill_reach: 1.0,
            fitted: [false; Founder::COUNT],
        }
    }

    /// **The recorded fits** (`examples/fauna_model.rs fit gen=<S>/gen lj=<S>/lj2`, 2026-10-05),
    /// each from the runs named, with its spread:
    ///
    /// - **Responses**, by simulation (pattern-oriented, Grimm et al. 2005): the per-window
    ///   assimilation of each detritivore over every D1b arm (main, gut, brood, branch, rate)
    ///   and seed (4 5 7 8 9 12 13 15), minutes 20-240, 920 windows, matched in log space
    ///   by the model run alone on each seed's clamped stocks.
    ///   - littershredder, litter: ψ = 0.43 flat over litter 33-1350 (model/sim window intake
    ///     median 0.97, IQR 0.89-1.12). The litter on patch never limits: the gut does.
    ///   - capgnawer: caps ψ = 0.26 · C / (C + 0.93 · (n − 1)) (Beddington–DeAngelis: the
    ///     caps are shared), litter 0 (median 0.78, IQR 0.57-1.09). The cap-litter split is not
    ///     identifiable from the logs (they carry total intake only); the grid put the litter
    ///     share at 0.
    ///   - ripple-snail, film: ψ = 0.50 · F / (F + 3.4) (median 1.00, IQR 0.73-1.39).
    /// - **Nest crowding** `crowd / (n − 1)`, D1b windows with adults, median [IQR]:
    ///   shredder 0.182 [0.119-0.273], capgnawer 0.143 [0-0.429], snail 0.469 [0.429-0.515].
    /// - **The observation refit** (`fit obs-only`, from [`Fits::recorded_4h`]; D1b branch 8 h
    ///   observation, training seeds 4 5 7 8, count + adults + juveniles + hatched at 360-720,
    ///   log error): interference on a grid from 0 to 3 × the 4 h median stock per other body,
    ///   ψ_max set so ψ at the 4 h median stock and count stays where the 4 h fit put it (ψ_max
    ///   not capped at 1: ψ itself is), and nest aggregation 1-4. A response whose interference
    ///   the 4 h intake fit found (the capgnawer's) keeps it.
    ///   - Shredder: ψ_max 0.477, interference 2.805, aggregation 2 (error 5.57; no
    ///     interference 6.26, the grid's 28 per body 5.94: weakly identified).
    ///   - Snail: ψ_max 0.582, half 3.401, interference 0.102, aggregation 2 (error 20.27;
    ///     flat in interference, 20.3-21.7 up to 2 per body). The earlier 1.02 at ψ_max 1.0 sat
    ///     on the grid's top and on the ψ_max cap, which lowered ψ: an artefact.
    ///   - Capgnawer: aggregation 1 (flat).
    ///   - **Held out** (seeds 9 12 13 15, `d1b`): 83 of 99 cells pass; the shredder's adults
    ///     and matured from m600, its count at m720, and the snail's matured fail (too many
    ///     mature: the within-cohort spread the model lacks).
    /// - **Day on a dark face**: 0 for all three on patch (dark-awake share of daytime median
    ///   0, IQR to 0.01-0.07): their daytime waking is the emergency latch, which the model
    ///   runs as a law.
    /// - **Motor effort**: 0 (median of spend less upkeep, IQR −0.37 to 0.005): the heuristic
    ///   detritivores stand and feed.
    /// - **Egg loss**: 0 of 1,124 eggs laid.
    /// - **The frondgrazer's foliage response** (`fitbrowser`): ψ = 0.276 · F / (F + 41.5), by
    ///   simulation on the D1b branch runs (no hunters), its count at every hour to 720 on the
    ///   seeds' clamped edible foliage (median 415), training seeds 4 5 7 8 (log error 1.65);
    ///   the held-out seeds pass at 240/480/720 (sim 18/22/52, model 15/27/59).
    /// - **The hunter** (`fit_hunter` in the example, refitted at each D2b run, deterministic):
    ///   the dormant eggs' loss 0.333 (24 of 72 eggs gone from the bank in the first ten minutes
    ///   after laying, d2, 8 seeds); carrion only at its own kills; a kill's reach 0.358 (the
    ///   hunters' assimilation over d2 m120-240 matched, 5.47 summed over 8 seeds); then the
    ///   kill rates per prey body per hunting LJ-second: shredder 8.7e-5, capgnawer 7.6e-5,
    ///   snail 3.9e-5, frondgrazer 4.4e-5 (kills matched, 125 vs 128); 2.38 strikes per kill.
    ///   The stay at a kill is not identifiable (the mean field's kill flux keeps the carcass
    ///   topped up while the hunter hunts) and stays infinite.
    pub fn recorded() -> Fits {
        let mut f = Fits::recorded_4h();
        {
            let s = f.of_mut(Founder::Blind);
            s.response[Food::Litter.index()] = Some(Response { psi_max: 0.477, half: 0.0, interference: 2.805 });
            s.nest_aggregation = 2.0;
        }
        {
            let r = f.of_mut(Founder::RippleSnail);
            r.response[Food::Film.index()] = Some(Response { psi_max: 0.582, half: 3.401, interference: 0.102 });
            r.nest_aggregation = 2.0;
        }
        {
            let g = f.of_mut(Founder::Browser);
            g.response[Food::Foliage.index()] = Some(Response::holling(0.276, 41.5));
            f.fitted[Founder::Browser.index()] = true;
        }
        f
    }

    /// **The 4 h fits alone**, before the observation refit: the responses by simulation
    /// (no interference for the shredder and the snail), the behaviour rates, aggregation 1.
    /// The observation refit (`fit obs-only`) starts here.
    pub fn recorded_4h() -> Fits {
        let mut f = Fits::unfitted();
        {
            let s = f.of_mut(Founder::Blind);
            s.response[Food::Litter.index()] = Some(Response::holling(0.43, 0.0));
            s.response[Food::Carrion.index()] = Some(Response::holling(0.0, 0.0));
            s.crowd_per_body = 0.182;
            s.motor_effort = 0.0;
        }
        {
            let c = f.of_mut(Founder::Capgnawer);
            c.response[Food::Litter.index()] = Some(Response::holling(0.0, 0.0));
            c.response[Food::CapTissue.index()] = Some(Response { psi_max: 0.26, half: 0.0, interference: 0.9323 });
            c.crowd_per_body = 0.143;
            c.motor_effort = 0.0;
        }
        {
            let r = f.of_mut(Founder::RippleSnail);
            r.response[Food::Film.index()] = Some(Response::holling(0.50, 3.401));
            r.crowd_per_body = 0.469;
            r.motor_effort = 0.0;
        }
        for x in [Founder::Blind, Founder::Capgnawer, Founder::RippleSnail] {
            f.fitted[x.index()] = true;
        }
        f
    }

    pub fn of(&self, f: Founder) -> &LineageFit {
        &self.lineage[f.index()]
    }

    pub fn of_mut(&mut self, f: Founder) -> &mut LineageFit {
        &mut self.lineage[f.index()]
    }
}
