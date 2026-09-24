//! **Package N: vaulttree, lanternberry and siphonreed**
//! (`design/handoffs/voxel-new-plants-2026-09-23.md`, "Tests"). Written before the
//! rule, by a separate pass, from the brief and `design/organism-anatomy-2026-09-21.md`
//! §3. The browse-line claim needs the fauna crate's mouth and lives in
//! `crates/cubarium-voxel-fauna/tests/new_plants_browse.rs`.
//!
//! ## The interface these tests assume
//!
//! The smallest additions the brief implies; everything else is today's API.
//!
//! - **Three new [`Species`] variants**, `Vaulttree`, `Lanternberry` and `Siphonreed`,
//!   named `"vaulttree"`, `"lanternberry"`, `"siphonreed"`, **appended** to
//!   [`Species::ALL`] (indices 6, 7, 8) so every ledger slot of the six live species
//!   keeps its meaning. `FloraConfig::species(s)` returns their configs; all three are
//!   [`Trophic::Photo`].
//! - **`SpeciesConfig::water_depth_min_m: f64`** (brief, model addition 2): the settled
//!   standing water, metres, the species needs on its site or a 4-neighbour. `0.0` is
//!   "no requirement" and is what the other eight species carry.
//! - **A settled-water reader on the core view:**
//!   `cubarium_voxel::VoxelView::standing_depth_m(&self, x: i64, y: u32, z: u32) -> f64`
//!   — the depth of **settled standing water** over the support face `(x, y, z)`: water
//!   held at rest, supported from below, and **zero for water in transit** (falling
//!   through or running off), which [`cubarium_voxel::VoxelView::water_depth_m`] counts
//!   (`design/handoffs/voxel-terrain-note-standing-depth-2026-09-22.md`). The name and
//!   home are this pass's guess at the smallest reader; the brief leaves the reader to the
//!   implementer. If it lands elsewhere or under another name, rename the one call in
//!   [`standing`] — nothing else here reads it, and the tests assert the germination
//!   predicate, not the reader.
//! - **The germination predicate carries the new gate:**
//!   [`cubarium_voxel_flora::establishment_gates`]`(..).passes()` (and so
//!   [`cubarium_voxel_flora::can_establish`]) refuses a species whose
//!   `water_depth_min_m` the settled water on its site and its 4-neighbours does not
//!   meet. The five existing gate fields keep their meaning; whether `Gates` grows a
//!   field for the new gate is the implementer's call and is not read here.
//! - **The fall** is laid by the one death path, `step`'s `die`, whatever killed the
//!   stand. The tests kill a vaulttree by **drowning** — a pool over its site deeper than
//!   `drown_depth_m`, one `Flora::step`, no world step — because that is a death in one
//!   tick; the wood at death is then exactly the seeded wood. They find the line by
//!   scanning `FloraView::ground` for `dead_wood > 0`, so the site list's order and the
//!   hash's inputs are free, with one assumption: **the direction depends on the site
//!   (and seeds and tick), not on the terrain around it**, so the same site in a world
//!   with some columns removed falls along the same line.
//! - Profiles are read off `SpeciesConfig::profile` as today (package L's
//!   `Profile { wood_fraction_max, height_m_max, layers }`); crowns through the resolved
//!   geometry, as `tests/ladder_growth.rs` reads them, plus the two metre fields
//!   `crown_height_m` / `crown_radius_m` that package L named.
//!
//! **Modality kept from the anatomy table.** Trunk porosity is asserted only where §3
//! gives one (vault limbs 0.85, the lanternberry stem fan 0.7). Layer order inside a
//! stage is not asserted. Contiguous foliage layers of one kind, radius and porosity are
//! **merged** before comparison, so an implementer may split the lanternberry's
//! `0.3–1.0` foliage at the browse line (one way to express the brief's accepted browse
//! pattern) without failing the anatomy match.
//!
//! ## Status before the implementation
//!
//! This file **does not compile** today: `Species::Vaulttree`, `Species::Lanternberry`,
//! `Species::Siphonreed`, `SpeciesConfig::water_depth_min_m` and
//! `VoxelView::standing_depth_m` do not exist. Once they do, every test here is expected
//! to pass only with the rule in place — the fall and the standing-water gate are new
//! behaviour, and the profiles and sizes are new data.
//!
//! No world here is stepped more than 30 ticks; most are not stepped at all.

use cubarium_voxel::{Command as WorldCommand, Config as VoxelConfig, Material, World};
use cubarium_voxel_flora::{
    Command, Flora, FloraConfig, Layer, LayerKind, Profile, Site, Species, Trophic,
    establishment_gates,
};

