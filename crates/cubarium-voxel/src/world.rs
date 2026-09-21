use anyhow::{bail, ensure};
use serde::{Deserialize, Serialize};

use crate::recipe::Landform;
use crate::{Config, Ledger, Material};

/// The sky-visibility fan: `(dx, dy, dz, weight)` for each of 17 rays. The zenith
/// carries weight 1 and each elevated ray the sine of its elevation, which is the
/// cosine weighting of a hemispherical integral sampled at two rings. Written out
/// rather than computed so the table is exactly symmetric under negating `dx`: an `x`
/// reflection of a fixture reflects its sky visibility exactly.
const RAY_FAN: [(f64, f64, f64, f64); 17] = [
    // Zenith.
    (0.0, 1.0, 0.0, 1.0),
    // Eight azimuths at 60 degrees of elevation, weight sin 60.
    (0.5, 0.8660254037844386, 0.0, 0.8660254037844386),
    (
        0.3535533905932738,
        0.8660254037844386,
        0.3535533905932738,
        0.8660254037844386,
    ),
    (0.0, 0.8660254037844386, 0.5, 0.8660254037844386),
    (
        -0.3535533905932738,
        0.8660254037844386,
        0.3535533905932738,
        0.8660254037844386,
    ),
    (-0.5, 0.8660254037844386, 0.0, 0.8660254037844386),
    (
        -0.3535533905932738,
        0.8660254037844386,
        -0.3535533905932738,
        0.8660254037844386,
    ),
    (0.0, 0.8660254037844386, -0.5, 0.8660254037844386),
    (
        0.3535533905932738,
        0.8660254037844386,
        -0.3535533905932738,
        0.8660254037844386,
    ),
    // The same eight at 30 degrees, weight sin 30.
    (0.8660254037844386, 0.5, 0.0, 0.5),
    (0.6123724356957946, 0.5, 0.6123724356957946, 0.5),
    (0.0, 0.5, 0.8660254037844386, 0.5),
    (-0.6123724356957946, 0.5, 0.6123724356957946, 0.5),
    (-0.8660254037844386, 0.5, 0.0, 0.5),
    (-0.6123724356957946, 0.5, -0.6123724356957946, 0.5),
    (0.0, 0.5, -0.8660254037844386, 0.5),
    (0.6123724356957946, 0.5, -0.6123724356957946, 0.5),
];

/// A frontend or a test changes the world only through these. Every one of them takes
/// effect at the moment [`World::apply`] is called, paused or not, and `apply` returns
/// the volume in cubic metres it accepted — see there for the receipt contract.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Command {
    /// One rain event: `volume_m3` spread now over the sky-exposed cells, sharing it
    /// evenly and handing what a full cell refuses to the others. A non-finite or
    /// non-positive volume is refused whole and nothing is booked.
    RainPulse { volume_m3: f64 },
    /// Add free water into one voxel, clipped to its remaining void space. `apply`
    /// returns the volume accepted; the rest is refused, and a non-finite or negative
    /// volume is refused whole with nothing booked.
    AddWater {
        x: i64,
        y: u32,
        z: u32,
        volume_m3: f64,
    },
    /// Replace a voxel's material, preserving the water volume the voxel held.
    ///
    /// Its free and pore water are converted to cubic metres first, then the new
    /// material keeps what fits: air keeps free water up to its void, soil and rock keep
    /// pore water up to their own pore capacity, and a solid turned to air releases its
    /// pore water as free water. The rest is displaced to the nearest void with room —
    /// wrapped face-adjacent void path distance, sharing equally among equal-distance
    /// recipients before any farther one — and only volume with no reachable room at all
    /// is booked as `Ledger::displaced_out`.
    SetMaterial {
        x: i64,
        y: u32,
        z: u32,
        material: Material,
    },
    /// Take pore water out of one voxel: the plant layer's one bounded withdrawal.
    ///
    /// Capped by the stock actually in that voxel, so a whole stand of plants asking
    /// for more than the soil holds gets the soil's water and no more. `apply` returns
    /// the accepted volume as a **negative** number — the [`Command::ChargeAquifer`]
    /// withdrawal convention — and books it as [`Ledger::transpiration_out`], a loss:
    /// transpired water leaves the world. A non-finite or negative volume is refused
    /// whole with nothing booked, and a voxel with no pore capacity (air, bedrock)
    /// accepts nothing and books nothing.
    WithdrawPore {
        x: i64,
        y: u32,
        z: u32,
        volume_m3: f64,
    },
    /// Add water to the lumped atmosphere store: the closed budget's "make it rain"
    /// lever, which puts water aloft for the next shower to bring down rather than
    /// dropping it on the world now.
    ///
    /// Booked as [`Ledger::user_atmosphere_in`] and counted in `atmosphere_in`, so the
    /// store's own residual stays zero and the world's total water rises by exactly this
    /// much. Refused whole — nothing booked — on a non-finite or non-positive volume, or
    /// on an **open-budget** world, which has no atmosphere to add to.
    AddAtmosphere { volume_m3: f64 },
    /// Add to (or, negative, remove from) the aquifer store. A withdrawal is capped by
    /// the stock actually there, and `apply` returns it as a negative volume.
    ChargeAquifer { volume_m3: f64 },
    /// Open or close the named outlet.
    SetOutlet { open: bool },
}

