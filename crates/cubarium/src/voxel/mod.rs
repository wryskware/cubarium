//! `cubarium voxel`: run a [`cubarium_voxel::World`] at a fixed 20 Hz and draw it as
//! pixel art into a ring raster, through the same sinks the rest of the host uses.
//!
//! The loop is [`crate::run::drive`]'s shape — one [`Clock`], one `encode_raster` per
//! rendered frame, one sink — with two differences: the world is a voxel strip rather
//! than the fixture scenes, and a stdin reader thread lets the run be poked while it is
//! running (pause, single-step, speed, rain, set a cell's material, save, load, inspect,
//! outlet, quit).
//!
//! **`w` and `l` are terrain only** (Astra R9.6). `World::save`/`World::load` carry the
//! voxels, the water and the world's own tick, and **nothing** of the plant or animal
//! layers: there is no atomic world/flora/fauna envelope yet, and the plant layer does not
//! serialize at all. So `l` is refused in place whenever this run holds ecology state — any
//! stand, any animal, any provisioned ground, any seed bank, any nonzero ecological ledger
//! — because restoring another world's terrain under living stands would leave them
//! standing on water and rock they never grew in, with their ledgers still describing the
//! terrain that is gone. A terrain snapshot is resumed with `--load PATH` at startup, which
//! begins the ecology fresh on it, and that is what the refusal says.
//!
//! `--speed` scales world ticks per clock tick through an accumulator, so a fractional
//! speed slows the world down without touching the clock: the picture is still drawn at
//! `--fps` and the simulation still advances in whole 20 Hz ticks.
//!
//! The projection, the palette and the autotiling live in [`project`] and [`present`];
//! the hand-authored fixture in [`scene`]; the stands' own geometry and palettes in
//! [`stand`]. The preview sink is deliberately absent: the minifb window refuses ring
//! rasters and that refusal is correct.
//!
//! The run owns a [`cubarium_voxel_flora::Flora`] beside the world and steps it once
//! after every world tick, in that order: the plant layer reads the water the core has
//! just moved and withdraws from it, never the other way round. Its stands are drawn
//! into the same canvas, in the same traversal, as the terrain and the water.
//!
//! Since voxel round 5c it owns a [`cubarium_voxel_fauna::Fauna`] too, stepped once after
//! the plant layer and for the same reason the plant layer follows the world: the animals
//! eat the foliage the plants have just grown. One coupled tick is `World::step`,
//! `Flora::step`, `Fauna::step`, and the animals are drawn in the same traversal as
//! everything else, as the **interim** glyph of [`animal`].

pub mod animal;
pub mod habitat;
pub mod present;
pub mod project;
pub mod scene;
pub mod stand;

use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver};
use std::time::{Duration, Instant};

use anyhow::{Context, Result, bail};
use cubarium_render::Canvas;
use cubarium_surface::{Scale, Topology};
use cubarium_voxel::{Command as VoxelCommand, Material, World};
use cubarium_voxel_fauna::{
    Command as FaunaCommand, Controller, Fauna, FaunaConfig, FaunaLedger, Founder, Response,
    Senses, Species as Beast,
};
use cubarium_voxel_flora::{
    Command as FloraCommand, Flora, FloraConfig, FloraLedger, Site, Species,
};
use cubarium_search::es::voxel::{EpisodeDriver, VoxelPolicyFile};
use cubarium_voxel_sim::{Arena, Sim, SimConfig};
use serde::{Deserialize, Serialize};

use crate::cli::{Voxel, VoxelArenaArg, VoxelControllerArg, VoxelSceneArg, VoxelSinkArg};
use crate::clock::{Clock, Step};
use crate::sink::gpu::voxel::{VoxelGpuSink, VoxelGpuSinkOptions};
use crate::sink::{FrameSink, GpuTargetKind, Output, PngSink, WebSink, WorldShape};

use present::VoxelPresenter;
use project::Projection;

/// The stdin commands, in the one place both the banner and the usage line read them
/// from, so a new command cannot be added to only one of the two.
const COMMANDS: &str = "p pause/resume, s step, +/- speed, r [m3] rain, a M3 charge the aquifer (negative withdraws), \
                        m X Y Z air|rock|soil|bedrock set material, \
                        f X Z bloomcrown|umbrellafrond [wood] seed a stand, \
                        c X Z clear a stand, \
                        g X Z frondgrazer [body] introduce an animal, \
                        w PATH save the terrain only, \
                        l PATH load a terrain only (refused once anything is alive), \
                        i X Y Z inspect, o outlet, q quit";

/// Default rain volume for the `r` command, in cubic metres.
const DEFAULT_RAIN_M3: f64 = 1.0;
/// `--speed` is clamped to this range; zero is `p` (pause), not a speed.
const MIN_SPEED: f64 = 1.0 / 64.0;
const MAX_SPEED: f64 = 64.0;

// --- The config file -----------------------------------------------------------------

/// `--config`: the presentation, plus the world to build. Missing fields take the
/// defaults below and unknown fields are errors, so a typo is not silently ignored.
///
/// ```toml
/// tilt_degrees = 30.0   # the chosen camera
/// px_per_voxel = 4      # the chosen camera
/// raster_height = 0     # 0 derives it from the world
/// haze = 0.55
/// water_alpha = 0.5
///
/// [world]
/// width = 128
/// height = 48
/// # depth is deliberately absent: it comes from `cubarium_voxel::Config::default()`, so
/// # the habitat's chosen depth lives in the core and the presenter never pins its own.
/// ```
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct VoxelConfig {
    /// Elevation of the orthographic view, in degrees. Reaches the picture only as
    /// `round(px_per_voxel · tan(tilt))` whole pixels of lift per voxel of depth. The
    /// chosen camera is 30°.
    pub tilt_degrees: f64,
    /// Pixels per voxel edge. The chosen camera is 4: below that the sprite atlas stops
    /// reading.
    pub px_per_voxel: u32,
    /// Ring raster height in pixels; `0` derives it so the whole strip fits. The width is
    /// always `world.width · px_per_voxel`, so the strip fills it exactly.
    pub raster_height: u16,
    /// How far the back wall fades toward the haze colour, `0..=1`.
    pub haze: f32,
    /// Opacity of one voxel of free water, `0..=1`.
    pub water_alpha: f32,
    /// Worker threads for the in-phase splits of the tick
    /// (`cubarium_voxel_sim::SimConfig::threads`); `0` means
    /// [`std::thread::available_parallelism`]. Execution only — it reaches no rule, no
    /// number and no picture. **Placeholder** (`design/backlog.md` §1).
    pub threads: usize,
    /// The world to build. Every field optional, every default the core's own — the
    /// habitat's extent, `depth` included, is the core's decision and not the presenter's.
    pub world: cubarium_voxel::Config,
}

impl Default for VoxelConfig {
    fn default() -> VoxelConfig {
        VoxelConfig {
            // The camera chosen by the storyboard study: 30°, 4 px per voxel.
            tilt_degrees: 30.0,
            px_per_voxel: 4,
            raster_height: 0,
            haze: 0.55,
            water_alpha: 0.5,
            threads: 0,
            // Including the depth: `cubarium_voxel` owns how deep the habitat is.
            world: cubarium_voxel::Config::default(),
        }
    }
}

/// `[world]`: [`cubarium_voxel::Config`] with every field optional.
///
/// Load a [`VoxelConfig`] from TOML.
pub fn load_config(path: &Path) -> Result<VoxelConfig> {
    let text = std::fs::read_to_string(path)
        .with_context(|| format!("reading the voxel config {}", path.display()))?;
    toml::from_str(&text).with_context(|| format!("parsing the voxel config {}", path.display()))
}

// --- The run -------------------------------------------------------------------------

