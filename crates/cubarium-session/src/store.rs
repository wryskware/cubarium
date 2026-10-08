//! **Saves**: the layered snapshot (`world-N.voxel`, `flora-N.bin`, `fauna-N.bin`) that
//! `cubarium voxel --load DIR` resumes and a session loads, a small `meta.json` beside it
//! that a save-slot list reads without loading the world, and the two ways of reading a
//! directory back:
//! - [`resume`]: the ambient display's `--load`, which discards a directory of stale
//!   snapshots and founds afresh (the panel must never crash-loop on an old format);
//! - [`load_save`]: a session's load, which refuses a save from another version with a
//!   plain error and never discards anything.
//!
//! Always fresh, never migrate: a save of another schema is never read into this build.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use cubarium_observatory::query::{self, Schemas};
use cubarium_voxel::World;
use cubarium_voxel_fauna::Fauna;
use cubarium_voxel_flora::Flora;
use serde::{Deserialize, Serialize};

/// The metadata file beside a save.
pub const META_FILE: &str = "meta.json";
/// [`SaveMeta::format`]: the metadata file's own layout.
pub const META_FORMAT: u32 = 1;

/// Parse the tick from a snapshot filename like `world-1730.voxel`.
pub fn parse_voxel_tick(name: &str) -> Option<u64> {
    let name = name.strip_prefix("world-")?;
    let name = name.strip_suffix(".voxel")?;
    name.parse().ok()
}

/// Prune older voxel snapshots in `dir`, keeping the newest `keep` snapshots.
pub fn prune_voxel_snapshots(dir: &Path, keep: usize) {
    let keep = keep.max(1);
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    let mut candidates: Vec<(u64, PathBuf)> = Vec::new();
    for entry in entries.flatten() {
        let p = entry.path();
        if p.is_file() {
            let name = p.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if let Some(tick) = parse_voxel_tick(name) {
                candidates.push((tick, p));
            }
        }
    }
    candidates.sort_by(|a, b| b.0.cmp(&a.0));
    let kept: Vec<u64> = candidates.iter().take(keep).map(|c| c.0).collect();
    for (_, old_path) in candidates.into_iter().skip(keep) {
        let _ = std::fs::remove_file(old_path);
    }
    // The layers go with their world: any whose world was pruned (or never landed) goes.
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for p in entries.flatten().map(|e| e.path()) {
        let name = p.file_name().and_then(|n| n.to_str()).unwrap_or("");
        let tick = ["flora-", "fauna-"]
            .iter()
            .find_map(|k| name.strip_prefix(k))
            .and_then(|n| n.strip_suffix(".bin"))
            .and_then(|n| n.parse::<u64>().ok());
        if tick.is_some_and(|t| !kept.contains(&t)) {
            let _ = std::fs::remove_file(p);
        }
    }
}

/// The plant and animal layers saved beside `world-N.voxel`: each layer's own
/// always-fresh envelope (the same bytes `voxel_census save=` writes as `flora.bin` and
/// `fauna.bin`).
pub fn layer_files(dir: &Path, tick: u64) -> (PathBuf, PathBuf) {
    (
        dir.join(format!("flora-{tick}.bin")),
        dir.join(format!("fauna-{tick}.bin")),
    )
}

/// Write `bytes` to `path` through a synced `tmp-` file and a rename: whole or absent.
pub fn write_whole(dir: &Path, path: &Path, bytes: &[u8]) -> bool {
    use std::io::Write;
    let name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("snapshot");
    let tmp_file = dir.join(format!("tmp-{name}"));
    let written = std::fs::File::create(&tmp_file).and_then(|mut f| {
        f.write_all(bytes)?;
        f.sync_all()
    });
    if written.is_ok() && std::fs::rename(&tmp_file, path).is_ok() {
        true
    } else {
        let _ = std::fs::remove_file(&tmp_file);
        false
    }
}

