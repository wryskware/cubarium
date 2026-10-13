//! **LARVA's laws** (`design/larva-nuts-plan-2026-10-12.md` §7; written before the
//! rule in the package's brief): laws only — closed books, stoichiometry, the decay law,
//! the browse fraction, emergence counting, a host's loss, the host share, a seedporter's
//! bite and sight, and no juvenile bellwing body — never a tuned value, ratio or hash.
//!
//! Fixtures are hand-built at the 0.25 m reference voxel: soil at `y = 1..=2`, so a plain
//! column's support face is `y = 2` and its surface is 0.75 m. A rock column at `x = 6`
//! carries a latticevine on its `−x` faces at `z = 2`, rows 3..=10. Vaulttrees are seeded
//! full-grown. The field is stepped directly ([`crate::larva::step_field`]) on its own clock
//! unless a test needs the whole layer.

use cubarium_voxel::weather::{DayClock, DayMode};
use cubarium_voxel::{Command as WorldCommand, Config as VoxelConfig, DT, Material, World};
use cubarium_voxel_flora::{
    Command as FloraCommand, Face, FaceDir, FaceLayer, Flora, FloraConfig, Site, Species as Plant, VineSeed,
};

use crate::larva::{Cohort, LarvaHost, LarvaLedger};
use crate::{Actions, Animal, Command, Fauna, FaunaConfig, Food, Founder, Pose, Scripted, StartingStores};

const V: f64 = 0.25;
const BW: Founder = Founder::Bellwing;
const SP: Founder = Founder::Seedporter;
const EAST: f64 = std::f64::consts::FRAC_PI_2;
const WALL_X: i64 = 6;
const WALL_Z: u32 = 2;
/// The wall's `−x` face plane, metres.
const WALL_FACE_M: f64 = 6.0 * V;

// ------------------------------------------------------------------ fixtures

fn config(width: u32, height: u32, depth: u32) -> VoxelConfig {
    VoxelConfig {
        width,
        height,
        depth,
        voxel_m: V,
        seed: 7,
        day: DayClock { mode: DayMode::Off, ..DayClock::default() },
        ..VoxelConfig::default()
    }
}

/// A plain with soil at `y = 1..=2` at field capacity, and the vine's rock column.
fn world(width: u32, height: u32, depth: u32) -> World {
    let mut w = World::empty(config(width, height, depth));
    let c = w.config().clone();
    let m = cubarium_voxel::Materials::DEFAULT;
    let cap = m.pore_capacity(Material::Soil) * c.voxel_volume();
    let fc = m.field_capacity(Material::Soil);
    for z in 0..c.depth {
        for x in 0..i64::from(c.width) {
            for y in 1..=2 {
                w.apply(WorldCommand::AddWater { x, y, z, volume_m3: fc * cap });
                w.apply(WorldCommand::SetMaterial { x, y, z, material: Material::Soil });
            }
        }
        for y in 1..=12 {
            w.apply(WorldCommand::SetMaterial { x: WALL_X, y, z, material: Material::Rock });
        }
    }
    w
}

/// The vine on the wall's `−x` faces at `z = WALL_Z`, rows 3..=10, every face leafy. Its id
/// and faces.
fn vine(flora: &mut Flora, world: &World) -> (u64, Vec<Face>) {
    let first = Face::new(WALL_X as u32, 3, WALL_Z, FaceDir::NegX);
    let id = flora
        .seed_vine(world, VineSeed { root: Site { x: WALL_X as u32 - 1, y: 2, z: WALL_Z }, first, reserve: 100.0, lineage: None })
        .expect("fixture: the vine");
    let mut faces = vec![first];
    for y in 4..=10 {
        let f = Face::new(WALL_X as u32, y, WALL_Z, FaceDir::NegX);
        assert!(flora.cover_face(world, id, f, 1.0), "fixture: cover {f:?}");
        faces.push(f);
    }
    (id, faces)
}

/// A full-grown vaulttree on `(x, z)`: its site and id. It hangs no fruit, so a glider's
/// mouth finds larvae there and nothing else.
fn vaulttree(flora: &mut Flora, world: &World, x: i64, z: u32) -> (Site, u64) {
    let wood = flora.config().species(Plant::Vaulttree).wood_max;
    assert!(flora.apply(world, FloraCommand::Seed { x, z, species: Plant::Vaulttree, wood }));
    let fv = flora.view();
    let s = fv.stands.iter().find(|s| i64::from(s.site.x) == x && s.site.z == z).expect("the stand");
    assert!(crate::larva::stand_hosts(&fv, s), "fixture: a crowned vaulttree with leaves hosts larvae");
    (s.site, s.id)
}

fn flora_config() -> FloraConfig {
    let mut c = FloraConfig::default();
    c.vaulttree.fruit_per_s = 0.0;
    c.vaulttree.fruit_cap = 0.0;
    c
}

/// The bellwing's larva table, adjusted.
fn with_larva(f: impl Fn(&mut crate::larva::Larva)) -> FaunaConfig {
    let mut c = FaunaConfig::default();
    let mut t = c.founders[BW.index()].larva.expect("the bellwing has a larva table");
    f(&mut t);
    c.founders[BW.index()].larva = Some(t);
    c
}

fn larva_table(fauna: &Fauna) -> crate::larva::Larva {
    fauna.config().founder(BW).larva.expect("the bellwing's larva table")
}

/// The emergence body and the size a larva emerges at (body and reserve).
fn emerge(fauna: &Fauna) -> (f64, f64) {
    crate::larva::emerge_size(fauna, BW).expect("a larva lineage")
}

fn incubation(fauna: &Fauna) -> f64 {
    fauna.config().founder(BW).core.reproduction.incubation_s
}

fn n_tissue(fauna: &Fauna) -> f64 {
    fauna.config().founder(BW).core.n_tissue
}

/// `count` larvae of one body, at `age_s`, at the larva's tissue stoichiometry.
fn cohort(fauna: &Fauna, count: f64, body: f64, age_s: f64) -> Cohort {
    let organic = count * body;
    let sc = fauna.config().founder(BW).core;
    Cohort { count, organic, mineral: sc.n_tissue * organic, energy: sc.energy_density * organic, age_s, unfed_s: 0.0 }
}

fn put(fauna: &mut Fauna, host: LarvaHost, site: Site, anchor: Option<Face>, (n, body, age): (f64, f64, f64)) {
    let c = cohort(fauna, n, body, age);
    fauna.introduce_larvae(BW, host, site, anchor, c);
}

/// One turn of the field's clock: the tick set to its next due tick, then the field's step.
/// Emerged adults join the layer, as the step's reproduction puts them in.
fn field_step(fauna: &mut Fauna, world: &World, flora: &mut Flora) -> Vec<Animal> {
    let stride = cubarium_voxel::ticks_at_least_one(larva_table(fauna).step_s);
    fauna.tick = (fauna.tick / stride + 1) * stride;
    let mut born = Vec::new();
    crate::larva::step_field(fauna, &world.view(), flora, &mut born);
    for a in &born {
        fauna.insert(*a);
    }
    born
}

