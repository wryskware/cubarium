//! **Who drives the animals**: the trained centres a world installs by default, and the
//! one function that installs a lineage's driver over a whole layer.

use std::path::PathBuf;

use anyhow::{Context, Result, bail};
use cubarium_search::es::voxel::{EpisodeDriver, VoxelPolicyFile};
use cubarium_voxel_fauna::{Controller, Fauna, Founder};

/// The trained centres a world installs by default, carried inside the binary so the live
/// world needs no files on disk. Provenance, and the rule that a centre is replaced and
/// never edited, are in `crates/cubarium/assets/policies/README.md`.
pub const BUILT_IN_POLICIES: [(Founder, &str, &str); 2] = [
    (
        Founder::Blind,
        "littershredder-curio-c1-2026-10-02.json",
        include_str!("../../cubarium/assets/policies/littershredder-curio-c1-2026-10-02.json"),
    ),
    (
        Founder::Browser,
        "frondgrazer-curio-c1-2026-10-02.json",
        include_str!("../../cubarium/assets/policies/frondgrazer-curio-c1-2026-10-02.json"),
    ),
];

/// Parse one embedded centre into a driver, with the same checks [`VoxelPolicyFile::load`]
/// makes of a file on disk: the schema token first, then the file's own validation of
/// lineage, weight count, finite weights and this build's founder-manifest digest.
///
/// A centre that no longer matches this build's manifest fails here rather than being
/// reinterpreted, which is the point of shipping the trainer's own file untouched: the
/// binary refuses to pretend that weights trained against another schema are its default.
pub fn built_in_driver(founder: Founder) -> Result<EpisodeDriver> {
    let (_, name, json) = BUILT_IN_POLICIES
        .iter()
        .find(|(f, _, _)| *f == founder)
        .ok_or_else(|| anyhow::anyhow!("no built-in centre for the {} lineage", founder.name()))?;
    let file: VoxelPolicyFile = serde_json::from_str(json)
        .with_context(|| format!("the built-in centre {name} is not a policy file"))?;
    anyhow::ensure!(
        file.schema == cubarium_search::es::voxel::store::POLICY_SCHEMA,
        "the built-in centre {name}: schema `{}` is not {}",
        file.schema,
        cubarium_search::es::voxel::store::POLICY_SCHEMA,
    );
    let declared = file
        .founder()
        .map_err(|e| anyhow::anyhow!("the built-in centre {name}: {e}"))?;
    anyhow::ensure!(
        declared == founder,
        "the built-in centre {name} declares the {} lineage, not {}",
        declared.name(),
        founder.name(),
    );
    file.driver()
        .map_err(|e| anyhow::anyhow!("the built-in centre {name}: {e}"))
}

/// The built-in centre for `founder` if it validates against this build, else `None`
/// with a **loud** line on stderr: the lineage falls back to the observation-only
/// heuristic the seeder registered.
///
/// Contract v2 (`design/handoffs/voxel-retrain-2026-09-22.md`, P5-A item 10) moved both
/// founder manifests' digests, so the centres shipped under schema 1 are refused until the
/// retrain (P5-C) replaces them. A refused centre is never reinterpreted and never
/// silently swapped: the run says which lineage is on the fallback and why.
pub fn built_in_or_fallback(founder: Founder) -> Option<EpisodeDriver> {
    match built_in_driver(founder) {
        Ok(driver) => Some(driver),
        Err(e) => {
            eprintln!(
                "WARNING: {e:#}\nWARNING: the {} founders fall back to the observation-only \
                 heuristic: the built-in centre was trained against another founder manifest \
                 and is refused, not reinterpreted.",
                founder.name()
            );
            None
        }
    }
}

/// Install the ambient run's built-in trained centres on every founder lineage of an
/// already-seeded layer, and say so on stderr — one line per lineage naming its driver,
/// the same announcement `cubarium voxel`'s ambient run makes by default. An example that
/// wants to measure the shipped world's founders, and not the seeder's bare
/// observation-only heuristics, calls this once after `habitat::seed` (or
/// `habitat::install_heuristics` on an empty world), before stepping the sim.
///
/// A lineage whose built-in centre this build refuses keeps the seeder's heuristic, with
/// a loud line saying so ([`built_in_or_fallback`]).
pub fn install_default_founders(fauna: &mut Fauna) -> Result<()> {
    install_founders_with(fauna, &[])
}

