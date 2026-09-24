//! `vine_fixture`: a hand-built rock wall under latticevine, drawn by the GPU tile layer
//! at several `px_per_voxel` and by the CPU presenter's flat cells, for judging the look
//! (`design/handoffs/latticevine-visual-2026-09-24.md`).
//!
//! ```text
//! cargo run -p cubarium --release --example vine_fixture -- OUT_DIR [--px 4,6,12] [--no-vines]
//!     [--lit]
//! ```
//!
//! The wall faces the camera. On it: a vine climbing from a foot pocket, thinner low down
//! (a grazing line) and bare at its ragged edge; a vine hanging from a soil ledge on the
//! wall's top; a dormant vine; and a pillar standing out from the wall whose vine wraps
//! its `+x` side and runs under the overhang on top of it. Three faces are posed in bud,
//! flower and fruit, and a stand at the climbing vine's foot claims two of its cells.
//! Writes `gpu-<px>px.png` per level — the default look, textures off, the latticevine as
//! plain voxel cells — then `gpu-<px>px-textured.png` with the experimental textures and
//! the vine tile layer, and `cpu-6px.png`; each upscaled to about 1400 px wide.
//! `--no-vines` draws the same wall with no cover. `--lit` draws the GPU pictures in the
//! lit tier (package L), named `lit-gpu-*.png`.
//!
//! `--terrarium WORLD.voxel` draws a saved world instead: its terrain, with
//! [`TERRARIUM_FOUNDERS`] latticevine founders rooted by the flora's own founder rule and
//! each grown by hand over up to [`TERRARIUM_FACES`] faces (the simulation takes hours to
//! cover a wall), a few spurs posed. Written as `terrarium-*.png`.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{Context, Result, bail};

use cubarium::sink::gpu::{VoxelGpuSink, VoxelGpuSinkOptions};
use cubarium::voxel::model::ModelLibrary;
use cubarium::voxel::present::VoxelPresenter;
use cubarium::voxel::project::Projection;
use cubarium::voxel::VoxelConfig;
use cubarium_render::Canvas;
use cubarium_surface::{Scale, Topology};
use cubarium_voxel::{Command as VoxelCommand, Config, Material, World};
use cubarium_voxel_fauna::{Fauna, FaunaConfig};
use cubarium_voxel_flora::{
    Command as FloraCommand, Face, FaceDir, FaceDraw, Flora, FloraConfig, Site, Species, SpurPhase,
    VineId, VineSeed,
};

const W: u32 = 44;
const H: u32 = 22;
const D: u32 = 8;
/// The wall's front row of voxels: its `−z` faces look at the camera.
const WALL_Z: u32 = 4;
const WALL_TOP: u32 = 15;

fn set(world: &mut World, x: i64, y: u32, z: u32, material: Material) {
    world.apply(VoxelCommand::SetMaterial { x, y, z, material });
}

fn build_world() -> World {
    let c = Config {
        width: W,
        height: H,
        depth: D,
        voxel_m: 0.125,
        ..Config::default()
    };
    let mut w = World::empty(c);
    for z in 0..D {
        for x in 0..i64::from(W) {
            set(&mut w, x, 0, z, Material::Bedrock);
            for y in 1..3 {
                set(&mut w, x, y, z, Material::Soil);
            }
            // The wall: rock from the floor to its top, a soil cap along its crest where
            // the hanging vine roots.
            if (4..=32).contains(&x) && z >= WALL_Z {
                for y in 3..=WALL_TOP {
                    let m = if y == WALL_TOP { Material::Soil } else { Material::Rock };
                    set(&mut w, x, y, z, m);
                }
            }
        }
    }
    // A pillar standing out from the wall's right end, and an overhang on top of it that
    // reaches towards the camera.
    for y in 3..=12 {
        for x in 36..=37 {
            set(&mut w, x, y, 3, Material::Rock);
        }
    }
    for x in 36..=38 {
        set(&mut w, x, 12, 2, Material::Rock);
    }
    w
}

/// Cover `want` for `vine`, in as many passes as the adjacency needs; returns how many
/// faces were refused.
fn grow(flora: &mut Flora, world: &World, vine: VineId, mut want: Vec<(Face, f64)>) -> usize {
    loop {
        let before = want.len();
        want.retain(|&(f, leaf)| !flora.cover_face(world, vine, f, leaf));
        if want.len() == before {
            return want.len();
        }
    }
}

