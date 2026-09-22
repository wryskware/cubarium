//! Edible stock: how much of the standing foliage could a browser actually eat?
//!
//! ```text
//! cargo run --release -p cubarium --example voxel_edible_stock -- 6 preset=small
//! ```
//!
//! The census (`voxel_census`) prints counts and pooled totals; the plant autopsy steps
//! no fauna and knows no preset; the founder autopsy measures dying bodies against the
//! nearest crown. None of them can say what share of the foliage is *reachable*, *seen*
//! and *route-connected* — so "low food is absent / unreachable / unseen / not acquired"
//! stayed undecided after four diagnoses
//! (`design/7_Research/organism-systems-audit-2026-09-21.md` §2, §10 order 0).
//!
//! This steps the shipped world exactly as the census does — same preset arm, same lake
//! gate, same seeded habitat, same built-in trained centres — and at `t = 0` and every
//! thirty simulated minutes prints three CSV blocks measuring the chain:
//!
//! 1. `stock`, per plant species: stands, foliage organic, and the organic that is
//!    reachable, visible and route-connected, each under **today's implemented rules**
//!    and under the **decided** band and fan.
//! 2. `lineage`, per founder: bodies alive, their standing-height distribution, mean
//!    distance to the nearest reachable stand, and the share with one inside 2 m.
//! 3. `detritus`: litter, carrion and glowcap cap organic, and the share of each lying
//!    in a walkable component that holds a living shredder.
//!
//! Nothing here steps, senses or feeds: every rule it asks is the model's own, through
//! `cubarium_voxel_fauna`'s encounter helpers. What differs between the two arms is
//! *only* the mouth band, the body the band and the fan are anchored to, and the fan
//! itself — the set of legal standing faces is the same in both, so the comparison
//! isolates the anatomy and not the terrain.
//!
//! The rules and their sources are written down in
//! `design/voxel-encounter-contract-2026-09-21.md`.

use std::collections::{BTreeMap, HashMap, HashSet};

use cubarium::voxel::VoxelConfig;
use cubarium::voxel::habitat;
use cubarium::voxel::install_default_founders;
use cubarium::voxel::scene;
use cubarium_voxel::{Command as WorldCommand, VoxelView};
use cubarium_voxel_fauna::{
    ConeHit, Fauna, FaunaConfig, Founder, Manifest, Pose, Senses, SightMap, TICK_HZ,
    band_crown_layers, crown_columns, crown_layer, eye_above_surface_m, eye_origin_m,
    foliage_stands_in_layers, level_components, mouth_columns_from_face, mouth_crown_layers_at,
    ray_direction_deg, standable_faces,
};
use cubarium_voxel_flora::{Flora, FloraConfig, Site, Species as Plant};
use cubarium_voxel_sim::{Sim, SimConfig};

/// One simulated half hour, in ticks.
const TICKS_PER_REPORT: u64 = 30 * 60 * TICK_HZ as u64;

/// First seed a `preset=` arm offers the host's lake gate, as `voxel_census` uses.
const PRESET_SEED_BASE: u64 = 1;

// ---------------------------------------------------------------------------
// The decided anatomy (`design/handoffs/voxel-organism-decisions-2026-09-21.md`).
// These are *hypotheses measured against*, not model values: nothing below is read by
// the simulation.
// ---------------------------------------------------------------------------

/// Decisions §1: the browser adult is the ladder's 0.375 m animal, and the ladder draws
/// it 6 × 3 × 3 voxels of 0.125 m, so width and height are a third of the length.
const DECIDED_BROWSER_LENGTH_M: f64 = 0.375;
const DECIDED_BROWSER_HEIGHT_M: f64 = DECIDED_BROWSER_LENGTH_M / 3.0;
/// Decisions §2: one physical mouth band `[0, 1.33 × body height]` over the standing
/// surface — 0.249375 m for the adult, the dossier's raised neck.
const DECIDED_BAND_CEILING_M: f64 = 1.33 * DECIDED_BROWSER_HEIGHT_M;
/// Decisions §6: the eye at 0.8 × body height, and five pitches instead of three.
const DECIDED_EYE_HEIGHT_M: f64 = 0.8 * DECIDED_BROWSER_HEIGHT_M;
const DECIDED_PITCHES_DEG: [f64; 5] = [-40.0, -20.0, 0.0, 20.0, 40.0];

