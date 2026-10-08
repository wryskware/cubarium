//! Plants as models (`[figures] plants = "models"`, `cubarium_gpu::plants`) on a running GPU
//! sink, lit tier: at rest the plant pass draws exactly the stamped picture, across the
//! ring's seam too; in a gust its plants lean and their roots stay put. Mechanisms only, no
//! tuned numbers. Needs a Vulkan device and skips with a printed reason when there is none.

use cubarium::sink::GpuTargetKind;
use cubarium::sink::gpu::voxel::{VoxelGpuSink, VoxelGpuSinkOptions};
use cubarium::voxel::figure::PlantDraw;
use cubarium::voxel::project::Projection;
use cubarium::voxel::{Lighting, VoxelConfig};
use cubarium_voxel::{Command, Config, Material, World};
use cubarium_voxel_fauna::{Fauna, FaunaConfig};
use cubarium_voxel_flora::{Command as FloraCommand, Flora, FloraConfig, Species};

const SEED_Z: u32 = 4;

fn world(c: &Config) -> World {
    let mut w = World::empty(c.clone());
    for x in 0..i64::from(c.width) {
        for z in 0..c.depth {
            w.apply(Command::SetMaterial { x, y: 0, z, material: Material::Soil });
        }
    }
    w
}

fn config(c: &Config, plants: PlantDraw, set: &str) -> VoxelConfig {
    let mut cfg = VoxelConfig {
        sim: cubarium_session::SimParams { world: c.clone(), ..Default::default() },
        lighting: Lighting::Lit,
        ..VoxelConfig::default()
    };
    cfg.figures.plants = plants;
    if !set.is_empty() {
        cfg.plants = toml::from_str(set).unwrap();
    }
    cfg
}

/// The raster of one tick of `world` and `flora` drawn by a sink of `cfg` at clock
/// `(tick, fraction)`, turned `view_x` columns; `None` without a Vulkan device.
fn draw(cfg: &VoxelConfig, world: &World, flora: &Flora, at: (u64, f64), view_x: i32) -> Option<(Vec<u8>, u32, u32, u32, i32)> {
    let c = &cfg.sim.world;
    let proj = Projection::new(cfg.tilt_degrees, cfg.px_per_voxel, cfg.raster_height, c).unwrap();
    let mut sink = match VoxelGpuSink::new(
        cfg,
        proj,
        VoxelGpuSinkOptions { target: GpuTargetKind::Headless, ..VoxelGpuSinkOptions::default() },
    ) {
        Ok(sink) => sink,
        Err(e) => {
            println!("skipped: no Vulkan device for the voxel GPU renderer ({e:#})");
            return None;
        }
    };
    sink.set_view_x(view_x);
    let fauna = Fauna::new(FaunaConfig::default());
    let mut view = world.view();
    view.tick = at.0;
    let started = std::time::Instant::now();
    loop {
        sink.set_clock(at.0, at.1);
        let staged = sink.stage_view(&view, flora.view(), fauna.view());
        sink.render().unwrap();
        if staged && !sink.light_pending() {
            break;
        }
        assert!(started.elapsed().as_secs() < 60, "the sky plane never arrived");
        std::thread::sleep(std::time::Duration::from_millis(2));
    }
    sink.set_clock(at.0, at.1);
    sink.render().unwrap();
    let p = sink.params();
    Some((sink.read_raster().unwrap(), p.raster_w, p.s, p.rise, p.base))
}

fn grove(c: &Config, xs: &[i64]) -> (World, Flora) {
    let w = world(c);
    let fc = FloraConfig::default();
    let mut flora = Flora::new(fc.clone());
    for &x in xs {
        let wood = fc.species(Species::Vaulttree).wood_max;
        assert!(flora.apply(&w, FloraCommand::Seed { x, z: SEED_Z, species: Species::Vaulttree, wood }));
    }
    (w, flora)
}