/// Atomically write a snapshot to `dir` and prune older snapshots.
///
/// The bytes reach the disk before the rename makes them the snapshot: a
/// `world-N.voxel` is either whole or absent, even across a power cut, and a crash
/// mid-write leaves only a `tmp-` file nothing reads. The plant and animal layers
/// (`flora-N.bin`, `fauna-N.bin`) are written first, so a world file never appears
/// before the layers that go with it; `meta.json` last, describing the newest.
///
/// Returns whether the world file landed.
pub fn save_voxel_snapshot(dir: &Path, world: &World, layers: Option<(&Flora, &Fauna)>) -> bool {
    let tick = world.tick();
    if let Some((flora, fauna)) = layers {
        let (flora_file, fauna_file) = layer_files(dir, tick);
        let _ = write_whole(
            dir,
            &flora_file,
            &cubarium_voxel_flora::snapshot::encode(flora),
        ) && write_whole(dir, &fauna_file, &fauna.save());
    }
    let snap_file = dir.join(format!("world-{tick}.voxel"));
    let landed = write_whole(dir, &snap_file, &world.save());
    if landed {
        eprintln!("cubarium voxel: saved snapshot {}", snap_file.display());
        prune_voxel_snapshots(dir, 5);
        if let Some((flora, fauna)) = layers {
            let meta = SaveMeta::of(world, flora, fauna);
            if let Ok(json) = serde_json::to_vec_pretty(&meta) {
                let _ = write_whole(dir, &dir.join(META_FILE), &json);
            }
        }
    }
    landed
}

/// The whole world, plants and animals into `dir`, made if missing, in the layout
/// `--load DIR` resumes, with its `meta.json`. The metadata written, or why not.
pub fn save_whole(dir: &Path, world: &World, flora: &Flora, fauna: &Fauna) -> Result<SaveMeta> {
    std::fs::create_dir_all(dir).with_context(|| format!("save to {}", dir.display()))?;
    save_voxel_snapshot(dir, world, Some((flora, fauna)));
    let tick = world.tick();
    let (flora_file, fauna_file) = layer_files(dir, tick);
    let world_file = dir.join(format!("world-{tick}.voxel"));
    anyhow::ensure!(
        world_file.is_file() && flora_file.is_file() && fauna_file.is_file(),
        "save to {} failed",
        dir.display()
    );
    Ok(read_meta(dir).unwrap_or_else(|_| SaveMeta::of(world, flora, fauna)))
}

/// [`save_whole`], said on stderr: where it went, or why it did not.
pub fn save_whole_world(dir: &Path, world: &World, flora: &Flora, fauna: &Fauna) {
    match save_whole(dir, world, flora, fauna) {
        Ok(_) => eprintln!(
            "cubarium voxel: saved the whole world to {}/ — resume with `--load {}`",
            dir.display(),
            dir.display()
        ),
        Err(e) => eprintln!("cubarium voxel: {e:#}"),
    }
}

/// **What a save-slot list shows** without loading the world: written beside every whole
/// save as `meta.json`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SaveMeta {
    /// [`META_FORMAT`].
    pub format: u32,
    /// The schemas of the three saved layers: a save whose schemas are not this build's
    /// is listed and refused, never loaded.
    pub schema: Schemas,
    pub tick: u64,
    /// The world's day (from 1) and time on its 24-hour dial.
    pub day: i64,
    pub time_of_day: String,
    /// `[width, height, depth]` in voxels, and metres per voxel.
    pub size: [u32; 3],
    pub voxel_m: f64,
    /// The landform kind and the climate preset.
    pub landform: String,
    pub climate: String,
    pub seed: u64,
    /// Living bodies per lineage, and stands per plant species, by code name, in their
    /// fixed orders (zeros included).
    pub animals: Vec<(String, u32)>,
    pub plants: Vec<(String, u32)>,
    /// When it was written, seconds since the Unix epoch.
    pub saved_unix_s: u64,
}

impl SaveMeta {
    /// The metadata of the world in hand.
    pub fn of(world: &World, flora: &Flora, fauna: &Fauna) -> SaveMeta {
        let summary = query::summary(world, "");
        let clock = query::clock(world);
        let pops = query::populations(flora, fauna);
        SaveMeta {
            format: META_FORMAT,
            schema: Schemas::current(),
            tick: world.tick(),
            day: clock.day,
            time_of_day: clock.time_of_day,
            size: [summary.width, summary.height, summary.depth],
            voxel_m: summary.voxel_m,
            landform: summary.landform,
            climate: summary.climate,
            seed: summary.seed,
            animals: pops.animals.iter().map(|a| (a.species.clone(), a.alive)).collect(),
            plants: pops.plants.iter().map(|p| (p.species.clone(), p.stands)).collect(),
            saved_unix_s: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |d| d.as_secs()),
        }
    }

    /// Whether this build can load the save it describes: this build's schemas, or the
    /// one-version legacy reader's ([`readable`]).
    pub fn loadable(&self) -> bool {
        let now = Schemas::current();
        readable(self.schema.world, now.world, LEGACY.world)
            && readable(self.schema.flora, now.flora, LEGACY.flora)
            && readable(self.schema.fauna, now.fauna, LEGACY.fauna)
    }
}

