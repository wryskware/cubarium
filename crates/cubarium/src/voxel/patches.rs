//! **Connected patches of one plant on the ground** (MEADOW-3, 2026-10-06): Wrysk wants "at
//! least one continuous meadow patch in the display world", so the census reads the largest
//! connected patch of springturf (and of lanternberry groves and vaulttree forest) and how
//! many patches reach a size.
//!
//! A readout only: no sim rule reads it.
//!
//! - **Held faces:** the support faces ([`Site`]s) whose top plant place
//!   ([`FaceLayer::Plant`]) the species holds. A stand beneath a cover
//!   ([`FaceLayer::Under`]) holds no face of its own and is left out.
//! - **Surface adjacency:** two held faces touch when their columns are neighbours (one step
//!   in `x`, which wraps around the ring's `width`, or one step in `z`) and their heights
//!   differ by at most [`STEP_MAX`] voxels: a one-voxel stair is continuous ground, as it is
//!   for cover (`cover::stair_up`); a cliff or a ledge over a cave floor is not.
//! - **Drained ground** (the share's denominator, [`drained_ground`]): the whole-web model's
//!   drained zones (`design/webmodel/meadow.md`), read now rather than voted over snapshots:
//!   soil faces that are their column's highest (not under a roof), with terrain sky at
//!   least velvetpad's light gate (the model's `dim` band and up), where springturf's
//!   aeration and standing-water gates pass.

use std::collections::HashMap;

use cubarium_voxel::{Material, VoxelView};
use cubarium_voxel_flora::{FaceLayer, FloraConfig, Site, Species, Stand, establishment_gates_with_sky};

/// The largest height step between two faces of one patch, in voxels.
pub const STEP_MAX: u32 = 1;

/// One species' patches.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Patches {
    /// Faces held in all.
    pub faces: usize,
    /// Faces in the largest connected patch.
    pub largest: usize,
    /// Patches of at least the asked size.
    pub at_least: usize,
}

/// The faces `species` holds in the top plant place (not [`FaceLayer::Under`]), from a
/// flora view's `stands`.
pub fn held_faces(stands: &[Stand], species: Species) -> Vec<Site> {
    stands.iter().filter(|s| s.species == species && s.place == FaceLayer::Plant).map(|s| s.site).collect()
}

/// The faces of drained ground now (the module doc).
pub fn drained_ground(view: &VoxelView<'_>, config: &FloraConfig) -> usize {
    drained_ground_water(view, config).0
}

/// The faces of drained ground now, and the mean root-box water the turf reads on them
/// (available-water scale, [`establishment_gates_with_sky`]'s `mean_water`; 0 with no
/// drained face). The dry season's readout (DRY-SEASON).
pub fn drained_ground_water(view: &VoxelView<'_>, config: &FloraConfig) -> (usize, f64) {
    let c = view.config;
    let turf = config.species(Species::Springturf);
    let sky_min = config.species(Species::Velvetpad).establish_light_min;
    let mut n = 0;
    let mut water = 0.0;
    for z in 0..c.depth {
        for x in 0..c.width {
            let Some(y) = view.supports_in_column(i64::from(x), z).into_iter().max() else { continue };
            if view.material_at(i64::from(x), y, z) != Material::Soil {
                continue;
            }
            let sky = view.sky_visibility(i64::from(x), y, z);
            if sky < sky_min {
                continue;
            }
            let g = establishment_gates_with_sky(view, Site { x, y, z }, turf, sky, 0.0, 0.0);
            if g.aeration_ok && g.depth_ok {
                n += 1;
                water += g.mean_water.unwrap_or(0.0);
            }
        }
    }
    (n, if n > 0 { water / n as f64 } else { 0.0 })
}

