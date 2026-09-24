//! **The latticevine face cover**, written before the rule: the fifteen behaviour tests of
//! `design/handoffs/latticevine-cover-sim-2026-09-23.md`, against the API sketch in
//! `src/cover.rs`. The spec is D15 Revision 2
//! (`design/art-direction/species-dossier-D15-latticevine-2026-09-23.md`).
//!
//! Every world here is hand-built and small: a bedrock floor, soil at `y = 1..=2`, and rock
//! walls, ledges or overhangs placed by hand, on the 0.25 m reference voxel. Most tests
//! use a **wall at `x = 6`** whose west faces (`FaceDir::NegX`, front voxel at `x = 5`)
//! carry the cover, rooted at the **foot** `(5, 2, z)`.
//!
//! Soil water is written through the material's own `wilting_point` / `field_capacity`
//! (the package-F available-water scale), never as a pore literal. Rain is read the way
//! the model reads it, `VoxelView::is_raining`: two worlds with the same terrain and soil,
//! one open-budget world with a positive rain rate (raining) and one without, and the
//! plant layer — which borrows its world per call — stepped against whichever the test
//! needs. The same twin trick switches the root between wet and dry soil.
//!
//! Config helpers set every vine number a test depends on explicitly, so the implementer's
//! defaults change none of these results. `FloraView::organic`, `mineral` and `energy`
//! must count the vines: the residual checks here are the crate's own closure rule.

use std::collections::{BTreeMap, BTreeSet};

use cubarium_voxel::{Command as WorldCommand, Config as VoxelConfig, Material, World};
use cubarium_voxel_flora::{
    Command, CoveredFace, DT, Face, FaceDir, Flora, FloraConfig, Site, Species, SpurPhase, Vine,
    VineConfig, VineId, VineSeed, face_eligible, face_light, snapshot,
};

// ------------------------------------------------------------------ fixtures

/// A hand-built floor plan: dimensions and the solids laid over the soil floor, last write
/// wins. Built into a [`World`] as many times as a test needs twins.
#[derive(Clone)]
struct Plan {
    width: u32,
    height: u32,
    depth: u32,
    cells: BTreeMap<(i64, u32, u32), Material>,
}

impl Plan {
    /// Soil at `y = 1..=2` everywhere over the bedrock row.
    fn new(width: u32, height: u32, depth: u32) -> Plan {
        let mut p = Plan {
            width,
            height,
            depth,
            cells: BTreeMap::new(),
        };
        for x in 0..width as i64 {
            for z in 0..depth {
                for y in 1..=2 {
                    p.set(x, y, z, Material::Soil);
                }
            }
        }
        p
    }

    fn set(&mut self, x: i64, y: u32, z: u32, m: Material) {
        self.cells.insert((x, y, z), m);
    }

    /// A rock wall at `x = 6`, `y = 1..=top`, across every `z`.
    fn wall(width: u32, height: u32, depth: u32, top: u32) -> Plan {
        let mut p = Plan::new(width, height, depth);
        for z in 0..depth {
            for y in 1..=top {
                p.set(6, y, z, Material::Rock);
            }
        }
        p
    }

    /// The world: every soil voxel holding available water `avail` (0 = wilting point,
    /// 1 = field capacity); rock dry; raining every tick when `raining`.
    fn world(&self, avail: f64, raining: bool) -> World {
        let mut w = World::empty(VoxelConfig {
            width: self.width,
            height: self.height,
            depth: self.depth,
            voxel_m: 0.25,
            seed: 11,
            rain_m_per_s: if raining { 1e-6 } else { 0.0 },
            ..VoxelConfig::default()
        });
        let (wp, fc) = (
            Material::Soil.wilting_point(),
            Material::Soil.field_capacity(),
        );
        let pore = wp + avail * (fc - wp);
        let cap = Material::Soil.pore_capacity() * w.config().voxel_volume();
        for (&(x, y, z), &m) in &self.cells {
            if m == Material::Soil && pore > 0.0 {
                let got = w.apply(WorldCommand::AddWater {
                    x,
                    y,
                    z,
                    volume_m3: pore * cap,
                });
                assert!(
                    (got - pore * cap).abs() < 1e-15,
                    "fixture: the void took {got}"
                );
            }
            w.apply(WorldCommand::SetMaterial {
                x,
                y,
                z,
                material: m,
            });
        }
        w
    }
}

/// The west face of the wall at height `y`, slab `z`.
fn wf(y: u32, z: u32) -> Face {
    Face::new(6, y, z, FaceDir::NegX)
}

/// The wall's foot on slab `z`: the soil support face whose air voxel fronts `wf(3, z)`.
fn foot(z: u32) -> Site {
    Site { x: 5, y: 2, z }
}

fn ticks(seconds: f64) -> usize {
    (seconds / DT).round() as usize
}

/// Every vine number set explicitly: fast enough that a few hundred ticks show the rule,
/// litter never decomposes (so litter totals are exact), and nothing else in the layer
/// moves. Wet soil (available 1) is above `wet_water`; dry soil (available 0) is below
/// `dry_water`.
fn fast() -> FloraConfig {
    let mut c = FloraConfig {
        decomposition: 0.0,
        wood_decomposition: 0.0,
        carrion_decomposition: 0.0,
        ..FloraConfig::default()
    };
    c.latticevine = VineConfig {
        rooting_depth: 2,
        rooting_radius: 1,
        transpiration_m3_per_s: 1e-9,
        wilt_water: 0.1,
        full_water: 0.5,
        dry_water: 0.2,
        wet_water: 0.5,
        dry_spell_s: 0.5,
        wet_spell_s: 1.0,
        leaf_fall_s: 0.5,
        assimilation: 20.0,
        underside_light: 0.15,
        leaf_mass: 0.1,
        runner_mass: 0.05,
        leaf_upkeep: 0.01,
        wood_upkeep: 0.001,
        regrow_rate: 1.0,
        spread_cost: 0.2,
        spread_check_s: DT,
        sister_cost: 0.5,
        contest_threshold: 0.3,
        contest_check_s: DT,
        spur_reserve_min: 0.1,
        spur_window_s: 3.0,
        spur_jitter_s: 2.0,
        bud_s: 0.5,
        flower_s: 0.5,
        fruit_s: 0.5,
        spent_s: 0.5,
        nectar: 0.01,
        fruit: 0.02,
        energy_density: 1.0,
        ..VineConfig::default()
    };
    c
}

/// [`fast`] with the economy stopped: no income, no upkeep, no regrowth, and spread,
/// sisters and spurs unaffordable. Reserves stay exactly where a test put them, so a
/// comparison of reserve per face is exact across a step.
fn frozen() -> FloraConfig {
    let mut c = fast();
    let v = &mut c.latticevine;
    v.assimilation = 0.0;
    v.leaf_upkeep = 0.0;
    v.wood_upkeep = 0.0;
    v.regrow_rate = 0.0;
    v.spread_cost = 1e12;
    v.sister_cost = 1e12;
    v.spur_reserve_min = 1e12;
    c
}