/// Read-only access to the world for drawing and inspection. Borrowed from a `World`;
/// never owned by a renderer.
#[derive(Clone, Copy, Debug)]
pub struct VoxelView<'a> {
    pub config: &'a Config,
    /// One entry per voxel in `Config::index` order.
    pub material: &'a [Material],
    /// Free water as a fraction of the voxel's void volume, `0..=1`. Zero in solids.
    pub free: &'a [f64],
    /// Pore water as a fraction of the voxel's pore capacity, `0..=1`. Zero in air.
    pub pore: &'a [f64],
    pub tick: u64,
    /// Counts committed material changes. A reader that caches anything derived from
    /// the terrain alone — sky visibility, support faces — recomputes when this moves
    /// and never otherwise.
    pub terrain_version: u64,
    pub ledger: &'a Ledger,
    /// Aquifer store in cubic metres.
    pub aquifer_m3: f64,
    /// The lumped atmosphere store in cubic metres; zero under the open budget.
    pub atmosphere_m3: f64,
    /// Allowance left in the current shower, in cubic metres. Zero when not raining.
    pub shower_left_m3: f64,
    /// Whether the named outlet is exporting.
    pub outlet_open: bool,
    /// The outlet cell `(x, y, z)`, if the world names one.
    pub outlet: Option<(u32, u32, u32)>,
    /// The spring cell `(x, y, z)`, if the world names one.
    pub spring: Option<(u32, u32, u32)>,
}

impl<'a> VoxelView<'a> {
    /// True if rain is falling this tick (prescribed or active shower).
    #[inline]
    pub fn is_raining(&self) -> bool {
        self.config.rain_m_per_s > 0.0 || self.shower_left_m3 > 0.0
    }

    #[inline]
    pub fn material_at(&self, x: i64, y: u32, z: u32) -> Material {
        self.material[self.config.index(x, y, z)]
    }
    #[inline]
    pub fn free_at(&self, x: i64, y: u32, z: u32) -> f64 {
        self.free[self.config.index(x, y, z)]
    }
    #[inline]
    pub fn pore_at(&self, x: i64, y: u32, z: u32) -> f64 {
        self.pore[self.config.index(x, y, z)]
    }
    /// Highest solid voxel in column `(x, z)`, or `None` if the column is all air.
    pub fn surface_y(&self, x: i64, z: u32) -> Option<u32> {
        (0..self.config.height)
            .rev()
            .find(|&y| self.material_at(x, y, z).is_solid())
    }
    /// Whether `(x, y, z)` is a **support face**: a solid voxel whose top face is
    /// exposed to void inside the world. Plants stand on these, never on a column's
    /// skyline as such: the skyline solid at `y = height - 1` has no room above it and
    /// is not a support, while a roofed floor is one.
    pub fn is_support(&self, x: i64, y: u32, z: u32) -> bool {
        let c = self.config;
        y + 1 < c.height
            && z < c.depth
            && self.material_at(x, y, z).is_solid()
            && !self.material_at(x, y + 1, z).is_solid()
    }

    /// Every support face in column `(x, z)`, ascending in `y`.
    pub fn supports_in_column(&self, x: i64, z: u32) -> Vec<u32> {
        if z >= self.config.depth {
            return Vec::new();
        }
        (0..self.config.height)
            .filter(|&y| self.is_support(x, y, z))
            .collect()
    }

    /// Depth of the free water standing on the support face `(x, y, z)`, in metres, as
    /// the stores hold it: the fills of the void cells from `y + 1` up while each one
    /// holds water, summed and multiplied by `voxel_m`. Unrounded — a 0.3-full cell is
    /// 0.3 voxels of water — and zero when the cell directly above is dry, so a film
    /// running down a slope reads as the film it is.
    pub fn water_depth_m(&self, x: i64, y: u32, z: u32) -> f64 {
        let c = self.config;
        if z >= c.depth {
            return 0.0;
        }
        let mut depth = 0.0;
        for yy in y + 1..c.height {
            if self.material_at(x, yy, z).is_solid() {
                break;
            }
            let f = self.free_at(x, yy, z);
            if !(f > 0.0) {
                break;
            }
            depth += f;
        }
        depth * c.voxel_m
    }

