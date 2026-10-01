//! The terrain generators' parameter rows (`world.landform.*`, package R2-world): the
//! staged [`crate::Recipe`] (`world.landform.staged.*`) and the designed terrarium
//! (`world.landform.terrarium.*`), which share an erosion budget and a water recipe
//! (`world.landform.*.erosion.*`, `world.landform.*.water.*`).
//!
//! The default landform is `ridge`, a unit variant with no numbers of its own, so every
//! row here is [`Row::optional`]: it matches a field only in a config that holds that
//! variant. Units, search intervals and provenance from the audit's `parameters.tsv`
//! where it has a row (`water.recipe.*`, `terrarium.*`); `placeholder` otherwise.
//! A search interval is `[lo, hi]` over the field's plausible values, not a validity
//! bound: the range is the validity.

use crate::registry::{Range, Row};

/// One row under `prefix`: a path, unit, range, search interval, and what it is.
struct T {
    prefix: &'static str,
    rows: Vec<Row>,
}

impl T {
    fn new(prefix: &'static str) -> T {
        T { prefix, rows: Vec::new() }
    }

    /// A numeric field searched over `[lo, hi]`.
    fn f(
        &mut self,
        name: &str,
        unit: &'static str,
        range: Range,
        (lo, hi): (f64, f64),
        what: &'static str,
    ) -> &mut T {
        self.rows.push(
            Row::new(format!("{}.{name}", self.prefix), unit, range, what)
                .search(lo, hi)
                .optional(),
        );
        self
    }

    /// The same, searched on a log scale.
    fn log(
        &mut self,
        name: &str,
        unit: &'static str,
        range: Range,
        (lo, hi): (f64, f64),
        what: &'static str,
    ) -> &mut T {
        self.f(name, unit, range, (lo, hi), what);
        let row = self.rows.pop().expect("a row was just pushed").log();
        self.rows.push(row);
        self
    }

    /// A row that is not searched: a seed stream.
    fn fixed(&mut self, name: &str, unit: &'static str, range: Range, what: &'static str) -> &mut T {
        self.rows.push(
            Row::new(format!("{}.{name}", self.prefix), unit, range, what)
                .prov("structural")
                .optional(),
        );
        self
    }
}

/// The erosion budget, under `prefix` (`...erosion`).
fn erosion(prefix: &'static str) -> Vec<Row> {
    let mut t = T::new(prefix);
    let (n, p) = (Range::nonneg(), Range::positive());
    t.f("iterations", "solver iterations", Range::at_least(0.0), (0.0, 300.0), "how many iterations the erosion solver runs (0: the identity)")
        .f("rain", "model rain per column per iteration", n, (0.1, 4.0), "model rain per sample column per iteration (with `capacity`, the scale of the cut)")
        .log("capacity", "sediment per unit discharge per unit slope", n, (5e-4, 1e-2), "sediment one unit of discharge carries per unit of slope")
        .f("erode", "fraction of the carrying deficit", n, (0.1, 1.0), "fraction of the carrying deficit taken off the bed each iteration")
        .f("deposit", "fraction of the excess load", n, (0.1, 1.0), "fraction of the excess load dropped each iteration")
        .f("bedrock_resistance", "x loose sediment", n, (1.0, 20.0), "how much slower hardness-1 bedrock yields than loose sediment")
        .log("max_cut_soft_m", "m per iteration", n, (0.01, 0.5), "deepest one iteration may cut at a soft bed")
        .log("max_cut_hard_m", "m per iteration", n, (0.002, 0.1), "deepest one iteration may cut at a hard bed")
        .f("repose", "tangent of the angle", n, (0.4, 1.4), "tangent of the angle of repose for loose sediment")
        .f("repose_sweeps", "sweeps per iteration", Range::at_least(0.0), (0.0, 6.0), "relaxation sweeps of the repose rule per iteration");
    let _ = p;
    t.rows
}