fn dt_step(fauna: &Fauna) -> f64 {
    cubarium_voxel::ticks_at_least_one(larva_table(fauna).step_s) as f64 * DT
}

/// `(organic, mineral, energy)` of the field.
fn field(fauna: &Fauna) -> [f64; 3] {
    let f = fauna.view().larvae;
    [f.organic(), f.mineral(), f.energy()]
}

fn count(fauna: &Fauna) -> f64 {
    fauna.view().larvae.count_of(BW)
}

fn ledger(fauna: &Fauna) -> LarvaLedger {
    fauna.view().ledger.larva
}

fn close(got: f64, want: f64, what: &str) {
    assert!((got - want).abs() <= 1e-9 * got.abs().max(want.abs()).max(1e-9), "{what}: {got} against {want}");
}

/// Both layers' books close.
fn layers_close(fauna: &Fauna, flora: &Flora) {
    let av = fauna.view();
    let fv = flora.view();
    for (name, got, want) in [
        ("fauna organic", av.organic(), av.ledger.expected_organic()),
        ("fauna mineral", av.mineral(), av.ledger.expected_mineral()),
        ("fauna energy", av.energy(), av.ledger.expected_energy()),
        ("flora organic", fv.organic(), fv.ledger.expected_organic()),
        ("flora mineral", fv.mineral(), fv.ledger.expected_mineral()),
        ("flora energy", fv.energy(), fv.ledger.expected_energy()),
    ] {
        assert!((got - want).abs() <= 1e-9 * got.abs().max(1.0), "the {name} ledger must close: {got} against {want}");
    }
}

fn animal(fauna: &Fauna, id: u64) -> Animal {
    *fauna.view().animal(id).expect("the body is alive")
}

fn hold(fauna: &mut Fauna, id: u64, forward: f64, feed: f64) {
    assert!(fauna.set_controller(id, Box::new(Scripted::new(vec![Actions { forward, turn: 0.0, feed }]))));
}

/// A world with the vine and a vaulttree, each hosting larvae: eggs, young larvae and a
/// cohort about to emerge on the tree, young larvae on the vine.
fn hosted(fauna_config: FaunaConfig) -> (World, Flora, Fauna, (Site, u64), (u64, Vec<Face>)) {
    let world = world(40, 64, 16);
    let mut flora = Flora::new(flora_config());
    let tree = vaulttree(&mut flora, &world, 20, 8);
    let vine = vine(&mut flora, &world);
    let mut fauna = Fauna::new(fauna_config);
    let egg = fauna.config().founder(BW).core.reproduction.egg_organic;
    let inc = incubation(&fauna);
    let (_, target) = emerge(&fauna);
    put(&mut fauna, LarvaHost::Stand(tree.1), tree.0, None, (30.0, egg, 0.0));
    put(&mut fauna, LarvaHost::Stand(tree.1), tree.0, None, (12.0, 4.0 * egg, inc + 600.0));
    put(&mut fauna, LarvaHost::Stand(tree.1), tree.0, None, (4.5, 0.995 * target, inc + 9000.0));
    let root = flora.view().cover.vine(vine.0).expect("the vine").root;
    put(&mut fauna, LarvaHost::Vine(vine.0), root, Some(vine.1[2]), (20.0, 3.0 * egg, inc + 300.0));
    (world, flora, fauna, tree, vine)
}

// ------------------------------------------------------------------ 1. closed books

/// **Closed books per step** (§7): over every turn of the field's clock, for organic matter,
/// mineral and energy, leaf in + eggs in + founded in = the field's change + emerged + eaten
/// + litter + respired; and both layers' own books close through it, emergence included.
#[test]
fn the_books_close_every_step_for_organic_mineral_and_energy() {
    let (world, mut flora, mut fauna, _, _) = hosted(FaunaConfig::default());
    layers_close(&fauna, &flora);
    let mut emerged = 0;
    let mut pooled = false;
    for _ in 0..60 {
        let (f0, l0) = (field(&fauna), ledger(&fauna));
        emerged += field_step(&mut fauna, &world, &mut flora).len();
        let (f1, l1) = (field(&fauna), ledger(&fauna));
        let d = |a: crate::larva::Books, b: crate::larva::Books| [b.organic - a.organic, b.mineral - a.mineral, b.energy - a.energy];
        let ins: Vec<[f64; 3]> = vec![d(l0.eggs_in, l1.eggs_in), d(l0.founded_in, l1.founded_in), d(l0.leaf_in, l1.leaf_in)];
        let outs: Vec<[f64; 3]> = vec![
            d(l0.emerged_out, l1.emerged_out),
            d(l0.eaten_seedporter_out, l1.eaten_seedporter_out),
            d(l0.eaten_browser_out, l1.eaten_browser_out),
            d(l0.litter_out, l1.litter_out),
        ];
        let respired = [l1.respired_out - l0.respired_out, 0.0, l1.heat_out - l0.heat_out];
        for (k, name) in ["organic", "mineral", "energy"].iter().enumerate() {
            let inflow: f64 = ins.iter().map(|x| x[k]).sum();
            let outflow: f64 = outs.iter().map(|x| x[k]).sum::<f64>() + respired[k];
            let change = f1[k] - f0[k];
            assert!(
                (change - (inflow - outflow)).abs() <= 1e-12 * f0[k].abs().max(inflow.abs()).max(1e-12),
                "{name}: the field changed {change}, in {inflow} out {outflow}"
            );
        }
        layers_close(&fauna, &flora);
        pooled |= fauna.view().larvae.pool_of(BW).count > 0.0;
    }
    assert!(emerged > 0, "fixture: the step's flows include an emergence");
    assert!(pooled, "fixture: the ready pool held larvae (LARVA-FIX)");
    assert!(ledger(&fauna).leaf_in.organic > 0.0, "fixture: and leaf eaten");
}

// ------------------------------------------------------------------ 2. stoichiometry

