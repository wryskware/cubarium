//! `voxel_fidelity`: the CPU voxel presenter and the GPU slab walk, on the same worlds.
//!
//! ```text
//! cargo run -p cubarium --release --example voxel_fidelity -- --out DIR [--px 4] [--seconds 60]
//! ```
//!
//! For each scene it renders the identical `(World, Flora)` twice — once through
//! `crate::voxel::present::VoxelPresenter` into a `Canvas`, once through
//! `VoxelGpuSink` with `--gpu-target headless` — and writes `<scene>-cpu.png`,
//! `<scene>-gpu.png` and `<scene>-diff.png` beside per-channel maximum and mean
//! absolute differences.
//!
//! **Exact agreement is not the bar and never could be.** The CPU canvas holds linear
//! `f32` and encodes sRGB through `cubarium_render::srgb`'s table; the GPU writes linear
//! `f32` into an `R8G8B8A8_SRGB` attachment and the hardware encodes it, which Vulkan
//! pins to within 0.6 ULP of the correctly rounded result rather than to the table. And
//! the voxel texture carries free water quantised to eight bits. So the report is the
//! *distribution* of the difference and the checklist of rules, not a hash.
//!
//! The scenes exercise every rule the presenter's own tests name: the authored fixture
//! has a ridge across the seam, a terrace of quantised steps, a covered passage, a
//! jutting shelf and a hollow with standing water; the generated world has the core's
//! own landform and water table; and both are run again after 60 s of the default rain,
//! with stands seeded so the trunk, crown and canopy rules are on screen. The part-class
//! census printed per scene says which plant rules each picture actually contains, so a
//! rule that was never drawn is reported as untested instead of as passing.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};

use cubarium::sink::gpu::voxel::{VoxelGpuSink, VoxelGpuSinkOptions};
use cubarium::sink::GpuTargetKind;
use cubarium::voxel::present::VoxelPresenter;
use cubarium::voxel::project::Projection;
use cubarium::voxel::{VoxelConfig, scene as authored_scene};
use cubarium_render::Canvas;
use cubarium_surface::{Scale, Topology};
use cubarium_voxel::{Command as VoxelCommand, Config, Material, World};
use cubarium_voxel_flora::{Command as FloraCommand, Flora, FloraConfig, Species};

/// The run's own `r` with no argument.
const DEFAULT_RAIN_M3: f64 = 1.0;
/// World ticks a second.
const TICK_HZ: u32 = 20;
/// Seconds of the default rain before a seeded strip holds a **seed cohort**, and so
/// before the sprout glyph is anywhere in the picture.
///
/// Measured, not chosen: `propagule_rate` is 0.0002 of reserve a second, and on this
/// fixture the first bank appears between 300 and 400 s and the last is gone by 1000 s.
/// Without a scene in that window the sprout rule is never drawn and the checklist
/// cannot say anything about it.
const SPROUT_SECONDS: f64 = 400.0;

struct Args {
    out: PathBuf,
    px: u32,
    seconds: f64,
    roof_walk: bool,
    /// Print this many of the worst-differing pixels per scene, with their coordinates
    /// and the voxel column they fall in. How a difference gets diagnosed.
    worst: usize,
    /// Time both renderers over this many frames of the authored scene instead of
    /// comparing anything. The run loop caps at `clock::MAX_FPS`, so a run's reported
    /// fps cannot separate two renderers that both clear it; this can.
    bench: usize,
}

fn parse() -> Result<Args> {
    let mut a = Args {
        out: PathBuf::from("design/7_Research/assets/voxel-render/fidelity"),
        px: 4,
        seconds: 60.0,
        roof_walk: false,
        worst: 0,
        bench: 0,
    };
    let mut it = std::env::args().skip(1);
    while let Some(flag) = it.next() {
        match flag.as_str() {
            "--out" => a.out = PathBuf::from(it.next().context("--out DIR")?),
            "--px" => a.px = it.next().context("--px N")?.parse()?,
            "--seconds" => a.seconds = it.next().context("--seconds N")?.parse()?,
            "--roof-walk" => a.roof_walk = true,
            "--worst" => a.worst = it.next().context("--worst N")?.parse()?,
            "--bench" => a.bench = it.next().context("--bench N")?.parse()?,
            other => bail!("unknown flag {other}"),
        }
    }
    Ok(a)
}

