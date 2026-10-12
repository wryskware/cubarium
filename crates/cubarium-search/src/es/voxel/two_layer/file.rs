//! The two-layer policy file (§10.4): a versioned format of its own. A plain brain keeps
//! today's `cub-voxel-policy-1` file ([`VoxelPolicyFile`]) unchanged, and loads into the
//! two-layer runtime as a plain shape over the same vector.

use std::path::Path;
use std::sync::Arc;

use cubarium_voxel_fauna::Founder;
use serde::{Deserialize, Serialize};

use super::brain::TwoLayerNet;
use super::shape::{TwoLayerLayout, TwoLayerShape};
use crate::es::bits::hex_f64s;
use crate::es::voxel::store::{POLICY_SCHEMA, VoxelPolicyFile};
use crate::es::voxel::{parse_founder, voxel_schema_digest};

/// The two-layer policy file's schema token. A file that is not this is refused by name.
pub const TWO_LAYER_POLICY_SCHEMA: &str = "cub-voxel-policy-two-layer-1";

/// One two-layer centre, self-contained.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TwoLayerPolicyFile {
    /// [`TWO_LAYER_POLICY_SCHEMA`].
    pub schema: String,
    pub build: String,
    /// The founder lineage the weights were authored against.
    pub founder: String,
    /// That founder's manifest digest when the file was written: the observation layout
    /// and actions the bottom reads and writes.
    pub digest: u64,
    pub train_seed: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub generation: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub score: Option<f64>,
    /// How the weights came to be (`clone`, `search`, …). Recorded, not validated.
    #[serde(default)]
    pub origin: String,
    /// The brain's dimensions; the bottom's must match the founder's manifest.
    pub shape: TwoLayerShape,
    /// The exact weights, `[bottom GRU | top]` ([`super::shape`]'s layout).
    #[serde(with = "hex_f64s")]
    pub theta: Vec<f64>,
}

impl TwoLayerPolicyFile {
    /// A file for `founder` from validated weights.
    pub fn new(founder: Founder, net: &TwoLayerNet, train_seed: u64, origin: &str) -> Self {
        TwoLayerPolicyFile {
            schema: TWO_LAYER_POLICY_SCHEMA.into(),
            build: env!("CARGO_PKG_VERSION").into(),
            founder: founder.name().into(),
            digest: voxel_schema_digest(founder),
            train_seed,
            generation: None,
            score: None,
            origin: origin.into(),
            shape: net.shape().clone(),
            theta: net.theta().to_vec(),
        }
    }

    pub fn write(&self, path: &Path) -> Result<(), String> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("cannot create {}: {e}", parent.display()))?;
        }
        let json = serde_json::to_string_pretty(self).map_err(|e| e.to_string())?;
        std::fs::write(path, json).map_err(|e| format!("cannot write {}: {e}", path.display()))
    }

    /// Read and validate.
    pub fn load(path: &Path) -> Result<TwoLayerPolicyFile, String> {
        let bytes =
            std::fs::read(path).map_err(|e| format!("cannot read {}: {e}", path.display()))?;
        let file: TwoLayerPolicyFile =
            serde_json::from_slice(&bytes).map_err(|e| format!("{}: {e}", path.display()))?;
        file.validate(&path.display().to_string())?;
        Ok(file)
    }

    /// The schema token, the founder, a bottom matching its manifest, the manifest
    /// digest, a non-plain shape, the vector's length and finiteness. Refused by name.
    pub fn validate(&self, name: &str) -> Result<(), String> {
        if self.schema != TWO_LAYER_POLICY_SCHEMA {
            return Err(format!(
                "{name}: schema `{}` is not {TWO_LAYER_POLICY_SCHEMA}",
                self.schema
            ));
        }
        let founder = parse_founder(&self.founder).map_err(|e| format!("{name}: {e}"))?;
        let manifest = founder.manifest();
        let plain = TwoLayerShape::plain_from_manifest(&manifest);
        let s = &self.shape;
        if s.observation != plain.observation
            || s.hidden != plain.hidden
            || s.actions != plain.actions
            || s.legacy_options != plain.legacy_options
        {
            return Err(format!(
                "{name}: the bottom (I={}, H={}, actions={}, options={}) is not {}'s manifest \
                 (I={}, H={}, actions={}, options={})",
                s.observation,
                s.hidden,
                s.actions.len(),
                s.legacy_options,
                founder.name(),
                plain.observation,
                plain.hidden,
                plain.actions.len(),
                plain.legacy_options
            ));
        }
        if self.digest != voxel_schema_digest(founder) {
            return Err(format!(
                "{name}: policy digest {:#018x} is not this build's {} manifest digest {:#018x}",
                self.digest,
                founder.name(),
                voxel_schema_digest(founder)
            ));
        }
        if s.is_plain() {
            return Err(format!(
                "{name}: a plain brain keeps today's {POLICY_SCHEMA} file; this format is for \
                 brains with a top or discrete heads"
            ));
        }
        TwoLayerNet::new(s.clone(), self.theta.clone())
            .map(|_| ())
            .map_err(|e| format!("{name}: {e}"))
    }

    /// The validated network.
    pub fn net(&self) -> Result<Arc<TwoLayerNet>, String> {
        self.validate("policy")?;
        Ok(Arc::new(TwoLayerNet::new(self.shape.clone(), self.theta.clone())?))
    }

    /// The layout of this file's vector.
    pub fn layout(&self) -> TwoLayerLayout {
        TwoLayerLayout::new(&self.shape)
    }
}

/// A plain file as a two-layer network: the founder's plain shape over the file's own
/// vector, unchanged. The file is validated as every plain load is.
pub fn plain_net(file: &VoxelPolicyFile) -> Result<Arc<TwoLayerNet>, String> {
    let founder = file.founder()?;
    let shape = TwoLayerShape::plain_from_manifest(&founder.manifest());
    Ok(Arc::new(TwoLayerNet::new(shape, file.theta.clone())?))
}

/// Either policy format.
#[derive(Clone, Debug, PartialEq)]
pub enum BrainFile {
    /// Today's `cub-voxel-policy-1` file: a plain brain.
    Plain(VoxelPolicyFile),
    /// A [`TWO_LAYER_POLICY_SCHEMA`] file.
    TwoLayer(TwoLayerPolicyFile),
}

impl BrainFile {
    /// Load either format, by its schema token. A plain file goes through
    /// [`VoxelPolicyFile::load`] exactly as today.
    pub fn load(path: &Path) -> Result<BrainFile, String> {
        #[derive(Deserialize)]
        struct Token {
            schema: String,
        }
        let bytes =
            std::fs::read(path).map_err(|e| format!("cannot read {}: {e}", path.display()))?;
        let token: Token =
            serde_json::from_slice(&bytes).map_err(|e| format!("{}: {e}", path.display()))?;
        match token.schema.as_str() {
            POLICY_SCHEMA => VoxelPolicyFile::load(path).map(BrainFile::Plain),
            TWO_LAYER_POLICY_SCHEMA => TwoLayerPolicyFile::load(path).map(BrainFile::TwoLayer),
            other => Err(format!(
                "{}: schema `{other}` is neither {POLICY_SCHEMA} nor {TWO_LAYER_POLICY_SCHEMA}",
                path.display()
            )),
        }
    }

    /// The validated network this file runs.
    pub fn net(&self) -> Result<Arc<TwoLayerNet>, String> {
        match self {
            BrainFile::Plain(f) => plain_net(f),
            BrainFile::TwoLayer(f) => f.net(),
        }
    }
}
