//! Shared founding and relocation preference: roof, glowcap darkness, terrain sky.

use std::cmp::Ordering;

use cubarium_voxel::VoxelView;
use cubarium_voxel_flora::{Site, SpeciesConfig};

use crate::{DenChoice, Pack};

#[derive(Clone, Copy, Debug)]
pub struct DenQuality {
    pub choice: DenChoice,
    pub darkness: f64,
    pub sky: f64,
}

impl DenQuality {
    pub fn roofed(self) -> bool {
        self.choice != DenChoice::FoundingPoint
    }

    /// Best first: any roof beats open ground, then darker and less sky-visible sites.
    pub fn compare(self, other: Self) -> Ordering {
        other
            .roofed()
            .cmp(&self.roofed())
            .then(other.darkness.total_cmp(&self.darkness))
            .then(self.sky.total_cmp(&other.sky))
    }
}

pub fn den_quality(
    view: &VoxelView<'_>,
    glowcap: &SpeciesConfig,
    site: Site,
    pack: &Pack,
) -> DenQuality {
    let roof = (site.y + 1..view.config.height)
        .find(|&y| view.material_at(i64::from(site.x), y, site.z).is_solid());
    let choice = match roof {
        Some(y) if f64::from(y - site.y - 1) * view.config.voxel_m <= pack.den_cover_m => {
            DenChoice::Covered
        }
        Some(_) => DenChoice::MostOverhead,
        None => DenChoice::FoundingPoint,
    };
    let sky = view.sky_visibility(i64::from(site.x), site.y, site.z);
    DenQuality {
        choice,
        darkness: glowcap.darkness(sky),
        sky,
    }
}
