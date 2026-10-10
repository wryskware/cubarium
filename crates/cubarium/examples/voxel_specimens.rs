//! `voxel_specimens`: every organism's baked model on one flat strip, drawn by the CPU
//! voxel presenter, for judging the look without a seed's luck, wilt or depth haze.
//!
//! ```text
//! cargo run -p cubarium --release --example voxel_specimens -- OUT.png [--glyphs] [--wilt W]
//!     [--gpu [--px N] [--textures DIR | --textures-only DIR] [--lit]] [--rider]
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
//! left to right, in the front slab. Bloomcrown, lanternberry and glowcap get a fourth,
//! adult and ripe. The three founders stand at the end, two each, facing each way
//! (frondgrazer, littershredder, lanternjaw), and after them the
//! unsimulated animals the models carry as `specimens` (model pass MP-A: chorister,
//! ripple-snail, capgnawer, seedporter, bellwing; the loftstrider; the seedporter perched, on
//! all fours and gliding, its pick's pose being its climbing one; the lanternjaw larva), drawn from the library only, never
//! simulated. Every stand is drawn at full
//! moisture (or at `--wilt W`, a moisture of `1 − W`), with full foliage. The picture is
//! written at 2× nearest, the panel's own upscale.
//!
//! `--cured` draws every stand of a species that cures (springturf) cured: its foliage
//! dropped and its crown in the species' cured colour.
//!
//! `--shared` puts a full-grown glowcap on the face of every adult photosynthetic producer
//! (a face holds one plant and one fungus, `FaceLayer`): the presenter draws it one column
//! over, beside the stem.
//!
//! `--contest` puts a full-grown springturf on the face of every woody producer at every
//! size (a face holds two plants, `FaceLayer::Under`, package CONTEST): beneath a crown
//! that stands above it, over a seedling still under the sward. Placement only, with the
//! existing models: the look is Wrysk's. The stands stand two rows deeper (z 3, not 1), so
//! the turf drawn in front of a trunk (package RENDER-UNDER) is inside the world.
//!
//! `--rider` puts a parasite lanternjaw hatchling on the first grazer's back (its interim
//! plain-voxel look, `design/handoffs/voxel-predation-parasite-2026-10-01.md`).
//!
//! `--models DIR` reads the baked models from `DIR` instead of `assets/voxel-models`.
//! `--candidates SPECIES:I,J,…` draws only that species, full-grown, one stand per listed
//! index, the stand drawing variant `I` of the adult step (its id is chosen so the
//! library's own variant pick lands there), and no animals. With `--models` pointing at a
//! scratch library whose adult step holds candidate models as variants, this puts model
//! candidates (the 2026-10-07 browsed-foliage round) side by side in the presenter.
//!
//! `--states SPECIES` draws only that species, at wood 35 %, 65 % and 100 % of its
//! maximum (left to right, a gap between), each size once per foliage state of its model
//! (`foliage-states-implementation-2026-10-08.md`): the stand's total foliage set inside
//! that state's band of the config's `foliage_state_below`, so the presenter's own choice
//! picks it. No animals.
//!
//! `--loftstrider-bins` draws only the loftstrider's five founder bins, newborn to adult,
//! using live bodies at the bins' length keys. `--voxel-m M` chooses the baked grid size.
//!
//! VAULT-SHAPE (`design/handoffs/vaulttree-proportions-2026-10-10.md`): `--crown-radius
//! SPECIES:R` sets the species' full-grown crown radius (`crown_radius_m[1]`, metres) in the
//! strip's flora config, so a scratch library baked at another radius is drawn at its own
//! adult step and spaced for it. `--scale` adds the loftstrider and the seedporter (the
//! models' specimens) after a `--candidates` or `--stand` strip, for scale. `--stand
//! SPECIES:N:SPACING_M` draws N full-grown stands of the species and nothing else, on a
//! triangular lattice whose nearest neighbours are SPACING_M apart (rows of three, two,
//! three, … alternating half a step), so overlapping crowns show how a stand fills.
//!
//! VAULT-SHAPE round 2: `--turn Q,Q,…` turns each stand of a `--candidates`, `--stand` or
//! `--grove` strip (in its order) by Q quarter turns, exactly as `model::turn` turns an
//! animal (1 faces −z); a missing entry is 0. The sheet does it by giving each stand its
//! own turned copy of its variant in the species' full-grown step, so the presenter is
//! unchanged (its plant pass does not turn plants). `--grove SPECIES:SPACING_M` draws
//! seven full-grown stands in a loose cluster (a centre and six round it at 1.0–1.2
//! spacings), base variants 0 1 2 0 1 2 0 unless `--pick I,J,…` names them. `--view-turn
//! Q` turns the whole grove (its layout and every tree) by Q more quarters: the grove seen
//! from another side. `--lean-turns` says the library's full-grown step holds four lean
//! copies of each variant (`v × 4 + j`, the bake's scratch `lean.turns`, the shared lean
//! turned by j quarters), and picks copy `(4 − turn) mod 4` for each stand, so every tree
//! leans the same way in the world whatever its turn.

