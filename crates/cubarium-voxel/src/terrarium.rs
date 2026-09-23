//! The **terrarium**: a designed ring of terraced massifs, built up rather than carved
//! out of relief.
//!
//! Wrysk, 2026-09-22, after `design/handoffs/landscape-generation-rethink-2026-09-22.md`:
//! the concept reference's depth and organic forms, with the terraces and the
//! spaciousness of a fancy cat tree, massed like Kowloon Walled City — a continuous ring
//! with one or two peak spires, busier mids and a densely packed base; mainly open
//! levels, with some chambers; a ring, not a floating island. A separate generator from
//! [`crate::generate`]'s natural relief, not a recipe of it.
//!
//! The **massif** is a designed heightfield. A smooth profile rises from a foot near
//! the glass to a crest at the back wall, tall in some stretches and low in others, and
//! is cut into terraces whose storey height, level offset, tread slope and riser all
//! drift around the ring: a level is never one height all round, a terrace followed
//! round the ring climbs to the next or falls to the last, and a riser is a cliff in one
//! stretch and a ramp in another. One or two needle **spires** rise out of the crest, the
//! first straddling the seam so it frames both edges of the screen. Loose material is
//! laid over it all and [`crate::erosion`] runs: the veneer slides off the risers into
//! talus, channels cut the slopes, soil gathers on the treads.
//!
//! Then the solid work. Two or three **shelves** carry an upper level forward toward the
//! glass, the massif rising on behind them, and a **hall** is opened under each: an open,
//! roofed floor carrying on from the level in front of its mouth, arched across bays
//! between pillars, seen through the glass in section. The first hall has headroom for
//! the tallest trees. One or two **arches** span a saddle along the back wall, a deck from
//! one mass to the next with sky under it.
//!
//! Water: the **lake** is graded into a yard held clear at the massif's foot and stands
//! at the **water level**, which is the water table: the lake is joined to the aquifer
//! through a drain across its floor, so there is no outlet and it rises and falls with the
//! table, and everything open below that line floods with it. The **stream** rises in a
//! pool on a high plateau, the one whose course scores best — how far it falls, how much
//! of it the camera sees, its waterfalls, how far round the ring it runs — and follows a
//! channel cut down to the lake. It is groundwater, the aquifer coming up at the spring,
//! so the weather keeps its own water. The finish applies the
//! two-voxel rule ([`crate::tidy`]), removes anything unsupported and lays soil on every
//! open floor, deep on gentle ground. The report counts **grove sites**: floors with the
//! headroom, crown room and soil the tallest trees need.
//!
//! The camera is [`crate::hollows`]'s: a 30° section through `z = 0`, where a voxel of
//! height hides two of depth. The terracing only ever rises toward the back, a hall is
//! never deeper than one and a half times its height, and an arch stands against the back
//! wall, so floors stay in view by construction rather than by a pass that cuts them down
//! afterwards.

use serde::{Deserialize, Serialize};

use crate::generate::{Budget, Heightfield, LakeDatum, repair_isolated};
use crate::noise::{ring_cells, ring_noise, smoothstep};
use crate::recipe::{Erosion, Water};
use crate::{Config, Material, World};

/// What a terrarium is built from, in metres unless said otherwise. Ranges are
/// `[least, most]`, drawn per part from the world's seed.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Terrarium {
    /// Ground at the front glass, metres above `y = 0`.
    pub ground_m: f64,
    /// How far the ground climbs from the glass to the back wall.
    pub ground_rise_m: f64,
    /// Gentle relief on the ground, either way.
    pub ground_relief_m: f64,
    /// How steeply the ground climbs away from the lake around the ring, metres per
    /// metre, so everything that lands on it drains to the lake.
    pub ground_fall: f64,
    /// Soil on every open floor.
    pub soil_m: f64,
    /// Loose material laid over the massif before erosion, which moves it.
    pub mantle_m: f64,
    /// The crest's height above the foot, at the back wall: its lowest and tallest
    /// stretches.
    pub crest_m: [f64; 2],
    /// How long a tall or low stretch of crest is.
    pub crest_wavelength_m: f64,
    /// Where the profile leaves the ground, as a share of the depth: negative reaches past
    /// the front glass, so the lower terraces meet it in section.
    pub foot: [f64; 2],
    /// How long a stretch of foot reaching forward or held back is.
    pub foot_wavelength_m: f64,
    /// The profile's shape from foot to crest: `1` is straight, more keeps the low
    /// terraces deep and crowds the high ones at the back.
    pub profile: f64,
    /// Storey height, from one terrace to the next: its range around the ring.
    pub storey_m: [f64; 2],
    /// How long a stretch of one storey height is.
    pub storey_wavelength_m: f64,
    /// How far the levels drift up and down around the ring, in storeys either way.
    /// Past half a storey a terrace followed round the ring meets the next one up.
    pub drift: f64,
    /// How long one rise or fall of the drift is.
    pub drift_wavelength_m: f64,
    /// How much of a storey a tread climbs across its depth: `0` is flat.
    pub tread: [f64; 2],
    /// How much of a storey a riser takes: small is a cliff, large a ramp.
    pub riser: [f64; 2],
    /// How long a stretch of one tread and riser character is.
    pub texture_wavelength_m: f64,
    /// Gentle roll on the treads, either way.
    pub roll_m: f64,
    pub roll_wavelength_m: f64,
    /// Hardness of the massif's rock for erosion, `0..=1`.
    pub hardness: f64,
    /// The erosion run over the designed terrain.
    pub erosion: Erosion,
    /// Thickness of the rock left over a hall.
    pub slab_m: f64,
    /// How many shelves reach out toward the glass over a hall: an upper level carried
    /// forward over an open, roofed floor, seen through the glass in section.
    pub shelves: [u32; 2],
    /// A shelf's length around the ring.
    pub shelf_m: [f64; 2],
    /// Headroom in a hall. The first takes the most: a grove floor for the tallest trees.
    pub hall_m: [f64; 2],
    /// Where a shelf's edge stands, as a share of the depth from the glass.
    pub shelf_front: [f64; 2],
    /// The widest a hall spans without a pillar.
    pub hall_span_m: f64,
    /// Width of a pillar in a hall.
    pub pillar_m: [f64; 2],
    /// How many arches span a saddle along the back wall, sky under them.
    pub arches: [u32; 2],
    /// An arch's span, abutment to abutment.
    pub arch_span_m: [f64; 2],
    /// Thickness of an arch's deck at its crown.
    pub arch_m: f64,
    /// How far an arch's deck reaches from the back wall toward the glass.
    pub arch_depth_m: [f64; 2],
    /// The least clear height under an arch's crown.
    pub arch_window_m: f64,
    /// What a grove site for the tallest trees needs: clear headroom, room for a crown
    /// of this radius from half that height up, and soil this deep. Level floors that
    /// wide gather soil that deep. Measured, not forced.
    pub grove_headroom_m: f64,
    pub grove_radius_m: f64,
    pub grove_soil_m: f64,
    /// How many spires rise from the crest. The first straddles the seam.
    pub spires: [u32; 2],
    /// Top of a spire.
    pub spire_top_m: [f64; 2],
    /// A spire's radius near its foot.
    pub spire_radius_m: [f64; 2],
    /// Radius of every rounded edge.
    pub edge_m: f64,
    /// How far a gentle warp of the plan moves the terraces' edges.
    pub warp_m: f64,
    /// Wavelength of the warp.
    pub warp_wavelength_m: f64,
    /// Length of the lake around the ring.
    pub lake_m: [f64; 2],
    /// Depth of the lake's middle below the ground.
    pub lake_depth_m: f64,
    /// Where the water stands, above the ground the lake is graded into. It is the water
    /// table: the lake is joined to the aquifer through a drain in its floor and rises and
    /// falls with it, and everything open below this line floods with the lake. Soil below
    /// it is saturated, so a grove wants its roots above it.
    pub water_level_m: f64,
    /// How deep the yard held clear for the lake is, as a share of the depth.
    pub yard: f64,
    /// A pool's radius around the ring.
    pub pool_m: [f64; 2],
    /// A pool's depth.
    pub pool_depth_m: f64,
    /// The water it starts with and the weather after, as a staged recipe states it.
    pub water: Water,
}

impl Default for Terrarium {
    fn default() -> Terrarium {
        Terrarium::SMALL
    }
}