/// The recipe's water, under `prefix` (`...water`).
fn water(prefix: &'static str) -> Vec<Row> {
    let mut t = T::new(prefix);
    let (n, f) = (Range::nonneg(), Range::fraction());
    t.log("inventory_m", "m over the footprint", n, (0.5, 3.0), "total water the world starts with (0: a dry world)")
        .f("atmosphere_fraction", "fraction of the inventory", f, (0.0, 0.2), "share of the inventory held aloft for the cycle")
        .f("aquifer_head_m", "m above y=0", n, (0.25, 2.0), "water table at creation, charged out of the inventory")
        .log("rain_m_per_s", "m/s", n, (1e-5, 1e-4), "R0: the rate a shower falls at; the climate's modes rain at multiples of it")
        .log("evaporation_m_per_s", "m/s", n, (1e-6, 3e-5), "evaporation from open water at the mean temperature and RH 0.6")
        .f("lake_depth_m", "m above the lake floor", n, (0.25, 1.0), "how deep the lake stands before it overflows into the outlet")
        .f("min_lake_m2", "m2", n, (0.0, 12.0), "smallest open-water surface a generated world is accepted with (0: any)")
        .log("reentry_m3_per_s", "m3/s", n, (1e-4, 1e-2), "the stream returned at the spring cell")
        .f("min_tier_pools", "pools", Range::at_least(0.0), (0.0, 3.0), "open-sky pools above the lake a generated world must hold water in")
        .log("lake_drain_m2_per_s", "m3/s per m of head", n, (1e-2, 1.0), "lake floor joined to the aquifer: flow = k * (lake - table)")
        .f("lake_table_share", "fraction of post-lake inventory", f, (0.5, 1.0), "most of the inventory left after the lake that may go into the water table")
        .f("ring_stream_floor_m", "m", n, (0.0, 0.5), "how far under its founding table the aquifer-fed stream may draw it")
        .f("lake_floor_band", "voxels", Range::at_least(0.0), (0.0, 4.0), "how far above the lowest standing water a body is still the lake")
        .f("pool_min_fill", "fraction of a cell", f, (0.01, 0.2), "how full a void cell must be to count as pool water rather than runoff")
        .f("min_pool_cells", "visible surface cells", Range::at_least(0.0), (1.0, 12.0), "visible surface cells a pool needs before the seed gate counts it");
    let mut rows = t.rows;
    // The tsv's sources.
    for r in &mut rows {
        let tail = r.path.rsplit('.').next().unwrap_or_default().to_string();
        match tail.as_str() {
            "inventory_m" => {
                r.provenance = "placeholder/tuned (sized to lift the table to the lake floor)";
                r.unit = "m over footprint";
            }
            "aquifer_head_m" => r.provenance = "tuned",
            "rain_m_per_s" => {
                r.provenance = "placeholder ('as high as the rule allows')";
                r.grade = "A";
            }
            "evaporation_m_per_s" => r.provenance = "tuned (kept under R0)",
            "lake_depth_m" => r.provenance = "tuned (hollow-count vs readability)",
            "reentry_m3_per_s" => r.provenance = "tuned:T2 sweep",
            "lake_drain_m2_per_s" => r.provenance = "derived:100x reentry",
            _ => {}
        }
    }
    rows
}