/// The two shipped voxel sizes: `small` / `desktop` (0.125 m) and `default` (0.25 m).
const GRIDS: [f64; 2] = [0.125, 0.25];

/// Package L's seedling ceiling on height and radius, metres.
const SEEDLING_M: f64 = 0.125;

/// The brief's table: `(species, height m [min, max], radius m [min, max])`.
const TABLE: [(Species, [f64; 2], [f64; 2]); 3] = [
    (Species::Vaulttree, [2.25, 3.5], [0.625, 1.0]),
    (Species::Lanternberry, [0.625, 1.125], [0.25, 0.4375]),
    (Species::Siphonreed, [0.75, 1.5], [0.0625, 0.125]),
];

const NEW: [Species; 3] = [
    Species::Vaulttree,
    Species::Lanternberry,
    Species::Siphonreed,
];

fn close(got: f64, want: f64, tol: f64) -> bool {
    (got - want).abs() <= tol
}

// ------------------------------------------------------------------------ fixtures

const SUPPORT: u32 = 2;

/// A flat plain of soil in `1..=SUPPORT`, `width × depth` columns, tall enough for a
/// 3.5 m crown and a pool over the support face on either grid.
fn plain(voxel_m: f64, width: u32, depth: u32) -> World {
    let height = ((4.0 / voxel_m).ceil() as u32) + SUPPORT + 2;
    let mut w = World::empty(VoxelConfig {
        width,
        height,
        depth,
        voxel_m,
        seed: 29,
        ..VoxelConfig::default()
    });
    for z in 0..depth {
        for x in 0..i64::from(width) {
            for y in 1..=SUPPORT {
                w.apply(WorldCommand::SetMaterial {
                    x,
                    y,
                    z,
                    material: Material::Soil,
                });
            }
        }
    }
    w
}

/// Turn one **air** voxel into soil holding exactly `pore` of its capacity
/// (`tests/flora.rs`'s fixture).
fn wet_soil(w: &mut World, x: i64, y: u32, z: u32, pore: f64) {
    let cap = Material::Soil.pore_capacity() * w.config().voxel_volume();
    let got = w.apply(WorldCommand::AddWater {
        x,
        y,
        z,
        volume_m3: pore * cap,
    });
    assert!((got - pore * cap).abs() < 1e-12, "the void took {got}");
    w.apply(WorldCommand::SetMaterial {
        x,
        y,
        z,
        material: Material::Soil,
    });
    assert!((w.view().pore_at(x, y, z) - pore).abs() < 1e-12);
    assert_eq!(w.view().free_at(x, y, z), 0.0, "nothing left standing");
}

/// Fill the void cells over `site` with whole cells of water until the free water over
/// its face is deeper than `depth_m`: a pool, placed directly, with no world step.
fn flood(w: &mut World, site: Site, depth_m: f64) {
    let v = w.config().voxel_m;
    let cell = w.config().voxel_volume();
    let mut y = site.y + 1;
    while !(w.view().water_depth_m(i64::from(site.x), site.y, site.z) > depth_m) {
        assert!(
            y + 1 < w.config().height,
            "the fixture is too short to drown past {depth_m} m at {v} m voxels"
        );
        w.apply(WorldCommand::AddWater {
            x: i64::from(site.x),
            y,
            z: site.z,
            volume_m3: cell,
        });
        y += 1;
    }
}

// ------------------------------------------------------------------ the species exist

/// The three are species of the model: in [`Species::ALL`], named, parseable, plants,
/// appended after the six live species (whose ledger indices do not move), and their
/// configs validate at both grids.
#[test]
fn vaulttree_lanternberry_and_siphonreed_are_species() {
    assert_eq!(Species::COUNT, 9);
    let old = [
        Species::Bloomcrown,
        Species::Umbrellafrond,
        Species::Springturf,
        Species::Stonecushion,
        Species::Velvetpad,
        Species::Glowcap,
    ];
    for (i, s) in old.into_iter().enumerate() {
        assert_eq!(s.index(), i, "{s:?}'s ledger slot moved");
    }
    for (s, name) in NEW
        .into_iter()
        .zip(["vaulttree", "lanternberry", "siphonreed"])
    {
        assert!(Species::ALL.contains(&s), "{s:?} is not in ALL");
        assert!(s.index() >= old.len(), "{s:?} must be appended");
        assert_eq!(Species::ALL[s.index()], s);
        assert_eq!(s.name(), name);
        assert_eq!(Species::parse(name), Some(s));
        assert_eq!(FloraConfig::default().species(s).trophic, Trophic::Photo);
    }
    for voxel_m in GRIDS {
        FloraConfig::for_voxel_size(voxel_m)
            .validate()
            .unwrap_or_else(|e| panic!("{voxel_m} m config: {e}"));
    }
}

