//! The search genome's generic decode (R3: `design/ecology-parameter-registry-2026-09-28.md`
//! §3–6; `design/handoffs/cotrain-stage0-2026-10-01.md` §2). No ticks.

use cubarium_voxel::genome::Genome;
use cubarium_voxel::registry;
use cubarium_voxel_fauna::Founder;
use cubarium_voxel_flora::Species;

use crate::params::{Params, load_genome};

const LEGACY: &str = include_str!("../../../config/training/search/legacy.toml");
const COTRAIN: &str = include_str!("../../../config/training/search/cotrain.toml");

fn genome(text: &str) -> Genome {
    load_genome(text).unwrap_or_else(|e| panic!("a valid search file: {e}\n{text}"))
}

fn decoded(g: &Genome) -> Params {
    let mut p = Params::defaults();
    p.apply_genome(g).unwrap_or_else(|e| panic!("decodes: {e}"));
    p
}

/// Every numeric leaf of the plant and animal configs, by path.
fn leaves(p: &Params) -> Vec<(String, f64)> {
    let mut out = cubarium_voxel_flora::registry()
        .numeric_leaves(&registry::to_value(&p.flora).expect("serializes"));
    out.extend(
        cubarium_voxel_fauna::registry()
            .numeric_leaves(&registry::to_value(&p.fauna).expect("serializes")),
    );
    out
}

/// The leaves whose value differs between `a` and `b`: `(path, a, b)`.
fn changed(a: &Params, b: &Params) -> Vec<(String, f64, f64)> {
    let (la, lb) = (leaves(a), leaves(b));
    assert_eq!(la.len(), lb.len(), "the same leaves");
    la.into_iter()
        .zip(lb)
        .filter(|((_, x), (_, y))| x != y)
        .map(|((p, x), (_, y))| (p, x, y))
        .collect()
}

fn close(a: f64, b: f64) -> bool {
    (a - b).abs() <= 1e-12 * a.abs().max(b.abs()).max(1e-300)
}

/// Decoding a shipped search file with every gene at x1 gives today's configs, in every
/// crate: the world's, the plants' and the animals'.
#[test]
fn every_shipped_search_file_decodes_at_x1_to_todays_configs() {
    for (name, text) in [("legacy", LEGACY), ("cotrain", COTRAIN)] {
        let g = genome(text);
        assert!(!g.genes.is_empty(), "{name} has genes");
        for gene in &g.genes {
            assert_eq!(gene.value, 1.0, "{name}: {} starts at x1", gene.name);
        }
        let p = decoded(&g);
        let d = Params::defaults();
        assert_eq!(p.world, d.world, "{name}: the world");
        assert_eq!(p.flora, d.flora, "{name}: the plants");
        assert_eq!(p.fauna, d.fauna, "{name}: the animals");
    }
}

/// Stage 1's search is a few meaningful knobs, not the registry: 20–48 genes (registry §4: "about 20–40"), every one
/// a multiplier whose box holds x1.
#[test]
fn the_cotrain_search_is_a_few_dozen_multipliers_around_today() {
    let g = genome(COTRAIN);
    assert!(
        (20..=48).contains(&g.genes.len()),
        "{} genes: {:?}",
        g.genes.len(),
        g.names()
    );
    for gene in &g.genes {
        let (lo, hi) = gene.range;
        assert!(
            lo > 0.0 && lo <= 1.0 && hi >= 1.0,
            "{}: [{lo}, {hi}]",
            gene.name
        );
    }
    assert!(
        !g.retrain,
        "stage 1 does not move a trained controller's digest"
    );
}

