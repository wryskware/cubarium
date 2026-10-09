//! **Sparse, staggered flowering** (`design/handoffs/plants-flowering-vines-2026-10-03.md`),
//! written before the rule.
//!
//! Wrysk, 2026-10-03: a latticevine flowered and fruited on one timer, every cell at once;
//! flowers and fruit should be much sparser, at random times, so a big colony always shows
//! a fair number scattered across it. Lanternberries a little sparser and more randomly
//! spaced. A bloomcrown always in flower. And founding vines never took the big open
//! faces.
//!
//! - A big hand-built colony on the **shipped** spur knobs: the share of its faces in
//!   flower or fruit stays inside a band through a world day (7,200 s), with no global
//!   spike, scattered over the whole wall; almost every face flowers at some point.
//! - The colony's long-run nectar and fruit per face match the production knobs, whatever
//!   the share.
//! - The books close over the cycles, with flowers sipped, fruit eaten and the rest fallen.
//! - A bloomcrown's nectar never touches zero through a day and a night with no consumer,
//!   and at night a drained stock refills.
//! - A grove of lanternberries hangs fruit out of step: neither all nor none, and a bush
//!   eaten out rests before it fruits again (FLOWER-FRUIT, Wrysk 2026-10-09; it used to
//!   refill at once).
//! - A founder takes a big sheer lit wall over a taller staircase.

use std::collections::BTreeSet;

use cubarium_voxel::weather::{DayClock, DayMode};
use cubarium_voxel::{Command as WorldCommand, Config as VoxelConfig, Material, World};
use cubarium_voxel_flora::{
    Command, DT, Face, FaceDir, Flora, FloraConfig, Site, Species, SpurPhase, VineId, VineSeed,
};

/// The world day the animals' daily needs are written in (`LifeConfig::ecological_day_s`).
const DAY_S: f64 = 7200.0;

fn ticks(seconds: f64) -> usize {
    (seconds / DT).round() as usize
}

/// A 0.25 m world, `width × height × depth`, soil at `y = 1..=2` at field capacity, the
/// solids in `cells` over it (last write wins), the day clock `day`.
fn world(width: u32, height: u32, depth: u32, cells: &[((i64, u32, u32), Material)], day: DayClock) -> World {
    let mut w = World::empty(VoxelConfig {
        width,
        height,
        depth,
        voxel_m: 0.25,
        seed: 17,
        day,
        ..VoxelConfig::default()
    });
    let m = cubarium_voxel::Materials::DEFAULT;
    let cap = m.pore_capacity(Material::Soil) * w.config().voxel_volume();
    let fc = m.field_capacity(Material::Soil);
    let mut all: Vec<((i64, u32, u32), Material)> = Vec::new();
    for z in 0..depth {
        for x in 0..width as i64 {
            for y in 1..=2 {
                all.push(((x, y, z), Material::Soil));
            }
        }
    }
    all.extend_from_slice(cells);
    // The last write per cell, then water before material (as the other fixtures do).
    let mut last: std::collections::BTreeMap<(i64, u32, u32), Material> = Default::default();
    for &(k, mat) in &all {
        last.insert(k, mat);
    }
    for (&(x, y, z), &mat) in &last {
        if mat == Material::Soil {
            w.apply(WorldCommand::AddWater { x, y, z, volume_m3: fc * cap });
        }
        w.apply(WorldCommand::SetMaterial { x, y, z, material: mat });
    }
    w
}

fn noon() -> DayClock {
    DayClock { start_hour: 12.0, ..DayClock::default() }
}

fn closes(flora: &Flora, when: &str) {
    closes_within(flora, when, 1e-9);
}

/// [`closes`] at a relative tolerance: six hours of a 20 × 20 plain's soil organic
/// (~5,000) carry ~1e-9 of rounding whether or not the bushes rest (measured: −5.98e-6
/// with no rest, −6.05e-6 with the shipped rest).
fn closes_within(flora: &Flora, when: &str, tol: f64) {
    let v = flora.view();
    for (name, got, want) in [
        ("organic", v.organic(), v.ledger.expected_organic()),
        ("mineral", v.mineral(), v.ledger.expected_mineral()),
        ("energy", v.energy(), v.ledger.expected_energy()),
    ] {
        assert!(
            (got - want).abs() <= tol * got.abs().max(1.0),
            "{when}: the {name} ledger must close: {got} against {want}"
        );
    }
}

