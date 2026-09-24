//! The lit tier's two planes, built on the CPU for `voxel.frag` (package L,
//! `design/handoffs/presentation-plan-2026-09-24.md`).
//!
//! **The ecology's light model is the source of truth.** The picture may shade more
//! finely than the model's sky fan, never differently, so neither plane is a second
//! light model:
//!
//! * the **sky plane** is the core's own [`VoxelView::sky_visibility`], called unchanged,
//!   at every open cell. Open cell `(x, y, z)` stores `sky_visibility(x, y − 1, z)`, the
//!   fan from the foot of the open cell: a top face (whose open cell is the one above it)
//!   reads exactly the model's number, and a wall reads the fan from the foot of the cell
//!   in front of it. It is a function of the terrain alone, so it is keyed on the terrain
//!   like the roof table and computed on a thread of its own ([`SkyWorker`]);
//! * the **canopy plane** is the flora model's foliage layers, attenuating straight down as
//!   the shade model applies them (`crates/cubarium-voxel-flora/src/step.rs`,
//!   `shade_layers_into`): each layer is a disc of `radius_v` cells about its stand's
//!   column at its own drawn cell, and lets `exp(−k · (1 − porosity) · stock / area)`
//!   through to anything under it. Rebuilt every pack ([`Canopy`]).

use std::sync::mpsc::{self, Receiver, Sender};

use cubarium_gpu::voxel::CANOPY_STEPS;
use cubarium_voxel::{Config, Ledger, Material, VoxelView};
use cubarium_voxel_flora::FloraView;

#[inline]
fn quantise(v: f64) -> u8 {
    (v.clamp(0.0, 1.0) * 255.0).round() as u8
}

/// The sky plane of `material` into `out`, in the voxel texture's order
/// (`(z · height + y) · width + x`), over `threads` threads.
///
/// Every air cell gets its value; a solid cell is never an open cell and gets 0. A cell on
/// the floor row has no cell under it and takes its own top face's fan. A cell whose foot
/// is at or above the highest solid anywhere is open sky without asking: every ray in the
/// fan climbs, so none of them can meet a solid, and the answer is exactly 1.
pub(crate) fn sky_plane(config: &Config, material: &[Material], threads: usize, out: &mut [u8]) {
    let (w, h, d) = (
        config.width as usize,
        config.height as usize,
        config.depth as usize,
    );
    assert_eq!(material.len(), w * h * d, "one material per voxel");
    assert_eq!(out.len(), w * h * d, "one sky byte per voxel");
    let ledger = Ledger::default();
    let view = VoxelView {
        config,
        material,
        free: &[],
        pore: &[],
        tick: 0,
        terrain_version: 0,
        ledger: &ledger,
        aquifer_m3: 0.0,
        atmosphere_m3: 0.0,
        shower_left_m3: 0.0,
        outlet_open: false,
        outlet: None,
        spring: None,
    };
    let top = material
        .iter()
        .enumerate()
        .filter(|(_, m)| m.is_solid())
        .map(|(i, _)| config.coords(i).1)
        .max();
    let slab = |z: u32, rows: &mut [u8]| {
        for y in 0..h as u32 {
            for x in 0..w {
                let i = y as usize * w + x;
                if view.material_at(x as i64, y, z).is_solid() {
                    rows[i] = 0;
                    continue;
                }
                let foot = y.saturating_sub(1);
                rows[i] = if top.is_none_or(|t| foot >= t) {
                    255
                } else {
                    quantise(view.sky_visibility(x as i64, foot, z))
                };
            }
        }
    };
    // One z slab is one contiguous run of the output. Slabs are dealt round-robin, so
    // each thread gets front and back slabs alike and the costly ones spread.
    let threads = threads.clamp(1, d.max(1));
    let mut groups: Vec<Vec<(u32, &mut [u8])>> = (0..threads).map(|_| Vec::new()).collect();
    for (z, rows) in out.chunks_mut(w * h).enumerate() {
        groups[z % threads].push((z as u32, rows));
    }
    std::thread::scope(|s| {
        for group in groups {
            let slab = &slab;
            s.spawn(move || {
                for (z, rows) in group {
                    slab(z, rows);
                }
            });
        }
    });
}

/// The sky plane, computed off the loop thread.
///
/// The packer submits the terrain whenever it moves and polls for the plane each pack; a
/// newer submission supersedes an older one still waiting, and a finished plane for a
/// terrain that has since moved is thrown away by its id. Dropping the worker ends the
/// thread once it finishes whatever it is computing.
pub(crate) struct SkyWorker {
    jobs: Sender<(u64, Config, Vec<Material>)>,
    done: Receiver<(u64, Vec<u8>)>,
}