/// **The one-version legacy reader's schemas** (SAVE-EXTENSIBLE, 2026-10-08): the last
/// layers whose configs sat positionally inside the state. Each layer's snapshot module
/// reads them; retire them there and this goes with them.
pub const LEGACY: Schemas = Schemas {
    world: cubarium_voxel::snapshot::LEGACY_SCHEMA,
    flora: cubarium_voxel_flora::snapshot::LEGACY_SCHEMA,
    fauna: cubarium_voxel_fauna::LEGACY_SCHEMA,
};

/// Whether a layer saved under schema `found` loads in a build of `now`.
fn readable(found: u32, now: u32, legacy: u32) -> bool {
    found == now || found == legacy
}

/// The `meta.json` of the save in `dir`, read alone.
pub fn read_meta(dir: &Path) -> Result<SaveMeta> {
    let path = dir.join(META_FILE);
    let bytes = std::fs::read(&path).with_context(|| format!("reading {}", path.display()))?;
    serde_json::from_slice(&bytes).with_context(|| format!("parsing {}", path.display()))
}

/// One entry of a save-slot list.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SlotInfo {
    pub path: PathBuf,
    /// The directory's name.
    pub name: String,
    pub meta: Option<SaveMeta>,
    /// Whether [`load_save`] would take it, as far as the metadata says.
    pub loadable: bool,
    /// Why not, in words, when it is not.
    pub problem: Option<String>,
}

/// Every save directory directly under `root`, newest metadata first, read without
/// loading any world. A save of another version is listed, marked not loadable.
pub fn list_saves(root: &Path) -> Vec<SlotInfo> {
    let Ok(entries) = std::fs::read_dir(root) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for dir in entries.flatten().map(|e| e.path()).filter(|p| p.is_dir()) {
        let name = dir.file_name().and_then(|n| n.to_str()).unwrap_or("").to_string();
        let has_world = std::fs::read_dir(&dir).is_ok_and(|mut it| {
            it.any(|e| e.is_ok_and(|e| parse_voxel_tick(&e.file_name().to_string_lossy()).is_some()))
        });
        let (meta, loadable, problem) = match read_meta(&dir) {
            Ok(meta) if meta.loadable() => (Some(meta), has_world, (!has_world).then(|| "no world file".to_string())),
            Ok(meta) => {
                let p = other_version(&meta.schema);
                (Some(meta), false, Some(p))
            }
            Err(_) if has_world => (None, false, Some("no metadata (saved by an older build)".to_string())),
            Err(_) => continue,
        };
        out.push(SlotInfo { path: dir, name, meta, loadable, problem });
    }
    out.sort_by(|a, b| {
        let t = |s: &SlotInfo| s.meta.as_ref().map_or(0, |m| m.saved_unix_s);
        t(b).cmp(&t(a)).then_with(|| a.name.cmp(&b.name))
    });
    out
}

fn other_version(found: &Schemas) -> String {
    let now = Schemas::current();
    format!(
        "made by another version of Cubarium (schemas world {} flora {} fauna {}; this build \
         reads {} {} {}); it can't be loaded",
        found.world, found.flora, found.fauna, now.world, now.flora, now.fauna
    )
}

/// Why a save did not load.
#[derive(Debug)]
pub enum LoadError {
    /// The save is a format this build no longer reads (always fresh, never migrate).
    OtherVersion { layer: &'static str, found: u32, expected: u32 },
    /// No save there.
    Missing(String),
    /// Something there, but not a whole save.
    Damaged(String),
}

impl std::fmt::Display for LoadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LoadError::OtherVersion { layer, found, expected } => write!(
                f,
                "this save is from another version of Cubarium ({layer} schema {found}, this \
                 build reads {expected}) and can't be loaded; start a fresh world"
            ),
            LoadError::Missing(why) => write!(f, "no save to load: {why}"),
            LoadError::Damaged(why) => write!(f, "the save is damaged: {why}"),
        }
    }
}

impl std::error::Error for LoadError {}

/// A save read back whole.
pub struct LoadedSave {
    pub world: World,
    pub flora: Flora,
    pub fauna: Fauna,
    pub dir: PathBuf,
    pub world_file: PathBuf,
}

/// The schema tag at the front of a saved layer.
fn schema_tag(bytes: &[u8]) -> Option<u32> {
    postcard::take_from_bytes::<u32>(bytes).ok().map(|(t, _)| t)
}

/// The newest `world-N.voxel` in `dir`, by tick.
fn newest_world(dir: &Path) -> Option<(u64, PathBuf)> {
    std::fs::read_dir(dir)
        .ok()?
        .flatten()
        .map(|e| e.path())
        .filter_map(|p| {
            let tick = parse_voxel_tick(p.file_name()?.to_str()?)?;
            p.is_file().then_some((tick, p))
        })
        .max_by_key(|(t, _)| *t)
}

