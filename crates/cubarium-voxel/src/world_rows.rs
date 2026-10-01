//! The world's parameter rows (`world.*`, package R1): [`crate::Config`], its day clock and
//! its climate. Units, grades and provenance from the audit's `parameters.tsv` where it has
//! a row; `placeholder` otherwise. The terrain recipe (`world.landform`) is an opaque
//! subtree: pure terrain shape, validated by its own recipe.

use crate::registry::{Range, Row};

fn p(path: &str, unit: &'static str, range: Range, what: &'static str) -> Row {
    Row::new(format!("world.{path}"), unit, range, what)
}

pub(crate) fn rows() -> Vec<Row> {
    let nonneg = Range::nonneg();
    let positive = Range::positive();
    // The climate's keys refused only NaN and negatives (and the rain modes' gains only
    // NaN): an infinite hazard is a fixture's "certain", so infinity stays allowed there.
    let nonneg_inf = Range::nonneg_or_inf();
    let positive_inf = Range::nonneg_or_inf().open_lo();
    let any_inf = Range::any_or_inf();
    let mut rows = vec![
        // The extents and seeds: structural, not ecological.
        p("width", "voxels", Range::at_least(1.0), "voxels along the strip (x wraps)").prov("structural"),
        p("height", "voxels", Range::at_least(1.0), "voxels of vertical extent").prov("structural"),
        p("depth", "voxels", Range::at_least(1.0), "voxels into the habitat").prov("structural"),
        p("voxel_m", "m", positive, "edge length of one voxel").grade("N").prov("decision"),
        p("seed", "-", nonneg, "seed for every deterministic stream").prov("structural"),
        p("noise_seed", "-", nonneg, "seed for the landform's final wobble alone (0: from `seed`)")
            .prov("structural"),
        p("water_substeps", "substeps per tick", Range::at_least(1.0), "free-water solver substeps per tick")
            .grade("N")
            .prov("numerics"),
        p("free_transfer_cap", "fraction of a cell per substep", nonneg, "cap on one cell's free-water change per substep (0: none)")
            .grade("N")
            .prov("numerics"),
        // Water.
        p("rain_m_per_s", "m/s", nonneg, "R0: prescribed rain onto exposed tops (open budget) and the unit of a mode's rate_r0")
            .search(1e-5, 1e-4)
            .log()
            .grade("A")
            .prov("placeholder"),
        p("evaporation_m_per_s", "m/s", nonneg, "evaporation from exposed free-water surfaces")
            .search(1e-6, 3e-5)
            .log()
            .grade("D")
            .prov("tuned"),
        p("spring_k_m2_per_s", "m2/s", nonneg, "spring conductance from the aquifer: Q = k * max(head - h_spring, 0)")
            .grade("C"),
        p("aquifer_porosity", "fraction of footprint", positive, "aquifer pore space per metre of head (>= soil's 0.35)")
            .grade("B")
            .prov("derived"),
        p("initial_aquifer_head_m", "m above y=0", nonneg, "water table at creation (0: a dry aquifer)")
            .grade("C")
            .prov("tuned"),
        p("outlet_m3_per_s", "m3/s", nonneg, "free water an open outlet exports").grade("C").prov("tuned"),
        p("initial_atmosphere_m3", "m3", nonneg, "water aloft at creation (closed budget)").prov("structural"),
        p("reentry_m3_per_s", "m3/s", nonneg, "closed budget: the stream returned at the spring cell").grade("C").prov("tuned"),
        p("reentry_floor_head_m", "m above y=0", nonneg, "water table at which the aquifer-fed stream stops").prov("structural"),
        p("lake_drain_m2_per_s", "m3/s per m of head", nonneg, "lake floor joined to the aquifer: flow = k * (lake - table)")
            .grade("C")
            .prov("derived"),
        // The day clock.
        p("day.length_min", "min of sim time", positive, "one day and night (cycle); the 'day' the climate counts in")
            .grade("C")
            .prov("decision"),
        p("day.daylight", "fraction of the cycle", Range::closed(0.05, 0.95), "the share of the cycle the sun is up")
            .search(0.4, 0.7)
            .grade("B")
            .prov("decision"),
        p("day.start_hour", "h on the day's 24 h clock", Range::closed(0.0, 24.0).open_hi(), "where a fresh world starts")
            .prov("decision"),
        p("day.latitude", "deg north", Range::closed(-90.0, 90.0), "local clock: the place whose sunrise the day follows")
            .optional()
            .prov("decision"),
        p("day.longitude", "deg east", Range::closed(-180.0, 180.0), "local clock: the place's longitude")
            .optional()
            .prov("decision"),
        p("day.plant_night_factor", "fraction of the daytime rate", Range::fraction(), "plant maintenance and senescence in the dark (1: no night rule)")
            .evolve()
            .prov("decision"),
        // The climate.
        p("climate.weather_speed", "x nature", positive, "how much faster than nature the weather runs, when fixed (\"auto\" otherwise)")
            .optional()
            .search(1.0, 12.0)
            .log()
            .grade("C")
            .prov("decision"),
        p("climate.temperature_c", "degC", nonneg_inf, "mean air temperature").grade("A").prov("preset"),
        p("climate.diurnal_c", "degC", nonneg_inf, "diurnal swing, +/-").search(2.0, 5.0).grade("A").prov("preset"),
        p("climate.drift_sd_c", "degC", nonneg_inf, "day-to-day drift's standard deviation").grade("B").prov("preset"),
        p("climate.drift_days", "days of nature", nonneg_inf, "the drift's time scale").grade("B").prov("preset"),
        p("climate.fronts_per_day", "per day", nonneg_inf, "cold fronts per day").grade("B").prov("preset"),
        p("climate.front_min_c", "degC", nonneg_inf, "shallowest front").grade("B").prov("preset"),
        p("climate.front_max_c", "degC", nonneg_inf, "deepest front").grade("B").prov("preset"),
        p("climate.front_onset_min", "min of nature", nonneg_inf, "a front's cooling ramp").grade("B").prov("preset"),
        p("climate.front_recovery_days", "days of nature", nonneg_inf, "a front's recovery").grade("B").prov("preset"),
        p("climate.front_window_min", "min of nature", nonneg_inf, "F = 1 this long after a front arrives").grade("B").prov("preset"),
        p("climate.rain_cool_min", "min of nature", nonneg_inf, "rain cooling's time constant").grade("B").prov("preset"),
        p("climate.capacity_share", "fraction of the world's water", positive_inf, "what the air holds at the mean temperature")
            .search(0.01, 0.1)
            .log()
            .grade("A")
            .prov("preset"),
        p("climate.afternoon_width", "fraction of the cycle", nonneg_inf, "the afternoon convection bump's width").grade("A").prov("preset"),
        p("climate.cooling_cap_c_per_h", "degC per sim hour", nonneg_inf, "cap on the cooling rate K").grade("N").prov("numerics"),
        p("climate.dry_gap_min_s", "s", nonneg_inf, "shortest wait after an event before a natural start (0/0: off)")
            .search(0.0, 1800.0)
            .grade("C")
            .prov("decision"),
        p("climate.dry_gap_max_s", "s", nonneg_inf, "longest wait after an event before a natural start")
            .search(0.0, 1800.0)
            .grade("C")
            .prov("decision"),
        p("climate.fog_form_rh", "RH", nonneg_inf, "fog forms at or above (visual only)").prov("visual"),
        p("climate.fog_clear_rh", "RH", nonneg_inf, "fog clears below (visual only)").prov("visual"),
        p("climate.fog_span_rh", "RH", nonneg_inf, "fog density span (visual only)").prov("visual"),
        p("climate.fog_dawn_before", "fraction of the cycle", nonneg_inf, "dawn window before sunrise (visual only)").prov("visual"),
        p("climate.fog_dawn_after", "fraction of the cycle", nonneg_inf, "dawn window after sunrise (visual only)").prov("visual"),
        p("climate.fog_burn_after", "fraction of the cycle", nonneg_inf, "fog burns off this far past sunrise (visual only)").prov("visual"),
        p("climate.fog_cooling_c_per_h", "degC per sim hour", nonneg_inf, "fog forms while the air cools faster (visual only)").prov("visual"),
        p("climate.fog_form_min", "min of nature", nonneg_inf, "fog's forming time (visual only)").prov("visual"),
        p("climate.fog_clear_min", "min of nature", nonneg_inf, "fog's clearing time (visual only)").prov("visual"),
        p("climate.strike_mean_s", "s", nonneg_inf, "mean lightning interval in a downpour (visual only)").prov("visual"),
        p("climate.strike_min_s", "s", nonneg_inf, "shortest lightning interval (visual only)").prov("visual"),
        p("climate.strike_max_s", "s", nonneg_inf, "longest lightning interval (visual only)").prov("visual"),
        p("climate.storm_stream_gain", "x stream flow", nonneg_inf, "stream swell at a downpour's peak (0: off)").grade("C").prov("decision"),
        p("climate.opening_min_s", "s", nonneg_inf, "the host's opening rain falls at least this long").grade("N").prov("preset"),
    ];
    for mode in ["drizzle", "shower", "downpour"] {
        let m = |field: &str, unit: &'static str, range: Range, what: &'static str| {
            p(&format!("climate.{mode}.{field}"), unit, range, what).prov("tuned")
        };
        rows.extend([
            m("lambda_max_per_h", "events per sim hour", nonneg_inf, "ceiling of the event hazard").search(0.5, 5.0).log().grade("C"),
            m("rh50", "RH or moisture M", any_inf, "humidity at which the hazard's logistic is half").search(0.7, 1.3).grade("C"),
            m("width", "RH or moisture M", positive_inf, "the logistic's width").grade("N"),
            m("g_aft", "x afternoon bump", any_inf, "convective boost (1 + g_aft * A)").search(0.5, 30.0).log().grade("C"),
            m("g_cool", "x cooling rate K", any_inf, "hazard boost from cooling air").search(0.0, 2.0).grade("C"),
            m("g_front", "x fresh-front flag", any_inf, "hazard boost in a fresh front").search(0.0, 5.0).grade("C"),
            m("rate_r0", "x R0", nonneg_inf, "the rate it rains at").search(0.05, 4.0).log().grade("A"),
            m("stop_rh", "RH or moisture M", nonneg_inf, "the event ends when its reading falls to this").search(0.3, 0.9).grade("C"),
            m("longest_min", "min of nature", positive_inf, "or when it has rained this long").search(10.0, 120.0).log().grade("B"),
            m("cool_c", "degC", any_inf, "while it rains the air relaxes toward this much cooler").grade("B"),
        ]);
    }
    rows
}