impl Terrarium {
    /// The panel's ring: 160 x 72 x 24 at 0.125 m, 20 m around, 9 m tall, 3 m deep.
    pub const SMALL: Terrarium = Terrarium {
        ground_m: 1.0,
        ground_rise_m: 0.25,
        ground_relief_m: 0.0625,
        ground_fall: 0.08,
        soil_m: 0.25,
        mantle_m: 0.25,
        crest_m: [1.5, 6.5],
        crest_wavelength_m: 6.0,
        foot: [-0.3, 0.3],
        foot_wavelength_m: 5.0,
        profile: 1.0,
        storey_m: [2.0, 2.75],
        storey_wavelength_m: 8.0,
        drift: 0.6,
        drift_wavelength_m: 8.0,
        tread: [0.0, 0.2],
        riser: [0.02, 0.3],
        texture_wavelength_m: 5.0,
        roll_m: 0.125,
        roll_wavelength_m: 2.5,
        hardness: 0.6,
        erosion: Erosion::SMALL,
        slab_m: 0.75,
        shelves: [2, 3],
        shelf_m: [3.0, 6.0],
        hall_m: [1.5, 2.75],
        shelf_front: [0.0, 0.15],
        hall_span_m: 3.0,
        pillar_m: [0.75, 1.25],
        arches: [1, 2],
        arch_span_m: [2.0, 6.0],
        arch_m: 0.5,
        arch_depth_m: [0.75, 1.0],
        arch_window_m: 0.5,
        grove_headroom_m: 4.0,
        grove_radius_m: 1.0,
        grove_soil_m: 0.75,
        spires: [1, 2],
        spire_top_m: [7.25, 8.0],
        spire_radius_m: [0.5, 0.75],
        edge_m: 0.25,
        warp_m: 0.375,
        warp_wavelength_m: 3.0,
        lake_m: [3.0, 4.5],
        water_level_m: 0.25,
        lake_depth_m: 0.375,
        yard: 0.35,
        pool_m: [0.5, 0.875],
        pool_depth_m: 0.25,
        water: Water {
            // The stream no longer returns the sky's water, so the showers must. A shower
            // rains whatever the sky holds, up to more than it ever holds, so rain follows
            // evaporation; the low trigger sets how often. A small sky: the level is set by
            // the ground, not by what is aloft.
            shower_volume_m3: 1.0,
            shower_trigger_fraction: 0.005,
            // Evaporation well under the rain rate, so a shower reaches the ground rather
            // than lifting straight back off the films it wets. It no longer has to power
            // the stream, only the weather.
            evaporation_m_per_s: 1.0e-5,
            atmosphere_fraction: 0.02,
            // Groundwater now, so its size is for the eye and costs the weather nothing.
            reentry_m3_per_s: 1.6e-3,
            reentry_from_aquifer: true,
            lake_drain_m2_per_s: 0.2,
            min_lake_m2: 2.0,
            lake_depth_m: 0.375,
            ..Water::SMALL
        },
    };

    /// The desktop ring: 256 x 128 x 48 at 0.125 m, 32 m around, 16 m tall, 6 m deep.
    pub const DESK: Terrarium = Terrarium {
        ground_m: 1.25,
        ground_rise_m: 0.5,
        ground_relief_m: 0.0625,
        ground_fall: 0.06,
        soil_m: 0.25,
        mantle_m: 0.375,
        crest_m: [2.5, 12.5],
        crest_wavelength_m: 9.0,
        foot: [-0.25, 0.3],
        foot_wavelength_m: 7.0,
        profile: 1.0,
        storey_m: [2.75, 3.5],
        storey_wavelength_m: 10.0,
        drift: 0.6,
        drift_wavelength_m: 10.0,
        tread: [0.0, 0.2],
        riser: [0.02, 0.3],
        texture_wavelength_m: 7.0,
        roll_m: 0.25,
        roll_wavelength_m: 3.5,
        hardness: 0.6,
        erosion: Erosion::DEFAULT,
        slab_m: 1.0,
        shelves: [3, 4],
        shelf_m: [4.0, 8.0],
        hall_m: [2.0, 3.5],
        shelf_front: [0.0, 0.2],
        hall_span_m: 4.0,
        pillar_m: [1.0, 1.5],
        arches: [1, 2],
        arch_span_m: [3.0, 9.0],
        arch_m: 0.75,
        arch_depth_m: [1.0, 1.5],
        arch_window_m: 0.75,
        grove_headroom_m: 4.0,
        grove_radius_m: 1.0,
        grove_soil_m: 0.75,
        spires: [1, 2],
        spire_top_m: [13.0, 14.5],
        spire_radius_m: [0.75, 1.1],
        edge_m: 0.375,
        warp_m: 0.5,
        warp_wavelength_m: 4.0,
        lake_m: [4.0, 6.0],
        water_level_m: 0.375,
        lake_depth_m: 0.5,
        yard: 0.32,
        pool_m: [0.75, 1.25],
        pool_depth_m: 0.375,
        water: Water {
            // The stream no longer returns the sky's water, so the showers must. A shower
            // rains whatever the sky holds, up to more than it ever holds, so rain follows
            // evaporation; the low trigger sets how often. A small sky: the level is set by
            // the ground, not by what is aloft.
            shower_volume_m3: 3.0,
            shower_trigger_fraction: 0.005,
            // Evaporation well under the rain rate, so a shower reaches the ground rather
            // than lifting straight back off the films it wets. It no longer has to power
            // the stream, only the weather.
            evaporation_m_per_s: 1.0e-5,
            atmosphere_fraction: 0.02,
            // Groundwater now, so its size is for the eye and costs the weather nothing.
            reentry_m3_per_s: 8.0e-3,
            reentry_from_aquifer: true,
            lake_drain_m2_per_s: 0.8,
            min_lake_m2: 3.0,
            lake_depth_m: 0.5,
            ..Water::DEFAULT
        },
    };

    /// The shipped terrariums by `[world.landform] preset` name.
    pub const PRESETS: [(&'static str, Terrarium); 2] = [
        ("terrarium-small", Terrarium::SMALL),
        ("terrarium", Terrarium::DESK),
    ];

    /// A shipped terrarium by name.
    pub fn preset(name: &str) -> Option<Terrarium> {
        Terrarium::PRESETS
            .iter()
            .find(|(n, _)| *n == name)
            .map(|(_, t)| *t)
    }

    pub fn validate(&self) -> anyhow::Result<()> {
        let ranges = [
            ("crest_m", self.crest_m),
            ("storey_m", self.storey_m),
            ("shelf_m", self.shelf_m),
            ("hall_m", self.hall_m),
            ("pillar_m", self.pillar_m),
            ("arch_span_m", self.arch_span_m),
            ("arch_depth_m", self.arch_depth_m),
            ("spire_top_m", self.spire_top_m),
            ("spire_radius_m", self.spire_radius_m),
            ("lake_m", self.lake_m),
            ("pool_m", self.pool_m),
        ];
        for (name, [lo, hi]) in ranges {
            anyhow::ensure!(
                lo.is_finite() && hi.is_finite() && lo > 0.0 && lo <= hi,
                "terrarium.{name} must be a positive [least, most], not [{lo}, {hi}]"
            );
        }
        for (name, [lo, hi]) in [("tread", self.tread), ("riser", self.riser)] {
            anyhow::ensure!(
                (0.0..=1.0).contains(&lo) && (0.0..=1.0).contains(&hi) && lo <= hi,
                "terrarium.{name} is a [least, most] share of a storey, not [{lo}, {hi}]"
            );
        }
        anyhow::ensure!(
            self.foot[0] <= self.foot[1] && self.foot[1] < 1.0 && self.yard < 1.0,
            "terrarium: foot and yard are shares of the depth, foot [least, most]"
        );
        for (name, v) in [
            ("profile", self.profile),
            ("crest_wavelength_m", self.crest_wavelength_m),
            ("foot_wavelength_m", self.foot_wavelength_m),
            ("storey_wavelength_m", self.storey_wavelength_m),
            ("drift_wavelength_m", self.drift_wavelength_m),
            ("texture_wavelength_m", self.texture_wavelength_m),
            ("roll_wavelength_m", self.roll_wavelength_m),
            ("warp_wavelength_m", self.warp_wavelength_m),
        ] {
            anyhow::ensure!(v > 0.0 && v.is_finite(), "terrarium.{name} must be positive");
        }
        for (name, [lo, hi]) in [("spires", self.spires), ("shelves", self.shelves), ("arches", self.arches)] {
            anyhow::ensure!(lo <= hi, "terrarium.{name} is [least, most]");
        }
        anyhow::ensure!(
            (0.0..1.0).contains(&self.shelf_front[0]) && self.shelf_front[0] <= self.shelf_front[1] && self.shelf_front[1] < 1.0,
            "terrarium.shelf_front is a [least, most] share of the depth"
        );
        self.water.validate()
    }
}

/// What one terrarium build made. Diagnostics: nothing reads it back.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Report {
    pub spires: usize,
    /// Cubic metres of rock and loose material erosion moved.
    pub eroded_m3: u64,
    /// Halls opened under shelves, and the voxels carved out of them.
    pub halls: usize,
    pub carved: usize,
    pub arches: usize,
    /// Grove sites for the tallest trees (headroom, crown room, soil; crowns apart), and
    /// the floor area any one could stand on.
    pub groves: usize,
    pub grove_m2: u32,
    pub pools: usize,
    /// The stream: its length from the spring to the lake, and how far it falls.
    pub river_m: f64,
    pub river_drop_m: f64,
    /// The stream's course, spring first, as `(x, z)` columns.
    pub river: Vec<(u32, u32)>,
    /// What the two-voxel rule changed.
    pub tidied: crate::tidy::Thin,
    /// Solid cells with no path to the floor, removed.
    pub unsupported: usize,
    pub lake: LakeDatum,
}