/// An observer id no animal has, so every living body occludes a diagnostic ray.
const NO_OBSERVER: u64 = u64::MAX;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let hours: f64 = args.get(1).map_or(6.0, |a| {
        a.parse()
            .expect("usage: voxel_edible_stock [HOURS] [preset=NAME] [seed=N] [heuristic]")
    });
    let preset: Option<&'static cubarium_voxel::Preset> = args
        .iter()
        .find_map(|a| a.strip_prefix("preset="))
        .map(|name| {
            cubarium_voxel::Preset::find(name).unwrap_or_else(|| {
                let known: Vec<&str> = cubarium_voxel::PRESETS.iter().map(|p| p.name).collect();
                panic!("no landform preset is called {name:?}; the shipped ones are {known:?}")
            })
        });
    let seed_base: u64 = args
        .iter()
        .find_map(|a| a.strip_prefix("seed=").and_then(|s| s.parse().ok()))
        .unwrap_or(PRESET_SEED_BASE);
    let heuristic = args.iter().any(|a| a == "heuristic");

    let cfg = VoxelConfig::default();
    // The census's own preset arm, verbatim, so the two agree at t = 0.
    let (mut world, flora_cfg) = match preset {
        Some(preset) => {
            let (world, seed, rejected) =
                cubarium::voxel::ambient_world(&preset.config(), seed_base);
            eprintln!(
                "scene: preset {} ({}x{}x{} at {} m, seed {seed}, {rejected} rejected)",
                preset.name,
                world.config().width,
                world.config().height,
                world.config().depth,
                world.config().voxel_m,
            );
            let flora_cfg = FloraConfig::for_voxel_size(world.config().voxel_m);
            (world, flora_cfg)
        }
        None => {
            eprintln!("scene: authored world");
            (scene::authored(cfg.world.clone()), FloraConfig::default())
        }
    };
    let mut flora = Flora::new(flora_cfg);
    let mut fauna = Fauna::new(FaunaConfig::default());
    let seeded = habitat::seed(&mut world, &mut flora, &mut fauna);
    eprintln!(
        "seeded: stands={} logs={} litter_tiles={} founders={:?} animals={}",
        seeded.stands,
        seeded.logs,
        seeded.litter_tiles,
        seeded.founders,
        seeded.animals()
    );
    if heuristic {
        eprintln!("founders: the observation-only heuristic (control)");
    } else {
        install_default_founders(&mut fauna).expect("the built-in centres validate");
    }
    let mut senses = Senses::new();
    senses.settle(&world.view(), &flora.view());
    let mut sim = Sim::new(world, flora, fauna, SimConfig::default(), Some(senses));
    if sim.world().config().closed_water_budget && !sim.world().outlet_open() {
        sim.world_mut()
            .apply(WorldCommand::SetOutlet { open: true });
    }

    print_headers();
    let seeded_browser_faces: Vec<(u32, u32, u32)> = sim
        .fauna()
        .view()
        .animals
        .iter()
        .filter(|a| a.founder == Some(Founder::Browser))
        .map(|a| (a.site.x, a.site.y, a.site.z))
        .collect();
    let first = report(&sim, 0, &seeded_browser_faces);
    let mut last = first.clone();
    let total_ticks = (hours * 3600.0 * f64::from(TICK_HZ)) as u64;
    let mut tick = 0u64;
    while tick < total_ticks {
        sim.step();
        tick += 1;
        if tick % TICKS_PER_REPORT == 0 {
            last = report(&sim, tick / (60 * TICK_HZ as u64), &seeded_browser_faces);
        }
    }
    print_summary(&first, &last);
}

