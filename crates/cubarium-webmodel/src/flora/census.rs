//! **The face census**: one record per support face of a saved world, read through the
//! plant layer's own predicates, so the model's zones, their areas and the initial
//! occupancy come from the sim's state and not from a guess.
//!
//! A save is `world.bin` + `flora.bin` (a staged checkpoint or a study save). The animals
//! are not read: the census is the producers' and the ground's. Reading a save with the
//! build that shares its schemas is not a migration; a save of another schema is refused by
//! the decoders themselves.

use std::path::Path;

use cubarium_voxel::{Material, World};
use cubarium_voxel_flora::{Flora, FloraConfig, Species, Trophic};

/// The photosynthetic species, in [`Species::ALL`] order (the glowcap left out).
pub const PHOTO: [Species; 8] = [
    Species::Bloomcrown,
    Species::Umbrellafrond,
    Species::Springturf,
    Species::Stonecushion,
    Species::Velvetpad,
    Species::Vaulttree,
    Species::Lanternberry,
    Species::Siphonreed,
];

/// One stand's state as the census reads it.
#[derive(Clone, Copy, Debug, Default)]
pub struct StandRec {
    pub wood: f64,
    pub foliage: f64,
    pub reserve: f64,
    /// `L_eff` the stand earned on last tick (after the species' light response).
    pub light: f64,
    /// `μ` it saw last tick.
    pub moisture: f64,
    pub aeration_stress: f64,
    pub cured: bool,
    /// Edible foliage: above every layer's grazing floor.
    pub edible: f64,
    /// The stand's mineral inventory (`Stand::mineral`).
    pub mineral: f64,
}

/// One support face.
#[derive(Clone, Debug)]
pub struct Face {
    pub x: u32,
    pub y: u32,
    pub z: u32,
    pub soil: bool,
    /// Soil voxels in the column from the face's voxel down, unbroken (at most
    /// [`SOIL_DEPTH_MAX`]): how deep a root could go before rock (the meadow lever e).
    pub soil_depth: u32,
    /// The column's highest support face: where a dropped, blown or spore seed lands
    /// (`seeds.rs:1299`); a face under a roof gets runners and dark spores only.
    pub highest: bool,
    /// Terrain sky visibility (no canopy).
    pub sky: f64,
    /// For each photo species ([`PHOTO`] order): whether its water gates (pore, aeration,
    /// flooding, standing water) pass, and whether its terrain light gate passes.
    pub water_ok: [bool; 8],
    pub light_ok: [bool; 8],
    /// Springturf's root-box mean available water and saturated fraction (`None`: no
    /// plant-available band), velvetpad's saturated fraction, the standing water depth.
    pub turf_water: Option<f64>,
    pub turf_saturated: f64,
    pub pad_saturated: f64,
    pub depth_m: f64,
    /// The glowcap's darkness here and whether its gates pass with the ground's substrate.
    pub darkness: f64,
    pub glow_ok: bool,
    /// Canopy transmission at a ground cover's crown top (springturf's full height), from
    /// every other stand's foliage above it.
    pub canopy: f64,
    pub plant: Option<(Species, StandRec)>,
    pub fungus: Option<StandRec>,
    pub litter: f64,
    pub dead_wood: f64,
    pub mineral: f64,
    pub film: f64,
    /// Mineral in the litter, the dead wood and the seed bank.
    pub litter_mineral: f64,
    pub dead_wood_mineral: f64,
    pub seed_mineral: f64,
    /// Each photo species' root-box mean available water (`None`: no band) and saturated
    /// fraction ([`PHOTO`] order): what its `μ`, its gates and its aeration stress read.
    pub water: [Option<f64>; 8],
    pub saturated: [f64; 8],
    /// Whole banked seeds per species ([`Species::ALL`] order).
    pub seeds: [u32; Species::COUNT],
    /// Per photo species ([`PHOTO`] order): the optical depth over its **newborn's** crown
    /// top from every other site's foliage above it, split by the shading species
    /// ([`Species::ALL`] order): `exp(−Σ τ)` is the canopy transmission the canopy gate
    /// (`step::canopy_pays`, `step.rs:3899`) reads, terrain sky not in it.
    pub nb_tau: [[f32; Species::COUNT]; 8],
    /// The same over the crown top of the plant stand holding the face (zero when bare),
    /// split by shading species: what its light (`StandRec::light`) was cut by.
    pub pl_tau: [f32; Species::COUNT],
}