/// [`fast`] with no spreading and no sisters: the cover stays the faces a test built.
fn fixed_cover() -> FloraConfig {
    let mut c = fast();
    c.latticevine.spread_cost = 1e12;
    c.latticevine.sister_cost = 1e12;
    c
}

fn seed(flora: &mut Flora, world: &World, root: Site, first: Face, reserve: f64) -> VineId {
    flora
        .seed_vine(
            world,
            VineSeed {
                root,
                first,
                reserve,
                lineage: None,
            },
        )
        .unwrap_or_else(|| panic!("fixture: founder at {root:?} on {first:?} refused"))
}

/// Hand-build `id`'s cover on the wall's west faces `ys`, slab `z`, at full leaf.
fn cover_column(
    flora: &mut Flora,
    world: &World,
    id: VineId,
    z: u32,
    ys: std::ops::RangeInclusive<u32>,
) {
    for y in ys {
        assert!(
            flora.cover_face(world, id, wf(y, z), 1.0),
            "fixture: cover_face refused {:?} for vine {id}",
            wf(y, z)
        );
    }
}

fn vine(flora: &Flora, id: VineId) -> Vine {
    *flora
        .view()
        .cover
        .vine(id)
        .unwrap_or_else(|| panic!("vine {id} is gone"))
}

fn face(flora: &Flora, f: Face) -> Option<CoveredFace> {
    flora.view().cover.face(f).copied()
}

fn owner(flora: &Flora, f: Face) -> Option<VineId> {
    face(flora, f).map(|c| c.owner)
}

/// Faces owned by any vine of `lineage`.
fn lineage_faces(flora: &Flora, lineage: u64) -> BTreeSet<Face> {
    let view = flora.view();
    view.cover
        .faces()
        .into_iter()
        .filter(|c| view.cover.vine(c.owner).map(|v| v.lineage) == Some(lineage))
        .map(|c| c.face)
        .collect()
}

fn total_litter(flora: &Flora) -> f64 {
    flora.view().ground.iter().map(|g| g.litter).sum()
}

fn litter_at(flora: &Flora, site: Site) -> f64 {
    flora.view().ground_at(site).map_or(0.0, |g| g.litter)
}

/// The crate's closure rule, with the vines in the totals.
fn assert_residuals(flora: &Flora, when: &str) {
    let v = flora.view();
    for (name, got, scale) in [
        (
            "organic",
            v.organic() - v.ledger.expected_organic(),
            v.organic(),
        ),
        (
            "mineral",
            v.mineral() - v.ledger.expected_mineral(),
            v.mineral(),
        ),
        (
            "energy",
            v.energy() - v.ledger.expected_energy(),
            v.energy(),
        ),
    ] {
        assert!(
            got.abs() <= 1e-9 * scale.abs().max(1.0),
            "{when}: flora {name} residual {got:e} (the vines must be in the totals and \
             every vine flow in the ledger)"
        );
    }
}

/// Step against the dry twin until `id` is dormant and every face it owns is leafless.
fn run_into_dormancy(flora: &mut Flora, dry: &mut World, id: VineId) {
    for _ in 0..200 {
        flora.step(dry);
        let v = vine(flora, id);
        let leafless = flora
            .view()
            .cover
            .faces_of(id)
            .into_iter()
            .all(|c| c.leafiness == 0.0);
        if v.dormant && leafless {
            return;
        }
    }
    panic!("a vine on dry soil never went dormant and leafless in 200 ticks (10 s)");
}

// ------------------------------------------------------------------ 1. climbing, no cap

/// A founder at a wall's foot covers its first face and, with water and light, climbs
/// the whole of a 94-face wall — far past any plausible size cap — stopping only at the
/// top. The same founder on dry soil (no income; its reserve buys a few faces at most)
/// does not get there: the climb is paid by income, not by the founder's purse.
#[test]
fn t01_a_founder_at_a_wall_foot_climbs_a_tall_wall_to_the_top_with_no_cap() {
    let top = 96;
    let plan = Plan::wall(12, 100, 1, top);
    let config = fast();
    let reserve = 3.0 * config.latticevine.spread_cost + 0.05;

    let climb = |avail: f64| -> (usize, bool) {
        let mut world = plan.world(avail, false);
        let mut flora = Flora::new(config.clone());
        let id = seed(&mut flora, &world, foot(0), wf(3, 0), reserve);
        let first = face(&flora, wf(3, 0)).expect("the founder covers its first face");
        assert_eq!(first.owner, id, "the first face is the founder's");
        assert!(first.rooted, "the first face carries the root");
        assert!(first.leafiness > 0.0, "a founder starts leafy");
        let lineage = vine(&flora, id).lineage;
        let mut reached = false;
        for _ in 0..400 {
            flora.step(&mut world);
            if lineage_faces(&flora, lineage).contains(&wf(top, 0)) {
                reached = true;
                break;
            }
        }
        assert_residuals(&flora, "after the climb");
        let covered = lineage_faces(&flora, lineage);
        if reached {
            for y in 3..=top {
                assert!(
                    covered.contains(&wf(y, 0)),
                    "the cover reached the top but skipped {:?}",
                    wf(y, 0)
                );
            }
        }
        (covered.len(), reached)
    };

    let (wet_faces, wet_reached) = climb(1.0);
    assert!(
        wet_reached,
        "a watered, lit vine must climb all {} faces of the wall within 400 ticks; it \
         covered {wet_faces} (a cap or a stall)",
        top - 2
    );
    let (dry_faces, dry_reached) = climb(0.0);
    assert!(
        !dry_reached && dry_faces < wet_faces,
        "without water the climb must stall: dry covered {dry_faces}, wet {wet_faces}"
    );
}

// ------------------------------------------------------------------ 2. hanging, undersides