// ------------------------------------------------------------------------- profiles

/// One anatomy-table layer: `share` and `porosity` are `None` where §3 is silent.
#[derive(Debug, Clone, Copy)]
struct Want {
    kind: LayerKind,
    band: [f64; 2],
    radius: f64,
    share: Option<f64>,
    porosity: Option<f64>,
}

fn trunk(band: [f64; 2], radius: f64, porosity: Option<f64>) -> Want {
    Want {
        kind: LayerKind::Trunk,
        band,
        radius,
        share: None,
        porosity,
    }
}

fn leaves(kind: LayerKind, band: [f64; 2], radius: f64, share: f64, porosity: f64) -> Want {
    Want {
        kind,
        band,
        radius,
        share: Some(share),
        porosity: Some(porosity),
    }
}

/// The stage's layers with contiguous foliage-bearing layers of one kind, radius and
/// porosity merged into one (bands joined, shares summed): the anatomy's layer, however
/// the implementer chose to cut it.
fn merged(stage: &Profile) -> Vec<Layer> {
    let mut foliage: Vec<Layer> = stage
        .layers
        .iter()
        .copied()
        .filter(|l| l.kind.bears_foliage())
        .collect();
    foliage.sort_by(|a, b| a.band[0].total_cmp(&b.band[0]));
    let mut out: Vec<Layer> = stage
        .layers
        .iter()
        .copied()
        .filter(|l| !l.kind.bears_foliage())
        .collect();
    let mut run: Option<Layer> = None;
    for l in foliage {
        run = match run {
            Some(mut r)
                if r.kind == l.kind
                    && close(r.radius, l.radius, 1e-12)
                    && close(r.porosity, l.porosity, 1e-12)
                    && close(r.band[1], l.band[0], 1e-9) =>
            {
                r.band[1] = l.band[1];
                r.share += l.share;
                Some(r)
            }
            Some(r) => {
                out.push(r);
                Some(l)
            }
            None => Some(l),
        };
    }
    out.extend(run);
    out
}

fn matches(l: &Layer, w: &Want) -> bool {
    l.kind == w.kind
        && close(l.band[0], w.band[0], 1e-9)
        && close(l.band[1], w.band[1], 1e-9)
        && close(l.radius, w.radius, 1e-9)
        && w.share.is_none_or(|s| close(l.share, s, 1e-9))
        && w.porosity.is_none_or(|p| close(l.porosity, p, 1e-9))
}

/// Every wanted layer is present exactly once and nothing else is.
fn assert_stage(species: Species, i: usize, stage: &Profile, want: &[Want]) {
    let got = merged(stage);
    assert_eq!(
        got.len(),
        want.len(),
        "{species:?} stage {i}: {got:?}\nwant {want:?}"
    );
    for w in want {
        let n = got.iter().filter(|l| matches(l, w)).count();
        assert_eq!(n, 1, "{species:?} stage {i}: {w:?} in {got:?}");
    }
}

/// Anatomy §3, vaulttree: seedling ≤ 0.1 a capped rosette; juvenile ≤ 0.4 trunk and
/// crown; adult trunk, limbs (a sparse Trunk, p 0.85), lobes with sky gaps (p 0.45) and
/// hanging drape (p 0.9).
#[test]
fn vaulttree_profile_is_the_anatomy_table() {
    let sc = FloraConfig::default().species(Species::Vaulttree).clone();
    assert_eq!(sc.profile.len(), 3, "seedling, juvenile, adult");
    assert_eq!(sc.capped_seedling(), Some((0.1, SEEDLING_M)));
    assert!(close(sc.profile[1].wood_fraction_max, 0.4, 1e-12));
    assert!(sc.profile[2].wood_fraction_max >= 1.0);
    use LayerKind::{Drape, Foliage};
    assert_stage(
        Species::Vaulttree,
        0,
        &sc.profile[0],
        &[leaves(Foliage, [0.0, 1.0], 1.0, 1.0, 0.4)],
    );
    assert_stage(
        Species::Vaulttree,
        1,
        &sc.profile[1],
        &[
            trunk([0.0, 0.6], 0.15, None),
            leaves(Foliage, [0.6, 1.0], 1.0, 1.0, 0.4),
        ],
    );
    assert_stage(
        Species::Vaulttree,
        2,
        &sc.profile[2],
        &[
            trunk([0.0, 0.5], 0.15, None),
            trunk([0.5, 0.7], 0.6, Some(0.85)),
            leaves(Foliage, [0.7, 1.0], 1.0, 0.85, 0.45),
            leaves(Drape, [0.35, 0.5], 0.7, 0.15, 0.9),
        ],
    );
}