fn main() -> Result<()> {
    let args = parse()?;
    std::fs::create_dir_all(&args.out)?;
    let world_cfg = Config::default();
    let cfg = VoxelConfig {
        px_per_voxel: args.px,
        world: world_cfg.clone(),
        ..VoxelConfig::default()
    };
    let proj = Projection::new(cfg.tilt_degrees, cfg.px_per_voxel, cfg.raster_height, &world_cfg)?;
    println!(
        "projection: {}x{} raster, {} px per voxel, depth step {} px, world {}x{}x{}",
        proj.raster_w, proj.raster_h, proj.s, proj.rise, proj.width, proj.height, proj.depth
    );

    // One device and one renderer for every scene: they share a world shape and a
    // projection, so a second `VoxelRenderer` would only be a second set of textures.
    let mut gpu = match VoxelGpuSink::new(
        &cfg,
        proj,
        VoxelGpuSinkOptions {
            target: GpuTargetKind::Headless,
            capture: None,
            roof_from_texture: !args.roof_walk,
        },
    ) {
        Ok(sink) => sink,
        Err(e) => {
            println!("skipped: no Vulkan device for the GPU renderer ({e:#})");
            return Ok(());
        }
    };

    if args.bench > 0 {
        return bench(&args, &cfg, proj, &world_cfg, &mut gpu);
    }
    let ticks = (args.seconds * f64::from(TICK_HZ)).round() as u64;
    let mut worst = 0u8;
    for (name, rained) in [("authored", false), ("authored-rained", true)] {
        let (world, flora) = build(&world_cfg, true, rained.then_some(ticks));
        worst = worst.max(compare(&args, name, &cfg, proj, &world, &flora, &mut gpu)?);
    }
    for (name, rained) in [("generated", false), ("generated-rained", true)] {
        let (world, flora) = build(&world_cfg, false, rained.then_some(ticks));
        worst = worst.max(compare(&args, name, &cfg, proj, &world, &flora, &mut gpu)?);
    }
    let (world, flora) = fixtures(&world_cfg);
    worst = worst.max(compare(&args, "fixtures", &cfg, proj, &world, &flora, &mut gpu)?);
    let sprout_ticks = (SPROUT_SECONDS * f64::from(TICK_HZ)).round() as u64;
    let (world, flora) = build(&world_cfg, true, Some(sprout_ticks));
    worst = worst.max(compare(&args, "authored-sprout", &cfg, proj, &world, &flora, &mut gpu)?);
    println!("\nworst single-channel difference over every scene: {worst}/255");
    Ok(())
}

/// One scene: a world, stands seeded on it, and optionally `ticks` of the default rain.
///
/// The stands are seeded at fixed columns rather than left to the model's own
/// recruitment, because a picture with no plants in it cannot test the plant rules. The
/// rain is the run's own `r` with no argument — one default pulse a second — because
/// `Config::rain_m_per_s` defaults to zero and a world nobody rains on never pools.
fn build(cfg: &Config, authored: bool, ticks: Option<u64>) -> (World, Flora) {
    let mut world =
        if authored { authored_scene::authored(cfg.clone()) } else { World::new(cfg.clone()) };
    let mut flora = Flora::new(FloraConfig::default());
    // Six stands spread across the strip and the habitat's depth: both species, both
    // near the seam and away from it, at full wood so the crown disc is at its widest.
    for (i, (x, z)) in [(0i64, 0u32), (12, 3), (40, 1), (70, 6), (96, 2), (127, 5)]
        .into_iter()
        .enumerate()
    {
        let species = if i % 2 == 0 { Species::Bloomcrown } else { Species::Umbrellafrond };
        let wood = flora.config().species(species).wood_max;
        flora.apply(&world, FloraCommand::Seed { x, z, species, wood });
    }
    if let Some(ticks) = ticks {
        for t in 0..ticks {
            if t % u64::from(TICK_HZ) == 0 {
                world.apply(VoxelCommand::RainPulse { volume_m3: DEFAULT_RAIN_M3 });
            }
            world.step();
            flora.step(&mut world);
        }
    }
    (world, flora)
}

