//! **The console**: the run controls (pause, step, speed) and the development commands a
//! typed line applies to a running world. One copy: `cubarium voxel`'s stdin and a
//! session's `command` both go through [`Console::handle`], and the observatory's
//! [`cubarium_observatory::Command`] becomes a console line ([`console_line`]) so a
//! dashboard, a typed line and Godot change the run the same way.
//!
//! [`Pacer`] turns the console's speed and pause into whole world ticks per clock tick.

use std::path::PathBuf;
use std::time::SystemTime;

use cubarium_voxel::{Command as VoxelCommand, Material, World};
use cubarium_voxel_fauna::{Command as FaunaCommand, Fauna, FaunaLedger, Founder};
use cubarium_voxel_flora::{Command as FloraCommand, Flora, FloraLedger, Site, Species};
use cubarium_voxel_sim::habitat;

/// The console commands, in the one place both the banner and the usage line read them
/// from, so a new command cannot be added to only one of the two.
pub const COMMANDS: &str = "p [on|off] pause/resume, s [N] step, +/- speed, speed X set the speed, r [m3] rain, a M3 charge the aquifer (negative withdraws), \
                        h M3 add water to the atmosphere (closed budget only), \
                        m X Y Z air|rock|soil|bedrock set material, \
                        f X Z bloomcrown|umbrellafrond [wood] seed a stand, \
                        c X Z clear a stand, \
                        g X Z introduce a frondgrazer, \
                        e [N] bury N lanternjaw egg clutches where prey live, \
                        j [N] introduce N adult lanternjaws where prey live, \
                        k [N] found N chorister packs on covered dens near prey, \
                        save [DIR] save the whole world (default saves/<time>-t<tick>/), \
                        save-terrain PATH save the terrain only, \
                        l PATH load a terrain only (refused once anything is alive), \
                        i X Y Z inspect, o outlet, q quit";

/// Default rain volume for the `r` command, in cubic metres.
pub const DEFAULT_RAIN_M3: f64 = 1.0;
/// The speed is clamped to this range; zero is `p` (pause), not a speed.
pub const MIN_SPEED: f64 = 1.0 / 64.0;
pub const MAX_SPEED: f64 = 64.0;

/// The run state a command may change, and the one function that changes it.
///
/// It owns the scalars and borrows the world per call rather than the other way round,
/// so the loop can read `paused` and `speed` while it still holds the world. What a
/// command has to say is collected ([`Console::take_said`]) rather than printed: the
/// binary prints it as `cubarium voxel: …`, a session hands it to its reader.
pub struct Console {
    pub speed: f64,
    pub paused: bool,
    pub pending_steps: u32,
    pub outlet: bool,
    pub quit: bool,
    /// The run's `(width, height, depth)`: a terrain load of another shape is refused.
    pub dims: (u32, u32, u32),
    /// A whole-world save `save [DIR]` asked for, for the loop to start off the frame.
    pub save_to: Option<PathBuf>,
    /// Where `save` with no directory goes, from the time and the world's tick.
    pub default_save_dir: fn(SystemTime, u64) -> PathBuf,
    said: Vec<String>,
}

/// `saves/t<tick>`: a session's `save` with no directory.
pub fn tick_save_dir(_now: SystemTime, tick: u64) -> PathBuf {
    PathBuf::from("saves").join(format!("t{tick}"))
}

macro_rules! say {
    ($out:expr, $($arg:tt)*) => {
        $out.push(format!($($arg)*))
    };
}

impl Console {
    pub fn new(speed: f64, dims: (u32, u32, u32)) -> Console {
        Console {
            speed,
            paused: false,
            pending_steps: 0,
            outlet: false,
            quit: false,
            dims,
            save_to: None,
            default_save_dir: tick_save_dir,
            said: Vec::new(),
        }
    }

    /// What the commands since the last call had to say, one line each, without the
    /// `cubarium voxel: ` prefix.
    pub fn take_said(&mut self) -> Vec<String> {
        std::mem::take(&mut self.said)
    }

