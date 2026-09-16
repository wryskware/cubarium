//! `run --fresh --neural <policy.json>`: the display's seeding door.
//!
//! The control exists so a trained forager can be *seen* on the cube. What it must get
//! right is narrow and checkable: the right number of neural animals in the world it
//! creates, and a refusal — not a second cohort — when a later run resumes that world.

mod support;

use cubarium_surface::{Scale, Topology};
use support::{Scratch, parse, run};

/// An exported policy file the runner can read, written the way `es-export` writes one.
/// Synthesised rather than copied from `runs/`, so the test does not depend on a training
/// run's output surviving in the tree.
fn write_policy(scratch: &Scratch, name: &str, seed: u64) -> std::path::PathBuf {
    // The ecology the policy claims: the shipped defaults, which is the world `--fresh`
    // creates here. Its hash is the one `es-export` records (`calibrate::config_hash`).
    write_policy_in(scratch, name, seed, "default", &cubarium_core::WorldConfig::default())
}

/// The same, claiming a named ecology: the label and hash `es-train --config` would record.
fn write_policy_in(
    scratch: &Scratch,
    name: &str,
    seed: u64,
    label: &str,
    config: &cubarium_core::WorldConfig,
) -> std::path::PathBuf {
    let theta = cubarium_search::es::tensor::initial_center(seed);
    let hash = cubarium_search::calibrate::config_hash(config);
    let file = cubarium_search::es::export::PolicyFile::new(&theta, "test", 0, 59, label, hash)
        .expect("an exportable centre");
    scratch.write(name, &serde_json::to_string(&file).expect("writing the policy file"))
}

/// A world config that is a different ecology from the defaults: one plant constant moved,
/// the way the calibration's `fast-leaf` moved it.
fn fast_leaf_like() -> cubarium_core::WorldConfig {
    let mut config = cubarium_core::WorldConfig::default();
    config.plant.foliage_rate = 0.006;
    config.validate().expect("a config the core accepts");
    config
}

fn refusal(args: &[&str]) -> String {
    format!(
        "{:#}",
        cubarium::run_world(&parse(args)).expect_err("the run must be refused")
    )
}

#[test]
fn a_fresh_seeded_world_holds_exactly_the_cohort_and_a_resume_refuses_to_seed_again() {
    let scratch = Scratch::new("neural-seed");
    let state = scratch.join("state");
    let policy = write_policy(&scratch, "center.json", 20_260_915);

    let seeded = run(&[
        "--sink", "none", "--speed", "0", "--seconds", "2", "--fresh",
        "--state", state.to_str().unwrap(),
        "--neural", policy.to_str().unwrap(),
        "--neural-count", "3",
    ]);
    assert_eq!(seeded.final_tick, 40);
    assert_eq!(
        seeded.neural_animals, 3,
        "three copies were asked for and three must be in the world"
    );
    assert!(
        seeded.population >= 3,
        "the legacy founders are there too: population {}",
        seeded.population
    );
    assert!(
        seeded.mass_residual.abs() < 1e-6,
        "founding a body from outside must still close the material box: {}",
        seeded.mass_residual
    );

    // The same state directory again, resuming, with the same control: refused by name.
    let err = format!(
        "{:#}",
        cubarium::run_world(&parse(&[
            "--sink", "none", "--speed", "0", "--seconds", "1",
            "--state", state.to_str().unwrap(),
            "--neural", policy.to_str().unwrap(),
        ]))
        .expect_err("a resumed world must not be seeded a second time")
    );
    assert!(err.contains("--neural seeds a new world"), "{err}");
    assert!(err.contains("--fresh"), "the refusal names the control that would work: {err}");

    // And the resume without the control carries the cohort it already has: the policy
    // rides the snapshot (schema 15), so a restart does not lose the animals.
    let resumed = run(&[
        "--sink", "none", "--speed", "0", "--seconds", "1",
        "--state", state.to_str().unwrap(),
    ]);
    assert_eq!(resumed.loaded_tick, Some(40), "the resume started from the seeded snapshot");
    assert_eq!(resumed.neural_animals, 3, "the cohort survived the snapshot round trip");
}

