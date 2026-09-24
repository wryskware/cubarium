//! `voxel_specimens`: every organism's baked model on one flat strip, drawn by the CPU
//! voxel presenter, for judging the look without a seed's luck, wilt or depth haze.
//!
//! ```text
//! cargo run -p cubarium --release --example voxel_specimens -- OUT.png [--glyphs] [--wilt W]
//!     [--gpu [--px N] [--textures DIR | --textures-only DIR]]
//! ```
//!
//! `--gpu` draws the same strip with the GPU renderer instead (headless), at `--px`
//! (default 6) with the face textures in `assets/voxel-textures`. `--textures DIR` puts
//! `DIR` over that set, so a scratch directory holding only a few candidate species
//! (`DIR/masters/species/<species>/…`) previews them among everything else;
//! `--textures-only DIR` draws with `DIR` alone (a directory that does not exist draws
//! untextured). The CPU picture is always at 6.
//!
//! Each producer stands at three sizes (wood at 10 %, 40 % and 100 % of its maximum),
//! left to right, in the front slab. Bloomcrown and lanternberry get a fourth, adult and
//! ripe. Both founders stand at the end, facing each way. Every stand is drawn at full
//! moisture (or at `--wilt W`, a moisture of `1 − W`), with full foliage. The picture is
//! written at 2× nearest, the panel's own upscale.

use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{Context, Result, bail};

use cubarium::sink::gpu::{VoxelGpuSink, VoxelGpuSinkOptions};
use cubarium::voxel::model::ModelLibrary;
use cubarium::voxel::present::VoxelPresenter;
use cubarium::voxel::project::Projection;
use cubarium::voxel::{OrganismLook, VoxelConfig};
use cubarium_render::Canvas;
use cubarium_surface::{Scale, Topology};
use cubarium_voxel::{Command as VoxelCommand, Config, Material, World};
use cubarium_voxel_fauna::{Command as FaunaCommand, Fauna, FaunaConfig, Founder, StartingStores};
use cubarium_voxel_flora::{Command as FloraCommand, Flora, FloraConfig, FloraView, Species};

/// Voxels between one specimen's crown edge and the next one's.
const GAP: i64 = 3;

