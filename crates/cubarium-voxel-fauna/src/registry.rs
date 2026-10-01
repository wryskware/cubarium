//! The animal layer's parameter table (`fauna.*`, package R1): one row per numeric field of
//! [`crate::FaunaConfig`], mounted per lineage. The generic code is
//! [`cubarium_voxel::registry`].

use cubarium_voxel::registry::Registry;

/// The animal layer's table.
pub fn registry() -> &'static Registry {
    static TABLE: std::sync::OnceLock<Registry> = std::sync::OnceLock::new();
    TABLE.get_or_init(|| Registry::new("fauna", Vec::new(), Vec::new(), Vec::new()))
}