/// The ridge generator's rows (`world.ridge.*`): always present, whatever the landform.
fn ridge() -> Vec<Row> {
    let (n, p, f) = (Range::nonneg(), Range::positive(), Range::fraction());
    let mut t = T::new("world.ridge");
    t.f("front_fraction", "fraction of the height", f, (0.15, 0.45), "surface height at the front")
        .f("ridge_amp_fraction", "fraction of the height", f, (0.08, 0.3), "the ridge's amplitude")
        .f("ridge_amp_min", "voxels", n, (0.5, 3.0), "the ridge's least amplitude")
        .f("second_amp_fraction", "fraction of the height", f, (0.0, 0.12), "the second harmonic's amplitude")
        .f("climb_fraction", "fraction of the height", f, (0.1, 0.4), "how far the back stands above the front")
        .f("basin_depth", "ridge amplitudes", n, (0.3, 1.2), "the receiving basin's floor under the front, in ridge amplitudes")
        .f("basin_keep", "fraction of the depth", f, (0.0, 0.5), "relief below the basin floor keeps this share of its depth")
        .f("wobble_terms", "sine terms", Range::at_least(0.0), (0.0, 8.0), "terms of the weak correlated wobble")
        .f("wobble_first_harmonic", "harmonic", Range::at_least(0.0), (1.0, 6.0), "the wobble's first harmonic around the ring")
        .f("wobble_harmonic_step", "harmonics", Range::at_least(0.0), (1.0, 4.0), "the step between the wobble's harmonics")
        .f("wobble_amp", "voxels", n, (0.0, 1.5), "the first wobble term's amplitude")
        .f("wobble_amp_falloff", "per term", n, (0.0, 2.0), "how fast the wobble's amplitude falls per term")
        .f("wobble_depth_freq.*", "cycles per voxel of depth", n, (0.02, 0.4), "range of the wobble's frequencies in depth")
        .f("soil_slope", "voxels", n, (0.0, 6.0), "soil depth on flat ground from slope")
        .f("soil_slope_cap", "voxels per voxel", p, (0.5, 2.0), "the slope at which the slope's soil vanishes")
        .f("soil_deposit", "voxels", n, (0.0, 6.0), "soil depth added by deposition in the basin")
        .f("soil_base", "voxels", n, (0.0, 2.0), "soil depth everywhere")
        .f("soil_max_rows", "voxels", n, (2.0, 12.0), "the deepest soil")
        .f("rock_thick", "voxels", n, (2.0, 12.0), "thickness of the rock body under the soil")
        .f("rock_thick_swing", "voxels", n, (0.0, 6.0), "the rock body's swing around the ring")
        .f("strata_warp", "voxels", n, (0.0, 4.0), "how far the hard strata warp around the ring")
        .f("strata_thickness", "voxels", p, (1.0, 4.0), "thickness of one rock layer")
        .f("strata_period", "layers", Range::at_least(1.0), (2.0, 8.0), "how many layers apart the hard ones are")
        .f("pockets", "pockets", Range::at_least(0.0), (0.0, 8.0), "soil pockets inside the rock")
        .f("pocket_cy_min", "voxels", n, (1.0, 4.0), "the lowest a pocket's centre may be")
        .f("pocket_height_fraction", "fraction of the height", f, (0.2, 0.8), "the highest a pocket's centre may be, as a share of the height")
        .f("pocket_cy_floor", "voxels", n, (2.0, 6.0), "the least that highest bound may be")
        .f("pocket_rx.*", "voxels", p, (1.0, 8.0), "a pocket's radius along the ring")
        .f("pocket_ry.*", "voxels", p, (1.0, 6.0), "a pocket's radius in height")
        .f("pocket_rz.*", "voxels", p, (1.0, 6.0), "a pocket's radius in depth");
    // The ridge rows are not optional: the default config holds them.
    for r in &mut t.rows {
        r.optional = false;
    }
    t.rows
}