fn main() -> Result<()> {
    let mut out = None;
    let mut glyphs = false;
    let mut wilt = 0.0f64;
    let mut gpu = false;
    let mut px = 6u32;
    let mut textures: Option<PathBuf> = None;
    let mut layered = false;
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        match a.as_str() {
            "--glyphs" => glyphs = true,
            "--gpu" => gpu = true,
            "--px" => px = args.next().context("--px N")?.parse()?,
            "--textures" => {
                textures = Some(PathBuf::from(args.next().context("--textures DIR")?));
                layered = true;
            }
            "--textures-only" => {
                textures = Some(PathBuf::from(args.next().context("--textures-only DIR")?));
                layered = false;
            }
            "--wilt" => wilt = args.next().context("--wilt W")?.parse()?,
            _ if out.is_none() => out = Some(PathBuf::from(a)),
            _ => bail!("unexpected argument {a}"),
        }
    }
    let out = out.context("usage: voxel_specimens OUT.png [--glyphs] [--wilt W]")?;

    let flora_cfg = FloraConfig::default();
    // Lay the specimens out first, so the world is exactly as wide as they need.
    let voxel_m = 0.125;
    let mut plan: Vec<(Species, f64, bool)> = Vec::new();
    for species in Species::ALL {
        for f in [0.1, 0.4, 1.0] {
            plan.push((species, f, false));
        }
        if matches!(species, Species::Bloomcrown | Species::Lanternberry) {
            plan.push((species, 1.0, true));
        }
    }
    let mut x = GAP;
    let mut columns = Vec::new();
    for &(species, _, _) in &plan {
        let r =
            (flora_cfg.species(species).crown_radius_m_at(f64::INFINITY) / voxel_m).ceil() as i64;
        let r = r.max(1);
        columns.push(x + r);
        x += 2 * r + 1 + GAP;
    }
    let animal_x: Vec<i64> = (0..4).map(|k| x + 3 + k * 9).collect();
    let width = (x + 4 * 9 + 6) as u32;

    let world_cfg = Config {
        width,
        height: 40,
        depth: 6,
        voxel_m,
        ..Config::default()
    };
    let mut world = World::empty(world_cfg.clone());
    for z in 0..world_cfg.depth {
        for x in 0..i64::from(world_cfg.width) {
            world.apply(VoxelCommand::SetMaterial {
                x,
                y: 0,
                z,
                material: Material::Bedrock,
            });
            for y in 1..3 {
                world.apply(VoxelCommand::SetMaterial {
                    x,
                    y,
                    z,
                    material: Material::Soil,
                });
            }
        }
    }

    let mut flora = Flora::new(flora_cfg.clone());
    for (&(species, f, _), &x) in plan.iter().zip(&columns) {
        let wood = flora_cfg.species(species).wood_max * f;
        if !flora.apply(
            &world,
            FloraCommand::Seed {
                x,
                z: 1,
                species,
                wood,
            },
        ) {
            bail!("could not seed {species:?} at x={x}");
        }
    }
    let mut fauna = Fauna::new(FaunaConfig::default());
    for (k, &x) in animal_x.iter().enumerate() {
        let founder = if k < 2 {
            Founder::Browser
        } else {
            Founder::Blind
        };
        let heading_rad = if k % 2 == 0 { 1.5708 } else { -1.5708 };
        if !fauna.apply(
            &world,
            FaunaCommand::IntroduceFounder {
                x,
                z: 1,
                founder,
                stores: StartingStores::FULL,
                heading_rad,
            },
        ) {
            bail!("could not place {} at x={x}", founder.name());
        }
    }

    // Every stand healthy (or at the asked wilt), full of foliage, and the ripe copies
    // with a whole package in their parcel.
    let view = flora.view();
    let mut stands = view.stands.to_vec();
    for s in &mut stands {
        let sc = flora_cfg.species(s.species);
        s.moisture = (1.0 - wilt).clamp(0.0, 1.0);
        let ripe = plan
            .iter()
            .zip(&columns)
            .any(|(&(sp, _, ripe), &x)| ripe && sp == s.species && i64::from(s.site.x) == x);
        s.parcel = if ripe { sc.propagule_package() } else { 0.0 };
    }
    let view = FloraView {
        stands: &stands,
        crowns: cubarium_voxel_flora::CrownCache::none(),
        ..view
    };

    let cfg = VoxelConfig {
        world: world_cfg.clone(),
        raster_height: 0,
        organisms: if glyphs {
            OrganismLook::Glyphs
        } else {
            OrganismLook::Models
        },
        ..VoxelConfig::default()
    };
    let models: Option<Arc<ModelLibrary>> = if glyphs {
        None
    } else {
        Some(Arc::new(
            ModelLibrary::load(&cfg.models_dir, voxel_m).context("the baked models")?,
        ))
    };
    if gpu {
        let under = layered.then(|| cfg.textures_dir.clone());
        let cfg = VoxelConfig {
            px_per_voxel: px,
            textures: true,
            textures_dir: textures.unwrap_or_else(|| cfg.textures_dir.clone()),
            ..cfg
        };
        let proj = Projection::new(cfg.tilt_degrees, px, 0, &world_cfg)?;
        let mut sink = VoxelGpuSink::new(
            &cfg,
            proj,
            VoxelGpuSinkOptions {
                models,
                textures_under: under,
                ..Default::default()
            },
        )?;
        anyhow::ensure!(
            sink.stage_view(&world.view(), view, fauna.view()),
            "no staging buffer"
        );
        sink.render()?;
        let rgba = sink.read_raster()?;
        let rgb: Vec<u8> = rgba.chunks_exact(4).flat_map(|p| [p[0], p[1], p[2]]).collect();
        return write_2x(&out, u32::from(proj.raster_w), u32::from(proj.raster_h), &rgb);
    }
    let proj = Projection::new(cfg.tilt_degrees, 6, 0, &world_cfg)?;
    let topology = Topology::Ring {
        w: proj.raster_w,
        h: proj.raster_h,
    };
    let mut canvas = Canvas::new(topology, Scale::ONE);
    let mut raster = cube_proto::Raster::black(proj.raster_w, proj.raster_h);
    VoxelPresenter::new(
        VoxelConfig {
            px_per_voxel: 6,
            ..cfg
        },
        proj,
    )
    .with_models(models)
    .draw_with_fauna(&world.view(), view, Some(fauna.view()), &mut canvas);
    canvas.encode_raster(&mut raster);

    let (w, h) = (u32::from(proj.raster_w), u32::from(proj.raster_h));
    write_2x(&out, w, h, raster.as_bytes())?;
    println!(
        "{} specimens and {} animals on a {}x{}x{} strip -> {} ({}x{})",
        plan.len(),
        animal_x.len(),
        world_cfg.width,
        world_cfg.height,
        world_cfg.depth,
        out.display(),
        w * 2,
        h * 2
    );
    Ok(())
}

/// Write `src` (`w × h` RGB8) at 2× nearest, the panel's own upscale.
fn write_2x(out: &std::path::Path, w: u32, h: u32, src: &[u8]) -> Result<()> {
    let mut rgb = vec![0u8; (w * 2 * h * 2 * 3) as usize];
    for y in 0..h * 2 {
        for x in 0..w * 2 {
            let s = (((y / 2) * w + x / 2) * 3) as usize;
            let d = ((y * w * 2 + x) * 3) as usize;
            rgb[d..d + 3].copy_from_slice(&src[s..s + 3]);
        }
    }
    let file = std::fs::File::create(&out).with_context(|| format!("{}", out.display()))?;
    let mut enc = png::Encoder::new(std::io::BufWriter::new(file), w * 2, h * 2);
    enc.set_color(png::ColorType::Rgb);
    enc.set_depth(png::BitDepth::Eight);
    enc.write_header()?.write_image_data(&rgb)?;
    Ok(())
}
