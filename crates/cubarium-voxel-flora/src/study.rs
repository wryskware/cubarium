//! **Study probes** (velvetpad niche study, `design/handoffs/velvetpad-study-2026-10-03.md`):
//! diagnostics that drive the plant layer's own functions under conditions a caller holds
//! fixed. Nothing in a world tick calls this module.
//!
//! - [`probe`]: one stand's §4 growth ([`crate::step::grow_stand`], the tick's own loop
//!   body) and its donor funding (`seeds::propagate`'s first half, restated below), under a
//!   held root-box water reading, a held light and a held site mineral, through a recorded
//!   day clock. The single-stand fixture of the viability grid.
//! - [`canopy_at`]: the foliage transmission over a face at a receiver height, the shade
//!   model's sweep (`step::light_per_stand`) for a point.

use cubarium_voxel::World;

use crate::step::{Booked, Climate, Drink, Substrate, grow_stand, light_response};
use crate::{FloraConfig, FloraView, Ground, Site, Species, Stage, Stand};

/// One tick of the world's clock as a plant reads it (`step::Climate`).
#[derive(Clone, Copy, Debug)]
pub struct ClimateSample {
    pub daylight: f64,
    pub upkeep: f64,
    pub night: f64,
    pub to_dawn_s: Option<f64>,
}

impl ClimateSample {
    /// What `world` asks of a plant this tick, at `config`'s `upkeep_q10`.
    pub fn of(world: &World, config: &FloraConfig) -> ClimateSample {
        let c = Climate::of(world, config.upkeep_q10);
        ClimateSample {
            daylight: c.daylight,
            upkeep: c.upkeep,
            night: c.night,
            to_dawn_s: c.to_dawn_s,
        }
    }

    fn climate(self) -> Climate {
        Climate {
            daylight: self.daylight,
            upkeep: self.upkeep,
            night: self.night,
            to_dawn_s: self.to_dawn_s,
            draw_key: 0,
        }
    }
}

/// How the probe's stand starts.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Start {
    /// Full grown: `wood_max`, full foliage (`alpha · W`), reserve at `donor_reserve_floor`
    /// of its cap (a donor that has been spending down to its floor).
    Adult,
    /// One minimum seed package (`seed_mass · alive_min / w_frac`), split as a germination
    /// splits it: the stand a gap gets.
    Newborn,
}

/// The conditions one probe holds, read every tick.
#[derive(Clone, Copy, Debug)]
pub struct Held {
    /// Root-box mean available water (1 = field capacity): `μ` is the species' ramp of it.
    pub water: f64,
    /// Root-box saturated fraction: what aeration stress closes on.
    pub saturated: f64,
    /// Raw light at the crown, `sky · canopy transmission`, before the species' response.
    pub light: f64,
    /// The site's mineral pool, reset to this every tick.
    pub mineral: f64,
}

/// What one probe measured.
#[derive(Clone, Copy, Debug, Default)]
pub struct Outcome {
    pub alive: bool,
    /// Seconds from the start to death, when it died.
    pub died_s: Option<f64>,
    pub wood: [f64; 2],
    pub foliage: [f64; 2],
    pub reserve: [f64; 2],
    /// Gross fixation, organic units, over the run.
    pub fixed: f64,
    /// Organic matter set aside for propagules (net of construction), over the run.
    pub funded: f64,
    /// Whole packages the parcel filled (each would leave as a seed or a runner).
    pub packages: u32,
    /// Seconds from the start to `wood ≥ donor_min`, when it got there.
    pub donor_s: Option<f64>,
    /// Least wood seen.
    pub min_wood: f64,
}

impl Outcome {
    /// Net carbon balance over the run: the change in standing material plus what was
    /// set aside for propagules (output, not loss). Litter shed and respiration are the
    /// losses already inside the change.
    pub fn net(&self) -> f64 {
        (self.wood[1] + self.foliage[1] + self.reserve[1]) - (self.wood[0] + self.foliage[0] + self.reserve[0])
            + self.funded
    }
}

/// The stand a probe starts from, on a stand-alone site.
pub fn start_stand(config: &FloraConfig, species: Species, start: Start) -> Stand {
    let sc = config.species(species);
    let (wood, foliage, reserve) = match start {
        Start::Adult => (
            sc.wood_max,
            sc.alpha * sc.wood_max,
            sc.donor_reserve_floor * sc.reserve_cap * sc.wood_max,
        ),
        Start::Newborn => crate::seeds::newborn_stocks(sc, crate::seeds::package_of(sc)),
    };
    let mut s = Stand {
        id: 0,
        site: Site { x: 0, y: 1, z: 0 },
        species,
        stage: Stage::Alive,
        wood,
        foliage,
        reserve,
        light: 0.0,
        moisture: 0.0,
        water_m3: 0.0,
        mineral: sc.n_tissue * (wood + foliage + reserve),
        aeration_stress: 0.0,
        parcel: 0.0,
        layer_stock: [0.0; crate::MAX_FOLIAGE_LAYERS],
        profile_stage: 0,
        defence: 0.0,
        nectar: 0.0,
        fruit: 0.0,
        fruit_age_s: 0.0,
        fruit_rest_s: 0.0,
    };
    s.bin_foliage(sc, config.voxel_m);
    s
}