/// **A session's load**: the newest whole save in `dir`. A save from another version is
/// refused with [`LoadError::OtherVersion`]; nothing is discarded, and nothing is migrated.
pub fn load_save(dir: &Path) -> std::result::Result<LoadedSave, LoadError> {
    if !dir.is_dir() {
        return Err(LoadError::Missing(format!("{} is not a save directory", dir.display())));
    }
    let now = Schemas::current();
    if let Ok(meta) = read_meta(dir) {
        for (layer, found, expected, legacy) in [
            ("world", meta.schema.world, now.world, LEGACY.world),
            ("flora", meta.schema.flora, now.flora, LEGACY.flora),
            ("fauna", meta.schema.fauna, now.fauna, LEGACY.fauna),
        ] {
            if !readable(found, expected, legacy) {
                return Err(LoadError::OtherVersion { layer, found, expected });
            }
        }
    }
    let (tick, world_file) = newest_world(dir)
        .ok_or_else(|| LoadError::Missing(format!("no world-N.voxel in {}", dir.display())))?;
    let read = |p: &Path| std::fs::read(p).map_err(|e| LoadError::Damaged(format!("{}: {e}", p.display())));
    let bytes = read(&world_file)?;
    let world = match World::load(&bytes) {
        Ok(w) => w,
        Err(e) => {
            return Err(match e.downcast_ref::<cubarium_voxel::SchemaMismatch>() {
                Some(m) => LoadError::OtherVersion { layer: "world", found: m.found, expected: m.expected },
                None => LoadError::Damaged(format!("{}: {e:#}", world_file.display())),
            });
        }
    };
    let (flora_file, fauna_file) = layer_files(dir, tick);
    if !flora_file.is_file() || !fauna_file.is_file() {
        return Err(LoadError::Damaged(format!(
            "the plants and animals of tick {tick} are missing from {}",
            dir.display()
        )));
    }
    let flora_bytes = read(&flora_file)?;
    if let Some(t) = schema_tag(&flora_bytes).filter(|t| !readable(*t, now.flora, LEGACY.flora)) {
        return Err(LoadError::OtherVersion { layer: "flora", found: t, expected: now.flora });
    }
    let fauna_bytes = read(&fauna_file)?;
    if let Some(t) = schema_tag(&fauna_bytes).filter(|t| !readable(*t, now.fauna, LEGACY.fauna)) {
        return Err(LoadError::OtherVersion { layer: "fauna", found: t, expected: now.fauna });
    }
    let flora = cubarium_voxel_flora::snapshot::decode(&flora_bytes)
        .map_err(|e| LoadError::Damaged(format!("{}: {e:#}", flora_file.display())))?;
    let fauna = Fauna::load(&fauna_bytes)
        .map_err(|e| LoadError::Damaged(format!("{}: {e:#}", fauna_file.display())))?;
    Ok(LoadedSave { world, flora, fauna, dir: dir.to_path_buf(), world_file })
}

/// The plant and animal layers saved with `world-N.voxel`, if both are there and load.
/// `None` (said on stderr when the files exist) means the caller seeds afresh.
pub fn load_layers(dir: &Path, tick: u64) -> Option<(Flora, Fauna)> {
    let (flora_file, fauna_file) = layer_files(dir, tick);
    if !flora_file.is_file() || !fauna_file.is_file() {
        return None;
    }
    let loaded = std::fs::read(&flora_file)
        .map_err(anyhow::Error::from)
        .and_then(|b| cubarium_voxel_flora::snapshot::decode(&b))
        .and_then(|flora| {
            let fauna = std::fs::read(&fauna_file)
                .map_err(anyhow::Error::from)
                .and_then(|b| Fauna::load(&b))?;
            Ok((flora, fauna))
        });
    match loaded {
        Ok(layers) => Some(layers),
        Err(e) => {
            eprintln!(
                "cubarium voxel: the plants and animals saved at tick {tick} do not load \
                 ({e:#}); seeding afresh"
            );
            None
        }
    }
}

/// A resumed world, its label, whether it was resumed, and its saved plants and animals
/// when they load.
pub type Resumed = (World, String, bool, Option<(Flora, Fauna)>);