    /// How many contiguous [`Material::Soil`] voxels sit at `(x, y, z)` and below it,
    /// `y` included: one column's contiguous soil, and nothing else. Zero when
    /// `(x, y, z)` is itself not soil.
    ///
    /// It is **not** a plant's root box and says nothing about one (Astra R7.6). The
    /// flora layer collects the `Material::Soil` voxels of the whole box itself, per
    /// voxel and not contiguously (`crates/cubarium-voxel-flora/src/step.rs`'s
    /// `root_box`), so a stand on a rock face can have soil in reach beside or under it
    /// while this returns zero for its own column.
    pub fn soil_below(&self, x: i64, y: u32, z: u32) -> u32 {
        if z >= self.config.depth || y >= self.config.height {
            return 0;
        }
        let mut n = 0;
        for yy in (0..=y).rev() {
            if self.material_at(x, yy, z) != Material::Soil {
                break;
            }
            n += 1;
        }
        n
    }

    /// Pore water in one voxel, cubic metres: its fill times its material's pore
    /// capacity times the voxel volume. Zero in air and bedrock.
    pub fn pore_water_m3(&self, x: i64, y: u32, z: u32) -> f64 {
        let i = self.config.index(x, y, z);
        self.pore[i] * self.material[i].pore_capacity() * self.config.voxel_volume()
    }

    /// Geometric sky visibility of the top face of `(x, y, z)`: the cosine-weighted
    /// fraction of a fixed 17-ray fan from the centre of that face that leaves the world
    /// without entering a solid.
    ///
    /// The fan is the zenith (weight 1), eight azimuths at 45-degree steps at 60 degrees
    /// of elevation (weight `sin 60`) and the same eight at 30 degrees (weight
    /// `sin 30`). A ray leaves the world at `y >= height` or at `z` outside
    /// `0..depth`; `x` wraps, so the strip never has a side to escape through. Marching
    /// is by fixed sub-voxel steps of a quarter voxel, sampling the voxel each step
    /// lands in: cheap, and exactly symmetric under an `x` reflection because the
    /// azimuth set is closed under negating the `x` component.
    ///
    /// Pure geometry. Canopies are not here — a plant layer multiplies its own
    /// attenuation onto this.
    pub fn sky_visibility(&self, x: i64, y: u32, z: u32) -> f64 {
        let c = self.config;
        if z >= c.depth || y + 1 >= c.height {
            // Nothing above the top face of the topmost row to be blocked by, and
            // nothing to march through either: it is open sky.
            return if z < c.depth { 1.0 } else { 0.0 };
        }
        let origin = (x as f64 + 0.5, (y + 1) as f64, z as f64 + 0.5);
        let mut total = 0.0;
        let mut open = 0.0;
        for &(dx, dy, dz, weight) in RAY_FAN.iter() {
            total += weight;
            if self.ray_escapes(origin, (dx, dy, dz)) {
                open += weight;
            }
        }
        open / total
    }

    /// March one ray by quarter-voxel steps: `true` if it leaves the world, `false` if
    /// it enters a solid first. Every ray in the fan climbs, so `y` grows without bound
    /// and the walk always ends.
    fn ray_escapes(&self, origin: (f64, f64, f64), dir: (f64, f64, f64)) -> bool {
        let c = self.config;
        const STEP: f64 = 0.25;
        let limit = 8 * c.height as u64 + 8 * c.depth as u64 + 16;
        for k in 1..=limit {
            let t = STEP * k as f64;
            let px = origin.0 + dir.0 * t;
            let py = origin.1 + dir.1 * t;
            let pz = origin.2 + dir.2 * t;
            if py >= c.height as f64 {
                return true;
            }
            if pz < 0.0 || pz >= c.depth as f64 {
                return true;
            }
            let (vy, vz) = (py.floor() as u32, pz.floor() as u32);
            if self.material_at(px.floor() as i64, vy, vz).is_solid() {
                return false;
            }
        }
        // Unreachable for a climbing ray; a blocked answer is the safe one.
        false
    }

    /// Total free plus pore plus aquifer water in cubic metres: the **in-world** stores,
    /// which is what [`Ledger::expected_stored`] explains. The atmosphere is not in here
    /// under either budget; [`VoxelView::total_water_m3`] adds it.
    pub fn stored_m3(&self) -> f64 {
        let v = self.config.voxel_volume();
        let mut total = self.aquifer_m3;
        for (i, m) in self.material.iter().enumerate() {
            total += self.free[i] * v * if m.is_solid() { 0.0 } else { 1.0 };
            total += self.pore[i] * v * m.pore_capacity();
        }
        total
    }

    /// Every store the world owns: [`VoxelView::stored_m3`] plus the atmosphere.
    pub fn total_water_m3(&self) -> f64 {
        self.stored_m3() + self.atmosphere_m3
    }