/// Anatomy §3, lanternberry: seedling ≤ 0.2 a capped rosette (brief: "capped"); adult a
/// fan of stems (Trunk r 0.4, p 0.7) under foliage 0.3–1.0, p 0.5. No juvenile stage.
#[test]
fn lanternberry_profile_is_the_anatomy_table() {
    let sc = FloraConfig::default()
        .species(Species::Lanternberry)
        .clone();
    assert_eq!(sc.profile.len(), 2, "seedling and adult");
    assert_eq!(sc.capped_seedling(), Some((0.2, SEEDLING_M)));
    assert!(sc.profile[1].wood_fraction_max >= 1.0);
    assert_stage(
        Species::Lanternberry,
        0,
        &sc.profile[0],
        &[leaves(LayerKind::Foliage, [0.0, 1.0], 1.0, 1.0, 0.5)],
    );
    assert_stage(
        Species::Lanternberry,
        1,
        &sc.profile[1],
        &[
            trunk([0.0, 0.3], 0.4, Some(0.7)),
            leaves(LayerKind::Foliage, [0.3, 1.0], 1.0, 1.0, 0.5),
        ],
    );
}

/// Anatomy §3, siphonreed: one stage, a porous column, no trunk and no seedling cap.
#[test]
fn siphonreed_profile_is_one_porous_column() {
    let sc = FloraConfig::default().species(Species::Siphonreed).clone();
    assert_eq!(sc.profile.len(), 1, "one stage for the whole life");
    assert!(sc.profile[0].wood_fraction_max >= 1.0);
    assert_eq!(sc.capped_seedling(), None);
    assert_stage(
        Species::Siphonreed,
        0,
        &sc.profile[0],
        &[leaves(LayerKind::Foliage, [0.0, 1.0], 1.0, 1.0, 0.7)],
    );
}

// ------------------------------------------------------------------- sizes in metres

#[derive(Debug, Clone, Copy)]
struct Crown {
    height_m: f64,
    radius_m: f64,
    stage: usize,
}

/// Seed one stand at `wood` and read its crown back out of its resolved layers
/// (`tests/ladder_growth.rs`'s reading: the exact inverse of `layers_of`).
fn crown(voxel_m: f64, species: Species, wood: f64) -> Crown {
    let world = plain(voxel_m, 4, 1);
    let config = FloraConfig::for_voxel_size(voxel_m);
    let sc = config.species(species).clone();
    let mut flora = Flora::new(config);
    assert!(
        flora.apply(
            &world,
            Command::Seed {
                x: 1,
                z: 0,
                species,
                wood,
            },
        ),
        "{species:?} at wood {wood} refused"
    );
    let site = Site {
        x: 1,
        y: SUPPORT,
        z: 0,
    };
    let view = flora.view();
    let stand = view.stand_at(site).expect("the seeded stand");
    let resolved = view.profile_layers(stand);
    let stage = sc.profile_at(wood);
    assert_eq!(resolved.len(), stage.layers.len(), "every layer resolved");
    let base_m = f64::from(site.y) * voxel_m;
    let (top_i, top) = stage
        .layers
        .iter()
        .enumerate()
        .max_by(|a, b| a.1.band[1].total_cmp(&b.1.band[1]))
        .unwrap();
    let (wide_i, wide) = stage
        .layers
        .iter()
        .enumerate()
        .max_by(|a, b| a.1.radius.total_cmp(&b.1.radius))
        .unwrap();
    Crown {
        height_m: (resolved[top_i].band_m[1] - base_m) / top.band[1],
        radius_m: resolved[wide_i].radius_m / wide.radius,
        stage: sc.profile_index(wood),
    }
}