impl SkyWorker {
    pub(crate) fn spawn() -> SkyWorker {
        let (jobs, inbox) = mpsc::channel::<(u64, Config, Vec<Material>)>();
        let (outbox, done) = mpsc::channel();
        // Every core the process may use but one, which the loop keeps: this runs once
        // at start-up and again only when the terrain is edited.
        let threads = std::thread::available_parallelism()
            .map_or(1, |n| n.get())
            .saturating_sub(1)
            .max(1);
        std::thread::Builder::new()
            .name("cubarium-sky".into())
            .spawn(move || {
                while let Ok(mut job) = inbox.recv() {
                    while let Ok(newer) = inbox.try_recv() {
                        job = newer;
                    }
                    let (id, config, material) = job;
                    let mut plane = vec![0u8; material.len()];
                    sky_plane(&config, &material, threads, &mut plane);
                    if outbox.send((id, plane)).is_err() {
                        break;
                    }
                }
            })
            .expect("spawning the sky plane's thread");
        SkyWorker { jobs, done }
    }

    /// Compute the sky plane of this terrain, under `id`.
    pub(crate) fn submit(&self, id: u64, config: &Config, material: &[Material]) {
        // A worker that has gone away leaves the plane as it was; nothing else breaks.
        let _ = self.jobs.send((id, config.clone(), material.to_vec()));
    }

    /// The newest plane finished since the last poll, with its id.
    pub(crate) fn poll(&self) -> Option<(u64, Vec<u8>)> {
        let mut newest = None;
        while let Ok(done) = self.done.try_recv() {
            newest = Some(done);
        }
        newest
    }
}

/// One foliage step over a column: the drawn cell of the layer's disc and what it lets
/// through.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct Step {
    cell: u8,
    t: f32,
}

/// The canopy plane: per column, the foliage layers over it as up to [`CANOPY_STEPS`]
/// steps, highest first, each carrying the product of its own transmission and every one
/// above it.
///
/// A face of a voxel at height `y` is under a layer when the layer's disc is drawn in a
/// cell above `y`: the ground and the trunk under a crown, and never the crown's own cells
/// (the shader's `canopyAt`). A column under more than [`CANOPY_STEPS`] distinct layers
/// merges its two lowest into the lower one's cell, which keeps the product — and so the
/// ground under all of them — exact and lets only a face between those two layers see
/// through the upper one.
pub(crate) struct Canopy {
    width: u32,
    depth: u32,
    steps: Vec<[Step; CANOPY_STEPS + 1]>,
    count: Vec<u8>,
}

impl Canopy {
    pub(crate) fn new(width: u32, depth: u32) -> Canopy {
        let n = width as usize * depth as usize;
        Canopy {
            width,
            depth,
            steps: vec![[Step::default(); CANOPY_STEPS + 1]; n],
            count: vec![0; n],
        }
    }

    /// Rebuild from `flora`'s foliage layers and write the plane into `out`
    /// ([`cubarium_gpu::voxel::VoxelStaging::canopy`]'s layout).
    pub(crate) fn build(&mut self, flora: FloraView<'_>, out: &mut [u8]) {
        self.count.fill(0);
        let k = flora.config.shade_k_per_m2;
        let (w, d) = (i64::from(self.width), i64::from(self.depth));
        for stand in flora.stands {
            for layer in flora.layers(stand) {
                let t = (-k * (1.0 - layer.porosity) * layer.stock / layer.area_m2).exp();
                if t.is_nan() || t >= 1.0 || layer.cell <= 0 {
                    continue;
                }
                let cell = layer.cell.min(255) as u8;
                let r = layer.radius_v.max(0.0);
                let reach = r.floor() as i64;
                let (x0, z0) = (i64::from(stand.site.x), i64::from(stand.site.z));
                for dz in -reach..=reach {
                    let z = z0 + dz;
                    if z < 0 || z >= d {
                        continue;
                    }
                    for dx in -reach..=reach {
                        if ((dx * dx + dz * dz) as f64) > r * r {
                            continue;
                        }
                        let x = (x0 + dx).rem_euclid(w);
                        self.add((z * w + x) as usize, cell, t as f32);
                    }
                }
            }
        }
        self.write(out);
    }

    fn add(&mut self, col: usize, cell: u8, t: f32) {
        let steps = &mut self.steps[col];
        let n = self.count[col] as usize;
        if let Some(s) = steps[..n].iter_mut().find(|s| s.cell == cell) {
            s.t *= t;
            return;
        }
        // Insert keeping the cells descending.
        let at = steps[..n].iter().position(|s| s.cell < cell).unwrap_or(n);
        steps.copy_within(at..n, at + 1);
        steps[at] = Step { cell, t };
        if n + 1 > CANOPY_STEPS {
            let low = steps[CANOPY_STEPS];
            steps[CANOPY_STEPS - 1].t *= low.t;
            steps[CANOPY_STEPS - 1].cell = low.cell;
            self.count[col] = CANOPY_STEPS as u8;
        } else {
            self.count[col] = (n + 1) as u8;
        }
    }

