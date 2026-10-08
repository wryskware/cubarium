//! Wall time of terrain-first picking through a [`Session`], as the app does it: draw
//! `count` terrain candidates in parallel, then found the first ready one, printing each
//! candidate's time, every founding stage's start, the total, and the process's resident
//! memory before and after. Development measurement only.
//!
//! `terrain_first_timing CONFIG [count=4] [seed=N] [threads=0] [tries=3]`

use std::time::{Duration, Instant};

use cubarium_session::{Session, SessionOptions, SimParams, WorldSpec};

fn rss_mb() -> f64 {
    std::fs::read_to_string("/proc/self/status")
        .ok()
        .and_then(|s| {
            s.lines()
                .find(|l| l.starts_with("VmRSS:"))
                .and_then(|l| l.split_whitespace().nth(1).and_then(|k| k.parse::<f64>().ok()))
        })
        .map_or(0.0, |kb| kb / 1024.0)
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let config = args.first().expect("terrain_first_timing CONFIG [count=] [seed=] [threads=] [tries=]").clone();
    let arg = |name: &str| args.iter().find_map(|a| a.strip_prefix(&format!("{name}=")).map(str::to_string));
    let count: usize = arg("count").and_then(|v| v.parse().ok()).unwrap_or(4);
    let seed: Option<u64> = arg("seed").and_then(|v| v.parse().ok());
    let threads: usize = arg("threads").and_then(|v| v.parse().ok()).unwrap_or(0);
    let tries: usize = arg("tries").and_then(|v| v.parse().ok()).unwrap_or(3);
    let session = Session::new(SessionOptions {
        exec: SimParams { threads, ..SimParams::default() },
        feed: false,
        placement_tries: tries,
        ..SessionOptions::default()
    });
    let spec = WorldSpec { config: Some(config.into()), seed, ..WorldSpec::default() };
    println!("rss at start {:.0} MB", rss_mb());
    let t0 = Instant::now();
    session.generate_candidates(&spec, count).expect("the spec parses");
    let mut done = vec![None; count];
    loop {
        let infos = session.candidates();
        for c in &infos {
            if c.state != "generating" && done[c.index].is_none() {
                done[c.index] = Some(t0.elapsed().as_secs_f64());
                println!(
                    "candidate {} {} at {:.2} s: seed {:?}, feasible {:?}, terrain {:?}{}{}",
                    c.index,
                    c.state,
                    t0.elapsed().as_secs_f64(),
                    c.seed,
                    c.feasible,
                    c.terrain,
                    c.error.as_deref().map(|e| format!(", error: {e}")).unwrap_or_default(),
                    if c.notes.is_empty() { String::new() } else { format!(", notes: {:?}", c.notes) },
                );
            }
        }
        if done.iter().all(Option::is_some) {
            break;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    let generated = t0.elapsed().as_secs_f64();
    println!("{count} candidates in {generated:.2} s; rss {:.0} MB", rss_mb());
    let Some(pick) = session.candidates().iter().find(|c| c.state == "ready").map(|c| c.index) else {
        println!("no candidate ready");
        return;
    };
    let terrain_before = session.candidates()[pick].terrain.clone();
    let t1 = Instant::now();
    session.promote(pick).expect("founding starts");
    let mut phase = String::new();
    let mut attempt = 0;
    let info = loop {
        let f = session.founding();
        if f.attempt != attempt || f.phase != phase {
            println!(
                "  {:7.2} s  try {}/{}  {}  (progress {:.2})",
                t1.elapsed().as_secs_f64(),
                f.attempt,
                f.attempts,
                f.phase,
                f.progress
            );
            attempt = f.attempt;
            phase = f.phase.clone();
        }
        if f.state != "founding" {
            break f;
        }
        std::thread::sleep(Duration::from_millis(20));
    };
    let founded = t1.elapsed().as_secs_f64();
    println!(
        "founding {}: {founded:.2} s, label {:?}, error {:?}; terrain {:?} -> {:?}",
        info.state, info.label, info.error, terrain_before, info.terrain
    );
    let deadline = Instant::now() + Duration::from_secs(30);
    while !session.status().running && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(20));
    }
    println!("running {}; rss {:.0} MB; total {:.2} s", session.status().running, rss_mb(), t0.elapsed().as_secs_f64());
    drop(session);
    println!("rss after the session drops {:.0} MB", rss_mb());
}