    /// Apply one console line. Anything unrecognised says the usage and changes nothing;
    /// a bad argument is reported and the world is left alone.
    ///
    /// The plant and animal layers come in as further borrows rather than living in
    /// `Console`, for the same reason the world does: the loop reads `paused` and `speed`
    /// while it still holds all three.
    pub fn handle(&mut self, world: &mut World, flora: &mut Flora, fauna: &mut Fauna, line: &str) {
        let line = line.trim();
        if line.is_empty() {
            return;
        }
        let mut parts = line.split_whitespace();
        let verb = parts.next().unwrap_or_default();
        let rest: Vec<&str> = parts.collect();
        let out = &mut self.said;

        match verb {
            "q" | "quit" => self.quit = true,
            "p" | "pause" => {
                self.paused = match rest.first().copied() {
                    None => !self.paused,
                    Some("on") => true,
                    Some("off") => false,
                    Some(other) => {
                        say!(out, "`p [on|off]` pauses or resumes, not {other:?}");
                        return;
                    }
                };
                say!(out, "{}", if self.paused { "paused" } else { "running" });
            }
            "s" | "step" => {
                let n = match rest.first().map(|t| t.parse::<u32>()) {
                    None => 1,
                    Some(Ok(n)) if n > 0 => n,
                    _ => {
                        say!(out, "`s [N]` steps N ticks (a positive whole number)");
                        return;
                    }
                };
                self.paused = true;
                self.pending_steps = self.pending_steps.saturating_add(n);
            }
            "+" => {
                self.speed = (self.speed * 2.0).min(MAX_SPEED);
                say!(out, "speed {}", self.speed);
            }
            "-" => {
                self.speed = (self.speed / 2.0).max(MIN_SPEED);
                say!(out, "speed {}", self.speed);
            }
            "speed" => match rest.first().map(|t| t.parse::<f64>()) {
                Some(Ok(v)) if v.is_finite() && v > 0.0 => {
                    self.speed = v.clamp(MIN_SPEED, MAX_SPEED);
                    say!(out, "speed {}", self.speed);
                }
                _ => say!(
                    out,
                    "`speed X` wants a positive multiplier ({MIN_SPEED}..{MAX_SPEED}; pause with `p`)"
                ),
            },
            "r" | "rain" => {
                let volume = match rest.first() {
                    None => DEFAULT_RAIN_M3,
                    Some(text) => match text.parse::<f64>() {
                        Ok(v) if v.is_finite() && v > 0.0 => v,
                        _ => {
                            say!(out, "`r [m3]` wants a positive volume");
                            return;
                        }
                    },
                };
                let took = world.apply(VoxelCommand::RainPulse { volume_m3: volume });
                say!(out, "rain {volume} m3, accepted {took} m3");
            }
            "a" | "aquifer" => {
                let volume = match rest.first().map(|t| t.parse::<f64>()) {
                    Some(Ok(v)) if v.is_finite() && v != 0.0 => v,
                    _ => {
                        say!(out, "`a M3` wants a nonzero volume (negative withdraws)");
                        return;
                    }
                };
                let took = world.apply(VoxelCommand::ChargeAquifer { volume_m3: volume });
                say!(
                    out,
                    "aquifer {volume:+} m3, accepted {took:+} m3, head now {:.3} m",
                    world.aquifer_head_m()
                );
            }
            // The closed budget's "make it rain": water goes **aloft**, and the shower
            // scheduler brings it down when the store passes its trigger. `r` still
            // drops a pulse on the world now; this is the lever that feeds the cycle.
            "h" | "atmosphere" => {
                let volume = match rest.first().map(|t| t.parse::<f64>()) {
                    Some(Ok(v)) if v.is_finite() && v > 0.0 => v,
                    _ => {
                        say!(out, "`h M3` wants a positive volume");
                        return;
                    }
                };
                let took = world.apply(VoxelCommand::AddAtmosphere { volume_m3: volume });
                if took > 0.0 {
                    say!(out, "atmosphere +{took} m3, store now {:.3} m3", world.atmosphere_m3());
                } else {
                    say!(
                        out,
                        "refused — this world runs the open water budget and has no atmosphere store"
                    );
                }
            }
            "o" | "outlet" => {
                self.outlet = !world.outlet_open();
                world.apply(VoxelCommand::SetOutlet { open: self.outlet });
                say!(out, "outlet {}", if self.outlet { "open" } else { "closed" });
            }
            "w" | "save" => {
                let dir = rest.first().map_or_else(
                    || (self.default_save_dir)(SystemTime::now(), world.tick()),
                    PathBuf::from,
                );
                self.save_to = Some(dir);
            }
            "save-terrain" => match rest.first() {
                // Terrain only, and the message says so: `World::save` holds the voxels,
                // the water and the world's tick, and neither layer of the ecology.
                Some(path) => match std::fs::write(path, world.save()) {
                    Ok(()) => say!(out, "saved {path} — terrain and water only, no plants and no animals"),
                    Err(e) => say!(out, "saving {path}: {e}"),
                },
                None => say!(out, "`save-terrain PATH` needs a path"),
            },
            "l" | "load" => match rest.first() {
                Some(path) => {
                    // A terrain load is not an ecosystem restore (Astra R9.6): refuse it
                    // in place while anything of the ecology is standing, and say what.
                    if let Some(why) = ecology_state(flora, fauna) {
                        say!(
                            out,
                            "`l` loads the **terrain only** and this run has {why}; an in-place \
                             load would leave them on another world's terrain with ledgers \
                             describing this one. Restart with `--load {path}`, which starts the \
                             ecology fresh on that terrain."
                        );
                        return;
                    }
                    let read = std::fs::read(path)
                        .map_err(anyhow::Error::from)
                        .and_then(|b| World::load(&b));
                    match read {
                        Ok(loaded) => {
                            // The picture's geometry is fixed for the life of the run, so a
                            // world of a different shape would draw into the wrong raster.
                            let c = loaded.config();
                            if (c.width, c.height, c.depth) != self.dims {
                                say!(
                                    out,
                                    "{path} is {}x{}x{}, this run draws {}x{}x{}; restart with a \
                                     matching config",
                                    c.width,
                                    c.height,
                                    c.depth,
                                    self.dims.0,
                                    self.dims.1,
                                    self.dims.2
                                );
                            } else {
                                *world = loaded;
                                self.outlet = world.outlet_open();
                                say!(
                                    out,
                                    "loaded the terrain of {path} at tick {} — the plants and the \
                                     animals are this run's own",
                                    world.tick()
                                );
                            }
                        }
                        Err(e) => say!(out, "loading {path}: {e:#}"),
                    }
                }
                None => say!(out, "`l PATH` needs a path (terrain only)"),
            },
            "i" | "inspect" => {
                let Some((x, y, z)) = coords(world, "i X Y Z", &rest, out) else {
                    return;
                };
                say!(out, "{}", cell_state(world, x, y, z));
                // The site is the cell itself, not the column's skyline: `i` on a support
                // face reports what grows there and what the ground holds.
                let site = site_of(world, x, y, z);
                let fv = flora.view();
                // A face holds a plant and a fungus (`FaceLayer`): each on its own line.
                let here = fv.stands_on(site);
                if here.is_empty() {
                    say!(out, "  no stand");
                }
                for s in here {
                    say!(
                        out,
                        "  {} {:?} W {:.4} P {:.4} Q {:.4} mineral {:.5} light {:.3} moisture \
                         {:.3} aeration stress {:.3}",
                        s.species.name(),
                        s.stage,
                        s.wood,
                        s.foliage,
                        s.reserve,
                        s.mineral,
                        s.light,
                        s.moisture,
                        s.aeration_stress,
                    );
                }
                // The animals standing on this face, in id order. More than one is
                // ordinary: a newborn appears on its parent's face.
                for a in fauna.view().animals_at(site) {
                    say!(
                        out,
                        "  {} #{} body {:.4} reserve {:.4} mineral {:.5} energy {:.4} age {} \
                         ticks {:?}",
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
                    say!(
                        out,
                        "  ground N {:.4} litter {:.4} ({:.4} energy, {:.5} mineral) dead wood \
                         {:.4} ({:.5} mineral)",
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
                        say!(
                            out,
                            "  seed {} organic {:.6} mineral {:.7} age {} ticks (bin from tick {})",
                            c.species.name(),
                            c.organic,
                            c.mineral,
                            c.age_ticks(fv.tick),
                            c.bin_start_tick,
                        );
                    }
                    if g.seeds.is_empty() {
                        say!(out, "  no seed cohort");
                    }
                }
            }
            "f" | "flora" => {
                let usage = "f X Z bloomcrown|umbrellafrond [wood]";
                let Some((x, z)) = column(world, usage, &rest, out) else {
                    return;
                };
                let Some(species) = rest.get(2).and_then(|n| Species::parse(n)) else {
                    say!(
                        out,
                        "`{usage}` — `{}` is not a species",
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
                            say!(out, "`{usage}` wants a positive wood");
                            return;
                        }
                    },
                };
                if flora.apply(world, FloraCommand::Seed { x, z, species, wood }) {
                    let site = highest_site(world, x, z);
                    say!(out, "seeded {} at {:?} with W {wood}", species.name(), site);
                } else {
                    say!(
                        out,
                        "no stand at ({x}, {z}): either the column has no support face, one \
                         already grows there, or W {wood} is below {}'s alive_min {}",
                        species.name(),
                        flora.config().species(species).alive_min,
                    );
                }
            }
            "c" | "clear" => {
                let Some((x, z)) = column(world, "c X Z", &rest, out) else {
                    return;
                };
                if flora.apply(world, FloraCommand::Clear { x, z }) {
                    say!(out, "cleared the stand at ({x}, {z})");
                } else {
                    say!(out, "no stand to clear at ({x}, {z})");
                }
            }
            // Development controls (predation §2.9): a clutch of lanternjaw eggs, or
            // adult lanternjaws, where the prey live — refused with the reason when none do.
            "e" | "eggs" | "j" | "lanternjaw" => {
                let n = match rest.first().map(|t| t.parse::<usize>()) {
                    None => 1,
                    Some(Ok(n)) if n > 0 => n,
                    _ => {
                        say!(out, "`{verb} [N]` wants a positive count");
                        return;
                    }
                };
                if matches!(verb, "e" | "eggs") {
                    match habitat::bury_founding_clutches(world, flora, fauna, Founder::Lanternjaw, n) {
                        Ok(eggs) => say!(out, "buried {eggs} lanternjaw eggs where the prey live"),
                        Err(why) => say!(out, "no clutch buried — {why}"),
                    }
                } else {
                    match habitat::introduce_lanternjaws(world, flora, fauna, n) {
                        Ok(ids) => say!(out, "introduced {} adult lanternjaws (ids {ids:?})", ids.len()),
                        Err(why) => say!(out, "no lanternjaw introduced — {why}"),
                    }
                }
            }
            // Development control (roster R3): N chorister packs on covered dens near the
            // prey, refused with the reason when none can be founded.
            "k" | "chorister" => {
                let n = match rest.first().map(|t| t.parse::<usize>()) {
                    None => 1,
                    Some(Ok(n)) if n > 0 => n,
                    _ => {
                        say!(out, "`{verb} [N]` wants a positive count");
                        return;
                    }
                };
                match habitat::introduce_packs(world, flora, fauna, Founder::Chorister, n) {
                    Ok(ids) => say!(out, "founded {} chorister packs (ids {ids:?})", ids.len()),
                    Err(why) => say!(out, "no chorister pack founded — {why}"),
                }
            }
            "g" | "grazer" => {
                let usage = "g X Z";
                let Some((x, z)) = column(world, usage, &rest, out) else {
                    return;
                };
                // A grown frondgrazer (the browser founder) with half a reserve, as the
                // seeder founds one, with its lineage's controller.
                if fauna.apply(
                    world,
                    FaunaCommand::IntroduceFounder {
                        x,
                        z,
                        founder: Founder::Browser,
                        stores: habitat::founder_arrival(&fauna.config().founding),
                        heading_rad: 0.0,
                    },
                ) {
                    let id = fauna.view().ledger.births - 1;
                    fauna.install_founder_controller(id, Founder::Browser);
                    say!(out, "introduced a frondgrazer at {:?}", highest_site(world, x, z));
                } else {
                    say!(out, "no frondgrazer at ({x}, {z}): the column has no support face");
                }
            }
            "m" | "material" => {
                let usage = "m X Y Z air|rock|soil|bedrock";
                let Some((x, y, z)) = coords(world, usage, &rest, out) else {
                    return;
                };
                let material = match rest.get(3).map(|w| w.to_ascii_lowercase()).as_deref() {
                    Some("air") => Material::Air,
                    Some("rock") => Material::Rock,
                    Some("soil") => Material::Soil,
                    Some("bedrock") => Material::Bedrock,
                    other => {
                        say!(out, "`{usage}` — `{}` is not a material", other.unwrap_or(""));
                        return;
                    }
                };
                // The core moves the water the old material held; the new state is the
                // receipt, so a displaced cell says so itself.
                world.apply(VoxelCommand::SetMaterial { x, y, z, material });
                say!(out, "set {material:?}, now {}", cell_state(world, x, y, z));
            }
            other => say!(out, "`{other}`? — {COMMANDS}"),
        }
    }
}