// ---------------------------------------------------------------------------
// The two arms.
// ---------------------------------------------------------------------------

/// One arm's mouth: which body it measures with, and which crown layers it accepts from
/// a face at `standing_y`.
#[derive(Clone, Copy)]
struct Mouth {
    manifest: Manifest,
    /// `None` means today's implemented whole-voxel mouth; `Some(c)` a metre band with
    /// ceiling `c` over the standing surface.
    band_ceiling_m: Option<f64>,
}

impl Mouth {
    fn today() -> Mouth {
        Mouth {
            manifest: Founder::Browser.manifest(),
            band_ceiling_m: None,
        }
    }

    /// The decided body and the decided band. The manifest's own geometry is the only
    /// thing changed: length and width take the ladder's adult, so the horizontal reach
    /// — `0.25 × body length`, "as today" (decisions §2) — and the footprint follow.
    fn decided() -> Mouth {
        let mut manifest = Founder::Browser.manifest();
        manifest.body_length_m = DECIDED_BROWSER_LENGTH_M;
        manifest.body_width_m = DECIDED_BROWSER_HEIGHT_M;
        Mouth {
            manifest,
            band_ceiling_m: Some(DECIDED_BAND_CEILING_M),
        }
    }

    fn layers(&self, standing_y: u32, voxel_m: f64) -> std::ops::RangeInclusive<i64> {
        match self.band_ceiling_m {
            None => mouth_crown_layers_at(standing_y, &self.manifest, voxel_m),
            Some(c) => band_crown_layers(standing_y, voxel_m, c),
        }
    }
}

/// One arm's eye: where it sits over the standing surface, and which pitches it casts.
#[derive(Clone, Copy)]
struct Fan {
    /// `None` means today's anchor — one and a half **voxels** over the standing face.
    eye_height_m: Option<f64>,
    pitches_deg: &'static [f64],
    pools_occlude: bool,
}

const TODAY_FAN: Fan = Fan {
    eye_height_m: None,
    pitches_deg: &cubarium_voxel_fauna::BROWSER_RAY_PITCH_OFFSETS_DEG,
    pools_occlude: true,
};

const DECIDED_FAN: Fan = Fan {
    eye_height_m: Some(DECIDED_EYE_HEIGHT_M),
    pitches_deg: &DECIDED_PITCHES_DEG,
    pools_occlude: false,
};

// ---------------------------------------------------------------------------
// One report.
// ---------------------------------------------------------------------------

#[derive(Clone, Default)]
struct SpeciesRow {
    stands: usize,
    foliage: f64,
    reach_today: f64,
    reach_band: f64,
    visible_today: f64,
    visible_decided: f64,
    route_today: f64,
    route_band: f64,
    /// As `route_*`, but against the components the **seeded** founders stood in rather
    /// than the living ones. After a lineage dies out the live measure is zero by
    /// definition and says nothing about the landscape; this one still does.
    route_seeded_today: f64,
    route_seeded_band: f64,
}

