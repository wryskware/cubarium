//! Fixed ten-minute copied-world mechanism diagnosis; no live IO or biology changes.
#[path = "hunter_compare/audit.rs"]
#[allow(dead_code)]
mod inventory;
#[path = "astra_quiet_diagnosis/observer.rs"]
mod observer;
use anyhow::{Context, Result, ensure};
use clap::Parser;
use cubarium_core::{
    CareCommand, CareDose, CareKind, CareTarget, World, WorldState, decode_snapshot,
    encode_snapshot,
};
use inventory::{Inventory, Sum};
use serde_json::{Value, json};
use std::{
    fs::{self, File, OpenOptions},
    io::{BufWriter, Write},
    path::{Path, PathBuf},
    process::Command,
};
const TICKS: u64 = 12000;
const BUILD: &str = concat!(env!("CARGO_PKG_VERSION"), "+", env!("CUBARIUM_GIT_HASH"));
#[derive(Parser)]
struct Args {
    cohort: PathBuf,
    out: PathBuf,
}
fn sha(path: &Path) -> Result<String> {
    let out = Command::new("sha256sum").arg(path).output()?;
    ensure!(out.status.success(), "checksum failed");
    let text = String::from_utf8(out.stdout)?;
    let hash = text.split_whitespace().next().context("checksum absent")?;
    ensure!(
        hash.len() == 64 && hash.bytes().all(|c| c.is_ascii_hexdigit()),
        "invalid checksum"
    );
    Ok(hash.into())
}
fn file(path: &Path) -> Result<BufWriter<File>> {
    Ok(BufWriter::new(
        OpenOptions::new().write(true).create_new(true).open(path)?,
    ))
}
fn line(w: &mut BufWriter<File>, v: &Value) -> Result<()> {
    serde_json::to_writer(&mut *w, v)?;
    w.write_all(b"\n")?;
    Ok(())
}
fn save(path: &Path, v: &Value) -> Result<()> {
    let mut f = file(path)?;
    line(&mut f, v)?;
    f.flush()?;
    f.get_ref().sync_all()?;
    Ok(())
}
fn snapshot(path: &Path, s: &WorldState) -> Result<()> {
    let mut f = file(path)?;
    f.write_all(&encode_snapshot(s, BUILD))?;
    f.flush()?;
    f.get_ref().sync_all()?;
    Ok(())
}
fn run(initial: &WorldState, feed: bool, dir: &Path) -> Result<Value> {
    run_ticks(initial, feed, dir, TICKS)
}
fn run_ticks(initial: &WorldState, feed: bool, dir: &Path, ticks: u64) -> Result<Value> {
    fs::create_dir(dir)?;
    let mut world = World::from_state(initial.clone()).map_err(|e| anyhow::anyhow!(e))?;
    let mut observer = observer::Observer::new(initial);
    let mut samples = file(&dir.join("samples.jsonl"))?;
    let mut events = file(&dir.join("events.jsonl"))?;
    save(
        &dir.join("opening.json"),
        &json!({"build":BUILD,"config":initial.config,
        "state_hash":cubarium_core::snapshot::state_hash(initial).to_string(),"feed":feed}),
    )?;
    line(&mut samples, &observer.sample(initial)?)?;
    let opening = Inventory::read(initial);
    let opening_ledger = initial.energy_ledgers();
    let mut light = Sum::default();
    let mut heat = Sum::default();
    let mut peak = [0.0f64; 4];
    let mut boundary_peak = [0.0f64; 2];
    let mut imported = [0.0; 2];
    let limits = [
        opening.material,
        opening.energy,
        opening.water,
        opening.energy,
    ]
    .map(|v| 1e-8 * v.max(1.0));
    let mut audit_passed = true;
    for elapsed in 0..ticks {
        if elapsed == 600 && feed {
            let before = Inventory::read(&world.state);
            let receipt = world.apply_care(&CareCommand {
                seq: 1,
                apply_after_tick: world.tick(),
                kind: CareKind::Feed,
                target: CareTarget {
                    face: 0,
                    u: 32.0,
                    v: 48.0,
                },
                dose: CareDose::STANDARD,
            });
            let applied = receipt
                .outcome
                .applied()
                .context("prescribed feed refused")?;
            imported = [applied.material_in, applied.energy_in];
            let after = Inventory::read(&world.state);
            boundary_peak = [
                (after.material - before.material - imported[0]).abs(),
                (after.energy - before.energy - imported[1]).abs(),
            ];
            ensure!(
                boundary_peak[0] < limits[0] && boundary_peak[1] < limits[1],
                "care boundary audit"
            );
            line(&mut events, &json!({"stream":"care","receipt":receipt}))?;
        }
        let counters = world.step();
        let (transient_light, transient_heat) = (counters.light_in, counters.heat_out);
        world.check_invariants().map_err(|e| anyhow::anyhow!(e))?;
        for event in world.drain_events() {
            line(&mut events, &json!({"stream":"life","event":event}))?;
        }
        ensure!(
            world.drain_hunter_events().is_empty(),
            "unexpected hunter event"
        );
        for record in observer.observe(&world.state)? {
            line(&mut events, &record)?;
        }
        let s = &world.state;
        let now = Inventory::read(s);
        let residuals = [
            now.material - opening.material - imported[0],
            now.energy
                - opening.energy
                - s.energy_ledgers().net_since(opening_ledger)
                - imported[1],
            now.water - opening.water - (s.rain_in_total - initial.rain_in_total)
                + (s.evap_out_total - initial.evap_out_total),
            now.energy - opening.energy - light.value() - transient_light
                + heat.value()
                + transient_heat
                - imported[1],
        ];
        for i in 0..4 {
            ensure!(residuals[i].is_finite(), "nonfinite audit");
            peak[i] = peak[i].max(residuals[i].abs());
            audit_passed &= residuals[i].abs() < limits[i];
        }
        ensure!(
            s.care.feed_material_in == imported[0] && s.care.feed_energy_in == imported[1],
            "care receipt/ledger mismatch"
        );
        if (elapsed + 1) % 200 == 0 {
            line(&mut samples, &observer.sample(&world.state)?)?;
            let c = world.telemetry();
            light.add(c.light_in);
            heat.add(c.heat_out);
        }
    }
    let (observed, bouts) = observer.finish();
    for b in bouts {
        line(&mut events, &b)?;
    }
    samples.flush()?;
    events.flush()?;
    samples.get_ref().sync_all()?;
    events.get_ref().sync_all()?;
    snapshot(&dir.join("closing.cubw"), &world.state)?;
    let result = json!({"technical_complete":audit_passed,"audit_passed":audit_passed,"build":BUILD,
        "peak_residuals_material_corrected_energy_water_independent_energy":peak,"limits":limits,
        "immediate_care_boundary_peaks":boundary_peak,"actual_feed_imports":imported,
        "closing_hash":cubarium_core::snapshot::state_hash(&world.state).to_string(),
        "population":world.population(),"observation":observed,
        "limits_of_interpretation":"fed is any field intake, not amount or source. Growth is observed same-ID structure gain. Escrow changes are post-step observations, not exact transaction records. Post-step energy below the configured oxidation threshold is exposure, not measured burn; feeding and prior payments occur before that branch. Dead-within-tick intake/funding can be absent. All full IDs retained up to an explicit20000 capacity; no motion metric."});
    save(&dir.join("result.json"), &result)?;
    Ok(result)
}
fn main() -> Result<()> {
    let args = Args::parse();
    let manifest_path = args.cohort.join("manifest.json");
    let m: Value = serde_json::from_slice(&fs::read(&manifest_path)?)?;
    ensure!(
        m["complete"] == true && m["kind"] == "pre-hunter-cohort-preparation",
        "wrong cohort"
    );
    let rows = m["openings"].as_array().context("openings")?;
    ensure!(rows.len() == 12, "all12 required");
    let mut openings = Vec::new();
    for seed in 1..=12u64 {
        let matches: Vec<_> = rows.iter().filter(|r| r["seed"] == seed).collect();
        ensure!(matches.len() == 1, "seed missing/duplicate");
        let row = matches[0];
        let path = PathBuf::from(row["snapshot"].as_str().context("snapshot path")?);
        ensure!(
            sha(&path)? == row["sha256"].as_str().context("opening SHA")?,
            "opening changed"
        );
        let (meta, s) = decode_snapshot(&fs::read(&path)?)?;
        ensure!(
            meta.schema == 9 && s.tick == 144000 && s.config.seed == seed,
            "opening identity"
        );
        ensure!(
            s.hunters == Default::default() && s.care == Default::default(),
            "primary inputs must be care/hunter-free"
        );
        openings.push((seed, s));
    }
    fs::create_dir(&args.out)?;
    save(
        &args.out.join("manifest.json"),
        &json!({"kind":"astra-ordinary-quiet-diagnosis-v1","build":BUILD,
        "binary_sha256":sha(&std::env::current_exe()?)?,"cohort_manifest_sha256":sha(&manifest_path)?,
        "cohort":m,"ticks":TICKS,"sample_every":200,"feed_at":600,"feed_target":{"face":0,"u":32,"v":48},
        "arms":["baseline","feed"],"note":"fixed10min all12; read-only copied worlds; no ambient or phenotype changes"}),
    )?;
    let mut results = Vec::new();
    for (seed, s) in openings {
        for feed in [false, true] {
            let arm = if feed { "feed" } else { "baseline" };
            let path = args.out.join(format!("seed-{seed}-{arm}"));
            let result = match run(&s, feed, &path) {
                Ok(v) => json!({"seed":seed,"arm":arm,"technical_complete":v["technical_complete"],
                    "closing_hash":v["closing_hash"],"error":null}),
                Err(e) => {
                    json!({"seed":seed,"arm":arm,"technical_complete":false,"error":format!("{e:#}")})
                }
            };
            eprintln!("seed{seed} {arm}: {}", result["technical_complete"]);
            results.push(result);
        }
    }
    let complete = results.iter().all(|r| r["technical_complete"] == true);
    save(
        &args.out.join("summary.json"),
        &json!({"technical_complete":complete,"results":results}),
    )?;
    ensure!(
        complete,
        "one or more diagnostic arms failed; retained summary"
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn short_both_arms_audit_and_baseline_matches_unobserved_state() {
        let root = std::env::temp_dir().join(format!(
            "astra-quiet-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir(&root).unwrap();
        let initial = World::new(Default::default()).unwrap().state;
        for feed in [false, true] {
            let result = run_ticks(
                &initial,
                feed,
                &root.join(if feed { "feed" } else { "baseline" }),
                800,
            )
            .unwrap();
            assert_eq!(result["audit_passed"], true);
            assert_eq!(
                result["actual_feed_imports"],
                json!(if feed { [3.0, 6.0] } else { [0.0, 0.0] })
            );
            if !feed {
                let mut direct = World::from_state(initial.clone()).unwrap();
                for _ in 0..800 {
                    direct.step();
                }
                assert_eq!(
                    result["closing_hash"],
                    cubarium_core::snapshot::state_hash(&direct.state).to_string()
                );
            }
        }
        fs::remove_dir_all(root).unwrap();
    }
}