/// A gene moves exactly the leaves its path matches, by its multiplier, and nothing else:
/// one leaf, a segment group, and a per-lineage split.
#[test]
fn a_gene_moves_exactly_the_paths_it_matches() {
    let d = Params::defaults();

    let one = decoded(&genome(
        r#"
[[gene]]
name = "turf_growth"
path = "flora.springturf.foliage_rate"
range = [0.25, 4.0]
value = 2.0
"#,
    ));
    let diff = changed(&d, &one);
    assert_eq!(diff.len(), 1, "{diff:?}");
    assert_eq!(diff[0].0, "flora.springturf.foliage_rate");
    assert!(close(diff[0].2, 2.0 * diff[0].1));

    // A segment group: every photosynthetic species' leaf fall, and nothing else.
    let group = decoded(&genome(
        r#"
[[gene]]
name = "leaf_fall"
path = "flora.@photo.senescence"
range = [0.25, 4.0]
value = 0.5
"#,
    ));
    let diff = changed(&d, &group);
    let photo: Vec<String> = Species::ALL
        .into_iter()
        .filter(|s| d.flora.species(*s).trophic == cubarium_voxel_flora::Trophic::Photo)
        .filter(|s| d.flora.species(*s).senescence > 0.0)
        .map(|s| format!("flora.{}.senescence", s.name()))
        .collect();
    let mut moved: Vec<String> = diff.iter().map(|c| c.0.clone()).collect();
    moved.sort();
    let mut want = photo.clone();
    want.sort();
    assert_eq!(moved, want);
    for (_, a, b) in &diff {
        assert!(close(*b, 0.5 * a));
    }

    // `tie = "each"`: one gene per lineage, each moving only its own lineage's leaf.
    let mut each = genome(
        r#"
[[gene]]
name = "lifespan"
path = "fauna.founders.*.core.lifespan_median_s"
tie = "each"
range = [0.25, 8.0]
"#,
    );
    let names = each.names();
    for f in Founder::ALL {
        assert!(
            names.contains(&format!("lifespan@{}", f.name())),
            "{names:?}"
        );
    }
    each.set("lifespan@lanternjaw", 3.0).expect("in range");
    let diff = changed(&d, &decoded(&each));
    assert_eq!(diff.len(), 1, "{diff:?}");
    assert_eq!(
        diff[0].0,
        "fauna.founders.lanternjaw.core.lifespan_median_s"
    );
    assert!(close(diff[0].2, 3.0 * diff[0].1));
    assert!(
        each.set("lifespan@lanternjaw", 9.0).is_err(),
        "outside the gene's range"
    );
}