/// **Larval mineral is always `n_tissue` × tissue, and growth never exceeds what the leaf's
/// mineral funds** (§7). Through every step every cohort's mineral is `n_tissue` × its
/// tissue; with no deaths, the field's tissue gain in a step is at most the leaf mineral
/// eaten over `n_tissue`, and at most `foliage_yield` × the leaf eaten; on a vine whose leaf
/// carries no mineral the field never gains tissue at all.
#[test]
fn larval_mineral_is_always_n_tissue_times_tissue_and_growth_is_bound_by_leaf_mineral() {
    let (world, mut flora, mut fauna, _, vine) = hosted(with_larva(|t| t.death_per_s = 0.0));
    let n = n_tissue(&fauna);
    let y = larva_table(&fauna).foliage_yield;
    assert_eq!(flora.view().cover.vine(vine.0).expect("the vine").mineral, 0.0, "fixture: the vine's leaf carries no mineral");
    for _ in 0..40 {
        let organic_before: Vec<(LarvaHost, f64)> = fauna.view().larvae.hosts().map(|(k, h)| (*k, h.organic())).collect();
        let l0 = ledger(&fauna);
        let leaf_before = fauna.view().ledger.larva.leaf_vaulttree;
        let born = field_step(&mut fauna, &world, &mut flora);
        let l1 = ledger(&fauna);
        for (_, h) in fauna.view().larvae.hosts() {
            for c in &h.cohorts {
                assert!((c.mineral - n * c.organic).abs() <= 1e-12 * c.organic.max(1e-12), "mineral {} against {} x {}", c.mineral, n, c.organic);
            }
        }
        if born.is_empty() {
            let leaf = crate::larva::Books {
                organic: l1.leaf_in.organic - l0.leaf_in.organic,
                mineral: l1.leaf_in.mineral - l0.leaf_in.mineral,
                energy: 0.0,
            };
            let gained: f64 = fauna
                .view()
                .larvae
                .hosts()
                .map(|(k, h)| h.organic() - organic_before.iter().find(|(j, _)| j == k).map_or(0.0, |(_, o)| *o))
                .sum();
            assert!(gained <= leaf.mineral / n + 1e-15, "tissue gained {gained} beyond the leaf mineral's {}", leaf.mineral / n);
            assert!(gained <= y * leaf.organic + 1e-15, "tissue gained {gained} beyond yield x leaf {}", y * leaf.organic);
            let vine_before = organic_before.iter().find(|(k, _)| *k == LarvaHost::Vine(vine.0)).map_or(0.0, |(_, o)| *o);
            let vine_now = fauna.view().larvae.host(LarvaHost::Vine(vine.0)).map_or(0.0, |h| h.organic());
            assert!(vine_now <= vine_before, "no tissue out of mineral-free leaf: {vine_before} to {vine_now}");
        }
        let _ = leaf_before;
    }
    assert!(ledger(&fauna).leaf_vaulttree > 0.0, "fixture: the tree's larvae ate");
}

// ------------------------------------------------------------------ 3. the decay law

/// **With no eaters the expected count follows `exp(−death_per_s · t)`** (§7): a cohort on a
/// vaulttree, nothing eating it and short of emergence, numbers `n₀ · exp(−μ t)` after
/// every step, eggs and larvae alike.
#[test]
fn with_no_eaters_the_count_follows_the_background_death_rate() {
    let world = world(40, 64, 16);
    let mut flora = Flora::new(flora_config());
    let tree = vaulttree(&mut flora, &world, 20, 8);
    let mut fauna = Fauna::new(FaunaConfig::default());
    let mu = larva_table(&fauna).death_per_s;
    assert!(mu > 0.0, "fixture: a background death rate");
    let egg = fauna.config().founder(BW).core.reproduction.egg_organic;
    let n0 = 100.0;
    put(&mut fauna, LarvaHost::Stand(tree.1), tree.0, None, (n0, egg, 0.0));
    let dt = dt_step(&fauna);
    for k in 1..=60 {
        assert!(field_step(&mut fauna, &world, &mut flora).is_empty(), "fixture: short of emergence");
        let want = n0 * (-mu * dt * f64::from(k)).exp();
        close(count(&fauna), want, &format!("count after {k} steps"));
    }
}

// ------------------------------------------------------------------ 4. browsing

/// **A browser eating a fraction of a vaulttree's edible foliage takes that fraction of its
/// larvae; none from vines** (§7). A loftstrider browses a vaulttree hosting larvae: on every
/// tick it bites (off the field's own clock), the tree's larvae fall by exactly the share of
/// its edible foliage the bite took, eggs included, and what they carried rides in its bite;
/// the vine's larvae, which nothing browses, keep their count.
#[test]
fn a_browser_eating_a_fraction_of_a_vaulttree_takes_that_fraction_of_its_larvae_and_none_from_vines() {
    let mut cfg = with_larva(|t| t.death_per_s = 0.0);
    cfg.founders[Founder::Loftstrider.index()].core.bite_half_stock = 0.0;
    let world = world(40, 64, 16);
    let mut flora = Flora::new(flora_config());
    let tree = vaulttree(&mut flora, &world, 20, 8);
    let vine = vine(&mut flora, &world);
    let mut fauna = Fauna::new(cfg);
    let egg = fauna.config().founder(BW).core.reproduction.egg_organic;
    let inc = incubation(&fauna);
    put(&mut fauna, LarvaHost::Stand(tree.1), tree.0, None, (40.0, egg, 0.0));
    put(&mut fauna, LarvaHost::Stand(tree.1), tree.0, None, (25.0, 5.0 * egg, inc + 100.0));
    let root = flora.view().cover.vine(vine.0).expect("the vine").root;
    put(&mut fauna, LarvaHost::Vine(vine.0), root, Some(vine.1[0]), (10.0, 5.0 * egg, inc + 100.0));
    assert!(fauna.apply(
        &world,
        Command::IntroduceFounder { x: 16, z: 8, founder: Founder::Loftstrider, stores: StartingStores { body: 1.0, reserve: 0.0 }, heading_rad: EAST },
    ));
    let id = fauna.view().ledger.births - 1;
    hold(&mut fauna, id, 0.0, 1.0);
    let stride = cubarium_voxel::ticks_at_least_one(larva_table(&fauna).step_s);
    let vine_count = |f: &Fauna| f.view().larvae.host(LarvaHost::Vine(vine.0)).map_or(0.0, |h| h.count());
    let vine0 = vine_count(&fauna);
    let mut checked = 0;
    for _ in 0..120 {
        let field_due = fauna.tick % stride == 0;
        let edible = |fl: &Flora| {
            let fv = flora_view(fl);
            fv.stand_in(tree.0, FaceLayer::Plant).map_or(0.0, |s| fv.edible_foliage(s))
        };
        let (e0, n0, b0) = (edible(&flora), fauna.view().larvae.host(LarvaHost::Stand(tree.1)).map_or(0.0, |h| h.count()), ledger(&fauna).eaten_browser_out);
        let bites0 = fauna.view().ledger.bites_by_founder[Founder::Loftstrider.index()];
        fauna.step(&world, &mut flora);
        let bit = fauna.view().ledger.bites_by_founder[Founder::Loftstrider.index()] > bites0;
        if field_due || !bit || !(n0 > 0.0) {
            continue;
        }
        let (e1, n1) = (edible(&flora), fauna.view().larvae.host(LarvaHost::Stand(tree.1)).map_or(0.0, |h| h.count()));
        let fraction = (e0 - e1) / e0;
        close(n0 - n1, fraction * n0, "larvae taken against the foliage share");
        assert!(ledger(&fauna).eaten_browser_out.organic > b0.organic, "their tissue went into the bite");
        checked += 1;
    }
    assert!(checked > 0, "fixture: the loftstrider browsed the hosting tree");
    close(vine_count(&fauna), vine0, "the vine's larvae");
}

fn flora_view(flora: &Flora) -> cubarium_voxel_flora::FloraView<'_> {
    flora.view()
}

// ------------------------------------------------------------------ 5. emergence

/// A row of full-grown vaulttrees on `z = 8`, one per `x`: their sites and ids.
fn vaulttrees(flora: &mut Flora, world: &World, xs: &[i64]) -> Vec<(Site, u64)> {
    xs.iter().map(|&x| vaulttree(flora, world, x, 8)).collect()
}