#[derive(Clone)]
struct Report {
    minute: u64,
    rows: Vec<(&'static str, SpeciesRow)>,
    total: SpeciesRow,
    lineages: Vec<(&'static str, LineageRow)>,
    detritus: Vec<(&'static str, f64, f64)>,
    residuals: [f64; 7],
}

#[derive(Clone, Default)]
struct LineageRow {
    alive: usize,
    heights: BTreeMap<u32, usize>,
    mean_distance_today: Option<f64>,
    mean_distance_band: Option<f64>,
    share_within_2m_today: Option<f64>,
    share_within_2m_band: Option<f64>,
}

fn report(sim: &Sim, minute: u64, seeded_browser_faces: &[(u32, u32, u32)]) -> Report {
    let wv = sim.world().view();
    let fv = sim.flora().view();
    let av = sim.fauna().view();
    let c = wv.config;
    let v = c.voxel_m;

    let today = Mouth::today();
    let decided = Mouth::decided();
    let browser_core = sim.fauna().config().founder(Founder::Browser).core;
    let blind_manifest = Founder::Blind.manifest();
    let blind_core = sim.fauna().config().founder(Founder::Blind).core;

    // One set of legal standing faces, shared by both arms: a support face with wadeable
    // water and the browser's headroom over it. Keeping it the same in both arms is what
    // makes the band and the fan the only variables.
    let faces = standable_faces(&wv, &today.manifest, browser_core.wade_depth_m);
    let face_index: HashMap<(u32, u32, u32), usize> = faces
        .iter()
        .enumerate()
        .map(|(i, f)| ((f.x, f.y, f.z), i))
        .collect();
    let components = level_components(&faces, c.width);

    // The mouth columns of every legal face, per arm, and the index from a column back
    // to the faces that can put a mouth over it.
    let mouth_cols = |mouth: &Mouth| -> (Vec<Vec<(i64, u32)>>, HashMap<(u32, u32), Vec<usize>>) {
        let mut per_face: Vec<Vec<(i64, u32)>> = Vec::with_capacity(faces.len());
        let mut map: HashMap<(u32, u32), Vec<usize>> = HashMap::new();
        for (i, face) in faces.iter().enumerate() {
            let cols = mouth_columns_from_face(&wv, *face, &mouth.manifest);
            for &(cx, cz) in &cols {
                map.entry((cx.rem_euclid(i64::from(c.width)) as u32, cz))
                    .or_default()
                    .push(i);
            }
            per_face.push(cols);
        }
        (per_face, map)
    };
    let (cols_today, reach_today_map) = mouth_cols(&today);
    let (cols_band, reach_band_map) = mouth_cols(&decided);

    // The components a living browser stands in.
    let browser_components: HashSet<usize> = av
        .animals
        .iter()
        .filter(|a| a.founder == Some(Founder::Browser))
        .filter_map(|a| face_index.get(&(a.site.x, a.site.y, a.site.z)))
        .map(|&i| components[i])
        .collect();
    let seeded_components: HashSet<usize> = seeded_browser_faces
        .iter()
        .filter_map(|key| face_index.get(key))
        .map(|&i| components[i])
        .collect();

    let sight_today = SightMap::new(&wv, &fv, &av, TODAY_FAN.pools_occlude);
    let sight_decided = SightMap::new(&wv, &fv, &av, DECIDED_FAN.pools_occlude);

    let mut rows: Vec<(&'static str, SpeciesRow)> = Plant::ALL
        .iter()
        .map(|s| (s.name(), SpeciesRow::default()))
        .collect();
    let mut total = SpeciesRow::default();
    // Per stand: is it reachable at all, and from a face in a browser's component?
    let mut reachable_today_sites: Vec<(Site, f64)> = Vec::new();
    let mut reachable_band_sites: Vec<(Site, f64)> = Vec::new();

    for stand in fv.stands.iter() {
        let row = &mut rows[stand.species.index()].1;
        row.stands += 1;
        total.stands += 1;
        if stand.foliage <= 0.0 {
            continue;
        }
        row.foliage += stand.foliage;
        total.foliage += stand.foliage;
        let layer = crown_layer(&fv, stand);
        let columns = crown_columns(&fv, &wv, stand);

        for (mouth, map, per_face, reach, route, route_seeded, sites) in [
            (
                &today,
                &reach_today_map,
                &cols_today,
                &mut row.reach_today,
                &mut row.route_today,
                &mut row.route_seeded_today,
                &mut reachable_today_sites,
            ),
            (
                &decided,
                &reach_band_map,
                &cols_band,
                &mut row.reach_band,
                &mut row.route_band,
                &mut row.route_seeded_band,
                &mut reachable_band_sites,
            ),
        ] {
            let mut any = false;
            let mut connected = false;
            let mut connected_seeded = false;
            for column in &columns {
                for &i in map.get(column).map(Vec::as_slice).unwrap_or(&[]) {
                    let face = faces[i];
                    let layers = mouth.layers(face.y, v);
                    if !layers.contains(&layer) {
                        continue;
                    }
                    // The acceptance rule itself, not a re-derivation of it.
                    if !foliage_stands_in_layers(&fv, &wv, &per_face[i], &layers)
                        .iter()
                        .any(|(site, _)| *site == stand.site)
                    {
                        continue;
                    }
                    any = true;
                    connected |= browser_components.contains(&components[i]);
                    connected_seeded |= seeded_components.contains(&components[i]);
                    if connected && connected_seeded {
                        break;
                    }
                }
                if connected && connected_seeded {
                    break;
                }
            }
            if any {
                *reach += stand.foliage;
                sites.push((stand.site, stand.foliage));
            }
            if connected {
                *route += stand.foliage;
            }
            if connected_seeded {
                *route_seeded += stand.foliage;
            }
        }

        for (fan, sight, into) in [
            (&TODAY_FAN, &sight_today, &mut row.visible_today),
            (&DECIDED_FAN, &sight_decided, &mut row.visible_decided),
        ] {
            if sees_stand(&wv, sight, &faces, &columns, layer, fan, &today.manifest) {
                *into += stand.foliage;
            }
        }
    }
    for (_, row) in &rows {
        total.reach_today += row.reach_today;
        total.reach_band += row.reach_band;
        total.visible_today += row.visible_today;
        total.visible_decided += row.visible_decided;
        total.route_today += row.route_today;
        total.route_band += row.route_band;
        total.route_seeded_today += row.route_seeded_today;
        total.route_seeded_band += row.route_seeded_band;
    }

    // --- lineages ---
    let mut lineages = Vec::new();
    for founder in Founder::ALL {
        let mut lr = LineageRow::default();
        let mut d_today: Vec<f64> = Vec::new();
        let mut d_band: Vec<f64> = Vec::new();
        for a in av.animals.iter().filter(|a| a.founder == Some(founder)) {
            lr.alive += 1;
            *lr.heights.entry(a.site.y).or_default() += 1;
            if founder != Founder::Browser {
                continue;
            }
            for (sites, into) in [
                (&reachable_today_sites, &mut d_today),
                (&reachable_band_sites, &mut d_band),
            ] {
                if let Some(d) = sites
                    .iter()
                    .map(|(site, _)| horizontal_m(&wv, &a.pose, *site))
                    .fold(None, |best: Option<f64>, d| {
                        Some(best.map_or(d, |b| b.min(d)))
                    })
                {
                    into.push(d);
                }
            }
        }
        lr.mean_distance_today = mean(&d_today);
        lr.mean_distance_band = mean(&d_band);
        lr.share_within_2m_today = share_within(&d_today, 2.0);
        lr.share_within_2m_band = share_within(&d_band, 2.0);
        debug_assert!(founder == Founder::Browser || lr.mean_distance_today.is_none());
        lineages.push((founder.name(), lr));
    }

    // --- detritus, against the shredders' own components ---
    let blind_faces = standable_faces(&wv, &blind_manifest, blind_core.wade_depth_m);
    let blind_index: HashMap<(u32, u32, u32), usize> = blind_faces
        .iter()
        .enumerate()
        .map(|(i, f)| ((f.x, f.y, f.z), i))
        .collect();
    let blind_components = level_components(&blind_faces, c.width);
    let shredder_components: HashSet<usize> = av
        .animals
        .iter()
        .filter(|a| a.founder == Some(Founder::Blind))
        .filter_map(|a| blind_index.get(&(a.site.x, a.site.y, a.site.z)))
        .map(|&i| blind_components[i])
        .collect();
    let in_shredder_reach = |site: Site| -> bool {
        blind_index
            .get(&(site.x, site.y, site.z))
            .is_some_and(|&i| shredder_components.contains(&blind_components[i]))
    };
    let mut detritus = Vec::new();
    for (name, total_organic, connected) in [
        pool(
            "litter",
            fv.ground.iter().map(|g| (g.site, g.litter)),
            &in_shredder_reach,
        ),
        pool(
            "carrion",
            fv.ground.iter().map(|g| (g.site, g.carrion)),
            &in_shredder_reach,
        ),
        pool(
            "glowcap_cap",
            fv.stands
                .iter()
                .filter(|s| s.species == Plant::Glowcap)
                .map(|s| (s.site, s.foliage)),
            &in_shredder_reach,
        ),
    ] {
        detritus.push((name, total_organic, connected));
    }

    let residuals = [
        fv.organic() - fv.ledger.expected_organic(),
        fv.mineral() - fv.ledger.expected_mineral(),
        fv.energy() - fv.ledger.expected_energy(),
        av.organic() - av.ledger.expected_organic(),
        av.mineral() - av.ledger.expected_mineral(),
        av.energy() - av.ledger.expected_energy(),
        wv.total_residual(),
    ];

    let report = Report {
        minute,
        rows,
        total,
        lineages,
        detritus,
        residuals,
    };
    print_report(&report);
    report
}

/// Whether any eye on a legal face within the cone's range sees one of this stand's
/// crown cells unoccluded, at one of the fan's pitches.
///
/// The yaw is aimed at the cell, because the fan sweeps every yaw as the body turns; the
/// pitch set and the eye height are the arm. The march, the sub-step, the step cap and
/// the order of the occlusion tests are the live cone's.
fn sees_stand(
    wv: &VoxelView<'_>,
    sight: &SightMap,
    faces: &[Site],
    columns: &[(u32, u32)],
    layer: i64,
    fan: &Fan,
    manifest: &Manifest,
) -> bool {
    let c = wv.config;
    let v = c.voxel_m;
    let range = manifest.cone_range_m;
    let cells: HashSet<usize> = columns
        .iter()
        .map(|&(x, z)| c.index(i64::from(x), layer as u32, z))
        .collect();
    let targets: Vec<(f64, f64, f64)> = columns
        .iter()
        .map(|&(x, z)| {
            (
                (f64::from(x) + 0.5) * v,
                (layer as f64 + 0.5) * v,
                (f64::from(z) + 0.5) * v,
            )
        })
        .collect();

    // Faces nearest the stand first: most stands are seen from the first one tried.
    let mut near: Vec<(f64, usize)> = Vec::new();
    for (i, face) in faces.iter().enumerate() {
        let fx = (f64::from(face.x) + 0.5) * v;
        let fz = (f64::from(face.z) + 0.5) * v;
        let best = targets
            .iter()
            .map(|t| {
                let dx = wrapped_delta(t.0 - fx, f64::from(c.width) * v);
                let dz = t.2 - fz;
                (dx * dx + dz * dz).sqrt()
            })
            .fold(f64::INFINITY, f64::min);
        if best <= range {
            near.push((best, i));
        }
    }
    near.sort_by(|a, b| a.0.partial_cmp(&b.0).expect("finite distances"));

    for (_, i) in near {
        let face = faces[i];
        let pose = Pose::at_site(face, v);
        let origin = match fan.eye_height_m {
            None => eye_origin_m(wv, &pose, face.y),
            Some(h) => eye_above_surface_m(&pose, face.y, v, h),
        };
        for target in &targets {
            let dx = wrapped_delta(target.0 - origin.0, f64::from(c.width) * v);
            let dz = target.2 - origin.2;
            if (dx * dx + dz * dz).sqrt() > range {
                continue;
            }
            let yaw = dx.atan2(dz).to_degrees();
            for &pitch in fan.pitches_deg {
                let dir = ray_direction_deg(0.0, yaw, pitch);
                if let Some((_, hit, cell)) = sight.first_hit(wv, NO_OBSERVER, origin, dir, range) {
                    if hit == ConeHit::FoliageCrown && cells.contains(&cell) {
                        return true;
                    }
                }
            }
        }
    }
    false
}

fn pool<'a>(
    name: &'static str,
    entries: impl Iterator<Item = (Site, f64)> + 'a,
    connected: &dyn Fn(Site) -> bool,
) -> (&'static str, f64, f64) {
    let (mut total, mut inside) = (0.0, 0.0);
    for (site, amount) in entries {
        if amount <= 0.0 {
            continue;
        }
        total += amount;
        if connected(site) {
            inside += amount;
        }
    }
    (name, total, inside)
}

fn horizontal_m(wv: &VoxelView<'_>, pose: &Pose, site: Site) -> f64 {
    let v = wv.config.voxel_m;
    let width_m = f64::from(wv.config.width) * v;
    let dx = wrapped_delta((f64::from(site.x) + 0.5) * v - pose.x, width_m);
    let dz = (f64::from(site.z) + 0.5) * v - pose.z;
    (dx * dx + dz * dz).sqrt()
}

fn wrapped_delta(d: f64, width_m: f64) -> f64 {
    let mut d = d;
    while d > width_m / 2.0 {
        d -= width_m;
    }
    while d < -width_m / 2.0 {
        d += width_m;
    }
    d
}

fn mean(xs: &[f64]) -> Option<f64> {
    (!xs.is_empty()).then(|| xs.iter().sum::<f64>() / xs.len() as f64)
}

/// The share of a non-empty sample within `limit`. A lineage the statistic does not
/// apply to has no sample and gets `None`, not a zero that reads as a finding.
fn share_within(xs: &[f64], limit: f64) -> Option<f64> {
    (!xs.is_empty()).then(|| xs.iter().filter(|&&d| d <= limit).count() as f64 / xs.len() as f64)
}

// ---------------------------------------------------------------------------
// Output.
// ---------------------------------------------------------------------------

fn print_headers() {
    println!(
        "stock,sim_min,species,stands,foliage,reach_today,reach_band,visible_today,\
         visible_decided,route_today,route_band,route_seeded_today,route_seeded_band,\
         f_reach_today,f_reach_band,f_visible_today,f_visible_decided,f_route_today,f_route_band,\
         f_route_seeded_today,f_route_seeded_band"
    );
    println!(
        "lineage,sim_min,founder,alive,heights,mean_m_to_reachable_today,\
         mean_m_to_reachable_band,share_2m_today,share_2m_band"
    );
    println!("detritus,sim_min,pool,organic,in_component,fraction");
    println!(
        "ledger,sim_min,flora_organic,flora_mineral,flora_energy,fauna_organic,fauna_mineral,\
         fauna_energy,water"
    );
}

fn fraction(part: f64, whole: f64) -> f64 {
    if whole > 0.0 { part / whole } else { 0.0 }
}

fn print_species(minute: u64, name: &str, r: &SpeciesRow) {
    println!(
        "stock,{minute},{name},{},{:.6},{:.6},{:.6},{:.6},{:.6},{:.6},{:.6},{:.6},{:.6},\
         {:.4},{:.4},{:.4},{:.4},{:.4},{:.4},{:.4},{:.4}",
        r.stands,
        r.foliage,
        r.reach_today,
        r.reach_band,
        r.visible_today,
        r.visible_decided,
        r.route_today,
        r.route_band,
        r.route_seeded_today,
        r.route_seeded_band,
        fraction(r.reach_today, r.foliage),
        fraction(r.reach_band, r.foliage),
        fraction(r.visible_today, r.foliage),
        fraction(r.visible_decided, r.foliage),
        fraction(r.route_today, r.foliage),
        fraction(r.route_band, r.foliage),
        fraction(r.route_seeded_today, r.foliage),
        fraction(r.route_seeded_band, r.foliage),
    );
}

/// A statistic that does not apply prints as `-` rather than as zero.
fn optional(value: Option<f64>, places: usize) -> String {
    value.map_or("-".to_string(), |v| format!("{v:.places$}"))
}

fn print_report(r: &Report) {
    for (name, row) in &r.rows {
        print_species(r.minute, name, row);
    }
    print_species(r.minute, "ALL", &r.total);
    for (name, l) in &r.lineages {
        let heights: Vec<String> = l.heights.iter().map(|(y, n)| format!("{y}:{n}")).collect();
        println!(
            "lineage,{},{name},{},{},{},{},{},{}",
            r.minute,
            l.alive,
            heights.join(" "),
            optional(l.mean_distance_today, 3),
            optional(l.mean_distance_band, 3),
            optional(l.share_within_2m_today, 4),
            optional(l.share_within_2m_band, 4),
        );
    }
    for (name, total, inside) in &r.detritus {
        println!(
            "detritus,{},{name},{:.6},{:.6},{:.4}",
            r.minute,
            total,
            inside,
            fraction(*inside, *total)
        );
    }
    println!(
        "ledger,{},{:.3e},{:.3e},{:.3e},{:.3e},{:.3e},{:.3e},{:.3e}",
        r.minute,
        r.residuals[0],
        r.residuals[1],
        r.residuals[2],
        r.residuals[3],
        r.residuals[4],
        r.residuals[5],
        r.residuals[6],
    );
}

fn print_summary(first: &Report, last: &Report) {
    println!("summary,metric,t0,final");
    let pairs: [(&str, f64, f64); 10] = [
        ("foliage", first.total.foliage, last.total.foliage),
        (
            "f_reach_today",
            fraction(first.total.reach_today, first.total.foliage),
            fraction(last.total.reach_today, last.total.foliage),
        ),
        (
            "f_reach_band",
            fraction(first.total.reach_band, first.total.foliage),
            fraction(last.total.reach_band, last.total.foliage),
        ),
        (
            "f_visible_today",
            fraction(first.total.visible_today, first.total.foliage),
            fraction(last.total.visible_today, last.total.foliage),
        ),
        (
            "f_visible_decided",
            fraction(first.total.visible_decided, first.total.foliage),
            fraction(last.total.visible_decided, last.total.foliage),
        ),
        (
            "f_route_today",
            fraction(first.total.route_today, first.total.foliage),
            fraction(last.total.route_today, last.total.foliage),
        ),
        (
            "f_route_band",
            fraction(first.total.route_band, first.total.foliage),
            fraction(last.total.route_band, last.total.foliage),
        ),
        (
            "f_route_seeded_today",
            fraction(first.total.route_seeded_today, first.total.foliage),
            fraction(last.total.route_seeded_today, last.total.foliage),
        ),
        (
            "f_route_seeded_band",
            fraction(first.total.route_seeded_band, first.total.foliage),
            fraction(last.total.route_seeded_band, last.total.foliage),
        ),
        (
            "browsers_alive",
            first.lineages[Founder::Browser.index()].1.alive as f64,
            last.lineages[Founder::Browser.index()].1.alive as f64,
        ),
    ];
    for (name, a, b) in pairs {
        println!("summary,{name},{a:.6},{b:.6}");
    }
    for (i, name) in [
        "flora_organic",
        "flora_mineral",
        "flora_energy",
        "fauna_organic",
        "fauna_mineral",
        "fauna_energy",
        "water",
    ]
    .into_iter()
    .enumerate()
    {
        println!(
            "summary,residual_{name},{:.3e},{:.3e}",
            first.residuals[i], last.residuals[i]
        );
    }
}
