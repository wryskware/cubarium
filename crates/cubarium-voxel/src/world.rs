use serde::{Deserialize, Serialize};

use crate::{Config, Ledger, Material};

/// A frontend or a test changes the world only through these.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Command {
    /// One rain event: `volume_m3` spread over exposed top surfaces this tick.
    RainPulse { volume_m3: f64 },
    /// Add free water into one voxel (clipped to its void space; the rest is refused
    /// and reported in the return value of `apply`).
    AddWater { x: i64, y: u32, z: u32, volume_m3: f64 },
    /// Replace a voxel's material. Water it displaces moves to available space or is
    /// booked as `displaced_out`; pore water above the new capacity likewise.
    SetMaterial { x: i64, y: u32, z: u32, material: Material },
    /// Add to (or, negative, remove from) the aquifer store.
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
    pub free: &'a [f32],
    /// Pore water as a fraction of the voxel's pore capacity, `0..=1`. Zero in air.
    pub pore: &'a [f32],
    pub tick: u64,
    pub ledger: &'a Ledger,
    /// Aquifer store in cubic metres.
    pub aquifer_m3: f64,
    /// Whether the named outlet is exporting.
    pub outlet_open: bool,
    /// The outlet cell `(x, y, z)`, if the world names one.
    pub outlet: Option<(u32, u32, u32)>,
    /// The spring cell `(x, y, z)`, if the world names one.
    pub spring: Option<(u32, u32, u32)>,
}

impl<'a> VoxelView<'a> {
    #[inline]
    pub fn material_at(&self, x: i64, y: u32, z: u32) -> Material {
        self.material[self.config.index(x, y, z)]
    }
    #[inline]
    pub fn free_at(&self, x: i64, y: u32, z: u32) -> f32 {
        self.free[self.config.index(x, y, z)]
    }
    #[inline]
    pub fn pore_at(&self, x: i64, y: u32, z: u32) -> f32 {
        self.pore[self.config.index(x, y, z)]
    }
    /// Highest solid voxel in column `(x, z)`, or `None` if the column is all air.
    pub fn surface_y(&self, x: i64, z: u32) -> Option<u32> {
        (0..self.config.height).rev().find(|&y| self.material_at(x, y, z).is_solid())
    }
    /// Total free plus pore plus aquifer water in cubic metres.
    pub fn stored_m3(&self) -> f64 {
        let v = self.config.voxel_volume();
        let mut total = self.aquifer_m3;
        for (i, m) in self.material.iter().enumerate() {
            total += self.free[i] as f64 * v * if m.is_solid() { 0.0 } else { 1.0 };
            total += self.pore[i] as f64 * v * m.pore_capacity();
        }
        total
    }
}

/// The world. Stepped at [`crate::TICK_HZ`]; pure given its inputs.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct World {
    pub(crate) config: Config,
    pub(crate) material: Vec<Material>,
    pub(crate) free: Vec<f32>,
    pub(crate) pore: Vec<f32>,
    pub(crate) aquifer_m3: f64,
    pub(crate) outlet_open: bool,
    pub(crate) tick: u64,
    pub(crate) ledger: Ledger,
    /// The one named outlet: when open it exports free water out of this cell.
    /// Generation picks the lowest void cell of the receiving basin.
    pub(crate) outlet_cell: Option<(u32, u32, u32)>,
    /// Where the aquifer discharges when its head rises above the cell. Generation
    /// picks a low void cell part way up the basin flank.
    pub(crate) spring_cell: Option<(u32, u32, u32)>,
}

impl World {
    /// Generate a world from its config. Deterministic in `config.seed`.
    pub fn new(config: Config) -> World {
        let n = config.cells();
        let mut world = World {
            material: vec![Material::Air; n],
            free: vec![0.0; n],
            pore: vec![0.0; n],
            aquifer_m3: 0.0,
            outlet_open: false,
            tick: 0,
            ledger: Ledger::default(),
            outlet_cell: None,
            spring_cell: None,
            config,
        };
        crate::generate::landform(&mut world);
        world.ledger.initial_stored = world.view().stored_m3();
        world
    }

    /// An all-air world over one bedrock floor row: the fixture builder for tests and
    /// hand-authored scenes.
    pub fn empty(config: Config) -> World {
        let n = config.cells();
        let mut material = vec![Material::Air; n];
        for x in 0..config.width as i64 {
            for z in 0..config.depth {
                material[config.index(x, 0, z)] = Material::Bedrock;
            }
        }
        World {
            free: vec![0.0; n],
            pore: vec![0.0; n],
            aquifer_m3: 0.0,
            outlet_open: false,
            tick: 0,
            ledger: Ledger::default(),
            outlet_cell: None,
            spring_cell: None,
            material,
            config,
        }
    }

    pub fn config(&self) -> &Config {
        &self.config
    }

    pub fn tick(&self) -> u64 {
        self.tick
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
            ledger: &self.ledger,
            aquifer_m3: self.aquifer_m3,
            outlet_open: self.outlet_open,
            outlet: self.outlet_cell,
            spring: self.spring_cell,
        }
    }

    /// Advance one tick: prescribed rain and evaporation, free-water substeps,
    /// infiltration, drainage, spring discharge, outlet export.
    pub fn step(&mut self) {
        crate::water::step(self);
        self.tick += 1;
    }

    /// Apply a command now. Returns the volume actually accepted for water commands.
    pub fn apply(&mut self, command: Command) -> f64 {
        crate::water::apply(self, command)
    }

    /// Serialize the whole world. Refuses nothing; `load` refuses other schemas.
    pub fn save(&self) -> Vec<u8> {
        crate::snapshot::encode(self)
    }

    pub fn load(bytes: &[u8]) -> anyhow::Result<World> {
        crate::snapshot::decode(bytes)
    }
}