const STREAM: u64 = 0x_5445_5252_4152_4955;

struct Rng(u64);

impl Rng {
    fn new(seed: u64) -> Rng {
        Rng(seed ^ 0x9E37_79B9_7F4A_7C15)
    }
    fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
    fn unit(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 / (1u64 << 53) as f64
    }
    fn range(&mut self, lo: f64, hi: f64) -> f64 {
        lo + (hi - lo) * self.unit()
    }
    /// Uniform in `lo..=hi`.
    fn int(&mut self, lo: i64, hi: i64) -> i64 {
        if hi <= lo {
            return lo;
        }
        lo + (self.next_u64() % (hi - lo + 1) as u64) as i64
    }
}

/// What put a cell where it is. Decides what the finish may touch.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Tag {
    Air,
    /// Void that must stay void: the lake, a bowl's water and the sky over it, a notch.
    Keep,
    Ground,
    Mass,
    /// A bowl's bed and rim, the lake's bed: rock that holds water, never soil.
    Basin,
}

struct Grid {
    w: i64,
    h: i32,
    d: i32,
    mat: Vec<Material>,
    tag: Vec<Tag>,
}

impl Grid {
    fn new(c: &Config) -> Grid {
        let n = c.cells();
        Grid {
            w: c.width as i64,
            h: c.height as i32,
            d: c.depth as i32,
            mat: vec![Material::Air; n],
            tag: vec![Tag::Air; n],
        }
    }
    fn at(&self, x: i64, y: i32, z: i32) -> Option<usize> {
        if y < 0 || y >= self.h || z < 0 || z >= self.d {
            return None;
        }
        let x = x.rem_euclid(self.w);
        Some(((y as i64 * self.d as i64 + z as i64) * self.w + x) as usize)
    }
    fn solid(&self, x: i64, y: i32, z: i32) -> bool {
        self.at(x, y, z).is_some_and(|i| self.mat[i].is_solid())
    }
    fn tag(&self, x: i64, y: i32, z: i32) -> Tag {
        self.at(x, y, z).map_or(Tag::Air, |i| self.tag[i])
    }
    /// Void and free to build in.
    fn free(&self, x: i64, y: i32, z: i32) -> bool {
        self.at(x, y, z)
            .is_some_and(|i| !self.mat[i].is_solid() && self.tag[i] != Tag::Keep)
    }
    fn put(&mut self, x: i64, y: i32, z: i32, tag: Tag) {
        if let Some(i) = self.at(x, y, z)
            && self.tag[i] != Tag::Keep
        {
            self.mat[i] = if y < 2 { Material::Bedrock } else { Material::Rock };
            self.tag[i] = tag;
        }
    }
    fn clear(&mut self, x: i64, y: i32, z: i32) {
        if let Some(i) = self.at(x, y, z)
            && self.tag[i] != Tag::Basin
        {
            self.mat[i] = Material::Air;
            if self.tag[i] != Tag::Keep {
                self.tag[i] = Tag::Air;
            }
        }
    }
    fn keep(&mut self, x: i64, y: i32, z: i32) {
        if let Some(i) = self.at(x, y, z) {
            self.mat[i] = Material::Air;
            self.tag[i] = Tag::Keep;
        }
    }
    /// Keep a void cell open; leave a solid one alone.
    fn keep_air(&mut self, x: i64, y: i32, z: i32) {
        if let Some(i) = self.at(x, y, z)
            && !self.mat[i].is_solid()
        {
            self.tag[i] = Tag::Keep;
        }
    }
}

#[derive(Clone, Debug)]
struct Spire {
    /// Its centre column around the ring and its depth.
    cx: f64,
    cz: f64,
    top: f64,
    r: f64,
}

/// How far a face recedes at distance `d` from an edge rounded to radius `e`.
fn round_in(e: f64, d: f64) -> f64 {
    if e <= 0.0 || d >= e {
        return 0.0;
    }
    let a = e - d.max(0.0);
    e - (e * e - a * a).max(0.0).sqrt()
}

/// Everything the passes share. Heights and lengths are voxels unless named `_m`.
struct Build<'a> {
    c: &'a Config,
    t: &'a Terrarium,
    g: Grid,
    rng: Rng,
    /// Height the massif rises from.
    foot_y: f64,
    /// Per column around the ring: the profile's crest at the back wall; where its foot
    /// leaves the ground (negative past the glass); the level offset, which drifts so a
    /// terrace climbs and falls around the ring; the local storey height; how much of a
    /// storey a tread climbs; how much of a storey a riser takes (small is a cliff, large
    /// a ramp).
    crest: Vec<f64>,
    foot: Vec<f64>,
    phase: Vec<f64>,
    storey: Vec<f64>,
    tread: Vec<f64>,
    riser: Vec<f64>,
    spires: Vec<Spire>,
    shelves: Vec<Shelf>,
    /// The lake's middle column, which the ground falls toward.
    lake_x: i64,
    /// The ground under everything, per column, voxels.
    base: Vec<f64>,
    /// Top solid row of each column after erosion: the terrain as a heightfield.
    ground: Vec<i32>,
    /// Loose material over each column's bedrock after erosion, voxels.
    loose: Vec<i32>,
    /// Columns graded into the lake.
    lake: Vec<bool>,
    /// Columns an arch's deck stands over: no stream is routed under one.
    arch: Vec<bool>,
    /// The rows the water fills: everything below this row, open to the sky and joined
    /// to the lake, is under water.
    water_y: i32,
    report: Report,
}

impl Build<'_> {
    fn v(&self, m: f64) -> i32 {
        (m / self.c.voxel_m).round() as i32
    }
    fn draw(&mut self, r: [f64; 2]) -> i32 {
        let (lo, hi) = (self.v(r[0]), self.v(r[1]));
        self.rng.int(lo as i64, hi.max(lo) as i64) as i32
    }
    fn ground_at(&self, x: i64, z: i32) -> i32 {
        let x = x.rem_euclid(self.g.w);
        self.ground[(z.clamp(0, self.g.d - 1) as i64 * self.g.w + x) as usize]
    }
    /// A per-column field at a fractional column, linearly between neighbours.
    fn at(field: &[f64], x: f64) -> f64 {
        let n = field.len() as f64;
        let x = (x - 0.5).rem_euclid(n);
        let (i, f) = (x.floor() as usize, x.fract());
        field[i] * (1.0 - f) + field[(i + 1) % field.len()] * f
    }

    /// The designed terrain height at a point of the plan, before erosion: the massif's
    /// profile, rising from its foot to its crest, cut into terraces whose height, spacing,
    /// tread and riser all drift around the ring.
    fn terraced(&self, x: f64, z: f64) -> f64 {
        let d = self.g.d as f64;
        let (crest, foot) = (Build::at(&self.crest, x), Build::at(&self.foot, x));
        let share = ((z - foot) / (d - foot)).clamp(0.0, 1.0);
        let h = self.foot_y + (crest - self.foot_y) * share.powf(self.t.profile);
        let (phase, storey) = (Build::at(&self.phase, x), Build::at(&self.storey, x).max(4.0));
        let (tread, riser) = (Build::at(&self.tread, x), Build::at(&self.riser, x));
        let u = (h - self.foot_y - phase) / storey;
        let (n, f) = (u.floor(), u - u.floor());
        let step = tread * f + (1.0 - tread) * smoothstep(1.0 - riser, 1.0, f);
        let terraced = self.foot_y + phase + storey * (n + step);
        let spire = self
            .spires
            .iter()
            .map(|s| {
                let dx = (x - s.cx).rem_euclid(self.g.w as f64);
                let dx = dx.min(self.g.w as f64 - dx);
                let r = (dx * dx + ((z - s.cz) / 0.85).powi(2)).sqrt() / s.r;
                // A needle with a flared foot and a rounded tip.
                if r >= 1.6 {
                    0.0
                } else {
                    s.top * (1.0 - (r / 1.6).powf(0.7))
                }
            })
            .fold(0.0f64, f64::max);
        let (w, d) = (self.g.w as f64, self.g.d as f64);
        let shelf = self.shelves.iter().map(|s| s.height(x, z, w, d)).fold(0.0f64, f64::max);
        terraced.max(spire).max(shelf)
    }

    /// The ground under everything at `(x, z)`, voxels: low at the glass, climbing gently
    /// to the back and away from the lake.
    fn base_at(&self, x: i64, z: i32) -> f64 {
        let (w, d) = (self.g.w, self.g.d);
        let vm = self.c.voxel_m;
        let circ = w as f64 * vm;
        let (xm, zm) = ((x.rem_euclid(w) as f64 + 0.5) * vm, (z as f64 + 0.5) * vm);
        let around = (x - self.lake_x).rem_euclid(w).min((self.lake_x - x).rem_euclid(w));
        let relief = ring_noise(xm, zm, circ, ring_cells(circ, 3.0), self.c.seed ^ STREAM ^ 0x47);
        self.t.ground_m / vm
            + self.t.ground_fall * around as f64
            + self.t.ground_rise_m / vm * z as f64 / (d - 1).max(1) as f64
            + self.t.ground_relief_m / vm * relief.clamp(-1.0, 1.0)
    }
}

