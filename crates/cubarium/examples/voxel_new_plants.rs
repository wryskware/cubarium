//! Package N's measurement: the vaulttree, lanternberry and siphonreed on the shipped
//! presets (`design/handoffs/voxel-new-plants-2026-09-23.md`, "Measure").
//!
//! ```text
//! cargo run --release -p cubarium --example voxel_new_plants -- preset=default seed=1 \
//!     marks=60,360 [threads=1]
//! ```
//!
//! The census's preset arm, verbatim — the host's founding loop, the built-in trained
//! centres, the outlet opened — stepped to the last mark. CSV on stdout, one block kind
//! per first column:
//!
//! - `species`: at each mark, per plant species, faces eligible after the pre-roll, founders
//!   seeded, stands alive, establishments since t = 0 (a stand whose id is new after a
//!   tick), packages landed (`propagule_landed / package`), and browser bites.
//! - `fall`: every vaulttree death — its tick, the line's share count
//!   `round(r / voxel)` and distinct sites ([`cubarium_voxel_flora::fall_line`] on the
//!   stand as it stood the tick before), and its wood then.
//! - `lines`: at each mark, sites on any vault line, glowcap stands alive on or beside
//!   one (within one cell sideways and one row), and glowcap establishments there.
//!
//! Nothing here changes the model; it reads the stands every tick.

use std::collections::BTreeSet;

use cubarium::voxel::habitat;
use cubarium::voxel::install_default_founders;
use cubarium_voxel::Command as WorldCommand;
use cubarium_voxel_fauna::{Senses, TICK_HZ};
use cubarium_voxel_flora::{FloraConfig, Site, Species, fall_line};
use cubarium_voxel_sim::{Sim, SimConfig};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let arg = |k: &str| args.iter().find_map(|a| a.strip_prefix(k));
    let preset_name = arg("preset=").unwrap_or("default");
    let preset = cubarium_voxel::Preset::find(preset_name)
        .unwrap_or_else(|| panic!("no landform preset is called {preset_name:?}"));
    let seed: u64 = arg("seed=").map_or(1, |s| s.parse().expect("seed=N"));
    let threads: usize = arg("threads=").map_or(1, |s| s.parse().expect("threads=N"));
    let mut marks: Vec<u64> = arg("marks=")
        .unwrap_or("60")
        .split(',')
        .map(|m| m.parse().expect("marks=MIN,MIN"))
        .collect();
    marks.sort_unstable();

    let founded = cubarium::voxel::ambient_habitat(
        &preset.config(),
        seed,
        FloraConfig::for_voxel_size,
        habitat::FOUNDER_COUNTS,
    );
    let (world, flora, mut fauna, seeded) =
        (founded.world, founded.flora, founded.fauna, founded.seeded);
    eprintln!(
        "scene: preset {} seed {} (kept {}), {}",
        preset.name,
        seed,
        founded.seed,
        if founded.accepted { "accepted" } else { "NOT accepted" }
    );
    install_default_founders(&mut fauna).expect("the built-in centres validate");
    let mut senses = Senses::new();
    senses.settle(&world.view(), &flora.view());
    let mut sim = Sim::new(world, flora, fauna, SimConfig { threads }, Some(senses));
    if sim.world().config().closed_water_budget && !sim.world().outlet_open() {
        sim.world_mut()
            .apply(WorldCommand::SetOutlet { open: true });
    }

    let tag = format!("{},{}", preset.name, seed);
    println!(
        "species,preset,seed,min,species,eligible,seeded,alive,established,packages_landed,bites"
    );
    println!("fall,preset,seed,tick,shares,sites,wood");
    println!("lines,preset,seed,min,line_sites,glowcap_on_lines,glowcap_established_on_lines");

    let mut established = [0u64; Species::COUNT];
    let mut glowcap_on_line_births = 0u64;
    let mut line_sites: BTreeSet<Site> = BTreeSet::new();
    let ticks_per_min = 60 * u64::from(TICK_HZ);
    let last = marks.last().copied().unwrap_or(0) * ticks_per_min;
    let mut tick = 0u64;
    let mut next_mark = 0usize;
    while tick < last {
        let births_before = sim.flora().view().ledger.births;
        let vaults: Vec<_> = sim
            .flora()
            .view()
            .stands
            .iter()
            .filter(|s| s.species == Species::Vaulttree)
            .copied()
            .collect();
        sim.step();
        tick += 1;
        let fv = sim.flora().view();
        for s in fv.stands.iter().filter(|s| s.id >= births_before) {
            established[s.species.index()] += 1;
            if s.species == Species::Glowcap && near_line(&line_sites, s.site) {
                glowcap_on_line_births += 1;
            }
        }
        for v in &vaults {
            if fv.stands.iter().any(|s| s.id == v.id) {
                continue;
            }
            let line = fall_line(fv.config, &sim.world().view(), v);
            let shares: usize = line.iter().map(|l| l.1).sum();
            println!("fall,{tag},{tick},{shares},{},{:.4}", line.len(), v.wood);
            line_sites.extend(line.iter().map(|l| l.0));
        }
        if next_mark < marks.len() && tick == marks[next_mark] * ticks_per_min {
            let min = marks[next_mark];
            next_mark += 1;
            let a = sim.fauna().view();
            for sp in Species::ALL {
                let sc = fv.config.species(sp);
                let package = sc.alive_min / sc.propagule_split[0];
                let alive = fv.stands.iter().filter(|s| s.species == sp).count();
                println!(
                    "species,{tag},{min},{},{},{},{alive},{},{:.1},{}",
                    sp.name(),
                    seeded.eligible_by_species[sp.index()],
                    seeded.stands_by_species[sp.index()],
                    established[sp.index()],
                    fv.ledger.propagule_landed[sp.index()] / package,
                    a.ledger.bites_by_plant[sp.index()],
                );
            }
            let caps = fv
                .stands
                .iter()
                .filter(|s| s.species == Species::Glowcap && near_line(&line_sites, s.site))
                .count();
            println!(
                "lines,{tag},{min},{},{caps},{glowcap_on_line_births}",
                line_sites.len()
            );
        }
    }
}

/// On a vault line site or beside one: one cell sideways (x unwrapped here; the seam is a
/// rounding error in a count) and one row up or down, the glowcap's own mycelium reach.
fn near_line(lines: &BTreeSet<Site>, s: Site) -> bool {
    lines.iter().any(|l| {
        l.x.abs_diff(s.x) <= 1 && l.z.abs_diff(s.z) <= 1 && l.y.abs_diff(s.y) <= 1
    })
}