/// The connected patches of `held` under surface adjacency on a ring `width` columns
/// round, counting those of at least `min_faces` faces. A face listed twice counts once.
pub fn patches(held: &[Site], width: u32, min_faces: usize) -> Patches {
    let width = width.max(1);
    let mut by_column: HashMap<(u32, u32), Vec<(u32, usize)>> = HashMap::new();
    let mut sites: Vec<Site> = held.to_vec();
    sites.sort_unstable();
    sites.dedup();
    for (i, s) in sites.iter().enumerate() {
        by_column.entry((s.x % width, s.z)).or_default().push((s.y, i));
    }
    let mut seen = vec![false; sites.len()];
    let mut out = Patches { faces: sites.len(), ..Patches::default() };
    let mut stack: Vec<usize> = Vec::new();
    for start in 0..sites.len() {
        if seen[start] {
            continue;
        }
        seen[start] = true;
        stack.push(start);
        let mut size = 0usize;
        while let Some(i) = stack.pop() {
            size += 1;
            let s = sites[i];
            let x = s.x % width;
            let mut cols: Vec<(u32, u32)> = vec![((x + 1) % width, s.z), ((x + width - 1) % width, s.z), (x, s.z + 1)];
            if s.z > 0 {
                cols.push((x, s.z - 1));
            }
            cols.sort_unstable();
            cols.dedup();
            for col in cols {
                if col == (x, s.z) {
                    continue;
                }
                let Some(list) = by_column.get(&col) else { continue };
                for &(y, j) in list {
                    if !seen[j] && y.abs_diff(s.y) <= STEP_MAX {
                        seen[j] = true;
                        stack.push(j);
                    }
                }
            }
        }
        out.largest = out.largest.max(size);
        if size >= min_faces {
            out.at_least += 1;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(x: u32, y: u32, z: u32) -> Site {
        Site { x, y, z }
    }

    /// A real stand, seeded by the flora layer on a one-column soil slab.
    fn a_stand(species: Species) -> Stand {
        use cubarium_voxel::{Command as WorldCommand, Config as VoxelConfig, Material, World};
        use cubarium_voxel_flora::{Command, Flora, FloraConfig};
        let mut w = World::empty(VoxelConfig { width: 1, height: 8, depth: 1, voxel_m: 1.0, seed: 5, ..VoxelConfig::default() });
        let pore = cubarium_voxel::Materials::DEFAULT.field_capacity(Material::Soil);
        let cap = w.config().materials.pore_capacity(Material::Soil) * w.config().voxel_volume();
        for y in 1..=2u32 {
            w.apply(WorldCommand::AddWater { x: 0, y, z: 0, volume_m3: pore * cap });
            w.apply(WorldCommand::SetMaterial { x: 0, y, z: 0, material: Material::Soil });
        }
        let mut flora = Flora::new(FloraConfig::default());
        let wood = flora.config().species(species).wood_max;
        assert!(flora.apply(&w, Command::SeedOnFace { site: s(0, 2, 0), species, wood }), "fixture: the layer took the stand");
        let st = *flora.view().stands.iter().find(|st| st.species == species).expect("fixture: the stand");
        assert_eq!(st.place, FaceLayer::Plant, "fixture: a founder holds the top place");
        st
    }

    /// Only the top plant place holds a face: a turf stand beneath a cover
    /// ([`FaceLayer::Under`]) is left out, and so is another species.
    #[test]
    fn a_stand_beneath_a_cover_holds_no_face() {
        let turf = a_stand(Species::Springturf);
        let at = |x: u32, place: FaceLayer| Stand { site: s(x, 2, 0), place, ..turf };
        let mut other = at(3, FaceLayer::Plant);
        other.species = Species::Velvetpad;
        let stands = [at(0, FaceLayer::Plant), at(1, FaceLayer::Under), at(2, FaceLayer::Plant), other];
        let held = held_faces(&stands, Species::Springturf);
        assert_eq!(held, vec![s(0, 2, 0), s(2, 2, 0)]);
        // The face beneath was the bridge: without it the two are separate patches.
        let p = patches(&held, 64, 1);
        assert_eq!((p.largest, p.at_least), (1, 2));
    }

    /// A 3 × 3 block and a separate pair: two patches, the largest nine faces; at least
    /// two faces counts both, at least three only the block.
    #[test]
    fn neighbouring_columns_join_and_a_gap_splits() {
        let mut held: Vec<Site> = (0..3).flat_map(|x| (0..3).map(move |z| s(10 + x, 5, z))).collect();
        held.extend([s(20, 5, 0), s(21, 5, 0)]);
        let p = patches(&held, 64, 2);
        assert_eq!((p.faces, p.largest, p.at_least), (11, 9, 2));
        assert_eq!(patches(&held, 64, 3).at_least, 1);
        // Diagonal columns do not touch.
        let p = patches(&[s(0, 5, 0), s(1, 5, 1)], 64, 1);
        assert_eq!((p.largest, p.at_least), (1, 2));
    }

    /// The ring wraps in `x`: the last column touches the first. Depth (`z`) does not wrap.
    #[test]
    fn the_ring_wraps_in_x_only() {
        let p = patches(&[s(63, 5, 2), s(0, 5, 2)], 64, 1);
        assert_eq!(p.largest, 2, "x = 63 and x = 0 are neighbours on a ring 64 wide");
        let p = patches(&[s(63, 5, 2), s(0, 5, 2)], 65, 1);
        assert_eq!(p.largest, 1, "not on a ring 65 wide");
        let p = patches(&[s(5, 5, 0), s(5, 5, 47)], 64, 1);
        assert_eq!(p.largest, 1, "depth does not wrap");
    }

    /// A stair of `STEP_MAX` joins; a taller step (a cliff, or a ledge over a floor)
    /// splits.
    #[test]
    fn a_stair_joins_and_a_cliff_splits() {
        let p = patches(&[s(0, 5, 0), s(1, 5 + STEP_MAX, 0), s(2, 5 + 2 * STEP_MAX, 0)], 64, 1);
        assert_eq!(p.largest, 3);
        let p = patches(&[s(0, 5, 0), s(1, 5 + STEP_MAX + 1, 0)], 64, 1);
        assert_eq!((p.largest, p.at_least), (1, 2));
        // A face listed twice counts once.
        assert_eq!(patches(&[s(0, 5, 0), s(0, 5, 0)], 64, 1).faces, 1);
    }
}
