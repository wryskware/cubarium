//! Deterministic headless FOG-OCCLUDE fixtures and weather-only GPU timestamps.
//! Uses the desktop's projection, palette and baked organism models. The bank is a
//! hand-built input, so main/branch captures differ only in their renderer.
//! `--out DIR --case animals|grove|mixed|no-weather|no-occluder --px 6 --frames 300`
use anyhow::{Context, Result, bail, ensure};
use cubarium::voxel::model::{PALETTE, Tag};
use cubarium::voxel::{load_config, load_models, project::Projection};
use cubarium_gpu::figures::{
    CLASS_NONE, FigureFrame, FigureInstance, FigureTile, FigureVoxel, PaletteEntry, open_faces,
};
use cubarium_gpu::plants::{PlantRec, SETTLED, pack_cell};
use cubarium_gpu::target::{Headless, write_png};
use cubarium_gpu::voxel::{
    FrameClock, PART_CROWN, PART_TRUNK, SLOW_PLANES, VoxelStaging, VoxelTextures, write_upload,
};
use cubarium_gpu::weather::Weather;
use cubarium_gpu::{Gpu, VoxelRenderer, VoxelStyle, VoxelTexel};
use cubarium_voxel_fauna::Founder;
use cubarium_voxel_flora::Species;
use std::io::Write;
use std::path::{Path, PathBuf};