/// Grows `stand` through `climate` (one sample per tick) under `held(tick)`: the tick's
/// own growth step, then the donor's funding as `seeds::propagate` takes it, a full parcel
/// leaving as one package. Nothing lands: the probe measures the stand, not its offspring.
pub fn probe(
    config: &FloraConfig,
    mut stand: Stand,
    climate: &[ClimateSample],
    held: impl Fn(usize) -> Held,
) -> Outcome {
    let sc = config.species(stand.species);
    let mut out = Outcome {
        alive: true,
        wood: [stand.wood; 2],
        foliage: [stand.foliage; 2],
        reserve: [stand.reserve; 2],
        min_wood: stand.wood,
        ..Outcome::default()
    };
    if stand.wood >= sc.donor_min {
        out.donor_s = Some(0.0);
    }
    let mut g = Ground::new(stand.site, 0.0);
    let build = 1.0 + sc.build;
    let package = crate::seeds::package_of(sc);
    for (t, c) in climate.iter().enumerate() {
        let h = held(t);
        g.mineral = h.mineral;
        let drink = Drink {
            moisture: sc.moisture_at(h.water),
            taken_m3: 0.0,
            saturated: h.saturated,
            darkness: 0.0,
        };
        let mut book = Booked::default();
        let died = grow_stand(
            config,
            &mut stand,
            &mut g,
            light_response(sc, h.light),
            c.climate(),
            drink,
            Substrate::default(),
            &mut book,
        );
        out.fixed += book.fixed;
        if died {
            out.alive = false;
            out.died_s = Some((t + 1) as f64 * crate::DT);
            break;
        }
        if stand.wood >= sc.donor_min {
            let ask = (sc.propagule_rate * crate::DT).max(0.0);
            let floor = sc.donor_reserve_floor * sc.reserve_cap * stand.wood;
            let take = (stand.reserve - floor).max(0.0).min(ask).min(stand.reserve.max(0.0));
            if take > 0.0 {
                stand.reserve -= take;
                stand.parcel += take / build;
                out.funded += take / build;
            }
            if out.donor_s.is_none() {
                out.donor_s = Some((t + 1) as f64 * crate::DT);
            }
        }
        if package > 0.0 && stand.parcel >= package {
            stand.parcel -= package;
            out.packages += 1;
        }
        out.min_wood = out.min_wood.min(stand.wood);
    }
    out.wood[1] = stand.wood;
    out.foliage[1] = stand.foliage;
    out.reserve[1] = stand.reserve;
    out
}

/// The share of open light that reaches height `receiver_top_v` (voxels above the world
/// floor) over `site`, through every foliage layer whose top is above it and whose disc
/// covers the column — the shade model's own factor, `exp(−k (1 − p) stock / area)` per
/// layer — skipping the stand that holds `site` itself. Terrain sky is not in it.
pub fn canopy_at(view: &FloraView<'_>, width: u32, depth: u32, site: Site, receiver_top_v: f64) -> f64 {
    let config = view.config;
    let voxel_m = config.voxel_m;
    let (x, z) = (site.x as f64, site.z as f64);
    let w = f64::from(width);
    let wrapped = |a: f64, b: f64| {
        let d = (a - b).rem_euclid(w);
        if d > w / 2.0 { d - w } else { d }
    };
    let mut here = 1.0;
    let mut visit = |s: &Stand| {
        if s.site == site {
            return;
        }
        for l in crate::layer_iter(config, s, voxel_m) {
            if !l.kind.bears_foliage() || l.band_v[1] <= receiver_top_v {
                continue;
            }
            let dx = wrapped(s.site.x as f64, x);
            let dz = s.site.z as f64 - z;
            if dx * dx + dz * dz > l.radius_v * l.radius_v {
                continue;
            }
            here *= (-config.shade_k_per_m2 * (1.0 - l.porosity) * l.stock / l.area_m2).exp();
        }
    };
    match view.stands_near(site.x as i64, site.z, width, depth) {
        Some(near) => near.iter().for_each(|&i| visit(&view.stands[i as usize])),
        None => view.stands.iter().for_each(visit),
    }
    here
}
