use anyhow::{bail, ensure};
use serde::{Deserialize, Serialize};

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

/// The fill a cell must hold for the water above it to count as **standing** on it:
/// [`VoxelView::standing_depth_m`]'s one threshold. Half a cell: a pool's cells sit
/// 0.87–0.96 full, a shower's falling column a third or less. A placeholder
/// (`design/backlog.md` §1).
pub const STANDING_SUPPORT_FILL: f64 = 0.5;

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
    /// The weather and the time of day, for drawing: the pinned seam
    /// ([`crate::weather`]).
    pub weather: crate::weather::WeatherView,
    /// Every column's solid cells as bits, the world's [`SolidColumns`]: what
    /// [`VoxelView::is_support`], [`VoxelView::is_solid`] and the column queries read.
    /// Empty while a world is being generated and on a world taller than
    /// [`SOLID_MASK_ROWS`], and every reader then takes the direct test over `material`.
    solid: &'a [u128],
}

impl<'a> VoxelView<'a> {
    /// A view of terrain alone, for a reader that holds a copy of `material` off the world
    /// (the renderer's sky plane): no water, no stores, and no solid mask, so every support
    /// and column query takes the direct test over `material`.
    pub fn of_terrain(config: &'a Config, material: &'a [Material], ledger: &'a Ledger) -> Self {
        VoxelView {
            config,
            material,
            free: &[],
            pore: &[],
            tick: 0,
            terrain_version: 0,
            ledger,
            aquifer_m3: 0.0,
            atmosphere_m3: 0.0,
            shower_left_m3: 0.0,
            outlet_open: false,
            outlet: None,
            spring: None,
            weather: crate::weather::WeatherView::CLEAR_NOON,
            solid: &[],
        }
    }
}

/// The tallest world whose columns [`SolidColumns`] keeps as one bit mask each. Taller
/// worlds answer the support queries from `material` directly.
pub const SOLID_MASK_ROWS: u32 = 128;

/// `x` wrapped into `0..width` without a division when it is already there, which it
/// nearly always is: `Config::index`'s wrap, for the per-column lookups.
#[inline]
fn wrap_x(x: i64, width: u32) -> usize {
    let w = i64::from(width);
    if (0..w).contains(&x) {
        x as usize
    } else {
        x.rem_euclid(w) as usize
    }
}

/// The support faces of one column's solid mask: a solid row with a non-solid row over
/// it, below the top row of a `height`-row world.
#[inline]
fn support_bits(solid: u128, height: u32) -> u128 {
    let below_top = match height {
        0 | 1 => 0,
        h if h - 1 >= 128 => u128::MAX,
        h => (1u128 << (h - 1)) - 1,
    };
    solid & !(solid >> 1) & below_top
}

/// Rows `lo..=hi` of a column mask; `lo <= hi < 128`.
#[inline]
fn row_range(lo: u32, hi: u32) -> u128 {
    (u128::MAX >> (127 - hi)) & (u128::MAX << lo)
}