fn wall(x: u32, y: u32) -> Face {
    Face::new(x, y, WALL_Z, FaceDir::NegZ)
}

fn build_flora(world: &World) -> Result<(Flora, Vec<FaceDraw>)> {
    let mut flora = Flora::new(FloraConfig::default());
    let seed = |flora: &mut Flora, root: Site, first: Face| {
        flora
            .seed_vine(
                world,
                VineSeed {
                    root,
                    first,
                    reserve: 1.0,
                    lineage: None,
                },
            )
            .with_context(|| format!("rooting a vine at {root:?} on {first:?}"))
    };
    let mut refused = 0;

    // A: climbs from the foot at x = 10, a lattice 7 wide and 9 tall; thin below y = 5
    // (the grazing line), bare along a ragged right edge.
    let a = seed(&mut flora, Site { x: 10, y: 2, z: 3 }, wall(10, 3))?;
    let mut want = Vec::new();
    for y in 3..12u32 {
        for x in 7..14u32 {
            let ragged = (x + y * 3) % 5 == 0 && y > 6;
            if ragged || (x == 13 && y > 9) {
                continue;
            }
            let leaf = if x >= 12 {
                0.1
            } else if y < 5 {
                0.4
            } else {
                1.0
            };
            want.push((wall(x, y), leaf));
        }
    }
    refused += grow(&mut flora, world, a, want);

    // B: hangs from the soil crest at x = 24, down the wall.
    let b = seed(
        &mut flora,
        Site {
            x: 24,
            y: WALL_TOP,
            z: WALL_Z,
        },
        wall(24, WALL_TOP),
    )?;
    let mut want = Vec::new();
    for y in 7..=WALL_TOP {
        for x in 21..28u32 {
            let reach = WALL_TOP - y;
            if x.abs_diff(24) > 1 + reach / 2 {
                continue;
            }
            want.push((wall(x, y), if y < 9 { 0.35 } else { 1.0 }));
        }
    }
    refused += grow(&mut flora, world, b, want);

    // C: a dormant vine at the foot, x = 17.
    let c = seed(&mut flora, Site { x: 17, y: 2, z: 3 }, wall(17, 3))?;
    let mut want = Vec::new();
    for y in 3..8u32 {
        for x in 16..19u32 {
            want.push((wall(x, y), 0.8));
        }
    }
    refused += grow(&mut flora, world, c, want);

    // P: the pillar's own vine, from its foot, up its front, round its `+x` side, and
    // under the overhang.
    let p = seed(
        &mut flora,
        Site { x: 36, y: 2, z: 2 },
        Face::new(36, 3, 3, FaceDir::NegZ),
    )?;
    let mut want = Vec::new();
    for y in 3..=11u32 {
        for x in 36..=37u32 {
            want.push((Face::new(x, y, 3, FaceDir::NegZ), 1.0));
        }
        want.push((Face::new(37, y, 3, FaceDir::PosX), 0.8));
    }
    for x in 36..=38u32 {
        want.push((Face::new(x, 12, 2, FaceDir::Down), 0.9));
    }
    refused += grow(&mut flora, world, p, want);
    if refused > 0 {
        eprintln!("vine_fixture: {refused} faces refused by the cover's adjacency rules");
    }

    // A stand at the climbing vine's foot: the organism wins the cells it stands in.
    if !flora.apply(
        world,
        FloraCommand::Seed {
            x: 8,
            z: 3,
            species: Species::Bloomcrown,
            wood: FloraConfig::default().species(Species::Bloomcrown).wood_max * 0.4,
        },
    ) {
        bail!("could not seed the stand at the foot");
    }

    // The poses the simulation would take hours to reach: C dormant, spurs in each phase.
    let mut draws = flora.view().cover.draw();
    for d in &mut draws {
        if d.owner == c {
            d.dormant = true;
        }
        let at = (d.face.x, d.face.y, d.face.dir);
        d.spur = match at {
            (9, 8, FaceDir::NegZ) | (24, 11, FaceDir::NegZ) => SpurPhase::Flower,
            (11, 7, FaceDir::NegZ) | (23, 12, FaceDir::NegZ) => SpurPhase::Fruit,
            (8, 10, FaceDir::NegZ) | (25, 13, FaceDir::NegZ) => SpurPhase::Bud,
            (10, 5, FaceDir::NegZ) => SpurPhase::Spent,
            _ => SpurPhase::Bare,
        };
    }
    Ok((flora, draws))
}