/// Time both renderers on the authored scene, one frame at a time, with no readback.
///
/// The CPU number is `VoxelPresenter::draw` plus `Canvas::encode_raster` — what a frame
/// of `--sink web` or `--sink png` pays before the sink sees a pixel. The GPU number is
/// `stage_world` once and then `render` per frame through the headless target, which is
/// record, submit and wait on the fence: the whole cost of a frame, not just the shader.
fn bench(
    args: &Args,
    cfg: &VoxelConfig,
    proj: Projection,
    world_cfg: &Config,
    gpu: &mut VoxelGpuSink,
) -> Result<()> {
    let (world, flora) = build(world_cfg, true, None);
    let n = args.bench;
    let topology = Topology::Ring { w: proj.raster_w, h: proj.raster_h };
    let mut canvas = Canvas::new(topology, Scale::ONE);
    let mut raster = cube_proto::Raster::black(proj.raster_w, proj.raster_h);
    let mut presenter = VoxelPresenter::new(cfg.clone(), proj);
    // One of each first, so neither number is paying for a cold cache or a first pass.
    presenter.draw(&world.view(), flora.view(), &mut canvas);
    gpu.stage_world(&world, &flora);
    gpu.render()?;

    let at = std::time::Instant::now();
    for _ in 0..n {
        presenter.draw(&world.view(), flora.view(), &mut canvas);
        canvas.encode_raster(&mut raster);
    }
    let cpu_ms = at.elapsed().as_secs_f64() * 1e3 / n as f64;

    let at = std::time::Instant::now();
    for _ in 0..n {
        gpu.render()?;
    }
    let gpu_ms = at.elapsed().as_secs_f64() * 1e3 / n as f64;

    let at = std::time::Instant::now();
    for _ in 0..n {
        gpu.stage_world(&world, &flora);
    }
    let pack_ms = at.elapsed().as_secs_f64() * 1e3 / n as f64;

    let p = gpu.params();
    println!(
        "bench over {n} frames at {}x{} ({} px/voxel, depth step {}):",
        p.raster_w, p.raster_h, p.s, p.rise
    );
    println!("  CPU presenter draw + encode {cpu_ms:.3} ms/frame ({:.0} fps)", 1e3 / cpu_ms);
    println!("  GPU record + submit + fence {gpu_ms:.3} ms/frame ({:.0} fps)", 1e3 / gpu_ms);
    println!(
        "  CPU pack of one tick        {pack_ms:.3} ms, {:.0} KiB uploaded",
        p.upload_bytes() as f64 / 1024.0
    );
    Ok(())
}

/// A hand-built strip for the two rules neither the fixture nor the generator draws.
///
/// **The partial roof over a water top.** The presenter's own
/// `a_partial_roof_clips_a_water_top_instead_of_culling_it` builds it: a quarter-full
/// water voxel at `(x, 6, 1)` under a single rock at `(x, 7, 0)`, whose front covers the
/// first row of the water top's band and not the second. Neither the authored scene nor
/// the generator produces that overlap anywhere — the census below counts zero of it on
/// all four — so the rule would otherwise go untested.
///
/// **A pool deep in z with a lid over part of it**, for the one-surface-per-pixel rule
/// with something nearer than the water in the way.
fn fixtures(cfg: &Config) -> (World, Flora) {
    let mut world = World::empty(cfg.clone());
    let v = cfg.voxel_volume();
    // A floor to stand the water on, and a basin with rock walls.
    for z in 0..cfg.depth {
        for x in 0..i64::from(cfg.width) {
            world.apply(VoxelCommand::SetMaterial { x, y: 0, z, material: Material::Bedrock });
            world.apply(VoxelCommand::SetMaterial { x, y: 1, z, material: Material::Soil });
        }
    }
    // Twelve copies of the presenter's own partial-roof fixture, spread across the strip.
    for k in 0..12i64 {
        let x = 4 + k * 10;
        world.apply(VoxelCommand::AddWater { x, y: 6, z: 1, volume_m3: v * 0.25 });
        world.apply(VoxelCommand::SetMaterial { x, y: 7, z: 0, material: Material::Rock });
    }
    // A pool eight slabs deep with a rock lid over its near half, so a water top meets a
    // roof and a water body meets both.
    for z in 0..8u32 {
        for x in 60..76i64 {
            for y in 2..5u32 {
                world.apply(VoxelCommand::AddWater { x, y, z, volume_m3: v });
            }
        }
    }
    for x in 60..70i64 {
        world.apply(VoxelCommand::SetMaterial { x, y: 6, z: 0, material: Material::Rock });
        world.apply(VoxelCommand::SetMaterial { x, y: 6, z: 1, material: Material::Rock });
    }
    (world, Flora::new(FloraConfig::default()))
}

