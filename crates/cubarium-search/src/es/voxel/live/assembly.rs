//! **The E0 and E1 start recipes**: stubs, tests first.

use cubarium_voxel::{Config, World};
use cubarium_voxel_fauna::{Fauna, Founder};
use cubarium_voxel_flora::{Flora, Site};
use serde::{Deserialize, Serialize};

use super::ecology::EcologyParams;
use super::start::Manifest;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct E1Recipe {
    pub plant_hours: f64,
    pub consumer_multiplier: f64,
    pub juvenile_share: f64,
    pub juvenile_body: f64,
    pub prey_per_chorister: f64,
    pub pack_min: usize,
    pub pack_max: usize,
    pub max_bodies: usize,
    pub max_attempts: usize,
}

impl Default for E1Recipe {
    fn default() -> E1Recipe {
        E1Recipe {
            plant_hours: 4.0,
            consumer_multiplier: 3.0,
            juvenile_share: 0.25,
            juvenile_body: 0.5,
            prey_per_chorister: 10.0,
            pack_min: 2,
            pack_max: 6,
            max_bodies: 400,
            max_attempts: 800,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Placement {
    pub lineage: String,
    pub wanted: usize,
    pub attempted: usize,
    pub placed: usize,
    pub failed: usize,
    pub unplaceable: usize,
    pub capped: usize,
    pub juveniles: usize,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct PackPlacement {
    pub prey_bodies: usize,
    pub rule: String,
    pub size: usize,
    pub placed: usize,
    pub failed: Option<String>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Injected {
    pub organic: f64,
    pub mineral: f64,
    pub energy: f64,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct FaunaReport {
    pub bodies: Vec<Placement>,
    pub pack: PackPlacement,
    pub eggs: usize,
    pub eggs_failed: Option<String>,
    pub injected: Injected,
    pub attempts: usize,
}

pub fn construct_fauna(
    _world: &World,
    _flora: &mut Flora,
    _fauna: &mut Fauna,
    _recipe: &E1Recipe,
    _today: [usize; Founder::COUNT],
    _sites: &[Site],
    _wettest: &[f64],
) -> FaunaReport {
    todo!()
}

pub struct Built {
    pub manifest: Manifest,
    pub world: World,
    pub flora: Flora,
    pub fauna: Fauna,
}

pub fn e0_start(_world: &Config, _eco: &EcologyParams, _seed: u64, _policy: Vec<String>) -> Result<Built, String> {
    todo!()
}
