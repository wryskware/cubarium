//! Pure view-state rules shared by the feed and the legacy presenter.
//! No world writes, rendering types or simulation policy.

use cubarium_voxel_fauna::{Animal, WallClimb};
use cubarium_voxel_flora::{FloraView, Ground, MAX_FOLIAGE_LAYERS, SpeciesConfig, Stand};

/// Legacy starving tint condition; presentation only, unchanged from animal.rs.
pub fn is_starving(animal: &Animal) -> bool {
    animal.reserve <= 0.001 && animal.body <= 0.015
}

/// Direction into the climbed wall: ahead on ascent, behind travel on descent.
pub fn wall_toward(climb: &WallClimb) -> (i32, i32) {
    let (dx, dz) = (i32::from(climb.dir.0), i32::from(climb.dir.1));
    if climb.ascending { (dx, dz) } else { (-dx, -dz) }
}

pub const WILT_LEVELS: u8 = 5;

/// The wilt level of a stand at moisture `μ`.
pub fn wilt_level(moisture: f64) -> u8 {
    let wilt = 1.0 - moisture.clamp(0.0, 1.0);
    let top = f64::from(WILT_LEVELS - 1);
    if wilt.is_finite() { (wilt * top).round().clamp(0.0, top) as u8 } else { 0 }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FilmMark {
    pub site: cubarium_voxel_flora::Site,
    pub strength: f32,
}

/// Every film site's floor mark, in site order ([`FilmMark`]).
pub fn film_marks(flora: FloraView<'_>) -> Vec<FilmMark> {
    let c = flora.config;
    let capacity = c.film_capacity_per_m2 * c.voxel_m * c.voxel_m;
    flora
        .film_sites()
        .map(|(site, film)| FilmMark {
            site,
            strength: if capacity > 0.0 { (film / capacity).clamp(0.0, 1.0) as f32 } else { 1.0 },
        })
        .collect()
}

/// Stock and capacity for each model foliage index, including whole-stand fallback
/// for an index absent from the current profile stage.
pub fn foliage_layers(stand: &Stand, sc: &SpeciesConfig) -> ([f64; MAX_FOLIAGE_LAYERS], [f64; MAX_FOLIAGE_LAYERS]) {
    let cap = sc.alpha * stand.wood.max(0.0);
    let mut stocks = [stand.foliage; MAX_FOLIAGE_LAYERS];
    let mut capacities = [cap; MAX_FOLIAGE_LAYERS];
    for (fi, (_, layer)) in sc.profile_at(stand.wood).foliage_layers().take(MAX_FOLIAGE_LAYERS).enumerate() {
        stocks[fi] = stand.layer_stock[fi];
        capacities[fi] = layer.share * cap;
    }
    (stocks, capacities)
}

pub fn foliage_fractions(stand: &Stand, sc: &SpeciesConfig) -> [f64; MAX_FOLIAGE_LAYERS] {
    let (stocks, capacities) = foliage_layers(stand, sc);
    std::array::from_fn(|i| if capacities[i] > 0.0 { stocks[i] / capacities[i] } else { 0.0 })
}

/// Parcel fullness, in propagule packages (may exceed one).
pub fn parcel_fullness(stand: &Stand, sc: &SpeciesConfig) -> f64 {
    let package = sc.propagule_package();
    if package > 0.0 { stand.parcel / package } else { 0.0 }
}

/// The warm accent's ripeness: nectar present, else fruit fullness, else seed parcel.
pub fn ripeness(stand: &Stand, sc: &SpeciesConfig) -> f64 {
    if sc.nectar_cap > 0.0 && sc.nectar_per_s > 0.0 {
        if stand.nectar > 0.0 { 1.0 } else { 0.0 }
    } else if sc.fruit_cap > 0.0 && sc.fruit_per_s > 0.0 {
        (stand.fruit / sc.fruit_cap).clamp(0.0, 1.0)
    } else {
        parcel_fullness(stand, sc)
    }
}

/// Whether a detritus pool crosses the legacy presenter's visible-stock threshold.
pub fn detritus_visible(stock: f64) -> bool {
    stock >= 0.05
}

/// The legacy ground marks: detritus visibility and the selected seed species.
pub fn ground_marks(g: &Ground, tick: u64, flora: &cubarium_voxel_flora::FloraConfig) -> [u8; 4] {
    let seed = g.seed_mark(tick, flora).map_or(0, |sp| sp.index() as u8 + 1);
    [
        u8::from(detritus_visible(g.dead_wood)),
        u8::from(detritus_visible(g.litter)),
        u8::from(detritus_visible(g.carrion)),
        seed,
    ]
}

pub mod vine;

/// The legacy warm-bloom opening threshold, in propagule packages.
pub const RIPE_AT: f64 = 0.5;