/// The default cohort is four, one per side face, and they are alive and moving after 20 s
/// of world time — the brief's "verify, do not assume".
#[test]
fn the_default_cohort_is_four_and_they_are_still_there_after_twenty_seconds() {
    let scratch = Scratch::new("neural-default");
    let state = scratch.join("state");
    let policy = write_policy(&scratch, "center.json", 7);

    let outcome = run(&[
        "--sink", "none", "--speed", "0", "--seconds", "20", "--fresh",
        "--state", state.to_str().unwrap(),
        "--neural", policy.to_str().unwrap(),
    ]);
    assert_eq!(outcome.final_tick, 400);
    assert_eq!(outcome.neural_animals, 4, "the documented default cohort");
}

/// A file that is not an exported policy stops the run before a world is written, rather
/// than starting an unseeded world that looks like the one that was asked for.
#[test]
fn an_unreadable_policy_file_refuses_the_run() {
    let scratch = Scratch::new("neural-bad");
    let state = scratch.join("state");
    let bad = scratch.write("not-a-policy.json", "{\"schema\":\"something-else\"}\n");

    let err = format!(
        "{:#}",
        cubarium::run_world(&parse(&[
            "--sink", "none", "--speed", "0", "--seconds", "1", "--fresh",
            "--state", state.to_str().unwrap(),
            "--neural", bad.to_str().unwrap(),
        ]))
        .expect_err("a policy file that will not parse must refuse the run")
    );
    assert!(err.contains("not-a-policy.json"), "the refusal names the file: {err}");
}

/// Present *and moving*: the seeded bodies are ordinary organisms driven by the policy, so
/// after 20 s of world time each of them has left the cell it was founded in. Built through
/// the same core door the runner uses, because `RunOutcome` reports counts and not
/// positions.
#[test]
fn every_seeded_animal_leaves_its_starting_cell() {
    use cubarium_core::{World, WorldConfig};
    use cubarium_surface::{CellId, Face, Vec2, cell_of};

    let theta = cubarium_search::es::tensor::initial_center(20_260_915);
    let policy = cubarium_search::es::tensor::policy(&theta).expect("a valid policy");

    let mut world = World::new(WorldConfig::default()).expect("the ordinary legacy world");
    let mut seeded = Vec::new();
    for k in 0..4u8 {
        let face = Face::from_index(k).expect("four side faces");
        let cell = CellId::new(Topology::Cube, Scale::ONE, face, 8, 8);
        let id = world
            .found_neural_animal(cell.center(Topology::Cube, Scale::ONE), Vec2::new(1.0, 0.0), policy.clone())
            .expect("a fresh world has room for four more");
        seeded.push((id, cell));
    }
    assert_eq!(world.neural_population(), 4);

    let mut visited: Vec<std::collections::BTreeSet<_>> =
        seeded.iter().map(|(_, c)| [*c].into_iter().collect()).collect();
    for _ in 0..400 {
        world.step();
        world.drain_events();
        for (i, (id, _)) in seeded.iter().enumerate() {
            if let Some(o) = world.state.organisms.get(*id) {
                visited[i].insert(cell_of(Topology::Cube, Scale::ONE, &o.pos));
            }
        }
    }
    world.check_invariants().expect("the world stays consistent");

    for (i, (id, start)) in seeded.iter().enumerate() {
        let o = world.state.organisms.get(*id).expect("alive after 20 s");
        assert!(
            visited[i].len() >= 2,
            "copy {i} never left {start:?}: {} cells visited, now at {:?}",
            visited[i].len(),
            cell_of(Topology::Cube, Scale::ONE, &o.pos)
        );
    }
}

