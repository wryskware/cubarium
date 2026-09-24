//! The lit tier on the GPU, on hand-built fixtures: the voxel AO's crease lines (a band
//! along each face edge whose side neighbour in the open plane occludes, and a corner
//! square where only the diagonal does), and the sun's cast shadow (a pillar on a flat
//! floor, against its analytic shadow).
//!
//! Needs a device and **skips with a printed reason** when there is none, as
//! `tests/voxel_gpu.rs` does. The ladder is set fine (256 rungs, no floor, no tint, no
//! haze, no grain) so a texel's colour is its base times `sky × AO` and nothing else, and
//! the sky is the core's own number for the face's open cell, so every expected colour is
//! computed here from the occluders written out by hand.

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

/// One face under test: which voxel, which face, which of its open plane's neighbours
/// occlude — sides `[left, right, lo, hi]` and diagonals `[lo-left, lo-right, hi-left,
/// hi-right]`, where lo/hi are down/up on a front face and near/far on a top face — and
/// the voxel whose sky it reads (the foot of its open cell).
struct Face {
    what: &'static str,
    voxel: (i64, u32, u32),
    top: bool,
    sides: [bool; 4],
    corners: [bool; 4],
    foot: (i64, u32, u32),
}

/// The crease width the shader draws on a face `rows` texels tall at `s` px a voxel.
fn crease_width(s: u32, rows: u32) -> u32 {
    ((s + 4) / 8).max(1).min((rows / 2).max(1))
}