/// A vine rooted on a ledge top covers the lip face below it, hangs down the cliff and
/// turns onto the overhang's underside. And an underside earns less than a side under the
/// same sky: [`face_light`] is the sky times 1 for a side and `underside_light` for an
/// underside.
///
/// ```text
///   y 10  R R R R R R R R S .      S = the soil ledge top, the root (8, 10)
///   y  6  R R R R R R R R R .      the cliff's +x face, y = 6..=10, fronts x = 9
///   y  5  R R R R R R . . . .      the recess: undersides (6..=8, 6, Down)
///   y  3  R R R R R R . . . .
///   y  1  soil floor
/// ```
#[test]
fn t02_a_ledge_top_vine_hangs_down_and_onto_the_underside_which_earns_less() {
    let mut plan = Plan::new(16, 16, 1);
    for x in 0..=8 {
        for y in 6..=10 {
            plan.set(x, y, 0, Material::Rock);
        }
    }
    for x in 0..=5 {
        for y in 3..=5 {
            plan.set(x, y, 0, Material::Rock);
        }
    }
    plan.set(8, 10, 0, Material::Soil);
    let mut world = plan.world(1.0, false);
    let config = fast();
    let mut flora = Flora::new(config.clone());
    let lip = Face::new(8, 10, 0, FaceDir::PosX);
    let id = seed(&mut flora, &world, Site { x: 8, y: 10, z: 0 }, lip, 5.0);
    assert!(
        face(&flora, lip).is_some_and(|c| c.owner == id && c.rooted),
        "a ledge-top founder covers the lip face below its root"
    );
    let lineage = vine(&flora, id).lineage;
    let underside = |f: &Face| f.dir == FaceDir::Down;
    for _ in 0..300 {
        flora.step(&mut world);
        if lineage_faces(&flora, lineage).iter().any(underside) {
            break;
        }
    }
    let covered = lineage_faces(&flora, lineage);
    for y in 6..=10 {
        let f = Face::new(8, y, 0, FaceDir::PosX);
        assert!(covered.contains(&f), "the vine must hang down to {f:?}");
    }
    assert!(
        covered.iter().any(underside),
        "the vine must turn onto the overhang's underside; it covers {covered:?}"
    );
    assert_residuals(&flora, "after hanging");

    let v = &config.latticevine;
    for sky in [0.1, 0.5, 1.0] {
        let side = face_light(v, FaceDir::PosX, sky);
        for dir in [FaceDir::NegX, FaceDir::PosZ, FaceDir::NegZ] {
            assert!(
                (face_light(v, dir, sky) - side).abs() < 1e-12,
                "all four sides see the same light at sky {sky}"
            );
        }
        assert!(
            (side - sky).abs() < 1e-12,
            "a side's factor is 1: {side} at {sky}"
        );
        let under = face_light(v, FaceDir::Down, sky);
        assert!(
            (under - sky * v.underside_light).abs() < 1e-12,
            "an underside's factor is underside_light: {under} at {sky}"
        );
        assert!(under < side, "an underside earns less under the same sky");
    }
}

// ------------------------------------------------------------------ 3. first come holds

/// Spread never takes a healthy owned face, a sister's or a stranger's. Three columns of
/// one wall: a young vine A in the middle, a stranger B and A's sister C (same lineage)
/// covering the outer two with deep reserves. All three race for the middle column's
/// free faces; at no tick does any face that was healthy (leafiness at or above the
/// contest threshold) before and after the tick change owner.
#[test]
fn t03_spread_never_takes_a_healthy_owned_face_sister_or_stranger() {
    let top = 12;
    let plan = Plan::wall(12, 16, 3, top);
    let mut world = plan.world(1.0, false);
    let config = fast();
    let threshold = config.latticevine.contest_threshold;
    let mut flora = Flora::new(config);
    let a = seed(&mut flora, &world, foot(1), wf(3, 1), 1.0);
    let lineage = vine(&flora, a).lineage;
    let b = seed(&mut flora, &world, foot(0), wf(3, 0), 1e3);
    cover_column(&mut flora, &world, b, 0, 4..=top);
    let c = flora
        .seed_vine(
            &world,
            VineSeed {
                root: foot(2),
                first: wf(3, 2),
                reserve: 1e3,
                lineage: Some(lineage),
            },
        )
        .expect("fixture: the sister founder");
    assert_eq!(
        vine(&flora, c).lineage,
        lineage,
        "a sister joins its lineage"
    );
    assert_ne!(vine(&flora, b).lineage, lineage, "a stranger does not");
    cover_column(&mut flora, &world, c, 2, 4..=top);

    let start = flora.view().cover.faces().len();
    let mut before: BTreeMap<Face, CoveredFace> = flora
        .view()
        .cover
        .faces()
        .into_iter()
        .map(|c| (c.face, c))
        .collect();
    for tick in 0..200 {
        flora.step(&mut world);
        let now: BTreeMap<Face, CoveredFace> = flora
            .view()
            .cover
            .faces()
            .into_iter()
            .map(|c| (c.face, c))
            .collect();
        for (f, was) in &before {
            if let Some(is) = now.get(f)
                && was.leafiness >= threshold
                && is.leafiness >= threshold
            {
                assert_eq!(
                    is.owner, was.owner,
                    "tick {tick}: healthy face {f:?} changed owner {} -> {}",
                    was.owner, is.owner
                );
            }
        }
        before = now;
    }
    assert!(
        flora.view().cover.faces().len() > start,
        "the fixture must actually spread onto the middle column"
    );
    assert_residuals(&flora, "after the race");
}

// ------------------------------------------------------------------ 4. contest

/// A face cropped below the contest threshold goes to the adjacent owner with more reserve
/// per covered face; its owner keeps it when ahead **or tied**. Two vines of three faces on
/// neighbouring columns, the economy frozen so reserves are exactly what the arm set.
#[test]
fn t04_a_weak_face_goes_to_the_neighbour_with_more_reserve_per_face_owner_keeps_ties() {
    let plan = Plan::wall(12, 10, 2, 5);
    for (arm, reserve_a, reserve_b, b_wins) in [
        ("challenger ahead", 0.3, 3.0, true),
        ("owner ahead", 3.0, 0.3, false),
        ("tied", 1.5, 1.5, false),
    ] {
        let mut world = plan.world(1.0, false);
        let mut flora = Flora::new(frozen());
        let a = seed(&mut flora, &world, foot(0), wf(3, 0), reserve_a);
        cover_column(&mut flora, &world, a, 0, 4..=5);
        let b = seed(&mut flora, &world, foot(1), wf(3, 1), reserve_b);
        cover_column(&mut flora, &world, b, 1, 4..=5);
        assert_eq!(flora.view().cover.faces_of(a).len(), 3, "{arm}: fixture");
        assert_eq!(flora.view().cover.faces_of(b).len(), 3, "{arm}: fixture");

        let weak = wf(4, 0);
        assert!(flora.crop(weak, 1e9).is_some(), "{arm}: crop the face bare");
        for _ in 0..3 {
            flora.step(&mut world);
        }
        let expect = if b_wins { b } else { a };
        assert_eq!(
            owner(&flora, weak),
            Some(expect),
            "{arm}: A {reserve_a}/3 per face against B {reserve_b}/3 — the weak face \
             belongs to {}",
            if b_wins { "B" } else { "A" }
        );
        let (na, nb) = if b_wins { (2, 4) } else { (3, 3) };
        assert_eq!(flora.view().cover.faces_of(a).len(), na, "{arm}: A's faces");
        assert_eq!(flora.view().cover.faces_of(b).len(), nb, "{arm}: B's faces");
        assert_residuals(&flora, arm);
    }
}

// ------------------------------------------------------------------ 5. sisters