/// **No fractional adult is lost: ready remainders pool per lineage** (LARVA-FIX). Five
/// vaulttrees each hold one cohort of a fractional count at the emergence size; at the next
/// turn of the clock the field emerges, in all, the floor of the summed count, each body at
/// the host whose cohort carried the pool over a whole larva (hosts in the field's order),
/// with a pool larva's share; the pool keeps the rest; nothing goes to litter; every
/// currency is conserved exactly (field, pool and emerged together). No upkeep or death
/// here, so emergence is the only flow.
#[test]
fn ready_remainders_pool_per_lineage_and_emerge_whole_adults_with_nothing_to_litter() {
    let world = world(48, 64, 16);
    let mut flora = Flora::new(flora_config());
    let trees = vaulttrees(&mut flora, &world, &[12, 18, 24, 30, 36]);
    let mut cfg = with_larva(|t| t.death_per_s = 0.0);
    cfg.founders[BW.index()].core.maintenance_per_s = 0.0;
    let mut fauna = Fauna::new(cfg);
    let (body, target) = emerge(&fauna);
    let inc = incubation(&fauna);
    let counts = [0.3, 0.45, 0.6, 0.35, 0.5];
    for (t, n) in trees.iter().zip(counts) {
        put(&mut fauna, LarvaHost::Stand(t.1), t.0, None, (n, target, inc + 1.0));
    }
    // Where each whole larva completes, walking the hosts in the field's (key) order.
    let mut order: Vec<(LarvaHost, Site, f64)> = trees.iter().zip(counts).map(|(t, n)| (LarvaHost::Stand(t.1), t.0, n)).collect();
    order.sort_by(|a, b| a.0.cmp(&b.0));
    let (mut sum, mut want_sites) = (0.0, Vec::new());
    for (_, site, n) in &order {
        let before = sum;
        sum += n;
        for _ in 0..((sum + 1e-9).floor() - (before + 1e-9).floor()) as usize {
            want_sites.push(*site);
        }
    }
    let total: f64 = counts.iter().sum();
    let [o0, m0, e0] = field(&fauna);
    let litter0 = ledger(&fauna).litter_out;
    let born = field_step(&mut fauna, &world, &mut flora);
    assert_eq!(born.len() as f64, total.floor(), "the floor of the summed count");
    let l = ledger(&fauna);
    assert_eq!(l.emerged, born.len() as u64);
    assert_eq!(l.litter_out, litter0, "nothing to litter at emergence");
    let pool = fauna.view().larvae.pool_of(BW);
    close(pool.count, total - total.floor(), "the pool keeps the rest");
    assert_eq!(fauna.view().larvae.hosts().count(), 0, "every ready cohort left its host");
    let [o1, m1, e1] = field(&fauna);
    close(o1 + l.emerged_out.organic, o0, "organic conserved");
    close(m1 + l.emerged_out.mineral, m0, "mineral conserved");
    close(e1 + l.emerged_out.energy, e0, "energy conserved");
    close(o1, pool.organic, "the field's matter is the pool's");
    let mut sites: Vec<Site> = born.iter().map(|a| a.site).collect();
    sites.sort_by_key(|s| (s.x, s.y, s.z));
    want_sites.sort_by_key(|s| (s.x, s.y, s.z));
    assert_eq!(sites, want_sites, "each adult at the host that completed it");
    for a in &born {
        assert_eq!(a.founder, Some(BW));
        close(a.body, body, "the emergence body");
        close(a.body + a.reserve, target, "a pool larva's tissue");
    }
    layers_close(&fauna, &flora);
    // The pool carries over: more ready larvae on one host complete the next adult there.
    let more = 1.0 - pool.count + 0.25;
    put(&mut fauna, LarvaHost::Stand(trees[0].1), trees[0].0, None, (more, target, inc + 1.0));
    let born = field_step(&mut fauna, &world, &mut flora);
    assert_eq!(born.len(), 1, "the carried remainder completes an adult");
    assert_eq!(born[0].site, trees[0].0, "at the host that completed it");
    close(fauna.view().larvae.pool_of(BW).count, 0.25, "and the pool keeps the new rest");
}

// ------------------------------------------------------------------ 6. a host's loss

/// **A host's death drops its larvae to litter** (§7): the vaulttree is cleared; at the next
/// turn of the clock its larvae are gone, booked dead with their host, and their tissue lies
/// in the litter of its face.
#[test]
fn a_hosts_death_drops_its_larvae_to_litter() {
    let world = world(40, 64, 16);
    let mut flora = Flora::new(flora_config());
    let tree = vaulttree(&mut flora, &world, 20, 8);
    let mut fauna = Fauna::new(FaunaConfig::default());
    let egg = fauna.config().founder(BW).core.reproduction.egg_organic;
    let inc = incubation(&fauna);
    put(&mut fauna, LarvaHost::Stand(tree.1), tree.0, None, (15.0, 3.0 * egg, inc + 50.0));
    put(&mut fauna, LarvaHost::Stand(tree.1), tree.0, None, (5.0, egg, 0.0));
    assert!(flora.apply(&world, FloraCommand::Clear { x: 20, z: 8 }));
    let n = count(&fauna);
    let organic = fauna.view().larvae.organic();
    let litter = |fl: &Flora| fl.view().ground_at(tree.0).map_or(0.0, |g| g.litter);
    let before = litter(&flora);
    field_step(&mut fauna, &world, &mut flora);
    assert!(fauna.view().larvae.is_empty(), "the host's larvae are gone");
    close(ledger(&fauna).died_host, n, "booked dead with their host");
    close(litter(&flora) - before, organic, "their tissue in the litter of its face");
    layers_close(&fauna, &flora);
}