/// Render one scene on both renderers, write the three PNGs, print the differences, and
/// hand back the worst single-channel difference.
#[allow(clippy::too_many_arguments)]
fn compare(
    args: &Args,
    name: &str,
    cfg: &VoxelConfig,
    proj: Projection,
    world: &World,
    flora: &Flora,
    gpu: &mut VoxelGpuSink,
) -> Result<u8> {
    let (w, h) = (u32::from(proj.raster_w), u32::from(proj.raster_h));
    let topology = Topology::Ring { w: proj.raster_w, h: proj.raster_h };
    let mut canvas = Canvas::new(topology, Scale::ONE);
    let mut raster = cube_proto::Raster::black(proj.raster_w, proj.raster_h);
    VoxelPresenter::new(cfg.clone(), proj).draw(&world.view(), flora.view(), &mut canvas);
    canvas.encode_raster(&mut raster);
    let cpu = raster.as_bytes();

    gpu.stage_world(world, flora);
    gpu.render()?;
    let gpu_rgba = gpu.read_raster()?;

    // --- the difference ---
    let n = (w * h) as usize;
    let mut max = [0u8; 3];
    let mut sum = [0u64; 3];
    let mut over = [0u64; 4]; // pixels whose worst channel differs by > 1, 2, 4, 8
    let mut diff_png = vec![0u8; n * 3];
    let mut worst_at: Vec<(u8, usize)> = Vec::new();
    for i in 0..n {
        let mut worst = 0u8;
        for c in 0..3 {
            let d = cpu[i * 3 + c].abs_diff(gpu_rgba[i * 4 + c]);
            max[c] = max[c].max(d);
            sum[c] += u64::from(d);
            worst = worst.max(d);
            // The diff image is the difference times eight, so a one-code disagreement
            // is visible instead of black-on-black.
            diff_png[i * 3 + c] = u8::try_from(u32::from(d) * 8).unwrap_or(255);
        }
        for (k, limit) in [1u8, 2, 4, 8].into_iter().enumerate() {
            if worst > limit {
                over[k] += 1;
            }
        }
        if worst > 1 && worst_at.len() < 4096 {
            worst_at.push((worst, i));
        }
    }
    let mean = |c: usize| sum[c] as f64 / n as f64;
    println!(
        "\n{name}: {} voxels of plant ({}), {} water voxels\n  \
         max |diff| r/g/b {}/{}/{}   mean {:.4}/{:.4}/{:.4}\n  \
         pixels differing by >1: {} ({:.3}%), >2: {}, >4: {}, >8: {}  of {n}",
        plant_census(world, flora).0,
        plant_census(world, flora).1,
        water_voxels(world),
        max[0],
        max[1],
        max[2],
        mean(0),
        mean(1),
        mean(2),
        over[0],
        over[0] as f64 * 100.0 / n as f64,
        over[1],
        over[2],
        over[3],
    );

    if args.worst > 0 && !worst_at.is_empty() {
        worst_at.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
        for &(d, i) in worst_at.iter().take(args.worst) {
            let (sx, sy) = ((i as u32 % w) as i32, (i as u32 / w) as i32);
            println!(
                "  worst {d:3} at ({sx:4},{sy:3}) voxel column x = {:3}, dx = {}:                  cpu {:?} gpu {:?}",
                sx / proj.s as i32,
                sx % proj.s as i32,
                &cpu[i * 3..i * 3 + 3],
                &gpu_rgba[i * 4..i * 4 + 3],
            );
        }
    }

    for r in rules(world, flora, cfg, proj, cpu, &gpu_rgba) {
        let verdict = match r.verdict {
            Some((cpu_says, gpu_agrees)) if cpu_says > 0 => {
                format!("{gpu_agrees}/{cpu_says} verdicts agree, {} ties", r.ties)
            }
            Some(_) => format!("no pair read the rule ({} ties)", r.ties),
            None => "census only".to_string(),
        };
        println!(
            "  {:<52} {:>7} voxels {:>8} px  worst {:>3}  {verdict}",
            r.name, r.voxels, r.pixels, r.worst
        );
    }

    let dir = &args.out;
    write_rgb(&dir.join(format!("{name}-cpu.png")), w, h, cpu)?;
    let gpu_rgb: Vec<u8> = gpu_rgba.chunks_exact(4).flat_map(|p| p[..3].to_vec()).collect();
    write_rgb(&dir.join(format!("{name}-gpu.png")), w, h, &gpu_rgb)?;
    write_rgb(&dir.join(format!("{name}-diff.png")), w, h, &diff_png)?;
    Ok(max[0].max(max[1]).max(max[2]))
}

