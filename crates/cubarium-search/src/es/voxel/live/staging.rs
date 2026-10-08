//! Run-directory protocol shared by the long headless training commands.
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use cubarium_voxel_sim::{Sim, saved, staged};

#[derive(Debug)]
pub struct Run {
    pub dir: PathBuf,
    started: Instant,
    deadline: Option<Instant>,
    first_deadline: Instant,
    calibrated: AtomicBool,
    error: Mutex<Option<String>>,
    paused: Mutex<Option<String>>,
    status_lock: Mutex<()>,
}

impl Run {
    pub fn open(
        dir: &Path,
        resume: bool,
        wall_seconds: f64,
        first_seconds: f64,
    ) -> Result<Arc<Self>, String> {
        std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
        if !wall_seconds.is_finite()
            || wall_seconds <= 0.0
            || !first_seconds.is_finite()
            || first_seconds <= 0.0
        {
            return Err("wall budget must be finite and positive".into());
        }
        for marker in ["DONE", "PAUSED", "FAILED"] {
            let path = dir.join(marker);
            if path.exists() {
                std::fs::remove_file(path).map_err(|e| e.to_string())?;
            }
        }
        if resume && dir.join("STOP").exists() {
            std::fs::remove_file(dir.join("STOP")).map_err(|e| e.to_string())?;
        }
        let started = Instant::now();
        let run = Arc::new(Self {
            dir: dir.into(),
            started,
            deadline: Some(
                started
                    .checked_add(
                        Duration::try_from_secs_f64(wall_seconds).map_err(|e| e.to_string())?,
                    )
                    .ok_or("wall budget overflows clock")?,
            ),
            first_deadline: started
                .checked_add(
                    Duration::try_from_secs_f64(first_seconds.min(wall_seconds))
                        .map_err(|e| e.to_string())?,
                )
                .ok_or("first-generation budget overflows clock")?,
            calibrated: AtomicBool::new(false),
            error: Mutex::new(None),
            paused: Mutex::new(None),
            status_lock: Mutex::new(()),
        });
        println!(
            "# staging: {}; first boundary deadline = min(--first-generation-seconds={first_seconds:.1}, --wall-seconds={wall_seconds:.1}), including setup; STOP pauses at a boundary",
            dir.display()
        );
        run.status(0, 0, 0.0, "pending", "RUNNING")?;
        Ok(run)
    }

    pub fn status(
        &self,
        generation: u32,
        window: u64,
        speed: f64,
        check: &str,
        state: &str,
    ) -> Result<(), String> {
        let _guard = self.status_lock.lock().expect("status lock");
        atomic_text(
            &self.dir.join("status"),
            &format!(
                "generation={generation} window={window} wall_speed={speed:.2}_ticks/s elapsed={:.2}s last_check={check} state={state}\n",
                self.started.elapsed().as_secs_f64()
            ),
        )?;
        println!(
            "stage: generation={generation} window={window} wall_speed={speed:.2}_ticks/s last_check={check} state={state}"
        );
        Ok(())
    }

    pub fn fail(&self, why: String) {
        let mut error = self.error.lock().expect("run error");
        if error.is_none() {
            *error = Some(why);
        }
    }

    pub fn check(&self) -> Result<(), String> {
        if let Some(e) = self.error.lock().expect("run error").clone() {
            return Err(e);
        }
        if self.deadline.is_some_and(|d| Instant::now() >= d)
            || (!self.calibrated.load(Ordering::Relaxed) && Instant::now() >= self.first_deadline)
        {
            return Err(
                "wall budget: deadline reached; resume from the last boundary checkpoint".into(),
            );
        }
        Ok(())
    }

    /// Call only with the boundary's checkpoint already committed.
    pub fn boundary(&self, next: u32, measured_seconds: f64) -> bool {
        if measured_seconds > 0.0 {
            self.calibrated.store(true, Ordering::Relaxed);
        }
        let why = if self.dir.join("STOP").exists() {
            Some(format!("STOP at boundary {next}; checkpoint written"))
        } else if self.deadline.is_some_and(|d| {
            Instant::now()
                .checked_add(Duration::from_secs_f64(measured_seconds.max(0.0)))
                .is_none_or(|end| end >= d)
        }) {
            Some(format!(
                "wall budget: insufficient time for boundary {next} (measured {measured_seconds:.2} s)"
            ))
        } else {
            None
        };
        if let Some(why) = why {
            println!("# PAUSED: {why}");
            *self.paused.lock().expect("pause") = Some(why);
            true
        } else {
            false
        }
    }

