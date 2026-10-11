//! Per-lineage ecology settings (`design/handoffs/voxel-browser-movement-2026-09-28.md`, 2):
//! yield, upkeep and lifespan per lineage; the file's optional lineage key; the search's
//! dimensions; the tightened plant persistence. No ticks.

use cubarium_voxel_fauna::{FaunaConfig, Founder};

use super::ecology::EcologyParams;
use super::search::species_lost;

#[test]
fn yield_genes_cannot_write_above_the_registry_ceiling() {
    let mut e = EcologyParams::legacy();
    e.set("founder.yield_by_food", 1.5).expect("in range");
    let decoded = e.fauna();
    for f in Founder::ALL {
        for food in cubarium_voxel_fauna::Food::ALL {
            let path = format!("fauna.founders.{}.yield_by_food.{}", f.name(), food.name());
            let (_, hi) = cubarium_voxel_fauna::registry()
                .row_for(&path)
                .expect("yield row")
                .search
                .expect("search ceiling");
            assert!(decoded.founder(f).yield_for(food) <= hi, "{path}");
        }
    }
}

fn scratch(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("cub-lineage-params-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("temp dir");
    dir.join(name)
}

/// A lineage-keyed multiplier reaches only that lineage; the shared key reaches both, and
/// the two multiply.
#[test]
fn a_lineage_key_reaches_only_its_lineage() {
    let base = FaunaConfig::default();
    let mut e = EcologyParams::legacy();
    e.set("founder.yield_by_food@frondgrazer", 1.5)
        .expect("in range");
    e.set("founder.maintenance_per_s@littershredder", 2.0)
        .expect("in range");
    e.set("founder.lifespan_median_s@frondgrazer", 0.5)
        .expect("in range");
    let c = e.fauna();
    let (b, s) = (Founder::Browser, Founder::Blind);
    assert_eq!(
        c.founder(b).yield_by_food[0],
        1.5 * base.founder(b).yield_by_food[0]
    );
    assert_eq!(c.founder(s).yield_by_food, base.founder(s).yield_by_food);
    assert_eq!(
        c.founder(s).core.maintenance_per_s,
        2.0 * base.founder(s).core.maintenance_per_s
    );
    assert_eq!(
        c.founder(b).core.maintenance_per_s,
        base.founder(b).core.maintenance_per_s
    );
    assert_eq!(
        c.founder(b).core.lifespan_median_s,
        0.5 * base.founder(b).core.lifespan_median_s
    );
    assert_eq!(
        c.founder(s).core.lifespan_median_s,
        base.founder(s).core.lifespan_median_s
    );
    // Shared and per-lineage multiply.
    e.set("founder.yield_by_food", 0.5).expect("in range");
    let c = e.fauna();
    assert!(
        (c.founder(b).yield_by_food[0] - 0.75 * base.founder(b).yield_by_food[0]).abs() < 1e-15
    );
    assert!(
        e.set("founder.yield_by_food@krill", 1.0).is_err(),
        "no such lineage"
    );
}

/// The file: old shared keys still load, and an `[ecology.<lineage>]` table sets that
/// lineage's multipliers; a save round-trips.
#[test]
fn the_file_takes_an_optional_lineage_key_and_old_files_still_load() {
    let old = scratch("old.toml");
    std::fs::write(
        &old,
        "[ecology]\n\"founder.yield_by_food\" = 0.9\n\"flora.senescence\" = 0.5\n",
    )
    .expect("write");
    let e = EcologyParams::load(&old).expect("an old file loads");
    assert_eq!(e.get("founder.yield_by_food"), Some(0.9));
    assert_eq!(e.get("founder.yield_by_food@frondgrazer"), Some(1.0));
    let new = scratch("new.toml");
    std::fs::write(
        &new,
        "[ecology]\n\"flora.senescence\" = 0.5\n[ecology.frondgrazer]\n\"founder.maintenance_per_s\" = 0.7\n",
    )
    .expect("write");
    let e = EcologyParams::load(&new).expect("a lineage table loads");
    assert_eq!(e.get("founder.maintenance_per_s@frondgrazer"), Some(0.7));
    assert_eq!(e.get("founder.maintenance_per_s@littershredder"), Some(1.0));
    let back = scratch("back.toml");
    e.save(&back, "round trip").expect("save");
    assert_eq!(EcologyParams::load(&back).expect("reload"), e);
}

/// Tightened plant persistence: a species the default world kept counts as lost when the
/// candidate lost it, or kept under a quarter of the default's stands of it (from four).
#[test]
fn a_species_is_lost_when_gone_or_far_below_the_default() {
    // start, candidate end, default end
    assert!(!species_lost(3, 2, 5), "kept, near the default");
    assert!(species_lost(3, 0, 5), "gone where the default kept it");
    assert!(!species_lost(3, 0, 0), "the default lost it too");
    assert!(!species_lost(0, 0, 5), "never started with it");
    assert!(species_lost(6, 3, 24), "under a quarter of 24");
    assert!(!species_lost(6, 6, 24), "a quarter of 24");
    assert!(
        !species_lost(2, 0, 0) && !species_lost(2, 1, 3),
        "under four stands, only a loss counts"
    );
}