// ------------------------------------------------------------------ the vine colony

const WALL_X: u32 = 6;
const WALL_TOP: u32 = 14;
const DEPTH: u32 = 40;

/// A rock wall at `x = 6`, `y = 1..=14`, across `z = 0..40`, its west faces `y = 3..=14`
/// (12 × 40 = 480) covered by one vine rooted at the foot of slab 0, at full leaf, with a
/// purse that pays every spur for days. The cover is fixed (spreading and sisters
/// unaffordable) and drinks nothing, so the root never dries; every spur number is the
/// shipped one unless `tune` changes it.
fn colony(tune: impl Fn(&mut FloraConfig)) -> (World, Flora, VineId, Vec<Face>) {
    let mut cells = Vec::new();
    for z in 0..DEPTH {
        for y in 1..=WALL_TOP {
            cells.push(((i64::from(WALL_X), y, z), Material::Rock));
        }
    }
    let w = world(12, 18, DEPTH, &cells, noon());
    let mut c = FloraConfig::default();
    c.latticevine.spread_cost = 1e12;
    c.latticevine.sister_cost = 1e12;
    c.latticevine.transpiration_m3_per_s = 0.0;
    tune(&mut c);
    let mut flora = Flora::new(c);
    let wf = |y: u32, z: u32| Face::new(WALL_X, y, z, FaceDir::NegX);
    let id = flora
        .seed_vine(
            &w,
            VineSeed { root: Site { x: WALL_X - 1, y: 2, z: 0 }, first: wf(3, 0), reserve: 1e4, lineage: None },
        )
        .expect("fixture: the founder");
    let mut faces = vec![wf(3, 0)];
    for z in 0..DEPTH {
        for y in 3..=WALL_TOP {
            if (y, z) == (3, 0) {
                continue;
            }
            assert!(flora.cover_face(&w, id, wf(y, z), 1.0), "fixture: cover {:?}", wf(y, z));
            faces.push(wf(y, z));
        }
    }
    assert_eq!(faces.len(), 480);
    (w, flora, id, faces)
}

/// One step and what the colony's spurs did in it: the nectar flowers opened with and the
/// fruit bunches set with (read off the phase changes).
struct Watch {
    phase: Vec<SpurPhase>,
}

impl Watch {
    fn new(flora: &Flora, faces: &[Face]) -> Watch {
        Watch { phase: faces.iter().map(|&f| flora.view().cover.face(f).expect("covered").spur).collect() }
    }

    /// Step once; `(nectar opened, fruit set)` this tick.
    fn step(&mut self, flora: &mut Flora, world: &mut World, faces: &[Face]) -> (f64, f64) {
        flora.step(world);
        let (mut n, mut b) = (0.0, 0.0);
        let cover = &flora.view().cover;
        for (i, &f) in faces.iter().enumerate() {
            let c = cover.face(f).expect("covered");
            if c.spur != self.phase[i] {
                if c.spur == SpurPhase::Flower {
                    n += c.nectar;
                }
                if c.spur == SpurPhase::Fruit {
                    b += c.fruit;
                }
                self.phase[i] = c.spur;
            }
        }
        (n, b)
    }
}