impl Face {
    /// A bare soil face with nothing on it: a fixture's starting point.
    pub fn bare(x: u32, y: u32, z: u32, sky: f64, mineral: f64) -> Face {
        Face {
            x,
            y,
            z,
            soil: true,
            soil_depth: SOIL_DEPTH_MAX,
            highest: true,
            sky,
            water_ok: [true; 8],
            light_ok: [true; 8],
            turf_water: Some(1.0),
            turf_saturated: 0.0,
            pad_saturated: 0.0,
            depth_m: 0.0,
            darkness: 0.0,
            glow_ok: true,
            canopy: 1.0,
            plant: None,
            fungus: None,
            litter: 0.0,
            dead_wood: 0.0,
            mineral,
            film: 0.0,
            litter_mineral: 0.0,
            dead_wood_mineral: 0.0,
            seed_mineral: 0.0,
            water: [Some(1.0); 8],
            saturated: [0.0; 8],
            seeds: [0; Species::COUNT],
            nb_tau: [[0.0; Species::COUNT]; 8],
            pl_tau: [0.0; Species::COUNT],
        }
    }
}

/// The deepest soil run [`Face::soil_depth`] counts.
pub const SOIL_DEPTH_MAX: u32 = 32;

/// Read a save's world and plant layer.
pub fn read_save(dir: &Path) -> Result<(World, Flora), String> {
    let read = |n: &str| std::fs::read(dir.join(n)).map_err(|e| format!("{}/{n}: {e}", dir.display()));
    let world = World::load(&read("world.bin")?).map_err(|e| format!("world: {e:#}"))?;
    let flora = cubarium_voxel_flora::snapshot::decode(&read("flora.bin")?).map_err(|e| format!("flora: {e:#}"))?;
    Ok((world, flora))
}

fn rec(fv: &cubarium_voxel_flora::FloraView<'_>, s: &cubarium_voxel_flora::Stand) -> StandRec {
    StandRec {
        wood: s.wood,
        foliage: s.foliage,
        reserve: s.reserve,
        light: s.light,
        moisture: s.moisture,
        aeration_stress: s.aeration_stress,
        cured: s.cured,
        edible: fv.layers(s).map(|l| l.edible()).sum(),
        mineral: s.mineral,
    }
}