/// Run `cubarium voxel`.
pub fn run_voxel(args: &Voxel, stop: &AtomicBool) -> Result<()> {
    let cfg = match &args.config {
        Some(path) => load_config(path)?,
        None => VoxelConfig::default(),
    };
    args.validate()?;
    if args.sink == VoxelSinkArg::Png && args.seconds <= 0.0 {
        bail!("`--sink png` needs `--seconds N`, or it would capture until interrupted");
    }
    // `World::new` and `World::empty` panic on a config no world can be built from —
    // building one from nonsense is a programming error. A config *file* is input, so the
    // boundary is here: a bad file is reported, not a backtrace.
    cfg.world.validate().with_context(|| match &args.config {
        Some(path) => format!("the `[world]` in {}", path.display()),
        None => "the built-in world defaults".to_string(),
    })?;

    // The regular display remains its own coupled world. `--arena` is an explicit
    // development mode which instead owns a frozen P1 sensing layout and a controller
    // installed through fauna's ordinary controller boundary.
    let sim_config = SimConfig {
        threads: if cfg.threads == 0 {
            SimConfig::default().threads
        } else {
            cfg.threads
        },
    };
    let (mut sim, scene_label) = match args.arena {
        Some(arena) => {
            let (founder, driver) = sensing_driver(args, arena)?;
            let mut arena = Arena::build(founder, args.arena_seed);
            let senses = arena.prepare_senses();
            let animal_id = arena
                .animal_id
                .ok_or_else(|| anyhow::anyhow!("sensing arena did not place its founder"))?;
            let controller = driver.fresh();
            let controller: Box<dyn Controller> = if args.arena_diagnostics {
                Box::new(DiagnosticController::new(founder, controller))
            } else {
                controller
            };
            anyhow::ensure!(
                arena.fauna.set_controller(animal_id, controller),
                "sensing arena founder {animal_id} disappeared before its controller installed"
            );
            eprintln!(
                "cubarium voxel: sensing arena {} seed {} with {}{}",
                founder.name(),
                args.arena_seed,
                driver.name(),
                if args.arena_diagnostics {
                    " (controller diagnostics on stderr)"
                } else {
                    ""
                },
            );
            (
                arena.into_sim_prepared(SimConfig { threads: 1 }, senses),
                format!("sensing {} arena seed {}", founder.name(), args.arena_seed),
            )
        }
        None => {
            let mut world = match &args.load {
                Some(path) => {
                    let bytes = std::fs::read(path)
                        .with_context(|| format!("reading the world {}", path.display()))?;
                    World::load(&bytes).with_context(|| format!("loading {}", path.display()))?
                }
                None => {
                    let world_cfg = cfg.world.clone();
                    match args.scene {
                        VoxelSceneArg::Authored => scene::authored(world_cfg),
                        VoxelSceneArg::Generated => World::new(world_cfg),
                    }
                }
            };
            let mut flora = Flora::new(FloraConfig::default());
            let mut fauna = Fauna::new(FaunaConfig::default());
            let drivers = founder_drivers(args)?;
            if !args.empty {
                let seeded = habitat::seed(&mut world, &mut flora, &mut fauna);
                eprintln!(
                    "cubarium voxel: seeded the example habitat — {} stands, {} logs, \
                     {} litter tiles, {} littershredders, {} frondgrazer founders \
                     (--empty for a bare world)",
                    seeded.stands,
                    seeded.logs,
                    seeded.litter_tiles,
                    seeded.founders[Founder::Blind.index()],
                    seeded.founders[Founder::Browser.index()],
                );
            } else {
                // An empty world grows its founders by hand; the lineages still need
                // their recipes registered so anything born is driven.
                habitat::install_heuristics(&mut fauna);
            }
            install_founder_controllers(&mut fauna, &drivers)?;
            for (founder, driver) in &drivers {
                eprintln!(
                    "cubarium voxel: {} founders are driven by {}",
                    founder.name(),
                    driver.name(),
                );
            }
            let label = match (&args.load, args.scene) {
                (Some(p), _) => p.display().to_string(),
                (None, VoxelSceneArg::Authored) => "authored".to_string(),
                (None, VoxelSceneArg::Generated) => "generated".to_string(),
            };
            // The live schedule's own sensory state, settled against the layers as they
            // stand before the first tick: the ambient world's founders smell the litter
            // this flora actually drops and has eaten, at the field's own cadence. An
            // `--empty` world has no litter, so the field is empty and its update is a
            // walk over nothing.
            let mut senses = Senses::new();
            senses.settle(&world.view(), &flora.view());
            (
                Sim::new(world, flora, fauna, sim_config, Some(senses)),
                label,
            )
        }
    };

    let proj = Projection::new(
        cfg.tilt_degrees,
        cfg.px_per_voxel,
        cfg.raster_height,
        sim.world().config(),
    )?;
    let topology = Topology::Ring {
        w: proj.raster_w,
        h: proj.raster_h,
    };
    let shape = WorldShape::new(topology, Scale::ONE);

    let speed = args.speed.clamp(MIN_SPEED, MAX_SPEED);
    let web = |port| {
        WebSink::with_world(
            port,
            "voxel strip",
            crate::sink::web::Source {
                speed,
                ..Default::default()
            },
            None,
            shape,
        )
    };
    let mut out: Out = match args.sink {
        VoxelSinkArg::Png => Out::cpu(
            Box::new(PngSink::new(&args.out, args.every)?),
            cfg.clone(),
            proj,
        ),
        VoxelSinkArg::Web => Out::cpu(Box::new(web(args.web_port)?), cfg.clone(), proj),
        VoxelSinkArg::Gpu => {
            crate::sink::gpu::voxel::check(proj)?;
            let mut gpu = VoxelGpuSink::new(
                &cfg,
                proj,
                VoxelGpuSinkOptions {
                    target: args.gpu_target.unwrap_or_else(GpuTargetKind::detect),
                    capture: args.gpu_capture.clone(),
                    roof_from_texture: !args.gpu_roof_walk,
                },
            )?;
            if args.gpu_web_rate > 0.0 {
                gpu = gpu.with_web(web(args.web_port)?, args.gpu_web_rate);
            }
            Out::Gpu(Box::new(gpu))
        }
    };

    eprintln!(
        "cubarium voxel: tick schedule on {} thread(s)",
        sim.config().threads
    );

    let c = sim.world().config().clone();
    eprintln!(
        "cubarium voxel: {} world {}x{}x{} at {} m/voxel -> ring:{}x{} \
         ({} px/voxel, tilt {:.0} deg, depth step {} px)",
        scene_label,
        c.width,
        c.height,
        c.depth,
        c.voxel_m,
        proj.raster_w,
        proj.raster_h,
        proj.s,
        cfg.tilt_degrees,
        proj.rise,
    );
    eprintln!("cubarium voxel: stdin commands — {COMMANDS}");

    let commands = spawn_stdin_reader();

    let limit = (args.seconds > 0.0).then(|| Duration::from_secs_f64(args.seconds));
    let start = Instant::now();
    let mut clock = Clock::with_fps(start, args.fps);
    let (mut ticks, mut frames) = (0u64, 0u64);
    let mut ctl = Control::new(speed, proj);
    let mut debt = 0.0f64;
    // Whether the world or the plant layer has moved since the last frame drawn. The CPU
    // presenter re-reads the world every frame and does not care; the GPU packs one
    // texture per *tick*, so it needs to be told.
    let mut moved = true;

    while !ctl.quit {
        let now = Instant::now();
        if let Some(l) = limit
            && clock.elapsed(now) >= l
        {
            break;
        }
        if out.should_quit() || stop.load(Ordering::Relaxed) {
            break;
        }

        while let Ok(line) = commands.try_recv() {
            sim.with_layers_mut(|world, flora, fauna| ctl.handle(world, flora, fauna, &line));
            // A command may have moved a cell, seeded a stand or loaded a world; which
            // ones did is the command's business, and one extra pack is cheaper than a
            // rule here that has to be kept in step with `Control::handle`.
            moved = true;
        }
        if ctl.quit {
            break;
        }

        match clock.next_step(now) {
            Step::Tick => {
                if ctl.paused {
                    // A single `s` still advances exactly one world tick.
                    if ctl.pending_steps > 0 {
                        ctl.pending_steps -= 1;
                        sim.step();
                        ticks += 1;
                        moved = true;
                    }
                } else {
                    debt += ctl.speed;
                    while debt >= 1.0 {
                        debt -= 1.0;
                        sim.step();
                        ticks += 1;
                        moved = true;
                    }
                }
                out.observe_tick(sim.world().tick());
            }
            Step::Render { .. } => {
                let (world, flora, fauna) = sim.layers();
                out.render(world, flora, fauna, std::mem::take(&mut moved))?;
                frames += 1;
            }
            Step::Sleep(d) => std::thread::sleep(d),
            Step::Lagged { behind, log } => {
                if log {
                    eprintln!(
                        "cubarium voxel: behind by {:.0} ms; dropping render work",
                        behind.as_secs_f64() * 1e3
                    );
                }
            }
            Step::Paused { gap } => {
                eprintln!(
                    "cubarium voxel: clock re-based after a {:.1} s pause",
                    gap.as_secs_f64()
                );
            }
        }
    }

    out.finish()?;
    let elapsed = clock.elapsed(Instant::now()).as_secs_f64();
    let (world, flora, fauna) = sim.layers();
    let view = world.view();
    let fv = flora.view();
    eprintln!(
        "cubarium voxel: {ticks} ticks, {frames} frames in {elapsed:.2} s ({:.1} fps); \
         stored {:.3} m3, residual {:.3e} m3; \
         {} stands, flora residual {:.3e} organic, {:.3e} mineral, {:.3e} energy",
        frames as f64 / elapsed.max(1e-9),
        view.stored_m3(),
        view.stored_m3() - view.ledger.expected_stored(),
        fv.stands.len(),
        fv.organic() - fv.ledger.expected_organic(),
        fv.mineral() - fv.ledger.expected_mineral(),
        fv.energy() - fv.ledger.expected_energy(),
    );
    let av = fauna.view();
    eprintln!(
        "cubarium voxel: {} animals ({} born, {} dead, {} bites, {} steps), \
         fauna residual {:.3e} organic, {:.3e} mineral, {:.3e} energy",
        av.animals.len(),
        av.ledger.born,
        av.ledger.deaths,
        av.ledger.bites,
        av.ledger.steps,
        av.organic() - av.ledger.expected_organic(),
        av.mineral() - av.ledger.expected_mineral(),
        av.energy() - av.ledger.expected_energy(),
    );
    // The founder census: who of the sensed lineages is still standing, and what each
    // lineage actually got into its tissue. `assimilated` is the placed figure — what
    // became body and reserve — and not the bite's gross organic matter.
    let census: Vec<String> = Founder::ALL
        .into_iter()
        .map(|f| {
            let alive = av.animals.iter().filter(|a| a.founder == Some(f)).count();
            format!(
                "{} {alive} alive, {} bites, {:.3e} assimilated",
                f.name(),
                av.ledger.bites_by_founder[f.index()],
                av.ledger.assimilated_by_founder[f.index()],
            )
        })
        .collect();
    eprintln!("cubarium voxel: founders — {}", census.join("; "));
    Ok(())
}

