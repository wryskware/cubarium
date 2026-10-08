//! What the session's vitals recorder costs the sim thread: the same saved world run
//! through a [`Session`] at the top speed for `seconds` of wall time, once with
//! `[vitals] enabled = false` and once with it on, printing `Status::step_ms` (the step
//! alone), `Status::vitals_ms` (the recorder, mean per tick) and `Status::vitals_peak_ms`
//! (its slowest tick) for each. Development
//! measurement only.
//!
//! `vitals_cost SAVE_DIR [seconds=60] [threads=0] [arms=off,on]`: `arms` repeats and
//! orders them (alternate them on a busy machine).

use std::time::Duration;

use cubarium_session::{MAX_SPEED, Session, SessionOptions, SimParams, VitalsConfig, prepare_loaded};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let save = args.first().expect("vitals_cost SAVE_DIR [seconds=] [threads=]").clone();
    let arg = |name: &str| args.iter().find_map(|a| a.strip_prefix(&format!("{name}=")).map(str::to_string));
    let seconds: f64 = arg("seconds").and_then(|v| v.parse().ok()).unwrap_or(60.0);
    let threads: usize = arg("threads").and_then(|v| v.parse().ok()).unwrap_or(0);
    let arms: Vec<bool> = arg("arms")
        .unwrap_or_else(|| "off,on".into())
        .split(',')
        .map(|a| match a {
            "on" => true,
            "off" => false,
            other => panic!("arms are on or off, not {other}"),
        })
        .collect();
    for enabled in arms {
        let exec = SimParams { threads, vitals: VitalsConfig { enabled, ..VitalsConfig::default() }, ..SimParams::default() };
        let prepared = prepare_loaded(&exec, std::path::Path::new(&save)).expect("the save loads");
        let session = Session::new(SessionOptions { exec: exec.clone(), speed: MAX_SPEED, ..SessionOptions::default() });
        session.install(prepared);
        while !session.status().running {
            std::thread::sleep(Duration::from_millis(20));
        }
        let t0 = session.status().stepped;
        std::thread::sleep(Duration::from_secs_f64(seconds));
        let st = session.status();
        let rows = session.query("vitals", &serde_json::json!({})).unwrap();
        println!(
            "vitals {}: ticks {} step_ms {:.3} vitals_ms {:.4} vitals_peak_ms {:.3} minutes_held {}",
            if enabled { "on " } else { "off" },
            st.stepped - t0,
            st.step_ms,
            st.vitals_ms,
            st.vitals_peak_ms,
            rows["animals"].as_array().map_or(0, |a| {
                let mut m: Vec<String> = a.iter().map(|r| r[0].to_string()).collect();
                m.dedup();
                m.len()
            }),
        );
    }
}
