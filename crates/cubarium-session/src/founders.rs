//! **Who drives the animals**: heuristics by default, or explicitly supplied policies,
//! installed over a whole lineage.

use std::path::PathBuf;

use anyhow::{Result, bail};
use cubarium_search::es::voxel::{EpisodeDriver, VoxelPolicyFile};
use cubarium_voxel_fauna::{Controller, Fauna, Founder};

/// Keep the seeded layer's heuristic controllers and announce each lineage's driver.
pub fn install_default_founders(fauna: &mut Fauna) -> Result<()> {
    install_founders_with(fauna, &[])
}

/// Install each explicitly supplied policy; other lineages keep their seeded heuristic.
/// Files validate their schema, manifest and lineage before installation.
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
        }
    }
    install_founder_controllers(fauna, &drivers)?;
    for founder in Founder::ALL {
        if let Some(i) = drivers.iter().position(|(f, _)| *f == founder) {
            eprintln!(
                "{} founders are driven by {} ({})",
                founder.name(),
                drivers[i].1.name(),
                sources[i]
            );
        } else {
            eprintln!(
                "{} founders are driven by heuristic (default)",
                founder.name()
            );
        }
    }
    Ok(())
}

/// Only explicitly named lineages are policy-driven. A loader passes these to
/// `cubarium_voxel_sim::saved::load` before installing their policies.
pub fn policy_founders(policies: &[(Founder, PathBuf)]) -> Vec<Founder> {
    Founder::ALL
        .into_iter()
        .filter(|f| policies.iter().any(|(p, _)| p == f))
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

/// Choose an explicitly supplied policy or the default heuristic for every lineage.
/// The legacy heuristic selection is accepted as a no-op; an explicit policy wins.
pub fn founder_recipes(
    policies: &[(Founder, PathBuf)],
    _heuristics: &[Founder],
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
        } else {
            out.push(FounderRecipe {
                founder,
                driver: None,
                source: "the observation-only heuristic (default)".to_string(),
            });
        }
    }
    Ok(out)
}

/// Install the ambient run's founder controllers over a whole layer: a lineage given a
/// driver has its **birth factory and every standing body of that lineage** switched to
/// it, and a lineage given none keeps the heuristic the seeder registered. One factory
/// per lineage, and `EpisodeDriver::fresh` per body, so no two bodies share a hidden
/// state. With no explicitly supplied policy, the heuristic remains the default.
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
                "this world's {} founders were driven by a saved policy; pass \
                 `--founder-policy {}=<centre.json>` to name it, or start a fresh \
                 world. They are not silently put back on the heuristic.",
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