/// Load the `--founder-policy` files into one episode driver each, refusing a file
/// whose own declared lineage is not the one the flag named.
///
/// The file is the trainer's own policy file and validates itself — schema token,
/// lineage, vector length, finite weights, and a digest that is that founder manifest's
/// — so all this adds is the cross-check the user's spelling makes possible: a browser
/// centre asked to drive the blind lineage is refused by name rather than run against a
/// schema it was never trained on.
fn founder_drivers(args: &Voxel) -> Result<Vec<(Founder, EpisodeDriver)>> {
    let mut out = Vec::new();
    for (founder, path) in args.founder_policies()? {
        let file = VoxelPolicyFile::load(&path).map_err(anyhow::Error::msg)?;
        let declared = file.founder().map_err(anyhow::Error::msg)?;
        anyhow::ensure!(
            declared == founder,
            "--founder-policy {}={}: the file declares the {} lineage",
            founder.name(),
            path.display(),
            declared.name(),
        );
        out.push((founder, file.driver().map_err(anyhow::Error::msg)?));
    }
    Ok(out)
}

/// Install the ambient run's founder controllers over a whole layer: every lineage keeps
/// the heuristic the seeder registered unless `--founder-policy` named a saved GRU for
/// it, in which case that lineage's **birth factory and every standing body of that
/// lineage** are switched to the policy. One factory per lineage, and `EpisodeDriver::fresh`
/// per body, so no two bodies share a hidden state.
///
/// A layer that *remembers* being policy-driven — [`Fauna::policy_driven`], which a
/// snapshot carries — and is given no flag for that lineage is **refused here**, not
/// quietly demoted to a heuristic: a world whose animals were a trained policy is not
/// the same world with the heuristic put back, and silently substituting one is exactly
/// the re-anchoring `always-fresh-never-migrate` forbids.
fn install_founder_controllers(
    fauna: &mut Fauna,
    drivers: &[(Founder, EpisodeDriver)],
) -> Result<()> {
    for founder in Founder::ALL {
        if fauna.policy_driven(founder) && !drivers.iter().any(|(f, _)| *f == founder) {
            bail!(
                "this world's {} founders were driven by a saved policy; pass \
                 `--founder-policy {}=<centre.json>` to run them again, or start a fresh \
                 world. They are not silently put back on the heuristic.",
                founder.name(),
                founder.name(),
            );
        }
    }
    for (founder, driver) in drivers {
        let driver = driver.clone();
        fauna.set_founder_factory(
            *founder,
            std::sync::Arc::new(move || -> Box<dyn Controller> { driver.fresh() }),
        );
        fauna.set_policy_driven(*founder, true);
    }
    // Re-install over every standing founder body, so a loaded layer's bodies and the
    // bodies a policy flag took over are both driven by the lineage's current recipe.
    let bodies: Vec<(u64, Founder)> = fauna
        .view()
        .animals
        .iter()
        .filter_map(|a| a.founder.map(|f| (a.id, f)))
        .collect();
    for (id, founder) in bodies {
        if drivers.iter().any(|(f, _)| *f == founder) {
            anyhow::ensure!(
                fauna.install_founder_controller(id, founder),
                "the {} policy could not be installed on body {id}",
                founder.name()
            );
        }
    }
    Ok(())
}

/// Select the exact controller body a sensing-arena run installs. Policy files validate
/// their own founder and digest before handing out an [`EpisodeDriver`], so this host
/// only has to reject a user spelling that asks a blind arena to run a browser policy.
fn sensing_driver(
    args: &Voxel,
    arena: VoxelArenaArg,
) -> Result<(Founder, cubarium_search::es::voxel::EpisodeDriver)> {
    use cubarium_search::es::voxel::VoxelControl;

    let founder = match arena {
        VoxelArenaArg::Blind => Founder::Blind,
        VoxelArenaArg::Browser => Founder::Browser,
    };
    let driver = match args.controller {
        VoxelControllerArg::Heuristic => EpisodeDriver::control(VoxelControl::Heuristic, founder),
        VoxelControllerArg::NoIntake => EpisodeDriver::control(VoxelControl::NoIntake, founder),
        VoxelControllerArg::StationaryFeeding => {
            EpisodeDriver::control(VoxelControl::StationaryFeeding, founder)
        }
        VoxelControllerArg::Gru => {
            let path = args
                .policy
                .as_ref()
                .expect("Voxel::validate required --policy");
            let policy = VoxelPolicyFile::load(path).map_err(anyhow::Error::msg)?;
            let policy_founder = policy.founder().map_err(anyhow::Error::msg)?;
            anyhow::ensure!(
                policy_founder == founder,
                "{} policy cannot run the {} arena",
                policy_founder.name(),
                founder.name(),
            );
            policy.driver().map_err(anyhow::Error::msg)?
        }
    };
    Ok((founder, driver))
}

/// An opt-in stderr observer placed directly around the controller seam. It receives
/// exactly the same packet as its wrapped controller, emits one line only when the
/// founder's cadence calls `drive`, and never inspects world state or the target layout.
struct DiagnosticController {
    founder: Founder,
    inner: Box<dyn Controller>,
    samples: u64,
}

impl DiagnosticController {
    fn new(founder: Founder, inner: Box<dyn Controller>) -> DiagnosticController {
        DiagnosticController {
            founder,
            inner,
            samples: 0,
        }
    }
}

impl Controller for DiagnosticController {
    fn drive(&mut self, observation: &[f64]) -> Response {
        let response = self.inner.drive(observation);
        self.samples += 1;
        let nonzero = observation.iter().filter(|v| **v != 0.0).count();
        eprintln!(
            "cubarium sensing: {} sample {} inputs={} nonzero={} response={response:?}",
            self.founder.name(),
            self.samples,
            observation.len(),
            nonzero,
        );
        response
    }

    fn reset(&mut self) {
        self.samples = 0;
        self.inner.reset();
    }
}