use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{Context, Result, bail};

use cubarium::sink::gpu::{VoxelGpuSink, VoxelGpuSinkOptions};
use cubarium::voxel::animal::Specimen;
use cubarium::voxel::model::{self, ModelLibrary};
use cubarium::voxel::stand::Cell;
use cubarium::voxel::present::VoxelPresenter;
use cubarium::voxel::project::Projection;
use cubarium::voxel::{Lighting, OrganismLook, VoxelConfig};
use cubarium_render::Canvas;
use cubarium_surface::{Scale, Topology};
use cubarium_voxel::{Command as VoxelCommand, Config, Material, World};
use cubarium_voxel_fauna::{
    Attachment, Command as FaunaCommand, Fauna, FaunaConfig, FaunaView, Founder, StartingStores,
};
use cubarium_voxel_flora::{Command as FloraCommand, Flora, FloraConfig, FloraView, Species};

/// Voxels between one specimen's crown edge and the next one's.
const GAP: i64 = 3;

/// The founders on the strip, two each, and the width of each one's slot in cells.
const FOUNDERS: [Founder; 6] = [
    Founder::Browser,
    Founder::Browser,
    Founder::Blind,
    Founder::Blind,
    Founder::Lanternjaw,
    Founder::Lanternjaw,
];
const SLOT: i64 = 10;

/// `--grove`: the seven trees as (bearing in degrees, distance in spacings) from the
/// centre: a loose cluster, the six round the centre a little irregular.
const GROVE: [(f64, f64); 7] = [
    (0.0, 0.0),
    (10.0, 1.0),
    (65.0, 1.15),
    (120.0, 1.0),
    (180.0, 1.2),
    (235.0, 1.05),
    (300.0, 1.1),
];

/// The unsimulated animals drawn after the founders: manifest name, heading (π/2 faces
/// +x, side-on; π faces the viewer), and how many voxels above the ground it hovers.
const SPECIMENS: [(&str, f64, u32); 10] = [
    ("chorister", std::f64::consts::FRAC_PI_2, 0),
    ("ripple-snail", std::f64::consts::FRAC_PI_2, 0),
    ("capgnawer", std::f64::consts::PI, 0),
    ("seedporter", std::f64::consts::PI, 0),
    ("bellwing", std::f64::consts::PI, 3),
    ("loftstrider", std::f64::consts::PI, 0),
    // the seedporter's other poses: perched (its tail hangs two cells below its feet, so
    // its perch is two cells up), on all fours side-on, gliding
    ("seedporter-stand", std::f64::consts::PI, 2),
    ("seedporter-ground", std::f64::consts::FRAC_PI_2, 0),
    ("seedporter-glide", std::f64::consts::PI, 3),
    // the lanternjaw's rider stage (D16, the leechfork), side-on, on the ground
    ("lanternjaw-larva", std::f64::consts::FRAC_PI_2, 0),
];

