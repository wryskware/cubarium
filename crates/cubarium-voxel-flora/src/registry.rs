//! The plant layer's parameter table (`flora.*`, package R1): one row per numeric field of
//! [`crate::FloraConfig`], mounted per species. The generic code is
//! [`cubarium_voxel::registry`].

use cubarium_voxel::registry::Registry;

/// The plant layer's table.
pub fn registry() -> &'static Registry {
    static TABLE: std::sync::OnceLock<Registry> = std::sync::OnceLock::new();
    TABLE.get_or_init(|| Registry::new("flora", Vec::new(), Vec::new(), Vec::new()))
}