/// A derived field follows its base gene — walking and the mouth keep their ratio to
/// upkeep, an absolute body quantity its mass scale, a seed bank's attrition its age —
/// and stays put when the search frees it, by path or by the tie's name. A gene on the
/// derived field itself multiplies on top.
#[test]
fn a_derived_field_follows_its_base_gene_and_stays_put_when_freed() {
    let d = Params::defaults();
    let motor = |p: &Params, f: Founder| p.fauna.founder(f).motor_respiration_per_s;
    let bite = |p: &Params, f: Founder| p.fauna.founder(f).core.bite_per_s;
    let upkeep = r#"
[[gene]]
name = "upkeep"
path = "fauna.founders.frondgrazer.core.maintenance_per_s"
range = [0.25, 4.0]
value = 2.0
"#;
    let follows = decoded(&genome(upkeep));
    assert!(close(
        motor(&follows, Founder::Browser),
        2.0 * motor(&d, Founder::Browser)
    ));
    assert!(close(
        bite(&follows, Founder::Browser),
        2.0 * bite(&d, Founder::Browser)
    ));
    assert_eq!(motor(&follows, Founder::Blind), motor(&d, Founder::Blind));

    let freed = decoded(&genome(&format!(
        "free = [\"fauna.founders.frondgrazer.motor_respiration_per_s\"]\n{upkeep}"
    )));
    assert_eq!(
        motor(&freed, Founder::Browser),
        motor(&d, Founder::Browser),
        "freed by path"
    );
    assert!(close(
        bite(&freed, Founder::Browser),
        2.0 * bite(&d, Founder::Browser)
    ));

    let by_name = decoded(&genome(&format!(
        "free = [\"intake_follows_upkeep\"]\n{upkeep}"
    )));
    assert_eq!(
        bite(&by_name, Founder::Browser),
        bite(&d, Founder::Browser),
        "freed by name"
    );
    assert!(close(
        motor(&by_name, Founder::Browser),
        2.0 * motor(&d, Founder::Browser)
    ));

    let on_top = decoded(&genome(&format!(
        r#"{upkeep}
[[gene]]
name = "motor_factor"
path = "fauna.founders.frondgrazer.motor_respiration_per_s"
range = [0.5, 10.0]
value = 3.0
"#
    )));
    assert!(close(
        motor(&on_top, Founder::Browser),
        6.0 * motor(&d, Founder::Browser)
    ));

    // The mass scale (a digest row, so only with `retrain`) carries the eight absolute
    // quantities with it, and the placeholder species follows the browser's body.
    let heavy = decoded(&genome(
        r#"
retrain = true
[[gene]]
name = "mass"
path = "fauna.founders.frondgrazer.mass_scale"
range = [0.1, 10.0]
value = 2.0
"#,
    ));
    let (a, b) = (
        d.fauna.founder(Founder::Browser).core,
        heavy.fauna.founder(Founder::Browser).core,
    );
    assert!(close(b.body_max, 2.0 * a.body_max) && close(b.birth_cost, 2.0 * a.birth_cost));
    assert!(close(
        heavy.fauna.frondgrazer.body_max,
        2.0 * d.fauna.frondgrazer.body_max
    ));
    assert_eq!(
        b.maintenance_per_s, a.maintenance_per_s,
        "a per-unit rate stays"
    );

    // An inverse tie: a seed bank kept twice as long loses its seeds half as fast.
    let banked = decoded(&genome(
        r#"
[[gene]]
name = "dormancy"
path = "flora.springturf.seed_max_age_s"
range = [0.25, 4.0]
value = 2.0
"#,
    ));
    let (s0, s1) = (
        d.flora.species(Species::Springturf),
        banked.flora.species(Species::Springturf),
    );
    assert!(close(
        s1.seed_attrition_per_s,
        0.5 * s0.seed_attrition_per_s
    ));
}

/// The clock gene `K_bio` runs every biological process faster together: rates up,
/// durations down, the derived fields with their bases, neutral at x1.
#[test]
fn the_clock_gene_speeds_rates_and_shortens_durations() {
    let mut g = genome(COTRAIN);
    assert!(g.names().iter().any(|n| n == "K_bio"), "{:?}", g.names());
    g.set("K_bio", 2.0).expect("in range");
    let (d, p) = (Params::defaults(), decoded(&g));
    for f in Founder::ALL {
        let (a, b) = (d.fauna.founder(f), p.fauna.founder(f));
        assert!(
            close(b.core.maintenance_per_s, 2.0 * a.core.maintenance_per_s),
            "{f:?}"
        );
        assert!(
            close(b.motor_respiration_per_s, 2.0 * a.motor_respiration_per_s),
            "{f:?}"
        );
        assert!(
            close(b.core.lifespan_median_s, 0.5 * a.core.lifespan_median_s),
            "{f:?}"
        );
    }
    let (a, b) = (
        d.flora.species(Species::Bloomcrown),
        p.flora.species(Species::Bloomcrown),
    );
    assert!(close(b.foliage_rate, 2.0 * a.foliage_rate));
    assert!(close(b.defence_half_life_s, 0.5 * a.defence_half_life_s));
    assert!(close(p.flora.decomposition, 2.0 * d.flora.decomposition));
    assert!(close(
        p.flora.wood_decomposition,
        2.0 * d.flora.wood_decomposition
    ));
}