/// Every support face of `world`, under `config`'s predicates (the save's own when `None`).
pub fn census(world: &World, flora: &Flora, config: Option<&FloraConfig>) -> Vec<Face> {
    let view = world.view();
    let c = view.config;
    let fv = flora.view();
    let config = config.unwrap_or(fv.config);
    let turf = config.species(Species::Springturf);
    let top = turf.crown_height(turf.wood_max, c.voxel_m);
    let nb_height: [f64; 8] = std::array::from_fn(|k| {
        let sc = config.species(PHOTO[k]);
        sc.crown_height(crate::flora::stand::StandState::newborn(sc, crate::flora::stand::package_of(sc)).wood, c.voxel_m)
    });
    let shade = Shade::new(&fv, c.width, c.depth, c.voxel_m);
    let mut out = Vec::new();
    for z in 0..c.depth {
        for x in 0..c.width {
            let supports = view.supports_in_column(x as i64, z);
            let col_top = supports.iter().copied().max();
            for y in supports {
                let site = cubarium_voxel_flora::Site { x, y, z };
                let sky = view.sky_visibility(x as i64, y, z);
                let mut water_ok = [false; 8];
                let mut light_ok = [false; 8];
                let mut turf_water = None;
                let mut water = [None; 8];
                let mut saturated = [0.0; 8];
                let (mut turf_sat, mut pad_sat, mut depth) = (0.0, 0.0, 0.0);
                for (k, sp) in PHOTO.iter().enumerate() {
                    let g = cubarium_voxel_flora::establishment_gates_with_sky(&view, site, config.species(*sp), sky, 0.0, 0.0);
                    water_ok[k] = g.pore_ok && g.aeration_ok && g.depth_ok && g.standing_ok;
                    light_ok[k] = g.light_ok;
                    water[k] = g.mean_water;
                    saturated[k] = g.saturated_fraction;
                    if *sp == Species::Springturf {
                        turf_water = g.mean_water;
                        turf_sat = g.saturated_fraction;
                        depth = g.water_depth_m;
                    }
                    if *sp == Species::Velvetpad {
                        pad_sat = g.saturated_fraction;
                    }
                }
                let glow = fv.establishment_gates(&view, site, Species::Glowcap);
                let gsc = config.species(Species::Glowcap);
                let (mut litter, mut dead_wood, mut mineral, mut film) = (0.0, 0.0, config.initial_mineral, 0.0);
                let (mut litter_mineral, mut dead_wood_mineral, mut seed_mineral) = (0.0, 0.0, 0.0);
                let mut seeds = [0u32; Species::COUNT];
                if let Some(g) = fv.ground_at(site) {
                    litter = g.litter;
                    dead_wood = g.dead_wood;
                    mineral = g.mineral;
                    film = g.film;
                    litter_mineral = g.litter_mineral;
                    dead_wood_mineral = g.dead_wood_mineral;
                    seed_mineral = g.seeds.iter().map(|c| c.mineral).sum();
                    for cohort in &g.seeds {
                        let p = config.species(cohort.species).propagule_package();
                        if p > 0.0 {
                            seeds[cohort.species.index()] += (cohort.organic / p).round() as u32;
                        }
                    }
                }
                let tops: [f64; 8] = std::array::from_fn(|k| f64::from(y) + nb_height[k]);
                let nb_tau = shade.tau(&fv, site, &tops);
                let pl_tau = match fv.stand_at(site) {
                    Some(st) => shade.tau(&fv, site, &[f64::from(y) + config.species(st.species).crown_height(st.wood, c.voxel_m); 8])[0],
                    None => [0.0; Species::COUNT],
                };
                let plant = fv.stand_at(site).map(|s| (s.species, rec(&fv, s)));
                let fungus = fv.fungus_at(site).filter(|s| config.species(s.species).trophic == Trophic::Saprotroph).map(|s| rec(&fv, s));
                out.push(Face {
                    x,
                    y,
                    z,
                    soil: view.material_at(x as i64, y, z) == Material::Soil,
                    soil_depth: (0..=y.min(SOIL_DEPTH_MAX - 1)).take_while(|&d| view.material_at(x as i64, y - d, z) == Material::Soil).count() as u32,
                    highest: Some(y) == col_top,
                    sky,
                    water_ok,
                    light_ok,
                    turf_water,
                    turf_saturated: turf_sat,
                    pad_saturated: pad_sat,
                    depth_m: depth,
                    darkness: gsc.darkness(sky),
                    glow_ok: glow.passes(),
                    canopy: (-shade.tau(&fv, site, &[f64::from(y) + top; 8])[0].iter().map(|&v| f64::from(v)).sum::<f64>()).exp(),
                    plant,
                    fungus,
                    litter,
                    dead_wood,
                    mineral,
                    film,
                    litter_mineral,
                    dead_wood_mineral,
                    seed_mineral,
                    water,
                    saturated,
                    seeds,
                    nb_tau,
                    pl_tau,
                });
            }
        }
    }
    out
}

/// The CSV header of [`to_csv`].
pub fn csv_header() -> String {
    let mut h = String::from(
        "x,y,z,soil,sky,water_ok,light_ok,turf_water,turf_sat,pad_sat,depth,dark,glow_ok,canopy,plant,pw,pp,pq,pl,pmu,pstress,pcured,ped,pmin,fw,fp,fq,fmin,litter,dead_wood,mineral,film,litter_min,dead_wood_min,seed_min",
    );
    for sp in PHOTO {
        h += &format!(",w_{0},sat_{0}", sp.name());
    }
    for sp in Species::ALL {
        h += &format!(",seed_{}", sp.name());
    }
    for sp in PHOTO {
        h += &format!(",nbc_{}", sp.name());
    }
    h
}

fn bits(b: &[bool; 8]) -> String {
    b.iter().map(|&v| if v { '1' } else { '0' }).collect()
}