/// **Asynchronous.** With `s` the config's `spur_share`, after one mean cycle to settle,
/// through a world day sampled every minute: the share of faces carrying a flower or fruit
/// stays in `[0.5 s, 1.6 s]` (its mean in `[0.8 s, 1.2 s]`), the share in flower never
/// passes `0.7 s` (a third of `s` expected; a global flush would be most of the wall),
/// the flowering and fruiting faces sit in every quarter of the wall at every sample, and
/// at least three quarters of the faces flower at some point in the day.
#[test]
fn a_big_colony_flowers_a_scattered_few_at_a_time_through_a_world_day() {
    let (mut w, mut flora, _, faces) = colony(|_| {});
    let vc = flora.config().latticevine.clone();
    let s = vc.spur_share;
    for _ in 0..ticks(vc.spur_cycle_s()) {
        flora.step(&mut w);
    }
    let n = faces.len() as f64;
    let quarter = |f: Face| usize::from(f.z >= DEPTH / 2) * 2 + usize::from(f.y > (3 + WALL_TOP) / 2);
    let mut ever: BTreeSet<Face> = BTreeSet::new();
    let (mut sum, mut samples, mut max_flower) = (0.0, 0, 0.0f64);
    for t in 0..ticks(DAY_S) {
        flora.step(&mut w);
        let cover = &flora.view().cover;
        for &f in &faces {
            if cover.face(f).expect("covered").spur == SpurPhase::Flower {
                ever.insert(f);
            }
        }
        if t % ticks(60.0) == 0 {
            let mut quarters = [0usize; 4];
            let (mut on, mut flower) = (0, 0);
            for &f in &faces {
                let c = cover.face(f).expect("covered");
                if matches!(c.spur, SpurPhase::Flower | SpurPhase::Fruit) {
                    on += 1;
                    quarters[quarter(f)] += 1;
                }
                flower += usize::from(c.spur == SpurPhase::Flower);
            }
            let share = on as f64 / n;
            assert!((0.5 * s..=1.6 * s).contains(&share), "at {t}: {share} of the wall in flower or fruit");
            assert!(quarters.iter().all(|&q| q > 0), "at {t}: every quarter carries some: {quarters:?}");
            max_flower = max_flower.max(flower as f64 / n);
            sum += share;
            samples += 1;
        }
    }
    let mean = sum / f64::from(samples);
    assert!((0.8 * s..=1.2 * s).contains(&mean), "mean share {mean}");
    assert!(max_flower <= 0.7 * s, "a spike: {max_flower} of the wall in flower at once");
    assert!(
        ever.len() as f64 >= 0.75 * n,
        "the flowers move over the wall: {} of {} faces flowered in the day",
        ever.len(),
        faces.len()
    );
}

/// **Production per area is the knob.** Over a world day after one cycle to settle, the
/// nectar the colony's flowers opened with and the fruit its bunches set with are
/// `faces · rate · 7,200 s` within 10 %, on the config's knobs and again with the share
/// doubled and the production rates doubled (the same production per face as the rates,
/// whatever the share). The fruit rate is a pollinated colony's (FLOWER-FRUIT): no
/// pollinator visits here, so the floor is set to a full bunch.
#[test]
fn the_long_run_nectar_and_fruit_per_face_are_the_production_knobs() {
    for (share, k) in [(None, 1.0), (Some(0.2), 2.0)] {
        let (mut w, mut flora, _, faces) = colony(|c| {
            if let Some(s) = share {
                c.latticevine.spur_share = s;
            }
            c.latticevine.nectar_per_face_s *= k;
            c.latticevine.fruit_per_face_s *= k;
            c.latticevine.pollination_floor = 1.0;
        });
        let vc = flora.config().latticevine.clone();
        for _ in 0..ticks(vc.spur_cycle_s()) {
            flora.step(&mut w);
        }
        let mut watch = Watch::new(&flora, &faces);
        let (mut nectar, mut fruit) = (0.0, 0.0);
        for _ in 0..ticks(DAY_S) {
            let (n, b) = watch.step(&mut flora, &mut w, &faces);
            nectar += n;
            fruit += b;
        }
        let area = faces.len() as f64 * DAY_S;
        let (want_n, want_f) = (vc.nectar_per_face_s * area, vc.fruit_per_face_s * area);
        assert!((nectar / want_n - 1.0).abs() <= 0.10, "share {share:?}: nectar {nectar} against {want_n}");
        assert!((fruit / want_f - 1.0).abs() <= 0.10, "share {share:?}: fruit {fruit} against {want_f}");
    }
}