    fn write(&self, out: &mut [u8]) {
        let w = self.width as usize;
        let per_row = CANOPY_STEPS / 2;
        for (col, steps) in self.steps.iter().enumerate() {
            let (x, z) = (col % w, col / w);
            let n = self.count[col] as usize;
            let mut through = 1.0f32;
            for (k, step) in steps[..CANOPY_STEPS].iter().enumerate() {
                let at = ((z * per_row + k / 2) * w + x) * 4 + (k % 2) * 2;
                if k < n {
                    through *= step.t;
                    out[at] = step.cell;
                    out[at + 1] = quantise(f64::from(through));
                } else {
                    out[at] = 0;
                    out[at + 1] = 0;
                }
            }
        }
    }

    /// The transmission the shader's `canopyAt` reads for a face of a voxel at height
    /// `y` in column `(x, z)`, from a written plane.
    #[cfg(test)]
    pub(crate) fn read(plane: &[u8], width: u32, x: u32, z: u32, y: u32) -> f32 {
        let (w, per_row) = (width as usize, CANOPY_STEPS / 2);
        let mut t = 1.0;
        for k in 0..CANOPY_STEPS {
            let at = ((z as usize * per_row + k / 2) * w + x as usize) * 4 + (k % 2) * 2;
            if u32::from(plane[at]) <= y {
                break;
            }
            t = f32::from(plane[at + 1]) / 255.0;
        }
        t
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cubarium_voxel::{Command, World};
    use cubarium_voxel_flora::{Command as FloraCommand, Flora, FloraConfig, Species};

    fn config() -> Config {
        Config {
            width: 24,
            height: 12,
            depth: 6,
            ..Config::default()
        }
    }

    fn set(world: &mut World, x: i64, y: u32, z: u32, material: Material) {
        world.apply(Command::SetMaterial { x, y, z, material });
    }

    /// Ground at y = 0..2, a wall at the back, and a roofed passage: open cells under open
    /// sky, against a wall and under a roof.
    fn world(c: &Config) -> World {
        let mut world = World::empty(c.clone());
        for z in 0..c.depth {
            for x in 0..i64::from(c.width) {
                set(&mut world, x, 0, z, Material::Bedrock);
                set(&mut world, x, 1, z, Material::Soil);
            }
        }
        for x in 0..i64::from(c.width) {
            for y in 2..4 {
                set(&mut world, x, y, c.depth - 1, Material::Rock);
            }
        }
        for x in 10..16i64 {
            for z in 0..c.depth {
                set(&mut world, x, 5, z, Material::Rock);
            }
        }
        world
    }

    /// Every open cell holds the core's own number for the fan from its foot, quantised
    /// to a byte, so a top face reads exactly the model's sky visibility; and the answer
    /// does not depend on how many threads computed it.
    #[test]
    fn the_sky_plane_is_the_cores_sky_visibility_at_each_open_cells_foot() {
        let c = config();
        let world = world(&c);
        let view = world.view();
        let n = view.material.len();
        let (mut one, mut four) = (vec![0u8; n], vec![0u8; n]);
        sky_plane(&c, view.material, 1, &mut one);
        sky_plane(&c, view.material, 4, &mut four);
        assert!(one == four, "the plane does not depend on the thread count");
        let at = |x: u32, y: u32, z: u32| {
            one[(z as usize * c.height as usize + y as usize) * c.width as usize + x as usize]
        };
        let mut checked = 0;
        for z in 0..c.depth {
            for y in 1..c.height {
                for x in 0..c.width {
                    if view.material_at(i64::from(x), y, z).is_solid() {
                        assert_eq!(at(x, y, z), 0, "a solid is no open cell");
                        continue;
                    }
                    let want = quantise(view.sky_visibility(i64::from(x), y - 1, z));
                    assert_eq!(at(x, y, z), want, "open cell ({x}, {y}, {z})");
                    checked += 1;
                }
            }
        }
        assert!(checked > 500);
        // The three places the fixture is for.
        assert_eq!(at(2, 2, 1), 255, "open ground, far from the wall");
        assert!(at(12, 2, 2) < 128, "under the roof: {}", at(12, 2, 2));
        let wall = at(2, 2, c.depth - 2);
        assert!(
            (130..=210).contains(&wall),
            "in front of the wall about two thirds of the sky: {wall}"
        );
    }

    fn flora(world: &World, stands: &[(i64, u32, Species)]) -> Flora {
        let mut flora = Flora::new(FloraConfig::default());
        for &(x, z, species) in stands {
            let wood = flora.config().species(species).wood_max;
            assert!(flora.apply(
                world,
                FloraCommand::Seed {
                    x,
                    z,
                    species,
                    wood
                }
            ));
        }
        flora
    }

    /// The ground under a crown is dimmed by exactly the layers whose discs are drawn
    /// above it, as the shade model multiplies them; the crown's own cells and anything
    /// outside its disc are not; two crowns over one column multiply.
    #[test]
    fn the_canopy_plane_attenuates_straight_down_by_the_models_layers() {
        let c = Config {
            width: 32,
            height: 40,
            depth: 12,
            ..Config::default()
        };
        let mut world = World::empty(c.clone());
        for z in 0..c.depth {
            for x in 0..i64::from(c.width) {
                set(&mut world, x, 0, z, Material::Bedrock);
                set(&mut world, x, 1, z, Material::Soil);
            }
        }
        let flora = flora(
            &world,
            &[(8, 6, Species::Umbrellafrond), (9, 6, Species::Bloomcrown)],
        );
        let view = flora.view();
        let mut canopy = Canopy::new(c.width, c.depth);
        let mut plane = vec![0u8; c.width as usize * c.depth as usize * CANOPY_STEPS * 2];
        canopy.build(view, &mut plane);

        // What the shade model would multiply for a receiver on the ground at (x, z).
        let k = view.config.shade_k_per_m2;
        let model = |x: i64, z: i64, y: u32| -> f64 {
            let mut t = 1.0;
            for s in view.stands {
                for l in view.layers(s) {
                    let (dx, dz) = (x - i64::from(s.site.x), z - i64::from(s.site.z));
                    let inside = ((dx * dx + dz * dz) as f64) <= l.radius_v * l.radius_v;
                    if inside && l.cell > i64::from(y) {
                        t *= (-k * (1.0 - l.porosity) * l.stock / l.area_m2).exp();
                    }
                }
            }
            t
        };
        let (mut shaded, mut both) = (0, 0);
        for z in 0..c.depth {
            for x in 0..c.width {
                let want = model(i64::from(x), i64::from(z), 1);
                let got = Canopy::read(&plane, c.width, x, z, 1);
                assert!(
                    (f64::from(got) - want).abs() <= 1.0 / 255.0,
                    "ground at ({x}, {z}): the plane says {got}, the model {want}"
                );
                if want < 0.999 {
                    shaded += 1;
                }
                let stands_over = view
                    .stands
                    .iter()
                    .filter(|s| {
                        view.layers(s).any(|l| {
                            let (dx, dz) = (
                                i64::from(x) - i64::from(s.site.x),
                                i64::from(z) - i64::from(s.site.z),
                            );
                            ((dx * dx + dz * dz) as f64) <= l.radius_v * l.radius_v
                        })
                    })
                    .count();
                if stands_over == 2 {
                    both += 1;
                }
            }
        }
        assert!(shaded > 0, "the crowns shade some ground");
        assert!(both > 0, "the fixture overlaps its two crowns");
        // A crown's own cell is not under itself: at the highest disc's cell, nothing.
        let top = view
            .stands
            .iter()
            .flat_map(|s| view.layers(s).map(|l| l.cell))
            .max()
            .unwrap();
        assert_eq!(Canopy::read(&plane, c.width, 8, 6, top as u32), 1.0);
        // Far from both, open.
        assert_eq!(Canopy::read(&plane, c.width, 24, 1, 1), 1.0);
    }

    /// Five distinct layers over one column: the ground under all of them keeps the whole
    /// product, and the steps stay highest first.
    #[test]
    fn a_column_under_more_layers_than_steps_keeps_the_grounds_product() {
        let mut canopy = Canopy::new(1, 1);
        let layers = [(30u8, 0.9f32), (10, 0.8), (20, 0.7), (5, 0.6), (25, 0.5)];
        for (cell, t) in layers {
            canopy.add(0, cell, t);
        }
        let mut plane = vec![0u8; CANOPY_STEPS * 2];
        canopy.write(&mut plane);
        let all: f32 = layers.iter().map(|l| l.1).product();
        assert!((Canopy::read(&plane, 1, 0, 0, 1) - all).abs() <= 1.0 / 255.0);
        assert!((Canopy::read(&plane, 1, 0, 0, 26) - 0.9).abs() <= 1.0 / 255.0);
        assert!((Canopy::read(&plane, 1, 0, 0, 21) - 0.45).abs() <= 1.0 / 255.0);
        assert_eq!(Canopy::read(&plane, 1, 0, 0, 30), 1.0);
        let cells: Vec<u8> = (0..CANOPY_STEPS)
            .map(|k| plane[(k / 2) * 4 + (k % 2) * 2])
            .collect();
        assert!(cells.windows(2).all(|p| p[0] > p[1]), "{cells:?}");
    }
}