/// Build the terrarium into `world`: its material, its outlet and its spring.
pub fn build(world: &mut World, t: &Terrarium) -> Report {
    let c = world.config.clone();
    let (w, cells) = (c.width as usize, c.width as usize * c.depth as usize);
    let mut b = Build {
        c: &c,
        t,
        g: Grid::new(&c),
        rng: Rng::new(c.seed ^ STREAM),
        foot_y: 0.0,
        crest: vec![0.0; w],
        foot: vec![0.0; w],
        phase: vec![0.0; w],
        storey: vec![0.0; w],
        tread: vec![0.0; w],
        riser: vec![0.0; w],
        spires: Vec::new(),
        shelves: Vec::new(),
        lake_x: 0,
        base: vec![0.0; cells],
        ground: vec![0; cells],
        loose: vec![0; cells],
        lake: vec![false; cells],
        water_y: 0,
        arch: vec![false; cells],
        report: Report::default(),
    };
    let site = layout(&mut b);
    heightfield(&mut b);
    lay_lake(&mut b, &site);
    lay_ground(&mut b);
    halls(&mut b);
    arches(&mut b);
    let spring = river(&mut b);
    finish(&mut b);

    b.report.spires = b.spires.len();
    let (sites, cells) = groves(&b);
    b.report.groves = sites;
    b.report.grove_m2 = (cells as f64 * c.voxel_m * c.voxel_m).round() as u32;
    let lake = datum(&b);
    b.report.lake = lake;
    // The drain is the lake's whole graded floor: the first void over every lake column.
    let (w, d) = (c.width as i64, c.depth as i32);
    let drain: Vec<(u32, u32, u32)> = (0..(w * d as i64) as usize)
        .filter(|&i| b.lake[i])
        .filter_map(|i| {
            let (x, z) = (i as i64 % w, (i as i64 / w) as i32);
            let y = (0..c.height as i32).find(|&y| !b.g.solid(x, y, z))?;
            Some((x as u32, y as u32, z as u32))
        })
        .collect();
    world.material = b.g.mat;
    world.lake_drain = drain;
    // No outlet: the lake stands at the water table, joined to the aquifer through a
    // drain in its floor, and the stream at the spring is that aquifer coming up. The
    // hydrate fills everything open below the water row; the datum is the row above it.
    world.outlet_cell = None;
    world.lake_datum_y = Some((b.water_y + 1) as u32);
    world.config.reentry_floor_head_m = (lake.floor_y + 1) as f64 * c.voxel_m;
    // The stream is the only spring: no head-driven seep, which under a table raised to
    // the lake would gush wherever the spring sat below it.
    world.config.spring_k_m2_per_s = 0.0;
    world.spring_cell = spring;
    repair_isolated(world);
    b.report
}

// --- layout ------------------------------------------------------------------------------------

struct Site {
    centre: i64,
    /// The lake's columns, `(first, last)` inclusive.
    span: (i64, i64),
}

/// A smooth field around the ring between `lo` and `hi`, pushed toward its ends when
/// `contrast` is above one.
fn ring_field(b: &Build, lo: f64, hi: f64, wavelength_m: f64, contrast: f64, stream: u64) -> Vec<f64> {
    let w = b.g.w as usize;
    let vm = b.c.voxel_m;
    let circ = w as f64 * vm;
    let cells = ring_cells(circ, wavelength_m);
    let fine = ring_cells(circ, wavelength_m / 2.5);
    let seed = b.c.seed ^ STREAM ^ stream;
    (0..w)
        .map(|x| {
            let xm = (x as f64 + 0.5) * vm;
            let n = 0.75 * ring_noise(xm, 0.0, circ, cells, seed) + 0.25 * ring_noise(xm, 0.0, circ, fine, seed ^ 1);
            let u = (0.5 + 0.5 * (contrast * n).clamp(-1.0, 1.0)).clamp(0.0, 1.0);
            let u = if contrast > 1.0 { u * u * (3.0 - 2.0 * u) } else { u };
            lo + (hi - lo) * u
        })
        .collect()
}

/// Lay out the massif's fields around the ring, its spires, and the lake's place in a
/// yard at its foot.
fn layout(b: &mut Build) -> Site {
    let (w, h, d) = (b.g.w, b.g.h as f64, b.g.d as f64);
    let t = b.t;
    b.foot_y = b.v(t.ground_m) as f64;
    let foot_y = b.foot_y;
    let v = |m: f64| m / b.c.voxel_m;
    b.crest = ring_field(b, foot_y + v(t.crest_m[0]), foot_y + v(t.crest_m[1]), t.crest_wavelength_m, 1.6, 0x43);
    b.foot = ring_field(b, t.foot[0] * d, t.foot[1] * d, t.foot_wavelength_m, 1.0, 0x46);
    b.storey = ring_field(b, v(t.storey_m[0]), v(t.storey_m[1]), t.storey_wavelength_m, 1.0, 0x53);
    // The offset drifts through more than a storey: a terrace followed round the ring
    // climbs to the next one or falls to the last, which is how the levels connect.
    let drift = (b.storey.iter().sum::<f64>() / b.storey.len() as f64) * t.drift;
    b.phase = ring_field(b, -drift, drift, t.drift_wavelength_m, 1.0, 0x50);
    b.tread = ring_field(b, t.tread[0], t.tread[1], t.texture_wavelength_m, 1.0, 0x54);
    b.riser = ring_field(b, t.riser[0], t.riser[1], t.texture_wavelength_m, 1.3, 0x52);

    // The lake's place: where the crest is middling, and a yard is cleared in front.
    let (lo, hi) = (foot_y + v(t.crest_m[0]), foot_y + v(t.crest_m[1]));
    let mid = (lo + hi) / 2.0;
    let mut order: Vec<usize> = (0..w as usize).collect();
    order.sort_by(|&a, &c| (b.crest[a] - mid).abs().total_cmp(&(b.crest[c] - mid).abs()));
    let centre = order[b.rng.int(0, (order.len() / 8).max(1) as i64 - 1) as usize] as i64;
    let len = b.draw(t.lake_m) as i64;
    let span = (centre - len / 2, centre + len / 2);
    let yard = t.yard * d;
    let margin = (len / 2 + 8) as f64;
    for x in 0..w {
        let off = (x - centre).rem_euclid(w).min((centre - x).rem_euclid(w)) as f64;
        let blend = ((off - margin) / margin).clamp(0.0, 1.0);
        let i = x as usize;
        b.foot[i] = b.foot[i].max(yard * (1.0 - blend) + b.foot[i] * blend);
    }

    // Spires: the first on the seam, a second on the tallest stretch of the far half,
    // each needle standing up out of the crest near the back.
    let count = b.rng.int(t.spires[0] as i64, t.spires[1] as i64) as usize;
    let lake_off = |x: i64| (x - centre).rem_euclid(w).min((centre - x).rem_euclid(w));
    for s in 0..count {
        let r = b.draw(t.spire_radius_m) as f64;
        // Clear of the lake and a quarter of the ring from any other spire.
        let far = |x: i64| {
            lake_off(x) > 4 * r as i64
                && b.spires.iter().all(|sp| {
                    let o = (x as f64 + 0.5 - sp.cx).rem_euclid(w as f64);
                    o.min(w as f64 - o) > w as f64 / 4.0
                })
        };
        let cx = if s == 0 && far(0) {
            0
        } else {
            let Some(cx) = (0..w).filter(|&x| far(x)).max_by(|&a, &c| b.crest[a as usize].total_cmp(&b.crest[c as usize])) else {
                continue;
            };
            cx
        };
        let top = (b.draw(t.spire_top_m) as f64).min(h - 3.0);
        b.spires.push(Spire {
            cx: cx as f64 + 0.5,
            cz: d - 1.6 * r - 1.0,
            top,
            r,
        });
    }

    // Shelves: the first, over the tallest hall, where the crest stands highest; the rest
    // spread round the ring, clear of the lake and of each other. Each stands a hall and a
    // slab above the level in front of its edge.
    b.lake_x = centre;
    let count = b.rng.int(t.shelves[0] as i64, t.shelves[1] as i64) as usize;
    let clear = len / 2 + b.v(1.0) as i64;
    let gap = b.v(1.0) as i64;
    let mut taken = vec![(centre - clear, 2 * clear + 1)];
    for k in 0..count {
        let slen = b.draw(t.shelf_m) as i64;
        let fits: Vec<i64> = (0..w).filter(|&x0| taken.iter().all(|&s| apart(s, (x0, slen), gap, w))).collect();
        if fits.is_empty() {
            break;
        }
        let x0 = if k == 0 {
            let crest = |x0: i64| b.crest[(x0 + slen / 2).rem_euclid(w) as usize];
            fits.iter().copied().max_by(|&a, &c| crest(a).total_cmp(&crest(c))).unwrap_or(0)
        } else {
            fits[b.rng.int(0, fits.len() as i64 - 1) as usize]
        };
        // The first hall is a grove floor: headroom for the tallest trees.
        let hall = if k == 0 { b.v(t.hall_m[1].max(t.grove_headroom_m + 0.5)) } else { b.draw(t.hall_m) };
        let front = b.rng.range(t.shelf_front[0], t.shelf_front[1]) * d;
        let zf = (front - 1.0).max(0.0);
        let before = (x0..x0 + slen)
            .map(|x| b.terraced(x as f64 + 0.5, zf + 0.5).max(b.base_at(x, zf as i32)))
            .fold(0.0, f64::max);
        let top = (before + (hall + b.v(t.slab_m) + 1) as f64).min(h - 6.0);
        let tilt = b.rng.range(-0.1, 0.1);
        // The massif rises on behind a shelf: it is one level of a stack, not a table.
        let storey = b.storey[(x0 + slen / 2).rem_euclid(w) as usize];
        let reach = b.v(1.5) as f64;
        for dx in -(reach as i64)..slen + reach as i64 {
            let to_end = if dx < 0 { -dx as f64 } else if dx >= slen { (dx - slen + 1) as f64 } else { 0.0 };
            let blend = 1.0 - smoothstep(0.0, reach, to_end);
            let i = (x0 + dx).rem_euclid(w) as usize;
            let want = (top + 0.75 * storey).min(h - 4.0);
            b.crest[i] = b.crest[i].max(b.crest[i] + (want - b.crest[i]).max(0.0) * blend);
        }
        taken.push((x0, slen));
        b.shelves.push(Shelf { x0, len: slen, front, hall, top, tilt });
    }
    Site { centre, span }
}

