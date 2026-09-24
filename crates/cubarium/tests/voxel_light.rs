//! The lit tier's voxel AO on the GPU, on a hand-built fixture: each face's four corner
//! counts, from the eight cells of its open plane, interpolated across its texels.
//!
//! Needs a device and **skips with a printed reason** when there is none, as
//! `tests/voxel_gpu.rs` does. The ladder is set fine (256 rungs, no floor, no tint, no
//! haze, no grain) so a texel's colour is its base times `sky × AO` and nothing else, and
//! the sky is the core's own number for the face's open cell, so every expected colour is
//! computed here from the corner counts written out by hand.

use cubarium::sink::GpuTargetKind;
use cubarium::sink::gpu::voxel::{VoxelGpuSink, VoxelGpuSinkOptions};
use cubarium::voxel::project::Projection;
use cubarium::voxel::{LightConfig, Lighting, VoxelConfig};
use cubarium_voxel::{Command, Config, Material, World};
use cubarium_voxel_fauna::{Fauna, FaunaConfig};
use cubarium_voxel_flora::{Flora, FloraConfig};

const GAIN: f32 = 3.0;

fn srgb(l: f32) -> f32 {
    let l = l.clamp(0.0, 1.0);
    if l <= 0.003_130_8 {
        l * 12.92
    } else {
        1.055 * l.powf(1.0 / 2.4) - 0.055
    }
}

/// One face under test: which voxel, which face, its corners (0 = both sides occluded,
/// 3 = open) as `[c00, c10, c01, c11]` — u across +x, v up (+y) on a front face and back
/// (+z) on a top face — and the voxel whose sky it reads (the foot of its open cell).
struct Face {
    what: &'static str,
    voxel: (i64, u32, u32),
    top: bool,
    corners: [f32; 4],
    foot: (i64, u32, u32),
}

#[test]
fn voxel_ao_darkens_each_corner_by_its_neighbour_count() {
    let c = Config {
        width: 16,
        height: 8,
        depth: 4,
        ..Config::default()
    };
    let mut world = World::empty(c.clone());
    let mut set = |x: i64, y: u32, z: u32, material: Material| {
        world.apply(Command::SetMaterial { x, y, z, material });
    };
    for z in 0..c.depth {
        for x in 0..i64::from(c.width) {
            set(x, 0, z, Material::Bedrock);
            set(x, 1, z, Material::Rock);
        }
    }
    // A pillar two voxels tall, and a concave corner of two single blocks.
    set(6, 2, 1, Material::Rock);
    set(6, 3, 1, Material::Rock);
    set(10, 2, 1, Material::Rock);
    set(9, 2, 2, Material::Rock);

    let faces = [
        Face {
            what: "the floor left of the pillar: its right side occluded",
            voxel: (5, 1, 1),
            top: true,
            corners: [3.0, 2.0, 3.0, 2.0],
            foot: (5, 1, 1),
        },
        Face {
            what: "the floor in front of the pillar: its back side occluded",
            voxel: (6, 1, 0),
            top: true,
            corners: [3.0, 3.0, 2.0, 2.0],
            foot: (6, 1, 0),
        },
        Face {
            what: "the floor in the concave corner: right and back, so that corner is 0",
            voxel: (9, 1, 1),
            top: true,
            corners: [3.0, 2.0, 2.0, 0.0],
            foot: (9, 1, 1),
        },
        Face {
            what: "the pillar's front: the floor in front occludes its lower corners",
            voxel: (6, 2, 1),
            top: false,
            corners: [1.0, 1.0, 3.0, 3.0],
            foot: (6, 1, 0),
        },
    ];

    let cfg = VoxelConfig {
        world: c.clone(),
        px_per_voxel: 6,
        haze: 0.0,
        dither: 0.0,
        sky_gradient: false,
        lighting: Lighting::Lit,
        light: LightConfig {
            levels: 256,
            ambient_gain: GAIN,
            ambient_floor: 0.0,
            ao: 1.0,
            ambient_tint: 0.0,
        },
        ..VoxelConfig::default()
    };
    let proj = Projection::new(cfg.tilt_degrees, cfg.px_per_voxel, cfg.raster_height, &c).unwrap();
    let mut gpu = match VoxelGpuSink::new(
        &cfg,
        proj,
        VoxelGpuSinkOptions {
            target: GpuTargetKind::Headless,
            ..VoxelGpuSinkOptions::default()
        },
    ) {
        Ok(sink) => sink,
        Err(e) => {
            println!("skipped: no Vulkan device for the voxel GPU renderer ({e:#})");
            return;
        }
    };
    let (flora, fauna) = (
        Flora::new(FloraConfig::default()),
        Fauna::new(FaunaConfig::default()),
    );
    // The sky plane is computed off this thread: stage until it has arrived.
    let started = std::time::Instant::now();
    loop {
        assert!(gpu.stage_world(&world, &flora, &fauna));
        if !gpu.light_pending() {
            break;
        }
        assert!(
            started.elapsed().as_secs() < 30,
            "the sky plane never arrived"
        );
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    gpu.render().expect("one GPU frame");
    let rgba = gpu.read_raster().expect("the raster reads back");
    let w = usize::from(proj.raster_w);

    let rock = cubarium::present::srgb_linear(cubarium::voxel::present::ROCK_SRGB);
    let view = world.view();
    let (s, rise) = (proj.s, proj.rise);
    for face in &faces {
        let (x, y, z) = face.voxel;
        let sky = (view.sky_visibility(face.foot.0, face.foot.1, face.foot.2) * 255.0).round()
            as f32
            / 255.0;
        let (col, row, fw, fh) = if face.top {
            proj.top_rect(x, y, z)
        } else {
            proj.front_rect(x, y, z)
        };
        assert_eq!((fw, fh), (s, if face.top { rise } else { s }));
        // Every texel but the face's edge rows and columns, where the flat tier's edge
        // treatments (the contour row, the side columns) would lean the colour.
        for dy in 1..fh - 1 {
            for dx in 1..fw - 1 {
                let fu = (dx as f32 + 0.5) / fw as f32;
                let fv = 1.0 - (dy as f32 + 0.5) / fh as f32;
                let [c00, c10, c01, c11] = face.corners;
                let lerp = |a: f32, b: f32, t: f32| a + (b - a) * t;
                let a = lerp(lerp(c00, c10, fu), lerp(c01, c11, fu), fv);
                let ao = a / 3.0;
                let rung = (sky * ao * 255.0 + 0.5).floor() / 255.0;
                let want: Vec<u8> = rock
                    .iter()
                    .map(|b| (srgb(b * GAIN * rung) * 255.0).round() as u8)
                    .collect();
                let (px, py) = ((col + dx as i32) as usize, (row + dy as i32) as usize);
                let got = &rgba[(py * w + px) * 4..(py * w + px) * 4 + 3];
                for k in 0..3 {
                    assert!(
                        got[k].abs_diff(want[k]) <= 2,
                        "{}: texel ({dx}, {dy}) is {got:?}, want {want:?} (sky {sky}, AO {ao})",
                        face.what
                    );
                }
            }
        }
    }
}
