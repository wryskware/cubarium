use anyhow::{bail, ensure};
use serde::{Deserialize, Serialize};

use crate::{Config, Ledger, Material};

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
    AddWater { x: i64, y: u32, z: u32, volume_m3: f64 },
    /// Replace a voxel's material, preserving the water volume the voxel held.
    ///
    /// Its free and pore water are converted to cubic metres first, then the new
    /// material keeps what fits: air keeps free water up to its void, soil and rock keep
    /// pore water up to their own pore capacity, and a solid turned to air releases its
    /// pore water as free water. The rest is displaced to the nearest void with room —
    /// wrapped face-adjacent void path distance, sharing equally among equal-distance
    /// recipients before any farther one — and only volume with no reachable room at all
    /// is booked as `Ledger::displaced_out`.
    SetMaterial { x: i64, y: u32, z: u32, material: Material },
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
    pub fn free_at(&self, x: i64, y: u32, z: u32) -> f64 {
        self.free[self.config.index(x, y, z)]
    }
    #[inline]
    pub fn pore_at(&self, x: i64, y: u32, z: u32) -> f64 {
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
            total += self.free[i] * v * if m.is_solid() { 0.0 } else { 1.0 };
            total += self.pore[i] * v * m.pore_capacity();
        }
        total
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
    /// hand-authored scenes. Panics on a config [`Config::validate`] refuses.
    pub fn empty(config: Config) -> World {
        config.validate().expect("World::empty needs a valid Config");
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

    /// Apply a command now — immediately, including while a frontend has the world
    /// paused; nothing is queued. Returns the signed volume in cubic metres actually
    /// accepted, zero for a non-water command or a refused amount. See
    /// [`crate::water::apply`] for the whole receipt contract.
    pub fn apply(&mut self, command: Command) -> f64 {
        crate::water::apply(self, command)
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
        for (name, len) in
            [("material", self.material.len()), ("free", self.free.len()), ("pore", self.pore.len())]
        {
            ensure!(len == n, "{name} has {len} entries, not one per cell ({n})");
        }
        for (name, store) in [("free", &self.free), ("pore", &self.pore)] {
            if let Some((i, bad)) =
                store.iter().copied().enumerate().find(|&(_, f)| !(0.0..=1.0).contains(&f))
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
            ("displaced_out", self.ledger.displaced_out),
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