/// **The books close over the cycles.** The colony on a short clock (every phase a tenth)
/// for two hours: every minute a sip from a face in flower and a bite from a face in
/// fruit, the rest of the fruit falling to the litter; organic matter, mineral and energy
/// close at every minute, and the vine's mineral store is drawn on by what was eaten.
#[test]
fn the_books_close_with_flowers_sipped_fruit_eaten_and_the_rest_fallen() {
    let (mut w, mut flora, id, faces) = colony(|c| {
        let v = &mut c.latticevine;
        v.bud_s /= 10.0;
        v.flower_s /= 10.0;
        v.fruit_s /= 10.0;
        v.spent_s /= 10.0;
    });
    let (mut sips, mut bites) = (0, 0);
    for minute in 0..120 {
        for _ in 0..ticks(60.0) {
            flora.step(&mut w);
        }
        let cover = &flora.view().cover;
        let flower = faces.iter().copied().find(|&f| cover.face(f).is_some_and(|c| c.nectar > 0.0));
        let fruit = faces.iter().copied().find(|&f| cover.face(f).is_some_and(|c| c.fruit > 0.0));
        if let Some(f) = flower {
            let t = flora.take_nectar(f, 1e-4).expect("a sip");
            assert!(t.organic > 0.0 && t.mineral > 0.0, "nectar carries mineral: {t:?}");
            sips += 1;
        }
        if let Some(f) = fruit {
            assert!(flora.take_fruit(f, 1e9).is_some(), "a bite");
            bites += 1;
        }
        closes(&flora, &format!("minute {minute}"));
    }
    assert!(sips > 100 && bites > 100, "sips {sips}, bites {bites}");
    assert!(flora.view().cover.vine(id).is_some(), "the vine lives");
    let litter: f64 = flora.view().ground.iter().map(|g| g.litter).sum();
    assert!(litter > 0.0, "uneaten fruit fell");
}

// ------------------------------------------------------------------ bloomcrown

/// **A bloomcrown is always in flower.** A full-grown bloomcrown on the shipped numbers, a
/// 20-minute day clock, no consumer: once its nectar first fills, it never touches zero
/// through two whole days and nights. And at midnight, drained to nothing, it is making
/// nectar again within ten seconds.
#[test]
fn a_bloomcrown_holds_nectar_through_day_and_night_and_refills_at_night() {
    let day = DayClock { mode: DayMode::Cycle, length_min: 20.0, start_hour: 12.0, ..DayClock::default() };
    let mut w = world(12, 12, 6, &[], day);
    let mut flora = Flora::new(FloraConfig::default());
    let wood = flora.config().bloomcrown.wood_max;
    assert!(flora.apply(&w, Command::Seed { x: 5, z: 3, species: Species::Bloomcrown, wood }));
    let site = Site { x: 5, y: 2, z: 3 };
    let nectar = |f: &Flora| f.view().stand_at(site).expect("the bloomcrown").nectar;
    let mut t = 0;
    while nectar(&flora) <= 0.0 {
        w.step_with(1);
        flora.step(&mut w);
        t += 1;
        assert!(t < ticks(600.0), "the first nectar comes within ten minutes");
    }
    let (mut nights, mut was_dark, mut drained) = (0, false, false);
    for t in 0..ticks(2.0 * 20.0 * 60.0) {
        w.step_with(1);
        flora.step(&mut w);
        let dark = w.life_daylight() == 0.0;
        if dark && !was_dark {
            nights += 1;
        }
        was_dark = dark;
        if !drained && dark && nights == 1 {
            assert!(flora.take_stand_nectar(site, 1e9).is_some(), "fixture: drained at night");
            drained = true;
            let mut k = 0;
            while nectar(&flora) <= 0.0 {
                w.step_with(1);
                flora.step(&mut w);
                k += 1;
                assert!(k < ticks(10.0), "a drained bloomcrown makes nectar again at night");
            }
            continue;
        }
        assert!(nectar(&flora) > 0.0, "tick {t} ({}): the flower is never empty", if dark { "night" } else { "day" });
    }
    assert!(nights >= 2, "two nights passed: {nights}");
    closes(&flora, "after two days");
}

// ------------------------------------------------------------------ lanternberry