impl<'a> VoxelView<'a> {
    /// True if rain is **falling** this tick.
    ///
    /// Under the closed cycle that is a shower and nothing else:
    /// [`Config::rain_m_per_s`] is the rate a shower falls *at*, not a statement that one
    /// is falling, and it is always positive on a cycling world. Reading it as weather
    /// drew rain streaks on the deployed panel every tick of a world that actually rained
    /// one minute in ten (Wrysk, 2026-09-21). Under the open budget the rain really is
    /// prescribed every tick, so there the rate is the answer.
    #[inline]
    pub fn is_raining(&self) -> bool {
        if self.config.closed_water_budget {
            self.shower_left_m3 > 0.0
        } else {
            self.config.rain_m_per_s > 0.0
        }
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
    ///
    /// Read from the world's per-column solid bits ([`SolidColumns`]), one load and a
    /// shift, and the same answer as [`VoxelView::is_support_direct`] in every cell.
    #[inline]
    pub fn is_support(&self, x: i64, y: u32, z: u32) -> bool {
        let c = self.config;
        if !(y + 1 < c.height && z < c.depth) {
            return false;
        }
        match self.solid_column(x, z) {
            Some(m) => {
                let support = (m >> y) & 0b11 == 0b01;
                debug_assert_eq!(support, self.is_support_direct(x, y, z), "({x}, {y}, {z})");
                support
            }
            None => self.is_support_direct(x, y, z),
        }
    }

    /// [`VoxelView::is_support`] read off `material` itself: the definition the lookup is
    /// checked against, and what a world without the lookup answers with.
    pub fn is_support_direct(&self, x: i64, y: u32, z: u32) -> bool {
        let c = self.config;
        y + 1 < c.height
            && z < c.depth
            && self.material_at(x, y, z).is_solid()
            && !self.material_at(x, y + 1, z).is_solid()
    }

    /// Whether `(x, y, z)` is solid: `material_at(x, y, z).is_solid()` read from the
    /// column bits. `y` and `z` must be in range, as for [`VoxelView::material_at`].
    #[inline]
    pub fn is_solid(&self, x: i64, y: u32, z: u32) -> bool {
        debug_assert!(y < self.config.height && z < self.config.depth);
        match self.solid_column(x, z) {
            Some(m) => (m >> y) & 1 == 1,
            None => self.material_at(x, y, z).is_solid(),
        }
    }

    /// Whether column `(x, z)` holds a solid anywhere in rows `lo..=hi` (`hi` may run past
    /// the world): `(lo..=hi).any(|y| self.is_solid(x, y, z))` in one mask operation.
    #[inline]
    pub fn solid_in(&self, x: i64, z: u32, lo: u32, hi: u32) -> bool {
        let c = self.config;
        if z >= c.depth || c.height == 0 {
            return false;
        }
        let hi = hi.min(c.height - 1);
        if lo > hi {
            return false;
        }
        match self.solid_column(x, z) {
            Some(m) => m & row_range(lo, hi) != 0,
            None => (lo..=hi).any(|y| self.material_at(x, y, z).is_solid()),
        }
    }

    /// Column `(x, z)`'s solid bits, when the world keeps them. `z` must be in range.
    #[inline]
    fn solid_column(&self, x: i64, z: u32) -> Option<u128> {
        if self.solid.is_empty() {
            return None;
        }
        let c = self.config;
        self.solid
            .get(z as usize * c.width as usize + wrap_x(x, c.width))
            .copied()
    }

    /// The lowest support face of column `(x, z)` in rows `lo..=hi` (`hi` may run past
    /// the world), or `None`: `(lo..=hi).find(|&y| self.is_support(x, y, z))` in one
    /// mask operation.
    #[inline]
    pub fn lowest_support_in(&self, x: i64, z: u32, lo: u32, hi: u32) -> Option<u32> {
        let c = self.config;
        if z >= c.depth || c.height < 2 {
            return None;
        }
        let hi = hi.min(c.height - 2);
        if lo > hi {
            return None;
        }
        let found = match self.solid_column(x, z) {
            Some(m) => {
                let s = support_bits(m, c.height) & row_range(lo, hi);
                (s != 0).then(|| s.trailing_zeros())
            }
            None => (lo..=hi).find(|&y| self.is_support_direct(x, y, z)),
        };
        debug_assert_eq!(found, (lo..=hi).find(|&y| self.is_support_direct(x, y, z)));
        found
    }

    /// The highest support face of column `(x, z)` in rows `lo..=hi`, or `None`:
    /// `(lo..=hi).rev().find(|&y| self.is_support(x, y, z))` in one mask operation.
    #[inline]
    pub fn highest_support_in(&self, x: i64, z: u32, lo: u32, hi: u32) -> Option<u32> {
        let c = self.config;
        if z >= c.depth || c.height < 2 {
            return None;
        }
        let hi = hi.min(c.height - 2);
        if lo > hi {
            return None;
        }
        let found = match self.solid_column(x, z) {
            Some(m) => {
                let s = support_bits(m, c.height) & row_range(lo, hi);
                (s != 0).then(|| 127 - s.leading_zeros())
            }
            None => (lo..=hi).rev().find(|&y| self.is_support_direct(x, y, z)),
        };
        debug_assert_eq!(found, (lo..=hi).rev().find(|&y| self.is_support_direct(x, y, z)));
        found
    }

    /// The support face of column `(x, z)` nearest the row `standing`, at most `down`
    /// rows below it or `up` above (clipped to the world), **ties to the higher**; `None`
    /// when there is none. The step rule's own question
    /// (`cubarium_voxel_fauna`'s `step_target_layer`): the nearest face at or above and
    /// the nearest below, one mask operation each.
    #[inline]
    pub fn nearest_support(
        &self,
        x: i64,
        z: u32,
        standing: u32,
        down: u32,
        up: u32,
    ) -> Option<u32> {
        let c = self.config;
        if z >= c.depth {
            return None;
        }
        let lo = standing.saturating_sub(down);
        let hi = standing.saturating_add(up).min(c.height.saturating_sub(1));
        if lo > hi {
            return None;
        }
        if let Some(m) = self.solid_column(x, z) {
            // One mask: the column's faces in range, split at `standing`.
            let top = hi.min(c.height.saturating_sub(2));
            if c.height < 2 || lo > top {
                return None;
            }
            let s = support_bits(m, c.height) & row_range(lo, top);
            let at_or_above = s & (u128::MAX << standing.min(127));
            let at_or_above = if standing > 127 { 0 } else { at_or_above };
            let below = s & !at_or_above;
            let found = match (at_or_above != 0, below != 0) {
                (false, false) => None,
                (true, false) => Some(at_or_above.trailing_zeros()),
                (false, true) => Some(127 - below.leading_zeros()),
                (true, true) => {
                    let (a, b) = (at_or_above.trailing_zeros(), 127 - below.leading_zeros());
                    Some(if a - standing <= standing - b { a } else { b })
                }
            };
            debug_assert_eq!(found, self.nearest_support_direct(x, z, standing, down, up));
            return found;
        }
        self.nearest_support_direct(x, z, standing, down, up)
    }

    /// [`VoxelView::nearest_support`] by the column queries (each the walk it stands
    /// for when the world keeps no mask).
    fn nearest_support_direct(
        &self,
        x: i64,
        z: u32,
        standing: u32,
        down: u32,
        up: u32,
    ) -> Option<u32> {
        let c = self.config;
        let lo = standing.saturating_sub(down);
        let hi = standing.saturating_add(up).min(c.height.saturating_sub(1));
        if z >= c.depth || lo > hi {
            return None;
        }
        let above = if standing <= hi {
            self.lowest_support_in(x, z, lo.max(standing), hi)
        } else {
            None
        };
        let below = if standing > lo {
            self.highest_support_in(x, z, lo, (standing - 1).min(hi))
        } else {
            None
        };
        match (above, below) {
            (Some(a), Some(b)) => Some(if a - standing <= standing - b { a } else { b }),
            (a, b) => a.or(b),
        }
    }

    /// Every support face in column `(x, z)`, ascending in `y`.
    pub fn supports_in_column(&self, x: i64, z: u32) -> Vec<u32> {
        let c = self.config;
        if z >= c.depth {
            return Vec::new();
        }
        match self.solid_column(x, z) {
            Some(m) => {
                let mut s = support_bits(m, c.height);
                let mut out = Vec::with_capacity(s.count_ones() as usize);
                while s != 0 {
                    out.push(s.trailing_zeros());
                    s &= s - 1;
                }
                out
            }
            None => (0..c.height)
                .filter(|&y| self.is_support_direct(x, y, z))
                .collect(),
        }
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

    /// Depth of the **settled standing water** on the support face `(x, y, z)`, metres:
    /// water held up from below, and not water in transit
    /// (`design/handoffs/voxel-terrain-note-standing-depth-2026-09-22.md`).
    ///
    /// The same walk as [`VoxelView::water_depth_m`], with one more stop: a cell's water
    /// counts only while the cell **under** it is the face itself or at least
    /// [`STANDING_SUPPORT_FILL`] full. A pool's cells sit 0.87–0.96 full with water on
    /// them and read whole; a shower's falling column (fills 0.11 / 0.25 / 0.37 … over a
    /// film) reads as its bottom cell and nothing above it, because a third-full cell is
    /// not holding up the water over it.
    ///
    /// A **reader over the fills, not solver state.** The solver has no per-cell
    /// "at rest" flag, so moving water that is deeper than half a cell — a full stream
    /// channel — reads as standing here, and the partial top cell of a pool sitting on a
    /// half-full cell counts. What it does refuse is the case the note measured: a falling
    /// column read as depth over a film.
    pub fn standing_depth_m(&self, x: i64, y: u32, z: u32) -> f64 {
        let c = self.config;
        if z >= c.depth {
            return 0.0;
        }
        let mut depth = 0.0;
        let mut below = 1.0;
        for yy in y + 1..c.height {
            if self.material_at(x, yy, z).is_solid() {
                break;
            }
            let f = self.free_at(x, yy, z);
            if !(f > 0.0) || below < STANDING_SUPPORT_FILL {
                break;
            }
            depth += f;
            below = f;
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

/// The default worker count for `available` reported cores: all but one, and never
/// fewer than one.
///
/// Pure so the policy is testable without owning the machine's core count.
fn threads_for(available: usize) -> usize {
    available.saturating_sub(1).max(1)
}

/// The thread count the plain entry points ([`World::step`], the fauna crate's
/// `Fauna::step`) split their read-only phases across: **every core the OS reports but
/// one**, or one if it reports nothing.
///
/// Parallel is the default (Wrysk, 2026-09-18) and the board runs nothing else, so the
/// tick may have all of it bar a single core (Wrysk, 2026-09-21). That one is not spare
/// capacity: the frame the tick just produced still has to be recorded, submitted and
/// page-flipped, and a tick that owns every core starves the thread doing it.
/// `step_with(1)` is the serial run, and `cubarium-voxel-sim`'s `SimConfig::threads` is
/// the host's knob.
pub fn default_threads() -> usize {
    thread_override().unwrap_or_else(|| {
        threads_for(std::thread::available_parallelism().map_or(1, std::num::NonZero::get))
    })
}

/// `dst.copy_from_slice(src)`, cut into contiguous pieces across the process's water pool
/// of `threads` workers when there are enough bytes to pay for the hand-off (the
/// `parallel` feature); else on the calling thread. [`World::restore_water_from`]'s copy.
pub(crate) fn copy_dense<T: Copy + Send + Sync>(dst: &mut [T], src: &[T], threads: usize) {
    #[cfg(feature = "parallel")]
    if threads > 1 && std::mem::size_of_val(src) >= 1 << 20 {
        use rayon::prelude::*;
        let pool = cubarium_rules::water::host::pool(threads);
        let piece = src.len().div_ceil(threads * 2).max(1);
        pool.install(|| {
            dst.par_chunks_mut(piece)
                .zip(src.par_chunks(piece))
                .for_each(|(d, s)| d.copy_from_slice(s));
        });
        return;
    }
    let _ = threads;
    dst.copy_from_slice(src);
}

/// Set by [`set_thread_override`]; `0` is unset.
static THREAD_OVERRIDE: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

/// Pin the process's default thread count: [`default_threads`], and so [`World::step`],
/// `Fauna::step` and `cubarium-voxel-sim`'s `SimConfig::default()`, all return `threads`
/// from now on. `0` clears it. For a tool run many times side by side (the gate's
/// autopsies and censuses), so each run takes its share of the machine, not all of it
/// (`threads=` on those examples). Execution only, like every thread count here.
pub fn set_thread_override(threads: usize) {
    THREAD_OVERRIDE.store(threads, std::sync::atomic::Ordering::Relaxed);
}

/// The pinned thread count, if any: [`set_thread_override`]'s, else a positive
/// `CUBARIUM_THREADS` from the environment (read once), else `None`.
pub fn thread_override() -> Option<usize> {
    static ENV: std::sync::OnceLock<Option<usize>> = std::sync::OnceLock::new();
    match THREAD_OVERRIDE.load(std::sync::atomic::Ordering::Relaxed) {
        0 => *ENV.get_or_init(|| {
            std::env::var("CUBARIUM_THREADS")
                .ok()
                .and_then(|s| s.parse().ok())
                .filter(|&n: &usize| n > 0)
        }),
        n => Some(n),
    }
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
    /// The tick the next shower is **due**, when
    /// [`Config::shower_interval_max_s`] asks for a schedule. Drawn from the world's own
    /// seed at creation and again every time a shower ends, so the weather is a property
    /// of the world and not of the clock. Zero, and ignored, with no schedule.
    pub(crate) next_shower_tick: u64,
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
    /// A lake's floor drain, joining it to the aquifer (see
    /// [`Config::lake_drain_m2_per_s`]): the void cells on the lake's floor, the
    /// conductance shared among them. Empty is no drain.
    #[serde(default)]
    pub(crate) lake_drain: Vec<(u32, u32, u32)>,
    /// The row a generator asks its lake filled to — the lake holds the rows below it —
    /// when no outlet marks it. Read once, by [`crate::hydrate::hydrate`].
    #[serde(default)]
    pub(crate) lake_datum_y: Option<u32>,
    /// **A cache, not state:** the void cells that hold free water, and the porous cells
    /// that hold pore water — the active sets the water phases iterate instead of the
    /// grid (`design/7_Research/voxel-tick-profile-2026-09-18.md`). Maintained by the
    /// store primitives that write the arrays; **not serialized**, so a decoded world
    /// rebuilds them on its first step, and equality is set equality.
    #[serde(skip)]
    pub(crate) wet: crate::sparse::CellSet,
    #[serde(skip)]
    pub(crate) damp: crate::sparse::CellSet,
    /// **A cache, not state:** the porous cells whose pore water is over their material's
    /// field capacity — the cells `drain` can move water out of (package D), a subset of
    /// `damp`. Maintained by the same pore primitives and rebuilt with the other two.
    #[serde(skip)]
    pub(crate) drainable: crate::sparse::CellSet,
    /// **A cache, not state:** every column's void runs, derived from `material` alone.
    /// Rebuilt lazily when [`World::terrain_version`] moves and never otherwise. Not
    /// serialized, so a decoded world starts dirty. World-owned rather than thread-local:
    /// two worlds can share dimensions and a terrain version without sharing a floor plan
    /// (`design/handoffs/voxel-exchange-geometry-2026-09-18.md`).
    #[serde(skip)]
    pub(crate) void_runs: VoidRuns,
    /// **A cache, not state:** every column's solid cells as bits, what the support-face
    /// lookup reads ([`SolidColumns`]). Not serialized: a decoded world rebuilds it with
    /// its active sets.
    #[serde(skip)]
    pub(crate) solid: SolidColumns,
}

/// Every column's **solid cells**, one bit per row (bit `y` of entry `z * width + x`):
/// the terrain half of the support-face test, so [`VoxelView::is_support`] is one load
/// and a shift and the step rule's nearest face one mask operation
/// (`design/handoffs/voxel-hot-path-geometry-2026-09-24.md`, item 3).
///
/// Terrain alone: water and plants never enter the test, so nothing but a material
/// change moves it. **A cache, not state**, derived from `material` and kept current by
/// every path that writes `material` once a world is built — `SetMaterial` sets its one
/// bit, `repair_isolated` its pockets', and [`World::rebuild_active_sets`] (the end of
/// construction, a snapshot's decode, a fixture's direct write) rebuilds it whole. A
/// clone copies it, which is right: a clone shares the terrain. Empty — and every reader
/// then takes the direct test — before a world's first rebuild (mid-generation) and on a
/// world taller than [`SOLID_MASK_ROWS`].
#[derive(Clone, Debug, Default)]
pub(crate) struct SolidColumns {
    pub(crate) bits: Vec<u128>,
}

/// A cache compares equal to anything: `material`, which `World`'s equality compares,
/// is what it is derived from.
impl PartialEq for SolidColumns {
    fn eq(&self, _: &SolidColumns) -> bool {
        true
    }
}

impl SolidColumns {
    /// Rebuild from `material` whole.
    fn rebuild(&mut self, config: &Config, material: &[Material]) {
        self.bits.clear();
        if config.height > SOLID_MASK_ROWS {
            return;
        }
        let plane = config.width as usize * config.depth as usize;
        self.bits.resize(plane, 0);
        for (y, row) in material.chunks_exact(plane).enumerate() {
            for (col, m) in row.iter().enumerate() {
                if m.is_solid() {
                    self.bits[col] |= 1u128 << y;
                }
            }
        }
    }

    /// Record one cell's new material: `i` is its `Config::index`. Nothing to do while
    /// the cache is not built.
    pub(crate) fn set(&mut self, config: &Config, i: usize, material: Material) {
        if self.bits.is_empty() {
            return;
        }
        let plane = config.width as usize * config.depth as usize;
        let (col, y) = (i % plane, i / plane);
        let bit = 1u128 << y;
        if material.is_solid() {
            self.bits[col] |= bit;
        } else {
            self.bits[col] &= !bit;
        }
    }
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
    /// One bit per non-solid row, when the world is at most [`MASK_ROWS`] cells tall.
    /// This is the same geometry as `runs`, in a form the water exchange can query
    /// without filling a dense per-cell displacement table every substep. Taller worlds
    /// use `runs`.
    pub(crate) mask: Vec<u128>,
    /// Per column: the lowest row of the run that is open to the sky — the run whose top
    /// is the world's top row — or `height` when the top cell itself is solid and the
    /// sky meets nothing it can wet. Rain and evaporation read it instead of walking every
    /// column down from the ceiling each tick; like the rest of this cache it moves only
    /// with the terrain.
    pub(crate) sky_floor: Vec<u32>,
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
            sky_floor: Vec::new(),
            version: 0,
            dirty: true,
        }
    }
}

/// The tallest world whose columns fit one [`VoidRuns::mask`] word: the water exchange's
/// bitmask path covers heights up to this, and the dense scan is the fallback above it.
/// The panel's ring is 72 rows, so a 64-bit word was not enough.
pub(crate) const MASK_ROWS: usize = cubarium_rules::water::MASK_ROWS;

/// `len` set bits starting at row `y`: one run of rows in a column mask.
pub(crate) use cubarium_rules::water::run_bits;

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

/// Ticks of no movement at all that count as rest. A world nothing is happening in should
/// not have to be stepped through the whole convergence window to say so.
pub const SETTLE_REST: u32 = 20;

/// Nothing has moved, at all, for the last [`SETTLE_REST`] samples.
fn at_rest(history: &[(f64, usize)]) -> bool {
    let n = SETTLE_REST as usize;
    if history.len() <= n {
        return false;
    }
    let last = history[history.len() - 1];
    history[history.len() - 1 - n..].iter().all(|&h| h == last)
}

/// Both readings have moved by less than their tolerance over the last `window` samples.
///
/// The pooled volume's is one per cent of itself. The wet-cell count's is one per cent of
/// **the pooled cell count**, with a floor of eight cells: a pool whose edge gains and
/// loses a film of a few cells as a shower falls on it is a settled pool, and on a world
/// with no water at all the floor is what stops an empty comparison deciding anything.
fn settled_over(history: &[(f64, usize)], window: usize) -> bool {
    if history.len() <= window {
        return false;
    }
    let (p0, c0) = history[history.len() - 1 - window];
    let (pooled, cells) = history[history.len() - 1];
    let volume_ok = (pooled - p0).abs() <= 0.01 * pooled.abs().max(p0.abs()).max(1e-12);
    let room = (0.01 * cells.max(c0) as f64).max(8.0);
    volume_ok && (cells as f64 - c0 as f64).abs() <= room
}

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
        // A staged landscape states the weather that keeps its inventory moving, and it
        // states it before anything is built, so the config the world runs on — and the
        // config its snapshot carries — is the one the recipe asked for.
        let mut config = config;
        if let Some(water) = config.landform.water().copied() {
            water.cycle_into(&mut config);
        }
        let n = config.cells();
        let mut world = World {
            material: vec![Material::Air; n],
            free: vec![0.0; n],
            pore: vec![0.0; n],
            aquifer_m3: 0.0,
            atmosphere_m3: 0.0,
            shower_left_m3: 0.0,
            next_shower_tick: 0,
            outlet_open: false,
            tick: 0,
            terrain_version: 0,
            ledger: Ledger::default(),
            outlet_cell: None,
            spring_cell: None,
            lake_drain: Vec::new(),
            lake_datum_y: None,
            wet: crate::sparse::CellSet::default(),
            damp: crate::sparse::CellSet::default(),
            drainable: crate::sparse::CellSet::default(),
            void_runs: VoidRuns::default(),
            solid: SolidColumns::default(),
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
        if let Some(water) = world.config.landform.water().copied() {
            crate::hydrate::hydrate(&mut world, &water);
            world.ledger = Ledger::default();
        }
        world.ledger.initial_stored = world.view().stored_m3();
        world.ledger.initial_atmosphere = world.atmosphere_m3;
        world.next_shower_tick = crate::water::next_shower_tick(&world.config, 0, 0);
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
            next_shower_tick: 0,
            outlet_open: false,
            tick: 0,
            terrain_version: 0,
            ledger: Ledger::default(),
            outlet_cell: None,
            spring_cell: None,
            lake_drain: Vec::new(),
            lake_datum_y: None,
            wet: crate::sparse::CellSet::default(),
            damp: crate::sparse::CellSet::default(),
            drainable: crate::sparse::CellSet::default(),
            void_runs: VoidRuns::default(),
            solid: SolidColumns::default(),
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
        world.next_shower_tick = crate::water::next_shower_tick(&world.config, 0, 0);
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
    /// The lake's floor drain: the cells on its floor, empty if the world has none.
    pub fn lake_drain(&self) -> &[(u32, u32, u32)] {
        &self.lake_drain
    }

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

    /// The tick the next shower is due. Meaningless without
    /// [`Config::shower_interval_max_s`].
    pub fn next_shower_tick(&self) -> u64 {
        self.next_shower_tick
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
            weather: crate::weather::WeatherView::legacy(
                self.atmosphere_m3,
                self.ledger.expected_total(),
                if self.config.closed_water_budget {
                    self.shower_left_m3 > 0.0
                } else {
                    self.config.rain_m_per_s > 0.0
                },
                self.config.rain_m_per_s,
            ),
            solid: &self.solid.bits,
        }
    }

    /// Advance one tick: prescribed rain and evaporation, free-water substeps,
    /// infiltration, drainage, spring discharge, outlet export. [`World::step_with`]
    /// takes a thread count explicitly.
    pub fn step(&mut self) {
        self.step_with(default_threads());
    }

    /// [`World::step`] with a thread count. Execution only: it sizes the rayon pool the
    /// water phases split their columns across (`crate::water::exchange`), and `1` runs
    /// every phase on the calling thread.
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
    ///
    /// A world with a **stream running** through it settles on the same terms and the test
    /// is still the right one: river re-entry ([`crate::water::reentry`]) moves water from
    /// the sky to the spring and the outlet sends it back, so at steady state the *stored*
    /// volume and the wet-cell count are both constant while the water itself never stops
    /// moving. Flow-through converges here; only accumulation and drainage do not. And a
    /// ring with a stream in it is not dry-locked whatever its sky holds, because
    /// `dry_locked` asks for **no pool at all** and the stream makes one.
    ///
    /// `watch` is called with the world after every tick this steps, for a caller that
    /// has to see the water on its way to rest (the habitat's startup pre-roll remembers
    /// the deepest water every face saw).
    pub fn settle_watching(&mut self, cap_ticks: u32, mut watch: impl FnMut(&World)) -> Settle {
        let window = SETTLE_WINDOW as usize;
        let mut history: Vec<(f64, usize)> = Vec::with_capacity(window + 1);
        let mut stored: Vec<f64> = Vec::with_capacity(window + 1);
        let mut ticks = 0;
        let mut converged;
        loop {
            let pooled = self.pooled_m3();
            let cells = self.wet_cells();
            history.push((pooled, cells));
            stored.push(self.view().stored_m3());
            converged = at_rest(&history) || settled_over(&history, window);
            if converged || ticks >= cap_ticks {
                break;
            }
            self.step();
            watch(self);
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

    /// Make the next scheduled shower **due now**: the world opens with a shower instead
    /// of waiting out its first drawn gap. Later gaps are drawn exactly as before, from
    /// where this shower ends. Returns whether the world has a schedule at all; a world
    /// without one ([`Config::shower_interval_max_s`] zero) is untouched.
    pub fn bring_shower_forward(&mut self) -> bool {
        if !(self.config.shower_interval_max_s > 0.0) {
            return false;
        }
        self.next_shower_tick = self.tick;
        true
    }

    /// Step until the water has stopped moving, or until `cap_ticks`: see
    /// [`World::settle_watching`] for the rule, which this is with nobody watching.
    pub fn settle(&mut self, cap_ticks: u32) -> Settle {
        self.settle_watching(cap_ticks, |_| {})
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
    ///
    /// It rebuilds the support-face lookup ([`SolidColumns`]) too: every path that writes
    /// `material` directly ends here, and so does a decoded world.
    pub(crate) fn rebuild_active_sets(&mut self) {
        self.solid.rebuild(&self.config, &self.material);
        let n = self.config.cells();
        let plane = self.config.width as usize * self.config.depth as usize;
        let height = self.config.height as usize;
        if height > MASK_ROWS {
            self.wet.reset(n);
            self.damp.reset(n);
            self.drainable.reset(n);
            for i in 0..n {
                if !self.material[i].is_solid() && self.free[i] > 0.0 {
                    self.wet.insert(i);
                }
                if self.material[i].pore_capacity() > 0.0 && self.pore[i] > 0.0 {
                    self.damp.insert(i);
                }
                if crate::water::drains(self, i) {
                    self.drainable.insert(i);
                }
            }
            return;
        }
        // At most 128 rows: every set is one row mask per column.
        self.wet.reset_columns(n, plane);
        self.damp.reset_columns(n, plane);
        self.drainable.reset_columns(n, plane);
        for y in 0..height {
            for col in 0..plane {
                let i = y * plane + col;
                if !self.material[i].is_solid() && self.free[i] > 0.0 {
                    self.wet.insert_at(y, col);
                }
                if self.material[i].pore_capacity() > 0.0 && self.pore[i] > 0.0 {
                    self.damp.insert_at(y, col);
                }
                if crate::water::drains(self, i) {
                    self.drainable.insert_at(y, col);
                }
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
        #[cfg(feature = "profile")]
        let _timer = crate::profile::start(crate::profile::Phase::VoidRuns);
        #[cfg(feature = "profile")]
        crate::profile::add(crate::profile::Count::VoidRunRebuilds, 1);
        let plane = self.config.width as usize * self.config.depth as usize;
        let height = self.config.height as usize;
        let VoidRuns {
            offset,
            runs,
            mask,
            sky_floor,
            ..
        } = &mut self.void_runs;
        runs.clear();
        offset.clear();
        mask.clear();
        sky_floor.clear();
        offset.reserve(plane + 1);
        sky_floor.reserve(plane);
        let masked = height <= MASK_ROWS;
        if masked {
            mask.reserve(plane);
        }
        for col in 0..plane {
            offset.push(runs.len() as u32);
            let mut col_mask = 0u128;
            let mut floor = height as u32;
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
                if top + 1 == height {
                    floor = y as u32;
                }
                if masked {
                    col_mask |= run_bits(y, top - y + 1);
                }
                y = top + 1;
            }
            if masked {
                mask.push(col_mask);
            }
            sky_floor.push(floor);
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
        let drains = self.lake_drain.iter().map(|&c| ("lake drain", Some(c)));
        for (name, cell) in [("outlet", self.outlet_cell), ("spring", self.spring_cell)].into_iter().chain(drains) {
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

#[cfg(test)]
mod default_threads_tests {
    use super::threads_for;

    /// The board runs nothing else, so the tick takes every core but one (Wrysk,
    /// 2026-09-21). The one left over is what the presenter's submit, the display
    /// daemon's page flip and the OS run on; taking it too is what turns a 60 fps
    /// panel into a frozen one.
    #[test]
    fn the_default_thread_count_leaves_one_core_to_everything_that_is_not_the_tick() {
        assert_eq!(threads_for(8), 7, "eight cores, seven workers");
        assert_eq!(threads_for(4), 3);
        assert_eq!(threads_for(2), 1);
    }

    /// `available_parallelism` can report one, and a pool of zero workers is not a
    /// serial run — it is a run with nobody in it.
    #[test]
    fn a_single_core_still_gets_one_worker() {
        assert_eq!(threads_for(1), 1);
        assert_eq!(threads_for(0), 1, "a count the OS could not report is one");
    }
}