    /// The in-world conservation residual, `stored - expected`. Zero to floating-point
    /// noise under either budget, and any term in it is a real leak.
    pub fn water_residual(&self) -> f64 {
        self.stored_m3() - self.ledger.expected_stored()
    }

    /// The atmosphere's own residual, `atmosphere - expected`. Zero under either budget:
    /// an open-budget world never deposits and never draws.
    pub fn atmosphere_residual(&self) -> f64 {
        self.atmosphere_m3 - self.ledger.expected_atmosphere()
    }

    /// Both residuals at once — the one number a closed-budget run asserts on. Not a sum
    /// of signed errors that could cancel: it is `total - expected_total`, and the two
    /// halves are checkable separately above.
    pub fn total_residual(&self) -> f64 {
        self.total_water_m3() - self.ledger.expected_total()
    }
}

/// The thread count the plain entry points ([`World::step`], the fauna crate's
/// `Fauna::step`) split their read-only phases across: every core the OS reports, or one
/// if it reports nothing. Parallel is the default (Wrysk, 2026-09-18); `step_with(1)` is
/// the serial run, and `cubarium-voxel-sim`'s `SimConfig::threads` is the host's knob.
pub fn default_threads() -> usize {
    std::thread::available_parallelism().map_or(1, std::num::NonZero::get)
}

/// The world. Stepped at [`crate::TICK_HZ`]; pure given its inputs.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct World {
    pub(crate) config: Config,
    pub(crate) material: Vec<Material>,
    pub(crate) free: Vec<f64>,
    pub(crate) pore: Vec<f64>,
    pub(crate) aquifer_m3: f64,
    /// The lumped atmosphere: water aloft, with no position in the world. Filled by
    /// evaporation, transpiration and the outlet's export under a closed budget
    /// ([`Config::closed_water_budget`]) and emptied by showers; always zero under the
    /// open budget. Route C will give the water in here a place to be; route B only
    /// counts it.
    pub(crate) atmosphere_m3: f64,
    /// How much of the shower now falling is still to come, cubic metres. Zero between
    /// showers. **Not a store**: the water is still in `atmosphere_m3` and this is only
    /// the allowance this shower has left against it.
    pub(crate) shower_left_m3: f64,
    pub(crate) outlet_open: bool,
    pub(crate) tick: u64,
    /// Bumped by every material change a command actually commits. See
    /// [`VoxelView::terrain_version`].
    pub(crate) terrain_version: u64,
    pub(crate) ledger: Ledger,
    /// The one named outlet: when open it exports free water out of this cell.
    /// Generation picks the lowest void cell of the receiving basin.
    pub(crate) outlet_cell: Option<(u32, u32, u32)>,
    /// Where the aquifer discharges when its head rises above the cell. Generation
    /// picks a low void cell part way up the basin flank.
    pub(crate) spring_cell: Option<(u32, u32, u32)>,
    /// **A cache, not state:** the void cells that hold free water, and the porous cells
    /// that hold pore water — the active sets the water phases iterate instead of the
    /// grid (`design/7_Research/voxel-tick-profile-2026-09-18.md`). Maintained by the
    /// store primitives that write the arrays; **not serialized**, so a decoded world
    /// rebuilds them on its first step, and equality is set equality.
    #[serde(skip)]
    pub(crate) wet: crate::sparse::CellSet,
    #[serde(skip)]
    pub(crate) damp: crate::sparse::CellSet,
    /// **A cache, not state:** every column's void runs, derived from `material` alone.
    /// Rebuilt lazily when [`World::terrain_version`] moves and never otherwise. Not
    /// serialized, so a decoded world starts dirty. World-owned rather than thread-local:
    /// two worlds can share dimensions and a terrain version without sharing a floor plan
    /// (`design/handoffs/voxel-exchange-geometry-2026-09-18.md`).
    #[serde(skip)]
    pub(crate) void_runs: VoidRuns,
}

/// Every column's **void runs**: the maximal stacks of non-solid cells, one entry per run.
/// Geometry only — a function of `material` and nothing else — so it is rebuilt exactly
/// when [`World::terrain_version`] moves: a `SetMaterial` that commits a change, or a
/// generation carve. Water, and which of the run's cells is wet, is **not** in here; the
/// exchange recomputes heads, `room_target`, drives, offers and acceptance every substep.
///
/// **A cache, not state**: not serialized, so a decoded world comes back dirty and the
/// next exchange builds it; a clone copies it, which is valid because a clone shares the
/// terrain. Since two worlds can share dimensions and a terrain version, it lives on the
/// `World` and not in the thread-local `SCRATCH`.
#[derive(Clone, Debug)]
pub(crate) struct VoidRuns {
    /// Per column `c`: the first index into [`Self::runs`] that belongs to column `c`.
    /// `plane + 1` entries, so column `c`'s runs are `runs[offset[c]..offset[c + 1]]`.
    /// Empty when dirty.
    pub(crate) offset: Vec<u32>,
    /// Every run, grouped by column in ascending `(col, y0)` order.
    pub(crate) runs: Vec<VoidRun>,
    /// One bit per non-solid row, when the world is at most 64 cells tall. This is the
    /// same geometry as `runs`, in a form the water exchange can query without filling
    /// a dense per-cell displacement table every substep. Taller worlds use `runs`.
    pub(crate) mask: Vec<u64>,
    /// The `terrain_version` this cache was built from.
    version: u64,
    /// Set when the cache has never been built in this world, or after a decode.
    dirty: bool,
}