/// The brief's table in metres: the two fields hold it, and the resolved crown is at
/// `[h_max, r_max]` full-grown on both grids. The two capped species grow **from the
/// seedling** (package L §2): at most 0.125 m in both while a seedling, ≈ 0.125 m just
/// past it. Siphonreed, uncapped, grows linearly from `[h_min, r_min]`.
#[test]
fn new_species_crowns_are_the_brief_s_table_in_metres() {
    for (species, h, r) in TABLE {
        let sc = FloraConfig::default().species(species).clone();
        assert_eq!(sc.crown_height_m, h, "{species:?} crown_height_m");
        assert_eq!(sc.crown_radius_m, r, "{species:?} crown_radius_m");
        let wmax = sc.wood_max;
        for voxel_m in GRIDS {
            let full = crown(voxel_m, species, wmax);
            assert!(
                close(full.height_m, h[1], 1e-9) && close(full.radius_m, r[1], 1e-9),
                "{species:?} full-grown on {voxel_m} m: {full:?}"
            );
            match sc.capped_seedling() {
                Some((w0, cap)) => {
                    assert_eq!(cap, SEEDLING_M);
                    let seedling = crown(
                        voxel_m,
                        species,
                        (w0 * (1.0 - 1e-9) * wmax).max(sc.alive_min),
                    );
                    assert_eq!(seedling.stage, 0, "{species:?}: the fixture is a seedling");
                    assert!(
                        seedling.height_m <= SEEDLING_M + 1e-9
                            && seedling.radius_m <= SEEDLING_M + 1e-9,
                        "{species:?} seedling on {voxel_m} m: {seedling:?}"
                    );
                    let past = crown(voxel_m, species, w0 * (1.0 + 1e-6) * wmax);
                    assert!(past.stage >= 1);
                    assert!(
                        close(past.height_m, SEEDLING_M, 1e-3)
                            && close(past.radius_m, SEEDLING_M, 1e-3),
                        "{species:?} just past the seedling on {voxel_m} m: {past:?}"
                    );
                }
                None => {
                    for wf in [sc.alive_min / wmax, 0.5] {
                        let c = crown(voxel_m, species, wf * wmax);
                        let want_h = h[0] + wf * (h[1] - h[0]);
                        let want_r = r[0] + wf * (r[1] - r[0]);
                        assert!(
                            close(c.height_m, want_h, 1e-9) && close(c.radius_m, want_r, 1e-9),
                            "{species:?} at wf {wf} on {voxel_m} m: {c:?}, want h {want_h} r {want_r}"
                        );
                    }
                }
            }
        }
    }
}

// -------------------------------------------------------------------------- the fall

/// What one vaulttree's death left: every ground site holding dead wood, and the stand's
/// wood and crown radius at death.
struct Fall {
    origin: Site,
    wood: f64,
    radius_m: f64,
    logs: Vec<(Site, f64)>,
}

/// Seed a vaulttree of `wood` at the centre of `world`, drown it, step the flora once.
fn fall(world: &mut World, wood: f64) -> Fall {
    let voxel_m = world.config().voxel_m;
    let (w, d) = (world.config().width, world.config().depth);
    let config = FloraConfig::for_voxel_size(voxel_m);
    let sc = config.species(Species::Vaulttree).clone();
    let mut flora = Flora::new(config);
    let origin = Site {
        x: w / 2,
        y: SUPPORT,
        z: d / 2,
    };
    assert!(flora.apply(
        world,
        Command::Seed {
            x: i64::from(origin.x),
            z: origin.z,
            species: Species::Vaulttree,
            wood,
        },
    ));
    assert_eq!(flora.view().stand_at(origin).map(|s| s.wood), Some(wood));
    for g in flora.view().ground {
        assert_eq!(g.dead_wood, 0.0, "no dead wood before the death");
    }
    flood(world, origin, sc.drown_depth_m);
    let organic_before = flora.view().organic();
    flora.step(world);
    let view = flora.view();
    assert!(view.stand_at(origin).is_none(), "the vaulttree drowned");
    assert_eq!(view.ledger.deaths, 1);
    let residual = view.organic() - view.ledger.expected_organic();
    assert!(
        residual.abs() <= 1e-9 * organic_before.max(1.0),
        "organic residual {residual} after the fall"
    );
    let logs = view
        .ground
        .iter()
        .filter(|g| g.dead_wood > 0.0)
        .map(|g| (g.site, g.dead_wood))
        .collect();
    Fall {
        origin,
        wood,
        radius_m: sc.crown_radius_m_at(wood),
        logs,
    }
}

/// The brief's site count, `round(crown_radius / voxel)`, never fewer than one: the wood
/// has to land somewhere.
fn sites_for(radius_m: f64, voxel_m: f64) -> usize {
    ((radius_m / voxel_m).round() as usize).max(1)
}

/// Offset of `s` from `o` in voxels, `(dx, dz)`. The fixtures centre the origin, so
/// nothing wraps.
fn offset(o: Site, s: Site) -> (f64, f64) {
    (
        f64::from(s.x) - f64::from(o.x),
        f64::from(s.z) - f64::from(o.z),
    )
}

/// The logs sorted outward from the origin.
fn outward(f: &Fall) -> Vec<(Site, f64)> {
    let mut logs = f.logs.clone();
    logs.sort_by(|a, b| {
        let (ax, az) = offset(f.origin, a.0);
        let (bx, bz) = offset(f.origin, b.0);
        (ax * ax + az * az).total_cmp(&(bx * bx + bz * bz))
    });
    logs
}

