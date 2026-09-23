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
//! Then the solid work: wherever erosion left a cliff standing, a **gallery** — an open
//! level under the terrace above, arched, in bays between pillars, narrow near the ground
//! and wide higher up — is carved back into it, and the terrace's slab reaches out over
//! the mouth as a lip.
//!
//! Water: the **lake** is graded into a yard held clear at the massif's foot, and a
//! chain of **pools** is cut into the treads above it, each spilling over its tread's
//! edge into the pool below and the last into the lake. The finish applies the
//! two-voxel rule ([`crate::tidy`]), removes anything unsupported and lays soil on every
//! open floor.
//!
//! The camera is [`crate::hollows`]'s: a 30° section through `z = 0`, where a voxel of
//! height hides two of depth. The terracing only ever rises toward the back and a
//! gallery is never deeper than twice its height, so floors stay in view by construction
//! rather than by a pass that cuts them down afterwards.

use serde::{Deserialize, Serialize};

use crate::generate::{Budget, Heightfield, LakeDatum, repair_isolated, seat_outlet};
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
    /// Thickness of the slab over a gallery.
    pub slab_m: f64,
    /// How far the slab reaches out over a gallery's mouth.
    pub overhang_m: f64,
    /// A gallery's height: the least cliff worth opening, and the most it opens.
    pub gallery_height_m: [f64; 2],
    /// A gallery's depth into the cliff, in multiples of its height. The camera sees two
    /// voxels of floor per voxel of height, so more than two hides the back.
    pub gallery_depth: f64,
    /// Width of one bay in the galleries above the ground floor.
    pub bay_m: [f64; 2],
    /// Width of one bay on the ground floor: smaller rooms, more of them.
    pub base_bay_m: [f64; 2],
    /// Width of a pillar between bays.
    pub pillar_m: [f64; 2],
    /// Chance a bay is opened; the rest stay solid between open stretches.
    pub bay_chance: f64,
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
        ground_m: 0.5,
        ground_rise_m: 0.25,
        ground_relief_m: 0.0625,
        ground_fall: 0.04,
        soil_m: 0.25,
        mantle_m: 0.25,
        crest_m: [1.5, 7.0],
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
        slab_m: 0.375,
        overhang_m: 0.375,
        gallery_height_m: [0.75, 2.25],
        gallery_depth: 1.6,
        bay_m: [1.0, 2.5],
        base_bay_m: [0.625, 1.25],
        pillar_m: [0.375, 0.75],
        bay_chance: 0.8,
        spires: [1, 2],
        spire_top_m: [7.25, 8.0],
        spire_radius_m: [0.5, 0.75],
        edge_m: 0.25,
        warp_m: 0.375,
        warp_wavelength_m: 3.0,
        lake_m: [3.0, 4.5],
        lake_depth_m: 0.375,
        yard: 0.35,
        pool_m: [0.5, 0.875],
        pool_depth_m: 0.25,
        water: Water {
            min_lake_m2: 2.0,
            lake_depth_m: 0.375,
            ..Water::SMALL
        },
    };

    /// The desktop ring: 256 x 128 x 48 at 0.125 m, 32 m around, 16 m tall, 6 m deep.
    pub const DESK: Terrarium = Terrarium {
        ground_m: 0.75,
        ground_rise_m: 0.5,
        ground_relief_m: 0.0625,
        ground_fall: 0.03,
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
        slab_m: 0.5,
        overhang_m: 0.5,
        gallery_height_m: [1.0, 3.0],
        gallery_depth: 1.6,
        bay_m: [1.25, 3.0],
        base_bay_m: [0.75, 1.5],
        pillar_m: [0.5, 1.0],
        bay_chance: 0.8,
        spires: [1, 2],
        spire_top_m: [13.0, 14.5],
        spire_radius_m: [0.75, 1.1],
        edge_m: 0.375,
        warp_m: 0.5,
        warp_wavelength_m: 4.0,
        lake_m: [4.0, 6.0],
        lake_depth_m: 0.5,
        yard: 0.32,
        pool_m: [0.75, 1.25],
        pool_depth_m: 0.375,
        water: Water {
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
            ("gallery_height_m", self.gallery_height_m),
            ("bay_m", self.bay_m),
            ("base_bay_m", self.base_bay_m),
            ("pillar_m", self.pillar_m),
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
        anyhow::ensure!(
            self.spires[0] <= self.spires[1],
            "terrarium.spires is [least, most]"
        );
        self.water.validate()
    }
}

