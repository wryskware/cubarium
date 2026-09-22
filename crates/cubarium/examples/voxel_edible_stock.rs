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
//!    reachable, visible and route-connected under the **implemented** rules, plus one
//!    column for the part of decisions §6 that is still unimplemented — the five-pitch
//!    fan with transparent ground pools, which package 5's retrain owns.
//! 2. `lineage`, per founder: bodies alive, their standing-height distribution, mean
//!    distance to the nearest reachable stand, and the share with one inside 2 m.
//! 3. `detritus`: litter, carrion and glowcap cap organic, and the share of each lying
//!    in a walkable component that holds a living shredder.
//!
//! Nothing here steps, senses or feeds: every rule it asks is the model's own, through
//! `cubarium_voxel_fauna`'s encounter helpers. Since package 1b
//! (`design/handoffs/voxel-body-anchors-2026-09-22.md`) the mouth band, the body and
//! the eye **are** the model's, so the old "today against decided" mouth arms have
//! collapsed into one; the fan's pitch set is the only thing still measured as a
//! hypothesis.
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
    ConeHit, Diet, Fauna, FaunaConfig, Founder, Manifest, Pose, Senses, SightMap, TICK_HZ,
    climb_voxels, eye_origin_m, layer_columns, mouth_columns_from_face, mouth_crown_layers_at,
    ray_direction_deg, reachable_layers_of, standable_faces, walkable_components,
};
use cubarium_voxel_flora::{Flora, FloraConfig, Site, Species as Plant};
use cubarium_voxel_sim::{Sim, SimConfig};

/// One simulated half hour, in ticks.
const TICKS_PER_REPORT: u64 = 30 * 60 * TICK_HZ as u64;

/// First seed a `preset=` arm offers the host's lake gate, as `voxel_census` uses.
const PRESET_SEED_BASE: u64 = 1;

// ---------------------------------------------------------------------------
// The one part of decisions §6 the model does **not** yet run: the five-pitch fan, and
// a ground pool that is not a wall. Both are package 5's and the backlog's, and both
// are measured here as hypotheses. The body, the band and the eye are no longer
// hypotheses — they are read off the live physiology.
// ---------------------------------------------------------------------------

/// Decisions §6: five pitches instead of the manifest's three.
const DECIDED_PITCHES_DEG: [f64; 5] = [-40.0, -20.0, 0.0, 20.0, 40.0];