/// [`install_default_founders`], with each lineage named in `policies` driven by that
/// saved policy file instead of the built-in centre (P5-C: `policy=<lineage>=<file>` on
/// the examples, so a candidate runs in the live tools). The file validates itself
/// against this build exactly as `--founder-policy` does, and a file declaring another
/// lineage than the one it is named for is refused.
pub fn install_founders_with(fauna: &mut Fauna, policies: &[(Founder, PathBuf)]) -> Result<()> {
    let mut drivers = Vec::with_capacity(Founder::ALL.len());
    let mut sources = Vec::with_capacity(Founder::ALL.len());
    for founder in Founder::ALL {
        if let Some((_, path)) = policies.iter().find(|(f, _)| *f == founder) {
            let file = VoxelPolicyFile::load(path).map_err(anyhow::Error::msg)?;
            let declared = file.founder().map_err(anyhow::Error::msg)?;
            anyhow::ensure!(
                declared == founder,
                "policy={}={}: the file declares the {} lineage",
                founder.name(),
                path.display(),
                declared.name(),
            );
            drivers.push((founder, file.driver().map_err(anyhow::Error::msg)?));
            sources.push(format!("the policy file {}", path.display()));
        } else if !BUILT_IN_POLICIES.iter().any(|(f, _, _)| *f == founder) {
            // No trained centre exists for this lineage (the lanternjaw, until P2): it
            // keeps the heuristic the seeder registered, and nothing is refused.
            continue;
        } else if let Some(driver) = built_in_or_fallback(founder) {
            drivers.push((founder, driver));
            sources.push("the built-in centre".to_string());
        }
    }
    install_founder_controllers(fauna, &drivers)?;
    for ((founder, driver), source) in drivers.iter().zip(&sources) {
        eprintln!(
            "{} founders are driven by {} ({source})",
            founder.name(),
            driver.name(),
        );
    }
    Ok(())
}

/// The lineages [`install_founders_with`] drives by a policy for these `policies`: each
/// one named, and every lineage with a built-in centre. What a loader passes to
/// `cubarium_voxel_sim::saved::load` before installing them.
pub fn policy_founders(policies: &[(Founder, PathBuf)]) -> Vec<Founder> {
    Founder::ALL
        .into_iter()
        .filter(|f| policies.iter().any(|(p, _)| p == f) || BUILT_IN_POLICIES.iter().any(|(b, _, _)| b == f))
        .collect()
}

/// The examples' `policy=<lineage>=<file>` arguments, in order: one per lineage at most.
pub fn policy_args(args: &[String]) -> Result<Vec<(Founder, PathBuf)>> {
    let mut out: Vec<(Founder, PathBuf)> = Vec::new();
    for spec in args.iter().filter_map(|a| a.strip_prefix("policy=")) {
        let Some((name, path)) = spec.split_once('=') else {
            bail!("policy= takes `<lineage>=<file>`, not `{spec}`");
        };
        let founder = cubarium_search::es::voxel::parse_founder(name)
            .map_err(|e| anyhow::anyhow!("policy={spec}: {e}"))?;
        anyhow::ensure!(!path.trim().is_empty(), "policy={spec} names no file");
        anyhow::ensure!(
            !out.iter().any(|(f, _)| *f == founder),
            "policy= names {} twice; one policy per lineage",
            founder.name()
        );
        out.push((founder, PathBuf::from(path.trim())));
    }
    Ok(out)
}

/// What one founder lineage of the ambient run is driven by, and how it was chosen. The
/// source is carried so the run can say it out loud: a world whose animals are a trained
/// policy and a world whose animals are a reflex look alike from outside.
#[derive(Clone)]
pub struct FounderRecipe {
    pub founder: Founder,
    /// `None` is the observation-only heuristic the seeder registered.
    pub driver: Option<EpisodeDriver>,
    pub source: String,
}

