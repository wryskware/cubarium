//! The GPU voxel renderer against the CPU presenter, short.
//!
//! Two things are checked here and neither takes a second:
//!
//! 1. the **Rust twin of the shader's projection inversion**
//!    (`cubarium_gpu::voxel::slab_hit`) against `Projection::front_rect` and
//!    `Projection::top_rect`, which needs no device at all — if those two disagree the
//!    shader is asking the wrong voxel for every pixel;
//! 2. one small world drawn by **both renderers**, which needs a device and **skips with
//!    a printed reason** when there is none. It is deliberately not the fidelity example:
//!    that example renders five worlds of 147,456 voxels each and runs a thousand ticks,
//!    which is a report, not a test. This is the same comparison on a 32×12×4 fixture, so
//!    a regression in the walk fails the suite rather than waiting for someone to look at
//!    a PNG.

use cubarium::sink::GpuTargetKind;
use cubarium::sink::gpu::voxel::{VoxelGpuSink, VoxelGpuSinkOptions};
use cubarium::voxel::VoxelConfig;
use cubarium::voxel::present::VoxelPresenter;
use cubarium::voxel::project::Projection;
use cubarium_gpu::voxel::slab_hit;
use cubarium_render::Canvas;
use cubarium_surface::{Scale, Topology};
use cubarium_voxel::{Command, Config, Material, World};
use cubarium_voxel_fauna::{Command as FaunaCommand, Fauna, FaunaConfig, Species as Beast};
use cubarium_voxel_flora::{Command as FloraCommand, Flora, FloraConfig, Species};

fn config() -> Config {
    Config { width: 32, height: 12, depth: 4, ..Config::default() }
}

/// The shader's first three lines, against the projection they invert.
///
/// For every voxel and every pixel of its two rectangles, the slab lookup must name that
/// voxel and that row: the front rectangle as `level = y` at face row `dy`, and the top
/// rectangle as `level = y + 1` at cap row `dy` — the cap of a voxel being the bottom
/// `rise` rows of the band above it, which is the whole reason the presenter draws a cap
/// only where the voxel above is air.
#[test]
fn the_shaders_slab_lookup_names_the_voxel_the_projection_gave_the_pixel() {
    for &(tilt, s) in &[(30.0, 4u32), (35.0, 6), (30.0, 8), (12.0, 4)] {
        let c = config();
        let proj = Projection::new(tilt, s, 0, &c).unwrap();
        let params = cubarium::sink::gpu::voxel::params_of(
            &VoxelConfig { tilt_degrees: tilt, px_per_voxel: s, world: c.clone(), ..VoxelConfig::default() },
            proj,
            true,
        );
        for z in 0..c.depth {
            for y in 0..c.height {
                let (_, fr, _, fh) = proj.front_rect(0, y, z);
                for dy in 0..fh as i32 {
                    let hit = slab_hit(&params, fr + dy, z)
                        .unwrap_or_else(|| panic!("front row {dy} of ({y}, {z}) is off the slab"));
                    assert_eq!(
                        (hit.level, hit.front_row(proj.s)),
                        (y as i32, dy as u32),
                        "front face of (y {y}, z {z}) row {dy} at s = {s}"
                    );
                }
                let (_, tr, _, th) = proj.top_rect(0, y, z);
                for dy in 0..th as i32 {
                    let hit = slab_hit(&params, tr + dy, z)
                        .unwrap_or_else(|| panic!("cap row {dy} of ({y}, {z}) is off the slab"));
                    assert_eq!(
                        (hit.level, hit.cap_row(proj.rise)),
                        (y as i32 + 1, Some(dy as u32)),
                        "top face of (y {y}, z {z}) row {dy} at s = {s}"
                    );
                }
            }
        }
        // Below the floor line nothing is owned, in this slab or any deeper one.
        assert!(slab_hit(&params, proj.base, 0).is_none());
    }
}

