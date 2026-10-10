//! ROLES5 (Wrysk 2026-10-10): the shipped frondgrazer gives at most one birth per world
//! day. The rule is read from each shipped preset, so the assertion is the relation Wrysk
//! asked for ("once a day birth rate"), not a tuned number.

use super::predation_fixture::*;
use crate::{Fauna, FaunaConfig, Founder, Reproduction, StartingStores};

const GRAZER: Founder = Founder::Browser;

/// The presets that found frondgrazers on a live world.
const SHIPPED: &[&str] = &[
    "config/desktop/terrarium-d72.toml",
    "config/desktop/terrarium-d96.toml",
    "config/desktop/terrarium.toml",
    "config/desktop/terrarium-wide.toml",
    "config/desktop/voxel.toml",
    "config/tachyon/voxel.toml",
    "config/voidrunner/terrarium-d72.toml",
    "config/voidrunner/terrarium-d96.toml",
    "config/voidrunner/terrarium.toml",
];

fn preset(path: &str) -> toml::Table {
    let full = format!("{}/../../{path}", env!("CARGO_MANIFEST_DIR"));
    let text = std::fs::read_to_string(&full).unwrap_or_else(|e| panic!("{full}: {e}"));
    text.parse::<toml::Table>().unwrap_or_else(|e| panic!("{full}: {e}"))
}

fn table<'a>(t: &'a toml::Table, path: &[&str]) -> Option<&'a toml::Table> {
    path.iter().try_fold(t, |t, k| t.get(*k)?.as_table())
}

/// The frondgrazer's rule as `doc` sets it over the code default, and the world day, s.
fn grazer_rule(doc: &toml::Table) -> (Reproduction, f64) {
    let base = FaunaConfig::default().founders[GRAZER.index()].core.reproduction;
    let mut value = toml::Table::try_from(base).expect("the rule as TOML");
    if let Some(set) = table(doc, &["fauna", "founders", "frondgrazer", "core", "reproduction"]) {
        for (k, v) in set {
            value.insert(k.clone(), v.clone());
        }
    }
    let rule: Reproduction = value.try_into().expect("the preset's rule");
    let day_min = table(doc, &["world", "day"])
        .and_then(|d| d.get("length_min"))
        .and_then(|v| v.as_float().or_else(|| v.as_integer().map(|i| i as f64)))
        .unwrap_or(cubarium_voxel::weather::DayClock::default().length_min);
    (rule, day_min * 60.0)
}

/// A frondgrazer kept fed past its every gate gives births at least a world day apart, on
/// every shipped preset, and does give them (so the gap is measured, not vacuous).
#[test]
fn a_shipped_frondgrazer_gives_at_most_one_birth_per_world_day() {
    let world = clock_off(510);
    for path in SHIPPED {
        let doc = preset(path);
        assert!(
            table(&doc, &["fauna", "founders", "frondgrazer"]).is_some(),
            "{path} founds no frondgrazer"
        );
        let (rule, day_s) = grazer_rule(&doc);
        let mut c = FaunaConfig::default();
        c.founders[GRAZER.index()].core.reproduction = rule;
        let mut f = Fauna::new(c);
        let parent = introduce(&mut f, &world, GRAZER, StartingStores::FULL);
        let day_ticks = (day_s / super::DT).round() as u64;
        let mut births = Vec::new();
        for tick in 0..3 * day_ticks + day_ticks / 2 {
            let index = f.animals.iter().position(|a| a.id == parent).expect("the parent");
            let sc = crate::body::effective_config(&f.config, &f.animals[index]);
            f.animals[index].reserve = sc.reproduction.package_cost(&sc) * 10.0 + sc.reproduction.surplus_floor;
            super::advance_eligibility(&mut f, index, &sc, &rule);
            let mut newborns = Vec::new();
            super::gestate(&mut f, index, &sc, &rule, (world.config().seed, None), &mut newborns);
            if !newborns.is_empty() {
                assert_eq!(newborns.len(), 1, "{path}: one calf a birth");
                births.push(tick);
            }
        }
        assert!(births.len() >= 2, "{path}: a fed frondgrazer gave {} births in 3.5 days", births.len());
        for w in births.windows(2) {
            assert!(
                w[1] - w[0] >= day_ticks,
                "{path}: births {} ticks apart, under the {day_ticks}-tick world day",
                w[1] - w[0]
            );
        }
    }
}