/// Cover reaching a free soil ledge roots a sister there with the parent's lineage. The
/// faces nearer the sister's root along the cover switch to it; the rest stay. Afterwards
/// the two keep separate stocks: a bite from the sister moves the sister's reserve and
/// leaves the parent's exactly where an unbitten twin has it.
///
/// ```text
///   y 7   . L R      L = soil ledge (5, 7); its west face and underside carry cover
///   y 6   . . R      the wall's west faces y = 3..=6, root at the foot (5, 2)
///   y 3   . . R
///   y 2   S S R      path from the foot: 3, 4, 5, 6, ledge underside, ledge west face
///        x=4 5 6
/// ```
#[test]
fn t05_cover_reaching_free_soil_roots_a_sister_that_takes_the_nearer_faces() {
    let mut plan = Plan::wall(12, 16, 1, 10);
    plan.set(5, 7, 0, Material::Soil);
    let mut world = plan.world(1.0, false);
    let mut config = fixed_cover();
    config.latticevine.sister_cost = 0.5;
    let mut flora = Flora::new(config.clone());
    let parent = seed(&mut flora, &world, foot(0), wf(3, 0), 5.0);
    cover_column(&mut flora, &world, parent, 0, 4..=6);
    let ledge_under = Face::new(5, 7, 0, FaceDir::Down);
    let ledge_west = Face::new(5, 7, 0, FaceDir::NegX);
    assert!(
        flora.cover_face(&world, parent, ledge_under, 1.0),
        "fixture: the ledge's underside is cover-adjacent to (6, 6) by the concave corner"
    );
    assert!(
        flora.cover_face(&world, parent, ledge_west, 1.0),
        "fixture: the ledge's west face is cover-adjacent to its underside by the edge"
    );
    let ledge = Site { x: 5, y: 7, z: 0 };
    let lineage = vine(&flora, parent).lineage;

    let mut sister = None;
    for _ in 0..200 {
        flora.step(&mut world);
        sister = flora
            .view()
            .cover
            .vines()
            .iter()
            .find(|v| v.id != parent)
            .copied();
        if sister.is_some() {
            break;
        }
    }
    let sister = sister.expect("cover on a free soil ledge must root a sister within 200 ticks");
    assert_eq!(sister.root, ledge, "the sister roots on the ledge");
    assert_eq!(
        sister.lineage, lineage,
        "a sister shares the parent's lineage"
    );
    assert_eq!(
        flora.view().cover.vines().len(),
        2,
        "one sister, one parent"
    );
    for f in [ledge_west, ledge_under, wf(6, 0)] {
        assert_eq!(
            owner(&flora, f),
            Some(sister.id),
            "{f:?} is nearer the ledge root along the cover: the sister's"
        );
    }
    for f in [wf(3, 0), wf(4, 0), wf(5, 0)] {
        assert_eq!(
            owner(&flora, f),
            Some(parent),
            "{f:?} is nearer the foot along the cover: the parent's"
        );
    }
    assert!(
        face(&flora, ledge_west).unwrap().rooted,
        "the sister's root face"
    );
    assert!(
        face(&flora, wf(3, 0)).unwrap().rooted,
        "the parent's root face"
    );
    assert_residuals(&flora, "after the sister rooted");

    // Independence: one twin untouched, one with the sister bitten (kept above the
    // contest threshold, so no face changes hands).
    let (mut plain, mut plain_world) = (flora.clone(), world.clone());
    let (mut bitten, mut bitten_world) = (flora, world);
    let bite = 0.2 * config.latticevine.leaf_mass;
    assert!(
        bitten.crop(wf(6, 0), bite).is_some(),
        "the sister's face gives a bite"
    );
    for _ in 0..20 {
        plain.step(&mut plain_world);
        bitten.step(&mut bitten_world);
    }
    let (p0, p1) = (vine(&plain, parent), vine(&bitten, parent));
    let close = |a: f64, b: f64| (a - b).abs() <= 1e-12 * a.abs().max(b.abs()).max(1e-12);
    assert!(
        close(p0.reserve, p1.reserve) && close(p0.water, p1.water),
        "the parent's stocks must not feel a bite on its sister: reserve {} vs {}, water \
         {} vs {}",
        p0.reserve,
        p1.reserve,
        p0.water,
        p1.water
    );
    let (s0, s1) = (vine(&plain, sister.id), vine(&bitten, sister.id));
    assert!(
        !close(s0.reserve, s1.reserve),
        "the bitten sister's own reserve must move: {} vs {}",
        s0.reserve,
        s1.reserve
    );
}

// ------------------------------------------------------------------ 6. drought

/// A sustained dry spell brings dormancy, and not before it has lasted `dry_spell_s`. The
/// leaves fall to 0 and their whole mass lands as litter on the ground below the faces;
/// the runners stay, every face keeps its owner.
#[test]
fn t06_a_sustained_dry_spell_brings_dormancy_and_the_leaves_become_litter() {
    let plan = Plan::wall(12, 14, 1, 10);
    let mut dry = plan.world(0.0, false);
    let config = fixed_cover();
    let leaf_mass = config.latticevine.leaf_mass;
    let spell = ticks(config.latticevine.dry_spell_s);
    let mut flora = Flora::new(config);
    let id = seed(&mut flora, &dry, foot(0), wf(3, 0), 100.0);
    cover_column(&mut flora, &dry, id, 0, 4..=6);
    let leaf: f64 = flora
        .view()
        .cover
        .faces_of(id)
        .into_iter()
        .map(|c| c.leafiness * leaf_mass)
        .sum();
    assert!(leaf > 0.0, "fixture: four leafy faces");

    for _ in 0..spell / 2 {
        flora.step(&mut dry);
    }
    assert!(
        !vine(&flora, id).dormant,
        "half a dry spell is not a sustained one"
    );
    run_into_dormancy(&mut flora, &mut dry, id);

    let faces = flora.view().cover.faces_of(id);
    assert_eq!(
        faces.iter().map(|c| c.face).collect::<Vec<_>>(),
        vec![wf(3, 0), wf(4, 0), wf(5, 0), wf(6, 0)],
        "the runners stay: every face keeps its owner"
    );
    let litter = litter_at(&flora, foot(0));
    assert!(
        (litter - leaf).abs() <= 1e-9 * leaf,
        "the fallen leaf mass {leaf} must lie as litter at the foot, found {litter}"
    );
    assert!(
        (total_litter(&flora) - leaf).abs() <= 1e-9 * leaf,
        "and nowhere else"
    );
    assert_residuals(&flora, "dormant");
}

// ------------------------------------------------------------------ 7. hysteresis