/// **A grove of lanternberries fruits out of step.** Sixteen full-grown bushes, four
/// voxels apart, held at noon, nobody eating, the shipped fruit numbers: after two hours to
/// settle, sampled every minute for four hours, the share hanging fruit is never all and
/// never none, and its mean is between 0.5 and 0.85 (sparser than always). A bush eaten
/// out rests (FLOWER-FRUIT, Wrysk 2026-10-09): at its next plant tick its crop is over and
/// a rest is drawn, with no fruit.
#[test]
fn a_lanternberry_grove_fruits_out_of_step_and_an_eaten_bush_rests() {
    let mut w = world(20, 12, 20, &[], noon());
    // The plain is never stepped (no rain), so the bushes drink nothing and it never dries.
    let mut c = FloraConfig::default();
    c.lanternberry.transpiration_m3_per_s = 0.0;
    let mut flora = Flora::new(c);
    let sc = flora.config().lanternberry.clone();
    let mut sites = Vec::new();
    for i in 0..4 {
        for j in 0..4 {
            let (x, z) = (2 + 4 * i, 2 + 4 * j);
            assert!(flora.apply(&w, Command::Seed { x, z, species: Species::Lanternberry, wood: sc.wood_max }));
            sites.push(Site { x: x as u32, y: 2, z: z as u32 });
        }
    }
    let fruiting = |f: &Flora| sites.iter().filter(|&&s| f.view().stand_at(s).is_some_and(|st| st.fruit > 0.0)).count();
    for _ in 0..ticks(7200.0) {
        flora.step(&mut w);
    }
    let (mut sum, mut n, mut some_bare, mut some_hung) = (0.0, 0, 0, 0);
    for t in 0..ticks(4.0 * 3600.0) {
        flora.step(&mut w);
        if t % ticks(60.0) == 0 {
            let k = fruiting(&flora);
            assert!(sites.iter().all(|&s| flora.view().stand_at(s).is_some()), "the grove lives");
            assert!(k > 0, "minute {}: no bush hangs fruit", t / ticks(60.0));
            some_bare += usize::from(k < sites.len());
            some_hung += usize::from(k > 0);
            sum += k as f64 / sites.len() as f64;
            n += 1;
        }
    }
    let mean = sum / f64::from(n);
    assert!((0.5..=0.85).contains(&mean), "mean share hanging fruit {mean}");
    assert!(some_bare as f64 >= 0.9 * f64::from(n), "some bush stands bare almost always: {some_bare} of {n}");
    assert_eq!(some_hung, n as usize);

    // Eaten out, once its set is made: it rests, bare.
    let made = |st: &cubarium_voxel_flora::Stand| st.fruit > 0.0 && st.crop.to_make == 0.0;
    let s = sites.iter().copied().find(|&s| flora.view().stand_at(s).is_some_and(made)).expect("one hangs");
    assert!(flora.take_stand_fruit(s, 1e9).is_some());
    for _ in 0..ticks(60.0) {
        flora.step(&mut w);
        let st = flora.view().stand_at(s).expect("alive");
        assert!(st.fruit == 0.0 && st.fruit_rest_s > 0.0, "it rests, bare: {st:?} (mean rest {} s)", sc.fruit_rest_s);
    }
    closes_within(&flora, "the grove", 1e-8);
}

// ------------------------------------------------------------------ founding

/// **A founder takes the big sheer wall.** A soil staircase climbing eleven one-voxel
/// risers to a peak (`x = 20..=30`) and down eleven the other side and, apart from it, a
/// rock wall six voxels high and eight deep (`x = 6`, its faces four high over the soil
/// floor). One founder. The staircase is
/// the taller wall counted up its risers, which was the old rank; the sheer wall is the
/// bigger lit open face, and the founder roots at its foot.
#[test]
fn a_founder_takes_the_big_sheer_wall_over_a_taller_staircase() {
    let mut cells = Vec::new();
    for z in 0..8 {
        for y in 1..=6 {
            cells.push(((6, y, z), Material::Rock));
        }
        // Up eleven risers to a one-voxel peak at x = 30, and down eleven the other side:
        // nowhere a face more than one voxel high.
        for x in 20..=40i64 {
            let top = 13 - (x - 30).unsigned_abs() as u32;
            for y in 1..=top {
                cells.push(((x, y, z), Material::Soil));
            }
        }
    }
    let w = world(48, 20, 8, &cells, noon());
    let mut c = FloraConfig::default();
    c.latticevine.founders = 1;
    c.latticevine.founders_per_m2 = 0.0;
    let mut flora = Flora::new(c);
    assert_eq!(flora.seed_vine_founders(&w), 1);
    let v = flora.view().cover.vines()[0];
    assert_eq!(v.root_face.x, 6, "the founder roots at the sheer wall, not the staircase: {v:?}");
    assert_eq!(v.root.y, 2, "at its foot: {v:?}");
}



