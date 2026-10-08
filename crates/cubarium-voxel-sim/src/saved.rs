//! **One safe load** (staged runs, 2026-10-04): reading a saved habitat back and giving
//! its animals their drivers again, in one call every tool uses.
//!
//! Controllers and birth recipes are session state: the fauna snapshot skips them
//! (`Fauna`'s `serde(skip)`), so a layer read back from bytes has bodies with nothing
//! driving them. Every one rests where it stands — no steps, no bites, no burrs — and a
//! birth gets no mind. A 5 h study batch ran for 2 h like that before anyone looked. So
//! no tool decodes the fauna layer itself: [`load`] (a save directory) and [`decode`]
//! (three byte buffers) put every lineage's heuristic back as its birth factory and on
//! every standing founder body ([`crate::habitat::reinstall_heuristics`]), and refuse a
//! save whose lineage was driven by a saved policy unless the caller says it puts that
//! driver back itself.
//!
//! The layout is the studies' (`world.bin`, `flora.bin`, `fauna.bin`, `minute`); the
//! render tools' `world.voxel` is read too. [`save`] writes the studies' layout, whole or
//! not at all (a `.partial` directory renamed into place).

use std::path::Path;

use cubarium_voxel::World;
use cubarium_voxel_fauna::{Fauna, Founder, TICK_HZ};
use cubarium_voxel_flora::Flora;

use crate::habitat;

/// A save read back, its animals driven again.
pub struct Saved {
    pub world: World,
    pub flora: Flora,
    pub fauna: Fauna,
    /// The sim minute it was saved at: the `minute` file, else the world's tick.
    pub minute: u64,
}

/// Give a loaded layer its drivers back: every lineage's heuristic as its birth factory
/// and on every standing founder body. `policy` names the lineages the caller drives by
/// a saved policy **after** this (it installs them over the heuristics); a lineage the
/// save records as policy-driven and `policy` does not name is refused — a policy-driven
/// world is never silently put back on the heuristic.
pub fn restore(fauna: &mut Fauna, policy: &[Founder]) -> Result<(), String> {
    for f in Founder::ALL {
        if fauna.policy_driven(f) && !policy.contains(&f) {
            return Err(format!(
                "its {} founders were driven by a saved policy and nothing here puts that policy \
                 back; this tool would run them on the heuristic",
                f.name()
            ));
        }
    }
    habitat::reinstall_heuristics(fauna);
    Ok(())
}

/// Decode three layers and [`restore`] the animals' drivers.
pub fn decode(world: &[u8], flora: &[u8], fauna: &[u8], policy: &[Founder]) -> Result<(World, Flora, Fauna), String> {
    let world = World::load(world).map_err(|e| format!("world: {e:#}"))?;
    let flora = cubarium_voxel_flora::snapshot::decode(flora).map_err(|e| format!("flora: {e:#}"))?;
    let mut fauna = Fauna::load(fauna).map_err(|e| format!("fauna: {e:#}"))?;
    restore(&mut fauna, policy)?;
    Ok((world, flora, fauna))
}

/// Read a save directory and [`restore`] its animals' drivers.
pub fn load(dir: &Path, policy: &[Founder]) -> Result<Saved, String> {
    let read = |n: &str| std::fs::read(dir.join(n)).map_err(|e| format!("{}: {e}", dir.join(n).display()));
    let world_file = if dir.join("world.bin").is_file() { "world.bin" } else { "world.voxel" };
    let (world, flora, fauna) =
        decode(&read(world_file)?, &read("flora.bin")?, &read("fauna.bin")?, policy).map_err(|e| format!("{}: {e}", dir.display()))?;
    let minute = match std::fs::read_to_string(dir.join("minute")) {
        Ok(text) => text.trim().parse().map_err(|e| format!("{}/minute: {e}", dir.display()))?,
        Err(_) => world.tick() / (60 * u64::from(TICK_HZ)),
    };
    Ok(Saved { world, flora, fauna, minute })
}