/// One short wet pulse during dormancy regreens nothing. A sustained wet spell does, from
/// the root outward: no face greens before the face nearer the root has, and the outermost
/// greens strictly after the rooted one.
#[test]
fn t07_a_short_wet_pulse_does_not_regreen_a_sustained_spell_regreens_root_first() {
    let plan = Plan::wall(12, 14, 1, 10);
    let mut dry = plan.world(0.0, false);
    let mut wet = plan.world(1.0, false);
    let config = fixed_cover();
    let wet_spell = ticks(config.latticevine.wet_spell_s);
    let mut flora = Flora::new(config);
    let id = seed(&mut flora, &dry, foot(0), wf(3, 0), 100.0);
    cover_column(&mut flora, &dry, id, 0, 4..=8);
    run_into_dormancy(&mut flora, &mut dry, id);

    let assert_bare = |flora: &Flora, when: &str| {
        assert!(vine(flora, id).dormant, "{when}: still dormant");
        for c in flora.view().cover.faces_of(id) {
            assert_eq!(c.leafiness, 0.0, "{when}: {:?} regreened", c.face);
        }
    };
    for t in 0..wet_spell / 2 {
        flora.step(&mut wet);
        assert_bare(&flora, &format!("wet pulse tick {t}"));
    }
    for t in 0..wet_spell / 2 {
        flora.step(&mut dry);
        assert_bare(&flora, &format!("dry again tick {t}"));
    }

    let mut first_green: BTreeMap<u32, usize> = BTreeMap::new();
    for t in 0..300 {
        flora.step(&mut wet);
        for c in flora.view().cover.faces_of(id) {
            if c.leafiness > 0.0 {
                first_green.entry(c.face.y).or_insert(t);
            }
        }
        if first_green.len() == 6 {
            break;
        }
    }
    assert_eq!(
        first_green.len(),
        6,
        "a sustained wet spell regreens every face; greened: {first_green:?}"
    );
    assert!(!vine(&flora, id).dormant, "and ends dormancy");
    let order: Vec<usize> = (3..=8).map(|y| first_green[&y]).collect();
    assert!(
        order.windows(2).all(|w| w[0] <= w[1]),
        "regreening runs from the root outward: first-green ticks by height {order:?}"
    );
    assert!(
        order[5] > order[0],
        "the outermost face greens after the rooted one, not all at once: {order:?}"
    );
    assert_residuals(&flora, "regreened");
}

// ------------------------------------------------------------------ 8. starving

/// A dormant vine that cannot pay its woody upkeep loses its **outermost** faces first:
/// at every tick the faces it still owns are an unbroken run from its root, the first
/// loss comes only once it is dormant, and a lost face is unowned. The colony shrinks back
/// toward its root — some tick holds part of it — rather than vanishing whole.
#[test]
fn t08_a_starving_dormant_vine_loses_its_outermost_faces_first() {
    let plan = Plan::wall(12, 14, 1, 10);
    let mut dry = plan.world(0.0, false);
    let mut config = frozen();
    config.latticevine.wood_upkeep = 0.1;
    let mut flora = Flora::new(config);
    let id = seed(&mut flora, &dry, foot(0), wf(3, 0), 1.0);
    cover_column(&mut flora, &dry, id, 0, 4..=8);

    let mut lost = 0;
    let mut held = 6;
    let mut saw_partial = false;
    for t in 0..400 {
        flora.step(&mut dry);
        let alive = flora.view().cover.vine(id).copied();
        let ys: Vec<u32> = flora
            .view()
            .cover
            .faces_of(id)
            .into_iter()
            .map(|c| c.face.y)
            .collect();
        let expect: Vec<u32> = (3..3 + ys.len() as u32).collect();
        assert_eq!(
            ys, expect,
            "tick {t}: the faces held must run unbroken from the root, outermost lost first"
        );
        if ys.len() < held {
            if let Some(v) = alive {
                assert!(v.dormant, "tick {t}: a face was lost before dormancy");
            }
            for y in 3 + ys.len() as u32..3 + held as u32 {
                assert_eq!(
                    owner(&flora, wf(y, 0)),
                    None,
                    "tick {t}: a lost face is unowned"
                );
            }
            lost += held - ys.len();
            held = ys.len();
        }
        saw_partial |= (1..6).contains(&ys.len());
    }
    assert!(
        lost >= 2,
        "a starving dormant vine must lose faces; lost {lost}"
    );
    assert!(
        saw_partial,
        "the cover must shrink toward the root, holding part of itself at some tick"
    );
    assert_residuals(&flora, "starved");
}

// ------------------------------------------------------------------ 9. spurs

/// The first tick each of the vine's faces left `Bare`, over `n` ticks.
fn spur_starts(
    flora: &mut Flora,
    world: &mut World,
    id: VineId,
    n: usize,
) -> BTreeMap<Face, usize> {
    let mut starts = BTreeMap::new();
    for t in 0..n {
        flora.step(world);
        for c in flora.view().cover.faces_of(id) {
            if c.spur != SpurPhase::Bare {
                starts.entry(c.face).or_insert(t);
            }
        }
    }
    starts
}

/// Spurs cycle only with reserve and after rain, on scattered ticks, and never while
/// dormant. Four arms on the same eight-face vine.
#[test]
fn t09_spurs_cycle_only_after_rain_with_reserve_scattered_and_never_dormant() {
    let plan = Plan::wall(12, 16, 1, 12);
    let build = |config: FloraConfig, avail: f64| {
        let (sky, rain) = (plan.world(avail, false), plan.world(avail, true));
        let mut flora = Flora::new(config);
        let id = seed(&mut flora, &sky, foot(0), wf(3, 0), 100.0);
        cover_column(&mut flora, &sky, id, 0, 4..=10);
        (flora, id, sky, rain)
    };

    // No rain: nothing starts.
    let (mut flora, id, mut sky, _) = build(fixed_cover(), 1.0);
    let starts = spur_starts(&mut flora, &mut sky, id, 160);
    assert!(starts.is_empty(), "no shower, no cycle: {starts:?}");

    // A shower ends with reserve in hand: every spur starts, and not on one tick.
    let (mut flora, id, mut sky, mut rain) = build(fixed_cover(), 1.0);
    flora.step(&mut rain);
    let starts = spur_starts(&mut flora, &mut sky, id, 160);
    assert_eq!(
        starts.len(),
        8,
        "every face's spur cycles after a shower: {starts:?}"
    );
    let distinct: BTreeSet<usize> = starts.values().copied().collect();
    assert!(
        distinct.len() >= 2,
        "starts are jittered, not all on one tick: {starts:?}"
    );

    // The same shower without the reserve: nothing starts.
    let mut poor = fixed_cover();
    poor.latticevine.spur_reserve_min = 1e12;
    let (mut flora, id, mut sky, mut rain) = build(poor, 1.0);
    flora.step(&mut rain);
    let starts = spur_starts(&mut flora, &mut sky, id, 160);
    assert!(starts.is_empty(), "no reserve, no cycle: {starts:?}");

    // Dormant: a shower on a dormant vine starts nothing.
    let (mut flora, id, mut sky, mut rain) = build(fixed_cover(), 0.0);
    run_into_dormancy(&mut flora, &mut sky, id);
    flora.step(&mut rain);
    let starts = spur_starts(&mut flora, &mut sky, id, 160);
    assert!(
        starts.is_empty(),
        "a dormant vine's spurs do not cycle: {starts:?}"
    );
}

// ------------------------------------------------------------------ 10. nectar and fruit