/// Where the run's frames come from: the CPU presenter into a [`FrameSink`], or the
/// GPU's slab-walk shader straight onto a target.
///
/// The two cannot be one `dyn FrameSink`, and the reason is the whole point of the GPU
/// path: a `FrameSink` consumes *pixels*, and `--sink gpu` exists so that nobody
/// rasterises the strip on the CPU. `FrameSink`'s `observe_world` hook is no help here
/// either — it carries the ring's `RenderView`, and this world is a voxel strip. So the
/// loop names the two, and everything they share is these four methods.
enum Out {
    Cpu {
        sink: Box<dyn FrameSink>,
        presenter: VoxelPresenter,
        canvas: Canvas,
        raster: cube_proto::Raster,
    },
    Gpu(Box<VoxelGpuSink>),
}

impl Out {
    fn cpu(sink: Box<dyn FrameSink>, cfg: VoxelConfig, proj: Projection) -> Out {
        let topology = Topology::Ring {
            w: proj.raster_w,
            h: proj.raster_h,
        };
        Out::Cpu {
            sink,
            presenter: VoxelPresenter::new(cfg, proj),
            canvas: Canvas::new(topology, Scale::ONE),
            raster: cube_proto::Raster::black(proj.raster_w, proj.raster_h),
        }
    }

    /// Draw one frame. `moved` says the world has changed since the last one, which is
    /// what the GPU path packs a new voxel texture on; the CPU path reads the world
    /// afresh every frame and ignores it.
    fn render(&mut self, world: &World, flora: &Flora, fauna: &Fauna, moved: bool) -> Result<()> {
        match self {
            Out::Cpu {
                sink,
                presenter,
                canvas,
                raster,
            } => {
                presenter.draw_with_fauna(&world.view(), flora.view(), Some(fauna.view()), canvas);
                canvas.encode_raster(raster);
                sink.submit(Output::Ring(raster))
            }
            Out::Gpu(gpu) => {
                if moved {
                    gpu.stage_world(world, flora, fauna);
                }
                gpu.render()
            }
        }
    }

    fn observe_tick(&mut self, tick: u64) {
        if let Out::Cpu { sink, .. } = self {
            sink.observe_tick(tick);
        }
    }

    fn should_quit(&mut self) -> bool {
        match self {
            Out::Cpu { sink, .. } => sink.should_quit(),
            Out::Gpu(gpu) => gpu.should_quit(),
        }
    }

    fn finish(&mut self) -> Result<()> {
        match self {
            Out::Cpu { sink, .. } => sink.finish(),
            Out::Gpu(gpu) => gpu.finish(),
        }
    }
}

/// The run state a stdin command may change, and the one function that changes it.
///
/// It owns the scalars and borrows the world per call rather than the other way round,
/// so the loop can read `paused` and `speed` while it still holds the world.
struct Control {
    speed: f64,
    paused: bool,
    pending_steps: u32,
    outlet: bool,
    quit: bool,
    proj: Projection,
}

impl Control {
    fn new(speed: f64, proj: Projection) -> Control {
        Control {
            speed,
            paused: false,
            pending_steps: 0,
            outlet: false,
            quit: false,
            proj,
        }
    }