/// One CSV line per face.
pub fn to_csv(f: &Face) -> String {
    let (pn, p) = match f.plant {
        Some((s, r)) => (s.name(), r),
        None => ("-", StandRec::default()),
    };
    let fu = f.fungus.unwrap_or_default();
    let mut line = format!(
        "{},{},{},{},{:.4},{},{},{},{:.4},{:.4},{:.4},{:.4},{},{:.4},{},{:.5},{:.5},{:.5},{:.4},{:.4},{:.4},{},{:.5},{:.6},{:.5},{:.5},{:.5},{:.6},{:.5},{:.5},{:.5},{:.5},{:.6},{:.6},{:.6}",
        f.x,
        f.y,
        f.z,
        u8::from(f.soil),
        f.sky,
        bits(&f.water_ok),
        bits(&f.light_ok),
        f.turf_water.map_or("NA".into(), |w| format!("{w:.4}")),
        f.turf_saturated,
        f.pad_saturated,
        f.depth_m,
        f.darkness,
        u8::from(f.glow_ok),
        f.canopy,
        pn,
        p.wood,
        p.foliage,
        p.reserve,
        p.light,
        p.moisture,
        p.aeration_stress,
        u8::from(p.cured),
        p.edible,
        p.mineral,
        fu.wood,
        fu.foliage,
        fu.reserve,
        fu.mineral,
        f.litter,
        f.dead_wood,
        f.mineral,
        f.film,
        f.litter_mineral,
        f.dead_wood_mineral,
        f.seed_mineral,
    );
    for k in 0..8 {
        line += &format!(",{},{:.4}", f.water[k].map_or("NA".into(), |w| format!("{w:.4}")), f.saturated[k]);
    }
    for n in f.seeds {
        line += &format!(",{n}");
    }
    for t in f.nb_tau {
        line += &format!(",{:.4}", (-t.iter().map(|&v| f64::from(v)).sum::<f64>()).exp());
    }
    line
}

/// Every stand's foliage layers indexed by column, for the newborn canopy reading.
struct Shade {
    /// Per column `x * depth + z`: the shading layers of the stands rooted there:
    /// `(site, species index, top, radius, optical depth)`.
    cols: Vec<Vec<(cubarium_voxel_flora::Site, usize, f64, f64, f64)>>,
    reach: i64,
    width: u32,
    depth: u32,
}

impl Shade {
    fn new(fv: &cubarium_voxel_flora::FloraView<'_>, width: u32, depth: u32, voxel_m: f64) -> Shade {
        let config = fv.config;
        let mut cols = vec![Vec::new(); (width * depth) as usize];
        let mut reach = 0.0f64;
        for s in fv.stands {
            for l in cubarium_voxel_flora::layer_iter(config, s, voxel_m) {
                if !l.kind.bears_foliage() {
                    continue;
                }
                // The shade model's own factor, `exp(−k (1 − p) stock / area)` (`lib.rs:2857`).
                let tau = config.shade_k_per_m2 * (1.0 - l.porosity) * l.stock / l.area_m2;
                reach = reach.max(l.radius_v);
                cols[(s.site.x * depth + s.site.z) as usize].push((s.site, s.species.index(), l.band_v[1], l.radius_v, tau));
            }
        }
        Shade { cols, reach: reach.ceil() as i64 + 1, width, depth }
    }

    /// `study::canopy_at`'s sweep for several receiver tops at once, split by shader.
    fn tau(&self, _fv: &cubarium_voxel_flora::FloraView<'_>, site: cubarium_voxel_flora::Site, tops: &[f64; 8]) -> [[f32; Species::COUNT]; 8] {
        let mut out = [[0.0f32; Species::COUNT]; 8];
        let w = i64::from(self.width);
        for dx in -self.reach..=self.reach {
            let x = (i64::from(site.x) + dx).rem_euclid(w) as u32;
            for dz in -self.reach..=self.reach {
                let z = i64::from(site.z) + dz;
                if z < 0 || z >= i64::from(self.depth) {
                    continue;
                }
                for &(s, sp, top, r, tau) in &self.cols[(x * self.depth + z as u32) as usize] {
                    if s == site || (dx * dx + dz * dz) as f64 > r * r {
                        continue;
                    }
                    for k in 0..8 {
                        if top > tops[k] {
                            out[k][sp] += tau as f32;
                        }
                    }
                }
            }
        }
        out
    }
}