/// Brief, model addition 1: at death a vaulttree's wood is laid as dead wood on
/// `round(crown_radius / voxel)` sites **along one line** from its site, each an equal
/// share, and the line's total is the wood at death. On a fully supported plain, on both
/// grids, full-grown and juvenile. The direction is the model's; the test only asks that
/// the sites are collinear with the origin, all on one side of it, and no further than
/// the count reaches.
#[test]
fn a_vaulttree_falls_along_round_r_over_voxel_sites_and_the_line_holds_its_wood() {
    for voxel_m in GRIDS {
        for wf in [1.0, 0.3] {
            let wood_max = FloraConfig::default().species(Species::Vaulttree).wood_max;
            let n_max = sites_for(1.0, voxel_m);
            let side = 2 * n_max as u32 + 5;
            let mut world = plain(voxel_m, side, side);
            let f = fall(&mut world, wf * wood_max);
            let n = sites_for(f.radius_m, voxel_m);
            let what = format!("wf {wf} on {voxel_m} m (r {} m, n {n})", f.radius_m);

            assert_eq!(f.logs.len(), n, "{what}: logs {:?}", f.logs);
            let total: f64 = f.logs.iter().map(|l| l.1).sum();
            assert!(
                close(total, f.wood, 1e-9 * f.wood.max(1.0)),
                "{what}: the line holds {total}, the wood at death was {}",
                f.wood
            );
            for (s, w) in &f.logs {
                assert_eq!(s.y, SUPPORT, "{what}: a log off the plain at {s:?}");
                assert!(
                    close(*w, f.wood / n as f64, 1e-9),
                    "{what}: {s:?} holds {w}, an equal share is {}",
                    f.wood / n as f64
                );
            }

            let logs = outward(&f);
            let (fx, fz) = offset(f.origin, logs.last().unwrap().0);
            let len = (fx * fx + fz * fz).sqrt();
            for (s, _) in &logs {
                let (dx, dz) = offset(f.origin, *s);
                assert!(
                    dx.abs().max(dz.abs()) <= n as f64,
                    "{what}: {s:?} is further than {n} sites from the origin"
                );
                if len > 0.0 {
                    let along = (dx * fx + dz * fz) / len;
                    let across = (dx * fz - dz * fx).abs() / len;
                    assert!(
                        along >= -1e-9,
                        "{what}: {s:?} is behind the origin; a fall has one direction"
                    );
                    assert!(
                        across <= 0.75,
                        "{what}: {s:?} is {across} voxels off the line to {:?}",
                        logs.last().unwrap().0
                    );
                }
            }
        }
    }
}

/// Brief, model addition 1: "a site without support gives its share to the nearest
/// supported one on the line". Fall once on the full plain to learn the line, then fall
/// again from the same site in the same world with the **far half** of the line's
/// columns removed (a cliff edge: no support anywhere in those columns). Every removed
/// site's share lands on the last supported site before the edge; the total is still the
/// wood at death.
#[test]
fn an_unsupported_site_s_share_goes_to_the_nearest_supported_site_on_the_line() {
    for voxel_m in GRIDS {
        let wood_max = FloraConfig::default().species(Species::Vaulttree).wood_max;
        let n = sites_for(1.0, voxel_m);
        assert!(n >= 2, "the fixture needs a line of at least two sites");
        let side = 2 * n as u32 + 5;

        let mut open = plain(voxel_m, side, side);
        let line = outward(&fall(&mut open, wood_max));
        assert_eq!(line.len(), n, "{voxel_m} m: the full-plain line");

        let keep = n / 2;
        let mut cliff = plain(voxel_m, side, side);
        for (s, _) in &line[keep..] {
            for y in 0..cliff.config().height {
                cliff.apply(WorldCommand::SetMaterial {
                    x: i64::from(s.x),
                    y,
                    z: s.z,
                    material: Material::Air,
                });
            }
            assert!(
                cliff
                    .view()
                    .supports_in_column(i64::from(s.x), s.z)
                    .is_empty()
            );
        }
        let f = fall(&mut cliff, wood_max);
        let share = f.wood / n as f64;
        let what = format!("{voxel_m} m, {n} sites, the last {} removed", n - keep);

        let total: f64 = f.logs.iter().map(|l| l.1).sum();
        assert!(
            close(total, f.wood, 1e-9 * f.wood.max(1.0)),
            "{what}: total {total}"
        );
        assert_eq!(f.logs.len(), keep, "{what}: logs {:?}", f.logs);
        for (i, (s, _)) in line[..keep].iter().enumerate() {
            let got = f.logs.iter().find(|l| l.0 == *s).map_or(0.0, |l| l.1);
            let want = if i + 1 == keep {
                share * (1 + n - keep) as f64
            } else {
                share
            };
            assert!(
                close(got, want, 1e-9),
                "{what}: {s:?} (line site {i}) holds {got}, want {want}"
            );
        }
    }
}