/// The two renderers on one small world with terrain, a roof, standing water, a partial
/// water cell and a stand.
///
/// The bar is **two codes**, not zero. The CPU canvas encodes sRGB through
/// `cubarium_render::srgb`'s table and the GPU writes linear into an `R8G8B8A8_SRGB`
/// attachment the hardware encodes; the voxel texture also carries free water quantised
/// to eight bits. One code of disagreement is the encoders, and the fidelity example
/// reports the distribution. Anything above two is a rule that was ported wrong.
#[test]
fn the_gpu_draws_the_same_small_world_as_the_cpu_presenter() {
    let c = config();
    let cfg = VoxelConfig { world: c.clone(), ..VoxelConfig::default() };
    let proj = Projection::new(cfg.tilt_degrees, cfg.px_per_voxel, cfg.raster_height, &c).unwrap();

    let mut gpu = match VoxelGpuSink::new(
        &cfg,
        proj,
        VoxelGpuSinkOptions {
            target: GpuTargetKind::Headless,
            capture: None,
            roof_from_texture: true,
        },
    ) {
        Ok(sink) => sink,
        Err(e) => {
            println!("skipped: no Vulkan device for the voxel GPU renderer ({e:#})");
            return;
        }
    };

    let (world, flora) = fixture(&c);
    // One interim grazer on the strip too (round 5c), so the animal part class is
    // compared on both renderers and not only the plants'.
    let mut fauna = Fauna::new(FaunaConfig::default());
    let body = fauna.config().species(Beast::Frondgrazer).body_max;
    assert!(
        fauna.apply(&world, FaunaCommand::Introduce { x: 3, z: 1, species: Beast::Frondgrazer, body }),
        "the fixture has a support face at (3, 1) for the grazer"
    );
    let mut canvas = Canvas::new(Topology::Ring { w: proj.raster_w, h: proj.raster_h }, Scale::ONE);
    let mut raster = cube_proto::Raster::black(proj.raster_w, proj.raster_h);
    VoxelPresenter::new(cfg, proj).draw_with_fauna(
        &world.view(),
        flora.view(),
        Some(fauna.view()),
        &mut canvas,
    );
    canvas.encode_raster(&mut raster);

    gpu.stage_world(&world, &flora, &fauna);
    gpu.render().expect("one GPU frame");
    let rgba = gpu.read_raster().expect("the raster reads back");

    let cpu = raster.as_bytes();
    let w = usize::from(proj.raster_w);
    let mut worst = (0u8, 0usize);
    for i in 0..cpu.len() / 3 {
        for k in 0..3 {
            let d = cpu[i * 3 + k].abs_diff(rgba[i * 4 + k]);
            if d > worst.0 {
                worst = (d, i);
            }
        }
    }
    assert!(
        worst.0 <= 2,
        "the two renderers differ by {} at ({}, {}): cpu {:?} gpu {:?}",
        worst.0,
        worst.1 % w,
        worst.1 / w,
        &cpu[worst.1 * 3..worst.1 * 3 + 3],
        &rgba[worst.1 * 4..worst.1 * 4 + 3],
    );
}

/// A world with every feature the walk has to get right, small enough to be a test:
/// a soil floor over bedrock, a rock ridge with an overhang, a pool several slabs deep,
/// one quarter-full water cell under a rock lip, and a seeded stand.
fn fixture(c: &Config) -> (World, Flora) {
    let mut world = World::empty(c.clone());
    let v = c.voxel_volume();
    let set = |w: &mut World, x: i64, y: u32, z: u32, m: Material| {
        w.apply(Command::SetMaterial { x, y, z, material: m });
    };
    for z in 0..c.depth {
        for x in 0..i64::from(c.width) {
            set(&mut world, x, 0, z, Material::Bedrock);
            set(&mut world, x, 1, z, Material::Rock);
            set(&mut world, x, 2, z, Material::Soil);
            // A ridge that climbs into depth, so a riser and a receding plateau exist.
            if (8..20).contains(&x) {
                for y in 3..=3 + z {
                    set(&mut world, x, y, z, Material::Soil);
                }
            }
        }
    }
    // An overhang: a shelf at y = 7 over open air, which is what casts the roof shadow.
    for x in 22..28i64 {
        set(&mut world, x, 7, 0, Material::Rock);
        set(&mut world, x, 7, 1, Material::Rock);
    }
    // A pool four slabs deep across the seam, and one partial cell under a rock lip —
    // the presenter's own `a_partial_roof_clips_a_water_top` geometry.
    for z in 0..c.depth {
        for x in -2..6i64 {
            for y in 3..5u32 {
                world.apply(Command::AddWater { x, y, z, volume_m3: v });
            }
        }
    }
    world.apply(Command::AddWater { x: 24, y: 6, z: 1, volume_m3: v * 0.25 });
    // And some pore water, so the wet-soil darkening is on screen.
    for x in 8..20i64 {
        world.apply(Command::AddWater { x, y: 2, z: 2, volume_m3: v * 0.2 });
    }

    let mut flora = Flora::new(FloraConfig::default());
    for (x, z, species) in
        [(14i64, 2u32, Species::Bloomcrown), (2, 1, Species::Umbrellafrond)]
    {
        let wood = flora.config().species(species).wood_max;
        flora.apply(&world, FloraCommand::Seed { x, z, species, wood });
    }
    (world, flora)
}