/// Write a save directory: the three layers and the minute, into `DIR.partial` first and
/// renamed over `DIR`, so a run cut off mid-write never leaves half a save.
pub fn save(dir: &Path, world: &World, flora: &Flora, fauna: &Fauna, minute: u64) -> Result<(), String> {
    let mut partial = dir.as_os_str().to_owned();
    partial.push(".partial");
    let partial = std::path::PathBuf::from(partial);
    let _ = std::fs::remove_dir_all(&partial);
    std::fs::create_dir_all(&partial).map_err(|e| format!("{}: {e}", partial.display()))?;
    let put = |name: &str, bytes: &[u8]| std::fs::write(partial.join(name), bytes).map_err(|e| format!("{}: {e}", partial.join(name).display()));
    put("world.bin", &world.save())?;
    put("flora.bin", &cubarium_voxel_flora::snapshot::encode(flora))?;
    put("fauna.bin", &fauna.save())?;
    put("minute", minute.to_string().as_bytes())?;
    if dir.exists() {
        std::fs::remove_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    }
    std::fs::rename(&partial, dir).map_err(|e| format!("{} -> {}: {e}", partial.display(), dir.display()))
}

/// **Every animal is driven**: a controller on every founder body and a birth factory for
/// every lineage with a body standing. The reason it fails, when it does.
pub fn driven(fauna: &Fauna) -> Result<(), String> {
    let v = fauna.view();
    let mut idle = [0usize; Founder::COUNT];
    let mut present = [false; Founder::COUNT];
    for a in v.animals {
        let Some(f) = a.founder else { continue };
        present[f.index()] = true;
        if !fauna.has_controller(a.id) {
            idle[f.index()] += 1;
        }
    }
    let mut why = Vec::new();
    for f in Founder::ALL {
        if idle[f.index()] > 0 {
            why.push(format!("{} {} bodies have no controller", idle[f.index()], f.name()));
        }
        if present[f.index()] && !fauna.has_founder_factory(f) {
            why.push(format!("{} has no birth factory", f.name()));
        }
    }
    if why.is_empty() { Ok(()) } else { Err(why.join("; ")) }
}

/// **Rewrite saved layers in the current format** (SAVE-EXTENSIBLE's one-version legacy
/// reader): every layer file under `root` (recursively) — `world.bin`, `world.voxel` or
/// `world-N.voxel`, `flora*.bin`, `fauna*.bin` — that this build reads but does not write is
/// decoded and written back in the current format, whole or not at all (a `.resave` file
/// renamed over it). Files already current are left alone. The paths rewritten.
///
/// Run it (`voxel_resave`) over a save library while its legacy schemas are still readable,
/// so the legacy reader can be retired without losing the library.
pub fn resave(root: &Path) -> Result<Vec<std::path::PathBuf>, String> {
    let mut done = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let entries = std::fs::read_dir(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
        for entry in entries {
            let path = entry.map_err(|e| format!("{}: {e}", dir.display()))?.path();
            if path.is_dir() {
                stack.push(path);
                continue;
            }
            let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            let layer = if name == "world.bin" || (name.starts_with("world") && name.ends_with(".voxel")) {
                cubarium_voxel::snapshot::SCHEMA
            } else if name.starts_with("flora") && name.ends_with(".bin") {
                cubarium_voxel_flora::snapshot::SCHEMA
            } else if name.starts_with("fauna") && name.ends_with(".bin") {
                cubarium_voxel_fauna::SCHEMA
            } else {
                continue;
            };
            let raw = std::fs::read(&path).map_err(|e| format!("{}: {e}", path.display()))?;
            if cubarium_voxel::snapshot::schema_tag(&raw) == Some(layer) {
                continue;
            }
            let bytes = if name.starts_with("world") {
                World::load(&raw).map(|w| w.save())
            } else if name.starts_with("flora") {
                cubarium_voxel_flora::snapshot::decode(&raw).map(|f| cubarium_voxel_flora::snapshot::encode(&f))
            } else {
                Fauna::load(&raw).map(|f| f.save())
            }
            .map_err(|e| format!("{}: {e:#}", path.display()))?;
            let mut tmp = path.as_os_str().to_owned();
            tmp.push(".resave");
            let tmp = std::path::PathBuf::from(tmp);
            std::fs::write(&tmp, &bytes).map_err(|e| format!("{}: {e}", tmp.display()))?;
            std::fs::rename(&tmp, &path).map_err(|e| format!("{} -> {}: {e}", tmp.display(), path.display()))?;
            done.push(path);
        }
    }
    done.sort();
    Ok(done)
}