#[test]
fn at_rest_the_plant_pass_draws_the_stamped_picture_across_the_seam_too() {
    let c = Config { width: 40, height: 24, depth: 8, ..Config::default() };
    // One plant straddling the seam (x = 0, its crown reaching round to the far edge) and
    // one mid-ring.
    let (w, flora) = grove(&c, &[0, 20]);
    let still = "bend = 0.0";
    for view_x in [0, 2] {
        let Some((stamped, ..)) = draw(&config(&c, PlantDraw::Stamped, ""), &w, &flora, (1, 0.0), view_x) else {
            return;
        };
        let (models, ..) = draw(&config(&c, PlantDraw::Models, still), &w, &flora, (1, 0.0), view_x).unwrap();
        let differ = stamped.chunks_exact(4).zip(models.chunks_exact(4)).filter(|(a, b)| a != b).count();
        assert_eq!(differ, 0, "view turned {view_x} columns: {differ} pixels differ from the stamped picture");
    }
}

#[test]
fn a_plant_behind_water_is_drawn_under_the_water_as_it_was() {
    let c = Config { width: 40, height: 24, depth: 8, ..Config::default() };
    let (mut w, flora) = grove(&c, &[20]);
    // A sheet of water hanging in front of the tree (falling water, unstepped).
    let v = c.voxel_volume();
    for x in 14..27 {
        for y in 2..14 {
            w.apply(Command::AddWater { x, y, z: SEED_Z - 2, volume_m3: v });
        }
    }
    let Some((stamped, ..)) = draw(&config(&c, PlantDraw::Stamped, ""), &w, &flora, (1, 0.0), 0) else {
        return;
    };
    let (models, ..) = draw(&config(&c, PlantDraw::Models, "bend = 0.0"), &w, &flora, (1, 0.0), 0).unwrap();
    let differ = stamped.chunks_exact(4).zip(models.chunks_exact(4)).filter(|(a, b)| a != b).count();
    assert_eq!(differ, 0, "{differ} pixels differ: a plant seen through water is the walk's, under the water");
}

#[test]
fn in_a_gust_the_plants_lean_and_their_roots_stay_put() {
    let c = Config { width: 40, height: 24, depth: 8, ..Config::default() };
    let (w, flora) = grove(&c, &[20]);
    // A breeze with no travel, flutter or peak change, at full strength in the hold.
    let wind = "[wind]\nsweeps = 0\nflutter = 0.0\npeak_vary = 0.0\nrise_s = 1.0\nhold_s = 20.0";
    let gust = (cubarium_voxel::TICK_HZ as u64 * 5, 0.0);
    let Some((rest, rw, s, rise, base)) =
        draw(&config(&c, PlantDraw::Models, &format!("bend = 0.0\n{wind}")), &w, &flora, gust, 0)
    else {
        return;
    };
    let (lean, ..) =
        draw(&config(&c, PlantDraw::Models, &format!("bend = 0.2\nmax_tip_voxels = 3.0\n{wind}")), &w, &flora, gust, 0)
            .unwrap();
    let rows = rest.len() as u32 / (4 * rw);
    let differs = |y: u32| (0..rw).any(|x| {
        let i = ((y * rw + x) * 4) as usize;
        rest[i..i + 4] != lean[i..i + 4]
    });
    // The root is the cell over the soil (y = 1): its front face is the band of rows
    // above `base − s − SEED_Z·rise`, and nothing of the plant is drawn below it.
    let root_bottom = base - s as i32 - (SEED_Z * rise) as i32;
    let root_top = root_bottom - s as i32;
    assert!((0..rows).any(differs), "the gust moves the plant");
    for y in root_top.max(0) as u32..rows {
        assert!(!differs(y), "row {y}: the root's band and below never move");
    }
    let first = (0..rows).find(|&y| differs(y)).unwrap();
    assert!((first as i32) < root_top, "what moves is above the root");
    // A cell that moved has left its resting place: there the world behind it shows, as it
    // does with no plant at all, and not the cell still standing (the walk drew it).
    let bare = Flora::new(FloraConfig::default());
    let (empty, ..) =
        draw(&config(&c, PlantDraw::Models, &format!("bend = 0.0\n{wind}")), &w, &bare, gust, 0).unwrap();
    let vacated = rest
        .chunks_exact(4)
        .zip(lean.chunks_exact(4))
        .zip(empty.chunks_exact(4))
        .filter(|((r, l), e)| r != e && l == e)
        .count();
    assert!(vacated > 0, "some pixel the plant covered at rest shows the bare world when it leans");
}