impl Default for VoidRuns {
    fn default() -> VoidRuns {
        VoidRuns {
            offset: Vec::new(),
            runs: Vec::new(),
            mask: Vec::new(),
            version: 0,
            dirty: true,
        }
    }
}

/// One maximal stack of non-solid cells in one column: rows `y0..=top` inclusive.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct VoidRun {
    pub(crate) y0: u32,
    pub(crate) top: u32,
}

/// Set equality, not layout equality: a **stale** cache carries no information, so it
/// equals anything — the same rule as [`crate::sparse::CellSet`], so a snapshot round trip
/// compares two worlds holding the same terrain whether or not either has built its cache.
impl PartialEq for VoidRuns {
    fn eq(&self, other: &VoidRuns) -> bool {
        self.dirty || other.dirty || self.runs == other.runs
    }
}

/// Ticks the convergence test looks back over.
pub const SETTLE_WINDOW: u32 = 100;

/// What [`World::settle`] measured.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Settle {
    /// Ticks actually stepped.
    pub ticks: u32,
    /// Whether the pooled volume and the wet-cell count both stopped moving.
    pub converged: bool,
    /// Free water standing at the end, cubic metres.
    pub pooled_m3: f64,
    /// Void cells holding it.
    pub free_cells: usize,
    /// Pore water at the end, cubic metres.
    pub pore_m3: f64,
    /// Change in total stored water over the last [`SETTLE_WINDOW`] ticks: storage drift,
    /// which should be nothing at all in a world with no rain and no evaporation.
    pub drift_m3_per_100: f64,
    /// Settled **and** uninhabitable: water aloft past the shower trigger and not one
    /// pool on the ground. A stable dry state is not a wet habitat (plan §4.4).
    pub dry_locked: bool,
}

impl World {
    /// Generate a world from its config. Deterministic in `config.seed`. Panics on a
    /// config [`Config::validate`] refuses.
    pub fn new(config: Config) -> World {
        config.validate().expect("World::new needs a valid Config");
        let n = config.cells();
        let mut world = World {
            material: vec![Material::Air; n],
            free: vec![0.0; n],
            pore: vec![0.0; n],
            aquifer_m3: 0.0,
            atmosphere_m3: 0.0,
            shower_left_m3: 0.0,
            outlet_open: false,
            tick: 0,
            terrain_version: 0,
            ledger: Ledger::default(),
            outlet_cell: None,
            spring_cell: None,
            wet: crate::sparse::CellSet::default(),
            damp: crate::sparse::CellSet::default(),
            void_runs: VoidRuns::default(),
            config,
        };
        crate::generate::landform(&mut world);
        world.aquifer_m3 = world
            .config
            .aquifer_volume_for_head(world.config.initial_aquifer_head_m);
        world.atmosphere_m3 = if world.config.closed_water_budget {
            world.config.initial_atmosphere_m3
        } else {
            0.0
        };
        // A staged recipe's water inventory, poured into the geometry it just built. A
        // `Ridge` world and any recipe with no inventory are left exactly as they were.
        //
        // `hydrate` books what it adds, because it is safe to call on a world that already
        // holds water. This world does not: it has just been generated, so the inventory
        // **is** what it began with, and the booking is dropped in favour of recording it
        // as the initial stores below.
        if let Landform::Staged(recipe) = world.config.landform.clone() {
            crate::hydrate::hydrate(&mut world, &recipe.water);
            world.ledger = Ledger::default();
        }
        world.ledger.initial_stored = world.view().stored_m3();
        world.ledger.initial_atmosphere = world.atmosphere_m3;
        // The active sets are built here rather than lazily so that a world is never in a
        // state where its cache disagrees with its arrays.
        world.rebuild_active_sets();
        world
    }