/// What one terrarium build made. Diagnostics: nothing reads it back.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Report {
    pub spires: usize,
    /// Cubic metres of rock and loose material erosion moved.
    pub eroded_m3: u64,
    /// Voxels opened into galleries.
    pub carved: usize,
    pub pools: usize,
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
    fn chance(&mut self, p: f64) -> bool {
        self.unit() < p
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
    Pad,
    Post,
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
    /// First solid `z` at height `y` in column `x`, from the front.
    fn face(&self, x: i64, y: i32, from: i32) -> Option<i32> {
        (from.max(0)..self.d).find(|&z| self.solid(x, y, z))
    }
    /// The nearest face across rows `lo..=hi`: a face wanders a voxel in and out from row
    /// to row, and whatever stands in front of it has to clear all of them.
    fn face_over(&self, x: i64, lo: i32, hi: i32, from: i32) -> Option<i32> {
        (lo..=hi).filter_map(|y| self.face(x, y, from)).min()
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
    /// Top solid row of each column after erosion: the terrain as a heightfield.
    ground: Vec<i32>,
    /// Loose material over each column's bedrock after erosion, voxels.
    loose: Vec<i32>,
    /// Columns graded into the lake.
    lake: Vec<bool>,
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
        terraced.max(spire)
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
        ground: vec![0; cells],
        loose: vec![0; cells],
        lake: vec![false; cells],
        report: Report::default(),
    };
    let site = layout(&mut b);
    heightfield(&mut b, site.centre);
    lay_lake(&mut b, &site);
    lay_ground(&mut b);
    galleries(&mut b);
    let spring = pools(&mut b, site.centre);
    finish(&mut b);

    b.report.spires = b.spires.len();
    let lake = datum(&b);
    b.report.lake = lake;
    let h = c.height as i32;
    world.material = b.g.mat;
    world.outlet_cell = Some((
        lake.rim.0 as u32,
        (lake.level_y + 1).min(h - 1) as u32,
        lake.rim.1 as u32,
    ));
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
        let cx = if s == 0 {
            0
        } else {
            (w / 4..3 * w / 4)
                .filter(|&x| lake_off(x) > 4 * r as i64)
                .max_by(|&a, &c| b.crest[a as usize].total_cmp(&b.crest[c as usize]))
                .unwrap_or(w / 2)
        };
        if lake_off(cx) <= 3 * r as i64 {
            continue;
        }
        let top = (b.draw(t.spire_top_m) as f64).min(h - 3.0);
        b.spires.push(Spire {
            cx: cx as f64 + 0.5,
            cz: d - 1.6 * r - 1.0,
            top,
            r,
        });
    }
    Site { centre, span }
}

// --- the heightfield -----------------------------------------------------------------------------