/// **The ambient display's `--load PATH`**: resume a world file or a state directory, if
/// there is one to resume; `None` means found a new one. A directory whose every snapshot
/// is of an older format is discarded, loudly, and a fresh world founded; anything else
/// that does not load refuses by name.
pub fn resume(path: &Path) -> Result<Option<Resumed>> {
    if path.is_file() {
        let bytes =
            std::fs::read(path).with_context(|| format!("reading the world {}", path.display()))?;
        let world = World::load(&bytes).with_context(|| format!("loading {}", path.display()))?;
        let label = path.display().to_string();
        eprintln!("cubarium voxel: resumed world from {}", path.display());
        return Ok(Some((world, label, true, None)));
    } else if path.is_dir() {
        let mut candidates: Vec<(u64, std::time::SystemTime, PathBuf)> = Vec::new();
        for entry in std::fs::read_dir(path)
            .with_context(|| format!("reading state directory {}", path.display()))?
        {
            let entry = entry?;
            let p = entry.path();
            if p.is_file() {
                let name = p.file_name().and_then(|n| n.to_str()).unwrap_or("");
                if name.ends_with(".voxel") {
                    let tick = parse_voxel_tick(name).unwrap_or(0);
                    let mtime = entry
                        .metadata()
                        .and_then(|m| m.modified())
                        .unwrap_or(std::time::SystemTime::UNIX_EPOCH);
                    candidates.push((tick, mtime, p));
                }
            }
        }
        candidates.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| b.1.cmp(&a.1)));
        if !candidates.is_empty() {
            // A refusal is *stale* when the file is a perfectly good snapshot of a
            // format this build no longer speaks, and *damaged* when it is anything
            // else. Told apart by the error's own type, never by its text.
            let mut stale: Vec<(PathBuf, u32)> = Vec::new();
            for (tick, _, cand_path) in &candidates {
                match std::fs::read(cand_path) {
                    Ok(bytes) => match World::load(&bytes) {
                        Ok(world) => {
                            eprintln!(
                                "cubarium voxel: resuming {} at tick {tick}",
                                cand_path.display()
                            );
                            let label = format!("resumed from {}", cand_path.display());
                            let layers = load_layers(path, *tick);
                            if let Some((flora, fauna)) = &layers {
                                eprintln!(
                                    "cubarium voxel: resumed its plants and animals — \
                                     {} stands, {} latticevines, {} animals",
                                    flora.view().stands.len(),
                                    flora.view().cover.vines().len(),
                                    fauna.view().animals.len(),
                                );
                            }
                            return Ok(Some((world, label, true, layers)));
                        }
                        Err(e) => {
                            eprintln!(
                                "cubarium voxel: skipping corrupt snapshot {}: {e}",
                                cand_path.display()
                            );
                            if let Some(m) = e.downcast_ref::<cubarium_voxel::SchemaMismatch>() {
                                stale.push((cand_path.clone(), m.found));
                            }
                        }
                    },
                    Err(e) => {
                        eprintln!(
                            "cubarium voxel: skipping unreadable snapshot {}: {e}",
                            cand_path.display()
                        );
                    }
                }
            }
            // **Stale is not damaged.** A directory whose every snapshot is simply of
            // an older format holds nothing that can be recovered — the standing rule
            // is fresh, never migrate — and refusing to start leaves the panel showing
            // the last frame it drew until somebody ssh's in. That is exactly what it
            // did: five worlds of schema 13 against a build that speaks 15, and a
            // crash loop (Wrysk, 2026-09-21, who does not want them kept). So they are
            // discarded, loudly, and a fresh world founded. Anything else still
            // refuses by name below.
            if stale.len() == candidates.len() {
                let mut had: Vec<u32> = stale.iter().map(|(_, n)| *n).collect();
                had.sort_unstable();
                had.dedup();
                let names: Vec<String> = had.iter().map(|n| n.to_string()).collect();
                for (p, _) in &stale {
                    std::fs::remove_file(p).with_context(|| {
                        format!("discarding the stale snapshot {}", p.display())
                    })?;
                }
                eprintln!(
                    "cubarium voxel: discarded {} snapshot(s) of schema {} in {}; \
                     founding a fresh world (schema {})",
                    stale.len(),
                    names.join(", "),
                    path.display(),
                    cubarium_voxel::snapshot::SCHEMA,
                );
            } else {
                bail!(
                    "{} snapshot file(s) are present in {} and none of them loaded",
                    candidates.len(),
                    path.display()
                );
            }
        }
        eprintln!(
            "cubarium voxel: no loadable snapshot in {}; creating a new world",
            path.display()
        );
    } else {
        // Path does not exist yet
        if path.extension().is_some() {
            bail!("cannot load world from {}: file not found", path.display());
        } else {
            std::fs::create_dir_all(path)
                .with_context(|| format!("creating state directory {}", path.display()))?;
            eprintln!(
                "cubarium voxel: created state directory {}; creating a new world",
                path.display()
            );
        }
    }
    Ok(None)
}