/// Nectar exists only on faces in flower, fruit only on faces in fruit. `take_nectar` and
/// `take_fruit` deplete them to nothing and book exactly what they hand over as consumed.
/// Fruit nobody eats falls to litter when the spur goes spent.
#[test]
fn t10_nectar_only_in_flower_fruit_only_in_fruit_both_depletable_uneaten_fruit_to_litter() {
    let plan = Plan::wall(12, 16, 1, 12);
    let mut sky = plan.world(1.0, false);
    let mut rain = plan.world(1.0, true);
    let mut config = fixed_cover();
    config.latticevine.flower_s = 1.0;
    config.latticevine.fruit_s = 1.0;
    let mut flora = Flora::new(config);
    let id = seed(&mut flora, &sky, foot(0), wf(3, 0), 100.0);
    cover_column(&mut flora, &sky, id, 0, 4..=10);
    assert!(
        flora.take_nectar(wf(3, 0), 1.0).is_none(),
        "a bare spur has no nectar"
    );
    assert!(
        flora.take_fruit(wf(3, 0), 1.0).is_none(),
        "a bare spur has no fruit"
    );
    flora.step(&mut rain);

    // Drain one stock through `take` in two bites and check the books.
    fn drain(
        flora: &mut Flora,
        f: Face,
        stock: fn(&CoveredFace) -> f64,
        take: fn(&mut Flora, Face, f64) -> Option<cubarium_voxel_flora::Taken>,
        what: &str,
    ) {
        let have = stock(&face(flora, f).unwrap());
        let out0 = flora.view().ledger.consumed_organic_out;
        let first = take(flora, f, 0.5 * have).unwrap_or_else(|| panic!("{what}: a first bite"));
        assert!(
            first.organic > 0.0 && first.organic <= 0.5 * have + 1e-15,
            "{what}: the bite is bounded by the ask: {} of {have}",
            first.organic
        );
        let left = stock(&face(flora, f).unwrap());
        assert!(
            (left - (have - first.organic)).abs() <= 1e-12,
            "{what}: the stock drops by the bite: {left}"
        );
        let second = take(flora, f, 1e9).unwrap_or_else(|| panic!("{what}: the rest"));
        assert!(
            (second.organic - left).abs() <= 1e-12,
            "{what}: a greedy bite takes what is left, no more"
        );
        assert_eq!(stock(&face(flora, f).unwrap()), 0.0, "{what}: depleted");
        assert!(
            take(flora, f, 1e9).is_none(),
            "{what}: nothing after depletion"
        );
        let booked = flora.view().ledger.consumed_organic_out - out0;
        assert!(
            (booked - (first.organic + second.organic)).abs() <= 1e-12,
            "{what}: the ledger books exactly what was taken: {booked}"
        );
    }

    let (mut ate_nectar, mut ate_fruit, mut fell) = (false, false, false);
    for t in 0..300 {
        let before: BTreeMap<Face, CoveredFace> = flora
            .view()
            .cover
            .faces_of(id)
            .into_iter()
            .map(|c| (c.face, c))
            .collect();
        let litter0 = total_litter(&flora);
        flora.step(&mut sky);
        let now = flora.view().cover.faces_of(id);
        let mut dropped = 0.0;
        for c in &now {
            assert!(
                c.nectar >= 0.0 && c.fruit >= 0.0,
                "tick {t}: negative stock"
            );
            assert!(
                c.nectar == 0.0 || c.spur == SpurPhase::Flower,
                "tick {t}: {:?} holds nectar {} in {:?}",
                c.face,
                c.nectar,
                c.spur
            );
            assert!(
                c.fruit == 0.0 || c.spur == SpurPhase::Fruit,
                "tick {t}: {:?} holds fruit {} in {:?}",
                c.face,
                c.fruit,
                c.spur
            );
            let was = before[&c.face];
            if was.spur == SpurPhase::Fruit && c.spur == SpurPhase::Spent && was.fruit > 0.0 {
                dropped += was.fruit;
                fell = true;
            }
        }
        assert!(
            total_litter(&flora) - litter0 >= dropped - 1e-12,
            "tick {t}: uneaten fruit {dropped} must fall to litter"
        );
        if !ate_nectar
            && let Some(c) = now
                .iter()
                .find(|c| c.spur == SpurPhase::Flower && c.nectar > 0.0)
        {
            assert!(
                flora.take_fruit(c.face, 1.0).is_none(),
                "a flower has no fruit"
            );
            drain(
                &mut flora,
                c.face,
                |c| c.nectar,
                Flora::take_nectar,
                "nectar",
            );
            ate_nectar = true;
        }
        if !ate_fruit
            && let Some(c) = now
                .iter()
                .find(|c| c.spur == SpurPhase::Fruit && c.fruit > 0.0)
        {
            assert!(
                flora.take_nectar(c.face, 1.0).is_none(),
                "a fruit has no nectar"
            );
            drain(&mut flora, c.face, |c| c.fruit, Flora::take_fruit, "fruit");
            ate_fruit = true;
        }
        if ate_nectar && ate_fruit && fell {
            break;
        }
    }
    assert!(
        ate_nectar,
        "some face must flower with nectar after a shower"
    );
    assert!(ate_fruit, "some face must fruit with beads after a shower");
    assert!(fell, "some uneaten fruit must reach spent and fall");
    assert_residuals(&flora, "after feeding");
}

// ------------------------------------------------------------------ 11. shared water

/// A vine and a stand rooted on the same site both drink, through one split: with the
/// vine thirsty enough to empty the root box on its own, the stand still gets a share —
/// and less than it gets alone. The world's water is conserved: the pore water lost is
/// exactly what the two drank, the core's transpiration ledger and the flora's agree, and
/// the core's residual stays zero.
#[test]
fn t11_a_vine_and_a_stand_on_one_site_share_its_water_and_water_is_conserved() {
    let plan = Plan::wall(12, 10, 1, 8);
    let mut config = fixed_cover();
    config.latticevine.transpiration_m3_per_s = 1.0;
    let springturf = |flora: &mut Flora, world: &World| {
        let wood = flora.config().species(Species::Springturf).wood_max;
        assert!(flora.apply(
            world,
            Command::SeedOnFace {
                site: foot(0),
                species: Species::Springturf,
                wood,
            }
        ));
    };

    let mut alone_world = plan.world(1.0, false);
    let mut alone = Flora::new(config.clone());
    springturf(&mut alone, &alone_world);
    alone.step(&mut alone_world);
    let alone_got = alone.view().stand_at(foot(0)).expect("standing").water_m3;
    assert!(alone_got > 0.0, "fixture: a stand alone drinks");

    let mut world = plan.world(1.0, false);
    let mut flora = Flora::new(config);
    springturf(&mut flora, &world);
    let id = seed(&mut flora, &world, foot(0), wf(3, 0), 5.0);
    let pore0 = world.pore_m3();
    let core0 = world.view().ledger.transpiration_out;
    let flora0 = flora.view().ledger.transpired_m3;
    flora.step(&mut world);
    let stand_got = flora.view().stand_at(foot(0)).expect("standing").water_m3;
    let vine_got = vine(&flora, id).drank_m3;
    assert!(vine_got > 0.0, "the vine drinks: {vine_got}");
    assert!(
        stand_got > 0.0,
        "the stand still drinks beside a thirsty vine"
    );
    assert!(
        stand_got < alone_got,
        "they compete for one box: the stand got {stand_got} beside the vine, {alone_got} alone"
    );
    let drunk = stand_got + vine_got;
    let tol = 1e-9 * drunk;
    assert!(
        ((pore0 - world.pore_m3()) - drunk).abs() <= tol,
        "the soil lost exactly what the two drank"
    );
    assert!(
        ((world.view().ledger.transpiration_out - core0) - drunk).abs() <= tol,
        "the core books it as transpiration"
    );
    assert!(
        ((flora.view().ledger.transpired_m3 - flora0) - drunk).abs() <= tol,
        "the flora ledger agrees"
    );
    assert!(
        world.view().water_residual().abs() <= 1e-12,
        "the world's water closes: {:e}",
        world.view().water_residual()
    );
    assert_residuals(&flora, "after drinking");
}