/// How many voxels of each plant part a scene actually contains, so the report can say
/// which plant rules the picture tested: `(blocks, a description by class)`.
fn plant_census(world: &World, flora: &Flora) -> (usize, String) {
    use cubarium::voxel::stand::{Part, Stands};
    let c = world.config();
    let view = world.view();
    let mut stands = Stands::empty(c.width, c.height, c.depth);
    stands.rebuild(&view, flora.view());
    let (mut trunk, mut crown, mut heart, mut sprout) = (0usize, 0, 0, 0);
    for z in 0..c.depth {
        for y in 0..c.height {
            for x in 0..i64::from(c.width) {
                match stands.at(x, i64::from(y), z) {
                    Part::Trunk(_) => trunk += 1,
                    Part::Crown { heart: true, .. } => heart += 1,
                    Part::Crown { heart: false, .. } => crown += 1,
                    Part::Sprout(_) => sprout += 1,
                    Part::None => {}
                }
            }
        }
    }
    (
        trunk + crown + heart + sprout,
        format!("{trunk} trunk, {crown} crown, {heart} heart, {sprout} sprout"),
    )
}

fn water_voxels(world: &World) -> usize {
    let c = world.config();
    let view = world.view();
    let mut n = 0;
    for z in 0..c.depth {
        for y in 0..c.height {
            for x in 0..i64::from(c.width) {
                if view.free_at(x, y, z) > 1e-4 {
                    n += 1;
                }
            }
        }
    }
    n
}

fn write_rgb(path: &Path, w: u32, h: u32, rgb: &[u8]) -> Result<()> {
    let file = std::fs::File::create(path)?;
    let mut encoder = png::Encoder::new(std::io::BufWriter::new(file), w, h);
    encoder.set_color(png::ColorType::Rgb);
    encoder.set_depth(png::BitDepth::Eight);
    encoder.write_header()?.write_image_data(rgb)?;
    Ok(())
}

// --- the per-rule checklist -----------------------------------------------------------

/// One presenter rule, as this example can speak about it.
struct Rule {
    name: &'static str,
    /// Voxels in the scene whose geometry satisfies the rule's precondition: whether the
    /// picture **contains** the rule at all.
    voxels: u64,
    /// Pixels of those voxels' governed rectangles, and the worst CPU/GPU difference
    /// over them. Some are occluded by nearer faces, which does not weaken the
    /// agreement claim — it only means the pixel is not isolating the rule.
    pixels: u64,
    worst: u8,
    /// Where a local pixel pair decides the rule (a rim row against the row below it, a
    /// bevel column against its neighbour), how many of those pairs read the rule's own
    /// way on the CPU, and on how many of those the GPU agrees. `None` where no such
    /// pair exists and the rule is carried by the census and the match alone.
    verdict: Option<(u64, u64)>,
    /// Pairs whose CPU luminance difference is under one 8-bit code, so the sign of the
    /// comparison is inside the sRGB rounding and says nothing. Not counted as verdicts
    /// either way; reported so that "every verdict agrees" cannot be hiding them.
    ties: u64,
}