/// Founders rooted in `--terrarium` mode.
const TERRARIUM_FOUNDERS: u32 = 14;
/// Faces each founder is grown over by hand.
const TERRARIUM_FACES: usize = 60;

fn terrarium_flora(world: &World) -> (Flora, Vec<FaceDraw>) {
    let mut cfg = FloraConfig::for_voxel_size(world.config().voxel_m);
    cfg.latticevine.founders = TERRARIUM_FOUNDERS;
    let mut flora = Flora::new(cfg);
    let rooted = flora.seed_vine_founders(world);
    let wc = world.config().clone();
    let vines: Vec<(VineId, Face)> = flora
        .view()
        .cover
        .vines()
        .iter()
        .map(|v| (v.id, v.root_face))
        .collect();
    let mut rng = 0x2545_F491_4F6C_DD1Du64;
    let mut next = move || {
        rng ^= rng << 13;
        rng ^= rng >> 7;
        rng ^= rng << 17;
        rng
    };
    for (id, root) in vines {
        // Breadth first from the root face, in the face's own plane, upward first.
        let mut frontier = std::collections::VecDeque::from([root]);
        let mut grown = 1;
        while let Some(f) = frontier.pop_front() {
            if grown >= TERRARIUM_FACES {
                break;
            }
            for (dx, dy, dz) in cubarium::voxel::vine::plane_steps(f.dir) {
                let (y, z) = (i64::from(f.y) + dy, i64::from(f.z) + dz);
                if y < 0 || y >= i64::from(wc.height) || z < 0 || z >= i64::from(wc.depth) {
                    continue;
                }
                let x = (i64::from(f.x) + dx).rem_euclid(i64::from(wc.width)) as u32;
                let n = Face::new(x, y as u32, z as u32, f.dir);
                let leaf = match next() % 10 {
                    0 => 0.1,
                    1..=3 => 0.4,
                    _ => 1.0,
                };
                if grown < TERRARIUM_FACES && flora.cover_face(world, id, n, leaf) {
                    grown += 1;
                    frontier.push_back(n);
                }
            }
        }
    }
    let mut draws = flora.view().cover.draw();
    for d in &mut draws {
        d.spur = match next() % 16 {
            0 => SpurPhase::Flower,
            1 => SpurPhase::Fruit,
            2 => SpurPhase::Bud,
            _ => SpurPhase::Bare,
        };
    }
    println!(
        "vine_fixture: terrarium: {rooted} founders rooted, {} faces covered",
        draws.len()
    );
    (flora, draws)
}

fn write_zoomed(out: &Path, w: u32, h: u32, rgb: &[u8]) -> Result<()> {
    let k = (1400 / w).max(1);
    let (zw, zh) = (w * k, h * k);
    let mut big = vec![0u8; (zw * zh * 3) as usize];
    for y in 0..zh {
        for x in 0..zw {
            let s = (((y / k) * w + x / k) * 3) as usize;
            let d = ((y * zw + x) * 3) as usize;
            big[d..d + 3].copy_from_slice(&rgb[s..s + 3]);
        }
    }
    let file = std::fs::File::create(out).with_context(|| format!("{}", out.display()))?;
    let mut enc = png::Encoder::new(std::io::BufWriter::new(file), zw, zh);
    enc.set_color(png::ColorType::Rgb);
    enc.set_depth(png::BitDepth::Eight);
    enc.write_header()?.write_image_data(&big)?;
    Ok(())
}