    /// An all-air world over one bedrock floor row: the fixture builder for tests and
    /// hand-authored scenes. Charged to [`Config::initial_aquifer_head_m`] like any
    /// other world. Panics on a config [`Config::validate`] refuses.
    pub fn empty(config: Config) -> World {
        config
            .validate()
            .expect("World::empty needs a valid Config");
        let n = config.cells();
        let mut material = vec![Material::Air; n];
        for x in 0..config.width as i64 {
            for z in 0..config.depth {
                material[config.index(x, 0, z)] = Material::Bedrock;
            }
        }
        let mut world = World {
            free: vec![0.0; n],
            pore: vec![0.0; n],
            aquifer_m3: 0.0,
            atmosphere_m3: 0.0,
            shower_left_m3: 0.0,
            outlet_open: false,
            tick: 0,
            terrain_version: 0,
            ledger: Ledger::default(),
            outlet_cell: None,
            spring_cell: None,
            wet: crate::sparse::CellSet::default(),
            damp: crate::sparse::CellSet::default(),
            void_runs: VoidRuns::default(),
            material,
            config,
        };
        world.aquifer_m3 = world
            .config
            .aquifer_volume_for_head(world.config.initial_aquifer_head_m);
        world.atmosphere_m3 = if world.config.closed_water_budget {
            world.config.initial_atmosphere_m3
        } else {
            0.0
        };
        world.ledger.initial_stored = world.view().stored_m3();
        world.ledger.initial_atmosphere = world.atmosphere_m3;
        world.rebuild_active_sets();
        world
    }

    pub fn config(&self) -> &Config {
        &self.config
    }

    pub fn tick(&self) -> u64 {
        self.tick
    }

    /// How many material changes this world has committed. See
    /// [`VoxelView::terrain_version`].
    pub fn terrain_version(&self) -> u64 {
        self.terrain_version
    }

    /// The named outlet cell, if the world has one.
    pub fn outlet_cell(&self) -> Option<(u32, u32, u32)> {
        self.outlet_cell
    }

    /// Name a different outlet cell, or `None` for no outlet.
    pub fn set_outlet_cell(&mut self, cell: Option<(u32, u32, u32)>) {
        self.outlet_cell = cell;
    }

    /// Whether the outlet is currently exporting.
    pub fn outlet_open(&self) -> bool {
        self.outlet_open
    }

    /// The cell the aquifer discharges into, if the world has one.
    pub fn spring_cell(&self) -> Option<(u32, u32, u32)> {
        self.spring_cell
    }

    /// Name a different spring cell, or `None` for no spring.
    pub fn set_spring_cell(&mut self, cell: Option<(u32, u32, u32)>) {
        self.spring_cell = cell;
    }

    /// Aquifer head in metres above `y = 0`.
    /// The lumped atmosphere store in cubic metres.
    pub fn atmosphere_m3(&self) -> f64 {
        self.atmosphere_m3
    }

    /// Cubic metres left to fall in the shower now running; zero between showers.
    pub fn shower_left_m3(&self) -> f64 {
        self.shower_left_m3
    }

    pub fn aquifer_head_m(&self) -> f64 {
        self.config.aquifer_head_m(self.aquifer_m3)
    }