// --- the heightfield -----------------------------------------------------------------------------

/// Sample the designed terrain through a gentle warp of the plan, lay loose material on
/// it, and let erosion work: the veneer slides off the risers into talus at their feet,
/// channels cut the ramps, and the treads collect soil.
fn heightfield(b: &mut Build) {
    let (w, d) = (b.g.w, b.g.d);
    let vm = b.c.voxel_m;
    let circ = w as f64 * vm;
    let warp = b.t.warp_m / vm;
    let warp_cells = ring_cells(circ, b.t.warp_wavelength_m);
    let roll = b.t.roll_m / vm;
    let roll_cells = ring_cells(circ, b.t.roll_wavelength_m);
    let seed = b.c.seed ^ STREAM;
    let n = (w * d as i64) as usize;
    let mut bedrock = vec![0.0f64; n];
    let mut loose = vec![0.0f64; n];
    for z in 0..d {
        for x in 0..w {
            let (xm, zm) = ((x as f64 + 0.5) * vm, (z as f64 + 0.5) * vm);
            // The ground in front of and under the massif: low, climbing gently to the
            // back and away from the lake.
            let ground = b.base_at(x, z);
            b.base[(z as i64 * w + x) as usize] = ground;
            let px = x as f64 + 0.5 + warp * ring_noise(xm, zm, circ, warp_cells, seed ^ 1);
            let pz = z as f64 + 0.5 + warp * ring_noise(xm, zm + 50.0, circ, warp_cells, seed ^ 3);
            let mut top = b.terraced(px, pz);
            if top > ground + 1.0 {
                top += roll * ring_noise(xm, zm, circ, roll_cells, seed ^ 0x52);
            }
            let i = (z as i64 * w + x) as usize;
            let surface = top.max(ground) * vm;
            let mantle = b.t.mantle_m.min(surface - 2.0 * vm).max(0.0);
            bedrock[i] = surface - mantle;
            loose[i] = mantle;
        }
    }
    let mut field = Heightfield {
        width: w as usize,
        depth: d as usize,
        cell_m: vm,
        circumference_m: circ,
        spill_m: bedrock.iter().zip(&loose).map(|(r, l)| r + l).collect(),
        bedrock_m: bedrock,
        sediment_m: loose,
        hardness: vec![0.0; n],
        discharge: vec![0.0; n],
        hard_cap: vec![false; n],
        pool_rock: vec![i32::MAX; n],
        pool_spill: vec![i32::MIN; n],
        budget: Budget::default(),
    };
    let hardness = b.t.hardness;
    crate::erosion::erode(&mut field, &b.t.erosion, 0.45, |_, _, _| hardness);
    let h = b.g.h;
    for i in 0..n {
        let top = (field.surface_m(i) / vm).round() as i32;
        b.ground[i] = top.clamp(2, h - 3);
        b.loose[i] = (field.sediment_m[i] / vm).round() as i32;
    }
    b.report.eroded_m3 = ((field.budget.removed_bedrock_m + field.budget.removed_sediment_m) * vm * vm).round() as u64;
}

/// Grade the lake into the yard: deepest across its middle, shelving to nothing at its
/// ends, from the glass back toward the massif's foot. Nothing may be built over it.
fn lay_lake(b: &mut Build, site: &Site) {
    let w = b.g.w;
    let (span, centre) = (site.span, site.centre);
    let half = ((span.1 - span.0) as f64 / 2.0).max(1.0);
    let depth = b.v(b.t.lake_depth_m).max(2) as f64;
    let reach = b.v(b.t.warp_m) + 2;
    let back = |b: &Build, x: i64| -> i32 {
        (Build::at(&b.foot, x as f64 + 0.5).floor() as i32 - reach).clamp(2, b.g.d - 4)
    };
    // Graded from one flat reference, the lowest ground it is cut into, so the ground's
    // own relief leaves no bumps in its bed.
    let reference = (span.0..=span.1)
        .flat_map(|x| (0..=back(b, x)).map(move |z| (x, z)))
        .map(|(x, z)| b.ground_at(x, z))
        .min()
        .unwrap_or(0);
    b.water_y = reference + 1 + b.v(b.t.water_level_m);
    for x in span.0..=span.1 {
        let u = (x - centre) as f64 / half;
        let dep = (depth * (1.0 - u * u)).round() as i32;
        if dep < 1 {
            continue;
        }
        for z in 0..=back(b, x) {
            let i = (z as i64 * w + x.rem_euclid(w)) as usize;
            b.ground[i] = b.ground[i].min(reference - dep);
            b.lake[i] = true;
            let top = b.ground[i];
            for y in top + 1..b.g.h {
                b.g.keep(x, y, z);
            }
        }
    }
}

/// Lay the heightfield as voxels: rock, with the loose material erosion left on top as
/// soil, and the lake's bed as rock that holds water.
fn lay_ground(b: &mut Build) {
    let (w, d) = (b.g.w, b.g.d);
    for z in 0..d {
        for x in 0..w {
            let i = (z as i64 * w + x) as usize;
            let (top, loose, lake) = (b.ground[i], b.loose[i], b.lake[i]);
            for y in 0..=top {
                let Some(j) = b.g.at(x, y, z) else { continue };
                let (mat, tag) = if y < 2 {
                    (Material::Bedrock, Tag::Ground)
                } else if lake && y > top - 2 {
                    (Material::Rock, Tag::Basin)
                } else if y > top - loose {
                    (Material::Soil, Tag::Ground)
                } else {
                    (Material::Rock, Tag::Ground)
                };
                b.g.mat[j] = mat;
                b.g.tag[j] = tag;
            }
        }
    }
}

// --- shelves, halls and arches -------------------------------------------------------------------

/// A shelf: an upper level carried forward toward the glass, a hall under it.
#[derive(Clone, Debug)]
struct Shelf {
    /// Its first column around the ring and its length, columns.
    x0: i64,
    len: i64,
    /// Where its edge stands from the glass across its middle, voxels.
    front: f64,
    /// Headroom in its hall, voxels, the height of its top at its middle, and how much
    /// the top climbs per column around the ring.
    hall: i32,
    top: f64,
    tilt: f64,
}

impl Shelf {
    /// How far column `x` is into the shelf, `None` outside it.
    fn into(&self, x: f64, w: f64) -> Option<f64> {
        let dx = (x - self.x0 as f64).rem_euclid(w);
        (dx < self.len as f64).then_some(dx)
    }
    /// Where the shelf's edge stands at column `x`: at its front across the middle,
    /// curving back to the wall at its ends, so in plan it is a rounded tongue.
    fn edge(&self, x: f64, w: f64, d: f64) -> Option<f64> {
        let dx = self.into(x, w)?;
        let to_end = dx.min(self.len as f64 - dx);
        let r = (self.len as f64 / 4.0).clamp(1.0, 10.0);
        let u = smoothstep(0.0, r, to_end);
        Some(self.front + (d - self.front) * (1.0 - u).powf(1.5))
    }
    /// The shelf's height at a point of the plan, zero off it: level, rising a little to
    /// the back.
    fn height(&self, x: f64, z: f64, w: f64, d: f64) -> f64 {
        match self.edge(x, w, d) {
            Some(edge) if z >= edge => {
                let dx = self.into(x, w).unwrap_or(0.0) - self.len as f64 / 2.0;
                self.top + self.tilt * dx + 0.08 * (z - edge)
            }
            _ => 0.0,
        }
    }
}

