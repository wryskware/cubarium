//! Mechanisms only, on tiny founded worlds: no pinned numbers, no tuned values, nothing
//! past a few hundred ticks.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use cubarium_observatory::Command;
use serde_json::{Value, json};

use crate::feed::{CHUNK, CHUNK_CELLS, View};
use crate::session::prepare_new;
use crate::{Session, SessionOptions, SimParams, WorldSpec, store};

/// A scratch directory of the test's own, removed when it drops.
struct Scratch(PathBuf);

impl Scratch {
    fn new(tag: &str) -> Scratch {
        let dir = std::env::temp_dir().join(format!(
            "cubarium-session-{tag}-{}-{}",
            std::process::id(),
            Instant::now().elapsed().as_nanos()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        Scratch(dir)
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// A tiny world's config file: a 32 x 24 x 4 strip at 0.25 m, with a lake gate that
/// passes anything and a one-second opening shower, so founding is quick.
fn tiny_config(dir: &Path) -> PathBuf {
    let path = dir.join("tiny.toml");
    std::fs::write(
        &path,
        "tilt_degrees = 30.0 # a presentation key: the session ignores it\n\
         threads = 2\n\
         [world]\nwidth = 32\nheight = 24\ndepth = 4\nvoxel_m = 0.25\n\
         [world.landform]\npreset = \"default\"\n\
         [world.landform.water]\nmin_lake_m2 = 0.0\nmin_tier_pools = 0\n\
         [world.climate]\nopening_min_s = 1.0\n",
    )
    .unwrap();
    path
}

fn spec(dir: &Path, seed: u64) -> WorldSpec {
    WorldSpec { config: Some(tiny_config(dir)), seed: Some(seed), ..WorldSpec::default() }
}

fn options() -> SessionOptions {
    SessionOptions { exec: SimParams { threads: 2, ..SimParams::default() }, ..SessionOptions::default() }
}

/// A session running a freshly founded tiny world, and its scratch directory.
fn running(tag: &str) -> (Session, Scratch) {
    let scratch = Scratch::new(tag);
    let s = spec(&scratch.0, 5);
    let prepared = prepare_new(&s.sim_params().unwrap(), &s).expect("a tiny world founds");
    let session = Session::new(options());
    session.install(prepared);
    wait(&session, "the world is installed", |st| st.running && st.generation == 1);
    (session, scratch)
}

fn wait(session: &Session, what: &str, until: impl Fn(&crate::Status) -> bool) -> crate::Status {
    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
        let st = session.status();
        if until(&st) {
            return st;
        }
        assert!(Instant::now() < deadline, "timed out waiting until {what}: {st:?}");
        std::thread::sleep(Duration::from_millis(5));
    }
}

fn pause(session: &Session) -> u64 {
    session.command(Command::Pause);
    wait(session, "paused", |st| st.paused).tick
}

/// The first view of the current generation (the reset may still be in flight).
fn take_current(session: &Session) -> View {
    let generation = session.status().generation;
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        let v = session.take_view();
        if v.generation == generation {
            return v;
        }
        assert!(Instant::now() < deadline, "no view of generation {generation}");
        std::thread::sleep(Duration::from_millis(5));
    }
}

/// The material byte of cell `(x, y, z)` in the chunks of `view`, if its chunk is there.
fn material_in(view: &View, (x, y, z): (u32, u32, u32)) -> Option<u8> {
    let at = [x / CHUNK, y / CHUNK, z / CHUNK];
    let c = view.chunks.iter().find(|c| c.at == at)?;
    let (lx, ly, lz) = ((x % CHUNK) as usize, (y % CHUNK) as usize, (z % CHUNK) as usize);
    Some(c.bytes[(lz * CHUNK as usize + ly) * CHUNK as usize + lx])
}

/// Air cells of the world in a full view, one per distinct chunk, at most `n`.
fn air_cells(view: &View, n: usize) -> Vec<(u32, u32, u32)> {
    let [w, h, d] = view.dims;
    let mut out: Vec<(u32, u32, u32)> = Vec::new();
    for y in (0..h).rev() {
        for z in 0..d {
            for x in 0..w {
                if material_in(view, (x, y, z)) == Some(0)
                    && !out.iter().any(|o| [o.0 / CHUNK, o.1 / CHUNK, o.2 / CHUNK] == [x / CHUNK, y / CHUNK, z / CHUNK])
                {
                    out.push((x, y, z));
                    if out.len() == n {
                        return out;
                    }
                }
            }
        }
    }
    out
}

/// Takes views until `pred` holds over everything taken since the call, or panics.
fn take_until(session: &Session, what: &str, pred: impl Fn(&[View]) -> bool) -> Vec<View> {
    let deadline = Instant::now() + Duration::from_secs(10);
    let mut taken = Vec::new();
    loop {
        taken.push(session.take_view());
        if pred(&taken) {
            return taken;
        }
        assert!(Instant::now() < deadline, "timed out waiting until {what}");
        std::thread::sleep(Duration::from_millis(10));
    }
}

fn counts(meta_or_pops: &Value, key: &str) -> Vec<(String, u64)> {
    meta_or_pops[key]
        .as_array()
        .unwrap()
        .iter()
        .map(|row| match row {
            Value::Array(pair) => (pair[0].as_str().unwrap().to_string(), pair[1].as_u64().unwrap()),
            obj => (
                obj["species"].as_str().unwrap().to_string(),
                obj.get("alive").or(obj.get("stands")).and_then(Value::as_u64).unwrap(),
            ),
        })
        .collect()
}

#[test]
fn pause_stops_ticks() {
    let (session, _dir) = running("pause");
    session.console("speed 8");
    let before = session.status().tick;
    wait(&session, "the world ticks", |st| st.tick > before);
    let at = pause(&session);
    // Anything already due when the pause landed has landed by now.
    std::thread::sleep(Duration::from_millis(100));
    let at = session.status().tick.max(at);
    std::thread::sleep(Duration::from_millis(400));
    assert_eq!(session.status().tick, at, "a paused world does not tick");
}

#[test]
fn step_n_while_paused_advances_exactly_n_ticks() {
    let (session, _dir) = running("step");
    pause(&session);
    std::thread::sleep(Duration::from_millis(100));
    let t0 = session.status().tick;
    session.command(Command::Step(5));
    wait(&session, "five steps", |st| st.tick >= t0 + 5);
    std::thread::sleep(Duration::from_millis(300));
    let st = session.status();
    assert_eq!(st.tick, t0 + 5, "exactly the steps asked for");
    assert!(st.paused, "stepping leaves it paused");
}

#[test]
fn save_then_load_gives_back_the_tick_and_the_species_counts() {
    let (session, dir) = running("roundtrip");
    session.console("speed 4");
    let before = session.status().tick;
    wait(&session, "the world ticks", |st| st.tick > before);
    pause(&session);
    std::thread::sleep(Duration::from_millis(100));
    let save = dir.0.join("slot");
    let meta = session.save_blocking(&save, Duration::from_secs(30)).expect("saves");
    let meta_json = serde_json::to_value(&meta).unwrap();
    assert_eq!(meta.tick, session.status().tick, "the save is of the world as it stood");
    // Move on, then come back.
    session.command(Command::Step(3));
    wait(&session, "three more steps", |st| st.tick == meta.tick + 3);
    let generation = session.status().generation;
    session.load_blocking(&save, Duration::from_secs(30)).expect("loads");
    let st = wait(&session, "the loaded world is installed", |st| st.generation > generation);
    assert_eq!(st.tick, meta.tick, "the tick the save was made at");
    assert!(st.paused, "loaded over a paused world, it stays paused");
    // The first publication of the loaded world.
    let deadline = Instant::now() + Duration::from_secs(10);
    let pops = loop {
        if let Ok(snap) = session.query("snapshot", &json!({}))
            && snap["summary"]["tick"].as_u64() == Some(meta.tick)
        {
            break snap["populations"].clone();
        }
        assert!(Instant::now() < deadline, "no publication of the loaded world");
        std::thread::sleep(Duration::from_millis(10));
    };
    assert_eq!(counts(&pops, "animals"), counts(&meta_json, "animals"));
    assert_eq!(counts(&pops, "plants"), counts(&meta_json, "plants"));
}

#[test]
fn a_save_from_another_version_is_refused_with_a_plain_error() {
    let (session, dir) = running("schema");
    pause(&session);
    let save = dir.0.join("slot");
    session.save_blocking(&save, Duration::from_secs(30)).expect("saves");

    // Its metadata says another schema.
    let meta_path = save.join(store::META_FILE);
    let mut meta: Value = serde_json::from_slice(&std::fs::read(&meta_path).unwrap()).unwrap();
    let real = meta["schema"]["world"].as_u64().unwrap();
    meta["schema"]["world"] = json!(real + 1000);
    std::fs::write(&meta_path, serde_json::to_vec(&meta).unwrap()).unwrap();
    let generation = session.status().generation;
    let err = session.load_blocking(&save, Duration::from_secs(30)).expect_err("refused");
    assert!(err.contains("another version"), "{err}");
    assert_eq!(session.status().generation, generation, "the running world is kept");

    // No metadata, and the world file itself is of another schema.
    std::fs::remove_file(&meta_path).unwrap();
    let world = std::fs::read_dir(&save)
        .unwrap()
        .flatten()
        .map(|e| e.path())
        .find(|p| p.extension().is_some_and(|e| e == "voxel"))
        .unwrap();
    let mut bytes = postcard::to_stdvec(&(real as u32 + 1000)).unwrap();
    bytes.extend_from_slice(&[0u8; 16]);
    std::fs::write(&world, bytes).unwrap();
    let err = session.load_blocking(&save, Duration::from_secs(30)).expect_err("refused");
    assert!(err.contains("another version"), "{err}");
    // The reply came straight from the loop; the notice travels through the reader's hub.
    let deadline = Instant::now() + Duration::from_secs(10);
    while !session.take_notices().iter().any(|n| n.kind == "load_failed") {
        assert!(Instant::now() < deadline, "the refusal is told to the reader");
        std::thread::sleep(Duration::from_millis(5));
    }
}

#[test]
fn the_metadata_is_read_without_loading_the_world() {
    let (session, dir) = running("meta");
    pause(&session);
    let saves = dir.0.join("saves");
    let slot = saves.join("one");
    let meta = session.save_blocking(&slot, Duration::from_secs(30)).expect("saves");
    // The world file is gone: what is read is the metadata alone.
    for p in std::fs::read_dir(&slot).unwrap().flatten().map(|e| e.path()) {
        if p.extension().is_some_and(|e| e == "voxel") {
            std::fs::write(&p, b"not a world").unwrap();
        }
    }
    let read = store::read_meta(&slot).expect("reads");
    assert_eq!(read, meta);
    let st = session.status();
    assert_eq!(read.size, [32, 24, 4]);
    assert_eq!(read.tick, st.tick);
    assert!(!read.animals.is_empty() && !read.plants.is_empty(), "species are listed");

    // A slot list shows it, and marks one of another version as not loadable.
    let other = saves.join("two");
    std::fs::create_dir_all(&other).unwrap();
    let mut old = serde_json::to_value(&meta).unwrap();
    old["schema"]["fauna"] = json!(0);
    std::fs::write(other.join(store::META_FILE), serde_json::to_vec(&old).unwrap()).unwrap();
    std::fs::write(other.join("world-1.voxel"), b"").unwrap();
    let slots = store::list_saves(&saves);
    let one = slots.iter().find(|s| s.name == "one").expect("listed");
    assert!(one.loadable && one.meta.as_ref() == Some(&meta));
    let two = slots.iter().find(|s| s.name == "two").expect("listed");
    assert!(!two.loadable);
    assert!(two.problem.as_deref().is_some_and(|p| p.contains("another version")), "{:?}", two.problem);
    let via_query = session.query("saves", &json!({ "dir": saves.display().to_string() })).unwrap();
    assert_eq!(via_query.as_array().map(Vec::len), Some(2));
}

#[test]
fn candidates_are_independent_and_promoting_one_drops_the_others() {
    let scratch = Scratch::new("candidates");
    let session = Session::new(options());
    session.generate_candidates(&spec(&scratch.0, 21), 2).expect("starts");
    let deadline = Instant::now() + Duration::from_secs(120);
    let infos = loop {
        let infos = session.candidates();
        assert_eq!(infos.len(), 2);
        assert!(infos.iter().all(|c| c.state != "failed"), "{infos:?}");
        if infos.iter().all(|c| c.state == "ready") {
            break infos;
        }
        assert!(Instant::now() < deadline, "the candidates did not found in time");
        std::thread::sleep(Duration::from_millis(20));
    };
    assert_eq!(session.status().generation, 0, "nothing runs before a promotion");
    // Each is its own world: the seed it was asked for, its own view.
    let seeds: Vec<u64> = infos.iter().map(|c| c.summary.as_ref().unwrap().seed).collect();
    assert_eq!(seeds, vec![21, 22]);
    let (a, b) = (session.candidate_view(0).unwrap(), session.candidate_view(1).unwrap());
    assert!(a.full && b.full);
    assert_eq!(a.chunks.len(), b.chunks.len());
    assert!(a.frame.is_some() && b.frame.is_some());

    session.promote(0).expect("promotes");
    assert!(session.candidates().is_empty(), "the others are dropped");
    assert!(session.candidate_view(1).is_none());
    assert!(session.promote(1).is_err());
    let st = wait(&session, "the promoted world runs", |st| st.running && st.generation == 1);
    assert_eq!(Some(st.label), infos[0].label.clone(), "it is candidate 0");
}

#[test]
fn a_changed_voxel_appears_in_the_next_view() {
    let (session, _dir) = running("voxel");
    pause(&session);
    let first = take_current(&session);
    let cell = air_cells(&first, 1)[0];
    session.console(&format!("m {} {} {} rock", cell.0, cell.1, cell.2));
    take_until(&session, "the edit is in a view", |views| {
        views.iter().any(|v| material_in(v, cell) == Some(2))
    });
}

#[test]
fn two_ticks_without_a_take_accumulate_both_changes() {
    let (session, _dir) = running("union");
    pause(&session);
    let first = take_current(&session);
    let cells = air_cells(&first, 2);
    assert_eq!(cells.len(), 2, "two air cells in two chunks");
    let t0 = session.status().tick;
    session.console(&format!("m {} {} {} rock", cells[0].0, cells[0].1, cells[0].2));
    session.command(Command::Step(1));
    wait(&session, "the first tick", |st| st.tick == t0 + 1);
    session.console(&format!("m {} {} {} rock", cells[1].0, cells[1].1, cells[1].2));
    session.command(Command::Step(1));
    wait(&session, "the second tick", |st| st.tick == t0 + 2);
    // Once the second tick's feed has reached the hub, take once.
    let deadline = Instant::now() + Duration::from_secs(10);
    while session.pending_frame_tick() != Some(t0 + 2) {
        assert!(Instant::now() < deadline, "the second tick's feed never arrived");
        std::thread::sleep(Duration::from_millis(5));
    }
    let v = session.take_view();
    assert_eq!(material_in(&v, cells[0]), Some(2), "the first tick's change");
    assert_eq!(material_in(&v, cells[1]), Some(2), "the second tick's change");
}

#[test]
fn the_first_take_after_a_load_returns_every_chunk() {
    let (session, dir) = running("full");
    pause(&session);
    let save = dir.0.join("slot");
    session.save_blocking(&save, Duration::from_secs(30)).expect("saves");
    // Drain whatever is pending, so only the load can fill the next take.
    let _ = take_current(&session);
    session.load_blocking(&save, Duration::from_secs(30)).expect("loads");
    let v = take_current(&session);
    let [cx, cy, cz] = v.chunk_dims;
    assert!(v.full);
    assert_eq!(v.chunks.len(), (cx * cy * cz) as usize, "every chunk");
    assert!(v.chunks.iter().all(|c| c.bytes.len() == 3 * CHUNK_CELLS));
    assert!(v.frame.is_some(), "and the organisms");
    let next = session.take_view();
    assert!(!next.full && next.chunks.len() < v.chunks.len(), "only the first take is whole");
}

#[test]
fn a_slow_reader_never_blocks_a_tick() {
    let (session, _dir) = running("slow");
    session.console("speed 4");
    let before = session.status().stepped;
    wait(&session, "the world ticks", |st| st.stepped > before);
    let session = std::sync::Arc::new(session);
    let reader = {
        let s = session.clone();
        std::thread::spawn(move || s.hold_hub_for(Duration::from_millis(800)))
    };
    // The reader now holds the hub; the loop goes on stepping.
    std::thread::sleep(Duration::from_millis(100));
    let during = session.status().stepped;
    std::thread::sleep(Duration::from_millis(500));
    let after = session.status().stepped;
    assert!(!reader.is_finished(), "the reader was still holding the hub");
    assert!(after > during, "ticks went on while the reader held the hub ({during} -> {after})");
    reader.join().unwrap();
    // And what was produced meanwhile reaches the reader once it lets go.
    let v = session.take_view();
    assert!(v.frame.is_some());
}

#[test]
fn a_spec_with_an_unknown_field_is_refused() {
    let bad: Result<WorldSpec, _> = serde_json::from_value(json!({ "sise": [1, 2, 3] }));
    assert!(bad.is_err());
    let scratch = Scratch::new("spec");
    let path = scratch.0.join("bad.toml");
    std::fs::write(&path, "[world]\nwidht = 4\n").unwrap();
    let spec = WorldSpec { config: Some(path), ..WorldSpec::default() };
    assert!(spec.sim_params().is_err(), "a misspelt [world] key is an error");
}

/// The shipped terrarium configs give a session its parameter set: the simulation half,
/// the presentation keys left to the binary.
#[test]
fn the_shipped_terrarium_configs_give_a_parameter_set() {
    let repo = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    for file in [
        "config/desktop/terrarium.toml",
        "config/desktop/terrarium-d72.toml",
        "config/desktop/terrarium-d96.toml",
        "config/desktop/terrarium-wide.toml",
        "config/voidrunner/terrarium.toml",
    ] {
        let spec = WorldSpec { config: Some(repo.join(file)), ..WorldSpec::default() };
        let params = spec.sim_params().unwrap_or_else(|e| panic!("{file}: {e:#}"));
        params.resolve_sets(&[]).unwrap_or_else(|e| panic!("{file}: {e:#}"));
    }
}
