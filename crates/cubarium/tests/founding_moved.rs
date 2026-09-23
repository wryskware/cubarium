//! P5-B item 1: the founding loop moved out of this binary crate into
//! `cubarium-voxel-sim`, and the host calls the moved code unchanged in behaviour.
//!
//! The check is the brief's: **counts, not hashes** — the founded world's seed, the
//! refusals, the acceptance verdict, the pre-roll's ticks, and what the seeder planted
//! and placed, on `default` seed base 1. The expected numbers were read off `main` at
//! e69747b, before the move, with this same test. A change to the seeder, the pre-roll
//! or the fauna's placement rules moves them honestly; re-read them then.
//!
//! It founds a whole `default` world (a ~5,000-tick pre-roll), so it is a study run by
//! name, not part of the fast suite:
//!
//! ```text
//! cargo test --release -p cubarium --test founding_moved -- --ignored
//! ```

use cubarium::voxel::habitat;
use cubarium_voxel_fauna::Founder;
use cubarium_voxel_flora::FloraConfig;

#[test]
#[ignore = "study: run by name — founds the whole `default` preset (~15 s in release)"]
fn the_moved_founding_loop_founds_default_seed_1_as_main_did() {
    let cfg = cubarium_voxel::Preset::find("default")
        .expect("the default preset")
        .config();
    let founded = cubarium::voxel::ambient_habitat(
        &cfg,
        1,
        FloraConfig::for_voxel_size,
        habitat::FOUNDER_COUNTS,
    );
    let s = &founded.seeded;
    let got = (
        founded.seed,
        founded.lake_rejected,
        founded.habitat_rejected,
        founded.accepted,
        s.acceptance.accepted,
    );
    assert_eq!(got, (1, 0, 0, true, true), "seed, refusals and the verdict");
    assert_eq!(
        (s.pre_roll.ticks, s.pre_roll.shower_ticks, s.pre_roll.drain.ticks),
        (4845, 2156, 1889),
        "the pre-roll stepped what it stepped on main"
    );
    assert_eq!(s.stands, 74, "stands planted");
    assert_eq!(s.stands_by_species, [13, 7, 30, 13, 4, 7], "stands by species");
    assert_eq!(s.founders, FOUNDERS, "founders placed, by lineage");
    assert_eq!(s.shortfall, [0; Founder::COUNT], "no founder short");
    assert_eq!(
        founded.fauna.view().animals.len(),
        s.animals(),
        "the fauna holds exactly the founders"
    );
    assert_eq!(
        founded.flora.view().stands.len(),
        s.stands,
        "the flora holds exactly the stands"
    );
}

/// Founders placed on `default` seed 1 at e69747b, littershredders then frondgrazers.
const FOUNDERS: [usize; Founder::COUNT] = [8, 8];