// ------------------------------------------------------------------ 12. crop

/// `crop` thins a face's leafiness by exactly the leaf mass it hands over, bounded by the
/// ask and by the leaf there is, books it as consumed, and never takes the runner: the
/// face stays owned at leafiness 0 and then gives nothing.
#[test]
fn t12_crop_thins_leafiness_and_returns_the_mass_taken() {
    let plan = Plan::wall(12, 10, 1, 8);
    let world = plan.world(1.0, false);
    let config = frozen();
    let leaf_mass = config.latticevine.leaf_mass;
    let mut flora = Flora::new(config);
    let id = seed(&mut flora, &world, foot(0), wf(3, 0), 1.0);
    cover_column(&mut flora, &world, id, 0, 4..=4);
    let f = wf(4, 0);

    let out0 = flora.view().ledger.consumed_organic_out;
    let bite = flora
        .crop(f, 0.25 * leaf_mass)
        .expect("a leafy face gives a bite");
    assert!(
        (bite.organic - 0.25 * leaf_mass).abs() <= 1e-12,
        "a bite within the leaf is the ask: {}",
        bite.organic
    );
    let c = face(&flora, f).unwrap();
    assert!(
        (c.leafiness - 0.75).abs() <= 1e-12,
        "leafiness falls by bite / leaf_mass: {}",
        c.leafiness
    );
    let rest = flora.crop(f, 1e9).expect("the rest of the leaf");
    assert!(
        (rest.organic - 0.75 * leaf_mass).abs() <= 1e-12,
        "a greedy bite takes the leaf there is: {}",
        rest.organic
    );
    let c = face(&flora, f).expect("a cropped face is still covered: the runner stays");
    assert_eq!(c.owner, id, "and still owned");
    assert_eq!(c.leafiness, 0.0, "and bare");
    assert!(flora.crop(f, 1e9).is_none(), "a bare runner gives nothing");
    assert!(
        flora.crop(wf(6, 0), 1e9).is_none(),
        "an uncovered face gives nothing"
    );
    let booked = flora.view().ledger.consumed_organic_out - out0;
    assert!(
        (booked - (bite.organic + rest.organic)).abs() <= 1e-12,
        "the ledger books exactly the crop: {booked}"
    );
    assert_residuals(&flora, "after cropping");
}

// ------------------------------------------------------------------ 13. terrain edits

/// A face goes when its voxel stops being solid, or when its front fills with solid or
/// with water; the rest of the vine stays. A vine whose root support goes dies, and its
/// faces become unowned. The books close through each edit.
#[test]
fn t13_terrain_edits_remove_faces_and_losing_the_root_kills_the_vine() {
    let plan = Plan::wall(12, 14, 1, 10);
    let build = || {
        let world = plan.world(1.0, false);
        let mut flora = Flora::new(frozen());
        let id = seed(&mut flora, &world, foot(0), wf(3, 0), 1.0);
        cover_column(&mut flora, &world, id, 0, 4..=6);
        (flora, world, id)
    };
    let outer = wf(6, 0);
    let voxel_volume = plan.world(1.0, false).config().voxel_volume();

    for (arm, edit) in [
        (
            "voxel removed",
            WorldCommand::SetMaterial {
                x: 6,
                y: 6,
                z: 0,
                material: Material::Air,
            },
        ),
        (
            "front filled with rock",
            WorldCommand::SetMaterial {
                x: 5,
                y: 6,
                z: 0,
                material: Material::Rock,
            },
        ),
        (
            "front filled with water",
            WorldCommand::AddWater {
                x: 5,
                y: 6,
                z: 0,
                volume_m3: voxel_volume,
            },
        ),
    ] {
        let (mut flora, mut world, id) = build();
        assert!(
            face_eligible(&world.view(), outer),
            "{arm}: eligible before"
        );
        world.apply(edit);
        assert!(
            !face_eligible(&world.view(), outer),
            "{arm}: ineligible after"
        );
        flora.step(&mut world);
        assert_eq!(face(&flora, outer), None, "{arm}: the face is removed");
        for y in 3..=5 {
            assert_eq!(owner(&flora, wf(y, 0)), Some(id), "{arm}: the rest stays");
        }
        assert_residuals(&flora, arm);
    }

    let (mut flora, mut world, id) = build();
    world.apply(WorldCommand::SetMaterial {
        x: 5,
        y: 2,
        z: 0,
        material: Material::Air,
    });
    flora.step(&mut world);
    assert!(
        flora.view().cover.vine(id).is_none(),
        "a vine whose root support is gone dies"
    );
    assert!(
        flora
            .view()
            .cover
            .faces()
            .into_iter()
            .all(|c| c.owner != id),
        "and none of its faces stay owned"
    );
    for y in 3..=6 {
        assert_eq!(
            owner(&flora, wf(y, 0)),
            None,
            "{:?} is bare rock again",
            wf(y, 0)
        );
    }
    assert_residuals(&flora, "root lost");
}

// ------------------------------------------------------------------ 14. snapshot

/// A snapshot keeps every vine and every covered face as they were; the schema is bumped
/// past the pre-latticevine 5, and a schema-5 payload is refused.
#[test]
fn t14_a_snapshot_round_trip_keeps_the_cover_and_an_old_schema_is_refused() {
    let plan = Plan::wall(12, 14, 1, 10);
    let mut world = plan.world(1.0, false);
    let mut flora = Flora::new(fast());
    let id = seed(&mut flora, &world, foot(0), wf(3, 0), 2.0);
    cover_column(&mut flora, &world, id, 0, 4..=5);
    flora.crop(wf(5, 0), 0.05).expect("a bite");
    for _ in 0..5 {
        flora.step(&mut world);
    }

    let back = snapshot::decode(&snapshot::encode(&flora)).expect("its own format");
    assert_eq!(
        back.view().cover.vines(),
        flora.view().cover.vines(),
        "vines kept"
    );
    assert_eq!(
        back.view().cover.faces(),
        flora.view().cover.faces(),
        "faces kept"
    );
    assert_eq!(
        back.config().latticevine,
        flora.config().latticevine,
        "vine config kept"
    );

    assert!(
        snapshot::SCHEMA > 5,
        "the flora schema must be bumped for the cover (still {})",
        snapshot::SCHEMA
    );
    let bytes = snapshot::encode(&flora);
    let tag_len = postcard::to_stdvec(&snapshot::SCHEMA).unwrap().len();
    let mut old = postcard::to_stdvec(&5u32).unwrap();
    old.extend_from_slice(&bytes[tag_len..]);
    assert!(
        snapshot::decode(&old).is_err(),
        "a schema-5 world is refused"
    );
}