/// **A standing host that offers no leaf keeps its larvae, which starve; a host that is gone
/// drops them** (LARVA-FIX). A vine stripped of every leaf, and a vaulttree stripped of its
/// edible foliage, keep their hatched larvae: they lose tissue to upkeep each step and die
/// once unfed for `starve_s`, booked starved and never as host loss. A vine taken out of the
/// cover drops its larvae at the next turn of the clock, booked dead with their host.
#[test]
fn a_leafless_host_keeps_its_larvae_which_starve_and_a_removed_vine_drops_them() {
    let world = world(40, 64, 16);
    let mut flora = Flora::new(flora_config());
    let tree = vaulttree(&mut flora, &world, 20, 8);
    let (vine_id, faces) = vine(&mut flora, &world);
    let mut fauna = Fauna::new(with_larva(|t| t.death_per_s = 0.0));
    let starve = larva_table(&fauna).starve_s;
    assert!(starve > 0.0, "fixture: larvae last a while unfed");
    let egg = fauna.config().founder(BW).core.reproduction.egg_organic;
    let inc = incubation(&fauna);
    let root = flora.view().cover.vine(vine_id).expect("the vine").root;
    for f in &faces {
        let _ = flora.crop(*f, 1.0e9);
    }
    let _ = flora.take_foliage_by_insects(tree.0, 1.0e9);
    {
        let fv = flora.view();
        assert!(fv.cover.iter().filter(|f| f.owner == vine_id).all(|f| f.leafiness <= 0.0), "fixture: no leafy face");
        assert!(!crate::larva::stand_hosts(&fv, fv.stand_at(tree.0).expect("the tree")), "fixture: no edible leaf");
    }
    put(&mut fauna, LarvaHost::Vine(vine_id), root, Some(faces[2]), (20.0, 3.0 * egg, inc + 300.0));
    put(&mut fauna, LarvaHost::Stand(tree.1), tree.0, None, (10.0, 3.0 * egg, inc + 300.0));
    let n = count(&fauna);
    let dt = dt_step(&fauna);
    let most = (starve / dt).ceil() as usize + 2;
    let mut steps = 0;
    let mut last = fauna.view().larvae.organic();
    while !fauna.view().larvae.is_empty() && steps < most {
        field_step(&mut fauna, &world, &mut flora);
        steps += 1;
        let now = fauna.view().larvae.organic();
        if !fauna.view().larvae.is_empty() {
            assert!(now < last, "unfed larvae lose tissue to upkeep: {last} to {now}");
            for (k, _) in [(LarvaHost::Vine(vine_id), ()), (LarvaHost::Stand(tree.1), ())] {
                assert!(fauna.view().larvae.host(k).is_some(), "{k:?} keeps its larvae while they last");
            }
        }
        last = now;
    }
    assert!(fauna.view().larvae.is_empty(), "they starved within starve_s ({steps} steps)");
    assert!(steps as f64 * dt >= starve - dt, "not before starve_s unfed ({steps} steps of {dt} s)");
    let l = ledger(&fauna);
    assert_eq!(l.died_host, 0.0, "no host loss");
    close(l.died_starved, n, "every larva starved");
    assert_eq!(l.emerged, 0);

    // A vine taken out of the cover drops its larvae.
    let world = self::world(40, 64, 16);
    let mut flora = Flora::new(flora_config());
    let (vine_id, faces) = vine(&mut flora, &world);
    let mut fauna = Fauna::new(with_larva(|t| t.death_per_s = 0.0));
    let root = flora.view().cover.vine(vine_id).expect("the vine").root;
    put(&mut fauna, LarvaHost::Vine(vine_id), root, Some(faces[2]), (20.0, 3.0 * egg, inc + 300.0));
    let n = count(&fauna);
    assert_eq!(flora.clear_vines_at(root), 1, "fixture: the vine is removed");
    field_step(&mut fauna, &world, &mut flora);
    assert!(fauna.view().larvae.is_empty(), "the removed vine's larvae are gone");
    close(ledger(&fauna).died_host, n, "booked dead with their host");
    assert_eq!(ledger(&fauna).died_starved, 0.0);
}

/// **Founding places no cohort on a host that offers no leaf** (LARVA-FIX): of two vines
/// and two vaulttrees, one of each stripped of its leaf, the founding cohorts go only to the
/// leafy ones (both take some).
#[test]
fn founding_places_no_cohort_on_a_host_that_offers_no_leaf() {
    let world = world(40, 64, 16);
    let mut flora = Flora::new(flora_config());
    let leafy_tree = vaulttree(&mut flora, &world, 20, 8);
    let bare_tree = vaulttree(&mut flora, &world, 30, 8);
    let (leafy_vine, _) = vine(&mut flora, &world);
    let bare_z = 10;
    let first = Face::new(WALL_X as u32, 3, bare_z, FaceDir::NegX);
    let bare_vine = flora
        .seed_vine(&world, VineSeed { root: Site { x: WALL_X as u32 - 1, y: 2, z: bare_z }, first, reserve: 100.0, lineage: None })
        .expect("fixture: the second vine");
    let mut bare_faces = vec![first];
    for y in 4..=6 {
        let f = Face::new(WALL_X as u32, y, bare_z, FaceDir::NegX);
        assert!(flora.cover_face(&world, bare_vine, f, 1.0), "fixture: cover {f:?}");
        bare_faces.push(f);
    }
    for f in &bare_faces {
        let _ = flora.crop(*f, 1.0e9);
    }
    let _ = flora.take_foliage_by_insects(bare_tree.0, 1.0e9);
    let mut fauna = Fauna::new(FaunaConfig::default());
    assert!(fauna.apply(
        &world,
        Command::IntroduceAloft { pose: Pose { x: 4.0, z: 2.0, heading_rad: EAST, y_m: 1.25 }, founder: BW, stores: StartingStores::FULL },
    ));
    assert!(larva_table(&fauna).founding_hosts >= 4, "fixture: room for every host");
    let n = fauna.found_larvae(&world, &flora, BW);
    assert!(n > 0.0, "founding put larvae down");
    let f = fauna.view().larvae;
    assert!(f.host(LarvaHost::Stand(bare_tree.1)).is_none(), "none on the leafless vaulttree");
    assert!(f.host(LarvaHost::Vine(bare_vine)).is_none(), "none on the leafless vine");
    assert!(f.host(LarvaHost::Stand(leafy_tree.1)).is_some(), "the leafy vaulttree takes some");
    assert!(f.host(LarvaHost::Vine(leafy_vine)).is_some(), "the leafy vine takes some");
    layers_close(&fauna, &flora);
}

// ------------------------------------------------------------------ 7. the host share

/// **A share of 0 or 1 lays only on vines or only on vaulttrees when both are in range**
/// (§7): a ready bellwing with a leafy vine and a crowned vaulttree both within its nest
/// search goes to (or lays at) the vine with `vaulttree_share` 0 and the vaulttree with 1,
/// for every clutch it might carry.
#[test]
fn a_share_of_zero_or_one_lays_only_on_vines_or_only_on_vaulttrees() {
    for share in [0.0, 1.0] {
        let world = world(40, 64, 16);
        let mut flora = Flora::new(flora_config());
        let tree = vaulttree(&mut flora, &world, 2, 4);
        let vine = vine(&mut flora, &world);
        let mut fauna = Fauna::new(with_larva(|t| t.vaulttree_share = share));
        let flight = fauna.config().founder(BW).flight.expect("a flyer");
        assert!(
            crate::larva::crown_band(&flora.view(), flora.view().stand_at(tree.0).expect("tree")).expect("a crown").0 - 0.75 <= flight.band_max_m,
            "fixture: the crown is under the bellwing's ceiling"
        );
        assert!(fauna.apply(
            &world,
            Command::IntroduceAloft { pose: Pose { x: WALL_FACE_M - 0.6, z: (f64::from(WALL_Z) + 0.5) * V, heading_rad: EAST, y_m: 1.25 }, founder: BW, stores: StartingStores::FULL },
        ));
        let i = fauna.animals.len() - 1;
        for given in [0.0, 0.003, 0.006, 0.009, 0.012] {
            fauna.animals[i].reproduction.given = given;
            let goal = crate::step::larva_goal(&mut fauna, i, &world.view(), &flora).expect("a host in range");
            let on_tree = match goal {
                crate::step::LarvaGoal::Here(at) => {
                    if share == 0.0 {
                        assert_eq!(at.key, LarvaHost::Vine(vine.0));
                    }
                    at.key == LarvaHost::Stand(tree.1)
                }
                crate::step::LarvaGoal::Fly(p) => {
                    let (tx, tz) = ((f64::from(tree.0.x) + 0.5) * V, (f64::from(tree.0.z) + 0.5) * V);
                    (p.x - tx).abs() < 1e-9 && (p.z - tz).abs() < 1e-9
                }
            };
            assert_eq!(on_tree, share == 1.0, "share {share}: {goal:?}");
        }
    }
}