    /// Apply one stdin line. Anything unrecognised prints the usage and changes nothing;
    /// a bad argument is reported and the world is left alone.
    ///
    /// The plant and animal layers come in as further borrows rather than living in
    /// `Control`, for the same reason the world does: the loop reads `paused` and `speed`
    /// while it still holds all three.
    fn handle(&mut self, world: &mut World, flora: &mut Flora, fauna: &mut Fauna, line: &str) {
        let line = line.trim();
        if line.is_empty() {
            return;
        }
        let mut parts = line.split_whitespace();
        let verb = parts.next().unwrap_or_default();
        let rest: Vec<&str> = parts.collect();

        match verb {
            "q" | "quit" => self.quit = true,
            "p" | "pause" => {
                self.paused = !self.paused;
                eprintln!(
                    "cubarium voxel: {}",
                    if self.paused { "paused" } else { "running" }
                );
            }
            "s" | "step" => {
                self.paused = true;
                self.pending_steps += 1;
            }
            "+" => {
                self.speed = (self.speed * 2.0).min(MAX_SPEED);
                eprintln!("cubarium voxel: speed {}", self.speed);
            }
            "-" => {
                self.speed = (self.speed / 2.0).max(MIN_SPEED);
                eprintln!("cubarium voxel: speed {}", self.speed);
            }
            "r" | "rain" => {
                let volume = match rest.first() {
                    None => DEFAULT_RAIN_M3,
                    Some(text) => match text.parse::<f64>() {
                        Ok(v) if v.is_finite() && v > 0.0 => v,
                        _ => {
                            eprintln!("cubarium voxel: `r [m3]` wants a positive volume");
                            return;
                        }
                    },
                };
                let took = world.apply(VoxelCommand::RainPulse { volume_m3: volume });
                eprintln!("cubarium voxel: rain {volume} m3, accepted {took} m3");
            }
            "a" | "aquifer" => {
                let volume = match rest.first().map(|t| t.parse::<f64>()) {
                    Some(Ok(v)) if v.is_finite() && v != 0.0 => v,
                    _ => {
                        eprintln!(
                            "cubarium voxel: `a M3` wants a nonzero volume (negative withdraws)"
                        );
                        return;
                    }
                };
                let took = world.apply(VoxelCommand::ChargeAquifer { volume_m3: volume });
                eprintln!(
                    "cubarium voxel: aquifer {volume:+} m3, accepted {took:+} m3, head now {:.3} m",
                    world.aquifer_head_m()
                );
            }
            "o" | "outlet" => {
                self.outlet = !world.outlet_open();
                world.apply(VoxelCommand::SetOutlet { open: self.outlet });
                eprintln!(
                    "cubarium voxel: outlet {}",
                    if self.outlet { "open" } else { "closed" }
                );
            }
            "w" | "save" => match rest.first() {
                // Terrain only, and the message says so: `World::save` holds the voxels,
                // the water and the world's tick, and neither layer of the ecology.
                Some(path) => match std::fs::write(path, world.save()) {
                    Ok(()) => eprintln!(
                        "cubarium voxel: saved {path} — terrain and water only, no plants \
                         and no animals"
                    ),
                    Err(e) => eprintln!("cubarium voxel: saving {path}: {e}"),
                },
                None => eprintln!("cubarium voxel: `w PATH` needs a path (terrain only)"),
            },
            "l" | "load" => match rest.first() {
                Some(path) => {
                    // A terrain load is not an ecosystem restore (Astra R9.6): refuse it
                    // in place while anything of the ecology is standing, and say what.
                    if let Some(why) = ecology_state(flora, fauna) {
                        eprintln!(
                            "cubarium voxel: `l` loads the **terrain only** and this run has \
                             {why}; an in-place load would leave them on another world's \
                             terrain with ledgers describing this one. Restart with \
                             `--load {path}`, which starts the ecology fresh on that terrain."
                        );
                        return;
                    }
                    let read = std::fs::read(path)
                        .map_err(anyhow::Error::from)
                        .and_then(|b| World::load(&b));
                    match read {
                        Ok(loaded) => {
                            // The projection is fixed for the life of the run, so a world
                            // of a different shape would draw into the wrong raster.
                            let c = loaded.config();
                            if (c.width, c.height, c.depth)
                                != (self.proj.width, self.proj.height, self.proj.depth)
                            {
                                eprintln!(
                                    "cubarium voxel: {path} is {}x{}x{}, this run draws \
                                     {}x{}x{}; restart with a matching config",
                                    c.width,
                                    c.height,
                                    c.depth,
                                    self.proj.width,
                                    self.proj.height,
                                    self.proj.depth
                                );
                            } else {
                                *world = loaded;
                                self.outlet = world.outlet_open();
                                eprintln!(
                                    "cubarium voxel: loaded the terrain of {path} at tick \
                                     {} — the plants and the animals are this run's own",
                                    world.tick()
                                );
                            }
                        }
                        Err(e) => eprintln!("cubarium voxel: loading {path}: {e:#}"),
                    }
                }
                None => eprintln!("cubarium voxel: `l PATH` needs a path (terrain only)"),
            },
            "i" | "inspect" => {
                let Some((x, y, z)) = coords(world, "i X Y Z", &rest) else {
                    return;
                };
                eprintln!("cubarium voxel: {}", cell_state(world, x, y, z));
                // The site is the cell itself, not the column's skyline: `i` on a
                // support face reports what grows there and what the ground holds.
                let site = site_of(world, x, y, z);
                let fv = flora.view();
                match fv.stand_at(site) {
                    Some(s) => eprintln!(
                        "cubarium voxel:   {} {:?} W {:.4} P {:.4} Q {:.4} mineral {:.5} \
                         light {:.3} moisture {:.3} aeration stress {:.3}",
                        s.species.name(),
                        s.stage,
                        s.wood,
                        s.foliage,
                        s.reserve,
                        s.mineral,
                        s.light,
                        s.moisture,
                        s.aeration_stress,
                    ),
                    None => eprintln!("cubarium voxel:   no stand"),
                }
                // The animals standing on this face, in id order. More than one is
                // ordinary: a newborn appears on its parent's face.
                for a in fauna.view().animals_at(site) {
                    eprintln!(
                        "cubarium voxel:   {} #{} body {:.4} reserve {:.4} mineral {:.5} \
                         energy {:.4} age {} ticks {:?}",
                        a.species.name(),
                        a.id,
                        a.body,
                        a.reserve,
                        a.mineral,
                        a.energy,
                        a.age_ticks,
                        a.state,
                    );
                }
                if let Some(g) = fv.ground_at(site) {
                    eprintln!(
                        "cubarium voxel:   ground N {:.4} litter {:.4} ({:.4} energy, \
                         {:.5} mineral) dead wood {:.4} ({:.5} mineral)",
                        g.mineral,
                        g.litter,
                        g.litter_energy,
                        g.litter_mineral,
                        g.dead_wood,
                        g.dead_wood_mineral,
                    );
                    // The seed bank, in the order the ground holds it: species, then
                    // arrival bin, oldest first. The age is the bin's, measured from the
                    // tick its window opened.
                    for c in &g.seeds {
                        eprintln!(
                            "cubarium voxel:   seed {} organic {:.6} mineral {:.7} age {} ticks \
                             (bin from tick {})",
                            c.species.name(),
                            c.organic,
                            c.mineral,
                            c.age_ticks(fv.tick),
                            c.bin_start_tick,
                        );
                    }
                    if g.seeds.is_empty() {
                        eprintln!("cubarium voxel:   no seed cohort");
                    }
                }
            }
            "f" | "flora" => {
                let usage = "f X Z bloomcrown|umbrellafrond [wood]";
                let Some((x, z)) = column(world, usage, &rest) else {
                    return;
                };
                let Some(species) = rest.get(2).and_then(|n| Species::parse(n)) else {
                    eprintln!(
                        "cubarium voxel: `{usage}` — `{}` is not a species",
                        rest.get(2).copied().unwrap_or(""),
                    );
                    return;
                };
                // A founder with no wood named is a full-grown one: the picture this
                // command exists for is a stand you can see.
                let wood = match rest.get(3) {
                    None => flora.config().species(species).wood_max,
                    Some(text) => match text.parse::<f64>() {
                        Ok(v) if v.is_finite() && v > 0.0 => v,
                        _ => {
                            eprintln!("cubarium voxel: `{usage}` wants a positive wood");
                            return;
                        }
                    },
                };
                if flora.apply(
                    world,
                    FloraCommand::Seed {
                        x,
                        z,
                        species,
                        wood,
                    },
                ) {
                    let site = highest_site(world, x, z);
                    eprintln!(
                        "cubarium voxel: seeded {} at {:?} with W {wood}",
                        species.name(),
                        site,
                    );
                } else {
                    eprintln!(
                        "cubarium voxel: no stand at ({x}, {z}): either the column has no \
                         support face, one already grows there, or W {wood} is below \
                         {}'s alive_min {}",
                        species.name(),
                        flora.config().species(species).alive_min,
                    );
                }
            }
            "c" | "clear" => {
                let Some((x, z)) = column(world, "c X Z", &rest) else {
                    return;
                };
                if flora.apply(world, FloraCommand::Clear { x, z }) {
                    eprintln!("cubarium voxel: cleared the stand at ({x}, {z})");
                } else {
                    eprintln!("cubarium voxel: no stand to clear at ({x}, {z})");
                }
            }
            "g" | "grazer" => {
                let usage = "g X Z frondgrazer [body]";
                let Some((x, z)) = column(world, usage, &rest) else {
                    return;
                };
                let Some(species) = rest.get(2).and_then(|n| Beast::parse(n)) else {
                    eprintln!(
                        "cubarium voxel: `{usage}` — `{}` is not an animal",
                        rest.get(2).copied().unwrap_or(""),
                    );
                    return;
                };
                // No body named is a grown one: the picture this command exists for is an
                // animal you can see, as `f`'s is a stand you can see.
                let body = match rest.get(3) {
                    None => fauna.config().species(species).body_max,
                    Some(text) => match text.parse::<f64>() {
                        Ok(v) if v.is_finite() && v > 0.0 => v,
                        _ => {
                            eprintln!("cubarium voxel: `{usage}` wants a positive body");
                            return;
                        }
                    },
                };
                if fauna.apply(
                    world,
                    FaunaCommand::Introduce {
                        x,
                        z,
                        species,
                        body,
                    },
                ) {
                    eprintln!(
                        "cubarium voxel: introduced {} at {:?} with body {body}",
                        species.name(),
                        highest_site(world, x, z),
                    );
                } else {
                    eprintln!(
                        "cubarium voxel: no animal at ({x}, {z}): either the column has no \
                         support face, or body {body} is below {}'s body_min {}",
                        species.name(),
                        fauna.config().species(species).body_min,
                    );
                }
            }
            "m" | "material" => {
                let usage = "m X Y Z air|rock|soil|bedrock";
                let Some((x, y, z)) = coords(world, usage, &rest) else {
                    return;
                };
                let material = match rest.get(3).map(|w| w.to_ascii_lowercase()).as_deref() {
                    Some("air") => Material::Air,
                    Some("rock") => Material::Rock,
                    Some("soil") => Material::Soil,
                    Some("bedrock") => Material::Bedrock,
                    other => {
                        eprintln!(
                            "cubarium voxel: `{usage}` — `{}` is not a material",
                            other.unwrap_or("")
                        );
                        return;
                    }
                };
                // The core moves the water the old material held; the new state is the
                // receipt, so a displaced cell says so itself.
                world.apply(VoxelCommand::SetMaterial { x, y, z, material });
                eprintln!(
                    "cubarium voxel: set {material:?}, now {}",
                    cell_state(world, x, y, z)
                );
            }
            other => eprintln!("cubarium voxel: `{other}`? — {COMMANDS}"),
        }
    }
}

/// `X Y Z` from a command's arguments, or `None` with the reason printed.
///
/// `x` wraps — the strip is a ring and has no end to fall off — so any integer is a
/// column. `y` and `z` have real ends, so one outside the world is refused and named
/// rather than folded into a cell the caller did not mean.
/// What a terrain-only load would leave standing, in words, or `None` when this run holds
/// no ecology at all and `World::load` can safely replace the terrain under it.
///
/// Astra R9.6: `World::save`/`World::load` carry the voxels, the water and the world's
/// tick, and neither layer of the ecology — so an in-place load against living stands is
/// not a restore of anything. The checks are the review's list: any stand, any animal, any
/// seed bank, any provisioned ground, and a nonzero ledger on either layer (compared
/// against `Default`, so a term added later is covered without touching this).
fn ecology_state(flora: &Flora, fauna: &Fauna) -> Option<String> {
    let fv = flora.view();
    let av = fauna.view();
    if !fv.stands.is_empty() {
        return Some(format!("{} stand(s) growing", fv.stands.len()));
    }
    if !av.animals.is_empty() {
        return Some(format!("{} animal(s) alive", av.animals.len()));
    }
    let banks = fv.ground.iter().filter(|g| !g.seeds.is_empty()).count();
    if banks > 0 {
        return Some(format!("{banks} site(s) holding a seed bank"));
    }
    if !fv.ground.is_empty() {
        return Some(format!("{} provisioned ground site(s)", fv.ground.len()));
    }
    if *fv.ledger != FloraLedger::default() {
        return Some("a nonzero plant ledger".to_string());
    }
    if *av.ledger != FaunaLedger::default() {
        return Some("a nonzero animal ledger".to_string());
    }
    None
}

