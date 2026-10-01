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

/// Every landform row.
pub(crate) fn rows() -> Vec<Row> {
    let (n, p, f) = (Range::nonneg(), Range::positive(), Range::fraction());
    let mut rows = Vec::new();
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
        .f("pockets", "pockets", Range::at_least(0.0), (0.0, 8.0), "soil pockets inside the rock");
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
        .f("shore_lip_m", "m", n, (0.0, 1.0), "width of the shore's dry lip at the water's edge");
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
        .f("stream_depth_m", "m", p, (0.125, 0.5), "how deep under the ground the stream runs");
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