fn main() -> Result<()> {
    let mut out = None;
    let mut glyphs = false;
    let mut wilt = 0.0f64;
    let mut cured = false;
    let mut gpu = false;
    let mut px = 6u32;
    let mut textures: Option<PathBuf> = None;
    let mut layered = false;
    let mut lit = false;
    let mut rider = false;
    let mut shared = false;
    let mut contest = false;
    let mut models_dir_arg: Option<PathBuf> = None;
    let mut candidates: Option<(Species, Vec<usize>)> = None;
    let mut states_of: Option<Species> = None;
    let mut loftstrider_bins = false;
    let mut voxel_m = 0.125;
    let mut radius_override: Option<(Species, f64)> = None;
    let mut scale = false;
    let mut stand: Option<(Species, usize, f64)> = None;
    let mut grove: Option<(Species, f64)> = None;
    let mut turns: Vec<u8> = Vec::new();
    let mut picks: Option<Vec<usize>> = None;
    let mut view_turn = 0u8;
    let mut lean_turns = false;
    let species_named = |name: &str| {
        Species::ALL
            .into_iter()
            .find(|s| s.name() == name)
            .with_context(|| format!("no species {name}"))
    };
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        match a.as_str() {
            "--glyphs" => glyphs = true,
            "--lit" => lit = true,
            "--rider" => rider = true,
            "--shared" => shared = true,
            "--contest" => contest = true,
            "--gpu" => gpu = true,
            "--loftstrider-bins" => loftstrider_bins = true,
            "--scale" => scale = true,
            "--crown-radius" => {
                let v = args.next().context("--crown-radius SPECIES:R")?;
                let (name, r) = v.split_once(':').context("--crown-radius SPECIES:R")?;
                radius_override = Some((species_named(name)?, r.parse()?));
            }
            "--stand" => {
                let v = args.next().context("--stand SPECIES:N:SPACING_M")?;
                let mut it = v.split(':');
                let name = it.next().context("--stand SPECIES:N:SPACING_M")?;
                let n = it.next().context("--stand SPECIES:N:SPACING_M")?.parse()?;
                let d = it.next().context("--stand SPECIES:N:SPACING_M")?.parse()?;
                stand = Some((species_named(name)?, n, d));
            }
            "--grove" => {
                let v = args.next().context("--grove SPECIES:SPACING_M")?;
                let (name, d) = v.split_once(':').context("--grove SPECIES:SPACING_M")?;
                grove = Some((species_named(name)?, d.parse()?));
            }
            "--turn" => {
                let v = args.next().context("--turn Q,Q,...")?;
                turns = v.split(',').map(|q| q.parse::<u8>().map(|q| q % 4)).collect::<Result<_, _>>()?;
            }
            "--pick" => {
                let v = args.next().context("--pick I,J,...")?;
                picks = Some(v.split(',').map(str::parse).collect::<Result<_, _>>()?);
            }
            "--view-turn" => view_turn = args.next().context("--view-turn Q")?.parse::<u8>()? % 4,
            "--lean-turns" => lean_turns = true,
            "--voxel-m" => voxel_m = args.next().context("--voxel-m M")?.parse()?,
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
            "--cured" => cured = true,
            "--models" => models_dir_arg = Some(PathBuf::from(args.next().context("--models DIR")?)),
            "--candidates" => {
                let v = args.next().context("--candidates SPECIES:I,J,...")?;
                let (name, list) = v.split_once(':').context("--candidates SPECIES:I,J,...")?;
                let species = Species::ALL
                    .into_iter()
                    .find(|s| s.name() == name)
                    .with_context(|| format!("no species {name}"))?;
                let idx = list.split(',').map(|i| i.parse()).collect::<Result<Vec<usize>, _>>()?;
                candidates = Some((species, idx));
            }
            "--states" => {
                let name = args.next().context("--states SPECIES")?;
                states_of = Some(
                    Species::ALL
                        .into_iter()
                        .find(|s| s.name() == name)
                        .with_context(|| format!("no species {name}"))?,
                );
            }
            _ if out.is_none() => out = Some(PathBuf::from(a)),
            _ => bail!("unexpected argument {a}"),
        }
    }
    let out = out.context("usage: voxel_specimens OUT.png [--glyphs] [--wilt W]")?;

    let mut flora_cfg = FloraConfig::default();
    if let Some((species, r)) = radius_override {
        flora_cfg.species_mut(species).crown_radius_m[1] = r;
    }
    // Lay the specimens out first, so the world is exactly as wide as they need.
    let mut plan: Vec<(Species, f64, bool)> = Vec::new();
    if let Some((species, idx)) = &candidates {
        plan.extend(idx.iter().map(|_| (*species, 1.0, false)));
    }
    // `--states`: per size, one stand per state, each with its total foliage fraction.
    let mut state_fraction: Vec<Option<f64>> = Vec::new();
    let mut extra_gap: Vec<i64> = Vec::new();
    if let Some(species) = states_of {
        let below = &flora_cfg.species(species).foliage_state_below;
        let n = below.len() + 1;
        for f in [0.35, 0.65, 1.0] {
            for k in 0..n {
                let hi = if k == 0 { 1.0 } else { below[k - 1] };
                let lo = if k + 1 == n { 0.0 } else { below[k] };
                plan.push((species, f, false));
                state_fraction.push(Some(0.5 * (lo + hi)));
                extra_gap.push(if k + 1 == n { 3 } else { 0 });
            }
        }
    }
    // `--stand`: the stands' (x, z) cells on the lattice, after the plan is laid out.
    let mut stand_cells: Vec<(i64, u32)> = Vec::new();
    if let Some((species, n, _)) = stand {
        plan.extend((0..n).map(|_| (species, 1.0, false)));
    }
    if let Some((species, _)) = grove {
        plan.extend((0..GROVE.len()).map(|_| (species, 1.0, false)));
    }
    let solo = candidates.is_some()
        || states_of.is_some()
        || loftstrider_bins
        || stand.is_some()
        || grove.is_some();
    for species in Species::ALL.into_iter().filter(|_| !solo) {
        for f in [0.1, 0.4, 1.0] {
            plan.push((species, f, false));
        }
        if matches!(species, Species::Bloomcrown | Species::Lanternberry | Species::Glowcap) {
            plan.push((species, 1.0, true));
        }
    }
    let mut x = GAP;
    let mut columns = Vec::new();
    let mut stand_depth = 0u32;
    if let Some((species, n, d)) = stand {
        let r = (flora_cfg.species(species).crown_radius_m_at(f64::INFINITY) / voxel_m).ceil() as i64;
        let step = d / voxel_m;
        let x0 = GAP + r;
        let z0 = r + 2;
        for k in 0..n {
            // rows of three then two, alternating half a step: 0 1 2 / 3 4 / 5 6 7 / …
            let (row, col) = (k / 5 * 2 + usize::from(k % 5 >= 3), if k % 5 >= 3 { k % 5 - 3 } else { k % 5 });
            let off = if row % 2 == 1 { 0.5 } else { 0.0 };
            let cx = x0 + ((col as f64 + off) * step).round() as i64;
            let cz = z0 + (row as f64 * step * 0.75f64.sqrt()).round() as i64;
            stand_cells.push((cx, cz as u32));
            columns.push(cx);
            stand_depth = stand_depth.max((cz + r + 3) as u32);
            x = x.max(cx + r + 1 + GAP);
        }
    }
    if let Some((species, d)) = grove {
        // The cluster's cells about its centre, turned with the view.
        let r = (flora_cfg.species(species).crown_radius_m_at(f64::INFINITY) / voxel_m).ceil() as i64;
        let step = d / voxel_m;
        let at: Vec<(i64, i64)> = GROVE
            .iter()
            .map(|&(a, k)| {
                let a = a.to_radians();
                let (dx, dz) = ((k * a.cos() * step).round() as i32, (k * a.sin() * step).round() as i32);
                let (dx, dz) = model::turn(view_turn, dx, dz);
                (i64::from(dx), i64::from(dz))
            })
            .collect();
        let (lx, lz) = (at.iter().map(|p| p.0).min().unwrap_or(0), at.iter().map(|p| p.1).min().unwrap_or(0));
        for &(dx, dz) in &at {
            let cx = GAP + r + dx - lx;
            let cz = r + 2 + dz - lz;
            stand_cells.push((cx, cz as u32));
            columns.push(cx);
            stand_depth = stand_depth.max((cz + r + 3) as u32);
            x = x.max(cx + r + 1 + GAP);
        }
    }
    for &(species, _, _) in plan.iter().skip(stand_cells.len()) {
        let r =
            (flora_cfg.species(species).crown_radius_m_at(f64::INFINITY) / voxel_m).ceil() as i64;
        let r = r.max(1);
        columns.push(x + r);
        x += 2 * r + 1 + GAP + extra_gap.get(columns.len() - 1).copied().unwrap_or(0);
    }
    let founders = if solo { 0 } else { FOUNDERS.len() };
    let tallest = plan
        .iter()
        .map(|&(sp, _, _)| (flora_cfg.species(sp).crown_height_m_at(f64::INFINITY) / voxel_m).ceil() as u32)
        .max()
        .unwrap_or(0);
    let animal_x: Vec<i64> = (0..founders as i64).map(|k| x + 4 + k * SLOT).collect();
    let mut width = (x + founders as i64 * SLOT + 6) as u32;

    // The unsimulated animals (the models' `specimens`) after the founders, each in its
    // own footprint: side-on as the chorister and ripple-snail picks are, the others facing
    // the viewer as theirs do, the bellwing hovering.
    let models_dir = models_dir_arg.unwrap_or(VoxelConfig::default().models_dir);
    let mut models: Option<Arc<ModelLibrary>> = if glyphs {
        None
    } else {
        Some(Arc::new(
            ModelLibrary::load(&models_dir, voxel_m).context("the baked models")?,
        ))
    };
    // Deep enough for the widest specimen (the loftstrider's legs and fronds span 21 cells).
    // `--candidates`: the crown's full depth in front of and behind the stem (round 1's lone
    // trees stood at z 1 in an 8-deep world, so most of a wide crown was clipped).
    let candidate_z = candidates.as_ref().map(|(species, _)| {
        (flora_cfg.species(*species).crown_radius_m_at(f64::INFINITY) / voxel_m).ceil() as u32 + 2
    });
    let mut depth = 8u32.max(stand_depth).max(candidate_z.map_or(0, |z| 2 * z + 1));
    let mut specimens = Vec::new();
    let mut loftstriders = Vec::new();
    if loftstrider_bins {
        anyhow::ensure!(
            !glyphs && !rider && candidates.is_none() && states_of.is_none(),
            "--loftstrider-bins needs models and its own strip"
        );
        let lib = models.as_ref().context("the baked models")?;
        let model = lib
            .animal(Founder::Loftstrider)
            .context("the loftstrider founder bake")?;
        let fauna_cfg = FaunaConfig::default();
        let phys = fauna_cfg.founder(Founder::Loftstrider);
        let mut sx = GAP;
        for bin in &model.bins {
            let turned: Vec<_> = bin
                .cells
                .iter()
                .map(|m| model::turn(2, m.offset[0], m.offset[2]))
                .collect();
            let x0 = turned.iter().map(|t| t.0).min().context("an empty bin")?;
            let x1 = turned.iter().map(|t| t.0).max().unwrap();
            let z0 = turned.iter().map(|t| t.1).min().unwrap();
            let z1 = turned.iter().map(|t| t.1).max().unwrap();
            depth = depth.max((z1 - z0 + 3) as u32);
            loftstriders.push((
                sx - i64::from(x0),
                u32::try_from(1 - z0).context("a bin's depth anchor")?,
                (bin.length_m / phys.adult_length_m).powi(3),
            ));
            sx += i64::from(x1 - x0) + 1 + GAP;
        }
        width = (sx + GAP) as u32;
    }
    if let Some(lib) = models.as_ref().filter(|_| !solo || scale) {
        let mut sx = x + founders as i64 * SLOT + 3;
        for (name, heading_rad, lift) in SPECIMENS {
            if solo && !matches!(name, "loftstrider" | "seedporter") {
                continue;
            }
            let Some(cells) = lib.specimen(name).and_then(|m| m.select(f64::INFINITY)) else {
                eprintln!("voxel_specimens: no {name} model; skipped");
                continue;
            };
            let q = model::quarter_turns(heading_rad);
            let turned: Vec<(i32, i32)> =
                cells.iter().map(|m| model::turn(q, m.offset[0], m.offset[2])).collect();
            let (x0, x1) = (turned.iter().map(|t| t.0).min().unwrap_or(0), turned.iter().map(|t| t.0).max().unwrap_or(0));
            let z0 = turned.iter().map(|t| t.1).min().unwrap_or(0);
            let z1 = turned.iter().map(|t| t.1).max().unwrap_or(0);
            depth = depth.max((z1 - z0 + 3) as u32);
            specimens.push(Specimen {
                name: name.to_string(),
                anchor: Cell {
                    x: sx - i64::from(x0),
                    y: 3 + lift,
                    z: (1 - z0).max(1) as u32,
                },
                heading_rad,
            });
            sx += i64::from(x1 - x0) + 1 + GAP;
        }
        width = (sx + 3) as u32;
    }

    let world_cfg = Config {
        width,
        height: 40u32.max(tallest + 8),
        depth,
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
    // The stands' row. `--contest` stands them two rows deeper, so the turf drawn in front
    // of a full-grown vaulttree's trunk (two cells in front of its stem) is in the world.
    // `--candidates` stands deep enough that their whole crown is in the world.
    let stand_z = if contest { 3 } else { candidate_z.unwrap_or(1) };
    for (k, (&(species, f, _), &x)) in plan.iter().zip(&columns).enumerate() {
        let wood = flora_cfg.species(species).wood_max * f;
        let z = stand_cells.get(k).map_or(stand_z, |c| c.1);
        if !flora.apply(
            &world,
            FloraCommand::Seed {
                x,
                z,
                species,
                wood,
            },
        ) {
            bail!("could not seed {species:?} at x={x}");
        }
        // `--shared`: a glowcap on the adult producer's own face.
        if shared && f >= 1.0 && flora_cfg.species(species).trophic == cubarium_voxel_flora::Trophic::Photo {
            let glow = flora_cfg.species(Species::Glowcap).wood_max;
            if !flora.apply(&world, FloraCommand::Seed { x, z: stand_z, species: Species::Glowcap, wood: glow }) {
                bail!("could not seed a glowcap under {species:?} at x={x}");
            }
        }
        // `--contest`: a sward sharing the woody producer's face.
        let sc = flora_cfg.species(species);
        if contest && sc.trophic == cubarium_voxel_flora::Trophic::Photo && !sc.ground_cover && sc.overtop_per_check > 0.0 {
            let turf = flora_cfg.species(Species::Springturf).wood_max;
            let site = cubarium_voxel_flora::Site { x: x as u32, y: 2, z: stand_z };
            if !flora.apply(&world, FloraCommand::SeedBeneath { site, species: Species::Springturf, wood: turf }) {
                bail!("could not seed a springturf beside {species:?} at x={x}");
            }
        }
    }
    let mut fauna = Fauna::new(FaunaConfig::default());
    for &(x, z, body) in &loftstriders {
        anyhow::ensure!(
            fauna.apply(
                &world,
                FaunaCommand::IntroduceFounder {
                    x,
                    z,
                    founder: Founder::Loftstrider,
                    stores: StartingStores { body, reserve: 1.0 },
                    heading_rad: std::f64::consts::PI,
                }
            ),
            "could not place a loftstrider bin at x={x}"
        );
    }
    for (k, &x) in animal_x.iter().enumerate() {
        let founder = FOUNDERS[k];
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

    // A hatchling riding the first grazer: placed beside it, then attached in the view
    // drawn (the view is the presenter's whole input).
    let mut animals = fauna.view().animals.to_vec();
    if rider {
        let x = animal_x[0];
        let young = StartingStores {
            body: 0.1,
            reserve: 0.5,
        };
        if !fauna.apply(
            &world,
            FaunaCommand::IntroduceFounder {
                x,
                z: 3,
                founder: Founder::Lanternjaw,
                stores: young,
                heading_rad: 1.5708,
            },
        ) {
            bail!("could not place a hatchling at x={x}");
        }
        animals = fauna.view().animals.to_vec();
        let host = *animals
            .iter()
            .find(|a| a.founder == Some(Founder::Browser))
            .context("a grazer")?;
        let r = animals
            .iter_mut()
            .find(|a| a.founder == Some(Founder::Lanternjaw))
            .context("the hatchling")?;
        r.attached = Some(Attachment { host: host.id });
        r.site = host.site;
        r.pose = host.pose;
    }
    let fauna_view = FaunaView {
        animals: &animals,
        ..fauna.view()
    };

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
        // `--cured`: every stand of a species that cures, cured (its foliage dropped).
        if cured && sc.cure_after_s > 0.0 {
            s.cured = true;
            s.foliage *= 1.0 - sc.cure_shed;
        }
        // `--states`: the stand's total foliage at its state's fraction, every layer alike.
        let k = columns.iter().position(|&x| x == i64::from(s.site.x));
        if let Some(f) = k.and_then(|k| state_fraction.get(k).copied().flatten()) {
            let want = f * sc.alpha * s.wood;
            let scale = if s.foliage > 0.0 { want / s.foliage } else { 0.0 };
            for v in &mut s.layer_stock {
                *v *= scale;
            }
            s.foliage = want;
        }
    }
    // `--candidates`, `--grove`, or a `--stand` with `--turn`/`--pick`/`--lean-turns`: the
    // species' full-grown step is rewritten to one variant per listed stand (its picked
    // variant turned by its quarter turns), and each stand gets an id that picks its own.
    let listed_species = candidates.as_ref().map(|c| c.0).or(stand.map(|s| s.0)).or(grove.map(|g| g.0));
    let rewrite = candidates.is_some() || grove.is_some() || !turns.is_empty() || picks.is_some() || lean_turns;
    if let (Some(species), Some(lib), true) = (listed_species, models.as_ref(), rewrite) {
        let mut model = lib.plant(species).context("no model for the listed species")?.clone();
        let last = model.steps.len().checked_sub(1).context("an empty model")?;
        let adult = &model.steps[last];
        let copies = if lean_turns { 4 } else { 1 };
        let base = adult.variants.len() / copies;
        anyhow::ensure!(base > 0, "the full-grown step has fewer than {copies} variants");
        let n = plan.len();
        let mut variants = Vec::with_capacity(n);
        let mut states = vec![Vec::with_capacity(n); adult.states.len()];
        for k in 0..n {
            let v = match (&candidates, &picks) {
                (Some((_, idx)), _) => idx[k],
                (None, Some(p)) => p.get(k).copied().unwrap_or(k % base),
                (None, None) => k % base,
            };
            anyhow::ensure!(v < base, "no variant {v}");
            let q = (turns.get(k).copied().unwrap_or(0) + view_turn) % 4;
            let src = v * copies + if lean_turns { usize::from((4 - q) % 4) } else { 0 };
            let turned = |cells: &[model::ModelCell]| -> Vec<model::ModelCell> {
                cells
                    .iter()
                    .map(|c| {
                        let (dx, dz) = model::turn(q, c.offset[0], c.offset[2]);
                        model::ModelCell { offset: [dx, c.offset[1], dz], ..*c }
                    })
                    .collect()
            };
            variants.push(turned(&adult.variants[src]));
            for (st, per) in states.iter_mut().zip(&adult.states) {
                st.push(turned(&per[src]));
            }
        }
        model.steps[last].variants = variants;
        model.steps[last].states = states;
        let mut lib = (**lib).clone();
        lib.insert_plant(species, model);
        let lib = Arc::new(lib);
        let model = lib.plant(species).context("the rewritten model")?;
        let sc = flora_cfg.species(species);
        let mut next = 1_000_000u64;
        for s in stands.iter_mut().filter(|s| s.species == species) {
            let k = (0..n)
                .find(|&k| {
                    columns[k] == i64::from(s.site.x)
                        && stand_cells.get(k).map_or(true, |c| c.1 == s.site.z)
                })
                .context("a stand off the plan")?;
            loop {
                next += 1;
                let got = model
                    .pick_sized(sc.crown_height_m_at(s.wood), sc.crown_radius_m_at(s.wood), next)
                    .context("no model step")?;
                if got == (last, k) {
                    break;
                }
            }
            s.id = next;
        }
        models = Some(lib);
    }
    let view = FloraView {
        stands: &stands,
        crowns: cubarium_voxel_flora::CrownCache::none(),
        ..view
    };

    let cfg = VoxelConfig {
        sim: cubarium_session::SimParams { world: world_cfg.clone(), ..Default::default() },
        raster_height: 0,
        organisms: if glyphs {
            OrganismLook::Glyphs
        } else {
            OrganismLook::Models
        },
        ..VoxelConfig::default()
    };
    if gpu {
        let under = layered.then(|| cfg.textures_dir.clone());
        let cfg = VoxelConfig {
            px_per_voxel: px,
            textures: true,
            textures_dir: textures.unwrap_or_else(|| cfg.textures_dir.clone()),
            lighting: if lit { Lighting::Lit } else { Lighting::Flat },
            ..cfg
        };
        let proj = Projection::new(cfg.tilt_degrees, px, 0, &world_cfg)?;
        let mut sink = VoxelGpuSink::new(
            &cfg,
            proj,
            VoxelGpuSinkOptions {
                models,
                textures_under: under,
                specimens: specimens.clone(),
                ..Default::default()
            },
        )?;
        anyhow::ensure!(
            sink.stage_view(&world.view(), view, fauna_view),
            "no staging buffer"
        );
        // The lit tier's sky plane is computed off this thread.
        while sink.light_pending() {
            std::thread::sleep(std::time::Duration::from_millis(5));
            anyhow::ensure!(
                sink.stage_view(&world.view(), view, fauna_view),
                "no staging buffer"
            );
        }
        if lit {
            let e = sink.emitters();
            let mut kinds: Vec<([u32; 3], usize)> = Vec::new();
            for (_, rgb) in e {
                let k = rgb.map(|c| (c * 1000.0) as u32);
                match kinds.iter_mut().find(|(c, _)| *c == k) {
                    Some((_, n)) => *n += 1,
                    None => kinds.push((k, 1)),
                }
            }
            println!("voxel_specimens: {} emitting voxels by colour (linear x1000): {kinds:?}", e.len());
        }
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
    .with_specimens(specimens.clone())
    .draw_with_fauna(&world.view(), view, Some(fauna_view), &mut canvas);
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
