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

/// A world's home is where it was last loaded from or saved into through the session: a
/// founded world has none, a refused load or a console save does not move it, and
/// another world installed, or the world closed, drops it.
#[test]
fn the_home_follows_the_worlds_own_loads_and_saves() {
    let (session, dir) = running("home");
    pause(&session);
    assert_eq!(session.home(), None, "a founded world has no home");
    let (a, b) = (dir.0.join("a"), dir.0.join("b"));
    session.save_blocking(&a, Duration::from_secs(30)).expect("saves");
    assert_eq!(session.home(), Some(a.clone()), "saved into a");
    session.save_blocking(&b, Duration::from_secs(30)).expect("saves");
    assert_eq!(session.stopper().home(), Some(b.clone()), "saved into b, read through the stopper");

    session.console(&format!("save {}", dir.0.join("console").display()));
    let deadline = Instant::now() + Duration::from_secs(10);
    while !dir.0.join("console").join(store::META_FILE).exists() || session.status().saving {
        assert!(Instant::now() < deadline, "the console save is written");
        std::thread::sleep(Duration::from_millis(5));
    }
    assert_eq!(session.home(), Some(b.clone()), "a console save is not a home");

    let missing = dir.0.join("missing");
    session.load_blocking(&missing, Duration::from_secs(30)).expect_err("nothing to load");
    assert_eq!(session.home(), Some(b.clone()), "a refused load keeps the home");

    let generation = session.status().generation;
    session.load_blocking(&a, Duration::from_secs(30)).expect("loads");
    wait(&session, "the load is installed", |st| st.generation > generation);
    assert_eq!(session.home(), Some(a.clone()), "loaded from a");

    let s = spec(&dir.0, 6);
    session.install(prepare_new(&s.sim_params().unwrap(), &s).expect("founds"));
    wait(&session, "the new world is installed", |st| st.generation > generation + 1);
    assert_eq!(session.home(), None, "a newly founded world has no home");

    session.load_blocking(&a, Duration::from_secs(30)).expect("loads");
    assert_eq!(session.home(), Some(a.clone()));
    session.close();
    wait(&session, "closed", |st| !st.running);
    assert_eq!(session.home(), None, "no world, no home");
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

/// Every candidate settled (ready, failed or cancelled), within `secs`.
fn settled(session: &Session, secs: u64) -> Vec<crate::CandidateInfo> {
    let deadline = Instant::now() + Duration::from_secs(secs);
    loop {
        let infos = session.candidates();
        if !infos.is_empty() && infos.iter().all(|c| c.state != "generating") {
            return infos;
        }
        assert!(Instant::now() < deadline, "the candidates did not settle in time: {infos:?}");
        std::thread::sleep(Duration::from_millis(20));
    }
}

/// The founding ended (any state but `founding`), within `secs`.
fn founded(session: &Session, secs: u64) -> crate::FoundingInfo {
    let deadline = Instant::now() + Duration::from_secs(secs);
    loop {
        let f = session.founding();
        if f.state != "founding" {
            return f;
        }
        assert!(Instant::now() < deadline, "the founding did not end in time: {f:?}");
        std::thread::sleep(Duration::from_millis(20));
    }
}

fn no_workers(session: &Session) {
    let deadline = Instant::now() + Duration::from_secs(60);
    while session.workers() > 0 {
        assert!(Instant::now() < deadline, "{} workers still running", session.workers());
        std::thread::sleep(Duration::from_millis(20));
    }
}

/// The material planes of a whole view, chunk by chunk (water left out).
fn materials(view: &View) -> Vec<([u32; 3], Vec<u8>)> {
    let mut out: Vec<([u32; 3], Vec<u8>)> =
        view.chunks.iter().map(|c| (c.at, c.bytes[..CHUNK_CELLS].to_vec())).collect();
    out.sort_by_key(|(at, _)| *at);
    out
}

fn options_judged(accept: fn(&cubarium_voxel_sim::habitat::Seeded) -> bool, tries: usize) -> SessionOptions {
    SessionOptions { accept: Some(accept), placement_tries: tries, ..options() }
}

/// **Candidates are bare terrains**: drawn side by side, nothing on them founded and
/// nothing running, each its own world with the seed it was asked for, its terrain hash
/// and the feasibility check's verdict.
#[test]
fn terrain_candidates_are_drawn_without_founding() {
    let scratch = Scratch::new("candidates");
    let session = Session::new(options());
    session.generate_candidates(&spec(&scratch.0, 21), 2).expect("starts");
    let infos = settled(&session, 120);
    assert!(infos.iter().all(|c| c.state == "ready"), "{infos:?}");
    assert_eq!(session.status().generation, 0, "nothing runs before a pick");
    let seeds: Vec<Option<u64>> = infos.iter().map(|c| c.seed).collect();
    assert_eq!(seeds, vec![Some(21), Some(22)]);
    for c in &infos {
        let pops = serde_json::to_value(c.populations.as_ref().expect("populations")).unwrap();
        let counted = |key: &str, field: &str| -> u64 {
            pops[key].as_array().map_or(0, |a| a.iter().map(|x| x[field].as_u64().unwrap_or(0)).sum())
        };
        assert_eq!(counted("animals", "alive") + counted("animals", "eggs"), 0, "no animal on a terrain: {pops}");
        assert_eq!(counted("plants", "stands"), 0, "no plant on a terrain: {pops}");
        assert!(c.terrain.is_some() && c.feasible.is_some(), "{c:?}");
    }
    let (a, b) = (session.candidate_view(0).unwrap(), session.candidate_view(1).unwrap());
    assert!(a.full && b.full);
    assert_ne!(materials(&a), materials(&b), "two seeds, two terrains");
    assert!(a.frame.as_ref().is_some_and(|f| f.animals.ids.is_empty() && f.plants.ids.is_empty()));
    no_workers(&session);
}

/// **A renderer reading a candidate holds its world, not the candidate list**: while a
/// reader is inside [`crate::CandidateSource::with`] (a thumbnail's renderer opening and
/// packing it, which takes seconds on a wide world), the picker's `candidates()` and a new
/// batch both go through, and the world handed out stays whole after the list moves on.
#[test]
fn a_candidate_reader_never_holds_the_list() {
    let scratch = Scratch::new("reader");
    let session = std::sync::Arc::new(Session::new(options()));
    session.generate_candidates(&spec(&scratch.0, 21), 1).expect("starts");
    assert_eq!(settled(&session, 120)[0].state, "ready");
    assert!(session.candidate_ready(0) && !session.candidate_ready(1));
    let source = session.candidate_source();
    let held = source.get(0).expect("a ready candidate's world");
    let (w, h, d) = (held.world.config().width, held.world.config().height, held.world.config().depth);
    let other = scratch.0.clone();
    let answered = source
        .with(0, |world, _, _| {
            // From another thread, as the picker polls on Godot's: it must not wait for us.
            let (tx, rx) = std::sync::mpsc::channel();
            let s = session.clone();
            let t = std::thread::spawn(move || {
                let listed = s.candidates().len();
                let batch = s.generate_candidates(&spec(&other, 31), 2);
                let _ = tx.send((listed, batch));
            });
            (rx.recv_timeout(Duration::from_secs(30)), t, world.config().width)
        })
        .expect("the candidate is ready");
    let (got, t, width) = answered;
    t.join().unwrap();
    let (listed, batch) = got.expect("candidates() and a new batch went through while a reader held a world");
    assert_eq!((listed, width), (1, w));
    assert!(batch.is_ok(), "{batch:?}");
    // The new batch replaced the list; the world handed out before is still the whole one.
    assert_eq!(session.candidates().len(), 2);
    assert_eq!(held.world.view().material.len(), (w * h * d) as usize);
    session.cancel();
    no_workers(&session);
}

/// **Founding the picked terrain keeps it**: the founded, running world has the
/// candidate's terrain (hash and material, chunk for chunk), the founding is told
/// through its stages to `done`, and the other candidates are dropped once it runs.
#[test]
fn founding_the_picked_candidate_keeps_its_terrain() {
    let scratch = Scratch::new("found");
    let session = Session::new(options_judged(|_| true, 3));
    session.generate_candidates(&spec(&scratch.0, 21), 2).expect("starts");
    let infos = settled(&session, 120);
    let before = materials(&session.candidate_view(1).unwrap());
    session.promote(1).expect("founding starts");
    assert!(session.promote(0).is_err(), "one founding at a time");
    let mut phases: Vec<String> = Vec::new();
    let deadline = Instant::now() + Duration::from_secs(120);
    let f = loop {
        let f = session.founding();
        if !f.phase.is_empty() && phases.last() != Some(&f.phase) {
            phases.push(f.phase.clone());
        }
        assert!((0.0..=1.0).contains(&f.progress), "{f:?}");
        if f.state != "founding" {
            break f;
        }
        assert!(Instant::now() < deadline, "the founding did not end: {f:?}");
        std::thread::sleep(Duration::from_millis(1));
    };
    assert_eq!(f.state, "done", "{f:?}");
    assert_eq!(f.terrain, infos[1].terrain, "the founded world's terrain hash is the candidate's");
    assert_eq!(f.attempt, 1);
    assert!(phases.len() > 1, "the stages were told: {phases:?}");
    wait(&session, "the founded world runs", |st| st.running && st.generation == 1);
    assert!(session.candidates().is_empty(), "the candidates are dropped");
    let after = materials(&take_current(&session));
    assert_eq!(before, after, "the running world's terrain is the candidate's, chunk for chunk");
    no_workers(&session);
}

/// **A refused placement is retried on the same terrain, then reported**: every try
/// refused, the founding fails with a plain message after the tries it was given, the
/// candidates stay to pick from, and nothing is installed.
#[test]
fn a_refused_founding_retries_on_the_same_terrain_and_reports() {
    let scratch = Scratch::new("refused");
    let session = Session::new(options_judged(|_| false, 2));
    session.generate_candidates(&spec(&scratch.0, 21), 1).expect("starts");
    let infos = settled(&session, 120);
    session.promote(0).expect("founding starts");
    let f = founded(&session, 120);
    assert_eq!(f.state, "failed", "{f:?}");
    assert_eq!((f.attempt, f.attempts), (2, 2), "both tries made: {f:?}");
    assert!(f.error.as_deref().is_some_and(|e| e.contains("terrain")), "{f:?}");
    assert_eq!(f.terrain, infos[0].terrain, "on the candidate's terrain");
    assert_eq!(session.candidates().len(), 1, "the candidate stays");
    assert_eq!(session.candidates()[0].terrain, infos[0].terrain, "unchanged");
    std::thread::sleep(Duration::from_millis(100));
    assert_eq!(session.status().generation, 0, "nothing was installed");
    no_workers(&session);
}

/// **Cancel stops both**: candidates still being drawn (a spec no terrain can host, with
/// draws to spare, so they would go on) become `cancelled` and their threads end; a
/// founding in flight becomes `cancelled` at once, installs nothing, and its thread ends.
#[test]
fn cancel_stops_the_draws_and_the_founding() {
    let scratch = Scratch::new("cancel");
    let session = Session::new(SessionOptions { terrain_tries: 100_000, ..options() });
    let hopeless = WorldSpec {
        config: Some(tiny_config(&scratch.0)),
        seed: None,
        sets: vec!["fauna.founders.frondgrazer.founding_count=100000".into()],
        ..WorldSpec::default()
    };
    session.generate_candidates(&hopeless, 2).expect("starts");
    std::thread::sleep(Duration::from_millis(300));
    assert_eq!(session.workers(), 2, "both still drawing");
    session.cancel();
    assert!(session.candidates().iter().all(|c| c.state == "cancelled"), "{:?}", session.candidates());
    no_workers(&session);

    // The founding's verdict waits for the test, so the cancel lands mid-founding.
    static RELEASED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
    fn held(_: &cubarium_voxel_sim::habitat::Seeded) -> bool {
        while !RELEASED.load(std::sync::atomic::Ordering::SeqCst) {
            std::thread::sleep(Duration::from_millis(1));
        }
        false
    }
    let session = Session::new(options_judged(held, 3));
    session.generate_candidates(&spec(&scratch.0, 21), 1).expect("starts");
    settled(&session, 120);
    session.promote(0).expect("founding starts");
    let deadline = Instant::now() + Duration::from_secs(60);
    while session.founding().phase != "placing" {
        assert!(Instant::now() < deadline, "the founding never reached its verdict: {:?}", session.founding());
        std::thread::sleep(Duration::from_millis(5));
    }
    session.cancel();
    assert_eq!(session.founding().state, "cancelled", "cancelled at once");
    RELEASED.store(true, std::sync::atomic::Ordering::SeqCst);
    no_workers(&session);
    assert_eq!(session.founding().attempt, 1, "no further try was made");
    assert_eq!(session.founding().state, "cancelled", "it stays cancelled");
    assert_eq!(session.status().generation, 0, "nothing was installed");
    assert_eq!(session.candidates()[0].state, "ready", "a ready candidate stays");
}

/// **Stopping a session stops every thread it started** ([`crate::SessionStop`], what the
/// Godot extension does before its library is unloaded, and what a drop does): the loop
/// is joined, and draws still going and a founding in flight are cancelled and end on
/// their own without being waited for. A second stop finds nothing running.
#[test]
fn stopping_a_session_stops_its_draws_its_founding_and_its_loop() {
    let scratch = Scratch::new("stop");
    let session = Session::new(SessionOptions { terrain_tries: 100_000, ..options() });
    let hopeless = WorldSpec {
        config: Some(tiny_config(&scratch.0)),
        seed: None,
        sets: vec!["fauna.founders.frondgrazer.founding_count=100000".into()],
        ..WorldSpec::default()
    };
    session.generate_candidates(&hopeless, 2).expect("starts");
    std::thread::sleep(Duration::from_millis(300));
    assert_eq!(session.workers(), 2, "both still drawing");
    let stop = session.stopper();
    assert!(stop.shutdown(), "it had threads to stop");
    no_workers(&session);
    assert!(!stop.shutdown(), "a second stop finds nothing running");
    assert!(stop.alive(), "the session itself is still there");
    drop(session);
    assert!(!stop.alive(), "and gone once dropped");

    // The founding's verdict waits for the test, so the stop lands mid-founding.
    static RELEASED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
    fn held(_: &cubarium_voxel_sim::habitat::Seeded) -> bool {
        while !RELEASED.load(std::sync::atomic::Ordering::SeqCst) {
            std::thread::sleep(Duration::from_millis(1));
        }
        false
    }
    let session = Session::new(options_judged(held, 3));
    session.generate_candidates(&spec(&scratch.0, 21), 1).expect("starts");
    settled(&session, 120);
    session.promote(0).expect("founding starts");
    let deadline = Instant::now() + Duration::from_secs(60);
    while session.founding().phase != "placing" {
        assert!(Instant::now() < deadline, "the founding never reached its verdict: {:?}", session.founding());
        std::thread::sleep(Duration::from_millis(5));
    }
    assert!(session.stopper().shutdown(), "it had threads to stop, and did not wait for the held founding");
    RELEASED.store(true, std::sync::atomic::Ordering::SeqCst);
    no_workers(&session);
    assert_eq!(session.founding().attempt, 1, "the cancelled founding made no further try");
}

/// **The water slider's mechanism**: a spec's `water_level_m` is the terrarium's water
/// table in the world it makes, the same seeds bring the same terrains back at another
/// level, and a landform with no water table refuses it.
#[test]
fn a_water_level_is_the_terrariums_and_seeds_bring_terrains_back() {
    let scratch = Scratch::new("water");
    let path = scratch.0.join("terrarium.toml");
    std::fs::write(
        &path,
        "threads = 2\n[world]\nwidth = 64\nheight = 32\ndepth = 12\nvoxel_m = 0.125\n\
         [world.landform]\npreset = \"terrarium\"\nwater_level_m = 0.375\n\
         [world.landform.water]\nmin_lake_m2 = 0.0\nmin_tier_pools = 0\n",
    )
    .unwrap();
    let at = |level: Option<f64>| WorldSpec { config: Some(path.clone()), water_level_m: level, empty: true, ..WorldSpec::default() };
    let level_of = |spec: &WorldSpec| match spec.sim_params().unwrap().world.landform {
        cubarium_voxel::Landform::Terrarium(t) => t.water_level_m,
        _ => panic!("a terrarium"),
    };
    assert_eq!(level_of(&at(None)), 0.375, "the file's own");
    for (_, m) in crate::params::WATER_LEVELS {
        assert_eq!(level_of(&at(Some(m))), m);
    }
    let info = Session::new(options()).query("spec", &serde_json::to_value(at(None)).unwrap()).unwrap();
    assert_eq!(info["landform"], "terrarium");
    assert_eq!(info["water_level_m"], 0.375);
    let staged = WorldSpec { water_level_m: Some(0.75), ..spec(&scratch.0, 1) };
    assert!(staged.sim_params().is_err(), "a staged ring has no water table");

    let session = Session::new(options());
    session.generate_candidates(&at(Some(0.375)), 2).expect("starts");
    let low = settled(&session, 120);
    let seeds: Vec<Option<u64>> = low.iter().map(|c| c.seed).collect();
    assert!(seeds.iter().all(Option::is_some), "{low:?}");
    session.generate_candidates_seeded(&at(Some(1.125)), &seeds).expect("starts");
    let high = settled(&session, 120);
    assert_eq!(high.iter().map(|c| c.seed).collect::<Vec<_>>(), seeds, "the same terrains back");
    no_workers(&session);
}

/// The new-world picker's load, in one process: candidates drawn side by side on the
/// shared water pool (worlds of 512 columns and more step water on it), each with seeds
/// drawn rather than asked and more frondgrazer founders than a small world can place, so
/// the feasibility check refuses every draw and each candidate fails after its draws (the
/// path the app was on when it died on 2026-10-08); and a second batch of a different size
/// started while the first is still drawing. Nothing crashes, and every candidate of the
/// batch that counts settles with the refusal said plainly.
#[test]
fn candidates_of_two_sizes_drawn_side_by_side_through_refusals() {
    let scratch = Scratch::new("candidates-drawn");
    let session = Session::new(SessionOptions { terrain_tries: 4, ..options() });
    let drawn = |size: [u32; 3]| WorldSpec {
        config: Some(tiny_config(&scratch.0)),
        size: Some(size),
        seed: None,
        sets: vec!["fauna.founders.frondgrazer.founding_count=100000".into()],
        ..WorldSpec::default()
    };
    session.generate_candidates(&drawn([64, 24, 8]), 4).expect("starts");
    session.generate_candidates(&drawn([96, 20, 6]), 4).expect("starts");
    let infos = settled(&session, 240);
    assert_eq!(infos.len(), 4);
    for c in &infos {
        assert_eq!(c.state, "failed", "{c:?}");
        assert!(c.error.as_deref().is_some_and(|e| e.contains("can't be placed")), "{c:?}");
    }
    no_workers(&session);
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
    let before = take_current(&session);
    session.load_blocking(&save, Duration::from_secs(30)).expect("loads");
    let v = take_current(&session);
    let [cx, cy, cz] = v.chunk_dims;
    assert!(v.full);
    assert_eq!(v.chunks.len(), (cx * cy * cz) as usize, "every chunk");
    assert!(v.chunks.iter().all(|c| c.bytes.len() == 3 * CHUNK_CELLS));
    assert!(v.frame.is_some(), "and the organisms");
    assert_eq!(
        v.chunks.iter().map(|c| (&c.at, &c.marks, &c.vines)).collect::<Vec<_>>(),
        before.chunks.iter().map(|c| (&c.at, &c.marks, &c.vines)).collect::<Vec<_>>(),
        "full surface replacement lists travel with the reset"
    );
    assert_eq!(v.frame.as_ref().unwrap().carcasses, before.frame.as_ref().unwrap().carcasses);
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

/// Every preset the `presets` query lists is one a spec can name: a world spec built on
/// each landform name and each climate name parses, and a name it does not list does not.
#[test]
fn every_listed_preset_is_one_a_spec_can_name() {
    let scratch = Scratch::new("presets");
    let session = Session::new(options());
    let table = session.query("presets", &Value::Null).expect("the presets query answers");
    let names = |key: &str| -> Vec<String> {
        table[key]
            .as_array()
            .unwrap_or_else(|| panic!("`{key}` is a list"))
            .iter()
            .map(|p| p["name"].as_str().expect("each preset has a name").to_string())
            .collect()
    };
    let (terrain, climate) = (names("terrain"), names("climate"));
    assert!(!terrain.is_empty() && !climate.is_empty());
    let base = tiny_config(&scratch.0);
    let with = |terrain: Option<&str>, climate: Option<&str>| WorldSpec {
        config: Some(base.clone()),
        terrain: terrain.map(String::from),
        climate: climate.map(String::from),
        ..WorldSpec::default()
    };
    for t in &terrain {
        with(Some(t), None).sim_params().unwrap_or_else(|e| panic!("terrain {t}: {e:#}"));
    }
    for c in &climate {
        with(None, Some(c)).sim_params().unwrap_or_else(|e| panic!("climate {c}: {e:#}"));
    }
    let unlisted = "not-a-listed-preset";
    assert!(!terrain.iter().any(|t| t == unlisted));
    assert!(with(Some(unlisted), None).sim_params().is_err(), "an unlisted landform is refused");
    assert!(with(None, Some(unlisted)).sim_params().is_err(), "an unlisted climate is refused");
}

#[test]
fn a_hand_built_surface_and_carcass_survive_a_full_take_after_load() {
    use cubarium_voxel_flora::{Carcass, DepositKind};
    let scratch = Scratch::new("surfaces-load");
    let mut world = crate::feed_tests::world();
    let mut flora = crate::feed_tests::flora();
    crate::feed_tests::deposit(&mut flora, 20, DepositKind::DeadWood, 1.0);
    crate::feed_tests::deposit(&mut flora, 3, DepositKind::Carrion, 1.0);
    assert!(flora.lay_carcass(cubarium_voxel_flora::Site { x: 3, y: 1, z: 1 },
        Carcass { lineage: cubarium_voxel_fauna::Founder::Browser.index() as u8, heading_rad: 0.0, organic: 1.0 }));
    crate::feed_tests::vine(&mut world, &mut flora);
    let session = Session::new(options());
    session.install(crate::Prepared { world, flora, fauna: cubarium_voxel_fauna::Fauna::new(Default::default()),
        exec: options().exec, label: "fixture".into() });
    wait(&session, "installed", |s| s.running);
    pause(&session);
    let before = take_current(&session);
    assert!(before.chunks.iter().any(|c| !c.marks.is_empty()));
    assert!(before.chunks.iter().any(|c| !c.vines.is_empty()));
    assert!(!before.frame.as_ref().unwrap().carcasses.ids.is_empty());
    let save = scratch.0.join("slot");
    session.save_blocking(&save, Duration::from_secs(30)).unwrap();
    session.load_blocking(&save, Duration::from_secs(30)).unwrap();
    let after = take_current(&session);
    assert!(after.full);
    assert!(after.generation > before.generation);
    assert_eq!(after.chunks.len(), before.chunks.len());
    for (a, b) in after.chunks.iter().zip(&before.chunks) {
        assert_eq!((&a.at, &a.marks, &a.vines), (&b.at, &b.marks, &b.vines));
    }
    assert_eq!(after.frame.as_ref().unwrap().carcasses, before.frame.as_ref().unwrap().carcasses);
}

/// Ticks between vitals rows in [`vitals_rows_cover_every_column_and_live_lineage`]: a
/// "minute" short enough to pass two boundaries in a few dozen ticks.
const VITALS_EVERY: u64 = 20;

#[test]
fn vitals_rows_cover_every_column_and_live_lineage() {
    let scratch = Scratch::new("vitals");
    let s = spec(&scratch.0, 5);
    let prepared = prepare_new(&s.sim_params().unwrap(), &s).expect("a tiny world founds");
    let cadence = cubarium_observatory::Cadence { paused_wall_s: 0.05, ..Default::default() };
    let session = Session::new(SessionOptions { vitals_every: Some(VITALS_EVERY), cadence, speed: 32.0, ..options() });
    session.install(prepared);
    let start = wait(&session, "the world is installed", |st| st.running && st.generation == 1).tick;
    // Three row intervals at speed, then paused on a boundary (steps run at the clock's
    // own pace, so only the few to the boundary are stepped).
    wait(&session, "three row intervals", |st| st.tick >= start + 3 * VITALS_EVERY);
    let at = pause(&session);
    std::thread::sleep(Duration::from_millis(50));
    let at = session.status().tick.max(at);
    let to_boundary = (VITALS_EVERY - at % VITALS_EVERY) % VITALS_EVERY;
    session.command(Command::Step(to_boundary as u32));
    let tick = wait(&session, "the steps to a boundary", |st| st.tick == at + to_boundary).tick;
    assert_eq!(tick % VITALS_EVERY, 0);

    let v = session.query("vitals", &json!({})).unwrap();
    let names = |key: &str| -> Vec<String> {
        v[key].as_array().unwrap().iter().map(|c| c["name"].as_str().unwrap().to_string()).collect()
    };
    let animal_columns = names("animal_columns");
    let plant_columns = names("plant_columns");
    assert_eq!(animal_columns.join(","), cubarium_observatory::vitals::VITALS_HEADER);
    assert_eq!(plant_columns.join(","), cubarium_observatory::vitals::PLANTS_HEADER);
    assert!(v["animal_columns"].as_array().unwrap().iter().all(|c| c["doc"].as_str().is_some_and(|d| !d.is_empty())));
    let animals = v["animals"].as_array().unwrap();
    let plants = v["plants"].as_array().unwrap();
    assert!(animals.iter().all(|r| r.as_array().unwrap().len() == animal_columns.len()), "every animal row carries every column");
    assert!(plants.iter().all(|r| r.as_array().unwrap().len() == plant_columns.len()), "every plant row carries every column");
    let minute_of = |r: &Value| r[0].as_f64().unwrap();
    let mut minutes: Vec<f64> = animals.iter().map(minute_of).collect();
    minutes.dedup();
    assert!(minutes.len() >= 3, "the start row and two boundaries: {minutes:?}");
    assert_eq!(v["started_minute"].as_f64(), Some(minutes[0]));
    assert_eq!(v["from_minute"].as_f64(), Some(minutes[0]));

    // One row per lineage each minute, and every lineage alive at the last one has its row.
    let lineage = animal_columns.iter().position(|c| c == "lineage").unwrap();
    let n = animal_columns.iter().position(|c| c == "n").unwrap();
    for m in &minutes {
        let mut at: Vec<&str> = animals.iter().filter(|r| minute_of(r) == *m).map(|r| r[lineage].as_str().unwrap()).collect();
        let len = at.len();
        at.sort_unstable();
        at.dedup();
        assert_eq!(at.len(), len, "one row per lineage at minute {m}");
    }
    let last = *minutes.last().unwrap();
    assert_eq!(last, tick as f64 / crate::vitals::VitalsRun::MINUTE as f64, "the newest row is the boundary the world stands on");
    let deadline = Instant::now() + Duration::from_secs(10);
    let live = loop {
        let snap = session.query("snapshot", &json!({})).unwrap();
        if snap["summary"]["tick"].as_u64() == Some(tick) {
            let mut l: Vec<String> = snap["animals"].as_array().unwrap().iter().map(|a| a["species"].as_str().unwrap().to_string()).collect();
            l.sort_unstable();
            l.dedup();
            break l;
        }
        assert!(Instant::now() < deadline, "no publication of the paused tick");
        std::thread::sleep(Duration::from_millis(10));
    };
    assert!(!live.is_empty(), "the tiny world has animals");
    let mut rows: Vec<String> = animals
        .iter()
        .filter(|r| minute_of(r) == last && r[n].as_u64().unwrap_or(0) > 0)
        .map(|r| r[lineage].as_str().unwrap().to_string())
        .collect();
    rows.sort_unstable();
    assert_eq!(rows, live, "the lineages with bodies at the boundary are the live ones");
    let plant_species: Vec<&str> = plants.iter().filter(|r| minute_of(r) == last).map(|r| r[1].as_str().unwrap()).collect();
    assert_eq!(plant_species.len(), cubarium_voxel_flora::Species::ALL.len(), "one plant row per species");

    // `since` filters: only minutes after it.
    let since = session.query("vitals", &json!({ "since": minutes[1] })).unwrap();
    let after: Vec<f64> = since["animals"].as_array().unwrap().iter().map(minute_of).collect();
    assert!(!after.is_empty() && after.iter().all(|m| *m > minutes[1]), "since {} gave {after:?}", minutes[1]);
    assert!(since["plants"].as_array().unwrap().iter().all(|r| minute_of(r) > minutes[1]));
    assert!(session.status().vitals_ms > 0.0, "the recorder's cost is reported");
    let refused = session.query("no-such-query", &json!({})).unwrap_err();
    assert!(refused.contains("vitals"), "the unknown-query error lists vitals: {refused}");
}