/// Every landform row.
pub(crate) fn rows() -> Vec<Row> {
    let (n, p, f) = (Range::nonneg(), Range::positive(), Range::fraction());
    let mut rows = Vec::new();
    rows.extend(ridge());
    rows.extend(erosion("world.landform.*.erosion"));
    rows.extend(water("world.landform.*.water"));

    // ---- the staged recipe -------------------------------------------------------
    let mut t = T::new("world.landform.staged");
    t.f("base_m", "m above y=0", n, (1.0, 6.0), "elevation the front edge is built around")
        .f("back_rise_m", "m", n, (0.5, 5.0), "how much higher the back wall stands than the front")
        .f("basin_floor_m", "m above y=0", n, (0.5, 3.0), "relief below this is compressed rather than cut")
        .f("depth_scale", "x", p, (0.25, 1.0), "depth anisotropy: features are stretched into z by this factor")
        .f("relief_wavelength_m", "m", p, (8.0, 32.0), "wavelength of the coarsest relief octave: the landform size")
        .f("relief_m", "m", p, (1.0, 5.0), "amplitude of the whole relief ladder")
        .f("relief_octaves", "octaves", Range::at_least(0.0), (3.0, 8.0), "octaves the relief ladder asks for")
        .f("persistence", "ratio", p, (0.3, 0.7), "amplitude ratio between successive octaves")
        .f("lacunarity", "ratio", Range::at_least(1.0).open_lo(), (1.5, 3.0), "frequency ratio between successive octaves (above 1)")
        .f("ridge_wavelength_m", "m", p, (4.0, 16.0), "wavelength of the coarsest ridged octave")
        .f("ridge_relief_m", "m", n, (0.5, 3.0), "amplitude of the ridged noise where the rocky mask is full")
        .f("ridge_octaves", "octaves", Range::at_least(0.0), (1.0, 5.0), "octaves the ridged ladder asks for")
        .f("rocky_wavelength_m", "m", p, (10.0, 40.0), "wavelength of the field that decides which stretches are rocky")
        .f("rocky_fraction", "fraction of the ring", n, (0.2, 0.7), "roughly what share of the ring the ridged noise reaches")
        .f("warp_wavelength_m", "m", p, (8.0, 32.0), "wavelength of the domain warp")
        .f("warp_m", "m", n, (0.0, 3.0), "how far the domain warp displaces the ridged noise")
        .f("min_feature_voxels", "voxels", p, (1.5, 4.0), "the finest octave kept, in voxels of wavelength")
        .f("mantle_m", "m", p, (0.1, 0.75), "loose weathered sediment the landscape starts mantled in")
        .f("soil_max_m", "m", n, (0.5, 3.0), "deepest sediment a column may carry into the voxels")
        .f("strata_m", "m", p, (1.0, 4.0), "one soft-to-hard cycle of the strata, metres of height")
        .f("strata_warp_m", "m", n, (0.0, 1.0), "how far the strata are warped around the ring")
        .f("hardness_soft", "0..1", n, (0.1, 0.4), "hardness of the softest rock")
        .f("hardness_hard", "0..1", n, (0.7, 1.0), "hardness of the hardest rock")
        .f("hardness_region_m", "m", p, (8.0, 36.0), "wavelength of the slow regional variation in hardness")
        .f("bedrock_hardness", "0..1", n, (0.5, 0.9), "at or above this hardness the rock voxelises as bedrock")
        .f("core_m", "m above y=0", n, (0.5, 3.0), "everything below this height is the impermeable core")
        .f("pockets", "pockets", Range::at_least(0.0), (0.0, 8.0), "soil pockets inside the rock")
        .f("core_blend_m", "m", n, (0.1, 1.5), "how thick the core's upper edge blends into the strata")
        .f("hardness_band_share", "fraction", f, (0.4, 0.95), "how much of the strata's hardness is the banding, the rest the slow regional variation")
        .f("rocky_edge_width", "noise units", p, (0.2, 1.0), "the width of the rocky mask's soft edge")
        .f("bench_ramp_factor", "x", n, (2.0, 8.0), "how the benches' ramp length is read against the rocky mask's wavelength")
        .f("basin_keep", "fraction of the depth", f, (0.0, 0.5), "relief below the basin floor keeps this share of its depth")
        .f("min_climb_share", "fraction of back_rise_m", f, (0.2, 1.0), "the back wall stands at least this share of back_rise_m above the front");
    rows.extend(t.rows);

    let mut t = T::new("world.landform.staged.tiers");
    t.f("count", "elevations", Range::at_least(0.0), (0.0, 6.0), "how many shelf elevations are available (0: the original relief)")
        .f("rise_m", "m", p, (0.7, 3.0), "height of one riser")
        .f("edge_wavelength_m", "m", p, (4.0, 24.0), "typical spacing and edge wavelength of shelf masses, around the ring")
        .f("edge_warp_m", "m", n, (0.0, 1.5), "how far a shelf edge wanders, in metres of depth")
        .f("flat", "fraction", n, (0.0, 1.0), "how much of the recipe's relief survives on shelf floors")
        .f("pool_radius_m", "m", p, (0.25, 2.0), "radius of an elevated shelf pool")
        .f("lake_radius_m", "m", p, (1.5, 5.0), "radius of the lake on the lowest ground")
        .f("pool_depth_m", "m", p, (0.2, 1.5), "how deep a pool's bowl is cut below its own rim")
        .f("notch_width_m", "m", n, (0.1, 1.0), "width of the spillway notch in a pool's front rim")
        .f("front_setback_m", "m", n, (0.0, 0.75), "how far back from a shelf's front edge the bowl starts")
        .f("shore_m", "m", n, (0.0, 5.0), "how far out from the lake its wet shore reaches (0: no shore)")
        .f("shore_slope", "m per m", n, (0.1, 0.6), "the shore's rise, metres per metre out from the water")
        .f("shore_lip_m", "m", n, (0.0, 1.0), "width of the shore's dry lip at the water's edge")
        .f("mass_spacing_share", "x edge_wavelength_m", p, (0.5, 1.5), "a shelf mass's spacing around the ring, in edge wavelengths")
        .f("mass_jitter", "fraction of the spacing", n, (0.0, 0.4), "how far a mass is jittered from its even spacing")
        .f("mass_taper", "fraction of the radius per elevation", n, (0.0, 0.3), "how much narrower each higher elevation of a mass is")
        .f("mass_radius_share", "x edge_wavelength_m", p, (0.2, 0.5), "a mass's half-width at its lowest elevation, in edge wavelengths")
        .f("mass_min_pool_radii", "x pool_radius_m", n, (1.0, 4.0), "the least a mass's half-width may be, in pool radii")
        .f("front_base", "fraction of the depth", f, (0.0, 0.3), "a shelf's front edge at elevation 0, as a share of the depth")
        .f("front_step", "fraction of the depth", f, (0.05, 0.3), "how much further back each elevation's front edge stands")
        .f("front_bow", "fraction of the depth", n, (0.0, 0.3), "how far a shelf's front edge is pushed back at the mass's ends")
        .f("back_base", "fraction of the depth", f, (0.3, 0.8), "a shelf's back edge at elevation 0, as a share of the depth")
        .f("back_step", "fraction of the depth", n, (0.0, 0.3), "how much further back each elevation's back edge stands")
        .f("back_edge_share", "fraction", n, (0.0, 1.0), "how much of the edge noise pulls the back edge in")
        .f("back_bow", "fraction of the depth", n, (0.0, 0.3), "how far a shelf's back edge is pulled in at the mass's ends")
        .f("pool_bowl_rows", "voxels", Range::at_least(0.0), (1.0, 4.0), "how deep an upper pool's bowl is cut, in voxels of water over its bed")
        .f("pool_taper", "fraction of the bowl's length", n, (0.0, 0.7), "how much a pool's bowl tapers away from its notch")
        .f("lake_beach_rows", "voxels", Range::at_least(0.0), (0.0, 4.0), "rings of the lake's beach, each one voxel deeper going in")
        .f("ramp_step_m", "m", p, (0.25, 0.75), "the steepest step a ramp through a riser may have (what a body can walk)")
        .f("shore_soil_rows", "voxels", Range::at_least(0.0), (1.0, 8.0), "rows of soil under the waterline the wet shore lays");
    rows.extend(t.rows);

    let mut t = T::new("world.landform.staged.benches");
    t.f("strength", "fraction", n, (0.0, 1.0), "how far the bedrock surface is pulled onto the band top under it")
        .f("ramp_m", "m", n, (0.2, 4.0), "how much of the rocky mask's edge is given over to ramping the pull in");
    rows.extend(t.rows);

    let mut t = T::new("world.landform.staged.hollows");
    t.f("clearance_m", "m", p, (0.5, 1.25), "void a hollow must have above a floor before anyone can stand under it")
        .f("soft_hardness", "0..1", n, (0.3, 0.6), "at or below this hardness a band is soft enough to notch out or hold a gallery")
        .f("bank_slope", "m per m", n, (0.6, 2.0), "how steep the ground must fall toward a neighbour for a column to count as a bank")
        .f("cap_thickness_m", "m", n, (0.2, 0.8), "thickness of the hard cap over an undercut")
        .f("cap_drop_m", "m", n, (0.4, 1.5), "how far below a hard bank a neighbour must be cut to flag it")
        .f("undercut_depth_m", "m", n, (0.4, 1.5), "how far back into the bank the soft band is notched")
        .f("undercut_density", "fraction of eligible banks", n, (0.0, 1.0), "share of eligible banks that are notched")
        .f("grotto_wavelength_m", "m", p, (3.0, 12.0), "size of the patches the undercuts come in")
        .f("front_bias", "fraction", n, (0.0, 1.0), "how strongly hollows are pulled toward the front cut")
        .f("gallery_density", "fraction of deep soft rock", n, (0.0, 0.6), "share of the soft rock at depth that opens into galleries")
        .f("gallery_wavelength_m", "m", p, (1.5, 6.0), "size of the galleries")
        .f("gallery_flatten", "fraction", n, (0.1, 1.0), "how far the gallery noise is stretched vertically")
        .f("gallery_min_depth_m", "m", n, (0.3, 1.5), "how far under the surface a gallery has to start")
        .f("mouth_reach_m", "m", n, (0.3, 2.0), "how far a mouth may be cut through rock to open a gallery")
        .f("skylight_m", "m", n, (0.3, 2.0), "how close a gallery's ceiling must come to the surface for a skylight")
        .f("chamber_radius_share", "x grotto_wavelength_m", p, (0.15, 0.5), "a chamber's half-width, in grotto wavelengths")
        .f("chamber_depth_factor", "x undercut_depth_m", p, (1.0, 4.0), "a chamber's depth into the bank, in undercut depths")
        .f("chamber_room_factor", "x clearance_m", p, (1.0, 3.0), "a chamber's room height, in clearances")
        .f("chamber_spacing_m", "m of ring per chamber", p, (8.0, 40.0), "the ring length each chamber is asked for")
        .f("chamber_max", "chambers", Range::at_least(0.0), (0.0, 6.0), "most chambers a ring is given")
        .f("chamber_taper", "fraction", p, (0.3, 1.0), "how far a chamber's far wall is rounded in")
        .f("gallery_gate", "noise units", Range::any(), (0.3, 0.9), "the gallery noise's gate at zero density")
        .f("gallery_gate_slope", "noise units per unit density", n, (0.6, 1.8), "how fast the gate falls with gallery density")
        .fixed("stream", "-", n, "this section's own seed stream");
    rows.extend(t.rows);

    let mut t = T::new("world.landform.staged.streams");
    for name in ["relief", "ridge", "warp", "rocky", "hardness", "material"] {
        t.fixed(name, "-", n, "seed stream of one generation pass");
    }
    rows.extend(t.rows);

    // ---- the designed terrarium --------------------------------------------------
    let mut t = T::new("world.landform.terrarium");
    let at_least_0 = Range::at_least(0.0);
    t.f("ground_m", "m above y=0", n, (0.5, 2.5), "ground at the front glass")
        .f("ground_rise_m", "m", n, (0.1, 1.0), "how far the ground climbs from the glass to the back wall")
        .f("ground_relief_m", "m", n, (0.0, 0.25), "gentle relief on the ground, either way")
        .f("ground_fall", "m per m", n, (0.02, 0.15), "how steeply the ground climbs away from the lake around the ring")
        .f("soil_m", "m", n, (0.125, 1.0), "soil on every open floor")
        .f("mantle_m", "m", n, (0.1, 0.75), "loose material laid over the massif before erosion")
        .f("crest_m.*", "m", p, (1.0, 15.0), "the crest's height above the foot at the back wall: lowest and tallest stretches")
        .f("crest_wavelength_m", "m", p, (4.0, 24.0), "how long a tall or low stretch of crest is")
        .f("foot.*", "fraction of the depth", Range::closed(-1.0, 1.0).open_hi(), (-0.5, 0.5), "where the profile leaves the ground (negative: past the front glass)")
        .f("foot_wavelength_m", "m", p, (4.0, 24.0), "how long a stretch of foot reaching forward or held back is")
        .f("profile", "exponent", p, (0.5, 3.0), "the profile's shape from foot to crest (1: straight)")
        .f("storey_m.*", "m", p, (1.0, 5.0), "storey height, from one terrace to the next")
        .f("storey_wavelength_m", "m", p, (4.0, 24.0), "how long a stretch of one storey height is")
        .f("drift", "storeys", n, (0.0, 0.5), "how far the levels drift up and down around the ring")
        .f("drift_wavelength_m", "m", p, (4.0, 32.0), "how long one rise or fall of the drift is")
        .f("tread.*", "fraction of a storey", f, (0.0, 0.5), "how much of a storey a tread climbs across its depth")
        .f("riser.*", "fraction of a storey", f, (0.01, 0.6), "how much of a storey a riser takes")
        .f("texture_wavelength_m", "m", p, (2.0, 16.0), "how long a stretch of one tread and riser character is")
        .f("roll_m", "m", n, (0.0, 0.5), "gentle roll on the treads, either way")
        .f("roll_wavelength_m", "m", p, (1.0, 8.0), "wavelength of the roll")
        .f("hardness", "0..1", f, (0.3, 0.9), "hardness of the massif's rock for erosion")
        .f("slab_m", "m", n, (0.25, 1.5), "thickness of the rock left over a hall")
        .f("shelves.*", "shelves", at_least_0, (0.0, 5.0), "how many shelves reach out toward the glass over a hall")
        .f("shelf_m.*", "m", p, (1.0, 8.0), "a shelf's length around the ring")
        .f("hall_m.*", "m", p, (1.0, 5.0), "headroom in a hall")
        .f("shelf_front.*", "fraction of the depth", Range::closed(0.0, 1.0).open_hi(), (0.0, 0.4), "where a shelf's edge stands, as a share of the depth from the glass")
        .f("hall_span_m", "m", n, (2.0, 12.0), "the widest a hall spans without a pillar")
        .f("pillar_m.*", "m", p, (0.5, 3.0), "width of a pillar in a hall")
        .f("arches.*", "arches", at_least_0, (0.0, 4.0), "how many arches span a saddle along the back wall")
        .f("arch_span_m.*", "m", p, (2.0, 12.0), "an arch's span, abutment to abutment")
        .f("arch_m", "m", n, (0.25, 1.5), "thickness of an arch's deck at its crown")
        .f("arch_depth_m.*", "m", p, (0.5, 3.0), "how far an arch's deck reaches from the back wall toward the glass")
        .f("arch_window_m", "m", n, (0.25, 1.5), "the least clear height under an arch's crown")
        .f("grove_headroom_m", "m", n, (1.0, 6.0), "clear headroom a grove site for the tallest trees needs")
        .f("grove_radius_m", "m", n, (0.5, 3.0), "room for a crown of this radius a grove site needs")
        .f("grove_soil_m", "m", n, (0.25, 1.5), "soil this deep a grove site needs")
        .f("spires.*", "spires", at_least_0, (0.0, 6.0), "how many spires rise from the crest")
        .f("spire_top_m.*", "m", p, (4.0, 18.0), "top of a spire")
        .f("spire_radius_m.*", "m", p, (0.25, 2.0), "a spire's radius near its foot")
        .f("edge_m", "m", n, (0.1, 0.75), "radius of every rounded edge")
        .f("warp_m", "m", n, (0.0, 1.0), "how far a gentle warp of the plan moves the terraces' edges")
        .f("warp_wavelength_m", "m", p, (2.0, 16.0), "wavelength of the warp")
        .f("lake_m.*", "m", p, (2.0, 8.0), "length of the lake around the ring")
        .f("lake_depth_m", "m", n, (0.25, 1.0), "depth of the lake's middle below the ground")
        .f("water_level_m", "m above the lowest yard ground", n, (0.125, 1.0), "where the water stands, above the ground the lake is graded into")
        .f("yard", "fraction of the depth", Range::closed(0.0, 1.0).open_hi(), (0.2, 0.5), "how deep the yard held clear for the lake is")
        .f("pool_m.*", "m", p, (0.25, 2.0), "a pool's radius around the ring")
        .f("pool_depth_m", "m", n, (0.125, 0.75), "a pool's depth")
        .f("stream_width_m", "m", p, (0.25, 1.5), "the stream's bed: how wide its trough is bank to bank")
        .f("stream_depth_m", "m", p, (0.125, 0.5), "how deep under the ground the stream runs")
        .f("ground_relief_wavelength_m", "m", p, (1.5, 8.0), "wavelength of the ground's gentle relief")
        .f("crest_contrast", "x", p, (1.0, 3.0), "contrast of the crest field (1: plain; above 1 pushes it toward its ends)")
        .f("riser_contrast", "x", p, (1.0, 3.0), "contrast of the riser field")
        .f("spire_squash", "x", p, (0.5, 1.5), "how much a spire's depth axis is squashed")
        .f("spire_flare", "radii", p, (1.0, 3.0), "how many radii a spire's flared foot reaches")
        .f("spire_tip_exponent", "exponent", p, (0.3, 1.5), "the exponent of a spire's tip")
        .f("spire_spacing_share", "fraction of the ring", f, (0.1, 0.5), "how far apart two spires stand")
        .f("spire_lake_clearance", "radii", n, (2.0, 8.0), "how many radii a spire keeps clear of the lake")
        .f("spire_back_radii", "radii", n, (1.0, 3.0), "how many radii a spire stands in from the back wall")
        .f("lake_pick_share", "fraction of the ring", Range::closed(0.0, 1.0).open_lo(), (0.05, 0.5), "the lake is placed among this share of the ring's most middling crest columns")
        .f("yard_blend_voxels", "voxels", Range::at_least(0.0), (2.0, 16.0), "how far beyond the lake's half-length the yard blends back")
        .f("shelf_gap_m", "m", n, (0.5, 3.0), "gap between two shelves")
        .f("shelf_lake_clear_m", "m", n, (0.5, 3.0), "clearance of a shelf from the lake")
        .f("shelf_tilt", "voxels per column", n, (0.0, 0.3), "the most a shelf's top tilts per column")
        .f("shelf_blend_m", "m", n, (0.5, 4.0), "the reach over which the massif blends up to a shelf")
        .f("shelf_storey_share", "fraction of a storey", n, (0.25, 1.25), "how much of a storey the massif rises on behind a shelf")
        .f("first_hall_extra_m", "m", n, (0.0, 2.0), "extra headroom the first hall is given over the grove's")
        .f("shelf_rise", "voxels per voxel of depth", n, (0.0, 0.3), "how much a shelf rises toward the back")
        .f("shelf_edge_exponent", "exponent", p, (0.5, 3.0), "the exponent of a shelf's rounded tongue")
        .f("erosion_soft_hardness", "0..1", n, (0.2, 0.7), "the hardness below which the erosion run treats rock as soft")
        .f("hall_arch_share", "fraction of the headroom", n, (0.1, 0.6), "how much of the headroom a bay's arch takes")
        .f("hall_foot_flare", "voxels", n, (0.0, 6.0), "how many voxels a hall's wall flares into the floor")
        .f("hall_depth_factor", "headrooms", p, (0.75, 1.5), "how deep a hall goes, in headrooms")
        .f("hall_floor_rise", "voxels per voxel of depth", n, (0.0, 0.3), "how much a hall's floor rises toward the back")
        .f("hall_back_round", "fraction of the headroom", n, (0.1, 0.8), "how much of the headroom the back wall rounds over")
        .f("arch_gap_m", "m", n, (0.5, 3.0), "gap between two arches")
        .f("arch_vault_exponent.*", "exponent", p, (1.2, 4.0), "range of an arch vault's exponent (round, flat or pointed)")
        .f("arch_hump.*", "voxels", Range::any(), (-3.0, 4.0), "range a deck humps (or sags, negative) by")
        .f("arch_waver", "voxels", n, (0.0, 3.0), "the waver of an arch's underside")
        .f("arch_widen", "voxels", n, (0.0, 4.0), "how much a deck widens into its abutments")
        .f("arch_widen_waver", "voxels", n, (0.0, 3.0), "the waver of the deck's widening")
        .f("river_climb_cost", "cost per voxel climbed", n, (5.0, 100.0), "what a route pays per voxel it climbs (a notch cut through)")
        .f("river_fall_cost", "cost per voxel dropped", n, (0.0, 5.0), "what a route pays per voxel it drops over a lip")
        .f("river_side_cost", "cost per voxel", n, (0.0, 20.0), "what a route pays per voxel the ground beside it falls away")
        .f("river_visible_weight", "score per visible column", n, (0.0, 1.0), "what a spring scores per column of its course the camera sees")
        .f("river_climb_weight", "score per voxel climbed", n, (0.0, 10.0), "what a spring's score loses per voxel its course climbs")
        .f("river_bed_slope", "voxels per column", n, (0.05, 0.5), "how much the stream's bed falls per column")
        .f("river_drop_every", "columns", Range::at_least(1.0), (2.0, 10.0), "columns between the stream's extra one-voxel falls");
    // The tsv's own numbers.
    for r in &mut t.rows {
        let tail = r.path.rsplit('.').next().unwrap_or_default().to_string();
        match tail.as_str() {
            "soil_m" | "grove_soil_m" | "water_level_m" => r.provenance = "decision (Wrysk 2026-09-22/23)",
            "lake_depth_m" => r.provenance = "tuned (hollow-count vs readability)",
            _ => {}
        }
    }
    rows.extend(t.rows);
    rows
}