/// Whether two stretches of the ring, `(start, length)`, stand at least `gap` apart.
fn apart(a: (i64, i64), b: (i64, i64), gap: i64, w: i64) -> bool {
    (b.0 - a.0).rem_euclid(w) >= a.1 + gap && (a.0 - b.0).rem_euclid(w) >= b.1 + gap
}

/// Open a hall under every shelf: from the level in front of its mouth, back no deeper
/// than the camera sees, under the shelf less a slab, arched across bays between
/// pillars, the back wall rounding into the ceiling.
fn halls(b: &mut Build) {
    let (w, d) = (b.g.w, b.g.d);
    let slab = b.v(b.t.slab_m).max(2);
    let span = (b.v(b.t.hall_span_m).max(8)) as i64;
    let shelves = b.shelves.clone();
    let surf = b.ground.clone();
    let at = |x: i64, z: i32| surf[(z.clamp(0, d - 1) as i64 * w + x.rem_euclid(w)) as usize];
    let mut carved = 0usize;
    let mut halls = 0usize;
    for s in &shelves {
        // Flanks at both ends bear the shelf; pillars split what lies between into bays.
        let flank = (b.v(b.t.pillar_m[0]) * 2 / 3) as i64;
        let open = s.len - 2 * flank;
        if open < 8 {
            continue;
        }
        let bays = ((open + span - 1) / span).max(1);
        let pillar = if bays > 1 { b.draw(b.t.pillar_m) as i64 } else { 0 };
        let bay = (open - pillar * (bays - 1)) as f64 / bays as f64;
        let hall = s.hall as f64;
        let before = carved;
        for dx in 0..open {
            let x = s.x0 + flank + dx;
            let u = dx as f64 + 0.5;
            let k = (u / (bay + pillar as f64)).floor();
            let into = u - k * (bay + pillar as f64);
            if into > bay {
                continue;
            }
            // The mouth: the first row back from the glass where the shelf stands, and
            // the level in front of it, which the hall's floor carries on from.
            let Some(mouth) = (0..d).find(|&z| at(x, z) >= s.top as i32 - slab) else {
                continue;
            };
            let floor = if mouth > 0 { at(x, mouth - 1) } else { b.base[x.rem_euclid(w) as usize].round() as i32 };
            // A hall stands above the water: its floor is dry ground, not a flooded bowl.
            let floor = floor.max(b.water_y);
            let arch = (hall * 0.35).min(bay / 2.0);
            let to_side = into.min(bay - into);
            let head = hall - round_in(arch, to_side);
            // Every wall flares into the floor.
            let foot = round_in(3.0, to_side).round() as i32;
            let deep = ((1.5 * head) as i32).min(d - 4 - mouth);
            for z in mouth..mouth + deep {
                let fl = floor + ((z - mouth) as f64 * 0.1) as i32;
                let to_back = (mouth + deep - z) as f64 - 0.5;
                let ceil = (floor as f64 + head - round_in(head * 0.4, to_back)) as i32;
                let ceil = ceil.min(at(x, z) - slab);
                for y in fl + 1 + foot..=ceil {
                    if b.g.solid(x, y, z) && b.g.tag(x, y, z) == Tag::Ground {
                        b.g.clear(x, y, z);
                        carved += 1;
                    }
                }
            }
        }
        if carved > before {
            halls += 1;
        }
    }
    b.report.halls = halls;
    b.report.carved = carved;
}

/// Span a saddle along the back wall with an arch: a deck from one mass to the next,
/// its underside vaulted, sky under it. Sitting against the wall, it hides no floor.
fn arches(b: &mut Build) {
    let (w, d) = (b.g.w, b.g.d);
    let t = b.t;
    let count = b.rng.int(t.arches[0] as i64, t.arches[1] as i64) as usize;
    let deck = b.v(t.arch_m).max(3);
    let window = b.v(t.arch_window_m).max(4);
    let (smin, smax) = (b.v(t.arch_span_m[0]).max(6) as i64, b.v(t.arch_span_m[1]) as i64);
    let (bmin, bmax) = (b.v(t.arch_depth_m[0]).max(3), b.v(t.arch_depth_m[1]));
    let bmax = bmax.max(bmin).min(d / 2);
    // The skyline along the back wall: each column's highest ground within the deck's reach.
    let back: Vec<i32> = (0..w)
        .map(|x| (d - bmax..d).map(|z| b.ground_at(x, z)).max().unwrap_or(0))
        .collect();
    let sky = |x: i64| back[x.rem_euclid(w) as usize];
    // Every saddle an arch could span: both abutments solid masses at least as high as
    // the deck, the window under the middle of it at least `window` clear.
    let mut spans: Vec<(i64, i64, i32, i32)> = Vec::new();
    for xa in 0..w {
        for len in smin..=smax.max(smin) {
            let xb = xa + len;
            let top = sky(xa).min(sky(xb)) - 1;
            if sky(xa - 1) < top || sky(xb + 1) < top {
                continue;
            }
            // The middle half of the span must clear the window; toward the abutments
            // the underside comes down to meet whatever stands there.
            let valley = (xa + len / 4..=xb - len / 4).map(sky).max().unwrap_or(top);
            let clear = top - deck - valley;
            if clear >= window {
                spans.push((xa, len, top, clear));
            }
        }
    }
    let mut made: Vec<(i64, i64)> = Vec::new();
    let gap = b.v(1.0) as i64;
    for _ in 0..count {
        let open: Vec<&(i64, i64, i32, i32)> = spans
            .iter()
            .filter(|s| made.iter().all(|&m| apart(m, (s.0, s.1 + 1), gap, w)))
            .collect();
        if open.is_empty() {
            break;
        }
        // Prefer a generous window, not the deepest gorge.
        let best = open.iter().map(|s| s.3.min(3 * window)).max().unwrap_or(0);
        let good: Vec<&&(i64, i64, i32, i32)> = open.iter().filter(|s| s.3.min(3 * window) >= best - 2).collect();
        let &&(xa, len, top, clear) = good[b.rng.int(0, good.len() as i64 - 1) as usize];
        let reach = b.rng.int(bmin as i64, bmax as i64) as i32;
        let under = (top - deck) as f64;
        // No two arches alike: a round, flat or pointed vault, a deck that humps or sags,
        // a gentle waver on every edge, in steps two columns wide.
        let p = b.rng.range(1.7, 2.8);
        let hump = b.rng.range(-1.0, 2.5);
        let phase: [f64; 3] = [b.rng.range(0.0, 6.3), b.rng.range(0.0, 6.3), b.rng.range(0.0, 6.3)];
        let waver = |x: i64, k: usize| {
            let u = (x & !1) as f64 / 7.0;
            (u + phase[k]).sin() * 0.6 + (2.3 * u + phase[(k + 1) % 3]).sin() * 0.4
        };
        for x in xa..=xa + len {
            let v = 2.0 * (x - xa) as f64 / len as f64 - 1.0;
            let vault = 1.0 - (1.0 - v.abs().powf(p)).max(0.0).powf(1.0 / p);
            let underside = under - clear as f64 * vault + 1.5 * waver(x, 0);
            let top = top + (hump * (1.0 - v * v) + waver(x, 1)).round() as i32;
            // The deck widens into its abutments.
            let deep = reach + (2.0 * v.abs().powi(4) + 1.2 * waver(x, 2)).round() as i32;
            for z in d - deep..d {
                let ground = b.ground_at(x, z);
                let lo = (underside.round() as i32).max(ground + 1);
                // Its front top edge chamfered.
                let hi = top - i32::from(z == d - deep);
                for y in lo..=hi {
                    if b.g.free(x, y, z) {
                        b.g.put(x, y, z, Tag::Mass);
                    }
                }
                b.arch[(z as i64 * w + x.rem_euclid(w)) as usize] = true;
            }
        }
        made.push((xa, len + 1));
    }
    b.report.arches = made.len();
}

// --- the river ----------------------------------------------------------------------------------

/// What one candidate spring's stream would be: the columns it runs through to the
/// water, and how it scores.
struct Course {
    path: Vec<(i64, i32)>,
    score: f64,
}

