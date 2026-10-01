//! Parasite hatchlings (`design/handoffs/voxel-predation-parasite-2026-10-01.md`, "Census
//! columns"): the voxel census prints riders attached now, riders attached so far,
//! drop-offs and riders freed by a host's death. Red until the implementation commit.
//!
//! The census is an example binary with no library surface, so this reads its source: a
//! header column that is never printed is the regression this guards.

const CENSUS: &str = include_str!("../examples/voxel_census.rs");

#[test]
fn the_voxel_census_prints_the_parasite_columns() {
    for column in [
        "riders_now",
        "riders_attached",
        "rider_dropoffs",
        "riders_freed",
    ] {
        assert!(
            CENSUS.contains(&format!(",{column}")),
            "the census header has no `{column}` column"
        );
    }
    // The cumulative three come from the fauna ledger.
    for field in ["riders_attached", "rider_dropoffs", "riders_freed"] {
        assert!(
            CENSUS.contains(&format!(".{field}")),
            "the census row does not read the ledger's `{field}`"
        );
    }
}