fn coords(world: &World, usage: &str, rest: &[&str]) -> Option<(i64, u32, u32)> {
    let triple = (
        rest.first().and_then(|t| t.parse::<i64>().ok()),
        rest.get(1).and_then(|t| t.parse::<u32>().ok()),
        rest.get(2).and_then(|t| t.parse::<u32>().ok()),
    );
    let (Some(x), Some(y), Some(z)) = triple else {
        eprintln!("cubarium voxel: `{usage}` wants three integers for X Y Z");
        return None;
    };
    let c = world.config();
    if y >= c.height || z >= c.depth {
        eprintln!(
            "cubarium voxel: ({x}, {y}, {z}) is outside a {}x{}x{} world",
            c.width, c.height, c.depth
        );
        return None;
    }
    Some((x, y, z))
}

/// `X Z` from a command's arguments: a column, for the commands that act on whichever
/// support face is the highest one in it. `x` wraps; `z` is refused if it is outside.
fn column(world: &World, usage: &str, rest: &[&str]) -> Option<(i64, u32)> {
    let pair = (
        rest.first().and_then(|t| t.parse::<i64>().ok()),
        rest.get(1).and_then(|t| t.parse::<u32>().ok()),
    );
    let (Some(x), Some(z)) = pair else {
        eprintln!("cubarium voxel: `{usage}` wants two integers for X Z");
        return None;
    };
    let c = world.config();
    if z >= c.depth {
        eprintln!(
            "cubarium voxel: z = {z} is outside a {}-deep world",
            c.depth
        );
        return None;
    }
    Some((x, z))
}

/// The flora site a cell names: the cell itself, with `x` wrapped as a site stores it.
fn site_of(world: &World, x: i64, y: u32, z: u32) -> Site {
    Site {
        x: x.rem_euclid(i64::from(world.config().width)) as u32,
        y,
        z,
    }
}

/// The site `f` and `c` act on: the highest support face of a column, which is what
/// `cubarium_voxel_flora::highest_support` picks, so the echo names the cell the command
/// really touched.
fn highest_site(world: &World, x: i64, z: u32) -> Option<Site> {
    cubarium_voxel_flora::highest_support(&world.view(), x, z)
}

/// One cell's state as `i` reports it, and as `m` echoes back after an edit.
fn cell_state(world: &World, x: i64, y: u32, z: u32) -> String {
    let view = world.view();
    format!(
        "({}, {y}, {z}) {:?} free {:.4} pore {:.4}",
        x.rem_euclid(i64::from(view.config.width)),
        view.material_at(x, y, z),
        view.free_at(x, y, z),
        view.pore_at(x, y, z),
    )
}