/// Where the stream rises and how it runs to the lake. Every column is given its cheapest
/// way down to the water — downhill or level costs its length, climbing a voxel costs a
/// notch cut through it — and every high, level plateau is scored as a spring by the
/// stream it would feed: how far it falls, how much of it the camera sees, how many
/// waterfalls it makes and how far round the ring it carries. The best is cut in: a pool
/// at the spring, a two-wide channel down every tread, the falls kept open, a plunge pool
/// under each tall one. The spring sits in the top pool.
fn river(b: &mut Build) -> Option<(u32, u32, u32)> {
    let (w, h, d) = (b.g.w, b.g.h, b.g.d);
    let n = (w * d as i64) as usize;
    let col = |x: i64, z: i32| (z as i64 * w + x.rem_euclid(w)) as usize;
    // The open surface: each column's highest solid.
    let top: Vec<i32> = (0..n)
        .map(|i| {
            let (x, z) = (i as i64 % w, (i as i64 / w) as i32);
            (0..h).rev().find(|&y| b.g.solid(x, y, z)).unwrap_or(0)
        })
        .collect();
    // The stream ends in **the lake**, not in whatever the raised level flooded nearest:
    // those it may cross, the lake it must reach.
    let wet: Vec<bool> = (0..n).map(|i| b.lake[i]).collect();
    let arch = b.arch.clone();
    let water = |i: usize| wet[i];
    let blocked = |i: usize| arch[i];
    let seen = |x: i64, z: i32| -> bool {
        let (mut ry, mut rz) = (top[col(x, z)] as f64 + 1.01, z as f64 + 0.5);
        while rz >= 0.0 && ry < h as f64 {
            if b.g.solid(x, ry as i32, rz as i32) {
                return false;
            }
            rz -= 0.25;
            ry += 0.125;
        }
        true
    };
    // Cheapest way down from every column, searched out from the water.
    const CLIMB: f64 = 30.0;
    const FALL: f64 = 0.35;
    let mut cost = vec![f64::INFINITY; n];
    let mut next = vec![usize::MAX; n];
    let mut heap = std::collections::BinaryHeap::new();
    for i in 0..n {
        if water(i) {
            cost[i] = 0.0;
            heap.push(std::cmp::Reverse((0u64, i)));
        }
    }
    let steps = [(1i64, 0i32, 1.0), (-1, 0, 1.0), (0, 1, 1.0), (0, -1, 1.0), (1, 1, 1.4), (1, -1, 1.4), (-1, 1, 1.4), (-1, -1, 1.4)];
    while let Some(std::cmp::Reverse((c, i))) = heap.pop() {
        let c = c as f64 / 1000.0;
        if c > cost[i] {
            continue;
        }
        let (x, z) = (i as i64 % w, (i as i64 / w) as i32);
        for &(dx, dz, len) in &steps {
            let (ax, az) = (x + dx, z + dz);
            if az < 0 || az >= d {
                continue;
            }
            let a = col(ax, az);
            if water(a) || blocked(a) {
                continue;
            }
            // Water at `a` runs into `i`: its length, a little for every voxel it drops so
            // it follows a ramp where there is one, and a notch cut for every voxel it climbs.
            let climb = (top[i] - top[a]).max(0) as f64;
            let fall = (top[a] - top[i] - 1).max(0) as f64;
            let ca = c + len + FALL * fall + CLIMB * climb;
            if ca < cost[a] {
                cost[a] = ca;
                next[a] = i;
                heap.push(std::cmp::Reverse(((ca * 1000.0) as u64, a)));
            }
        }
    }
    // Springs: level plateaus in the upper half of the ring's height, room for a pool.
    let highest = (0..n).filter(|&i| !blocked(i)).map(|i| top[i]).max().unwrap_or(0);
    let water_y = b.water_y;
    let lowest_spring = water_y + (highest - water_y) / 2;
    let level = |x: i64, z: i32| {
        (-3..=3).all(|dx| {
            (-2..=2).all(|dz| {
                let zz = z + dz;
                (1..d - 1).contains(&zz) && (top[col(x + dx, zz)] - top[col(x, z)]).abs() <= 1
            })
        })
    };
    let mut best: Option<Course> = None;
    for i in 0..n {
        let (x, z) = (i as i64 % w, (i as i64 / w) as i32);
        if top[i] < lowest_spring || !cost[i].is_finite() || blocked(i) || water(i) || !level(x, z) {
            continue;
        }
        let mut path = vec![(x, z)];
        let mut j = i;
        while !water(j) && next[j] != usize::MAX {
            j = next[j];
            path.push((j as i64 % w, (j as i64 / w) as i32));
        }
        let drop = (top[i] - water_y) as f64;
        let visible = path.iter().filter(|&&(x, z)| seen(x, z)).count() as f64;
        let falls = path
            .windows(2)
            .filter(|p| top[col(p[0].0, p[0].1)] - top[col(p[1].0, p[1].1)] >= 3)
            .count() as f64;
        let climb: f64 = path
            .windows(2)
            .map(|p| (top[col(p[1].0, p[1].1)] - top[col(p[0].0, p[0].1)]).max(0) as f64)
            .sum();
        // No reward for distance: the long way round to the lake reads as the wrong way.
        let score = drop + 0.3 * visible + 3.0 * falls - 3.0 * climb;
        if best.as_ref().is_none_or(|c| score > c.score) {
            best = Some(Course { path, score });
        }
    }
    let course = best?;
    b.report.river_m = course.path.len() as f64 * b.c.voxel_m;
    let &(sx, sz) = course.path.first()?;
    b.report.river_drop_m = (top[col(sx, sz)] - b.water_y) as f64 * b.c.voxel_m;

    // The spring's pool.
    let dep = b.v(b.t.pool_depth_m).max(2);
    let ys = top[col(sx, sz)];
    let rx = b.draw(b.t.pool_m).max(3);
    carve_pool(b, sx, ys, sz, rx, 2, dep);
    b.report.pools += 1;
    let spring = Some((sx.rem_euclid(w) as u32, (ys - 1) as u32, sz as u32));

    // The channel: a groove two rows deep, its bed never rising downstream, two wide —
    // the path's column and whichever neighbour across the flow stands nearer its height.
    // It leaves the pool a row under the plateau's surface, so the pool's only way out is
    // the channel and it never brims over onto the plateau and down every face.
    let mut bed = ys - 2;
    let path = course.path;
    b.report.river = path.iter().map(|&(x, z)| (x.rem_euclid(w) as u32, z as u32)).collect();
    for k in 1..path.len() {
        let (px, _) = path[k - 1];
        let (x, z) = path[k];
        if water(col(x, z)) {
            // Into the lake: keep the last fall open.
            let mut y = bed;
            while y > 0 && !b.g.solid(x, y, z) {
                b.g.keep_air(x, y, z);
                y -= 1;
            }
            break;
        }
        let here = top[col(x, z)];
        let fell = bed - (here - 1);
        bed = bed.min(here - 2);
        // A level run still falls a voxel every few columns, so the water does not have
        // to pile up at the spring to push itself along and thin to nothing downstream.
        if k % 5 == 0 {
            bed = (bed - 1).max(here - 3).min(bed);
        }
        // Across the flow: `x` for a step in `z`, `z` otherwise.
        let (ox, oz) = if x == px { (1i64, 0i32) } else { (0, 1) };
        let pair = [(x + ox, z + oz), (x - ox, z - oz)]
            .into_iter()
            .filter(|&(_, zz)| (0..d).contains(&zz))
            .min_by_key(|&(xx, zz)| (top[col(xx, zz)] - here).abs())
            .unwrap_or((x, z));
        // Water only passes between cells that share a face: a diagonal step also cuts
        // the corner cell between the two, or the channel leaks at every one.
        let (_, pz) = path[k - 1];
        let corner = (x != px && z != pz).then_some((x, pz));
        for (k2, (cx, cz)) in [Some((x, z)), corner, Some(pair)].into_iter().flatten().enumerate() {
            let t = top[col(cx, cz)];
            // The course's own column is always cut, a notch through any rise it climbs,
            // and so is a corner; the partner only where it stands near the bed, not up a
            // riser beside it.
            if k2 > 0 && Some((cx, cz)) == Some(pair) && t > bed + 4 {
                continue;
            }
            for y in bed + 1..=t.max(bed + 1) {
                b.g.keep(cx, y, cz);
            }
            b.g.put(cx, bed, cz, Tag::Basin);
        }
        // A tall fall lands in a plunge pool, if the landing is level enough to hold one.
        if fell >= 5 && k + 2 < path.len() && level(x, z) {
            carve_pool(b, x, here, z, 3, 2, 2);
            b.report.pools += 1;
        }
        // The fall itself, open from the lip down.
        if fell > 1 {
            for y in here + 1..=here + fell {
                b.g.keep_air(x, y, z);
                b.g.keep_air(pair.0, y, pair.1);
            }
        }
    }
    spring
}

/// A bowl sunk into the floor at row `ys`, rock-bedded two voxels round, with open sky
/// kept over it.
fn carve_pool(b: &mut Build, x: i64, ys: i32, cz: i32, rx: i32, r: i32, dep: i32) {
    let (xf, rf, df) = (rx as f64, r as f64, dep as f64);
    for xx in x - rx as i64 - 2..=x + rx as i64 + 2 {
        for zz in cz - r - 2..=cz + r + 2 {
            for yy in ys - dep - 2..=ys {
                let (u, v, s) = ((xx - x) as f64, (zz - cz) as f64, (ys - yy) as f64);
                let inner = (u / xf).powi(2) + (v / rf).powi(2) + (s / df).powi(2);
                let outer = (u / (xf + 2.0)).powi(2) + (v / (rf + 2.0)).powi(2) + (s / (df + 2.0)).powi(2);
                if inner <= 1.0 {
                    b.g.keep(xx, yy, zz);
                } else if outer <= 1.0 {
                    b.g.put(xx, yy, zz, Tag::Basin);
                }
            }
            let (u, v) = ((xx - x) as f64 / xf, (zz - cz) as f64 / rf);
            if u * u + v * v <= 1.0 {
                for yy in ys + 1..b.g.h {
                    b.g.keep_air(xx, yy, zz);
                }
            }
        }
    }
}