// ------------------------------------------------------------------ 8. a seedporter's bite

/// **A seedporter's larva bite carries `n_tissue` mineral per organic and lowers the count**
/// (§7): a hungry seedporter perched in a vaulttree hosting larvae, holding feed, bites
/// them: its stomach's larva pool and the field's books hold mineral at `n_tissue` × the
/// organic eaten, and the tree's larvae number less by the larvae it took.
#[test]
fn a_seedporter_larva_bite_carries_n_tissue_mineral_and_lowers_the_count() {
    let world = world(40, 64, 16);
    let mut flora = Flora::new(flora_config());
    let tree = vaulttree(&mut flora, &world, 20, 8);
    let mut fauna = Fauna::new(FaunaConfig::default());
    let egg = fauna.config().founder(BW).core.reproduction.egg_organic;
    let inc = incubation(&fauna);
    put(&mut fauna, LarvaHost::Stand(tree.1), tree.0, None, (20.0, 20.0 * egg, inc + 100.0));
    let top = flora
        .view()
        .layers_at(tree.0)
        .filter(|l| l.foliage_index.is_some())
        .map(|l| l.cell)
        .max()
        .expect("a crown");
    let (x, z) = ((f64::from(tree.0.x) + 0.5) * V, (f64::from(tree.0.z) + 0.5) * V);
    assert!(fauna.apply(
        &world,
        Command::IntroduceAloft { pose: Pose { x, z, heading_rad: EAST, y_m: (top + 1) as f64 * V }, founder: SP, stores: StartingStores { body: 1.0, reserve: 0.3 } },
    ));
    let id = fauna.view().ledger.births - 1;
    hold(&mut fauna, id, 0.0, 1.0);
    // It settles on the crown it was put on.
    for _ in 0..40 {
        if animal(&fauna, id).mobility.mode == crate::MobilityMode::Perched {
            break;
        }
        fauna.step(&world, &mut flora);
    }
    assert_eq!(animal(&fauna, id).mobility.mode, crate::MobilityMode::Perched, "fixture: perched in the crown");
    let n0 = count(&fauna);
    let mut n = 0;
    while fauna.view().ledger.bites_by_founder_food[SP.index()][Food::Larva.index()] == 0 && n < 200 {
        fauna.step(&world, &mut flora);
        n += 1;
    }
    assert!(fauna.view().ledger.bites_by_founder_food[SP.index()][Food::Larva.index()] > 0, "it bit the larvae ({n} ticks)");
    let nt = n_tissue(&fauna);
    let pool = animal(&fauna, id).stomach.pools[Food::Larva.index()];
    let eaten = ledger(&fauna).eaten_seedporter_out;
    close(eaten.mineral, nt * eaten.organic, "the bite's mineral per organic");
    assert!(pool.organic > 0.0, "in the larva pool");
    assert!(pool.mineral <= nt * pool.organic * (1.0 + 1e-9), "the pool holds no more than the larvae's mineral");
    assert!(count(&fauna) < n0, "the count fell");
    close(n0 - count(&fauna), ledger(&fauna).eaten_seedporter + ledger(&fauna).died_background, "by the larvae taken (and any background death)");
}

/// **A seedporter more than `larva_sight_m` away does not see the larvae** (§7): facing a
/// vine hosting larvae, with its sight's own range far longer, it counts the vine as food in
/// sight from nearer than its larva sight and not from farther.
#[test]
fn a_seedporter_beyond_larva_sight_does_not_see_the_larvae() {
    let sight = FaunaConfig::default().founder(SP).larva_sight_m;
    assert!(sight > 0.0, "the seedporter looks for larvae");
    for (gap, seen) in [(0.6 * sight, true), (1.6 * sight, false)] {
        let world = world(40, 64, 16);
        let mut flora = Flora::new(flora_config());
        let vine = vine(&mut flora, &world);
        let mut fauna = Fauna::new(FaunaConfig::default());
        let egg = fauna.config().founder(BW).core.reproduction.egg_organic;
        let inc = incubation(&fauna);
        let root = flora.view().cover.vine(vine.0).expect("the vine").root;
        // The anchor at the eye's row, so the gap is all horizontal.
        put(&mut fauna, LarvaHost::Vine(vine.0), root, Some(vine.1[0]), (10.0, 5.0 * egg, inc + 100.0));
        // On the ground west of the wall, facing it.
        let x = WALL_FACE_M - gap;
        let col = (x / V).floor() as i64;
        assert!(fauna.apply(
            &world,
            Command::IntroduceFounder { x: col, z: WALL_Z, founder: SP, stores: StartingStores { body: 1.0, reserve: 0.3 }, heading_rad: EAST },
        ));
        let i = fauna.animals.len() - 1;
        let key_face = vine.1[0];
        let found = crate::step::fruit_candidates(&fauna, i, &world.view(), &flora.view(), 100.0).expect("a glider");
        let hit = found.iter().any(|c| {
            let (sx, _, sz) = c.3;
            let (fx, _, fz) = key_face.front(&world.view()).expect("front");
            (sx - (f64::from(fx) + 0.5) * V).abs() < 1e-9 && (sz - (f64::from(fz) + 0.5) * V).abs() < 1e-9
        });
        assert_eq!(hit, seen, "gap {gap} m against larva sight {sight} m: {found:?}");
    }
}

// ------------------------------------------------------------------ 9. no juvenile body