/// Brief, model addition 1: "… or the origin". A vaulttree on a one-column pillar —
/// every other column of the world removed — has no supported site on any line, and its
/// whole wood lands on its own site.
#[test]
fn with_no_supported_site_on_the_line_the_fall_lands_on_the_origin() {
    for voxel_m in GRIDS {
        let wood_max = FloraConfig::default().species(Species::Vaulttree).wood_max;
        let side = 2 * sites_for(1.0, voxel_m) as u32 + 5;
        let mut world = plain(voxel_m, side, side);
        let origin = (side / 2, side / 2);
        for z in 0..side {
            for x in 0..side {
                if (x, z) == origin {
                    continue;
                }
                for y in 0..=SUPPORT {
                    world.apply(WorldCommand::SetMaterial {
                        x: i64::from(x),
                        y,
                        z,
                        material: Material::Air,
                    });
                }
            }
        }
        let f = fall(&mut world, wood_max);
        assert_eq!(
            f.logs.len(),
            1,
            "{voxel_m} m: one log on a pillar: {:?}",
            f.logs
        );
        assert_eq!(f.logs[0].0, f.origin);
        assert!(
            close(f.logs[0].1, f.wood, 1e-9 * f.wood.max(1.0)),
            "{voxel_m} m: the origin holds {} of {}",
            f.logs[0].1,
            f.wood
        );
    }
}

// ------------------------------------------------------------------ standing water

/// The one call to the settled-water reader (see the module doc).
fn standing(world: &World, x: i64, y: u32, z: u32) -> f64 {
    world.view().standing_depth_m(x, y, z)
}

/// Which water the basin beside the reed's site holds.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Basin {
    /// None at all: the control that shows the gate exists.
    Dry,
    /// A pool of whole cells in a bedrock basin, stepped 30 ticks with no rain: at rest.
    Settled,
    /// Water running over a lip beside the site and falling off it: a spout over the lip
    /// fed for three ticks (by then a flow-through film a fifth of a cell deep on the lip,
    /// the landed water running off to the floor below), then a column of cells 0.3 full
    /// over the film, as a shower leaves one mid-fall (the standing-depth note's
    /// `0.11 / 0.25 / 0.37 …`). The world is not stepped after the column is placed.
    ///
    /// Measured on today's solver: the film's bottom cell is 0.19 full and
    /// `water_depth_m` reads the lip at 0.27 m. Any reader that stops at unsupported water
    /// sees at most that bottom cell, about 0.05 m; the fixture therefore assumes
    /// `water_depth_min_m` is a pool's depth and not a rain film's (bloomcrown's
    /// `drown_depth_m` doc is the precedent: 0.05 m means a pool).
    Transit,
}

