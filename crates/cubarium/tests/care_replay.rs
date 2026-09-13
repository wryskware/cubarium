//! Care recovery: what a crash at each point in the admission protocol leaves behind, and
//! whether the world that comes back is the one that would have existed uninterrupted.
//!
//! The comparisons are all on `ecology_hash`, which is FNV-1a over the schema 7 projection
//! of the state — the ecology alone, with the care ledgers excluded. That is the right
//! measure here: two worlds that lived the same life must match on it even though their
//! care bookkeeping differs.
//!
//! Most of these tests build the journal by hand rather than racing an HTTP request into a
//! particular tick. That is deliberate: a recovery test whose scheduled boundary depends on
//! how fast a socket was that day proves nothing repeatable. The one test that does use the
//! live path — the delayed acknowledgement — is about timing, so it has to.

mod support;

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use cubarium_core::{World, decode_snapshot, ecology_hash};

use support::{Scratch, parse, run, snapshot_ticks};

/// Checkpoints every 5 simulated seconds, so a short run leaves plenty to resume from.
const FAST_CHECKPOINTS: &str = "[capacity]\ncheckpoint_seconds = 5.0\n";

/// Every test in this file runs one at a time, because the journal hooks are installed
/// process-wide and `Journal::open` consults them wherever it is called from — including
/// inside `run_world`. A test that merely replays a journal would otherwise inherit another
/// test's injected delay or failure, and an unlimited-speed run that picked up an uncertain
/// write would hold forever with no stop signal to end it.
///
/// Taking the guard also clears any hook a previously *failing* test left behind.
fn hook_guard() -> std::sync::MutexGuard<'static, ()> {
    static LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
    let guard = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    cubarium::care::JournalHooks::uninstall();
    guard
}

/// One `accepted` journal line, exactly as the host writes it.
fn accepted_line(seq: u64, boundary: u64, kind: &str, face: u8, u: u8, v: u8) -> String {
    format!(
        r#"{{"rec":"accepted","seq":{seq},"apply_after_tick":{boundary},"client":"test.1","request":{seq},"kind":"{kind}","target":{{"face":{face},"u":{u},"v":{v}}}}}"#
    )
}

