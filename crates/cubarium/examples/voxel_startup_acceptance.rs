//! Startup acceptance: what the founding loop drew, refused and kept, per preset.
//!
//! ```text
//! cargo run --release -p cubarium --example voxel_startup_acceptance -- small default wide
//! ```
//!
//! The founding loop is the host's own (`cubarium::voxel::ambient_habitat`: lake gate,
//! pre-roll with the opening shower, seeding, the acceptance check, redrawn on a
//! refusal), from seed base 1 as the census arms use. The loop says why it refused each
//! try on stderr; this prints what it kept, as CSV blocks on stdout:
//!
//! - `startup`: seed, tries refused, accepted, pre-roll ticks and the wall seconds one
//!   pre-roll of the kept seed takes on this machine, stands per m², imported organic;
//! - `species`: stands, stands per m² and eligible faces per producer (0 = unmet niche);
//! - `component`: per lineage, each founder-occupied component's founders, stock,
//!   `stock / (founders · upkeep)` and `production / (founders · upkeep)`.
//!
//! Nothing here steps an ecology.
//! `design/handoffs/voxel-startup-acceptance-2026-09-22.md`.

use std::time::Instant;

use cubarium::voxel::habitat;
use cubarium_voxel::World;
use cubarium_voxel_fauna::Founder;
use cubarium_voxel_flora::{FloraConfig, Species};

const SEED_BASE: u64 = 1;

fn main() {
    let names: Vec<String> = std::env::args().skip(1).collect();
    let names = if names.is_empty() {
        vec!["small".into(), "default".into(), "wide".into()]
    } else {
        names
    };
    println!(
        "startup,preset,seed,lake_rejected,habitat_rejected,accepted,preroll_ticks,\
         shower_ticks,drain_ticks,drain_converged,preroll_wall_s,founding_wall_s,area_m2,\
         stands,stands_per_m2,imported_organic"
    );
    println!("species,preset,species,stands,stands_per_m2,eligible_faces");
    println!(
        "component,preset,lineage,wanted,placed,upkeep_per_h,component,founders,stock,\
         producers,stock_per_founder_hour,production_per_founder_hour,habitable"
    );
    for name in &names {
        let preset = cubarium_voxel::Preset::find(name)
            .unwrap_or_else(|| panic!("no landform preset is called {name:?}"));
        let cfg = preset.config();
        let t = Instant::now();
        let founded = cubarium::voxel::ambient_habitat(
            &cfg,
            SEED_BASE,
            FloraConfig::for_voxel_size,
            habitat::FOUNDER_COUNTS,
        );
        let founding_s = t.elapsed().as_secs_f64();
        // One pre-roll of the kept seed on its own, timed: what a try costs in startup.
        let mut probe = World::new(cubarium_voxel::Config {
            seed: founded.seed,
            ..cfg.clone()
        });
        let t = Instant::now();
        habitat::pre_roll(&mut probe, true);
        let preroll_s = t.elapsed().as_secs_f64();

        let wc = founded.world.config();
        let area = f64::from(wc.width) * f64::from(wc.depth) * wc.cell_area();
        let s = &founded.seeded;
        let pr = s.pre_roll;
        println!(
            "startup,{},{},{},{},{},{},{},{},{},{:.2},{:.2},{:.1},{},{:.4},{:.4}",
            preset.name,
            founded.seed,
            founded.lake_rejected,
            founded.habitat_rejected,
            founded.accepted,
            pr.ticks,
            pr.shower_ticks,
            pr.drain.ticks,
            pr.drain.converged,
            preroll_s,
            founding_s,
            area,
            s.stands,
            s.stands as f64 / area,
            s.imported_organic,
        );
        for sp in Species::ALL {
            let n = s.stands_by_species[sp.index()];
            println!(
                "species,{},{},{n},{:.4},{}",
                preset.name,
                sp.name(),
                n as f64 / area,
                s.eligible_by_species[sp.index()],
            );
        }
        for founder in Founder::ALL {
            let l = &s.acceptance.lineages[founder.index()];
            for c in &l.components {
                let per = l.upkeep_per_h * c.founders as f64;
                println!(
                    "component,{},{},{},{},{:.4},{},{},{:.4},{},{:.2},{:.2},{}",
                    preset.name,
                    founder.name(),
                    l.wanted,
                    l.placed,
                    l.upkeep_per_h,
                    c.component,
                    c.founders,
                    c.food.stock,
                    c.food.producers,
                    c.food.stock / per,
                    c.food.production_per_h / per,
                    c.habitable,
                );
            }
            if l.components.is_empty() {
                println!(
                    "component,{},{},{},{},{:.4},-,0,0,0,0,0,false",
                    preset.name,
                    founder.name(),
                    l.wanted,
                    l.placed,
                    l.upkeep_per_h
                );
            }
        }
    }
}