/// An observer id no animal has: a ray cast from a face nobody occupies is occluded by
/// every living body, as the live cone's is by every body but the observer's own.
const NO_OBSERVER: u64 = u64::MAX;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let hours: f64 = args.get(1).map_or(6.0, |a| {
        a.parse().expect(
            "usage: voxel_edible_stock [HOURS] [preset=NAME] [seed=N] [heuristic] \
                 [lollipop]",
        )
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
    // The **before** arm, on this build: every plant reduced to the one-disc lollipop
    // the model was before `design/handoffs/voxel-plant-layers-2026-09-22.md`, so a
    // reach or route number can be compared against the layered one on the same
    // landforms, the same seed and the same bodies rather than against a recorded
    // figure from another revision.
    let lollipop = args.iter().any(|a| a == "lollipop");

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
    if lollipop {
        eprintln!("plants: one_layer_species (the pre-layers control)");
    }
    let flora_cfg = if lollipop {
        flora_cfg.one_layer_species()
    } else {
        flora_cfg
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

/// One arm's fan: the pitches it casts, and whether a ground pool blocks a ray. The
/// **eye is the model's** on both arms now (`0.8 × body height` over the standing
/// surface), so what is left to compare is decisions §6's pitch set and its "a ground
/// pool is not a wall", neither of which is implemented.
#[derive(Clone, Copy)]
struct Fan {
    pitches_deg: &'static [f64],
    pools_occlude: bool,
}

const LIVE_FAN: Fan = Fan {
    pitches_deg: &cubarium_voxel_fauna::BROWSER_RAY_PITCH_OFFSETS_DEG,
    pools_occlude: true,
};

const FIVE_PITCH_FAN: Fan = Fan {
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
    reach: f64,
    visible: f64,
    /// What the still-unimplemented five-pitch fan would see (decisions §6, package 5).
    visible_five_pitch: f64,
    route: f64,
    /// As `route`, but against the components the **seeded** founders stood in rather
    /// than the living ones. After a lineage dies out the live measure is zero by
    /// definition and says nothing about the landscape; this one still does.
    route_seeded: f64,
}

/// One **foliage layer index** of one species, pooled over every stand that has one:
/// how much tissue stands in it and how much of that a browser can reach. The band is
/// stock-weighted, so it reads as "where this layer's tissue actually is".
#[derive(Clone)]
struct LayerRow {
    species: Plant,
    index: usize,
    stands: usize,
    stock: f64,
    reach: f64,
    route: f64,
    /// Stock-weighted sums; divided by `stock` when printed.
    band_lo_m: f64,
    band_hi_m: f64,
}

#[derive(Clone)]
struct Report {
    minute: u64,
    layers: Vec<LayerRow>,
    rows: Vec<(&'static str, SpeciesRow)>,
    total: SpeciesRow,
    lineages: Vec<(&'static str, LineageRow)>,
    detritus: Vec<(&'static str, f64, f64)>,
    residuals: [f64; 7],
    /// The flora's organic, mineral and energy stocks: the denominators of the first
    /// three residuals.
    totals: [f64; 3],
}

#[derive(Clone, Default)]
struct LineageRow {
    alive: usize,
    heights: BTreeMap<u32, usize>,
    mean_distance_m: Option<f64>,
    share_within_2m: Option<f64>,
}

fn report(sim: &Sim, minute: u64, seeded_browser_faces: &[(u32, u32, u32)]) -> Report {
    let wv = sim.world().view();
    let fv = sim.flora().view();
    let av = sim.fauna().view();
    let c = wv.config;
    let v = c.voxel_m;

    let browser_phys = *sim.fauna().config().founder(Founder::Browser);
    let browser_core = browser_phys.core;
    // The **adult** body of each lineage: the ceiling this landscape offers a grown
    // animal, which is what a stock measurement is about. It is the model's own
    // geometry now, not a hypothesis (`FounderPhysiology::adult_body`).
    let browser_body = browser_phys.adult_body();
    let browser_manifest = Founder::Browser.manifest();
    let blind_phys = *sim.fauna().config().founder(Founder::Blind);
    let blind_core = blind_phys.core;
    let blind_body = blind_phys.adult_body();
    // The route rule, one function, one climb per lineage
    // (`design/handoffs/voxel-founder-step-2026-09-22.md`).
    let browser_climb = climb_voxels(&browser_phys, c.voxel_m);
    let blind_climb = climb_voxels(&blind_phys, c.voxel_m);

    // The legal standing faces: a support face with wadeable water and the browser's
    // own height of void over it.
    let faces = standable_faces(&wv, &browser_body, browser_core.wade_depth_m);
    let face_index: HashMap<(u32, u32, u32), usize> = faces
        .iter()
        .enumerate()
        .map(|(i, f)| ((f.x, f.y, f.z), i))
        .collect();
    let components = walkable_components(&faces, c.width, browser_climb);

    // The mouth columns of every legal face, and the index from a column back to the
    // faces that can put a mouth over it.
    let (per_face_cols, reach_map) = {
        let mut per_face: Vec<Vec<(i64, u32)>> = Vec::with_capacity(faces.len());
        let mut map: HashMap<(u32, u32), Vec<usize>> = HashMap::new();
        for (i, face) in faces.iter().enumerate() {
            let cols = mouth_columns_from_face(&wv, *face, &browser_body);
            for &(cx, cz) in &cols {
                map.entry((cx.rem_euclid(i64::from(c.width)) as u32, cz))
                    .or_default()
                    .push(i);
            }
            per_face.push(cols);
        }
        (per_face, map)
    };

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

    // A hypothetical eye on an occupied face is that occupant's eye, so a body does not
    // block its own view — the live cone's rule (`senses::ray_first_hit`'s observer_id).
    let occupant: HashMap<(u32, u32, u32), u64> = av
        .animals
        .iter()
        .map(|a| ((a.site.x, a.site.y, a.site.z), a.id))
        .collect();

    let sight_live = SightMap::new(&wv, &fv, &av, LIVE_FAN.pools_occlude);
    let sight_five = SightMap::new(&wv, &fv, &av, FIVE_PITCH_FAN.pools_occlude);

    let mut rows: Vec<(&'static str, SpeciesRow)> = Plant::ALL
        .iter()
        .map(|s| (s.name(), SpeciesRow::default()))
        .collect();
    let mut total = SpeciesRow::default();
    // Per stand: is it reachable at all, and from a face in a browser's component?
    let mut reachable_sites: Vec<(Site, f64)> = Vec::new();

    // Since the layers package the unit of the measurement is a **layer**, not a
    // stand: an adult bloomcrown's basal rosette is reachable and its crown is not, and
    // a per-stand answer could only say one or the other
    // (`design/handoffs/voxel-plant-layers-2026-09-22.md`; package 0's "could not
    // measure 1", within-stand shares).
    let mut layer_rows: Vec<LayerRow> = Vec::new();
    for stand in fv.stands.iter() {
        rows[stand.species.index()].1.stands += 1;
        total.stands += 1;
        if stand.foliage <= 0.0 {
            continue;
        }
        // Permission, not geometry (decisions §3): a glowcap cap standing in a
        // browser's band is reachable fungal tissue and not browser food, so it is
        // counted in the standing foliage and never in the reach. It is the shredder's
        // food and is reported as such in the detritus block below.
        let edible = Diet::Vascular.accepts(fv.config.species(stand.species).trophic);
        let mut counted_site = false;
        for layer in fv.layers(stand) {
            if !(layer.stock > 0.0) {
                continue;
            }
            let li = layer.foliage_index.unwrap_or(0);
            let row = &mut rows[stand.species.index()].1;
            row.foliage += layer.stock;
            total.foliage += layer.stock;
            let columns = layer_columns(&wv, stand, &layer);

            let mut any = false;
            let mut connected = false;
            let mut connected_seeded = false;
            for column in columns.iter().filter(|_| edible) {
                for &i in reach_map.get(column).map(Vec::as_slice).unwrap_or(&[]) {
                    let face = faces[i];
                    let band = mouth_crown_layers_at(face.y, &browser_body, v);
                    if !band.contains(&layer.cell) {
                        continue;
                    }
                    // The acceptance rule itself, not a re-derivation of it: the
                    // model's own per-layer scan, asked whether *this* layer is one of
                    // the ones the mouth at this face may take from.
                    if !reachable_layers_of(&fv, &wv, stand, &per_face_cols[i], &band)
                        .iter()
                        .any(|(index, _)| *index == li)
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
                row.reach += layer.stock;
                if !counted_site {
                    reachable_sites.push((stand.site, stand.foliage));
                    counted_site = true;
                }
            }
            if connected {
                row.route += layer.stock;
            }
            if connected_seeded {
                row.route_seeded += layer.stock;
            }

            let mut seen = [false; 2];
            for (i, (fan, sight)) in [(&LIVE_FAN, &sight_live), (&FIVE_PITCH_FAN, &sight_five)]
                .into_iter()
                .enumerate()
            {
                seen[i] = sees_stand(
                    &wv,
                    sight,
                    &faces,
                    &occupant,
                    &columns,
                    layer.cell,
                    fan,
                    &browser_manifest,
                    &browser_body,
                );
            }
            let row = &mut rows[stand.species.index()].1;
            if seen[0] {
                row.visible += layer.stock;
            }
            if seen[1] {
                row.visible_five_pitch += layer.stock;
            }

            let slot = layer_rows
                .iter_mut()
                .find(|r| r.species == stand.species && r.index == li);
            let slot = match slot {
                Some(slot) => slot,
                None => {
                    layer_rows.push(LayerRow {
                        species: stand.species,
                        index: li,
                        stands: 0,
                        stock: 0.0,
                        reach: 0.0,
                        route: 0.0,
                        band_lo_m: 0.0,
                        band_hi_m: 0.0,
                    });
                    layer_rows.last_mut().expect("just pushed")
                }
            };
            slot.stands += 1;
            slot.stock += layer.stock;
            slot.band_lo_m += layer.band_m[0] * layer.stock;
            slot.band_hi_m += layer.band_m[1] * layer.stock;
            if any {
                slot.reach += layer.stock;
            }
            if connected {
                slot.route += layer.stock;
            }
        }
    }
    layer_rows.sort_by_key(|r| (r.species.index(), r.index));
    // The invariant the whole per-layer measurement rests on, checked where it is
    // measured and not only in a unit test: what the layers report **is** the standing
    // foliage, so no stock is counted twice and none is lost between the two views.
    {
        let scalar: f64 = fv.stands.iter().map(|s| s.foliage.max(0.0)).sum();
        let layered: f64 = layer_rows.iter().map(|r| r.stock).sum();
        assert!(
            (layered - scalar).abs() <= 1e-9 * scalar.abs().max(1.0),
            "the per-layer stocks sum to {layered} and the stands hold {scalar}"
        );
        assert!(
            (total.foliage - scalar).abs() <= 1e-9 * scalar.abs().max(1.0),
            "the species rows sum to {} and the stands hold {scalar}",
            total.foliage
        );
    }
    for (_, row) in &rows {
        total.reach += row.reach;
        total.visible += row.visible;
        total.visible_five_pitch += row.visible_five_pitch;
        total.route += row.route;
        total.route_seeded += row.route_seeded;
    }

    // --- lineages ---
    let mut lineages = Vec::new();
    for founder in Founder::ALL {
        let mut lr = LineageRow::default();
        let mut distances: Vec<f64> = Vec::new();
        for a in av.animals.iter().filter(|a| a.founder == Some(founder)) {
            lr.alive += 1;
            *lr.heights.entry(a.site.y).or_default() += 1;
            if founder != Founder::Browser {
                continue;
            }
            if let Some(d) = reachable_sites
                .iter()
                .map(|(site, _)| horizontal_m(&wv, &a.pose, *site))
                .fold(None, |best: Option<f64>, d| {
                    Some(best.map_or(d, |b| b.min(d)))
                })
            {
                distances.push(d);
            }
        }
        lr.mean_distance_m = mean(&distances);
        lr.share_within_2m = share_within(&distances, 2.0);
        debug_assert!(founder == Founder::Browser || lr.mean_distance_m.is_none());
        lineages.push((founder.name(), lr));
    }

    // --- detritus, against the shredders' own components ---
    let blind_faces = standable_faces(&wv, &blind_body, blind_core.wade_depth_m);
    let blind_index: HashMap<(u32, u32, u32), usize> = blind_faces
        .iter()
        .enumerate()
        .map(|(i, f)| ((f.x, f.y, f.z), i))
        .collect();
    let blind_components = walkable_components(&blind_faces, c.width, blind_climb);
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

    // The stocks the residuals below are a residual **of**, so a relative bound can be
    // read off the report rather than estimated from it.
    let totals = [fv.organic(), fv.mineral(), fv.energy()];
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
        layers: layer_rows,
        rows,
        total,
        lineages,
        detritus,
        residuals,
        totals,
    };
    print_report(&report);
    report
}

/// Whether any eye on a legal face within the cone's range sees one of this stand's
/// crown cells unoccluded, at one of the fan's pitches.
///
/// The yaw is aimed at the cell, because the fan sweeps every yaw as the body turns; the
/// **pitch set and the pool rule** are the arm, and the eye is the model's own. The
/// march, the sub-step, the step cap and the order of the occlusion tests are the live
/// cone's.
#[allow(clippy::too_many_arguments)]
fn sees_stand(
    wv: &VoxelView<'_>,
    sight: &SightMap,
    faces: &[Site],
    occupant: &HashMap<(u32, u32, u32), u64>,
    columns: &[(u32, u32)],
    layer: i64,
    fan: &Fan,
    manifest: &Manifest,
    body: &cubarium_voxel_fauna::Body,
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
        let observer = occupant
            .get(&(face.x, face.y, face.z))
            .copied()
            .unwrap_or(NO_OBSERVER);
        let pose = Pose::at_site(face, v);
        let origin = eye_origin_m(wv, &pose, face.y, body);
        for target in &targets {
            let dx = wrapped_delta(target.0 - origin.0, f64::from(c.width) * v);
            let dz = target.2 - origin.2;
            if (dx * dx + dz * dz).sqrt() > range {
                continue;
            }
            let yaw = dx.atan2(dz).to_degrees();
            for &pitch in fan.pitches_deg {
                let dir = ray_direction_deg(0.0, yaw, pitch);
                if let Some((_, hit, cell)) = sight.first_hit(wv, observer, origin, dir, range) {
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
        "stock,sim_min,species,stands,foliage,reach,visible,visible_five_pitch,route,\
         route_seeded,f_reach,f_visible,f_visible_five_pitch,f_route,f_route_seeded"
    );
    println!("layer,sim_min,species,layer,stands,band_lo_m,band_hi_m,stock,reach,route,f_reach");
    println!("lineage,sim_min,founder,alive,heights,mean_m_to_reachable,share_2m");
    println!("detritus,sim_min,pool,organic,in_component,fraction");
    println!(
        "ledger,sim_min,flora_organic,flora_mineral,flora_energy,fauna_organic,fauna_mineral,\
         fauna_energy,water,flora_organic_total,flora_mineral_total,flora_energy_total"
    );
}

fn fraction(part: f64, whole: f64) -> f64 {
    if whole > 0.0 { part / whole } else { 0.0 }
}

fn print_species(minute: u64, name: &str, r: &SpeciesRow) {
    println!(
        "stock,{minute},{name},{},{:.6},{:.6},{:.6},{:.6},{:.6},{:.6},\
         {:.4},{:.4},{:.4},{:.4},{:.4}",
        r.stands,
        r.foliage,
        r.reach,
        r.visible,
        r.visible_five_pitch,
        r.route,
        r.route_seeded,
        fraction(r.reach, r.foliage),
        fraction(r.visible, r.foliage),
        fraction(r.visible_five_pitch, r.foliage),
        fraction(r.route, r.foliage),
        fraction(r.route_seeded, r.foliage),
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
    for l in &r.layers {
        println!(
            "layer,{},{},{},{},{:.4},{:.4},{:.6},{:.6},{:.6},{:.4}",
            r.minute,
            l.species.name(),
            l.index,
            l.stands,
            fraction(l.band_lo_m, l.stock),
            fraction(l.band_hi_m, l.stock),
            l.stock,
            l.reach,
            l.route,
            fraction(l.reach, l.stock),
        );
    }
    for (name, l) in &r.lineages {
        let heights: Vec<String> = l.heights.iter().map(|(y, n)| format!("{y}:{n}")).collect();
        println!(
            "lineage,{},{name},{},{},{},{}",
            r.minute,
            l.alive,
            heights.join(" "),
            optional(l.mean_distance_m, 3),
            optional(l.share_within_2m, 4),
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
        "ledger,{},{:.3e},{:.3e},{:.3e},{:.3e},{:.3e},{:.3e},{:.3e},{:.6},{:.6},{:.6}",
        r.minute,
        r.residuals[0],
        r.residuals[1],
        r.residuals[2],
        r.residuals[3],
        r.residuals[4],
        r.residuals[5],
        r.residuals[6],
        r.totals[0],
        r.totals[1],
        r.totals[2],
    );
}

fn print_summary(first: &Report, last: &Report) {
    println!("summary,metric,t0,final");
    let pairs: [(&str, f64, f64); 7] = [
        ("foliage", first.total.foliage, last.total.foliage),
        (
            "f_reach",
            fraction(first.total.reach, first.total.foliage),
            fraction(last.total.reach, last.total.foliage),
        ),
        (
            "f_visible",
            fraction(first.total.visible, first.total.foliage),
            fraction(last.total.visible, last.total.foliage),
        ),
        (
            "f_visible_five_pitch",
            fraction(first.total.visible_five_pitch, first.total.foliage),
            fraction(last.total.visible_five_pitch, last.total.foliage),
        ),
        (
            "f_route",
            fraction(first.total.route, first.total.foliage),
            fraction(last.total.route, last.total.foliage),
        ),
        (
            "f_route_seeded",
            fraction(first.total.route_seeded, first.total.foliage),
            fraction(last.total.route_seeded, last.total.foliage),
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