/// An unknown path, group or key in a search file is an error that names it.
#[test]
fn an_unknown_path_in_a_search_file_is_an_error() {
    for path in [
        "fauna.founders.frondgrazer.core.no_such_field",
        "flora.no_such_species.alpha",
        "flora.@no_such_group.alpha",
        "nowhere.at.all",
    ] {
        let text = format!("[[gene]]\nname = \"g\"\npath = \"{path}\"\nrange = [0.5, 2.0]\n");
        let err = load_genome(&text).expect_err(path);
        assert!(
            err.contains(path) || err.contains("no_such") || err.contains("nowhere"),
            "{err}"
        );
    }
    let typo = "[[gene]]\nname = \"g\"\npath = \"flora.decomposition\"\nrnage = [0.5, 2.0]\n";
    assert!(load_genome(typo).is_err(), "an unknown key in a gene");
    let free = "free = [\"no_such_tie\"]\n[[gene]]\nname = \"g\"\npath = \"flora.decomposition\"\nrange = [0.5, 2.0]\n";
    assert!(load_genome(free).is_err(), "an unknown free entry");
}

/// A `digest` row — a quantity a trained brain's sense contract hashes — named as a gene
/// is refused unless the file says `retrain = true`.
#[test]
fn a_digest_row_named_as_a_gene_is_refused() {
    for path in [
        "fauna.founders.frondgrazer.mass_scale",
        "fauna.founders.frondgrazer.adult_length_m",
        "fauna.founders.*.cruise_body_lengths_per_s",
        "fauna.founders.littershredder.mouth_ceiling_fraction",
        "fauna.cue.pool_bulk_density",
    ] {
        let text = format!("[[gene]]\nname = \"g\"\npath = \"{path}\"\nrange = [0.5, 2.0]\n");
        let err = load_genome(&text).expect_err(path);
        assert!(err.contains("digest"), "{path}: {err}");
        assert!(
            load_genome(&format!("retrain = true\n{text}")).is_ok(),
            "{path} with retrain"
        );
    }
}

/// One gene on both a derived field and its base would count twice: refused.
#[test]
fn a_gene_on_a_field_and_its_base_together_is_refused() {
    let text = r#"
[[gene]]
name = "both"
path = ["fauna.founders.frondgrazer.core.maintenance_per_s", "fauna.founders.frondgrazer.motor_respiration_per_s"]
range = [0.5, 2.0]
"#;
    assert!(load_genome(text).is_err());
}

/// A genome round-trips through its file: the values come back, the decode is the same.
#[test]
fn a_genome_file_round_trips() {
    let mut g = genome(COTRAIN);
    let names = g.names();
    for (k, n) in names.iter().enumerate() {
        let (lo, hi) = g.genes[k].range;
        g.set(n, (lo * hi).sqrt()).expect("in range");
    }
    let back = genome(&g.to_toml("a test genome"));
    assert_eq!(back.names(), names);
    for (a, b) in g.genes.iter().zip(&back.genes) {
        assert!(close(a.value, b.value), "{}", a.name);
    }
    assert_eq!(decoded(&g), decoded(&back));
}

/// Roster R0: **a lineage a world founds none of has no gene.** With the lanternjaw's
/// `founding_count` at 0, a `tie = "each"` over `fauna.founders.*` splits over the other
/// two lineages only; the shipped table (every lineage founded) still splits over all.
#[test]
fn a_lineage_at_founding_count_zero_has_no_gene() {
    let text = r#"
[[gene]]
name = "upkeep"
path = "fauna.founders.*.core.maintenance_per_s"
tie = "each"
range = [0.5, 2.0]
"#;
    let file = || cubarium_voxel::genome::SearchFile::parse(text).expect("a search file");
    let shipped = Params::defaults().expand_genome(file()).expect("expands");
    assert_eq!(shipped.names().len(), Founder::COUNT, "{:?}", shipped.names());
    let mut p = Params::defaults();
    p.fauna.founders[Founder::Lanternjaw.index()].founding_count = 0;
    let names = p.expand_genome(file()).expect("expands").names();
    assert!(!names.contains(&"upkeep@lanternjaw".to_string()), "{names:?}");
    for f in [Founder::Blind, Founder::Browser] {
        assert!(names.contains(&format!("upkeep@{}", f.name())), "{names:?}");
    }
}