struct Probe<'a> {
    proj: Projection,
    cpu: &'a [u8],
    gpu: &'a [u8],
}

impl Probe<'_> {
    /// The CPU and GPU pixel at a raster position, wrapping in x and clipping in y just
    /// as `present::Surface` does.
    fn at(&self, col: i32, row: i32) -> Option<([u8; 3], [u8; 3])> {
        let w = i32::from(self.proj.raster_w);
        if row < 0 || row >= i32::from(self.proj.raster_h) {
            return None;
        }
        let i = (row as usize) * w as usize + col.rem_euclid(w) as usize;
        Some((
            [self.cpu[i * 3], self.cpu[i * 3 + 1], self.cpu[i * 3 + 2]],
            [self.gpu[i * 4], self.gpu[i * 4 + 1], self.gpu[i * 4 + 2]],
        ))
    }

    fn worst_over(&self, rect: (i32, i32, u32, u32), rule: &mut Rule) {
        let (col, row, w, h) = rect;
        for dy in 0..h as i32 {
            for dx in 0..w as i32 {
                if let Some((c, g)) = self.at(col + dx, row + dy) {
                    rule.pixels += 1;
                    for k in 0..3 {
                        rule.worst = rule.worst.max(c[k].abs_diff(g[k]));
                    }
                }
            }
        }
    }

    /// Does the pixel at `a` read brighter than the one at `b`? Counted separately on
    /// each renderer so the verdict is "the rule is visible, and both agree it is".
    fn brighter(&self, a: (i32, i32), b: (i32, i32), rule: &mut Rule) {
        let lum = |c: [u8; 3]| {
            0.2126 * f32::from(c[0]) + 0.7152 * f32::from(c[1]) + 0.0722 * f32::from(c[2])
        };
        let (Some((ca, ga)), Some((cb, gb))) = (self.at(a.0, a.1), self.at(b.0, b.1)) else {
            return;
        };
        let v = rule.verdict.get_or_insert((0, 0));
        // One 8-bit code of luminance is the sRGB encode's own noise floor, and the two
        // renderers do not share an encoder. A pair inside it is a tie, not a verdict.
        if (lum(ca) - lum(cb)).abs() <= 1.0 {
            rule.ties += 1;
            return;
        }
        if lum(ca) > lum(cb) {
            v.0 += 1;
            if lum(ga) > lum(gb) {
                v.1 += 1;
            }
        }
    }
}