/// Sample the designed terrain through a gentle warp of the plan, lay loose material on
/// it, and let erosion work: the veneer slides off the risers into talus at their feet,
/// channels cut the ramps, and the treads collect soil.
fn heightfield(b: &mut Build, lake_x: i64) {
    let (w, d) = (b.g.w, b.g.d);
    let vm = b.c.voxel_m;
    let circ = w as f64 * vm;
    let warp = b.t.warp_m / vm;
    let warp_cells = ring_cells(circ, b.t.warp_wavelength_m);
    let roll = b.t.roll_m / vm;
    let roll_cells = ring_cells(circ, b.t.roll_wavelength_m);
    let relief_cells = ring_cells(circ, 3.0);
    let seed = b.c.seed ^ STREAM;
    let n = (w * d as i64) as usize;
    let mut bedrock = vec![0.0f64; n];
    let mut loose = vec![0.0f64; n];
    for z in 0..d {
        for x in 0..w {
            let (xm, zm) = ((x as f64 + 0.5) * vm, (z as f64 + 0.5) * vm);
            // The ground in front of and under the massif: low, climbing gently to the
            // back and away from the lake.
            let around = (x - lake_x).rem_euclid(w).min((lake_x - x).rem_euclid(w));
            let ground = b.t.ground_m / vm
                + b.t.ground_fall * around as f64
                + b.t.ground_rise_m / vm * z as f64 / (d - 1).max(1) as f64
                + b.t.ground_relief_m / vm * ring_noise(xm, zm, circ, relief_cells, seed ^ 0x47).clamp(-1.0, 1.0);
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

// --- galleries ------------------------------------------------------------------------------------

/// Carve an open level into every cliff erosion left standing: behind the foot of a
/// riser, under the terrace above it less a slab, back no deeper than the camera sees,
/// in bays between pillars — narrow near the ground, wide higher up. The terrace's slab
/// reaches out over the mouth as a lip.
fn galleries(b: &mut Build) {
    let (w, h, d) = (b.g.w, b.g.h, b.g.d);
    let t = b.t;
    let slab = b.v(t.slab_m).max(2);
    let lowest = b.v(t.gallery_height_m[0]).max(4);
    let tallest = b.v(t.gallery_height_m[1]).max(lowest);
    let lip = b.v(t.overhang_m);
    let e = b.v(t.edge_m) as f64;
    let near_ground = b.foot_y as i32 + b.v(t.gallery_height_m[1]) + 2;
    // Bay rhythms around the ring: one for the ground floor, several for the floors
    // above, chosen by a floor's height so stacked galleries do not line their pillars up.
    let mut rhythms: Vec<Vec<(f64, f64)>> = Vec::new();
    for k in 0..5 {
        rhythms.push(ring_bays(b, k == 0));
    }
    let bay_of = |x: i64, floor: i32| -> Option<(f64, f64)> {
        let k = if floor <= near_ground { 0 } else { 1 + (floor / 12).rem_euclid(4) as usize };
        let xf = x as f64 + 0.5;
        rhythms[k].iter().find_map(|&(start, width)| {
            let into = (xf - start).rem_euclid(w as f64);
            (into < width).then_some((into, width))
        })
    };
    let surf: Vec<i32> = b.ground.clone();
    let at = |x: i64, z: i32| surf[(z.clamp(0, d - 1) as i64 * w + x.rem_euclid(w)) as usize];
    let mut carved = 0usize;
    let mut lips: Vec<(i64, i32, i32)> = Vec::new();
    for x in 0..w {
        let mut z = 1;
        while z < d - 4 {
            let lower = at(x, z - 1);
            let upper = (z..(z + 3).min(d)).map(|zz| at(x, zz)).max().unwrap_or(lower);
            let lake = b.lake[((z - 1) as i64 * w + x) as usize];
            if lake || upper - lower < lowest + slab {
                z += 1;
                continue;
            }
            let height = (upper - slab - lower).min(tallest);
            let depth = ((t.gallery_depth * height as f64) as i32).min(d - 4 - z);
            let Some((into, width)) = bay_of(x, lower) else {
                z += 3;
                continue;
            };
            if depth < 3 {
                z += 3;
                continue;
            }
            let a = (e * 1.5).min(width / 2.0).min(height as f64 / 2.0);
            let to_side = into.min(width - into);
            let rounded_off = |u: f64, v: f64| u < a && v < a && (a - u).powi(2) + (a - v).powi(2) > a * a;
            let mut mouth = None;
            for zz in z..z + depth {
                let ceil = (at(x, zz) - slab).min(lower + height);
                for y in lower + 1..=ceil {
                    let below_ceiling = (ceil + 1 - y) as f64;
                    let to_back = (z + depth - zz) as f64;
                    if rounded_off(below_ceiling, to_side) || rounded_off(below_ceiling, to_back) {
                        continue;
                    }
                    if b.g.tag(x, y, zz) == Tag::Ground {
                        b.g.clear(x, y, zz);
                        carved += 1;
                        mouth.get_or_insert(zz);
                    }
                }
            }
            if let Some(m) = mouth {
                lips.push((x, m, at(x, m).min(h - 1)));
            }
            z += depth + 2;
        }
    }
    // Lips: the slab over each mouth reaches out, its underside and tip rounded.
    if lip > 0 {
        for (x, mouth, top) in lips {
            for k in 1..=lip {
                let zz = mouth - k;
                if zz < 1 {
                    break;
                }
                let tip = (lip - k) as f64 + 0.5;
                for y in top - slab + 1..=top {
                    let under = (y - (top - slab + 1)) as f64 + 0.5;
                    let over = (top - y) as f64 + 0.5;
                    if round_in(e.min(slab as f64 / 2.0), under) > tip || round_in(e, over) > tip {
                        continue;
                    }
                    if b.g.free(x, y, zz) {
                        b.g.put(x, y, zz, Tag::Mass);
                    }
                }
            }
        }
    }
    b.report.carved = carved;
}

/// Bays around the whole ring, pillars between: narrow at the base, wide in the mids.
/// Some bays stay solid, rooms between open stretches.
fn ring_bays(b: &mut Build, base: bool) -> Vec<(f64, f64)> {
    let w = b.g.w as f64;
    let range = if base { b.t.base_bay_m } else { b.t.bay_m };
    let mut out = Vec::new();
    let start = b.rng.range(0.0, w);
    let mut x = 0.0;
    loop {
        let pillar = b.draw(b.t.pillar_m) as f64;
        let bay = b.draw(range) as f64;
        if x + pillar + bay > w - b.v(b.t.pillar_m[0]) as f64 {
            break;
        }
        if b.rng.chance(b.t.bay_chance) {
            out.push(((start + x + pillar).rem_euclid(w), bay));
        }
        x += pillar + bay;
    }
    out
}

// --- pools ---------------------------------------------------------------------------------------

/// Cut a chain of pools into the treads above the lake, top down. Each sits on open,
/// gently sloping ground, takes the fall from the pool above, and spills through a notch
/// over its tread's edge; the lowest spills into the lake. The spring is in the top pool.
fn pools(b: &mut Build, centre: i64) -> Option<(u32, u32, u32)> {
    let (w, h, d) = (b.g.w, b.g.h, b.g.d);
    let x = centre + b.rng.int(-1, 1);
    let dep = b.v(b.t.pool_depth_m).max(2);
    // The open floor of columns `x` and `x + 1`, front to back.
    let surface = |b: &Build, z: i32| -> Option<i32> {
        let ys = (2..h - 1).rev().find(|&y| b.g.solid(x, y, z))?;
        (ys + 1..h)
            .all(|y| !b.g.solid(x + 1, y, z))
            .then_some(ys)
            .filter(|_| b.g.solid(x + 1, ys, z) || b.g.solid(x + 1, ys - 1, z))
    };
    let open: Vec<Option<i32>> = (0..d).map(|z| surface(b, z)).collect();
    // Treads: runs of open floor rising or falling no more than a voxel a step.
    let mut treads: Vec<(i32, i32)> = Vec::new();
    let mut z = 0;
    while z < d {
        if b.lake[(z as i64 * w + x.rem_euclid(w)) as usize] || open[z as usize].is_none() {
            z += 1;
            continue;
        }
        let start = z;
        while z + 1 < d
            && let (Some(a), Some(c)) = (open[z as usize], open[z as usize + 1])
            && (a - c).abs() <= 1
        {
            z += 1;
        }
        treads.push((start, z));
        z += 1;
    }
    let tread_of = |z: i32| treads.iter().copied().find(|&(lo, hi)| z >= lo && z <= hi);
    let mut landing: Option<i32> = None;
    let mut spring = None;
    // Start on the highest tread long enough to hold a pool.
    let mut next = treads
        .iter()
        .copied()
        .filter(|&(lo, hi)| hi - lo >= 7)
        .max_by_key(|&(lo, _)| open[lo as usize].unwrap_or(0));
    while let Some((z_lo, z_hi)) = next {
        let rx = b.draw(b.t.pool_m).max(3);
        let mut r = rx;
        let centre = |r: i32| landing.map_or(z_lo + 2 + r, |z| z - (r - 2));
        while r >= 2 && (centre(r) - r < z_lo + 2 || centre(r) + r > z_hi + 2) {
            r -= 1;
        }
        if r < 2 {
            break;
        }
        let cz = centre(r);
        let ys = (cz - r..=cz + r)
            .filter_map(|z| open.get(z as usize).copied().flatten())
            .max()
            .unwrap_or(0);
        carve_pool(b, x, ys, cz, rx, r, dep);
        // The notch: a groove one row deep and two wide from the bowl to the edge.
        let mut z = cz - r;
        while z >= 0 && (b.g.solid(x, ys, z) || b.g.solid(x + 1, ys, z)) {
            for xx in [x, x + 1] {
                b.g.keep(xx, ys, z);
                b.g.put(xx, ys - 1, z, Tag::Basin);
            }
            for xx in [x - 1, x + 2] {
                if b.g.solid(xx, ys, z) {
                    b.g.put(xx, ys, z, Tag::Basin);
                }
            }
            z -= 1;
        }
        // The fall, kept open to whatever it lands on.
        let fall = z.max(0);
        let mut y = ys;
        while y > 0 && !b.g.solid(x, y, fall) && !b.g.solid(x + 1, y, fall) {
            b.g.keep_air(x, y, fall);
            b.g.keep_air(x + 1, y, fall);
            y -= 1;
        }
        b.report.pools += 1;
        if spring.is_none() {
            spring = Some((x.rem_euclid(w) as u32, (ys - 1) as u32, cz as u32));
        }
        landing = Some(fall);
        if b.lake[(fall as i64 * w + x.rem_euclid(w)) as usize] {
            break;
        }
        next = tread_of(fall).filter(|&(lo, _)| lo < z_lo);
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
    let depth = b.v(b.t.soil_m).max(1);
    let (w, h, d) = (b.g.w, b.g.h, b.g.d);
    for z in 0..d {
        for x in 0..w {
            for y in (2..h).rev() {
                let Some(i) = b.g.at(x, y, z) else { continue };
                if !b.g.mat[i].is_solid() || b.g.solid(x, y + 1, z) {
                    continue;
                }
                for k in 0..depth {
                    let Some(j) = b.g.at(x, y - k, z) else { break };
                    if b.g.mat[j] != Material::Rock || b.g.tag[j] == Tag::Basin || y - k < 2 {
                        break;
                    }
                    b.g.mat[j] = Material::Soil;
                }
            }
        }
    }
}

/// The lake's datum on the ground it was graded into: the rim is the lowest ground
/// around it, and the outlet sits on a column of its own edge just under that.
fn datum(b: &Build) -> LakeDatum {
    let c = b.c;
    let (w, d) = (b.g.w, b.g.d);
    // The top of each column's first solid run: the ground as water standing on it sees it.
    let mut floor = vec![0i32; (w * d as i64) as usize];
    let lake = &b.lake;
    for z in 0..d {
        for x in 0..w {
            let mut y = 0;
            while b.g.solid(x, y + 1, z) {
                y += 1;
            }
            floor[(z as i64 * w + x) as usize] = y;
        }
    }
    let Some(low) = (0..floor.len()).filter(|&i| lake[i]).min_by_key(|&i| floor[i]) else {
        return LakeDatum::default();
    };
    let mut rim = i32::MAX;
    for i in 0..floor.len() {
        if !lake[i] {
            continue;
        }
        let (x, z) = (i as i64 % w, (i as i64 / w) as i32);
        for (dx, dz) in [(-1i64, 0i32), (1, 0), (0, -1), (0, 1)] {
            let (nx, nz) = ((x + dx).rem_euclid(w), z + dz);
            if nz < 0 || nz >= d {
                continue;
            }
            let j = (nz as i64 * w + nx) as usize;
            if !lake[j] {
                rim = rim.min(floor[j]);
            }
        }
    }
    let level = if rim == i32::MAX { floor[low] + 1 } else { rim };
    seat_outlet(&floor, c, low, floor[low], level)
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