fn main() -> Result<()> {
    let mut out = PathBuf::new();
    let mut case = String::from("animals");
    let mut px = 6;
    let mut frames = 1;
    let mut it = std::env::args().skip(1);
    while let Some(k) = it.next() {
        let v = it.next().context("argument needs a value")?;
        match k.as_str() {
            "--out" => out = v.into(),
            "--case" => case = v,
            "--px" => px = v.parse()?,
            "--frames" => frames = v.parse()?,
            _ => bail!("unknown argument {k}"),
        }
    }
    ensure!(
        !out.as_os_str().is_empty() && frames > 0,
        "--out DIR and positive --frames required"
    );
    ensure!(
        ["animals", "grove", "mixed", "no-weather", "no-occluder"].contains(&case.as_str()),
        "unknown case {case}"
    );
    std::fs::create_dir_all(&out)?;
    let mut cfg = load_config(Path::new("config/desktop/terrarium.toml"))?;
    cfg.px_per_voxel = px;
    let proj = Projection::new(cfg.tilt_degrees, px, cfg.raster_height, &cfg.sim.world)?;
    let mut p = cubarium::sink::gpu::voxel::params_of(&cfg, proj, true);
    p.lit = true;
    p.effects.volumetric = false;
    let gpu = Gpu::open(&[])?;
    eprintln!(
        "device={}, case={case}, px={px}, raster={}x{}",
        gpu.name, p.raster_w, p.raster_h
    );
    let lib = load_models(&cfg).context("fixture requires the normal baked models")?;
    let mut r = VoxelRenderer::new(&gpu, p, &VoxelTextures::empty(p.s, p.rise))?;
    let mut world = vec![VoxelTexel::default(); p.voxel_count()];
    // A low bank across the back of the strip. Near objects stand above its fog: the
    // crown/animal and the low air behind it share a projected ray, not a world depth.
    for z in 0..p.depth {
        let top = if z < 12 { 4 } else { 2 + (z - 12) / 6 };
        for y in 0..top {
            for x in 0..p.width {
                world[VoxelStaging::index(p.width, p.height, x, y, z)] =
                    VoxelTexel::pack(3, 0, 0.0, true, 0.0, 0);
            }
        }
    }
    let colours: Vec<_> = PALETTE
        .iter()
        .map(|(_, rgb)| cubarium::present::srgb_linear(*rgb))
        .collect();
    let mut plant_cells = vec![];
    let mut plants = vec![PlantRec::default()];
    if case == "grove" || case == "mixed" || case == "no-weather" {
        ensure!(p.plants.is_some(), "desktop config must use the plant pass");
        let tree = lib.plant(Species::Vaulttree).context("vaulttree model")?;
        for (j, root_x) in [50, 78, 112, 144, 180, 210].into_iter().enumerate() {
            let cells = tree.select(4.0, j as u64).context("vaulttree size")?;
            let plant_id = plants.len() as u32;
            plants.push(PlantRec {
                x: root_x as f32,
                amp: 0.12,
                lag: 0.0,
            });
            for c in cells {
                let [dx, dy, dz] = c.offset;
                let (x, y, z) = (root_x + dx, 4 + dy, 8 + dz);
                if x < 0
                    || y < 0
                    || z < 0
                    || x >= p.width as i32
                    || y >= p.height as i32
                    || z >= p.depth as i32
                {
                    continue;
                }
                let part = if c.tag == Tag::Trunk {
                    PART_TRUNK
                } else {
                    PART_CROWN
                };
                let texel = VoxelTexel::pack(0, part, 0.0, true, 0.0, c.material);
                world[VoxelStaging::index(p.width, p.height, x as u32, y as u32, z as u32)] = texel;
                plant_cells.push(pack_cell(
                    [x as u32, y as u32, z as u32],
                    plant_id,
                    (dy as f32 / 32.0).clamp(0.0, 1.0),
                    false,
                    SETTLED,
                    0,
                ));
            }
        }
        eprintln!(
            "vaulttrees={}, plant cells={}",
            plants.len() - 1,
            plant_cells.len()
        );
    }
    ensure!(
        r.stage([1; SLOW_PLANES], |st| {
            write_upload(
                &world, p.width, p.height, p.depth, st.owed, st.voxels, st.carried,
            );
            st.roof.fill(0);
            st.sky.fill(255);
            st.canopy.fill(0);
            st.glow.fill(0);
            cubarium::voxel::appearance::write_atlas(p.s, p.rise, st.glyphs);
            for (i, &rgb) in colours.iter().enumerate() {
                st.styles[i] = VoxelStyle::new(rgb, rgb, rgb);
            }
            if let Some(list) = st.plants {
                list.cells = plant_cells;
                list.plants = plants;
            }
        }),
        "staging failed"
    );
    if case == "animals" || case == "mixed" || case == "no-weather" {
        let model = lib.animal(Founder::Browser).context("frondgrazer model")?;
        let cells = model.select(0.9).context("frondgrazer size")?;
        let offsets: Vec<[i8; 3]> = cells
            .iter()
            .map(|c| c.offset.map(|v| i8::try_from(v).expect("baked cell range")))
            .collect();
        let voxels: Vec<_> = offsets
            .iter()
            .zip(cells)
            .zip(open_faces(&offsets))
            .map(|((&c, m), open)| FigureVoxel::new(c, m.material, CLASS_NONE, false, open, 1))
            .collect();
        ensure!(r.put_figure_models(0, &voxels), "model buffer capacity");
        let radius = cells
            .iter()
            .flat_map(|c| c.offset)
            .map(|v| v.abs() as f32)
            .fold(1.0, f32::max)
            * 1.8;
        r.set_figures(FigureFrame {
            figures: [42.15, 72.35, 105.5, 136.75, 170.25, 205.4]
                .into_iter()
                .enumerate()
                .map(|(j, x)| {
                    let turn = j as f32 * 0.31;
                    let (sn, cs) = turn.sin_cos();
                    FigureInstance {
                        id: j as u64,
                        a: FigureTile {
                            first: 0,
                            count: voxels.len() as u32,
                            pos: [x, 4.0, 3.5 + j as f32 * 2.0],
                            axes: [[cs, 0.0, sn], [0.0, 1.0, 0.0], [-sn, 0.0, cs]],
                            centre: [0.0, 2.0, 0.0],
                            radius,
                            pose: [0.0; 4],
                            palette: 0,
                            bones: None,
                        },
                        b: None,
                        weight_b: 0.0,
                        alpha: 1.0,
                        highlight: 0.0,
                    }
                })
                .collect(),
            palette: colours
                .iter()
                .map(|&base| PaletteEntry { base, emit: None })
                .collect(),
            supersample: 4,
            ..Default::default()
        });
        eprintln!("frondgrazers=6, model cells={}", voxels.len());
    }
    r.set_weather(Weather {
        fog: if case == "no-weather" { 0.0 } else { 0.8 },
        ..Weather::CLEAR_NOON
    });
    r.set_fog_floor(4.0);
    let mut target = Headless::new(&gpu, &r)?;
    let mut timing = std::io::BufWriter::new(std::fs::File::create(out.join("timing.csv"))?);
    writeln!(timing, "frame,weather_ms,world_ms")?;
    for i in 0..frames {
        r.set_clock(FrameClock::at(i as u64 + 20, 0.0, 12.0, 20.0, false));
        target.draw(&gpu, &mut r, ())?;
        // A clear fixture can reuse its raster. Timings of that path are not benchmarked.
        let weather = r.weather_gpu_ms(&gpu).unwrap_or(0.0);
        let world = r.gpu_split(&gpu).map_or(0.0, |s| s[1]);
        writeln!(timing, "{i},{weather:.9},{world:.9}")?;
    }
    let rgba = r.read_raster(&gpu)?;
    std::fs::write(out.join("frame.rgba"), &rgba)?;
    write_png(&out.join("frame.png"), p.raster_w, p.raster_h, &rgba)?;
    target.destroy(&gpu);
    r.destroy(&gpu);
    Ok(())
}