/// A strip one cell deep at 0.25 m (x wraps). Returns the world, the reed's site and the
/// neighbouring support face the water is on.
///
/// - `x = 2`: the reed's column, soil `1..=k` at pore 0.98 (saturated), face `y = k`;
/// - `x = 3`: the basin, bedrock floor, face `y = 0` — or, in the transit fixture, a
///   bedrock lip, face `y = 1`;
/// - `x = 4`: a bedrock wall `1..=k+1` holding the pool — **absent** in the transit
///   fixture, so what lands on the lip runs off to the open floor `x = 4..=7, 0, 1`.
///
/// While the world steps the reed's column is bedrock (the pool's other wall; bedrock
/// holds no pore water), so no water seeps into it; it is turned to wet soil only
/// afterwards, with no step between that and the reading. `k` makes the pool at least twice the reed's `water_depth_min_m`,
/// level with the reed's face.
fn bank(basin: Basin) -> (World, Site, Site) {
    let voxel_m = 0.25;
    let min = FloraConfig::for_voxel_size(voxel_m)
        .species(Species::Siphonreed)
        .water_depth_min_m;
    assert!(min > 0.0, "siphonreed needs standing water");
    let k = ((2.0 * min / voxel_m).ceil() as u32).max(2);
    let column = (2.0 * min / (0.3 * voxel_m)).ceil() as u32;
    let spout = column + 5;
    let mut w = World::empty(VoxelConfig {
        width: 8,
        height: (spout + 2).max(k + 3),
        depth: 1,
        voxel_m,
        seed: 31,
        ..VoxelConfig::default()
    });
    let solid = |w: &mut World, x: i64, y: u32| {
        w.apply(WorldCommand::SetMaterial {
            x,
            y,
            z: 0,
            material: Material::Bedrock,
        });
    };
    for y in 1..=k {
        solid(&mut w, 2, y);
    }
    let cell = w.config().voxel_volume();
    match basin {
        Basin::Dry => {
            for y in 1..=k + 1 {
                solid(&mut w, 4, y);
            }
        }
        Basin::Settled => {
            for y in 1..=k + 1 {
                solid(&mut w, 4, y);
            }
            for y in 1..=k {
                w.apply(WorldCommand::AddWater {
                    x: 3,
                    y,
                    z: 0,
                    volume_m3: cell,
                });
            }
            for _ in 0..30 {
                w.step();
            }
        }
        Basin::Transit => {
            solid(&mut w, 3, 1);
            for _ in 0..3 {
                w.apply(WorldCommand::AddWater {
                    x: 3,
                    y: spout,
                    z: 0,
                    volume_m3: 0.5 * cell,
                });
                w.step();
            }
            // The column falling onto the film: every cell above the film's bottom one
            // topped up to 0.3 full.
            for y in 3..3 + column {
                let free = w.view().free_at(3, y, 0);
                w.apply(WorldCommand::AddWater {
                    x: 3,
                    y,
                    z: 0,
                    volume_m3: (0.3 - free).max(0.0) * cell,
                });
            }
        }
    }
    for y in 1..=k {
        w.apply(WorldCommand::SetMaterial {
            x: 2,
            y,
            z: 0,
            material: Material::Air,
        });
        wet_soil(&mut w, 2, y, 0, 0.98);
    }
    let beside_y = if basin == Basin::Transit { 1 } else { 0 };
    (
        w,
        Site { x: 2, y: k, z: 0 },
        Site {
            x: 3,
            y: beside_y,
            z: 0,
        },
    )
}

/// Brief, model addition 2: a siphonreed establishes beside **settled** standing water at
/// least `water_depth_min_m` deep, and not where the only water beside it is in transit —
/// even though [`cubarium_voxel::VoxelView::water_depth_m`] reads the transit column as
/// deep enough — and not beside a dry basin. Every other gate passes in all three
/// fixtures (saturated soil, which the reed tolerates at `establish_saturated_max` 1.0;
/// open sky; a dry face of its own), so the refusal is the standing-water gate's.
#[test]
fn a_siphonreed_establishes_beside_settled_standing_water_and_not_beside_transit_water() {
    let config = FloraConfig::for_voxel_size(0.25);
    let sc = config.species(Species::Siphonreed).clone();
    let min = sc.water_depth_min_m;
    assert_eq!(
        sc.establish_saturated_max, 1.0,
        "saturation-tolerant, as the frond"
    );
    assert!(
        sc.drown_depth_m > min,
        "it stands in the water it needs: drown {} vs min {min}",
        sc.drown_depth_m
    );
    for other in Species::ALL {
        if other != Species::Siphonreed {
            assert_eq!(
                config.species(other).water_depth_min_m,
                0.0,
                "{other:?} has no standing-water requirement"
            );
        }
    }

    let mut verdicts = Vec::new();
    for basin in [Basin::Dry, Basin::Settled, Basin::Transit] {
        let (world, site, face) = bank(basin);
        let view = world.view();
        assert!(view.is_support(i64::from(face.x), face.y, face.z));
        let g = establishment_gates(&view, site, &sc);
        assert!(
            g.pore_ok && g.aeration_ok && g.depth_ok && g.light_ok && g.substrate_ok,
            "{basin:?}: a gate other than standing water shut: {g:?}"
        );
        let beside = view.water_depth_m(i64::from(face.x), face.y, face.z);
        let settled = standing(&world, i64::from(face.x), face.y, face.z);
        match basin {
            Basin::Dry => assert_eq!(beside, 0.0),
            Basin::Settled => assert!(
                settled >= min,
                "the settled pool reads {settled} m standing, want ≥ {min}"
            ),
            Basin::Transit => {
                assert!(
                    beside >= min,
                    "fixture: water_depth_m must be fooled by the column ({beside} m < {min})"
                );
                assert!(
                    settled < min,
                    "water in transit read as {settled} m of standing water"
                );
            }
        }
        verdicts.push((basin, g.passes()));
    }
    assert_eq!(
        verdicts,
        vec![
            (Basin::Dry, false),
            (Basin::Settled, true),
            (Basin::Transit, false)
        ],
        "establishes beside settled water only"
    );
}
