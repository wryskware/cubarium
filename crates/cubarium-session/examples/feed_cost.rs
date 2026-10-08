//! **What the view feed costs the loop**, per feed update (one per clock period), on a
//! real world: with no reader taking views, and with a reader taking them at 60 Hz the way
//! Godot will. Also the tick's own cost beside it, and the loop's cost with the feed off.
//!
//! ```text
//! cargo run --release -p cubarium-session --example feed_cost -- \
//!     config=config/desktop/terrarium.toml seed=7 seconds=15 [size=640,128,96] [empty=1] [cache=DIR]
//! ```
//! `cache=DIR` keeps the founded world there and loads it on the next run.

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use cubarium_session::prepare_new;
use cubarium_session::feed::FeedConfig;
use cubarium_session::{Session, SessionOptions, WorldSpec, store};

fn arg(args: &[String], key: &str) -> Option<String> {
    args.iter().find_map(|a| a.strip_prefix(&format!("{key}=")).map(str::to_string))
}

fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let config = arg(&args, "config").map(PathBuf::from);
    let seconds: f64 = arg(&args, "seconds").map_or(15.0, |s| s.parse().unwrap());
    let size = arg(&args, "size").map(|s| {
        let v: Vec<u32> = s.split(',').map(|x| x.parse().unwrap()).collect();
        [v[0], v[1], v[2]]
    });
    let spec = WorldSpec {
        config,
        size,
        seed: arg(&args, "seed").map(|s| s.parse().unwrap()),
        empty: arg(&args, "empty").is_some_and(|v| v == "1"),
        ..WorldSpec::default()
    };
    let params = spec.sim_params()?;
    let cache = arg(&args, "cache").map(PathBuf::from).unwrap_or_else(|| std::env::temp_dir().join("feed-cost-world"));
    if store::read_meta(&cache).is_err() {
        let at = Instant::now();
        let p = prepare_new(&params, &spec)?;
        eprintln!("founded in {:.1} s", at.elapsed().as_secs_f64());
        store::save_whole(&cache, &p.world, &p.flora, &p.fauna)?;
    }
    let meta = store::read_meta(&cache)?;
    println!(
        "world {}x{}x{} ({} cells), tick {}",
        meta.size[0],
        meta.size[1],
        meta.size[2],
        meta.size.iter().map(|&x| x as u64).product::<u64>(),
        meta.tick
    );
    // `sweeps=1,4,20`: also time the feed alone at these pore sweeps.
    let sweeps: Vec<u32> = arg(&args, "sweeps").map_or(Vec::new(), |s| s.split(',').map(|x| x.parse().unwrap()).collect());
    let mut arms: Vec<(String, bool, bool, u32)> = vec![
        ("feed off".into(), false, false, 4),
        ("feed on, no reader".into(), true, false, FeedConfig::default().pore_sweep),
        ("feed on, reader 60 Hz".into(), true, true, FeedConfig::default().pore_sweep),
    ];
    if !sweeps.is_empty() {
        arms = sweeps.iter().map(|&n| (format!("no reader, pore_sweep {n}"), true, false, n)).collect();
    }
    println!("arm                       | step ms/tick | feed ms/update | updates | chunks taken");
    for (name, feed, reader, pore_sweep) in arms {
        let session = Arc::new(Session::new(SessionOptions {
            exec: params.clone(),
            feed,
            feed_config: FeedConfig { pore_sweep },
            ..SessionOptions::default()
        }));
        session
            .load_blocking(&cache, Duration::from_secs(120))
            .map_err(anyhow::Error::msg)?;
        let stop = Arc::new(AtomicBool::new(false));
        let taker = reader.then(|| {
            let (s, stop) = (session.clone(), stop.clone());
            std::thread::spawn(move || {
                let mut chunks = 0usize;
                while !stop.load(Ordering::Relaxed) {
                    chunks += s.take_view().chunks.len();
                    std::thread::sleep(Duration::from_millis(16));
                }
                chunks
            })
        });
        std::thread::sleep(Duration::from_secs_f64(seconds));
        stop.store(true, Ordering::Relaxed);
        let taken = taker.map(|t| t.join().unwrap());
        let st = session.status();
        println!(
            "{name:<25} | {:>12.2} | {:>14.3} | {:>7} | {}",
            st.step_ms,
            st.feed_ms,
            st.feed_updates,
            taken.map_or("-".to_string(), |n| n.to_string())
        );
    }
    Ok(())
}