    pub fn view(&self) -> VoxelView<'_> {
        VoxelView {
            config: &self.config,
            material: &self.material,
            free: &self.free,
            pore: &self.pore,
            tick: self.tick,
            terrain_version: self.terrain_version,
            ledger: &self.ledger,
            aquifer_m3: self.aquifer_m3,
            atmosphere_m3: self.atmosphere_m3,
            shower_left_m3: self.shower_left_m3,
            outlet_open: self.outlet_open,
            outlet: self.outlet_cell,
            spring: self.spring_cell,
        }
    }

    /// Advance one tick: prescribed rain and evaporation, free-water substeps,
    /// infiltration, drainage, spring discharge, outlet export. The exchange's column
    /// scan splits across [`default_threads`] workers; [`World::step_with`] takes the
    /// count explicitly.
    pub fn step(&mut self) {
        self.step_with(default_threads());
    }

    /// [`World::step`] with a thread count for the one phase that splits, the exchange's
    /// read-only column scan. `1` runs it on this thread. Execution only: the count can
    /// reach no result (`water::exchange`'s doc), and with the `parallel` feature off it
    /// is ignored.
    pub fn step_with(&mut self, threads: usize) {
        crate::water::step(self, threads);
        self.advance_tick();
    }

    /// Step until the water has stopped moving, or until `cap_ticks`, and say which.
    ///
    /// **Settling is measured, not assumed** (`design/terrain-generation-plan-2026-09-21.md`
    /// §4.4). A fresh world's pools are placed by geometry and its pore water by a
    /// retention rule; the solver then redistributes both, and how long that takes is a
    /// property of the landscape, not a constant anybody can write down. Convergence is
    /// the pooled volume and the wet-cell count each changing by under one per cent
    /// across the last [`SETTLE_WINDOW`] ticks.
    ///
    /// Nothing here is a claim about the water **cycle**: a world can be hydrostatically
    /// settled and still locked dry, which is what [`Settle::dry_locked`] reports.
    pub fn settle(&mut self, cap_ticks: u32) -> Settle {
        let window = SETTLE_WINDOW as usize;
        let mut history: Vec<(f64, usize)> = Vec::with_capacity(window + 1);
        let mut stored: Vec<f64> = Vec::with_capacity(window + 1);
        let mut ticks = 0;
        let mut converged = false;
        loop {
            let pooled = self.pooled_m3();
            let cells = self.wet_cells();
            history.push((pooled, cells));
            stored.push(self.view().stored_m3());
            if history.len() > window {
                let (p0, c0) = history[history.len() - 1 - window];
                let near = |a: f64, b: f64| (a - b).abs() <= 0.01 * a.abs().max(b.abs()).max(1e-12);
                if near(p0, pooled) && near(c0 as f64, cells as f64) {
                    converged = true;
                }
            }
            if converged || ticks >= cap_ticks {
                break;
            }
            self.step();
            ticks += 1;
        }
        let pooled_m3 = history.last().map(|h| h.0).unwrap_or(0.0);
        let drift = if stored.len() > window {
            stored[stored.len() - 1] - stored[stored.len() - 1 - window]
        } else {
            stored.last().copied().unwrap_or(0.0) - stored.first().copied().unwrap_or(0.0)
        };
        Settle {
            ticks,
            converged,
            pooled_m3,
            free_cells: history.last().map(|h| h.1).unwrap_or(0),
            pore_m3: self.pore_m3(),
            drift_m3_per_100: drift,
            dry_locked: pooled_m3 <= 0.0
                && self.atmosphere_m3
                    > self.config.shower_trigger_fraction * self.view().total_water_m3(),
        }
    }

    /// Free water standing in the world, cubic metres.
    pub fn pooled_m3(&self) -> f64 {
        self.free.iter().sum::<f64>() * self.config.voxel_volume()
    }

    /// Pore water held in soil and rock, cubic metres.
    pub fn pore_m3(&self) -> f64 {
        (0..self.config.cells())
            .map(|i| self.pore[i] * self.material[i].pore_capacity())
            .sum::<f64>()
            * self.config.voxel_volume()
    }

    /// Void cells holding free water.
    pub fn wet_cells(&self) -> usize {
        self.free.iter().filter(|&&f| f > 0.0).count()
    }

    /// Move the tick counter on, after every phase that reads it has run.
    ///
    /// Split out of [`World::step`] for `cubarium-voxel-sim`, whose schedule runs the
    /// water phases one at a time and needs somewhere to put this. Never a rule: it is
    /// the clock, and it moves exactly once per tick either way.
    pub fn advance_tick(&mut self) {
        self.tick += 1;
    }

    /// Apply a command now — immediately, including while a frontend has the world
    /// paused; nothing is queued. Returns the signed volume in cubic metres actually
    /// accepted, zero for a non-water command or a refused amount. See
    /// [`crate::water::apply`] for the whole receipt contract.
    pub fn apply(&mut self, command: Command) -> f64 {
        crate::water::apply(self, command)
    }

    /// Rebuild the water active sets from the arrays, which is the only thing that can
    /// re-establish them after a snapshot, generation, or any other direct write. One
    /// full scan; the step calls it when a set says it is stale.
    pub(crate) fn rebuild_active_sets(&mut self) {
        let n = self.config.cells();
        self.wet.reset(n);
        self.damp.reset(n);
        for i in 0..n {
            if !self.material[i].is_solid() && self.free[i] > 0.0 {
                self.wet.insert(i);
            }
            if self.material[i].pore_capacity() > 0.0 && self.pore[i] > 0.0 {
                self.damp.insert(i);
            }
        }
    }

    /// Build the void-run geometry cache if the terrain has moved since it was built, or if
    /// this world has never built it — a fresh world, or one decoded from a snapshot. A
    /// clone keeps a valid cache (a clone shares the terrain, so it shares the geometry).
    ///
    /// One full scan of `material`, and the only writer of the cache. Called by the
    /// exchange before any worker reads geometry; the scan is inside the exchange timing,
    /// not hidden outside the tick.
    pub(crate) fn ensure_void_runs(&mut self) {
        if !self.void_runs.dirty && self.void_runs.version == self.terrain_version {
            return;
        }
        let plane = self.config.width as usize * self.config.depth as usize;
        let height = self.config.height as usize;
        let VoidRuns {
            offset, runs, mask, ..
        } = &mut self.void_runs;
        runs.clear();
        offset.clear();
        mask.clear();
        offset.reserve(plane + 1);
        if height <= 64 {
            mask.reserve(plane);
        }
        for col in 0..plane {
            offset.push(runs.len() as u32);
            let mut col_mask = 0u64;
            let mut y = 0usize;
            while y < height {
                if self.material[y * plane + col].is_solid() {
                    y += 1;
                    continue;
                }
                let mut top = y;
                while top + 1 < height && !self.material[(top + 1) * plane + col].is_solid() {
                    top += 1;
                }
                runs.push(VoidRun {
                    y0: y as u32,
                    top: top as u32,
                });
                if height <= 64 {
                    let width = top - y + 1;
                    let bits = if width == 64 {
                        u64::MAX
                    } else {
                        ((1u64 << width) - 1) << y
                    };
                    col_mask |= bits;
                }
                y = top + 1;
            }
            if height <= 64 {
                mask.push(col_mask);
            }
        }
        offset.push(runs.len() as u32);
        let version = self.terrain_version;
        self.void_runs.version = version;
        self.void_runs.dirty = false;
    }

    /// Serialize the whole world. Refuses nothing; `load` refuses other schemas.
    pub fn save(&self) -> Vec<u8> {
        crate::snapshot::encode(self)
    }

    /// Read a world back. Refuses another schema tag, corrupt bytes, and anything
    /// [`World::validate_loaded`] finds wrong with the world itself.
    pub fn load(bytes: &[u8]) -> anyhow::Result<World> {
        crate::snapshot::decode(bytes)
    }

    /// What a loaded world has to satisfy before it may replace a running one: a valid
    /// config, one entry per cell in each array, every fraction a real number in
    /// `0..=1` **and one its own material can actually hold** — no free water in a
    /// solid, no pore water where [`Material::pore_capacity`] is zero — a finite
    /// non-negative aquifer and finite ledger terms, and outlet and spring cells inside
    /// the world.
    ///
    /// The material check is not decoration: `VoxelView` hands both fractions straight
    /// out, while the store accounting reads free water only in voids and pore water
    /// only against the material's own capacity. A snapshot with free water in bedrock
    /// would draw and be queried as wet and weigh nothing at all.
    pub(crate) fn validate_loaded(&self) -> anyhow::Result<()> {
        self.config.validate()?;
        let n = self.config.cells();
        for (name, len) in [
            ("material", self.material.len()),
            ("free", self.free.len()),
            ("pore", self.pore.len()),
        ] {
            ensure!(len == n, "{name} has {len} entries, not one per cell ({n})");
        }
        for (name, store) in [("free", &self.free), ("pore", &self.pore)] {
            if let Some((i, bad)) = store
                .iter()
                .copied()
                .enumerate()
                .find(|&(_, f)| !(0.0..=1.0).contains(&f))
            {
                bail!("{name}[{i}] is {bad}, not a fraction in 0..=1");
            }
        }
        for (i, &m) in self.material.iter().enumerate() {
            ensure!(
                !m.is_solid() || self.free[i] == 0.0,
                "free[{i}] is {}, but {m:?} holds no free water",
                self.free[i]
            );
            ensure!(
                m.pore_capacity() > 0.0 || self.pore[i] == 0.0,
                "pore[{i}] is {}, but {m:?} has no pore space",
                self.pore[i]
            );
        }
        ensure!(
            self.aquifer_m3.is_finite() && self.aquifer_m3 >= 0.0,
            "the aquifer store is {}, not a volume",
            self.aquifer_m3
        );
        for (name, store) in [
            ("atmosphere", self.atmosphere_m3),
            ("shower allowance", self.shower_left_m3),
        ] {
            ensure!(
                store.is_finite() && store >= 0.0,
                "the {name} store is {store}, not a volume"
            );
        }
        ensure!(
            self.config.closed_water_budget || self.atmosphere_m3 == 0.0,
            "an open-budget world holds no atmosphere, but this one holds {}",
            self.atmosphere_m3
        );
        ensure!(
            self.ledger.initial_stored.is_finite() && self.ledger.initial_stored >= 0.0,
            "initial_stored is {}, not a volume",
            self.ledger.initial_stored
        );
        for (name, flux) in [
            ("rain_in", self.ledger.rain_in),
            ("user_in", self.ledger.user_in),
            ("evaporation_out", self.ledger.evaporation_out),
            ("outlet_out", self.ledger.outlet_out),
            ("transpiration_out", self.ledger.transpiration_out),
            ("displaced_out", self.ledger.displaced_out),
            ("atmosphere_in", self.ledger.atmosphere_in),
            ("atmosphere_out", self.ledger.atmosphere_out),
            ("user_atmosphere_in", self.ledger.user_atmosphere_in),
            ("initial_atmosphere", self.ledger.initial_atmosphere),
        ] {
            ensure!(flux.is_finite(), "the ledger's {name} is {flux}");
        }
        for (name, cell) in [("outlet", self.outlet_cell), ("spring", self.spring_cell)] {
            if let Some((x, y, z)) = cell {
                ensure!(
                    x < self.config.width && y < self.config.height && z < self.config.depth,
                    "the {name} cell ({x}, {y}, {z}) is outside a {}x{}x{} world",
                    self.config.width,
                    self.config.height,
                    self.config.depth
                );
            }
        }
        Ok(())
    }
}