/// Write a journal holding an epoch record and the given accepted lines.
fn write_journal(dir: &Path, lines: &[String]) {
    let mut text = String::from(r#"{"rec":"epoch","epoch":"test-epoch","build":"test"}"#);
    text.push('\n');
    for line in lines {
        text.push_str(line);
        text.push('\n');
    }
    std::fs::write(dir.join("care.jsonl"), text).unwrap();
}

/// Seed a world and return its state directory and the tick of its newest snapshot.
fn seed(scratch: &Scratch, name: &str, seconds: &str) -> (PathBuf, u64) {
    let state = scratch.join(name);
    let config = scratch.write(&format!("{name}.toml"), FAST_CHECKPOINTS);
    let out = run(&[
        "--sink", "none", "--speed", "0", "--seconds", seconds, "--fresh", "--seed", "4242",
        "--config", config.to_str().unwrap(),
        "--state", state.to_str().unwrap(),
    ]);
    (state, out.final_tick)
}

/// Copy every snapshot and the journal out of `from` into a fresh directory, leaving the
/// original untouched. A recovery test needs several independent runs from one state.
fn fork_state(scratch: &Scratch, from: &Path, name: &str) -> PathBuf {
    let to = scratch.join(name);
    std::fs::create_dir_all(&to).unwrap();
    for entry in std::fs::read_dir(from).unwrap().flatten() {
        let file = entry.file_name();
        let n = file.to_string_lossy();
        if n == ".lock" {
            continue;
        }
        std::fs::copy(entry.path(), to.join(&file)).unwrap();
    }
    to
}

/// Keep only the snapshot at `tick`, so a run has to resume from that exact one.
fn keep_only_snapshot(dir: &Path, tick: u64) {
    for (t, path) in cubarium::state::list_snapshots(dir) {
        if t != tick {
            std::fs::remove_file(path).unwrap();
        }
    }
}

/// The ecology hash of the newest snapshot in a directory, with its tick.
fn newest_ecology(dir: &Path) -> (u64, u64) {
    let (tick, path) = cubarium::state::list_snapshots(dir).into_iter().next().expect("a snapshot");
    let bytes = std::fs::read(&path).unwrap();
    let (_meta, state) = decode_snapshot(&bytes).expect("the snapshot decodes");
    (tick, ecology_hash(&state))
}

/// The world that *would* have existed: load `from`, step to each command's boundary,
/// apply it through the core exactly as the runner does, and step on to `final_tick`.
fn reference_ecology(
    snapshot: &Path,
    commands: &[(u64, u64, cubarium_core::care::CareKind, u8, u8, u8)],
    final_tick: u64,
) -> u64 {
    use cubarium_core::care::{CareCommand, CareTarget};
    let bytes = std::fs::read(snapshot).unwrap();
    let (_meta, state) = decode_snapshot(&bytes).unwrap();
    let mut world = World::from_state(state).unwrap();
    for (seq, boundary, kind, face, u, v) in commands {
        while world.tick() < *boundary {
            world.step();
            world.drain_events();
        }
        let receipt = world.apply_care(&CareCommand {
            seq: *seq,
            apply_after_tick: *boundary,
            kind: *kind,
            target: CareTarget { face: *face, u: f64::from(*u), v: f64::from(*v) },
        });
        assert_ne!(
            receipt.outcome.reason(),
            Some("out of order"),
            "the reference world refused seq {seq}"
        );
        assert_ne!(receipt.outcome.reason(), Some("wrong boundary"));
    }
    while world.tick() < final_tick {
        world.step();
        world.drain_events();
    }
    assert_eq!(world.tick(), final_tick);
    ecology_hash(&world.state)
}

/// The contract's step 6, and root's first runner correction in one test: a durable
/// accepted record is replayed at its own boundary, the resulting ecology is exactly the
/// one an uninterrupted world would have had, and none of it depends on `--care`.
///
/// This is also the "crash after `fsync`, before application" crash point: the journal
/// holds the record, the snapshot does not, and nothing was ever applied in the original
/// process.
#[test]
fn a_durable_command_replays_at_its_boundary_without_care_and_matches_an_uninterrupted_world() {
    let _serial = hook_guard();
    let scratch = Scratch::new("replay-at-b");
    let (seeded, seeded_tick) = seed(&scratch, "seed", "20");
    assert_eq!(seeded_tick, 400);
    keep_only_snapshot(&seeded, 400);

    // A feed and a clean, accepted at two different boundaries after the snapshot.
    let commands = [
        (1u64, 420u64, cubarium_core::care::CareKind::Feed, 0u8, 32u8, 32u8),
        (2, 480, cubarium_core::care::CareKind::Clean, 0, 32, 32),
    ];
    let lines: Vec<String> = commands
        .iter()
        .map(|(seq, b, kind, f, u, v)| accepted_line(*seq, *b, kind.as_str(), *f, *u, *v))
        .collect();

    // Recovery without `--care`. Disabling new input must not disable recovery of input
    // already accepted: skipping it here would advance past B and checkpoint a history the
    // journal disagrees with, and enabling care later would refuse the command as older
    // than the snapshot — after the ecology had already diverged.
    let plain = fork_state(&scratch, &seeded, "plain");
    write_journal(&plain, &lines);
    let config = scratch.write("fast.toml", FAST_CHECKPOINTS);
    let out = run(&[
        "--sink", "none", "--speed", "0", "--seconds", "10",
        "--config", config.to_str().unwrap(),
        "--state", plain.to_str().unwrap(),
    ]);
    assert_eq!(out.loaded_tick, Some(400));
    assert_eq!(out.final_tick, 600);

    let (tick, replayed) = newest_ecology(&plain);
    assert_eq!(tick, 600);
    let reference =
        reference_ecology(&seeded.join("world-400.cubw"), &commands, 600);
    assert_eq!(
        replayed, reference,
        "a replayed world must be ecologically identical to one that was never interrupted"
    );

    // And the same recovery with `--care` enabled reaches the same ecology.
    let cared = fork_state(&scratch, &seeded, "cared");
    write_journal(&cared, &lines);
    let out = run(&[
        "--sink", "png", "--care", "--mirror-web", "--web-port", "0", "--speed", "20",
        "--seconds", "10", "--every", "1000",
        "--out", scratch.join("captures-unused").to_str().unwrap(),
        "--config", config.to_str().unwrap(),
        "--state", cared.to_str().unwrap(),
    ]);
    assert_eq!(out.final_tick, 600);
    let (_, with_care) = newest_ecology(&cared);
    assert_eq!(with_care, reference, "--care must not change what recovery produces");
}

/// The "crash after application, before any outcome record or checkpoint" point. The world
/// applied the command and then died before writing a snapshot, so recovery starts from an
/// *older* snapshot and has to arrive at the same place.
#[test]
fn replaying_from_an_older_snapshot_reaches_the_same_ecology() {
    let _serial = hook_guard();
    let scratch = Scratch::new("replay-older");
    let (seeded, _) = seed(&scratch, "seed", "30");
    let ticks = snapshot_ticks(&seeded);
    assert!(ticks.contains(&600) && ticks.contains(&400), "{ticks:?}");

    let commands = [(1u64, 420u64, cubarium_core::care::CareKind::Feed, 2u8, 10u8, 50u8)];
    let lines = vec![accepted_line(1, 420, "feed", 2, 10, 50)];
    let config = scratch.write("fast.toml", FAST_CHECKPOINTS);

    // Recovery from the newest snapshot before the boundary...
    let near = fork_state(&scratch, &seeded, "near");
    keep_only_snapshot(&near, 400);
    write_journal(&near, &lines);
    run(&[
        "--sink", "none", "--speed", "0", "--seconds", "10",
        "--config", config.to_str().unwrap(),
        "--state", near.to_str().unwrap(),
    ]);
    let (near_tick, near_hash) = newest_ecology(&near);

    // ...and recovery from a much older one, which has to re-run 200 more ticks first.
    let far = fork_state(&scratch, &seeded, "far");
    keep_only_snapshot(&far, 200);
    write_journal(&far, &lines);
    run(&[
        "--sink", "none", "--speed", "0", "--seconds", "20",
        "--config", config.to_str().unwrap(),
        "--state", far.to_str().unwrap(),
    ]);
    let (far_tick, far_hash) = newest_ecology(&far);

    assert_eq!(near_tick, 600);
    assert_eq!(far_tick, 600);
    assert_eq!(
        near_hash, far_hash,
        "recovery from an older snapshot must reach the same ecology at the same tick"
    );
    assert_eq!(near_hash, reference_ecology(&seeded.join("world-200.cubw"), &commands, 600));
}

/// A shower is 120 ticks long, so a snapshot in the middle of one has to carry its
/// progress. Stop 40 ticks in, resume, and the water that lands afterwards must be the
/// water an uninterrupted shower would have delivered.
#[test]
fn a_shower_interrupted_by_a_snapshot_resumes_where_it_left_off() {
    let _serial = hook_guard();
    let scratch = Scratch::new("mid-shower");
    let (seeded, _) = seed(&scratch, "seed", "20");
    keep_only_snapshot(&seeded, 400);
    let commands = [(1u64, 420u64, cubarium_core::care::CareKind::Rain, 1u8, 20u8, 20u8)];
    let lines = vec![accepted_line(1, 420, "rain", 1, 20, 20)];
    let config = scratch.write("fast.toml", FAST_CHECKPOINTS);

    // Stop at tick 460: the shower started at 420 and has 80 of its 120 samples left.
    let split = fork_state(&scratch, &seeded, "split");
    write_journal(&split, &lines);
    let first = run(&[
        "--sink", "none", "--speed", "0", "--seconds", "3",
        "--config", config.to_str().unwrap(),
        "--state", split.to_str().unwrap(),
    ]);
    assert_eq!(first.final_tick, 460, "the snapshot lands inside the shower");
    // The journal is not replayed again on the resume: the snapshot's `admitted_seq` has
    // already consumed seq 1, and the shower itself is in the snapshot.
    let second = run(&[
        "--sink", "none", "--speed", "0", "--seconds", "7",
        "--config", config.to_str().unwrap(),
        "--state", split.to_str().unwrap(),
    ]);
    assert_eq!(second.loaded_tick, Some(460));
    assert_eq!(second.final_tick, 600);
    let (tick, split_hash) = newest_ecology(&split);
    assert_eq!(tick, 600);

    assert_eq!(
        split_hash,
        reference_ecology(&seeded.join("world-400.cubw"), &commands, 600),
        "a shower resumed from a mid-shower snapshot must deliver the same water"
    );
}

/// An unadmitted command for a boundary the snapshot has already passed is an inconsistent
/// recovery state. It must refuse to start rather than apply the command late, and it must
/// refuse before writing anything.
#[test]
fn a_boundary_already_in_the_past_refuses_to_start() {
    let _serial = hook_guard();
    let scratch = Scratch::new("past-boundary");
    let (seeded, _) = seed(&scratch, "seed", "20");
    let state = fork_state(&scratch, &seeded, "state");
    keep_only_snapshot(&state, 400);
    write_journal(&state, &[accepted_line(1, 380, "feed", 0, 1, 1)]);
    let before = std::fs::read_to_string(state.join("care.jsonl")).unwrap();

    let args = parse(&[
        "--sink", "none", "--speed", "0", "--seconds", "1",
        "--state", state.to_str().unwrap(),
    ]);
    let err = format!("{:#}", cubarium::run_world(&args).unwrap_err());
    assert!(err.contains("inconsistent recovery state"), "{err}");
    assert!(err.contains("already at tick 400"), "{err}");
    assert_eq!(snapshot_ticks(&state), vec![400], "nothing newer was written");
    // The epoch record for the attempt is written (every start writes one); what must not
    // change is the history — no command was admitted, aborted, or given an outcome.
    let after = std::fs::read_to_string(state.join("care.jsonl")).unwrap();
    assert!(after.starts_with(&before), "the existing history must be preserved byte for byte");
    assert_eq!(after.matches(r#""rec":"accepted""#).count(), 1, "{after}");
    assert_eq!(after.matches(r#""rec":"outcome""#).count(), 0, "{after}");
}

/// A hole in the sequence — the second record of a batch never reached the disk — is a
/// recovery error, not something to skip past.
#[test]
fn a_hole_in_the_recovered_schedule_refuses_to_start() {
    let _serial = hook_guard();
    let scratch = Scratch::new("schedule-hole");
    let (seeded, _) = seed(&scratch, "seed", "20");
    let state = fork_state(&scratch, &seeded, "state");
    keep_only_snapshot(&state, 400);
    write_journal(
        &state,
        &[accepted_line(1, 420, "feed", 0, 1, 1), accepted_line(3, 440, "clean", 0, 1, 1)],
    );
    let args = parse(&[
        "--sink", "none", "--speed", "0", "--seconds", "1",
        "--state", state.to_str().unwrap(),
    ]);
    let err = format!("{:#}", cubarium::run_world(&args).unwrap_err());
    assert!(err.contains("not contiguous"), "{err}");
    assert!(err.contains("expected seq 2"), "{err}");
}

// --- the live path -------------------------------------------------------------------

/// A running `--care` host, driven through the library on its own thread.
struct LiveRun {
    stop: std::sync::Arc<AtomicBool>,
    handle: std::thread::JoinHandle<anyhow::Result<cubarium::RunOutcome>>,
    port: u16,
}

impl LiveRun {
    fn start(state: &Path, config: &Path, seconds: &str) -> LiveRun {
        // Port 0 binds an ephemeral port; the test finds it by polling `/care/status`
        // across the whole loopback range would be absurd, so the run is told a port the
        // test chose by binding and releasing one first.
        let probe = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = probe.local_addr().unwrap().port();
        drop(probe);
        let args = parse(&[
            "--sink", "png", "--care", "--mirror-web",
            "--web-port", &port.to_string(), "--speed", "20", "--seconds", seconds,
            "--every", "100000",
            "--out", state.parent().unwrap().join("captures").to_str().unwrap(),
            "--config", config.to_str().unwrap(),
            "--state", state.to_str().unwrap(),
        ]);
        let stop = std::sync::Arc::new(AtomicBool::new(false));
        let flag = std::sync::Arc::clone(&stop);
        let handle =
            std::thread::spawn(move || cubarium::run_world_until(&args, &flag));
        LiveRun { stop, handle, port }
    }

    fn addr(&self) -> String {
        format!("127.0.0.1:{}", self.port)
    }

    /// One care request over a real socket, with every header the contract requires.
    fn post(&self, path: &str, body: &str) -> (u16, String) {
        use std::io::{Read, Write};
        let mut s = std::net::TcpStream::connect(self.addr()).expect("connecting to the viewer");
        s.set_read_timeout(Some(Duration::from_secs(20))).unwrap();
        write!(
            s,
            "POST {path} HTTP/1.1\r\nHost: 127.0.0.1:{}\r\nOrigin: http://127.0.0.1:{}\r\n\
             Content-Type: application/json\r\nX-Cubarium-Care: 1\r\n\
             Content-Length: {}\r\nConnection: close\r\n\r\n{body}",
            self.port,
            self.port,
            body.len()
        )
        .unwrap();
        s.flush().unwrap();
        let mut raw = Vec::new();
        s.read_to_end(&mut raw).unwrap();
        let text = String::from_utf8_lossy(&raw).into_owned();
        let code = text
            .split_whitespace()
            .nth(1)
            .and_then(|c| c.parse().ok())
            .unwrap_or_else(|| panic!("no status line in {text}"));
        let body = text.split("\r\n\r\n").nth(1).unwrap_or_default().to_string();
        (code, body)
    }

    fn get(&self, path: &str) -> serde_json::Value {
        use std::io::{Read, Write};
        let mut s = std::net::TcpStream::connect(self.addr()).expect("connecting to the viewer");
        s.set_read_timeout(Some(Duration::from_secs(20))).unwrap();
        write!(s, "GET {path} HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n").unwrap();
        s.flush().unwrap();
        let mut raw = Vec::new();
        s.read_to_end(&mut raw).unwrap();
        let text = String::from_utf8_lossy(&raw).into_owned();
        let body = text.split("\r\n\r\n").nth(1).unwrap_or_default().to_string();
        serde_json::from_str(&body).unwrap_or_else(|e| panic!("{path}: {e}\n{body}"))
    }

    /// Wait until the viewer answers, so a test never races the listener.
    fn wait_ready(&self) {
        let deadline = Instant::now() + Duration::from_secs(20);
        while Instant::now() < deadline {
            if std::net::TcpStream::connect_timeout(
                &self.addr().parse().unwrap(),
                Duration::from_millis(200),
            )
            .is_ok()
            {
                return;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        panic!("the viewer never started listening on {}", self.addr());
    }

    fn register(&self) -> String {
        let deadline = Instant::now() + Duration::from_secs(20);
        loop {
            let (code, body) = self.post("/care/register", "{}");
            if code == 200 {
                let v: serde_json::Value = serde_json::from_str(&body).unwrap();
                return v["client"].as_str().unwrap().to_string();
            }
            assert!(Instant::now() < deadline, "registration kept failing: {code} {body}");
            std::thread::sleep(Duration::from_millis(20));
        }
    }

    fn finish(self) -> anyhow::Result<cubarium::RunOutcome> {
        self.stop.store(true, Ordering::Relaxed);
        self.handle.join().expect("the run thread")
    }
}

/// The contract's central claim, and the evidence the review asked for: an acknowledgement
/// that takes longer than several ticks does *not* let the world walk past the boundary the
/// command was accepted for, and the command is applied exactly once when it lands.
#[test]
fn a_delayed_acknowledgement_holds_the_world_at_b_and_applies_once() {
    let _serial = hook_guard();
    let scratch = Scratch::new("delayed-ack");
    let (seeded, _) = seed(&scratch, "seed", "20");
    let state = fork_state(&scratch, &seeded, "state");
    keep_only_snapshot(&state, 400);
    let config = scratch.write("fast.toml", FAST_CHECKPOINTS);

    // Two and a half seconds of `fsync`: at `--speed 20` an unheld world would cover about
    // fifty ticks in that time.
    let hooks = cubarium::care::JournalHooks::default();
    hooks.delay(2500);
    hooks.install();

    let live = LiveRun::start(&state, &config, "60");
    live.wait_ready();
    let client = live.register();

    let before = live.get("/care/status");
    let started_at = before["world_tick"].as_u64().unwrap();

    let posted = std::thread::spawn({
        // The POST waits for the durable acknowledgement, which is the point.
        let addr = live.addr();
        let client = client.clone();
        let port = live.port;
        move || {
            use std::io::{Read, Write};
            let body = format!(
                r#"{{"client":"{client}","request":1,"kind":"feed","target":{{"face":0,"u":32,"v":32}}}}"#
            );
            let mut s = std::net::TcpStream::connect(&addr).unwrap();
            s.set_read_timeout(Some(Duration::from_secs(30))).unwrap();
            write!(
                s,
                "POST /care HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\n\
                 Content-Type: application/json\r\nX-Cubarium-Care: 1\r\n\
                 Content-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            )
            .unwrap();
            s.flush().unwrap();
            let mut raw = Vec::new();
            s.read_to_end(&mut raw).unwrap();
            String::from_utf8_lossy(&raw).into_owned()
        }
    });

    // Catch the hold in the act: `/care/status` must report `holding` at a boundary, and
    // `world_tick` must stop moving while it does.
    let deadline = Instant::now() + Duration::from_secs(20);
    let mut held_at = None;
    while Instant::now() < deadline {
        let status = live.get("/care/status");
        if status["care"] == "holding" {
            held_at = status["holding_at"].as_u64();
            break;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    let held_at = held_at.expect("the world never reported holding at a boundary");
    assert!(held_at >= started_at, "the hold is at or after the tick the test observed");

    // Sampled across the middle of the hold, the world does not move.
    let mut ticks = Vec::new();
    for _ in 0..6 {
        let status = live.get("/care/status");
        if status["care"] != "holding" {
            break;
        }
        ticks.push(status["world_tick"].as_u64().unwrap());
        std::thread::sleep(Duration::from_millis(150));
    }
    assert!(ticks.len() >= 3, "the hold was too short to sample: {ticks:?}");
    assert!(
        ticks.iter().all(|t| *t == held_at),
        "the world advanced past its held boundary {held_at}: {ticks:?}"
    );

    let answer = posted.join().unwrap();
    assert!(answer.starts_with("HTTP/1.1 202 Accepted"), "{answer}");
    assert!(
        answer.contains(&format!(r#""apply_after_tick":{held_at}"#)),
        "the receipt must name the boundary the world was held at: {answer}"
    );

    // Applied exactly once, and the world is moving again.
    let deadline = Instant::now() + Duration::from_secs(20);
    let mut receipt = None;
    while Instant::now() < deadline {
        let status = live.get("/care/status");
        let rows = status["receipts"].as_array().cloned().unwrap_or_default();
        if let Some(row) = rows.iter().find(|r| r["seq"] == 1)
            && row["state"] != "queued"
            && row["state"] != "accepted"
        {
            receipt = Some(row.clone());
            break;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    let receipt = receipt.expect("the command never produced a receipt");
    assert_eq!(receipt["state"], "applied", "{receipt}");
    assert_eq!(receipt["apply_after_tick"], held_at);
    assert!(receipt["applied"]["material_in"].as_f64().unwrap() > 0.0, "{receipt}");

    // An applied receipt is published at the held boundary, before the next
    // simulation tick is due. Observe actual resumption rather than assuming
    // the first HTTP response after that publication is already a later tick.
    let deadline = Instant::now() + Duration::from_secs(20);
    let mut after = live.get("/care/status");
    while after["world_tick"].as_u64().unwrap() <= held_at && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(20));
        after = live.get("/care/status");
    }
    assert_eq!(after["care"], "ready", "the hold must end: {after}");
    assert!(after["world_tick"].as_u64().unwrap() > held_at, "the world must resume: {after}");

    cubarium::care::JournalHooks::uninstall();
    let outcome = live.finish().expect("the run must stop cleanly");
    assert!(outcome.final_tick > held_at);

    // And the journal holds exactly one accepted record for seq 1: applied once, not twice.
    let journal = std::fs::read_to_string(state.join("care.jsonl")).unwrap();
    let accepted = journal.lines().filter(|l| l.contains(r#""rec":"accepted""#)).count();
    assert_eq!(accepted, 1, "seq 1 was journaled more than once:\n{journal}");
}

/// An uncertain write — the record is durable, the writer reported a failure — must hold
/// the world at that boundary, refuse further care, and never append anything again. On
/// restart the surviving record replays at its own boundary.
#[test]
fn an_uncertain_write_holds_the_world_and_the_record_replays_after_a_restart() {
    let _serial = hook_guard();
    let scratch = Scratch::new("uncertain-write");
    let (seeded, _) = seed(&scratch, "seed", "20");
    let state = fork_state(&scratch, &seeded, "state");
    keep_only_snapshot(&state, 400);
    let config = scratch.write("fast.toml", FAST_CHECKPOINTS);

    let hooks = cubarium::care::JournalHooks::default();
    hooks.fail_after_sync(1);
    hooks.install();

    let live = LiveRun::start(&state, &config, "60");
    live.wait_ready();
    let client = live.register();
    let (code, body) = live.post(
        "/care",
        &format!(
            r#"{{"client":"{client}","request":1,"kind":"feed","target":{{"face":0,"u":32,"v":32}}}}"#
        ),
    );
    assert_eq!(code, 503, "an uncertain write must not be reported as acceptance: {body}");
    assert!(body.contains("care failed"), "{body}");

    // The world is held, and stays held: nothing resumes it.
    let deadline = Instant::now() + Duration::from_secs(20);
    let mut failed_at = None;
    while Instant::now() < deadline {
        let status = live.get("/care/status");
        if status["care"] == "failed" {
            failed_at = status["holding_at"].as_u64();
            break;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    let failed_at = failed_at.expect("care never reported failing");
    std::thread::sleep(Duration::from_millis(600));
    let status = live.get("/care/status");
    assert_eq!(status["care"], "failed");
    assert_eq!(
        status["world_tick"], failed_at,
        "the world must not advance past an unresolved boundary: {status}"
    );
    // Further care is refused rather than queued behind a hold that will never end.
    let (code, _) = live.post(
        "/care",
        &format!(
            r#"{{"client":"{client}","request":2,"kind":"clean","target":{{"face":0,"u":1,"v":1}}}}"#
        ),
    );
    assert_eq!(code, 503);

    cubarium::care::JournalHooks::uninstall();
    let outcome = live.finish().expect("a clean stop must still succeed");
    assert_eq!(outcome.final_tick, failed_at, "the final snapshot is written at the held tick");

    // The record survived the failure the writer reported, and nothing was appended after
    // it — no outcome record, no second epoch.
    let journal = std::fs::read_to_string(state.join("care.jsonl")).unwrap();
    assert_eq!(journal.matches(r#""rec":"accepted""#).count(), 1, "{journal}");
    assert_eq!(journal.matches(r#""rec":"outcome""#).count(), 0, "{journal}");

    // Restart: the surviving record replays at its own boundary, which is exactly the tick
    // the held world stopped at.
    let resumed = run(&[
        "--sink", "none", "--speed", "0", "--seconds", "10",
        "--config", config.to_str().unwrap(),
        "--state", state.to_str().unwrap(),
    ]);
    assert_eq!(resumed.loaded_tick, Some(failed_at));
    let (_, recovered) = newest_ecology(&state);
    let commands =
        [(1u64, failed_at, cubarium_core::care::CareKind::Feed, 0u8, 32u8, 32u8)];
    assert_eq!(
        recovered,
        reference_ecology(&seeded.join("world-400.cubw"), &commands, resumed.final_tick),
        "the recovered world must match one that applied the command at the same boundary"
    );
}

/// The visual scenario: one short matched pair of runs, one fed and showered and cleaned,
/// one left alone, both writing captures to /tmp so a human can look at them.
#[test]
fn a_matched_care_and_no_care_scenario_writes_captures() {
    let _serial = hook_guard();
    let scratch = Scratch::new("visual");
    let (seeded, _) = seed(&scratch, "seed", "20");
    let config = scratch.write("fast.toml", FAST_CHECKPOINTS);
    // Under this test's own scratch root: a fixed shared path lets two concurrent cargo
    // processes erase each other's captures. Copied to a stable place at the end, so a
    // human still has somewhere predictable to look.
    let out_dir = scratch.join("captures");

    // Cared for: three commands at known boundaries, through the journal so the scenario is
    // repeatable rather than dependent on socket timing.
    let cared = fork_state(&scratch, &seeded, "cared");
    keep_only_snapshot(&cared, 400);
    write_journal(
        &cared,
        &[
            accepted_line(1, 402, "feed", 0, 32, 32),
            accepted_line(2, 404, "rain", 0, 32, 32),
            accepted_line(3, 406, "clean", 2, 20, 20),
        ],
    );
    let with_care = run(&[
        "--sink", "png", "--care", "--mirror-web", "--web-port", "0",
        "--speed", "20", "--seconds", "8", "--every", "30",
        "--out", out_dir.join("cared").to_str().unwrap(),
        "--config", config.to_str().unwrap(),
        "--state", cared.to_str().unwrap(),
    ]);

    // Left alone: the same world, the same ticks, no commands.
    let alone = fork_state(&scratch, &seeded, "alone");
    keep_only_snapshot(&alone, 400);
    let without_care = run(&[
        "--sink", "png", "--speed", "20", "--seconds", "8", "--every", "30",
        "--out", out_dir.join("alone").to_str().unwrap(),
        "--config", config.to_str().unwrap(),
        "--state", alone.to_str().unwrap(),
    ]);

    assert_eq!(with_care.final_tick, without_care.final_tick);
    let (_, cared_hash) = newest_ecology(&cared);
    let (_, alone_hash) = newest_ecology(&alone);
    assert_ne!(cared_hash, alone_hash, "care that changed nothing would not be care");

    let captures: Vec<PathBuf> = std::fs::read_dir(out_dir.join("cared"))
        .expect("the cared run wrote captures")
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e == "png"))
        .collect();
    assert!(!captures.is_empty(), "no PNG captures in {}", out_dir.join("cared").display());
    assert!(
        std::fs::read_dir(out_dir.join("alone")).unwrap().count() > 0,
        "the no-care run wrote no captures"
    );

    // Kept for a human to look at, named for this process so concurrent runs do not
    // overwrite each other.
    let keep = PathBuf::from(format!("/tmp/cubarium-care-captures-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&keep);
    for side in ["cared", "alone"] {
        std::fs::create_dir_all(keep.join(side)).unwrap();
        for entry in std::fs::read_dir(out_dir.join(side)).unwrap().flatten() {
            let _ = std::fs::copy(entry.path(), keep.join(side).join(entry.file_name()));
        }
    }
    eprintln!("care/no-care captures kept in {}", keep.display());
}