    fn end(&self, state: &str, reason: &str) -> Result<(), String> {
        // Keep the last generation, window and check in the terminal status.
        let old = std::fs::read_to_string(self.dir.join("status")).unwrap_or_default();
        let old = old.trim_end().split(" state=").next().unwrap_or("");
        atomic_text(&self.dir.join("status"), &format!("{old} state={state}\n"))?;
        atomic_text(&self.dir.join(state), &format!("{reason}\n"))
    }
}

/// Errors, including checkpoint I/O errors and setup panics, always leave FAILED.
pub fn execute<T: Default>(
    dir: &Path,
    resume: bool,
    wall_seconds: f64,
    first_seconds: f64,
    f: impl FnOnce(&Arc<Run>) -> Result<T, String>,
) -> Result<T, String> {
    let run = Run::open(dir, resume, wall_seconds, first_seconds)?;
    let result =
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| f(&run))).unwrap_or_else(|p| {
            Err(format!(
                "run panicked: {}",
                p.downcast_ref::<String>()
                    .map(String::as_str)
                    .or_else(|| p.downcast_ref::<&str>().copied())
                    .unwrap_or("unknown panic")
            ))
        });
    match result {
        Ok(value) => {
            if let Some(why) = run.paused.lock().expect("pause").as_ref() {
                run.end("PAUSED", why)?;
            } else {
                run.end("DONE", "completed all requested boundaries")?;
            }
            Ok(value)
        }
        Err(why) if why.starts_with("wall budget:") => {
            println!("# PAUSED: {why}");
            run.end("PAUSED", &why)?;
            Ok(T::default())
        }
        Err(why) => {
            run.end("FAILED", &why)?;
            Err(why)
        }
    }
}

pub fn atomic_text(path: &Path, text: &str) -> Result<(), String> {
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, text).map_err(|e| format!("{}: {e}", tmp.display()))?;
    std::fs::rename(&tmp, path).map_err(|e| format!("{}: {e}", path.display()))
}

/// Drop an incomplete tail and any records beyond the restored boundary. The
/// checkpoint, rather than a possibly newer log, decides what was committed.
pub fn repair_log(path: &Path, keep: impl Fn(&serde_json::Value) -> bool) -> Result<(), String> {
    let bytes = match std::fs::read(path) {
        Ok(bytes) => bytes,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(e) => return Err(format!("{}: {e}", path.display())),
    };
    let mut text = String::new();
    for line in String::from_utf8_lossy(&bytes).lines() {
        if let Ok(record) = serde_json::from_str::<serde_json::Value>(line)
            && keep(&record)
        {
            text.push_str(&serde_json::to_string(&record).map_err(|e| e.to_string())?);
            text.push('\n');
        }
    }
    atomic_text(path, &text)
}

pub fn check_config(dir: &Path) -> staged::StageConfig {
    staged::StageConfig {
        dir: dir.into(),
        segment_min: 1,
        check_min: 1,
        keep: 1,
        expect: Vec::new(),
        still: Vec::new(),
        residual_tol: 1e-5,
        ticks_per_min: staged::TICKS_PER_MIN,
    }
}

pub fn instant(sim: &Sim) -> Result<(), String> {
    staged::instant_check(sim, 1e-5)
}

pub fn save_world(dir: &Path, sim: &Sim) -> Result<(), String> {
    let (world, flora, fauna) = sim.layers();
    saved::save(
        dir,
        world,
        flora,
        fauna,
        world.tick() / staged::TICKS_PER_MIN,
    )
}

#[derive(Clone, Debug)]
pub struct Segment {
    pub run: Arc<Run>,
    pub generation: u32,
    pub full: bool,
    pub arm: String,
}