fn main() -> Result<()> {
    let mut dir: Option<PathBuf> = None;
    let mut pxs = vec![4u32, 6, 12];
    let mut no_vines = false;
    let mut lit = false;
    let mut terrarium: Option<PathBuf> = None;
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        match a.as_str() {
            "--px" => {
                pxs = args
                    .next()
                    .context("--px 4,6,12")?
                    .split(',')
                    .map(|s| s.trim().parse::<u32>())
                    .collect::<Result<_, _>>()?
            }
            "--no-vines" => no_vines = true,
            "--lit" => lit = true,
            "--terrarium" => {
                terrarium = Some(PathBuf::from(args.next().context("--terrarium WORLD.voxel")?))
            }
            _ if dir.is_none() => dir = Some(PathBuf::from(a)),
            _ => bail!("unexpected argument {a}"),
        }
    }
    let dir = dir.context("usage: vine_fixture OUT_DIR [--px 4,6,12] [--no-vines]")?;
    std::fs::create_dir_all(&dir)?;

    let (world, prefix) = match &terrarium {
        Some(path) => (
            World::load(&std::fs::read(path).with_context(|| format!("{}", path.display()))?)?,
            "terrarium-",
        ),
        None => (build_world(), ""),
    };
    let (flora, mut draws) = match terrarium {
        Some(_) => terrarium_flora(&world),
        None => build_flora(&world)?,
    };
    if no_vines {
        draws.clear();
    }
    let fauna = Fauna::new(FaunaConfig::default());
    let terrarium_mode = !prefix.is_empty();
    let wc = world.config().clone();
    let base = VoxelConfig {
        world: wc.clone(),
        raster_height: 0,
        haze: if terrarium_mode { 0.55 } else { 0.25 },
        ..VoxelConfig::default()
    };
    let models = Some(Arc::new(
        ModelLibrary::load(&base.models_dir, wc.voxel_m).context("the baked models")?,
    ));
    println!(
        "vine_fixture: {} covered faces in {} vines ({} poses)",
        draws.len(),
        flora.view().cover.vines().len(),
        draws.iter().filter(|d| d.spur != SpurPhase::Bare || d.dormant).count()
    );

    for (textures, &px) in [false, true].into_iter().flat_map(|t| pxs.iter().map(move |p| (t, p))) {
        let cfg = VoxelConfig {
            px_per_voxel: px,
            textures,
            lighting: if lit {
                cubarium::voxel::Lighting::Lit
            } else {
                cubarium::voxel::Lighting::Flat
            },
            ..base.clone()
        };
        let proj = Projection::new(cfg.tilt_degrees, px, 0, &wc)?;
        let mut sink = VoxelGpuSink::new(
            &cfg,
            proj,
            VoxelGpuSinkOptions {
                models: models.clone(),
                ..Default::default()
            },
        )?;
        sink.set_cover_draws(Some(draws.clone()));
        anyhow::ensure!(
            sink.stage_view(&world.view(), flora.view(), fauna.view()),
            "no staging buffer"
        );
        // The lit tier's sky plane is computed off this thread.
        while sink.light_pending() {
            std::thread::sleep(std::time::Duration::from_millis(5));
            anyhow::ensure!(
                sink.stage_view(&world.view(), flora.view(), fauna.view()),
                "no staging buffer"
            );
        }
        sink.render()?;
        let rgba = sink.read_raster()?;
        let rgb: Vec<u8> = rgba.chunks_exact(4).flat_map(|p| [p[0], p[1], p[2]]).collect();
        let (w, h) = (u32::from(proj.raster_w), u32::from(proj.raster_h));
        let tier = if lit { "lit-" } else { "" };
        let name = if textures {
            format!("{prefix}{tier}gpu-{px}px-textured.png")
        } else {
            format!("{prefix}{tier}gpu-{px}px.png")
        };
        write_zoomed(&dir.join(name), w, h, &rgb)?;
    }

    let cpu_px = if terrarium_mode { 4 } else { 6 };
    let proj = Projection::new(base.tilt_degrees, cpu_px, 0, &wc)?;
    let mut canvas = Canvas::new(
        Topology::Ring {
            w: proj.raster_w,
            h: proj.raster_h,
        },
        Scale::ONE,
    );
    let mut raster = cube_proto::Raster::black(proj.raster_w, proj.raster_h);
    let mut cpu = VoxelPresenter::new(
        VoxelConfig {
            px_per_voxel: cpu_px,
            ..base.clone()
        },
        proj,
    )
    .with_models(models);
    cpu.set_cover_draws(Some(draws));
    cpu.draw_with_fauna(&world.view(), flora.view(), Some(fauna.view()), &mut canvas);
    canvas.encode_raster(&mut raster);
    write_zoomed(
        &dir.join(format!("{prefix}cpu-{cpu_px}px.png")),
        u32::from(proj.raster_w),
        u32::from(proj.raster_h),
        raster.as_bytes(),
    )?;
    println!("vine_fixture: -> {}", dir.display());
    Ok(())
}