/// Read stdin lines on a thread and hand them to the loop.
///
/// Detached on purpose: a blocking `read_line` cannot be cancelled portably, and the
/// process is about to exit anyway. When stdin closes the thread ends and the loop
/// simply stops getting commands.
fn spawn_stdin_reader() -> Receiver<String> {
    let (tx, rx) = mpsc::channel();
    std::thread::Builder::new()
        .name("cubarium-voxel-stdin".into())
        .spawn(move || {
            use std::io::BufRead;
            let stdin = std::io::stdin();
            for line in stdin.lock().lines() {
                match line {
                    Ok(line) => {
                        if tx.send(line).is_err() {
                            return;
                        }
                    }
                    Err(_) => return,
                }
            }
        })
        .expect("spawning the voxel stdin reader");
    rx
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The documented defaults, and a partial config file that only overrides some of
    /// them: everything else keeps the default rather than being refused.
    #[test]
    fn the_config_defaults_are_the_documented_ones_and_a_partial_file_loads() {
        let d = VoxelConfig::default();
        assert_eq!(d.tilt_degrees, 30.0);
        assert_eq!(d.px_per_voxel, 4);
        assert_eq!(d.raster_height, 0);
        assert_eq!(d.haze, 0.55);
        assert_eq!(d.water_alpha, 0.5);
        assert_eq!(d.world, cubarium_voxel::Config::default());

        let cfg: VoxelConfig =
            toml::from_str("tilt_degrees = 35.0\n[world]\nwidth = 64\ndepth = 8\n").unwrap();
        assert_eq!(cfg.tilt_degrees, 35.0);
        assert_eq!(
            cfg.px_per_voxel, 4,
            "an unmentioned field keeps its default"
        );
        assert_eq!(cfg.world.width, 64);
        assert_eq!(cfg.world.depth, 8);
        assert_eq!(cfg.world.height, cubarium_voxel::Config::default().height);

        // A typo is an error, not a silently ignored key.
        assert!(toml::from_str::<VoxelConfig>("tilt_degree = 35.0\n").is_err());
        assert!(toml::from_str::<VoxelConfig>("[world]\nwidht = 64\n").is_err());
    }

    /// The committed example config parses, is the default picture, and does not pin the
    /// world depth: that follows `cubarium_voxel::Config::default()`, so changing the
    /// habitat's depth in the core changes the example without editing it.
    #[test]
    fn the_example_config_parses_and_leaves_the_depth_to_the_core() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("voxel.example.toml");
        let cfg = load_config(&path).unwrap();
        assert_eq!(cfg, VoxelConfig::default());
        assert_eq!(cfg.world.depth, cubarium_voxel::Config::default().depth);

        let text = std::fs::read_to_string(&path).unwrap();
        assert!(
            !text.lines().any(|l| l.trim_start().starts_with("depth")),
            "the example must not set `depth`; it mirrors the core default"
        );
        // The chosen camera, written down where a reader of the file will find it.
        assert!(text.contains("tilt_degrees = 30.0") && text.contains("px_per_voxel = 4"));
    }

    /// A config file whose `[world]` no world can be built from is an error, not a panic:
    /// `World::new` asserts, so the run validates first.
    #[test]
    fn a_world_no_world_can_be_built_from_is_refused_not_panicked_on() {
        let cfg: VoxelConfig =
            toml::from_str("[world]\ndepth = 0\n").expect("it parses; it is just impossible");
        let err = cfg
            .world
            .validate()
            .expect_err("a zero dimension must be refused");
        assert!(format!("{err}").contains("nonzero dimensions"), "{err}");

        let bad: VoxelConfig = toml::from_str("[world]\nvoxel_m = 0.0\n").unwrap();
        assert!(bad.world.validate().is_err());
        assert!(VoxelConfig::default().world.validate().is_ok());
    }

    /// Stdin commands reach the world: pause toggles, speed halves and doubles, a
    /// single step is owed, and a nonsense line changes nothing.
    #[test]
    fn stdin_commands_change_exactly_what_they_name() {
        let c = cubarium_voxel::Config {
            width: 16,
            height: 8,
            depth: 2,
            ..Default::default()
        };
        let mut world = World::empty(c.clone());
        let mut flora = Flora::new(FloraConfig::default());
        let mut fauna = Fauna::new(FaunaConfig::default());
        let mut ctl = Control::new(1.0, Projection::new(30.0, 4, 0, &c).unwrap());

        ctl.handle(&mut world, &mut flora, &mut fauna, "p");
        assert!(ctl.paused);
        ctl.handle(&mut world, &mut flora, &mut fauna, "p");
        assert!(!ctl.paused);
        ctl.handle(&mut world, &mut flora, &mut fauna, "-");
        assert_eq!(ctl.speed, 0.5);
        ctl.handle(&mut world, &mut flora, &mut fauna, "+");
        ctl.handle(&mut world, &mut flora, &mut fauna, "+");
        assert_eq!(ctl.speed, 2.0);
        ctl.handle(&mut world, &mut flora, &mut fauna, "s");
        assert!(ctl.paused && ctl.pending_steps == 1);
        ctl.handle(&mut world, &mut flora, &mut fauna, "o");
        assert!(ctl.outlet);
        // The aquifer takes a charge, gives back only what it holds, and is booked.
        ctl.handle(&mut world, &mut flora, &mut fauna, "a 2.5");
        assert_eq!(world.view().aquifer_m3, 2.5);
        ctl.handle(&mut world, &mut flora, &mut fauna, "a -10");
        assert_eq!(world.view().aquifer_m3, 0.0);
        assert_eq!(world.view().ledger.user_in, 0.0);
        // Nonsense, and a bad argument, change nothing.
        ctl.handle(&mut world, &mut flora, &mut fauna, "nonsense 1 2 3");
        ctl.handle(&mut world, &mut flora, &mut fauna, "r not-a-volume");
        ctl.handle(&mut world, &mut flora, &mut fauna, "a");
        ctl.handle(&mut world, &mut flora, &mut fauna, "a 0");
        ctl.handle(&mut world, &mut flora, &mut fauna, "a nan");
        assert_eq!(world.view().aquifer_m3, 0.0);
        ctl.handle(&mut world, &mut flora, &mut fauna, "i 0 999 0");
        ctl.handle(&mut world, &mut flora, &mut fauna, "i only-one");
        ctl.handle(&mut world, &mut flora, &mut fauna, "w");
        assert_eq!((ctl.speed, ctl.pending_steps, ctl.outlet), (2.0, 1, true));
        assert!(!ctl.quit);
        ctl.handle(&mut world, &mut flora, &mut fauna, "i 0 0 0");
        ctl.handle(&mut world, &mut flora, &mut fauna, "q");
        assert!(ctl.quit);
    }

    /// **R9.6: `w` and `l` are terrain only, and an in-place load is refused while
    /// anything of the ecology is standing.** A saved terrain, one edited cell, and then a
    /// stand: `l` refuses and the edit survives, because loading would have put the stand
    /// on another world's terrain with its ledger still describing this one. The same
    /// bytes load into a run that holds no ecology, which is what `--load` does at startup.
    /// **A world that remembers a policy is not quietly put back on the heuristic.**
    ///
    /// `install_founder_controllers` switches a named lineage's birth factory and every
    /// standing body of that lineage onto the driver, and records that the lineage is
    /// policy-driven. A layer that already carries that record and is offered no driver
    /// for it is refused by name. The stand-in driver here is the heuristic control —
    /// the refusal and the install are about *which lineage a driver was supplied for*,
    /// not about what is inside it, and using a control keeps weights out of a unit test.
    #[test]
    fn a_policy_driven_lineage_is_refused_rather_than_demoted_on_load() {
        use cubarium_search::es::voxel::VoxelControl;
        use cubarium_voxel_fauna::StartingStores;

        let world = crate::voxel::scene::authored(cubarium_voxel::Config {
            width: 32,
            height: 16,
            depth: 4,
            ..cubarium_voxel::Config::default()
        });
        let mut fauna = Fauna::new(FaunaConfig::default());
        crate::voxel::habitat::install_heuristics(&mut fauna);
        let placed = (0..32i64).any(|x| {
            fauna.apply(
                &world,
                FaunaCommand::IntroduceFounder {
                    x,
                    z: 1,
                    founder: Founder::Blind,
                    stores: StartingStores::HUNGRY,
                    heading_rad: 0.0,
                },
            )
        });
        assert!(placed, "a founder body stands somewhere on the strip");

        // Nothing declared: the heuristics stand and nothing is refused.
        install_founder_controllers(&mut fauna, &[]).expect("the default is the heuristics");
        assert!(!fauna.policy_driven(Founder::Blind));

        // A driver for the blind lineage takes it over and is recorded.
        let driver = EpisodeDriver::control(VoxelControl::Heuristic, Founder::Blind);
        install_founder_controllers(&mut fauna, &[(Founder::Blind, driver)])
            .expect("a supplied driver installs");
        assert!(fauna.policy_driven(Founder::Blind));
        assert!(!fauna.policy_driven(Founder::Browser));

        // That record survives a save and load, and the loaded layer refuses to run
        // without the flag rather than falling back.
        let mut loaded = Fauna::load(&fauna.save()).expect("the layer round-trips");
        crate::voxel::habitat::install_heuristics(&mut loaded);
        let err = format!(
            "{:#}",
            install_founder_controllers(&mut loaded, &[]).expect_err("a demotion in silence")
        );
        assert!(err.contains("littershredder"), "{err}");
        assert!(err.contains("--founder-policy"), "{err}");

        // With the flag back, it loads.
        let driver = EpisodeDriver::control(VoxelControl::Heuristic, Founder::Blind);
        install_founder_controllers(&mut loaded, &[(Founder::Blind, driver)])
            .expect("the policy supplied again");
    }

    #[test]
    fn a_terrain_load_is_refused_while_the_ecology_is_standing() {
        let c = cubarium_voxel::Config {
            width: 16,
            height: 8,
            depth: 2,
            ..Default::default()
        };
        let proj = Projection::new(30.0, 4, 0, &c).unwrap();
        let path =
            std::env::temp_dir().join(format!("cubarium-voxel-terrain-{}.bin", std::process::id()));
        let at = path.to_str().expect("a utf-8 temp path").to_string();

        let mut world = World::empty(c.clone());
        let mut flora = Flora::new(FloraConfig::default());
        let mut fauna = Fauna::new(FaunaConfig::default());
        let mut ctl = Control::new(1.0, proj);
        assert_eq!(
            ecology_state(&flora, &fauna),
            None,
            "a fresh run holds no ecology"
        );

        ctl.handle(&mut world, &mut flora, &mut fauna, &format!("w {at}"));
        assert!(path.exists(), "the terrain was saved");

        // A cell edited after the save, so a load that happened would be visible.
        ctl.handle(&mut world, &mut flora, &mut fauna, "m 3 2 1 soil");
        assert_eq!(world.view().material_at(3, 2, 1), Material::Soil);

        // Something alive. `f` seeds a stand on the column's highest support face.
        ctl.handle(&mut world, &mut flora, &mut fauna, "f 3 1 bloomcrown");
        assert_eq!(flora.view().stands.len(), 1, "a stand is standing");
        assert!(ecology_state(&flora, &fauna).unwrap().contains("stand"));

        ctl.handle(&mut world, &mut flora, &mut fauna, &format!("l {at}"));
        assert_eq!(
            world.view().material_at(3, 2, 1),
            Material::Soil,
            "the load was refused and the world was left alone"
        );
        assert_eq!(flora.view().stands.len(), 1, "and so was the stand");

        // Ground and a nonzero ledger are enough on their own: clearing the stand leaves
        // both, and the refusal stands.
        ctl.handle(&mut world, &mut flora, &mut fauna, "c 3 1");
        assert!(flora.view().stands.is_empty());
        let why = ecology_state(&flora, &fauna).expect("the ground and the ledger remain");
        assert!(why.contains("ground") || why.contains("ledger"), "{why}");
        ctl.handle(&mut world, &mut flora, &mut fauna, &format!("l {at}"));
        assert_eq!(
            world.view().material_at(3, 2, 1),
            Material::Soil,
            "still refused"
        );

        // The same bytes into a run with no ecology in it: accepted, and the edit is gone.
        let mut fresh_flora = Flora::new(FloraConfig::default());
        let mut fresh_fauna = Fauna::new(FaunaConfig::default());
        ctl.handle(
            &mut world,
            &mut fresh_flora,
            &mut fresh_fauna,
            &format!("l {at}"),
        );
        assert_eq!(
            world.view().material_at(3, 2, 1),
            Material::Air,
            "the terrain came back as it was saved"
        );

        // And an animal alone is enough to refuse.
        ctl.handle(
            &mut world,
            &mut fresh_flora,
            &mut fresh_fauna,
            "m 3 2 1 soil",
        );
        let mut fauna = Fauna::new(FaunaConfig::default());
        assert!(fauna.apply(
            &world,
            FaunaCommand::Introduce {
                x: 3,
                z: 1,
                species: Beast::Frondgrazer,
                body: 0.02,
            }
        ));
        let mut flora = Flora::new(FloraConfig::default());
        assert!(ecology_state(&flora, &fauna).unwrap().contains("animal"));
        ctl.handle(&mut world, &mut flora, &mut fauna, &format!("l {at}"));
        assert_eq!(
            world.view().material_at(3, 2, 1),
            Material::Soil,
            "refused for the animal"
        );

        let _ = std::fs::remove_file(&path);
        // Both help texts say what the two commands carry.
        assert!(COMMANDS.contains("terrain"), "{COMMANDS}");
    }

    /// Terrain editing from stdin: paused, `m` changes one cell, and the `i` that follows
    /// reads the new material off the same view it prints from. The interaction the local
    /// tool was missing — you can now dig a channel and watch the pool find it.
    #[test]
    fn a_paused_terrain_edit_changes_the_cell_the_next_inspect_reports() {
        let c = cubarium_voxel::Config {
            width: 16,
            height: 8,
            depth: 2,
            ..Default::default()
        };
        let mut world = World::empty(c.clone());
        let mut flora = Flora::new(FloraConfig::default());
        let mut fauna = Fauna::new(FaunaConfig::default());
        let mut ctl = Control::new(1.0, Projection::new(30.0, 4, 0, &c).unwrap());

        ctl.handle(&mut world, &mut flora, &mut fauna, "p");
        assert!(ctl.paused);
        assert_eq!(world.view().material_at(3, 2, 1), Material::Air);

        ctl.handle(&mut world, &mut flora, &mut fauna, "m 3 2 1 soil");
        assert_eq!(world.view().material_at(3, 2, 1), Material::Soil);
        // What the following `i` prints is this, off the edited world.
        let seen = cell_state(&world, 3, 2, 1);
        assert!(seen.contains("(3, 2, 1) Soil"), "{seen}");
        ctl.handle(&mut world, &mut flora, &mut fauna, "i 3 2 1");

        // `x` wraps, as everything in the ring does.
        ctl.handle(&mut world, &mut flora, &mut fauna, "m -13 2 1 rock");
        assert_eq!(world.view().material_at(3, 2, 1), Material::Rock);

        // A `y` or `z` outside the world, an unknown material and a short line are all
        // refused, and leave the cell alone.
        for bad in [
            "m 3 99 1 air",
            "m 3 2 9 air",
            "m 3 2 1 lava",
            "m 3 2 1",
            "m",
            "material",
        ] {
            ctl.handle(&mut world, &mut flora, &mut fauna, bad);
            assert_eq!(
                world.view().material_at(3, 2, 1),
                Material::Rock,
                "`{bad}` edited a cell"
            );
        }

        ctl.handle(&mut world, &mut flora, &mut fauna, "m 3 2 1 air");
        assert_eq!(world.view().material_at(3, 2, 1), Material::Air);
        // Both help texts really do offer the command that was just used.
        assert!(COMMANDS.contains("m X Y Z air|rock|soil|bedrock"));
    }

    /// `f`, `c` and the flora half of `i` parse: a founder lands on the highest support
    /// of the column named, a missing wood means a full-grown one, every bad line is
    /// refused without touching the stands, and `c` takes the stand back off again.
    #[test]
    fn the_flora_commands_seed_clear_and_inspect_exactly_what_they_name() {
        let c = cubarium_voxel::Config {
            width: 16,
            height: 8,
            depth: 2,
            ..Default::default()
        };
        let mut world = World::empty(c.clone());
        for z in 0..c.depth {
            for x in 0..i64::from(c.width) {
                for y in 0..=2 {
                    world.apply(VoxelCommand::SetMaterial {
                        x,
                        y,
                        z,
                        material: Material::Soil,
                    });
                }
            }
        }
        let mut flora = Flora::new(FloraConfig::default());
        let mut fauna = Fauna::new(FaunaConfig::default());
        let mut ctl = Control::new(1.0, Projection::new(30.0, 4, 0, &c).unwrap());
        let site = Site { x: 3, y: 2, z: 1 };

        // A founder with no wood named is full-grown: the stand this command exists to
        // put in the picture is one you can see.
        ctl.handle(&mut world, &mut flora, &mut fauna, "f 3 1 bloomcrown");
        let stand = *flora
            .view()
            .stand_at(site)
            .expect("a stand on the column's skyline");
        assert_eq!(stand.species, Species::Bloomcrown);
        assert_eq!(stand.wood, flora.config().bloomcrown.wood_max);
        assert_eq!(flora.view().stands.len(), 1);

        // An explicit wood is taken, and `x` wraps as everything in the ring does.
        ctl.handle(
            &mut world,
            &mut flora,
            &mut fauna,
            "f -12 0 umbrellafrond 0.25",
        );
        let other = *flora
            .view()
            .stand_at(Site { x: 4, y: 2, z: 0 })
            .expect("the wrapped column");
        assert_eq!(other.species, Species::Umbrellafrond);
        assert_eq!(other.wood, 0.25);

        // Everything refusable: a second stand on one site, an unknown species, a `z`
        // outside the world, wood below `alive_min`, a bad number, a short line.
        for bad in [
            "f 3 1 bloomcrown",
            "f 5 1 lichen",
            "f 5 9 bloomcrown",
            "f 5 1 bloomcrown 0.0001",
            "f 5 1 bloomcrown not-a-number",
            "f 5",
            "f",
            "flora",
        ] {
            ctl.handle(&mut world, &mut flora, &mut fauna, bad);
            assert_eq!(flora.view().stands.len(), 2, "`{bad}` changed the stands");
        }

        // `i` on the site reports the stand and its ground; on a bare cell it says so.
        // Neither prints anything the run can act on, so the assertion is that the
        // lookups are the ones the printout claims.
        assert!(
            flora.view().ground_at(site).is_some(),
            "seeding creates the ground"
        );
        assert!(flora.view().stand_at(Site { x: 9, y: 2, z: 1 }).is_none());
        ctl.handle(&mut world, &mut flora, &mut fauna, "i 3 2 1");
        ctl.handle(&mut world, &mut flora, &mut fauna, "i 9 2 1");
        ctl.handle(&mut world, &mut flora, &mut fauna, "i 3 9 1");

        // `c` takes the stand off the highest support of its column, once.
        ctl.handle(&mut world, &mut flora, &mut fauna, "c 3 1");
        assert!(flora.view().stand_at(site).is_none());
        assert_eq!(flora.view().stands.len(), 1);
        for bad in ["c 3 1", "c 3 9", "c 3", "c", "clear"] {
            ctl.handle(&mut world, &mut flora, &mut fauna, bad);
            assert_eq!(flora.view().stands.len(), 1, "`{bad}` changed the stands");
        }

        // The removal is booked, not hidden: the residual line the run prints is the
        // seeded material less what `c` took back.
        let fv = flora.view();
        assert!(fv.ledger.removed_organic_out > 0.0);
        assert!((fv.organic() - fv.ledger.expected_organic()).abs() < 1e-9);

        // Both help texts really do offer the commands that were just used.
        assert!(COMMANDS.contains("f X Z bloomcrown|umbrellafrond [wood]"));
        assert!(COMMANDS.contains("c X Z"));
    }

    /// `w` then `l` round-trips the world through the snapshot the core writes, and a
    /// world of a different shape is refused rather than drawn into the wrong raster.
    #[test]
    fn save_and_load_round_trip_and_a_mismatched_world_is_refused() {
        let c = cubarium_voxel::Config {
            width: 16,
            height: 8,
            depth: 2,
            ..Default::default()
        };
        let mut world = scene::authored(c.clone());
        let proj = Projection::new(30.0, 4, 0, &c).unwrap();
        let dir = std::env::temp_dir().join(format!("cubarium-voxel-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("world.bin");

        let before = world.view().stored_m3();
        let mut flora = Flora::new(FloraConfig::default());
        let mut fauna = Fauna::new(FaunaConfig::default());
        let mut ctl = Control::new(1.0, proj);
        ctl.handle(
            &mut world,
            &mut flora,
            &mut fauna,
            &format!("w {}", path.display()),
        );
        assert!(path.exists());

        let mut fresh = World::empty(c.clone());
        ctl.handle(
            &mut fresh,
            &mut flora,
            &mut fauna,
            &format!("l {}", path.display()),
        );
        assert!(
            (fresh.view().stored_m3() - before).abs() < 1e-9,
            "the pool came back"
        );

        // A world of another shape is named and refused.
        let other = dir.join("other.bin");
        let wide = cubarium_voxel::Config {
            width: 32,
            ..c.clone()
        };
        std::fs::write(&other, World::empty(wide).save()).unwrap();
        let mut keep = World::empty(c.clone());
        ctl.handle(
            &mut keep,
            &mut flora,
            &mut fauna,
            &format!("l {}", other.display()),
        );
        assert_eq!(keep.config().width, 16, "the run kept its own world");

        std::fs::remove_dir_all(&dir).unwrap();
    }
}