impl Segment {
    pub fn world_dir(&self, seed: u64) -> PathBuf {
        self.run
            .dir
            .join("worlds")
            .join(format!("g{}-{}-seed{seed}", self.generation, self.arm))
    }
    pub fn check_world(
        &self,
        sim: &Sim,
        seed: u64,
        window: Option<&staged::Window>,
    ) -> Result<String, String> {
        let result = instant(sim).and_then(|()| match window {
            Some(w) => w.verdict(sim, &check_config(&self.run.dir)),
            None => Ok("drivers and ledgers OK".into()),
        });
        if let Err(why) = &result {
            let dir = self.world_dir(seed);
            let saved = save_world(&dir, sim);
            let why = format!(
                "world {} seed {seed}: {why}{}",
                dir.display(),
                saved
                    .err()
                    .map_or(String::new(), |e| format!("; checkpoint failed: {e}"))
            );
            self.run.fail(why.clone());
            return Err(why);
        }
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cubarium_voxel_fauna::{Fauna, FaunaConfig, Founder};
    use cubarium_voxel_flora::Flora;

    pub(super) fn scratch(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("cub-training-stage-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn stop_commits_a_boundary_and_resume_clears_the_request() {
        let dir = scratch("stop");
        execute(&dir, false, 60.0, 60.0, |run| {
            atomic_text(&dir.join("checkpoint"), "1")?;
            std::fs::write(dir.join("STOP"), "pause").unwrap();
            assert!(run.boundary(1, 0.01));
            Ok(())
        })
        .unwrap();
        assert!(dir.join("PAUSED").is_file());
        execute(&dir, true, 60.0, 60.0, |run| {
            assert_eq!(
                std::fs::read_to_string(dir.join("checkpoint")).unwrap(),
                "1"
            );
            assert!(!run.boundary(1, 0.01));
            Ok(())
        })
        .unwrap();
        assert!(dir.join("DONE").is_file());
        assert!(!dir.join("PAUSED").exists());
        assert!(!dir.join("FAILED").exists());
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn undriven_generation_zero_fails_with_world_and_seed() {
        let dir = scratch("undriven");
        let cfg = cubarium_voxel::Config {
            width: 64,
            height: 24,
            depth: 6,
            ..Default::default()
        };
        let mut world = cubarium_voxel_sim::scene::authored(cfg);
        let mut flora = Flora::new(Default::default());
        let mut fauna = Fauna::new(FaunaConfig::placeholder());
        let mut counts = [0; Founder::COUNT];
        counts[Founder::Blind.index()] = 1;
        cubarium_voxel_sim::habitat::seed_with_founder_counts(
            &mut world, &mut flora, &mut fauna, counts,
        );
        let id = fauna.view().animals[0].id;
        fauna.take_controller(id);
        let sim = Sim::ambient(
            world,
            flora,
            fauna,
            cubarium_voxel_sim::SimConfig::with_threads(1),
        );
        let err = execute(&dir, false, 60.0, 60.0, |run| {
            Segment {
                run: run.clone(),
                generation: 0,
                full: true,
                arm: "candidate0".into(),
            }
            .check_world(&sim, 7, None)
            .map(|_| ())
        })
        .unwrap_err();
        assert!(
            err.contains("seed 7") && err.contains("no controller"),
            "{err}"
        );
        assert!(dir.join("worlds/g0-candidate0-seed7/fauna.bin").is_file());
        assert!(dir.join("FAILED").is_file());
        assert!(!dir.join("DONE").exists());
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn checkpoint_write_error_is_terminal() {
        let dir = scratch("io");
        std::fs::create_dir(dir.join("checkpoint.tmp")).unwrap();
        let err = execute(&dir, false, 60.0, 60.0, |_| {
            atomic_text(&dir.join("checkpoint.json"), "checkpoint")
        })
        .unwrap_err();
        assert!(err.contains("checkpoint.tmp"), "{err}");
        assert!(dir.join("FAILED").is_file());
        assert!(!dir.join("PAUSED").exists());
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn resumed_logs_end_at_the_checkpoint_and_drop_partial_records() {
        let dir = scratch("log-recovery");
        let log = dir.join("generations.jsonl");
        std::fs::write(
            &log,
            "{\"generation\":0}\n{\"generation\":1}\n{\"generation\":2}\n{\"generation\":",
        )
        .unwrap();
        repair_log(&log, |r| r["generation"].as_u64().is_some_and(|g| g < 2)).unwrap();
        let records = std::fs::read_to_string(&log).unwrap();
        assert_eq!(records, "{\"generation\":0}\n{\"generation\":1}\n");
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn the_first_boundary_has_a_deadline_and_later_ones_use_measured_time() {
        let dir = scratch("deadline");
        let mut run = Run::open(&dir, false, 60.0, 30.0).unwrap();
        Arc::get_mut(&mut run).unwrap().first_deadline = Instant::now();
        assert!(run.check().unwrap_err().starts_with("wall budget:"));
        run.calibrated.store(true, Ordering::Relaxed);
        assert!(run.check().is_ok());
        assert!(run.boundary(1, 61.0));
        std::fs::remove_dir_all(dir).unwrap();
    }
}
