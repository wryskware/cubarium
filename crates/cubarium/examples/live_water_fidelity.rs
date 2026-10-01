//! **Is coarse water good enough?** The training world stepped with the live water leg
//! and with the training-only coarse one (`World::set_water_every`), one CSV row per
//! simulated minute: measurement only.
//!
//! ```text
//! live_water_fidelity seed=1 minutes=60 water_every=4 \
//!     policy=browser=<centre.json> policy=littershredder=<centre.json> > run.csv
//! ```
//!
//! Columns: the lake (volume, visible area, level), the soil's mean pore fill, plant
//! stands, standing foliage and the ground covers' share, the two lineages' populations,
//! and what the animals read of the water — the mean `Wet` response every founder
//! controller was handed this minute, the mean standing depth at the bodies' faces
//! (sampled every second), and the cumulative drownings and births.

use std::sync::{Arc, Mutex};
use std::time::Instant;

use cubarium::voxel::policy_args;
use cubarium_search::es::voxel::live::readout::Scan;
use cubarium_search::es::voxel::live::{ecology::EcologyParams, world};
use cubarium_search::es::voxel::{EpisodeDriver, VoxelPolicyFile};
use cubarium_voxel_fauna::{Controller, Departure, Founder, Response};

/// The `Wet` module's response channel in both manifests (offset 13).
const WET: usize = 13;

type Tally = Arc<Mutex<(f64, u64)>>;

/// A founder controller that tallies the `Wet` response it is handed.
struct WetTap {
    inner: Box<dyn Controller>,
    tally: Tally,
}

impl Controller for WetTap {
    fn drive(&mut self, observation: &[f64]) -> Response {
        let mut t = self.tally.lock().expect("tally");
        t.0 += observation[WET];
        t.1 += 1;
        drop(t);
        self.inner.drive(observation)
    }
    fn reset(&mut self) {
        self.inner.reset();
    }
}

fn arg<T: std::str::FromStr>(args: &[String], key: &str, default: T) -> T {
    args.iter()
        .find_map(|a| a.strip_prefix(key).and_then(|v| v.parse().ok()))
        .unwrap_or(default)
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let seed: u64 = arg(&args, "seed=", 1);
    let minutes: u64 = arg(&args, "minutes=", 60);
    let every: u32 = arg(&args, "water_every=", 1);
    let cfg = world::training_config();
    let (mut sim, info) = world::found_world(
        &cfg,
        seed,
        cubarium_voxel_sim::habitat::FOUNDER_COUNTS,
        &EcologyParams::defaults(),
    );
    eprintln!(
        "founded seed {} accepted={} founders={:?} stands={} water_every={every}",
        info.seed, info.accepted, info.founders, info.stands
    );
    let tally: Tally = Arc::new(Mutex::new((0.0, 0)));
    let drivers: Vec<(Founder, EpisodeDriver)> = policy_args(&args)
        .expect("policy=")
        .into_iter()
        .map(|(f, p)| {
            let file = VoxelPolicyFile::load(&p).expect("policy file");
            (f, file.driver().expect("driver"))
        })
        .collect();
    {
        let fauna = sim.fauna_mut();
        let teachers = fauna.config().teachers;
        for (founder, driver) in &drivers {
            let (driver, tally) = (driver.clone(), Arc::clone(&tally));
            fauna.set_founder_factory(
                *founder,
                Arc::new(move || -> Box<dyn Controller> {
                    Box::new(WetTap {
                        inner: driver.fresh(teachers),
                        tally: Arc::clone(&tally),
                    })
                }),
            );
            fauna.set_policy_driven(*founder, true);
        }
        let bodies: Vec<(u64, Founder)> = fauna
            .view()
            .animals
            .iter()
            .filter_map(|a| a.founder.map(|f| (a.id, f)))
            .collect();
        for (id, f) in bodies {
            fauna.install_founder_controller(id, f);
        }
    }
    sim.world_mut().set_water_every(every);
    println!(
        "minute,lake_m3,lake_m2,lake_level_m,soil_moisture,stands,foliage,cover_share,\
         pop_blind,pop_browser,wet_mean,standing_depth_mean,drowned,starved,births,raining_s,tick_ms"
    );
    let per_min = 60 * u64::from(cubarium_voxel_fauna::TICK_HZ);
    let t = Instant::now();
    for minute in 1..=minutes {
        let (mut depth_sum, mut depth_n, mut raining) = (0.0, 0u64, 0u64);
        let t0 = Instant::now();
        for tick in 0..per_min {
            sim.step();
            if tick % u64::from(cubarium_voxel_fauna::TICK_HZ) == 0 {
                let (w, _, fauna) = sim.layers();
                let v = w.view();
                if v.is_raining() {
                    raining += 1;
                }
                for a in fauna.view().animals.iter().filter(|a| a.founder.is_some()) {
                    depth_sum += v.standing_depth_m(i64::from(a.site.x), a.site.y, a.site.z);
                    depth_n += 1;
                }
            }
        }
        let tick_ms = t0.elapsed().as_secs_f64() * 1e3 / per_min as f64;
        let scan = Scan::of(&sim);
        let f = sim.flora().view();
        let covers: usize = f
            .stands
            .iter()
            .filter(|s| f.config.species(s.species).ground_cover)
            .count();
        let a = sim.fauna().view();
        let pop = |fd: Founder| a.animals.iter().filter(|x| x.founder == Some(fd)).count();
        let (wet, n) = std::mem::take(&mut *tally.lock().expect("tally"));
        println!(
            "{minute},{:.4},{:.3},{:.3},{:.5},{},{:.4},{:.3},{},{},{:.5},{:.5},{},{},{},{raining},{tick_ms:.3}",
            scan.lake_volume_m3,
            scan.lake_area_m2,
            scan.lake_level_m,
            scan.soil_moisture,
            f.stands.len(),
            scan.foliage,
            covers as f64 / f.stands.len().max(1) as f64,
            pop(Founder::Blind),
            pop(Founder::Browser),
            if n > 0 { wet / n as f64 } else { f64::NAN },
            if depth_n > 0 {
                depth_sum / depth_n as f64
            } else {
                f64::NAN
            },
            a.ledger.departed(Departure::Drowned),
            a.ledger.departed(Departure::Starved),
            a.ledger.births,
        );
    }
    eprintln!("wall {:.1} s", t.elapsed().as_secs_f64());
}