/// **No juvenile bellwing bodies are made; adults emerge only from the field** (§7). A
/// ready bellwing hovering in a vaulttree's crown lays: its clutch goes into the field on that
/// tree, no clutch lies on any face, and no body is born. Its larvae, on a short larval
/// period, emerge as adult bodies at the emergence body, the only bellwings born.
#[test]
fn a_bellwing_lays_into_the_field_and_no_juvenile_body_is_made() {
    let mut cfg = with_larva(|t| {
        t.period_s = 600.0;
        t.death_per_s = 0.0;
        t.vaulttree_share = 1.0;
    });
    let core = &mut cfg.founders[BW.index()].core;
    core.reproduction = crate::Reproduction {
        surplus_hold_s: DT,
        birth_interval_s: 1.0e6 * DT,
        incubation_s: 20.0 * DT,
        clutch_hold_max_s: 0.0,
        nest_radius_m: 0.0,
        ..core.reproduction
    };
    let world = world(40, 64, 16);
    let mut flora = Flora::new(flora_config());
    let tree = vaulttree(&mut flora, &world, 20, 8);
    let (low, _) = crate::larva::crown_band(&flora.view(), flora.view().stand_at(tree.0).expect("tree")).expect("a crown");
    let mut fauna = Fauna::new(cfg);
    let (x, z) = ((f64::from(tree.0.x) + 0.5) * V, (f64::from(tree.0.z) + 0.5) * V);
    assert!(low + 0.05 - 0.75 <= fauna.config().founder(BW).flight.expect("a flyer").band_max_m, "fixture: under its ceiling");
    assert!(fauna.apply(
        &world,
        Command::IntroduceAloft { pose: Pose { x, z, heading_rad: EAST, y_m: low + 0.05 }, founder: BW, stores: StartingStores::FULL },
    ));
    let id = fauna.view().ledger.births - 1;
    assert!(fauna.set_controller(id, Box::new(Scripted::new(vec![Actions { forward: 0.0, turn: 0.0, feed: 0.0 }]))));
    let mut n = 0;
    while fauna.view().ledger.clutches_by_founder[BW.index()] == 0 && n < 40 {
        fauna.step(&world, &mut flora);
        n += 1;
    }
    assert_eq!(fauna.view().ledger.clutches_by_founder[BW.index()], 1, "it laid ({n} ticks)");
    assert!(fauna.view().clutches.iter().all(|c| c.lineage != BW), "no bellwing clutch on a face");
    assert!(fauna.view().larvae.host(LarvaHost::Stand(tree.1)).is_some(), "its eggs are larvae on the tree");
    let births = fauna.view().ledger.births;
    let (body, _) = emerge(&fauna);
    // Up to the emergence (incubation and a 600 s larval period on the field's clock).
    for _ in 0..(20 * 60 * 15) {
        fauna.step(&world, &mut flora);
        if ledger(&fauna).emerged > 0 {
            break;
        }
    }
    let l = ledger(&fauna);
    assert!(l.emerged > 0, "its larvae emerged");
    let newborn: Vec<&Animal> = fauna.view().animals.iter().filter(|a| a.id >= births).collect();
    assert_eq!(newborn.len() as u64, l.emerged, "every bellwing born came out of the field");
    for a in newborn {
        assert_eq!(a.founder, Some(BW));
        assert!(a.body >= body * (1.0 - 1e-9), "an adult at the emergence body, not a juvenile: {}", a.body);
    }
    assert_eq!(fauna.view().ledger.hatched, 0, "nothing hatched from a clutch");
    let _ = id;
}

// ------------------------------------------------------------------ 10. the tree check (ECO-INT)

/// Every lineage awake, no ground response, no troop pull, no bellwing catching: a seedporter
/// on its own heuristic forages and checks without other pulls.
fn check_config() -> FaunaConfig {
    let mut c = FaunaConfig::default();
    for p in &mut c.founders {
        p.active = crate::Activity::Always;
        p.vigilance = crate::Vigilance::default();
        p.catch_bellwings = false;
        p.herding = crate::Herding::default();
    }
    c
}

/// A hungry seedporter on the ground at column `(x, z)` facing `heading`, on its heuristic.
fn hungry_on_ground(fauna: &mut Fauna, world: &World, x: i64, z: u32, heading: f64) -> u64 {
    assert!(fauna.apply(world, Command::IntroduceFounder { x, z, founder: SP, stores: StartingStores { body: 1.0, reserve: 0.3 }, heading_rad: heading }));
    let id = fauna.animals.last().expect("introduced").id;
    let teachers = fauna.config.teachers;
    fauna.controllers.set(id, crate::heuristic(SP, teachers));
    id
}

fn larva_bites(fauna: &Fauna) -> u64 {
    fauna.view().ledger.bites_by_founder_food[SP.index()][Food::Larva.index()]
}

/// **A hungry seedporter with no food in sight goes to a leafy latticevine holding hatched
/// larvae, reaches them and eats them** (ECO-INT; Wrysk 2026-10-12: seedporters "check" plants
/// for food they cannot see): standing along the wall farther than its larva sight and inside
/// its check search, it sees no food, walks on the check, and the vine's larvae fall by what
/// it eats, into its larva pool.
#[test]
fn a_hungry_seedporter_checks_a_leafy_vine_and_eats_its_larvae() {
    let world = world(40, 64, 40);
    let mut flora = Flora::new(flora_config());
    let vine = vine(&mut flora, &world);
    let mut fauna = Fauna::new(check_config());
    fauna.set_births_enabled(false);
    let egg = fauna.config().founder(BW).core.reproduction.egg_organic;
    let inc = incubation(&fauna);
    let root = flora.view().cover.vine(vine.0).expect("the vine").root;
    put(&mut fauna, LarvaHost::Vine(vine.0), root, Some(vine.1[1]), (20.0, 5.0 * egg, inc + 100.0));
    assert!(crate::larva::visible(&fauna, LarvaHost::Vine(vine.0)).is_some(), "fixture: hatched larvae it would see");
    // Along the wall's west side, farther than its larva sight, within its check search,
    // facing away.
    let phys = *fauna.config().founder(SP);
    let (sight, search) = (phys.larva_sight_m, phys.glider.expect("a glider").check_search_m);
    let z = WALL_Z + ((2.0 * sight + 1.0) / V).ceil() as u32;
    let gap = f64::from(z - WALL_Z) * V;
    assert!(gap > sight && gap + 2.0 < search, "fixture: beyond larva sight ({gap} m > {sight} m), inside the check ({search} m)");
    let id = hungry_on_ground(&mut fauna, &world, WALL_X - 2, z, 0.0);
    let i = fauna.animals.iter().position(|a| a.id == id).expect("alive");
    let seen = crate::step::fruit_candidates(&fauna, i, &world.view(), &flora.view(), 100.0).expect("a glider");
    assert!(seen.is_empty(), "fixture: no food in sight: {seen:?}");
    let n0 = count(&fauna);
    let mut checked = false;
    for _ in 0..8000 {
        fauna.step(&world, &mut flora);
        checked |= fauna.steer_of(id) == Some(crate::Steer::Check);
        if larva_bites(&fauna) > 0 {
            break;
        }
    }
    assert!(checked, "it walks to the vine on the check");
    assert!(larva_bites(&fauna) > 0, "it eats the vine's larvae: {:?}", animal(&fauna, id).mobility);
    assert!(count(&fauna) < n0, "the vine's larvae fell");
    assert!(animal(&fauna, id).stomach.pools[Food::Larva.index()].organic > 0.0, "into its larva pool");
    assert!(ledger(&fauna).eaten_seedporter > 0.0, "booked as eaten by a seedporter");
    let memory = fauna.tree_checks_of(id).expect("a memory");
    assert!(memory.seen.iter().any(|c| c.host == LarvaHost::Vine(vine.0)), "it remembers the vine: {memory:?}");
    layers_close(&fauna, &flora);
}