/// **Whole world ticks per clock tick** from the console's speed and pause: a fractional
/// speed carries its remainder over, a paused run steps only the steps asked for, one per
/// clock tick. Shared by `cubarium voxel`'s frame clock and a session's loop.
#[derive(Clone, Copy, Debug, Default)]
pub struct Pacer {
    debt: f64,
}

impl Pacer {
    /// The world ticks due on this clock tick, taking any pending single step.
    pub fn due(&mut self, console: &mut Console) -> u32 {
        if console.paused {
            // A single `s` still advances exactly one world tick per clock tick.
            if console.pending_steps > 0 {
                console.pending_steps -= 1;
                1
            } else {
                0
            }
        } else {
            self.debt += console.speed;
            let n = self.debt.floor();
            self.debt -= n;
            n as u32
        }
    }
}

/// What a terrain-only load would leave standing, in words, or `None` when this run holds
/// no ecology at all and `World::load` can safely replace the terrain under it.
///
/// Astra R9.6: `World::save`/`World::load` carry the voxels, the water and the world's
/// tick, and neither layer of the ecology — so an in-place load against living stands is
/// not a restore of anything. The checks are the review's list: any stand, any animal, any
/// seed bank, any provisioned ground, and a nonzero ledger on either layer (compared
/// against `Default`, so a term added later is covered without touching this).
pub fn ecology_state(flora: &Flora, fauna: &Fauna) -> Option<String> {
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

/// `X Y Z` from a command's arguments, or `None` with the reason said.
///
/// `x` wraps — the strip is a ring and has no end to fall off — so any integer is a
/// column. `y` and `z` have real ends, so one outside the world is refused and named
/// rather than folded into a cell the caller did not mean.
fn coords(world: &World, usage: &str, rest: &[&str], out: &mut Vec<String>) -> Option<(i64, u32, u32)> {
    let triple = (
        rest.first().and_then(|t| t.parse::<i64>().ok()),
        rest.get(1).and_then(|t| t.parse::<u32>().ok()),
        rest.get(2).and_then(|t| t.parse::<u32>().ok()),
    );
    let (Some(x), Some(y), Some(z)) = triple else {
        say!(out, "`{usage}` wants three integers for X Y Z");
        return None;
    };
    let c = world.config();
    if y >= c.height || z >= c.depth {
        say!(out, "({x}, {y}, {z}) is outside a {}x{}x{} world", c.width, c.height, c.depth);
        return None;
    }
    Some((x, y, z))
}

/// `X Z` from a command's arguments: a column, for the commands that act on whichever
/// support face is the highest one in it. `x` wraps; `z` is refused if it is outside.
fn column(world: &World, usage: &str, rest: &[&str], out: &mut Vec<String>) -> Option<(i64, u32)> {
    let pair = (
        rest.first().and_then(|t| t.parse::<i64>().ok()),
        rest.get(1).and_then(|t| t.parse::<u32>().ok()),
    );
    let (Some(x), Some(z)) = pair else {
        say!(out, "`{usage}` wants two integers for X Z");
        return None;
    };
    let c = world.config();
    if z >= c.depth {
        say!(out, "z = {z} is outside a {}-deep world", c.depth);
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
pub fn cell_state(world: &World, x: i64, y: u32, z: u32) -> String {
    let view = world.view();
    format!(
        "({}, {y}, {z}) {:?} free {:.4} pore {:.4}",
        x.rem_euclid(i64::from(view.config.width)),
        view.material_at(x, y, z),
        view.free_at(x, y, z),
        view.pore_at(x, y, z),
    )
}

/// A reader's [`cubarium_observatory::Command`] as the console line that does the same:
/// the dashboard and Godot change the run through the console's own path.
pub fn console_line(command: cubarium_observatory::Command) -> String {
    use cubarium_observatory::Command;
    match command {
        Command::SetSpeed(x) => format!("speed {x}"),
        Command::Pause => "p on".into(),
        Command::Resume => "p off".into(),
        Command::Faster => "+".into(),
        Command::Slower => "-".into(),
        Command::Step(n) => format!("s {n}"),
    }
}