// ------------------------------------------------------------------ 15. the view

/// `FloraView` gives the presenter, per covered face, its owner, leafiness, rooted flag,
/// the owner's dormancy and the spur phase — one entry per face, agreeing with the record.
#[test]
fn t15_the_view_exposes_owner_leafiness_rooted_dormant_and_spur_per_face() {
    let plan = Plan::wall(12, 14, 1, 10);
    let mut wet = plan.world(1.0, false);
    let mut dry = plan.world(0.0, false);
    let mut flora = Flora::new(frozen());
    let id = seed(&mut flora, &wet, foot(0), wf(3, 0), 1.0);
    cover_column(&mut flora, &wet, id, 0, 4..=5);
    flora.crop(wf(5, 0), 0.03).expect("a bite");
    flora.step(&mut wet);

    let check = |flora: &Flora, dormant: bool| {
        let view = flora.view();
        let draw = view.cover.draw();
        let faces = view.cover.faces();
        assert_eq!(draw.len(), 3, "one entry per covered face");
        assert_eq!(faces.len(), 3);
        for (d, c) in draw.iter().zip(&faces) {
            assert_eq!(d.face, c.face, "sorted by face, like the records");
            assert_eq!(d.owner, id);
            assert_eq!(d.leafiness, c.leafiness, "{:?}", d.face);
            assert_eq!(
                d.rooted,
                d.face == wf(3, 0),
                "{:?}: only the root face",
                d.face
            );
            assert_eq!(d.dormant, dormant, "{:?}: the owner's dormancy", d.face);
            assert_eq!(d.spur, c.spur, "{:?}", d.face);
        }
    };
    check(&flora, false);
    let leaves: Vec<f64> = flora
        .view()
        .cover
        .draw()
        .iter()
        .map(|d| d.leafiness)
        .collect();
    assert!(
        leaves[2] < leaves[0],
        "the bitten face reads thinner: {leaves:?}"
    );
    run_into_dormancy(&mut flora, &mut dry, id);
    check(&flora, true);
}

// ------------------------------------------------------------------ 16–17. pace (the defaults)

/// The pace fixture: a rock wall at `x = 6`, 38 faces tall and 5 slabs deep (190 west
/// faces), on the reference voxel, soil at `avail`, and one founder at the foot of the
/// middle slab on the **shipped defaults**. The dossier's "slow to cover (like
/// stonecushion, a long-lived rock plant)": a founder's cover is paid for by what its
/// faces earn, and a face earns back its cost over a long time.
fn pace_world(avail: f64, thin: bool) -> (Flora, World, u64) {
    let mut plan = Plan::wall(12, 48, 5, 40);
    if thin {
        // A thin pocket: rock under one layer of soil, so the root box holds half the soil.
        for x in 0..12 {
            for z in 0..5 {
                plan.set(x, 1, z, Material::Rock);
            }
        }
    }
    let world = plan.world(avail, false);
    let config = FloraConfig::default();
    let reserve = config.latticevine.founder_reserve;
    let mut flora = Flora::new(config);
    let id = seed(&mut flora, &world, foot(2), wf(3, 2), reserve);
    let lineage = vine(&flora, id).lineage;
    (flora, world, lineage)
}

/// Faces covered by the founder's lineage after each checkpoint, stepping flora alone.
/// Also the founder's root water at the end (the available-water scale).
fn pace_run(avail: f64, thin: bool, checkpoints_s: &[f64]) -> (Vec<usize>, f64) {
    let (mut flora, mut world, lineage) = pace_world(avail, thin);
    let mut out = Vec::new();
    let mut done = 0;
    for &t in checkpoints_s {
        let n = ticks(t);
        for _ in done..n {
            flora.step(&mut world);
        }
        done = n;
        out.push(lineage_faces(&flora, lineage).len());
    }
    assert_residuals(&flora, &format!("pace at avail {avail}"));
    let root = flora
        .view()
        .cover
        .vines()
        .iter()
        .find(|v| v.lineage == lineage && v.root == foot(2))
        .map_or(f64::NAN, |v| v.water);
    (out, root)
}

/// Well watered (soil near saturation) and lit, a founder covers at most fifteen faces in
/// its first ten minutes: no doubling every couple of minutes.
#[test]
fn t16_a_watered_founder_covers_at_most_fifteen_faces_in_ten_minutes() {
    let n = pace_run(1.9, false, &[600.0]).0[0];
    assert!(
        n <= 15,
        "slow to cover: one founder covered {n} faces in 600 s"
    );
}

/// The same wall over three hours: at least sixty faces and still growing (or the wall
/// full). And a **dry root** at the same light — the same wall over a thin pocket (one
/// layer of soil over rock) at half its drained water, so the root box holds a small store
/// the cover itself drinks down — plateaus well below that: its peak is at most half the
/// watered cover, its last hour's growth at most a fifth of the watered one's, and what
/// stopped it is water — its root box has fallen deep into the moisture ramp while the
/// watered root still reads full moisture — and not any cap (the wall has 190 faces).
/// Flora stepped alone, one run each on its own thread, sampled every ten minutes.
#[test]
fn t17_three_hours_cover_sixty_faces_when_watered_and_a_dry_root_plateaus_below() {
    let every: Vec<f64> = (1..=18).map(|k| 600.0 * k as f64).collect();
    let (wet, dry) = std::thread::scope(|s| {
        let wet = s.spawn(|| pace_run(1.9, false, &every));
        let dry = s.spawn(|| pace_run(0.5, true, &every));
        (wet.join().unwrap(), dry.join().unwrap())
    });
    let ((wet, wet_water), (dry, dry_water)) = (wet, dry);
    eprintln!(
        "faces every 10 min over 3 h — wet {wet:?} (root water {wet_water:.3}), \
         dry {dry:?} (root water {dry_water:.3})"
    );
    let vc = FloraConfig::default().latticevine;
    let w3 = wet[17];
    assert!(w3 >= 60, "three watered hours cover at least 60 faces: {wet:?}");
    assert!(
        w3 > wet[14] || w3 >= 190,
        "and are still growing in the last half hour, or have filled the wall: {wet:?}"
    );
    let peak = dry.iter().copied().max().unwrap_or(0);
    assert!(
        2 * peak <= w3,
        "a dry root plateaus well below the watered one: dry {dry:?}, wet {wet:?}"
    );
    assert!(
        5 * dry[17].saturating_sub(dry[11]) <= w3 - wet[11],
        "the dry cover has all but stopped in the last hour: dry {dry:?}, wet {wet:?}"
    );
    let low = vc.wilt_water + 0.4 * (vc.full_water - vc.wilt_water);
    assert!(
        dry_water < low && wet_water >= vc.full_water,
        "water is the limit: the dry root reads {dry_water:.3} (under {low:.3}), the \
         watered root {wet_water:.3}"
    );
}