/// `--neural` seeds the world `--config` describes, so the policy's recorded ecology must
/// be that config's: a policy trained in one ecology is refused, by name, against another —
/// the same refusal `es-evaluate` and `es-population` make. The digest check alone would
/// let a forager trained where foliage triples in a minute loose in the shipped defaults.
#[test]
fn a_policy_trained_in_another_ecology_is_refused_against_this_world() {
    let scratch = Scratch::new("neural-foreign-ecology");
    let state = scratch.join("state");
    let policy = write_policy_in(&scratch, "fast-leaf-center.json", 3, "fast-leaf", &fast_leaf_like());

    // No `--config`: the world is the shipped defaults, the policy is not.
    let err = refusal(&[
        "--sink", "none", "--speed", "0", "--seconds", "1", "--fresh",
        "--state", state.to_str().unwrap(),
        "--neural", policy.to_str().unwrap(),
    ]);
    assert!(err.contains("fast-leaf-center.json"), "the refusal names the file: {err}");
    assert!(err.contains("trained in ecology fast-leaf"), "and the ecology it claims: {err}");
    assert!(err.contains("config hash"), "and says what differs: {err}");
    assert!(support::snapshot_ticks(&state).is_empty(), "a refused run writes no world");

    // The other way round: a defaults policy against a `--config` that is a different ecology.
    let toml = toml::to_string(&fast_leaf_like()).expect("a config serialises");
    let config = scratch.write("fast-leaf.toml", &toml);
    let defaults_policy = write_policy(&scratch, "default-center.json", 3);
    let err = refusal(&[
        "--sink", "none", "--speed", "0", "--seconds", "1", "--fresh",
        "--state", state.to_str().unwrap(),
        "--config", config.to_str().unwrap(),
        "--neural", defaults_policy.to_str().unwrap(),
    ]);
    assert!(err.contains("trained in ecology default"), "{err}");
    assert!(err.contains("this evaluation is fast-leaf"), "the world's ecology is named by the file's stem: {err}");
}

/// A policy that records no ecology at all — every file written before the ecology was
/// part of the protocol — is "unknown", not "the defaults", and is refused for that reason.
#[test]
fn a_policy_without_a_recorded_ecology_is_refused() {
    let scratch = Scratch::new("neural-unknown-ecology");
    let state = scratch.join("state");
    let theta = cubarium_search::es::tensor::initial_center(5);
    let mut file = cubarium_search::es::export::PolicyFile::new(&theta, "test", 0, 59, "default", 0)
        .expect("an exportable centre");
    file.config = None;
    file.config_hash = None;
    let policy = scratch.write("old-center.json", &serde_json::to_string(&file).unwrap());

    let err = refusal(&[
        "--sink", "none", "--speed", "0", "--seconds", "1", "--fresh",
        "--state", state.to_str().unwrap(),
        "--neural", policy.to_str().unwrap(),
    ]);
    assert!(err.contains("old-center.json"), "{err}");
    assert!(err.contains("records no config hash"), "refused as unknown, not accepted as default: {err}");
}

/// The matching case with a named ecology: a policy trained under `--config <toml>` seeds a
/// world created from that same file, and `--seed` — an override applied after the config is
/// loaded, which `es-train` also applies per layout — is not part of the ecology's identity.
#[test]
fn a_policy_trained_in_the_configured_ecology_seeds_that_world() {
    let scratch = Scratch::new("neural-matching-ecology");
    let state = scratch.join("state");
    let toml = toml::to_string(&fast_leaf_like()).expect("a config serialises");
    let config = scratch.write("fast-leaf.toml", &toml);
    // Hash the config the way the runner and `es-train` both read it: from the file.
    let loaded = cubarium::runner::load_config(&config).expect("the file loads");
    let policy = write_policy_in(&scratch, "fast-leaf-center.json", 3, "fast-leaf", &loaded);

    let seeded = run(&[
        "--sink", "none", "--speed", "0", "--seconds", "1", "--fresh",
        "--state", state.to_str().unwrap(),
        "--config", config.to_str().unwrap(),
        "--seed", "7",
        "--neural", policy.to_str().unwrap(),
        "--neural-count", "2",
    ]);
    assert_eq!(seeded.neural_animals, 2, "the matching policy seeds its cohort");
    assert_eq!(seeded.config.seed, 7, "the seed override still applies");
    assert_eq!(seeded.config.plant.foliage_rate, 0.006, "in the configured ecology");
}