/// **A vine host and a stand host with the same numeric id are remembered apart**: a vine
/// checked empty does not mark the stand of that number, nor the other way round, and the
/// check's order drops only the one remembered empty.
#[test]
fn a_vine_and_a_stand_of_the_same_number_are_remembered_apart() {
    use crate::treecheck::{order, recall, remember};
    let world = world(40, 64, 16);
    let mut fauna = Fauna::new(check_config());
    let id = hungry_on_ground(&mut fauna, &world, 2, 8, 0.0);
    let n = 7u64;
    let (vine, stand) = (LarvaHost::Vine(n), LarvaHost::Stand(n));
    let memory = 1_000u64;
    remember(&mut fauna, id, vine, false);
    let now = fauna.tick;
    assert_eq!(recall(&fauna, id, vine, now, memory).map(|c| c.held_food), Some(false), "the vine is remembered empty");
    assert!(recall(&fauna, id, stand, now, memory).is_none(), "the stand of the same number is not remembered");
    let cands = [(3.0, stand, ()), (3.0, vine, ())];
    let left: Vec<LarvaHost> = order(&cands, None, |h| recall(&fauna, id, h, now, memory)).into_iter().map(|c| c.1).collect();
    assert_eq!(left, vec![stand], "only the empty vine is dropped");
    remember(&mut fauna, id, stand, true);
    assert_eq!(recall(&fauna, id, stand, now, memory).map(|c| c.held_food), Some(true), "the stand is its own entry");
    assert_eq!(recall(&fauna, id, vine, now, memory).map(|c| c.held_food), Some(false), "the vine's entry is unchanged");
}

/// **A vaulttree holding larvae but no nuts counts as holding food when checked**: it is a
/// host to check, what it holds is food, and a hungry seedporter that checks it finds food
/// there (the check's readout) and eats the larvae.
#[test]
fn a_vaulttree_with_larvae_and_no_nuts_holds_food_when_checked() {
    let world = world(40, 64, 16);
    let mut flora = Flora::new(flora_config());
    let tree = vaulttree(&mut flora, &world, 26, 8);
    let mut fauna = Fauna::new(check_config());
    fauna.set_births_enabled(false);
    let egg = fauna.config().founder(BW).core.reproduction.egg_organic;
    let inc = incubation(&fauna);
    put(&mut fauna, LarvaHost::Stand(tree.1), tree.0, None, (20.0, 5.0 * egg, inc + 100.0));
    let phys = *fauna.config().founder(SP);
    {
        let fv = flora.view();
        let s = fv.stand_at(tree.0).expect("the tree");
        assert_eq!(s.fruit, 0.0, "fixture: no nuts");
        let host = crate::treecheck::Host::Stand(s);
        assert!(crate::treecheck::may_hold_hidden_food(&fauna, &fv, &phys, host), "a crowned vaulttree is a host to check");
        assert!(crate::treecheck::hidden_food(&fauna, &fv, host), "its larvae are food it holds");
    }
    let id = hungry_on_ground(&mut fauna, &world, 26 - 16, 8, -EAST);
    let n0 = count(&fauna);
    for _ in 0..8000 {
        fauna.step(&world, &mut flora);
        if larva_bites(&fauna) > 0 {
            break;
        }
    }
    let t = fauna.check_tally();
    assert!(t.checks >= 1 && t.found >= 1, "the check found food in the tree: {t:?}, {:?}", animal(&fauna, id).mobility);
    assert!(larva_bites(&fauna) > 0 && count(&fauna) < n0, "it eats the tree's larvae");
    layers_close(&fauna, &flora);
}

/// A hungry seedporter on its heuristic on the ground at `(x, z)` facing east among three
/// crowned vaulttrees (hosts within its check search), its catch on; with `carcass`, a kill's
/// carcass lies under its mouth. Its id, after `ticks` steps.
fn seedporter_among_hosts(carcass: bool, ticks: u32) -> (World, Flora, Fauna, u64) {
    let world = world(48, 64, 16);
    let mut flora = Flora::new(flora_config());
    for x in [12, 24, 36] {
        vaulttree(&mut flora, &world, x, 8);
    }
    let mut cfg = check_config();
    cfg.founders[SP.index()].catch_bellwings = true;
    let mut fauna = Fauna::new(cfg);
    fauna.set_births_enabled(false);
    let id = hungry_on_ground(&mut fauna, &world, 18, 8, EAST);
    fauna.step_with(&world, &mut flora, 1);
    if carcass {
        let a = animal(&fauna, id);
        let g = fauna.config().founder(SP).body_at(a.body);
        for (x, z) in crate::body::mouth_columns(&world.view(), &a.pose, &g) {
            let site = Site { x: x.rem_euclid(i64::from(world.view().config.width)) as u32, y: a.site.y, z };
            assert!(flora.deposit(site, cubarium_voxel_flora::Deposit { kind: cubarium_voxel_flora::DepositKind::Carrion, organic: 1.0, mineral: 0.05, energy: 2.0 }));
            assert!(flora.lay_carcass(site, cubarium_voxel_flora::Carcass { lineage: Founder::Bellwing.index() as u8, heading_rad: EAST, organic: 1.0 }));
        }
        assert!(crate::hunt::mouth_on_carcass(&flora.view(), &world.view(), &a, &g), "fixture: its mouth is over the carcass");
    }
    let i = fauna.animals.iter().position(|a| a.id == id).expect("alive");
    fauna.animals[i].eating = true;
    for _ in 0..ticks {
        fauna.step(&world, &mut flora);
        if let Some(a) = fauna.animals.iter_mut().find(|a| a.id == id) {
            a.eating = true;
        }
    }
    (world, flora, fauna, id)
}

/// **A seedporter eating a carcass near several hosts makes no check walk, switches no aim
/// and shuns nothing** (ECO-INT; the wide-world churn: a check is for a hungry body that sees
/// no food, and one eating has food). Held hungry over a carcass under its mouth among three
/// crowned vaulttrees for many times the pressed-walk clock, it eats carrion, sets out on no
/// check walk, switches no aim, its stillness counts as no pressed walk, and it gives up no
/// target. Without the carcass the same body does set out on a check (the hosts are in reach
/// of the check).
#[test]
fn a_seedporter_eating_near_several_hosts_makes_no_check_walk_and_shuns_nothing() {
    let press = FaunaConfig::default().teachers.press_replan_s;
    assert!(press > 0.0, "fixture: the pressed-walk rule is on");
    let ticks = (10.0 * press / DT).ceil() as u32;
    let (_, _, mut fauna, id) = seedporter_among_hosts(true, ticks);
    assert!(fauna.view().ledger.eaten_carrion_by_founder[SP.index()] > 0.0, "it eats the carcass");
    let t = fauna.check_tally();
    assert_eq!((t.walks, t.switches), (0, 0), "no check walk, no switch: {t:?}");
    assert!(fauna.tree_checks_of(id).is_none_or(|m| m.aim.is_none()), "no aim");
    assert!(!fauna.pressed.get(&id).is_some_and(|&p| p > 0), "its meal is no pressed walk");
    assert!(fauna.walk_shunned(id, SP).is_empty(), "it shuns nothing");

    let (_, _, fauna, _) = seedporter_among_hosts(false, ticks);
    assert!(fauna.check_tally().walks >= 1, "fixture: without food it checks the hosts: {:?}", fauna.check_tally());
}