/// Choose each lineage's driver for the ambient run: a saved policy file first
/// (`--founder-policy`), then the heuristic (`--founder-heuristic`, the disclosed
/// control), and otherwise the trained centre built into the binary, which is the default
/// since Wrysk's 2026-09-20 decision — founders on these centres survived the generated
/// closed world where the heuristics died.
///
/// A policy file is the trainer's own and validates itself — schema token, lineage,
/// vector length, finite weights, and a digest that is that founder manifest's — so all
/// this adds is the cross-check the user's spelling makes possible: a browser centre asked
/// to drive the blind lineage is refused by name rather than run against a schema it was
/// never trained on.
pub fn founder_recipes(
    policies: &[(Founder, PathBuf)],
    heuristics: &[Founder],
) -> Result<Vec<FounderRecipe>> {
    let mut out = Vec::new();
    for founder in Founder::ALL {
        if let Some((_, path)) = policies.iter().find(|(f, _)| *f == founder) {
            let file = VoxelPolicyFile::load(path).map_err(anyhow::Error::msg)?;
            let declared = file.founder().map_err(anyhow::Error::msg)?;
            anyhow::ensure!(
                declared == founder,
                "--founder-policy {}={}: the file declares the {} lineage",
                founder.name(),
                path.display(),
                declared.name(),
            );
            out.push(FounderRecipe {
                founder,
                driver: Some(file.driver().map_err(anyhow::Error::msg)?),
                source: format!("--founder-policy {}", path.display()),
            });
        } else if heuristics.contains(&founder) {
            out.push(FounderRecipe {
                founder,
                driver: None,
                source: "--founder-heuristic, the disclosed control".to_string(),
            });
        } else if !BUILT_IN_POLICIES.iter().any(|(f, _, _)| *f == founder) {
            out.push(FounderRecipe {
                founder,
                driver: None,
                source: "no trained centre exists yet (predation P2)".to_string(),
            });
        } else {
            let name = BUILT_IN_POLICIES
                .iter()
                .find(|(f, _, _)| *f == founder)
                .map(|(_, n, _)| *n)
                .unwrap_or("none");
            let driver = built_in_or_fallback(founder);
            let source = if driver.is_some() {
                format!("the built-in centre {name}")
            } else {
                format!(
                    "the observation-only heuristic — FALLBACK: the built-in centre {name} \
                     is refused by this build's founder manifest"
                )
            };
            out.push(FounderRecipe {
                founder,
                driver,
                source,
            });
        }
    }
    Ok(out)
}

/// Install the ambient run's founder controllers over a whole layer: a lineage given a
/// driver has its **birth factory and every standing body of that lineage** switched to
/// it, and a lineage given none keeps the heuristic the seeder registered. One factory
/// per lineage, and `EpisodeDriver::fresh` per body, so no two bodies share a hidden
/// state. Since the trained centres became the default, "given none" means the run asked
/// for the control with `--founder-heuristic`.
///
/// A layer that *remembers* being policy-driven — [`Fauna::policy_driven`], which a
/// snapshot carries — and is given no driver for that lineage is **refused here**, not
/// quietly demoted to a heuristic: a world whose animals were a trained policy is not
/// the same world with the reflex put back, and silently substituting one is exactly
/// the re-anchoring `always-fresh-never-migrate` forbids.
///
/// The record the layer carries is per lineage and a [`Fauna::policy_digest`]
/// (`weights_fnv1a`, `0` for a heuristic), so what is refused is not only the demotion
/// but a swap of one centre for another: a loaded world whose recorded digest is nonzero
/// and does not match the digest of the driver offered for that lineage is refused by
/// name, the same re-anchoring the bare-demotion refusal above forbids.
pub fn install_founder_controllers(
    fauna: &mut Fauna,
    drivers: &[(Founder, EpisodeDriver)],
) -> Result<()> {
    for founder in Founder::ALL {
        if fauna.policy_driven(founder) && !drivers.iter().any(|(f, _)| *f == founder) {
            bail!(
                "this world's {} founders were driven by a saved policy; drop \
                 `--founder-heuristic {}` to run them on the built-in centre, or pass \
                 `--founder-policy {}=<centre.json>` to name one, or start a fresh \
                 world. They are not silently put back on the heuristic.",
                founder.name(),
                founder.name(),
                founder.name(),
            );
        }
    }
    for (founder, driver) in drivers {
        let digest = driver.digest();
        let recorded = fauna.policy_digest(*founder);
        if recorded != 0 && recorded != digest {
            bail!(
                "this world's {} founders were driven by a saved policy, centre digest \
                 {recorded:#018x}; the centre supplied here is a different one (digest \
                 {digest:#018x}). That is not the same world under a new centre, so the \
                 policy is refused rather than swapped in silence — supply the same \
                 centre, or start a fresh world.",
                founder.name(),
            );
        }
    }
    // The reflexes a centre falls back on run on the layer's own numbers.
    let teachers = fauna.config().teachers;
    for (founder, driver) in drivers {
        let digest = driver.digest();
        let driver = driver.clone();
        fauna.set_founder_factory(
            *founder,
            std::sync::Arc::new(move || -> Box<dyn Controller> { driver.fresh(teachers) }),
        );
        fauna.set_policy_driven(*founder, true);
        fauna.set_policy_digest(*founder, digest);
    }
    // Re-install over every standing founder body, so a loaded layer's bodies and the
    // bodies a policy flag took over are both driven by the lineage's current recipe.
    let bodies: Vec<(u64, Founder)> = fauna
        .view()
        .animals
        .iter()
        .filter_map(|a| a.founder.map(|f| (a.id, f)))
        .collect();
    for (id, founder) in bodies {
        if drivers.iter().any(|(f, _)| *f == founder) {
            anyhow::ensure!(
                fauna.install_founder_controller(id, founder),
                "the {} policy could not be installed on body {id}",
                founder.name()
            );
        }
    }
    Ok(())
}