#[test]
fn voxel_ao_is_a_crease_band_along_occluded_edges() {
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

    let no = [false; 4];
    let faces = [
        Face {
            what: "the floor left of the pillar: a band on its right",
            voxel: (5, 1, 1),
            top: true,
            sides: [false, true, false, false],
            corners: no,
            foot: (5, 1, 1),
        },
        Face {
            what: "the floor in front of the pillar: a band at its back",
            voxel: (6, 1, 0),
            top: true,
            sides: [false, false, false, true],
            corners: no,
            foot: (6, 1, 0),
        },
        Face {
            what: "the floor in the concave corner: bands on the right and at the back",
            voxel: (9, 1, 1),
            top: true,
            sides: [false, true, false, true],
            corners: no,
            foot: (9, 1, 1),
        },
        Face {
            what: "the floor diagonal to the corner block: a square in its back right corner",
            voxel: (8, 1, 1),
            top: true,
            sides: no,
            corners: [false, false, false, true],
            foot: (8, 1, 1),
        },
        Face {
            what: "the pillar's front: the floor in front puts a band along its bottom",
            voxel: (6, 2, 1),
            top: false,
            sides: [false, false, true, false],
            corners: [true, true, false, false],
            foot: (6, 1, 0),
        },
    ];

    let cfg = VoxelConfig {
        world: c.clone(),
        // 16 px: a 2-px crease, so it reaches inside the edge rows the test leaves out.
        px_per_voxel: 16,
        haze: 0.0,
        dither: 0.0,
        sky_gradient: false,
        lighting: Lighting::Lit,
        light: LightConfig {
            levels: 256,
            ambient_gain: GAIN,
            ambient_floor: 0.0,
            ao: 0.5,
            ambient_tint: 0.0,
            // No sun: every texel is its ambient rung alone.
            sun: [0.0; 3],
            sun_tint: 0.0,
            ..LightConfig::default()
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
        let bw = crease_width(s, fh);
        let mut banded = 0;
        for dy in 1..fh - 1 {
            for dx in 1..fw - 1 {
                let (l, r) = (dx < bw, dx >= fw - bw);
                let (hi, lo) = (dy < bw, dy >= fh - bw);
                let [sl, sr, slo, shi] = face.sides;
                let [cll, clr, chl, chr] = face.corners;
                let band = (l && sl) || (r && sr) || (lo && slo) || (hi && shi)
                    || (l && lo && cll) || (r && lo && clr) || (l && hi && chl) || (r && hi && chr);
                banded += usize::from(band);
                let ao = if band { 0.5 } else { 1.0 };
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
        assert!(banded > 0, "{}: the crease reaches inside the edge rows", face.what);
    }
}

/// A pillar on a flat floor under a sun from the front left: every floor texel whose ray
/// toward the sun passes through the pillar is exactly one rung darker than the texels
/// whose ray misses it.
///
/// The ladder has two rungs and no floor, AO and tints are off, so a floor texel is its
/// base times `GAIN × (round(sky) + sun)`, with the sky the core's own number at the
/// floor. The shadow is the pillar's box swept along the sun direction; texels within a
/// hair of its edge are not asserted, since which side a texel centre falls on there is
/// rounding.
#[test]
fn a_pillar_casts_its_analytic_shadow_one_rung_down() {
    const GAIN: f32 = 2.0;
    let c = Config {
        width: 16,
        height: 10,
        depth: 12,
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
    let (px, pz, top) = (5i64, 4u32, 5u32);
    for y in 2..top {
        set(px, y, pz, Material::Rock);
    }
    let sun = [-1.0f32, 1.5, -1.0];
    let cfg = VoxelConfig {
        world: c.clone(),
        px_per_voxel: 8,
        haze: 0.0,
        dither: 0.0,
        sky_gradient: false,
        lighting: Lighting::Lit,
        light: LightConfig {
            levels: 2,
            ambient_gain: GAIN,
            ambient_floor: 0.0,
            ao: 0.0,
            ambient_tint: 0.0,
            sun,
            sun_tint: 0.0,
            ..LightConfig::default()
        },
        ..VoxelConfig::default()
    };
    let proj = Projection::new(cfg.tilt_degrees, cfg.px_per_voxel, cfg.raster_height, &c).unwrap();
    let Some(rgba) = lit_frame(&cfg, proj, &world) else {
        return;
    };
    let w = usize::from(proj.raster_w);
    let rock = cubarium::present::srgb_linear(cubarium::voxel::present::ROCK_SRGB);
    let view = world.view();
    let len = sun.iter().map(|k| k * k).sum::<f32>().sqrt();
    let l = sun.map(|k| k / len);

    // Where along the ray from `p` toward the sun it is inside the pillar grown by `pad`
    // (negative shrinks it): the slab test.
    let hits = |p: [f32; 3], pad: f32| -> bool {
        let lo = [px as f32 - pad, 2.0 - pad, pz as f32 - pad];
        let hi = [px as f32 + 1.0 + pad, top as f32 + pad, pz as f32 + 1.0 + pad];
        let (mut t0, mut t1) = (0.0f32, f32::INFINITY);
        for k in 0..3 {
            let (a, b) = ((lo[k] - p[k]) / l[k], (hi[k] - p[k]) / l[k]);
            t0 = t0.max(a.min(b));
            t1 = t1.min(a.max(b));
        }
        t0 < t1
    };
    let (s, rise) = (proj.s, proj.rise);
    let (mut shadowed, mut lit) = (0, 0);
    for z in 0..c.depth {
        for x in 0..i64::from(c.width) {
            // The pillar's own column is behind the pillar on screen.
            if x == px {
                continue;
            }
            let sky = (view.sky_visibility(x, 1, z) * 255.0).round() / 255.0;
            let (col, row, _, _) = proj.top_rect(x, 1, z);
            // Row 0 is the contour row where the ground ends going back.
            for dy in 1..rise {
                for dx in 0..s {
                    let p = [
                        x as f32 + (dx as f32 + 0.5) / s as f32,
                        2.0,
                        z as f32 + 1.0 - (dy as f32 + 0.5) / rise as f32,
                    ];
                    let sun = if hits(p, -0.02) {
                        0.0
                    } else if !hits(p, 0.02) {
                        1.0
                    } else {
                        continue;
                    };
                    if sun == 0.0 {
                        shadowed += 1;
                    } else {
                        lit += 1;
                    }
                    let light = GAIN * ((sky as f32).round() + sun);
                    let want: Vec<u8> = rock
                        .iter()
                        .map(|b| (srgb(b * light) * 255.0).round() as u8)
                        .collect();
                    let (qx, qy) = ((col + dx as i32) as usize, (row + dy as i32) as usize);
                    let got = &rgba[(qy * w + qx) * 4..(qy * w + qx) * 4 + 3];
                    for k in 0..3 {
                        assert!(
                            got[k].abs_diff(want[k]) <= 2,
                            "floor ({x}, {z}) texel ({dx}, {dy}): {got:?}, want {want:?} \
                             ({})",
                            if sun == 0.0 { "in the shadow" } else { "in the sun" }
                        );
                    }
                }
            }
        }
    }
    assert!(shadowed > 50, "the fixture puts texels in the shadow: {shadowed}");
    assert!(lit > 500, "and most of the floor in the sun: {lit}");
}

/// One headless lit frame of `world`, once its sky plane has arrived; `None` (printed)
/// without a device.
fn lit_frame(cfg: &VoxelConfig, proj: Projection, world: &World) -> Option<Vec<u8>> {
    let mut gpu = match VoxelGpuSink::new(
        cfg,
        proj,
        VoxelGpuSinkOptions {
            target: GpuTargetKind::Headless,
            ..VoxelGpuSinkOptions::default()
        },
    ) {
        Ok(sink) => sink,
        Err(e) => {
            println!("skipped: no Vulkan device for the voxel GPU renderer ({e:#})");
            return None;
        }
    };
    let (flora, fauna) = (
        Flora::new(FloraConfig::default()),
        Fauna::new(FaunaConfig::default()),
    );
    let started = std::time::Instant::now();
    loop {
        assert!(gpu.stage_world(world, &flora, &fauna));
        if !gpu.light_pending() {
            break;
        }
        assert!(started.elapsed().as_secs() < 30, "the sky plane never arrived");
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    gpu.render().expect("one GPU frame");
    Some(gpu.read_raster().expect("the raster reads back"))
}

/// Package W's boundary rule: the lit tier changes the water's colour only inside the
/// flat tier's water pixels. A pixel is "water" in a tier where its colour differs between
/// the world with its free water and the same world dry; on a fixture with a pool (and a
/// brim-full cell in it, open above), a falling column and a thin sheet, `flat` and `lit`
/// (with the sun, reflection and ripples at their defaults) mark exactly the same pixels.
#[test]
fn lit_water_marks_exactly_the_flat_tiers_water_pixels() {
    let c = Config {
        width: 16,
        height: 10,
        depth: 6,
        ..Config::default()
    };
    let mut dry = World::empty(c.clone());
    let mut set = |x: i64, y: u32, z: u32, material: Material| {
        dry.apply(Command::SetMaterial { x, y, z, material });
    };
    for z in 0..c.depth {
        for x in 0..i64::from(c.width) {
            set(x, 0, z, Material::Bedrock);
            set(x, 1, z, Material::Rock);
        }
    }
    // A basin two deep at x 3..7, z 1..4, walled at y 2..3; a ledge at x 11 for the falling
    // column to leave from.
    for z in 0..5 {
        for x in 2..9 {
            let inside = (3..8).contains(&x) && (1..4).contains(&z);
            if !inside {
                set(x, 2, z, Material::Rock);
                set(x, 3, z, Material::Rock);
            }
        }
    }
    for y in 2..7 {
        set(12, y, 3, Material::Rock);
    }
    let cell = c.voxel_m.powi(3);
    let mut wet = dry.clone();
    let mut add = |x: i64, y: u32, z: u32, fraction: f64| {
        wet.apply(Command::AddWater {
            x,
            y,
            z,
            volume_m3: fraction * cell,
        });
    };
    for z in 1..4 {
        for x in 3..8 {
            add(x, 2, z, 1.0);
            // The top layer: part-full, and brim-full at (5, 3, 2) with air over it.
            add(x, 3, z, if (x, z) == (5, 2) { 1.0 } else { 0.6 });
        }
    }
    // A falling column in front of the ledge: every other cell, over air.
    for y in [3u32, 5, 7] {
        add(11, y, 3, 0.05);
    }
    // A thin sheet on the open floor.
    for x in 10..14 {
        add(x, 2, 1, 0.1);
    }

    let mut masks = Vec::new();
    for lighting in [Lighting::Flat, Lighting::Lit] {
        let cfg = VoxelConfig {
            world: c.clone(),
            px_per_voxel: 8,
            lighting,
            ..VoxelConfig::default()
        };
        let proj =
            Projection::new(cfg.tilt_degrees, cfg.px_per_voxel, cfg.raster_height, &c).unwrap();
        let (Some(with), Some(without)) = (
            lit_frame(&cfg, proj, &wet),
            lit_frame(&cfg, proj, &dry),
        ) else {
            return;
        };
        let mask: Vec<bool> = with
            .chunks(4)
            .zip(without.chunks(4))
            .map(|(a, b)| a[..3] != b[..3])
            .collect();
        masks.push(mask);
    }
    let count = masks[0].iter().filter(|&&m| m).count();
    assert!(count > 300, "the fixture's water covers only {count} pixels");
    let differ = masks[0].iter().zip(&masks[1]).filter(|(a, b)| a != b).count();
    assert_eq!(differ, 0, "{differ} of {count} water pixels differ between flat and lit");
}