// --- finish ---------------------------------------------------------------------------------------


fn finish(b: &mut Build) {
    let c = b.c;
    let fixed: Vec<bool> = b
        .g
        .tag
        .iter()
        .map(|&t| matches!(t, Tag::Keep | Tag::Basin))
        .collect();
    b.report.tidied = crate::tidy::tidy(c, &mut b.g.mat, &fixed, 32);
    for i in 0..b.g.mat.len() {
        if b.g.mat[i].is_solid() && b.g.tag[i] == Tag::Air {
            b.g.tag[i] = Tag::Mass;
        }
        if !b.g.mat[i].is_solid() && !matches!(b.g.tag[i], Tag::Keep) {
            b.g.tag[i] = Tag::Air;
        }
    }
    b.report.unsupported = unsupported(b);
    soil(b);
}

/// Remove every solid with no face-connected path to the floor. A piece the tidy cut
/// loose, or a pad whose face was cut away behind it, does not hang in the air.
fn unsupported(b: &mut Build) -> usize {
    let c = b.c;
    let n = b.g.mat.len();
    let mut seen = vec![false; n];
    let mut stack: Vec<usize> = Vec::new();
    for x in 0..c.width as i64 {
        for z in 0..c.depth {
            let i = c.index(x, 0, z);
            if b.g.mat[i].is_solid() {
                seen[i] = true;
                stack.push(i);
            }
        }
    }
    while let Some(i) = stack.pop() {
        let (x, y, z) = c.coords(i);
        let (x, y, z) = (x as i64, y as i32, z as i32);
        for (dx, dy, dz) in [(-1, 0, 0), (1, 0, 0), (0, -1, 0), (0, 1, 0), (0, 0, -1), (0, 0, 1)] {
            if let Some(j) = b.g.at(x + dx, y + dy, z + dz)
                && !seen[j]
                && b.g.mat[j].is_solid()
            {
                seen[j] = true;
                stack.push(j);
            }
        }
    }
    let mut removed = 0;
    for i in 0..n {
        if b.g.mat[i].is_solid() && !seen[i] {
            b.g.mat[i] = Material::Air;
            b.g.tag[i] = Tag::Air;
            removed += 1;
        }
    }
    removed
}

/// Soil on every open floor: the top `soil_m` of each solid run under open air, except
/// where water is held by rock.
fn soil(b: &mut Build) {
    let shallow = b.v(b.t.soil_m).max(1);
    let deep = b.v(b.t.grove_soil_m).max(shallow);
    let r = b.v(b.t.grove_radius_m).max(1);
    let (w, h, d) = (b.g.w, b.g.h, b.g.d);
    for z in 0..d {
        for x in 0..w {
            for y in (2..h).rev() {
                let Some(i) = b.g.at(x, y, z) else { continue };
                if !b.g.mat[i].is_solid() || b.g.solid(x, y + 1, z) {
                    continue;
                }
                let depth = if level(&b.g, x, y, z, r) { deep } else { shallow };
                for k in 0..depth {
                    let Some(j) = b.g.at(x, y - k, z) else { break };
                    if b.g.mat[j] == Material::Soil {
                        continue;
                    }
                    if b.g.mat[j] != Material::Rock || b.g.tag[j] == Tag::Basin || y - k < 2 {
                        break;
                    }
                    b.g.mat[j] = Material::Soil;
                }
            }
        }
    }
}

/// Whether the floor at `(x, y, z)` is gentle ground that gathers soil: at `r / 2` round
/// it, six of the eight neighbours inside the walls have a floor within two voxels of it.
fn level(g: &Grid, x: i64, y: i32, z: i32, r: i32) -> bool {
    let floor_near = |x: i64, z: i32| (y - 2..=y + 2).any(|yy| g.solid(x, yy, z) && !g.solid(x, yy + 1, z));
    let k = (r / 2).max(1);
    let (mut inside, mut near) = (0, 0);
    for (dx, dz) in [(1, 0), (-1, 0), (0, 1), (0, -1), (1, 1), (1, -1), (-1, 1), (-1, -1)] {
        let (nx, nz) = (x + (dx * k) as i64, z + dz * k);
        if nz < 0 || nz >= g.d {
            continue;
        }
        inside += 1;
        near += i32::from(floor_near(nx, nz));
    }
    near >= inside - 2
}

/// Grove sites for the tallest trees: soil `grove_soil_m` deep, `grove_headroom_m` clear
/// over it, and no solid within `grove_radius_m` from half that height up, where a crown
/// spreads. Returns the sites, a crown's width apart, and the floor cells any one could
/// stand on.
fn groves(b: &Build) -> (usize, usize) {
    let (w, h, d) = (b.g.w, b.g.h, b.g.d);
    let head = b.v(b.t.grove_headroom_m);
    let r = b.v(b.t.grove_radius_m);
    let soil = b.v(b.t.grove_soil_m);
    let is_soil = |x: i64, y: i32, z: i32| b.g.at(x, y, z).is_some_and(|i| b.g.mat[i] == Material::Soil);
    let mut cells: Vec<(i64, i32)> = Vec::new();
    for z in 0..d {
        for x in 0..w {
            for y in 2..h {
                if !is_soil(x, y, z) || b.g.solid(x, y + 1, z) {
                    continue;
                }
                // Roots need air: at least the upper half of the root depth stands above the
                // water table. (How much saturation a species bears is its own
                // `establish_saturated_max`; this is the measure until the vaulttree has one.)
                if y + head >= h || y - soil / 2 + 1 < b.water_y || !(1..soil).all(|k| is_soil(x, y - k, z)) || (1..=head).any(|k| b.g.solid(x, y + k, z)) {
                    continue;
                }
                let crown = (-r..=r).all(|dz| {
                    (-r..=r).all(|dx| {
                        if dx * dx + dz * dz > r * r {
                            return true;
                        }
                        let zz = z + dz;
                        (0..d).contains(&zz) && (y + head / 2..=y + head).all(|yy| !b.g.solid(x + dx as i64, yy, zz))
                    })
                });
                if crown {
                    cells.push((x, z));
                }
            }
        }
    }
    let mut sites: Vec<(i64, i32)> = Vec::new();
    for &(x, z) in &cells {
        let far = sites.iter().all(|&(sx, sz)| {
            let o = (x - sx).rem_euclid(w);
            let o = o.min(w - o);
            o * o + ((z - sz) as i64).pow(2) >= (4 * r * r) as i64
        });
        if far {
            sites.push((x, z));
        }
    }
    (sites.len(), cells.len())
}

/// The lake's datum: its lowest floor, where the drain sits, and the row the water
/// stands at. `rim` is the drain's column.
fn datum(b: &Build) -> LakeDatum {
    let (w, d) = (b.g.w, b.g.d);
    let low = (0..(w * d as i64) as usize)
        .filter(|&i| b.lake[i])
        .min_by_key(|&i| {
            let (x, z) = (i as i64 % w, (i as i64 / w) as i32);
            (0..b.g.h).take_while(|&y| b.g.solid(x, y, z)).count()
        });
    let Some(i) = low else {
        return LakeDatum::default();
    };
    let (x, z) = (i as i64 % w, (i as i64 / w) as i32);
    let floor = (0..b.g.h).take_while(|&y| b.g.solid(x, y, z)).count() as i32 - 1;
    LakeDatum {
        floor_y: floor,
        level_y: b.water_y - 1,
        rim: (x as usize, z as usize),
    }
}

/// Floor cells — solid with open air above — and how many of them the camera sees: a ray
/// from each one toward the viewer rises half a voxel per voxel of depth and must leave
/// through the front glass or the sky without entering a solid. The rule the terrarium
/// is built to, measured.
pub fn floor_visibility(world: &World) -> (usize, usize) {
    let c = world.config();
    let solid = |x: i64, y: f64, z: f64| -> bool {
        if y < 0.0 || y >= c.height as f64 || z < 0.0 || z >= c.depth as f64 {
            return false;
        }
        world.material[c.index(x, y as u32, z as u32)].is_solid()
    };
    let (mut seen, mut all) = (0, 0);
    for i in 0..world.material.len() {
        if !world.material[i].is_solid() {
            continue;
        }
        let (x, y, z) = c.coords(i);
        if y + 1 >= c.height || world.material[c.index(x as i64, y + 1, z)].is_solid() {
            continue;
        }
        all += 1;
        let (mut ry, mut rz) = (y as f64 + 1.01, z as f64 + 0.5);
        let mut open = true;
        while rz >= 0.0 && ry < c.height as f64 {
            if solid(x as i64, ry, rz) {
                open = false;
                break;
            }
            rz -= 0.25;
            ry += 0.125;
        }
        if open {
            seen += 1;
        }
    }
    (seen, all)
}