/// Every rule the presenter's module doc names, counted and checked on one scene.
fn rules(
    world: &World,
    flora: &Flora,
    cfg: &VoxelConfig,
    proj: Projection,
    cpu: &[u8],
    gpu: &[u8],
) -> Vec<Rule> {
    use cubarium::voxel::stand::{Part, Stands};
    let view = world.view();
    let c = world.config();
    let mut stands = Stands::empty(c.width, c.height, c.depth);
    stands.rebuild(&view, flora.view());
    let p = Probe { proj, cpu, gpu };
    let (s, rise) = (proj.s as i32, proj.rise as i32);

    let names = [
        "rim light on an exposed top row",
        "side bevel on an exposed flank",
        "chamfer corner where rim meets flank",
        "riser lean, one voxel of ground into depth, no rim",
        "top-face contour row where the ground ends going back",
        "contour suppressed on a plateau receding in z",
        "roof shadow under a lip, falling off with depth",
        "per-row haze across a cap and a riser",
        "pore-water darkening of soil and rock",
        "free water filling from the bottom of its voxel",
        "one water surface per pixel across a deep pool",
        "a partial roof clipping a water top row by row",
        "trunk cylinder shading across four pixels",
        "crown silhouette edge and shaded skirt",
        "sprout glyph on a seed bank",
    ];
    let mut out: Vec<Rule> = names
        .iter()
        .map(|name| Rule { name, voxels: 0, pixels: 0, worst: 0, verdict: None, ties: 0 })
        .collect();

    let solid = |x: i64, y: i64, z: u32| -> bool {
        y >= 0 && y < i64::from(c.height) && view.material_at(x, y as u32, z).is_solid()
    };
    let free = |x: i64, y: i64, z: u32| -> f64 {
        if y < 0 || y >= i64::from(c.height) { 0.0 } else { view.free_at(x, y as u32, z) }
    };
    let fill = |x: i64, y: i64, z: u32| -> i32 {
        let f = free(x, y, z);
        if f <= 1e-4 { 0 } else { ((f as f32).clamp(0.0, 1.0) * s as f32).round().clamp(1.0, s as f32) as i32 }
    };
    let water_open_up = |x: i64, y: i64, z: u32| -> bool {
        fill(x, y, z) < s || (!solid(x, y + 1, z) && free(x, y + 1, z) <= 1e-4)
    };
    let roof_gap = |x: i64, y: u32, z: u32| -> u32 {
        (y + 1..c.height).find(|&yy| solid(x, i64::from(yy), z)).map_or(0, |yy| yy - y)
    };

    for z in 0..c.depth {
        for y in 0..c.height {
            let yi = i64::from(y);
            for x in 0..i64::from(c.width) {
                let m = view.material_at(x, y, z);
                let (fc, fr, _, _) = proj.front_rect(x, y, z);
                let (_, tr, _, _) = proj.top_rect(x, y, z);
                if m.is_solid() {
                    let open_up = !solid(x, yi + 1, z);
                    let open_left = !solid(x - 1, yi, z);
                    let open_right = !solid(x + 1, yi, z);
                    let riser = open_up
                        && z > 0
                        && !solid(x, yi, z - 1)
                        && solid(x, yi - 1, z - 1);
                    let back_continues =
                        z + 1 < c.depth && solid(x, yi, z + 1) && !solid(x, yi + 1, z + 1);

                    if open_up && !riser {
                        out[0].voxels += 1;
                        p.worst_over((fc, fr, proj.s, 1), &mut out[0]);
                        // The rim row against the row below it, one pixel in from the
                        // flanks so a bevel or a chamfer cannot be what is read.
                        if s >= 3 {
                            p.brighter((fc + 1, fr), (fc + 1, fr + 1), &mut out[0]);
                        }
                    }
                    if (open_left || open_right) && !riser && s >= 3 {
                        out[1].voxels += 1;
                        let side = if open_left { fc } else { fc + s - 1 };
                        let inner = if open_left { fc + 1 } else { fc + s - 2 };
                        p.worst_over((side, fr + 1, 1, proj.s - 1), &mut out[1]);
                        // Darker than its neighbour: the pair is read the other way
                        // round, so the neighbour must be the brighter one.
                        p.brighter((inner, fr + 2), (side, fr + 2), &mut out[1]);
                    }
                    if open_up && (open_left || open_right) && !riser && s >= 3 {
                        out[2].voxels += 1;
                        let side = if open_left { fc } else { fc + s - 1 };
                        let inner = if open_left { fc + 1 } else { fc + s - 2 };
                        p.worst_over((side, fr, 1, 1), &mut out[2]);
                        // The chamfer takes the *top* colour, brighter than the rim
                        // beside it.
                        p.brighter((side, fr), (inner, fr), &mut out[2]);
                    }
                    if riser {
                        out[3].voxels += 1;
                        p.worst_over((fc, fr, proj.s, proj.s), &mut out[3]);
                        // Lit at the head, leaning to the body colour at the foot.
                        p.brighter((fc + 1, fr), (fc + 1, fr + s - 1), &mut out[3]);
                    }
                    if open_up {
                        let r = if back_continues { &mut out[5] } else { &mut out[4] };
                        r.voxels += 1;
                        p.worst_over((fc, tr, proj.s, proj.rise), r);
                        if rise > 2 {
                            // Where the ground ends going back the cap's back row is
                            // shaded toward the body, so the row in front of it is
                            // brighter; where it carries on the two are equal and this
                            // pair is deliberately *not* counted as a verdict.
                            if !back_continues {
                                p.brighter((fc + 1, tr + 1), (fc + 1, tr), &mut out[4]);
                            }
                        }
                        let gap = roof_gap(x, y, z);
                        if gap > 0 {
                            out[6].voxels += 1;
                            p.worst_over((fc, tr, proj.s, proj.rise), &mut out[6]);
                        }
                    }
                    if rise >= 2 && open_up {
                        // The cap's haze is interpolated per row, so two rows of one cap
                        // are two colours. Counted, not verdicted: at `rise` = 2 the
                        // difference is a fraction of a code.
                        out[7].voxels += 1;
                        p.worst_over((fc, tr, proj.s, proj.rise), &mut out[7]);
                    }
                    if m.pore_capacity() > 0.0 && view.pore_at(x, y, z) > 0.01 {
                        out[8].voxels += 1;
                        p.worst_over((fc, fr, proj.s, proj.s), &mut out[8]);
                    }
                    continue;
                }

                // --- water ---
                let f = fill(x, yi, z);
                if f > 0 {
                    let bottom = fr + s;
                    let skin = bottom - f;
                    if f < s {
                        out[9].voxels += 1;
                        p.worst_over((fc, skin, proj.s, f as u32), &mut out[9]);
                        // The rows below the skin hold water and the rows above it do
                        // not, which is what "fills from the bottom" means: the skin row
                        // is the bright one and the row above it is whatever is behind.
                        p.brighter((fc + 1, skin), (fc + 1, skin - 1), &mut out[9]);
                    }
                    // A pixel over four or more slabs of water is the deep-pool case the
                    // one-surface-per-pixel rule is about.
                    if z >= 3
                        && (1..=3).all(|k| fill(x, yi, z - k) > 0)
                        && water_open_up(x, yi, z)
                    {
                        out[10].voxels += 1;
                        p.worst_over((fc, skin, proj.s, f as u32), &mut out[10]);
                    }
                    // A water top whose rows are *partly* owned by a nearer face: the
                    // row-by-row clip. The nearer slab's solid front and cap span
                    // `F - rise .. F + s`; a top band straddling that edge is the case.
                    if water_open_up(x, yi, z) && z > 0 {
                        let f_near = fr + rise;
                        let owned = |row: i32| {
                            (0..3i64).any(|dy| {
                                let yn = yi + dy;
                                let fn_ = f_near - dy as i32 * s;
                                if solid(x, yn, z - 1) {
                                    (fn_ - rise..fn_ + s).contains(&row)
                                } else {
                                    let nf = fill(x, yn, z - 1);
                                    if nf == 0 {
                                        return false;
                                    }
                                    let sk = fn_ + s - nf;
                                    let top =
                                        if water_open_up(x, yn, z - 1) { sk - rise } else { sk };
                                    (top..fn_ + s).contains(&row)
                                }
                            })
                        };
                        let rows: Vec<i32> = (skin - rise..skin).collect();
                        let n_owned = rows.iter().filter(|r| owned(**r)).count();
                        if n_owned > 0 && n_owned < rows.len() {
                            out[11].voxels += 1;
                            p.worst_over((fc, skin - rise, proj.s, proj.rise), &mut out[11]);
                        }
                    }
                }

                // --- plants ---
                match stands.at(x, yi, z) {
                    Part::Trunk(_) => {
                        out[12].voxels += 1;
                        p.worst_over((fc, fr, proj.s, proj.s), &mut out[12]);
                        // `TRUNK_LIGHT_AT` is 0.35 across the stem, so the second column
                        // is lit and the last is the shaded far edge.
                        if s >= 4 {
                            p.brighter((fc + 1, fr + 2), (fc + s - 1, fr + 2), &mut out[12]);
                        }
                    }
                    Part::Crown { .. } => {
                        out[13].voxels += 1;
                        p.worst_over((fc, fr, proj.s, proj.s), &mut out[13]);
                        p.worst_over((fc, tr, proj.s, proj.rise), &mut out[13]);
                        // The cap is lit and the skirt below it is shaded toward the
                        // stand's own wood.
                        p.brighter((fc + 1, tr + rise - 1), (fc + 1, fr + 1), &mut out[13]);
                    }
                    Part::Sprout(_) => {
                        out[14].voxels += 1;
                        p.worst_over((fc, fr, proj.s, proj.s), &mut out[14]);
                    }
                    Part::None => {}
                }
            }
        }
    }
    let _ = cfg;
    out
}
